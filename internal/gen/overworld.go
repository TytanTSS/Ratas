package gen

import (
	"math"
	"math/rand/v2"
	"sort"

	"ratas/internal/content"
	"ratas/internal/world"
)

type Rect struct{ X, Y, W, H int }

func (r Rect) Center() world.Pos { return world.Pos{X: r.X + r.W/2, Y: r.Y + r.H/2} }
func (r Rect) Contains(x, y int) bool {
	return x >= r.X && y >= r.Y && x < r.X+r.W && y < r.Y+r.H
}
func (r Rect) Overlaps(o Rect, gap int) bool {
	return r.X-gap < o.X+o.W && o.X-gap < r.X+r.W && r.Y-gap < o.Y+o.H && o.Y-gap < r.Y+r.H
}

type NPCSpawn struct {
	Role string
	Name string
	Pos  world.Pos
}

type Village struct {
	Name   string
	Center world.Pos
	Area   Rect
	Houses []Rect
	NPCs   []NPCSpawn
}

type Entrance struct {
	Pos      world.Pos
	Theme    string
	Name     string
	MaxDepth int
}

type Overworld struct {
	Name      string
	Level     *world.Level
	Villages  []Village
	Entrances []Entrance
	Start     world.Pos
	Regions   []Region
	RegionMap []uint8 // 1-based index into Regions per cell, 0 = none
	Landmarks []Landmark
}

// GenerateOverworld builds the surface map.
//
// Algorithm:
//  1. Elevation = fractal Perlin noise minus a radial falloff (island shape);
//     moisture and temperature (north is cold, south is hot, highlands are
//     cooler) are independent noise fields. All are classified by quantiles,
//     so the share of water, deserts or tundra is stable for any seed.
//  2. Biomes come from (elevation, moisture, temperature): sea, beaches,
//     plains, forests, swamps, hills and mountains, tundra and deserts; local
//     detail noise adds tree density, dunes, snowdrifts and reeds.
//  3. Rivers flow downhill from highlands to the sea (frozen in the north).
//  4. Villages are placed on the largest walkable landmass, far apart.
//  5. Far from villages the land is scarred: volcanic ash wastes around a
//     volcano and cursed lands with dead forests and graveyards.
//  6. Villages are joined by roads found with weighted A* (bridges over water).
//  7. Connected areas of one biome become named regions; landmarks (shrines,
//     stone circles, ruins, graveyards, bandit camps, oases) are stamped on
//     free land; dungeon entrances are scattered, regional dungeons inside
//     their regions, the deeper ones further from the start.
func GenerateOverworld(seed int64, w, h int) *Overworld {
	r := RNG(seed, "overworld")
	ow := &Overworld{Name: WorldName(r)}
	l := world.NewLevel("overworld", ow.Name, w, h, content.TileID("grass"))
	l.Lit = true
	l.Theme = "overworld"
	ow.Level = l

	elevN, moistN, detailN, tempN := NewPerlin(r), NewPerlin(r), NewPerlin(r), NewPerlin(r)
	n := w * h
	E := make([]float64, n)
	M := make([]float64, n)
	D := make([]float64, n)
	Tm := make([]float64, n)
	for y := 0; y < h; y++ {
		for x := 0; x < w; x++ {
			i := y*w + x
			// terminal cells are ~2x taller than wide: sample y twice as fast
			fx, fy := float64(x)/48.0, float64(y)*2/48.0
			e := (elevN.FBM(fx, fy, 6) + 1) / 2
			nx, ny := float64(x)/float64(w)*2-1, float64(y)/float64(h)*2-1
			d := 1 - (1-nx*nx)*(1-ny*ny)
			E[i] = e - 0.55*d*d
			M[i] = (moistN.FBM(fx*0.8+100, fy*0.8+100, 4) + 1) / 2
			D[i] = detailN.FBM(float64(x)/6, float64(y)*2/6, 2)
			Tm[i] = float64(y)/float64(h)*1.1 + tempN.FBM(fx*0.6+50, fy*0.6+50, 3)*0.55
		}
	}
	eq := quantiles(E)
	mq := quantiles(M)
	tq := quantiles(Tm)
	for i := range Tm {
		Tm[i] -= math.Max(0, E[i]-eq(0.75)) * 1.5
	}

	T := content.TileID
	cold := make([]bool, n)
	for y := 0; y < h; y++ {
		for x := 0; x < w; x++ {
			i := y*w + x
			e, m, det := E[i], M[i], D[i]
			isCold := Tm[i] < tq(0.15)
			isHot := Tm[i] > tq(0.8) && m < mq(0.62)
			cold[i] = isCold
			var t uint8
			switch {
			case e < eq(0.20):
				t = T("deep_water")
			case e < eq(0.28):
				t = T("water")
				if isCold {
					t = T("ice")
				}
			case e < eq(0.31):
				switch {
				case isCold:
					t = T("snow_ground")
				case isHot:
					t = T("desert_sand")
				default:
					t = T("sand")
				}
			case e > eq(0.985):
				t = T("snow")
			case e > eq(0.945) || (e > eq(0.87) && det > 0.25):
				switch {
				case isCold:
					t = T("ice_rock")
					if r.IntN(3) == 0 {
						t = T("snow")
					}
				case isHot:
					t = T("sandstone")
				default:
					t = T("mountain")
				}
			case e > eq(0.87):
				switch {
				case isCold:
					t = T("snowdrift")
				case isHot:
					t = T("dunes")
				default:
					t = T("hill")
				}
			case isCold:
				switch v := r.Float64(); {
				case v < 0.3+det*0.4 && m > mq(0.4):
					t = T("snow_pine")
				case v < 0.012:
					t = T("ice_rock")
				case det > 0.2:
					t = T("snowdrift")
				default:
					t = T("snow_ground")
				}
			case isHot:
				switch v := r.Float64(); {
				case v < 0.018:
					t = T("cactus")
				case v < 0.026:
					t = T("sandstone")
				case v < 0.03:
					t = T("dead_tree")
				case det > 0.12:
					t = T("dunes")
				default:
					t = T("desert_sand")
				}
			case m > mq(0.86) && e < eq(0.6):
				switch v := r.Float64(); {
				case v < 0.08:
					t = T("water")
				case v < 0.12:
					t = T("dead_tree")
				case v < 0.32 || det > 0.2:
					t = T("reeds")
				default:
					t = T("swamp")
				}
			case m > mq(0.52):
				density := 0.38 + det*0.5
				switch v := r.Float64(); {
				case v < density:
					if e > eq(0.7) || r.IntN(3) == 0 {
						t = T("pine")
					} else {
						t = T("tree")
					}
				case v < density+0.07:
					t = T("bush")
				default:
					t = T("forest_floor")
				}
			default:
				switch v := r.Float64(); {
				case v < 0.012:
					t = T("flowers")
				case v < 0.03:
					t = T("tree")
				case det > 0.18:
					t = T("tall_grass")
				case v < 0.5:
					t = T("grass2")
				default:
					t = T("grass")
				}
			}
			l.Tiles[i] = t
		}
	}

	carveRivers(r, l, E, eq)
	// rivers freeze in the north
	water, ice := T("water"), T("ice")
	for i, t := range l.Tiles {
		if t == water && cold[i] {
			l.Tiles[i] = ice
		}
	}

	main := largestRegion(l)
	ow.Villages = placeVillages(r, l, main)
	var start world.Pos
	if len(ow.Villages) > 0 {
		start = ow.Villages[0].Center
	}
	scarLand(r, l, main, ow.Villages, start)
	buildRoads(l, ow.Villages)
	main = largestRegion(l)
	ow.Regions, ow.RegionMap = nameRegions(r, l)
	ow.Landmarks = placeLandmarks(r, l, main, ow.Villages, start)
	main = largestRegion(l)
	ow.Entrances = placeEntrances(r, l, main, ow.Villages)

	if len(ow.Villages) > 0 {
		c := ow.Villages[0].Center
		ow.Start = findFree(l, world.Pos{X: c.X, Y: c.Y + 2})
	} else {
		for i, ok := range main {
			if ok {
				ow.Start = world.Pos{X: i % w, Y: i / w}
				break
			}
		}
	}
	return ow
}

// quantiles returns a function mapping q in [0,1] to the value at that quantile.
func quantiles(v []float64) func(float64) float64 {
	s := append([]float64(nil), v...)
	sort.Float64s(s)
	return func(q float64) float64 {
		i := int(q * float64(len(s)-1))
		return s[max(0, min(len(s)-1, i))]
	}
}

func carveRivers(r *rand.Rand, l *world.Level, E []float64, eq func(float64) float64) {
	water, deep := content.TileID("water"), content.TileID("deep_water")
	lo, hi := eq(0.75), eq(0.93)
	rivers := 0
	for attempt := 0; attempt < 400 && rivers < 7; attempt++ {
		x, y := r.IntN(l.W), r.IntN(l.H)
		e := E[y*l.W+x]
		if e < lo || e > hi {
			continue
		}
		visited := map[int]bool{}
		var path []world.Pos
		reached := false
		for steps := 0; steps < 900; steps++ {
			i := y*l.W + x
			visited[i] = true
			path = append(path, world.Pos{X: x, Y: y})
			t := l.Tiles[i]
			if steps > 0 && (t == water || t == deep) {
				reached = true
				break
			}
			best, bx, by := math.Inf(1), -1, -1
			for _, d := range world.AllDirs {
				dd := d.Delta()
				nx, ny := x+dd.X, y+dd.Y
				if !l.In(nx, ny) || visited[ny*l.W+nx] {
					continue
				}
				v := E[ny*l.W+nx] + r.Float64()*0.004
				if v < best {
					best, bx, by = v, nx, ny
				}
			}
			if bx < 0 {
				break
			}
			x, y = bx, by
		}
		if !reached || len(path) < 25 {
			continue
		}
		for k, p := range path {
			l.Set(p.X, p.Y, water)
			// widen the lower course of the river
			if k > len(path)/2 && k%2 == 0 {
				l.Set(p.X+1, p.Y, water)
			}
		}
		rivers++
	}
}

// largestRegion flood-fills walkable cells and returns a mask of the largest component.
func largestRegion(l *world.Level) []bool {
	n := l.W * l.H
	region := make([]int, n)
	for i := range region {
		region[i] = -1
	}
	bestID, bestSize := -1, 0
	id := 0
	stack := make([]int, 0, 1024)
	for i := 0; i < n; i++ {
		if region[i] >= 0 || !content.Tile(l.Tiles[i]).Walkable {
			continue
		}
		size := 0
		stack = append(stack[:0], i)
		region[i] = id
		for len(stack) > 0 {
			c := stack[len(stack)-1]
			stack = stack[:len(stack)-1]
			size++
			cx, cy := c%l.W, c/l.W
			for _, d := range world.AllDirs {
				dd := d.Delta()
				nx, ny := cx+dd.X, cy+dd.Y
				if !l.In(nx, ny) {
					continue
				}
				ni := ny*l.W + nx
				if region[ni] < 0 && content.Tile(l.Tiles[ni]).Walkable {
					region[ni] = id
					stack = append(stack, ni)
				}
			}
		}
		if size > bestSize {
			bestID, bestSize = id, size
		}
		id++
	}
	mask := make([]bool, n)
	for i := range mask {
		mask[i] = region[i] == bestID
	}
	return mask
}

func findFree(l *world.Level, p world.Pos) world.Pos {
	for rad := 0; rad < 30; rad++ {
		for dy := -rad; dy <= rad; dy++ {
			for dx := -rad; dx <= rad; dx++ {
				if max(abs(dx), abs(dy)) != rad {
					continue
				}
				if l.Walkable(p.X+dx, p.Y+dy) {
					return world.Pos{X: p.X + dx, Y: p.Y + dy}
				}
			}
		}
	}
	return p
}

func abs(v int) int {
	if v < 0 {
		return -v
	}
	return v
}

func isGround(t uint8) bool {
	def := content.Tile(t)
	switch def.Biome {
	case "plains", "forest", "sand", "hills", "swamp", "desert", "tundra", "ash", "cursed":
		return def.Walkable && def.Damage == 0
	}
	return false
}

func placeVillages(r *rand.Rand, l *world.Level, main []bool) []Village {
	var vs []Village
	used := map[string]bool{}
	const vw, vh = 28, 14
	want := 4 + r.IntN(2)
	for attempt := 0; attempt < 4000 && len(vs) < want; attempt++ {
		cx := 20 + r.IntN(l.W-40)
		cy := 12 + r.IntN(l.H-24)
		if !main[cy*l.W+cx] {
			continue
		}
		area := Rect{cx - vw/2, cy - vh/2, vw, vh}
		// mostly dry flat land
		good, total := 0, 0
		for y := area.Y; y < area.Y+area.H; y++ {
			for x := area.X; x < area.X+area.W; x++ {
				total++
				t := l.At(x, y)
				def := content.Tile(t)
				if def.Biome == "plains" || def.Biome == "forest" || def.Biome == "sand" {
					good++
				}
			}
		}
		if good*100/total < 85 {
			continue
		}
		far := true
		for _, v := range vs {
			if abs(v.Center.X-cx) < 55 && abs(v.Center.Y-cy) < 28 {
				far = false
				break
			}
		}
		if !far {
			continue
		}
		vs = append(vs, buildVillage(r, l, area, pickUnique(r, villageNames, used)))
	}
	return vs
}

func buildVillage(r *rand.Rand, l *world.Level, area Rect, name string) Village {
	T := content.TileID
	v := Village{Name: name, Center: area.Center(), Area: area}
	// clear an ellipse of grass
	for y := area.Y; y < area.Y+area.H; y++ {
		for x := area.X; x < area.X+area.W; x++ {
			dx := float64(x-v.Center.X) / float64(area.W/2)
			dy := float64(y-v.Center.Y) / float64(area.H/2)
			if dx*dx+dy*dy <= 1.05 {
				if r.IntN(2) == 0 {
					l.Set(x, y, T("grass"))
				} else {
					l.Set(x, y, T("grass2"))
				}
			}
		}
	}
	c := v.Center
	l.Set(c.X, c.Y, T("well"))
	plaza := Rect{c.X - 4, c.Y - 2, 9, 5}
	nh := 4 + r.IntN(3)
	for attempt := 0; attempt < 300 && len(v.Houses) < nh; attempt++ {
		hw, hh := Range(r, 6, 9), Range(r, 4, 5)
		hx := Range(r, area.X+1, area.X+area.W-hw-1)
		hy := Range(r, area.Y+1, area.Y+area.H-hh-1)
		hr := Rect{hx, hy, hw, hh}
		if hr.Overlaps(plaza, 1) {
			continue
		}
		ok := true
		for _, o := range v.Houses {
			if hr.Overlaps(o, 2) {
				ok = false
				break
			}
		}
		if !ok {
			continue
		}
		for y := hy; y < hy+hh; y++ {
			for x := hx; x < hx+hw; x++ {
				if x == hx || y == hy || x == hx+hw-1 || y == hy+hh-1 {
					l.Set(x, y, T("house_wall"))
				} else {
					l.Set(x, y, T("house_floor"))
				}
			}
		}
		// door on the wall facing the well
		hc := hr.Center()
		var door world.Pos
		switch world.DirTowards(hc, c) {
		case world.DirUp:
			door = world.Pos{X: hc.X, Y: hy}
		case world.DirDown:
			door = world.Pos{X: hc.X, Y: hy + hh - 1}
		case world.DirLeft:
			door = world.Pos{X: hx, Y: hc.Y}
		default:
			door = world.Pos{X: hx + hw - 1, Y: hc.Y}
		}
		l.Set(door.X, door.Y, T("door"))
		v.Houses = append(v.Houses, hr)
	}

	// population
	free := func() world.Pos {
		for i := 0; i < 200; i++ {
			x := Range(r, area.X+2, area.X+area.W-3)
			y := Range(r, area.Y+1, area.Y+area.H-2)
			if l.Walkable(x, y) && content.Tile(l.At(x, y)).Key != "house_floor" {
				return world.Pos{X: x, Y: y}
			}
		}
		return world.Pos{X: c.X + 1, Y: c.Y + 1}
	}
	houseSpot := 0
	inHouse := func() world.Pos {
		if houseSpot < len(v.Houses) {
			h := v.Houses[houseSpot]
			houseSpot++
			return h.Center()
		}
		return free()
	}
	for _, role := range content.NPCRoles() {
		if role.World {
			continue // wanderers live in the open world
		}
		cnt := Range(r, role.Count[0], role.Count[1])
		for k := 0; k < cnt; k++ {
			var p world.Pos
			switch role.Key {
			case "elder", "merchant", "smith", "healer", "priest":
				p = inHouse()
			default:
				p = free()
			}
			v.NPCs = append(v.NPCs, NPCSpawn{Role: role.Key, Name: PersonName(r), Pos: p})
		}
	}
	return v
}

func buildRoads(l *world.Level, vs []Village) {
	if len(vs) < 2 {
		return
	}
	T := content.TileID
	road, bridge := T("road"), T("bridge")
	water, deep := T("water"), T("deep_water")
	cost := func(x, y int) float64 {
		t := l.At(x, y)
		def := content.Tile(t)
		switch {
		case t == road || t == bridge:
			return 0.3
		case t == deep:
			return 25
		case t == water:
			return 6
		case def.Key == "snow":
			return -1
		case def.Key == "mountain":
			return 40
		case def.Key == "well" || def.Key == "house_wall" || def.Key == "door" || def.Key == "house_floor":
			return -1
		case !def.Walkable:
			return 4
		}
		return def.MoveCost
	}
	connected := []int{0}
	for i := 1; i < len(vs); i++ {
		best, bd := 0, math.MaxInt
		for _, j := range connected {
			d := vs[i].Center.Manhattan(vs[j].Center)
			if d < bd {
				best, bd = j, d
			}
		}
		from := world.Pos{X: vs[i].Center.X, Y: vs[i].Center.Y + 1}
		to := world.Pos{X: vs[best].Center.X, Y: vs[best].Center.Y + 1}
		path := world.FindPath(l.W, l.H, from, to, l.W*l.H, cost)
		for _, p := range path {
			t := l.At(p.X, p.Y)
			key := content.Tile(t).Key
			switch {
			case t == water || t == deep:
				l.Set(p.X, p.Y, bridge)
			case key == "well" || key == "house_wall" || key == "door" || key == "house_floor":
			default:
				l.Set(p.X, p.Y, road)
			}
		}
		connected = append(connected, i)
	}
}

func placeEntrances(r *rand.Rand, l *world.Level, main []bool, vs []Village) []Entrance {
	var start world.Pos
	if len(vs) > 0 {
		start = vs[0].Center
	}
	var es []Entrance
	used := map[string]bool{}
	themes := []string{"cave", "crypt", "cave", "crypt", "crypt", "cave"}
	for attempt := 0; attempt < 6000 && len(es) < len(themes); attempt++ {
		x, y := 4+r.IntN(l.W-8), 3+r.IntN(l.H-6)
		if !main[y*l.W+x] || !isGround(l.At(x, y)) {
			continue
		}
		p := world.Pos{X: x, Y: y}
		if abs(p.X-start.X) < 25 && abs(p.Y-start.Y) < 12 {
			continue
		}
		ok := true
		for _, v := range vs {
			if v.Area.Overlaps(Rect{x - 3, y - 2, 7, 5}, 2) {
				ok = false
			}
		}
		for _, e := range es {
			if abs(e.Pos.X-x) < 30 && abs(e.Pos.Y-y) < 15 {
				ok = false
			}
		}
		if !ok {
			continue
		}
		theme := themes[len(es)]
		// caves prefer hills, crypts prefer anything but hills
		biome := content.Tile(l.At(x, y)).Biome
		if theme == "cave" && biome != "hills" && attempt < 3000 {
			continue
		}
		es = append(es, Entrance{Pos: p, Theme: theme, Name: pickUnique(r, dungeonNames[theme], used)})
	}
	// deeper dungeons further from the start
	sort.Slice(es, func(i, j int) bool { return es[i].Pos.DistSq(start) < es[j].Pos.DistSq(start) })
	for i := range es {
		es[i].MaxDepth = 2 + i/2 + min(i, 1)
	}
	// every special region hides its own dungeon deep inside
	for _, rg := range regionalDungeons {
		for attempt := 0; attempt < 5000; attempt++ {
			x, y := 4+r.IntN(l.W-8), 3+r.IntN(l.H-6)
			t := content.Tile(l.At(x, y))
			if !main[y*l.W+x] || !isGround(l.At(x, y)) || t.Biome != rg.biome {
				continue
			}
			inside := 0
			for dy := -4; dy <= 4; dy++ {
				for dx := -8; dx <= 8; dx++ {
					if content.Tile(l.At(x+dx, y+dy)).Biome == rg.biome {
						inside++
					}
				}
			}
			if inside < 17*9*3/4 && attempt < 4000 {
				continue
			}
			ok := true
			for _, e := range es {
				if abs(e.Pos.X-x) < 16 && abs(e.Pos.Y-y) < 8 {
					ok = false
				}
			}
			if ok {
				es = append(es, Entrance{Pos: world.Pos{X: x, Y: y}, Theme: rg.theme, Name: pickUnique(r, dungeonNames[rg.theme], used), MaxDepth: 4})
				break
			}
		}
	}
	T := content.TileID
	for i := range es {
		p := es[i].Pos
		for dy := -1; dy <= 1; dy++ {
			for dx := -2; dx <= 2; dx++ {
				if l.In(p.X+dx, p.Y+dy) && !l.Walkable(p.X+dx, p.Y+dy) {
					l.Set(p.X+dx, p.Y+dy, T("rubble"))
				}
			}
		}
		l.Set(p.X, p.Y, T("dungeon"))
	}
	return es
}
