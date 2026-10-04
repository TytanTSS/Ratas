package game

import (
	"fmt"
	"math"
	"sort"
	"strings"
	"unicode/utf8"

	"ratas/internal/content"
	"ratas/internal/proto"
	"ratas/internal/world"
)

func firstRune(s string) rune {
	if s == "" {
		return '?'
	}
	r, _ := utf8.DecodeRuneInString(s)
	return r
}

// ItemViewOf describes an item for the client.
func ItemViewOf(st ItemStack) proto.ItemView {
	d := st.Def()
	if d == nil {
		return proto.ItemView{Key: st.Key, Name: st.Key, Glyph: '?', Qty: st.Qty}
	}
	var desc []string
	rarity := st.ItemRarity()
	if rarity > Common || slotFor(d) != "" {
		desc = append(desc, RarityName(int(rarity)))
	}
	if d.Kind == "weapon" {
		t := d.DmgType
		if t == "" {
			t = "blunt"
		}
		hands := "одноручное"
		if d.Hands >= 2 {
			hands = "двуручное"
		}
		k := st.statK()
		desc = append(desc, fmt.Sprintf("Урон %.0f-%.0f (%s), %s", math.Round(d.Damage[0]*k), math.Round(d.Damage[1]*k), DamageTypeName(t), hands))
	}
	if d.Heal > 0 {
		desc = append(desc, fmt.Sprintf("+%.0f здоровья", d.Heal))
	}
	if d.Mana > 0 {
		desc = append(desc, fmt.Sprintf("+%.0f маны", d.Mana))
	}
	desc = append(desc, statLines(scaled(d.Stats, st.statK()))...)
	desc = append(desc, statLines(st.Bonus)...)
	if d.OnHit != nil {
		desc = append(desc, fmt.Sprintf("При ударе %.0f%%: %s", d.OnHitPct, BuffDesc(d.OnHit)))
	}
	if d.Buff != nil && d.Desc == "" {
		desc = append(desc, BuffDesc(d.Buff))
	}
	if d.Desc != "" && (d.Kind != "consumable" || d.Buff != nil || d.Effect != "") {
		desc = append(desc, d.Desc)
	}
	return proto.ItemView{
		Key: st.Key, Name: st.Name(), Glyph: firstRune(d.Glyph), Color: d.Color, Kind: d.Kind,
		Qty: max(1, st.Qty), Value: st.Value(), Desc: strings.Join(desc, ", "), Hands: d.Hands,
		Rarity: int8(rarity),
	}
}

// scaled multiplies stats (a rarer copy of an item is stronger).
func scaled(m map[string]float64, k float64) map[string]float64 {
	if k == 1 {
		return m
	}
	out := make(map[string]float64, len(m))
	for key, v := range m {
		if math.Abs(v) >= 3 {
			out[key] = math.Round(v * k)
		} else {
			out[key] = math.Round(v*k*10) / 10
		}
	}
	return out
}

// BuffDesc describes an effect: "Горение (огонь 3/с, 3 с)".
func BuffDesc(b *content.BuffDef) string {
	var parts []string
	if b.Stun {
		parts = append(parts, "оглушение")
	}
	if b.DotPerSec > 0 {
		t := b.DmgType
		if t == "" {
			t = "poison"
		}
		parts = append(parts, fmt.Sprintf("%s %g/с", DamageTypeName(t), b.DotPerSec))
	} else if b.DotPerSec < 0 {
		parts = append(parts, fmt.Sprintf("лечение %g/с", -b.DotPerSec))
	}
	parts = append(parts, statLines(b.Stats)...)
	parts = append(parts, fmt.Sprintf("%g с", float64(b.DurationMs)/1000))
	return b.Name + " (" + strings.Join(parts, "; ") + ")"
}

func statLines(m map[string]float64) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	var out []string
	for _, k := range keys {
		out = append(out, fmt.Sprintf("%s %+g", StatName(k), m[k]))
	}
	return out
}

// statusOf summarizes the visible effects on an entity.
func statusOf(e *Entity) uint16 {
	var st uint16
	for _, b := range e.Buffs {
		d := &b.Def
		if d.DotPerSec > 0 {
			switch d.DmgType {
			case "fire":
				st |= proto.StatusBurning
			case "slash", "pierce", "blunt":
				st |= proto.StatusBleeding
			case "shadow", "arcane":
				st |= proto.StatusCursed
			case "holy", "lightning":
				st |= proto.StatusHoly
			case "cold":
				st |= proto.StatusChilled
			default:
				st |= proto.StatusPoisoned
			}
		}
		if d.Stun {
			st |= proto.StatusStunned
		}
		if d.Silence {
			st |= proto.StatusSilenced
		}
		if d.Stealth {
			st |= proto.StatusStealth
		}
		if d.Stats["move_speed"] < 0 {
			st |= proto.StatusChilled
		}
		for k, v := range d.Stats {
			if v < 0 && strings.HasPrefix(k, "res_") {
				st |= proto.StatusCursed
			}
			if v > 0 && strings.HasPrefix(k, "res_") && b.Source == e.ID || (v > 0 && k == "res_all") {
				st |= proto.StatusShielded
			}
		}
	}
	return st
}

// targetFor picks the enemy shown in a player's HUD: the one they fight, or
// the nearest visible one.
func (g *Game) targetFor(e *Entity) *Entity {
	p := e.Player
	if t := g.Entities[p.Target]; t != nil && t.Alive() && t.Level == e.Level && g.hostile(e, t) && g.canSee(e, t) &&
		g.Now-p.TargetAt < 12000 && e.Pos.Dist(t.Pos) <= g.Vision(e)+2 {
		return t
	}
	l := g.Levels[e.Level]
	var best *Entity
	bd := math.MaxInt
	vision := g.Vision(e)
	for _, o := range g.onLevel(e.Level) {
		if o.Monster == nil || !g.hostile(e, o) || !g.canSee(e, o) {
			continue
		}
		if d := e.Pos.DistSq(o.Pos); d < bd && e.Pos.Dist(o.Pos) <= vision {
			best, bd = o, d
		}
	}
	if best != nil && !world.LOS(l, e.Pos, best.Pos) {
		return nil
	}
	return best
}

func (g *Game) targetView(t *Entity) *proto.TargetView {
	v := &proto.TargetView{ID: t.ID, Name: t.Name, Color: t.Color, Res: map[string]int{}}
	if t.MaxHP > 0 {
		v.HP = uint8(math.Max(0, math.Min(100, 100*t.HP/t.MaxHP)))
	}
	if t.Monster != nil {
		v.Level = t.Monster.Lvl
		if d := content.Monster(t.Monster.Def); d != nil {
			v.Boss = d.Boss
		}
	}
	for _, dt := range content.DamageTypes() {
		if r := math.Round(t.stats.Resist(dt.Key)); r != 0 {
			v.Res[dt.Key] = int(r)
		}
	}
	for _, b := range t.Buffs {
		v.Effects = append(v.Effects, proto.BuffView{Name: b.Def.Name, Color: b.Def.Color, Left: int(b.Until - g.Now)})
	}
	return v
}

// Snapshot builds the per-tick view for one player.
func (g *Game) Snapshot(e *Entity) *proto.Snapshot {
	p := e.Player
	s := &proto.Snapshot{Tick: g.TickN, TimeOfDay: g.TimeOfDay()}
	self := proto.SelfView{
		X: e.Pos.X, Y: e.Pos.Y, HP: e.HP, MaxHP: e.MaxHP, MP: e.MP, MaxMP: e.MaxMP,
		XP: p.XP, XPNext: XPForLevel(p.Level), Level: p.Level, Gold: p.Gold,
		Dead: e.Dead, Vision: g.Vision(e), Facing: uint8(e.Facing),
		AttrPoints: p.AttrPoints, SkillPoints: p.SkillPoints,
	}
	if e.Dead {
		self.RespawnIn = int(math.Max(0, p.RespawnAt-g.Now))
	}
	for i, key := range p.Hotbar {
		if key == "" {
			continue
		}
		if a := content.Ability(key); a != nil && a.CooldownMs > 0 {
			if left := e.Cooldowns[key] - g.Now; left > 0 {
				self.Cooldown[i] = float32(left / float64(a.CooldownMs))
			}
		}
	}
	for _, b := range e.Buffs {
		self.Buffs = append(self.Buffs, proto.BuffView{Name: b.Def.Name, Color: b.Def.Color, Left: int(b.Until - g.Now)})
	}
	if l := g.Levels[e.Level]; l != nil {
		self.StandingOn = l.Def(e.Pos.X, e.Pos.Y).Interact
	}
	if e.Level == "overworld" {
		self.Region = g.RegionName(e.Pos)
	}
	if t := g.targetFor(e); t != nil {
		self.Target = g.targetView(t)
	}
	self.CanRise = e.Dead && g.Now >= p.CanRise
	self.Safe, self.PvP = g.safeZone(e), g.PvP
	if p.Copied != "" {
		self.Copied, self.CopiedLeft = p.Copied, int(p.CopiedEnd-g.Now)
	}
	for from, at := range p.Invites {
		if g.Now-at < 120000 && g.Online[from] != nil {
			self.Invites = append(self.Invites, from)
		}
	}
	sort.Strings(self.Invites)
	if pt := g.partyOf(e); pt != nil {
		for _, n := range pt.Members {
			m := g.Online[n]
			if m == nil || m == e {
				continue
			}
			pm := proto.PartyMember{Name: m.Name, Class: m.Player.Class, Level: m.Player.Level, HP: m.HP, MaxHP: m.MaxHP,
				MP: m.MP, MaxMP: m.MaxMP, Dead: m.Dead, Leader: pt.Leader == n, X: m.Pos.X, Y: m.Pos.Y, LevelID: m.Level}
			if m.Level != e.Level {
				if l := g.Levels[m.Level]; l != nil {
					pm.Where = l.Name
				}
			}
			for _, b := range m.Buffs {
				pm.Buffs = append(pm.Buffs, proto.BuffView{Name: b.Def.Name, Color: b.Def.Color, Left: int(b.Until - g.Now)})
			}
			self.Party = append(self.Party, pm)
		}
	}
	s.Self = self

	for _, o := range g.Entities {
		if o.Level != e.Level {
			continue
		}
		dx, dy := o.Pos.X-e.Pos.X, o.Pos.Y-e.Pos.Y
		if dx < -80 || dx > 80 || dy < -45 || dy > 45 {
			continue
		}
		if o.Kind == KPlayer && o.Dead && g.Now-o.Player.DiedAt > ReviveWindowMs {
			continue
		}
		hostile := g.hostile(e, o)
		if hostile && !g.canSee(e, o) {
			continue // stealthed enemy
		}
		v := proto.EntityView{
			ID: o.ID, X: o.Pos.X, Y: o.Pos.Y, Glyph: firstRune(o.Glyph), Color: o.Color,
			Kind: uint8(o.Kind), Name: o.Name, Hostile: hostile || (o.Faction == FMonster && o.Kind != KProjectile),
			Status: statusOf(o), Facing: uint8(o.Facing), Dead: o.Dead,
			Ally: o != e && (g.sameParty(e, o) || o.Owner == e.ID),
		}
		switch {
		case o.Monster != nil && o.Monster.Def == "illusion" && g.Entities[o.Owner] != nil:
			// illusions look like their maker
			own := g.Entities[o.Owner]
			v.Kind = uint8(own.Kind)
			v.Status |= proto.StatusIllusion
			if own.Player != nil {
				v.Def, v.Gear = own.Player.Class, gearOf(own)
			}
		case o.NPC != nil && o.NPC.Unique != "":
			v.Def = "unique:" + o.NPC.Unique
			if u := content.Unique(o.NPC.Unique); u != nil {
				v.Model = u.Model
			}
		case o.NPC != nil:
			v.Def = o.NPC.Role
			if r := content.NPCRole(o.NPC.Role); r != nil {
				v.Model = r.Model
			}
		case o.Monster != nil:
			v.Def = o.Monster.Def
			if d := content.Monster(o.Monster.Def); d != nil {
				v.Model = d.Model
			}
		case o.Player != nil:
			v.Def = o.Player.Class
			if c := content.Class(o.Player.Class); c != nil {
				v.Model = c.Model
			}
			v.Gear = gearOf(o)
		case o.Item != nil:
			v.Def = o.Item.Key
			v.Rarity = uint8(o.Item.ItemRarity())
			if v.Rarity >= uint8(Rare) {
				v.Color = RarityColor(int(v.Rarity)) // rare loot stands out on the map
			}
		case o.Proj != nil:
			v.Def = o.Proj.Ability
		}
		if o.MaxHP > 0 && o.Kind != KItem {
			v.HP = uint8(math.Max(0, math.Min(100, 100*o.HP/o.MaxHP)))
		}
		if o.Monster != nil {
			if d := content.Monster(o.Monster.Def); d != nil && d.Boss {
				v.Boss = true
			}
		}
		if o.SpeechUntil > g.Now {
			v.Speech = o.Speech
		}
		s.Entities = append(s.Entities, v)
	}
	s.FX = g.fx[e.Level]
	for name := range g.Online {
		s.Online = append(s.Online, name)
	}
	sort.Strings(s.Online)
	return s
}

// gearOf lists what a hero visibly wears: right hand, left hand, head, chest, back.
func gearOf(e *Entity) []string {
	eq := e.Player.Equip
	return []string{eq[SlotMain].Key, eq[SlotOff].Key, eq[SlotHead].Key, eq[SlotChest].Key, eq[SlotBack].Key}
}

// places lists what a player knows on the world map.
func (g *Game) places(e *Entity) []proto.Place {
	var out []proto.Place
	ow := g.Levels["overworld"]
	bs := e.Player.Explored["overworld"]
	seen := func(p world.Pos) bool { return ow != nil && bs.Get(p.Y*ow.W+p.X) }
	for _, v := range g.Villages {
		out = append(out, proto.Place{Name: v.Name, Kind: "village", X: v.Center.X, Y: v.Center.Y})
	}
	for _, en := range g.Entrances {
		if seen(en.Pos) {
			out = append(out, proto.Place{Name: en.Name, Kind: "dungeon:" + en.Theme, X: en.Pos.X, Y: en.Pos.Y})
		}
	}
	for _, i := range e.Player.Found {
		if i < len(g.Landmarks) {
			lm := g.Landmarks[i]
			out = append(out, proto.Place{Name: lm.Name, Kind: lm.Kind, X: lm.Pos.X, Y: lm.Pos.Y})
		}
	}
	for i, c := range g.regionCenters() {
		if seen(c) {
			out = append(out, proto.Place{Name: g.Regions[i].Name, Kind: "region:" + g.Regions[i].Kind, X: c.X, Y: c.Y})
		}
	}
	for _, q := range e.Player.Quests {
		if q.At != (world.Pos{}) && !q.Done {
			out = append(out, proto.Place{Name: q.Where, Kind: "quest", X: q.At.X, Y: q.At.Y})
		}
	}
	return out
}

// regionCenters returns a labelled point inside each region (its most
// central cell).
func (g *Game) regionCenters() []world.Pos {
	if len(g.regionAt) == len(g.Regions) {
		return g.regionAt
	}
	l := g.Levels["overworld"]
	if l == nil || len(g.RegionMap) != l.W*l.H {
		return nil
	}
	sx := make([]int, len(g.Regions))
	sy := make([]int, len(g.Regions))
	n := make([]int, len(g.Regions))
	for i, r := range g.RegionMap {
		if r == 0 || int(r) > len(g.Regions) {
			continue
		}
		sx[r-1] += i % l.W
		sy[r-1] += i / l.W
		n[r-1]++
	}
	out := make([]world.Pos, len(g.Regions))
	for i := range out {
		if n[i] == 0 {
			continue
		}
		c := world.Pos{X: sx[i] / n[i], Y: sy[i] / n[i]}
		// pull the label into the region if the centroid falls outside
		if int(g.RegionMap[c.Y*l.W+c.X]) != i+1 {
			best := -1
			for j, r := range g.RegionMap {
				if int(r) != i+1 {
					continue
				}
				p := world.Pos{X: j % l.W, Y: j / l.W}
				if d := p.DistSq(c); best < 0 || d < best {
					best = d
					out[i] = p
				}
			}
			continue
		}
		out[i] = c
	}
	g.regionAt = out
	return out
}

// Sheet builds the full character sheet.
func (g *Game) Sheet(e *Entity) *proto.PlayerSheet {
	p := e.Player
	sh := &proto.PlayerSheet{
		Name: e.Name, Class: p.Class, Level: p.Level, AttrPoints: p.AttrPoints, SkillPoints: p.SkillPoints,
		Attrs: map[string]float64{}, Stats: e.stats.Map(), Skills: map[string]int{},
		Abilities: append([]string(nil), p.Abilities...), Hotbar: p.Hotbar, Equip: map[string]proto.ItemView{},
	}
	for k, v := range p.Attrs {
		sh.Attrs[k] = v
	}
	for k, v := range p.Skills {
		sh.Skills[k] = v
	}
	sh.Stats["move_ms"] = e.stats.MoveMs
	sh.Stats["attack_ms"] = e.stats.AttackMs
	sh.Stats["kills"] = float64(p.Kills)
	for _, st := range p.Inventory {
		sh.Inventory = append(sh.Inventory, ItemViewOf(st))
	}
	for slot, st := range p.Equip {
		if st.Key != "" {
			sh.Equip[slot] = ItemViewOf(st)
		}
	}
	g.updateRelics(e)
	for _, q := range p.Quests {
		qv := proto.QuestView{
			Text: upperFirst(QuestText(&q)),
			Have: q.Have, Need: q.Need, Done: q.Done, Giver: q.Giver + ", " + q.Village, Unique: q.Unique != "",
			Where: q.Where, X: q.At.X, Y: q.At.Y,
		}
		if q.Reward != "" {
			qv.Reward = RewardName(q.Reward)
		} else {
			qv.Reward = fmt.Sprintf("%d золота", q.Gold)
		}
		if q.At != (world.Pos{}) && e.Level == "overworld" && !q.Done {
			qv.Where = fmt.Sprintf("%s — %s, ~%d шагов", q.Where, compassRU(e.Pos, q.At), e.Pos.Dist(q.At))
		}
		sh.Quests = append(sh.Quests, qv)
	}
	for _, c := range p.Classes {
		sh.Classes = append(sh.Classes, proto.ClassView{Key: c, Level: ClassLevel(p, c), Subclass: p.Subclasses[c]})
	}
	sh.Unlocks = append([]string(nil), p.Unlocks...)
	sh.Places = g.places(e)
	return sh
}

// LevelData returns the current level of a player for transmission.
func (g *Game) LevelData(e *Entity) *proto.LevelData {
	l := g.Levels[e.Level]
	ld := &proto.LevelData{ID: l.ID, Name: l.Name, W: l.W, H: l.H, Tiles: proto.Compress(l.Tiles), Lit: l.Lit, Depth: l.Depth}
	if bs := e.Player.Explored[l.ID]; bs != nil {
		ld.Explored = append([]byte(nil), bs...)
	}
	return ld
}

func upperFirst(s string) string {
	r := []rune(s)
	if len(r) > 0 {
		r[0] = []rune(strings.ToUpper(string(r[0])))[0]
	}
	return string(r)
}
