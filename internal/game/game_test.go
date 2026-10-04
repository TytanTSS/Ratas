package game

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/anthropics/anthropic-sdk-go/option"

	"ratas/internal/llm"

	"ratas/internal/content"
	"ratas/internal/proto"
	"ratas/internal/world"
)

func setup(t *testing.T) *Game {
	t.Helper()
	db, _, err := content.LoadDefault("")
	if err != nil {
		t.Fatal(err)
	}
	content.Use(db)
	return New(7, nil)
}

func run(g *Game, ticks int) {
	for i := 0; i < ticks; i++ {
		g.Tick()
		for _, p := range g.Online {
			g.Snapshot(p)
			g.TakeOutbox(p)
		}
		g.EndFrame()
	}
}

func TestJoinMoveAndExplore(t *testing.T) {
	g := setup(t)
	p, need, err := g.Join("Тест", "")
	if err != nil || !need || p != nil {
		t.Fatalf("expected class prompt, got %v %v %v", p, need, err)
	}
	p, _, err = g.Join("Тест", "warrior")
	if err != nil || p == nil {
		t.Fatal(err)
	}
	if _, _, err := g.Join("Тест", "warrior"); err != ErrNameTaken {
		t.Fatalf("duplicate name should fail, got %v", err)
	}
	if p.HP <= 0 || p.MaxHP < 50 {
		t.Fatalf("bad hp %v/%v", p.HP, p.MaxHP)
	}
	start := p.Pos
	moved := false
	for _, d := range world.AllDirs {
		g.SetInput(p, proto.Input{Move: uint8(d)})
		run(g, 5)
		if p.Pos != start {
			moved = true
			break
		}
	}
	if !moved {
		t.Fatal("player could not move in any direction")
	}
	if len(p.Player.Explored["overworld"]) == 0 {
		t.Fatal("explored map not updated")
	}
	run(g, 200) // spawner + ai
	monsters := 0
	for _, e := range g.Entities {
		if e.Kind == KMonster {
			monsters++
		}
	}
	if monsters == 0 {
		t.Fatal("no monsters spawned")
	}
}

func TestDungeonCombatAndSave(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Герой", "mage")
	// teleport into the first dungeon
	lvl := g.Level(DungeonLevelID(0, 1))
	if lvl == nil {
		t.Fatal("dungeon not generated")
	}
	g.changeLevel(p, lvl, lvl.Up)
	if p.Level != lvl.ID {
		t.Fatal("level change failed")
	}
	// make the hero strong and wait for enemies to come
	p.MaxHP, p.HP = 5000, 5000
	kills := 0
	for i := 0; i < 400; i++ {
		var target *Entity
		for _, e := range g.Entities {
			if e.Kind == KMonster && e.Level == lvl.ID {
				if target == nil || e.Pos.DistSq(p.Pos) < target.Pos.DistSq(p.Pos) {
					target = e
				}
			}
		}
		if target == nil {
			break
		}
		// walk next to it and cast
		g.moveEntity(p, findFree(lvl, target.Pos))
		p.MP = p.MaxMP
		g.SetInput(p, proto.Input{Ability: 1, Attack: true})
		before := p.Player.Kills
		run(g, 10)
		kills += p.Player.Kills - before
	}
	if kills == 0 {
		t.Fatal("no kills in dungeon")
	}
	if p.Player.XP == 0 && p.Player.Level == 1 {
		t.Fatal("no experience gained")
	}
	path := filepath.Join(t.TempDir(), "w.sav")
	if err := g.Save(path); err != nil {
		t.Fatal(err)
	}
	g2, err := Load(path, nil)
	if err != nil {
		t.Fatal(err)
	}
	p2, _, err := g2.Join("Герой", "")
	if err != nil || p2 == nil {
		t.Fatalf("rejoin failed: %v", err)
	}
	if p2.Player.Level != p.Player.Level || p2.Level != p.Level || p2.Player.Kills != p.Player.Kills {
		t.Fatalf("character not restored: %+v", p2.Player)
	}
	run(g2, 20)
	infos := ListSaves(filepath.Dir(path))
	if len(infos) != 1 || infos[0].WorldName != g.WorldName {
		t.Fatalf("ListSaves: %+v", infos)
	}
}

func TestSkillsAndItems(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Учёный", "rogue")
	g.GiveXP(p, 700)
	if p.Player.Level < 4 || p.Player.SkillPoints == 0 {
		t.Fatalf("level up failed: %d", p.Player.Level)
	}
	g.Command(p, proto.Command{Kind: "learn", Key: "swift"})
	if p.Player.Skills["swift"] != 1 {
		t.Fatal("learn failed")
	}
	g.Command(p, proto.Command{Kind: "learn", Key: "evasion"})
	g.Command(p, proto.Command{Kind: "learn", Key: "smoke_bomb"})
	if p.Player.Skills["smoke_bomb"] != 0 {
		t.Fatal("smoke bomb requires level 6")
	}
	g.Command(p, proto.Command{Kind: "learn", Key: "toughness"})
	if p.Player.Skills["toughness"] != 0 {
		t.Fatal("a warrior skill was learned without starting the class")
	}
	before := p.stats.MoveMs
	g.Command(p, proto.Command{Kind: "learn", Key: "swift"})
	if p.stats.MoveMs >= before {
		t.Fatal("move speed skill had no effect")
	}
	inv := len(p.Player.Inventory)
	g.Command(p, proto.Command{Kind: "potion"})
	if len(p.Player.Inventory) == inv && p.Player.Inventory[0].Qty == 2 {
		t.Fatal("potion not consumed")
	}
	g.Command(p, proto.Command{Kind: "alloc_attr", Key: "dex"})
	if p.Player.Attrs["dex"] != 10 {
		t.Fatalf("attr alloc failed: %v", p.Player.Attrs)
	}
}

func TestNPCDialogueWithClaude(t *testing.T) {
	g := setup(t)
	api := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		text, _ := json.Marshal(llm.NPCReply{Say: "Волки совсем обнаглели. Помоги нам!", Action: "offer_quest", QuestMonster: "wolf", QuestCount: 3})
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]any{
			"id": "msg_1", "type": "message", "role": "assistant", "model": "claude-opus-5-5",
			"content": []any{map[string]any{"type": "text", "text": string(text)}}, "stop_reason": "end_turn",
			"usage": map[string]any{"input_tokens": 1, "output_tokens": 1},
		})
	}))
	defer api.Close()
	g.Brain = llm.New("sk-test", "", option.WithBaseURL(api.URL))

	p, _, _ := g.Join("Герой", "warrior")
	var elder *Entity
	for _, e := range g.Entities {
		if e.NPC != nil && e.NPC.Role == "elder" {
			elder = e
			break
		}
	}
	if elder == nil {
		t.Fatal("no elder in the world")
	}
	g.moveEntity(p, findFree(g.Levels[p.Level], elder.Pos))
	g.openDialogue(p, elder)
	if d := g.TakeOutbox(p).Dialogue; d == nil || !d.AI {
		t.Fatalf("dialogue should open in AI mode: %+v", d)
	}
	g.Command(p, proto.Command{Kind: "talk", Text: "Есть работа?"})
	deadline := time.Now().Add(5 * time.Second)
	var last *proto.Dialogue
	for len(p.Player.Quests) == 0 && time.Now().Before(deadline) {
		g.Tick()
		if ob := g.TakeOutbox(p); ob != nil && ob.Dialogue != nil {
			last = ob.Dialogue
		}
		time.Sleep(20 * time.Millisecond)
	}
	if len(p.Player.Quests) != 1 || p.Player.Quests[0].Monster != "wolf" || p.Player.Quests[0].Need != 3 {
		t.Fatalf("quest from Claude not created: %+v", p.Player.Quests)
	}
	if ob := g.TakeOutbox(p); ob != nil && ob.Dialogue != nil {
		last = ob.Dialogue
	}
	if last == nil || !strings.Contains(last.Text, "Помоги нам") {
		t.Fatalf("NPC reply not delivered: %+v", last)
	}
	if mem := elder.NPC.Memory["Герой"]; len(mem) != 2 {
		t.Fatalf("NPC should remember the exchange, got %+v", mem)
	}
}

// TestSoak plays random inputs for many ticks with several players to catch
// panics and broken invariants in long sessions (day/night, spawner, dungeons).
func TestSoak(t *testing.T) {
	if testing.Short() {
		t.Skip()
	}
	g := setup(t)
	var players []*Entity
	for i, cls := range []string{"warrior", "ranger", "mage"} {
		p, _, err := g.Join(string(rune('А'+i))+"герой", cls)
		if err != nil {
			t.Fatal(err)
		}
		g.GiveXP(p, 5000)
		for _, s := range content.Skills() {
			for k := 0; k < 3; k++ {
				g.Command(p, proto.Command{Kind: "learn", Key: s.Key})
			}
		}
		players = append(players, p)
	}
	r := g.Rand()
	for tick := 0; tick < 12000; tick++ {
		for i, p := range players {
			switch r.IntN(12) {
			case 0, 1, 2, 3, 4, 5:
				g.SetInput(p, proto.Input{Move: uint8(1 + r.IntN(4))})
			case 6:
				g.SetInput(p, proto.Input{Ability: int8(1 + r.IntN(6))})
			case 7:
				g.SetInput(p, proto.Input{Attack: true})
			case 8:
				g.SetInput(p, proto.Input{Interact: true})
			case 9:
				if len(p.Player.Inventory) > 0 {
					g.Command(p, proto.Command{Kind: "use", Index: r.IntN(len(p.Player.Inventory))})
				}
			}
			p.MP = p.MaxMP
			if tick%1500 == 700+i*100 && !p.Dead {
				// hop between dungeon floors and the surface
				idx := r.IntN(len(g.Entrances))
				depth := 1 + r.IntN(g.Entrances[idx].MaxDepth)
				lvl := g.Level(DungeonLevelID(idx, depth))
				g.changeLevel(p, lvl, lvl.Up)
			}
		}
		g.Tick()
		for _, p := range g.Online {
			g.Snapshot(p)
			g.Sheet(p)
			g.TakeOutbox(p)
		}
		g.EndFrame()
		if tick%3000 == 0 {
			// occupancy grid must agree with entity positions
			for _, e := range g.Entities {
				if e.Blocks() && !e.Dead {
					if got := g.Levels[e.Level].Occupant(e.Pos.X, e.Pos.Y); got != e.ID {
						t.Fatalf("tick %d: occupancy mismatch for %s at %v: %d", tick, e.Name, e.Pos, got)
					}
				}
			}
		}
	}
	if err := g.Save(filepath.Join(t.TempDir(), "soak.sav")); err != nil {
		t.Fatal(err)
	}
	t.Logf("levels generated: %d, entities: %d, time of day %.2f", len(g.Levels), len(g.Entities), g.TimeOfDay())
}
