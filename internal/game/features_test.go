package game

import (
	"slices"
	"strings"
	"testing"

	"ratas/internal/content"
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

// Every unique character is placed in the world and every quest reward exists.
func TestNewUniquesPlaced(t *testing.T) {
	g := setup(t)
	placed := map[string]bool{}
	for _, e := range g.Entities {
		if e.NPC != nil && e.NPC.Unique != "" {
			placed[e.NPC.Unique] = true
		}
	}
	for _, k := range []string{"olkha", "koschei", "torgrim", "yaroslava", "evstafiy", "ignatiy"} {
		if !placed[k] {
			t.Errorf("the unique %s who teaches a secret class or subclass is not in the world", k)
		}
	}
	if len(placed) < 16 {
		t.Errorf("only %d uniques placed", len(placed))
	}
	for _, u := range content.Uniques() {
		kind, key, _ := strings.Cut(u.Reward, ":")
		ok := false
		switch kind {
		case "item":
			ok = content.Item(key) != nil
		case "class":
			ok = content.Class(key) != nil && content.Class(key).Secret
		case "subclass":
			ok = content.Subclass(key) != nil && content.Subclass(key).Secret
		case "skill":
			ok = content.Skill(key) != nil
		}
		if !ok {
			t.Errorf("unique %s: bad reward %q", u.Key, u.Reward)
		}
		switch u.Quest {
		case "slay", "boss":
			if content.Monster(u.Target) == nil {
				t.Errorf("unique %s: unknown target %q", u.Key, u.Target)
			}
		case "relics":
			if d := content.Item(u.Target); d == nil || d.Kind != "quest" {
				t.Errorf("unique %s: bad relic %q", u.Key, u.Target)
			}
		}
	}
}

// The new secret classes and subclasses can be opened and all their
// abilities work.
func TestNewSecretClasses(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "warrior")
	wild(t, g, p)
	pl := p.Player
	g.GiveXP(p, 20000)
	for _, r := range []string{"class:druid", "class:necromancer", "subclass:berserker"} {
		g.unlock(p, r)
	}
	for _, c := range []string{"druid", "necromancer"} {
		if why := CanStartClass(pl, c); why != "" {
			t.Fatalf("%s: %s", c, why)
		}
		g.startClass(p, c)
		if !pl.HasClass(c) {
			t.Fatalf("class %s not started", c)
		}
	}
	if !slices.Contains(pl.Abilities, "thorn_whip") || !slices.Contains(pl.Abilities, "bone_spear") {
		t.Fatalf("starting abilities: %v", pl.Abilities)
	}
	g.Admin(p, "/god")
	l := g.Levels[p.Level]
	for _, sk := range content.Skills() {
		b := content.Branch(sk.Branch)
		switch b.Key {
		case "druid", "grove_keeper", "shapeshifter", "necromancer", "bonelord", "plaguebringer",
			"berserker", "monster_hunter", "chronomancer", "inquisitor":
		default:
			continue
		}
		if sk.Grants == "" {
			continue
		}
		a := content.Ability(sk.Grants)
		// a fresh enemy next to the hero for every ability
		for _, o := range g.onLevel(p.Level) {
			if o.Monster != nil {
				g.Remove(o)
			}
		}
		w := spawnAt(g, "wolf", l, p.Pos.Add(world.Pos{X: 1}))
		w.Monster.Target = 0
		g.indexLevels()
		p.Facing = world.DirRight
		p.MP, p.Cooldowns = 1000, map[string]float64{}
		g.unlockAbility(p, sk.Grants)
		if !g.useAbility(p, sk.Grants, nil) {
			t.Fatalf("%s: the cast waits", sk.Grants)
		}
		if _, ok := p.Cooldowns[sk.Grants]; !ok && a.CooldownMs > 0 {
			t.Errorf("%s (%s) was not cast", a.Name, a.Kind)
			continue
		}
		switch a.Kind {
		case "summon":
			found := false
			for _, o := range g.Entities {
				if o.Owner == p.ID && o.Monster != nil && o.Monster.Def == a.Summon {
					found = true
				}
			}
			if !found {
				t.Errorf("%s: nothing summoned", a.Name)
			}
		case "buff":
			found := false
			for _, b := range p.Buffs {
				found = found || b.Def.Key == a.Buff.Key
			}
			if !found {
				t.Errorf("%s: no buff", a.Name)
			}
		case "nova", "cleave", "strike", "chain":
			if a.Damage[1] > 0 && w.HP >= w.MaxHP {
				t.Errorf("%s: the wolf next to the hero was not hurt", a.Name)
			}
		}
		run(g, 20)
	}
}

func TestCityLife(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "warrior")
	var city *VillageInfo
	for i := range g.Villages {
		if g.Villages[i].City {
			city = &g.Villages[i]
		}
	}
	if city == nil {
		t.Fatal("no city in the world")
	}
	find := func(role string) *Entity {
		for _, e := range g.Entities {
			if e.NPC != nil && e.NPC.Role == role && e.NPC.Village == city.Name {
				return e
			}
		}
		t.Fatalf("no %s in %s", role, city.Name)
		return nil
	}
	// the armorer lays out a daily stock of rare things
	arm := find("armorer")
	g.MoveForTest(p, arm)
	g.OpenDialogueForTest(p, arm)
	list := g.tradeList(arm)
	role := content.NPCRole("armorer")
	if len(list) != len(role.Goods)+role.Stock {
		t.Fatalf("trade list %d, want %d goods + %d stock", len(list), len(role.Goods), role.Stock)
	}
	last := list[len(list)-1]
	if last.Item.Rarity < int8(Uncommon) || last.Price <= 0 {
		t.Fatalf("stock item %+v", last)
	}
	p.Player.Gold = 100000
	g.buy(p, last.Item.Key, len(list)-1)
	if len(arm.NPC.Stock) != role.Stock-1 {
		t.Fatal("the bought stock item is still for sale")
	}
	bought := p.Player.Inventory[len(p.Player.Inventory)-1]
	if bought.Key != last.Item.Key || bought.ItemRarity() != Rarity(last.Item.Rarity) {
		t.Fatalf("bought %+v, wanted %+v", bought, last.Item)
	}
	// a new day, a new stock
	g.Now += DayMs
	g.tradeList(arm)
	if len(arm.NPC.Stock) != role.Stock {
		t.Fatal("the stock was not renewed")
	}

	// services
	smith := find("smith")
	g.MoveForTest(p, smith)
	g.OpenDialogueForTest(p, smith)
	before := p.Player.Equip[SlotMain].ItemRarity()
	idx := slices.IndexFunc(g.dialogueOptions(p, smith), func(o dialogueOption) bool { return o.action == "upgrade" })
	if idx < 0 {
		t.Fatal("the city smith offers no upgrade")
	}
	g.talkOption(p, idx)
	if p.Player.Equip[SlotMain].ItemRarity() != before+1 {
		t.Fatalf("upgrade: %v -> %v", before, p.Player.Equip[SlotMain].ItemRarity())
	}
	inn := find("innkeeper")
	g.MoveForTest(p, inn)
	g.OpenDialogueForTest(p, inn)
	p.HP = 1
	idx = slices.IndexFunc(g.dialogueOptions(p, inn), func(o dialogueOption) bool { return o.action == "rest" })
	g.talkOption(p, idx)
	if p.HP != p.MaxHP || !slices.ContainsFunc(p.Buffs, func(b Buff) bool { return b.Def.Key == "rested" }) {
		t.Fatal("no rest at the inn")
	}
	// village smiths have no services
	for _, e := range g.Entities {
		if e.NPC != nil && e.NPC.Role == "smith" && !g.isCity(e.NPC.Village) && len(g.serviceOptions(p, e)) > 0 {
			t.Fatal("a village smith offers city services")
		}
	}

	// townsfolk walk to the tavern at night
	var walker *Entity
	for _, e := range g.Entities {
		if e.NPC != nil && e.NPC.Role == "villager" && e.NPC.Village == city.Name && e.NPC.Night != (world.Pos{}) {
			walker = e
			break
		}
	}
	if walker == nil {
		t.Fatal("no townsfolk with a night spot")
	}
	g.PlaceForTest(p, "overworld", world.Pos{X: 5, Y: 5})
	g.Admin(p, "/time night")
	start := walker.Pos.Dist(walker.NPC.Night)
	run(g, 1200)
	if d := walker.Pos.Dist(walker.NPC.Night); d > max(10, start/2) {
		t.Fatalf("at night the citizen is still %d tiles from the tavern (was %d)", d, start)
	}
}
