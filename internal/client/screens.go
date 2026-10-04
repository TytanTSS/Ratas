package client

import (
	"fmt"
	"math"
	"sort"
	"strings"

	"github.com/gdamore/tcell/v2"

	"ratas/internal/content"
	"ratas/internal/game"
	"ratas/internal/i18n"
	"ratas/internal/proto"
)

// panel returns a centered box rectangle.
func (p *play) panel(wantW, wantH int, title string) (x, y, w, h int) {
	sw, sh := p.a.size()
	w, h = min(wantW, sw-2), min(wantH, sh-2)
	x, y = (sw-w)/2, (sh-h)/2
	p.a.cv.box(x, y, w, h, title, cBorder)
	return
}

func (p *play) footer(x, y, w int, text string) {
	p.a.cv.textClip(x+2, y, w-4, text, cDim, cPanel)
}

var kindNames = map[string]string{
	"weapon": "оружие", "shield": "щит, левая рука", "offhand": "левая рука", "head": "голова", "chest": "грудь",
	"belt": "пояс", "legs": "ноги", "back": "спина", "ring": "кольцо", "consumable": "расходуемое", "quest": "предмет задания",
}

// rarityCol colors item names by rarity; common items stay plain.
func rarityCol(it *proto.ItemView) tcell.Color {
	if it.Rarity <= 0 {
		return cText
	}
	return col(game.RarityColor(int(it.Rarity)))
}

func (p *play) drawInventory() {
	c := p.a.cv
	x, y, w, h := p.panel(96, 34, "Инвентарь")
	if p.sheet == nil {
		return
	}
	listW := min(44, w/2)
	slots, items := p.invRows()
	row := 0
	var selected *proto.ItemView
	line := func(ry int, label string, it *proto.ItemView, selectedRow bool) {
		bg := cPanel
		if selectedRow {
			bg = cSelBG
			c.fill(x+1, ry, listW, 1, ' ', cText, bg)
		}
		if it == nil {
			c.textClip(x+2, ry, listW-2, label+"(пусто)", cDim, bg)
			return
		}
		c.text(x+2, ry, label, cDim, bg)
		c.put(x+2+runeLen(label), ry, it.Glyph, col(it.Color), bg)
		name := it.Name
		if it.Qty > 1 {
			name += fmt.Sprintf(" ×%d", it.Qty)
		}
		c.textClip(x+4+runeLen(label), ry, listW-4-runeLen(label), name, rarityCol(it), bg)
	}
	cy := y + 2
	c.text(x+2, cy-1, "Надето", cAccent, cPanel)
	for _, s := range slots {
		var it *proto.ItemView
		if v, ok := p.sheet.Equip[s]; ok {
			it = &v
		}
		if row == p.sel {
			selected = it
		}
		line(cy, padRight(game.SlotNames[s]+":", 13), it, row == p.sel)
		cy++
		row++
	}
	cy++
	c.text(x+2, cy, fmt.Sprintf("Рюкзак %d/%d", len(items), game.InventorySize), cAccent, cPanel)
	cy++
	maxRows := y + h - 3 - cy
	start := 0
	if idx := p.sel - len(slots); idx >= maxRows {
		start = idx - maxRows + 1
	}
	for i := start; i < len(items) && cy < y+h-2; i++ {
		it := items[i]
		if len(slots)+i == p.sel {
			selected = &items[i]
		}
		line(cy, "", &it, len(slots)+i == p.sel)
		cy++
	}
	// details
	dx := x + listW + 3
	dw := w - listW - 5
	if selected != nil {
		c.put(dx, y+2, selected.Glyph, col(selected.Color), cPanel)
		c.textClip(dx+2, y+2, dw-2, selected.Name, rarityCol(selected), cPanel)
		kind := kindNames[selected.Kind]
		if selected.Kind == "weapon" {
			kind = "одноручное оружие"
			if selected.Hands >= 2 {
				kind = "двуручное оружие"
			}
		}
		c.textClip(dx, y+3, dw, kind+fmt.Sprintf(" • цена %d", selected.Value), cDim, cPanel)
		for i, l := range wrap(strings.ReplaceAll(selected.Desc, ", ", "\n"), dw) {
			c.textClip(dx, y+5+i, dw, l, cText, cPanel)
		}
	}
	c.text(dx, y+h-4, fmt.Sprintf("Золото: %d", p.snap.Self.Gold), col("#ffd700"), cPanel)
	p.footer(x, y+h-1, w, " Enter надеть·использовать·снять  L в левую руку  X выбросить  O сортировать  Esc ")
}

func (p *play) skillDetails(x, y, w int, s *content.SkillDef, rank int, why string) {
	c := p.a.cv
	c.textClip(x, y, w, s.Name, cAccent, cPanel)
	y++
	c.textClip(x, y, w, fmt.Sprintf("Ранг %d из %d • с уровня %d", rank, s.MaxRank, s.Level), cDim, cPanel)
	y++
	if s.Equip != "" {
		c.textClip(x, y, w, "Работает, если есть: "+game.GearNeedName(s.Equip), col("#e0c080"), cPanel)
		y++
	}
	y++
	for _, l := range wrap(s.Desc, w) {
		c.textClip(x, y, w, l, cText, cPanel)
		y++
	}
	y++
	if len(s.Stats) > 0 {
		c.text(x, y, "За каждый ранг:", cDim, cPanel)
		y++
		keys := make([]string, 0, len(s.Stats))
		for k := range s.Stats {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			name := game.StatName(k)
			c.textClip(x+1, y, w-1, fmt.Sprintf("%s %+g (сейчас %+g)", name, s.Stats[k], s.Stats[k]*float64(rank)), cGood, cPanel)
			y++
		}
		y++
	}
	if len(s.Requires) > 0 {
		var names []string
		for _, r := range s.Requires {
			if rd := content.Skill(r); rd != nil {
				names = append(names, rd.Name)
			}
		}
		c.textClip(x, y, w, "Требует: "+strings.Join(names, ", "), cDim, cPanel)
		y++
	}
	if a := content.Ability(s.Grants); a != nil {
		y++
		c.textClip(x, y, w, "Даёт умение:", cDim, cPanel)
		y++
		y = p.abilityDetails(x, y, w, a)
	}
	y++
	switch {
	case rank >= s.MaxRank:
		c.textClip(x, y, w, "Изучено полностью.", cGood, cPanel)
	case why == "" && p.sheet.SkillPoints > 0:
		c.textClip(x, y, w, "Enter — изучить.", cAccent, cPanel)
	case why == "":
		c.textClip(x, y, w, "Нет очков навыков.", cBad, cPanel)
	default:
		c.textClip(x, y, w, "Недоступно: "+why, cBad, cPanel)
	}
}

func (p *play) abilityDetails(x, y, w int, a *content.AbilityDef) int {
	c := p.a.cv
	c.textClip(x, y, w, a.Name, col(a.Color), cPanel)
	y++
	info := fmt.Sprintf("Мана %.0f • перезарядка %.1fс", a.Mana, float64(a.CooldownMs)/1000)
	c.textClip(x, y, w, info, cDim, cPanel)
	y++
	if a.Equip != "" {
		c.textClip(x, y, w, "Нужно: "+game.GearNeedName(a.Equip), col("#e0c080"), cPanel)
		y++
	}
	if a.Damage[1] > 0 {
		if a.Kind == "heal" {
			c.textClip(x, y, w, fmt.Sprintf("Лечение %.0f-%.0f", a.Damage[0], a.Damage[1]), cGood, cPanel)
		} else {
			t, tc := "оружие", cText
			if len(a.Split) > 0 {
				var names []string
				for _, k := range a.Split {
					names = append(names, game.DamageTypeName(k))
				}
				t, tc = strings.Join(names, " + "), col(a.Color)
			} else if d := content.DamageType(a.DmgType); d != nil {
				t, tc = strings.ToLower(d.Name), col(d.Color)
			} else if a.DmgType == "" && a.Kind != "strike" && a.Kind != "cleave" && a.Kind != "dash" {
				t, tc = "тайная магия", col("#d08aff")
			}
			nx := c.text(x, y, fmt.Sprintf("Урон %.0f-%.0f: ", a.Damage[0], a.Damage[1]), cText, cPanel)
			nx = c.text(nx, y, t, tc, cPanel)
			if a.Kind == "strike" || a.Kind == "cleave" {
				c.textClip(nx, y, x+w-nx, " + удар оружием", cDim, cPanel)
			}
		}
		y++
	}
	if a.Leech > 0 {
		c.textClip(x, y, w, fmt.Sprintf("Вампиризм %.0f%%", a.Leech), col("#ff7a9a"), cPanel)
		y++
	}
	for _, b := range []*content.BuffDef{a.OnHit, a.Buff} {
		if b == nil {
			continue
		}
		for _, l := range wrap(game.BuffDesc(b), w) {
			c.textClip(x, y, w, l, col(b.Color), cPanel)
			y++
		}
	}
	for _, l := range wrap(a.Desc, w) {
		c.textClip(x, y, w, l, cText, cPanel)
		y++
	}
	return y
}

var attrNames = map[string]string{"str": "Сила", "dex": "Ловкость", "int": "Интеллект", "vit": "Выносливость"}
var attrDesc = map[string]string{
	"str": "урон в ближнем бою, сила воинских умений",
	"dex": "крит, уклонение, скорость атаки, урон стрел",
	"int": "мана, её восстановление, сила заклинаний",
	"vit": "здоровье и его восстановление",
}

func (p *play) drawChar() {
	c := p.a.cv
	x, y, w, h := p.panel(100, 26, "Персонаж")
	sh := p.sheet
	if sh == nil {
		return
	}
	className := sh.Class
	if cl := content.Class(sh.Class); cl != nil {
		className = cl.Name
	}
	c.text(x+2, y+2, fmt.Sprintf("%s — %d уровень", sh.Name, sh.Level), cAccent, cPanel)
	cx := x + 4 + runeLen(fmt.Sprintf("%s — %d уровень", sh.Name, sh.Level))
	for _, cv := range sh.Classes {
		cl := content.Class(cv.Key)
		if cl == nil {
			continue
		}
		label := fmt.Sprintf("%s %d", i18n.T(cl.Name), cv.Level)
		if sc := content.Subclass(cv.Subclass); sc != nil {
			label += " (" + i18n.T(sc.Name) + ")"
		}
		if cx+runeLen(label) >= x+w-2 {
			break
		}
		cx = c.text(cx, y+2, label, col(cl.Color), cPanel) + 2
	}
	_ = className
	c.text(x+2, y+4, fmt.Sprintf("Свободные очки: %d", sh.AttrPoints), cGood, cPanel)
	for i, k := range attrKeys {
		ry := y + 6 + i*2
		bg := cPanel
		if i == p.sel {
			bg = cSelBG
			c.fill(x+1, ry, 36, 1, ' ', cText, bg)
		}
		c.text(x+2, ry, padRight(attrNames[k], 14), cText, bg)
		c.text(x+17, ry, fmt.Sprintf("%3.0f (%3.0f)", sh.Attrs[k], sh.Stats[k]), cAccent, bg)
		if sh.AttrPoints > 0 {
			c.text(x+30, ry, "[+]", cGood, bg)
		}
		c.textClip(x+2, ry+1, 36, attrDesc[k], cDim, cPanel)
	}
	// wide panels show resistances in a third column, narrow ones on a second page
	wide := w >= 96
	sx := x + 40
	if wide || p.tab%2 == 0 {
		p.charStats(sx, y+4, min(32, w-42), y+h-2)
	}
	if wide {
		p.charResists(x+73, y+4, w-75, y+h-2)
	} else if p.tab%2 == 1 {
		p.charResists(sx, y+4, w-42, y+h-2)
	}
	hint := " ↑↓ выбор  Enter/+ вложить очко  Esc "
	if !wide {
		hint = " ↑↓ выбор  Enter/+ вложить очко  Tab характеристики/сопротивления  Esc "
	}
	p.footer(x, y+h-1, w, hint)
}

func (p *play) charStats(x, y, w, maxY int) {
	c := p.a.cv
	st := p.sheet.Stats
	lines := []string{
		fmt.Sprintf("Здоровье      %.0f  (+%.1f/с)", st["max_hp"], st["hp_regen"]),
		fmt.Sprintf("Мана          %.0f  (+%.1f/с)", st["max_mp"], st["mp_regen"]),
		fmt.Sprintf("Урон оружия   %.0f-%.0f", st["dmg_min"], st["dmg_max"]),
		fmt.Sprintf("Броня         %.0f  (-%.0f%% физ.)", st["armor"], 100*st["armor"]/(math.Max(0, st["armor"])+15)),
		fmt.Sprintf("Уклонение     %.0f%%", st["dodge"]),
		fmt.Sprintf("Крит          %.0f%% ×%.1f", st["crit"], st["crit_mult"]),
		fmt.Sprintf("Ближний бой   %+.0f%%", st["melee_pct"]),
		fmt.Sprintf("Дальний бой   %+.0f%%", st["ranged_pct"]),
		fmt.Sprintf("Магия         %+.0f%%", st["spell_pct"]),
		fmt.Sprintf("Атака раз в   %.2fс", st["attack_ms"]/1000),
		fmt.Sprintf("Шаг раз в     %.2fс", st["move_ms"]/1000),
		fmt.Sprintf("Обзор         %+.0f", st["sight"]),
	}
	if v := st["life_leech"]; v != 0 {
		lines = append(lines, fmt.Sprintf("Вампиризм     %.1f%%", v))
	}
	if v := st["thorns"]; v != 0 {
		lines = append(lines, fmt.Sprintf("Шипы          %.0f", v))
	}
	for _, k := range [][2]string{{"block", "Блок"}, {"fury", "Ярость ран."}, {"duel_pct", "Один на один"},
		{"ambush_pct", "Из засады"}, {"heal_pct", "Сила лечения"}, {"mimic_pct", "Сила копий"}} {
		if v := st[k[0]]; v != 0 {
			lines = append(lines, fmt.Sprintf("%-14s+%.0f%%", i18n.T(k[1]), v))
		}
	}
	lines = append(lines, fmt.Sprintf("Убито врагов  %.0f", st["kills"]))
	c.text(x, y, "Характеристики", cAccent, cPanel)
	for i, l := range lines {
		if y+2+i >= maxY {
			break
		}
		c.textClip(x, y+2+i, w, l, cText, cPanel)
	}
}

func (p *play) charResists(x, y, w, maxY int) {
	c := p.a.cv
	st := p.sheet.Stats
	c.text(x, y, "Сопротивления", cAccent, cPanel)
	ry := y + 2
	for _, dt := range content.DamageTypes() {
		if ry >= maxY {
			return
		}
		v := st["res_"+dt.Key]
		fg := cDim
		switch {
		case v > 0:
			fg = cGood
		case v < 0:
			fg = cBad
		}
		c.textClip(x, ry, w-6, dt.Name, col(dt.Color), cPanel)
		c.text(x+w-5, ry, fmt.Sprintf("%4.0f%%", v), fg, cPanel)
		ry++
	}
	var bonus []string
	for _, dt := range content.DamageTypes() {
		if v := st[dt.Key+"_pct"]; v != 0 {
			bonus = append(bonus, fmt.Sprintf("%s %+.0f%%", i18n.T(dt.Short), v))
		}
		if v := st["add_"+dt.Key]; v != 0 {
			bonus = append(bonus, fmt.Sprintf("+%.0f %s", v, i18n.T(dt.Short)))
		}
	}
	if len(bonus) > 0 && ry+2 < maxY {
		ry++
		c.text(x, ry, "Урон по типам", cAccent, cPanel)
		ry++
		for _, l := range wrap(strings.Join(bonus, ", "), w) {
			if ry >= maxY {
				return
			}
			c.textClip(x, ry, w, l, cText, cPanel)
			ry++
		}
	}
}

func (p *play) drawJournal() {
	c := p.a.cv
	x, y, w, h := p.panel(84, 30, "Журнал заданий")
	if p.sheet == nil || len(p.sheet.Quests) == 0 {
		c.text(x+2, y+2, "Заданий нет. Поговорите со старостой деревни.", cDim, cPanel)
		p.footer(x, y+h-1, w, " Esc ")
		return
	}
	ry := y + 2
	for _, q := range p.sheet.Quests {
		if ry >= y+h-2 {
			break
		}
		fg := cText
		if q.Unique {
			fg = col("#ff9aff")
		}
		status := fmt.Sprintf("%d/%d", q.Have, q.Need)
		if q.Done {
			fg = cGood
			status = "выполнено — вернитесь за наградой"
		}
		c.textClip(x+2, ry, w-4, q.Text, fg, cPanel)
		c.textClip(x+4, ry+1, w-6, "Прогресс: "+status+" • выдал: "+q.Giver, cDim, cPanel)
		c.textClip(x+4, ry+2, w-6, "Награда: "+q.Reward, col("#ffd24a"), cPanel)
		ry += 3
		if q.Where != "" && !q.Done {
			c.textClip(x+4, ry, w-6, "Где: "+q.Where, cDim, cPanel)
			ry++
		}
		ry++
	}
	p.footer(x, y+h-1, w, " Esc ")
}

func (p *play) drawWorldMap() {
	c := p.a.cv
	sw, sh := p.a.size()
	if p.a.gfx != nil {
		// the graphical front-end paints the atlas under an empty grid
		c.fill(0, 0, sw, sh, ' ', cText, cBlack)
		p.footer(0, sh-1, sw, " M / Tab / Esc — закрыть карту ")
		return
	}
	c.box(0, 0, sw, sh, "Карта: "+p.level.Name, cBorder)
	lv := p.level
	iw, ih := sw-2, sh-3
	sx := max(1, (lv.W+iw-1)/iw)
	sy := max(1, (lv.H+ih-1)/ih)
	scale := max(sx, sy)
	ox := 1 + (iw-lv.W/scale)/2
	oy := 1 + (ih-lv.H/scale)/2
	for my := 0; my*scale < lv.H && my < ih; my++ {
		for mx := 0; mx*scale < lv.W && mx < iw; mx++ {
			// prefer interesting tiles inside the block
			var best *content.TileDef
			for by := 0; by < scale; by++ {
				for bx := 0; bx < scale; bx++ {
					wx, wy := mx*scale+bx, my*scale+by
					if !lv.In(wx, wy) || !p.explored.Get(wy*lv.W+wx) {
						continue
					}
					t := content.Tile(lv.At(wx, wy))
					if best == nil || t.Interact != "" || (!t.Walkable && best.Walkable && best.Interact == "") {
						best = t
					}
				}
			}
			if best != nil {
				c.put(ox+mx, oy+my, best.Rune, col(best.FG), cPanel)
			}
		}
	}
	in := func(x, y int) bool { return x/scale < iw && y/scale < ih }
	if lv.ID == "overworld" {
		for _, pl := range p.places() {
			if !in(pl.X, pl.Y) {
				continue
			}
			switch {
			case pl.Kind == "quest":
				c.put(ox+pl.X/scale, oy+pl.Y/scale, '!', col("#ffd24a"), col("#602020"))
			case pl.Kind == "village":
				c.text(max(1, ox+pl.X/scale-runeLen(pl.Name)/2), oy+pl.Y/scale-1, pl.Name, col("#ffe0a0"), cPanel)
			case pl.Kind == "city":
				name := "[" + pl.Name + "]"
				c.text(max(1, ox+pl.X/scale-runeLen(name)/2), oy+pl.Y/scale-1, name, col("#ffffff"), col("#5a4a2a"))
			}
		}
	}
	for _, e := range p.snap.Entities {
		if e.Kind == uint8(game.KPlayer) && in(e.X, e.Y) {
			c.put(ox+e.X/scale, oy+e.Y/scale, '@', col(e.Color), col("#303060"))
		}
	}
	for _, m := range p.snap.Self.Party {
		if m.LevelID == lv.ID && in(m.X, m.Y) {
			c.put(ox+m.X/scale, oy+m.Y/scale, '@', col("#60e070"), col("#204020"))
		}
	}
	s := p.snap.Self
	c.put(ox+s.X/scale, oy+s.Y/scale, '@', cAccent, col("#603030"))
	p.footer(0, sh-1, sw, fmt.Sprintf(" масштаб 1:%d • > подземелья • ! цели заданий • зелёные — группа • любая клавиша — закрыть ", scale))
}

func (p *play) drawHelp() {
	c := p.a.cv
	x, y, w, h := p.panel(78, 40, "Помощь")
	lines := [][2]string{
		{"WASD / стрелки", "ходьба (удерживайте для бега); шаг в врага — атака"},
		{"Пробел", "атаковать врага рядом"},
		{"E / Enter", "говорить, открыть дверь/сундук, пройти по лестнице"},
		{"1-6", "умения с панели (в окне — в сторону курсора мыши)"},
		{"Мышь (окно)", "ЛКМ — атака, ПКМ — умение 1, оба туда, где курсор"},
		{"Q / R", "выпить зелье здоровья / маны"},
		{"I", "инвентарь: 11 слотов, L — в левую руку"},
		{"K", "классы, подклассы, навыки и панель умений"},
		{"C", "персонаж и характеристики"},
		{"J", "журнал заданий"},
		{"G", "группа: пригласить, принять, покинуть"},
		{"M / Tab", "карта уровня"},
		{"T", "чат с другими игроками"},
		{"F5", "сохранить мир (хозяин)"},
		{"F9", "окно администратора (запуск с -admin)"},
		{"Ctrl+V", "вставить текст из буфера обмена в поле ввода"},
		{"Esc", "меню (пауза в одиночной игре)"},
		{"", ""},
		{"Диалоги", "↑↓ / цифры — варианты ответа; при включённом ИИ"},
		{"", "можно писать персонажу что угодно и нажать Enter"},
		{"", ""},
		{"Урон", "10 типов: рубящий, колющий, дробящий, огонь, холод,"},
		{"", "молния, яд, тайная магия, свет, тьма. Броня гасит только"},
		{"", "физический урон, остальное — сопротивления (C)."},
		{"", "Справа видно, к чему цель стойка и уязвима: скелет"},
		{"", "боится дробящего и света, ифрит неуязвим к огню."},
		{"", "Проклятия и метки снижают сопротивления врагов."},
		{"Классы", "на 5-м уровне класса выбирается подкласс; с 5-го"},
		{"", "уровня героя можно начать второй класс (K → «+ Класс»)."},
		{"", "Секретные классы и подклассы дают уникальные персонажи."},
		{"Мир", "у каждого региона свой нрав: пустыня, тундра, пепел,"},
		{"", "проклятые земли опаснее. Странники и стража бьются с"},
		{"", "чудищами. Вне деревень игроки могут напасть друг на"},
		{"", "друга — группа (G) защищает своих. Павшего героя можно"},
		{"", "воскресить в течение минуты."},
	}
	for i, l := range lines {
		if y+2+i >= y+h-1 {
			break
		}
		c.text(x+2, y+2+i, l[0], cAccent, cPanel)
		c.textClip(x+20, y+2+i, w-22, l[1], cText, cPanel)
	}
	p.footer(x, y+h-1, w, " любая клавиша — закрыть ")
}

func (p *play) drawPause() {
	c := p.a.cv
	items := p.pauseItems()
	x, y, w, h := p.panel(46, len(items)+4, "Меню")
	for i, it := range items {
		bg := cPanel
		fg := cText
		if i == p.sel {
			bg, fg = cSelBG, cAccent
			c.fill(x+1, y+2+i, w-2, 1, ' ', fg, bg)
		}
		c.textClip(x+3, y+2+i, w-5, it.label, fg, bg)
	}
	_ = h
}

func (p *play) drawDialogue(L layout) {
	c := p.a.cv
	d := p.dialogue
	if d == nil {
		return
	}
	w := min(76, L.mapW-2)
	textLines := wrap(d.Text, w-4)
	if d.Waiting {
		textLines = []string{d.Name + " обдумывает ответ..."}
	}
	h := len(textLines) + len(d.Options) + 5
	if d.AI {
		h += 2
	}
	h = min(h, L.mapH)
	x := (L.mapW - w) / 2
	y := max(1, L.logY-h)
	c.box(x, y, w, h, d.Name+" — "+d.Role, col("#7a6a3a"))
	ry := y + 1
	for _, l := range textLines {
		if ry >= y+h-1 {
			break
		}
		fg := cText
		if d.Waiting {
			fg = cDim
		}
		c.textClip(x+2, ry, w-4, l, fg, cPanel)
		ry++
	}
	ry++
	for i, o := range d.Options {
		if ry >= y+h-1 {
			break
		}
		bg, fg := cPanel, cText
		if i == p.sel {
			bg, fg = cSelBG, cAccent
			c.fill(x+1, ry, w-2, 1, ' ', fg, bg)
		}
		c.textClip(x+2, ry, w-4, fmt.Sprintf("%d. %s", i+1, o), fg, bg)
		ry++
	}
	if d.AI && ry < y+h-1 {
		ry++
		c.text(x+2, ry, "Вы:", cAccent, cPanel)
		p.talk.draw(c, x+6, ry, w-8, true)
	}
	hint := " ↑↓/цифры — выбор, Enter — ответить, Esc — уйти "
	if d.AI {
		hint = " пишите свой ответ или выберите вариант • Esc — уйти "
	}
	c.textClip(x+2, y+h-1, w-4, hint, cDim, cPanel)
}

func (p *play) drawTrade() {
	c := p.a.cv
	d := p.dialogue
	if d == nil || p.sheet == nil {
		return
	}
	x, y, w, h := p.panel(96, 28, "Торговля — "+d.Name)
	colW := (w - 5) / 2
	headers := []string{"Товары (купить)", "Ваш рюкзак (продать)"}
	for i, hd := range headers {
		fg := cDim
		if p.tradeCol == i {
			fg = cAccent
		}
		c.text(x+2+i*(colW+1), y+2, hd, fg, cPanel)
	}
	var detail *proto.ItemView
	price := 0
	for i, t := range d.Trade {
		ry := y + 4 + i
		if ry >= y+h-6 {
			break
		}
		bg := cPanel
		if p.tradeCol == 0 && i == p.sel {
			bg = cSelBG
			c.fill(x+1, ry, colW, 1, ' ', cText, bg)
			it := t.Item
			detail, price = &it, t.Price
		}
		c.put(x+2, ry, t.Item.Glyph, col(t.Item.Color), bg)
		c.textClip(x+4, ry, colW-11, t.Item.Name, rarityCol(&t.Item), bg)
		pc := col("#ffd700")
		if t.Price > p.snap.Self.Gold {
			pc = cBad
		}
		c.text(x+colW-5, ry, fmt.Sprintf("%5d", t.Price), pc, bg)
	}
	rx := x + 3 + colW
	for i, it := range p.sheet.Inventory {
		ry := y + 4 + i
		if ry >= y+h-6 {
			break
		}
		bg := cPanel
		pr := max(1, it.Value*2/5) // same formula as game.SellPrice
		if p.tradeCol == 1 && i == p.sel {
			bg = cSelBG
			c.fill(rx, ry, colW, 1, ' ', cText, bg)
			v := it
			detail, price = &v, pr
		}
		c.put(rx+1, ry, it.Glyph, col(it.Color), bg)
		name := it.Name
		if it.Qty > 1 {
			name += fmt.Sprintf(" ×%d", it.Qty)
		}
		c.textClip(rx+3, ry, colW-10, name, rarityCol(&it), bg)
		c.text(rx+colW-6, ry, fmt.Sprintf("%5d", pr), col("#ffd700"), bg)
	}
	if detail != nil {
		dy := y + h - 5
		n := c.text(x+2, dy, detail.Name, rarityCol(detail), cPanel)
		c.textClip(n, dy, w-2-n+x, " — "+detail.Desc, cText, cPanel)
		verb := "Купить"
		if p.tradeCol == 1 {
			verb = "Продать"
		}
		c.text(x+2, dy+1, fmt.Sprintf("%s за %d золота (Enter)", verb, price), cAccent, cPanel)
	}
	c.text(x+2, y+h-3, fmt.Sprintf("Ваше золото: %d", p.snap.Self.Gold), col("#ffd700"), cPanel)
	p.footer(x, y+h-1, w, " Tab/←→ сторона  ↑↓ выбор  Enter купить/продать  Esc назад ")
}

func (p *play) drawClass() {
	c := p.a.cv
	classes := startingClasses()
	_, sh := p.a.size()
	rows := max(1, min(len(classes), (sh-8)/3))
	x, y, w, h := p.panel(70, rows*3+6, "Выберите класс героя")
	start := max(0, min(p.sel-rows+1, len(classes)-rows))
	for i := start; i < len(classes) && i < start+rows; i++ {
		cl := classes[i]
		ry := y + 2 + (i-start)*3
		bg := cPanel
		if i == p.sel {
			bg = cSelBG
			c.fill(x+1, ry, w-2, 1, ' ', cText, bg)
		}
		c.text(x+3, ry, cl.Name, col(cl.Color), bg)
		c.text(x+20, ry, attrLine(cl.Attrs), cDim, bg)
		c.textClip(x+3, ry+1, w-6, cl.Desc, cText, cPanel)
	}
	if start > 0 {
		c.text(x+w-4, y+1, "↑", cDim, cPanel)
	}
	if start+rows < len(classes) {
		c.text(x+w-4, y+h-2, "↓", cDim, cPanel)
	}
	p.footer(x, y+h-1, w, " ↑↓ выбор  Enter подтвердить  Esc выйти ")
}
