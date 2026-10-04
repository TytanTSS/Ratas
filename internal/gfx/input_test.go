package gfx

import (
	"slices"
	"strings"
	"testing"
	"time"

	"github.com/gdamore/tcell/v2"
	"github.com/hajimehoshi/ebiten/v2"

	"ratas/internal/client"
	"ratas/internal/world"
)

// keyLog records the key events the window sends to the client.
type keyLog struct {
	tcell.SimulationScreen
	at   time.Duration
	keys []string
	when []time.Duration
}

func (l *keyLog) InjectKey(k tcell.Key, r rune, _ tcell.ModMask) {
	name := string(r)
	if k != tcell.KeyRune {
		name = tcell.KeyNames[k]
	}
	l.keys = append(l.keys, name)
	l.when = append(l.when, l.at)
}

// between lists the keys sent in [from, to).
func (l *keyLog) between(from, to time.Duration) []string {
	var out []string
	for i, w := range l.when {
		if w >= from && w < to {
			out = append(out, l.keys[i])
		}
	}
	return out
}

// keyScript plays presses and releases at 60 frames a second.
type keyScript struct {
	in  *inputState
	kb  *fakeKeys
	log *keyLog
	sc  *client.Scene
	t0  time.Time
	now time.Duration
}

func newKeyScript(walking bool) *keyScript {
	return &keyScript{in: newInput(), kb: newFakeKeys(), log: &keyLog{}, t0: time.Now(),
		sc: &client.Scene{Walking: walking}}
}

// until runs frames up to the moment d.
func (s *keyScript) until(d time.Duration) {
	for s.now < d {
		s.log.at = s.now
		s.in.update(s.log, s.kb, s.t0.Add(s.now), s.sc)
		s.kb.frameDone()
		s.now += time.Second / 60
	}
}

func count(keys []string, k string) int {
	return len(slices.DeleteFunc(slices.Clone(keys), func(o string) bool { return o != k }))
}

// TestHeldTurn: with → held, pressing ↑ turns at once and only ↑ repeats;
// releasing ↑ after the hero stepped up walks on to the right at once.
func TestHeldTurn(t *testing.T) {
	s := newKeyScript(true)
	s.sc.Self.Facing = uint8(world.DirRight)
	s.kb.press(ebiten.KeyArrowRight, 0)
	s.until(500 * time.Millisecond)
	if n := count(s.log.keys, "Right"); n < 5 {
		t.Fatalf("→ held for half a second sent %d times", n)
	}
	s.kb.press(ebiten.KeyArrowUp, 0)
	s.until(1000 * time.Millisecond)
	turn := s.log.between(500*time.Millisecond, 1000*time.Millisecond)
	if len(turn) == 0 || turn[0] != "Up" || count(turn, "Right") > 0 || count(turn, "Up") < 5 {
		t.Fatalf("pressing ↑ while → is held sends %v", turn)
	}
	// the hero has walked up
	s.sc.Self.Y, s.sc.Self.Facing = -3, uint8(world.DirUp)
	s.kb.release(ebiten.KeyArrowUp)
	s.until(1100 * time.Millisecond)
	back := s.log.between(1000*time.Millisecond, 1100*time.Millisecond)
	if len(back) == 0 || back[0] != "Right" || count(back, "Up") > 0 {
		t.Fatalf("releasing ↑ with → still held sends %v", back)
	}
	if first := s.log.when[len(s.log.when)-len(back)]; first > 1000*time.Millisecond+time.Second/60 {
		t.Errorf("→ comes back %v after ↑ is released", first-1000*time.Millisecond)
	}
}

// TestTapAside: a short tap of ↑ while walking right is not lost: → waits
// until the hero has stepped up.
func TestTapAside(t *testing.T) {
	s := newKeyScript(true)
	s.sc.Self.Facing = uint8(world.DirRight)
	s.kb.press(ebiten.KeyArrowRight, 0)
	s.until(500 * time.Millisecond)
	s.kb.press(ebiten.KeyArrowUp, 0)
	s.until(540 * time.Millisecond)
	s.kb.release(ebiten.KeyArrowUp)
	s.until(620 * time.Millisecond)
	if got := s.log.between(500*time.Millisecond, 620*time.Millisecond); count(got, "Right") > 0 {
		t.Fatalf("→ overrides the ↑ tap before the hero stepped up: %v", got)
	}
	s.sc.Self.X, s.sc.Self.Y, s.sc.Self.Facing = 0, -1, uint8(world.DirUp)
	s.until(660 * time.Millisecond)
	if got := s.log.between(620*time.Millisecond, 660*time.Millisecond); len(got) == 0 || got[0] != "Right" {
		t.Fatalf("after the step up the hero does not walk on: %v", got)
	}
	// a tap the hero never acts on (stunned) does not stop the walk for long
	s.kb.press(ebiten.KeyArrowDown, 0)
	s.until(700 * time.Millisecond)
	s.kb.release(ebiten.KeyArrowDown)
	s.until(1100 * time.Millisecond)
	got := s.log.between(700*time.Millisecond, 1100*time.Millisecond)
	if count(got, "Right") < 2 {
		t.Fatalf("an ignored tap stops the walk: %v", got)
	}
	if i := slices.Index(s.log.keys, "Down"); s.log.when[i+1] > s.log.when[i]+turnHold+time.Second/60 {
		t.Errorf("an ignored tap stops the walk for %v", s.log.when[i+1]-s.log.when[i])
	}
}

// TestTypingRollover: in a text field keys pressed over each other type
// each letter once.
func TestTypingRollover(t *testing.T) {
	s := newKeyScript(false)
	s.kb.press(ebiten.KeyA, 'a')
	s.until(60 * time.Millisecond)
	s.kb.press(ebiten.KeyD, 'd')
	s.until(110 * time.Millisecond)
	s.kb.release(ebiten.KeyD)
	s.until(400 * time.Millisecond)
	s.kb.release(ebiten.KeyA)
	s.until(500 * time.Millisecond)
	if got := strings.Join(s.log.keys, ""); got != "ad" {
		t.Fatalf("typing a and d over each other gives %q", got)
	}
}
