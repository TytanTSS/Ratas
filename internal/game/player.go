package game

import (
	"errors"
	"fmt"
	"math"
	"sort"

	"ratas/internal/content"
	"ratas/internal/proto"
	"ratas/internal/world"
)

const (
	InventorySize = 24
	HotbarSize    = 6
)

type Intent struct {
	Move      world.Dir
	MoveAt    float64
	Attack    bool
	AttackAt  float64
	Interact  bool
	Ability   int
	AbilityAt float64
	Aimed     bool      // the attack or ability is aimed at a tile (mouse)
	Aim       world.Pos // that tile
}

type Quest struct {
	ID      int       `json:"id"`
	Giver   string    `json:"giver"`
	GiverID EntityID  `json:"giver_id"`
	Village string    `json:"village"`
	Monster string    `json:"monster"`
	Need    int       `json:"need"`
	Have    int       `json:"have"`
	Gold    int       `json:"gold"`
	XP      int       `json:"xp"`
	Done    bool      `json:"done"`
	Kind    string    `json:"kind,omitempty"`   // "" (hunt), slay, boss, relics
	Unique  string    `json:"unique,omitempty"` // quest of a unique character
	Item    string    `json:"item,omitempty"`   // relics to collect
	Sources []string  `json:"sources,omitempty"`
	At      world.Pos `json:"at,omitempty"`    // where to go (champion lair, dungeon entrance)
	Where   string    `json:"where,omitempty"` // name of that place
	Reward  string    `json:"reward,omitempty"`
}

type PlayerState struct {
	Account     string                  `json:"account"`
	Class       string                  `json:"class"`
	Level       int                     `json:"level"`
	XP          int                     `json:"xp"`
	Attrs       map[string]float64      `json:"attrs"`
	AttrPoints  int                     `json:"attr_points"`
	SkillPoints int                     `json:"skill_points"`
	Skills      map[string]int          `json:"skills"`
	Abilities   []string                `json:"abilities"`
	Hotbar      [HotbarSize]string      `json:"hotbar"`
	Inventory   []ItemStack             `json:"inventory"`
	Equip       map[string]ItemStack    `json:"equip"`
	Gold        int                     `json:"gold"`
	Quests      []Quest                 `json:"quests,omitempty"`
	Explored    map[string]world.Bitset `json:"explored"`
	Kills       int                     `json:"kills"`
	Found       []int                   `json:"found,omitempty"`      // discovered landmarks
	Classes     []string                `json:"classes,omitempty"`    // classes being developed, the first is the starting one
	Subclasses  map[string]string       `json:"subclasses,omitempty"` // class -> chosen subclass
	Unlocks     []string                `json:"unlocks,omitempty"`    // secret classes and subclasses opened by quests
	Bosses      []string                `json:"bosses,omitempty"`     // bosses slain (for conversations)

	Intent    Intent             `json:"-"`
	Dirty     bool               `json:"-"`
	Talking   EntityID           `json:"-"`
	RespawnAt float64            `json:"-"`
	Target    EntityID           `json:"-"` // enemy shown in the HUD
	TargetAt  float64            `json:"-"`
	DiedAt    float64            `json:"-"`
	CanRise   float64            `json:"-"` // the respawn button works from this time
	PartyID   int                `json:"-"`
	Invites   map[string]float64 `json:"-"` // party invitations: inviter -> time
	Copied    string             `json:"-"` // ability copied by a mimic
	CopiedLvl int                `json:"-"`
	CopiedEnd float64            `json:"-"`
	aim       *world.Pos // the tile the current attack or ability is aimed at
	region    int
	lastHint  string
	regenAcc  float64
}

func XPForLevel(l int) int { return int(40 * math.Pow(float64(l), 1.6)) }

func (g *Game) newPlayer(name, classKey string) *Entity {
	c := content.Class(classKey)
	if c == nil || c.Secret { // secret classes are earned, not chosen
		cs := content.Classes()
		c = &cs[0]
	}
	e := &Entity{
		Kind: KPlayer, Name: name, Glyph: "@", Color: c.Color, Faction: FPlayer,
		Level: "overworld", Pos: g.Start, Facing: world.DirDown,
	}
	p := &PlayerState{
		Account: name, Class: c.Key, Level: 1, Gold: 25, Classes: []string{c.Key},
		Attrs: map[string]float64{}, Skills: map[string]int{}, Equip: map[string]ItemStack{},
		Explored: map[string]world.Bitset{}, Subclasses: map[string]string{},
	}
	for k, v := range c.Attrs {
		p.Attrs[k] = v
	}
	e.Player = p
	for _, a := range c.Abilities {
		g.unlockAbility(e, a)
	}
	for _, key := range c.Items {
		st := ItemStack{Key: key, Qty: 1}
		d := st.Def()
		slot := slotFor(d)
		if slot == SlotMain && p.Equip[SlotMain].Key != "" && !twoHanded(st) && !twoHanded(p.Equip[SlotMain]) {
			slot = SlotOff // a second one-handed weapon goes to the left hand
		}
		if slot == SlotOff && twoHanded(p.Equip[SlotMain]) {
			slot = ""
		}
		if slot != "" && p.Equip[slot].Key == "" {
			p.Equip[slot] = st
		} else {
			g.addItem(e, st)
		}
	}
	e.Recalc()
	e.HP, e.MP = e.MaxHP, e.MaxMP
	return e
}

var ErrNameTaken = errors.New("игрок с таким именем уже в игре")

// Join brings a player into the world. needClass is true for a new character
// without a chosen class: the client must ask and call Join again.
func (g *Game) Join(name, class string) (e *Entity, needClass bool, err error) {
	if _, ok := g.Online[name]; ok {
		return nil, false, ErrNameTaken
	}
	if saved, ok := g.Offline[name]; ok {
		delete(g.Offline, name)
		e = saved
		if g.Level(e.Level) == nil {
			e.Level, e.Pos = "overworld", g.Start
		}
		e.Pos = findFree(g.Levels[e.Level], e.Pos)
		e.Player.Intent = Intent{}
		e.Player.Talking = 0
		if e.Dead || e.HP <= 0 {
			e.Dead = false
			e.HP = e.MaxHP * 0.5
		}
		g.Spawn(e)
		g.Online[name] = e
		g.Log(e, "#a0e0ff", "С возвращением в %s, %s!", g.WorldName, name)
	} else {
		if class == "" {
			return nil, true, nil
		}
		e = g.newPlayer(name, class)
		e.Pos = findFree(g.Levels["overworld"], g.Start)
		g.Spawn(e)
		g.Online[name] = e
		g.Log(e, "#ffd24a", "Добро пожаловать в %s, %s! Время приключений.", g.WorldName, name)
		g.Log(e, "#a0a0a0", "WASD/стрелки — ходить, бег в монстра — атака, 1-6 — умения, E — взаимодействие, ? — помощь.")
	}
	g.LogAll("#80c0ff", "%s входит в мир.", name)
	e.Player.Dirty = true
	g.updateExplored(e)
	return e, false, nil
}

// Leave removes a player from the world but keeps the character for later.
func (g *Game) Leave(name string) {
	e, ok := g.Online[name]
	if !ok {
		return
	}
	g.closeDialogue(e, false)
	if g.partyOf(e) != nil {
		g.partyLeave(e)
	}
	delete(g.Online, name)
	g.Remove(e)
	e.Player.Intent = Intent{}
	g.Offline[name] = e
	g.LogAll("#80c0ff", "%s покидает мир.", name)
}

// SetInput records a real-time intent; it is executed on the next ticks.
func (g *Game) SetInput(e *Entity, in proto.Input) {
	it := &e.Player.Intent
	if in.Move != 0 {
		it.Move = world.Dir(in.Move)
		it.MoveAt = g.Now
	}
	if in.Attack || in.Ability > 0 {
		it.Aimed, it.Aim = in.Aim, world.Pos{X: int(in.AimX), Y: int(in.AimY)}
	}
	if in.Attack {
		it.Attack = true
		it.AttackAt = g.Now
	}
	if in.Interact {
		it.Interact = true
	}
	if in.Ability > 0 {
		it.Ability = int(in.Ability)
		it.AbilityAt = g.Now
	}
}

func (g *Game) updatePlayer(e *Entity) {
	p := e.Player
	if e.Dead {
		if g.Now >= p.RespawnAt {
			g.respawn(e)
		}
		return
	}
	if p.Copied != "" && g.Now >= p.CopiedEnd {
		g.Log(e, "#ff8ad8", "Скопированное умение рассеялось.")
		p.Copied = ""
		p.Dirty = true
	}
	s := &e.stats
	dt := TickMs / 1000
	e.HP = math.Min(e.MaxHP, e.HP+s.HPRegen*dt)
	e.MP = math.Min(e.MaxMP, e.MP+s.MPRegen*dt)

	l := g.Levels[e.Level]
	if def := l.Def(e.Pos.X, e.Pos.Y); def.Damage > 0 && g.TickN%10 == 0 {
		hz := single(hazardType(def), def.Damage/2)
		hz.DoT = true
		g.damage(nil, e, hz)
		if e.Dead {
			return
		}
	}
	if g.TickN%10 == 0 {
		g.updateExplored(e)
		g.checkSurroundings(e)
	}
	if e.stats.Stunned {
		p.Intent = Intent{}
		return
	}

	it := &p.Intent
	if it.Move != 0 && g.Now-it.MoveAt > 260 {
		it.Move = 0
	}
	if it.Ability != 0 && g.Now-it.AbilityAt > 400 {
		it.Ability = 0
	}
	if it.Attack && g.Now-it.AttackAt > 400 {
		it.Attack = false
	}
	if it.Interact {
		it.Interact = false
		g.interact(e)
	}
	if it.Aimed {
		p.aim = &it.Aim
	}
	if it.Ability != 0 && g.useHotbar(e, it.Ability) {
		it.Ability = 0
	}
	if it.Attack && g.Now >= e.NextAttack {
		it.Attack = false
		g.attackFacing(e)
	}
	p.aim = nil
	if it.Ability == 0 && !it.Attack {
		it.Aimed = false
	}
	if it.Move != 0 && g.Now >= e.NextMove {
		d := it.Move
		it.Move = 0
		g.playerStep(e, d)
	}
	if p.Talking != 0 {
		npc := g.Entities[p.Talking]
		if npc == nil || npc.Level != e.Level || npc.Pos.Dist(e.Pos) > 3 || g.hostile(npc, e) || npc.NPC == nil {
			g.closeDialogue(e, true)
		}
	}
}

func (g *Game) playerStep(e *Entity, d world.Dir) {
	e.Facing = d
	l := g.Levels[e.Level]
	to := e.Pos.Add(d.Delta())
	if !l.In(to.X, to.Y) {
		return
	}
	if o := g.At(e.Level, to); o != nil {
		switch {
		case g.hostile(e, o):
			if g.Now >= e.NextAttack {
				g.meleeAttack(e, o)
			}
		case o.NPC != nil:
			g.openDialogue(e, o)
			e.NextMove = g.Now + 250
		case o.Owner == e.ID && g.Now >= e.NextMove:
			// step through your own summons
			from := e.Pos
			l.ClearOccupant(from.X, from.Y, e.ID)
			l.ClearOccupant(to.X, to.Y, o.ID)
			e.Pos, o.Pos = to, from
			l.SetOccupant(to.X, to.Y, e.ID)
			l.SetOccupant(from.X, from.Y, o.ID)
			e.NextMove = g.Now + e.stats.MoveMs
			g.afterPlayerMove(e)
		}
		return
	}
	def := l.Def(to.X, to.Y)
	if def.Interact == "door" || def.Interact == "chest" || def.Interact == "shrine" {
		g.useTile(e, to)
		e.NextMove = g.Now + e.stats.MoveMs
		return
	}
	if !def.Walkable {
		return
	}
	g.moveEntity(e, to)
	e.NextMove = g.Now + e.stats.MoveMs*def.MoveCost
	g.afterPlayerMove(e)
}

func (g *Game) afterPlayerMove(e *Entity) {
	g.updateExplored(e)
	g.pickup(e)
	l := g.Levels[e.Level]
	hint := ""
	switch l.Def(e.Pos.X, e.Pos.Y).Interact {
	case "dungeon":
		if i := g.entranceAt(e.Pos); i >= 0 {
			hint = fmt.Sprintf("Вход: %s. Нажмите E, чтобы спуститься.", g.Entrances[i].Name)
		}
	case "stairs_down":
		hint = "Лестница вниз. Нажмите E."
	case "stairs_up":
		hint = "Лестница вверх. Нажмите E."
	}
	if hint != "" && hint != e.Player.lastHint {
		g.Log(e, "#ffd24a", "%s", hint)
	}
	e.Player.lastHint = hint
}

func (g *Game) updateExplored(e *Entity) {
	l := g.Levels[e.Level]
	bs := e.Player.Explored[l.ID]
	if len(bs) != (l.W*l.H+7)/8 {
		bs = world.NewBitset(l.W * l.H)
		e.Player.Explored[l.ID] = bs
	}
	world.FOV(l, e.Pos.X, e.Pos.Y, g.Vision(e), func(x, y int) { bs.Set(y*l.W + x) })
}

func (g *Game) entranceAt(p world.Pos) int {
	for i, en := range g.Entrances {
		if en.Pos == p {
			return i
		}
	}
	return -1
}

func (g *Game) interact(e *Entity) {
	l := g.Levels[e.Level]
	if in := l.Def(e.Pos.X, e.Pos.Y).Interact; in == "dungeon" || in == "stairs_down" || in == "stairs_up" {
		g.useStairs(e, in)
		return
	}
	cells := []world.Pos{e.Pos.Add(e.Facing.Delta())}
	for _, d := range world.AllDirs {
		cells = append(cells, e.Pos.Add(d.Delta()))
	}
	for _, c := range cells {
		if o := g.At(e.Level, c); o != nil && o.NPC != nil && !g.hostile(e, o) {
			e.Facing = world.DirTowards(e.Pos, c)
			g.openDialogue(e, o)
			return
		}
		if in := l.Def(c.X, c.Y).Interact; in == "door" || in == "chest" || in == "shrine" {
			e.Facing = world.DirTowards(e.Pos, c)
			g.useTile(e, c)
			return
		}
	}
	g.Log(e, "#808080", "Рядом нет ничего интересного.")
}

func (g *Game) useTile(e *Entity, c world.Pos) {
	l := g.Levels[e.Level]
	def := l.Def(c.X, c.Y)
	switch def.Interact {
	case "door":
		g.SetTile(l, c.X, c.Y, content.TileID(def.Becomes))
	case "chest":
		g.SetTile(l, c.X, c.Y, content.TileID(def.Becomes))
		g.Log(e, "#ffd24a", "Вы открываете сундук.")
		g.FX(e.Level, c, "", '*', "#ffd24a", 300)
		depth := max(1, l.Depth)
		g.dropGold(e.Level, c, g.rng.IntN(10*depth)+5*depth)
		for i := 0; i < 1+g.rng.IntN(2); i++ {
			if st, ok := g.randomItem(depth, 30); ok {
				g.dropItem(e.Level, c, st)
			}
		}
	case "shrine":
		g.prayAtShrine(e, l, c, def)
	}
}

func (g *Game) useStairs(e *Entity, kind string) {
	l := g.Levels[e.Level]
	switch kind {
	case "dungeon":
		idx := g.entranceAt(e.Pos)
		if idx < 0 {
			return
		}
		target := g.Level(DungeonLevelID(idx, 1))
		g.changeLevel(e, target, target.Up)
	case "stairs_down":
		target := g.Level(DungeonLevelID(l.Dungeon, l.Depth+1))
		if target == nil {
			return
		}
		g.changeLevel(e, target, target.Up)
	case "stairs_up":
		if l.Depth <= 1 {
			ow := g.Levels["overworld"]
			g.changeLevel(e, ow, g.Entrances[l.Dungeon].Pos)
		} else {
			target := g.Level(DungeonLevelID(l.Dungeon, l.Depth-1))
			g.changeLevel(e, target, target.Down)
		}
	}
}

func (g *Game) changeLevel(e *Entity, target *world.Level, near world.Pos) {
	g.closeDialogue(e, true)
	if old := g.Levels[e.Level]; old != nil {
		old.ClearOccupant(e.Pos.X, e.Pos.Y, e.ID)
	}
	e.Level = target.ID
	e.Pos = near
	if !target.Free(near.X, near.Y) {
		e.Pos = findFree(target, near)
	}
	target.SetOccupant(e.Pos.X, e.Pos.Y, e.ID)
	e.NextMove = g.Now + 300
	e.Player.lastHint = tileKey(target, e.Pos)
	g.updateExplored(e)
	g.Log(e, "#c0a0ff", "Вы входите: %s.", target.Name)
	if target.Depth > 0 && target.Down.X < 0 {
		g.Log(e, "#ff6a6a", "Здесь обитает нечто могущественное...")
	}
}

// rise is the player's own request to respawn in the village.
func (g *Game) rise(e *Entity) {
	if e.Dead && g.Now >= e.Player.CanRise {
		g.respawn(e)
	}
}

// revive brings a fallen hero back where they fell.
func (g *Game) revive(e *Entity, by *Entity) bool {
	p := e.Player
	if !e.Dead || g.Now-p.DiedAt > ReviveWindowMs {
		return false
	}
	l := g.Levels[e.Level]
	if !l.Free(e.Pos.X, e.Pos.Y) {
		e.Pos = findFree(l, e.Pos)
	}
	e.Dead = false
	e.Recalc()
	e.HP, e.MP = e.MaxHP*0.4, e.MaxMP*0.3
	l.SetOccupant(e.Pos.X, e.Pos.Y, e.ID)
	p.Dirty = true
	g.FX(e.Level, e.Pos, "Воскрешение!", 0, "#ffffc0", 1500)
	g.Log(e, "#ffffc0", "%s возвращает вас к жизни!", by.Name)
	g.Log(by, "#ffffc0", "Вы воскресили: %s.", e.Name)
	return true
}

func (g *Game) respawn(e *Entity) {
	e.Dead = false
	e.Recalc()
	e.HP, e.MP = e.MaxHP, e.MaxMP*0.5
	e.Buffs = nil
	ow := g.Levels["overworld"]
	if old := g.Levels[e.Level]; old != nil {
		old.ClearOccupant(e.Pos.X, e.Pos.Y, e.ID)
	}
	e.Level = "overworld"
	e.Pos = findFree(ow, g.Start)
	ow.SetOccupant(e.Pos.X, e.Pos.Y, e.ID)
	e.Player.Dirty = true
	g.updateExplored(e)
	g.Log(e, "#a0ffa0", "Вы приходите в себя у колодца деревни.")
}

// ReviveWindowMs is how long a fallen hero can be resurrected.
const ReviveWindowMs = 60000

func (g *Game) killPlayer(e *Entity, killer *Entity) {
	e.Dead = true
	e.HP = 0
	if l := g.Levels[e.Level]; l != nil {
		l.ClearOccupant(e.Pos.X, e.Pos.Y, e.ID)
	}
	p := e.Player
	p.DiedAt = g.Now
	p.RespawnAt = g.Now + ReviveWindowMs
	p.CanRise = g.Now + 3000
	p.Intent = Intent{}
	e.Buffs = nil
	lost := p.Gold / 10
	p.Gold -= lost
	g.closeDialogue(e, true)
	who := "окружающий мир"
	if killer != nil {
		if killer.Kind == KProjectile && killer.Proj != nil {
			killer = g.Entities[killer.Proj.Owner]
		}
	}
	if killer != nil {
		who = killer.Name
		if k := g.controller(killer); k.Player != nil && k != e && lost > 0 {
			k.Player.Gold += lost
			k.Player.Dirty = true
			g.Log(k, "#ffd700", "Вы забрали у %s %d золота.", e.Name, lost)
		}
	}
	g.Log(e, "#ff4a4a", "Вы погибли! Вас сразил(а) %s. Потеряно %d золота.", who, lost)
	g.Log(e, "#c0c0c0", "Enter — возродиться у колодца деревни. Союзник может воскресить вас в течение минуты.")
	for _, o := range g.Online {
		if o != e {
			g.Log(o, "#ff8a8a", "%s пал(а) в бою (%s).", e.Name, who)
		}
	}
	g.FX(e.Level, e.Pos, "", '%', "#ff2a2a", 1500)
	p.Dirty = true
}

// ---- progression ----

func (g *Game) GiveXP(e *Entity, amount int) {
	p := e.Player
	if p == nil || amount <= 0 {
		return
	}
	p.XP += amount
	for p.XP >= XPForLevel(p.Level) {
		p.XP -= XPForLevel(p.Level)
		p.Level++
		p.AttrPoints += 3
		p.SkillPoints++
		e.Recalc()
		e.HP, e.MP = e.MaxHP, e.MaxMP
		g.Log(e, "#ffff4a", "*** Уровень %d! +3 очка характеристик, +1 очко навыков (C и K). ***", p.Level)
		g.FX(e.Level, e.Pos, "УРОВЕНЬ!", 0, "#ffff4a", 1500)
		for _, o := range g.Online {
			if o != e {
				g.Log(o, "#c0c080", "%s достигает %d уровня.", e.Name, p.Level)
			}
		}
	}
	p.Dirty = true
}

func (g *Game) unlockAbility(e *Entity, key string) {
	p := e.Player
	for _, a := range p.Abilities {
		if a == key {
			return
		}
	}
	p.Abilities = append(p.Abilities, key)
	for i := range p.Hotbar {
		if p.Hotbar[i] == "" {
			p.Hotbar[i] = key
			break
		}
	}
	p.Dirty = true
}

func (g *Game) learnSkill(e *Entity, key string) {
	p := e.Player
	sd := content.Skill(key)
	if why := CanLearn(p, sd); why != "" {
		g.Log(e, "#ff8080", "Нельзя изучить: %s.", why)
		return
	}
	p.Skills[key]++
	p.SkillPoints--
	if sd.Grants != "" && p.Skills[key] == 1 {
		g.unlockAbility(e, sd.Grants)
		if ad := content.Ability(sd.Grants); ad != nil {
			g.Log(e, "#80ffff", "Новое умение: %s!", ad.Name)
		}
	}
	e.Recalc()
	g.Log(e, "#80ff80", "Навык «%s» — ранг %d/%d.", sd.Name, p.Skills[key], sd.MaxRank)
	p.Dirty = true
}

func (g *Game) allocAttr(e *Entity, key string) {
	p := e.Player
	switch key {
	case "str", "dex", "int", "vit":
	default:
		return
	}
	if p.AttrPoints <= 0 {
		return
	}
	p.AttrPoints--
	p.Attrs[key]++
	e.Recalc()
	p.Dirty = true
}

// ---- inventory ----

func (g *Game) addItem(e *Entity, st ItemStack) bool {
	p := e.Player
	if st.Qty <= 0 {
		st.Qty = 1
	}
	d := st.Def()
	if d != nil && d.Kind == "consumable" && len(st.Bonus) == 0 {
		for i := range p.Inventory {
			if p.Inventory[i].Key == st.Key {
				p.Inventory[i].Qty += st.Qty
				p.Dirty = true
				return true
			}
		}
	}
	if len(p.Inventory) >= InventorySize {
		return false
	}
	p.Inventory = append(p.Inventory, st)
	p.Dirty = true
	return true
}

func (g *Game) removeInvItem(e *Entity, idx int) {
	p := e.Player
	p.Inventory = append(p.Inventory[:idx], p.Inventory[idx+1:]...)
	p.Dirty = true
}

func (g *Game) pickup(e *Entity) {
	for _, o := range g.Entities {
		if o.Kind != KItem || o.Level != e.Level || o.Pos != e.Pos {
			continue
		}
		st := *o.Item
		if st.Key == "gold" {
			e.Player.Gold += st.Qty
			e.Player.Dirty = true
			g.Log(e, "#ffd700", "+%d золота.", st.Qty)
			g.Remove(o)
			continue
		}
		if g.addItem(e, st) {
			if d := st.Def(); d != nil && d.Kind == "quest" {
				defer g.updateRelics(e)
			}
			if st.Qty > 1 {
				g.Log(e, "#c0c0ff", "Подобрано: %s ×%d.", st.Name(), st.Qty)
			} else {
				g.Log(e, "#c0c0ff", "Подобрано: %s.", st.Name())
			}
			g.Remove(o)
		} else {
			g.Log(e, "#ff8080", "Инвентарь полон!")
		}
	}
}

func (g *Game) useItem(e *Entity, idx int) {
	p := e.Player
	if idx < 0 || idx >= len(p.Inventory) || e.Dead {
		return
	}
	st := p.Inventory[idx]
	d := st.Def()
	if d == nil {
		return
	}
	if equippable(d) {
		g.equip(e, idx, false)
		return
	}
	if d.Kind == "consumable" {
		if d.Effect == "return" && !g.canReturn(e) {
			return
		}
		if d.Heal > 0 {
			e.HP = math.Min(e.MaxHP, e.HP+d.Heal)
			g.FX(e.Level, e.Pos, "+"+itoa(int(d.Heal)), 0, "#60ff60", 900)
		}
		if d.Mana > 0 {
			e.MP = math.Min(e.MaxMP, e.MP+d.Mana)
			g.FX(e.Level, e.Pos, "+"+itoa(int(d.Mana)), 0, "#6090ff", 900)
		}
		// remove the item first: some effects reorganize the inventory
		p.Inventory[idx].Qty--
		if p.Inventory[idx].Qty <= 0 {
			g.removeInvItem(e, idx)
		}
		g.Log(e, "#a0ffa0", "Вы используете: %s.", d.Name)
		g.itemEffect(e, d)
		p.Dirty = true
	}
}

// QuickPotion drinks the first healing (kind=0) or mana (kind=1) potion.
func (g *Game) quickPotion(e *Entity, mana bool) {
	best := -1
	for i, st := range e.Player.Inventory {
		d := st.Def()
		if d == nil || d.Kind != "consumable" {
			continue
		}
		if (mana && d.Mana > 0) || (!mana && d.Heal > 0) {
			best = i
			break
		}
	}
	if best < 0 {
		g.Log(e, "#ff8080", "Нет подходящих зелий.")
		return
	}
	g.useItem(e, best)
}

func (g *Game) dropInv(e *Entity, idx int) {
	p := e.Player
	if idx < 0 || idx >= len(p.Inventory) {
		return
	}
	st := p.Inventory[idx]
	g.removeInvItem(e, idx)
	g.dropItem(e.Level, e.Pos, st)
	g.Log(e, "#a0a0a0", "Выброшено: %s.", st.Name())
}

// SortInventory orders items by kind then name.
func (g *Game) sortInventory(e *Entity) {
	inv := e.Player.Inventory
	order := map[string]int{"weapon": 0, "shield": 1, "offhand": 1, "head": 2, "chest": 3, "belt": 4, "legs": 5, "back": 6, "ring": 7, "consumable": 8, "quest": 9}
	sort.SliceStable(inv, func(i, j int) bool {
		di, dj := inv[i].Def(), inv[j].Def()
		if di == nil || dj == nil {
			return false
		}
		if order[di.Kind] != order[dj.Kind] {
			return order[di.Kind] < order[dj.Kind]
		}
		return inv[i].Name() < inv[j].Name()
	})
	e.Player.Dirty = true
}

// ---- commands ----

// Command handles discrete client actions.
func (g *Game) Command(e *Entity, c proto.Command) {
	if e == nil || e.Player == nil {
		return
	}
	p := e.Player
	switch c.Kind {
	case "alloc_attr":
		g.allocAttr(e, c.Key)
	case "learn":
		g.learnSkill(e, c.Key)
	case "hotbar":
		if c.Index < 0 || c.Index >= HotbarSize {
			return
		}
		if c.Key != "" {
			found := false
			for _, a := range p.Abilities {
				found = found || a == c.Key
			}
			if !found {
				return
			}
			for i := range p.Hotbar {
				if p.Hotbar[i] == c.Key {
					p.Hotbar[i] = ""
				}
			}
		}
		p.Hotbar[c.Index] = c.Key
		p.Dirty = true
	case "use":
		g.useItem(e, c.Index)
	case "equip_left":
		g.equip(e, c.Index, true)
	case "respawn":
		g.rise(e)
	case "start_class":
		g.startClass(e, c.Key)
	case "choose_subclass":
		g.chooseSubclass(e, c.Key)
	case "party_invite":
		g.partyInvite(e, c.Key)
	case "party_accept":
		g.partyAccept(e, c.Key)
	case "party_decline":
		g.partyDecline(e, c.Key)
	case "party_leave":
		g.partyLeave(e)
	case "party_kick":
		g.partyKick(e, c.Key)
	case "potion":
		g.quickPotion(e, c.Key == "mana")
	case "unequip":
		g.unequip(e, c.Key)
	case "drop":
		g.dropInv(e, c.Index)
	case "sort":
		g.sortInventory(e)
	case "talk":
		g.talkAI(e, c.Text)
	case "talk_option":
		g.talkOption(e, c.Index)
	case "talk_close":
		g.closeDialogue(e, false)
	case "buy":
		g.buy(e, c.Key)
	case "sell":
		g.sell(e, c.Index)
	case "chat":
		text := []rune(c.Text)
		if len(text) == 0 {
			return
		}
		if len(text) > 160 {
			text = text[:160]
		}
		msg := string(text)
		g.Say(e, msg, 5000)
		g.LogAll("#ffffff", "[%s] %s", e.Name, msg)
	}
}

func itoa(v int) string {
	if v == 0 {
		return "0"
	}
	neg := v < 0
	if neg {
		v = -v
	}
	var b [20]byte
	i := len(b)
	for v > 0 {
		i--
		b[i] = byte('0' + v%10)
		v /= 10
	}
	if neg {
		i--
		b[i] = '-'
	}
	return string(b[i:])
}
