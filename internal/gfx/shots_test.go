package gfx

import (
	"bytes"
	"fmt"
	"image"
	"image/png"
	"net/http"
	"os"
	"path"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/gdamore/tcell/v2"
	"github.com/hajimehoshi/ebiten/v2"

	"ratas/internal/client"
	"ratas/internal/config"
	"ratas/internal/content"
	"ratas/internal/game"
	"ratas/internal/server"
	"ratas/internal/world"
)

// RATAS_GFX_SHOTS=<dir or http URL> runs a scripted session in the real
// renderer (a desktop window, or a browser when built for js/wasm) and saves
// screenshots of the menu, day, night, a dungeon fight, a dialogue and menus.
func TestMain(m *testing.M) {
	if dir := os.Getenv("RATAS_GFX_SHOTS"); dir != "" {
		if err := runShots(dir); err != nil {
			fmt.Println("shots:", err)
			os.Exit(1)
		}
		os.Exit(0)
	}
	os.Exit(m.Run())
}

func runShots(dir string) error {
	if os.Getenv("RATAS_HOME") == "" {
		if home, err := os.MkdirTemp("", "ratas-shots"); err == nil {
			os.Setenv("RATAS_HOME", home)
		}
	}
	db, _, err := content.LoadDefault("")
	if err != nil {
		return err
	}
	content.Use(db)
	f, err := loadFonts()
	if err != nil {
		return err
	}
	st := &shared{}
	scr := &gridScreen{SimulationScreen: tcell.NewSimulationScreen("UTF-8"), st: st}
	scr.Init()
	g := &Game{st: st, scr: scr, done: make(chan error, 1), fonts: f, input: newInput(), last: time.Now(),
		shotReq: make(chan string), shotDone: make(chan error)}
	g.world = newWorldRenderer(f)
	g.menu = newMenuBackdrop(g.world)
	g.shotFull = true
	g.saveShot = func(img image.Image, p string) error {
		full := img.(*image.RGBA)
		b := full.Bounds()
		half := image.NewRGBA(image.Rect(0, 0, b.Dx()/2, b.Dy()/2))
		for y := 0; y < b.Dy()/2; y++ {
			for x := 0; x < b.Dx()/2; x++ {
				half.SetRGBA(x, y, full.RGBAAt(x*2, y*2))
			}
		}
		// a full-resolution crop around the hero (the map centre)
		cx, cy := b.Dx()*(b.Dx()-int(float64(sideCols)*g.cellW))/b.Dx()/2, b.Dy()/2
		crop := full.SubImage(image.Rect(cx-500, cy-330, cx+500, cy+330))
		if err := put(dir, p, half); err != nil {
			return err
		}
		return put(dir, strings.TrimSuffix(p, ".png")+"_zoom.png", crop)
	}
	ebiten.SetWindowSize(1280, 800)
	ebiten.SetWindowTitle("Ратас — тест графики")

	srvCh := make(chan *server.Server, 1)
	client.LocalServerHook = func(s *server.Server) { srvCh <- s }
	cfg := config.Default()
	cfg.AIEnabled, cfg.Name = false, "Герой"
	go func() { g.done <- client.RunWith(scr, st, cfg, nil, client.StartOptions{}) }()
	go script(g, srvCh, dir)
	if err := ebiten.RunGame(g); err != nil && err != ebiten.Termination {
		return err
	}
	return nil
}

const sideCols = 30

func put(dir, p string, img image.Image) error {
	if !strings.HasPrefix(dir, "http") {
		return writePNG(img, p)
	}
	var buf bytes.Buffer
	if err := png.Encode(&buf, img); err != nil {
		return err
	}
	resp, err := http.Post(dir+"?name="+path.Base(p), "image/png", &buf)
	if err != nil {
		return err
	}
	resp.Body.Close()
	return nil
}

func script(g *Game, srvCh chan *server.Server, dir string) {
	scr := g.scr
	shot := func(name string) {
		g.shotReq <- filepath.Join(dir, name+".png")
		if err := <-g.shotDone; err != nil {
			fmt.Println("screenshot:", err)
		}
	}
	key := func(k tcell.Key, n int) {
		for i := 0; i < n; i++ {
			scr.InjectKey(k, 0, tcell.ModNone)
			time.Sleep(160 * time.Millisecond)
		}
	}
	char := func(r rune) {
		scr.InjectKey(tcell.KeyRune, r, tcell.ModNone)
		time.Sleep(160 * time.Millisecond)
	}
	player := func(gm *game.Game) *game.Entity { return gm.Online["Герой"] }
	// regionSpot finds walkable land deep inside a biome
	regionSpot := func(gm *game.Game, biome string) (world.Pos, bool) {
		l := gm.Levels["overworld"]
		best, bn := world.Pos{}, 0
		for y := 6; y < l.H-6; y += 2 {
			for x := 10; x < l.W-10; x += 2 {
				if !l.Free(x, y) || l.Def(x, y).Biome != biome {
					continue
				}
				n := 0
				for dy := -5; dy <= 5; dy++ {
					for dx := -10; dx <= 10; dx++ {
						if l.Def(x+dx, y+dy).Biome == biome {
							n++
						}
					}
				}
				if n > bn {
					best, bn = world.Pos{X: x, Y: y}, n
				}
			}
		}
		return best, bn > 0
	}
	visit := func(gm *game.Game, level string, at world.Pos, tod float64, monsters ...string) {
		p := player(gm)
		gm.Now = tod * game.DayMs
		gm.PlaceForTest(p, level, at)
		for _, e := range gm.Entities {
			if e.Kind == game.KMonster && e.Level == p.Level && e.Pos.Dist(p.Pos) < 12 {
				gm.Remove(e)
			}
		}
		for i, m := range monsters {
			d := []world.Pos{{X: 3, Y: -2}, {X: -3, Y: -1}, {X: 2, Y: 2}, {X: -2, Y: 3}, {X: 4, Y: 1}, {X: -4, Y: -3}}[i%6]
			gm.SpawnForTest(m, p.Level, p.Pos.Add(d), 3)
		}
	}
	dungeon := func(gm *game.Game, theme string) string {
		for i, e := range gm.Entrances {
			if e.Theme == theme {
				return game.DungeonLevelID(i, 1)
			}
		}
		return game.DungeonLevelID(0, 1)
	}

	time.Sleep(4 * time.Second)
	shot("01_menu")
	key(tcell.KeyEnter, 1) // Новая игра
	key(tcell.KeyDown, 1)  // class
	key(tcell.KeyRight, 3) // priest
	key(tcell.KeyDown, 1)  // seed
	key(tcell.KeyCtrlU, 1)
	char('4')
	char('2')
	time.Sleep(300 * time.Millisecond)
	shot("02_new_game")
	key(tcell.KeyDown, 3) // mode, PvP, start
	key(tcell.KeyEnter, 1)
	srv := <-srvCh
	time.Sleep(3 * time.Second)
	shot("03_village")

	srv.Call(func(gm *game.Game) {
		p := player(gm)
		gm.ToughForTest(p)
		p.MaxMP, p.MP = 5000, 5000
		for _, a := range []string{"curse_weakness", "agony", "chain_lightning", "fireball", "frost_nova"} {
			gm.GrantForTest(p, a)
		}
		for _, it := range []string{"flame_sword", "frost_heart", "potion_fire_res", "antidote", "scroll_return", "tome_oblivion", "venom_dagger", "dragonscale"} {
			gm.GiveForTest(p, it)
		}
		gm.Paused = false
	})
	type place struct {
		name, biome string
		tod         float64
		monsters    []string
	}
	for _, pl := range []place{
		{"04_desert", "desert", 0.45, []string{"scorpion", "nomad", "mummy", "efreet", "scarab", "sand_worm"}},
		{"05_tundra", "tundra", 0.4, []string{"ice_wolf", "yeti", "frost_giant", "ice_elemental", "ice_wolf"}},
		{"06_ash", "ash", 0.62, []string{"fire_imp", "salamander", "magma_golem", "fire_drake", "fire_imp"}},
		{"07_cursed", "cursed", 0.7, []string{"shadow_wolf", "cultist", "banshee", "dark_knight", "risen", "wandering_skeleton"}},
	} {
		pl := pl
		srv.Call(func(gm *game.Game) {
			if at, ok := regionSpot(gm, pl.biome); ok {
				visit(gm, "overworld", at, pl.tod, pl.monsters...)
			}
		})
		time.Sleep(1800 * time.Millisecond)
		shot(pl.name)
	}
	// curses and damage over time on the cursed land's monsters
	for _, k := range []rune{'2', '3', '4', '1'} {
		char(k)
		time.Sleep(350 * time.Millisecond)
	}
	time.Sleep(500 * time.Millisecond)
	shot("08_curses")

	srv.Call(func(gm *game.Game) {
		for _, lm := range gm.Landmarks {
			if lm.Kind == "camp" {
				p := player(gm)
				gm.Now = 0.68 * game.DayMs
				gm.PlaceForTest(p, "overworld", lm.Pos.Add(world.Pos{X: 0, Y: 3}))
				break
			}
		}
	})
	time.Sleep(1800 * time.Millisecond)
	shot("09_bandit_camp")

	for _, d := range []struct {
		name, theme string
		monsters    []string
		keys        []rune
	}{
		{"10_temple", "temple", []string{"mummy", "tomb_guardian", "scarab", "skeleton", "pharaoh"}, []rune{'4'}},
		{"11_ice_cave", "ice", []string{"ice_wolf", "yeti", "ice_elemental", "wraith"}, []rune{'6', '5'}},
		{"12_volcano", "volcano", []string{"fire_imp", "salamander", "magma_golem", "orc", "magma_lord"}, nil},
		{"13_fortress", "fortress", []string{"cultist", "demon", "dark_knight", "shadow_wolf", "demon_lord"}, []rune{'5'}},
	} {
		d := d
		srv.Call(func(gm *game.Game) {
			id := dungeon(gm, d.theme)
			l := gm.Level(id)
			visit(gm, id, l.Up, 0.5, d.monsters...)
		})
		time.Sleep(1500 * time.Millisecond)
		for _, k := range d.keys {
			char(k)
			time.Sleep(400 * time.Millisecond)
		}
		time.Sleep(600 * time.Millisecond)
		shot(d.name)
	}

	char('c')
	time.Sleep(500 * time.Millisecond)
	shot("14_character")
	key(tcell.KeyEscape, 1)
	char('i')
	key(tcell.KeyDown, 3)
	time.Sleep(500 * time.Millisecond)
	shot("15_inventory")
	key(tcell.KeyEscape, 1)
	char('k')
	key(tcell.KeyRight, 1)
	time.Sleep(500 * time.Millisecond)
	shot("16_skills")
	key(tcell.KeyEscape, 1)

	// heroes in their gear, a party, allies of the open world against a gang
	var friend *game.Entity
	srv.Call(func(gm *game.Game) {
		p := player(gm)
		for _, it := range []string{"priest_mitre", "holy_plate", "mantle_light"} {
			gm.EquipForTest(p, it, false)
		}
		gm.EquipForTest(p, "holy_symbol", true)
		friend = gm.JoinForTest("Лира", "rogue")
		gm.EquipForTest(friend, "ranger_cloak", false)
		gm.PartyForTest(p, friend)
		gm.GrantForTest(p, "decoy")
		at, _ := regionSpot(gm, "")
		for _, v := range gm.Villages {
			at = v.Center.Add(world.Pos{X: 14, Y: 6})
			break
		}
		visit(gm, "overworld", at, 0.42, "bandit", "bandit_archer", "bandit_mage", "bandit_thief", "bandit_chief")
		gm.PlaceForTest(friend, "overworld", p.Pos.Add(world.Pos{X: -1, Y: 1}))
		for i, k := range []string{"knight_errant", "battle_mage", "hunter_npc"} {
			gm.SpawnForTest(k, "overworld", p.Pos.Add(world.Pos{X: -2 + 2*i, Y: -2}), 4)
		}
	})
	time.Sleep(1500 * time.Millisecond)
	shot("17_allies")

	// a unique character with something to offer
	srv.Call(func(gm *game.Game) {
		p := player(gm)
		for _, e := range gm.Entities {
			if e.NPC != nil && e.NPC.Unique != "" {
				gm.MoveForTest(p, e)
				break
			}
		}
	})
	time.Sleep(1500 * time.Millisecond)
	shot("18_unique")

	// the whole land explored (a level change sends the explored map)
	var back world.Pos
	srv.Call(func(gm *game.Game) {
		p := player(gm)
		back = p.Pos
		gm.ExploreForTest(p)
		gm.PlaceForTest(p, dungeon(gm, "cave"), world.Pos{})
	})
	time.Sleep(500 * time.Millisecond)
	srv.Call(func(gm *game.Game) {
		p := player(gm)
		gm.PlaceForTest(p, "overworld", back)
		gm.PlaceForTest(friend, "overworld", p.Pos.Add(world.Pos{X: 30, Y: 8}))
	})
	time.Sleep(700 * time.Millisecond)
	char('m')
	time.Sleep(1200 * time.Millisecond)
	shot("19_world_map")
	key(tcell.KeyEscape, 1)
	char('g')
	time.Sleep(500 * time.Millisecond)
	shot("20_party")
	key(tcell.KeyEscape, 1)

	// a fallen companion lies where it fell until it rises or is raised
	srv.Call(func(gm *game.Game) {
		p := player(gm)
		gm.Now = 0.45 * game.DayMs
		gm.PlaceForTest(p, "overworld", gm.Start.Add(world.Pos{X: 0, Y: 4}))
		gm.PlaceForTest(friend, "overworld", p.Pos.Add(world.Pos{X: 2, Y: 0}))
		gm.KillForTest(friend)
	})
	time.Sleep(1200 * time.Millisecond)
	shot("21_fallen")
	client.RequestClose(scr)
}
