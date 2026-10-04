package gfx

import (
	"image/color"
	"math"
)

// blob fills a shaded ellipse lit from the top-left.
func (p *pc) blob(cx, cy, rx, ry float64, c color.RGBA) {
	for y := int(cy - ry - 1); y <= int(cy+ry+1); y++ {
		for x := int(cx - rx - 1); x <= int(cx+rx+1); x++ {
			dx, dy := (float64(x)+0.5-cx)/rx, (float64(y)+0.5-cy)/ry
			if dx*dx+dy*dy > 1 {
				continue
			}
			l := 1.02 - 0.22*dx - 0.3*dy
			p.set(x, y, mul(c, l))
		}
	}
}

// thick draws a line of a given width.
func (p *pc) thick(x0, y0, x1, y1 float64, w float64, c color.RGBA) {
	n := int(math.Max(math.Abs(x1-x0), math.Abs(y1-y0))*2) + 1
	for i := 0; i <= n; i++ {
		t := float64(i) / float64(n)
		x, y := x0+(x1-x0)*t, y0+(y1-y0)*t
		for yy := int(y - w/2 + 0.5); yy < int(y+w/2+0.5); yy++ {
			for xx := int(x - w/2 + 0.5); xx < int(x+w/2+0.5); xx++ {
				p.set(xx, yy, c)
			}
		}
		if w <= 1 {
			p.set(int(x), int(y), c)
		}
	}
}

// ---- quadrupeds (side view, facing right) ----

type quad struct {
	w, h        int
	body        [4]float64 // cx cy rx ry
	head        [3]float64 // cx cy r
	snout       [4]float64
	neck        bool
	ear         string // pointy, round, small, horns
	tail        string // bushy, thin, short, long, spiked
	legs        []float64
	legW        float64
	legTop      float64
	fur, belly  color.RGBA
	accent      color.RGBA
	muzzle      color.RGBA
	tusk, mane  bool
	spots, wing bool
	eye         color.RGBA
}

func paintQuad(q quad, frame int) *pc {
	p := newPC(q.w, q.h, "quad")
	fur := q.fur
	dark := mul(fur, 0.6)
	b := q.body
	// tail
	tx, ty := b[0]-b[2]+0.5, b[1]-b[3]*0.3
	switch q.tail {
	case "bushy":
		p.blob(tx-1.8, ty-1.2, 2, 1.4, mul(fur, 0.9))
		p.blob(tx-3.2, ty-0.2, 1.4, 1.1, mix(fur, cWhite, 0.25))
	case "thin":
		for i := 0; i < 6; i++ {
			p.set(int(tx)-i, int(ty+float64(i)*0.4), mul(q.accent, 0.95))
		}
	case "short":
		p.set(int(tx)-1, int(ty)-1, dark)
		p.set(int(tx)-1, int(ty), fur)
	case "long", "spiked":
		for i := 0; i < 9; i++ {
			w := 2.2 - float64(i)*0.22
			p.blob(tx-float64(i)*0.9, ty+1+float64(i)*0.35, math.Max(0.6, w), math.Max(0.5, w*0.7), mul(fur, 0.95-0.03*float64(i)))
		}
		if q.tail == "spiked" {
			p.set(int(tx-8.5), int(ty+3), q.accent)
			p.set(int(tx-9), int(ty+2.6), q.accent)
		}
	}
	// legs: far ones darker, step frame alternates
	bottom := float64(q.h) - 1
	for i, lx := range q.legs {
		lift := 0.0
		if frame == 1 && i%2 == 1 {
			lift = 1
		}
		c := mul(fur, 0.82)
		if i%2 == 1 {
			c = mul(fur, 0.68)
		}
		for y := int(q.legTop); y <= int(bottom-lift); y++ {
			for x := int(lx); x < int(lx+q.legW); x++ {
				p.set(x, y, c)
			}
		}
		for x := int(lx); x < int(lx+q.legW); x++ {
			p.set(x, int(bottom-lift), mul(fur, 0.4))
		}
	}
	// wings behind the body
	if q.wing {
		wc := mix(q.accent, fur, 0.4)
		lift := 0.0
		if frame == 1 {
			lift = -1.5
		}
		for i := 0; i < 7; i++ {
			x0 := b[0] - 2 + float64(i)
			p.thick(b[0]+1, b[1]-b[3]+1, x0-3, b[1]-b[3]-6+lift+float64(i)*0.5, 1, mul(wc, 0.8+0.03*float64(i)))
		}
		p.thick(b[0]+1, b[1]-b[3]+1, b[0]-4, b[1]-b[3]-6+lift, 1, mul(fur, 0.5))
	}
	// body with a lighter belly
	p.blob(b[0], b[1], b[2], b[3], fur)
	for y := int(b[1]); y <= int(b[1]+b[3]); y++ {
		for x := int(b[0] - b[2]); x <= int(b[0]+b[2]); x++ {
			dx, dy := (float64(x)+0.5-b[0])/b[2], (float64(y)+0.5-b[1])/b[3]
			if dx*dx+dy*dy <= 1 && dy > 0.45 {
				p.set(x, y, mul(q.belly, 1-0.2*dy))
			}
		}
	}
	if q.spots {
		for i := 0; i < 5; i++ {
			x := b[0] - b[2]*0.6 + float64(i)*b[2]*0.3
			p.set(int(x), int(b[1]-b[3]*0.3+float64(i%2)), q.accent)
		}
	}
	if q.mane {
		for x := b[0] - b[2]*0.6; x < b[0]+b[2]*0.8; x += 1 {
			p.set(int(x), int(b[1]-b[3]-0.2), mul(fur, 0.5))
			if int(x)%2 == 0 {
				p.set(int(x), int(b[1]-b[3]-1.2), mul(fur, 0.45))
			}
		}
	}
	// neck and head
	h := q.head
	if q.neck {
		p.thick(b[0]+b[2]*0.6, b[1]-b[3]*0.4, h[0]-0.5, h[1]+0.8, 3, mul(fur, 0.95))
	}
	switch q.ear {
	case "pointy":
		p.set(int(h[0]-1), int(h[1]-h[2]-0.6), dark)
		p.set(int(h[0]-1), int(h[1]-h[2]+0.4), fur)
		p.set(int(h[0]+0.6), int(h[1]-h[2]-0.6), mul(fur, 0.8))
		p.set(int(h[0]+0.6), int(h[1]-h[2]+0.4), fur)
	case "round":
		p.blob(h[0]-1.2, h[1]-h[2]+0.2, 1.1, 1.1, mul(fur, 0.85))
		p.set(int(h[0]-1.2), int(h[1]-h[2]+0.2), q.accent)
	case "small":
		p.set(int(h[0]-0.5), int(h[1]-h[2]), dark)
	case "horns":
		p.thick(h[0]-1, h[1]-h[2]+0.5, h[0]-3.5, h[1]-h[2]-2, 1, cBone)
		p.thick(h[0]+0.5, h[1]-h[2]+0.5, h[0]-1.5, h[1]-h[2]-2.5, 1, mul(cBone, 0.85))
	}
	p.blob(h[0], h[1], h[2], h[2]*0.92, fur)
	s := q.snout
	muzzle := q.muzzle
	if muzzle.A == 0 {
		muzzle = mix(fur, q.belly, 0.35)
	}
	p.blob(s[0], s[1], s[2], s[3], muzzle)
	p.set(int(s[0]+s[2]-0.5), int(s[1]-s[3]*0.4), cDarkK) // nose
	eye := q.eye
	if eye.A == 0 {
		eye = cDarkK
	}
	p.set(int(h[0]+h[2]*0.35), int(h[1]-h[2]*0.25), eye)
	if q.tusk {
		p.set(int(s[0]+s[2]*0.2), int(s[1]+s[3]+0.2), cWhite)
		p.set(int(s[0]+s[2]*0.2)+1, int(s[1]+s[3]-0.6), cWhite)
	}
	return p
}

// ---- other beasts ----

func paintSpider(c, mark color.RGBA, frame int) *pc {
	p := newPC(20, 14, "spider")
	cx := 10.0
	leg := mul(c, 0.55)
	for i := 0; i < 4; i++ {
		fi := float64(i)
		lift := 0.0
		if frame == 1 && i%2 == 0 {
			lift = -1
		}
		y0 := 6.5 + fi*0.7
		kx, ky := 4.2+fi*0.4, 2.0+fi*2.2+lift
		fx, fy := 1.0+fi*0.4, 6.0+fi*2.2
		p.thick(cx-1, y0, cx-kx, ky, 1, leg)
		p.thick(cx-kx, ky, cx-kx-2.5+fi*0.3, fy, 1, mul(leg, 0.85))
		p.thick(cx, y0, cx+kx-1, ky, 1, mul(leg, 0.8))
		p.thick(cx+kx-1, ky, cx+kx+1.5-fi*0.3, fy, 1, mul(leg, 0.7))
		_ = fx
	}
	p.blob(cx-0.5, 5, 4.6, 4, c)
	if mark.A > 0 {
		p.set(int(cx-1), 4, mark)
		p.set(int(cx), 4, mark)
		p.set(int(cx-1), 5, mark)
		p.set(int(cx-1), 3, mul(mark, 0.8))
	}
	p.blob(cx-0.5, 9.5, 2.6, 2.2, mul(c, 0.85))
	red := hex("#ff3a3a")
	p.set(int(cx-2), 9, red)
	p.set(int(cx), 9, red)
	p.set(int(cx-1), 10, mul(red, 0.8))
	p.set(int(cx-2), 12, cWhite)
	p.set(int(cx), 12, cWhite)
	return p
}

// paintScorpion: side view facing right, tail curled over the back.
func paintScorpion(c color.RGBA, frame int) *pc {
	p := newPC(20, 15, "scorpion")
	leg := mul(c, 0.55)
	for i := 0; i < 4; i++ {
		fi := float64(i)
		lift := 0.0
		if frame == 1 && i%2 == 1 {
			lift = -1
		}
		x := 6 + fi*2.2
		p.thick(x, 10, x-1.2, 12.5+lift, 1, leg)
		p.thick(x-1.2, 12.5+lift, x-1.8, 14+lift, 1, mul(leg, 0.8))
	}
	// tail: segments rising from the rear and curling forward
	segs := [][3]float64{{4.5, 9.5, 1.7}, {3, 7.5, 1.6}, {2.6, 5.3, 1.5}, {3.4, 3.2, 1.4}, {5.2, 1.8, 1.3}, {7.3, 1.6, 1.2}}
	for i, sg := range segs {
		p.blob(sg[0], sg[1], sg[2], sg[2], mul(c, 0.95-0.03*float64(i)))
	}
	p.set(9, 2, hex("#3a1a10"))
	p.set(9, 3, hex("#3a1a10"))
	p.set(8, 4, hex("#5a2a10"))
	// body segments
	p.blob(10, 9.5, 5.5, 2.4, c)
	for x := 7; x <= 13; x += 2 {
		p.set(x, 8, mul(c, 0.7))
	}
	// head and pincers
	p.blob(15.5, 9.8, 1.8, 1.7, mul(c, 1.05))
	p.thick(16, 10.5, 18, 8.5, 1.2, mul(c, 0.9))
	p.blob(18.3, 7.6, 1.5, 1.2, mul(c, 1.1))
	p.set(19, 8, cDarkK)
	p.thick(16, 11, 18, 12, 1.2, mul(c, 0.8))
	p.blob(18.5, 12.3, 1.3, 1, mul(c, 0.95))
	p.set(16, 9, cDarkK)
	return p
}

func paintBeetle(c color.RGBA, frame int) *pc {
	p := newPC(12, 11, "beetle")
	for i := 0; i < 3; i++ {
		fi := float64(i)
		lift := 0.0
		if frame == 1 && i == 1 {
			lift = -1
		}
		p.thick(5, 4+fi*2, 1, 3+fi*2.5+lift, 1, mul(c, 0.4))
		p.thick(6, 4+fi*2, 10, 3+fi*2.5+lift, 1, mul(c, 0.35))
	}
	p.blob(5.5, 5, 3.6, 3.8, c)
	for y := 2; y < 9; y++ {
		p.set(5, y, mul(c, 0.5))
	}
	p.set(4, 3, mix(c, cWhite, 0.7))
	p.blob(5.5, 9.3, 1.8, 1.2, mul(c, 0.6))
	return p
}

func paintBat(c color.RGBA, frame int) *pc {
	p := newPC(20, 12, "bat")
	cx := 10.0
	up := 0.0
	if frame == 1 {
		up = -2.5
	}
	wing := mul(c, 0.75)
	for _, s := range []float64{-1, 1} {
		for i := 0; i < 8; i++ {
			fi := float64(i)
			top := 4 + up*(fi/8) + fi*0.15
			bot := 6 + fi*0.35
			if i%3 == 2 {
				bot -= 1
			}
			for y := int(top); y <= int(bot); y++ {
				p.set(int(cx+s*(2+fi)), y, mul(wing, 1-0.05*fi))
			}
		}
		p.thick(cx+s*2, 4, cx+s*9, 3+up, 1, mul(c, 0.5))
	}
	p.blob(cx-0.5, 6, 2.4, 2.8, c)
	p.set(int(cx-2), 3, c)
	p.set(int(cx+1), 3, mul(c, 0.8))
	p.set(int(cx-2), 5, hex("#ff4a4a"))
	p.set(int(cx), 5, hex("#ff4a4a"))
	p.set(int(cx-1), 7, cWhite)
	return p
}

func paintSlime(c color.RGBA, frame int) *pc {
	p := newPC(16, 12, "slime")
	sq := 0.0
	if frame == 1 {
		sq = 0.8
	}
	cx, cy := 7.5, 7.5+sq*0.5
	for y := 0; y < 12; y++ {
		for x := 0; x < 16; x++ {
			dx, dy := (float64(x)+0.5-cx)/(6.5+sq), (float64(y)+0.5-cy)/(4.5-sq)
			if dy > 0.75 || dx*dx+dy*dy > 1 {
				continue
			}
			l := 1.05 - 0.25*dx - 0.35*dy
			p.set(x, y, alpha(mul(c, l), 225))
		}
	}
	p.set(5, 5, alpha(cWhite, 230))
	p.set(4, 6, alpha(cWhite, 150))
	p.set(6, 7, cDarkK)
	p.set(9, 7, cDarkK)
	p.set(7, 9, mul(c, 0.4))
	p.set(8, 9, mul(c, 0.4))
	return p
}

func paintGhost(c color.RGBA, frame int, hair color.RGBA, hood bool) *pc {
	p := newPC(16, 20, "ghost")
	cx := 7.5
	body := alpha(c, 205)
	for y := 2; y < 20; y++ {
		half := 5.2
		if y < 7 {
			dy := float64(7-y) / 5
			half = 5.2 * math.Sqrt(math.Max(0, 1-dy*dy))
		}
		if y > 13 {
			half -= float64(y-13) * 0.35
		}
		wave := math.Sin(float64(y)*0.9+float64(frame)*2) * 0.8
		for x := int(cx - half + wave); x <= int(cx+half+wave); x++ {
			l := 1.08 - 0.3*(float64(x)-cx)/6
			a := uint8(205)
			if y > 14 {
				a = uint8(205 - (y-14)*30)
			}
			p.set(x, y, alpha(mul(body, l), a))
		}
	}
	if hood {
		hc := mul(c, 0.35)
		for y := 2; y < 9; y++ {
			for x := 2; x < 14; x++ {
				if p.get(x, y).A > 0 && (x < 5 || x > 10 || y < 4) {
					p.set(x, y, hc)
				}
			}
		}
		for x := 5; x <= 10; x++ {
			for y := 4; y < 9; y++ {
				p.set(x, y, cDarkK)
			}
		}
		e := hex("#7ad0ff")
		p.set(6, 6, e)
		p.set(9, 6, e)
	} else {
		p.set(5, 6, cDarkK)
		p.set(5, 7, cDarkK)
		p.set(9, 6, cDarkK)
		p.set(9, 7, cDarkK)
		p.set(7, 10, mul(cDarkK, 1.5))
		p.set(7, 11, mul(cDarkK, 1.5))
	}
	if hair.A > 0 {
		for y := 2; y < 15; y++ {
			p.set(2+(y%2), y, alpha(hair, 220))
			p.set(13-(y%2), y, alpha(mul(hair, 0.8), 220))
		}
		for x := 3; x < 13; x++ {
			p.set(x, 2, hair)
		}
	}
	// arms
	p.set(2, 10+frame, alpha(c, 180))
	p.set(1, 11+frame, alpha(c, 150))
	p.set(13, 10-frame+1, alpha(c, 180))
	p.set(14, 11-frame+1, alpha(c, 150))
	return p
}

func paintElemental(c color.RGBA, frame int) *pc {
	p := newPC(16, 20, "elemental")
	shard := func(cx, top, bot, half float64, col color.RGBA) {
		for y := int(top); y <= int(bot); y++ {
			k := (float64(y) - top) / (bot - top)
			w := half * (1 - math.Abs(k-0.4)*1.4)
			for x := int(cx - w); x <= int(cx+w); x++ {
				l := 1.15 - 0.4*(float64(x)-(cx-w))/math.Max(1, 2*w)
				p.set(x, y, alpha(mul(col, l), 235))
			}
		}
	}
	bob := float64(frame)
	shard(3.5, 6+bob, 13+bob, 2, mul(c, 0.85))
	shard(12, 5-bob, 12-bob, 2, mul(c, 0.8))
	shard(7.5, 2+bob*0.5, 17+bob*0.5, 3.6, c)
	shard(7.5, 13, 19, 1.5, mul(c, 0.7))
	p.set(6, 7+frame, cWhite)
	p.set(9, 7+frame, cWhite)
	p.set(5, 4, alpha(cWhite, 200))
	return p
}

func paintWorm(c color.RGBA, frame int) *pc {
	p := newPC(16, 22, "worm")
	sway := float64(frame)*0.8 - 0.4
	// sand mound
	p.blob(7.5, 20, 7, 2, hex("#c8a060"))
	for i := 0; i < 7; i++ {
		fi := float64(i)
		x := 7.5 + math.Sin(fi*0.7)*sway*1.5
		y := 18 - fi*2.3
		r := 3.6 - fi*0.15
		p.blob(x, y, r, 1.8, mul(c, 0.92+0.04*float64(i%2)))
		p.set(int(x-r+1), int(y), mul(c, 0.6))
	}
	// open mouth with teeth on top
	p.blob(7.5+sway, 3, 3.2, 1.8, hex("#4a1010"))
	for x := 5; x <= 10; x += 2 {
		p.set(x+int(sway), 2, cWhite)
		p.set(x+int(sway)+1, 4, mul(cWhite, 0.8))
	}
	return p
}

func paintTreant(c color.RGBA, frame int) *pc {
	p := newPC(24, 28, "treant")
	bark := mix(hex("#6a4a2a"), c, 0.15)
	// roots / legs
	lift := float64(frame)
	p.thick(9, 20, 6, 27-lift, 2.4, mul(bark, 0.85))
	p.thick(14, 20, 17, 27, 2.4, mul(bark, 0.75))
	// trunk
	for y := 8; y < 23; y++ {
		for x := 8; x < 16; x++ {
			l := 1.12 - 0.05*float64(x-8)
			if (x+y/3)%4 == 0 {
				l *= 0.75
			}
			p.set(x, y, mul(bark, l))
		}
	}
	// branch arms
	p.thick(8, 11, 3, 15+lift, 2, bark)
	p.thick(3, 15+lift, 1, 12+lift, 1, mul(bark, 0.9))
	p.thick(15, 11, 20, 15-lift, 2, mul(bark, 0.8))
	p.thick(20, 15-lift, 22, 12-lift, 1, mul(bark, 0.7))
	// face
	p.set(10, 13, hex("#ffd84a"))
	p.set(13, 13, hex("#ffd84a"))
	for x := 10; x <= 13; x++ {
		p.set(x, 16, mul(bark, 0.35))
	}
	// canopy
	p.blob(12, 6, 9, 5.5, c)
	p.blob(7, 7, 4.5, 3.5, mul(c, 1.08))
	p.blob(16.5, 5, 4.5, 3.5, mul(c, 0.92))
	for i := 0; i < 14; i++ {
		x, y := 4+p.r.IntN(16), 2+p.r.IntN(8)
		if p.get(x, y).A > 0 {
			p.set(x, y, mul(c, 1.3))
		}
	}
	return p
}
