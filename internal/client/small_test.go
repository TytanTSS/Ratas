package client

import (
	"os"
	"strings"
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

func TestPaste(t *testing.T) {
	old := readClipboard
	defer func() { readClipboard = old }()
	readClipboard = func() string { return "sk-ant-123\nвторая\tстрока\x07" }
	in := textInput{maxLen: 20}
	in.Set("ключ: ")
	if !in.handle(tcell.NewEventKey(tcell.KeyCtrlV, 0, tcell.ModCtrl)) {
		t.Fatal("Ctrl+V not handled")
	}
	if got := in.String(); got != "ключ: sk-ant-123 вто" {
		t.Fatalf("pasted %q", got)
	}
	in.Set("")
	in.handle(tcell.NewEventKey(tcell.KeyInsert, 0, tcell.ModShift))
	if in.String() == "" {
		t.Fatal("Shift+Insert did not paste")
	}

	// a bracketed paste from the terminal: line breaks do not submit
	var f pasteFilter
	evs := []tcell.Event{
		tcell.NewEventPaste(true), tcell.NewEventKey(tcell.KeyRune, 'a', 0),
		tcell.NewEventKey(tcell.KeyEnter, 0, 0), tcell.NewEventKey(tcell.KeyRune, 'b', 0),
		tcell.NewEventPaste(false), tcell.NewEventKey(tcell.KeyEnter, 0, 0),
	}
	var kept []string
	for _, ev := range evs {
		if f.keep(ev) {
			if k, ok := ev.(*tcell.EventKey); ok {
				kept = append(kept, k.Name())
			}
		}
	}
	if strings.Join(kept, ",") != "Rune[a],Rune[b],Enter" {
		t.Fatalf("kept %v", kept)
	}
}
