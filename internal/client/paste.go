package client

import (
	"strings"
	"unicode"

	"github.com/atotto/clipboard"
	"github.com/gdamore/tcell/v2"
)

// readClipboard returns the system clipboard text (tests replace it). On
// Linux it needs xclip, xsel or wl-clipboard; without them pasting through
// the terminal (Ctrl+Shift+V) still works.
var readClipboard = func() string {
	s, err := clipboard.ReadAll()
	if err != nil {
		return ""
	}
	return s
}

// isPasteKey: Ctrl+V, and Shift+Insert as in many terminals.
func isPasteKey(ev *tcell.EventKey) bool {
	return ev.Key() == tcell.KeyCtrlV || (ev.Key() == tcell.KeyInsert && ev.Modifiers()&tcell.ModShift != 0)
}

// paste inserts text into the field: one line, no control characters, up
// to the field's length.
func (t *textInput) paste(s string) {
	s = strings.NewReplacer("\r\n", " ", "\n", " ", "\r", " ", "\t", " ").Replace(s)
	for _, r := range s {
		if !unicode.IsPrint(r) {
			continue
		}
		if t.maxLen > 0 && len(t.value) >= t.maxLen {
			return
		}
		t.value = append(t.value, r)
	}
}

// pasteFilter drops line breaks inside a bracketed paste from the terminal so
// that pasting several lines does not submit the field.
type pasteFilter struct{ pasting bool }

func (f *pasteFilter) keep(ev tcell.Event) bool {
	switch e := ev.(type) {
	case *tcell.EventPaste:
		f.pasting = e.Start()
		return false
	case *tcell.EventKey:
		if f.pasting && (e.Key() == tcell.KeyEnter || e.Key() == tcell.KeyTab || e.Key() == tcell.KeyLF) {
			return false
		}
	}
	return true
}
