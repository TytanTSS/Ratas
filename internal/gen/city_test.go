package gen

import (
	"os"
	"testing"

	"ratas/internal/content"
	"ratas/internal/world"
)

// Every world gets a walled city reachable from the start through its gates,
// with townsfolk inside.
func TestCities(t *testing.T) {
	loadContent(t)
	for _, seed := range []int64{1, 2, 3, 7, 42, 99, 1337, 2024} {
		ow := GenerateOverworld(seed, 300, 150)
		l := ow.Level
		var cities []Village
		for _, v := range ow.Villages {
			if v.City {
				cities = append(cities, v)
			}
		}
		if len(cities) == 0 {
			t.Errorf("seed %d: no city", seed)
			continue
		}
		if ow.Villages[0].City {
			t.Errorf("seed %d: the start is in a city, not a village", seed)
		}
		// flood fill from the start over walkable land and opened doors
		seen := make([]bool, l.W*l.H)
		stack := []world.Pos{ow.Start}
		seen[ow.Start.Y*l.W+ow.Start.X] = true
		for len(stack) > 0 {
			p := stack[len(stack)-1]
			stack = stack[:len(stack)-1]
			for _, d := range world.AllDirs {
				q := p.Add(d.Delta())
				if !l.In(q.X, q.Y) || seen[q.Y*l.W+q.X] {
					continue
				}
				if def := l.Def(q.X, q.Y); def.Walkable || def.Interact == "door" {
					seen[q.Y*l.W+q.X] = true
					stack = append(stack, q)
				}
			}
		}
		for _, c := range cities {
			gates, walls := 0, 0
			a := c.Area
			for y := a.Y; y < a.Y+a.H; y++ {
				for x := a.X; x < a.X+a.W; x++ {
					switch content.Tile(l.At(x, y)).Key {
					case "city_gate":
						gates++
						if !seen[y*l.W+x] {
							t.Errorf("seed %d %s: gate (%d,%d) unreachable from the start", seed, c.Name, x, y)
						}
					case "city_wall":
						walls++
					}
				}
			}
			if gates != 12 || walls < 2*(a.W+a.H)-20 {
				t.Errorf("seed %d %s: %d gate tiles, %d wall tiles", seed, c.Name, gates, walls)
			}
			plaza := world.Pos{X: c.Center.X, Y: c.Center.Y + 2}
			if !seen[plaza.Y*l.W+plaza.X] {
				t.Errorf("seed %d %s: the plaza is unreachable", seed, c.Name)
			}
			roles := map[string]int{}
			for _, n := range c.NPCs {
				roles[n.Role]++
				if !a.Contains(n.Pos.X, n.Pos.Y) || !l.Walkable(n.Pos.X, n.Pos.Y) {
					t.Errorf("seed %d %s: %s placed at %v", seed, c.Name, n.Role, n.Pos)
				}
			}
			for _, r := range []string{"mayor", "innkeeper", "armorer", "enchanter", "smith", "guard", "villager"} {
				if roles[r] == 0 {
					t.Errorf("seed %d %s: no %s (%v)", seed, c.Name, r, roles)
				}
			}
		}
		if seed == 42 && os.Getenv("RATAS_DUMP_CITY") != "" {
			c := cities[0].Area
			var s []byte
			for y := c.Y - 1; y < c.Y+c.H+1; y++ {
				for x := c.X - 2; x < c.X+c.W+2; x++ {
					s = append(s, []byte(string(l.Def(x, y).Rune))...)
				}
				s = append(s, '\n')
			}
			os.WriteFile(os.Getenv("RATAS_DUMP_CITY"), s, 0o644)
		}
	}
}
