package gfx

import (
	"hash/fnv"
	"image"
	"image/color"
	"math"
	"math/rand/v2"
	"strconv"

	"github.com/hajimehoshi/ebiten/v2"
	"github.com/hajimehoshi/ebiten/v2/text/v2"

	"ratas/internal/content"
)

// All tile art is generated procedurally at 16x16 ("tall" objects 16x24) from
// the tile colours in the content files, so modded tiles get art too.
const spx = 16

type spriteSet struct {
	ground [][]*ebiten.Image // [variant][animation frame]
	object []*ebiten.Image   // variants drawn over a ground tile
	tall   bool              // object is 16x24 and depth-sorted with entities
	wall   bool              // tall block that fully covers its tile
}

// ---- colours ----

func hex(s string) color.RGBA {
	if len(s) == 7 && s[0] == '#' {
		v, err := strconv.ParseUint(s[1:], 16, 32)
		if err == nil {
			return color.RGBA{uint8(v >> 16), uint8(v >> 8), uint8(v), 255}
		}
	}
	return color.RGBA{128, 128, 128, 255}
}

func clampU8(v float64) uint8 { return uint8(math.Max(0, math.Min(255, v))) }

func mul(c color.RGBA, k float64) color.RGBA {
	return color.RGBA{clampU8(float64(c.R) * k), clampU8(float64(c.G) * k), clampU8(float64(c.B) * k), c.A}
}

func mix(a, b color.RGBA, t float64) color.RGBA {
	return color.RGBA{
		clampU8(float64(a.R)*(1-t) + float64(b.R)*t), clampU8(float64(a.G)*(1-t) + float64(b.G)*t),
		clampU8(float64(a.B)*(1-t) + float64(b.B)*t), clampU8(float64(a.A)*(1-t) + float64(b.A)*t),
	}
}

// alpha sets a straight (non-premultiplied) alpha; used by the pixel canvas.
func alpha(c color.RGBA, a uint8) color.RGBA { return color.RGBA{c.R, c.G, c.B, a} }

// pre returns c with opacity a as a premultiplied colour, as Ebiten expects.
func pre(c color.RGBA, a float64) color.RGBA {
	a = math.Max(0, math.Min(1, a))
	return color.RGBA{uint8(float64(c.R) * a), uint8(float64(c.G) * a), uint8(float64(c.B) * a), uint8(255 * a)}
}

var (
	black = color.RGBA{0, 0, 0, 255}
	white = color.RGBA{255, 255, 255, 255}
	bark  = color.RGBA{96, 64, 38, 255}
	stone = color.RGBA{128, 128, 140, 255}
)

// ---- pixel canvas ----

type pc struct {
	img  *image.RGBA
	w, h int
	r    *rand.Rand
}

func newPC(w, h int, seed string) *pc {
	f := fnv.New64a()
	f.Write([]byte(seed))
	return &pc{img: image.NewRGBA(image.Rect(0, 0, w, h)), w: w, h: h, r: rand.New(rand.NewPCG(f.Sum64(), 99))}
}

func (p *pc) set(x, y int, c color.RGBA) {
	if x < 0 || y < 0 || x >= p.w || y >= p.h || c.A == 0 {
		return
	}
	if c.A == 255 {
		p.img.SetRGBA(x, y, c)
		return
	}
	o := p.img.RGBAAt(x, y)
	t := float64(c.A) / 255
	n := mix(o, color.RGBA{c.R, c.G, c.B, 255}, t)
	n.A = clampU8(float64(o.A) + float64(c.A)*(1-float64(o.A)/255))
	p.img.SetRGBA(x, y, n)
}

func (p *pc) get(x, y int) color.RGBA { return p.img.RGBAAt(x, y) }

func (p *pc) fill(c color.RGBA) { p.rect(0, 0, p.w, p.h, c) }

func (p *pc) rect(x, y, w, h int, c color.RGBA) {
	for yy := y; yy < y+h; yy++ {
		for xx := x; xx < x+w; xx++ {
			p.set(xx, yy, c)
		}
	}
}

func (p *pc) circle(cx, cy, r float64, c color.RGBA) {
	for y := 0; y < p.h; y++ {
		for x := 0; x < p.w; x++ {
			dx, dy := float64(x)+0.5-cx, float64(y)+0.5-cy
			if dx*dx+dy*dy <= r*r {
				p.set(x, y, c)
			}
		}
	}
}

// ball draws a shaded sphere lit from the top-left.
func (p *pc) ball(cx, cy, r float64, c color.RGBA, jitter float64) {
	for y := 0; y < p.h; y++ {
		for x := 0; x < p.w; x++ {
			dx, dy := (float64(x)+0.5-cx)/r, (float64(y)+0.5-cy)/r
			if dx*dx+dy*dy > 1 {
				continue
			}
			l := 0.95 - 0.38*(dx+dy) + (p.r.Float64()-0.5)*jitter
			p.set(x, y, mul(c, l))
		}
	}
}

// noise jitters the brightness of opaque pixels.
func (p *pc) noise(amount float64) {
	for y := 0; y < p.h; y++ {
		for x := 0; x < p.w; x++ {
			c := p.get(x, y)
			if c.A == 0 {
				continue
			}
			p.img.SetRGBA(x, y, mul(c, 1+(p.r.Float64()-0.5)*amount))
		}
	}
}

func (p *pc) speckle(c color.RGBA, density float64) {
	for y := 0; y < p.h; y++ {
		for x := 0; x < p.w; x++ {
			if p.r.Float64() < density {
				p.set(x, y, c)
			}
		}
	}
}

func (p *pc) line(x0, y0, x1, y1 int, c color.RGBA) {
	dx, dy := abs(x1-x0), -abs(y1-y0)
	sx, sy := 1, 1
	if x0 > x1 {
		sx = -1
	}
	if y0 > y1 {
		sy = -1
	}
	err := dx + dy
	for {
		p.set(x0, y0, c)
		if x0 == x1 && y0 == y1 {
			return
		}
		e2 := 2 * err
		if e2 >= dy {
			err += dy
			x0 += sx
		}
		if e2 <= dx {
			err += dx
			y0 += sy
		}
	}
}

func abs(v int) int {
	if v < 0 {
		return -v
	}
	return v
}

func (p *pc) image() *ebiten.Image { return ebiten.NewImageFromImage(p.img) }

// ---- tile art ----

func tileColors(def *content.TileDef) (fg, bg color.RGBA) {
	fg = hex(def.FG)
	if def.BG != "" {
		bg = hex(def.BG)
	} else {
		bg = mul(fg, 0.3)
	}
	return
}

func grassBase(p *pc, fg color.RGBA, blades int) {
	p.fill(mul(fg, 0.46))
	p.noise(0.07)
	p.speckle(mul(fg, 0.56), 0.18)
	p.speckle(mul(fg, 0.7), 0.05)
	for i := 0; i < blades; i++ {
		x, y := p.r.IntN(spx), 2+p.r.IntN(spx-3)
		c := mul(fg, 0.85+p.r.Float64()*0.35)
		p.set(x, y, c)
		p.set(x, y-1, mul(c, 1.15))
	}
}

func waterFrame(p *pc, fg, bg color.RGBA, frame int, deep bool) {
	p.fill(bg)
	p.noise(0.08)
	n := 5
	if deep {
		n = 3
	}
	rr := rand.New(rand.NewPCG(7, 11))
	for i := 0; i < n; i++ {
		x := rr.IntN(spx)
		y := rr.IntN(spx)
		x = (x + frame*2 + y/4) % spx
		c := alpha(mix(bg, fg, 0.85), 220)
		for k := 0; k < 3; k++ {
			p.set((x+k)%spx, y, c)
		}
		p.set((x+1)%spx, (y+spx-1)%spx, alpha(white, 70))
	}
}

func lavaFrame(p *pc, fg, bg color.RGBA, frame int) {
	p.fill(bg)
	rr := rand.New(rand.NewPCG(3, 5))
	for i := 0; i < 7; i++ {
		x, y := rr.IntN(spx), rr.IntN(spx)
		r := 1.5 + rr.Float64()*2
		ph := math.Sin(float64(frame)*math.Pi/2 + float64(i))
		p.circle(float64(x), float64(y), r+ph*0.6, mul(fg, 0.9+0.2*ph))
	}
	for i := 0; i < 5; i++ {
		x, y := rr.IntN(spx), rr.IntN(spx)
		p.set((x+frame)%spx, y, color.RGBA{255, 230, 120, 255})
	}
}

func planks(p *pc, c color.RGBA, vertical bool) {
	for y := 0; y < spx; y++ {
		for x := 0; x < spx; x++ {
			k := x
			if !vertical {
				k = y
			}
			board := k / 4
			shade := 0.9 + 0.12*float64(board%2)
			col := mul(c, shade+(p.r.Float64()-0.5)*0.08)
			if k%4 == 3 {
				col = mul(c, 0.55)
			}
			p.set(x, y, col)
		}
	}
}

func flagstones(p *pc, c color.RGBA) {
	for by := 0; by < 2; by++ {
		for bx := 0; bx < 2; bx++ {
			sh := 0.85 + p.r.Float64()*0.25
			p.rect(bx*8, by*8, 8, 8, mul(c, sh))
		}
	}
	p.noise(0.14)
	mortar := mul(c, 0.5)
	for i := 0; i < spx; i++ {
		p.set(i, 7, mortar)
		p.set(7, i, mortar)
		p.set(i, 15, mul(c, 0.6))
		p.set(15, i, mul(c, 0.6))
	}
}

func groundArt(key string, fg, bg color.RGBA, v, frame int) *pc {
	p := newPC(spx, spx, key+strconv.Itoa(v))
	switch key {
	case "grass", "grass2":
		grassBase(p, fg, 4)
	case "tall_grass":
		grassBase(p, fg, 2)
		for i := 0; i < 9; i++ {
			x, y := p.r.IntN(spx), 4+p.r.IntN(spx-4)
			for k := 0; k < 3; k++ {
				p.set(x, y-k, mul(fg, 0.75+0.15*float64(k)))
			}
		}
	case "flowers":
		grassBase(p, hex("#5fa043"), 3)
		for i := 0; i < 3; i++ {
			x, y := 2+p.r.IntN(12), 2+p.r.IntN(12)
			petal := fg
			if i == 1 {
				petal = color.RGBA{240, 240, 255, 255}
			}
			p.set(x-1, y, petal)
			p.set(x+1, y, petal)
			p.set(x, y-1, petal)
			p.set(x, y+1, petal)
			p.set(x, y, color.RGBA{255, 220, 60, 255})
		}
	case "forest_floor":
		p.fill(mix(mul(fg, 0.5), color.RGBA{60, 45, 25, 255}, 0.35))
		p.noise(0.2)
		p.speckle(color.RGBA{90, 70, 40, 255}, 0.08)
		p.speckle(mul(fg, 0.8), 0.07)
	case "sand":
		p.fill(mul(fg, 0.82))
		p.noise(0.1)
		p.speckle(mul(fg, 0.95), 0.12)
		p.speckle(mul(fg, 0.65), 0.05)
	case "swamp", "reeds":
		p.fill(mix(bg, mul(fg, 0.45), 0.5))
		p.noise(0.18)
		for i := 0; i < 2; i++ {
			x, y := 2+p.r.IntN(11), 2+p.r.IntN(11)
			p.rect(x, y, 3, 2, color.RGBA{28, 48, 44, 255})
			p.set(x, y, color.RGBA{70, 100, 90, 255})
		}
		p.speckle(mul(fg, 0.75), 0.07)
		if key == "reeds" {
			for i := 0; i < 6; i++ {
				x, y := 1+p.r.IntN(14), 7+p.r.IntN(8)
				h := 4 + p.r.IntN(4)
				p.line(x, y, x, y-h, mul(fg, 0.9))
				p.set(x, y-h, color.RGBA{110, 80, 45, 255})
			}
		}
	case "hill":
		p.fill(mix(mul(fg, 0.5), color.RGBA{60, 90, 45, 255}, 0.45))
		p.noise(0.14)
		for y := 0; y < spx; y++ {
			for x := 0; x < spx; x++ {
				dx, dy := (float64(x)-7.5)/7, (float64(y)-9)/5
				if d := dx*dx + dy*dy; d < 1 {
					p.set(x, y, alpha(mul(fg, 0.95-0.35*dy), uint8(60*(1-d))))
				}
			}
		}
	case "road":
		p.fill(mul(fg, 0.6))
		p.noise(0.16)
		p.speckle(mul(fg, 0.8), 0.06)
		p.speckle(mul(fg, 0.4), 0.05)
	case "cobblestone", "city_gate":
		// rounded cobbles in staggered rows
		p.fill(mul(fg, 0.38))
		for row := 0; row < 4; row++ {
			off := (row % 2) * 2
			for col := -1; col < 4; col++ {
				cx := float64(col*4+off) + 1.5
				cy := float64(row*4) + 1.5
				sh := 0.7 + p.r.Float64()*0.3
				p.ball(cx, cy, 1.7, mul(fg, sh), 0.15)
			}
		}
		if key == "city_gate" {
			// iron-shod planks of the open gate
			for x := 0; x < spx; x++ {
				p.set(x, 0, mul(bg, 1.6))
				p.set(x, spx-1, mul(bg, 1.6))
			}
			p.rect(0, 0, 2, spx, mul(fg, 0.5))
			p.rect(spx-2, 0, 2, spx, mul(fg, 0.5))
		}
	case "garden":
		grassBase(p, hex("#5a9a3a"), 4)
		flowers := []color.RGBA{hex("#ff7a9a"), hex("#ffe070"), hex("#b080ff"), hex("#ffffff")}
		for i := 0; i < 7; i++ {
			x, y := 1+p.r.IntN(14), 1+p.r.IntN(14)
			p.set(x, y, flowers[(i+v)%len(flowers)])
			p.set(x, y+1, hex("#3a7a2a"))
		}
		for i := 0; i < spx; i++ {
			p.set(i, 0, hex("#8a6a4a"))
			p.set(i, spx-1, hex("#8a6a4a"))
		}
	case "bridge":
		planks(p, fg, false)
		p.rect(0, 0, spx, 1, mul(fg, 0.45))
		p.rect(0, spx-1, spx, 1, mul(fg, 0.45))
	case "water", "deep_water":
		waterFrame(p, fg, bg, frame, key == "deep_water")
	case "lava":
		lavaFrame(p, fg, bg, frame)
	case "house_floor":
		planks(p, fg, true)
	case "stone_floor", "rubble", "bones":
		flagstones(p, mul(stone, 0.55))
		if key == "rubble" {
			for i := 0; i < 5; i++ {
				x, y := p.r.IntN(14), p.r.IntN(14)
				p.rect(x, y, 2, 2, mul(fg, 0.9+p.r.Float64()*0.3))
				p.set(x, y, mul(fg, 1.3))
			}
		}
		if key == "bones" {
			bone := color.RGBA{220, 220, 205, 255}
			p.line(3, 11, 10, 8, bone)
			p.set(2, 11, bone)
			p.set(3, 12, bone)
			p.set(10, 7, bone)
			p.set(11, 8, bone)
			p.circle(11.5, 12.5, 2.2, bone)
			p.set(11, 12, black)
			p.set(12, 12, black)
		}
	case "cave_floor":
		p.fill(mul(fg, 0.62))
		p.noise(0.22)
		p.speckle(mul(fg, 0.42), 0.08)
		p.speckle(mul(fg, 0.85), 0.04)
	case "door_open":
		planks(p, color.RGBA{120, 92, 60, 255}, true)
		p.rect(0, 0, 2, spx, mul(fg, 0.55))
		p.rect(14, 0, 2, spx, mul(fg, 0.55))
	case "stairs_down", "stairs_up":
		p.fill(mul(stone, 0.45))
		for i := 0; i < 4; i++ {
			k := float64(i) / 3
			if key == "stairs_down" {
				k = 1 - k
			}
			c := mul(stone, 0.35+0.6*k)
			p.rect(2+i, 2+i*3, 12-2*i, 3, c)
			p.rect(2+i, 2+i*3, 12-2*i, 1, mul(c, 1.25))
		}
	case "dungeon":
		grassBase(p, hex("#4c8a35"), 2)
		p.circle(8, 9, 6, color.RGBA{60, 52, 48, 255})
		p.circle(8, 9.5, 4.6, color.RGBA{10, 6, 6, 255})
		for x := 4; x <= 12; x++ {
			p.set(x, 13, alpha(fg, 200))
		}
	case "void":
		p.fill(black)
	case "desert_sand", "dunes":
		p.fill(mul(fg, 0.86))
		p.noise(0.06)
		ph := float64(v) * 2
		for y := 0; y < spx; y++ {
			for x := 0; x < spx; x++ {
				w := math.Sin((float64(x)*0.45+float64(y)*1.1+ph)*0.9) * 0.5
				if key == "dunes" {
					w = math.Sin((float64(x)*0.25+float64(y)*0.7+ph)*0.8) * 0.9
				}
				if w > 0.35 {
					p.set(x, y, mul(fg, 0.97+w*0.12))
				} else if w < -0.42 {
					p.set(x, y, mul(fg, 0.74))
				}
			}
		}
		p.speckle(mul(fg, 0.62), 0.03)
	case "snow_ground", "snowdrift":
		p.fill(mul(fg, 0.93))
		p.noise(0.04)
		p.speckle(color.RGBA{196, 212, 236, 255}, 0.08)
		p.speckle(white, 0.03)
		if key == "snowdrift" {
			for y := 0; y < spx; y++ {
				for x := 0; x < spx; x++ {
					dx, dy := (float64(x)-7.5)/7, (float64(y)-9)/5
					if d := dx*dx + dy*dy; d < 1 {
						p.set(x, y, mul(fg, 1-0.15*dy-0.08*dx))
					}
				}
			}
			p.speckle(color.RGBA{200, 216, 240, 255}, 0.04)
		}
	case "ice", "ice_floor":
		base := mix(fg, bg, 0.45)
		if key == "ice_floor" {
			base = mul(fg, 0.78)
		}
		p.fill(base)
		p.noise(0.05)
		for i := 0; i < 3; i++ {
			x0, y0 := p.r.IntN(spx), p.r.IntN(spx)
			p.line(x0, y0, x0+p.r.IntN(9)-4, y0+p.r.IntN(9)-4, mix(base, white, 0.45))
		}
		for i := 0; i < 4; i++ {
			x := p.r.IntN(spx - 3)
			y := p.r.IntN(spx - 3)
			p.set(x, y+2, alpha(white, 140))
			p.set(x+1, y+1, alpha(white, 170))
			p.set(x+2, y, alpha(white, 140))
		}
	case "ash", "magma_crack":
		p.fill(mul(fg, 0.72))
		p.noise(0.18)
		p.speckle(mul(fg, 0.5), 0.1)
		p.speckle(mul(fg, 1.05), 0.06)
		if v == 1 {
			p.set(p.r.IntN(spx), p.r.IntN(spx), color.RGBA{255, 120, 40, 255})
		}
		if key == "magma_crack" {
			x, y := p.r.IntN(4), p.r.IntN(spx)
			for i := 0; i < 14; i++ {
				c := color.RGBA{255, 110 + uint8(p.r.IntN(80)), 30, 255}
				p.set(x, y, c)
				p.set(x, y+1, mul(c, 0.6))
				x++
				y += p.r.IntN(3) - 1
			}
		}
	case "blight_grass", "mushrooms":
		grassBase(p, hex("#6a6070"), 3)
		p.speckle(color.RGBA{40, 30, 44, 255}, 0.06)
		if key == "mushrooms" {
			for i := 0; i < 3; i++ {
				x, y := 2+p.r.IntN(11), 4+p.r.IntN(10)
				p.set(x, y, color.RGBA{230, 225, 210, 255})
				p.set(x, y-1, color.RGBA{230, 225, 210, 255})
				p.set(x-1, y-2, fg)
				p.set(x, y-2, mix(fg, white, 0.4))
				p.set(x+1, y-2, mul(fg, 0.8))
			}
		}
	case "web":
		p.fill(color.RGBA{58, 48, 42, 255})
		p.noise(0.2)
		wc := alpha(fg, 200)
		for i := 0; i < 4; i++ {
			a := float64(i)*math.Pi/4 + float64(v)
			p.line(8-int(math.Cos(a)*8), 8-int(math.Sin(a)*8), 8+int(math.Cos(a)*8), 8+int(math.Sin(a)*8), wc)
		}
		for r := 2.5; r < 8; r += 2.5 {
			for i := 0; i < 16; i++ {
				a := float64(i) / 16 * 2 * math.Pi
				p.set(8+int(math.Cos(a)*r), 8+int(math.Sin(a)*r), alpha(fg, 150))
			}
		}
	case "basalt_floor":
		p.fill(mul(fg, 0.7))
		p.noise(0.16)
		for i := 0; i < 3; i++ {
			x, y := p.r.IntN(spx), p.r.IntN(spx)
			p.line(x, y, x+3, y+1, mul(fg, 0.45))
		}
		if v == 2 {
			p.set(p.r.IntN(spx), p.r.IntN(spx), color.RGBA{220, 80, 30, 255})
		}
	case "sandstone_floor", "dark_floor":
		flagstones(p, mul(fg, 0.85))
	case "carpet":
		flagstones(p, hex("#4a3c4c"))
		p.rect(0, 2, spx, 12, mul(fg, 0.8))
		p.noise(0.05)
		p.rect(0, 3, spx, 1, color.RGBA{220, 180, 80, 255})
		p.rect(0, 12, spx, 1, color.RGBA{220, 180, 80, 255})
		for x := 2; x < spx; x += 5 {
			p.set(x, 7, color.RGBA{220, 180, 80, 255})
			p.set(x+1, 8, color.RGBA{220, 180, 80, 255})
		}
	default:
		p.fill(bg)
		p.noise(0.1)
	}
	return p
}

// objectArt draws a 16x16 or 16x24 object with a transparent background.
func objectArt(key string, fg, bg color.RGBA, v int) (*pc, bool, bool) {
	switch key {
	case "tree":
		p := newPC(spx, 24, key+strconv.Itoa(v))
		p.circle(8, 22, 4.5, alpha(black, 70))
		p.rect(7, 15, 2, 8, bark)
		p.rect(8, 15, 1, 8, mul(bark, 0.7))
		r := 6.2 + float64(v%3)*0.5
		p.ball(8, 9+float64(v%2), r, mul(fg, 0.95), 0.25)
		p.ball(5.5, 7, r*0.55, mul(fg, 1.05), 0.2)
		for i := 0; i < 6; i++ {
			x, y := 3+p.r.IntN(10), 3+p.r.IntN(10)
			if p.get(x, y).A > 0 {
				p.set(x, y, mul(fg, 1.35))
			}
		}
		return p, true, false
	case "pine":
		p := newPC(spx, 24, key+strconv.Itoa(v))
		p.circle(8, 22, 4, alpha(black, 70))
		p.rect(7, 18, 2, 5, bark)
		tiers := [][3]int{{1, 7, 3}, {6, 13, 5}, {11, 19, 7}}
		for _, t := range tiers {
			for y := t[0]; y <= t[1]; y++ {
				w := float64(y-t[0]+1) / float64(t[1]-t[0]+1) * float64(t[2])
				for x := 0; x < spx; x++ {
					d := float64(x) + 0.5 - 8
					if math.Abs(d) <= w {
						l := 1.05 - 0.45*(d/math.Max(1, w))*0.6 - 0.15*float64(y-t[0])/float64(t[1]-t[0]+1)
						p.set(x, y, mul(fg, l+(p.r.Float64()-0.5)*0.15))
					}
				}
			}
		}
		p.set(8, 0, mul(fg, 1.3))
		return p, true, false
	case "dead_tree":
		p := newPC(spx, 24, key)
		c := mul(fg, 0.9)
		p.circle(8, 22, 3.5, alpha(black, 60))
		p.rect(7, 8, 2, 15, c)
		p.line(8, 12, 3, 6, c)
		p.line(8, 10, 13, 4, c)
		p.line(4, 7, 3, 3, c)
		p.line(12, 5, 14, 2, c)
		return p, true, false
	case "bush":
		p := newPC(spx, spx, key+strconv.Itoa(v))
		p.circle(8, 13, 5, alpha(black, 60))
		p.ball(8, 9, 5.5, mul(fg, 0.95), 0.3)
		p.ball(5, 10, 3.5, mul(fg, 0.9), 0.3)
		p.ball(11, 10, 3.5, mul(fg, 0.85), 0.3)
		if v%2 == 0 {
			p.set(6, 8, color.RGBA{200, 40, 60, 255})
			p.set(10, 11, color.RGBA{200, 40, 60, 255})
		}
		return p, false, false
	case "mountain", "snow", "sandstone", "ice_rock", "basalt":
		p := newPC(spx, 24, key+strconv.Itoa(v))
		peak := 2 + v%3
		rock := mul(stone, 0.8)
		switch key {
		case "snow":
			rock = mul(stone, 0.95)
		case "sandstone", "ice_rock", "basalt":
			rock = fg
		}
		for y := peak; y < 24; y++ {
			half := float64(y-peak) / float64(24-peak) * 8.5
			for x := 0; x < spx; x++ {
				d := float64(x) + 0.5 - 8
				if math.Abs(d) > half {
					continue
				}
				c := rock
				l := 1.0
				if d > 0 {
					l = 0.62
				}
				c = mul(c, l+(p.r.Float64()-0.5)*0.12)
				snowLine := peak + 5
				switch key {
				case "snow":
					snowLine = peak + 12
				case "ice_rock":
					snowLine = peak + 3
				case "sandstone", "basalt":
					snowLine = -1
					if key == "sandstone" && (y-peak)%5 == 0 {
						c = mul(c, 0.8)
					}
				}
				if y < snowLine {
					c = mul(white, l*0.98)
				}
				p.set(x, y, c)
			}
		}
		p.line(8, peak+4, 6, peak+11, mul(rock, 0.55))
		if key == "basalt" && v == 1 {
			p.line(9, peak+8, 11, peak+15, color.RGBA{255, 100, 30, 255})
		}
		return p, true, false
	case "fountain":
		p := newPC(spx, spx, key+strconv.Itoa(v))
		p.circle(8, 9, 7.5, mul(hex("#b0a898"), 0.85))
		p.circle(8, 9, 6, hex("#c8c0b0"))
		p.circle(8, 9, 5, hex("#2a5a8a"))
		p.circle(7, 8, 3.5, hex("#3a7ab0"))
		p.rect(7, 3, 2, 7, hex("#d8d0c0"))
		p.circle(8, 3, 1.6, alpha(hex("#c0e8ff"), 220))
		for i := 0; i < 4; i++ {
			p.set(4+i*2+v%2, 6+i%2, alpha(white, 200))
		}
		return p, false, false
	case "market_stall":
		p := newPC(spx, 24, key+strconv.Itoa(v))
		awn := []color.RGBA{hex("#c03a2a"), hex("#2a6ac0"), hex("#3a9a3a")}[v%3]
		p.circle(8, 22, 6, alpha(black, 70))
		p.rect(2, 6, 1, 16, bark)
		p.rect(13, 6, 1, 16, bark)
		for x := 0; x < spx; x++ {
			c := awn
			if (x/2)%2 == 1 {
				c = hex("#f0e8d8")
			}
			p.rect(x, 2, 1, 5, c)
		}
		p.rect(1, 15, 14, 5, mul(bark, 1.2))
		p.rect(1, 15, 14, 1, mul(bark, 1.5))
		goods := []color.RGBA{hex("#e0402a"), hex("#ffd040"), hex("#80c040"), hex("#c08040")}
		for i := 0; i < 5; i++ {
			p.ball(float64(3+i*2+p.r.IntN(2)), 13.5, 1.2, goods[(i+v)%len(goods)], 0.1)
		}
		return p, true, false
	case "lamp_post":
		p := newPC(spx, 24, key)
		p.circle(8, 22, 3, alpha(black, 80))
		p.rect(7, 6, 2, 17, hex("#2a2a30"))
		p.rect(6, 21, 4, 2, hex("#3a3a40"))
		p.rect(5, 2, 6, 5, hex("#3a3a40"))
		p.rect(6, 3, 4, 3, hex("#ffe0a0"))
		p.set(7, 4, white)
		return p, true, false
	case "statue":
		p := newPC(spx, 24, key+strconv.Itoa(v))
		stoneC := hex("#d0d0c8")
		p.circle(8, 22, 6, alpha(black, 80))
		p.rect(3, 17, 10, 6, mul(stoneC, 0.7))
		p.rect(3, 17, 10, 1, mul(stoneC, 0.9))
		p.rect(6, 7, 4, 10, stoneC)
		p.circle(8, 5, 2.5, stoneC)
		p.rect(4, 8, 2, 6, mul(stoneC, 0.85))
		p.line(11, 3, 11, 15, mul(stoneC, 0.8))
		p.rect(10, 8, 2, 2, mul(stoneC, 0.85))
		return p, true, false
	case "well":
		p := newPC(spx, spx, key)
		p.circle(8, 9, 6.5, mul(stone, 0.75))
		p.circle(8, 9, 4.5, color.RGBA{20, 40, 70, 255})
		p.circle(7, 8, 1.2, alpha(fg, 160))
		p.rect(1, 2, 14, 2, bark)
		p.rect(2, 2, 1, 7, bark)
		p.rect(13, 2, 1, 7, bark)
		return p, false, false
	case "fence":
		p := newPC(spx, spx, key)
		p.rect(0, 7, spx, 2, fg)
		p.rect(0, 11, spx, 1, mul(fg, 0.8))
		p.rect(2, 4, 2, 10, mul(fg, 1.1))
		p.rect(12, 4, 2, 10, mul(fg, 1.1))
		return p, false, false
	case "chest", "chest_open":
		p := newPC(spx, spx, key)
		wood := color.RGBA{120, 80, 40, 255}
		p.circle(8, 14, 6, alpha(black, 70))
		if key == "chest" {
			p.rect(2, 5, 12, 9, wood)
			p.rect(2, 5, 12, 3, mul(wood, 1.25))
			p.rect(2, 8, 12, 1, mul(wood, 0.5))
			p.rect(4, 5, 1, 9, fg)
			p.rect(11, 5, 1, 9, fg)
			p.rect(7, 8, 2, 3, fg)
		} else {
			p.rect(2, 2, 12, 4, mul(wood, 0.8))
			p.rect(2, 7, 12, 7, wood)
			p.rect(3, 7, 10, 3, color.RGBA{20, 12, 8, 255})
			p.rect(4, 7, 1, 7, fg)
			p.rect(11, 7, 1, 7, fg)
		}
		return p, false, false
	case "altar":
		p := newPC(spx, spx, key)
		p.rect(1, 6, 14, 8, mul(fg, 0.45))
		p.rect(1, 6, 14, 2, mul(fg, 0.7))
		p.set(3, 4, color.RGBA{255, 220, 120, 255})
		p.set(12, 4, color.RGBA{255, 220, 120, 255})
		p.rect(3, 5, 1, 1, white)
		p.rect(12, 5, 1, 1, white)
		p.circle(8, 10, 2, fg)
		return p, false, false
	case "pillar":
		p := newPC(spx, 24, key)
		p.circle(8, 22, 5, alpha(black, 70))
		for y := 4; y < 22; y++ {
			for x := 4; x < 12; x++ {
				l := 1.1 - 0.09*float64(x-4)
				p.set(x, y, mul(fg, l*0.85))
			}
		}
		p.rect(3, 2, 10, 3, mul(fg, 1.0))
		p.rect(3, 20, 10, 3, mul(fg, 0.75))
		return p, true, false
	case "stone_wall", "cave_wall", "house_wall", "door", "ice_wall", "basalt_wall", "sandstone_wall", "dark_wall", "ruin_wall", "city_wall":
		return wallArt(key, fg, bg, v), true, true
	case "snow_pine":
		p, _, _ := objectArt("pine", fg, bg, v)
		for y := 0; y < 22; y++ {
			for x := 0; x < spx; x++ {
				c := p.get(x, y)
				if c.A == 0 || c == bark || y > 18 {
					continue
				}
				if p.get(x, y-1).A == 0 || (y%5 == 1 && (x+y)%3 == 0) {
					p.set(x, y, color.RGBA{236, 242, 255, 255})
				}
			}
		}
		return p, true, false
	case "charred_tree":
		p, _, _ := objectArt("dead_tree", fg, bg, v)
		p.set(7, 14, color.RGBA{255, 120, 40, 255})
		p.set(8, 18, color.RGBA{255, 90, 30, 255})
		return p, true, false
	case "twisted_tree":
		p := newPC(spx, 24, key+strconv.Itoa(v))
		trunk := color.RGBA{60, 44, 56, 255}
		p.circle(8, 22, 4.5, alpha(black, 70))
		p.thick(8, 23, 7, 16, 2, trunk)
		p.thick(7, 16, 9, 11, 2, mul(trunk, 0.9))
		p.thick(9, 12, 4, 8, 1, trunk)
		p.thick(8, 12, 13, 9, 1, mul(trunk, 0.8))
		p.ball(5, 6, 3.6, fg, 0.3)
		p.ball(11, 7, 3.4, mul(fg, 0.85), 0.3)
		p.ball(8, 4, 3, mul(fg, 1.1), 0.3)
		p.set(7, 15, color.RGBA{200, 255, 120, 255})
		p.set(9, 15, color.RGBA{200, 255, 120, 255})
		return p, true, false
	case "palm":
		p := newPC(spx, 24, key+strconv.Itoa(v))
		p.circle(8, 22, 4, alpha(black, 70))
		for y := 7; y < 23; y++ {
			x := 7 + int(math.Sin(float64(y)*0.25+float64(v))*1.5)
			c := color.RGBA{150, 110, 60, 255}
			if y%3 == 0 {
				c = mul(c, 0.75)
			}
			p.set(x, y, c)
			p.set(x+1, y, mul(c, 0.8))
		}
		top := 7 + int(math.Sin(7*0.25+float64(v))*1.5)
		for i := 0; i < 6; i++ {
			a := float64(i)/6*2*math.Pi + 0.4
			for t := 1.0; t < 6.5; t += 0.5 {
				x := float64(top) + 1 + math.Cos(a)*t
				y := 7 + math.Sin(a)*t*0.6 + t*t*0.06
				p.set(int(x), int(y), mul(fg, 1.05-t*0.05))
			}
		}
		p.set(top+1, 8, color.RGBA{110, 70, 30, 255})
		p.set(top, 9, color.RGBA{110, 70, 30, 255})
		return p, true, false
	case "cactus":
		p := newPC(spx, 24, key+strconv.Itoa(v))
		p.circle(8, 22, 3.5, alpha(black, 70))
		col := fg
		p.rect(7, 8, 3, 15, col)
		p.rect(7, 8, 1, 15, mul(col, 1.2))
		p.rect(9, 8, 1, 15, mul(col, 0.7))
		p.rect(3, 12, 2, 4, col)
		p.rect(3, 15, 4, 2, col)
		p.rect(12, 10, 2, 5, mul(col, 0.85))
		p.rect(10, 14, 3, 2, mul(col, 0.85))
		p.set(8, 7, mul(col, 1.1))
		for y := 9; y < 22; y += 3 {
			p.set(8, y, alpha(white, 120))
		}
		if v == 1 {
			p.set(8, 7, color.RGBA{240, 100, 160, 255})
		}
		return p, true, false
	case "gravestone":
		p := newPC(spx, spx, key+strconv.Itoa(v))
		p.circle(8, 14, 5, alpha(black, 60))
		st := fg
		p.rect(4, 4, 8, 10, st)
		p.rect(5, 3, 6, 1, st)
		p.rect(4, 4, 1, 10, mul(st, 1.15))
		p.rect(11, 4, 1, 10, mul(st, 0.7))
		p.rect(7, 6, 2, 6, mul(st, 0.55))
		p.rect(5, 8, 6, 1, mul(st, 0.55))
		p.rect(3, 13, 10, 2, color.RGBA{70, 56, 40, 255})
		if v == 2 {
			p.set(10, 5, color.RGBA{90, 120, 60, 255})
			p.set(11, 6, color.RGBA{90, 120, 60, 255})
		}
		return p, false, false
	case "menhir":
		p := newPC(spx, 24, key+strconv.Itoa(v))
		p.circle(8, 22, 4.5, alpha(black, 70))
		for y := 2 + v; y < 23; y++ {
			half := 3.2 - math.Max(0, float64(6-y))*0.4
			for x := int(8 - half); x <= int(8+half); x++ {
				l := 1.1 - 0.12*(float64(x)-8+half)
				p.set(x, y, mul(fg, l+(p.r.Float64()-0.5)*0.08))
			}
		}
		for y := 8; y < 18; y += 3 {
			p.set(7, y, color.RGBA{140, 200, 255, 255})
			p.set(8, y+1, color.RGBA{140, 200, 255, 200})
		}
		return p, true, false
	case "shrine", "shrine_used":
		p := newPC(spx, spx, key)
		p.circle(8, 14, 6, alpha(black, 60))
		st := color.RGBA{150, 146, 140, 255}
		p.rect(3, 9, 10, 5, st)
		p.rect(3, 9, 10, 1, mul(st, 1.2))
		p.rect(12, 9, 1, 5, mul(st, 0.7))
		p.rect(5, 6, 6, 3, mul(st, 0.9))
		if key == "shrine" {
			p.ball(8, 4, 2.6, fg, 0.1)
			p.set(7, 3, white)
		} else {
			p.circle(8, 5, 1.8, mul(fg, 0.6))
		}
		return p, false, false
	case "tent":
		p := newPC(spx, 24, key+strconv.Itoa(v))
		p.circle(8, 22, 6, alpha(black, 70))
		for y := 8; y < 23; y++ {
			half := float64(y-8) * 0.5
			for x := int(8 - half); x <= int(8+half); x++ {
				l := 1.1
				if float64(x) > 8 {
					l = 0.75
				}
				p.set(x, y, mul(fg, l))
			}
		}
		for y := 15; y < 23; y++ {
			half := float64(y-15) * 0.32
			for x := int(8 - half); x <= int(8+half); x++ {
				p.set(x, y, color.RGBA{30, 20, 14, 255})
			}
		}
		p.line(8, 6, 8, 9, bark)
		return p, true, false
	case "campfire", "brazier":
		p := newPC(spx, spx, key)
		if key == "campfire" {
			p.circle(8, 12, 5, color.RGBA{60, 56, 50, 255})
			p.thick(3, 13, 13, 10, 2, bark)
			p.thick(3, 10, 13, 13, 2, mul(bark, 0.8))
		} else {
			p.rect(5, 13, 1, 3, color.RGBA{60, 56, 60, 255})
			p.rect(10, 13, 1, 3, color.RGBA{60, 56, 60, 255})
			p.rect(3, 10, 10, 3, color.RGBA{90, 84, 90, 255})
			p.rect(3, 10, 10, 1, color.RGBA{130, 124, 130, 255})
		}
		p.ball(8, 8, 3.2, color.RGBA{255, 120, 30, 255}, 0.2)
		p.ball(8, 7, 2, color.RGBA{255, 210, 80, 255}, 0.1)
		p.set(8, 3, color.RGBA{255, 160, 40, 255})
		p.set(6, 5, color.RGBA{255, 140, 40, 255})
		return p, false, false
	case "crystal":
		p := newPC(spx, 24, key+strconv.Itoa(v))
		p.circle(8, 22, 4, alpha(black, 50))
		shard := func(cx, top, bot, half float64, c color.RGBA) {
			for y := int(top); y <= int(bot); y++ {
				k := (float64(y) - top) / (bot - top)
				w := half * math.Min(1, k*2.2)
				for x := int(cx - w); x <= int(cx+w); x++ {
					l := 1.25 - 0.45*(float64(x)-(cx-w))/math.Max(1, 2*w)
					p.set(x, y, alpha(mul(c, l), 230))
				}
			}
		}
		shard(5, 11, 22, 2, mul(fg, 0.8))
		shard(11, 9, 22, 2, mul(fg, 0.75))
		shard(8, 4+float64(v), 22, 2.8, fg)
		p.set(7, 8, white)
		return p, true, false
	case "sarcophagus", "sarcophagus_open":
		p := newPC(spx, spx, key)
		p.circle(8, 13, 6, alpha(black, 70))
		st := color.RGBA{170, 150, 110, 255}
		p.rect(3, 3, 10, 11, mul(st, 0.8))
		if key == "sarcophagus" {
			p.rect(3, 2, 10, 10, st)
			p.rect(3, 2, 10, 1, mul(st, 1.2))
			p.rect(7, 3, 2, 8, fg)
			p.rect(5, 5, 6, 1, fg)
			p.set(8, 4, color.RGBA{60, 120, 200, 255})
		} else {
			p.rect(4, 4, 8, 8, color.RGBA{24, 18, 14, 255})
			p.rect(9, 1, 6, 9, mul(st, 0.9))
			p.set(6, 9, color.RGBA{220, 220, 200, 255})
		}
		return p, false, false
	}
	return nil, false, false
}

// wallArt draws a 3/4 view block: a 16px top face and an 8px front face.
func wallArt(key string, fg, bg color.RGBA, v int) *pc {
	p := newPC(spx, 24, key+strconv.Itoa(v))
	var top, front color.RGBA
	switch key {
	case "cave_wall", "basalt_wall":
		top, front = mix(fg, bg, 0.3), mul(bg, 1.1)
	case "ice_wall":
		top, front = mul(fg, 0.85), mix(fg, bg, 0.5)
	case "house_wall", "door":
		top, front = mul(fg, 0.8), mul(fg, 0.55)
	default:
		top, front = mul(fg, 0.62), mul(fg, 0.42)
	}
	brick := key == "stone_wall" || key == "sandstone_wall" || key == "dark_wall" || key == "ruin_wall" || key == "city_wall"
	// top face
	p.rect(0, 0, spx, 16, top)
	p.noise(0.12)
	switch {
	case brick:
		for i := 0; i < spx; i++ {
			p.set(i, 0, mul(top, 1.35))
			p.set(i, 8, mul(top, 0.8))
		}
		p.rect(v%8+3, 1, 1, 7, mul(top, 0.8))
		p.rect((v*5)%8+7, 9, 1, 7, mul(top, 0.8))
		if key == "ruin_wall" {
			p.speckle(color.RGBA{80, 110, 60, 255}, 0.08)
		}
	case key == "ice_wall":
		p.speckle(mul(top, 1.15), 0.12)
		p.line(2, 3, 9, 12, alpha(white, 120))
	case key == "cave_wall" || key == "basalt_wall":
		p.speckle(mul(top, 0.7), 0.15)
		p.speckle(mul(top, 1.25), 0.06)
	case key == "house_wall" || key == "door":
		for y := 0; y < 16; y += 4 {
			for x := 0; x < spx; x++ {
				p.set(x, y, mul(top, 0.75))
			}
		}
		p.rect(0, 0, spx, 1, mul(top, 1.3))
	}
	// front face
	for y := 16; y < 24; y++ {
		for x := 0; x < spx; x++ {
			c := mul(front, 1-0.04*float64(y-16)+(p.r.Float64()-0.5)*0.1)
			switch {
			case brick:
				off := 0
				if (y-16)/4%2 == 1 {
					off = 4
				}
				if (y-16)%4 == 3 || (x+off)%8 == 7 {
					c = mul(front, 0.6)
				}
			case key == "ice_wall":
				if (x+y)%5 == 0 {
					c = mix(c, white, 0.25)
				}
			case key == "house_wall":
				if (y-16)%3 == 2 {
					c = mul(front, 0.65)
				}
			case key == "door":
				if x < 3 || x > 12 {
					c = mul(front, 0.7)
				} else {
					c = mul(color.RGBA{130, 90, 50, 255}, 1-0.04*float64(y-16))
					if x == 8 {
						c = mul(c, 0.6)
					}
					if x == 10 && y == 20 {
						c = color.RGBA{230, 200, 90, 255}
					}
				}
			}
			p.set(x, y, c)
		}
	}
	for x := 0; x < spx; x++ {
		p.set(x, 16, mul(front, 1.3))
		p.set(x, 23, mul(front, 0.5))
	}
	if key == "city_wall" {
		// battlements on the top face
		for x := 0; x < spx; x += 4 {
			p.rect(x, 0, 2, 3, mul(top, 1.25))
			p.rect(x, 3, 2, 1, mul(top, 0.7))
		}
	}
	if key == "ruin_wall" {
		// broken top: knock out a corner
		for y := 0; y < 6+v; y++ {
			for x := spx - 1 - (6 - y/2); x < spx; x++ {
				p.clear(x, y)
			}
		}
	}
	return p
}

// baseGround picks the ground drawn under an object tile.
func baseGround(def *content.TileDef, theme string) string {
	switch def.Biome {
	case "forest":
		return "forest_floor"
	case "plains":
		return "grass"
	case "swamp":
		return "swamp"
	case "hills", "snow":
		return "hill"
	case "sand":
		return "sand"
	case "desert":
		return "desert_sand"
	case "tundra":
		return "snow_ground"
	case "ash":
		return "ash"
	case "cursed":
		return "blight_grass"
	}
	switch theme {
	case "crypt":
		return "stone_floor"
	case "cave":
		return "cave_floor"
	case "ice":
		return "ice_floor"
	case "volcano":
		return "basalt_floor"
	case "temple":
		return "sandstone_floor"
	case "fortress":
		return "dark_floor"
	}
	return "grass"
}

var animated = map[string]bool{"water": true, "deep_water": true, "lava": true}

// spriteFor builds (once) the art for a tile type.
func (r *worldRenderer) spriteFor(def *content.TileDef) *spriteSet {
	if s, ok := r.sprites[def.Key]; ok {
		return s
	}
	fg, bg := tileColors(def)
	s := &spriteSet{}
	if obj, tall, wall := objectArt(def.Key, fg, bg, 0); obj != nil {
		s.tall, s.wall = tall, wall
		s.object = append(s.object, obj.image())
		for v := 1; v < 3; v++ {
			o, _, _ := objectArt(def.Key, fg, bg, v)
			s.object = append(s.object, o.image())
		}
	} else if isKnownGround(def.Key) {
		frames := 1
		if animated[def.Key] {
			frames = 4
		}
		for v := 0; v < 3; v++ {
			var fs []*ebiten.Image
			for f := 0; f < frames; f++ {
				fs = append(fs, groundArt(def.Key, fg, bg, v, f).image())
			}
			s.ground = append(s.ground, fs)
		}
	} else {
		// unknown (modded) tile: coloured block with its glyph
		s.ground = [][]*ebiten.Image{{groundArt("", fg, bg, 0, 0).image()}}
		s.object = []*ebiten.Image{r.glyphTile(def.Rune, fg)}
	}
	r.sprites[def.Key] = s
	return s
}

var knownGround = map[string]bool{
	"grass": true, "grass2": true, "tall_grass": true, "flowers": true, "forest_floor": true, "sand": true,
	"swamp": true, "reeds": true, "hill": true, "road": true, "bridge": true, "water": true, "deep_water": true,
	"lava": true, "house_floor": true, "stone_floor": true, "rubble": true, "bones": true, "cave_floor": true,
	"door_open": true, "stairs_down": true, "stairs_up": true, "dungeon": true, "void": true,
	"desert_sand": true, "dunes": true, "snow_ground": true, "snowdrift": true, "ice": true, "ash": true,
	"magma_crack": true, "blight_grass": true, "mushrooms": true, "web": true, "ice_floor": true,
	"basalt_floor": true, "sandstone_floor": true, "dark_floor": true, "carpet": true,
	"cobblestone": true, "city_gate": true, "garden": true,
}

func isKnownGround(k string) bool { return knownGround[k] }

// glyphTile renders a character as an object, for tiles without art.
func (r *worldRenderer) glyphTile(g rune, fg color.RGBA) *ebiten.Image {
	img := ebiten.NewImage(64, 64)
	face := &text.GoTextFace{Source: r.fonts.monoBold, Size: 44}
	op := &text.DrawOptions{}
	w := text.Advance(string(g), face)
	m := face.Metrics()
	op.GeoM.Translate(32-w/2, 32-(m.HAscent+m.HDescent)/2)
	op.ColorScale.ScaleWithColor(fg)
	text.Draw(img, string(g), face, op)
	return img
}

// ---- smooth images ----

// softDot is a white radial gradient used for glows, particles and shadows.
func softDot(size int) *ebiten.Image {
	img := image.NewRGBA(image.Rect(0, 0, size, size))
	c := float64(size) / 2
	for y := 0; y < size; y++ {
		for x := 0; x < size; x++ {
			d := math.Hypot(float64(x)+0.5-c, float64(y)+0.5-c) / c
			a := math.Max(0, 1-d)
			a = a * a * (3 - 2*a)
			v := uint8(255 * a)
			img.SetRGBA(x, y, color.RGBA{v, v, v, v})
		}
	}
	return ebiten.NewImageFromImage(img)
}

func vignetteImage(size int) *ebiten.Image {
	img := image.NewRGBA(image.Rect(0, 0, size, size))
	c := float64(size) / 2
	for y := 0; y < size; y++ {
		for x := 0; x < size; x++ {
			d := math.Hypot(float64(x)+0.5-c, float64(y)+0.5-c) / c
			a := math.Max(0, math.Min(1, (d-0.55)/0.75))
			img.SetRGBA(x, y, color.RGBA{0, 0, 0, uint8(255 * a * a)})
		}
	}
	return ebiten.NewImageFromImage(img)
}

func hexKey(c color.RGBA) string {
	return strconv.Itoa(int(c.R)) + "," + strconv.Itoa(int(c.G)) + "," + strconv.Itoa(int(c.B))
}

// appIcon draws the window icon: a golden ring with a tree.
func appIcon(size int) image.Image {
	p := newPC(16, 16, "icon")
	p.circle(8, 8, 7.8, color.RGBA{230, 180, 70, 255})
	p.circle(8, 8, 6.6, color.RGBA{18, 20, 36, 255})
	p.rect(7, 9, 2, 4, bark)
	p.ball(8, 7, 4, color.RGBA{60, 160, 70, 255}, 0.2)
	img := image.NewRGBA(image.Rect(0, 0, size, size))
	for y := 0; y < size; y++ {
		for x := 0; x < size; x++ {
			img.Set(x, y, p.img.At(x*16/size, y*16/size))
		}
	}
	return img
}
