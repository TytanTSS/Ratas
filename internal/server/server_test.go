package server

import (
	"net"
	"strings"
	"testing"
	"time"

	"ratas/internal/content"
	"ratas/internal/game"
	"ratas/internal/proto"
)

type testClient struct {
	t    *testing.T
	conn *proto.Conn
	msgs chan *proto.ServerMsg
	id   uint32
	snap *proto.Snapshot
	logs []string
}

func newTestClient(t *testing.T, c *proto.Conn) *testClient {
	tc := &testClient{t: t, conn: c, msgs: make(chan *proto.ServerMsg, 1024)}
	go func() {
		for {
			m, err := c.RecvServer()
			if err != nil {
				close(tc.msgs)
				return
			}
			tc.msgs <- m
		}
	}()
	return tc
}

// until reads messages until cond is true.
func (c *testClient) until(what string, cond func(m *proto.ServerMsg) bool) *proto.ServerMsg {
	c.t.Helper()
	timeout := time.After(5 * time.Second)
	for {
		select {
		case m, ok := <-c.msgs:
			if !ok {
				c.t.Fatalf("connection closed while waiting for %s", what)
			}
			if m.Snap != nil {
				c.snap = m.Snap
			}
			for _, l := range m.Logs {
				c.logs = append(c.logs, l.Text)
			}
			if m.Welcome != nil && m.Welcome.YouID != 0 {
				c.id = m.Welcome.YouID
			}
			if cond(m) {
				return m
			}
		case <-timeout:
			c.t.Fatalf("timeout waiting for %s", what)
		}
	}
}

// drain processes every queued message so c.snap is the latest state.
func (c *testClient) drain() {
	for {
		select {
		case m, ok := <-c.msgs:
			if !ok {
				return
			}
			if m.Snap != nil {
				c.snap = m.Snap
			}
			for _, l := range m.Logs {
				c.logs = append(c.logs, l.Text)
			}
		default:
			return
		}
	}
}

func (c *testClient) send(m proto.ClientMsg) {
	if err := c.conn.Send(m); err != nil {
		c.t.Fatal(err)
	}
}

func TestMultiplayer(t *testing.T) {
	db, _, err := content.LoadDefault("")
	if err != nil {
		t.Fatal(err)
	}
	content.Use(db)
	g := game.New(3, nil)
	srv := New(g)
	go srv.Run()
	defer srv.Stop()
	if err := srv.Listen("127.0.0.1:0"); err != nil {
		t.Fatal(err)
	}

	host := newTestClient(t, srv.ConnectLocal())
	host.send(proto.ClientMsg{Hello: &proto.Hello{Name: "Хозяин", Class: "warrior", Version: proto.Version}})
	host.until("host welcome", func(m *proto.ServerMsg) bool { return m.Welcome != nil && m.Level != nil })

	raw, err := net.Dial("tcp", srv.Listening())
	if err != nil {
		t.Fatal(err)
	}
	guest := newTestClient(t, proto.NewConn(raw))
	guest.send(proto.ClientMsg{Hello: &proto.Hello{Name: "Гость", Version: proto.Version}})
	w := guest.until("class prompt", func(m *proto.ServerMsg) bool { return m.Welcome != nil })
	if !w.Welcome.NeedClass || len(w.Welcome.Content) == 0 {
		t.Fatalf("expected class prompt with content, got %+v", w.Welcome)
	}
	guest.send(proto.ClientMsg{Cmd: &proto.Command{Kind: "choose_class", Key: "ranger"}})
	guest.until("guest joined", func(m *proto.ServerMsg) bool { return m.Welcome != nil && m.Level != nil && m.Sheet != nil })
	guest.until("snapshot with both players", func(m *proto.ServerMsg) bool {
		return m.Snap != nil && len(m.Snap.Online) == 2
	})
	sawHost := false
	for _, e := range guest.snap.Entities {
		if e.ID == host.id {
			sawHost = true
		}
	}
	if !sawHost {
		t.Fatal("guest does not see the host player")
	}

	// the guest walks: some direction must be free
	start := [2]int{guest.snap.Self.X, guest.snap.Self.Y}
	moved := false
	for dir := uint8(1); dir <= 4 && !moved; dir++ {
		for i := 0; i < 3; i++ {
			guest.send(proto.ClientMsg{Input: &proto.Input{Move: dir}})
			time.Sleep(120 * time.Millisecond)
		}
		time.Sleep(200 * time.Millisecond)
		guest.drain()
		moved = [2]int{guest.snap.Self.X, guest.snap.Self.Y} != start
	}
	if !moved {
		t.Fatal("guest could not move")
	}

	guest.send(proto.ClientMsg{Cmd: &proto.Command{Kind: "chat", Text: "привет!"}})
	host.until("chat line", func(m *proto.ServerMsg) bool {
		for _, l := range m.Logs {
			if strings.Contains(l.Text, "[Гость] привет!") {
				return true
			}
		}
		return false
	})

	// a second connection with the same name is rejected
	raw2, _ := net.Dial("tcp", srv.Listening())
	dup := newTestClient(t, proto.NewConn(raw2))
	dup.send(proto.ClientMsg{Hello: &proto.Hello{Name: "Гость", Class: "mage", Version: proto.Version}})
	dup.until("kick", func(m *proto.ServerMsg) bool { return m.Kick != "" })

	raw.Close()
	host.until("leave notice", func(m *proto.ServerMsg) bool {
		for _, l := range m.Logs {
			if strings.Contains(l.Text, "Гость покидает мир") {
				return true
			}
		}
		return false
	})
	var offline bool
	srv.Call(func(g *game.Game) { _, offline = g.Offline["Гость"] })
	if !offline {
		t.Fatal("guest character should be kept offline for rejoining")
	}
}

// Only the host of a world started with -admin may use admin commands.
func TestAdminRights(t *testing.T) {
	db, _, err := content.LoadDefault("")
	if err != nil {
		t.Fatal(err)
	}
	content.Use(db)
	g := game.New(5, nil)
	srv := New(g)
	srv.AdminHost = true
	go srv.Run()
	defer srv.Stop()
	if err := srv.Listen("127.0.0.1:0"); err != nil {
		t.Fatal(err)
	}
	host := newTestClient(t, srv.ConnectLocal())
	host.send(proto.ClientMsg{Hello: &proto.Hello{Name: "Хозяин", Class: "warrior", Version: proto.Version}})
	w := host.until("host welcome", func(m *proto.ServerMsg) bool { return m.Welcome != nil && m.Level != nil })
	if !w.Welcome.Admin {
		t.Fatal("the host is not an admin")
	}
	raw, err := net.Dial("tcp", srv.Listening())
	if err != nil {
		t.Fatal(err)
	}
	guest := newTestClient(t, proto.NewConn(raw))
	guest.send(proto.ClientMsg{Hello: &proto.Hello{Name: "Гость", Class: "rogue", Version: proto.Version}})
	w = guest.until("guest welcome", func(m *proto.ServerMsg) bool { return m.Welcome != nil && m.Level != nil })
	if w.Welcome.Admin {
		t.Fatal("a guest is an admin")
	}

	guest.send(proto.ClientMsg{Cmd: &proto.Command{Kind: "chat", Text: "/gold 500"}})
	guest.until("refusal", func(m *proto.ServerMsg) bool {
		for _, l := range m.Logs {
			if strings.Contains(l.Text, "-admin") {
				return true
			}
		}
		return false
	})
	host.send(proto.ClientMsg{Cmd: &proto.Command{Kind: "chat", Text: "/gold 500"}})
	host.until("gold", func(m *proto.ServerMsg) bool { return m.Snap != nil && m.Snap.Self.Gold >= 500 })
	guest.drain()
	if guest.snap.Self.Gold >= 500 {
		t.Fatal("the guest got the gold")
	}
	host.send(proto.ClientMsg{Cmd: &proto.Command{Kind: "admin", Text: "/reveal"}})
	host.until("level resent", func(m *proto.ServerMsg) bool { return m.Level != nil })
}
