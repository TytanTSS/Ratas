package client

import (
	"fmt"
	"slices"
	"sort"
	"strings"

	"github.com/gdamore/tcell/v2"

	"ratas/internal/content"
	"ratas/internal/game"
	"ratas/internal/proto"
)

// The skills window has a tab for known abilities, one per class the hero
// develops (its base branch and its subclass), the common branch, secret
// knowledge, and a tab to start another class.

type skillTab struct {
	kind  string // abilities, class, branch, new
	class string
	name  string
	color string
}

type skillRow struct {
	kind  string // header, skill, subclass, class
	text  string
	skill content.SkillDef
	sub   *content.SubclassDef
	class *content.ClassDef
}

// heroState is the part of the character the client needs for rule checks.
func (p *play) heroState() *game.PlayerState {
	sh := p.sheet
	ps := &game.PlayerState{Level: sh.Level, Skills: sh.Skills, SkillPoints: max(1, sh.SkillPoints),
		Class: sh.Class, Subclasses: map[string]string{}, Unlocks: sh.Unlocks}
	for _, c := range sh.Classes {
		ps.Classes = append(ps.Classes, c.Key)
		if c.Subclass != "" {
			ps.Subclasses[c.Key] = c.Subclass
		}
	}
	return ps
}

func (p *play) skillTabs() []skillTab {
	tabs := []skillTab{{kind: "abilities", name: "Умения", color: "#ffffff"}}
	if p.sheet == nil {
		return tabs
	}
	for _, cv := range p.sheet.Classes {
		if c := content.Class(cv.Key); c != nil {
			tabs = append(tabs, skillTab{kind: "class", class: c.Key, name: fmt.Sprintf("%s %d", c.Name, cv.Level), color: c.Color})
		}
	}
	for _, b := range content.Branches() {
		if b.Class != "" {
			continue
		}
		if b.Secret {
			has := false
			for k := range p.sheet.Skills {
				if sd := content.Skill(k); sd != nil && sd.Branch == b.Key {
					has = true
				}
			}
			if !has {
				continue
			}
		}
		tabs = append(tabs, skillTab{kind: "branch", class: b.Key, name: b.Name, color: b.Color})
	}
	return append(tabs, skillTab{kind: "new", name: "+ Класс", color: "#a0ffa0"})
}

func branchSkills(branch string) []content.SkillDef {
	var rows []content.SkillDef
	for _, s := range content.Skills() {
		if s.Branch == branch {
			rows = append(rows, s)
		}
	}
	sort.SliceStable(rows, func(i, j int) bool { return rows[i].Tier < rows[j].Tier })
	return rows
}

func classBranch(class, sub string) string {
	for _, b := range content.Branches() {
		if b.Class == class && b.Subclass == sub {
			return b.Key
		}
	}
	return ""
}

func (p *play) skillRowsFor(t skillTab) []skillRow {
	var rows []skillRow
	switch t.kind {
	case "class":
		c := content.Class(t.class)
		rows = append(rows, skillRow{kind: "header", text: "Ветка класса «" + c.Name + "»"})
		for _, s := range branchSkills(classBranch(t.class, "")) {
			rows = append(rows, skillRow{kind: "skill", skill: s})
		}
		sub := ""
		for _, cv := range p.sheet.Classes {
			if cv.Key == t.class {
				sub = cv.Subclass
			}
		}
		if sc := content.Subclass(sub); sc != nil {
			rows = append(rows, skillRow{kind: "header", text: "Подкласс «" + sc.Name + "»"})
			for _, s := range branchSkills(classBranch(t.class, sub)) {
				rows = append(rows, skillRow{kind: "skill", skill: s})
			}
		} else {
			rows = append(rows, skillRow{kind: "header", text: "Выбор подкласса"})
			for _, sc := range content.SubclassesOf(t.class) {
				rows = append(rows, skillRow{kind: "subclass", sub: sc})
			}
		}
	case "branch":
		for _, s := range branchSkills(t.class) {
			rows = append(rows, skillRow{kind: "skill", skill: s})
		}
	case "new":
		for i := range content.Classes() {
			c := &content.Classes()[i]
			if !slices.ContainsFunc(p.sheet.Classes, func(cv proto.ClassView) bool { return cv.Key == c.Key }) {
				rows = append(rows, skillRow{kind: "class", class: c})
			}
		}
	}
	return rows
}

func selectable(r skillRow) bool { return r.kind != "header" }

func (p *play) currentTab() skillTab {
	tabs := p.skillTabs()
	return tabs[p.tab%len(tabs)]
}

func (p *play) drawSkills() {
	c := p.a.cv
	x, y, w, h := p.panel(104, 34, "Навыки и классы")
	if p.sheet == nil {
		return
	}
	tabs := p.skillTabs()
	tab := p.tab % len(tabs)
	total := 0
	for _, t := range tabs {
		total += runeLen(t.name) + 3
	}
	if total <= w-4 {
		tx := x + 2
		for i, t := range tabs {
			fg, bg := col(t.color), cPanel
			if i == tab {
				bg = cSelBG
			} else {
				fg = shade(fg, 0.55, [3]float64{}, 0)
			}
			tx = c.text(tx, y+1, " "+t.name+" ", fg, bg) + 1
		}
	} else {
		t := tabs[tab]
		c.text(x+2, y+1, fmt.Sprintf("◀ %s ▶  (%d/%d)", t.name, tab+1, len(tabs)), col(t.color), cPanel)
	}
	pts := fmt.Sprintf("Очки навыков: %d", p.sheet.SkillPoints)
	c.text(x+w-runeLen(pts)-2, y+h-2, pts, cGood, cPanel)
	listW := min(48, w/2)
	dx, dw := x+listW+3, w-listW-5
	t := tabs[tab]
	if t.kind == "abilities" {
		p.drawAbilityList(x, y, listW, dx, dw)
		p.footer(x, y+h-1, w, " ←→ вкладка  ↑↓ выбор  1-6 назначить на панель  Esc ")
		return
	}
	rows := p.skillRowsFor(t)
	ps := p.heroState()
	maxRows := h - 5
	start := max(0, p.sel-maxRows+1)
	ry := y + 3
	for i := start; i < len(rows) && ry < y+h-2; i++ {
		r := rows[i]
		bg := cPanel
		if i == p.sel && selectable(r) {
			bg = cSelBG
			c.fill(x+1, ry, listW, 1, ' ', cText, bg)
		}
		switch r.kind {
		case "header":
			c.textClip(x+2, ry, listW-2, r.text, cAccent, cPanel)
		case "skill":
			s := r.skill
			rank := p.sheet.Skills[s.Key]
			why := game.CanLearn(ps, &s)
			fg := cText
			switch {
			case rank >= s.MaxRank:
				fg = cGood
			case rank > 0:
				fg = col("#a0e0a0")
			case why != "":
				fg = cDim
			}
			mark := "○"
			if rank > 0 {
				mark = "●"
			}
			if asciiUI {
				mark = map[bool]string{true: "*", false: "o"}[rank > 0]
			}
			c.text(x+3, ry, fmt.Sprintf("T%d", s.Tier), cDim, bg)
			c.text(x+6, ry, mark, fg, bg)
			c.textClip(x+8, ry, listW-14, s.Name, fg, bg)
			c.text(x+listW-5, ry, fmt.Sprintf("%d/%d", rank, s.MaxRank), fg, bg)
		case "subclass":
			sc := r.sub
			name, fg := sc.Name, col(sc.Color)
			why := game.CanChooseSubclass(ps, sc.Key)
			if sc.Secret && !ps.Unlocked("subclass", sc.Key) {
				name, fg = "??? (секретный подкласс)", cDim
			} else if why != "" {
				fg = shade(fg, 0.6, [3]float64{}, 0)
			}
			c.text(x+3, ry, "♦", fg, bg)
			c.textClip(x+5, ry, listW-6, name, fg, bg)
		case "class":
			cl := r.class
			name, fg := cl.Name, col(cl.Color)
			if cl.Secret && !ps.Unlocked("class", cl.Key) {
				name, fg = "??? (секретный класс)", cDim
			} else if game.CanStartClass(ps, cl.Key) != "" {
				fg = shade(fg, 0.6, [3]float64{}, 0)
			}
			c.text(x+3, ry, "♦", fg, bg)
			c.textClip(x+5, ry, listW-6, name, fg, bg)
		}
		if i == p.sel {
			p.skillRowDetails(dx, y+3, dw, r, ps)
		}
		ry++
	}
	if len(rows) == 0 {
		c.text(x+2, y+3, "Все классы уже начаты.", cDim, cPanel)
	}
	p.footer(x, y+h-1, w, " ←→ вкладка  ↑↓ выбор  Enter изучить/выбрать  1-6 на панель  Esc ")
}

func (p *play) drawAbilityList(x, y, listW, dx, dw int) {
	c := p.a.cv
	for i, key := range p.sheet.Abilities {
		a := content.Ability(key)
		if a == nil {
			continue
		}
		bg := cPanel
		if i == p.sel {
			bg = cSelBG
			c.fill(x+1, y+3+i, listW, 1, ' ', cText, bg)
		}
		slot := " "
		for s, hk := range p.sheet.Hotbar {
			if hk == key {
				slot = fmt.Sprint(s + 1)
			}
		}
		c.text(x+2, y+3+i, "["+slot+"]", cAccent, bg)
		c.textClip(x+6, y+3+i, listW-6, p.abilityName(key), col(a.Color), bg)
		if i == p.sel {
			p.abilityDetails(dx, y+3, dw, a)
		}
	}
	if len(p.sheet.Abilities) == 0 {
		c.text(x+2, y+3, "Пока нет умений. Изучайте навыки в ветках.", cDim, cPanel)
	}
}

// abilityName shows what a mimic has copied.
func (p *play) abilityName(key string) string {
	a := content.Ability(key)
	if a == nil {
		return key
	}
	if key == "copied" && p.snap != nil && p.snap.Self.Copied != "" {
		if orig := content.Ability(p.snap.Self.Copied); orig != nil {
			return "Копия: " + orig.Name
		}
	}
	return a.Name
}

func (p *play) skillRowDetails(x, y, w int, r skillRow, ps *game.PlayerState) {
	c := p.a.cv
	switch r.kind {
	case "skill":
		s := r.skill
		why := game.CanLearn(ps, &s)
		p.skillDetails(x, y, w, &s, p.sheet.Skills[s.Key], why)
	case "subclass":
		sc := r.sub
		why := game.CanChooseSubclass(ps, sc.Key)
		if sc.Secret && !ps.Unlocked("subclass", sc.Key) {
			c.textClip(x, y, w, "Секретный подкласс", cAccent, cPanel)
			for i, l := range wrap("Его открывает задание одного из уникальных персонажей мира. Говорите со странниками и ищите слухи.", w) {
				c.textClip(x, y+2+i, w, l, cDim, cPanel)
			}
			return
		}
		c.textClip(x, y, w, sc.Name, col(sc.Color), cPanel)
		ry := y + 2
		for _, l := range wrap(sc.Desc, w) {
			c.textClip(x, ry, w, l, cText, cPanel)
			ry++
		}
		ry++
		names := []string{}
		for _, s := range branchSkills(classBranch(sc.Class, sc.Key)) {
			names = append(names, s.Name)
		}
		for _, l := range wrap("Навыки: "+strings.Join(names, ", "), w) {
			c.textClip(x, ry, w, l, cDim, cPanel)
			ry++
		}
		ry++
		if why == "" {
			c.textClip(x, ry, w, "Enter — выбрать (у класса может быть только один подкласс).", cAccent, cPanel)
		} else {
			c.textClip(x, ry, w, "Недоступно: "+why, cBad, cPanel)
		}
	case "class":
		cl := r.class
		why := game.CanStartClass(ps, cl.Key)
		if cl.Secret && !ps.Unlocked("class", cl.Key) {
			c.textClip(x, y, w, "Секретный класс", cAccent, cPanel)
			for i, l := range wrap("Его открывает задание одного из уникальных персонажей мира.", w) {
				c.textClip(x, y+2+i, w, l, cDim, cPanel)
			}
			return
		}
		c.textClip(x, y, w, cl.Name, col(cl.Color), cPanel)
		ry := y + 2
		for _, l := range wrap(cl.Desc, w) {
			c.textClip(x, ry, w, l, cText, cPanel)
			ry++
		}
		ry++
		if why == "" {
			c.textClip(x, ry, w, "Enter — начать путь этого класса.", cAccent, cPanel)
		} else {
			c.textClip(x, ry, w, "Недоступно: "+why, cBad, cPanel)
		}
	}
}

func (p *play) keySkills(ev *tcell.EventKey) {
	tabs := p.skillTabs()
	t := tabs[p.tab%len(tabs)]
	var rows []skillRow
	n := 0
	if t.kind == "abilities" {
		if p.sheet != nil {
			n = len(p.sheet.Abilities)
		}
	} else if p.sheet != nil {
		rows = p.skillRowsFor(t)
		n = len(rows)
	}
	move := func(d int) {
		for i := 0; i < n; i++ {
			p.sel = (p.sel + d + n) % n
			if rows == nil || selectable(rows[p.sel]) {
				return
			}
		}
	}
	switch ev.Key() {
	case tcell.KeyEscape:
		p.mode = modeGame
		return
	case tcell.KeyLeft, tcell.KeyBacktab:
		p.tab = (p.tab + len(tabs) - 1) % len(tabs)
		p.sel = 0
		p.fixSel()
		return
	case tcell.KeyRight, tcell.KeyTab:
		p.tab = (p.tab + 1) % len(tabs)
		p.sel = 0
		p.fixSel()
		return
	case tcell.KeyUp:
		if n > 0 {
			move(-1)
		}
		return
	case tcell.KeyDown:
		if n > 0 {
			move(1)
		}
		return
	case tcell.KeyEnter:
		if p.sel < len(rows) {
			switch r := rows[p.sel]; r.kind {
			case "skill":
				p.cmd("learn", r.skill.Key, 0)
			case "subclass":
				p.cmd("choose_subclass", r.sub.Key, 0)
			case "class":
				p.cmd("start_class", r.class.Key, 0)
			}
		}
		return
	}
	if ev.Key() != tcell.KeyRune {
		return
	}
	r := ev.Rune()
	if r >= '1' && r <= '6' {
		var ability string
		if t.kind == "abilities" {
			if p.sheet != nil && p.sel < len(p.sheet.Abilities) {
				ability = p.sheet.Abilities[p.sel]
			}
		} else if p.sel < len(rows) && rows[p.sel].kind == "skill" {
			if p.sheet.Skills[rows[p.sel].skill.Key] > 0 {
				ability = rows[p.sel].skill.Grants
			}
		}
		if ability != "" {
			p.cmd("hotbar", ability, int(r-'1'))
		}
		return
	}
	if normKey(r) == 'k' {
		p.mode = modeGame
	}
}

// fixSel moves the selection off a header.
func (p *play) fixSel() {
	if p.sheet == nil {
		return
	}
	t := p.currentTab()
	if t.kind == "abilities" {
		return
	}
	rows := p.skillRowsFor(t)
	for p.sel < len(rows) && !selectable(rows[p.sel]) {
		p.sel++
	}
}
