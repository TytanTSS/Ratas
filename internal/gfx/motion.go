package gfx

import (
	"math"

	"ratas/internal/client"
	"ratas/internal/proto"
	"ratas/internal/world"
)

// Smooth movement. The world is a grid and the server moves creatures from
// cell to cell; the window draws them gliding instead. Every step the server
// reports starts a curve (a cubic Hermite segment) from where the creature
// is drawn now, with its current velocity, to the new cell. The curve takes
// as long as the creature's real step, so a walking creature never stops
// between cells and its velocity stays continuous: it speeds up from rest,
// keeps its pace, rounds corners instead of turning on the spot, and after
// the last step settles with a little inertia.

// segment is one piece of a creature's path.
type segment struct {
	p0x, p0y, v0x, v0y float64 // start: position (cells) and velocity (cells/s)
	p1x, p1y, v1x, v1y float64 // end
	t0, dur            float64 // start time and length (s); dur 0 = at rest
	settle             bool    // the short stop after the last step
}

// at evaluates the segment at time t: position and velocity.
func (s *segment) at(t float64) (x, y, vx, vy float64) {
	u := math.Max(0, math.Min(1, (t-s.t0)/s.dur))
	u2, u3 := u*u, u*u*u
	h00, h10, h01, h11 := 2*u3-3*u2+1, u3-2*u2+u, -2*u3+3*u2, u3-u2
	d00, d10, d01, d11 := 6*u2-6*u, 3*u2-4*u+1, -6*u2+6*u, 3*u2-2*u
	x = h00*s.p0x + h10*s.dur*s.v0x + h01*s.p1x + h11*s.dur*s.v1x
	y = h00*s.p0y + h10*s.dur*s.v0y + h01*s.p1y + h11*s.dur*s.v1y
	vx = (d00*s.p0x + d10*s.dur*s.v0x + d01*s.p1x + d11*s.dur*s.v1x) / s.dur
	vy = (d00*s.p0y + d10*s.dur*s.v0y + d01*s.p1y + d11*s.dur*s.v1y) / s.dur
	return
}

const (
	settleTime = 0.16 // seconds of inertia after the last step
	serverTick = 0.06 // a server tick (50 ms) and a little more
)

// moveTo starts the curve to a new cell reported by the server.
func (st *entState) moveTo(tx, ty int, now float64, kind uint8) {
	fx, fy := float64(tx), float64(ty)
	dx, dy := fx-st.x, fy-st.y
	dist := math.Hypot(dx, dy)
	prevX, prevY := float64(st.tx), float64(st.ty)
	st.tx, st.ty = tx, ty
	if dist > 3.5 { // teleports, stairs, a lost track: jump
		st.x, st.y, st.vx, st.vy = fx, fy, 0, 0
		st.seg = segment{p1x: fx, p1y: fy}
		return
	}
	speed := 1 / st.stepS
	// the direction the creature is walking in: from its previous cell
	ux, uy := fx-prevX, fy-prevY
	if l := math.Hypot(ux, uy); l > 0 && l < 2 {
		ux, uy = ux/l, uy/l
	} else if dist > 0 {
		ux, uy = dx/dist, dy/dist
	}
	end := 0.78 // keeps walking at the end of the step; settling stops it
	// a step lasts as long as the creature's real step plus one server tick:
	// the next step arrives before the curve ends even when it is a little
	// late, so walking never stalls. A creature that falls behind covers
	// more than a cell in the same time and catches up.
	dur := st.stepS*math.Max(0.3, math.Min(1, 0.25+0.87*dist)) + serverTick
	if kind == kindProjectile {
		end, dur = 1, st.stepS*math.Max(0.3, math.Min(1, dist))
	}
	st.seg = segment{
		p0x: st.x, p0y: st.y, v0x: st.vx, v0y: st.vy,
		p1x: fx, p1y: fy, v1x: ux * speed * end, v1y: uy * speed * end,
		t0: now, dur: dur,
	}
	st.moveAt = now
}

// advance moves the drawn position along the path.
func (st *entState) advance(now, dt float64, kind uint8) {
	s := &st.seg
	if s.dur <= 0 {
		st.x, st.y, st.vx, st.vy = s.p1x, s.p1y, 0, 0
		return
	}
	if now >= s.t0+s.dur {
		moving := math.Hypot(s.v1x, s.v1y) > 0.01
		if s.settle || !moving || kind == kindProjectile {
			st.x, st.y, st.vx, st.vy = s.p1x, s.p1y, 0, 0
			s.dur = 0
			return
		}
		// no new step came: glide a little past the cell and come back
		*s = segment{p0x: s.p1x, p0y: s.p1y, v0x: s.v1x, v0y: s.v1y,
			p1x: s.p1x, p1y: s.p1y, t0: s.t0 + s.dur, dur: settleTime, settle: true}
	}
	ox, oy := st.x, st.y
	st.x, st.y, st.vx, st.vy = s.at(now)
	st.walk += math.Hypot(st.x-ox, st.y-oy)
}

// syncEnts takes the creatures of a new snapshot; from is the time of the
// last drawn frame, where new curves begin.
func (r *worldRenderer) syncEnts(sc *client.Scene, from float64) {
	r.gen++
	for _, e := range sc.Entities {
		st := r.ents[e.ID]
		if st == nil {
			st = &entState{x: float64(e.X), y: float64(e.Y), tx: e.X, ty: e.Y, swing: e.Swing}
			st.seg.p1x, st.seg.p1y = st.x, st.y
			if e.Kind == kindProjectile {
				st.trail = [][2]float64{}
			}
			if e.ID == sc.YouID || e.Kind == kindProjectile {
				st.alpha = 1
			}
			r.ents[e.ID] = st
		}
		if e.Step > 0 {
			st.stepS = float64(e.Step) / 1000
		} else if st.stepS == 0 {
			st.stepS = 0.2
		}
		if st.tx != e.X || st.ty != e.Y {
			st.moveTo(e.X, e.Y, from, e.Kind)
		}
		d := world.Dir(e.Facing).Delta()
		st.fx, st.fy = float64(d.X), float64(d.Y)
		if e.Swing != st.swing {
			st.swing = e.Swing
			st.lunge = 1
		}
		st.gen = r.gen
	}
	for id, st := range r.ents {
		if st.gen != r.gen {
			delete(r.ents, id)
		}
	}
}

// animate advances every creature: paths, facing, swings and fading in and
// out of sight.
func (r *worldRenderer) animate(dt float64, sc *client.Scene) {
	vis := r.fov(sc)
	l := sc.Level
	fade := 1 - math.Exp(-dt*9)
	byID := make(map[uint32]*proto.EntityView, len(sc.Entities))
	for i := range sc.Entities {
		byID[sc.Entities[i].ID] = &sc.Entities[i]
	}
	for id, st := range r.ents {
		e := byID[id]
		if e == nil {
			continue
		}
		st.advance(r.t, dt, e.Kind)
		if st.trail != nil {
			st.trail = append(st.trail, [2]float64{st.x, st.y})
			if len(st.trail) > 7 {
				st.trail = st.trail[1:]
			}
		}
		// face where the creature goes, or where the server says it looks
		speed := math.Hypot(st.vx, st.vy)
		switch {
		case math.Abs(st.vx) > 0.25/st.stepS && math.Abs(st.vx) > math.Abs(st.vy)*0.5:
			st.left = st.vx < 0
		case speed < 0.2/st.stepS && st.fx != 0:
			st.left = st.fx < 0
		}
		st.lunge = math.Max(0, st.lunge-dt/0.22)
		st.flash = math.Max(0, st.flash-dt)
		cx, cy := int(math.Round(st.x)), int(math.Round(st.y))
		target := 0.0
		if l.In(cx, cy) && vis[cy*l.W+cx] {
			target = 1
		}
		if id == sc.YouID {
			target = 1
		}
		st.alpha += (target - st.alpha) * fade
	}
}

// pose is how a creature is drawn this frame: walking frame, lift off the
// ground (bobbing steps) and the lunge of an attack.
func (st *entState) pose() (frame int, bob, lungeX, lungeY float64) {
	speed := math.Hypot(st.vx, st.vy) * st.stepS // 1 = full walking pace
	if speed > 0.15 {
		half := st.walk * 2 // two footfalls per cell
		frame = int(half) % 2
		bob = math.Sin((half-math.Floor(half))*math.Pi) * math.Min(1, speed)
	}
	if st.lunge > 0 {
		k := math.Sin(st.lunge*math.Pi) * 0.24
		lungeX, lungeY = st.fx*k, st.fy*k
	}
	return
}
