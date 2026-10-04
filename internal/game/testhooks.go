package game

import (
	"ratas/internal/content"
	"ratas/internal/world"
)

// Helpers used by UI tests in other packages.

// MoveForTest places e next to target.
func (g *Game) MoveForTest(e, target *Entity) {
	l := g.Levels[target.Level]
	if e.Level != target.Level {
		g.changeLevel(e, l, target.Pos)
	}
	g.moveEntity(e, findFree(l, target.Pos))
}

// OpenDialogueForTest starts a conversation.
func (g *Game) OpenDialogueForTest(p, npc *Entity) { g.openDialogue(p, npc) }

// TeleportForTest moves a player to a level (generating it if needed).
func (g *Game) TeleportForTest(e *Entity, levelID string) bool {
	l := g.Level(levelID)
	if l == nil {
		return false
	}
	p := l.Up
	if l.Depth == 0 {
		p = g.Start
	}
	g.changeLevel(e, l, p)
	return true
}

// PlaceForTest puts a player on a level near a position.
func (g *Game) PlaceForTest(e *Entity, levelID string, p world.Pos) {
	l := g.Level(levelID)
	if l == nil {
		return
	}
	g.changeLevel(e, l, findFree(l, p))
}

// SpawnForTest creates a monster near a position.
func (g *Game) SpawnForTest(key, levelID string, p world.Pos, lvl int) *Entity {
	def := content.Monster(key)
	l := g.Level(levelID)
	if def == nil || l == nil {
		return nil
	}
	return g.newMonster(def, levelID, findFree(l, p), lvl)
}

// GiveForTest adds an item to a player's backpack.
func (g *Game) GiveForTest(e *Entity, key string) { g.addItem(e, ItemStack{Key: key, Qty: 1}) }

// GrantForTest unlocks an ability and puts it on the hotbar.
func (g *Game) GrantForTest(e *Entity, ability string) { g.unlockAbility(e, ability) }

// EquipForTest gives an item and puts it on (left: into the left hand).
func (g *Game) EquipForTest(e *Entity, key string, left bool) {
	g.addItem(e, ItemStack{Key: key, Qty: 1})
	for i, st := range e.Player.Inventory {
		if st.Key == key {
			g.equip(e, i, left)
			return
		}
	}
}

// ExploreForTest reveals the whole overworld and all its sights to a player.
func (g *Game) ExploreForTest(e *Entity) {
	ow := g.Levels["overworld"]
	bs := world.NewBitset(ow.W * ow.H)
	for i := 0; i < ow.W*ow.H; i++ {
		bs.Set(i)
	}
	e.Player.Explored["overworld"] = bs
	e.Player.Found = e.Player.Found[:0]
	for i := range g.Landmarks {
		e.Player.Found = append(e.Player.Found, i)
	}
	e.Player.Dirty = true
}

// PartyForTest puts two players into one party.
func (g *Game) PartyForTest(a, b *Entity) {
	g.partyInvite(a, b.Name)
	g.partyAccept(b, a.Name)
}

// KillForTest makes a creature fall.
func (g *Game) KillForTest(e *Entity) {
	e.HP = 0
	g.kill(e, nil)
}

// JoinForTest brings a second hero (without a connection) into the world.
func (g *Game) JoinForTest(name, class string) *Entity {
	e, _, _ := g.Join(name, class)
	return e
}

// ToughForTest makes a hero practically unkillable for scripted scenes.
func (g *Game) ToughForTest(e *Entity) {
	g.applyBuff(e, &content.BuffDef{Key: "test_tough", Name: "Испытание", DurationMs: 3600000,
		Stats: map[string]float64{"max_hp": 50000, "hp_regen": 500}}, e.ID)
	e.HP = e.MaxHP
}
