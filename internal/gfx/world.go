package gfx

import (
	"hash/fnv"
	"image"
	"image/color"
	"math"
	"math/rand/v2"
	"sort"
	"strconv"
	"strings"

	"github.com/hajimehoshi/ebiten/v2"
	"github.com/hajimehoshi/ebiten/v2/text/v2"
	"github.com/hajimehoshi/ebiten/v2/vector"

	"ratas/internal/client"
	"ratas/internal/content"
	"ratas/internal/game"
	"ratas/internal/gen"
	"ratas/internal/i18n"
	"ratas/internal/proto"
	"ratas/internal/world"
)

const (
	kindPlayer     = uint8(game.KPlayer)
	kindMonster    = uint8(game.KMonster)
	kindNPC        = uint8(game.KNPC)
	kindItem       = uint8(game.KItem)
	kindProjectile = uint8(game.KProjectile)
)

// tile sizes in logical pixels; multiples of the 16px art keep pixels crisp
var zoomLevels = []float64{32, 48, 64}

type entState struct {
	x, y   float64 // drawn position (cells), gliding between the server's cells
	vx, vy float64 // drawn velocity (cells per second)
	tx, ty int     // the last cell the server reported
	seg    segment // the curve being followed
	moveAt float64 // when the last step started
	stepS  float64 // seconds per step at the creature's pace
	walk   float64 // distance walked: drives walking frames and bobbing
	alpha  float64 // fades in and out of sight
	fx, fy float64 // facing direction
	swing  uint8   // last seen attack counter
	lunge  float64 // attack animation, 1 → 0
	flash  float64
	trail  [][2]float64
	gen    int
	left   bool // facing left (sprites are mirrored)
	you    bool // the player's own hero: turns sharper, it answers the keys
}

type floatText struct {
	x, y, t, life float64
	text          string
	col           color.RGBA
	big           bool
}

type particle struct {
	x, y, vx, vy  float64
	t, life, size float64
	grav          float64
	col           color.RGBA
	glow          bool
}

type flashLight struct {
	x, y, t, life, radius float64
	col                   color.RGBA
}

type worldRenderer struct {
	atlas   worldMap // the painted world map
	fonts   *fonts
	scale   float64
	zoomIdx int
	sprites map[string]*spriteSet
	models  map[string]*model
	icons   map[string]*ebiten.Image
	tops    map[uint32]float64 // screen y of the top of each drawn creature
	faces   map[string]*text.GoTextFace
	dot     *ebiten.Image
	vig     *ebiten.Image
	dark    *ebiten.Image
	darkPix []byte

	ents       map[uint32]*entState
	gen        int
	levelID    string
	camX, camY float64
	camOK      bool
	zoomCur    float64   // tile size now (glides to the chosen zoom level)
	dt         float64   // length of the last frame
	darkCur    []float32 // darkness of every tile, fading to its target
	darkID     string    // level of darkCur
	texts      []floatText
	parts      []particle
	lights     []flashLight
	ambient    []particle
	t          float64
	rng        *rand.Rand
	lastScene  *client.Scene

	vis    []bool
	visKey [5]int
	visLvl *world.Level

	lastView *view // the last drawn view: maps the mouse cursor to tiles
}

func newWorldRenderer(f *fonts) *worldRenderer {
	return &worldRenderer{
		fonts: f, scale: 1,
		sprites: map[string]*spriteSet{}, models: map[string]*model{}, icons: map[string]*ebiten.Image{},
		tops: map[uint32]float64{}, faces: map[string]*text.GoTextFace{},
		dot: softDot(64), vig: vignetteImage(256), ents: map[uint32]*entState{},
		rng: rand.New(rand.NewPCG(1, 2)),
	}
}

func (r *worldRenderer) setScale(s float64) {
	r.scale = s
	r.faces = map[string]*text.GoTextFace{}
}

func (r *worldRenderer) zoom(dir float64) {
	if dir > 0 && r.zoomIdx < len(zoomLevels)-1 {
		r.zoomIdx++
	} else if dir < 0 && r.zoomIdx > 0 {
		r.zoomIdx--
	} else {
		return
	}
	r.faces = map[string]*text.GoTextFace{}
}

func (r *worldRenderer) tileSize() int {
	z := r.zoomCur
	if z == 0 {
		z = zoomLevels[r.zoomIdx]
	}
	return int(math.Round(z * r.scale))
}

func (r *worldRenderer) face(src *text.GoTextFaceSource, name string, size float64) *text.GoTextFace {
	key := name + strconv.Itoa(int(size))
	f, ok := r.faces[key]
	if !ok {
		f = &text.GoTextFace{Source: src, Size: math.Max(8, size)}
		r.faces[key] = f
	}
	return f
}

func daylight(t float64) float64 {
	switch {
	case t >= 0.3 && t <= 0.7:
		return 1
	case t >= 0.85 || t <= 0.15:
		return 0
	case t < 0.3:
		return (t - 0.15) / 0.15
	default:
		return (0.85 - t) / 0.15
	}
}

// ---- simulation of visuals ----

func (r *worldRenderer) update(dt float64, sc *client.Scene, fx []proto.FX) {
	r.t += dt
	if sc.Level.ID != r.levelID {
		r.levelID = sc.Level.ID
		r.ents = map[uint32]*entState{}
		r.camOK = false
		r.texts, r.parts, r.lights, r.ambient = nil, nil, nil, nil
	}
	if sc != r.lastScene {
		// new steps start where the creatures were drawn last frame
		r.syncEnts(sc, r.t-dt)
		r.lastScene = sc
	}
	r.dt = dt
	r.animate(dt, sc)
	// zoom glides between levels
	want := zoomLevels[r.zoomIdx]
	if r.zoomCur == 0 || math.Abs(want-r.zoomCur) < 0.25 {
		r.zoomCur = want
	} else {
		r.zoomCur += (want - r.zoomCur) * (1 - math.Exp(-dt*12))
	}
	tx, ty := float64(sc.Self.X), float64(sc.Self.Y)
	if me := r.ents[sc.YouID]; me != nil {
		tx, ty = me.x, me.y
	}
	if !r.camOK || math.Abs(tx-r.camX)+math.Abs(ty-r.camY) > 12 {
		r.camX, r.camY, r.camOK = tx, ty, true
	} else {
		kc := 1 - math.Exp(-dt*7)
		r.camX += (tx - r.camX) * kc
		r.camY += (ty - r.camY) * kc
	}
	for _, f := range fx {
		r.spawnFX(f)
	}
	r.statusParticles(dt, sc)
	r.updateEffects(dt)
	r.updateAmbient(dt, sc.Level, sc.TimeOfDay)
}

func (r *worldRenderer) burst(x, y float64, n int, c color.RGBA, speed, grav, size float64, glow bool) {
	for i := 0; i < n; i++ {
		a := r.rng.Float64() * 2 * math.Pi
		s := speed * (0.4 + r.rng.Float64()*0.8)
		r.parts = append(r.parts, particle{
			x: x, y: y, vx: math.Cos(a) * s, vy: math.Sin(a)*s - grav*0.08,
			life: 0.35 + r.rng.Float64()*0.45, size: size * (0.7 + r.rng.Float64()*0.6), grav: grav, col: c, glow: glow,
		})
	}
}

func (r *worldRenderer) spawnFX(f proto.FX) {
	cx, cy := float64(f.X)+0.5, float64(f.Y)+0.5
	c := hex(f.Color)
	if f.Text != "" {
		big := strings.Contains(f.Text, "УРОВЕНЬ")
		r.texts = append(r.texts, floatText{
			x: cx + (r.rng.Float64()-0.5)*0.4, y: cy - 0.6,
			life: math.Max(0.8, float64(f.Ms)/1000), text: f.Text, col: c, big: big,
		})
		for _, st := range r.ents {
			if st.tx == f.X && st.ty == f.Y && st.trail == nil {
				st.flash = 0.16
			}
		}
		if big {
			r.burst(cx, cy, 40, color.RGBA{255, 230, 90, 255}, 4, 0, 0.16, true)
			r.lights = append(r.lights, flashLight{cx, cy, 0, 0.9, 4, color.RGBA{255, 220, 100, 255}})
		}
		return
	}
	switch f.Glyph {
	case '%':
		r.burst(cx, cy, 18, mix(c, black, 0.2), 3, 7, 0.13, false)
	case '~':
		r.burst(cx, cy, 4, c, 1, 0, 0.18, true)
	default:
		r.burst(cx, cy, 7, c, 3.2, 0, 0.11, true)
		r.lights = append(r.lights, flashLight{cx, cy, 0, 0.28, 1.6, c})
	}
}

func (r *worldRenderer) updateEffects(dt float64) {
	keepP := r.parts[:0]
	for _, p := range r.parts {
		p.t += dt
		if p.t >= p.life {
			continue
		}
		p.vy += p.grav * dt
		p.x += p.vx * dt
		p.y += p.vy * dt
		p.vx *= math.Exp(-dt * 3)
		keepP = append(keepP, p)
	}
	r.parts = keepP
	keepT := r.texts[:0]
	for _, t := range r.texts {
		t.t += dt
		if t.t < t.life {
			keepT = append(keepT, t)
		}
	}
	r.texts = keepT
	keepL := r.lights[:0]
	for _, l := range r.lights {
		l.t += dt
		if l.t < l.life {
			keepL = append(keepL, l)
		}
	}
	r.lights = keepL
}

// updateAmbient keeps fireflies around at night and dust motes underground.
func (r *worldRenderer) updateAmbient(dt float64, l *world.Level, tod float64) {
	want := 0
	var c color.RGBA
	switch {
	case l.Lit && daylight(tod) < 0.4:
		want, c = 26, color.RGBA{190, 255, 120, 255}
	case !l.Lit:
		want, c = 30, color.RGBA{200, 190, 170, 255}
	}
	keep := r.ambient[:0]
	for _, p := range r.ambient {
		p.t += dt
		p.x += p.vx * dt
		p.y += p.vy * dt
		p.vx += (r.rng.Float64() - 0.5) * dt * 2
		p.vy += (r.rng.Float64() - 0.5) * dt * 2
		if p.t < p.life && math.Abs(p.x-r.camX) < 16 && math.Abs(p.y-r.camY) < 11 && len(keep) < want {
			keep = append(keep, p)
		}
	}
	r.ambient = keep
	for len(r.ambient) < want {
		r.ambient = append(r.ambient, particle{
			x: r.camX + (r.rng.Float64()-0.5)*30, y: r.camY + (r.rng.Float64()-0.5)*20,
			vx: (r.rng.Float64() - 0.5) * 0.6, vy: (r.rng.Float64() - 0.5) * 0.6,
			life: 4 + r.rng.Float64()*6, size: 0.12, col: c, glow: l.Lit,
		})
	}
}

// ---- drawing ----

// view describes what to draw; the menu backdrop uses it without a scene.
type view struct {
	l        *world.Level
	vis      []bool       // nil = everything visible
	explored world.Bitset // nil = everything explored
	tod      float64
	ox, oy   float64 // screen position of tile (0,0)
	ts       int
	tx0, ty0 int
	tx1, ty1 int
}

func (v *view) visible(i int) bool   { return v.vis == nil || v.vis[i] }
func (v *view) explored_(i int) bool { return v.explored == nil || v.explored.Get(i) }

func tileHash(x, y int) int {
	h := fnv.New32a()
	h.Write([]byte{byte(x), byte(x >> 8), byte(y), byte(y >> 8)})
	return int(h.Sum32() >> 1)
}

func (r *worldRenderer) fov(sc *client.Scene) []bool {
	l := sc.Level
	key := [5]int{sc.LevelVer, sc.Self.X, sc.Self.Y, sc.Self.Vision, boolInt(sc.Self.Dead)}
	if r.visLvl == l && r.visKey == key {
		return r.vis
	}
	if len(r.vis) != l.W*l.H {
		r.vis = make([]bool, l.W*l.H)
	} else {
		clear(r.vis)
	}
	if !sc.Self.Dead {
		world.FOV(l, sc.Self.X, sc.Self.Y, sc.Self.Vision, func(x, y int) { r.vis[y*l.W+x] = true })
	}
	r.visLvl, r.visKey = l, key
	return r.vis
}

func boolInt(b bool) int {
	if b {
		return 1
	}
	return 0
}

func (r *worldRenderer) makeView(dst *ebiten.Image, l *world.Level, camX, camY float64, focus image.Point, area image.Rectangle) *view {
	ts := r.tileSize()
	W, H := dst.Bounds().Dx(), dst.Bounds().Dy()
	hx := float64(area.Dx()) / 2 / float64(ts)
	hy := float64(area.Dy()) / 2 / float64(ts)
	if float64(l.W) > 2*hx {
		camX = math.Max(hx-0.5, math.Min(float64(l.W)-hx-0.5, camX))
	} else {
		camX = float64(l.W)/2 - 0.5
	}
	if float64(l.H) > 2*hy {
		camY = math.Max(hy-0.5, math.Min(float64(l.H)-hy-0.5, camY))
	} else {
		camY = float64(l.H)/2 - 0.5
	}
	v := &view{l: l, ts: ts}
	v.ox = math.Floor(float64(focus.X) - (camX+0.5)*float64(ts))
	v.oy = math.Floor(float64(focus.Y) - (camY+0.5)*float64(ts))
	v.tx0 = max(0, int(math.Floor(-v.ox/float64(ts)))-1)
	v.ty0 = max(0, int(math.Floor(-v.oy/float64(ts)))-1)
	v.tx1 = min(l.W-1, int(math.Ceil((float64(W)-v.ox)/float64(ts)))+1)
	v.ty1 = min(l.H-1, int(math.Ceil((float64(H)-v.oy)/float64(ts)))+2)
	return v
}

func (v *view) px(x, y float64) (float64, float64) {
	return v.ox + x*float64(v.ts), v.oy + y*float64(v.ts)
}

var memoryTint = [3]float32{0.86, 0.9, 1}

func (r *worldRenderer) drawImg(dst, img *ebiten.Image, x, y, scale float64, memory bool) {
	op := &ebiten.DrawImageOptions{}
	op.GeoM.Scale(scale, scale)
	op.GeoM.Translate(x, y)
	if memory {
		// remembered tiles: slightly cold, the darkness map does the rest
		op.ColorScale.Scale(memoryTint[0], memoryTint[1], memoryTint[2], 1)
	}
	dst.DrawImage(img, op)
}

// swaying trees bend in the wind by this much (shear of the crown).
var swaying = map[string]float64{
	"tree": 0.035, "pine": 0.025, "snow_pine": 0.02, "palm": 0.05, "dead_tree": 0.015,
	"twisted_tree": 0.02, "charred_tree": 0.01,
}

// drawSway draws a tall object sheared around its foot.
func (r *worldRenderer) drawSway(dst, img *ebiten.Image, x, y, scale float64, memory bool, shear float64) {
	h := float64(img.Bounds().Dy()) * scale
	op := &ebiten.DrawImageOptions{}
	op.GeoM.Scale(scale, scale)
	op.GeoM.Translate(0, -h)
	op.GeoM.Skew(shear, 0)
	op.GeoM.Translate(x, y+h)
	if memory {
		op.ColorScale.Scale(memoryTint[0], memoryTint[1], memoryTint[2], 1)
	}
	dst.DrawImage(img, op)
}

// notBase are ground tiles that are details rather than the floor itself.
var notBase = map[string]bool{
	"road": true, "bridge": true, "carpet": true, "web": true, "rubble": true, "bones": true, "magma_crack": true,
	"mushrooms": true, "flowers": true, "lava": true, "water": true, "deep_water": true, "dungeon": true,
	"stairs_up": true, "stairs_down": true, "door_open": true, "void": true,
}

// groundUnder picks the ground drawn under an object: by its biome, or, for
// objects without one (gravestones, chests, braziers), the floor next to it.
func groundUnder(l *world.Level, x, y int, def *content.TileDef) *content.TileDef {
	if def.Biome == "" {
		for _, d := range world.AllDirs {
			dd := d.Delta()
			if !l.In(x+dd.X, y+dd.Y) {
				continue
			}
			n := content.Tile(l.At(x+dd.X, y+dd.Y))
			if isKnownGround(n.Key) && !notBase[n.Key] {
				return n
			}
		}
	}
	return groundTile(baseGround(def, l.Theme))
}

func groundTile(key string) *content.TileDef {
	if !content.HasTile(key) {
		return nil
	}
	return content.Tile(content.TileID(key))
}

// drawTiles draws the ground pass, then tall objects and entities row by row
// so that canopies and walls correctly hide what stands behind them.
func (r *worldRenderer) drawTiles(dst *ebiten.Image, v *view, rowEntities func(row int)) {
	l := v.l
	ts := float64(v.ts)
	k := ts / spx
	frame := int(r.t*3) % 4
	for y := v.ty0; y <= v.ty1; y++ {
		for x := v.tx0; x <= v.tx1; x++ {
			i := y*l.W + x
			if !v.explored_(i) {
				continue
			}
			def := content.Tile(l.Tiles[i])
			ss := r.spriteFor(def)
			mem := !v.visible(i)
			px, py := v.px(float64(x), float64(y))
			h := tileHash(x, y)
			if ss.ground != nil {
				g := ss.ground[h%len(ss.ground)]
				r.drawImg(dst, g[frame%len(g)], px, py, k, mem)
			} else if !ss.wall {
				if base := groundUnder(l, x, y, def); base != nil {
					bs := r.spriteFor(base)
					if bs.ground != nil {
						g := bs.ground[h%len(bs.ground)]
						r.drawImg(dst, g[frame%len(g)], px, py, k, mem)
					}
				}
			}
			if ss.object != nil && !ss.tall {
				obj := ss.object[h%len(ss.object)]
				r.drawImg(dst, obj, px, py, ts/float64(obj.Bounds().Dx()), mem)
			}
		}
	}
	for y := v.ty0; y <= v.ty1; y++ {
		for x := v.tx0; x <= v.tx1; x++ {
			i := y*l.W + x
			if !v.explored_(i) {
				continue
			}
			def := content.Tile(l.Tiles[i])
			ss := r.spriteFor(def)
			if ss.object == nil || !ss.tall {
				continue
			}
			px, py := v.px(float64(x), float64(y))
			img := ss.object[tileHash(x, y)%len(ss.object)]
			if amp := swaying[def.Key]; amp > 0 && l.Lit {
				// wind: a slow wave rolls over the woods, each tree a little out of step
				w := math.Sin(r.t*1.1-float64(x)*0.32-float64(y)*0.18) + 0.35*math.Sin(r.t*2.9+float64(tileHash(x, y)%17))
				r.drawSway(dst, img, px, py-ts/2, k, !v.visible(i), amp*w)
				continue
			}
			r.drawImg(dst, img, px, py-ts/2, k, !v.visible(i))
		}
		if rowEntities != nil {
			rowEntities(y)
		}
	}
}

func (r *worldRenderer) glow(dst *ebiten.Image, x, y, radius float64, c color.RGBA, a float64) {
	if a <= 0 || radius <= 0 {
		return
	}
	op := &ebiten.DrawImageOptions{}
	s := radius * 2 / 64
	op.GeoM.Scale(s, s)
	op.GeoM.Translate(x-radius, y-radius)
	op.ColorScale.Scale(float32(c.R)/255*float32(a), float32(c.G)/255*float32(a), float32(c.B)/255*float32(a), float32(a))
	op.Blend = ebiten.BlendLighter
	op.Filter = ebiten.FilterLinear
	dst.DrawImage(r.dot, op)
}

func (r *worldRenderer) shadow(dst *ebiten.Image, x, y, w, h, a float64) {
	op := &ebiten.DrawImageOptions{}
	op.GeoM.Scale(w/64, h/64)
	op.GeoM.Translate(x-w/2, y-h/2)
	op.ColorScale.Scale(0, 0, 0, float32(a))
	op.Filter = ebiten.FilterLinear
	dst.DrawImage(r.dot, op)
}

func (r *worldRenderer) draw(dst *ebiten.Image, sc *client.Scene, area image.Rectangle, scale float64) {
	l := sc.Level
	clear(r.tops)
	focus := image.Pt((area.Min.X+area.Max.X)/2, (area.Min.Y+area.Max.Y)/2)
	v := r.makeView(dst, l, r.camX, r.camY, focus, area)
	v.vis, v.explored, v.tod = r.fov(sc), sc.Explored, sc.TimeOfDay
	r.lastView = v
	ts := float64(v.ts)

	// entities grouped by row for depth sorting
	rows := map[int][]proto.EntityView{}
	var projectiles []proto.EntityView
	for _, e := range sc.Entities {
		if !l.In(e.X, e.Y) {
			continue
		}
		if e.Kind == kindProjectile {
			if v.vis[e.Y*l.W+e.X] {
				projectiles = append(projectiles, e)
			}
			continue
		}
		st := r.ents[e.ID]
		if st == nil || st.alpha < 0.02 {
			continue
		}
		row := int(math.Floor(st.y + 0.5))
		rows[row] = append(rows[row], e)
	}
	for _, list := range rows {
		sort.Slice(list, func(i, j int) bool {
			if (list[i].Kind == kindItem) != (list[j].Kind == kindItem) {
				return list[i].Kind == kindItem
			}
			if list[i].Dead != list[j].Dead {
				return list[i].Dead // bodies lie under the living
			}
			return list[i].X < list[j].X
		})
	}
	r.drawTiles(dst, v, func(row int) {
		for _, e := range rows[row] {
			r.drawEntity(dst, v, sc, e)
		}
	})

	// projectiles with glowing trails
	for _, e := range projectiles {
		st := r.ents[e.ID]
		if st == nil {
			continue
		}
		c := hex(e.Color)
		for i, p := range st.trail {
			k := float64(i+1) / float64(len(st.trail)+1)
			x, y := v.px(p[0]+0.5, p[1]+0.5)
			r.glow(dst, x, y, ts*0.18*k, c, 0.5*k)
		}
		x, y := v.px(st.x+0.5, st.y+0.5)
		r.glow(dst, x, y, ts*0.32, c, 0.9)
		r.glow(dst, x, y, ts*0.12, white, 0.9)
	}
	// particles
	for _, p := range r.parts {
		x, y := v.px(p.x, p.y)
		a := 1 - p.t/p.life
		if p.glow {
			r.glow(dst, x, y, p.size*ts*1.6, p.col, a)
		} else {
			vector.FillRect(dst, float32(x), float32(y), float32(p.size*ts*0.6), float32(p.size*ts*0.6), alpha(mul(p.col, a), uint8(255*a)), false)
		}
	}
	r.drawLighting(dst, v, sc)
	r.drawOverlays(dst, v, sc, area)
}

func (r *worldRenderer) drawEntity(dst *ebiten.Image, v *view, sc *client.Scene, e proto.EntityView) {
	st := r.ents[e.ID]
	ts := float64(v.ts)
	c := hex(e.Color)
	al := st.alpha // fading in and out of sight
	frame, bob, lx, ly := st.pose()
	cx, cy := v.px(st.x+0.5+lx, st.y+0.5+ly)
	if e.Kind == kindItem {
		bob := math.Sin(r.t*3+float64(e.ID)) * ts * 0.05
		r.shadow(dst, cx, cy+ts*0.3, ts*0.5, ts*0.18, 0.35*al)
		r.glow(dst, cx, cy+bob, ts*0.42, c, 0.3*al)
		if e.Rarity >= uint8(game.Uncommon) {
			// rare loot shines in its rarity color; legendary with a beam of light
			rc := hex(game.RarityColor(int(e.Rarity)))
			pulse := 0.75 + 0.25*math.Sin(r.t*3+float64(e.ID))
			r.glow(dst, cx, cy+bob, ts*(0.45+0.12*float64(e.Rarity)), rc, (0.18+0.1*float64(e.Rarity))*pulse*al)
			if e.Rarity >= uint8(game.Epic) {
				for i := 1; i <= 4; i++ {
					r.glow(dst, cx, cy-ts*0.35*float64(i), ts*0.3, rc, 0.16*pulse/float64(i)*al)
				}
			}
			c = hex(game.RarityColor(0))
			if d := content.Item(e.Def); d != nil {
				c = hex(d.Color)
			}
		}
		icon := r.icon(e.Glyph, c, e.Def)
		k := ts / spx * 0.85
		op := &ebiten.DrawImageOptions{}
		op.GeoM.Scale(k, k)
		op.GeoM.Translate(math.Round(cx-float64(icon.Bounds().Dx())*k/2), math.Round(cy+bob-float64(icon.Bounds().Dy())*k/2))
		op.ColorScale.ScaleAlpha(float32(al))
		dst.DrawImage(icon, op)
		return
	}
	m := r.modelOf(e)
	k := ts / spx
	if e.Boss {
		k *= 1.5
	}
	if e.Dead {
		r.drawBody(dst, m.frames[0], cx, cy+ts*0.44, k, ts)
		return
	}
	img := m.frames[frame]
	w, h := float64(img.Bounds().Dx())*k, float64(img.Bounds().Dy())*k
	feet := cy + ts*0.44
	lift := bob * ts * 0.07 // a small hop with every footfall
	if m.float {
		lift = ts*0.16 + math.Sin(r.t*2.6+float64(e.ID))*ts*0.06
	}
	hidden := e.Status&proto.StatusStealth != 0
	illusion := e.Status&proto.StatusIllusion != 0
	if !hidden {
		r.shadow(dst, cx, feet-ts*0.04, math.Max(ts*0.55, w*0.8)*(1-bob*0.12), ts*0.24, 0.5*al)
	}
	switch {
	case e.ID == sc.YouID:
		ellipse(dst, cx, feet-ts*0.04, ts*0.36, ts*0.12, math.Max(1, ts*0.04), pre(color.RGBA{255, 214, 110, 255}, 0.8*al))
	case e.Ally:
		// party members and own summons: a green ring
		ellipse(dst, cx, feet-ts*0.04, ts*0.36, ts*0.12, math.Max(1, ts*0.035), pre(color.RGBA{110, 230, 120, 255}, 0.85*al))
	case e.Kind == kindPlayer && e.Hostile && !illusion:
		ellipse(dst, cx, feet-ts*0.04, ts*0.36, ts*0.12, math.Max(1, ts*0.035), pre(color.RGBA{240, 80, 70, 255}, 0.8*al))
	case e.Kind == kindPlayer:
		ellipse(dst, cx, feet-ts*0.04, ts*0.36, ts*0.12, math.Max(1, ts*0.035), pre(c, 0.8*al))
	}
	if e.Boss {
		r.glow(dst, cx, feet-h*0.45, math.Max(w, h)*0.9, c, 0.16+0.06*math.Sin(r.t*3))
	}
	if e.Status&proto.StatusShielded != 0 {
		r.glow(dst, cx, feet-h*0.5, math.Max(w, h)*0.75, color.RGBA{255, 230, 140, 255}, 0.22+0.05*math.Sin(r.t*5))
	}
	top := math.Round(feet - h - lift)
	op := &ebiten.DrawImageOptions{}
	if st.left {
		op.GeoM.Scale(-k, k)
		op.GeoM.Translate(math.Round(cx+w/2), top)
	} else {
		op.GeoM.Scale(k, k)
		op.GeoM.Translate(math.Round(cx-w/2), top)
	}
	switch {
	case e.Status&proto.StatusChilled != 0:
		op.ColorScale.Scale(0.72, 0.88, 1.18, 1)
	case e.Status&proto.StatusPoisoned != 0:
		op.ColorScale.Scale(0.8, 1.06, 0.72, 1)
	}
	op.ColorScale.ScaleAlpha(float32(al))
	switch {
	case hidden:
		// in stealth: a faint shimmer only allies can see
		op.ColorScale.ScaleAlpha(float32(0.32 + 0.08*math.Sin(r.t*4+float64(e.ID))))
	case illusion:
		op.ColorScale.Scale(0.9, 0.82, 1.1, 1)
		op.ColorScale.ScaleAlpha(float32(0.72 + 0.12*math.Sin(r.t*9+float64(e.ID))))
	}
	dst.DrawImage(img, op)
	if illusion && math.Sin(r.t*3+float64(e.ID)) > 0.85 {
		// illusions flicker now and then
		op.GeoM.Translate(math.Round(ts*0.05), 0)
		op.ColorScale.ScaleAlpha(0.35)
		dst.DrawImage(img, op)
	}
	if st.flash > 0 {
		op.Blend = ebiten.BlendLighter
		a := float32(st.flash / 0.16)
		op.ColorScale.Reset()
		op.ColorScale.Scale(a, a, a, a)
		dst.DrawImage(img, op)
	}
	if e.Status&proto.StatusBurning != 0 {
		r.glow(dst, cx, feet-h*0.4, w*0.8, color.RGBA{255, 120, 30, 255}, 0.3+0.1*math.Sin(r.t*13))
	}
	if e.Status&proto.StatusSilenced != 0 {
		// a crossed-out mouth sign above the head
		sx, sy := float32(cx+ts*0.24), float32(top+ts*0.04)
		rad := float32(math.Max(3, ts*0.09))
		vector.FillCircle(dst, sx, sy, rad, color.RGBA{40, 10, 60, 200}, true)
		vector.StrokeCircle(dst, sx, sy, rad, float32(math.Max(1, ts*0.025)), color.RGBA{200, 120, 255, 255}, true)
		vector.StrokeLine(dst, sx-rad*0.7, sy+rad*0.7, sx+rad*0.7, sy-rad*0.7, float32(math.Max(1, ts*0.025)), color.RGBA{200, 120, 255, 255}, true)
	}
	if e.Status&proto.StatusStunned != 0 {
		for i := 0; i < 3; i++ {
			a := r.t*4 + float64(i)*2*math.Pi/3
			sx, sy := cx+math.Cos(a)*ts*0.24, top-ts*0.06+math.Sin(a)*ts*0.07
			vector.FillCircle(dst, float32(sx), float32(sy), float32(math.Max(1.5, ts*0.045)), color.RGBA{255, 230, 90, 255}, true)
		}
	}
	r.tops[e.ID] = top
}

// ellipse strokes an ellipse outline.
func ellipse(dst *ebiten.Image, cx, cy, rx, ry, width float64, c color.RGBA) {
	var p vector.Path
	const n = 28
	for i := 0; i <= n; i++ {
		a := float64(i) / n * 2 * math.Pi
		x, y := float32(cx+math.Cos(a)*rx), float32(cy+math.Sin(a)*ry)
		if i == 0 {
			p.MoveTo(x, y)
		} else {
			p.LineTo(x, y)
		}
	}
	op := &vector.DrawPathOptions{AntiAlias: true}
	op.ColorScale.ScaleWithColor(c)
	vector.StrokePath(dst, &p, &vector.StrokeOptions{Width: float32(width)}, op)
}

// statusParticles: flames, poison bubbles, blood, curses and frost around
// creatures under effects.
func (r *worldRenderer) statusParticles(dt float64, sc *client.Scene) {
	for _, e := range sc.Entities {
		if e.Status == 0 {
			continue
		}
		st := r.ents[e.ID]
		if st == nil {
			continue
		}
		x, y := st.x+0.5, st.y+0.25
		emit := func(rate float64, c color.RGBA, vy, grav, size float64, glow bool) {
			if r.rng.Float64() < rate*dt {
				r.parts = append(r.parts, particle{
					x: x + (r.rng.Float64()-0.5)*0.55, y: y + (r.rng.Float64()-0.5)*0.4,
					vx: (r.rng.Float64() - 0.5) * 0.4, vy: vy * (0.6 + r.rng.Float64()*0.8),
					life: 0.45 + r.rng.Float64()*0.45, size: size, grav: grav, col: c, glow: glow,
				})
			}
		}
		if e.Status&proto.StatusBurning != 0 {
			emit(16, color.RGBA{255, 140, 40, 255}, -1.4, -0.6, 0.09, true)
		}
		if e.Status&proto.StatusPoisoned != 0 {
			emit(7, color.RGBA{150, 230, 70, 255}, -0.7, 0, 0.07, true)
		}
		if e.Status&proto.StatusBleeding != 0 {
			emit(7, color.RGBA{190, 20, 20, 255}, 0.4, 5, 0.1, false)
		}
		if e.Status&proto.StatusCursed != 0 {
			emit(6, color.RGBA{180, 90, 255, 255}, -0.5, 0, 0.07, true)
		}
		if e.Status&proto.StatusChilled != 0 {
			emit(5, color.RGBA{200, 240, 255, 255}, 0.35, 0, 0.06, true)
		}
		if e.Status&proto.StatusHoly != 0 {
			emit(10, color.RGBA{255, 236, 150, 255}, -1, -0.3, 0.07, true)
		}
	}
}

func (r *worldRenderer) darkness(v *view, sc *client.Scene, x, y int) float64 {
	l := v.l
	if !l.In(x, y) {
		return 1
	}
	i := y*l.W + x
	if !v.explored_(i) {
		return 1
	}
	if !v.visible(i) {
		return 0.62
	}
	vision := math.Max(1, float64(sc.Self.Vision))
	sx, sy := float64(sc.Self.X), float64(sc.Self.Y)
	if me := r.ents[sc.YouID]; me != nil {
		sx, sy = me.x, me.y // the light moves with the hero, not by cells
	}
	d := math.Hypot(float64(x)-sx, float64(y)-sy) / vision
	if l.Lit {
		night := 1 - daylight(v.tod)
		return night * (0.18 + 0.6*d*d)
	}
	return math.Min(0.86, 0.04+0.82*math.Pow(d, 1.5))
}

func (r *worldRenderer) drawLighting(dst *ebiten.Image, v *view, sc *client.Scene) {
	l := v.l
	ts := float64(v.ts)
	W, H := dst.Bounds().Dx(), dst.Bounds().Dy()
	day := 1.0
	if l.Lit {
		day = daylight(v.tod)
		// cloud shadows drifting over the land
		if day > 0.2 {
			cr := rand.New(rand.NewPCG(uint64(len(l.ID)), 5))
			for i := 0; i < 9; i++ {
				bx := cr.Float64()*float64(l.W) + r.t*0.35
				by := cr.Float64()*float64(l.H) + r.t*0.12
				bx = math.Mod(bx, float64(l.W))
				by = math.Mod(by, float64(l.H))
				x, y := v.px(bx, by)
				s := (6 + cr.Float64()*6) * ts
				r.shadow(dst, x, y, s*1.6, s, 0.13*day)
			}
		}
		night := 1 - day
		if night > 0 {
			vector.FillRect(dst, 0, 0, float32(W), float32(H), color.RGBA{4, 8, 26, uint8(130 * night)}, false)
		}
		if day > 0 && day < 1 {
			warm := 1 - math.Abs(2*day-1)
			vector.FillRect(dst, 0, 0, float32(W), float32(H), pre(color.RGBA{255, 110, 20, 255}, 0.13*warm), false)
		}
	}
	// darkness map: one pixel per tile, scaled up with linear filtering
	x0, y0 := v.tx0-1, v.ty0-1
	dw, dh := v.tx1-x0+2, v.ty1-y0+2
	if r.dark == nil || r.dark.Bounds().Dx() != dw || r.dark.Bounds().Dy() != dh {
		r.dark = ebiten.NewImage(dw, dh)
		r.darkPix = make([]byte, dw*dh*4)
	}
	if r.darkID != l.ID || len(r.darkCur) != l.W*l.H {
		r.darkID = l.ID
		r.darkCur = make([]float32, l.W*l.H)
		for i := range r.darkCur {
			r.darkCur[i] = -1
		}
	}
	ease := float32(1 - math.Exp(-r.dt*7)) // the edge of sight glides, tiles fade in
	for yy := 0; yy < dh; yy++ {
		for xx := 0; xx < dw; xx++ {
			x, y := x0+xx, y0+yy
			a := float32(r.darkness(v, sc, x, y))
			if l.In(x, y) {
				i := y*l.W + x
				if cur := r.darkCur[i]; cur >= 0 {
					a = cur + (a-cur)*ease
				}
				r.darkCur[i] = a
			}
			r.darkPix[(yy*dw+xx)*4+3] = uint8(255 * a)
		}
	}
	r.dark.WritePixels(r.darkPix)
	op := &ebiten.DrawImageOptions{}
	op.GeoM.Scale(ts, ts)
	ox, oy := v.px(float64(x0), float64(y0))
	op.GeoM.Translate(ox, oy)
	op.Filter = ebiten.FilterLinear
	dst.DrawImage(r.dark, op)

	// coloured light sources (additive)
	sx, sy := v.px(float64(sc.Self.X)+0.5, float64(sc.Self.Y)+0.5)
	if me := r.ents[sc.YouID]; me != nil {
		sx, sy = v.px(me.x+0.5, me.y+0.5)
	}
	flicker := 0.9 + 0.1*math.Sin(r.t*11)*math.Sin(r.t*7.3)
	if !sc.Self.Dead {
		if !l.Lit {
			r.glow(dst, sx, sy, float64(sc.Self.Vision)*ts*0.8, color.RGBA{255, 160, 80, 255}, 0.16*flicker)
		} else if day < 0.6 {
			r.glow(dst, sx, sy, 6*ts, color.RGBA{255, 170, 90, 255}, 0.16*(1-day)*flicker)
		}
	}
	lava := 0
	for y := v.ty0; y <= v.ty1; y++ {
		for x := v.tx0; x <= v.tx1; x++ {
			i := y*l.W + x
			if !v.visible(i) {
				continue
			}
			def := content.Tile(l.Tiles[i])
			switch {
			case def.Damage > 0 && lava < 220:
				lava++
				px, py := v.px(float64(x)+0.5, float64(y)+0.5)
				r.glow(dst, px, py, ts*1.5, hex(def.FG), 0.13*(0.85+0.15*math.Sin(r.t*2+float64(x))))
			case def.Interact == "stairs_down" || def.Interact == "stairs_up" || def.Interact == "dungeon":
				px, py := v.px(float64(x)+0.5, float64(y)+0.5)
				r.glow(dst, px, py, ts*1.4, hex(def.FG), 0.14+0.05*math.Sin(r.t*2))
			case def.Key == "altar":
				px, py := v.px(float64(x)+0.5, float64(y)+0.5)
				r.glow(dst, px, py, ts*2, hex(def.FG), 0.2)
			case def.Light != "":
				px, py := v.px(float64(x)+0.5, float64(y)+0.4)
				lc := hex(def.Light)
				switch def.Key {
				case "campfire", "brazier":
					fl := 0.85 + 0.15*math.Sin(r.t*9+float64(x))*math.Sin(r.t*6.3+float64(y))
					r.glow(dst, px, py, ts*3.4, lc, 0.32*fl)
					r.glow(dst, px, py-ts*0.1, ts*0.7, color.RGBA{255, 230, 140, 255}, 0.5*fl)
					if r.rng.Float64() < 0.3 {
						r.parts = append(r.parts, particle{x: float64(x) + 0.5 + (r.rng.Float64()-0.5)*0.3, y: float64(y) + 0.35,
							vx: (r.rng.Float64() - 0.5) * 0.3, vy: -1.2 - r.rng.Float64(), life: 0.5 + r.rng.Float64()*0.5,
							size: 0.07, grav: -0.4, col: color.RGBA{255, 150, 50, 255}, glow: true})
					}
				default:
					r.glow(dst, px, py, ts*1.8, lc, 0.18+0.05*math.Sin(r.t*2+float64(x*7+y)))
				}
			}
		}
	}
	for _, e := range sc.Entities {
		if e.Kind != kindProjectile || !l.In(e.X, e.Y) || !v.visible(e.Y*l.W+e.X) {
			continue
		}
		if st := r.ents[e.ID]; st != nil {
			x, y := v.px(st.x+0.5, st.y+0.5)
			r.glow(dst, x, y, ts*2.2, hex(e.Color), 0.45)
		}
	}
	for _, fl := range r.lights {
		x, y := v.px(fl.x, fl.y)
		r.glow(dst, x, y, fl.radius*ts, fl.col, 0.6*(1-fl.t/fl.life))
	}
	for _, p := range r.ambient {
		x, y := v.px(p.x, p.y)
		blink := 0.5 + 0.5*math.Sin(p.t*3+p.x)
		if p.glow {
			r.glow(dst, x, y, ts*0.22, p.col, 0.8*blink*math.Min(1, (p.life-p.t)))
		} else {
			r.glow(dst, x, y, ts*0.08, p.col, 0.25*blink)
		}
	}
}

func drawCentered(dst *ebiten.Image, s string, face *text.GoTextFace, x, y float64, c color.RGBA, outline bool) {
	s = i18n.T(s)
	w := text.Advance(s, face)
	m := face.Metrics()
	tx, ty := x-w/2, y-(m.HAscent+m.HDescent)/2
	if outline {
		o := math.Max(1, face.Size/14)
		for _, d := range [][2]float64{{-o, 0}, {o, 0}, {0, -o}, {0, o}} {
			op := &text.DrawOptions{}
			op.GeoM.Translate(tx+d[0], ty+d[1])
			op.ColorScale.ScaleWithColor(color.RGBA{0, 0, 0, c.A})
			op.ColorScale.ScaleAlpha(0.85)
			text.Draw(dst, s, face, op)
		}
	}
	op := &text.DrawOptions{}
	op.GeoM.Translate(tx, ty)
	op.ColorScale.ScaleWithColor(c)
	text.Draw(dst, s, face, op)
}

func roundRect(dst *ebiten.Image, x, y, w, h, rad float32, c color.RGBA) {
	var p vector.Path
	p.MoveTo(x+rad, y)
	p.LineTo(x+w-rad, y)
	p.QuadTo(x+w, y, x+w, y+rad)
	p.LineTo(x+w, y+h-rad)
	p.QuadTo(x+w, y+h, x+w-rad, y+h)
	p.LineTo(x+rad, y+h)
	p.QuadTo(x, y+h, x, y+h-rad)
	p.LineTo(x, y+rad)
	p.QuadTo(x, y, x+rad, y)
	p.Close()
	op := &vector.DrawPathOptions{AntiAlias: true}
	op.ColorScale.ScaleWithColor(c)
	vector.FillPath(dst, &p, nil, op)
}

func (r *worldRenderer) drawOverlays(dst *ebiten.Image, v *view, sc *client.Scene, area image.Rectangle) {
	ts := float64(v.ts)
	l := v.l
	label := r.face(r.fonts.bold, "label", ts*0.3)
	// health bars and names
	for _, e := range sc.Entities {
		if !l.In(e.X, e.Y) || !r.seen(e.ID) || e.Kind == kindItem || e.Kind == kindProjectile {
			continue
		}
		st := r.ents[e.ID]
		if st == nil {
			continue
		}
		cx, cy := v.px(st.x+0.5, st.y+0.5)
		top := cy - ts*0.62
		if t, ok := r.tops[e.ID]; ok {
			top = t - ts*0.06
		}
		if e.Dead {
			if e.Kind == kindPlayer {
				drawCentered(dst, "† "+e.Name, label, cx, cy-ts*0.2, pre(color.RGBA{200, 200, 210, 255}, 0.85), true)
			}
			continue
		}
		showBar := (e.Hostile && e.HP < 100) || (e.Kind == kindPlayer && e.ID != sc.YouID) ||
			(e.Ally && e.HP < 100) || (e.Kind == kindNPC && e.HP < 100)
		if showBar {
			bw, bh := float32(ts*0.8), float32(math.Max(3, ts*0.08))
			bx, by := float32(cx)-bw/2, float32(top)-bh
			vector.FillRect(dst, bx-1, by-1, bw+2, bh+2, color.RGBA{0, 0, 0, 180}, false)
			fc := color.RGBA{220, 60, 60, 255}
			if !e.Hostile {
				fc = color.RGBA{90, 210, 90, 255}
			}
			vector.FillRect(dst, bx, by, bw*float32(e.HP)/100, bh, fc, false)
			top -= float64(bh) + 2
		}
		near := math.Abs(float64(e.X-sc.Self.X))+math.Abs(float64(e.Y-sc.Self.Y)) <= 4
		unique := strings.HasPrefix(e.Def, "unique:")
		if unique {
			// unique characters carry a golden star: they have something special to offer
			pulse := 0.8 + 0.2*math.Sin(r.t*3)
			star(dst, cx, top-ts*(0.2+0.04*math.Sin(r.t*2.4)), ts*0.16, pre(color.RGBA{255, 210, 90, 255}, pulse))
			top -= ts * 0.36
		}
		if (e.Kind == kindPlayer && e.ID != sc.YouID) || (e.Kind == kindNPC && (near || unique)) || (e.Ally && near) {
			name := e.Name
			col := pre(white, 0.92)
			switch {
			case e.Ally:
				col = pre(color.RGBA{140, 240, 140, 255}, 0.95)
			case e.Kind == kindPlayer && e.Hostile:
				col = pre(color.RGBA{255, 140, 130, 255}, 0.95)
			case unique:
				col = pre(color.RGBA{255, 170, 255, 255}, 0.95)
			case e.Kind == kindNPC:
				col = pre(color.RGBA{255, 225, 150, 255}, 0.92)
			}
			drawCentered(dst, name, label, cx, top-ts*0.16, col, true)
		}
	}
	// "press E" hint on stairs
	switch sc.Self.StandingOn {
	case "dungeon", "stairs_down", "stairs_up":
		if me := r.ents[sc.YouID]; me != nil {
			cx, cy := v.px(me.x+0.5, me.y+0.5)
			pulse := float32(0.75 + 0.25*math.Sin(r.t*5))
			s := float32(ts * 0.42)
			bx, by := float32(cx)-s/2, float32(cy-ts*1.05)-s/2
			roundRect(dst, bx, by, s, s, s*0.25, color.RGBA{uint8(40 * pulse), uint8(34 * pulse), uint8(20 * pulse), uint8(230 * pulse)})
			drawCentered(dst, "E", r.face(r.fonts.bold, "key", ts*0.3), cx, float64(by+s/2), color.RGBA{255, 220, 120, 255}, false)
		}
	}
	// speech bubbles
	bub := r.face(r.fonts.regular, "speech", math.Max(11*r.scale, ts*0.32))
	for _, e := range sc.Entities {
		if e.Speech == "" || !l.In(e.X, e.Y) || !r.seen(e.ID) {
			continue
		}
		st := r.ents[e.ID]
		if st == nil {
			continue
		}
		lines := wrapWords(i18n.T(e.Speech), 30)
		if len(lines) > 4 {
			lines = lines[:4]
		}
		lh := bub.Size * 1.25
		w := 0.0
		for _, ln := range lines {
			w = math.Max(w, text.Advance(ln, bub))
		}
		pad := bub.Size * 0.5
		bw, bh := w+pad*2, float64(len(lines))*lh+pad*1.4
		cx, cy := v.px(st.x+0.5, st.y+0.5)
		bx := math.Max(4, math.Min(float64(dst.Bounds().Dx())-bw-4, cx-bw/2))
		head := cy - ts*0.75
		if t, ok := r.tops[e.ID]; ok {
			head = t
		}
		by := head - bh - ts*0.3
		bg := pre(color.RGBA{246, 240, 225, 255}, 0.94)
		fg := color.RGBA{40, 34, 30, 255}
		if e.Hostile {
			bg, fg = color.RGBA{70, 18, 18, 235}, color.RGBA{255, 210, 200, 255}
		}
		roundRect(dst, float32(bx), float32(by), float32(bw), float32(bh), float32(pad), bg)
		var tail vector.Path
		tail.MoveTo(float32(cx-pad*0.6), float32(by+bh-1))
		tail.LineTo(float32(cx+pad*0.6), float32(by+bh-1))
		tail.LineTo(float32(cx), float32(by+bh+pad*0.9))
		tail.Close()
		op := &vector.DrawPathOptions{AntiAlias: true}
		op.ColorScale.ScaleWithColor(bg)
		vector.FillPath(dst, &tail, nil, op)
		for i, ln := range lines {
			top := &text.DrawOptions{}
			top.GeoM.Translate(bx+pad, by+pad*0.7+float64(i)*lh)
			top.ColorScale.ScaleWithColor(fg)
			text.Draw(dst, ln, bub, top)
		}
	}
	// floating numbers
	for _, t := range r.texts {
		k := t.t / t.life
		size := ts * 0.42
		if t.big {
			size = ts * 0.7
		} else if strings.HasSuffix(t.text, "!") {
			size = ts * 0.55
		}
		pop := 1 + 0.35*math.Max(0, 1-t.t/0.12)
		f := r.face(r.fonts.bold, "float", math.Round(size*pop))
		x, y := v.px(t.x, t.y-k*1.1)
		a := 1.0
		if k > 0.6 {
			a = 1 - (k-0.6)/0.4
		}
		drawCentered(dst, t.text, f, x, y, pre(t.col, a), true)
	}
	// boss health bar
	for _, e := range sc.Entities {
		if !e.Boss || !l.In(e.X, e.Y) || !r.seen(e.ID) {
			continue
		}
		bw := math.Min(float64(area.Dx())*0.5, 520*r.scale)
		bh := 12 * r.scale
		bx := float64(area.Min.X+area.Max.X)/2 - bw/2
		by := float64(area.Min.Y) + 34*r.scale
		drawCentered(dst, e.Name, r.face(r.fonts.bold, "boss", 15*r.scale), bx+bw/2, by-12*r.scale, color.RGBA{255, 140, 255, 255}, true)
		roundRect(dst, float32(bx-2), float32(by-2), float32(bw+4), float32(bh+4), float32(bh/2+2), color.RGBA{0, 0, 0, 200})
		if e.HP > 0 {
			roundRect(dst, float32(bx), float32(by), float32(bw*float64(e.HP)/100), float32(bh), float32(bh/2), color.RGBA{200, 40, 140, 255})
		}
		break
	}
	// vignette and states
	W, H := float64(dst.Bounds().Dx()), float64(dst.Bounds().Dy())
	op := &ebiten.DrawImageOptions{}
	op.GeoM.Scale(W/256, H/256)
	op.ColorScale.ScaleAlpha(0.7)
	op.Filter = ebiten.FilterLinear
	dst.DrawImage(r.vig, op)
	if sc.Self.Dead {
		vector.FillRect(dst, 0, 0, float32(W), float32(H), color.RGBA{60, 0, 0, 90}, false)
	}
	if sc.Paused {
		vector.FillRect(dst, 0, 0, float32(W), float32(H), color.RGBA{0, 0, 0, 90}, false)
	}
}

func wrapWords(s string, w int) []string {
	var out []string
	line := ""
	for _, word := range strings.Fields(s) {
		switch {
		case line == "":
			line = word
		case len([]rune(line))+1+len([]rune(word)) <= w:
			line += " " + word
		default:
			out = append(out, line)
			line = word
		}
	}
	if line != "" {
		out = append(out, line)
	}
	return out
}

// ---- menu backdrop: a slowly drifting generated landscape ----

type menuBackdrop struct {
	r     *worldRenderer
	level *world.Level
	t     float64
	ready chan *world.Level
}

func newMenuBackdrop(r *worldRenderer) *menuBackdrop {
	m := &menuBackdrop{r: r, ready: make(chan *world.Level, 1)}
	go func() { m.ready <- genBackdrop() }()
	return m
}

func (m *menuBackdrop) update(dt float64) {
	m.t += dt
	m.r.t += dt
	if m.level == nil {
		select {
		case l := <-m.ready:
			m.level = l
		default:
		}
	}
}

func (m *menuBackdrop) draw(dst *ebiten.Image, scale float64) {
	if m.level == nil {
		return
	}
	l := m.level
	W, H := dst.Bounds().Dx(), dst.Bounds().Dy()
	camX := float64(l.W)/2 + math.Sin(m.t*0.021)*float64(l.W)*0.3
	camY := float64(l.H)/2 + math.Sin(m.t*0.033+1)*float64(l.H)*0.25
	area := image.Rect(0, 0, W, H)
	v := m.r.makeView(dst, l, camX, camY, image.Pt(W/2, H/2), area)
	v.tod = math.Mod(0.32+m.t*0.006, 1)
	m.r.drawTiles(dst, v, nil)
	day := daylight(v.tod)
	vector.FillRect(dst, 0, 0, float32(W), float32(H), color.RGBA{4, 8, 26, uint8(150 * (1 - day))}, false)
	vector.FillRect(dst, 0, 0, float32(W), float32(H), color.RGBA{0, 0, 0, 80}, false)
	op := &ebiten.DrawImageOptions{}
	op.GeoM.Scale(float64(W)/256, float64(H)/256)
	op.Filter = ebiten.FilterLinear
	dst.DrawImage(m.r.vig, op)
}

func genBackdrop() *world.Level {
	return gen.GenerateOverworld(2024, 180, 90).Level
}

// drawBody draws a fallen creature lying on its side, greyed out.
func (r *worldRenderer) drawBody(dst, img *ebiten.Image, cx, feet, k, ts float64) {
	iw, ih := float64(img.Bounds().Dx()), float64(img.Bounds().Dy())
	r.shadow(dst, cx, feet-ts*0.1, ih*k*0.8, ts*0.2, 0.4)
	op := &ebiten.DrawImageOptions{}
	op.GeoM.Translate(-iw/2, -ih/2)
	op.GeoM.Scale(k, k)
	op.GeoM.Rotate(-math.Pi / 2)
	op.GeoM.Translate(math.Round(cx), math.Round(feet-iw*k/2))
	op.ColorScale.Scale(0.62, 0.56, 0.56, 1)
	dst.DrawImage(img, op)
}

// star draws a four-pointed sparkle with a dark outline.
func star(dst *ebiten.Image, cx, cy, rad float64, c color.RGBA) {
	var p vector.Path
	for i := 0; i < 8; i++ {
		a := float64(i)*math.Pi/4 - math.Pi/2
		d := rad
		if i%2 == 1 {
			d = rad * 0.36
		}
		x, y := float32(cx+math.Cos(a)*d), float32(cy+math.Sin(a)*d)
		if i == 0 {
			p.MoveTo(x, y)
		} else {
			p.LineTo(x, y)
		}
	}
	p.Close()
	op := &vector.DrawPathOptions{AntiAlias: true}
	op.ColorScale.ScaleWithColor(color.RGBA{20, 12, 4, 200})
	vector.StrokePath(dst, &p, &vector.StrokeOptions{Width: float32(math.Max(1.5, rad*0.3)), LineJoin: vector.LineJoinRound}, op)
	op.ColorScale.Reset()
	op.ColorScale.ScaleWithColor(c)
	vector.FillPath(dst, &p, nil, op)
}

// seen: the creature is (mostly) in sight, so its bars and words are shown.
func (r *worldRenderer) seen(id uint32) bool {
	st := r.ents[id]
	return st != nil && st.alpha > 0.5
}
