package game

import (
	"fmt"
	"math"
	"slices"
	"strconv"
	"strings"

	"ratas/internal/content"
	"ratas/internal/world"
)

// Admin mode is for testing: the host started with -admin (or everyone on a
// dedicated server started with -admin) can type commands into the chat.

const adminColor = "#ff9a3a"

// AdminCommand describes one command for the help and the quick panel.
type AdminCommand struct {
	Name, Args, Desc string
}

// AdminCommands lists the commands in the order of the help.
var AdminCommands = []AdminCommand{
	{"/god", "", "бессмертие вкл/выкл"},
	{"/nocd", "", "умения без маны и перезарядки вкл/выкл"},
	{"/heal", "", "полное здоровье и мана, снять эффекты"},
	{"/level", "N", "поднять героя до уровня N"},
	{"/xp", "N", "дать N опыта"},
	{"/gold", "N", "дать N золота"},
	{"/points", "N", "дать N очков навыков и характеристик"},
	{"/give", "предмет [кол-во] [редкость]", "дать предмет по ключу или части названия"},
	{"/items", "[фильтр]", "список предметов"},
	{"/spawn", "монстр [кол-во] [уровень]", "призвать монстров рядом"},
	{"/monsters", "[фильтр]", "список монстров"},
	{"/kill", "[радиус]", "убить врагов вокруг (по умолчанию 12)"},
	{"/tp", "X Y | уровень", "телепорт: в точку или на уровень (d3-2, overworld)"},
	{"/levels", "", "список уровней мира"},
	{"/find", "имя", "телепорт к NPC, уникальному персонажу или монстру"},
	{"/unique", "ключ", "поставить уникального персонажа рядом (даже если его нет в мире)"},
	{"/uniques", "", "список уникальных персонажей"},
	{"/unlock", "all | class:ключ | subclass:ключ | skill:ключ", "открыть секреты"},
	{"/reveal", "", "открыть карту текущего уровня"},
	{"/time", "day | night | ЧЧ", "сменить время суток"},
	{"/speed", "N", "бонус к скорости бега в процентах (0 — снять)"},
}

// Admin runs an admin command line; the caller checks the rights.
func (g *Game) Admin(e *Entity, line string) {
	if e == nil || e.Player == nil {
		return
	}
	f := strings.Fields(strings.TrimPrefix(strings.TrimSpace(line), "/"))
	if len(f) == 0 {
		return
	}
	cmd, args := strings.ToLower(f[0]), f[1:]
	num := func(i, def int) int {
		if i < len(args) {
			if v, err := strconv.Atoi(args[i]); err == nil {
				return v
			}
		}
		return def
	}
	p := e.Player
	say := func(format string, a ...any) { g.Log(e, adminColor, format, a...) }
	defer func() { p.Dirty = true }()
	switch cmd {
	case "help", "?":
		for _, c := range AdminCommands {
			say("%s %s — %s", c.Name, c.Args, c.Desc)
		}
	case "god":
		p.God = !p.God
		say("Бессмертие: %s.", onOff(p.God))
	case "nocd":
		p.NoCD = !p.NoCD
		say("Без маны и перезарядки: %s.", onOff(p.NoCD))
	case "heal":
		e.Buffs = nil
		e.Recalc()
		e.HP, e.MP = e.MaxHP, e.MaxMP
		say("Здоровье и мана восстановлены.")
	case "level", "lvl":
		n := min(99, num(0, p.Level+1))
		for p.Level < n {
			g.GiveXP(e, XPForLevel(p.Level)-p.XP)
		}
		say("Уровень героя: %d.", p.Level)
	case "xp":
		g.GiveXP(e, num(0, 1000))
	case "gold":
		n := num(0, 1000)
		p.Gold += n
		say("+%d золота.", n)
	case "points":
		n := num(0, 10)
		p.SkillPoints += n
		p.AttrPoints += n
		say("+%d очков навыков и характеристик.", n)
	case "give":
		g.adminGive(e, args)
	case "items":
		g.adminList(e, args, itemKeys())
	case "spawn":
		g.adminSpawn(e, args)
	case "monsters":
		g.adminList(e, args, monsterKeys())
	case "kill":
		r := num(0, 12)
		n := 0
		for _, o := range g.onLevel(e.Level) {
			if o.Monster != nil && o.Alive() && g.hostile(e, o) && o.Pos.Dist(e.Pos) <= r {
				o.HP = 0
				g.kill(o, e)
				n++
			}
		}
		say("Убито врагов: %d.", n)
	case "tp":
		g.adminTeleport(e, args)
	case "levels":
		ids := make([]string, 0, len(g.Levels))
		for id := range g.Levels {
			ids = append(ids, id)
		}
		slices.Sort(ids)
		for _, id := range ids {
			say("%s — %s", id, g.Levels[id].Name)
		}
		say("Подземелий: %d (d<номер>-<глубина>, ещё не созданные уровни создаются при входе).", len(g.Entrances))
	case "find":
		g.adminFind(e, strings.Join(args, " "))
	case "unique":
		g.adminUnique(e, strings.Join(args, " "))
	case "uniques":
		for _, u := range content.Uniques() {
			where := "нет в этом мире"
			for _, o := range g.Entities {
				if o.NPC != nil && o.NPC.Unique == u.Key {
					where = fmt.Sprintf("(%d,%d)", o.Pos.X, o.Pos.Y)
				}
			}
			say("%s — %s, %s: %s → %s", u.Key, u.Name, lower(u.Title), where, u.Reward)
		}
	case "unlock":
		g.adminUnlock(e, args)
	case "reveal":
		l := g.Levels[e.Level]
		bs := world.NewBitset(l.W * l.H)
		for i := 0; i < l.W*l.H; i++ {
			bs.Set(i)
		}
		p.Explored[l.ID] = bs
		if l.ID == "overworld" {
			p.Found = p.Found[:0]
			for i := range g.Landmarks {
				p.Found = append(p.Found, i)
			}
		}
		p.Resync = true
		say("Карта уровня открыта.")
	case "time":
		target := 0.5
		switch {
		case len(args) == 0 || args[0] == "day":
		case args[0] == "night":
			target = 0.0
		default:
			target = float64(num(0, 12)%24) / 24
		}
		// moving the clock forward keeps every timer consistent
		g.Now += math.Mod(target-g.TimeOfDay()+1, 1) * DayMs
		say("Время: %02d:00.", int(math.Round(target*24))%24)
	case "speed":
		n := num(0, 100)
		e.Buffs = slices.DeleteFunc(e.Buffs, func(b Buff) bool { return b.Def.Key == "admin_speed" })
		if n != 0 {
			g.applyBuff(e, &content.BuffDef{Key: "admin_speed", Name: "Скорость (админ)", DurationMs: 24 * 3600000,
				Stats: map[string]float64{"move_speed": float64(n)}}, e.ID)
		}
		e.Recalc()
		say("Бонус к скорости бега: %d%%.", n)
	default:
		say("Неизвестная команда /%s. Список: /help.", cmd)
	}
}

func onOff(b bool) string {
	if b {
		return "вкл"
	}
	return "выкл"
}

func itemKeys() []string {
	var out []string
	for _, it := range content.Items() {
		if it.Kind != "gold" {
			out = append(out, it.Key+" — "+it.Name)
		}
	}
	return out
}

func monsterKeys() []string {
	var out []string
	for _, m := range content.Monsters() {
		out = append(out, m.Key+" — "+m.Name)
	}
	return out
}

func (g *Game) adminList(e *Entity, args []string, all []string) {
	filter := strings.ToLower(strings.Join(args, " "))
	n := 0
	for _, s := range all {
		if filter != "" && !strings.Contains(strings.ToLower(s), filter) {
			continue
		}
		if n == 40 {
			g.Log(e, adminColor, "…и ещё. Уточните фильтр.")
			return
		}
		g.Log(e, adminColor, "%s", s)
		n++
	}
	if n == 0 {
		g.Log(e, adminColor, "Ничего не найдено.")
	}
}

// findDef finds a definition by exact key, then by a part of the key or name.
func findDef[T any](all []T, key func(*T) string, name func(*T) string, q string) *T {
	q = strings.ToLower(strings.TrimSpace(q))
	if q == "" {
		return nil
	}
	k := strings.ReplaceAll(q, " ", "_") // "long sword" is long_sword
	for i := range all {
		if key(&all[i]) == k {
			return &all[i]
		}
	}
	for i := range all {
		if strings.Contains(key(&all[i]), k) || strings.Contains(strings.ToLower(name(&all[i])), q) {
			return &all[i]
		}
	}
	return nil
}

// splitTail separates trailing numbers and words from a name of several words.
func splitTail(args []string, isTail func(string) bool) (name string, tail []string) {
	i := len(args)
	for i > 1 && isTail(args[i-1]) {
		i--
	}
	return strings.Join(args[:i], " "), args[i:]
}

func isNumber(s string) bool {
	_, err := strconv.Atoi(s)
	return err == nil
}

func (g *Game) adminGive(e *Entity, args []string) {
	name, tail := splitTail(args, func(s string) bool { return isNumber(s) || RarityByName(s) >= 0 })
	d := findDef(content.Items(), func(t *content.ItemDef) string { return t.Key }, func(t *content.ItemDef) string { return t.Name }, name)
	if d == nil {
		g.Log(e, adminColor, "Нет такого предмета: %q. Список: /items фильтр.", name)
		return
	}
	qty, rarity := 1, -1
	for _, t := range tail {
		if v, err := strconv.Atoi(t); err == nil {
			qty = max(1, min(v, 999))
		} else {
			rarity = RarityByName(t)
		}
	}
	if d.Kind == "gold" {
		e.Player.Gold += qty
		return
	}
	var last ItemStack
	for i := 0; i < qty; i++ {
		st := ItemStack{Key: d.Key, Qty: 1}
		if rarity >= 0 && slotFor(d) != "" {
			st = g.rollRarity(st, Rarity(rarity), max(1, e.Player.Level))
		}
		if !g.addItem(e, st) {
			g.dropItem(e.Level, e.Pos, st)
		}
		last = st
	}
	g.Log(e, adminColor, "Получено: %s ×%d (%s).", last.Name(), qty, lower(RarityName(int(last.ItemRarity()))))
}

func (g *Game) adminSpawn(e *Entity, args []string) {
	name, tail := splitTail(args, isNumber)
	d := findDef(content.Monsters(), func(t *content.MonsterDef) string { return t.Key }, func(t *content.MonsterDef) string { return t.Name }, name)
	if d == nil {
		g.Log(e, adminColor, "Нет такого монстра: %q. Список: /monsters фильтр.", name)
		return
	}
	n, lvl := 1, max(1, e.Player.Level)
	if len(tail) > 0 {
		n, _ = strconv.Atoi(tail[0])
		n = max(1, min(n, 30))
	}
	if len(tail) > 1 {
		lvl, _ = strconv.Atoi(tail[1])
		lvl = max(1, min(lvl, 99))
	}
	l := g.Levels[e.Level]
	front := e.Pos.Add(e.Facing.Delta()).Add(e.Facing.Delta())
	for i := 0; i < n; i++ {
		g.newMonster(d, e.Level, findFree(l, front), lvl)
	}
	g.indexLevels()
	g.Log(e, adminColor, "Призвано: %s ×%d (уровень %d).", d.Name, n, lvl)
}

func (g *Game) adminTeleport(e *Entity, args []string) {
	if len(args) >= 2 && isNumber(args[0]) && isNumber(args[1]) {
		x, _ := strconv.Atoi(args[0])
		y, _ := strconv.Atoi(args[1])
		l := g.Levels[e.Level]
		if !l.In(x, y) {
			g.Log(e, adminColor, "Точка вне уровня (%d×%d).", l.W, l.H)
			return
		}
		g.changeLevel(e, l, findFree(l, world.Pos{X: x, Y: y}))
		return
	}
	if len(args) == 1 {
		l := g.Level(args[0])
		if l == nil {
			g.Log(e, adminColor, "Нет уровня %q. Список: /levels.", args[0])
			return
		}
		p := l.Up
		if l.Depth == 0 {
			p = g.Start
		}
		g.changeLevel(e, l, p)
		return
	}
	g.Log(e, adminColor, "Использование: /tp X Y или /tp уровень.")
}

func (g *Game) adminFind(e *Entity, q string) {
	q = strings.ToLower(strings.TrimSpace(q))
	if q == "" {
		g.Log(e, adminColor, "Использование: /find имя.")
		return
	}
	var best *Entity
	for _, o := range g.Entities {
		if o == e || (o.NPC == nil && o.Monster == nil) || !o.Alive() {
			continue
		}
		match := strings.Contains(strings.ToLower(o.Name), q)
		if o.NPC != nil && (o.NPC.Unique == q || o.NPC.Role == q) {
			match = true
		}
		if o.Monster != nil && o.Monster.Def == q {
			match = true
		}
		if !match {
			continue
		}
		if best == nil || (o.Level == e.Level && (best.Level != e.Level || o.Pos.Dist(e.Pos) < best.Pos.Dist(e.Pos))) {
			best = o
		}
	}
	if best == nil {
		g.Log(e, adminColor, "Никого не найдено по %q.", q)
		return
	}
	g.MoveForTest(e, best)
	g.Log(e, adminColor, "Вы рядом: %s (%s, %d,%d).", best.Name, best.Level, best.Pos.X, best.Pos.Y)
}

func (g *Game) adminUnique(e *Entity, q string) {
	u := findDef(content.Uniques(), func(t *content.UniqueDef) string { return t.Key }, func(t *content.UniqueDef) string { return t.Name }, q)
	if u == nil {
		g.Log(e, adminColor, "Нет такого персонажа: %q. Список: /uniques.", q)
		return
	}
	for _, o := range g.Entities {
		if o.NPC != nil && o.NPC.Unique == u.Key {
			g.MoveForTest(e, o)
			g.Log(e, adminColor, "%s уже в мире — вы рядом.", u.Name)
			return
		}
	}
	if e.Level != "overworld" {
		g.Log(e, adminColor, "Уникальные персонажи живут на поверхности.")
		return
	}
	l := g.Levels[e.Level]
	p := findFree(l, e.Pos.Add(e.Facing.Delta()).Add(e.Facing.Delta()))
	g.spawnUnique(u, p, 60)
	g.indexLevels()
	g.Log(e, adminColor, "%s появляется рядом.", u.Name)
}

func (g *Game) adminUnlock(e *Entity, args []string) {
	if len(args) == 0 {
		g.Log(e, adminColor, "Использование: /unlock all | class:ключ | subclass:ключ | skill:ключ.")
		return
	}
	var rewards []string
	if args[0] == "all" {
		for _, c := range content.Classes() {
			if c.Secret {
				rewards = append(rewards, "class:"+c.Key)
			}
		}
		for _, s := range content.Subclasses() {
			if s.Secret {
				rewards = append(rewards, "subclass:"+s.Key)
			}
		}
		for _, s := range content.Skills() {
			if b := content.Branch(s.Branch); b != nil && b.Secret {
				rewards = append(rewards, "skill:"+s.Key)
			}
		}
	} else {
		rewards = args
	}
	for _, r := range rewards {
		g.unlock(e, r)
	}
}
