package client

import (
	"sort"
	"strings"
	"testing"
	"time"
	"unicode"

	"github.com/gdamore/tcell/v2"

	"ratas/internal/config"
	"ratas/internal/content"
	"ratas/internal/game"
	"ratas/internal/i18n"
	"ratas/internal/server"
)

// TestUIEnglish plays through the windows, a conversation and a trade in
// English: no Russian letter may stay on the screen.
func TestUIEnglish(t *testing.T) {
	t.Setenv("RATAS_HOME", t.TempDir())
	t.Cleanup(func() { i18n.SetLang(i18n.RU); LocalServerHook = nil })
	db, _, err := content.LoadDefault("")
	if err != nil {
		t.Fatal(err)
	}
	content.Use(db)
	srvCh := make(chan *server.Server, 1)
	LocalServerHook = func(s *server.Server) { srvCh <- s }
	scr := newCapScreen()
	s := &sim{t: t, scr: scr}
	cfg := config.Default()
	cfg.AIEnabled, cfg.Language = false, "en"
	done := make(chan error, 1)
	go func() {
		done <- RunOn(scr, cfg, nil, StartOptions{New: true, Seed: 7, Name: "Ярослав", Class: "priest"})
	}()
	time.Sleep(200 * time.Millisecond)
	scr.SetSize(130, 44)
	srv := <-srvCh

	russian := map[string]bool{}
	check := func(name string) {
		for _, line := range strings.Split(s.text(), "\n") {
			for _, r := range line {
				if unicode.Is(unicode.Cyrillic, r) {
					russian[strings.TrimSpace(line)] = true
					break
				}
			}
		}
		s.dump(name)
	}
	s.waitFor("Health", 10*time.Second)
	time.Sleep(400 * time.Millisecond)
	check("en game")
	for _, sc := range []struct {
		key  rune
		want string
	}{{'i', "Backpack"}, {'k', "Skill points"}, {'c', "Free points"}, {'j', "Quest journal"}, {'m', "Map"}, {'?', "Help"}, {'g', "Party"}} {
		s.rune(sc.key)
		s.waitFor(sc.want, 3*time.Second)
		check("en screen " + string(sc.key))
		if sc.key == 'k' {
			for i := 0; i < 3; i++ { // the class, common and new class tabs
				s.key(tcell.KeyRight)
				time.Sleep(150 * time.Millisecond)
				check("en skills tab")
			}
		}
		s.key(tcell.KeyEscape)
	}

	// talk to the elder: greeting, rumours, a quest
	talk := func(role string) {
		srv.Call(func(gm *game.Game) {
			p := gm.Online["Ярослав"]
			for _, e := range gm.Entities {
				if e.NPC != nil && e.NPC.Role == role && e.Level == p.Level {
					gm.MoveForTest(p, e)
					gm.OpenDialogueForTest(p, e)
					return
				}
			}
		})
	}
	talk("elder")
	s.waitFor("Farewell", 5*time.Second)
	check("en dialogue")
	for i := 0; i < 4; i++ {
		s.key(tcell.KeyEnter)
		time.Sleep(300 * time.Millisecond)
		check("en dialogue answer")
		s.key(tcell.KeyDown)
	}
	s.key(tcell.KeyEscape)
	talk("merchant")
	s.waitFor("Show me your goods", 5*time.Second)
	for !strings.Contains(s.text(), "Goods (buy)") {
		s.key(tcell.KeyDown)
		s.key(tcell.KeyEnter)
		time.Sleep(200 * time.Millisecond)
		if strings.Contains(s.text(), "Farewell") && !strings.Contains(s.text(), "Show me your goods") {
			break
		}
	}
	check("en trade")
	s.key(tcell.KeyEscape)
	s.key(tcell.KeyEscape)

	s.key(tcell.KeyEscape)
	s.waitFor("Continue", 3*time.Second)
	check("en pause")
	s.key(tcell.KeyUp)
	s.key(tcell.KeyEnter)
	s.waitFor("Main menu", 10*time.Second)
	check("en main menu")
	s.key(tcell.KeyEscape)
	s.waitFor("Quit the game?", 3*time.Second)
	s.key(tcell.KeyDown)
	s.key(tcell.KeyEnter)
	select {
	case err := <-done:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("app did not exit")
	}

	var lines []string
	for l := range russian {
		lines = append(lines, l)
	}
	sort.Strings(lines)
	for _, l := range lines {
		t.Errorf("Russian text on an English screen: %s", l)
	}
	misses := i18n.Misses(i18n.EN)
	sort.Strings(misses)
	t.Logf("transliterated or kept as is (%d): %s", len(misses), strings.Join(misses, " | "))
}
