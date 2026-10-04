package game

import (
	"math/rand/v2"

	"ratas/internal/content"
	"ratas/internal/gen"
	"ratas/internal/world"
)

// pendingNPC is a fallen guard or wanderer waiting to return.
type pendingNPC struct {
	e  *Entity
	at float64
}

// monsterState builds the combat state of a creature of a level.
func monsterState(def *content.MonsterDef, lvl int, home world.Pos) (*MonsterState, float64) {
	lvl = max(1, lvl)
	hpScale := 1 + 0.25*float64(lvl-1)
	dmgScale := monsterScale(lvl)
	return &MonsterState{
		Def: def.Key, Lvl: lvl, Home: home, State: "idle",
		Damage:     [2]float64{def.Damage[0] * dmgScale, def.Damage[1] * dmgScale},
		Armor:      def.Armor + float64(lvl-1)/2,
		MoveMs:     float64(def.MoveMs),
		AttackMs:   float64(def.AttackMs),
		XP:         int(float64(def.XP) * (1 + 0.3*float64(lvl-1))),
		Persistent: def.Boss || def.Elite,
	}, def.HP * hpScale
}

// makeNPC creates a villager, guard or wanderer; NPCs with a combat profile
// fight monsters on the players' side.
func (g *Game) makeNPC(role *content.NPCRoleDef, name, village string, p world.Pos, lvl int, gold int) *Entity {
	e := &Entity{
		Kind: KNPC, Name: name + " (" + role.Name + ")", Glyph: role.Glyph, Color: role.Color,
		Level: "overworld", Pos: p, Faction: FNeutral, HP: 50, MaxHP: 50, Facing: world.DirDown,
		NPC: &NPCState{Role: role.Key, PName: name, Home: p, Village: village, Gold: gold},
	}
	if def := content.Monster(role.Combat); def != nil {
		ms, hp := monsterState(def, lvl, p)
		ms.Persistent = true
		e.Monster, e.Faction, e.HP, e.MaxHP = ms, FPlayer, hp, hp
	}
	return g.Spawn(e)
}

// populateWanderers places knights, hunters, pilgrims and others in the wild.
func (g *Game) populateWanderers(r *rand.Rand) {
	ow := g.Levels["overworld"]
	for i := range content.NPCRoles() {
		role := &content.NPCRoles()[i]
		if !role.World {
			continue
		}
		n := gen.Range(r, role.Count[0], role.Count[1])
		for k := 0; k < n; k++ {
			p, ok := g.wildSpot(r, ow, 12, 60)
			if !ok {
				continue
			}
			g.makeNPC(role, gen.PersonName(r), "", p, g.overworldLevelAt(p, false)+2, 20+r.IntN(60))
		}
	}
}

// wildSpot finds free land outside villages, between minD and maxD steps
// from some village (or anywhere when there are none).
func (g *Game) wildSpot(r *rand.Rand, l *world.Level, minD, maxD int) (world.Pos, bool) {
	for tries := 0; tries < 400; tries++ {
		var p world.Pos
		if len(g.Villages) > 0 {
			v := g.Villages[r.IntN(len(g.Villages))].Center
			p = world.Pos{X: v.X + r.IntN(2*maxD+1) - maxD, Y: v.Y + (r.IntN(2*maxD+1)-maxD)/2}
			if p.Manhattan(v) < minD {
				continue
			}
		} else {
			p = world.Pos{X: r.IntN(l.W), Y: r.IntN(l.H)}
		}
		if l.Free(p.X, p.Y) && l.Def(p.X, p.Y).Interact == "" && l.Def(p.X, p.Y).Damage == 0 && !g.inVillage(p, 4) {
			return p, true
		}
	}
	return world.Pos{}, false
}

// travel moves a wandering NPC between villages and landmarks.
func (g *Game) travel(e *Entity) {
	n := e.NPC
	if g.Now < e.NextMove {
		return
	}
	if n.Travel == (world.Pos{}) || e.Pos.Dist(n.Travel) <= 2 || g.Now > n.TravelUntil {
		if e.Pos.Dist(n.Travel) <= 2 && g.chance(70) {
			n.TravelUntil = g.Now + 4000 + g.rng.Float64()*8000 // rest a little
			n.Travel = world.Pos{}
			if g.Now < n.TravelUntil {
				return
			}
		}
		var goals []world.Pos
		for _, v := range g.Villages {
			goals = append(goals, world.Pos{X: v.Center.X, Y: v.Center.Y + 3})
		}
		for _, lm := range g.Landmarks {
			goals = append(goals, lm.Pos)
		}
		if len(goals) == 0 {
			return
		}
		n.Travel = goals[g.rng.IntN(len(goals))]
		n.TravelUntil = g.Now + 240000
		n.Home = n.Travel
	}
	if e.Monster != nil {
		g.stepToward(e, n.Travel)
		e.NextMove += 140 // a travelling pace, slower than a charge
	}
}

// killAlly removes a fallen ally; guards and wanderers return later.
func (g *Game) killAlly(e *Entity) {
	g.Remove(e)
	g.FX(e.Level, e.Pos, "", '%', "#a03030", 1200)
	if e.Owner != 0 {
		if o := g.Entities[e.Owner]; o != nil && e.Expires > g.Now+500 {
			g.Log(o, "#c0c0c0", "%s пал.", e.Name)
		}
		return
	}
	if e.NPC != nil {
		for _, p := range g.playersOn(e.Level) {
			if p.Pos.Dist(e.Pos) <= 20 {
				g.Log(p, "#ff8a8a", "%s пал в бою.", e.Name)
			}
		}
		g.revivals = append(g.revivals, pendingNPC{e: e, at: g.Now + 180000})
	}
}

// reviveNPCs brings fallen NPCs back to their homes.
func (g *Game) reviveNPCs() {
	kept := g.revivals[:0]
	for _, pn := range g.revivals {
		if g.Now < pn.at {
			kept = append(kept, pn)
			continue
		}
		e := pn.e
		l := g.Levels[e.Level]
		if l == nil {
			continue
		}
		e.Dead, e.HP, e.Buffs = false, e.MaxHP, nil
		e.Pos = findFree(l, e.NPC.Home)
		if e.Monster != nil {
			e.Monster.Target, e.Monster.State = 0, "idle"
		}
		g.Spawn(e)
	}
	g.revivals = kept
}

// summonExpired removes summons and illusions whose time is up or whose
// owner is gone.
func (g *Game) summonExpired(e *Entity) bool {
	if e.Owner == 0 {
		return false
	}
	o := g.Entities[e.Owner]
	if (e.Expires > 0 && g.Now >= e.Expires) || o == nil || o.Level != e.Level || (o.Player != nil && o.Dead) {
		g.FX(e.Level, e.Pos, "", '*', "#c0a0ff", 400)
		g.Remove(e)
		return true
	}
	return false
}
