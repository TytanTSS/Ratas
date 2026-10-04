package game

import (
	"math"
	"math/rand/v2"
	"testing"

	"ratas/internal/content"
	"ratas/internal/world"
)

func spawnAt(g *Game, key string, l *world.Level, near world.Pos) *Entity {
	m := g.newMonster(content.Monster(key), l.ID, findFree(l, near), 1)
	m.HP, m.MaxHP = 1e6, 1e6
	return m
}

func TestResistances(t *testing.T) {
	g := setup(t)
	ow := g.Levels["overworld"]
	sk := spawnAt(g, "skeleton", ow, g.Start)
	blunt := g.damage(nil, sk, single("blunt", 10))
	pierce := g.damage(nil, sk, single("pierce", 10))
	holy := g.damage(nil, sk, single("holy", 10))
	poison := g.damage(nil, sk, single("poison", 10))
	armor := armorFactor(sk.stats.Armor)
	if math.Abs(blunt-10*armor*1.5) > 1e-6 || math.Abs(pierce-10*armor*0.5) > 1e-6 {
		t.Fatalf("physical resistances not applied: blunt %.2f pierce %.2f (armor %.2f)", blunt, pierce, armor)
	}
	if holy != 15 {
		t.Fatalf("armor must not reduce holy damage and vulnerability must add 50%%: %.2f", holy)
	}
	if poison != 0 {
		t.Fatalf("skeletons are immune to poison, took %.2f", poison)
	}
	// a two-part hit: only the poison part is ignored
	mixed := Damage{Parts: []DmgPart{{"slash", 10}, {"poison", 10}}}
	if got := g.damage(nil, sk, mixed); math.Abs(got-10*armor) > 1e-6 {
		t.Fatalf("mixed hit: %.2f", got)
	}
}

func TestCursesLowerResistances(t *testing.T) {
	g := setup(t)
	ow := g.Levels["overworld"]
	imp := spawnAt(g, "fire_imp", ow, g.Start)
	if r := imp.stats.Resist("fire"); r != 100 {
		t.Fatalf("imp fire resistance %v", r)
	}
	g.applyBuff(imp, content.Ability("elemental_mark").OnHit, 0)
	if r := imp.stats.Resist("fire"); r != 70 {
		t.Fatalf("mark should lower fire resistance to 70, got %v", r)
	}
	g.applyBuff(imp, content.Ability("curse_weakness").OnHit, 0)
	if r := imp.stats.Resist("cold"); r != -100 {
		t.Fatalf("cold resistance must be clamped at -100, got %v", r)
	}
	if got := g.damage(nil, imp, single("fire", 10)); math.Abs(got-5) > 1e-6 {
		t.Fatalf("fire through two curses: %.2f", got)
	}
	// player resistances are capped at 75
	p, _, _ := g.Join("Тест", "warrior")
	g.applyBuff(p, &content.BuffDef{Key: "x", DurationMs: 10000, Stats: map[string]float64{"res_fire": 300}}, p.ID)
	if r := p.stats.Resist("fire"); r != 75 {
		t.Fatalf("player resistance cap: %v", r)
	}
}

func TestBrokenArmor(t *testing.T) {
	g := setup(t)
	m := spawnAt(g, "orc", g.Levels["overworld"], g.Start)
	before := g.damage(nil, m, single("slash", 10))
	g.applyBuff(m, content.Ability("sunder").OnHit, 0)
	after := g.damage(nil, m, single("slash", 10))
	if after <= before*1.4 {
		t.Fatalf("sundered armor should hurt much more: %.2f -> %.2f", before, after)
	}
}

func TestChainLightning(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "shaman")
	if !g.TeleportForTest(p, "d0-1") {
		t.Fatal("no dungeon")
	}
	l := g.Levels[p.Level]
	for _, e := range g.Entities {
		if e.Monster != nil && e.Level == l.ID {
			g.Remove(e)
		}
	}
	var wolves []*Entity
	at := p.Pos
	for i := 0; i < 3; i++ {
		w := spawnAt(g, "wolf", l, at)
		wolves = append(wolves, w)
		at = w.Pos
	}
	p.MP = 100
	p.Player.Abilities = append(p.Player.Abilities, "chain_lightning")
	g.useAbility(p, "chain_lightning", nil)
	for i, w := range wolves {
		if w.HP >= w.MaxHP {
			t.Fatalf("wolf %d (%v, player %v) was not hit by the chain", i, w.Pos, p.Pos)
		}
		if w.stats.Resist("lightning") != -25 {
			t.Fatalf("wolf %d not shocked", i)
		}
	}
}

func TestThornsAndLeech(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "warrior")
	m := spawnAt(g, "wolf", g.Levels["overworld"], p.Pos)
	g.applyBuff(p, &content.BuffDef{Key: "t", DurationMs: 60000, Stats: map[string]float64{"thorns": 7, "dodge": -100}}, p.ID)
	hp := m.HP
	g.meleeAttack(m, p)
	if m.HP >= hp {
		t.Fatal("thorns did not hurt the attacker")
	}
	p.HP = 10
	g.damage(p, m, Damage{Parts: []DmgPart{{"slash", 20}}, Leech: 50})
	if p.HP <= 10 {
		t.Fatal("life leech did not heal")
	}
}

func TestStunnedMonsterDoesNotAct(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "warrior")
	m := spawnAt(g, "orc", g.Levels["overworld"], p.Pos)
	g.applyBuff(m, &content.BuffDef{Key: "stun", DurationMs: 5000, Stun: true}, p.ID)
	hp := p.HP
	run(g, 40)
	if p.HP < hp {
		t.Fatal("a stunned monster attacked")
	}
}

func TestConsumableEffects(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "warrior")
	pl := p.Player
	use := func(key string) {
		t.Helper()
		g.addItem(p, ItemStack{Key: key, Qty: 1})
		for i, st := range pl.Inventory {
			if st.Key == key {
				g.useItem(p, i)
				return
			}
		}
		t.Fatalf("no %s in inventory", key)
	}

	// antidote removes poison applied by others and protects
	g.applyBuff(p, content.Monster("spider").OnHit, 999)
	use("antidote")
	for _, b := range p.Buffs {
		if b.Def.Key == "venom" {
			t.Fatal("antidote did not cleanse")
		}
	}
	if p.stats.Resist("poison") < 50 {
		t.Fatal("antidote buff missing")
	}

	// oblivion returns every point
	g.GiveXP(p, 2000)
	pl.Skills["toughness"], pl.SkillPoints = 2, pl.SkillPoints-2
	pl.Attrs["str"] += 3
	pl.AttrPoints -= 3
	skillPts, attrPts := pl.SkillPoints+2, pl.AttrPoints+3
	use("tome_oblivion")
	if pl.SkillPoints != skillPts || pl.AttrPoints != attrPts || len(pl.Skills) != 0 || pl.Attrs["str"] != content.Class("warrior").Attrs["str"] {
		t.Fatalf("respec: skills %d/%d attrs %d/%d", pl.SkillPoints, skillPts, pl.AttrPoints, attrPts)
	}

	// return scroll brings the player home from a dungeon
	g.TeleportForTest(p, "d0-1")
	use("scroll_return")
	if p.Level != "overworld" || p.Pos.Dist(g.Start) > 5 {
		t.Fatalf("return scroll: %s %v", p.Level, p.Pos)
	}

	// fire oil adds fire damage to melee hits
	use("fire_oil")
	d := g.meleeDamage(p)
	found := false
	for _, part := range d.Parts {
		found = found || (part.Type == "fire" && part.Amount > 0)
	}
	if !found {
		t.Fatalf("fire oil: %+v", d)
	}
}

func TestShrine(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "warrior")
	l := g.Levels[p.Level]
	at := p.Pos.Add(world.Pos{X: 1})
	g.SetTile(l, at.X, at.Y, content.TileID("shrine"))
	p.Facing = world.DirRight
	p.HP = 1
	g.interact(p)
	if len(p.Buffs) == 0 || p.HP != p.MaxHP {
		t.Fatal("no blessing")
	}
	if l.Def(at.X, at.Y).Key != "shrine_used" {
		t.Fatal("shrine not used up")
	}
}

func TestRegionsAndLandmarks(t *testing.T) {
	g := setup(t)
	if len(g.Regions) < 3 || g.RegionName(g.Start) == "" {
		t.Fatalf("regions: %d, start region %q", len(g.Regions), g.RegionName(g.Start))
	}
	kinds := map[string]bool{}
	for _, r := range g.Regions {
		kinds[r.Kind] = true
	}
	if len(g.Landmarks) < 6 {
		t.Fatalf("only %d landmarks", len(g.Landmarks))
	}
	p, _, _ := g.Join("Тест", "warrior")
	lm := g.Landmarks[0]
	g.moveEntity(p, findFree(g.Levels["overworld"], lm.Pos))
	xp := p.Player.XP
	g.checkSurroundings(p)
	if len(p.Player.Found) != 1 || p.Player.XP <= xp {
		t.Fatalf("landmark %s not discovered", lm.Name)
	}
	t.Logf("%d regions (%v), %d landmarks", len(g.Regions), kinds, len(g.Landmarks))
}

// Every theme of dungeon must have monsters for its floors and a boss.
func TestDungeonThemesPopulated(t *testing.T) {
	setup(t)
	for _, theme := range []string{"cave", "crypt", "ice", "volcano", "temple", "fortress"} {
		boss := false
		for _, m := range content.Monsters() {
			for _, th := range m.Themes {
				boss = boss || (m.Boss && th == theme)
			}
		}
		if !boss {
			t.Errorf("theme %s has no boss", theme)
		}
		for depth := 1; depth <= 5; depth++ {
			if pickMonster(newTestRand(), theme, depth, false, true) == nil {
				t.Errorf("theme %s has no monsters at depth %d", theme, depth)
			}
		}
	}
	for _, biome := range []string{"plains", "forest", "swamp", "hills", "desert", "tundra", "ash", "cursed"} {
		if pickMonster(newTestRand(), biome, 0, true, false) == nil {
			t.Errorf("biome %s has no monsters", biome)
		}
	}
}

func newTestRand() *rand.Rand { return rand.New(rand.NewPCG(1, 2)) }
