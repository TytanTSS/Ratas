package game

import (
	"math"
	"slices"
	"sort"
	"strings"

	"ratas/internal/content"
	"ratas/internal/world"
)

// AbilityFunc implements one ability kind. target may be nil (e.g. no enemy
// in range); it returns false if the ability could not be used.
type AbilityFunc func(g *Game, caster *Entity, a *content.AbilityDef, target *Entity) bool

var abilityKinds = map[string]AbilityFunc{}

// RegisterAbilityKind adds a new ability behaviour. Abilities in content files refer to
// it by name in their "kind" field, so new kinds only need Go code once and
// can then be reused by any number of data-defined abilities.
func RegisterAbilityKind(kind string, fn AbilityFunc) { abilityKinds[kind] = fn }

func init() {
	RegisterAbilityKind("projectile", castProjectile)
	RegisterAbilityKind("nova", castNova)
	RegisterAbilityKind("cleave", castCleave)
	RegisterAbilityKind("strike", castStrike)
	RegisterAbilityKind("heal", castHeal)
	RegisterAbilityKind("dash", castDash)
	RegisterAbilityKind("buff", castBuff)
	RegisterAbilityKind("chain", castChain)
	RegisterAbilityKind("taunt", castTaunt)
	RegisterAbilityKind("decoy", castSummon)
	RegisterAbilityKind("summon", castSummon)
	RegisterAbilityKind("mimic", castMimic)
	RegisterAbilityKind("copied", castCopied)
	RegisterAbilityKind("echo", castEcho)
	RegisterAbilityKind("revive", castRevive)
	RegisterAbilityKind("death_sentence", castDeathSentence)
}

func hostileFactions(a, b Faction) bool {
	return (a == FPlayer && b == FMonster) || (a == FMonster && b == FPlayer)
}

// projectileHits reports whether a projectile hurts o.
func (g *Game) projectileHits(ps *ProjState, o *Entity) bool {
	if o.ID == ps.Owner {
		return false
	}
	if owner := g.Entities[ps.Owner]; owner != nil {
		return g.hostile(owner, o)
	}
	return o.Alive() && hostileFactions(ps.Faction, o.Faction)
}

func monsterScale(lvl int) float64 { return 1 + 0.18*float64(lvl-1) }

// ---- damage ----

func (g *Game) heal(e *Entity, amount float64) {
	if !e.Alive() {
		return
	}
	e.HP = math.Min(e.MaxHP, e.HP+amount)
	g.FX(e.Level, e.Pos, "+"+itoa(int(math.Round(amount))), 0, "#60ff60", 800)
}

func (g *Game) kill(e *Entity, killer *Entity) {
	switch {
	case e.Kind == KPlayer:
		g.killPlayer(e, killer)
	case e.Kind == KMonster && e.Faction == FMonster:
		g.killMonster(e, killer)
	default:
		g.killAlly(e)
	}
}

func (g *Game) killMonster(m *Entity, killer *Entity) {
	ms := m.Monster
	def := content.Monster(ms.Def)
	g.Remove(m)
	g.FX(m.Level, m.Pos, "", '%', "#a03030", 1200)
	if killer != nil && killer.Kind == KProjectile {
		killer = g.Entities[killer.Proj.Owner]
	}
	isBoss, isElite := def != nil && def.Boss, def != nil && def.Elite
	if def != nil {
		gold := g.roll(float64(def.Gold[0]), float64(def.Gold[1]+1)) * monsterScale(ms.Lvl)
		if killer != nil && killer.Player != nil {
			gold *= 1 + killer.stats.GoldFind/100
		}
		g.dropGold(m.Level, m.Pos, int(gold))
		depth := max(1, ms.Lvl)
		itemChance := 10.0
		switch {
		case isBoss:
			itemChance = 100
		case isElite:
			itemChance = 60
		}
		if g.chance(itemChance) {
			if st, ok := g.randomItem(depth, 30); ok {
				g.dropItem(m.Level, m.Pos, st)
			}
		}
		for _, key := range def.Drops {
			g.dropItem(m.Level, m.Pos, ItemStack{Key: key, Qty: 1})
		}
		if isBoss {
			if st, ok := g.randomItem(depth+2, 100); ok {
				g.dropItem(m.Level, m.Pos, st)
			}
		}
	}
	var heroes []string
	kc := g.controller(killer)
	for _, p := range g.playersOn(m.Level) {
		if !p.Alive() || (p.Pos.Dist(m.Pos) > 25 && !(kc != nil && g.sameParty(p, kc) && p.Pos.Dist(m.Pos) <= 50)) {
			continue
		}
		heroes = append(heroes, p.Name)
		g.GiveXP(p, ms.XP)
		p.Player.Kills++
		if p == killer {
			g.Log(p, "#e0e0e0", "Вы убили: %s (+%d опыта).", m.Name, ms.XP)
		} else {
			g.Log(p, "#c0c0c0", "%s повержен (+%d опыта).", m.Name, ms.XP)
		}
		g.questProgress(p, ms.Def)
		g.championSlain(p, m)
		g.relicDrop(p, m)
		if isBoss && !slices.Contains(p.Player.Bosses, ms.Def) {
			p.Player.Bosses = append(p.Player.Bosses, ms.Def)
		}
	}
	if isBoss {
		sort.Strings(heroes)
		g.LogAll("#ff4aff", "*** %s повержен! Герои: %v ***", m.Name, heroes)
		g.chronicle("%s сразил(и) %s", strings.Join(heroes, ", "), m.Name)
	}
	if ms.Champion != "" && len(heroes) > 0 {
		g.chronicle("%s одолел(и) чемпиона «%s»", strings.Join(heroes, ", "), m.Name)
	}
	if def != nil && def.Role == "leader" && ms.Squad != 0 {
		// the squad loses heart when its leader falls
		for _, o := range g.onLevel(m.Level) {
			if o.Monster != nil && o.Monster.Squad == ms.Squad && o != m && g.chance(40) {
				o.Monster.FleeUntil = g.Now + 4000
				g.Say(o, "Бежим!", 1500)
			}
		}
	}
	// let nearby elites know
	for _, o := range g.onLevel(m.Level) {
		if o.Monster != nil && o.Faction == FMonster && o.Pos.Dist(m.Pos) < 12 {
			o.Monster.Events = append(o.Monster.Events, "your ally "+m.Name+" was just killed")
		}
	}
}

func (g *Game) questProgress(p *Entity, monster string) {
	for i := range p.Player.Quests {
		q := &p.Player.Quests[i]
		if q.Done || q.Monster != monster || (q.Kind != "" && q.Kind != "boss") {
			continue
		}
		q.Have++
		if q.Have >= q.Need {
			q.Done = true
			g.Log(p, "#ffd24a", "Задание выполнено! Вернитесь к: %s (%s).", q.Giver, q.Village)
		}
		p.Player.Dirty = true
	}
}

func (g *Game) meleeAttack(a, d *Entity) {
	a.NextAttack = g.Now + a.stats.AttackMs
	a.Facing = world.DirTowards(a.Pos, d.Pos)
	a.Swings++
	dmg := g.meleeDamage(a)
	g.rollCrit(a, &dmg)
	g.weaponHit(a, d, g.damage(a, d, dmg))
}

// attackFacing attacks the enemy in front or any adjacent enemy; with a bow
// or crossbow it shoots the nearest enemy in sight.
func (g *Game) attackFacing(e *Entity) {
	if e.stats.Gear.Ranged {
		if t := g.autoTarget(e, &bowShot); t != nil {
			e.NextAttack = g.Now + e.stats.AttackMs*1.3
			e.Facing = world.DirTowards(e.Pos, t.Pos)
			e.Swings++
			castProjectile(g, e, &bowShot, t)
			return
		}
	}
	front := g.At(e.Level, e.Pos.Add(e.Facing.Delta()))
	if front != nil && g.hostile(e, front) {
		g.meleeAttack(e, front)
		return
	}
	for _, d := range world.AllDirs {
		if o := g.At(e.Level, e.Pos.Add(d.Delta())); o != nil && g.hostile(e, o) {
			g.meleeAttack(e, o)
			return
		}
	}
	e.NextAttack = g.Now + 200
}

// bowShot is the basic attack with a bow or crossbow: weapon damage.
var bowShot = content.AbilityDef{Key: "bow_shot", Name: "Выстрел", Kind: "projectile", Range: 9, SpeedMs: 35,
	Glyph: "dir", Color: "#e0d0a0", DmgType: "weapon"}

// ---- loot ----

func itemSpot(l *world.Level, p world.Pos) world.Pos {
	if l.Walkable(p.X, p.Y) {
		return p
	}
	for _, d := range world.AllDirs {
		q := p.Add(d.Delta())
		if l.Walkable(q.X, q.Y) {
			return q
		}
	}
	return p
}

func (g *Game) dropItem(level string, p world.Pos, st ItemStack) {
	d := st.Def()
	if d == nil {
		return
	}
	if st.Qty <= 0 {
		st.Qty = 1
	}
	l := g.Levels[level]
	g.Spawn(&Entity{Kind: KItem, Name: st.Name(), Glyph: d.Glyph, Color: d.Color, Level: level, Pos: itemSpot(l, p), Item: &st})
}

func (g *Game) dropGold(level string, p world.Pos, n int) {
	if n > 0 {
		g.dropItem(level, p, ItemStack{Key: "gold", Qty: n})
	}
}

type affix struct {
	stat, suffix string
	base         float64
}

var affixes = []affix{
	{"str", "силы", 1.5}, {"dex", "ловкости", 1.5}, {"int", "мудрости", 1.5}, {"vit", "здоровья", 1.5},
	{"max_hp", "жизни", 8}, {"crit", "точности", 2}, {"armor", "защиты", 1.5}, {"move_speed", "ветра", 4},
	{"spell_pct", "чародейства", 6}, {"melee_pct", "ярости", 6}, {"ranged_pct", "меткости", 6}, {"mp_regen", "покоя", 0.4},
	{"res_fire", "огнеупорности", 8}, {"res_cold", "тепла", 8}, {"res_lightning", "заземления", 8},
	{"res_poison", "противоядия", 9}, {"res_shadow", "рассвета", 8}, {"res_elemental", "стихий", 4},
	{"life_leech", "вампира", 1.2}, {"thorns", "шипов", 2}, {"add_fire", "пламени", 1.5}, {"add_cold", "стужи", 1.5},
	{"add_lightning", "грома", 1.5}, {"add_poison", "яда", 1.5}, {"holy_pct", "праведника", 6}, {"shadow_pct", "тьмы", 6},
}

// randomItem picks a weighted random item for a depth; equipment may get a
// random magic bonus with magicChance percent.
func (g *Game) randomItem(depth int, magicChance float64) (ItemStack, bool) {
	total := 0
	for _, it := range content.Items() {
		if it.Weight > 0 && it.Depth <= depth {
			total += it.Weight
		}
	}
	if total == 0 {
		return ItemStack{}, false
	}
	n := g.rng.IntN(total)
	for _, it := range content.Items() {
		if it.Weight <= 0 || it.Depth > depth {
			continue
		}
		n -= it.Weight
		if n >= 0 {
			continue
		}
		st := ItemStack{Key: it.Key, Qty: 1}
		if slotFor(&it) != "" && g.chance(magicChance) {
			af := affixes[g.rng.IntN(len(affixes))]
			v := af.base * (1 + float64(depth)*0.4) * g.roll(0.8, 1.2)
			if v >= 3 {
				v = math.Round(v)
			} else {
				v = math.Round(v*10) / 10
			}
			st.Bonus = map[string]float64{af.stat: v}
			st.Suffix = af.suffix
		}
		return st, true
	}
	return ItemStack{}, false
}

// ---- abilities ----

func (g *Game) useHotbar(e *Entity, slot int) bool {
	if slot < 1 || slot > HotbarSize {
		return true
	}
	key := e.Player.Hotbar[slot-1]
	if key == "" {
		g.Log(e, "#808080", "Слот %d пуст. Назначьте умение в окне навыков (K).", slot)
		return true
	}
	return g.useAbility(e, key, nil)
}

// useAbility returns true when the request is finished (cast or failed) and
// false when it should wait (cooldown).
func (g *Game) useAbility(c *Entity, key string, target *Entity) bool {
	a := content.Ability(key)
	if a == nil || !c.Alive() {
		return true
	}
	if g.Now < c.Cooldowns[key] {
		return false
	}
	if c.MP < a.Mana {
		g.Log(c, "#6090ff", "Недостаточно маны для «%s».", a.Name)
		return true
	}
	if c.stats.Silenced {
		g.Log(c, "#e0c8ff", "Вы под печатью и не можете применять умения.")
		return true
	}
	if c.Player != nil && !c.stats.Gear.Has(a.Equip) {
		g.Log(c, "#ff8080", "Для «%s» нужно: %s.", a.Name, GearNeedName(a.Equip))
		return true
	}
	fn := abilityKinds[a.Kind]
	if fn == nil {
		g.Log(c, "#ff8080", "Неизвестный тип умения: %s.", a.Kind)
		return true
	}
	if target == nil {
		target = g.autoTarget(c, a)
	}
	if !fn(g, c, a, target) {
		return true
	}
	c.MP -= a.Mana
	c.Cooldowns[key] = g.Now + float64(a.CooldownMs)
	return true
}

func abilityRange(a *content.AbilityDef) int {
	if a.Range > 0 {
		return a.Range
	}
	if a.Kind == "projectile" {
		return 8
	}
	return 1
}

// autoTarget finds the nearest visible enemy in range.
func (g *Game) autoTarget(c *Entity, a *content.AbilityDef) *Entity {
	if c.Monster != nil {
		if t := g.Entities[c.Monster.Target]; t != nil {
			return t
		}
	}
	rng := abilityRange(a)
	l := g.Levels[c.Level]
	var best *Entity
	bestD := math.MaxInt
	for _, o := range g.onLevel(c.Level) {
		if !g.hostile(c, o) || !g.canSee(c, o) {
			continue
		}
		d := c.Pos.DistSq(o.Pos)
		if c.Pos.Dist(o.Pos) > rng || d >= bestD {
			continue
		}
		if rng > 1 && !world.LOS(l, c.Pos, o.Pos) {
			continue
		}
		if rng <= 1 && c.Pos.Manhattan(o.Pos) != 1 {
			continue
		}
		best, bestD = o, d
	}
	return best
}

func dirGlyph(from, to world.Pos) rune {
	dx, dy := float64(to.X-from.X), float64(to.Y-from.Y)*2
	ang := math.Atan2(-dy, dx) * 180 / math.Pi // 0 = right, 90 = up
	if ang < 0 {
		ang += 180
	}
	switch {
	case ang < 22.5 || ang >= 157.5:
		return '-'
	case ang < 67.5:
		return '/'
	case ang < 112.5:
		return '|'
	default:
		return '\\'
	}
}

func castProjectile(g *Game, c *Entity, a *content.AbilityDef, target *Entity) bool {
	rng := abilityRange(a)
	var aim world.Pos
	if target != nil {
		aim = target.Pos
	} else {
		d := c.Facing.Delta()
		aim = world.Pos{X: c.Pos.X + d.X*rng, Y: c.Pos.Y + d.Y*rng}
	}
	if aim == c.Pos {
		return false
	}
	c.Facing = world.DirTowards(c.Pos, aim)
	count := max(1, a.Count)
	dx, dy := aim.X-c.Pos.X, aim.Y-c.Pos.Y
	dist := max(iabs(dx), iabs(dy))
	spread := max(1, dist/3)
	for i := 0; i < count; i++ {
		off := i - count/2
		t := aim
		if off != 0 {
			if iabs(dx) >= iabs(dy) {
				t.Y += off * spread
			} else {
				t.X += off * spread * 2
			}
		}
		path := world.LinePoints(c.Pos, t, rng)
		if len(path) == 0 {
			continue
		}
		dmg := g.abilityDamage(c, a)
		g.rollCrit(c, &dmg)
		glyph := a.Glyph
		if glyph == "dir" || glyph == "" {
			glyph = string(dirGlyph(c.Pos, path[len(path)-1]))
		}
		speed := float64(a.SpeedMs)
		if speed <= 0 {
			speed = 50
		}
		g.Spawn(&Entity{
			Kind: KProjectile, Name: a.Name, Glyph: glyph, Color: a.Color, Level: c.Level, Pos: c.Pos,
			Proj: &ProjState{Owner: c.ID, Faction: c.Faction, Path: path, StepMs: speed, Next: g.Now,
				Damage: dmg, Radius: a.Radius, OnHit: a.OnHit, Ability: a.Key},
		})
	}
	return true
}

func (g *Game) updateProjectile(p *Entity) {
	ps := p.Proj
	l := g.Levels[p.Level]
	for g.Now >= ps.Next {
		ps.Next += ps.StepMs
		if ps.Step >= len(ps.Path) {
			g.projectileImpact(p, p.Pos, nil)
			return
		}
		next := ps.Path[ps.Step]
		ps.Step++
		if !l.Transparent(next.X, next.Y) {
			g.projectileImpact(p, p.Pos, nil)
			return
		}
		p.Pos = next
		if o := g.At(p.Level, next); o != nil && g.projectileHits(ps, o) {
			g.projectileImpact(p, next, o)
			return
		}
	}
}

func (g *Game) projectileImpact(p *Entity, at world.Pos, hit *Entity) {
	ps := p.Proj
	g.Remove(p)
	owner := g.Entities[ps.Owner]
	if ps.Radius > 0 {
		g.FX(p.Level, at, "", '*', p.Color, 250)
		g.areaDamage(owner, ps.Faction, p.Level, at, ps.Radius, ps.Damage, ps.OnHit, '*', p.Color)
		return
	}
	if hit != nil {
		g.damage(owner, hit, ps.Damage)
		g.applyBuff(hit, ps.OnHit, ps.Owner)
	}
}

// areaDamage hits every enemy of faction within radius (and flashes the area).
func (g *Game) areaDamage(src *Entity, f Faction, level string, at world.Pos, radius int, dmg Damage, onHit *content.BuffDef, glyph rune, color string) {
	l := g.Levels[level]
	r2 := radius*radius + radius
	for dy := -radius; dy <= radius; dy++ {
		for dx := -radius; dx <= radius; dx++ {
			q := world.Pos{X: at.X + dx, Y: at.Y + dy}
			if dx*dx+dy*dy <= r2 && l.Transparent(q.X, q.Y) && (dx != 0 || dy != 0) {
				g.FX(level, q, "", glyph, color, 220)
			}
		}
	}
	var victims []*Entity
	for _, o := range g.onLevel(level) {
		if !o.Alive() || o.Pos.DistSq(at) > r2 || o.Kind == KProjectile || o.Kind == KItem {
			continue
		}
		if (src != nil && g.hostile(src, o)) || (src == nil && hostileFactions(f, o.Faction)) {
			victims = append(victims, o)
		}
	}
	for _, o := range victims {
		if len(dmg.Parts) > 0 {
			g.damage(src, o, dmg)
		} else {
			g.provoke(src, o)
		}
		g.applyBuff(o, onHit, idOf(src))
	}
}

func idOf(e *Entity) EntityID {
	if e == nil {
		return 0
	}
	return e.ID
}

func castNova(g *Game, c *Entity, a *content.AbilityDef, _ *Entity) bool {
	dmg := g.abilityDamage(c, a)
	g.rollCrit(c, &dmg)
	g.areaDamage(c, c.Faction, c.Level, c.Pos, max(1, a.Radius), dmg, a.OnHit, glyphOf(a, '*'), a.Color)
	return true
}

func castCleave(g *Game, c *Entity, a *content.AbilityDef, _ *Entity) bool {
	r := max(1, a.Radius)
	var victims []*Entity
	for _, o := range g.onLevel(c.Level) {
		if g.hostile(c, o) && c.Pos.Dist(o.Pos) <= r {
			victims = append(victims, o)
		}
	}
	for dy := -r; dy <= r; dy++ {
		for dx := -r; dx <= r; dx++ {
			if dx != 0 || dy != 0 {
				g.FX(c.Level, world.Pos{X: c.Pos.X + dx, Y: c.Pos.Y + dy}, "", glyphOf(a, '*'), a.Color, 200)
			}
		}
	}
	for _, o := range victims {
		dmg := g.abilityDamage(c, a)
		dmg.merge(g.meleeDamage(c), 0.5)
		g.rollCrit(c, &dmg)
		g.weaponHit(c, o, g.damage(c, o, dmg))
		g.applyBuff(o, a.OnHit, c.ID)
	}
	c.NextAttack = g.Now + c.stats.AttackMs
	return true
}

func castStrike(g *Game, c *Entity, a *content.AbilityDef, target *Entity) bool {
	if target == nil || c.Pos.Manhattan(target.Pos) != 1 {
		g.Log(c, "#808080", "Нет врага рядом для «%s».", a.Name)
		return false
	}
	c.Facing = world.DirTowards(c.Pos, target.Pos)
	c.Swings++
	hits := max(1, a.Count)
	weaponK := 1.0
	if hits > 1 {
		weaponK = 0.5
	}
	for i := 0; i < hits && target.Alive(); i++ {
		dmg := g.abilityDamage(c, a)
		dmg.merge(g.meleeDamage(c), weaponK)
		if a.Execute > 0 && target.MaxHP > 0 {
			dmg.scale(1 + a.Execute*(1-target.HP/target.MaxHP))
		}
		dmg.Backstab = a.Backstab
		g.rollCrit(c, &dmg)
		g.FX(c.Level, target.Pos, "", glyphOf(a, '!'), a.Color, 250)
		g.weaponHit(c, target, g.damage(c, target, dmg))
		g.applyBuff(target, a.OnHit, c.ID)
	}
	c.NextAttack = g.Now + c.stats.AttackMs
	return true
}

func castHeal(g *Game, c *Entity, a *content.AbilityDef, _ *Entity) bool {
	amount := g.abilityPower(c, a) * (1 + c.stats.HealPct/100)
	g.heal(c, amount)
	if a.Radius > 0 {
		for _, o := range g.onLevel(c.Level) {
			if o != c && o.Alive() && o.Blocks() && g.friendly(c, o) && c.Pos.Dist(o.Pos) <= a.Radius {
				g.heal(o, amount*0.7)
			}
		}
	}
	if a.Buff != nil {
		g.applyBuff(c, a.Buff, c.ID)
	}
	return true
}

func castDash(g *Game, c *Entity, a *content.AbilityDef, _ *Entity) bool {
	l := g.Levels[c.Level]
	d := c.Facing.Delta()
	pos := c.Pos
	for i := 0; i < abilityRange(a); i++ {
		next := pos.Add(d)
		if o := g.At(c.Level, next); o != nil {
			if g.hostile(c, o) && a.Damage[1] > 0 {
				dmg := g.abilityDamage(c, a)
				g.rollCrit(c, &dmg)
				g.damage(c, o, dmg)
				g.applyBuff(o, a.OnHit, c.ID)
			}
			break
		}
		if !l.Walkable(next.X, next.Y) {
			break
		}
		g.FX(c.Level, pos, "", glyphOf(a, '~'), a.Color, 250)
		pos = next
	}
	if pos == c.Pos {
		g.Log(c, "#808080", "Некуда рвануться.")
		return false
	}
	g.moveEntity(c, pos)
	if c.Player != nil {
		g.afterPlayerMove(c)
	}
	return true
}

func castBuff(g *Game, c *Entity, a *content.AbilityDef, _ *Entity) bool {
	if a.Buff == nil {
		return false
	}
	g.applyBuff(c, a.Buff, c.ID)
	g.FX(c.Level, c.Pos, a.Buff.Name+"!", 0, a.Color, 1000)
	if a.Radius > 0 {
		// party buff: allies around get it too
		for _, o := range g.onLevel(c.Level) {
			if o != c && o.Alive() && o.Blocks() && g.friendly(c, o) && c.Pos.Dist(o.Pos) <= a.Radius {
				g.applyBuff(o, a.Buff, c.ID)
				g.FX(o.Level, o.Pos, "", glyphOf(a, '+'), a.Color, 400)
			}
		}
	}
	return true
}

// castChain hits the target, then jumps to the nearest enemies around it.
func castChain(g *Game, c *Entity, a *content.AbilityDef, target *Entity) bool {
	if target == nil {
		g.Log(c, "#808080", "Нет цели для «%s».", a.Name)
		return false
	}
	l := g.Levels[c.Level]
	hit := map[EntityID]bool{}
	from := c.Pos
	k := 1.0
	for jump := 0; target != nil && jump <= max(1, a.Count); jump++ {
		for _, p := range world.LinePoints(from, target.Pos, 30) {
			g.FX(c.Level, p, "", glyphOf(a, '*'), a.Color, 180)
		}
		dmg := g.abilityDamage(c, a)
		dmg.scale(k)
		g.rollCrit(c, &dmg)
		hit[target.ID] = true
		g.damage(c, target, dmg)
		g.applyBuff(target, a.OnHit, c.ID)
		from = target.Pos
		k *= 0.85
		var next *Entity
		best := math.MaxInt
		for _, o := range g.onLevel(c.Level) {
			if hit[o.ID] || !g.hostile(c, o) {
				continue
			}
			if d := from.DistSq(o.Pos); d < best && from.Dist(o.Pos) <= 4 && world.LOS(l, from, o.Pos) {
				next, best = o, d
			}
		}
		target = next
	}
	return true
}

func glyphOf(a *content.AbilityDef, def rune) rune {
	if a.Glyph == "" || a.Glyph == "dir" {
		return def
	}
	return []rune(a.Glyph)[0]
}

// ---- buffs ----

func (g *Game) applyBuff(e *Entity, def *content.BuffDef, src EntityID) {
	if def == nil || !e.Alive() {
		return
	}
	until := g.Now + float64(def.DurationMs)
	for i := range e.Buffs {
		if e.Buffs[i].Def.Key == def.Key {
			e.Buffs[i].Until = until
			e.Buffs[i].Source = src
			return
		}
	}
	e.Buffs = append(e.Buffs, Buff{Def: *def, Until: until, Source: src})
	if def.Stun {
		g.FX(e.Level, e.Pos, "оглушён", 0, "#ffe080", 800)
	}
	e.Recalc()
	if e.Player != nil {
		e.Player.Dirty = true
	}
}

func (g *Game) updateBuffs(e *Entity) {
	if len(e.Buffs) == 0 {
		return
	}
	tickDot := g.TickN%10 == 0
	expired := false
	for i := 0; i < len(e.Buffs); i++ {
		b := e.Buffs[i]
		if tickDot && b.Def.DotPerSec != 0 {
			amt := b.Def.DotPerSec / 2
			if amt > 0 {
				t := b.Def.DmgType
				if t == "" {
					t = "poison"
				}
				dot := single(t, amt)
				dot.DoT = true
				g.damage(g.Entities[b.Source], e, dot)
				if !e.Alive() || g.Entities[e.ID] != e {
					return
				}
			} else {
				if src := g.Entities[b.Source]; src != nil {
					amt *= 1 + src.stats.HealPct/100
				}
				g.heal(e, -amt)
			}
		}
		if g.Now >= b.Until {
			expired = true
		}
	}
	if expired {
		kept := e.Buffs[:0]
		for _, b := range e.Buffs {
			if g.Now < b.Until {
				kept = append(kept, b)
			}
		}
		e.Buffs = kept
		e.Recalc()
		if e.Player != nil {
			e.Player.Dirty = true
		}
	}
}
