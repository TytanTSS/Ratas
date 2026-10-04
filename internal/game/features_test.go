package game

import (
	"slices"
	"strings"
	"testing"

	"ratas/internal/proto"
	"ratas/internal/world"
)

// A mouse-aimed ability goes to the enemy under the cursor, not the nearest.
func TestMouseAim(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "mage")
	wild(t, g, p)
	l := g.Levels[p.Level]
	near := spawnAt(g, "wolf", l, p.Pos.Add(world.Pos{X: 2}))
	far := spawnAt(g, "wolf", l, p.Pos.Add(world.Pos{X: -4}))
	near.Monster.Target, far.Monster.Target = 0, 0
	p.MP = 100
	p.Player.Hotbar[0] = "magic_missile"
	g.SetInput(p, proto.Input{Ability: 1, Aim: true, AimX: int32(far.Pos.X), AimY: int32(far.Pos.Y)})
	run(g, 30)
	if far.HP >= far.MaxHP {
		t.Fatalf("the aimed wolf was not hit (player %v, far %v)", p.Pos, far.Pos)
	}
	if near.HP < near.MaxHP {
		t.Fatal("the nearest wolf was hit instead of the aimed one")
	}

	// aimed at empty ground, a projectile flies there anyway
	p.Cooldowns = map[string]float64{}
	g.Remove(near)
	g.Remove(far)
	p.MP = 100
	g.SetInput(p, proto.Input{Ability: 1, Aim: true, AimX: int32(p.Pos.X), AimY: int32(p.Pos.Y - 3)})
	g.Tick()
	flying := false
	for _, e := range g.Entities {
		if e.Proj != nil && e.Proj.Owner == p.ID && e.Proj.Path[len(e.Proj.Path)-1].X == p.Pos.X {
			flying = true
		}
	}
	if !flying || p.Facing != world.DirUp {
		t.Fatalf("no projectile to empty ground: facing %v", p.Facing)
	}
}

// The knight's long reach hits an enemy two tiles away and diagonally.
func TestKnightReach(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "warrior")
	wild(t, g, p)
	l := g.Levels[p.Level]
	far := spawnAt(g, "wolf", l, p.Pos.Add(world.Pos{X: 2}))
	if far.Pos != p.Pos.Add(world.Pos{X: 2}) {
		t.Skip("no free tile")
	}
	far.Monster.Target = 0
	g.indexLevels()
	g.attackFacing(p)
	if far.HP < far.MaxHP {
		t.Fatal("hit two tiles away without the skill")
	}
	p.Player.Skills["long_reach"] = 1
	p.Recalc()
	if p.stats.Reach != 1 {
		t.Fatalf("reach %d", p.stats.Reach)
	}
	for i := 0; i < 10 && far.HP >= far.MaxHP; i++ { // attacks can miss
		g.attackFacing(p)
	}
	if far.HP >= far.MaxHP {
		t.Fatal("the long reach did not hit two tiles away")
	}
	if !g.inReach(p, &Entity{Level: p.Level, Pos: p.Pos.Add(world.Pos{X: 1, Y: 1})}) {
		t.Fatal("no diagonal reach")
	}
	if g.inReach(p, &Entity{Level: p.Level, Pos: p.Pos.Add(world.Pos{X: 3})}) {
		t.Fatal("reach too long")
	}
	// strike abilities reach too
	before := far.HP
	for i := 0; i < 10 && far.HP >= before; i++ {
		p.MP, p.Cooldowns = 100, map[string]float64{}
		g.useAbility(p, "power_strike", nil)
	}
	if far.HP >= before {
		t.Fatal("power strike did not reach")
	}
	// a bow does not get the melee reach
	p.Player.Equip[SlotMain] = ItemStack{Key: "hunting_bow", Qty: 1}
	p.Player.Equip[SlotOff] = ItemStack{}
	delete(p.Player.Equip, SlotOff)
	p.Recalc()
	if p.stats.Reach != 0 {
		t.Fatal("reach with a bow")
	}
}

func TestAdminCommands(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "warrior")
	wild(t, g, p)

	g.Admin(p, "/god")
	hp := p.HP
	g.damage(nil, p, single("fire", 1000))
	if p.HP != hp || !p.Alive() {
		t.Fatal("god mode took damage")
	}
	g.Admin(p, "/level 10")
	if p.Player.Level != 10 {
		t.Fatalf("level %d", p.Player.Level)
	}
	g.Admin(p, "/give long sword 2 legendary")
	n := 0
	for _, st := range p.Player.Inventory {
		if st.Key == "long_sword" {
			n++
			if st.ItemRarity() != Legendary || len(st.Bonus) != 4 {
				t.Fatalf("not legendary: %+v", st)
			}
		}
	}
	if n != 2 {
		t.Fatalf("gave %d swords", n)
	}
	g.Admin(p, "/give зелье здоровья 5")
	g.Admin(p, "/spawn wolf 3 4")
	wolves := 0
	for _, o := range g.onLevel(p.Level) {
		if o.Monster != nil && o.Monster.Def == "wolf" && o.Pos.Dist(p.Pos) < 6 {
			wolves++
			if o.Monster.Lvl != 4 {
				t.Fatalf("wolf level %d", o.Monster.Lvl)
			}
		}
	}
	if wolves < 3 {
		t.Fatalf("spawned %d wolves", wolves)
	}
	g.Admin(p, "/kill")
	for _, o := range g.onLevel(p.Level) {
		if o.Monster != nil && o.Alive() && o.Monster.Def == "wolf" && o.Pos.Dist(p.Pos) < 6 {
			t.Fatal("a wolf survived /kill")
		}
	}
	want := findFree(g.Levels["overworld"], world.Pos{X: 10, Y: 10})
	g.Admin(p, "/tp 10 10")
	if p.Pos != want {
		t.Fatalf("tp to %v", p.Pos)
	}
	g.Admin(p, "/tp d0-1")
	if p.Level != "d0-1" {
		t.Fatalf("tp to level: %s", p.Level)
	}
	g.Admin(p, "/tp overworld")
	g.Admin(p, "/unique aurelius")
	found := false
	for _, o := range g.onLevel(p.Level) {
		if o.NPC != nil && o.NPC.Unique == "aurelius" && o.Pos.Dist(p.Pos) < 6 {
			found = true
		}
	}
	if !found {
		t.Fatal("/unique did not bring Aurelius")
	}
	g.Admin(p, "/unlock all")
	if !slices.Contains(p.Player.Unlocks, "class:shaman") {
		t.Fatalf("unlocks: %v", p.Player.Unlocks)
	}
	g.Admin(p, "/time night")
	if g.Daylight() > 0.2 {
		t.Fatalf("daylight %v at night", g.Daylight())
	}
	g.Admin(p, "/nocd")
	p.MP = 0
	if !g.useAbility(p, "power_strike", nil) {
		t.Fatal("ability failed")
	}
	g.Admin(p, "/nonsense")
	g.Admin(p, "/help")
}

func TestRarity(t *testing.T) {
	g := setup(t)
	counts := map[Rarity]int{}
	for i := 0; i < 3000; i++ {
		st, ok := g.randomItem(6, 40)
		if !ok {
			t.Fatal("no item")
		}
		d := st.Def()
		if slotFor(d) == "" || d.Unique || d.Rarity != "" {
			continue
		}
		r := st.ItemRarity()
		counts[r]++
		if len(st.Bonus) != int(r) {
			t.Fatalf("%s: %d bonuses for rarity %d", st.Key, len(st.Bonus), r)
		}
	}
	for r := Common; r <= Legendary; r++ {
		if counts[r] == 0 {
			t.Fatalf("no %s items in 3000 finds: %v", RarityName(int(r)), counts)
		}
	}
	if !(counts[Common] > counts[Uncommon] && counts[Uncommon] > counts[Rare] && counts[Rare] > counts[Epic] && counts[Epic] > counts[Legendary]) {
		t.Fatalf("rarities are not getting rarer: %v", counts)
	}

	// a legendary copy is stronger and dearer than a common one
	common := ItemStack{Key: "long_sword", Qty: 1}
	leg := g.rollRarity(common, Legendary, 5)
	if leg.Value() <= common.Value()*4 {
		t.Fatalf("legendary value %d vs %d", leg.Value(), common.Value())
	}
	p, _, _ := g.Join("Тест", "warrior")
	p.Player.Equip[SlotMain] = common
	p.Recalc()
	base := p.stats.WeaponDmg
	p.Player.Equip[SlotMain] = ItemStack{Key: "long_sword", Qty: 1, Rarity: int(Legendary)}
	p.Recalc()
	if p.stats.WeaponDmg[1] <= base[1] {
		t.Fatalf("legendary damage %v vs %v", p.stats.WeaponDmg, base)
	}
	if v := ItemViewOf(leg); v.Rarity != int8(Legendary) || !strings.HasPrefix(v.Desc, "Легендарный") {
		t.Fatalf("view: %+v", v)
	}
	// artifacts are legendary by themselves, named epics epic
	if (ItemStack{Key: "sunfire_aegis"}).ItemRarity() != Legendary || (ItemStack{Key: "wanderer_boots"}).ItemRarity() != Epic {
		t.Fatal("artifact rarity")
	}
	if RarityByName("эпич") != int(Epic) || RarityByName("legendary") != int(Legendary) || RarityByName("необычный") != int(Uncommon) || RarityByName("обычный") != int(Common) {
		t.Fatal("rarity names")
	}
}

func TestBossDropsArtifacts(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "warrior")
	l := g.Levels[p.Level]
	got := 0
	for i := 0; i < 40; i++ {
		b := spawnAt(g, "lich", l, p.Pos)
		b.HP = 0
		g.kill(b, p)
	}
	for _, e := range g.Entities {
		if e.Item != nil && e.Item.Key == "morwen_phylactery" {
			got++
		}
	}
	if got < 5 || got > 25 {
		t.Fatalf("the lich dropped its phylactery %d times of 40", got)
	}
}
