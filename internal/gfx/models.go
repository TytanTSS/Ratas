package gfx

import (
	"image"
	"image/color"
)

// Creature models are pixel art painted in code: humanoids are assembled
// from a build (proportions), an outfit, headgear and a weapon; beasts and
// monsters are hand-drawn maps. Colours come from the creature definition,
// so a modded "ice wolf" with model "wolf" gets a recoloured wolf.

// ---- humanoid builds ----

type geom struct {
	w, h       int
	head       image.Rectangle
	torso      image.Rectangle
	armL, armR image.Rectangle // arms (sleeves); hands are drawn below them
	legL, legR image.Rectangle
	feet       int // top row of the boots (two rows)
}

func rc(x0, y0, x1, y1 int) image.Rectangle { return image.Rect(x0, y0, x1+1, y1+1) }

var builds = map[string]geom{
	"normal": {w: 16, h: 20, head: rc(4, 1, 11, 7), torso: rc(4, 8, 11, 13),
		armL: rc(2, 9, 3, 13), armR: rc(12, 9, 13, 13), legL: rc(5, 14, 7, 17), legR: rc(8, 14, 10, 17), feet: 18},
	"small": {w: 16, h: 20, head: rc(4, 5, 11, 11), torso: rc(5, 12, 10, 15),
		armL: rc(3, 12, 4, 15), armR: rc(11, 12, 12, 15), legL: rc(5, 16, 7, 17), legR: rc(8, 16, 10, 17), feet: 18},
	"big": {w: 20, h: 24, head: rc(6, 1, 13, 7), torso: rc(4, 8, 15, 16),
		armL: rc(1, 9, 3, 16), armR: rc(16, 9, 18, 16), legL: rc(5, 17, 8, 21), legR: rc(11, 17, 14, 21), feet: 22},
	"huge": {w: 24, h: 28, head: rc(8, 1, 15, 8), torso: rc(5, 9, 18, 19),
		armL: rc(1, 10, 4, 19), armR: rc(19, 10, 22, 19), legL: rc(6, 20, 10, 25), legR: rc(13, 20, 17, 25), feet: 26},
}

// hum describes a humanoid.
type hum struct {
	build      string
	skin       color.RGBA
	hair       color.RGBA // A == 0: bald
	hairStyle  string     // short, long, wild, pony
	beard      color.RGBA
	eyes       color.RGBA
	glow       bool // eyes glow (undead, cultists)
	outfit     string
	top, trim  color.RGBA
	pants      color.RGBA
	boots      color.RGBA
	head       string // hood, helmet, horned, wizard, crown, horns, feathers, nemes, turban, cap, skull, snout, ears
	headCol    color.RGBA
	mask       color.RGBA // cloth over the mouth
	weapon     string
	weaponCol  color.RGBA
	shield     color.RGBA
	shieldMark color.RGBA
	shieldForm string     // kite (default), round, tower
	offhand    string     // left hand: a second weapon (dual wield), book, orb, symbol
	offCol     color.RGBA // tint of the left-hand item
	cape       color.RGBA
	wings      color.RGBA
	tail       color.RGBA
	tusks      bool
	crown      bool
	legs       string // "", smoke, talons
}

var (
	cSkin     = hex("#f0c49c")
	cSkinTan  = hex("#c88e62")
	cSkinDark = hex("#8a5a3a")
	cBone     = hex("#e8e4d0")
	cSteel    = hex("#a8b0c0")
	cIron     = hex("#6a707e")
	cGold     = hex("#f0c040")
	cWood     = hex("#8a5a32")
	cLeather  = hex("#7a5232")
	cDarkK    = hex("#16121c")
	cWhite    = hex("#f4f4f4")
)

// box fills a rectangle with simple top-left lighting.
func (p *pc) box(r image.Rectangle, c color.RGBA) {
	for y := r.Min.Y; y < r.Max.Y; y++ {
		for x := r.Min.X; x < r.Max.X; x++ {
			k := 1.0
			switch {
			case x == r.Max.X-1 && r.Dx() > 1:
				k = 0.74
			case y == r.Min.Y:
				k = 1.12
			case x == r.Min.X && r.Dx() > 2:
				k = 1.05
			}
			if y == r.Max.Y-1 && r.Dy() > 2 {
				k *= 0.88
			}
			p.set(x, y, mul(c, k))
		}
	}
}

func (p *pc) clear(x, y int) {
	if x >= 0 && y >= 0 && x < p.w && y < p.h {
		p.img.SetRGBA(x, y, color.RGBA{})
	}
}

func (p *pc) px(x, y int, c color.RGBA) {
	if c.A > 0 {
		p.set(x, y, c)
	}
}

func shift(r image.Rectangle, dx, dy int) image.Rectangle { return r.Add(image.Pt(dx, dy)) }

// paintHumanoid draws a humanoid; frame 1 is the walking step.
func paintHumanoid(h hum, frame int) *pc {
	g, ok := builds[h.build]
	if !ok {
		g = builds["normal"]
	}
	// head room for hats, horns and wings; empty rows are cropped later
	const top = 6
	g.h += top
	g.feet += top
	for _, r := range []*image.Rectangle{&g.head, &g.torso, &g.armL, &g.armR, &g.legL, &g.legR} {
		*r = r.Add(image.Pt(0, top))
	}
	p := newPC(g.w, g.h, "hum")
	defer func() { cropTop(p) }()
	cx := g.w / 2 // the body is symmetric around cx-0.5
	t, hd := g.torso, g.head
	legL, legR := g.legL, g.legR
	armL, armR := g.armL, g.armR
	if frame == 1 {
		legL = shift(legL, 0, -1)
		armL = shift(armL, 0, 1)
		armR = shift(armR, 0, -1)
	}
	// behind the body: wings, cape, tail
	if h.wings.A > 0 {
		paintWings(p, g, h.wings, frame)
	}
	if h.cape.A > 0 {
		p.box(image.Rect(t.Min.X, t.Min.Y, t.Max.X, min(g.h, g.feet+1)), mul(h.cape, 0.8))
	}
	if h.tail.A > 0 {
		for i := 0; i < 5; i++ {
			p.px(t.Max.X+i/2, t.Max.Y-1+i/2-1, mul(h.tail, 0.9-0.06*float64(i)))
		}
		p.px(t.Max.X+3, t.Max.Y, mul(h.tail, 0.7))
	}

	// legs and feet
	switch {
	case h.legs == "smoke":
		for y := t.Max.Y; y < g.h; y++ {
			k := float64(y-t.Max.Y) / float64(g.h-t.Max.Y)
			half := int(float64(t.Dx()/2) * (1 - k*0.8))
			off := int(k * 2.5)
			for x := cx - half + off; x < cx+half+off; x++ {
				p.set(x, y, alpha(mul(h.top, 1-0.3*k), uint8(255*(1-k*0.6))))
			}
		}
	case h.outfit == "robe" || h.outfit == "wrapdress":
	default:
		pants := h.pants
		if pants.A == 0 {
			pants = mul(h.top, 0.7)
		}
		if h.outfit == "bones" {
			pants = cBone
			legL = image.Rect(legL.Min.X+1, legL.Min.Y, legL.Max.X-1, legL.Max.Y)
			legR = image.Rect(legR.Min.X+1, legR.Min.Y, legR.Max.X-1, legR.Max.Y)
		}
		if h.outfit == "fur" || h.outfit == "loin" || h.outfit == "golem" {
			pants = h.skin
			if h.outfit == "fur" {
				pants = h.top
			}
		}
		p.box(legL, pants)
		p.box(legR, mul(pants, 0.92))
		boots := h.boots
		if boots.A == 0 {
			boots = mul(cLeather, 0.8)
		}
		if h.outfit == "bones" || h.outfit == "loin" || h.outfit == "golem" || h.outfit == "fur" {
			boots = mul(pants, 0.85)
		}
		if h.legs == "talons" {
			boots = cGold
		}
		fy := g.feet
		bl := image.Rect(legL.Min.X-1, fy, legL.Max.X, fy+2)
		if frame == 1 {
			bl = shift(bl, 0, -1)
		}
		p.box(bl, boots)
		p.box(image.Rect(legR.Min.X, fy, legR.Max.X+1, fy+2), mul(boots, 0.9))
	}

	// torso and outfit
	paintOutfit(p, g, h, t, frame)

	// arms and hands
	sleeve := h.top
	switch h.outfit {
	case "bones":
		sleeve = cBone
	case "loin", "golem":
		sleeve = h.skin
	case "plate":
		sleeve = mul(h.top, 0.95)
	}
	if h.outfit == "bones" {
		armL = image.Rect(armL.Max.X-1, armL.Min.Y, armL.Max.X, armL.Max.Y)
		armR = image.Rect(armR.Min.X, armR.Min.Y, armR.Min.X+1, armR.Max.Y)
	}
	p.box(armL, sleeve)
	p.box(armR, mul(sleeve, 0.9))
	hand := h.skin
	if h.outfit == "bones" {
		hand = cBone
	}
	if h.outfit == "golem" {
		hand = mul(h.skin, 0.85)
	}
	p.box(image.Rect(armL.Min.X, armL.Max.Y, armL.Max.X, armL.Max.Y+1), hand)
	p.box(image.Rect(armR.Min.X, armR.Max.Y, armR.Max.X, armR.Max.Y+1), mul(hand, 0.9))
	if h.outfit == "plate" {
		// pauldrons
		p.box(image.Rect(armL.Min.X-1, armL.Min.Y-1, armL.Max.X+1, armL.Min.Y+1), mul(h.top, 1.1))
		p.box(image.Rect(armR.Min.X-1, armR.Min.Y-1, armR.Max.X+1, armR.Min.Y+1), h.top)
	}

	// head
	paintHead(p, g, h, hd)

	// weapon and shield in front
	if h.shield.A > 0 {
		paintShield(p, g, armL, h.shield, h.shieldMark, h.shieldForm)
	} else if h.offhand != "" {
		paintOffhand(p, g, armL, armR, h.offhand, h.offCol)
	}
	if h.weapon != "" {
		paintWeapon(p, g, armR, h.weapon, h.weaponCol)
	}
	return p
}

func paintOutfit(p *pc, g geom, h hum, t image.Rectangle, frame int) {
	top := h.top
	switch h.outfit {
	case "bones":
		// ribcage and spine
		for y := t.Min.Y; y < t.Max.Y-1; y++ {
			p.px(t.Min.X+t.Dx()/2-1, y, cBone)
			p.px(t.Min.X+t.Dx()/2, y, mul(cBone, 0.8))
			if (y-t.Min.Y)%2 == 0 && y < t.Max.Y-2 {
				for x := t.Min.X + 1; x < t.Max.X-1; x++ {
					p.px(x, y, mul(cBone, 0.95-0.04*float64(x-t.Min.X)))
				}
			}
		}
		p.box(image.Rect(t.Min.X+1, t.Max.Y-1, t.Max.X-1, t.Max.Y), mul(cBone, 0.85)) // pelvis
	case "loin", "golem":
		p.box(t, h.skin)
		if h.outfit == "golem" {
			for i := 0; i < t.Dx()*t.Dy()/6; i++ {
				x, y := t.Min.X+p.r.IntN(t.Dx()), t.Min.Y+p.r.IntN(t.Dy())
				p.px(x, y, mul(h.skin, 0.72))
			}
			if h.trim.A > 0 { // glowing cracks
				for y := t.Min.Y + 1; y < t.Max.Y-1; y += 2 {
					x := t.Min.X + 1 + p.r.IntN(max(1, t.Dx()-2))
					p.px(x, y, h.trim)
					p.px(x+1, y+1, h.trim)
				}
			}
		} else {
			// chest shading and belly line
			p.px(t.Min.X+t.Dx()/2-1, t.Min.Y+2, mul(h.skin, 0.8))
			p.px(t.Min.X+t.Dx()/2, t.Min.Y+2, mul(h.skin, 0.8))
			loin := h.pants
			if loin.A == 0 {
				loin = cLeather
			}
			p.box(image.Rect(t.Min.X, t.Max.Y-2, t.Max.X, t.Max.Y), loin)
		}
	case "fur":
		p.box(t, top)
		for i := 0; i < t.Dx()*t.Dy()/4; i++ {
			x, y := t.Min.X+p.r.IntN(t.Dx()), t.Min.Y+p.r.IntN(t.Dy())
			p.px(x, y, mul(top, 0.82+0.3*p.r.Float64()))
		}
		if h.trim.A > 0 {
			p.box(image.Rect(t.Min.X, t.Max.Y-2, t.Max.X, t.Max.Y-1), h.trim)
		}
	case "robe", "wrapdress":
		bottom := g.h
		for y := t.Min.Y; y < bottom; y++ {
			k := float64(y-t.Min.Y) / float64(bottom-t.Min.Y)
			grow := int(k*2.4 + 0.2)
			if y < t.Max.Y {
				grow = 0
			}
			r := image.Rect(t.Min.X-grow, y, t.Max.X+grow, y+1)
			p.box(r, mul(top, 1-0.15*k))
		}
		if h.trim.A > 0 {
			for y := t.Min.Y + 1; y < bottom-1; y++ {
				p.px(t.Min.X+t.Dx()/2-1, y, h.trim)
			}
			for x := t.Min.X - 2; x < t.Max.X+2; x++ {
				p.px(x, bottom-2, h.trim)
			}
		}
		if h.outfit == "wrapdress" {
			for y := t.Min.Y; y < bottom; y += 2 {
				for x := t.Min.X - 2; x < t.Max.X+2; x++ {
					if p.get(x, y).A > 0 {
						p.px(x, y, mul(top, 0.78))
					}
				}
			}
		}
		// feet peeking out
		p.px(t.Min.X+1, bottom-1, mul(cLeather, 0.7))
		p.px(t.Max.X-2, bottom-1, mul(cLeather, 0.6))
		belt := h.boots
		if belt.A == 0 {
			belt = mul(top, 0.6)
		}
		p.box(image.Rect(t.Min.X, t.Max.Y-2, t.Max.X, t.Max.Y-1), belt)
	case "plate":
		p.box(t, top)
		// breastplate highlight and tabard
		for y := t.Min.Y + 1; y < t.Max.Y-2; y++ {
			p.px(t.Min.X+1, y, mul(top, 1.25))
		}
		if h.trim.A > 0 {
			mid := t.Min.X + t.Dx()/2
			p.box(image.Rect(mid-1, t.Min.Y+1, mid+1, t.Max.Y+1), h.trim)
		}
		p.box(image.Rect(t.Min.X, t.Max.Y-2, t.Max.X, t.Max.Y-1), mul(cLeather, 0.8))
		p.px(t.Min.X+t.Dx()/2, t.Max.Y-2, cGold)
	case "wrap":
		p.box(t, top)
		for y := t.Min.Y; y < t.Max.Y; y += 2 {
			for x := t.Min.X; x < t.Max.X; x++ {
				p.px(x, y, mul(top, 0.8))
			}
		}
	default: // tunic, leather, rags
		p.box(t, top)
		if h.outfit == "leather" {
			// strap across the chest
			for i := 0; i < t.Dy()-1; i++ {
				p.px(t.Min.X+1+i*(t.Dx()-2)/max(1, t.Dy()-1), t.Min.Y+i, mul(cLeather, 0.75))
			}
		}
		if h.trim.A > 0 {
			p.box(image.Rect(t.Min.X, t.Min.Y, t.Max.X, t.Min.Y+1), h.trim)
		}
		belt := h.boots
		if belt.A == 0 {
			belt = cLeather
		}
		p.box(image.Rect(t.Min.X, t.Max.Y-2, t.Max.X, t.Max.Y-1), mul(belt, 0.85))
		p.px(t.Min.X+t.Dx()/2, t.Max.Y-2, cGold)
		if h.outfit == "rags" {
			for x := t.Min.X; x < t.Max.X; x += 2 {
				p.px(x, t.Max.Y, mul(top, 0.7))
			}
			p.px(t.Min.X+2, t.Min.Y+2, h.skin)
			p.px(t.Max.X-2, t.Min.Y+3, mul(h.skin, 0.9))
		}
	}
	_ = frame
}

func paintHead(p *pc, g geom, h hum, hd image.Rectangle) {
	skin := h.skin
	x0, y0, x1, y1 := hd.Min.X, hd.Min.Y, hd.Max.X-1, hd.Max.Y-1
	face := func() {
		for y := y0; y <= y1; y++ {
			for x := x0; x <= x1; x++ {
				corner := (y == y0 || y == y1) && (x == x0 || x == x1)
				if corner {
					continue
				}
				k := 1.0
				if x == x1 {
					k = 0.78
				}
				if x == x0 {
					k = 1.06
				}
				if y == y1 {
					k *= 0.9
				}
				p.set(x, y, mul(skin, k))
			}
		}
	}
	eyesY := y0 + (y1-y0)*3/5
	ex1, ex2 := x0+2, x1-2
	eye := h.eyes
	if eye.A == 0 {
		eye = cDarkK
	}
	drawEyes := func() {
		p.set(ex1, eyesY, eye)
		p.set(ex2, eyesY, eye)
		if h.glow {
			p.set(ex1, eyesY-1, alpha(eye, 110))
			p.set(ex2, eyesY-1, alpha(eye, 110))
		}
	}
	switch h.head {
	case "skull":
		skin = cBone
		face()
		for _, ex := range []int{ex1 - 1, ex2} {
			for dy := -1; dy <= 0; dy++ {
				p.set(ex, eyesY+dy, cDarkK)
				p.set(ex+1, eyesY+dy, mul(cDarkK, 1.6))
			}
			if h.glow {
				p.set(ex, eyesY, eye)
			}
		}
		for x := ex1; x <= ex2; x += 1 {
			if (x-ex1)%2 == 0 {
				p.set(x, y1, mul(cBone, 0.6))
			}
		}
		p.set((x0+x1)/2, eyesY+1, mul(cBone, 0.55))
	default:
		face()
		drawEyes()
		if h.tusks {
			p.set(ex1, y1, cWhite)
			p.set(ex2, y1, cWhite)
			p.set(ex1, y1-1, mul(cWhite, 0.9))
			p.set(ex2, y1-1, mul(cWhite, 0.9))
		}
	}
	if h.mask.A > 0 {
		for y := eyesY + 1; y <= y1; y++ {
			for x := x0; x <= x1; x++ {
				if p.get(x, y).A > 0 {
					p.set(x, y, mul(h.mask, 1-0.15*float64(x-x0)/float64(x1-x0)))
				}
			}
		}
	}
	// hair
	if h.hair.A > 0 && h.head != "hood" && h.head != "helmet" && h.head != "horned" && h.head != "nemes" && h.head != "skull" && h.head != "fur_hat" {
		hc := h.hair
		for x := x0 + 1; x < x1; x++ {
			p.set(x, y0-1+1, mul(hc, 1.1))
		}
		for x := x0; x <= x1; x++ {
			p.set(x, y0+1, hc)
		}
		p.set(x0, y0+2, mul(hc, 0.9))
		p.set(x1, y0+2, mul(hc, 0.75))
		p.set(x0+1, y0+2, mul(hc, 0.95))
		switch h.hairStyle {
		case "long":
			for y := y0 + 2; y <= y1+3; y++ {
				p.set(x0, y, mul(hc, 0.92))
				p.set(x1, y, mul(hc, 0.72))
			}
			p.set(x0-1, y1+1, mul(hc, 0.85))
			p.set(x1+1, y1+1, mul(hc, 0.7))
		case "wild":
			for x := x0 - 1; x <= x1+1; x += 2 {
				p.set(x, y0, hc)
			}
			p.set(x0-1, y0+2, hc)
			p.set(x1+1, y0+2, mul(hc, 0.75))
		case "pony":
			p.set(x1+1, y0+2, hc)
			p.set(x1+1, y0+3, mul(hc, 0.8))
			p.set(x1+2, y0+4, mul(hc, 0.7))
		}
	}
	if h.beard.A > 0 {
		bc := h.beard
		for y := eyesY + 1; y <= y1+2; y++ {
			w := (x1 - x0 + 1) / 2
			if y > y1 {
				w -= (y - y1) * 2
			}
			mid := (x0 + x1 + 1) / 2
			for x := mid - w; x < mid+w; x++ {
				if y == eyesY+1 && x > mid-2 && x < mid+1 {
					continue // mouth gap
				}
				p.set(x, y, mul(bc, 1-0.18*float64(x-(mid-w))/float64(max(1, 2*w))))
			}
		}
	}
	// headgear
	hc := h.headCol
	switch h.head {
	case "hood":
		for y := y0 - 1; y <= y1; y++ {
			for x := x0 - 1; x <= x1+1; x++ {
				inner := x > x0 && x < x1 && y > y0+1
				edge := (y == y0-1 && (x <= x0 || x >= x1)) || (y == y0 && (x == x0-1 || x == x1+1))
				if inner || edge {
					continue
				}
				k := 1.0
				if x >= x1 {
					k = 0.75
				}
				p.set(x, y, mul(hc, k))
			}
		}
		// face in the shadow of the hood
		for x := x0 + 1; x < x1; x++ {
			p.set(x, y0+2, mul(skin, 0.55))
		}
		if h.glow {
			p.set(ex1, eyesY, eye)
			p.set(ex2, eyesY, eye)
		}
		p.set((x0+x1)/2, y0-2, mul(hc, 0.9))
	case "helmet", "horned":
		for y := y0; y <= y0+3; y++ {
			for x := x0; x <= x1; x++ {
				if y == y0 && (x == x0 || x == x1) {
					continue
				}
				k := 1.0
				if x >= x1-1 {
					k = 0.75
				}
				if y == y0 {
					k *= 1.2
				}
				p.set(x, y, mul(hc, k))
			}
		}
		for y := y0 + 4; y <= y1; y++ { // cheek guards
			p.set(x0, y, mul(hc, 0.95))
			p.set(x1, y, mul(hc, 0.7))
		}
		for x := x0 + 1; x < x1; x++ { // visor slit
			p.set(x, y0+3, cDarkK)
		}
		if h.glow {
			p.set(ex1, y0+3, eye)
			p.set(ex2, y0+3, eye)
		}
		p.set((x0+x1)/2, y0-1, mul(hc, 1.1))
		if h.head == "horned" {
			horn := hex("#e8dcc0")
			p.set(x0-1, y0+1, horn)
			p.set(x0-2, y0, horn)
			p.set(x0-2, y0-1, mul(horn, 0.9))
			p.set(x1+1, y0+1, mul(horn, 0.85))
			p.set(x1+2, y0, mul(horn, 0.8))
			p.set(x1+2, y0-1, mul(horn, 0.75))
		}
	case "wizard":
		top := y0 - 1
		for y := top - 5; y <= y0+1; y++ {
			k := float64(y-(top-5)) / 7
			half := int(k*3.5 + 0.5)
			mid := (x0 + x1 + 1) / 2
			for x := mid - half; x < mid+half; x++ {
				c := hc
				if x >= mid+half-1 {
					c = mul(hc, 0.72)
				}
				p.set(x+int((1-k)*1.5), y, c)
			}
		}
		for x := x0 - 2; x <= x1+2; x++ { // brim
			p.set(x, y0+2, mul(hc, 0.85))
		}
		if h.trim.A > 0 {
			for x := x0; x <= x1; x++ {
				p.set(x, y0+1, h.trim)
			}
		}
	case "crown":
		for x := x0; x <= x1; x++ {
			p.set(x, y0, cGold)
			p.set(x, y0+1, mul(cGold, 0.8))
		}
		for x := x0; x <= x1; x += 2 {
			p.set(x, y0-1, cGold)
		}
		p.set((x0+x1)/2, y0, hex("#e03050"))
	case "horns":
		horn := hc
		if horn.A == 0 {
			horn = hex("#2a2026")
		}
		p.set(x0, y0, horn)
		p.set(x0-1, y0-1, horn)
		p.set(x0-1, y0-2, mul(horn, 0.8))
		p.set(x1, y0, mul(horn, 0.8))
		p.set(x1+1, y0-1, mul(horn, 0.75))
		p.set(x1+1, y0-2, mul(horn, 0.65))
	case "feathers":
		for x := x0; x <= x1; x++ {
			p.set(x, y0+1, hc)
		}
		p.set(x0+1, y0-1, hex("#e04a3a"))
		p.set(x0+1, y0, hex("#e04a3a"))
		p.set(x0+3, y0-2, cWhite)
		p.set(x0+3, y0-1, cWhite)
		p.set(x0+3, y0, mul(cWhite, 0.85))
		p.set(x1-1, y0-1, hex("#4a8ae0"))
		p.set(x1-1, y0, hex("#4a8ae0"))
	case "nemes":
		for y := y0; y <= y1+3; y++ {
			for x := x0 - 1; x <= x1+1; x++ {
				inner := x >= x0+1 && x <= x1-1 && y >= y0+2 && y <= y1
				if inner || (y > y1 && x > x0 && x < x1) {
					continue
				}
				c := cGold
				if (y-y0)%2 == 1 {
					c = hc
				}
				if x > x1-1 {
					c = mul(c, 0.75)
				}
				p.set(x, y, c)
			}
		}
		p.set((x0+x1)/2, y0-1, cGold)
	case "turban":
		for y := y0 - 1; y <= y0+2; y++ {
			for x := x0 - 1; x <= x1+1; x++ {
				if y == y0-1 && (x < x0+1 || x > x1-1) {
					continue
				}
				c := hc
				if (x+y)%3 == 0 {
					c = mul(hc, 0.82)
				}
				if x > x1 {
					c = mul(c, 0.75)
				}
				p.set(x, y, c)
			}
		}
		p.set((x0+x1)/2, y0, hex("#e03050"))
	case "cap":
		for x := x0 - 1; x <= x1+1; x++ {
			p.set(x, y0+1, mul(hc, 0.85))
		}
		for y := y0 - 1; y <= y0; y++ {
			for x := x0; x <= x1; x++ {
				p.set(x, y, mul(hc, 1-0.25*float64(x-x0)/float64(x1-x0)))
			}
		}
	case "snout":
		// reptile: darker scaly crown and yellow slit eyes
		for x := x0; x <= x1; x++ {
			p.set(x, y0, mul(skin, 0.75))
			p.set(x, y0+1, mul(skin, 0.85))
		}
		p.set(ex1, eyesY, hex("#f0e040"))
		p.set(ex2, eyesY, hex("#f0e040"))
		p.set((x0+x1)/2, y1, mul(skin, 0.6))
		p.set((x0+x1)/2+1, y1, mul(skin, 0.6))
	case "circlet":
		for x := x0; x <= x1; x++ {
			p.set(x, y0+1, mul(hc, 1-0.2*float64(x-x0)/float64(x1-x0)))
		}
		p.set((x0+x1)/2, y0+1, hex("#7ad0ff"))
		p.set((x0+x1)/2+1, y0+1, mul(hex("#7ad0ff"), 0.8))
	case "mitre":
		mid := (x0 + x1 + 1) / 2
		for y := y0 - 4; y <= y0+1; y++ {
			half := 2 + (y-(y0-4))/2
			for x := mid - half; x < mid+half; x++ {
				c := hc
				if x >= mid+half-1 {
					c = mul(hc, 0.75)
				}
				p.set(x, y, c)
			}
		}
		for y := y0 - 3; y <= y0+1; y++ { // golden band
			p.set(mid-1, y, cGold)
		}
		for x := mid - 3; x < mid+3; x++ {
			p.set(x, y0, mul(cGold, 0.9))
		}
	case "fur_hat":
		for y := y0 - 1; y <= y0+2; y++ {
			for x := x0 - 1; x <= x1+1; x++ {
				if y == y0-1 && (x < x0 || x > x1) {
					continue
				}
				c := hc
				if (x*7+y*3)%4 == 0 {
					c = mul(hc, 1.15)
				}
				if x >= x1 {
					c = mul(c, 0.78)
				}
				p.set(x, y, c)
			}
		}
	case "ears":
		p.set(x0-1, y0+2, skin)
		p.set(x0-2, y0+1, mul(skin, 0.9))
		p.set(x1+1, y0+2, mul(skin, 0.8))
		p.set(x1+2, y0+1, mul(skin, 0.7))
	}
	if h.crown {
		for x := x0; x <= x1; x++ {
			p.set(x, y0, cGold)
		}
		for x := x0; x <= x1; x += 2 {
			p.set(x, y0-1, mul(cGold, 1.1))
		}
		p.set((x0+x1)/2, y0, hex("#e03050"))
	}
}

func paintWings(p *pc, g geom, c color.RGBA, frame int) {
	t := g.torso
	lift := 0
	if frame == 1 {
		lift = -1
	}
	span := 5 + (g.w-16)/3
	for i := 0; i < span; i++ {
		y := t.Min.Y - 3 + i + lift
		reach := span - i
		for x := t.Min.X - 1 - reach; x < t.Min.X; x++ {
			k := 0.85 + 0.04*float64(i)
			if x == t.Min.X-1-reach {
				k = 1.15 // leading edge
			}
			p.set(x, y, mul(c, k))
		}
		for x := t.Max.X; x < t.Max.X+1+reach; x++ {
			k := 0.7 + 0.04*float64(i)
			if x == t.Max.X+reach {
				k = 1
			}
			p.set(x, y, mul(c, k))
		}
	}
	// scalloped lower edge
	for i := 0; i < span; i += 2 {
		p.clear(t.Min.X-span+i, t.Min.Y-3+span-i/2+lift)
		p.clear(t.Max.X+span-1-i, t.Min.Y-3+span-i/2+lift)
	}
}

func paintShield(p *pc, g geom, arm image.Rectangle, c, mark color.RGBA, form string) {
	x0 := arm.Min.X - 2
	y0 := arm.Min.Y + 1
	w, hh := 5, 6
	if g.w >= 20 {
		w, hh = 6, 8
	}
	switch form {
	case "tower":
		y0 -= 2
		hh += 3
	case "round":
		w, hh = w+1, w+1
	}
	for y := y0; y < y0+hh; y++ {
		for x := x0; x < x0+w; x++ {
			switch form {
			case "tower":
				if (y == y0 || y == y0+hh-1) && (x == x0 || x == x0+w-1) {
					continue
				}
			case "round":
				dx, dy := float64(x-x0)-float64(w-1)/2, float64(y-y0)-float64(hh-1)/2
				if dx*dx+dy*dy > float64(w*w)/4+0.3 {
					continue
				}
			default:
				bottom := y >= y0+hh-2 && (x == x0 || x == x0+w-1)
				if bottom || (y == y0+hh-1 && (x == x0+1 || x == x0+w-2)) {
					continue
				}
			}
			k := 1.0
			if x == x0+w-1 {
				k = 0.75
			}
			if x == x0 || y == y0 {
				k = 1.15
			}
			p.set(x, y, mul(c, k))
		}
	}
	if form == "round" {
		p.set(x0+w/2, y0+hh/2, mul(cSteel, 1.1)) // boss
	}
	if mark.A > 0 {
		mx, my := x0+w/2, y0+hh/2-1
		p.set(mx, my-1, mark)
		p.set(mx, my, mark)
		p.set(mx, my+1, mark)
		p.set(mx-1, my, mark)
		p.set(mx+1, my, mark)
	}
}

func paintWeapon(p *pc, g geom, arm image.Rectangle, kind string, tint color.RGBA) {
	hx := arm.Max.X // just right of the hand
	hy := arm.Max.Y // hand row
	blade := cSteel
	if tint.A > 0 {
		blade = tint
	}
	switch kind {
	case "sword":
		for y := hy - 8; y < hy; y++ {
			p.set(hx, y, mul(blade, 1.1))
			p.set(hx+1, y, mul(blade, 0.8))
		}
		p.set(hx, hy-9, mul(blade, 1.2))
		p.set(hx-1, hy, cGold)
		p.set(hx, hy, cGold)
		p.set(hx+1, hy, cGold)
		p.set(hx+2, hy, mul(cGold, 0.8))
		p.set(hx, hy+1, cWood)
	case "dagger":
		for y := hy - 4; y < hy; y++ {
			p.set(hx, y, mul(blade, 1.05))
		}
		p.set(hx-1, hy, cIron)
		p.set(hx+1, hy, cIron)
		p.set(hx, hy+1, cWood)
	case "axe":
		for y := hy - 8; y <= hy+1; y++ {
			p.set(hx, y, cWood)
		}
		for y := hy - 8; y <= hy-5; y++ {
			p.set(hx+1, y, mul(blade, 1.05))
			p.set(hx+2, y, mul(blade, 0.85))
		}
		p.set(hx+3, hy-7, mul(blade, 0.75))
		p.set(hx+3, hy-6, mul(blade, 0.75))
		p.set(hx-1, hy-7, mul(blade, 0.9))
	case "mace", "hammer", "club":
		for y := hy - 6; y <= hy+1; y++ {
			p.set(hx, y, cWood)
		}
		head := blade
		if kind == "club" {
			head = mul(cWood, 1.1)
		}
		for y := hy - 9; y <= hy-6; y++ {
			for x := hx - 1; x <= hx+1; x++ {
				p.set(x, y, mul(head, 1.1-0.15*float64(x-hx+1)))
			}
		}
		if kind == "hammer" {
			p.set(hx+2, hy-8, mul(head, 0.8))
			p.set(hx+2, hy-7, mul(head, 0.8))
			p.set(hx-2, hy-8, head)
			p.set(hx-2, hy-7, head)
		}
	case "spear":
		for y := hy - 12; y <= hy+3; y++ {
			p.set(hx, y, cWood)
		}
		p.set(hx, hy-13, blade)
		p.set(hx, hy-14, mul(blade, 1.2))
		p.set(hx-1, hy-12, mul(blade, 0.8))
		p.set(hx+1, hy-12, mul(blade, 0.8))
	case "staff", "skullstaff", "totem":
		top := max(1, hy-13)
		for y := top; y <= hy+3; y++ {
			p.set(hx, y, mul(cWood, 1.1))
			p.set(hx+1, y, mul(cWood, 0.75))
		}
		gem := tint
		if gem.A == 0 {
			gem = hex("#6ab0ff")
		}
		switch kind {
		case "skullstaff":
			p.set(hx, top-1, cBone)
			p.set(hx+1, top-1, mul(cBone, 0.8))
			p.set(hx, top-2, cBone)
			p.set(hx+1, top-2, mul(cBone, 0.8))
			p.set(hx, top-1, gem)
		case "totem":
			p.set(hx-1, top, gem)
			p.set(hx+2, top, gem)
			p.set(hx-1, top+1, cWhite)
			p.set(hx+2, top+2, hex("#e04a3a"))
		default:
			p.set(hx, top-1, mul(gem, 1.2))
			p.set(hx+1, top-1, gem)
			p.set(hx, top-2, gem)
			p.set(hx+1, top-2, mul(gem, 0.7))
		}
	case "wand":
		for y := hy - 4; y <= hy+1; y++ {
			p.set(hx, y, cWood)
		}
		gem := tint
		if gem.A == 0 {
			gem = hex("#c080ff")
		}
		p.set(hx, hy-5, gem)
		p.set(hx, hy-6, mul(gem, 0.8))
	case "scythe":
		for y := hy - 12; y <= hy+3; y++ {
			p.set(hx, y, mul(cWood, 0.8))
		}
		for x := hx - 5; x <= hx; x++ {
			p.set(x, hy-12, mul(blade, 1.1))
		}
		p.set(hx-5, hy-11, blade)
		p.set(hx-6, hy-10, mul(blade, 0.8))
	case "bow":
		// held in the other hand (viewer's left)
		bx := 1
		if g.w >= 20 {
			bx = 0
		}
		top, bot := hy-8, hy+2
		for y := top; y <= bot; y++ {
			x := bx
			if y == top || y == bot {
				x = bx + 2
			} else if y == top+1 || y == bot-1 {
				x = bx + 1
			}
			p.set(x, y, mul(cWood, 1.1))
			p.set(bx+2, y, alpha(cWhite, 150))
		}
	case "crossbow":
		// stock pointing forward, bow arms across its end
		for x := hx - 1; x <= hx+3; x++ {
			p.set(x, hy-1, mul(cWood, 1.1-0.08*float64(x-hx)))
		}
		p.set(hx-1, hy, mul(cWood, 0.8))
		for y := hy - 4; y <= hy+2; y++ {
			k := 1.0
			if y == hy-4 || y == hy+2 {
				k = 0.8
			}
			x := hx + 3
			if y == hy-4 || y == hy+2 {
				x = hx + 2
			}
			p.set(x, y, mul(blade, k))
		}
		p.set(hx+1, hy-3, alpha(cWhite, 150))
		p.set(hx+1, hy+1, alpha(cWhite, 150))
		p.set(hx+4, hy-1, mul(cSteel, 1.2))
	case "claws":
		claw := hex("#e8e0d0")
		left := arm.Min.X - (g.torso.Dx() + 2*arm.Dx())
		for _, x := range []int{arm.Min.X, arm.Max.X - 1, left, left + arm.Dx() - 1} {
			p.set(x, hy+1, claw)
		}
	}
}

// cropTop removes fully transparent rows above the picture.
func cropTop(p *pc) {
	y0 := 0
	for ; y0 < p.h-1; y0++ {
		empty := true
		for x := 0; x < p.w; x++ {
			if p.get(x, y0).A > 0 {
				empty = false
				break
			}
		}
		if !empty {
			break
		}
	}
	if y0 == 0 {
		return
	}
	img := image.NewRGBA(image.Rect(0, 0, p.w, p.h-y0))
	for y := y0; y < p.h; y++ {
		for x := 0; x < p.w; x++ {
			img.SetRGBA(x, y-y0, p.get(x, y))
		}
	}
	p.img, p.h = img, p.h-y0
}

// ---- hand-drawn beasts ----

// sprite parses a pixel map with a palette ('.' and ' ' are transparent).
func sprite(rows []string, pal map[rune]color.RGBA) *pc {
	w := 0
	for _, r := range rows {
		w = max(w, len([]rune(r)))
	}
	p := newPC(w, len(rows), "map")
	for y, row := range rows {
		for x, ch := range []rune(row) {
			if c, ok := pal[ch]; ok {
				p.set(x, y, c)
			}
		}
	}
	return p
}

// pal builds the palette of a beast from its main colour: a (base), A (dark),
// w (light), plus fixed detail colours.
func pal(main, second color.RGBA) map[rune]color.RGBA {
	if second.A == 0 {
		second = mix(main, cWhite, 0.45)
	}
	return map[rune]color.RGBA{
		'a': main, 'A': mul(main, 0.68), 'w': mix(main, cWhite, 0.35), 'd': mul(main, 0.45),
		'b': second, 'B': mul(second, 0.72),
		'K': cDarkK, 'W': cWhite, 'r': hex("#d83a3a"), 'y': hex("#f0e6b0"), 'e': hex("#ffd84a"),
		'g': hex("#9ae04a"), 'o': hex("#ff8a2a"), 'p': hex("#f08aa0"), 'c': hex("#8ad8ff"),
	}
}

// outlined adds a one pixel dark outline around the opaque pixels.
func outlined(p *pc) *pc {
	o := newPC(p.w+2, p.h+2, "outline")
	for y := 0; y < p.h; y++ {
		for x := 0; x < p.w; x++ {
			if c := p.get(x, y); c.A > 0 {
				o.img.SetRGBA(x+1, y+1, c)
			}
		}
	}
	for y := 0; y < o.h; y++ {
		for x := 0; x < o.w; x++ {
			if o.get(x, y).A > 0 {
				continue
			}
			var sum [3]float64
			n := 0
			for _, d := range [][2]int{{-1, 0}, {1, 0}, {0, -1}, {0, 1}} {
				sx, sy := x-1+d[0], y-1+d[1] // neighbour in the source image
				if sx < 0 || sy < 0 || sx >= p.w || sy >= p.h {
					continue
				}
				if c := p.get(sx, sy); c.A > 120 {
					sum[0] += float64(c.R)
					sum[1] += float64(c.G)
					sum[2] += float64(c.B)
					n++
				}
			}
			if n > 0 {
				k := 0.22 / float64(n)
				o.img.SetRGBA(x, y, color.RGBA{uint8(sum[0] * k), uint8(sum[1] * k), uint8(sum[2] * k), 235})
			}
		}
	}
	return o
}

// paintOffhand draws what is held in the left hand: a second weapon (the
// right-hand painter mirrored) or a book, orb or holy symbol.
func paintOffhand(p *pc, g geom, armL, armR image.Rectangle, kind string, c color.RGBA) {
	hx, hy := armL.Min.X-1, armL.Max.Y
	switch kind {
	case "book":
		cover := c
		if cover.A == 0 {
			cover = hex("#7a3a2a")
		}
		p.box(image.Rect(hx-2, hy-2, hx+2, hy+2), cover)
		for y := hy - 1; y <= hy; y++ {
			p.set(hx+1, y, hex("#f0e8d0"))
		}
		p.set(hx-1, hy-1, cGold)
	case "orb":
		orb := c
		if orb.A == 0 {
			orb = hex("#8ac0ff")
		}
		p.ball(float64(hx)-0.5, float64(hy)-1.5, 1.8, orb, 0)
		p.set(hx-1, hy-2, cWhite)
	case "symbol":
		sym := c
		if sym.A == 0 {
			sym = cGold
		}
		for y := hy - 3; y <= hy+1; y++ {
			p.set(hx, y, sym)
		}
		p.set(hx-1, hy-2, sym)
		p.set(hx+1, hy-2, mul(sym, 0.8))
		p.set(hx, hy-4, alpha(cWhite, 180))
	default:
		// a mirrored copy of the right-hand weapon
		tmp := newPC(p.w, p.h, "off")
		mirror := image.Rect(p.w-armL.Max.X, armL.Min.Y, p.w-armL.Min.X, armL.Max.Y)
		paintWeapon(tmp, g, mirror, kind, c)
		for y := 0; y < p.h; y++ {
			for x := 0; x < p.w; x++ {
				if px := tmp.get(x, y); px.A > 0 {
					p.set(p.w-1-x, y, mul(px, 0.9))
				}
			}
		}
	}
}
