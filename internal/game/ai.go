package game

import (
	"math"
	"slices"
	"strings"

	"ratas/internal/content"
	"ratas/internal/llm"
	"ratas/internal/world"
)

// The same tactical AI drives monsters and the creatures on the players'
// side (guards, wanderers, summons). Every creature has a role:
//   frontline  — engages and protects its archers and casters;
//   skirmisher — flanks, strikes the weakest from behind, retreats when hurt;
//   ranged/caster — keeps its distance, steps back from melee, shoots;
//   support    — stays back, heals wounded allies, then shoots;
//   leader     — fights in front and rallies the squad; its death breaks morale.

func roleOf(def *content.MonsterDef) string {
	if def.Role != "" {
		return def.Role
	}
	if def.Behavior == "ranged" {
		return "ranged"
	}
	return "frontline"
}

func squishy(def *content.MonsterDef) bool {
	switch roleOf(def) {
	case "ranged", "caster", "support":
		return true
	}
	return false
}

// monsterAbility returns the monster's main special ability or nil.
func monsterAbility(d *content.MonsterDef) *content.AbilityDef {
	if d == nil || d.Ability == "" {
		return nil
	}
	return content.Ability(d.Ability)
}

func monsterAbilities(d *content.MonsterDef) []*content.AbilityDef {
	var out []*content.AbilityDef
	if a := monsterAbility(d); a != nil {
		out = append(out, a)
	}
	for _, k := range d.Abilities {
		if a := content.Ability(k); a != nil {
			out = append(out, a)
		}
	}
	return out
}

func (g *Game) defOf(m *Entity) *content.MonsterDef {
	if def := content.Monster(m.Monster.Def); def != nil {
		return def
	}
	return content.Monster("bandit")
}

// updateMonster runs one creature's AI; it returns true while the creature
// is busy fighting (NPC fighters only wander when it is not).
func (g *Game) updateMonster(m *Entity) bool {
	ms := m.Monster
	if g.summonExpired(m) {
		return true
	}
	if g.Now < ms.NextThink {
		return ms.Target != 0
	}
	ms.NextThink = g.Now + 80 + g.rng.Float64()*60
	def := g.defOf(m)
	l := g.Levels[m.Level]
	if hzDef := l.Def(m.Pos.X, m.Pos.Y); hzDef.Damage > 0 && g.TickN%10 == 0 {
		hz := single(hazardType(hzDef), hzDef.Damage/2)
		hz.DoT = true
		g.damage(nil, m, hz)
		if !m.Alive() || g.Entities[m.ID] != m {
			return false
		}
	}
	if m.stats.Stunned || def.Behavior == "decoy" {
		return true
	}

	target := g.chooseTarget(m, def)
	if target == nil {
		ms.Target = 0
		g.idle(m)
		return false
	}
	// leashes: summons stay by their owner, guards by their post
	if m.Owner != 0 {
		if o := g.Entities[m.Owner]; o != nil && m.Pos.Dist(o.Pos) > 12 {
			ms.Target = 0
			g.stepToward(m, o.Pos)
			return true
		}
	}
	if m.NPC != nil && m.NPC.Travel == (world.Pos{}) && m.Pos.Dist(ms.Home) > 14 {
		ms.Target, ms.State = 0, "return"
		g.stepToward(m, ms.Home)
		return true
	}
	if m.Faction == FMonster {
		g.maybeAskTactic(m, def, target)
	}

	tactic := ""
	if g.Now < ms.TacticUntil {
		tactic = ms.Tactic
	}
	dist := m.Pos.Manhattan(target.Pos)
	hp := m.HP / m.MaxHP
	role := roleOf(def)

	// fleeing: badly hurt (once per fight), broken morale or ordered to retreat
	if hp < 0.2 && !def.Boss && ms.State != "fled" && role != "leader" && m.Faction == FMonster && tactic != "aggressive" && g.chance(40) {
		ms.State = "fled"
		ms.FleeUntil = g.Now + 3500
	}
	if tactic == "retreat" && ms.FleeUntil < g.Now {
		ms.FleeUntil = g.Now + 2500
	}
	if g.Now < ms.FleeUntil {
		if !g.stepAway(m, target.Pos) && dist == 1 && g.Now >= m.NextAttack {
			g.meleeAttack(m, target)
		}
		return true
	}

	abilities := monsterAbilities(def)
	ready := func(a *content.AbilityDef) bool {
		return !m.stats.Silenced && g.Now >= m.Cooldowns[a.Key] && m.MP >= a.Mana
	}
	// healers mend wounded allies first
	for _, a := range abilities {
		if a.Kind == "heal" && ready(a) && g.woundedAlly(m, max(1, a.Radius)) {
			g.useAbility(m, a.Key, nil)
			return true
		}
	}
	// war cries and blessings when the fight starts
	for _, a := range abilities {
		if a.Kind == "buff" && ready(a) && g.Now-ms.BuffedAt > 12000 {
			ms.BuffedAt = g.Now
			g.useAbility(m, a.Key, nil)
			if a.Buff != nil {
				g.Say(m, "В бой!", 1500)
			}
			return true
		}
	}
	// stomps and auras when enemies are close
	for _, a := range abilities {
		if a.Kind == "nova" && ready(a) && m.Pos.Dist(target.Pos) <= max(1, a.Radius) && g.chance(45) {
			g.useAbility(m, a.Key, target)
			return true
		}
	}
	var attack *content.AbilityDef
	for _, a := range abilities {
		if (a.Kind == "projectile" || a.Kind == "chain" || a.Kind == "strike") && ready(a) {
			attack = a
			break
		}
	}
	var inRange bool
	if attack != nil {
		inRange = m.Pos.Dist(target.Pos) <= abilityRange(attack) && world.LOS(l, m.Pos, target.Pos)
		if attack.Kind == "strike" {
			inRange = dist == 1
		}
	}
	if tactic == "use_ability" && attack != nil && inRange {
		g.useAbility(m, attack.Key, target)
		return true
	}
	if tactic == "defensive" && role != "support" {
		role = "ranged"
	}
	if tactic == "aggressive" && (role == "ranged" || role == "caster") && dist <= 2 {
		role = "frontline"
	}

	switch role {
	case "ranged", "caster", "support":
		g.keepDistance(m, target, attack, inRange)
	case "skirmisher":
		g.skirmish(m, target, attack, inRange)
	default:
		if dist == 1 {
			if attack != nil && attack.Kind == "strike" && inRange && g.chance(50) {
				g.useAbility(m, attack.Key, target)
			} else if g.Now >= m.NextAttack {
				g.meleeAttack(m, target)
			}
			return true
		}
		if attack != nil && inRange && attack.Kind != "strike" && dist > 2 && g.chance(30) {
			g.useAbility(m, attack.Key, target)
			return true
		}
		goal := target.Pos
		if tactic == "flank" && dist > 2 {
			goal = g.flankGoal(m, target)
		}
		g.stepToward(m, goal)
	}
	return true
}

// keepDistance: archers and casters stay a few steps away, step back from
// melee and shoot when they can.
func (g *Game) keepDistance(m, t *Entity, attack *content.AbilityDef, inRange bool) {
	l := g.Levels[m.Level]
	dist := m.Pos.Manhattan(t.Pos)
	minD, maxD := 3, 5
	if attack != nil && attack.Kind != "strike" {
		maxD = max(minD, min(7, abilityRange(attack)-1))
	} else if a := monsterAbility(g.defOf(m)); a != nil {
		maxD = max(minD, min(7, abilityRange(a)-1))
	}
	if dist < minD {
		if g.stepAway(m, t.Pos) {
			return
		}
		if dist == 1 && g.Now >= m.NextAttack {
			g.meleeAttack(m, t)
		}
		return
	}
	if attack != nil && inRange {
		g.useAbility(m, attack.Key, t)
		return
	}
	los := world.LOS(l, m.Pos, t.Pos)
	if dist > maxD || !los {
		g.stepToward(m, t.Pos)
		return
	}
	if g.chance(15) {
		g.strafe(m, t, minD, maxD)
	}
}

// skirmish: get behind the enemy, strike and slip away when hurt.
func (g *Game) skirmish(m, t *Entity, attack *content.AbilityDef, inRange bool) {
	ms := m.Monster
	if m.HP/m.MaxHP < 0.35 && g.Now-ms.RetreatAt > 9000 && m.Faction == FMonster {
		ms.RetreatAt = g.Now
		ms.FleeUntil = g.Now + 1800
		return
	}
	dist := m.Pos.Manhattan(t.Pos)
	if dist == 1 {
		if attack != nil && attack.Kind == "strike" && inRange && g.chance(50) {
			g.useAbility(m, attack.Key, t)
		} else if g.Now >= m.NextAttack {
			g.meleeAttack(m, t)
		}
		return
	}
	if attack != nil && inRange && attack.Kind == "projectile" && dist > 2 && g.chance(25) {
		g.useAbility(m, attack.Key, t)
		return
	}
	g.stepToward(m, g.flankGoal(m, t))
}

// flankGoal is a cell next to the target, preferably behind it.
func (g *Game) flankGoal(m, t *Entity) world.Pos {
	l := g.Levels[t.Level]
	behind := world.DirNone
	if t.Facing != world.DirNone {
		behind = t.Facing.Opposite()
	}
	if t.Monster != nil {
		if tt := g.Entities[t.Monster.Target]; tt != nil {
			behind = world.DirTowards(tt.Pos, t.Pos)
		}
	}
	try := []world.Dir{behind}
	for _, d := range world.AllDirs {
		if d != behind && d != behind.Opposite() {
			try = append(try, d)
		}
	}
	for _, d := range try {
		if d == world.DirNone {
			continue
		}
		p := t.Pos.Add(d.Delta())
		if p == m.Pos || l.Free(p.X, p.Y) {
			return p
		}
	}
	return t.Pos
}

// strafe steps sideways while staying in the wanted distance band and in sight.
func (g *Game) strafe(m, t *Entity, minD, maxD int) {
	if g.Now < m.NextMove {
		return
	}
	l := g.Levels[m.Level]
	for _, i := range g.rng.Perm(4) {
		to := m.Pos.Add(world.AllDirs[i].Delta())
		d := to.Manhattan(t.Pos)
		if d >= minD && d <= maxD && g.canStep(l, to) && l.Def(to.X, to.Y).Damage == 0 && world.LOS(l, to, t.Pos) {
			g.monsterMove(m, to)
			return
		}
	}
}

// woundedAlly reports whether some ally (or the creature itself) within r
// needs healing.
func (g *Game) woundedAlly(m *Entity, r int) bool {
	for _, o := range g.onLevel(m.Level) {
		if o.Alive() && o.Blocks() && o.MaxHP > 0 && o.HP/o.MaxHP < 0.65 && m.Pos.Dist(o.Pos) <= r && g.friendly(m, o) {
			return true
		}
	}
	return false
}

// chooseTarget keeps or picks the enemy to fight.
func (g *Game) chooseTarget(m *Entity, def *content.MonsterDef) *Entity {
	ms := m.Monster
	if tt := m.stats.Taunter; tt != 0 {
		if t := g.Entities[tt]; t != nil && t.Level == m.Level && g.hostile(m, t) {
			ms.Target, ms.LastSeen, ms.LastSeenAt = tt, t.Pos, g.Now
			return t
		}
	}
	if t := g.validTarget(m, def); t != nil {
		if g.Now >= ms.Retarget {
			ms.Retarget = g.Now + 1500 + g.rng.Float64()*1000
			if b := g.betterTarget(m, def, t); b != nil && b != t {
				ms.Target, ms.LastSeen, ms.LastSeenAt = b.ID, b.Pos, g.Now
				return b
			}
		}
		return t
	}
	if ms.Target != 0 {
		return nil // still chasing the last known position
	}
	return g.acquireTarget(m, def)
}

func (g *Game) validTarget(m *Entity, def *content.MonsterDef) *Entity {
	ms := m.Monster
	if ms.Target == 0 {
		return nil
	}
	t := g.Entities[ms.Target]
	if t == nil || t.Level != m.Level || !g.hostile(m, t) {
		ms.Target = 0
		return nil
	}
	l := g.Levels[m.Level]
	if m.Pos.Dist(t.Pos) <= def.Sight+4 && world.LOS(l, m.Pos, t.Pos) && g.canSee(m, t) {
		ms.LastSeen = t.Pos
		ms.LastSeenAt = g.Now
		return t
	}
	// lost sight: chase the last known position for a while
	if g.Now-ms.LastSeenAt > 5000 || m.Pos.Dist(t.Pos) > def.Sight*3 || t.stats.Stealthed {
		ms.Target = 0
		ms.State = "return"
		ms.Tactic = ""
		return nil
	}
	if m.Pos == ms.LastSeen {
		ms.LastSeenAt -= 1000
		return nil
	}
	g.stepToward(m, ms.LastSeen)
	return nil
}

// targetScore: lower is better. Skirmishers and shooters hunt the weakest,
// the frontline takes whoever is closest.
func (g *Game) targetScore(m *Entity, def *content.MonsterDef, o *Entity) float64 {
	d := float64(m.Pos.Dist(o.Pos))
	switch roleOf(def) {
	case "skirmisher", "ranged", "caster":
		return d*0.4 + 10*o.HP/math.Max(1, o.MaxHP)
	}
	return d
}

func (g *Game) betterTarget(m *Entity, def *content.MonsterDef, cur *Entity) *Entity {
	l := g.Levels[m.Level]
	switch roleOf(def) {
	case "frontline", "leader":
		// protect our archers and casters from enemies who reached them
		for _, o := range g.onLevel(m.Level) {
			if o == cur || !g.hostile(m, o) || m.Pos.Dist(o.Pos) > 6 {
				continue
			}
			for _, a := range g.onLevel(m.Level) {
				if a.Monster != nil && a != m && g.friendly(m, a) && a.Pos.Manhattan(o.Pos) == 1 && squishy(g.defOf(a)) {
					return o
				}
			}
		}
	case "skirmisher", "ranged", "caster":
		best, bs := cur, g.targetScore(m, def, cur)-2
		for _, o := range g.onLevel(m.Level) {
			if !g.hostile(m, o) || m.Pos.Dist(o.Pos) > def.Sight || !g.canSee(m, o) || !world.LOS(l, m.Pos, o.Pos) {
				continue
			}
			if s := g.targetScore(m, def, o); s < bs {
				best, bs = o, s
			}
		}
		return best
	}
	return nil
}

func (g *Game) acquireTarget(m *Entity, def *content.MonsterDef) *Entity {
	l := g.Levels[m.Level]
	sight := def.Sight
	if l.Lit && g.IsNight() && !def.Night && m.Faction == FMonster {
		sight = max(3, sight-2)
	}
	var best *Entity
	bs := math.MaxFloat64
	for _, o := range g.onLevel(m.Level) {
		if !g.hostile(m, o) || !g.canSee(m, o) {
			continue
		}
		d := m.Pos.Dist(o.Pos)
		if d > sight || (d > 2 && !world.LOS(l, m.Pos, o.Pos)) {
			continue
		}
		if s := g.targetScore(m, def, o); s < bs {
			best, bs = o, s
		}
	}
	if best == nil {
		return nil
	}
	ms := m.Monster
	ms.Target = best.ID
	ms.LastSeen = best.Pos
	ms.LastSeenAt = g.Now
	ms.State = "chase"
	ms.Path = nil
	// packs and squads alert each other
	for _, o := range g.onLevel(m.Level) {
		if o.Monster == nil || o == m || o.Monster.Target != 0 || !g.friendly(m, o) || o.Owner != 0 {
			continue
		}
		near := o.Pos.Dist(m.Pos) <= 6 || (ms.Squad != 0 && o.Monster.Squad == ms.Squad && o.Pos.Dist(m.Pos) <= 14)
		if near && m.Faction == FMonster {
			o.Monster.Target = best.ID
			o.Monster.LastSeen = best.Pos
			o.Monster.LastSeenAt = g.Now
			o.Monster.State = "chase"
		}
	}
	return best
}

// idle: summons follow their owner, monsters roam near home.
func (g *Game) idle(m *Entity) {
	if m.Owner != 0 {
		if o := g.Entities[m.Owner]; o != nil && m.Pos.Dist(o.Pos) > 2 {
			g.stepToward(m, o.Pos)
		}
		return
	}
	if m.NPC != nil {
		return
	}
	g.monsterIdle(m)
}

func (g *Game) monsterIdle(m *Entity) {
	ms := m.Monster
	if m.HP < m.MaxHP {
		m.HP = math.Min(m.MaxHP, m.HP+m.MaxHP*0.002)
	}
	if ms.State == "return" || ms.State == "fled" {
		if m.Pos.Dist(ms.Home) > 2 {
			g.stepToward(m, ms.Home)
			return
		}
		ms.State = "idle"
	}
	if g.Now >= m.NextMove && g.chance(6) {
		d := world.AllDirs[g.rng.IntN(4)]
		to := m.Pos.Add(d.Delta())
		l := g.Levels[m.Level]
		if l.Free(to.X, to.Y) && to.Dist(ms.Home) <= 6 && l.Def(to.X, to.Y).Damage == 0 && l.Def(to.X, to.Y).Interact == "" {
			g.monsterMove(m, to)
		}
	}
}

func (g *Game) monsterMove(m *Entity, to world.Pos) {
	l := g.Levels[m.Level]
	m.Facing = world.DirTowards(m.Pos, to)
	g.moveEntity(m, to)
	m.NextMove = g.Now + m.stats.MoveMs*l.Def(to.X, to.Y).MoveCost
}

// stepToward moves one step along an A* path (recomputed when needed).
func (g *Game) stepToward(m *Entity, goal world.Pos) bool {
	if g.Now < m.NextMove || m.Pos == goal {
		return false
	}
	l := g.Levels[m.Level]
	ms := m.Monster
	if ms != nil {
		if len(ms.Path) == 0 || ms.PathGoal != goal || !g.canStep(l, ms.Path[0]) || ms.Path[0].Manhattan(m.Pos) != 1 {
			ms.Path = world.FindPath(l.W, l.H, m.Pos, goal, 500, func(x, y int) float64 {
				def := l.Def(x, y)
				if def.Interact == "door" {
					return 2 // monsters can open doors
				}
				if !def.Walkable || def.Interact == "stairs_up" || def.Interact == "stairs_down" || def.Interact == "dungeon" {
					return -1
				}
				c := def.MoveCost
				if def.Damage > 0 {
					c += 15
				}
				if l.Occupant(x, y) != 0 {
					c += 6
				}
				return c
			})
			ms.PathGoal = goal
		}
		if len(ms.Path) > 0 {
			next := ms.Path[0]
			if def := l.Def(next.X, next.Y); def.Interact == "door" && def.Becomes != "" && l.Occupant(next.X, next.Y) == 0 {
				g.SetTile(l, next.X, next.Y, content.TileID(def.Becomes))
				m.NextMove = g.Now + m.stats.MoveMs*2
				return true
			}
			if next != goal && g.canStep(l, next) {
				ms.Path = ms.Path[1:]
				g.monsterMove(m, next)
				return true
			}
		}
	}
	// greedy fallback
	best, bd := world.Pos{}, math.MaxInt
	for _, d := range world.AllDirs {
		to := m.Pos.Add(d.Delta())
		if !g.canStep(l, to) {
			continue
		}
		if dd := to.Manhattan(goal); dd < bd {
			best, bd = to, dd
		}
	}
	if bd < m.Pos.Manhattan(goal) {
		g.monsterMove(m, best)
		return true
	}
	return false
}

func (g *Game) canStep(l *world.Level, p world.Pos) bool {
	if !l.Free(p.X, p.Y) {
		return false
	}
	def := l.Def(p.X, p.Y)
	return def.Interact == "" || def.Interact == "door"
}

// stepAway moves to the free neighbour that maximizes distance from a threat.
func (g *Game) stepAway(m *Entity, from world.Pos) bool {
	if g.Now < m.NextMove {
		return true
	}
	l := g.Levels[m.Level]
	best, bd := world.Pos{}, m.Pos.DistSq(from)
	for _, d := range world.AllDirs {
		to := m.Pos.Add(d.Delta())
		if !g.canStep(l, to) || l.Def(to.X, to.Y).Damage > 0 {
			continue
		}
		if dd := to.DistSq(from); dd > bd {
			best, bd = to, dd
		}
	}
	if bd > m.Pos.DistSq(from) {
		g.monsterMove(m, best)
		return true
	}
	return false
}

// ---- LLM tactics for elite enemies ----
// ---- LLM tactics for elite enemies ----

func (g *Game) maybeAskTactic(m *Entity, def *content.MonsterDef, target *Entity) {
	ms := m.Monster
	if !def.Elite || !g.Brain.Enabled() || ms.LLMPending || g.Now < ms.NextLLM {
		return
	}
	ms.NextLLM = g.Now + 7000
	var allies []string
	for _, o := range g.onLevel(m.Level) {
		if o.Monster != nil && o != m && o.Faction == m.Faction && o.Pos.Dist(m.Pos) <= 10 {
			allies = append(allies, o.Name)
		}
	}
	if len(allies) > 8 {
		allies = append(allies[:8], "...")
	}
	req := llm.TacticRequest{
		Name: m.Name, Persona: def.Persona, Boss: def.Boss,
		HPPct: int(100 * m.HP / m.MaxHP), Allies: allies,
		Enemy: target.Name, EnemyHPPct: int(100 * target.HP / math.Max(1, target.MaxHP)),
		Distance: m.Pos.Manhattan(target.Pos), Events: ms.Events,
	}
	if target.Player != nil {
		req.EnemyLevel, req.Lang = target.Player.Level, target.Player.Lang
		if c := content.Class(target.Player.Class); c != nil {
			req.EnemyClass = c.Name
		}
	}
	if ab := monsterAbility(def); ab != nil {
		req.AbilityName = ab.Name
	}
	ms.Events = nil
	id := m.ID
	if g.Brain.Tactic(req, func(r llm.TacticReply, err error) {
		g.Post(func() {
			m2 := g.Entities[id]
			if m2 == nil || m2.Monster == nil {
				return
			}
			m2.Monster.LLMPending = false
			if err == nil {
				g.applyTactic(m2, r)
			}
		})
	}) {
		ms.LLMPending = true
	}
}

func (g *Game) applyTactic(m *Entity, r llm.TacticReply) {
	ms := m.Monster
	if !slices.Contains(llm.Tactics, r.Tactic) {
		return
	}
	ms.Tactic = r.Tactic
	ms.TacticUntil = g.Now + 8000
	if say := strings.TrimSpace(r.Say); say != "" {
		if rs := []rune(say); len(rs) > 90 {
			say = string(rs[:90]) + "…"
		}
		g.Say(m, say, 4500)
		for _, p := range g.playersOn(m.Level) {
			if p.Pos.Dist(m.Pos) <= 18 {
				g.Log(p, "#ff9a6a", "%s: «%s»", m.Name, say)
			}
		}
	}
	if r.Tactic == "call_allies" {
		for _, o := range g.onLevel(m.Level) {
			if o.Monster != nil && o != m && o.Faction == m.Faction && o.Pos.Dist(m.Pos) <= 16 {
				o.Monster.Target = ms.Target
				o.Monster.LastSeen = ms.LastSeen
				o.Monster.LastSeenAt = g.Now
				o.Monster.State = "chase"
				g.FX(o.Level, o.Pos, "!", 0, "#ff4a4a", 800)
			}
		}
	}
}

// ---- NPCs ----

func (g *Game) updateNPC(e *Entity) {
	n := e.NPC
	talking := false
	for _, p := range g.Online {
		if p.Player.Talking == e.ID {
			e.Facing = world.DirTowards(e.Pos, p.Pos)
			talking = true
		}
	}
	if e.Monster != nil && !talking && g.updateMonster(e) {
		return // fighting
	}
	role := content.NPCRole(n.Role)
	if role != nil && role.World && !talking {
		g.travel(e)
	}
	if g.Now < n.NextThink || talking {
		return
	}
	n.NextThink = g.Now + 1200 + g.rng.Float64()*2500
	if role == nil {
		return
	}
	if n.NextChat == 0 {
		n.NextChat = g.Now + 5000 + g.rng.Float64()*40000
	}
	if g.Now >= n.NextChat && len(role.Lines) > 0 {
		for _, p := range g.playersOn(e.Level) {
			if p.Pos.Dist(e.Pos) <= 6 {
				g.Say(e, role.Lines[g.rng.IntN(len(role.Lines))], 4000)
				n.NextChat = g.Now + 30000 + g.rng.Float64()*30000
				break
			}
		}
	}
	if role.World || role.Wander <= 0 || g.Now < e.NextMove {
		return
	}
	// citizens keep a schedule: back to their spot when far from it
	if spot := g.scheduleSpot(e); e.Pos.Dist(spot) > role.Wander+1 {
		n.NextThink = g.Now + 300
		if g.walkToward(e, spot) {
			return
		}
	}
	if !g.chance(55) {
		return
	}
	l := g.Levels[e.Level]
	d := world.AllDirs[g.rng.IntN(4)]
	to := e.Pos.Add(d.Delta())
	def := l.Def(to.X, to.Y)
	if l.Free(to.X, to.Y) && def.Interact == "" && to.Dist(g.scheduleSpot(e)) <= role.Wander {
		e.Facing = d
		g.moveEntity(e, to)
		e.NextMove = g.Now + e.stats.MoveMs
	}
}
