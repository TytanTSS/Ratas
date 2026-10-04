package client

import (
	"os"
	"strings"
	"testing"
	"time"

	"github.com/gdamore/tcell/v2"

	"ratas/internal/config"
	"ratas/internal/content"
	"ratas/internal/game"
	"ratas/internal/proto"
	"ratas/internal/server"
)

func TestDialogueVisual(t *testing.T) {
	if os.Getenv("RATAS_SIM_DUMP") == "" {
		t.Skip("visual check only")
	}
	t.Setenv("RATAS_HOME", t.TempDir())
	db, _, _ := content.LoadDefault("")
	content.Use(db)
	scr := newCapScreen()
	scr.Init()
	scr.SetSize(110, 34)
	s := &sim{t: t, scr: scr}
	a := &App{scr: scr, cv: &canvas{s: scr}, cfg: config.Default(), events: make(chan tcell.Event, 64)}
	go func() {
		for {
			ev := scr.PollEvent()
			if ev == nil {
				return
			}
			a.events <- ev
		}
	}()
	g := game.New(11, nil)
	srv := server.New(g)
	go srv.Run()
	defer srv.Stop()
	p := newPlay(a, srv.ConnectLocal(), srv, "Тест", "ranger")
	go p.run()
	s.waitFor("Здоровье", 5*time.Second)
	talk := func(role string) {
		srv.Call(func(g *game.Game) {
			me := g.Online["Тест"]
			for _, e := range g.Entities {
				if e.NPC != nil && e.NPC.Role == role {
					g.Command(me, protoCmd("talk_close"))
					g.MoveForTest(me, e)
					g.OpenDialogueForTest(me, e)
					return
				}
			}
		})
	}
	talk("elder")
	s.waitFor("Есть работа?", 3*time.Second)
	s.key(tcell.KeyDown)
	s.key(tcell.KeyEnter)
	time.Sleep(300 * time.Millisecond)
	s.dump("dialog elder quest")
	s.key(tcell.KeyEscape)
	talk("merchant")
	txt := s.waitFor("Покажи товары", 3*time.Second)
	// pick the option by its number: the list differs between NPCs
	if i := strings.Index(txt, ". Покажи товары"); i > 0 {
		s.rune(rune(txt[i-1]))
	}
	s.waitFor("Товары (купить)", 3*time.Second)
	s.key(tcell.KeyEnter) // buy the first item
	time.Sleep(300 * time.Millisecond)
	s.dump("dialog trade")
}

func protoCmd(kind string) proto.Command { return proto.Command{Kind: kind} }
