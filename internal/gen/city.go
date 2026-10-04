package gen

import (
	"math/rand/v2"
	"sort"

	"ratas/internal/content"
	"ratas/internal/world"
)

var cityNames = []string{
	"Белокаменск", "Златоград", "Высокий Престол", "Светлоград", "Твердислав",
	"Каменная Гавань", "Звенигород", "Старый Кремень",
}

// City size in cells; terminal cells are about twice as tall as wide.
const cityW, cityH = 62, 30

// placeCities puts up to want walled cities on the main landmass: any dry
// land will do (the city is paved over), with little water or mountains.
func placeCities(r *rand.Rand, l *world.Level, main []bool, want int, used map[string]bool) []Village {
	var cs []Village
	for _, maxBad := range []int{6, 14, 25} {
		for attempt := 0; attempt < 3000 && len(cs) < want; attempt++ {
			cx := cityW/2 + 6 + r.IntN(l.W-cityW-12)
			cy := cityH/2 + 4 + r.IntN(l.H-cityH-8)
			area := Rect{cx - cityW/2, cy - cityH/2, cityW, cityH}
			bad, total := 0, 0
			for y := area.Y - 1; y <= area.Y+area.H; y++ {
				for x := area.X - 2; x <= area.X+area.W+1; x++ {
					total++
					def := content.Tile(l.At(x, y))
					switch {
					case !main[y*l.W+x] && !def.Walkable:
						bad += 2
					case def.Biome == "water" || def.Biome == "snow" || def.Key == "mountain" || def.Damage > 0:
						bad++
					}
				}
			}
			if bad*100/total > maxBad {
				continue
			}
			far := true
			for _, c := range cs {
				if c.Area.Overlaps(area, 30) {
					far = false
				}
			}
			if far {
				cs = append(cs, buildCity(r, l, area, pickUnique(r, cityNames, used)))
			}
		}
		if len(cs) > 0 {
			break
		}
	}
	return cs
}

// building is a house of a city with a purpose ("" for a plain home).
type building struct {
	Rect
	kind string
	door world.Pos
}

// City buildings with a purpose, the most important first (they get the
// largest houses); NPC roles name the one they work in.
var cityBuildings = []string{"temple", "townhall", "tavern", "armory", "smithy", "magic", "alchemy", "jewelry", "barracks"}

// buildCity stamps a walled stone city: gates on every side, two main streets
// crossing at a plaza with a fountain and a market, stone houses in four
// quarters, lamps along the streets.
func buildCity(r *rand.Rand, l *world.Level, area Rect, name string) Village {
	T := content.TileID
	v := Village{Name: name, Center: area.Center(), Area: area, City: true}
	c := v.Center
	x0, y0, x1, y1 := area.X, area.Y, area.X+area.W-1, area.Y+area.H-1
	for y := y0; y <= y1; y++ {
		for x := x0; x <= x1; x++ {
			switch {
			case x == x0 || x == x1 || y == y0 || y == y1:
				l.Set(x, y, T("city_wall"))
			default:
				l.Set(x, y, T("cobblestone"))
			}
		}
	}
	// a ring of grass outside the walls so the gates are never blocked
	for y := y0 - 1; y <= y1+1; y++ {
		for x := x0 - 2; x <= x1+2; x++ {
			if l.In(x, y) && !area.Contains(x, y) && !l.Walkable(x, y) {
				l.Set(x, y, T("grass"))
			}
		}
	}
	// corner towers
	for _, p := range []world.Pos{{X: x0, Y: y0}, {X: x1 - 2, Y: y0}, {X: x0, Y: y1 - 1}, {X: x1 - 2, Y: y1 - 1}} {
		for dy := 0; dy < 2; dy++ {
			for dx := 0; dx < 3; dx++ {
				l.Set(p.X+dx, p.Y+dy, T("city_wall"))
			}
		}
	}
	// gates
	for dx := -1; dx <= 1; dx++ {
		l.Set(c.X+dx, y0, T("city_gate"))
		l.Set(c.X+dx, y1, T("city_gate"))
	}
	for dy := -1; dy <= 1; dy++ {
		l.Set(x0, c.Y+dy, T("city_gate"))
		l.Set(x1, c.Y+dy, T("city_gate"))
	}

	// free ground: streets and the plaza stay open
	streetH := Rect{x0 + 1, c.Y - 1, area.W - 2, 3}
	streetV := Rect{c.X - 2, y0 + 1, 5, area.H - 2}
	plaza := Rect{c.X - 9, c.Y - 4, 19, 9}
	reserved := []Rect{streetH, streetV, plaza}

	// fountain, statues, lamps and market stalls on the plaza
	for dx := -1; dx <= 1; dx++ {
		l.Set(c.X+dx, c.Y, T("fountain"))
	}
	for _, p := range []world.Pos{{X: plaza.X, Y: plaza.Y}, {X: plaza.X + plaza.W - 1, Y: plaza.Y},
		{X: plaza.X, Y: plaza.Y + plaza.H - 1}, {X: plaza.X + plaza.W - 1, Y: plaza.Y + plaza.H - 1}} {
		l.Set(p.X, p.Y, T("lamp_post"))
	}
	l.Set(plaza.X+1, c.Y-3, T("statue"))
	l.Set(plaza.X+plaza.W-2, c.Y+3, T("statue"))
	for _, row := range []int{c.Y - 3, c.Y + 3} {
		for x := plaza.X + 3; x < plaza.X+plaza.W-3; x += 3 {
			if x >= streetV.X-1 && x <= streetV.X+streetV.W {
				continue
			}
			l.Set(x, row, T("market_stall"))
		}
	}
	// lamps along the main streets, just off the pavement
	for x := x0 + 4; x < x1-3; x += 8 {
		if !plaza.Contains(x, c.Y-2) {
			l.Set(x, c.Y-2, T("lamp_post"))
			l.Set(x, c.Y+2, T("lamp_post"))
		}
	}
	for y := y0 + 3; y < y1-2; y += 6 {
		if !plaza.Contains(c.X-3, y) && !streetH.Contains(c.X-3, y) {
			l.Set(c.X-3, y, T("lamp_post"))
			l.Set(c.X+3, y, T("lamp_post"))
		}
	}

	// houses in the four quarters
	quarters := []Rect{
		{x0 + 2, y0 + 2, streetV.X - x0 - 3, streetH.Y - y0 - 3},
		{streetV.X + streetV.W + 1, y0 + 2, x1 - streetV.X - streetV.W - 2, streetH.Y - y0 - 3},
		{x0 + 2, streetH.Y + streetH.H + 1, streetV.X - x0 - 3, y1 - streetH.Y - streetH.H - 2},
		{streetV.X + streetV.W + 1, streetH.Y + streetH.H + 1, x1 - streetV.X - streetV.W - 2, y1 - streetH.Y - streetH.H - 2},
	}
	// rows of houses with one-cell alleys between them; a few lots stay
	// empty as yards
	var houses []Rect
	for _, q := range quarters {
		for y := q.Y; y+4 <= q.Y+q.H; {
			rowH := min(Range(r, 4, 5), q.Y+q.H-y)
			w := 0
			for x := q.X; x+6 <= q.X+q.W; {
				if w == 0 {
					w = Range(r, 6, 11)
				}
				w = min(w, q.X+q.W-x)
				hr := Rect{x, y, w, rowH}
				blocked := false
				for _, o := range reserved {
					if hr.Overlaps(o, 1) {
						blocked = true
					}
				}
				if blocked {
					// a narrower house, or slide past the plaza
					if w > 6 {
						w--
					} else {
						x++
					}
					continue
				}
				if r.IntN(12) > 0 {
					houses = append(houses, hr)
				}
				x += w + 1
				w = 0
			}
			y += rowH + 1
		}
	}
	// the biggest houses become the important buildings
	sort.Slice(houses, func(i, j int) bool { return houses[i].W*houses[i].H > houses[j].W*houses[j].H })
	var bs []building
	for i, h := range houses {
		kind := ""
		if i < len(cityBuildings) {
			kind = cityBuildings[i]
		}
		bs = append(bs, stampHouse(l, h, kind, c))
	}
	// little gardens in the alleys
	for i := 0; i < 30; i++ {
		x, y := Range(r, x0+2, x1-2), Range(r, y0+2, y1-2)
		if content.Tile(l.At(x, y)).Key != "cobblestone" {
			continue
		}
		free := true
		for _, o := range reserved {
			if o.Contains(x, y) {
				free = false
			}
		}
		for _, b := range bs {
			if b.door.Dist(world.Pos{X: x, Y: y}) <= 2 {
				free = false
			}
		}
		if free {
			l.Set(x, y, T("garden"))
		}
	}

	// people
	byKind := map[string]building{}
	for _, b := range bs {
		if b.kind != "" {
			byKind[b.kind] = b
		}
	}
	inside := func(b building, k int) world.Pos {
		ctr := b.Center()
		p := world.Pos{X: ctr.X - 1 + k%3, Y: ctr.Y}
		if !l.Walkable(p.X, p.Y) {
			p = ctr
		}
		return p
	}
	streetSpot := func(near Rect) world.Pos {
		for i := 0; i < 300; i++ {
			x, y := Range(r, near.X, near.X+near.W-1), Range(r, near.Y, near.Y+near.H-1)
			if l.Walkable(x, y) && content.Tile(l.At(x, y)).Key != "stone_floor" && content.Tile(l.At(x, y)).Key != "carpet" &&
				content.Tile(l.At(x, y)).Key != "house_floor" {
				return world.Pos{X: x, Y: y}
			}
		}
		return world.Pos{X: c.X, Y: c.Y + 2}
	}
	gates := []world.Pos{{X: c.X, Y: y0 + 1}, {X: c.X, Y: y1 - 1}, {X: x0 + 1, Y: c.Y}, {X: x1 - 1, Y: c.Y}}
	tavern, hasTavern := byKind["tavern"]
	night := world.Pos{X: c.X, Y: c.Y + 2}
	if hasTavern {
		night = tavern.door
	}
	guards := 0
	for _, role := range content.NPCRoles() {
		cnt := Range(r, role.CityCount[0], role.CityCount[1])
		b, hasB := byKind[role.Building]
		for k := 0; k < cnt; k++ {
			s := NPCSpawn{Role: role.Key, Name: PersonName(r)}
			switch {
			case hasB:
				s.Pos = inside(b, k)
			case role.Combat != "":
				// guards keep the gates, the rest walk the plaza
				if guards < len(gates) {
					s.Pos = gates[guards]
				} else {
					s.Pos = streetSpot(plaza)
				}
				guards++
			case role.Trader:
				s.Pos = streetSpot(plaza)
			default:
				s.Pos = streetSpot(Rect{x0 + 2, y0 + 2, area.W - 4, area.H - 4})
				s.Night = night
			}
			v.NPCs = append(v.NPCs, s)
		}
	}
	for _, b := range bs {
		v.Houses = append(v.Houses, b.Rect)
	}
	return v
}

// stampHouse builds a stone house with its door toward the city center and
// furnishes it by purpose.
func stampHouse(l *world.Level, h Rect, kind string, c world.Pos) building {
	T := content.TileID
	floor := T("stone_floor")
	switch kind {
	case "tavern", "":
		floor = T("house_floor")
	case "temple", "townhall", "magic", "jewelry":
		floor = T("carpet")
	}
	for y := h.Y; y < h.Y+h.H; y++ {
		for x := h.X; x < h.X+h.W; x++ {
			if x == h.X || y == h.Y || x == h.X+h.W-1 || y == h.Y+h.H-1 {
				l.Set(x, y, T("stone_wall"))
			} else {
				l.Set(x, y, floor)
			}
		}
	}
	hc := h.Center()
	var door world.Pos
	switch world.DirTowards(hc, c) {
	case world.DirUp:
		door = world.Pos{X: hc.X, Y: h.Y}
	case world.DirDown:
		door = world.Pos{X: hc.X, Y: h.Y + h.H - 1}
	case world.DirLeft:
		door = world.Pos{X: h.X, Y: hc.Y}
	default:
		door = world.Pos{X: h.X + h.W - 1, Y: hc.Y}
	}
	l.Set(door.X, door.Y, T("door"))
	// furniture against the wall opposite the door
	back := world.Pos{X: hc.X, Y: h.Y + 1}
	if door.Y == h.Y {
		back = world.Pos{X: hc.X, Y: h.Y + h.H - 2}
	}
	switch kind {
	case "temple":
		l.Set(back.X, back.Y, T("altar"))
		l.Set(h.X+1, h.Y+1, T("brazier"))
		l.Set(h.X+h.W-2, h.Y+1, T("brazier"))
	case "smithy":
		l.Set(h.X+1, back.Y, T("brazier"))
	case "magic":
		l.Set(back.X, back.Y, T("crystal"))
	case "townhall", "barracks", "armory", "jewelry", "alchemy":
		if kind == "jewelry" || kind == "townhall" {
			l.Set(h.X+1, back.Y, T("chest_open"))
		}
	}
	return building{Rect: h, kind: kind, door: door}
}
