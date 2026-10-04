package game

import (
	"fmt"
	"slices"

	"ratas/internal/content"
	"ratas/internal/proto"
	"ratas/internal/world"
)

// Big cities: their traders also sell a daily stock of rare things, their
// craftsmen offer services, and townsfolk keep a day and night schedule.

// stockMarkup: city traders sell their rare stock dearer than its worth.
const stockMarkup = 1.25

func (g *Game) day() int { return int(g.Now / DayMs) }

// isCity reports whether a village name belongs to a big city.
func (g *Game) isCity(village string) bool {
	for _, v := range g.Villages {
		if v.Name == village {
			return v.City
		}
	}
	return false
}

// cityNPC: a citizen of a big city (services and daily stock work only there).
func (g *Game) cityNPC(npc *Entity) bool {
	return npc.NPC != nil && npc.NPC.Village != "" && g.isCity(npc.NPC.Village)
}

// stockFits: an item of one of the kinds or weapon types a trader stocks.
func stockFits(d *content.ItemDef, kinds []string) bool {
	return slices.Contains(kinds, d.Kind) || (d.Kind == "weapon" && slices.Contains(kinds, d.Weapon))
}

// refreshStock renews the daily stock of a city trader: random equipment of
// its kinds, from uncommon to (rarely) legendary.
func (g *Game) refreshStock(npc *Entity) {
	n := npc.NPC
	role := content.NPCRole(n.Role)
	if role == nil || role.Stock <= 0 || !g.cityNPC(npc) {
		n.Stock = nil
		return
	}
	if n.Stock != nil && n.StockDay == g.day() {
		return
	}
	n.StockDay = g.day()
	n.Stock = []ItemStack{}
	var pool []*content.ItemDef
	for i := range content.Items() {
		d := &content.Items()[i]
		if d.Weight > 0 && !d.Unique && d.Rarity == "" && slotFor(d) != "" && stockFits(d, role.StockKinds) {
			pool = append(pool, d)
		}
	}
	if len(pool) == 0 {
		return
	}
	lvl := 3
	for _, p := range g.Online {
		lvl = max(lvl, p.Player.Level)
	}
	for i := 0; i < role.Stock; i++ {
		d := pool[g.rng.IntN(len(pool))]
		r := Uncommon
		switch x := g.rng.Float64() * 100; {
		case x < 3:
			r = Legendary
		case x < 20:
			r = Epic
		case x < 60:
			r = Rare
		}
		n.Stock = append(n.Stock, g.rollRarity(ItemStack{Key: d.Key, Qty: 1}, r, lvl))
	}
}

func stockPrice(st ItemStack) int { return int(float64(st.Value()) * stockMarkup) }

// tradeList: the fixed goods first, then the daily stock.
func (g *Game) tradeList(npc *Entity) []proto.TradeItem {
	role := content.NPCRole(npc.NPC.Role)
	if role == nil || !role.Trader {
		return nil
	}
	var out []proto.TradeItem
	for _, key := range role.Goods {
		st := ItemStack{Key: key, Qty: 1}
		out = append(out, proto.TradeItem{Item: ItemViewOf(st), Price: st.Value()})
	}
	g.refreshStock(npc)
	for _, st := range npc.NPC.Stock {
		out = append(out, proto.TradeItem{Item: ItemViewOf(st), Price: stockPrice(st)})
	}
	return out
}

// buy takes item key at row idx of the trade list.
func (g *Game) buy(p *Entity, key string, idx int) {
	npc := g.talkingTo(p)
	if npc == nil {
		return
	}
	role := content.NPCRole(npc.NPC.Role)
	if role == nil || !role.Trader {
		return
	}
	var st ItemStack
	var price int
	fromStock := -1
	if i := idx - len(role.Goods); i >= 0 && i < len(npc.NPC.Stock) && npc.NPC.Stock[i].Key == key {
		st, price, fromStock = npc.NPC.Stock[i], stockPrice(npc.NPC.Stock[i]), i
	} else if slices.Contains(role.Goods, key) {
		st = ItemStack{Key: key, Qty: 1}
		price = st.Value()
	} else {
		return
	}
	if p.Player.Gold < price {
		g.Log(p, "#ff8080", "Не хватает золота (нужно %d).", price)
		return
	}
	if !g.addItem(p, st) {
		g.Log(p, "#ff8080", "Инвентарь полон!")
		return
	}
	p.Player.Gold -= price
	npc.NPC.Gold += price / 4
	if fromStock >= 0 {
		npc.NPC.Stock = slices.Delete(npc.NPC.Stock, fromStock, fromStock+1)
		g.sendDialogue(p, npc, "Отличный выбор! Такой вещи больше ни у кого нет.", true, false)
	}
	g.Log(p, "#ffd700", "Куплено: %s за %d золота.", st.Name(), price)
}

// ---- services ----

// serviceOptions are the extra conversation lines of a city craftsman.
func (g *Game) serviceOptions(p, npc *Entity) []dialogueOption {
	role := content.NPCRole(npc.NPC.Role)
	if role == nil || !g.cityNPC(npc) {
		return nil
	}
	var opts []dialogueOption
	for _, s := range role.Services {
		switch s {
		case "rest":
			opts = append(opts, dialogueOption{fmt.Sprintf("Снять комнату и поужинать (%d золота)", restPrice(p)), "rest"})
		case "upgrade":
			if st, ok := p.Player.Equip[SlotMain]; ok && st.Key != "" {
				if price, ok := upgradePrice(st); ok {
					opts = append(opts, dialogueOption{fmt.Sprintf("Улучшить оружие в руке (%d золота)", price), "upgrade"})
				}
			}
		case "song":
			opts = append(opts, dialogueOption{fmt.Sprintf("Спой мне песню (%d золота)", songPrice), "song"})
		case "bless":
			opts = append(opts, dialogueOption{fmt.Sprintf("Благослови меня (%d золота)", blessPrice), "bless"})
		}
	}
	return opts
}

const (
	songPrice  = 15
	blessPrice = 30
)

func restPrice(p *Entity) int { return 10 + 2*p.Player.Level }

// upgradePrice: each step of rarity costs more; artifacts and legendary items
// cannot be improved.
func upgradePrice(st ItemStack) (int, bool) {
	d := st.Def()
	r := st.ItemRarity()
	if d == nil || d.Unique || d.Rarity != "" || r >= Legendary {
		return 0, false
	}
	return 60*(1<<int(r)) + d.Value*(int(r)+1), true
}

var (
	restedBuff  = content.BuffDef{Key: "rested", Name: "Отдых", DurationMs: 600000, Stats: map[string]float64{"max_hp": 20, "hp_regen": 1, "mp_regen": 0.5}, Color: "#ffd8a0"}
	inspireBuff = content.BuffDef{Key: "inspired", Name: "Вдохновение", DurationMs: 300000, Stats: map[string]float64{"melee_pct": 10, "spell_pct": 10, "ranged_pct": 10}, Color: "#ff80c0"}
	blessedBuff = content.BuffDef{Key: "blessed", Name: "Благословение", DurationMs: 300000, Stats: map[string]float64{"res_all": 10, "res_shadow": 10}, Color: "#fff0b0"}
)

// service performs a paid service and returns the NPC's answer.
func (g *Game) service(p, npc *Entity, kind string) string {
	pl := p.Player
	pay := func(price int) bool {
		if pl.Gold < price {
			return false
		}
		pl.Gold -= price
		npc.NPC.Gold += price / 2
		pl.Dirty = true
		return true
	}
	switch kind {
	case "rest":
		if !pay(restPrice(p)) {
			return "Комната стоит денег, друг. Приходи, когда разбогатеешь."
		}
		g.applyBuff(p, &restedBuff, npc.ID)
		p.HP, p.MP = p.MaxHP, p.MaxMP
		g.FX(p.Level, p.Pos, "Отдых", 0, restedBuff.Color, 1200)
		return g.pick(npc, []string{"Мягкая постель, горячий ужин — и ты как новенький!", "Выспался? Вот и славно. Дорога ждёт.", "Ужин за счёт заведения. Шучу — уже оплачен."})
	case "upgrade":
		st, ok := pl.Equip[SlotMain]
		price, can := upgradePrice(st)
		if !ok || !can {
			return "С этим я ничего не сделаю."
		}
		if !pay(price) {
			return fmt.Sprintf("Работа тонкая — %d золота, не меньше.", price)
		}
		st = g.rollRarity(st, st.ItemRarity()+1, max(1, pl.Level))
		pl.Equip[SlotMain] = st
		p.Recalc()
		r := st.ItemRarity()
		g.Log(p, RarityColor(int(r)), "Улучшено: %s (%s).", st.Name(), lower(RarityName(int(r))))
		g.FX(p.Level, p.Pos, RarityName(int(r))+"!", 0, RarityColor(int(r)), 1500)
		return fmt.Sprintf("Держи. Перековал, заточил, закалил — теперь это %s клинок, не хуже королевского.", lower(RarityName(int(r))))
	case "song":
		if !pay(songPrice) {
			return "Песня стоит монету, а у тебя и той нет. Ладно, напою бесплатно: ля-ля-ля."
		}
		g.applyBuff(p, &inspireBuff, npc.ID)
		g.Say(npc, fmt.Sprintf("Ла-ла! О герое по имени %s сложат песни!", p.Name), 4000)
		return g.pick(npc, []string{"Эта баллада — о тебе! Иди и сделай её правдой.", "Песня о храбреце, что не знал страха. Узнаёшь?", "Пусть мелодия ведёт твой клинок!"})
	case "bless":
		if !pay(blessPrice) {
			return "Свет не торгует, но храм нуждается в пожертвованиях."
		}
		g.applyBuff(p, &blessedBuff, npc.ID)
		g.FX(p.Level, p.Pos, "Благословение", 0, blessedBuff.Color, 1200)
		return "Да хранит тебя Свет от тьмы и от дурной стали."
	}
	return "..."
}

// ---- schedule ----

// scheduleSpot is where a citizen wants to be now: around the tavern at
// night, at home by day.
func (g *Game) scheduleSpot(e *Entity) world.Pos {
	n := e.NPC
	if n.Night != (world.Pos{}) && g.IsNight() {
		return n.Night
	}
	return n.Home
}

// walkToward takes one step along a path to the target.
func (g *Game) walkToward(e *Entity, to world.Pos) bool {
	l := g.Levels[e.Level]
	path := world.FindPath(l.W, l.H, e.Pos, to, 600, func(x, y int) float64 {
		if (x != to.X || y != to.Y) && !l.Free(x, y) {
			return -1
		}
		if def := l.Def(x, y); def.Interact != "" || def.Damage > 0 {
			return -1
		}
		return 1
	})
	if len(path) == 0 {
		return false
	}
	next := path[0]
	if !l.Free(next.X, next.Y) {
		return false
	}
	e.Facing = world.DirTowards(e.Pos, next)
	g.moveEntity(e, next)
	e.NextMove = g.Now + e.stats.MoveMs*1.5
	return true
}
