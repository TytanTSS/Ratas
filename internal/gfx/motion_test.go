package gfx

import (
	"math"
	"testing"
)

// TestSmoothWalk: a creature walking cell by cell with jittery server timing
// moves without stops or jumps, rounds the corner and rests on its last cell.
func TestSmoothWalk(t *testing.T) {
	for _, you := range []bool{false, true} {
		smoothWalk(t, you)
	}
}

func smoothWalk(t *testing.T, you bool) {
	st := &entState{stepS: 0.16, you: you}
	st.seg.p1x, st.seg.p1y = 0, 0
	path := [][2]int{{1, 0}, {2, 0}, {3, 0}, {3, 1}, {3, 2}, {3, 3}}
	// the server steps every 160 ms but sends the world every 50 ms tick, and
	// the network adds a little jitter on top
	jitter := []float64{0, 0.012, 0.004, 0.015, 0, 0.008}
	var arrive []float64
	for i := range path {
		step := 0.1 + float64(i)*0.16
		arrive = append(arrive, math.Ceil(step/0.05)*0.05+jitter[i])
	}
	const dt = 1.0 / 60
	next := 0
	minSpeed, maxJump, maxOff := math.Inf(1), 0.0, 0.0
	for now := 0.0; now < 2; now += dt {
		for next < len(path) && now >= arrive[next] {
			// as in the renderer: the curve starts at the last drawn frame
			st.moveTo(path[next][0], path[next][1], now-dt, kindMonster)
			next++
		}
		ox, oy := st.x, st.y
		st.advance(now, dt, kindMonster)
		moved := math.Hypot(st.x-ox, st.y-oy)
		maxJump = math.Max(maxJump, moved)
		if now > arrive[0]+0.08 && now < arrive[len(arrive)-1] {
			minSpeed = math.Min(minSpeed, moved/dt)
		}
		// distance from the walked polyline (0,0)→(3,0)→(3,3)
		off := math.Min(math.Abs(st.y)+math.Max(0, st.x-3), math.Abs(st.x-3)+math.Max(0, -st.y))
		maxOff = math.Max(maxOff, off)
	}
	pace := 1 / st.stepS
	t.Logf("own hero %v: slowest %.2f of pace %.2f cells/s, biggest frame move %.3f, furthest from the path %.2f", you, minSpeed, pace, maxJump, maxOff)
	if minSpeed < 0.6*pace {
		t.Errorf("the walk slows to %.2f cells/s (pace %.2f): it stutters between cells", minSpeed, pace)
	}
	if maxJump > 0.16 {
		t.Errorf("a frame jumps %.2f cells", maxJump)
	}
	if maxOff > 0.3 {
		t.Errorf("the path strays %.2f cells from the cells walked", maxOff)
	}
	if st.x != 3 || st.y != 3 || st.vx != 0 || st.vy != 0 {
		t.Errorf("does not rest on the last cell: (%.3f, %.3f) v=(%.3f, %.3f)", st.x, st.y, st.vx, st.vy)
	}
	if st.walk < 5.5 || st.walk > 7 {
		t.Errorf("walked %.2f cells for 6 steps", st.walk)
	}
}

// TestTeleportJumps: far moves (stairs, blinks) are not animated.
func TestTeleportJumps(t *testing.T) {
	st := &entState{stepS: 0.2}
	st.moveTo(10, 0, 0, kindPlayer)
	st.advance(0.01, 0.01, kindPlayer)
	if st.x != 10 || st.y != 0 {
		t.Fatalf("teleport drawn at (%.2f, %.2f)", st.x, st.y)
	}
}
