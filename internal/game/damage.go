package game

import (
	"math"

	"ratas/internal/content"
)

// DmgPart is an amount of one damage type.
type DmgPart struct {
	Type   string
	Amount float64
}

// Damage describes one hit. A hit may carry several damage types (a flaming
// sword deals slashing and fire damage); each part is reduced by the target's
// resistance to its type, physical parts also by armor.
type Damage struct {
	Parts    []DmgPart
	Crit     bool
	Leech    float64 // percent of the damage dealt that heals the attacker
	DoT      bool    // damage over time, hazards, thorns: no dodge, no armor
	Backstab float64 // multiplier when attacking from stealth or a distracted target
}

func (d *Damage) add(t string, v float64) {
	if v <= 0 {
		return
	}
	for i := range d.Parts {
		if d.Parts[i].Type == t {
			d.Parts[i].Amount += v
			return
		}
	}
	d.Parts = append(d.Parts, DmgPart{t, v})
}

func (d *Damage) merge(o Damage, k float64) {
	for _, p := range o.Parts {
		d.add(p.Type, p.Amount*k)
	}
}

func (d *Damage) scale(k float64) {
	for i := range d.Parts {
		d.Parts[i].Amount *= k
	}
}

// Total is the raw amount before resistances.
func (d Damage) Total() float64 {
	t := 0.0
	for _, p := range d.Parts {
		t += p.Amount
	}
	return t
}

// Main is the type of the largest part.
func (d Damage) Main() string {
	best, bv := "", -1.0
	for _, p := range d.Parts {
		if p.Amount > bv {
			best, bv = p.Type, p.Amount
		}
	}
	return best
}

func single(t string, v float64) Damage { return Damage{Parts: []DmgPart{{t, v}}} }

func isPhysical(t string) bool {
	d := content.DamageType(t)
	return d == nil || d.Group == "physical"
}

// armorFactor: positive armor has diminishing returns, broken (negative)
// armor makes physical hits 3% stronger per point.
func armorFactor(armor float64) float64 {
	if armor >= 0 {
		return 1 - armor/(armor+15)
	}
	return 1 + math.Min(60, -armor)*0.03
}

// typeBonus applies the attacker's per-type damage bonuses.
func typeBonus(src *Entity, d *Damage) {
	for i := range d.Parts {
		d.Parts[i].Amount *= math.Max(0, 1+src.stats.TypeBonus(d.Parts[i].Type)/100)
	}
}

func (g *Game) rollCrit(e *Entity, d *Damage) {
	if g.chance(e.stats.Crit) {
		d.Crit = true
		d.scale(e.stats.CritMult)
	}
}

// meleeDamage is a weapon hit without the critical roll.
func (g *Game) meleeDamage(a *Entity) Damage {
	s := &a.stats
	dmg := g.roll(s.WeaponDmg[0], s.WeaponDmg[1])
	if a.Player != nil {
		dmg += s.Str * 0.6
	}
	dmg *= math.Max(0, 1+s.MeleePct/100)
	d := Damage{Leech: s.LifeLeech}
	d.add(s.WeaponType, dmg)
	if s.Gear.Dual {
		// the second weapon strikes along at half strength
		d.add(s.OffType, g.roll(s.OffDmg[0], s.OffDmg[1])*0.5*math.Max(0, 1+s.MeleePct/100))
	}
	for _, t := range content.DamageTypes() {
		if v := s.Mods["add_"+t.Key]; v > 0 {
			d.add(t.Key, v*g.roll(0.8, 1.2))
		}
	}
	typeBonus(a, &d)
	return d
}

func abilityType(c *Entity, a *content.AbilityDef) string {
	switch a.DmgType {
	case "weapon":
		return c.stats.WeaponType
	case "":
		switch a.Kind {
		case "strike", "cleave", "dash":
			return c.stats.WeaponType
		}
		return "arcane"
	}
	return a.DmgType
}

// abilityPower is the rolled and scaled strength of an ability (damage or healing).
func (g *Game) abilityPower(c *Entity, a *content.AbilityDef) float64 {
	v := g.roll(a.Damage[0], a.Damage[1])
	if c.Player != nil {
		s := &c.stats
		var attr, pct float64
		switch a.Scale {
		case "str":
			attr, pct = s.Str, s.MeleePct
		case "dex":
			attr, pct = s.Dex, s.RangedPct
		case "int":
			attr, pct = s.Int, s.SpellPct
		case "vit":
			attr = s.Vit
		}
		return (v + attr*a.ScaleK) * math.Max(0, 1+pct/100)
	}
	if c.Monster != nil {
		v *= monsterScale(c.Monster.Lvl)
	}
	return v
}

// abilityDamage is the damage of an ability without the critical roll.
func (g *Game) abilityDamage(c *Entity, a *content.AbilityDef) Damage {
	d := Damage{Leech: a.Leech + c.stats.LifeLeech, Backstab: a.Backstab}
	s := &c.stats
	if a.Damage[1] > 0 {
		v := g.abilityPower(c, a)
		if len(a.Split) > 0 {
			for _, t := range a.Split {
				d.add(t, v/float64(len(a.Split)))
			}
		} else {
			d.add(abilityType(c, a), v)
		}
	}
	// bows and crossbows put their own strength into every arrow
	if (a.Equip == "bow" || a.Key == bowShot.Key) && s.Gear.Ranged {
		v := g.roll(s.WeaponDmg[0], s.WeaponDmg[1])
		if c.Player != nil {
			v += s.Dex * 0.5
		}
		d.add(s.WeaponType, v*math.Max(0, 1+s.RangedPct/100))
	}
	typeBonus(c, &d)
	return d
}

// damage applies a hit after dodge, armor and resistances; it returns the
// health actually taken.
func (g *Game) damage(src, dst *Entity, d Damage) float64 {
	if !dst.Alive() || g.Entities[dst.ID] != dst || len(d.Parts) == 0 {
		return 0
	}
	if !d.DoT && src != nil && dst.stats.Dodge > 0 && g.chance(dst.stats.Dodge) {
		g.FX(dst.Level, dst.Pos, "уклон", 0, "#a0a0ff", 600)
		return 0
	}
	mult := 1.0
	if !d.DoT && src != nil {
		as := &src.stats
		if g.ambush(src, dst) {
			k := (1 + as.AmbushPct/100) * math.Max(1, d.Backstab)
			if k > 1.05 {
				mult *= k
				g.FX(dst.Level, dst.Pos, "в спину!", 0, "#c08aff", 700)
			}
		}
		if as.Stealthed {
			g.breakStealth(src)
		}
		if as.DuelPct > 0 && g.alone(src, dst) {
			mult *= 1 + as.DuelPct/100
		}
		if as.Fury > 0 && src.MaxHP > 0 {
			mult *= 1 + as.Fury/100*math.Max(0, 1-src.HP/src.MaxHP)
		}
	}
	blocked := !d.DoT && dst.stats.Block > 0 && g.chance(dst.stats.Block)
	if blocked {
		g.FX(dst.Level, dst.Pos, "блок", 0, "#a0c0ff", 600)
	}
	total := 0.0
	immune := true
	for _, p := range d.Parts {
		v := p.Amount * mult
		if v <= 0 {
			continue
		}
		if !d.DoT && isPhysical(p.Type) {
			v *= armorFactor(dst.stats.Armor)
			if blocked {
				v *= 0.25
			}
		}
		res := dst.stats.Resist(p.Type)
		if res < 100 {
			immune = false
		}
		total += v * (1 - res/100)
	}
	g.provoke(src, dst)
	if immune {
		if !d.DoT {
			g.FX(dst.Level, dst.Pos, "иммунитет", 0, "#9090a0", 700)
		}
		return 0
	}
	total = math.Max(1, total)
	dst.HP -= total
	color := "#ffffff"
	if t := content.DamageType(d.Main()); t != nil && t.Group != "physical" {
		color = t.Color
	}
	switch {
	case dst.Player != nil:
		color = "#ff4a4a"
	case d.Crit:
		color = "#ffff4a"
	}
	text := itoa(int(math.Round(total)))
	if d.Crit {
		text += "!"
	}
	g.FX(dst.Level, dst.Pos, text, 0, color, 700)
	if src != nil && d.Leech > 0 && src != dst && src.Alive() && g.Entities[src.ID] == src {
		g.leech(src, total*d.Leech/100)
	}

	if dst.HP <= 0 {
		g.kill(dst, src)
	}
	return total
}

// provoke makes a monster fight back and remembers the player's target.
func (g *Game) provoke(src, dst *Entity) {
	if src == nil || dst == nil {
		return
	}
	if src.Kind == KProjectile && src.Proj != nil {
		src = g.Entities[src.Proj.Owner]
		if src == nil {
			return
		}
	}
	if dst.Monster != nil && src != dst && g.hostile(dst, src) {
		ms := dst.Monster
		if t := g.Entities[ms.Target]; ms.Target == 0 || t == nil || !t.Alive() || dst.Pos.Dist(t.Pos) > dst.Pos.Dist(src.Pos)+3 {
			ms.Target = src.ID
		}
		if ms.Target == src.ID {
			ms.LastSeen = src.Pos
			ms.LastSeenAt = g.Now
		}
	}
	if pc := g.controller(src); pc.Player != nil && pc != dst {
		pc.Player.Target, pc.Player.TargetAt = dst.ID, g.Now
	}
	if dst.Player != nil {
		p := dst.Player
		if t := g.Entities[p.Target]; t == nil || !t.Alive() || g.Now-p.TargetAt > 4000 {
			p.Target, p.TargetAt = src.ID, g.Now
		}
	}
}

// ambush: attacking unseen (from stealth) or a target busy with someone else.
func (g *Game) ambush(src, dst *Entity) bool {
	if src.stats.Stealthed {
		return true
	}
	if dst.Monster == nil || dst.Monster.Target == 0 {
		return false
	}
	t := g.Entities[dst.Monster.Target]
	return t != nil && g.controller(t) != g.controller(src)
}

// alone: no other enemy of src stands near it (duels).
func (g *Game) alone(src, dst *Entity) bool {
	for _, o := range g.onLevel(src.Level) {
		if o != dst && o.Pos.Dist(src.Pos) <= 4 && g.hostile(src, o) {
			return false
		}
	}
	return true
}

func (g *Game) breakStealth(e *Entity) {
	kept := e.Buffs[:0]
	for _, b := range e.Buffs {
		if !b.Def.Stealth {
			kept = append(kept, b)
		}
	}
	e.Buffs = kept
	e.Recalc()
	if e.Player != nil {
		e.Player.Dirty = true
	}
}

func (g *Game) leech(e *Entity, amount float64) {
	if amount <= 0 || e.HP >= e.MaxHP {
		return
	}
	e.HP = math.Min(e.MaxHP, e.HP+amount)
	if amount >= 2 {
		g.FX(e.Level, e.Pos, "+"+itoa(int(math.Round(amount))), 0, "#60ff90", 600)
	}
}

// weaponHit finishes a melee hit: weapon effects and thorns.
func (g *Game) weaponHit(a, d *Entity, dealt float64) {
	if dealt <= 0 {
		return
	}
	s := &a.stats
	if s.WeaponOnHit != nil && d.Alive() && g.chance(s.OnHitPct) {
		g.applyBuff(d, s.WeaponOnHit, a.ID)
	}
	if s.OffOnHit != nil && d.Alive() && g.chance(s.OffPct) {
		g.applyBuff(d, s.OffOnHit, a.ID)
	}
	if t := d.stats.Thorns; t > 0 && a.Alive() && g.Entities[a.ID] == a {
		th := single("pierce", t)
		th.DoT = true
		g.damage(d, a, th)
	}
}

// hazardType is the damage type of a dangerous tile.
func hazardType(def *content.TileDef) string {
	if def.DmgType != "" {
		return def.DmgType
	}
	return "fire"
}

// isDebuff reports whether an effect is harmful (removed by cleansing).
func isDebuff(b *content.BuffDef) bool {
	if b.Stun || b.DotPerSec > 0 {
		return true
	}
	for _, v := range b.Stats {
		if v < 0 {
			return true
		}
	}
	return false
}
