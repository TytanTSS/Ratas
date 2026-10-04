package game

import (
	"fmt"
	"math"
	"math/rand/v2"
	"slices"
	"strings"

	"ratas/internal/content"
	"ratas/internal/world"
)

// MaxUniques is how many unique characters a world gets.
const MaxUniques = 11

// placeUniques settles unique characters in the wild. Those who teach secret
// classes and subclasses come first, the rest are picked at random.
func (g *Game) placeUniques(r *rand.Rand) {
	l := g.Levels["overworld"]
	all := content.Uniques()
	var first, rest []int
	for i, u := range all {
		if strings.HasPrefix(u.Reward, "class:") || strings.HasPrefix(u.Reward, "subclass:") {
			first = append(first, i)
		} else {
			rest = append(rest, i)
		}
	}
	r.Shuffle(len(rest), func(i, j int) { rest[i], rest[j] = rest[j], rest[i] })
	var placed []world.Pos
	for _, i := range append(first, rest...) {
		if len(placed) >= MaxUniques {
			break
		}
		u := &all[i]
		p, ok := g.uniqueSpot(r, l, u.Biomes, placed)
		if !ok {
			p, ok = g.uniqueSpot(r, l, nil, placed)
		}
		if !ok {
			continue
		}
		placed = append(placed, p)
		g.Spawn(&Entity{
			Kind: KNPC, Name: u.Name + ", " + lower(u.Title), Glyph: "@", Color: u.Color, Level: "overworld", Pos: p,
			Faction: FNeutral, HP: 100, MaxHP: 100, Facing: world.DirDown,
			NPC: &NPCState{Role: "unique", PName: u.Name, Home: p, Unique: u.Key, Gold: 40 + r.IntN(60)},
		})
	}
}

func (g *Game) uniqueSpot(r *rand.Rand, l *world.Level, biomes []string, taken []world.Pos) (world.Pos, bool) {
	for tries := 0; tries < 4000; tries++ {
		p := world.Pos{X: 4 + r.IntN(l.W-8), Y: 3 + r.IntN(l.H-6)}
		def := l.Def(p.X, p.Y)
		if !l.Free(p.X, p.Y) || def.Interact != "" || def.Damage > 0 || g.inVillage(p, 10) || p.Manhattan(g.Start) < 35 {
			continue
		}
		if len(biomes) > 0 && !slices.Contains(biomes, def.Biome) {
			continue
		}
		ok := true
		for _, o := range taken {
			if o.Dist(p) < 25 {
				ok = false
				break
			}
		}
		if ok {
			return p, true
		}
	}
	return world.Pos{}, false
}

// RewardName describes the reward of a unique quest.
func RewardName(reward string) string {
	kind, key, _ := strings.Cut(reward, ":")
	switch kind {
	case "item":
		if d := content.Item(key); d != nil {
			return fmt.Sprintf("артефакт «%s»", d.Name)
		}
	case "class":
		if c := content.Class(key); c != nil {
			return fmt.Sprintf("секретный класс «%s»", c.Name)
		}
	case "subclass":
		if sc := content.Subclass(key); sc != nil {
			return fmt.Sprintf("секретный подкласс «%s»", sc.Name)
		}
	case "skill":
		if s := content.Skill(key); s != nil {
			return fmt.Sprintf("тайный навык «%s»", s.Name)
		}
	}
	return reward
}

func (g *Game) uniqueQuest(p *Entity, key string) *Quest {
	for i := range p.Player.Quests {
		if q := &p.Player.Quests[i]; q.Unique == key {
			return q
		}
	}
	return nil
}

func uniqueDone(p *Entity, key string) bool {
	return slices.Contains(p.Player.Unlocks, "quest:"+key)
}

// offerUniqueQuest gives the quest of a unique character.
func (g *Game) offerUniqueQuest(p, npc *Entity, u *content.UniqueDef) string {
	pl := p.Player
	if len(pl.Quests) >= 6 {
		return "Сперва разберись со своими делами — у тебя и так полный журнал."
	}
	g.questSeq++
	q := Quest{ID: g.questSeq, Giver: u.Name, GiverID: npc.ID, Village: lower(u.Title), Kind: u.Quest, Unique: u.Key,
		Need: 1, Reward: u.Reward, Gold: 150 + 20*pl.Level, XP: XPForLevel(pl.Level) / 2}
	switch u.Quest {
	case "slay":
		c := g.championOf(u, npc.Pos, pl.Level)
		if c == nil {
			return "Твоя цель ускользнула... Приходи позже."
		}
		q.Monster, q.At, q.Where = c.Monster.Def, c.Pos, c.Name
	case "boss":
		q.Monster = u.Target
		if def := content.Monster(u.Target); def != nil {
			for _, e := range g.Entrances {
				if slices.Contains(def.Themes, e.Theme) {
					q.At, q.Where = e.Pos, e.Name
					break
				}
			}
		}
	case "relics":
		q.Item, q.Need, q.Sources = u.Target, max(1, u.Count), u.Sources
	}
	pl.Quests = append(pl.Quests, q)
	pl.Dirty = true
	g.Log(p, "#ff80ff", "Задание %s: %s. Награда — %s. Журнал — J.", u.Name, QuestText(&q), RewardName(u.Reward))
	return u.Offer
}

// championOf returns the champion of a slay quest, calling it into the wild
// (far from the quest giver) if there is none.
func (g *Game) championOf(u *content.UniqueDef, near world.Pos, lvl int) *Entity {
	if id, ok := g.Champions[u.Key]; ok {
		if c := g.Entities[id]; c != nil && c.Alive() {
			return c
		}
	}
	def := content.Monster(u.Target)
	if def == nil {
		return nil
	}
	l := g.Levels["overworld"]
	var spot world.Pos
	found := false
	for tries := 0; tries < 3000 && !found; tries++ {
		d := 35 + g.rng.IntN(55)
		a := g.rng.Float64() * 6.283
		p := world.Pos{X: near.X + int(float64(d)*math.Cos(a)), Y: near.Y + int(float64(d)*math.Sin(a)/2)}
		def2 := l.Def(p.X, p.Y)
		if l.Free(p.X, p.Y) && def2.Interact == "" && def2.Damage == 0 && !g.inVillage(p, 15) &&
			(slices.Contains(def.Themes, def2.Biome) || tries > 2000) {
			spot, found = p, true
		}
	}
	if !found {
		return nil
	}
	c := g.newMonster(def, "overworld", spot, lvl+2)
	c.Name = u.Champion
	c.MaxHP *= 3.5
	c.HP = c.MaxHP
	c.Monster.Damage[0] *= 1.4
	c.Monster.Damage[1] *= 1.4
	c.Monster.Armor += 3
	c.Monster.XP *= 4
	c.Monster.Champion = u.Key
	c.Monster.Persistent = true
	c.Recalc()
	g.Champions[u.Key] = c.ID
	// a champion has a bodyguard
	if minion := pickMonster(g.rng, l.Def(spot.X, spot.Y).Biome, 0, false, false); minion != nil {
		g.spawnGroup(g.rng, minion, l, spot, lvl)
	}
	return c
}

// championSlain completes slay quests when a champion falls.
func (g *Game) championSlain(p, m *Entity) {
	key := m.Monster.Champion
	if key == "" {
		return
	}
	delete(g.Champions, key)
	if q := g.uniqueQuest(p, key); q != nil && q.Kind == "slay" && !q.Done {
		q.Have, q.Done = 1, true
		p.Player.Dirty = true
		g.Log(p, "#ff80ff", "Чемпион повержен! Вернитесь к: %s.", q.Giver)
	}
}

// relicDrop: monsters of the right lands carry relics for active quests.
func (g *Game) relicDrop(p, m *Entity) {
	l := g.Levels[m.Level]
	where := l.Theme
	if l.ID == "overworld" {
		where = l.Def(m.Pos.X, m.Pos.Y).Biome
	}
	for i := range p.Player.Quests {
		q := &p.Player.Quests[i]
		if q.Kind != "relics" || q.Done || !slices.Contains(q.Sources, where) || !g.chance(35) {
			continue
		}
		g.dropItem(m.Level, m.Pos, ItemStack{Key: q.Item, Qty: 1})
	}
}

// updateRelics recounts relics in the backpack.
func (g *Game) updateRelics(p *Entity) {
	for i := range p.Player.Quests {
		q := &p.Player.Quests[i]
		if q.Kind != "relics" {
			continue
		}
		have := 0
		for _, st := range p.Player.Inventory {
			if st.Key == q.Item {
				have += max(1, st.Qty)
			}
		}
		was := q.Done
		q.Have, q.Done = min(have, q.Need), have >= q.Need
		if q.Done && !was {
			g.Log(p, "#ff80ff", "Собрано достаточно! Вернитесь к: %s.", q.Giver)
		}
	}
}

// finishUniqueQuest hands out the reward of a completed unique quest.
func (g *Game) finishUniqueQuest(p *Entity, u *content.UniqueDef) string {
	pl := p.Player
	q := g.uniqueQuest(p, u.Key)
	if q == nil {
		return ""
	}
	g.updateRelics(p)
	if !q.Done {
		switch q.Kind {
		case "relics":
			return fmt.Sprintf("Пока лишь %d из %d. Ищи у тварей: %s.", q.Have, q.Need, sourcesText(q.Sources))
		case "slay":
			return fmt.Sprintf("%s всё ещё жив. Ищи %s, примерно в %d шагах.", q.Where, compassRU(p.Pos, q.At), p.Pos.Dist(q.At))
		default:
			return "Дело не сделано. Возвращайся с победой."
		}
	}
	if q.Kind == "relics" {
		left := q.Need
		kept := pl.Inventory[:0]
		for _, st := range pl.Inventory {
			if st.Key == q.Item && left > 0 {
				take := min(left, max(1, st.Qty))
				left -= take
				st.Qty -= take
				if st.Qty <= 0 {
					continue
				}
			}
			kept = append(kept, st)
		}
		pl.Inventory = kept
	}
	pl.Quests = slices.DeleteFunc(pl.Quests, func(x Quest) bool { return x.Unique == u.Key })
	pl.Unlocks = append(pl.Unlocks, "quest:"+u.Key)
	pl.Gold += q.Gold
	g.GiveXP(p, q.XP)
	what := g.unlock(p, u.Reward)
	g.FX(p.Level, p.Pos, "Награда!", 0, "#ff80ff", 1800)
	g.chronicle("%s исполнил(а) просьбу %s и получил(а) %s", p.Name, u.Name, what)
	pl.Dirty = true
	return u.Done
}

func sourcesText(src []string) string {
	names := map[string]string{
		"desert": "пустыня", "ash": "пепельные пустоши", "volcano": "огненные недра", "temple": "гробницы",
		"cursed": "проклятые земли", "fortress": "цитадели", "crypt": "склепы", "swamp": "болота",
		"ice": "ледяные пещеры", "tundra": "тундра", "hills": "холмы", "cave": "пещеры", "forest": "леса", "plains": "равнины",
	}
	var out []string
	for _, s := range src {
		if n, ok := names[s]; ok {
			out = append(out, n)
		} else {
			out = append(out, s)
		}
	}
	return strings.Join(out, ", ")
}

// QuestText describes a quest for the journal.
func QuestText(q *Quest) string {
	name := q.Monster
	if d := content.Monster(q.Monster); d != nil {
		name = d.Name
	}
	switch q.Kind {
	case "slay":
		return fmt.Sprintf("сразить чемпиона «%s»", q.Where)
	case "boss":
		if q.Where != "" {
			return fmt.Sprintf("сразить: %s (%s)", name, q.Where)
		}
		return "сразить: " + name
	case "relics":
		item := q.Item
		if d := content.Item(q.Item); d != nil {
			item = d.Name
		}
		return fmt.Sprintf("собрать: %s ×%d (%s)", item, q.Need, sourcesText(q.Sources))
	}
	return fmt.Sprintf("убить: %s ×%d", name, q.Need)
}
