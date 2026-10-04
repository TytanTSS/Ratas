package client

import (
	"github.com/gdamore/tcell/v2"

	"ratas/internal/game"
	"ratas/internal/proto"
)

// adminAction is a one-key action of the admin window (F9).
type adminAction struct {
	label, cmd string
}

var adminActions = []adminAction{
	{"Бессмертие вкл/выкл", "/god"},
	{"Умения без маны и перезарядки вкл/выкл", "/nocd"},
	{"Полное здоровье и мана", "/heal"},
	{"+1 уровень", "/level"},
	{"+10 очков навыков и характеристик", "/points 10"},
	{"+1000 золота", "/gold 1000"},
	{"Убить врагов вокруг", "/kill"},
	{"Открыть карту уровня", "/reveal"},
	{"Сделать день", "/time day"},
	{"Сделать ночь", "/time night"},
	{"Открыть все секретные классы, подклассы и навыки", "/unlock all"},
	{"Легендарное оружие и доспех", "/give long_sword legendary"},
	{"Список уникальных персонажей", "/uniques"},
	{"Список уровней мира", "/levels"},
	{"Все команды (/help)", "/help"},
	{"Ввести команду…", ""},
}

func (p *play) admin() bool { return p.welcome != nil && p.welcome.Admin }

func (p *play) adminCmd(text string) {
	p.send(proto.ClientMsg{Cmd: &proto.Command{Kind: "admin", Text: text}})
}

func (p *play) keyAdmin(ev *tcell.EventKey) {
	n := len(adminActions)
	switch ev.Key() {
	case tcell.KeyEscape, tcell.KeyF9:
		p.mode = modeGame
	case tcell.KeyUp:
		p.sel = (p.sel + n - 1) % n
	case tcell.KeyDown, tcell.KeyTab:
		p.sel = (p.sel + 1) % n
	case tcell.KeyEnter:
		a := adminActions[min(p.sel, n-1)]
		if a.cmd == "" {
			p.mode = modeChat
			p.chat.Set("/")
			return
		}
		p.adminCmd(a.cmd)
		if a.cmd == "/give long_sword legendary" {
			p.adminCmd("/give plate_armor legendary")
		}
	case tcell.KeyRune:
		if ev.Rune() == '/' {
			p.mode = modeChat
			p.chat.Set("/")
		}
	}
}

func (p *play) drawAdmin() {
	c := p.a.cv
	x, y, w, h := p.panel(64, len(adminActions)+len(game.AdminCommands)/3+8, "Администратор (F9)")
	for i, a := range adminActions {
		bg, fg := cPanel, cText
		if i == p.sel {
			bg, fg = cSelBG, cAccent
			c.fill(x+1, y+2+i, w-2, 1, ' ', fg, bg)
		}
		c.textClip(x+3, y+2+i, w-5, a.label, fg, bg)
	}
	yy := y + 3 + len(adminActions)
	c.textClip(x+2, yy, w-4, "Команды в чате (T):", cAccent, cPanel)
	line := ""
	for _, cmd := range game.AdminCommands {
		if runeLen(line)+runeLen(cmd.Name)+1 > w-6 {
			yy++
			c.textClip(x+3, yy, w-5, line, cDim, cPanel)
			line = ""
		}
		line += cmd.Name + " "
	}
	if line != "" && yy+1 < y+h-1 {
		c.textClip(x+3, yy+1, w-5, line, cDim, cPanel)
	}
	p.footer(x, y+h-1, w, " Enter — выполнить • / — ввести команду • Esc — закрыть ")
}
