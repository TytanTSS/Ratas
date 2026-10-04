package game

import (
	"fmt"
	"math"
	"slices"
	"strings"

	"ratas/internal/content"
	"ratas/internal/llm"
	"ratas/internal/proto"
	"ratas/internal/world"
)

type dialogueOption struct {
	label  string
	action string
}

func (g *Game) dialogueOptions(p, npc *Entity) []dialogueOption {
	if u := content.Unique(npc.NPC.Unique); u != nil {
		opts := []dialogueOption{{"Кто ты?", "about"}}
		switch q := g.uniqueQuest(p, u.Key); {
		case uniqueDone(p, u.Key):
			opts = append(opts, dialogueOption{"Спасибо за всё", "thanks"})
		case q == nil:
			opts = append(opts, dialogueOption{"Чем я могу помочь?", "uoffer"})
		case q.Done || q.Kind == "relics":
			opts = append(opts, dialogueOption{"Я выполнил твою просьбу", "ufinish"})
		default:
			opts = append(opts, dialogueOption{"Напомни, что нужно сделать", "ufinish"})
		}
		return append(opts, dialogueOption{"Что слышно в мире?", "rumor"}, dialogueOption{"Прощай", "bye"})
	}
	role := content.NPCRole(npc.NPC.Role)
	opts := []dialogueOption{{"Как жизнь?", "mood"}, {"Что слышно в округе?", "rumor"}}
	if role != nil && len(role.Stories) > 0 {
		opts = append(opts, dialogueOption{"Расскажи о себе", "story"})
	}
	if role != nil && role.QuestGiver {
		opts = append(opts, dialogueOption{"Есть работа?", "quest"})
	}
	if role != nil && role.Trader {
		opts = append(opts, dialogueOption{"Покажи товары", "trade"})
	}
	opts = append(opts, g.serviceOptions(p, npc)...)
	return append(opts, dialogueOption{"Прощай", "bye"})
}

func labels(opts []dialogueOption) []string {
	out := make([]string, len(opts))
	for i, o := range opts {
		out[i] = o.label
	}
	return out
}

func (g *Game) npcTitle(npc *Entity) (string, string) {
	if u := content.Unique(npc.NPC.Unique); u != nil {
		return u.Name, u.Title
	}
	role := content.NPCRole(npc.NPC.Role)
	rn := npc.NPC.Role
	if role != nil {
		rn = role.Name
	}
	return npc.NPC.PName, rn
}

func (g *Game) sendDialogue(p, npc *Entity, text string, trade bool, waiting bool) {
	name, role := g.npcTitle(npc)
	d := &proto.Dialogue{
		NPC: npc.ID, Name: name, Role: role, Text: text,
		Options: labels(g.dialogueOptions(p, npc)), AI: g.Brain.Enabled(), Waiting: waiting,
	}
	if trade {
		d.Trade = g.tradeList(npc)
	}
	g.box(p).Dialogue = d
}

func (g *Game) openDialogue(p, npc *Entity) {
	if npc.NPC == nil || p.Dead || g.hostile(p, npc) {
		return
	}
	pl := p.Player
	pl.Talking = npc.ID
	npc.Facing = world.DirTowards(npc.Pos, p.Pos)
	text := g.greeting(p, npc)
	if reward := g.turnInQuests(p, npc); reward != "" {
		text = reward
	}
	g.sendDialogue(p, npc, text, false, false)
}

// greeting depends on who greets whom, when, and what the hero has done.
func (g *Game) greeting(p, npc *Entity) string {
	n := npc.NPC
	if n.Met == nil {
		n.Met = map[string]int{}
	}
	met := n.Met[p.Name]
	n.Met[p.Name]++
	rep := strings.NewReplacer("{village}", n.Village, "{player}", p.Name)
	if u := content.Unique(n.Unique); u != nil {
		if met > 0 {
			return rep.Replace(g.pick(npc, []string{"Снова ты, {player}. Я ждал.", "Вернулся? Хорошо.", "А, {player}. Говори."}))
		}
		return rep.Replace(u.Greeting)
	}
	role := content.NPCRole(n.Role)
	var parts []string
	switch {
	case p.HP < p.MaxHP*0.4:
		parts = append(parts, g.pick(npc, []string{"Да ты весь в крови!", "Эк тебя потрепало!", "Тебе бы к знахарке, путник."}))
	case g.Levels[npc.Level].Lit && g.IsNight() && n.Village != "":
		parts = append(parts, g.pick(npc, []string{"Поздно ты гуляешь.", "Ночь на дворе, а ты всё бродишь.", "В такую темень добрые люди по домам сидят."}))
	}
	if met > 0 && g.chance(60) {
		parts = append(parts, rep.Replace(g.pick(npc, []string{"Снова ты, {player}!", "А, {player}, рад видеть.", "Опять ты? Ну, заходи.", "{player}! Как дорога?"})))
	} else if role != nil && len(role.Greetings) > 0 {
		parts = append(parts, rep.Replace(role.Greetings[g.rng.IntN(len(role.Greetings))]))
	}
	if b := p.Player.Bosses; len(b) > 0 && g.chance(30) {
		if d := content.Monster(b[g.rng.IntN(len(b))]); d != nil {
			parts = append(parts, fmt.Sprintf("Говорят, это ты одолел %s? Уважаю.", d.Name))
		}
	}
	if len(parts) == 0 {
		return "..."
	}
	return strings.Join(parts, " ")
}

// pick returns a line not yet said by this NPC (lines repeat only when all were told).
func (g *Game) pick(npc *Entity, lines []string) string {
	n := npc.NPC
	if n.Said == nil {
		n.Said = map[string]bool{}
	}
	var fresh []string
	for _, l := range lines {
		if !n.Said[l] {
			fresh = append(fresh, l)
		}
	}
	if len(fresh) == 0 {
		for _, l := range lines {
			delete(n.Said, l)
		}
		fresh = lines
	}
	if len(fresh) == 0 {
		return "..."
	}
	l := fresh[g.rng.IntN(len(fresh))]
	n.Said[l] = true
	return l
}

func (g *Game) closeDialogue(p *Entity, notify bool) {
	if p.Player == nil || p.Player.Talking == 0 {
		return
	}
	p.Player.Talking = 0
	if notify {
		g.box(p).Dialogue = &proto.Dialogue{Close: true}
	}
}

func (g *Game) talkingTo(p *Entity) *Entity {
	npc := g.Entities[p.Player.Talking]
	if npc == nil || npc.NPC == nil || g.hostile(p, npc) {
		return nil
	}
	return npc
}

func (g *Game) talkOption(p *Entity, idx int) {
	npc := g.talkingTo(p)
	if npc == nil {
		g.closeDialogue(p, true)
		return
	}
	opts := g.dialogueOptions(p, npc)
	if idx < 0 || idx >= len(opts) {
		return
	}
	role := content.NPCRole(npc.NPC.Role)
	u := content.Unique(npc.NPC.Unique)
	text := "..."
	switch opts[idx].action {
	case "mood":
		text = g.moodLine(npc)
	case "rumor":
		text = g.rumor(p, npc)
	case "story":
		text = g.pick(npc, role.Stories)
	case "quest":
		text = g.cannedQuest(p, npc)
	case "trade":
		g.sendDialogue(p, npc, "Смотри, выбирай. Цены честные!", true, false)
		return
	case "about":
		text = g.pick(npc, u.About)
	case "uoffer":
		text = g.offerUniqueQuest(p, npc, u)
	case "ufinish":
		text = g.finishUniqueQuest(p, u)
	case "thanks":
		text = g.pick(npc, []string{"Это я должен благодарить тебя.", "Ступай со светом. Наши пути ещё пересекутся.", fmt.Sprintf("Мир тесен, %s. Ещё увидимся.", p.Name)})
	case "rest", "upgrade", "song", "bless":
		text = g.service(p, npc, opts[idx].action)
	case "bye":
		g.closeDialogue(p, true)
		return
	}
	g.sendDialogue(p, npc, text, false, false)
}

// moodLine: how the NPC feels, depending on what goes on around.
func (g *Game) moodLine(npc *Entity) string {
	var lines []string
	threats := 0
	for _, o := range g.onLevel(npc.Level) {
		if o.Faction == FMonster && o.Kind == KMonster && o.Pos.Dist(npc.Pos) <= 25 {
			threats++
		}
	}
	switch {
	case threats >= 3:
		lines = append(lines, "Чудища бродят у самой околицы. Страшно мне, честно скажу.", "Слышишь? Опять что-то воет за частоколом.")
	case threats == 0:
		lines = append(lines, "Тихо нынче, хвала небесам.", "Спокойный денёк выдался. Редкость по нынешним временам.")
	}
	if g.IsNight() {
		lines = append(lines, "Ночью я двери на засов запираю. И тебе советую.", "Не люблю ночь. Мёртвые не спят.")
	} else {
		lines = append(lines, "Работы невпроворот, а день короткий.", "Солнце греет — уже хорошо.")
	}
	if l := g.Levels["overworld"]; l != nil {
		if r := g.regionIndex(npc.Pos); r != 0 && g.Regions[r-1].Danger >= 2 {
			lines = append(lines, "Места у нас гиблые. Каждый день как последний.")
		}
	}
	if role := content.NPCRole(npc.NPC.Role); role != nil {
		lines = append(lines, role.Lines...)
	}
	return g.pick(npc, lines)
}

// rumor is a piece of real news about the world.
func (g *Game) rumor(p, npc *Entity) string {
	var lines []string
	if len(g.Chronicle) > 0 {
		for _, c := range g.Chronicle[max(0, len(g.Chronicle)-4):] {
			lines = append(lines, fmt.Sprintf("Слыхал новость? %s!", c))
		}
	}
	for _, e := range g.Entrances {
		if e.Pos.Dist(npc.Pos) < 120 {
			lines = append(lines, fmt.Sprintf("%s отсюда, шагах в %d, есть %s — «%s». Там, говорят, %d ярусов, и на дне сидит кто-то страшный.",
				upperFirst(compassRU(npc.Pos, e.Pos)), roundTo(npc.Pos.Dist(e.Pos), 10), themeWord(e.Theme), e.Name, e.MaxDepth))
		}
	}
	for _, lm := range g.Landmarks {
		if lm.Pos.Dist(npc.Pos) < 90 {
			lines = append(lines, fmt.Sprintf("%s есть %s. %s", upperFirst(compassRU(npc.Pos, lm.Pos)), lm.Name, landmarkWord(lm.Kind)))
		}
	}
	for _, o := range g.onLevel("overworld") {
		if o.NPC == nil || o.NPC.Unique == "" || o == npc {
			continue
		}
		if u := content.Unique(o.NPC.Unique); u != nil {
			lines = append(lines, fmt.Sprintf("Ходят слухи, что %s живёт %s — %s. Говорят, награда у него за помощь такая, что и не снилась.",
				compassRU(npc.Pos, o.Pos), u.Name, lower(u.Title)))
		}
	}
	for _, r := range g.Regions {
		if r.Danger >= 2 {
			lines = append(lines, fmt.Sprintf("Держись подальше от земель «%s». Оттуда мало кто возвращается.", r.Name))
		}
	}
	for _, m := range content.Monsters() {
		if m.Boss {
			lines = append(lines, fmt.Sprintf("Старики шепчут, что %s ещё жив и копит силы.", m.Name))
		}
	}
	if len(lines) == 0 {
		return "Ничего нового, путник."
	}
	return g.pick(npc, lines)
}

func roundTo(v, k int) int { return max(k, (v+k/2)/k*k) }

func themeWord(theme string) string {
	switch theme {
	case "cave":
		return "пещера"
	case "crypt":
		return "старый склеп"
	case "ice":
		return "ледяная пещера"
	case "volcano":
		return "огненный провал"
	case "temple":
		return "песчаная гробница"
	case "fortress":
		return "чёрная цитадель"
	}
	return "подземелье"
}

func landmarkWord(kind string) string {
	switch kind {
	case "shrine", "circle":
		return "Помолишься там — и сил прибудет."
	case "ruins":
		return "Камни помнят старую войну, а в развалинах, бывает, находят клады."
	case "graveyard":
		return "Ночью туда лучше не соваться — мертвецы встают."
	case "camp":
		return "Там засели разбойники. Обходи стороной."
	case "oasis":
		return "Вода там чистая, а пальмы — как в сказке."
	}
	return "Странное место."
}

// ---- quests ----

func (g *Game) turnInQuests(p, npc *Entity) string {
	pl := p.Player
	var parts []string
	kept := pl.Quests[:0]
	for _, q := range pl.Quests {
		if q.Done && q.GiverID == npc.ID && q.Unique == "" {
			pl.Gold += q.Gold
			g.GiveXP(p, q.XP)
			g.deed(p, "quests", 1)
			parts = append(parts, fmt.Sprintf("%d золота", q.Gold))
			g.Log(p, "#ffd24a", "Награда за задание: %d золота, %d опыта.", q.Gold, q.XP)
			continue
		}
		kept = append(kept, q)
	}
	pl.Quests = kept
	if len(parts) == 0 {
		return ""
	}
	pl.Dirty = true
	return fmt.Sprintf("Ты справился! Деревня тебе благодарна. Держи награду: %s.", strings.Join(parts, ", "))
}

func (g *Game) questMonsters() []*content.MonsterDef {
	var out []*content.MonsterDef
	for i := range content.Monsters() {
		m := &content.Monsters()[i]
		if m.Boss || (m.Depth == [2]int{0, 0} && m.Weight > 0 && !m.Elite) {
			out = append(out, m)
		}
	}
	return out
}

func (g *Game) addQuest(p, npc *Entity, monster string, count int) (*Quest, string) {
	pl := p.Player
	for _, q := range pl.Quests {
		if q.GiverID == npc.ID && !q.Done {
			return nil, "уже есть задание от этого персонажа"
		}
	}
	if len(pl.Quests) >= 5 {
		return nil, "слишком много заданий"
	}
	def := content.Monster(monster)
	if def == nil {
		return nil, "неизвестный монстр"
	}
	if def.Boss {
		count = 1
	}
	count = max(1, min(10, count))
	g.questSeq++
	q := Quest{
		ID: g.questSeq, Giver: npc.NPC.PName, GiverID: npc.ID, Village: npc.NPC.Village,
		Monster: monster, Need: count, Gold: 15 + count*def.XP/2, XP: count * def.XP * 3 / 2,
	}
	if def.Boss {
		q.Gold, q.XP = 150, def.XP
	}
	pl.Quests = append(pl.Quests, q)
	pl.Dirty = true
	g.Log(p, "#ffd24a", "Новое задание — охота: %s ×%d (награда %d золота). Журнал — J.", def.Name, count, q.Gold)
	return &pl.Quests[len(pl.Quests)-1], ""
}

func (g *Game) cannedQuest(p, npc *Entity) string {
	for _, q := range p.Player.Quests {
		if q.GiverID == npc.ID && !q.Done {
			name := q.Monster
			if d := content.Monster(q.Monster); d != nil {
				name = d.Name
			}
			return fmt.Sprintf("Ты ещё не закончил: %s — %d из %d.", name, q.Have, q.Need)
		}
	}
	ms := g.questMonsters()
	var pool []*content.MonsterDef
	for _, m := range ms {
		if !m.Boss || p.Player.Level >= 6 {
			pool = append(pool, m)
		}
	}
	if len(pool) == 0 {
		return "Пока работы нет."
	}
	m := pool[g.rng.IntN(len(pool))]
	q, why := g.addQuest(p, npc, m.Key, 3+g.rng.IntN(5))
	if q == nil {
		return fmt.Sprintf("Сначала разберись с другими делами (%s).", why)
	}
	if m.Boss {
		return fmt.Sprintf("Говорят, в подземелье правит %s. Уничтожь это зло — и %d золота твои.", m.Name, q.Gold)
	}
	return fmt.Sprintf("Нас одолевают твари. Убей %d: %s — и получишь %d золота.", q.Need, m.Name, q.Gold)
}

// ---- trade ----

func SellPrice(st ItemStack) int { return max(1, st.Value()*2/5) }

func (g *Game) sell(p *Entity, idx int) {
	npc := g.talkingTo(p)
	if npc == nil {
		return
	}
	role := content.NPCRole(npc.NPC.Role)
	if role == nil || !role.Trader {
		return
	}
	pl := p.Player
	if idx < 0 || idx >= len(pl.Inventory) {
		return
	}
	st := pl.Inventory[idx]
	price := SellPrice(st)
	if st.Qty > 1 {
		pl.Inventory[idx].Qty--
		pl.Dirty = true
	} else {
		g.removeInvItem(p, idx)
	}
	pl.Gold += price
	g.Log(p, "#ffd700", "Продано: %s за %d золота.", st.Name(), price)
}

// ---- AI conversation ----

func compass(from, to world.Pos) string {
	dx, dy := float64(to.X-from.X), float64(to.Y-from.Y)*2
	ang := math.Atan2(-dy, dx) * 180 / math.Pi
	if ang < 0 {
		ang += 360
	}
	dirs := []string{"east", "north-east", "north", "north-west", "west", "south-west", "south", "south-east"}
	return dirs[int((ang+22.5)/45)%8]
}

func compassRU(from, to world.Pos) string {
	m := map[string]string{"east": "на востоке", "north-east": "на северо-востоке", "north": "на севере", "north-west": "на северо-западе",
		"west": "на западе", "south-west": "на юго-западе", "south": "на юге", "south-east": "на юго-востоке"}
	return m[compass(from, to)]
}

func (g *Game) worldFacts(npc *Entity) []string {
	var facts []string
	type dist struct {
		i int
		d int
	}
	var ds []dist
	for i, e := range g.Entrances {
		ds = append(ds, dist{i, e.Pos.Manhattan(npc.Pos)})
	}
	slices.SortFunc(ds, func(a, b dist) int { return a.d - b.d })
	for k, d := range ds {
		if k >= 4 {
			break
		}
		e := g.Entrances[d.i]
		facts = append(facts, fmt.Sprintf("The dungeon \"%s\" (%s, %d levels deep) lies to the %s, about %d steps away.", e.Name, themeWord(e.Theme), e.MaxDepth, compass(npc.Pos, e.Pos), d.d))
	}
	for _, v := range g.Villages {
		if v.Name != npc.NPC.Village {
			facts = append(facts, fmt.Sprintf("The village %s lies to the %s.", v.Name, compass(npc.Pos, v.Center)))
		}
	}
	for _, lm := range g.Landmarks {
		if lm.Pos.Dist(npc.Pos) < 100 {
			facts = append(facts, fmt.Sprintf("Landmark: %s (%s) to the %s.", lm.Name, lm.Kind, compass(npc.Pos, lm.Pos)))
		}
	}
	for _, o := range g.onLevel("overworld") {
		if o.NPC != nil && o.NPC.Unique != "" && o != npc {
			if u := content.Unique(o.NPC.Unique); u != nil {
				facts = append(facts, fmt.Sprintf("Rumour: %s, %s, lives to the %s and rewards those who help with something extraordinary.", u.Name, u.Title, compass(npc.Pos, o.Pos)))
			}
		}
	}
	for _, r := range g.Regions {
		if r.Danger >= 2 {
			facts = append(facts, fmt.Sprintf("The lands called %s (%s) are deadly.", r.Name, r.Kind))
		}
	}
	for _, m := range content.Monsters() {
		if m.Boss {
			facts = append(facts, fmt.Sprintf("Rumour: %s rules the deepest level of a %s.", m.Name, strings.Join(m.Themes, "/")))
		}
	}
	for _, c := range g.Chronicle {
		facts = append(facts, "Recent news people talk about: "+c+".")
	}
	return facts
}

func (g *Game) talkAI(p *Entity, text string) {
	npc := g.talkingTo(p)
	if npc == nil {
		return
	}
	text = strings.TrimSpace(text)
	if rs := []rune(text); len(rs) > 300 {
		text = string(rs[:300])
	}
	if text == "" {
		return
	}
	n := npc.NPC
	role := content.NPCRole(n.Role)
	if !g.Brain.Enabled() {
		line := "Хм..."
		if role != nil && len(role.Lines) > 0 {
			line = role.Lines[g.rng.IntN(len(role.Lines))]
		}
		g.sendDialogue(p, npc, line, false, false)
		return
	}
	if n.Busy {
		g.Log(p, "#808080", "%s ещё обдумывает ответ...", n.PName)
		return
	}
	pl := p.Player
	req := llm.NPCRequest{
		NPCName: n.PName, Village: n.Village, World: g.WorldName, TimeOfDay: g.TimeName(),
		PlayerName: p.Name, PlayerLevel: pl.Level, Message: text, Purse: n.Gold,
		Facts: g.worldFacts(npc), TimesMet: max(0, n.Met[p.Name]-1), Mood: g.moodLine(npc),
	}
	if req.Village == "" {
		req.Village = "the wilds"
	}
	if r := g.regionIndex(npc.Pos); r != 0 {
		reg := g.Regions[r-1]
		req.Region = fmt.Sprintf("%s (%s, danger %d of 3)", reg.Name, reg.Kind, reg.Danger)
	}
	if role != nil {
		req.Role, req.Persona, req.Trader, req.CanGiveQuest = role.Name, role.Persona, role.Trader, role.QuestGiver
	}
	if u := content.Unique(n.Unique); u != nil {
		req.Role, req.Persona = u.Title, u.Persona
		state := "not given yet: the player can ask you about it"
		if q := g.uniqueQuest(p, u.Key); q != nil {
			state = "the player is working on it"
			if q.Done {
				state = "the player has done it and can claim the reward"
			}
		} else if uniqueDone(p, u.Key) {
			state = "already fulfilled by this player"
		}
		req.OwnQuest = fmt.Sprintf("%s Reward: %s. State: %s.", u.Offer, RewardName(u.Reward), state)
	}
	var classes []string
	for _, ck := range pl.Classes {
		if c := content.Class(ck); c != nil {
			name := c.Name
			if sc := content.Subclass(pl.Subclasses[ck]); sc != nil {
				name += " (" + sc.Name + ")"
			}
			classes = append(classes, name)
		}
	}
	req.PlayerClass = strings.Join(classes, ", ")
	for _, b := range pl.Bosses {
		if d := content.Monster(b); d != nil {
			req.PlayerDeeds = append(req.PlayerDeeds, "slew "+d.Name)
		}
	}
	if pl.Kills > 0 {
		req.PlayerDeeds = append(req.PlayerDeeds, fmt.Sprintf("has killed %d monsters", pl.Kills))
	}
	if p.HP < p.MaxHP*0.4 {
		req.PlayerDeeds = append(req.PlayerDeeds, "is badly wounded right now")
	}
	for _, q := range pl.Quests {
		name := q.Monster
		if d := content.Monster(q.Monster); d != nil {
			name = d.Name
		}
		status := fmt.Sprintf("%d/%d", q.Have, q.Need)
		if q.Done {
			status = "done, reward not yet collected"
		}
		req.PlayerQuests = append(req.PlayerQuests, fmt.Sprintf("slay %s for %s (%s)", name, q.Giver, status))
	}
	for _, t := range n.Memory[p.Name] {
		req.History = append(req.History, llm.Turn{Who: t.Who, Text: t.Text})
	}
	for _, key := range g.giftKeys(npc) {
		req.Gifts = append(req.Gifts, llm.Option{Key: key, Name: content.Item(key).Name})
	}
	for _, m := range g.questMonsters() {
		req.Monsters = append(req.Monsters, llm.Option{Key: m.Key, Name: m.Name})
	}
	n.Busy = true
	g.sendDialogue(p, npc, "", false, true)
	account, npcID := p.Name, npc.ID
	g.Brain.NPCTalk(req, func(r llm.NPCReply, err error) {
		g.Post(func() { g.applyNPCReply(account, npcID, text, r, err) })
	})
}

func (g *Game) giftKeys(npc *Entity) []string {
	var keys []string
	role := content.NPCRole(npc.NPC.Role)
	for _, it := range content.Items() {
		if it.Kind == "consumable" && it.Value <= 20 {
			keys = append(keys, it.Key)
		}
	}
	if role != nil {
		for _, k := range role.Goods {
			if d := content.Item(k); d != nil && d.Value <= 30 && !slices.Contains(keys, k) {
				keys = append(keys, k)
			}
		}
	}
	return keys
}

func (g *Game) applyNPCReply(account string, npcID EntityID, said string, r llm.NPCReply, err error) {
	npc := g.Entities[npcID]
	if npc == nil || npc.NPC == nil {
		return
	}
	n := npc.NPC
	n.Busy = false
	p := g.Online[account]
	if err != nil {
		if p != nil && p.Player.Talking == npcID {
			role := content.NPCRole(n.Role)
			line := "Хм... Прости, задумался."
			if role != nil && len(role.Lines) > 0 {
				line = role.Lines[g.rng.IntN(len(role.Lines))]
			}
			g.sendDialogue(p, npc, line, false, false)
			g.Log(p, "#806060", "(ИИ недоступен: %v)", err)
		}
		return
	}
	say := strings.TrimSpace(r.Say)
	if rs := []rune(say); len(rs) > 400 {
		say = string(rs[:400]) + "…"
	}
	if n.Memory == nil {
		n.Memory = map[string][]Turn{}
	}
	mem := append(n.Memory[account], Turn{"player", said}, Turn{"npc", say})
	if len(mem) > 16 {
		mem = mem[len(mem)-16:]
	}
	n.Memory[account] = mem
	if p == nil {
		return
	}
	g.Say(npc, say, 5000)
	trade := false
	role := content.NPCRole(n.Role)
	switch r.Action {
	case "give_gold":
		amt := max(0, min(r.Gold, n.Gold, 200))
		if amt > 0 {
			n.Gold -= amt
			p.Player.Gold += amt
			p.Player.Dirty = true
			g.Log(p, "#ffd700", "%s даёт вам %d золота.", n.PName, amt)
		}
	case "give_item":
		if slices.Contains(g.giftKeys(npc), r.Item) {
			if n.Gifts == nil {
				n.Gifts = map[string]float64{}
			}
			if g.Now-n.Gifts[account] > 5*60*1000 || n.Gifts[account] == 0 {
				st := ItemStack{Key: r.Item, Qty: 1}
				if g.addItem(p, st) {
					n.Gifts[account] = g.Now
					g.Log(p, "#c0c0ff", "%s дарит вам: %s.", n.PName, st.Name())
				}
			}
		}
	case "heal":
		if n.Gifts == nil {
			n.Gifts = map[string]float64{}
		}
		if g.Now-n.Gifts["heal:"+account] > 60*1000 || n.Gifts["heal:"+account] == 0 {
			n.Gifts["heal:"+account] = g.Now
			g.heal(p, p.MaxHP-p.HP)
			g.Log(p, "#80ff80", "%s лечит ваши раны.", n.PName)
		}
	case "offer_quest":
		if role != nil && role.QuestGiver {
			valid := false
			for _, m := range g.questMonsters() {
				valid = valid || m.Key == r.QuestMonster
			}
			if valid {
				g.addQuest(p, npc, r.QuestMonster, r.QuestCount)
			}
		}
	case "trade":
		trade = role != nil && role.Trader
	case "hostile":
		if p.Player.Talking == npcID {
			g.sendDialogue(p, npc, say, false, false)
		}
		g.npcTurnHostile(npc, p)
		return
	case "end":
		if p.Player.Talking == npcID {
			name, roleName := g.npcTitle(npc)
			g.box(p).Dialogue = &proto.Dialogue{NPC: npcID, Name: name, Role: roleName, Text: say, Options: []string{"Уйти"}, AI: true}
			p.Player.Talking = 0
		}
		return
	}
	if p.Player.Talking == npcID {
		g.sendDialogue(p, npc, say, trade, false)
	}
}

// npcTurnHostile makes an offended NPC attack the player.
func (g *Game) npcTurnHostile(npc, p *Entity) {
	g.closeDialogue(p, true)
	n := npc.NPC
	base := content.Monster("bandit")
	hp := 45.0
	dmg := [2]float64{4, 8}
	if n.Role == "guard" {
		hp, dmg = 80, [2]float64{6, 11}
	}
	npc.Kind = KMonster
	npc.Faction = FMonster
	npc.Color = "#ff5050"
	npc.HP, npc.MaxHP = hp, hp
	npc.Monster = &MonsterState{
		Def: base.Key, Lvl: 1 + p.Player.Level/2, Home: n.Home, Target: p.ID,
		LastSeen: p.Pos, LastSeenAt: g.Now, State: "chase",
		Damage: dmg, Armor: 2, MoveMs: 200, AttackMs: 1000, XP: 20, Persistent: true,
	}
	npc.NPC = nil
	npc.Recalc()
	g.Log(p, "#ff4a4a", "%s нападает на вас!", npc.Name)
}
