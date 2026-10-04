package game

import (
	"slices"
	"strings"
	"testing"

	"ratas/internal/content"
	"ratas/internal/proto"
	"ratas/internal/world"
)

func give(g *Game, p *Entity, key string) int {
	g.addItem(p, ItemStack{Key: key, Qty: 1})
	for i, st := range p.Player.Inventory {
		if st.Key == key {
			return i
		}
	}
	return -1
}

// wild puts a hero on empty land outside villages.
func wild(t *testing.T, g *Game, p *Entity) {
	t.Helper()
	l := g.Levels["overworld"]
	for y := 5; y < l.H-5; y++ {
		for x := 5; x < l.W-5; x++ {
			q := world.Pos{X: x, Y: y}
			if !g.inVillage(q, 10) && l.Free(x, y) && l.Def(x, y).Damage == 0 && l.Def(x, y).Interact == "" {
				free := 0
				for _, d := range world.AllDirs {
					if n := q.Add(d.Delta()); l.Free(n.X, n.Y) {
						free++
					}
				}
				if free == 4 && q.Dist(p.Pos) > 20 {
					g.PlaceForTest(p, "overworld", q)
					for _, e := range g.onLevel("overworld") {
						if e.Monster != nil && e.Pos.Dist(q) < 15 && e != p {
							g.Remove(e)
						}
					}
					g.indexLevels()
					return
				}
			}
		}
	}
	t.Fatal("no wild land")
}

func TestEquipmentSlots(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "warrior")
	eq := p.Player.Equip
	if eq[SlotMain].Key != "short_sword" || eq[SlotOff].Key != "wooden_shield" || eq[SlotHead].Key != "leather_cap" || eq[SlotLegs].Key != "cloth_pants" {
		t.Fatalf("starting gear: %+v", eq)
	}
	if !p.stats.Gear.Shield || p.stats.Gear.Weapon != "sword" {
		t.Fatalf("gear: %+v", p.stats.Gear)
	}
	// a two-handed weapon takes both hands
	g.equip(p, give(g, p, "greatsword"), false)
	if eq[SlotOff].Key != "" || eq[SlotMain].Key != "greatsword" || !p.stats.Gear.TwoHand {
		t.Fatalf("two-handed: %+v", eq)
	}
	// a shield in the left hand puts the two-handed weapon away
	g.equip(p, give(g, p, "round_shield"), false)
	if eq[SlotMain].Key != "" || eq[SlotOff].Key != "round_shield" {
		t.Fatalf("shield after two-handed: %+v", eq)
	}
	// dual wielding
	g.equip(p, give(g, p, "dagger"), false)
	g.equip(p, give(g, p, "short_sword"), true)
	if !p.stats.Gear.Dual || eq[SlotOff].Key != "short_sword" {
		t.Fatalf("dual: %+v %+v", eq, p.stats.Gear)
	}
	// four rings
	for _, r := range []string{"ring_vigor", "ring_mind", "ring_fire", "ring_frost", "ring_storm"} {
		g.equip(p, give(g, p, r), false)
	}
	for _, s := range ringSlots {
		if eq[s].Key == "" {
			t.Fatalf("ring slot %s empty: %+v", s, eq)
		}
	}
	if p.stats.Resist("fire") < 25 {
		t.Fatal("ring stats not applied")
	}
	// old saves
	old := &PlayerState{Equip: map[string]ItemStack{"weapon": {Key: "dagger"}, "armor": {Key: "chain_mail"}, "trinket": {Key: "ring_vigor"}}}
	migrateEquip(old)
	if old.Equip[SlotMain].Key != "dagger" || old.Equip[SlotChest].Key != "chain_mail" || old.Equip[SlotRing1].Key != "ring_vigor" {
		t.Fatalf("migration: %+v", old.Equip)
	}
}

func TestSkillsNeedGear(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "warrior")
	pl := p.Player
	pl.Skills["shield_mastery"] = 5
	pl.Subclasses["warrior"] = "guardian"
	p.Recalc()
	withShield := p.stats.Block
	g.unequip(p, SlotOff)
	if withShield < 20 || p.stats.Block != 0 {
		t.Fatalf("shield mastery: %v with a shield, %v without", withShield, p.stats.Block)
	}
	g.unlockAbility(p, "shield_bash")
	p.MP = 100
	g.useAbility(p, "shield_bash", nil)
	if p.Cooldowns["shield_bash"] > g.Now {
		t.Fatal("shield bash used without a shield")
	}
}

func TestClassesAndSubclasses(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "mage")
	pl := p.Player
	if why := CanChooseSubclass(pl, "pyromancer"); !strings.Contains(why, "уровень класса") {
		t.Fatalf("subclass before class level 5: %q", why)
	}
	if why := CanStartClass(pl, "warrior"); why == "" {
		t.Fatal("second class at level 1")
	}
	g.GiveXP(p, 5000)
	for _, k := range []string{"wisdom", "wisdom", "meditation", "meditation", "wisdom"} {
		g.learnSkill(p, k)
	}
	if ClassLevel(pl, "mage") != 5 {
		t.Fatalf("class level %d", ClassLevel(pl, "mage"))
	}
	if why := CanLearn(pl, content.Skill("fireball")); !strings.Contains(why, "подкласс") {
		t.Fatalf("subclass skill without the subclass: %q", why)
	}
	if CanChooseSubclass(pl, "sealer") == "" {
		t.Fatal("secret subclass without unlock")
	}
	g.chooseSubclass(p, "pyromancer")
	if pl.Subclasses["mage"] != "pyromancer" || CanChooseSubclass(pl, "cryomancer") == "" {
		t.Fatal("one subclass per class")
	}
	g.learnSkill(p, "fireball")
	if !slices.Contains(pl.Abilities, "fireball") {
		t.Fatal("subclass skill not learned")
	}
	// multiclass
	g.startClass(p, "warrior")
	if !pl.HasClass("warrior") || !slices.Contains(pl.Abilities, "power_strike") {
		t.Fatal("second class not started")
	}
	if CanStartClass(pl, "shaman") == "" {
		t.Fatal("secret class without unlock")
	}
	g.unlock(p, "class:shaman")
	if why := CanStartClass(pl, "shaman"); why != "" {
		t.Fatalf("unlocked secret class: %q", why)
	}
	g.unlock(p, "skill:titan_blood")
	if pl.Skills["titan_blood"] != 1 {
		t.Fatal("secret skill")
	}
	// oblivion keeps secret skills
	g.respec(p)
	if pl.Skills["titan_blood"] != 1 || len(pl.Subclasses) != 0 || len(pl.Classes) != 1 {
		t.Fatalf("respec: %+v %v %v", pl.Skills, pl.Subclasses, pl.Classes)
	}
}

func TestPvPAndParty(t *testing.T) {
	g := setup(t)
	a, _, _ := g.Join("Аня", "warrior")
	b, _, _ := g.Join("Боря", "rogue")
	if g.hostile(a, b) {
		t.Fatal("players fight in the village")
	}
	wild(t, g, a)
	g.PlaceForTest(b, "overworld", a.Pos.Add(world.Pos{X: 1}))
	if !g.hostile(a, b) {
		t.Fatal("no PvP in the wild")
	}
	hp := b.HP
	b.stats.Dodge = 0
	g.damage(a, b, single("slash", 10))
	if b.HP >= hp {
		t.Fatal("a player could not hurt another")
	}
	// party
	g.Command(a, proto.Command{Kind: "party_invite", Key: "Боря"})
	g.Command(b, proto.Command{Kind: "party_accept", Key: "Аня"})
	if !g.sameParty(a, b) || g.hostile(a, b) {
		t.Fatal("party members are still hostile")
	}
	snap := g.Snapshot(a)
	if len(snap.Self.Party) != 1 || snap.Self.Party[0].Name != "Боря" {
		t.Fatalf("party view: %+v", snap.Self.Party)
	}
	// area damage spares the party
	m := spawnAt(g, "wolf", g.Levels["overworld"], a.Pos)
	g.indexLevels()
	hp = b.HP
	g.areaDamage(a, a.Faction, a.Level, a.Pos, 3, single("fire", 20), nil, '*', "#fff")
	if b.HP < hp || m.HP >= m.MaxHP {
		t.Fatal("area damage hit the party or missed the wolf")
	}
	g.Command(b, proto.Command{Kind: "party_leave"})
	if g.sameParty(a, b) || g.partyOf(a) != nil {
		t.Fatal("leaving a two-person party should dissolve it")
	}
	g.PvP = false
	if g.hostile(a, b) {
		t.Fatal("PvP off")
	}
}

func TestDeathAndResurrection(t *testing.T) {
	g := setup(t)
	a, _, _ := g.Join("Аня", "priest")
	b, _, _ := g.Join("Боря", "warrior")
	wild(t, g, b)
	g.PlaceForTest(a, "overworld", b.Pos.Add(world.Pos{X: 1}))
	pos := b.Pos
	g.kill(b, nil)
	if !b.Dead || b.Level != "overworld" || b.Pos != pos {
		t.Fatal("the body must stay where the hero fell")
	}
	if v := g.Snapshot(a); !slices.ContainsFunc(v.Entities, func(e proto.EntityView) bool { return e.ID == b.ID && e.Dead }) {
		t.Fatal("the fallen hero is not visible")
	}
	g.Command(b, proto.Command{Kind: "respawn"})
	if !b.Dead {
		t.Fatal("respawned before the button works")
	}
	a.Player.Subclasses["priest"] = "saint"
	g.unlockAbility(a, "resurrection")
	a.MP = 100
	g.useAbility(a, "resurrection", nil)
	if b.Dead || b.HP <= 0 || b.Pos.Dist(pos) > 1 {
		t.Fatalf("not resurrected: dead=%v hp=%v", b.Dead, b.HP)
	}
	// after a minute nobody can help
	g.kill(b, nil)
	g.Now += ReviveWindowMs + 100
	if g.revive(b, a) {
		t.Fatal("resurrected after the window")
	}
}

func TestTauntStealthSilence(t *testing.T) {
	g := setup(t)
	a, _, _ := g.Join("Аня", "warrior")
	b, _, _ := g.Join("Боря", "rogue")
	wild(t, g, a)
	g.PlaceForTest(b, "overworld", a.Pos.Add(world.Pos{X: 3}))
	g.Command(a, proto.Command{Kind: "party_invite", Key: "Боря"})
	g.Command(b, proto.Command{Kind: "party_accept", Key: "Аня"})
	l := g.Levels["overworld"]
	orc := spawnAt(g, "orc", l, b.Pos.Add(world.Pos{X: 1}))
	g.indexLevels()
	orc.Monster.Target = b.ID
	g.unlockAbility(a, "taunt")
	a.MP = 100
	g.useAbility(a, "taunt", nil)
	if orc.Monster.Target != a.ID || orc.stats.Taunter != a.ID {
		t.Fatal("taunt did not pull the orc")
	}
	// stealth: monsters cannot see you from afar
	g.applyBuff(b, content.Ability("vanish").Buff, b.ID)
	orc2 := spawnAt(g, "orc", l, b.Pos.Add(world.Pos{X: 4}))
	g.indexLevels()
	if g.canSee(orc2, b) || g.acquireTarget(orc2, content.Monster("orc")) == b {
		t.Fatal("a stealthed hero was seen")
	}
	// attacking breaks stealth, from behind it hurts more
	g.damage(b, orc2, single("pierce", 5))
	if b.stats.Stealthed {
		t.Fatal("stealth not broken by attacking")
	}
	// silence stops monster abilities
	shaman := spawnAt(g, "goblin_shaman", l, a.Pos.Add(world.Pos{Y: 3}))
	g.applyBuff(shaman, content.Ability("seal").OnHit, a.ID)
	if !shaman.stats.Silenced {
		t.Fatal("seal did not silence")
	}
	g.useAbility(shaman, "m_lightning", a)
	if shaman.Cooldowns["m_lightning"] > g.Now {
		t.Fatal("a sealed monster cast a spell")
	}
}

func TestSummonsAndDecoys(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "rogue")
	wild(t, g, p)
	l := g.Levels["overworld"]
	wolf := spawnAt(g, "wolf", l, p.Pos.Add(world.Pos{X: 3}))
	wolf.Monster.Target = p.ID
	g.indexLevels()
	g.unlockAbility(p, "decoy")
	p.MP = 100
	g.useAbility(p, "decoy", nil)
	g.indexLevels()
	decoy := g.Entities[wolf.Monster.Target]
	if decoy == nil || decoy.Owner != p.ID || !g.hostile(wolf, decoy) {
		t.Fatal("the wolf did not turn to the illusion")
	}
	// summoned allies fight monsters and vanish in time
	g.unlockAbility(p, "summon_imp")
	g.useAbility(p, "summon_imp", nil)
	g.indexLevels()
	var imp *Entity
	for _, e := range g.onLevel(p.Level) {
		if e.Owner == p.ID && e.Monster.Def == "imp_minion" {
			imp = e
		}
	}
	if imp == nil || imp.Faction != FPlayer || g.hostile(p, imp) || !g.hostile(imp, wolf) {
		t.Fatal("imp not on our side")
	}
	run(g, 100)
	if wolf.HP >= wolf.MaxHP && g.Entities[wolf.ID] != nil {
		t.Fatal("the imp did not attack the wolf")
	}
	g.Now += 60000
	run(g, 5)
	if g.Entities[imp.ID] != nil {
		t.Fatal("the summon did not expire")
	}
}

func TestArcherKeepsDistanceAndHealerHeals(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "warrior")
	wild(t, g, p)
	p.HP, p.MaxHP = 1e6, 1e6
	l := g.Levels["overworld"]
	archer := spawnAt(g, "bandit_archer", l, p.Pos.Add(world.Pos{X: 1}))
	archer.Monster.Target = p.ID
	g.indexLevels()
	for i := 0; i < 30; i++ {
		run(g, 1)
	}
	if d := archer.Pos.Manhattan(p.Pos); d < 2 {
		t.Fatalf("the archer stayed in melee (%d)", d)
	}
	shaman := spawnAt(g, "orc_shaman", l, p.Pos.Add(world.Pos{Y: 5}))
	orc := spawnAt(g, "orc", l, shaman.Pos.Add(world.Pos{X: 1}))
	orc.HP = orc.MaxHP * 0.3
	shaman.Monster.Target = p.ID
	g.indexLevels()
	hp := orc.HP
	for i := 0; i < 40; i++ {
		run(g, 1)
	}
	if orc.HP <= hp {
		t.Fatal("the shaman did not heal the wounded orc")
	}
}

func TestSquadsGuardsAndWanderers(t *testing.T) {
	g := setup(t)
	l := g.Levels["overworld"]
	sq := content.Squads()[0]
	ms := g.spawnSquad(g.rng, &sq, l, g.Start.Add(world.Pos{X: 60}), 2)
	roles := map[string]bool{}
	for _, m := range ms {
		roles[roleOf(g.defOf(m))] = true
		if m.Monster.Squad == 0 {
			t.Fatal("squad id missing")
		}
	}
	if !roles["frontline"] || !roles["ranged"] {
		t.Fatalf("squad roles: %v", roles)
	}
	guards, wanderers, uniques := 0, 0, 0
	for _, e := range g.Entities {
		switch {
		case e.NPC != nil && e.NPC.Role == "guard":
			if e.Faction != FPlayer || e.Monster == nil {
				t.Fatal("guards do not fight")
			}
			guards++
		case e.NPC != nil && e.NPC.Unique != "":
			uniques++
		case e.NPC != nil:
			if r := content.NPCRole(e.NPC.Role); r != nil && r.World {
				wanderers++
			}
		}
	}
	if guards == 0 || wanderers < 4 || uniques < 6 {
		t.Fatalf("guards %d wanderers %d uniques %d", guards, wanderers, uniques)
	}
}

func TestUniqueQuests(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "warrior")
	var npc *Entity
	for _, e := range g.Entities {
		if e.NPC != nil && e.NPC.Unique == "morta" {
			npc = e
		}
	}
	if npc == nil {
		t.Skip("Морта не появилась в этом мире")
	}
	u := content.Unique("morta")
	g.offerUniqueQuest(p, npc, u)
	q := g.uniqueQuest(p, "morta")
	c := g.Entities[g.Champions["morta"]]
	if q == nil || c == nil || c.Name != u.Champion || c.Monster.Champion != "morta" {
		t.Fatal("no champion")
	}
	if c.MaxHP < content.Monster("necromancer").HP*3 {
		t.Fatal("the champion is not stronger")
	}
	g.PlaceForTest(p, "overworld", c.Pos)
	g.indexLevels()
	g.kill(c, p)
	if !q.Done {
		t.Fatal("slay quest not done")
	}
	g.finishUniqueQuest(p, u)
	if !p.Player.Unlocked("subclass", "deathbringer") || g.uniqueQuest(p, "morta") != nil {
		t.Fatal("no secret subclass")
	}
	// relics
	pl := p.Player
	r := content.Unique("ulf")
	g.offerUniqueQuest(p, npc, r)
	for i := 0; i < r.Count; i++ {
		g.addItem(p, ItemStack{Key: r.Target, Qty: 1})
	}
	g.updateRelics(p)
	if q := g.uniqueQuest(p, "ulf"); q == nil || !q.Done {
		t.Fatal("relics not counted")
	}
	g.finishUniqueQuest(p, r)
	if !pl.Unlocked("subclass", "sealer") || slices.ContainsFunc(pl.Inventory, func(s ItemStack) bool { return s.Key == r.Target }) {
		t.Fatal("relics not handed over")
	}
}

func TestMimicAndDeathSentence(t *testing.T) {
	g := setup(t)
	p, _, _ := g.Join("Тест", "rogue")
	wild(t, g, p)
	l := g.Levels["overworld"]
	mage := spawnAt(g, "bandit_mage", l, p.Pos.Add(world.Pos{X: 3}))
	g.indexLevels()
	g.unlockAbility(p, "mimicry")
	p.MP = 100
	g.useAbility(p, "mimicry", nil)
	if p.Player.Copied != "m_firebolt" || !slices.Contains(p.Player.Abilities, "copied") {
		t.Fatalf("copy: %q", p.Player.Copied)
	}
	g.useAbility(p, "copied", nil)
	run(g, 30)
	if mage.HP >= mage.MaxHP {
		t.Fatal("the copied firebolt did not hit")
	}
	// the death sentence kills weaker creatures
	p.Player.Level = 10
	weak := spawnAt(g, "wolf", l, p.Pos.Add(world.Pos{Y: 2}))
	g.indexLevels()
	g.unlockAbility(p, "death_sentence")
	g.useAbility(p, "death_sentence", nil)
	if g.Entities[weak.ID] != nil {
		t.Fatal("a weaker creature survived the sentence")
	}
}
