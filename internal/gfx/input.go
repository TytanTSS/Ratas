package gfx

import (
	"strings"
	"time"

	"github.com/gdamore/tcell/v2"
	"github.com/hajimehoshi/ebiten/v2"
	"github.com/hajimehoshi/ebiten/v2/inpututil"
)

// Terminals rely on the OS key repeat, which has a long initial delay. In a
// window we know which keys are held, so movement keys repeat smoothly.
const (
	repeatDelay = 170 * time.Millisecond
	repeatEvery = 55 * time.Millisecond
)

var specialKeys = map[ebiten.Key]tcell.Key{
	ebiten.KeyArrowUp: tcell.KeyUp, ebiten.KeyArrowDown: tcell.KeyDown,
	ebiten.KeyArrowLeft: tcell.KeyLeft, ebiten.KeyArrowRight: tcell.KeyRight,
	ebiten.KeyEnter: tcell.KeyEnter, ebiten.KeyNumpadEnter: tcell.KeyEnter,
	ebiten.KeyEscape: tcell.KeyEscape, ebiten.KeyBackspace: tcell.KeyBackspace2,
	ebiten.KeyTab: tcell.KeyTab, ebiten.KeyF1: tcell.KeyF1, ebiten.KeyF5: tcell.KeyF5, ebiten.KeyF9: tcell.KeyF9,
	ebiten.KeyDelete: tcell.KeyDelete, ebiten.KeyHome: tcell.KeyHome, ebiten.KeyEnd: tcell.KeyEnd,
}

var repeating = map[ebiten.Key]bool{
	ebiten.KeyArrowUp: true, ebiten.KeyArrowDown: true, ebiten.KeyArrowLeft: true, ebiten.KeyArrowRight: true,
	ebiten.KeyBackspace: true,
}

// letter keys used for movement; they repeat while held, whatever the layout
var moveLetters = map[ebiten.Key]string{
	ebiten.KeyW: "wц", ebiten.KeyA: "aф", ebiten.KeyS: "sы", ebiten.KeyD: "dв",
}

type hold struct {
	key   tcell.Key
	r     rune
	since time.Time
	next  time.Time
}

type inputState struct {
	held map[ebiten.Key]*hold
}

func newInput() *inputState { return &inputState{held: map[ebiten.Key]*hold{}} }

func (in *inputState) update(scr tcell.SimulationScreen) {
	now := time.Now()
	chars := ebiten.AppendInputChars(nil)
	mod := tcell.ModNone
	if ebiten.IsKeyPressed(ebiten.KeyShift) {
		mod |= tcell.ModShift
	}

	// paste: Ctrl+V (Cmd+V on macOS), Shift+Insert
	ctrl := ebiten.IsKeyPressed(ebiten.KeyControl) || ebiten.IsKeyPressed(ebiten.KeyMeta)
	if (ctrl && inpututil.IsKeyJustPressed(ebiten.KeyV)) || (mod&tcell.ModShift != 0 && inpututil.IsKeyJustPressed(ebiten.KeyInsert)) {
		scr.InjectKey(tcell.KeyCtrlV, 0, tcell.ModCtrl)
		chars = nil // the OS may also type a "v"
	}
	for k, tk := range specialKeys {
		if !inpututil.IsKeyJustPressed(k) {
			continue
		}
		if tk == tcell.KeyTab && mod&tcell.ModShift != 0 {
			tk = tcell.KeyBacktab
		}
		scr.InjectKey(tk, 0, mod)
		if repeating[k] {
			in.held[k] = &hold{key: tk, since: now, next: now.Add(repeatDelay)}
		}
	}
	for k, letters := range moveLetters {
		if !inpututil.IsKeyJustPressed(k) {
			continue
		}
		for _, r := range chars {
			if strings.ContainsRune(letters, toLower(r)) {
				in.held[k] = &hold{key: tcell.KeyRune, r: r, since: now, next: now.Add(repeatDelay)}
				break
			}
		}
	}
	for k, h := range in.held {
		if !ebiten.IsKeyPressed(k) {
			delete(in.held, k)
			continue
		}
		if now.After(h.next) {
			scr.InjectKey(h.key, h.r, tcell.ModNone)
			h.next = now.Add(repeatEvery)
		}
	}
	for _, r := range chars {
		if r < 32 || r == 127 {
			continue
		}
		if in.isOSRepeat(r, now) {
			continue
		}
		scr.InjectKey(tcell.KeyRune, r, tcell.ModNone)
	}
}

// isOSRepeat drops characters the OS generates for a key we already repeat.
func (in *inputState) isOSRepeat(r rune, now time.Time) bool {
	for _, h := range in.held {
		if h.key == tcell.KeyRune && h.r == r && now.Sub(h.since) > 100*time.Millisecond {
			return true
		}
	}
	return false
}

func toLower(r rune) rune {
	return []rune(strings.ToLower(string(r)))[0]
}
