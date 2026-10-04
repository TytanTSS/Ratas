// Package server runs the authoritative game loop and talks to clients.
// A local player connects through an in-memory pipe, remote players over TCP;
// both use the same protocol.
package server

import (
	"fmt"
	"log"
	"net"
	"strings"
	"sync"
	"time"
	"unicode/utf8"

	"ratas/internal/content"
	"ratas/internal/game"
	"ratas/internal/i18n"
	"ratas/internal/proto"
)

type session struct {
	conn    *proto.Conn
	out     chan *proto.ServerMsg
	host    bool
	name    string
	lang    string // the player's language
	entity  *game.Entity
	level   string
	pending bool // waiting for class choice
	closed  bool
}

type event struct {
	s    *session
	msg  *proto.ClientMsg
	gone bool
	fn   func()
}

type Server struct {
	Game          *game.Game
	SavePath      string // autosave target ("" = none)
	AutosaveEvery time.Duration
	Logf          func(format string, args ...any)
	AdminHost     bool // the host's own player may use admin commands
	AdminAll      bool // every player may use admin commands (a test server)

	inbox    chan event
	sessions map[*session]bool
	quit     chan struct{}
	done     chan struct{}
	content  []byte

	mu       sync.Mutex
	ln       net.Listener
	stopOnce sync.Once
}

func New(g *game.Game) *Server {
	return &Server{
		Game:          g,
		AutosaveEvery: 3 * time.Minute,
		Logf:          func(string, ...any) {},
		inbox:         make(chan event, 1024),
		sessions:      map[*session]bool{},
		quit:          make(chan struct{}),
		done:          make(chan struct{}),
		content:       content.Get().JSON(),
	}
}

// Run is the main loop; call it in its own goroutine.
func (s *Server) Run() {
	defer close(s.done)
	ticker := time.NewTicker(time.Duration(game.TickMs) * time.Millisecond)
	defer ticker.Stop()
	autosave := time.NewTicker(s.AutosaveEvery)
	defer autosave.Stop()
	for {
		select {
		case ev := <-s.inbox:
			s.handle(ev)
		case <-ticker.C:
			s.tick()
		case <-autosave.C:
			if s.SavePath != "" && len(s.Game.Online) > 0 {
				if err := s.Game.Save(s.SavePath); err != nil {
					s.Logf("autosave failed: %v", err)
				} else {
					s.Game.LogAll("#707070", "Автосохранение.")
				}
			}
		case <-s.quit:
			for sess := range s.sessions {
				s.send(sess, &proto.ServerMsg{Kick: "Сервер остановлен."})
				s.drop(sess)
			}
			return
		}
	}
}

// Stop shuts the server down (without saving).
func (s *Server) Stop() {
	s.stopOnce.Do(func() {
		s.mu.Lock()
		if s.ln != nil {
			s.ln.Close()
		}
		s.mu.Unlock()
		close(s.quit)
	})
	<-s.done
}

// Call runs fn on the game goroutine and waits for it.
func (s *Server) Call(fn func(g *game.Game)) {
	done := make(chan struct{})
	select {
	case s.inbox <- event{fn: func() { fn(s.Game); close(done) }}:
		select {
		case <-done:
		case <-s.done:
		}
	case <-s.done:
	}
}

// Listen opens the world for network players.
func (s *Server) Listen(addr string) error {
	ln, err := net.Listen("tcp", addr)
	if err != nil {
		return err
	}
	s.mu.Lock()
	s.ln = ln
	s.mu.Unlock()
	s.Logf("listening on %s", ln.Addr())
	go func() {
		for {
			c, err := ln.Accept()
			if err != nil {
				return
			}
			if tc, ok := c.(*net.TCPConn); ok {
				tc.SetNoDelay(true)
			}
			s.Logf("connection from %s", c.RemoteAddr())
			s.attach(c, false)
		}
	}()
	return nil
}

func (s *Server) Listening() string {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.ln == nil {
		return ""
	}
	return s.ln.Addr().String()
}

// ConnectLocal returns a client connection for the host's own player.
func (s *Server) ConnectLocal() *proto.Conn {
	a, b := net.Pipe()
	s.attach(a, true)
	return proto.NewConn(b)
}

func (s *Server) attach(c net.Conn, host bool) {
	sess := &session{conn: proto.NewConn(c), out: make(chan *proto.ServerMsg, 512), host: host}
	go func() {
		for m := range sess.out {
			if err := sess.conn.Send(m); err != nil {
				sess.conn.Close()
				for range sess.out {
				}
				return
			}
		}
		sess.conn.Close()
	}()
	go func() {
		for {
			m, err := sess.conn.RecvClient()
			if err != nil {
				s.post(event{s: sess, gone: true})
				return
			}
			if !s.post(event{s: sess, msg: m}) {
				return
			}
		}
	}()
}

func (s *Server) post(ev event) bool {
	select {
	case s.inbox <- ev:
		return true
	case <-s.quit:
		return false
	}
}

func (s *Server) send(sess *session, m *proto.ServerMsg) {
	if sess.closed {
		return
	}
	onlySnap := m.Snap != nil && m.Level == nil && m.Sheet == nil && m.Dialogue == nil && len(m.Logs) == 0 && len(m.Tiles) == 0 && m.Welcome == nil
	if onlySnap && len(sess.out) > cap(sess.out)/2 {
		return // a slow client skips frames
	}
	select {
	case sess.out <- m:
	default:
		s.Logf("client %s too slow, dropping", sess.name)
		s.drop(sess)
	}
}

func (s *Server) drop(sess *session) {
	if sess.closed {
		return
	}
	sess.closed = true
	if sess.entity != nil {
		s.Game.Leave(sess.name)
		sess.entity = nil
	}
	delete(s.sessions, sess)
	close(sess.out)
	if len(s.sessions) == 0 || s.onlyHost() {
		s.Game.Paused = false
	}
}

func (s *Server) onlyHost() bool {
	for sess := range s.sessions {
		if !sess.host {
			return false
		}
	}
	return true
}

func (s *Server) handle(ev event) {
	if ev.fn != nil {
		ev.fn()
		return
	}
	sess := ev.s
	if ev.gone {
		s.Logf("%s disconnected", sess.name)
		s.drop(sess)
		return
	}
	if sess.closed {
		return
	}
	m := ev.msg
	switch {
	case m.Hello != nil:
		s.hello(sess, m.Hello)
	case sess.entity == nil:
		if m.Cmd != nil && m.Cmd.Kind == "choose_class" && sess.pending {
			s.join(sess, m.Cmd.Key)
		}
	case m.Input != nil:
		s.Game.SetInput(sess.entity, *m.Input)
	case m.Cmd != nil:
		if m.Cmd.Kind == "pause" {
			// only the host can pause, and only when playing alone
			if sess.host && len(s.sessions) == 1 {
				s.Game.Paused = m.Cmd.Index == 1
			}
			return
		}
		if c := m.Cmd; c.Kind == "admin" || (c.Kind == "chat" && strings.HasPrefix(c.Text, "/")) {
			if s.isAdmin(sess) {
				s.Game.Log(sess.entity, "#a0a0a0", "> %s", c.Text)
				s.Game.Admin(sess.entity, c.Text)
			} else {
				s.Game.Log(sess.entity, "#ff8080", "Команды доступны только в режиме администратора (запуск с флагом -admin).")
			}
			return
		}
		s.Game.Command(sess.entity, *m.Cmd)
	}
}

func (s *Server) isAdmin(sess *session) bool {
	return s.AdminAll || (sess.host && s.AdminHost)
}

func validName(n string) string {
	n = strings.TrimSpace(n)
	if utf8.RuneCountInString(n) > 16 {
		n = string([]rune(n)[:16])
	}
	return n
}

func (s *Server) hello(sess *session, h *proto.Hello) {
	if h.Version != proto.Version {
		s.send(sess, &proto.ServerMsg{Kick: fmt.Sprintf("Несовместимая версия (сервер %d, клиент %d).", proto.Version, h.Version)})
		s.drop(sess)
		return
	}
	sess.name = validName(h.Name)
	sess.lang = i18n.Normalize(h.Lang)
	if sess.name == "" {
		s.send(sess, &proto.ServerMsg{Kick: "Пустое имя."})
		s.drop(sess)
		return
	}
	s.sessions[sess] = true
	s.join(sess, h.Class)
}

func (s *Server) join(sess *session, class string) {
	e, needClass, err := s.Game.Join(sess.name, class)
	if err != nil {
		s.send(sess, &proto.ServerMsg{Kick: err.Error()})
		s.drop(sess)
		return
	}
	w := &proto.Welcome{Content: s.content, WorldName: s.Game.WorldName, Seed: s.Game.Seed, AI: s.Game.Brain.Enabled(), Host: sess.host, Admin: s.isAdmin(sess)}
	if needClass {
		sess.pending = true
		w.NeedClass = true
		s.send(sess, &proto.ServerMsg{Welcome: w})
		return
	}
	sess.pending = false
	sess.entity = e
	e.Player.Lang = sess.lang
	sess.level = e.Level
	w.YouID = e.ID
	s.Logf("%s joined", sess.name)
	s.send(sess, &proto.ServerMsg{Welcome: w, Level: s.Game.LevelData(e), Sheet: s.Game.Sheet(e)})
	e.Player.Dirty = false
}

func (s *Server) tick() {
	g := s.Game
	g.Tick()
	for sess := range s.sessions {
		e := sess.entity
		if e == nil {
			continue
		}
		m := &proto.ServerMsg{}
		if sess.level != e.Level || e.Player.Resync {
			m.Level = g.LevelData(e)
			sess.level = e.Level
			e.Player.Resync = false
		} else {
			_, m.Tiles = g.FrameFX(e.Level)
		}
		m.Snap = g.Snapshot(e)
		if e.Player.Dirty || g.TickN%40 == 0 {
			m.Sheet = g.Sheet(e)
		}
		if ob := g.TakeOutbox(e); ob != nil {
			m.Logs = ob.Logs
			m.Dialogue = ob.Dialogue
		}
		s.send(sess, m)
	}
	for _, p := range g.Online {
		p.Player.Dirty = false
	}
	g.EndFrame()
}

// LANAddresses lists non-loopback IPv4 addresses to show to the host.
func LANAddresses() []string {
	var out []string
	ifaces, _ := net.Interfaces()
	for _, i := range ifaces {
		if i.Flags&net.FlagUp == 0 || i.Flags&net.FlagLoopback != 0 {
			continue
		}
		addrs, _ := i.Addrs()
		for _, a := range addrs {
			if ipn, ok := a.(*net.IPNet); ok && ipn.IP.To4() != nil {
				out = append(out, ipn.IP.String())
			}
		}
	}
	return out
}

// StdLogger returns a Logf that writes to the standard logger.
func StdLogger() func(string, ...any) { return log.Printf }
