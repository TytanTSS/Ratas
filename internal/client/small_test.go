package client

import (
	"os"
	"testing"
	"time"

	"github.com/gdamore/tcell/v2"

	"ratas/internal/config"
	"ratas/internal/content"
)

func TestSmallTerminal(t *testing.T) {
	if os.Getenv("RATAS_SIM_DUMP") == "" {
		t.Skip("visual check only")
	}
	t.Setenv("RATAS_HOME", t.TempDir())
	db, _, _ := content.LoadDefault("")
	content.Use(db)
	scr := newCapScreen()
	s := &sim{t: t, scr: scr}
	cfg := config.Default()
	cfg.AIEnabled = false
	go RunOn(scr, cfg, nil, StartOptions{New: true, Seed: 5, Name: "Мал", Class: "mage"})
	time.Sleep(200 * time.Millisecond)
	scr.SetSize(80, 24)
	s.waitFor("Здоровье", 10*time.Second)
	time.Sleep(500 * time.Millisecond)
	s.dump("small game")
	s.rune('k')
	time.Sleep(200 * time.Millisecond)
	s.dump("small skills")
	s.key(tcell.KeyEscape)
	s.rune('m')
	time.Sleep(200 * time.Millisecond)
	s.dump("small map")
}
