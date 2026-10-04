package client

import (
	"fmt"
	"math"
	"sort"
	"strings"
	"time"

	"github.com/gdamore/tcell/v2"

	"ratas/internal/content"
	"ratas/internal/game"
	"ratas/internal/proto"
)

const (
	sideW = 30
	logH  = 7
)

var nightTint = [3]float64{8, 12, 40}

func daylight(t float64) float64 {
	switch {
	case t >= 0.3 && t <= 0.7:
		return 1
	case t >= 0.85 || t <= 0.15:
		return 0
	case t < 0.3:
		return (t - 0.15) / 0.15
	default:
		return (0.85 - t) / 0.15
	}
}

type layout struct {
	w, h       int
	mapW, mapH int
	side       bool
	logY       int
}

func (p *play) layout() layout {
	w, h := p.a.size()
	l := layout{w: w, h: h}
	l.side = w >= 70
	l.mapW = w
	if l.side {
		l.mapW = w - sideW
	}
	lh := logH
	if h < 24 {
		lh = 4
	}
	l.mapH = max(1, h-1-lh)
	l.logY = 1 + l.mapH
	return l
}

func (p *play) draw() {
	c := p.a.cv
	L := p.layout()
	p.a.scr.Clear()
	c.fill(0, 0, L.w, L.h, ' ', cText, cBlack)
	if p.snap == nil || p.level == nil {
		msg := "Подключение..."
		if p.welcome != nil {
			msg = "Загрузка мира..."
		}
		if p.mode == modeClass {
			p.drawClass()
		} else {
			c.text((L.w-runeLen(msg))/2, L.h/2, msg, cAccent, cBlack)
		}
		p.a.scr.Show()
		return
	}
	if p.a.gfx != nil {
		// the graphical renderer draws the world under the text interface
		p.updateVisibility()
		p.publishScene(L)
	} else {
		p.drawMap(L)
	}
	p.drawTop(L)
	if L.side {
		p.drawSidebar(L)
	}
	p.drawLog(L)

	switch p.mode {
	case modeInventory:
		p.drawInventory()
	case modeSkills:
		p.drawSkills()
	case modeChar:
		p.drawChar()
	case modeJournal:
		p.drawJournal()
	case modeMap:
		p.drawWorldMap()
	case modeHelp:
		p.drawHelp()
	case modePause:
		p.drawPause()
	case modeDialogue:
		p.drawDialogue(L)
	case modeTrade:
		p.drawTrade()
	case modeClass:
		p.drawClass()
	case modeParty:
		p.drawParty()
	case modeAdmin:
		p.drawAdmin()
	case modeChat:
		y := L.logY - 1
		c.fill(0, y, L.mapW, 1, ' ', cText, cPanel)
		c.text(1, y, "Сказать:", cAccent, cPanel)
		p.chat.draw(c, 10, y, L.mapW-11, true)
	}
	if p.snap.Self.Dead && p.mode != modeMap {
		msg := fmt.Sprintf(" ВЫ ПОГИБЛИ • возрождение в деревне через %d с ", (p.snap.Self.RespawnIn+999)/1000)
		c.text((L.mapW-runeLen(msg))/2, L.mapH/2, msg, col("#ffffff"), col("#8a1010"))
		hint := " Союзник может воскресить вас, пока вы здесь "
		if p.snap.Self.CanRise {
			hint = " Enter — возродиться сейчас • союзник может воскресить вас "
		}
		c.text((L.mapW-runeLen(hint))/2, L.mapH/2+1, hint, col("#ffd0d0"), col("#401010"))
	}
	if p.paused {
		c.text((L.mapW-7)/2, 2, " ПАУЗА ", cBlack, cAccent)
	}
	if p.notice != "" && time.Since(p.noticeAt) < 1500*time.Millisecond {
		c.text((L.mapW-runeLen(p.notice)-2)/2, 3, " "+p.notice+" ", cBlack, cGood)
	}
	if p.region != "" && p.mode == modeGame && time.Since(p.regionAt) < 3500*time.Millisecond {
		banner := "   " + p.region + "   "
		bx := (L.mapW - runeLen(banner)) / 2
		c.fill(bx-2, 4, runeLen(banner)+4, 3, ' ', cText, col("#141420"))
		c.text(bx, 5, banner, col("#f0dca0"), col("#141420"))
		line := strings.Repeat("─", runeLen(banner)+2)
		if asciiUI {
			line = strings.Repeat("-", runeLen(banner)+2)
		}
		c.text(bx-1, 4, line, col("#6a5a3a"), col("#141420"))
		c.text(bx-1, 6, line, col("#6a5a3a"), col("#141420"))
	}
	p.a.scr.Show()
}

func (p *play) camera(L layout) (int, int) {
	s := p.snap.Self
	lv := p.level
	camX, camY := s.X-L.mapW/2, s.Y-L.mapH/2
	if lv.W <= L.mapW {
		camX = -(L.mapW - lv.W) / 2
	} else {
		camX = max(0, min(lv.W-L.mapW, camX))
	}
	if lv.H <= L.mapH {
		camY = -(L.mapH - lv.H) / 2
	} else {
		camY = max(0, min(lv.H-L.mapH, camY))
	}
	return camX, camY
}

func (p *play) drawMap(L layout) {
	c := p.a.cv
	lv := p.level
	s := p.snap.Self
	p.updateVisibility()
	camX, camY := p.camera(L)
	day := 1.0
	if lv.Lit {
		day = daylight(p.snap.TimeOfDay)
	}
	memFG := col("#34344a")
	oy := 1
	for sy := 0; sy < L.mapH; sy++ {
		for sx := 0; sx < L.mapW; sx++ {
			wx, wy := camX+sx, camY+sy
			if !lv.In(wx, wy) {
				continue
			}
			i := wy*lv.W + wx
			t := content.Tile(lv.Tiles[i])
			switch {
			case p.visible[i]:
				fg, bg := col(t.FG), col(t.BG)
				if t.BG == "" {
					bg = cBlack
				}
				var k, tt float64
				if lv.Lit {
					k = 0.42 + 0.58*day
					tt = (1 - day) * 0.3
				} else {
					d := math.Sqrt(float64((wx-s.X)*(wx-s.X) + (wy-s.Y)*(wy-s.Y)))
					k = 1 - 0.6*d/float64(max(1, s.Vision))
					tt = 0
				}
				c.put(sx, sy+oy, t.Rune, shade(fg, k, nightTint, tt), shade(bg, k, nightTint, tt*0.5))
			case p.explored.Get(i):
				c.put(sx, sy+oy, t.Rune, memFG, cBlack)
			}
		}
	}

	// entities
	ents := append([]proto.EntityView(nil), p.snap.Entities...)
	order := map[uint8]int{uint8(game.KItem): 0, uint8(game.KNPC): 1, uint8(game.KMonster): 2, uint8(game.KPlayer): 3, uint8(game.KProjectile): 4}
	sort.SliceStable(ents, func(i, j int) bool { return order[ents[i].Kind] < order[ents[j].Kind] })
	vis := func(x, y int) bool { return lv.In(x, y) && p.visible[y*lv.W+x] }
	for _, e := range ents {
		if !vis(e.X, e.Y) {
			continue
		}
		sx, sy := e.X-camX, e.Y-camY+oy
		if sx < 0 || sy < oy || sx >= L.mapW || sy >= oy+L.mapH {
			continue
		}
		fg := col(e.Color)
		bg := cBlack
		if t := content.Tile(lv.At(e.X, e.Y)); t.BG != "" {
			bg = shade(col(t.BG), 0.8, nightTint, 0)
		}
		if e.ID == p.welcome.YouID {
			bg = col("#202840")
		}
		if e.Boss {
			bg = col("#3a0a2a")
		}
		c.put(sx, sy, e.Glyph, fg, bg)
	}

	// effects
	now := time.Now()
	kept := p.fxs[:0]
	for _, f := range p.fxs {
		age := now.Sub(f.born).Seconds() * 1000
		if age > float64(f.Ms) {
			continue
		}
		kept = append(kept, f)
		if !vis(f.X, f.Y) {
			continue
		}
		sx, sy := f.X-camX, f.Y-camY+oy
		if f.Text == "" {
			if sx >= 0 && sx < L.mapW && sy >= oy && sy < oy+L.mapH {
				c.put(sx, sy, f.Glyph, col(f.Color), cBlack)
			}
			continue
		}
		rise := int(age / float64(f.Ms) * 2.5)
		ty := sy - 1 - rise
		tx := sx - runeLen(f.Text)/2
		if ty >= oy && ty < oy+L.mapH {
			for i, r := range []rune(f.Text) {
				if tx+i >= 0 && tx+i < L.mapW {
					c.put(tx+i, ty, r, col(f.Color), cBlack)
				}
			}
		}
	}
	p.fxs = kept

	// speech bubbles
	for _, e := range ents {
		if e.Speech == "" || !vis(e.X, e.Y) {
			continue
		}
		lines := wrap(e.Speech, 32)
		if len(lines) > 3 {
			lines = lines[:3]
		}
		sx, sy := e.X-camX, e.Y-camY+oy
		for i, l := range lines {
			ty := sy - len(lines) + i
			tx := max(0, min(L.mapW-runeLen(l), sx-runeLen(l)/2))
			if ty >= oy {
				c.textClip(tx, ty, L.mapW-tx, l, col("#ffffff"), col("#303048"))
			}
		}
	}

	// boss health bar
	for _, e := range ents {
		if e.Boss && vis(e.X, e.Y) {
			bw := min(40, L.mapW-4)
			bx := (L.mapW - bw) / 2
			c.textClip(bx, oy, bw, e.Name, col("#ff6aff"), cBlack)
			c.bar(bx, oy+1, bw, float64(e.HP)/100, col("#c02080"), col("#401030"))
			break
		}
	}
}

func (p *play) drawTop(L layout) {
	c := p.a.cv
	c.fill(0, 0, L.w, 1, ' ', cText, col("#181828"))
	t := p.snap.TimeOfDay
	hh := int(t * 24)
	mm := int((t*24 - float64(hh)) * 60)
	icon := "☀"
	if daylight(t) < 0.35 {
		icon = "☾"
	}
	if asciiUI {
		icon = ""
	}
	x := c.text(1, 0, p.level.Name, cAccent, col("#181828"))
	if r := p.snap.Self.Region; r != "" {
		x = c.text(x, 0, " • "+r, col("#d8c890"), col("#181828"))
	}
	if p.level.Lit {
		x = c.text(x+2, 0, fmt.Sprintf("%s %02d:%02d", icon, hh, mm), cText, col("#181828"))
	}
	x = c.text(x+2, 0, fmt.Sprintf("(%d,%d)", p.snap.Self.X, p.snap.Self.Y), cDim, col("#181828"))
	right := fmt.Sprintf("Игроков: %d", len(p.snap.Online))
	if p.snap.Self.PvP && len(p.snap.Online) > 1 {
		if p.snap.Self.Safe {
			right = "мирная зона • " + right
		} else {
			right = "бой между игроками • " + right
		}
	}
	if p.welcome.AI {
		right += " • ИИ вкл"
	}
	if p.admin() {
		right = "АДМИН (F9) • " + right
	}
	if p.isHost() {
		if a := p.srv.Listening(); a != "" {
			right += " • сервер " + a
		}
	} else {
		right += " • онлайн"
	}
	right += " • ? помощь"
	c.text(L.w-runeLen(right)-1, 0, right, cDim, col("#181828"))
	_ = x
}

func (p *play) drawSidebar(L layout) {
	c := p.a.cv
	x0 := L.w - sideW
	c.box(x0, 1, sideW, L.h-1, "", cBorder)
	s := p.snap.Self
	x := x0 + 2
	y := 2
	w := sideW - 4
	className := ""
	if p.sheet != nil {
		if cl := content.Class(p.sheet.Class); cl != nil {
			className = cl.Name
		}
	}
	c.textClip(x, y, w, p.name, cAccent, cPanel)
	y++
	c.textClip(x, y, w, fmt.Sprintf("%s, уровень %d", className, s.Level), cText, cPanel)
	y += 2
	c.text(x, y, fmt.Sprintf("Здоровье %d/%d", int(math.Ceil(s.HP)), int(s.MaxHP)), cText, cPanel)
	y++
	c.bar(x, y, w, s.HP/math.Max(1, s.MaxHP), cHP, col("#401010"))
	y++
	c.text(x, y, fmt.Sprintf("Мана     %d/%d", int(s.MP), int(s.MaxMP)), cText, cPanel)
	y++
	c.bar(x, y, w, s.MP/math.Max(1, s.MaxMP), cMP, col("#101840"))
	y++
	c.text(x, y, fmt.Sprintf("Опыт     %d/%d", s.XP, s.XPNext), cText, cPanel)
	y++
	c.bar(x, y, w, float64(s.XP)/math.Max(1, float64(s.XPNext)), cXP, col("#302808"))
	y++
	c.text(x, y, fmt.Sprintf("Золото   %d", s.Gold), col("#ffd700"), cPanel)
	y += 2

	c.text(x, y, "Умения", cAccent, cPanel)
	y++
	if p.sheet != nil {
		for i, key := range p.sheet.Hotbar {
			label := "—"
			fg := cDim
			if a := content.Ability(key); a != nil {
				label = p.abilityName(key)
				fg = col(a.Color)
				if a.Mana > s.MP {
					fg = cDim
				}
			}
			c.text(x, y, fmt.Sprintf("%d", i+1), cAccent, cPanel)
			c.textClip(x+2, y, w-8, label, fg, cPanel)
			if cd := s.Cooldown[i]; cd > 0 {
				c.bar(x+w-5, y, 5, float64(cd), col("#808080"), col("#303030"))
			}
			y++
		}
		hp, mp := 0, 0
		for _, it := range p.sheet.Inventory {
			if d := content.Item(it.Key); d != nil && d.Kind == "consumable" {
				if d.Heal > 0 {
					hp += it.Qty
				}
				if d.Mana > 0 {
					mp += it.Qty
				}
			}
		}
		c.text(x, y, fmt.Sprintf("Q лечение ×%d  R мана ×%d", hp, mp), cDim, cPanel)
		y++
	}
	y++
	if len(s.Buffs) > 0 {
		c.text(x, y, "Эффекты", cAccent, cPanel)
		y++
		for _, b := range s.Buffs {
			c.textClip(x, y, w, fmt.Sprintf("%s %dс", b.Name, (b.Left+999)/1000), col(b.Color), cPanel)
			y++
		}
		y++
	}
	// party members
	if len(s.Party) > 0 && y < L.h-6 {
		c.text(x, y, "Группа", cAccent, cPanel)
		y++
		for _, m := range s.Party {
			if y >= L.h-5 {
				break
			}
			name := m.Name
			if m.Leader {
				name = "♛ " + name
				if asciiUI {
					name = "* " + m.Name
				}
			}
			fg := cText
			if m.Dead {
				fg, name = cBad, name+" — пал"
			}
			c.textClip(x, y, w-6, name, fg, cPanel)
			c.text(x+w-5, y, fmt.Sprintf("%5s", fmt.Sprintf("ур%d", m.Level)), cDim, cPanel)
			y++
			if m.Where != "" {
				c.textClip(x+1, y, w-1, m.Where, cDim, cPanel)
			} else {
				c.bar(x, y, w-7, m.HP/math.Max(1, m.MaxHP), cHP, col("#401010"))
				c.bar(x+w-6, y, 6, m.MP/math.Max(1, m.MaxMP), cMP, col("#101840"))
			}
			y++
		}
		y++
	}
	if len(s.Invites) > 0 && y < L.h-4 {
		c.textClip(x, y, w, fmt.Sprintf("Приглашение: %s [G]", s.Invites[0]), cGood, cPanel)
		y += 2
	}
	// the enemy we fight (or the nearest one) with its resistances
	if t := s.Target; t != nil && y < L.h-6 {
		c.text(x, y, "Цель", cAccent, cPanel)
		if t.Level > 0 {
			lv := fmt.Sprintf("ур. %d", t.Level)
			c.text(x+w-runeLen(lv), y, lv, cDim, cPanel)
		}
		y++
		c.textClip(x, y, w, t.Name, col(t.Color), cPanel)
		y++
		c.bar(x, y, w, float64(t.HP)/100, cHP, col("#401010"))
		y++
		y = p.drawResists(x, y, w, L.h-4, t.Res)
		for _, e := range t.Effects {
			if y >= L.h-4 {
				break
			}
			c.textClip(x, y, w, fmt.Sprintf("• %s %dс", e.Name, (e.Left+999)/1000), col(e.Color), cPanel)
			y++
		}
		y++
	}
	if s.AttrPoints > 0 && y < L.h-3 {
		c.textClip(x, y, w, fmt.Sprintf("+%d очк. характеристик [C]", s.AttrPoints), cGood, cPanel)
		y++
	}
	if s.SkillPoints > 0 && y < L.h-3 {
		c.textClip(x, y, w, fmt.Sprintf("+%d очк. навыков [K]", s.SkillPoints), cGood, cPanel)
		y++
	}
	switch s.StandingOn {
	case "dungeon", "stairs_down", "stairs_up":
		if y < L.h-3 {
			c.textClip(x, y, w, "[E] пройти", cAccent, cPanel)
			y++
		}
	}
	if len(p.snap.Online) > 1 && y < L.h-3 {
		y++
		c.text(x, y, "В мире", cAccent, cPanel)
		y++
		for _, n := range p.snap.Online {
			if y >= L.h-2 {
				break
			}
			c.textClip(x, y, w, "• "+n, cText, cPanel)
			y++
		}
	}
}

// drawResists writes "Стойкость:" and "Уязвимость:" lists of damage types,
// wrapping inside width w; returns the next free row.
func (p *play) drawResists(x, y, w, maxY int, res map[string]int) int {
	c := p.a.cv
	for _, good := range []bool{true, false} {
		var items []string
		var colors []tcell.Color
		for _, dt := range content.DamageTypes() {
			v, ok := res[dt.Key]
			if !ok || (v > 0) != good {
				continue
			}
			items = append(items, fmt.Sprintf("%s %d%%", dt.Short, v))
			colors = append(colors, col(dt.Color))
		}
		if len(items) == 0 || y >= maxY {
			continue
		}
		label, lc := "Стойк.", cGood
		if !good {
			label, lc = "Уязв.", cBad
		}
		cx := c.text(x, y, label, lc, cPanel) + 1
		for i, it := range items {
			if cx+runeLen(it) > x+w {
				y++
				if y >= maxY {
					return y
				}
				cx = x + 2
			}
			cx = c.text(cx, y, it, colors[i], cPanel) + 1
		}
		y++
	}
	return y
}

func (p *play) drawLog(L layout) {
	c := p.a.cv
	lh := L.h - L.logY
	c.fill(0, L.logY, L.mapW, lh, ' ', cText, col("#0a0a12"))
	var lines []logEntry
	for i := len(p.logs) - 1; i >= 0 && len(lines) < lh; i-- {
		wrapped := wrap(p.logs[i].text, L.mapW-2)
		for j := len(wrapped) - 1; j >= 0 && len(lines) < lh; j-- {
			lines = append(lines, logEntry{wrapped[j], p.logs[i].color})
		}
	}
	for i, l := range lines {
		fg := col(l.color)
		if l.color == "" {
			fg = cText
		}
		if i > 2 {
			fg = shade(fg, 0.65, [3]float64{}, 0)
		}
		c.textClip(1, L.h-1-i, L.mapW-2, l.text, fg, col("#0a0a12"))
	}
}

func padRight(s string, n int) string {
	if d := n - runeLen(s); d > 0 {
		return s + strings.Repeat(" ", d)
	}
	return s
}

var _ = tcell.ColorDefault
