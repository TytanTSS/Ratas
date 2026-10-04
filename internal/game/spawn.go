package game

import (
	"math"
	"math/rand/v2"
	"slices"

	"ratas/internal/content"
	"ratas/internal/gen"
	"ratas/internal/world"
)

func (g *Game) newMonster(def *content.MonsterDef, level string, p world.Pos, lvl int) *Entity {
	ms, hp := monsterState(def, lvl, p)
	e := &Entity{
		Kind: KMonster, Name: def.Name, Glyph: def.Glyph, Color: def.Color, Level: level, Pos: p,
		Faction: FMonster, HP: hp, MaxHP: hp, Facing: world.DirDown, Monster: ms,
	}
	if def.Ally {
		e.Faction = FPlayer
	}
	return g.Spawn(e)
}

func (g *Game) spawnNPC(n gen.NPCSpawn, village string, r *rand.Rand) {
	role := content.NPCRole(n.Role)
	if role == nil {
		return
	}
	l := g.Levels["overworld"]
	p := n.Pos
	if !l.Free(p.X, p.Y) {
		p = findFree(l, p)
	}
	e := g.makeNPC(role, n.Name, village, p, g.overworldLevelAt(p, false)+1, 30+r.IntN(70))
	e.NPC.Night = n.Night
}

// pickMonster chooses a weighted random non-boss monster matching theme and depth.
func pickMonster(r *rand.Rand, theme string, depth int, night bool, allowElite bool) *content.MonsterDef {
	var pool []*content.MonsterDef
	total := 0
	for i := range content.Monsters() {
		m := &content.Monsters()[i]
		if m.Boss || m.Ally || m.Weight <= 0 || (m.Elite && !allowElite) || !slices.Contains(m.Themes, theme) {
			continue
		}
		// depth [0,0]: overworld only; [0,n]: overworld and dungeon floors 1..n
		if depth == 0 {
			if m.Depth[0] != 0 || (m.Night && !night) {
				continue
			}
		} else if depth < max(1, m.Depth[0]) || depth > m.Depth[1] {
			continue
		}
		pool = append(pool, m)
		total += m.Weight
	}
	if total == 0 {
		return nil
	}
	n := r.IntN(total)
	for _, m := range pool {
		n -= m.Weight
		if n < 0 {
			return m
		}
	}
	return pool[len(pool)-1]
}

func (g *Game) inVillage(p world.Pos, margin int) bool {
	for _, v := range g.Villages {
		a := v.Area
		if p.X >= a.X-margin && p.Y >= a.Y-margin/2 && p.X < a.X+a.W+margin && p.Y < a.Y+a.H+margin/2 {
			return true
		}
	}
	return false
}

func (g *Game) overworldLevelAt(p world.Pos, night bool) int {
	lvl := 1 + p.Manhattan(g.Start)/70
	if night {
		lvl++
	}
	if l := g.Levels["overworld"]; l != nil {
		lvl += biomeDanger[l.Def(p.X, p.Y).Biome]
	}
	return lvl
}

// pickSquad chooses a weighted random squad for a theme and depth.
func pickSquad(r *rand.Rand, theme string, depth int, night bool) *content.SquadDef {
	var pool []*content.SquadDef
	total := 0
	for i := range content.Squads() {
		sq := &content.Squads()[i]
		if sq.Weight <= 0 || !slices.Contains(sq.Themes, theme) {
			continue
		}
		if depth == 0 {
			if sq.Depth[0] != 0 || (sq.Night && !night) {
				continue
			}
		} else if depth < max(1, sq.Depth[0]) || depth > sq.Depth[1] {
			continue
		}
		pool = append(pool, sq)
		total += sq.Weight
	}
	if total == 0 {
		return nil
	}
	n := r.IntN(total)
	for _, sq := range pool {
		n -= sq.Weight
		if n < 0 {
			return sq
		}
	}
	return pool[len(pool)-1]
}

// spawnSquad places a mixed squad around p: the frontline in front, the
// archers and casters a little behind.
func (g *Game) spawnSquad(r *rand.Rand, sq *content.SquadDef, l *world.Level, p world.Pos, lvl int) []*Entity {
	g.squadSeq++
	var out []*Entity
	for _, mem := range sq.Members {
		def := content.Monster(mem.Monster)
		if def == nil {
			continue
		}
		n := gen.Range(r, mem.Count[0], mem.Count[1])
		for i := 0; i < n; i++ {
			at := p
			if squishy(def) {
				at = p.Add(world.Pos{X: r.IntN(5) - 2, Y: 2})
			}
			q := findFree(l, at)
			if q.Dist(p) > 6 {
				continue
			}
			m := g.newMonster(def, l.ID, q, lvl)
			m.Monster.Squad = g.squadSeq
			m.Monster.Home = p
			out = append(out, m)
		}
	}
	return out
}

// spawnGroup places a pack around p.
func (g *Game) spawnGroup(r *rand.Rand, def *content.MonsterDef, l *world.Level, p world.Pos, lvl int) []*Entity {
	n := gen.Range(r, max(1, def.Group[0]), max(1, def.Group[1]))
	var out []*Entity
	for i := 0; i < n; i++ {
		q := p
		if !l.Free(q.X, q.Y) || l.Def(q.X, q.Y).Interact != "" {
			q = findFree(l, p)
			if q.Dist(p) > 4 {
				continue
			}
		}
		out = append(out, g.newMonster(def, l.ID, q, lvl))
	}
	return out
}

// populateOverworld places persistent elite enemies across the map: every
// kind of elite at least once in a fitting biome, more if there is room.
func (g *Game) populateOverworld(r *rand.Rand) {
	l := g.Levels["overworld"]
	var elites []*content.MonsterDef
	for i := range content.Monsters() {
		m := &content.Monsters()[i]
		if m.Elite && !m.Boss && m.Depth[0] == 0 {
			elites = append(elites, m)
		}
	}
	placed := 0
	for round := 0; round < 2 && placed < 8; round++ {
		for _, def := range elites {
			for tries := 0; tries < 1500; tries++ {
				p := world.Pos{X: r.IntN(l.W), Y: r.IntN(l.H)}
				biome := l.Def(p.X, p.Y).Biome
				if !l.Free(p.X, p.Y) || !slices.Contains(def.Themes, biome) || g.inVillage(p, 25) || p.Manhattan(g.Start) < 60 {
					continue
				}
				g.newMonster(def, l.ID, p, g.overworldLevelAt(p, false)+1)
				// guards
				if minion := pickMonster(r, biome, 0, false, false); minion != nil {
					g.spawnGroup(r, minion, l, p, g.overworldLevelAt(p, false))
				}
				placed++
				break
			}
		}
	}
}

// populateLandmarks puts guards into bandit camps and the dead into graveyards.
func (g *Game) populateLandmarks(r *rand.Rand) {
	l := g.Levels["overworld"]
	for _, lm := range g.Landmarks {
		var keys []string
		switch lm.Kind {
		case "camp":
			keys = []string{"bandit_chief", "bandit", "bandit_archer"}
		case "graveyard":
			keys = []string{"wandering_skeleton", "ghost"}
		default:
			continue
		}
		lvl := g.overworldLevelAt(lm.Pos, false)
		for _, k := range keys {
			def := content.Monster(k)
			if def == nil {
				continue
			}
			for _, m := range g.spawnGroup(r, def, l, lm.Pos.Add(world.Pos{X: r.IntN(5) - 2, Y: 1}), lvl) {
				m.Monster.Persistent = true
			}
		}
	}
}

func (g *Game) populateDungeon(f *gen.DungeonFloor, ent gen.Entrance) {
	l := f.Level
	r := gen.RNG(g.Seed, "pop-"+l.ID)
	lvl := l.Depth + max(0, ent.MaxDepth-3)
	for _, p := range f.Monsters {
		if r.IntN(100) < 30 {
			if sq := pickSquad(r, ent.Theme, l.Depth, false); sq != nil {
				g.spawnSquad(r, sq, l, p, lvl)
				continue
			}
		}
		def := pickMonster(r, ent.Theme, l.Depth, false, true)
		if def == nil {
			continue
		}
		g.spawnGroup(r, def, l, p, lvl)
	}
	if f.Boss != nil {
		var bosses []*content.MonsterDef
		for i := range content.Monsters() {
			m := &content.Monsters()[i]
			if m.Boss && slices.Contains(m.Themes, ent.Theme) {
				bosses = append(bosses, m)
			}
		}
		if len(bosses) > 0 {
			def := bosses[r.IntN(len(bosses))]
			bp := *f.Boss
			if !l.Free(bp.X, bp.Y) {
				bp = findFree(l, bp)
			}
			g.newMonster(def, l.ID, bp, lvl+1)
			if minion := pickMonster(r, ent.Theme, l.Depth, false, false); minion != nil {
				g.spawnGroup(r, minion, l, bp.Add(world.Pos{X: 0, Y: 2}), lvl)
			}
		}
	}
	for _, p := range f.Items {
		if r.IntN(3) == 0 {
			g.dropGold(l.ID, p, 5+r.IntN(10*l.Depth+5))
			continue
		}
		if st, ok := g.randomItem(l.Depth, 20); ok {
			g.dropItem(l.ID, p, st)
		}
	}
}

// runSpawner keeps the overworld around players alive and removes far monsters.
func (g *Game) runSpawner() {
	ow := g.Levels["overworld"]
	if ow == nil {
		return
	}
	var players []*Entity
	for _, p := range g.Online {
		if p.Level == "overworld" && !p.Dead {
			players = append(players, p)
		}
	}
	if len(players) == 0 {
		return
	}
	night := g.IsNight()
	// despawn
	for _, e := range g.onLevel("overworld") {
		if e.Monster == nil || e.Faction != FMonster || e.Monster.Persistent || e.Monster.Target != 0 {
			continue
		}
		far := true
		for _, p := range players {
			if p.Pos.Dist(e.Pos) < 70 {
				far = false
				break
			}
		}
		if far {
			g.Remove(e)
		}
	}
	want := 6
	if night {
		want = 10
	}
	for _, p := range players {
		near := 0
		for _, e := range g.onLevel("overworld") {
			if e.Monster != nil && e.Faction == FMonster && e.Pos.Dist(p.Pos) <= 32 {
				near++
			}
		}
		if near >= want {
			continue
		}
		for tries := 0; tries < 25; tries++ {
			ang := g.rng.Float64() * 6.283
			dist := 22 + g.rng.Float64()*12
			q := world.Pos{X: p.Pos.X + int(dist*math.Cos(ang)), Y: p.Pos.Y + int(dist*math.Sin(ang)/2)}
			if !ow.Free(q.X, q.Y) || ow.Def(q.X, q.Y).Interact != "" || g.inVillage(q, 6) {
				continue
			}
			tooClose := false
			for _, o := range players {
				if o.Pos.Dist(q) < 16 {
					tooClose = true
				}
			}
			if tooClose {
				continue
			}
			biome := ow.Def(q.X, q.Y).Biome
			if g.chance(30) {
				if sq := pickSquad(g.rng, biome, 0, night); sq != nil {
					g.spawnSquad(g.rng, sq, ow, q, g.overworldLevelAt(q, night))
					break
				}
			}
			def := pickMonster(g.rng, biome, 0, night, false)
			if def == nil {
				continue
			}
			g.spawnGroup(g.rng, def, ow, q, g.overworldLevelAt(q, night))
			break
		}
	}
}
