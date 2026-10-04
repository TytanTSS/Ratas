package gen

import (
	"os"
	"strings"
	"testing"

	"ratas/internal/content"
	"ratas/internal/world"
)

func loadContent(t *testing.T) {
	t.Helper()
	db, _, err := content.LoadDefault("")
	if err != nil {
		t.Fatal(err)
	}
	content.Use(db)
}

func dump(l *world.Level) string {
	var b strings.Builder
	for y := 0; y < l.H; y++ {
		for x := 0; x < l.W; x++ {
			b.WriteRune(l.Def(x, y).Rune)
		}
		b.WriteByte('\n')
	}
	return b.String()
}

func TestOverworld(t *testing.T) {
	loadContent(t)
	for _, seed := range []int64{1, 2, 3, 42, 1337} {
		ow := GenerateOverworld(seed, 300, 150)
		if len(ow.Villages) < 3 {
			t.Errorf("seed %d: only %d villages", seed, len(ow.Villages))
		}
		if len(ow.Entrances) < 4 {
			t.Errorf("seed %d: only %d dungeon entrances", seed, len(ow.Entrances))
		}
		if !ow.Level.Walkable(ow.Start.X, ow.Start.Y) {
			t.Errorf("seed %d: start not walkable", seed)
		}
		dist := bfs(ow.Level, ow.Start)
		for _, e := range ow.Entrances {
			if dist[e.Pos.Y*ow.Level.W+e.Pos.X] < 0 {
				t.Errorf("seed %d: entrance %s unreachable", seed, e.Name)
			}
		}
		for _, v := range ow.Villages {
			if dist[(v.Center.Y+1)*ow.Level.W+v.Center.X] < 0 {
				t.Errorf("seed %d: village %s unreachable", seed, v.Name)
			}
		}
		themes := map[string]bool{}
		for _, e := range ow.Entrances {
			themes[e.Theme] = true
		}
		kinds := map[string]bool{}
		for _, r := range ow.Regions {
			kinds[r.Kind] = true
		}
		if len(ow.RegionMap) != 300*150 || len(ow.Regions) < 5 || len(ow.Landmarks) < 8 {
			t.Errorf("seed %d: %d regions, %d landmarks", seed, len(ow.Regions), len(ow.Landmarks))
		}
		for _, lm := range ow.Landmarks {
			if dist[lm.Pos.Y*ow.Level.W+lm.Pos.X] < 0 && ow.Level.Walkable(lm.Pos.X, lm.Pos.Y) {
				t.Errorf("seed %d: landmark %s unreachable", seed, lm.Name)
			}
		}
		t.Logf("seed %d: regions %v, dungeons %v, %d landmarks", seed, kinds, themes, len(ow.Landmarks))
		if seed == 42 && os.Getenv("RATAS_DUMP") != "" {
			os.WriteFile(os.Getenv("RATAS_DUMP")+"/overworld.txt", []byte(dump(ow.Level)), 0o644)
		}
	}
}

func TestDungeons(t *testing.T) {
	loadContent(t)
	for _, theme := range Themes() {
		for depth := 1; depth <= 5; depth++ {
			for seed := int64(0); seed < 10; seed++ {
				f := GenerateDungeon(seed, "d", "Тест", theme, 0, depth, 5)
				l := f.Level
				if !l.Walkable(l.Up.X, l.Up.Y) {
					t.Fatalf("%s/%d/%d: up stairs not walkable", theme, depth, seed)
				}
				dist := bfs(l, l.Up)
				if depth < 5 {
					if dist[l.Down.Y*l.W+l.Down.X] < 0 {
						t.Errorf("%s/%d/%d: down stairs unreachable", theme, depth, seed)
					}
				} else if f.Boss == nil || dist[f.Boss.Y*l.W+f.Boss.X] < 0 {
					t.Errorf("%s/%d/%d: boss spot missing or unreachable", theme, depth, seed)
				}
				if seed == 0 && depth == 2 && os.Getenv("RATAS_DUMP") != "" {
					os.WriteFile(os.Getenv("RATAS_DUMP")+"/"+theme+".txt", []byte(dump(l)), 0o644)
				}
			}
		}
	}
}
