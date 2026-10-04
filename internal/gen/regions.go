package gen

import (
	"math"
	"math/rand/v2"
	"slices"

	"ratas/internal/content"
	"ratas/internal/world"
)

// Region is a named connected area of one biome on the overworld.
type Region struct {
	Name   string `json:"name"`
	Kind   string `json:"kind"`   // plains, forest, swamp, hills, desert, tundra, ash, cursed
	Danger int    `json:"danger"` // 0..3, raises monster levels
}

// Landmark is a notable place on the overworld.
type Landmark struct {
	Name string    `json:"name"`
	Kind string    `json:"kind"` // shrine, circle, ruins, graveyard, camp, oasis
	Pos  world.Pos `json:"pos"`
}

// RegionDanger is how dangerous each kind of land is.
var RegionDanger = map[string]int{"swamp": 1, "hills": 1, "desert": 1, "tundra": 2, "cursed": 2, "ash": 3}

var regionalDungeons = []struct{ theme, biome string }{
	{"temple", "desert"}, {"ice", "tundra"}, {"volcano", "ash"}, {"fortress", "cursed"},
}

// ---- volcanic and cursed lands ----

func nearVillage(vs []Village, x, y, margin int) bool {
	for _, v := range vs {
		a := v.Area
		if x >= a.X-margin && y >= a.Y-margin/2 && x < a.X+a.W+margin && y < a.Y+a.H+margin/2 {
			return true
		}
	}
	return false
}

// scarLand turns two areas far from people into a volcanic waste and a
// cursed land.
func scarLand(r *rand.Rand, l *world.Level, main []bool, vs []Village, start world.Pos) {
	rx := max(10, l.W/12)
	ry := max(5, rx/2)
	var centers []world.Pos
	for _, kind := range []string{"ash", "cursed"} {
		c, ok := scarCenter(r, l, main, vs, start, centers, rx, ry)
		if !ok {
			continue
		}
		centers = append(centers, c)
		stampScar(r, l, c, rx, ry, vs, kind)
	}
}

func scarCenter(r *rand.Rand, l *world.Level, main []bool, vs []Village, start world.Pos, others []world.Pos, rx, ry int) (world.Pos, bool) {
	minStart := (l.W + l.H) / 5
	for attempt := 0; attempt < 3000; attempt++ {
		x := rx + r.IntN(max(1, l.W-2*rx))
		y := ry + r.IntN(max(1, l.H-2*ry))
		if !main[y*l.W+x] || abs(x-start.X)+abs(y-start.Y)*2 < minStart {
			continue
		}
		ok := true
		for _, v := range vs {
			dx, dy := v.Center.X-x, (v.Center.Y-y)*2
			if dx*dx+dy*dy < (rx+18)*(rx+18) {
				ok = false
			}
		}
		for _, o := range others {
			dx, dy := o.X-x, (o.Y-y)*2
			if dx*dx+dy*dy < (3*rx)*(3*rx) {
				ok = false
			}
		}
		if !ok {
			continue
		}
		land, total := 0, 0
		for dy := -ry; dy <= ry; dy++ {
			for dx := -rx; dx <= rx; dx++ {
				total++
				if isGround(l.At(x+dx, y+dy)) {
					land++
				}
			}
		}
		if land*100/total >= 55 {
			return world.Pos{X: x, Y: y}, true
		}
	}
	return world.Pos{}, false
}

var settlementTiles = map[string]bool{
	"house_wall": true, "house_floor": true, "door": true, "door_open": true, "well": true,
	"fence": true, "road": true, "bridge": true, "dungeon": true,
}

func stampScar(r *rand.Rand, l *world.Level, c world.Pos, rx, ry int, vs []Village, kind string) {
	T := content.TileID
	edgeN, detN := NewPerlin(r), NewPerlin(r)
	for y := c.Y - ry*3/2; y <= c.Y+ry*3/2; y++ {
		for x := c.X - rx*3/2; x <= c.X+rx*3/2; x++ {
			if !l.In(x, y) || nearVillage(vs, x, y, 3) {
				continue
			}
			def := l.Def(x, y)
			if settlementTiles[def.Key] || def.Key == "deep_water" {
				continue
			}
			dx, dy := float64(x-c.X)/float64(rx), float64(y-c.Y)/float64(ry)
			edge := 1 + edgeN.FBM(float64(x)/9, float64(y)*2/9, 2)*0.45
			k := math.Hypot(dx, dy) / edge
			if k > 1 {
				continue
			}
			// ragged border: the old land shows through
			if k > 0.82 && r.Float64() < (k-0.82)/0.18 {
				continue
			}
			det := detN.FBM(float64(x)/5, float64(y)*2/5, 2)
			v := r.Float64()
			var t string
			if kind == "ash" {
				switch {
				case def.Key == "water" || def.Key == "ice":
					if k < 0.6 {
						t = "lava"
					}
				case k < 0.09:
					t = "lava"
				case k < 0.2:
					t = "basalt" // the volcano cone
				case !def.Walkable && def.Biome != "plains" && def.Biome != "forest" && def.Biome != "tundra" && def.Biome != "desert":
					t = "basalt"
				case det > 0.4:
					t = "lava"
				case det > 0.3:
					t = "basalt"
				case v < 0.03:
					t = "charred_tree"
				case v < 0.07:
					t = "magma_crack"
				case def.Key == "tree" || def.Key == "pine" || def.Key == "snow_pine" || def.Key == "palm":
					t = "charred_tree"
				default:
					t = "ash"
				}
			} else {
				switch def.Key {
				case "tree", "pine", "snow_pine", "palm", "bush":
					t = "twisted_tree"
					if v < 0.3 {
						t = "dead_tree"
					}
				case "swamp", "reeds", "water", "ice", "mountain", "snow", "ice_rock", "sandstone", "cactus", "dead_tree":
				default:
					switch {
					case v < 0.025:
						t = "mushrooms"
					case v < 0.04:
						t = "dead_tree"
					case v < 0.045:
						t = "gravestone"
					case det > 0.35 && v < 0.5:
						t = "twisted_tree"
					default:
						t = "blight_grass"
					}
				}
			}
			if t != "" {
				l.Set(x, y, T(t))
			}
		}
	}
}

// ---- named regions ----

var regionNames = map[string][]string{
	"plains": {"Солнечные Луга", "Вольные Поля", "Ковыльная Степь", "Медовые Луга", "Долина Ветров", "Зелёный Дол", "Пастушьи Холмы", "Широкое Поле"},
	"forest": {"Шепчущий Лес", "Чернолесье", "Дубрава Старых Богов", "Зелёная Пуща", "Совиный Бор", "Еловый Край", "Туманный Лес", "Волчья Чаща", "Медвежий Бор", "Ясеневая Роща"},
	"swamp":  {"Гнилые Топи", "Туманные Болота", "Ведьмина Трясина", "Камышовые Плавни", "Чёрная Гать", "Лягушачьи Мхи"},
	"hills":  {"Каменные Холмы", "Гремящий Кряж", "Седые Предгорья", "Орлиные Утёсы", "Хребет Великана", "Ветреные Склоны"},
	"desert": {"Пески Забвения", "Пустыня Аш-Шарр", "Золотые Барханы", "Море Песка", "Выжженная Равнина"},
	"tundra": {"Ледяной Предел", "Белая Пустошь", "Земли Вечной Зимы", "Северная Тундра", "Стылые Равнины"},
	"ash":    {"Пепельные Пустоши", "Огненный Разлом", "Земли Пламени", "Пепелище"},
	"cursed": {"Проклятые Земли", "Сумрачный Край", "Долина Мёртвых", "Край Скорби"},
}

func regionKind(t uint8) string {
	switch b := content.Tile(t).Biome; b {
	case "plains", "sand":
		return "plains"
	case "hills", "snow":
		return "hills"
	case "forest", "swamp", "desert", "tundra", "ash", "cursed":
		return b
	}
	return ""
}

// nameRegions labels large connected areas of one kind of land and spreads
// the labels over roads, rivers and small patches.
func nameRegions(r *rand.Rand, l *world.Level) ([]Region, []uint8) {
	n := l.W * l.H
	labels := make([]int, n) // 0 = unvisited, -1 = too small, >0 region
	minSize := max(120, n/300)
	var regions []Region
	used := map[string]bool{}
	stack := make([]int, 0, 1024)
	var cells []int
	for i := 0; i < n; i++ {
		kind := regionKind(l.Tiles[i])
		if labels[i] != 0 || kind == "" {
			continue
		}
		cells = cells[:0]
		stack = append(stack[:0], i)
		labels[i] = -1
		for len(stack) > 0 {
			c := stack[len(stack)-1]
			stack = stack[:len(stack)-1]
			cells = append(cells, c)
			cx, cy := c%l.W, c/l.W
			for _, d := range world.AllDirs {
				dd := d.Delta()
				nx, ny := cx+dd.X, cy+dd.Y
				if !l.In(nx, ny) {
					continue
				}
				ni := ny*l.W + nx
				if labels[ni] == 0 && regionKind(l.Tiles[ni]) == kind {
					labels[ni] = -1
					stack = append(stack, ni)
				}
			}
		}
		if len(cells) < minSize || len(regions) >= 250 {
			continue
		}
		sx, sy := 0, 0
		for _, c := range cells {
			labels[c] = len(regions) + 1
			sx += c % l.W
			sy += c / l.W
		}
		center := world.Pos{X: sx / len(cells), Y: sy / len(cells)}
		regions = append(regions, Region{Name: regionName(r, kind, center, l, used), Kind: kind, Danger: RegionDanger[kind]})
	}
	// spread labels to everything else (breadth first from labelled cells)
	queue := make([]int, 0, n)
	for i, lb := range labels {
		if lb > 0 {
			queue = append(queue, i)
		}
	}
	for q := 0; q < len(queue); q++ {
		c := queue[q]
		cx, cy := c%l.W, c/l.W
		for _, d := range world.AllDirs {
			dd := d.Delta()
			nx, ny := cx+dd.X, cy+dd.Y
			if !l.In(nx, ny) {
				continue
			}
			ni := ny*l.W + nx
			if labels[ni] <= 0 {
				labels[ni] = labels[c]
				queue = append(queue, ni)
			}
		}
	}
	m := make([]uint8, n)
	for i, lb := range labels {
		if lb > 0 {
			m[i] = uint8(lb)
		}
	}
	return regions, m
}

func regionName(r *rand.Rand, kind string, c world.Pos, l *world.Level, used map[string]bool) string {
	list := regionNames[kind]
	var free []string
	for _, s := range list {
		if !used[s] {
			free = append(free, s)
		}
	}
	if len(free) > 0 {
		s := free[r.IntN(len(free))]
		used[s] = true
		return s
	}
	dir := "центр"
	dx, dy := float64(c.X)/float64(l.W)-0.5, (float64(c.Y)/float64(l.H)-0.5)*1.5
	switch {
	case math.Abs(dx) < 0.12 && math.Abs(dy) < 0.12:
	case math.Abs(dx) > math.Abs(dy) && dx > 0:
		dir = "восток"
	case math.Abs(dx) > math.Abs(dy):
		dir = "запад"
	case dy > 0:
		dir = "юг"
	default:
		dir = "север"
	}
	return list[r.IntN(len(list))] + " (" + dir + ")"
}

// ---- landmarks ----

var landmarkNames = map[string][]string{
	"shrine":    {"Святилище Перуна", "Святилище Велеса", "Святилище Лады", "Святилище Даждьбога", "Святилище Мокоши", "Святилище Стрибога", "Капище Сварога"},
	"circle":    {"Круг Камней", "Кольцо Древних", "Менгиры Предков", "Каменный Хоровод"},
	"ruins":     {"Руины Старой Башни", "Руины Заставы", "Развалины Храма", "Руины Сторожевого Поста", "Обломки Древнего Форта"},
	"graveyard": {"Старое Кладбище", "Забытый Погост", "Кладбище у Ручья", "Погост Безымянных"},
	"camp":      {"Лагерь Разбойников", "Логово Шайки", "Стоянка Головорезов"},
	"oasis":     {"Оазис Миражей", "Пальмовый Оазис", "Зелёный Оазис"},
}

var landmarkSpecs = []struct {
	kind   string
	count  int
	w, h   int
	biomes []string
}{
	{"camp", 2, 9, 7, []string{"plains", "hills", "forest", "sand"}},
	{"ruins", 3, 9, 7, []string{"plains", "forest", "hills", "desert", "tundra", "cursed", "sand"}},
	{"graveyard", 2, 11, 7, []string{"plains", "forest", "cursed"}},
	{"circle", 2, 9, 9, []string{"plains", "hills", "tundra", "forest"}},
	{"oasis", 2, 11, 7, []string{"desert"}},
	{"shrine", 5, 3, 3, []string{"plains", "forest", "hills", "desert", "tundra", "cursed", "swamp", "sand"}},
}

var groundFor = map[string]string{
	"plains": "grass", "sand": "sand", "forest": "forest_floor", "hills": "grass", "desert": "desert_sand",
	"tundra": "snow_ground", "cursed": "blight_grass", "swamp": "swamp", "ash": "ash",
}

func placeLandmarks(r *rand.Rand, l *world.Level, main []bool, vs []Village, start world.Pos) []Landmark {
	var out []Landmark
	used := map[string]bool{}
	for _, sp := range landmarkSpecs {
		placed := 0
		for attempt := 0; attempt < 3000 && placed < sp.count; attempt++ {
			x := sp.w + r.IntN(max(1, l.W-2*sp.w))
			y := sp.h + r.IntN(max(1, l.H-2*sp.h))
			if !main[y*l.W+x] {
				continue
			}
			biome := content.Tile(l.At(x, y)).Biome
			if !slices.Contains(sp.biomes, biome) || nearVillage(vs, x, y, sp.w) || abs(x-start.X)+abs(y-start.Y) < 20 {
				continue
			}
			ok := true
			for _, o := range out {
				if abs(o.Pos.X-x) < 18 && abs(o.Pos.Y-y) < 9 {
					ok = false
				}
			}
			if !ok || !freeArea(l, x-sp.w/2, y-sp.h/2, sp.w, sp.h) {
				continue
			}
			stampLandmark(r, l, sp.kind, world.Pos{X: x, Y: y}, sp.w, sp.h, groundFor[biome])
			out = append(out, Landmark{Name: pickUnique(r, landmarkNames[sp.kind], used), Kind: sp.kind, Pos: world.Pos{X: x, Y: y}})
			placed++
		}
	}
	return out
}

// freeArea: plain land (trees allowed) without water, rocks or buildings.
func freeArea(l *world.Level, x0, y0, w, h int) bool {
	for y := y0; y < y0+h; y++ {
		for x := x0; x < x0+w; x++ {
			if !l.In(x, y) {
				return false
			}
			def := l.Def(x, y)
			switch {
			case settlementTiles[def.Key], def.Damage > 0, def.Biome == "water", def.Key == "ice":
				return false
			case !def.Walkable && def.Key != "tree" && def.Key != "pine" && def.Key != "snow_pine" &&
				def.Key != "bush" && def.Key != "cactus" && def.Key != "dead_tree" && def.Key != "twisted_tree":
				return false
			}
		}
	}
	return true
}

func stampLandmark(r *rand.Rand, l *world.Level, kind string, c world.Pos, w, h int, ground string) {
	T := content.TileID
	if ground == "" {
		ground = "grass"
	}
	x0, y0 := c.X-w/2, c.Y-h/2
	for y := y0; y < y0+h; y++ {
		for x := x0; x < x0+w; x++ {
			if !l.Def(x, y).Walkable || kind != "shrine" {
				l.Set(x, y, T(ground))
			}
		}
	}
	set := func(x, y int, t string) { l.Set(x, y, T(t)) }
	switch kind {
	case "shrine":
		set(c.X, c.Y, "shrine")
		for _, d := range [][2]int{{-1, 1}, {1, 1}} {
			if r.IntN(2) == 0 && ground != "snow_ground" && ground != "desert_sand" && ground != "blight_grass" {
				set(c.X+d[0], c.Y+d[1], "flowers")
			}
		}
	case "circle":
		for i := 0; i < 8; i++ {
			a := float64(i) * math.Pi / 4
			set(c.X+int(math.Round(math.Cos(a)*3.4)), c.Y+int(math.Round(math.Sin(a)*3.4)), "menhir")
		}
		set(c.X, c.Y, "shrine")
	case "ruins":
		rx0, ry0, rw, rh := c.X-3, c.Y-2, 7, 5
		gate := r.IntN(4)
		for y := ry0; y < ry0+rh; y++ {
			for x := rx0; x < rx0+rw; x++ {
				border := x == rx0 || y == ry0 || x == rx0+rw-1 || y == ry0+rh-1
				switch {
				case border:
					isGate := (gate == 0 && y == ry0 && x == c.X) || (gate == 1 && y == ry0+rh-1 && x == c.X) ||
						(gate == 2 && x == rx0 && y == c.Y) || (gate == 3 && x == rx0+rw-1 && y == c.Y)
					switch {
					case isGate:
						set(x, y, "rubble")
					case r.Float64() < 0.7:
						set(x, y, "ruin_wall")
					default:
						set(x, y, "rubble")
					}
				case r.Float64() < 0.25:
					set(x, y, "rubble")
				}
			}
		}
		set(rx0+1, ry0+1, "pillar")
		set(c.X, c.Y, "chest")
		for i := 0; i < 4; i++ {
			set(x0+r.IntN(w), y0+r.IntN(h), "rubble")
		}
	case "graveyard":
		for y := y0 + 1; y < y0+h-1; y += 2 {
			for x := x0 + 1; x < x0+w-1; x += 2 {
				if r.Float64() < 0.75 {
					set(x, y, "gravestone")
				}
			}
		}
		set(x0, y0, "dead_tree")
		set(x0+w-1, y0+h-1, "dead_tree")
	case "camp":
		set(c.X, c.Y, "campfire")
		for _, d := range [][2]int{{-3, -2}, {3, -2}, {-3, 2}, {3, 2}} {
			if r.IntN(4) > 0 {
				set(c.X+d[0], c.Y+d[1], "tent")
			}
		}
		set(c.X, c.Y-2, "chest")
	case "oasis":
		for y := y0; y < y0+h; y++ {
			for x := x0; x < x0+w; x++ {
				dx, dy := float64(x-c.X)/3.6, float64(y-c.Y)/2.1
				d := dx*dx + dy*dy
				switch {
				case d <= 1:
					set(x, y, "water")
				case d <= 2.6 && r.Float64() < 0.3:
					set(x, y, "palm")
				case d <= 2.6:
					set(x, y, "grass")
					if r.Float64() < 0.3 {
						set(x, y, "tall_grass")
					}
				}
			}
		}
	}
}
