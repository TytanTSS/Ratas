package gfx

import (
	"math"
	"testing"
	"time"

	"github.com/hajimehoshi/ebiten/v2"
)

// fakeKeys is a keyboard driven by the test.
type fakeKeys struct {
	down, just map[ebiten.Key]bool
	typed      []rune
}

func newFakeKeys() *fakeKeys {
	return &fakeKeys{down: map[ebiten.Key]bool{}, just: map[ebiten.Key]bool{}}
}

func (f *fakeKeys) pressed(k ebiten.Key) bool     { return f.down[k] }
func (f *fakeKeys) justPressed(k ebiten.Key) bool { return f.just[k] }
func (f *fakeKeys) chars() []rune                 { return f.typed }

func (f *fakeKeys) press(k ebiten.Key, r rune) {
	f.down[k], f.just[k] = true, true
	if r != 0 {
		f.typed = append(f.typed, r)
	}
}

func (f *fakeKeys) release(k ebiten.Key) { delete(f.down, k) }

// frameDone forgets the presses of the frame.
func (f *fakeKeys) frameDone() { clear(f.just); f.typed = nil }

// TestTurnPipeline holds a direction key and presses another one, as a player
// does to turn while walking: the hero must turn on the next step and keep
// going the new way, go back to the old way when the new key is released,
// and a short tap must still make one step aside.
func TestTurnPipeline(t *testing.T) {
	if testing.Short() {
		t.Skip("real-time test")
	}
	ww := startWalk(t, 26, 10)
	defer ww.stop()
	kb := newFakeKeys()
	in := newInput()
	prev := time.Now()
	type sample struct {
		at     float64 // seconds since the script started
		x, y   int     // the hero's cell on the server
		dx, dy float64 // where the hero is drawn
	}
	var log []sample
	frame := func(start time.Time) {
		now := time.Now()
		ww.sink.mu.Lock()
		sc, fx := ww.sink.sc, ww.sink.fx
		ww.sink.fx = nil
		ww.sink.mu.Unlock()
		in.update(ww.scr, kb, now, sc)
		kb.frameDone()
		if sc != nil {
			ww.r.update(now.Sub(prev).Seconds(), sc, fx)
			if st := ww.r.ents[sc.YouID]; st != nil && !start.IsZero() {
				log = append(log, sample{now.Sub(start).Seconds(), sc.Self.X, sc.Self.Y, st.x, st.y})
			}
		}
		prev = now
	}
	for end := time.Now().Add(600 * time.Millisecond); time.Now().Before(end); {
		frame(time.Time{})
		time.Sleep(time.Second / 60)
	}
	// the script: seconds since the start and what the player does
	script := []struct {
		at   float64
		key  ebiten.Key
		down bool
	}{
		{0, ebiten.KeyArrowRight, true},
		{1.0, ebiten.KeyArrowUp, true}, // turn while still holding right
		{2.0, ebiten.KeyArrowUp, false},
		{2.8, ebiten.KeyArrowUp, true}, // a short tap: one step aside
		{2.86, ebiten.KeyArrowUp, false},
		{3.6, ebiten.KeyArrowRight, false},
	}
	start := time.Now()
	next := 0
	for time.Since(start) < 4*time.Second {
		el := time.Since(start).Seconds()
		for next < len(script) && el >= script[next].at {
			if s := script[next]; s.down {
				kb.press(s.key, 0)
			} else {
				kb.release(s.key)
			}
			next++
		}
		frame(start)
		time.Sleep(time.Second / 60)
	}
	if len(log) < 150 {
		t.Fatalf("only %d frames", len(log))
	}
	at := func(sec float64) sample {
		for _, s := range log {
			if s.at >= sec {
				return s
			}
		}
		return log[len(log)-1]
	}
	// steps made in [from, to): right and up
	steps := func(from, to float64) (right, up int) {
		var last *sample
		for i := range log {
			s := &log[i]
			if s.at < from || s.at >= to {
				continue
			}
			if last != nil {
				right += s.x - last.x
				up += last.y - s.y
			}
			last = s
		}
		return
	}
	// when the server hero first changes cell in this way after a moment
	first := func(from float64, moved func(a, b sample) bool) float64 {
		a := at(from)
		for _, s := range log {
			if s.at > from && moved(a, s) {
				return s.at - from
			}
		}
		return math.Inf(1)
	}
	turn := first(1.0, func(a, b sample) bool { return b.y < a.y })
	drawn := first(1.0, func(a, b sample) bool { return b.dy < float64(a.y)-0.25 })
	r1, u1 := steps(1.0+turn+0.05, 2.0)
	back := first(2.0, func(a, b sample) bool { return b.x > a.x })
	r2, u2 := steps(2.8, 3.6)
	t.Logf("turn: on the server after %.0f ms, drawn after %.0f ms; then %d steps up and %d right while both keys are held",
		turn*1000, drawn*1000, u1, r1)
	t.Logf("back to the right %.0f ms after up is released; a 60 ms tap: %d steps up, %d right", back*1000, u2, r2)
	step := 1 / 6.94 // the rogue's pace
	if turn > step+0.12 {
		t.Errorf("the hero turns %.0f ms after the key is pressed", turn*1000)
	}
	if drawn > step+0.2 {
		t.Errorf("the hero is seen turning %.0f ms after the key is pressed", drawn*1000)
	}
	if r1 != 0 || u1 < 3 {
		t.Errorf("with up pressed last the hero walks %d up and %d right", u1, r1)
	}
	if back > step+0.12 {
		t.Errorf("the hero goes on to the right %.0f ms after up is released", back*1000)
	}
	if u2 != 1 || r2 < 2 {
		t.Errorf("a short tap of up while walking right: %d steps up, %d right", u2, r2)
	}
}
