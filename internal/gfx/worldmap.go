package gfx

import (
	"image"
	"image/color"
	"math"
	"strings"

	"github.com/hajimehoshi/ebiten/v2"
	"github.com/hajimehoshi/ebiten/v2/text/v2"
	"github.com/hajimehoshi/ebiten/v2/vector"

	"ratas/internal/client"
	"ratas/internal/content"
	"ratas/internal/i18n"
	"ratas/internal/proto"
	"ratas/internal/world"
)

// The world map is drawn as an old atlas: the explored land is painted on
// parchment (coasts inked, forests dotted, mountains as little peaks), the
// unknown stays blank, and villages, dungeons, sights, quest goals and
// companions are marked on top.

const mapPx = 4 // pixels per tile of the painted map

var (
	cParchment = hex("#e9d9b2")
	cInk       = hex("#3b2a1a")
	cInkSoft   = hex("#6a5236")
)

// worldMap caches the painted land; it is repainted when the level or the
// explored area changes.
type worldMap struct {
	img  *ebiten.Image
	lvl  *world.Level
	ver  int
	seen int
	at   float64
}

// mapColor is the atlas colour of a tile.
func mapColor(def *content.TileDef) color.RGBA {
	k := def.Key
	switch {
	case k == "deep_water":
		return hex("#3d6c98")
	case k == "water":
		return hex("#6496c0")
	case k == "lava" || k == "magma_crack":
		return hex("#d8642c")
	case k == "ice" || k == "ice_floor":
		return hex("#c4dcea")
	case strings.Contains(k, "snow") && k != "snow_pine":
		return hex("#f2f2ee")
	case k == "snow_pine":
		return hex("#5f8270")
	case k == "sand" || k == "desert_sand" || k == "dunes" || k == "sandstone_floor":
		return hex("#e4c88c")
	case k == "cactus" || k == "palm":
		return hex("#a8b060")
	case k == "tree" || k == "pine" || k == "bush":
		return hex("#4f7a42")
	case k == "forest_floor":
		return hex("#78a058")
	case k == "grass" || k == "grass2" || k == "tall_grass" || k == "flowers":
		return hex("#98b872")
	case k == "swamp" || k == "reeds":
		return hex("#7a8650")
	case k == "dead_tree" || k == "twisted_tree" || k == "blight_grass" || k == "mushrooms":
		return hex("#7e7860")
	case k == "gravestone" || k == "bones":
		return hex("#8e8a7e")
	case k == "hill":
		return hex("#b0a072")
	case k == "mountain" || k == "ice_rock":
		return hex("#9a8e78")
	case k == "ash" || k == "basalt" || k == "charred_tree" || k == "basalt_floor":
		return hex("#6e6460")
	case k == "road" || k == "bridge":
		return hex("#c4a476")
	case k == "house_wall":
		return hex("#9a4e36")
	case k == "house_floor" || k == "door" || k == "door_open" || k == "carpet":
		return hex("#c8a478")
	case k == "dungeon" || k == "stairs_down" || k == "stairs_up":
		return hex("#2a1a14")
	case k == "void":
		return hex("#1c1612")
	}
	if !def.Walkable && !def.Transparent {
		return hex("#5e5448") // dungeon walls
	}
	fg, bg := tileColors(def)
	return mix(mix(bg, fg, 0.55), cParchment, 0.25)
}

func isWater(def *content.TileDef) bool {
	return def.Key == "water" || def.Key == "deep_water"
}

// paint draws the explored land of a level on parchment.
func paintMap(l *world.Level, explored world.Bitset) *image.RGBA {
	W, H := l.W*mapPx, l.H*mapPx
	img := image.NewRGBA(image.Rect(0, 0, W, H))
	seen := func(x, y int) bool {
		return l.In(x, y) && (explored == nil || explored.Get(y*l.W+x))
	}
	hash := func(x, y int) int { return tileHash(x, y) & 0xffff }
	set := func(x, y int, c color.RGBA) {
		if x >= 0 && y >= 0 && x < W && y < H {
			img.SetRGBA(x, y, c)
		}
	}
	// parchment with fibres
	for y := 0; y < H; y++ {
		for x := 0; x < W; x++ {
			n := float64(tileHash(x*7, y*13)%100)/100 - 0.5
			set(x, y, mul(cParchment, 1+n*0.05))
		}
	}
	defs := make([]*content.TileDef, len(l.Tiles))
	for i, t := range l.Tiles {
		defs[i] = content.Tile(t)
	}
	at := func(x, y int) *content.TileDef { return defs[y*l.W+x] }
	for ty := 0; ty < l.H; ty++ {
		for tx := 0; tx < l.W; tx++ {
			if !seen(tx, ty) {
				continue
			}
			def := at(tx, ty)
			base := mapColor(def)
			edge := false // next to the unknown: fade into the parchment
			coast := false
			for _, d := range [][2]int{{1, 0}, {-1, 0}, {0, 1}, {0, -1}} {
				nx, ny := tx+d[0], ty+d[1]
				if !l.In(nx, ny) {
					continue
				}
				if !seen(nx, ny) {
					edge = true
				} else if isWater(at(nx, ny)) != isWater(def) {
					coast = true
				}
			}
			if isWater(def) && coast {
				base = mix(base, hex("#a8cce0"), 0.45) // shallows
			}
			for py := 0; py < mapPx; py++ {
				for px := 0; px < mapPx; px++ {
					x, y := tx*mapPx+px, ty*mapPx+py
					n := float64(tileHash(x*3, y*5)%100)/100 - 0.5
					c := mul(base, 1+n*0.06)
					c = mix(c, cParchment, 0.14)
					if edge {
						c = mix(c, cParchment, 0.5)
					}
					set(x, y, c)
				}
			}
			ox, oy := tx*mapPx, ty*mapPx
			h := hash(tx, ty)
			switch def.Key {
			case "water", "deep_water":
				if h%7 == 0 && !coast { // waves
					wc := mix(base, cWhite, 0.35)
					set(ox, oy+2, wc)
					set(ox+1, oy+1, wc)
					set(ox+2, oy+2, wc)
				}
			case "tree", "pine", "snow_pine", "bush", "dead_tree", "twisted_tree", "charred_tree", "palm", "cactus":
				if h%2 == 0 {
					c := mul(base, 0.7)
					set(ox+1, oy+1, c)
					set(ox+2, oy+1, c)
					set(ox+1, oy+2, mul(c, 0.85))
					set(ox+2, oy+2, mul(c, 0.7))
					set(ox+1, oy+3, cInkSoft)
				}
			case "mountain", "ice_rock":
				if h%3 == 0 { // a small peak spilling over the neighbours
					for i := 0; i < 4; i++ {
						set(ox-i+1, oy+1+i, mul(cInk, 1.2))
						set(ox+i+2, oy+1+i, mul(base, 0.75))
					}
					set(ox+1, oy, cWhite)
					set(ox+2, oy, mix(cWhite, base, 0.4))
				}
			case "hill":
				if h%4 == 0 {
					c := mul(base, 0.72)
					set(ox, oy+2, c)
					set(ox+1, oy+1, c)
					set(ox+2, oy+1, c)
					set(ox+3, oy+2, c)
				}
			case "swamp", "reeds":
				if h%3 == 0 {
					set(ox+1, oy+1, hex("#56603a"))
					set(ox+1, oy+2, hex("#56603a"))
					set(ox+3, oy+3, hex("#6a8aa0"))
				}
			case "gravestone":
				set(ox+1, oy+1, cInkSoft)
				set(ox+1, oy+2, cInkSoft)
				set(ox, oy+1, cInkSoft)
				set(ox+2, oy+1, cInkSoft)
			}
			if coast && !isWater(def) {
				// inked coastline on the land side
				for _, d := range [][2]int{{1, 0}, {-1, 0}, {0, 1}, {0, -1}} {
					nx, ny := tx+d[0], ty+d[1]
					if !l.In(nx, ny) || !seen(nx, ny) || !isWater(at(nx, ny)) {
						continue
					}
					for i := 0; i < mapPx; i++ {
						x, y := ox+i, oy
						switch {
						case d[0] == 1:
							x, y = ox+mapPx-1, oy+i
						case d[0] == -1:
							x, y = ox, oy+i
						case d[1] == 1:
							x, y = ox+i, oy+mapPx-1
						}
						set(x, y, mix(cInk, base, 0.35))
					}
				}
			}
		}
	}
	return img
}

func (r *worldRenderer) mapImage(sc *client.Scene) *ebiten.Image {
	m := &r.atlas
	seen := 0
	for _, w := range sc.Explored {
		seen += popcount(uint64(w))
	}
	if m.img != nil && m.lvl == sc.Level && m.ver == sc.LevelVer && (m.seen == seen || r.t-m.at < 1) {
		return m.img
	}
	if m.img != nil {
		m.img.Deallocate()
	}
	m.img = ebiten.NewImageFromImage(paintMap(sc.Level, sc.Explored))
	m.lvl, m.ver, m.seen, m.at = sc.Level, sc.LevelVer, seen, r.t
	return m.img
}

func popcount(x uint64) int {
	n := 0
	for x != 0 {
		x &= x - 1
		n++
	}
	return n
}

// mapLabels places text on the map: labels stay inside the frame, and a
// label that would cover a marker or an earlier label is skipped.
type mapLabels struct {
	bounds image.Rectangle
	taken  []image.Rectangle
}

func (ls *mapLabels) reserve(r image.Rectangle) { ls.taken = append(ls.taken, r) }

func (ls *mapLabels) place(s string, face *text.GoTextFace, x, y float64) (float64, float64, bool) {
	r := textRect(s, face, x, y)
	b := ls.bounds
	if dx := b.Min.X + 2 - r.Min.X; dx > 0 {
		r = r.Add(image.Pt(dx, 0))
	}
	if dx := b.Max.X - 2 - r.Max.X; dx < 0 {
		r = r.Add(image.Pt(dx, 0))
	}
	if r.Min.Y < b.Min.Y || r.Max.Y > b.Max.Y || r.Min.X < b.Min.X {
		return 0, 0, false
	}
	for _, o := range ls.taken {
		if o.Overlaps(r) {
			return 0, 0, false
		}
	}
	ls.taken = append(ls.taken, r)
	return float64(r.Min.X+r.Max.X) / 2, float64(r.Min.Y+r.Max.Y) / 2, true
}

// inkText draws text with a parchment halo so it reads over the land.
func inkText(dst *ebiten.Image, s string, face *text.GoTextFace, x, y float64, fg, halo color.RGBA) {
	s = i18n.T(s)
	w := text.Advance(s, face)
	m := face.Metrics()
	tx, ty := x-w/2, y-(m.HAscent+m.HDescent)/2
	o := math.Max(1, face.Size/11)
	for _, d := range [][2]float64{{-o, 0}, {o, 0}, {0, -o}, {0, o}, {-o, -o}, {o, o}, {-o, o}, {o, -o}} {
		op := &text.DrawOptions{}
		op.GeoM.Translate(tx+d[0], ty+d[1])
		op.ColorScale.ScaleWithColor(halo)
		text.Draw(dst, s, face, op)
	}
	op := &text.DrawOptions{}
	op.GeoM.Translate(tx, ty)
	op.ColorScale.ScaleWithColor(fg)
	text.Draw(dst, s, face, op)
}

func textRect(s string, face *text.GoTextFace, x, y float64) image.Rectangle {
	w := text.Advance(i18n.T(s), face)
	h := face.Size * 1.2
	return image.Rect(int(x-w/2-2), int(y-h/2), int(x+w/2+2), int(y+h/2))
}

func fillPath(dst *ebiten.Image, p *vector.Path, c color.RGBA) {
	op := &vector.DrawPathOptions{AntiAlias: true}
	op.ColorScale.ScaleWithColor(c)
	vector.FillPath(dst, p, nil, op)
}

func strokePath(dst *ebiten.Image, p *vector.Path, w float64, c color.RGBA) {
	op := &vector.DrawPathOptions{AntiAlias: true}
	op.ColorScale.ScaleWithColor(c)
	vector.StrokePath(dst, p, &vector.StrokeOptions{Width: float32(w), LineJoin: vector.LineJoinRound}, op)
}

func poly(pts ...float64) *vector.Path {
	var p vector.Path
	for i := 0; i+1 < len(pts); i += 2 {
		if i == 0 {
			p.MoveTo(float32(pts[i]), float32(pts[i+1]))
		} else {
			p.LineTo(float32(pts[i]), float32(pts[i+1]))
		}
	}
	p.Close()
	return &p
}

// map markers, s is the marker size in pixels
func markVillage(dst *ebiten.Image, x, y, s float64) {
	walls := poly(x-s*0.5, y+s*0.45, x-s*0.5, y-s*0.05, x+s*0.5, y-s*0.05, x+s*0.5, y+s*0.45)
	roof := poly(x-s*0.65, y, x, y-s*0.6, x+s*0.65, y)
	fillPath(dst, walls, hex("#f4e8c8"))
	strokePath(dst, walls, s*0.12, cInk)
	fillPath(dst, roof, hex("#b04a32"))
	strokePath(dst, roof, s*0.12, cInk)
	vector.FillRect(dst, float32(x-s*0.1), float32(y+s*0.15), float32(s*0.2), float32(s*0.3), cInk, false)
}

func markDungeon(dst *ebiten.Image, x, y, s float64, glow color.RGBA) {
	rock := poly(x-s*0.6, y+s*0.4, x-s*0.45, y-s*0.2, x, y-s*0.55, x+s*0.45, y-s*0.2, x+s*0.6, y+s*0.4)
	fillPath(dst, rock, hex("#8a7a64"))
	strokePath(dst, rock, s*0.12, cInk)
	var arch vector.Path
	arch.MoveTo(float32(x-s*0.25), float32(y+s*0.4))
	arch.LineTo(float32(x-s*0.25), float32(y))
	arch.Arc(float32(x), float32(y), float32(s*0.25), math.Pi, 0, vector.Clockwise)
	arch.LineTo(float32(x+s*0.25), float32(y+s*0.4))
	arch.Close()
	fillPath(dst, &arch, mix(hex("#1a0e0a"), glow, 0.35))
}

func markSight(dst *ebiten.Image, x, y, s float64, kind string) {
	c := hex("#c8962a")
	switch kind {
	case "shrine":
		c = hex("#e0b030")
	case "graveyard":
		c = hex("#8a8a90")
	case "camp":
		c = hex("#c0502a")
	case "oasis":
		c = hex("#3aa0a0")
	case "circle":
		c = hex("#9a7ac0")
	}
	d := poly(x, y-s*0.5, x+s*0.38, y, x, y+s*0.5, x-s*0.38, y)
	fillPath(dst, d, c)
	strokePath(dst, d, s*0.12, cInk)
	vector.FillCircle(dst, float32(x), float32(y), float32(s*0.1), cWhite, true)
}

func markQuest(dst *ebiten.Image, x, y, s, pulse float64) {
	vector.StrokeCircle(dst, float32(x), float32(y), float32(s*(0.55+0.25*pulse)), float32(s*0.1), pre(hex("#d02020"), 0.9-0.5*pulse), true)
	pin := poly(x, y, x-s*0.32, y-s*0.55, x+s*0.32, y-s*0.55)
	fillPath(dst, pin, hex("#c82020"))
	vector.FillCircle(dst, float32(x), float32(y-s*0.7), float32(s*0.34), hex("#e03030"), true)
	vector.StrokeCircle(dst, float32(x), float32(y-s*0.7), float32(s*0.34), float32(s*0.08), cInk, true)
	vector.FillCircle(dst, float32(x-s*0.1), float32(y-s*0.8), float32(s*0.09), cWhite, true)
}

func markHero(dst *ebiten.Image, x, y, s float64, c color.RGBA, facing uint8) {
	a := []float64{-math.Pi / 2, 0, math.Pi / 2, math.Pi}[facing%4] // up, right, down, left
	ca, sa := math.Cos(a), math.Sin(a)
	pt := func(fx, fy float64) (float64, float64) { return x + (fx*ca-fy*sa)*s, y + (fx*sa+fy*ca)*s }
	x0, y0 := pt(0.7, 0)
	x1, y1 := pt(-0.45, -0.5)
	x2, y2 := pt(-0.2, 0)
	x3, y3 := pt(-0.45, 0.5)
	arrow := poly(x0, y0, x1, y1, x2, y2, x3, y3)
	fillPath(dst, arrow, c)
	strokePath(dst, arrow, s*0.14, cInk)
}

func compass(dst *ebiten.Image, x, y, s float64, face *text.GoTextFace) {
	vector.StrokeCircle(dst, float32(x), float32(y), float32(s*0.62), float32(math.Max(1, s*0.04)), pre(cInk, 0.7), true)
	for i := 0; i < 4; i++ {
		a := float64(i) * math.Pi / 2
		ca, sa := math.Cos(a-math.Pi/2), math.Sin(a-math.Pi/2)
		tip := s * 0.9
		w := s * 0.16
		l := poly(x, y, x+ca*tip, y+sa*tip, x+(-sa)*w, y+ca*w)
		r := poly(x, y, x+ca*tip, y+sa*tip, x+sa*w, y-ca*w)
		fillPath(dst, l, cInk)
		fillPath(dst, r, hex("#c8a868"))
		strokePath(dst, r, math.Max(1, s*0.03), cInk)
	}
	inkText(dst, "С", face, x, y-s*1.15, cInk, pre(cParchment, 0.9))
}

// drawWorldMap draws the atlas of the current level over the whole area.
func (r *worldRenderer) drawWorldMap(dst *ebiten.Image, sc *client.Scene, area image.Rectangle) {
	l := sc.Level
	sc0 := r.scale
	W, H := float64(area.Dx()), float64(area.Dy())
	vector.FillRect(dst, float32(area.Min.X), float32(area.Min.Y), float32(W), float32(H), hex("#2a1e14"), false)

	// the map fills the area; the parchment sheet hugs it, with room for
	// the frame and the title ribbon
	pad := 22 * sc0
	titleH := 40 * sc0
	k := math.Min((W-4*pad)/float64(l.W), (H-titleH-3*pad)/float64(l.H))
	k = math.Max(0.5, math.Min(k, 16*sc0))
	mw, mh := float64(l.W)*k, float64(l.H)*k
	ox := float64(area.Min.X) + (W-mw)/2
	oy := float64(area.Min.Y) + titleH + pad + (H-titleH-3*pad-mh)/2
	at := func(x, y int) (float64, float64) { return ox + (float64(x)+0.5)*k, oy + (float64(y)+0.5)*k }
	sheet := image.Rect(int(ox-pad*1.2), int(oy-titleH-pad*0.4), int(ox+mw+pad*1.2), int(oy+mh+pad*1.2))
	roundRect(dst, float32(sheet.Min.X)+3, float32(sheet.Min.Y)+5, float32(sheet.Dx()), float32(sheet.Dy()), float32(6*sc0), color.RGBA{0, 0, 0, 120})
	roundRect(dst, float32(sheet.Min.X), float32(sheet.Min.Y), float32(sheet.Dx()), float32(sheet.Dy()), float32(6*sc0), cParchment)

	img := r.mapImage(sc)
	op := &ebiten.DrawImageOptions{}
	op.GeoM.Scale(k/mapPx, k/mapPx)
	op.GeoM.Translate(ox, oy)
	op.Filter = ebiten.FilterLinear
	dst.DrawImage(img, op)

	// double frame with corner ornaments
	fx0, fy0 := float32(ox-6*sc0), float32(oy-6*sc0)
	fw, fh := float32(mw+12*sc0), float32(mh+12*sc0)
	vector.StrokeRect(dst, fx0, fy0, fw, fh, float32(2*sc0), cInk, true)
	vector.StrokeRect(dst, fx0+float32(4*sc0), fy0+float32(4*sc0), fw-float32(8*sc0), fh-float32(8*sc0), float32(sc0), cInkSoft, true)
	for _, c := range [][2]float32{{fx0, fy0}, {fx0 + fw, fy0}, {fx0, fy0 + fh}, {fx0 + fw, fy0 + fh}} {
		d := poly(float64(c[0]), float64(c[1])-6*sc0, float64(c[0])+6*sc0, float64(c[1]), float64(c[0]), float64(c[1])+6*sc0, float64(c[0])-6*sc0, float64(c[1]))
		fillPath(dst, d, hex("#a8402a"))
		strokePath(dst, d, sc0, cInk)
	}

	// title ribbon
	title := r.face(r.fonts.bold, "map-title", 20*sc0)
	name := i18n.T(l.Name)
	if l.ID == "overworld" {
		name = i18n.Tf("Карта мира — %s", l.Name)
	}
	tw := text.Advance(name, title) + 40*sc0
	tx, ty := float64(sheet.Min.X+sheet.Max.X)/2, float64(sheet.Min.Y)+titleH*0.62
	rib := poly(tx-tw/2-14*sc0, ty-13*sc0, tx+tw/2+14*sc0, ty-13*sc0, tx+tw/2, ty, tx+tw/2+14*sc0, ty+13*sc0, tx-tw/2-14*sc0, ty+13*sc0, tx-tw/2, ty)
	fillPath(dst, rib, hex("#8a2e22"))
	strokePath(dst, rib, 1.5*sc0, cInk)
	drawCentered(dst, name, title, tx, ty, hex("#f6e8c8"), true)

	labels := mapLabels{bounds: image.Rect(int(ox), int(oy), int(ox+mw), int(oy+mh))}
	small := r.face(r.fonts.bold, "map-small", 11*sc0)
	label := r.face(r.fonts.bold, "map-label", 13*sc0)
	regionFace := r.face(r.fonts.regular, "map-region", 13*sc0)
	ms := math.Max(12*sc0, math.Min(20*sc0, k*3)) // marker size
	// label centres just above or below a marker
	above := func(y float64, f *text.GoTextFace) float64 { return y - ms*0.75 - f.Size*0.65 - 1 }
	below := func(y float64, f *text.GoTextFace) float64 { return y + ms*0.55 + f.Size*0.65 + 1 }
	halo := pre(cParchment, 0.9)

	type person struct {
		name string
		x, y int
		c    color.RGBA
	}
	var people []person
	for _, m := range sc.Self.Party {
		if m.LevelID != l.ID || (m.X == sc.Self.X && m.Y == sc.Self.Y) {
			continue
		}
		people = append(people, person{name: m.Name, x: m.X, y: m.Y, c: hex("#5ad06a")})
	}
	for _, e := range sc.Entities {
		if e.Kind == kindPlayer && e.ID != sc.YouID && !e.Ally && !e.Dead {
			people = append(people, person{name: e.Name, x: e.X, y: e.Y, c: hex(e.Color)})
		}
	}
	pulse := 0.5 + 0.5*math.Sin(r.t*4)
	sx, sy := at(sc.Self.X, sc.Self.Y)
	var places []proto.Place
	if l.ID == "overworld" {
		places = sc.Places
	}
	kindOf := func(p proto.Place) string {
		switch {
		case p.Kind == "village", p.Kind == "quest":
			return p.Kind
		case strings.HasPrefix(p.Kind, "dungeon"):
			return "dungeon"
		case strings.HasPrefix(p.Kind, "region:"):
			return "region"
		}
		return "sight"
	}

	// markers, the legend and the compass are reserved first so that no
	// text covers them; then labels are placed by importance
	box := func(x, y, s float64) image.Rectangle {
		return image.Rect(int(x-s*0.6), int(y-s*0.7), int(x+s*0.6), int(y+s*0.5))
	}
	labels.reserve(box(sx, sy, ms*1.4))
	for _, p := range places {
		if kindOf(p) != "region" {
			x, y := at(p.X, p.Y)
			labels.reserve(box(x, y, ms))
		}
	}
	for _, p := range people {
		x, y := at(p.x, p.y)
		labels.reserve(box(x, y, ms*0.8))
	}
	legend := r.legendRect(ox+12*sc0, oy+mh-12*sc0, small, ms, l.ID == "overworld")
	labels.reserve(legend)
	cx, cy := ox+mw-30*sc0, oy+mh-34*sc0
	labels.reserve(image.Rect(int(cx-26*sc0), int(cy-34*sc0), int(cx+26*sc0), int(cy+26*sc0)))

	type mapText struct {
		s      string
		face   *text.GoTextFace
		x, y   float64
		fg, bg color.RGBA
	}
	var texts, regions []mapText
	put := func(list *[]mapText, s string, face *text.GoTextFace, x, y float64, fg, bg color.RGBA) bool {
		if lx, ly, ok := labels.place(s, face, x, y); ok {
			*list = append(*list, mapText{s, face, lx, ly, fg, bg})
			return true
		}
		return false
	}
	for _, p := range places {
		if kindOf(p) == "village" {
			x, y := at(p.X, p.Y)
			if !put(&texts, p.Name, label, x, above(y, label), cInk, halo) {
				put(&texts, p.Name, label, x, below(y, label), cInk, halo)
			}
		}
	}
	for _, p := range places {
		if kindOf(p) == "quest" {
			x, y := at(p.X, p.Y)
			put(&texts, p.Name, small, x, below(y, small), hex("#8a1414"), halo)
		}
	}
	for _, p := range people {
		x, y := at(p.x, p.y)
		put(&texts, p.name, small, x, above(y, small), mul(p.c, 0.55), halo)
	}
	for _, kind := range []string{"dungeon", "sight"} {
		for _, p := range places {
			if kindOf(p) != kind {
				continue
			}
			x, y := at(p.X, p.Y)
			fg := cInkSoft
			if kind == "dungeon" {
				fg = hex("#5a1e14")
			}
			// below the marker, or above when that is taken
			if !put(&texts, p.Name, small, x, below(y, small), fg, halo) {
				put(&texts, p.Name, small, x, above(y, small), fg, halo)
			}
		}
	}
	for _, p := range places {
		if kindOf(p) != "region" {
			continue
		}
		x, y := at(p.X, p.Y)
		s := strings.ToUpper(i18n.T(p.Name))
		if len([]rune(s)) <= 10 {
			s = spaced(s)
		}
		for _, dy := range []float64{0, -ms, ms, -2 * ms, 2 * ms} {
			if put(&regions, s, regionFace, x, y+dy, pre(hex("#5a3e22"), 0.72), pre(cParchment, 0.45)) {
				break
			}
		}
	}

	// region names lie under everything else, like on old maps
	for _, t := range regions {
		inkText(dst, t.s, t.face, t.x, t.y, t.fg, t.bg)
	}
	for _, p := range places {
		x, y := at(p.X, p.Y)
		switch kindOf(p) {
		case "sight":
			markSight(dst, x, y, ms*0.9, p.Kind)
		case "dungeon":
			markDungeon(dst, x, y, ms, themeGlow(strings.TrimPrefix(p.Kind, "dungeon:")))
		}
	}
	for _, p := range places {
		if kindOf(p) == "village" {
			x, y := at(p.X, p.Y)
			markVillage(dst, x, y, ms*1.2)
		}
	}
	for _, p := range places {
		if kindOf(p) == "quest" {
			x, y := at(p.X, p.Y)
			markQuest(dst, x, y, ms, pulse)
		}
	}
	for _, p := range people {
		x, y := at(p.x, p.y)
		vector.FillCircle(dst, float32(x), float32(y), float32(ms*0.34), p.c, true)
		vector.StrokeCircle(dst, float32(x), float32(y), float32(ms*0.34), float32(math.Max(1, ms*0.1)), cInk, true)
	}
	for _, t := range texts {
		inkText(dst, t.s, t.face, t.x, t.y, t.fg, t.bg)
	}
	// you are here
	vector.StrokeCircle(dst, float32(sx), float32(sy), float32(ms*(0.8+0.6*pulse)), float32(math.Max(2, ms*0.14)), pre(hex("#e03020"), 1-0.6*pulse), true)
	markHero(dst, sx, sy, ms*0.95, hex("#ffd24a"), sc.Self.Facing)

	compass(dst, cx, cy, 18*sc0, small)
	r.mapLegend(dst, legend, small, ms, l.ID == "overworld")
}

type legendRow struct {
	name string
	draw func(dst *ebiten.Image, x, y, ms float64)
}

func legendRows(overworld bool) []legendRow {
	rows := []legendRow{
		{"Вы", func(dst *ebiten.Image, x, y, ms float64) { markHero(dst, x, y, ms*0.6, hex("#ffd24a"), 1) }},
		{"Группа", func(dst *ebiten.Image, x, y, ms float64) {
			vector.FillCircle(dst, float32(x), float32(y), float32(ms*0.3), hex("#5ad06a"), true)
			vector.StrokeCircle(dst, float32(x), float32(y), float32(ms*0.3), float32(math.Max(1, ms*0.1)), cInk, true)
		}},
	}
	if overworld {
		rows = append(rows,
			legendRow{"Деревня", func(dst *ebiten.Image, x, y, ms float64) { markVillage(dst, x, y+ms*0.1, ms*0.9) }},
			legendRow{"Подземелье", func(dst *ebiten.Image, x, y, ms float64) { markDungeon(dst, x, y, ms*0.85, hex("#ff7a3a")) }},
			legendRow{"Место силы", func(dst *ebiten.Image, x, y, ms float64) { markSight(dst, x, y, ms*0.8, "shrine") }},
			legendRow{"Цель задания", func(dst *ebiten.Image, x, y, ms float64) { markQuest(dst, x, y+ms*0.35, ms*0.75, 0) }},
		)
	}
	return rows
}

func (r *worldRenderer) legendLine(ms float64) float64 { return math.Max(ms*1.25, 18*r.scale) }

// legendRect is where the legend goes: its lower left corner at (x, bottom).
func (r *worldRenderer) legendRect(x, bottom float64, face *text.GoTextFace, ms float64, overworld bool) image.Rectangle {
	rows := legendRows(overworld)
	w := 0.0
	for _, rw := range rows {
		w = math.Max(w, text.Advance(i18n.T(rw.name), face))
	}
	bw, bh := w+ms*1.6+20*r.scale, float64(len(rows))*r.legendLine(ms)+12*r.scale
	return image.Rect(int(x), int(bottom-bh), int(x+bw), int(bottom))
}

// mapLegend explains the markers.
func (r *worldRenderer) mapLegend(dst *ebiten.Image, at image.Rectangle, face *text.GoTextFace, ms float64, overworld bool) {
	sc0 := r.scale
	x, by := float64(at.Min.X), float64(at.Min.Y)
	roundRect(dst, float32(x), float32(by), float32(at.Dx()), float32(at.Dy()), float32(5*sc0), pre(cParchment, 0.92))
	vector.StrokeRect(dst, float32(x), float32(by), float32(at.Dx()), float32(at.Dy()), float32(sc0), cInkSoft, true)
	lh := r.legendLine(ms)
	for i, rw := range legendRows(overworld) {
		cy := by + 6*sc0 + (float64(i)+0.5)*lh
		rw.draw(dst, x+8*sc0+ms*0.6, cy, ms)
		op := &text.DrawOptions{}
		m := face.Metrics()
		op.GeoM.Translate(x+12*sc0+ms*1.3, cy-(m.HAscent+m.HDescent)/2)
		op.ColorScale.ScaleWithColor(cInk)
		text.Draw(dst, i18n.T(rw.name), face, op)
	}
}

func themeGlow(theme string) color.RGBA {
	switch theme {
	case "ice":
		return hex("#7ad0ff")
	case "crypt":
		return hex("#8aff7a")
	case "volcano":
		return hex("#ff6a1a")
	case "fortress":
		return hex("#c05aff")
	case "temple":
		return hex("#ffe08a")
	}
	return hex("#ff9a3a")
}

// spaced puts thin gaps between letters: "ПУСТОШИ" → "П У С Т О Ш И".
func spaced(s string) string {
	rs := []rune(s)
	var b strings.Builder
	for i, r := range rs {
		if i > 0 {
			b.WriteRune(' ')
		}
		b.WriteRune(r)
	}
	return b.String()
}
