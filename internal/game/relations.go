package game

import (
	"slices"
)

// controller is the entity responsible for e: the owner of a summon, the
// owner of a projectile, or e itself.
func (g *Game) controller(e *Entity) *Entity {
	if e == nil {
		return nil
	}
	if e.Kind == KProjectile && e.Proj != nil {
		if o := g.Entities[e.Proj.Owner]; o != nil {
			return g.controller(o)
		}
		return e
	}
	if e.Owner != 0 {
		if o := g.Entities[e.Owner]; o != nil && o != e {
			return o
		}
	}
	return e
}

// hostile reports whether a and b fight each other: monsters against players
// and their allies; players against players outside of parties and safe
// villages when PvP is on.
func (g *Game) hostile(a, b *Entity) bool {
	if a == nil || b == nil || a == b || !a.Alive() || !b.Alive() {
		return false
	}
	fa, fb := a.Faction, b.Faction
	if fa == FNeutral || fb == FNeutral {
		return false
	}
	if fa != fb {
		return true
	}
	if fa == FMonster {
		return false
	}
	pa, pb := g.controller(a), g.controller(b)
	return pa != pb && pa.Player != nil && pb.Player != nil && g.pvp(pa, pb)
}

// friendly reports whether c may heal or bless o (alive or not).
func (g *Game) friendly(c, o *Entity) bool {
	if c == o {
		return true
	}
	if c.Faction != o.Faction || c.Faction == FNeutral {
		return false
	}
	if c.Faction == FMonster {
		return true
	}
	pa, pb := g.controller(c), g.controller(o)
	return pa == pb || pa.Player == nil || pb.Player == nil || !g.pvp(pa, pb)
}

// pvp: can these two players hurt each other?
func (g *Game) pvp(a, b *Entity) bool {
	if !g.PvP || a.Level != b.Level || g.sameParty(a, b) {
		return false
	}
	return !g.safeZone(a) && !g.safeZone(b)
}

// safeZone: villages are safe from other players.
func (g *Game) safeZone(e *Entity) bool {
	return e.Level == "overworld" && g.inVillage(e.Pos, 2)
}

// onLevel lists the entities of a level (rebuilt every tick).
func (g *Game) onLevel(level string) []*Entity {
	if g.byLevel == nil {
		g.indexLevels()
	}
	return g.byLevel[level]
}

func (g *Game) indexLevels() {
	if g.byLevel == nil {
		g.byLevel = map[string][]*Entity{}
	}
	for k := range g.byLevel {
		g.byLevel[k] = g.byLevel[k][:0]
	}
	for _, e := range g.Entities {
		g.byLevel[e.Level] = append(g.byLevel[e.Level], e)
	}
}

// ---- parties ----

// MaxParty is the largest party.
const MaxParty = 6

// Party is a group of players who do not hurt each other and see each other's state.
type Party struct {
	ID      int
	Leader  string
	Members []string
}

func (g *Game) partyOf(e *Entity) *Party {
	if e == nil || e.Player == nil || e.Player.PartyID == 0 {
		return nil
	}
	return g.parties[e.Player.PartyID]
}

func (g *Game) sameParty(a, b *Entity) bool {
	pa := g.partyOf(a)
	return pa != nil && pa == g.partyOf(b)
}

// PartyMembers returns the online members of e's party (including e).
func (g *Game) PartyMembers(e *Entity) []*Entity {
	pt := g.partyOf(e)
	if pt == nil {
		return nil
	}
	var out []*Entity
	for _, n := range pt.Members {
		if m := g.Online[n]; m != nil {
			out = append(out, m)
		}
	}
	return out
}

func (g *Game) partyLog(pt *Party, color, format string, args ...any) {
	for _, n := range pt.Members {
		if m := g.Online[n]; m != nil {
			g.Log(m, color, format, args...)
		}
	}
}

func (g *Game) partyInvite(e *Entity, name string) {
	t := g.Online[name]
	switch {
	case t == nil || t == e:
		g.Log(e, "#ff8080", "Такого игрока нет в мире.")
		return
	case g.sameParty(e, t):
		g.Log(e, "#ff8080", "%s уже в вашей группе.", name)
		return
	}
	if pt := g.partyOf(e); pt != nil && len(pt.Members) >= MaxParty {
		g.Log(e, "#ff8080", "Группа полна (%d).", MaxParty)
		return
	}
	if t.Player.Invites == nil {
		t.Player.Invites = map[string]float64{}
	}
	t.Player.Invites[e.Name] = g.Now
	t.Player.Dirty = true
	g.Log(t, "#80ffa0", "%s приглашает вас в группу. Окно группы — G.", e.Name)
	g.Log(e, "#80ffa0", "Приглашение отправлено: %s.", name)
}

func (g *Game) partyAccept(e *Entity, from string) {
	p := e.Player
	at, ok := p.Invites[from]
	delete(p.Invites, from)
	inviter := g.Online[from]
	if !ok || inviter == nil || g.Now-at > 120000 {
		g.Log(e, "#ff8080", "Приглашение больше не действует.")
		return
	}
	pt := g.partyOf(inviter)
	if pt == nil {
		g.partySeq++
		pt = &Party{ID: g.partySeq, Leader: inviter.Name, Members: []string{inviter.Name}}
		if g.parties == nil {
			g.parties = map[int]*Party{}
		}
		g.parties[pt.ID] = pt
		inviter.Player.PartyID = pt.ID
		inviter.Player.Dirty = true
	}
	if len(pt.Members) >= MaxParty {
		g.Log(e, "#ff8080", "Группа уже полна.")
		return
	}
	if g.partyOf(e) != nil {
		g.partyLeave(e)
	}
	pt.Members = append(pt.Members, e.Name)
	p.PartyID = pt.ID
	p.Dirty = true
	g.partyLog(pt, "#80ffa0", "%s вступает в группу.", e.Name)
}

func (g *Game) partyDecline(e *Entity, from string) {
	delete(e.Player.Invites, from)
	e.Player.Dirty = true
	if inv := g.Online[from]; inv != nil {
		g.Log(inv, "#c0c0c0", "%s отклоняет приглашение.", e.Name)
	}
}

func (g *Game) partyLeave(e *Entity) {
	pt := g.partyOf(e)
	if pt == nil {
		return
	}
	g.removeFromParty(pt, e.Name)
	g.Log(e, "#c0c0c0", "Вы покинули группу.")
	g.partyLog(pt, "#c0c0c0", "%s покидает группу.", e.Name)
}

func (g *Game) partyKick(e *Entity, name string) {
	pt := g.partyOf(e)
	if pt == nil || pt.Leader != e.Name || name == e.Name || !slices.Contains(pt.Members, name) {
		return
	}
	g.removeFromParty(pt, name)
	if t := g.Online[name]; t != nil {
		g.Log(t, "#ff8080", "%s исключает вас из группы.", e.Name)
	}
	g.partyLog(pt, "#c0c0c0", "%s исключён из группы.", name)
}

func (g *Game) removeFromParty(pt *Party, name string) {
	pt.Members = slices.DeleteFunc(pt.Members, func(n string) bool { return n == name })
	if m := g.Online[name]; m != nil {
		m.Player.PartyID = 0
		m.Player.Dirty = true
	}
	if off := g.Offline[name]; off != nil {
		off.Player.PartyID = 0
	}
	if pt.Leader == name && len(pt.Members) > 0 {
		pt.Leader = pt.Members[0]
		g.partyLog(pt, "#80ffa0", "Новый лидер группы: %s.", pt.Leader)
	}
	if len(pt.Members) <= 1 {
		for _, n := range pt.Members {
			if m := g.Online[n]; m != nil {
				m.Player.PartyID = 0
				m.Player.Dirty = true
				g.Log(m, "#c0c0c0", "Группа распалась.")
			}
		}
		delete(g.parties, pt.ID)
	}
	for _, n := range pt.Members {
		if m := g.Online[n]; m != nil {
			m.Player.Dirty = true
		}
	}
}
