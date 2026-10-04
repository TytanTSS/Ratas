package game

import (
	"ratas/internal/content"
	"ratas/internal/world"
)

// canSee: stealthed enemies are invisible unless right next to you.
func (g *Game) canSee(c, o *Entity) bool {
	return !o.stats.Stealthed || c.Pos.Dist(o.Pos) <= 1 || g.friendly(c, o)
}

// castTaunt forces enemies around to attack the caster.
func castTaunt(g *Game, c *Entity, a *content.AbilityDef, _ *Entity) bool {
	r := max(1, a.Radius)
	n := 0
	for _, o := range g.onLevel(c.Level) {
		if !g.hostile(c, o) || c.Pos.Dist(o.Pos) > r {
			continue
		}
		g.applyBuff(o, a.OnHit, c.ID)
		if o.Monster != nil {
			o.Monster.Target = c.ID
			o.Monster.LastSeen, o.Monster.LastSeenAt = c.Pos, g.Now
		}
		g.FX(o.Level, o.Pos, "!", 0, a.Color, 800)
		n++
	}
	if a.Buff != nil {
		g.applyBuff(c, a.Buff, c.ID)
	}
	g.FX(c.Level, c.Pos, a.Name+"!", 0, a.Color, 1000)
	return true
}

// castSummon calls allies (summon) or illusions (decoy) next to the caster.
func castSummon(g *Game, c *Entity, a *content.AbilityDef, _ *Entity) bool {
	def := content.Monster(a.Summon)
	if def == nil {
		return false
	}
	// a new call replaces the old summons of the same kind
	for _, o := range g.onLevel(c.Level) {
		if o.Owner == c.ID && o.Monster != nil && o.Monster.Def == def.Key {
			g.Remove(o)
		}
	}
	l := g.Levels[c.Level]
	lvl := 1
	if c.Player != nil {
		lvl = c.Player.Level
	} else if c.Monster != nil {
		lvl = c.Monster.Lvl
	}
	var made []*Entity
	for i := 0; i < max(1, a.Count); i++ {
		p := findFree(l, c.Pos.Add(world.AllDirs[i%4].Delta()))
		if p.Dist(c.Pos) > 4 {
			break
		}
		m := g.newMonster(def, c.Level, p, lvl)
		m.Faction, m.Owner = c.Faction, c.ID
		m.Expires = g.Now + float64(max(1000, a.Duration))
		m.Monster.Persistent = true
		if a.Kind == "decoy" {
			m.Name = "Иллюзия: " + c.Name
			m.Glyph, m.Color = c.Glyph, c.Color
			m.HP, m.MaxHP = 20+float64(lvl)*8, 20+float64(lvl)*8
		} else {
			m.Name = def.Name + " (" + c.Name + ")"
		}
		g.FX(m.Level, m.Pos, "", '*', a.Color, 400)
		made = append(made, m)
	}
	if len(made) == 0 {
		return false
	}
	if a.Kind == "decoy" {
		// enemies who were after the caster turn to the illusions
		for _, o := range g.onLevel(c.Level) {
			if o.Monster != nil && g.hostile(o, c) && o.Pos.Dist(c.Pos) <= 8 && (o.Monster.Target == c.ID || o.Monster.Target == 0) {
				o.Monster.Target = made[g.rng.IntN(len(made))].ID
			}
		}
	}
	return true
}

// castMimic copies an ability of the nearest enemy.
func castMimic(g *Game, c *Entity, a *content.AbilityDef, target *Entity) bool {
	if target == nil || c.Player == nil {
		g.Log(c, "#808080", "Некого копировать.")
		return false
	}
	key, lvl := "", 1
	switch {
	case target.Monster != nil:
		def := content.Monster(target.Monster.Def)
		if def != nil {
			for _, k := range append([]string{def.Ability}, def.Abilities...) {
				if ad := content.Ability(k); ad != nil && copyable(ad) {
					key = k
					break
				}
			}
		}
		lvl = target.Monster.Lvl
	case target.Player != nil:
		var opts []string
		for _, k := range target.Player.Abilities {
			if ad := content.Ability(k); ad != nil && copyable(ad) {
				opts = append(opts, k)
			}
		}
		if len(opts) > 0 {
			key = opts[g.rng.IntN(len(opts))]
		}
		lvl = target.Player.Level
	}
	if key == "" {
		g.Log(c, "#808080", "У %s нет умения, которое можно скопировать.", target.Name)
		return false
	}
	p := c.Player
	p.Copied, p.CopiedLvl, p.CopiedEnd = key, min(lvl, p.Level), g.Now+120000
	g.unlockAbility(c, "copied")
	p.Dirty = true
	name := content.Ability(key).Name
	g.Log(c, "#ff8ad8", "Скопировано умение: %s (на 2 минуты, кнопка «Копия»).", name)
	g.FX(c.Level, c.Pos, "Копия: "+name, 0, "#ff8ad8", 1500)
	g.FX(target.Level, target.Pos, "", '*', "#ff8ad8", 400)
	return true
}

func copyable(a *content.AbilityDef) bool {
	switch a.Kind {
	case "mimic", "copied", "echo", "revive", "death_sentence":
		return false
	}
	return true
}

// copyPower is how strong a copy is: weaker than the original, depending on
// the levels of the original's owner and the mimic, plus mimic skills.
func copyPower(c *Entity) float64 {
	return 0.8 * (1 + c.stats.MimicPct/100) * monsterScale(max(1, c.Player.CopiedLvl))
}

func (g *Game) castCopy(c *Entity, target *Entity, k float64) bool {
	p := c.Player
	if p == nil || p.Copied == "" {
		g.Log(c, "#808080", "Сначала скопируйте умение врага.")
		return false
	}
	orig := content.Ability(p.Copied)
	if orig == nil {
		return false
	}
	clone := *orig
	pw := copyPower(c) * k
	clone.Damage = [2]float64{orig.Damage[0] * pw, orig.Damage[1] * pw}
	fn := abilityKinds[clone.Kind]
	if fn == nil || !copyable(&clone) {
		return false
	}
	if target == nil {
		target = g.autoTarget(c, &clone)
	}
	return fn(g, c, &clone, target)
}

func castCopied(g *Game, c *Entity, a *content.AbilityDef, target *Entity) bool {
	return g.castCopy(c, target, 1)
}

func castEcho(g *Game, c *Entity, a *content.AbilityDef, target *Entity) bool {
	return g.castCopy(c, target, 2)
}

// castRevive brings back a fallen ally next to the caster.
func castRevive(g *Game, c *Entity, a *content.AbilityDef, _ *Entity) bool {
	r := max(1, a.Range)
	for _, o := range g.onLevel(c.Level) {
		if o.Player != nil && o.Dead && o != c && c.Pos.Dist(o.Pos) <= r {
			if g.revive(o, c) {
				return true
			}
		}
	}
	g.Log(c, "#808080", "Рядом нет павших союзников, которых ещё можно вернуть.")
	return false
}

// castDeathSentence kills every creature around weaker than the caster.
func castDeathSentence(g *Game, c *Entity, a *content.AbilityDef, _ *Entity) bool {
	lvl := 1
	if c.Player != nil {
		lvl = c.Player.Level
	}
	r := max(1, a.Radius)
	g.areaFX(c.Level, c.Pos, r, glyphOf(a, '%'), a.Color)
	for _, o := range g.onLevel(c.Level) {
		if o.Monster == nil || !g.hostile(c, o) || c.Pos.Dist(o.Pos) > r {
			continue
		}
		def := content.Monster(o.Monster.Def)
		switch {
		case def != nil && def.Boss:
			o.HP -= o.MaxHP * 0.25
			g.FX(o.Level, o.Pos, "приговор", 0, a.Color, 1000)
			g.provoke(c, o)
			if o.HP <= 0 {
				g.kill(o, c)
			}
		case o.Monster.Lvl < lvl:
			g.FX(o.Level, o.Pos, "смерть", 0, a.Color, 1000)
			o.HP = 0
			g.kill(o, c)
		}
	}
	return true
}

func (g *Game) areaFX(level string, at world.Pos, r int, glyph rune, color string) {
	for dy := -r; dy <= r; dy++ {
		for dx := -r; dx <= r; dx++ {
			if dx*dx+dy*dy <= r*r+r && (dx != 0 || dy != 0) {
				g.FX(level, world.Pos{X: at.X + dx, Y: at.Y + dy}, "", glyph, color, 260)
			}
		}
	}
}
