package gfx

import (
	"slices"
	"strings"
	"time"

	"github.com/gdamore/tcell/v2"
	"github.com/hajimehoshi/ebiten/v2"
	"github.com/hajimehoshi/ebiten/v2/inpututil"

	"ratas/internal/client"
	"ratas/internal/world"
)

// Terminals rely on the OS key repeat, which has a long initial delay and
// repeats only the key pressed last. In a window we know which keys are
// held, so movement keys repeat smoothly. Of the movement keys only the one
// pressed last repeats: holding → and pressing ↑ turns the hero at once
// (both repeating would send the two ways in turn), and releasing ↑ while →
// is still held walks on to the right.
const (
	repeatDelay = 170 * time.Millisecond
	repeatEvery = 55 * time.Millisecond
	// a released movement key the hero has not stepped by yet keeps the other
	// held one waiting that long at most (the server forgets an order sooner)
	turnHold = 300 * time.Millisecond
)

var specialKeys = []struct {
	k  ebiten.Key
	tk tcell.Key
}{
	{ebiten.KeyArrowUp, tcell.KeyUp}, {ebiten.KeyArrowDown, tcell.KeyDown},
	{ebiten.KeyArrowLeft, tcell.KeyLeft}, {ebiten.KeyArrowRight, tcell.KeyRight},
	{ebiten.KeyEnter, tcell.KeyEnter}, {ebiten.KeyNumpadEnter, tcell.KeyEnter},
	{ebiten.KeyEscape, tcell.KeyEscape}, {ebiten.KeyBackspace, tcell.KeyBackspace2},
	{ebiten.KeyTab, tcell.KeyTab}, {ebiten.KeyF1, tcell.KeyF1}, {ebiten.KeyF5, tcell.KeyF5}, {ebiten.KeyF9, tcell.KeyF9},
	{ebiten.KeyDelete, tcell.KeyDelete}, {ebiten.KeyHome, tcell.KeyHome}, {ebiten.KeyEnd, tcell.KeyEnd},
}

var repeating = map[ebiten.Key]bool{
	ebiten.KeyArrowUp: true, ebiten.KeyArrowDown: true, ebiten.KeyArrowLeft: true, ebiten.KeyArrowRight: true,
	ebiten.KeyBackspace: true,
}

// letter keys used for movement; they repeat while held, whatever the layout
var moveLetters = []struct {
	k       ebiten.Key
	letters string
}{{ebiten.KeyW, "wц"}, {ebiten.KeyA, "aф"}, {ebiten.KeyS, "sы"}, {ebiten.KeyD, "dв"}}

// moveDirs: the way each movement key walks
var moveDirs = map[ebiten.Key]world.Dir{
	ebiten.KeyArrowUp: world.DirUp, ebiten.KeyArrowDown: world.DirDown,
	ebiten.KeyArrowLeft: world.DirLeft, ebiten.KeyArrowRight: world.DirRight,
	ebiten.KeyW: world.DirUp, ebiten.KeyS: world.DirDown,
	ebiten.KeyA: world.DirLeft, ebiten.KeyD: world.DirRight,
}

type hold struct {
	key   tcell.Key
	r     rune
	since time.Time
	next  time.Time
	// movement keys: the way, and the hero (cell and facing) at the press
	dir    world.Dir
	hx, hy int
	facing world.Dir
	quiet  bool // does not repeat any more (a key taken over in a menu)
}

type inputState struct {
	held map[ebiten.Key]*hold
	walk []ebiten.Key // held movement keys, the one pressed last at the end
	// a movement key released before the hero stepped its way: the key held
	// before it waits for that step, or the tap would be lost
	wait *hold
}

func newInput() *inputState { return &inputState{held: map[ebiten.Key]*hold{}} }

// keyboard is what the window knows about the keys in this frame.
type keyboard interface {
	pressed(k ebiten.Key) bool
	justPressed(k ebiten.Key) bool
	chars() []rune // characters typed, with the OS key repeat
}

type ebitenKeys struct{}

func (ebitenKeys) pressed(k ebiten.Key) bool     { return ebiten.IsKeyPressed(k) }
func (ebitenKeys) justPressed(k ebiten.Key) bool { return inpututil.IsKeyJustPressed(k) }
func (ebitenKeys) chars() []rune                 { return ebiten.AppendInputChars(nil) }

// update turns this frame's keys into terminal key events; sc is the last
// scene (nil in menus).
func (in *inputState) update(scr tcell.SimulationScreen, kb keyboard, now time.Time, sc *client.Scene) {
	chars := kb.chars()
	mod := tcell.ModNone
	if kb.pressed(ebiten.KeyShift) {
		mod |= tcell.ModShift
	}

	// paste: Ctrl+V (Cmd+V on macOS), Shift+Insert
	ctrl := kb.pressed(ebiten.KeyControl) || kb.pressed(ebiten.KeyMeta)
	if (ctrl && kb.justPressed(ebiten.KeyV)) || (mod&tcell.ModShift != 0 && kb.justPressed(ebiten.KeyInsert)) {
		scr.InjectKey(tcell.KeyCtrlV, 0, tcell.ModCtrl)
		chars = nil // the OS may also type a "v"
	}
	for _, sk := range specialKeys {
		if !kb.justPressed(sk.k) {
			continue
		}
		tk := sk.tk
		if tk == tcell.KeyTab && mod&tcell.ModShift != 0 {
			tk = tcell.KeyBacktab
		}
		scr.InjectKey(tk, 0, mod)
		if repeating[sk.k] {
			in.press(sk.k, &hold{key: tk}, now, sc)
		}
	}
	for _, ml := range moveLetters {
		if !kb.justPressed(ml.k) {
			continue
		}
		for _, r := range chars {
			if strings.ContainsRune(ml.letters, toLower(r)) {
				in.press(ml.k, &hold{key: tcell.KeyRune, r: r}, now, sc)
				break
			}
		}
	}
	for k, h := range in.held {
		if !kb.pressed(k) {
			in.release(k, h, now, sc)
		}
	}
	top := in.top()
	for k, h := range in.held {
		if h.quiet || h.dir != world.DirNone && k != top || now.Before(h.next) {
			continue
		}
		if k == top && in.waiting(now, sc) {
			continue
		}
		scr.InjectKey(h.key, h.r, tcell.ModNone)
		h.next = now.Add(repeatEvery)
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

// press starts holding a key that was just pressed (and sent).
func (in *inputState) press(k ebiten.Key, h *hold, now time.Time, sc *client.Scene) {
	h.since, h.next = now, now.Add(repeatDelay)
	if h.dir = moveDirs[k]; h.dir != world.DirNone {
		if len(in.walk) > 0 {
			// already walking: the new way goes on without a pause
			h.next = now.Add(repeatEvery)
		}
		if sc != nil {
			h.hx, h.hy, h.facing = sc.Self.X, sc.Self.Y, world.Dir(sc.Self.Facing)
		}
		in.walk = append(slices.DeleteFunc(in.walk, func(o ebiten.Key) bool { return o == k }), k)
		in.wait = nil
	}
	in.held[k] = h
}

// release forgets a key; if it was the movement key pressed last, the one
// held before it walks on.
func (in *inputState) release(k ebiten.Key, h *hold, now time.Time, sc *client.Scene) {
	delete(in.held, k)
	if h.dir == world.DirNone {
		return
	}
	last := in.top() == k
	in.walk = slices.DeleteFunc(in.walk, func(o ebiten.Key) bool { return o == k })
	if !last || len(in.walk) == 0 {
		return
	}
	prev := in.held[in.top()]
	if sc == nil || !sc.Walking {
		// in menus and text fields a key repeats only until another is pressed
		prev.quiet = true
		return
	}
	prev.next, prev.quiet = now, false
	if !stepped(h, sc) && now.Sub(h.since) < turnHold {
		in.wait = h
	}
}

// top is the movement key pressed last, if one is held.
func (in *inputState) top() ebiten.Key {
	if len(in.walk) == 0 {
		return -1
	}
	return in.walk[len(in.walk)-1]
}

// waiting: a released tap has not been stepped yet.
func (in *inputState) waiting(now time.Time, sc *client.Scene) bool {
	if in.wait == nil {
		return false
	}
	if stepped(in.wait, sc) || now.Sub(in.wait.since) >= turnHold {
		in.wait = nil
		return false
	}
	return true
}

// stepped tells whether the hero has acted on a movement key: it faces that
// way and has moved or turned since the press (a wall or a foe in the way
// turns the hero without moving it).
func stepped(h *hold, sc *client.Scene) bool {
	if sc == nil {
		return true
	}
	s := sc.Self
	return world.Dir(s.Facing) == h.dir && (s.X != h.hx || s.Y != h.hy || h.facing != h.dir)
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
