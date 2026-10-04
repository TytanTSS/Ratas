package client

import (
	"fmt"
	"math"
	"slices"

	"github.com/gdamore/tcell/v2"
)

type partyRow struct {
	kind string // invite, member, player
	name string
}

func (p *play) partyRows() []partyRow {
	var rows []partyRow
	if p.snap == nil {
		return rows
	}
	s := p.snap.Self
	for _, n := range s.Invites {
		rows = append(rows, partyRow{"invite", n})
	}
	members := []string{}
	for _, m := range s.Party {
		rows = append(rows, partyRow{"member", m.Name})
		members = append(members, m.Name)
	}
	for _, n := range p.snap.Online {
		if n != p.name && !slices.Contains(members, n) {
			rows = append(rows, partyRow{"player", n})
		}
	}
	return rows
}

func (p *play) drawParty() {
	c := p.a.cv
	x, y, w, h := p.panel(76, 26, "Группа")
	s := p.snap.Self
	rows := p.partyRows()
	ry := y + 2
	section := ""
	leader := ""
	for _, m := range s.Party {
		if m.Leader {
			leader = m.Name
		}
	}
	for i, r := range rows {
		if ry >= y+h-3 {
			break
		}
		title := map[string]string{"invite": "Приглашения", "member": "Ваша группа", "player": "Игроки в мире"}[r.kind]
		if r.kind == "member" && leader != "" {
			title += " (лидер: " + leader + ")"
		}
		if title != section {
			if section != "" {
				ry++
			}
			section = title
			c.text(x+2, ry, title, cAccent, cPanel)
			ry++
		}
		bg := cPanel
		if i == p.sel {
			bg = cSelBG
			c.fill(x+1, ry, w-2, 1, ' ', cText, bg)
		}
		switch r.kind {
		case "invite":
			c.textClip(x+3, ry, w-6, r.name+" зовёт вас в группу — Enter принять, N отказаться", cGood, bg)
		case "member":
			for _, m := range s.Party {
				if m.Name != r.name {
					continue
				}
				fg := cText
				if m.Dead {
					fg = cBad
				}
				c.textClip(x+3, ry, 18, m.Name, fg, bg)
				c.text(x+22, ry, fmt.Sprintf("ур.%d", m.Level), cDim, bg)
				if m.Where != "" {
					c.textClip(x+29, ry, w-31, m.Where, cDim, bg)
				} else {
					c.bar(x+29, ry, 20, m.HP/math.Max(1, m.MaxHP), cHP, col("#401010"))
					c.bar(x+50, ry, 12, m.MP/math.Max(1, m.MaxMP), cMP, col("#101840"))
				}
				if len(m.Buffs) > 0 && ry+1 < y+h-3 {
					ry++
					bx := x + 5
					for _, b := range m.Buffs {
						if bx+runeLen(b.Name) > x+w-3 {
							break
						}
						bx = c.text(bx, ry, b.Name, col(b.Color), cPanel) + 2
					}
				}
			}
		case "player":
			c.textClip(x+3, ry, w-6, r.name+" — Enter пригласить", cText, bg)
		}
		ry++
	}
	if len(rows) == 0 {
		c.text(x+2, y+2, "В мире никого, кроме вас. Откройте мир для сети, чтобы позвать друзей.", cDim, cPanel)
	}
	info := "Члены группы не ранят друг друга и видят здоровье друг друга."
	if s.PvP {
		info += " Остальные игроки вне деревень могут на вас напасть."
	}
	for i, l := range wrap(info, w-4) {
		if y+h-3+i < y+h-1 {
			c.textClip(x+2, y+h-3+i, w-4, l, cDim, cPanel)
		}
	}
	p.footer(x, y+h-1, w, " ↑↓ выбор  Enter пригласить/принять  N отказ  L покинуть  K исключить  Esc ")
}

func (p *play) keyParty(ev *tcell.EventKey) {
	rows := p.partyRows()
	n := len(rows)
	switch ev.Key() {
	case tcell.KeyEscape:
		p.mode = modeGame
		return
	case tcell.KeyUp:
		p.sel = (p.sel + n - 1) % max(1, n)
		return
	case tcell.KeyDown:
		p.sel = (p.sel + 1) % max(1, n)
		return
	case tcell.KeyEnter:
		if p.sel < n {
			switch r := rows[p.sel]; r.kind {
			case "invite":
				p.cmd("party_accept", r.name, 0)
			case "player":
				p.cmd("party_invite", r.name, 0)
			}
		}
		return
	}
	if ev.Key() != tcell.KeyRune {
		return
	}
	switch normKey(ev.Rune()) {
	case 'g':
		p.mode = modeGame
	case 'n':
		if p.sel < n && rows[p.sel].kind == "invite" {
			p.cmd("party_decline", rows[p.sel].name, 0)
		}
	case 'l':
		p.cmd("party_leave", "", 0)
	case 'k':
		if p.sel < n && rows[p.sel].kind == "member" {
			p.cmd("party_kick", rows[p.sel].name, 0)
		}
	}
}
