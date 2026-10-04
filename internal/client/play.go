package client

import (
	"fmt"
	"strings"
	"time"

	"github.com/gdamore/tcell/v2"

	"ratas/internal/content"
	"ratas/internal/game"
	"ratas/internal/proto"
	"ratas/internal/server"
	"ratas/internal/world"
)

type mode int

const (
	modeGame mode = iota
	modeInventory
	modeSkills
	modeChar
	modeJournal
	modeMap
	modeChat
	modePause
	modeHelp
	modeDialogue
	modeTrade
	modeClass
	modeParty
)

type logEntry struct {
	text  string
	color string
}

type fxEntry struct {
	proto.FX
	born time.Time
}

type play struct {
	a        *App
	conn     *proto.Conn
	srv      *server.Server // nil when connected to a remote world
	name     string
	class    string
	slotPath string

	inbox chan *proto.ServerMsg
	outq  chan proto.ClientMsg
	dead  chan error
	done  chan struct{}

	welcome  *proto.Welcome
	level    *world.Level
	levelVer int
	// copies handed to the graphical renderer
	sceneLevel *world.Level
	sceneVer   int
	explored   world.Bitset
	visible    []bool
	snap       *proto.Snapshot
	sheet      *proto.PlayerSheet
	logs       []logEntry
	fxs        []fxEntry
	dialogue   *proto.Dialogue

	mode     mode
	sel      int
	tab      int
	tradeCol int
	chat     textInput
	talk     textInput
	paused   bool
	quit     bool
	kick     string
	notice   string
	noticeAt time.Time
	region   string // current overworld region, announced with a banner
	regionAt time.Time
}

func newPlay(a *App, conn *proto.Conn, srv *server.Server, name, class string) *play {
	return &play{
		a: a, conn: conn, srv: srv, name: name, class: class,
		inbox: make(chan *proto.ServerMsg, 512),
		outq:  make(chan proto.ClientMsg, 128),
		dead:  make(chan error, 1),
		done:  make(chan struct{}),
		chat:  textInput{maxLen: 160},
		talk:  textInput{maxLen: 300},
	}
}

func (p *play) isHost() bool { return p.srv != nil }

func (p *play) send(m proto.ClientMsg) {
	select {
	case p.outq <- m:
	default:
	}
}

func (p *play) cmd(kind, key string, idx int) {
	p.send(proto.ClientMsg{Cmd: &proto.Command{Kind: kind, Key: key, Index: idx}})
}

func (p *play) run() {
	go func() {
		for m := range p.outq {
			if err := p.conn.Send(m); err != nil {
				return
			}
		}
	}()
	go func() {
		for {
			m, err := p.conn.RecvServer()
			if err != nil {
				select {
				case p.dead <- err:
				default:
				}
				return
			}
			select {
			case p.inbox <- m:
			case <-p.done:
				return
			}
		}
	}()
	p.send(proto.ClientMsg{Hello: &proto.Hello{Name: p.name, Class: p.class, Version: proto.Version}})
	defer func() {
		close(p.done)
		close(p.outq)
		p.conn.Close()
		if p.a.gfx != nil {
			p.a.gfx.Publish(nil)
		}
	}()

	frame := time.NewTicker(33 * time.Millisecond)
	defer frame.Stop()
	timeout := time.After(15 * time.Second)
	for !p.quit {
		select {
		case m := <-p.inbox:
			p.apply(m)
			// drain whatever else is queued before drawing
			for drained := false; !drained; {
				select {
				case m := <-p.inbox:
					p.apply(m)
				default:
					drained = true
				}
			}
		case err := <-p.dead:
			if p.kick == "" {
				p.kick = "Соединение потеряно: " + err.Error()
			}
			p.quit = true
		case ev, ok := <-p.a.events:
			if !ok {
				return
			}
			switch ev := ev.(type) {
			case *tcell.EventKey:
				p.key(ev)
			case *tcell.EventMouse:
				p.mouse(ev)
			case *tcell.EventResize:
				p.a.scr.Sync()
			case *tcell.EventInterrupt:
				if ev.Data() == closeRequest {
					p.a.closing = true
					p.quit = true
				}
			}
		case <-frame.C:
			p.draw()
		case <-timeout:
			if p.welcome == nil {
				p.kick = "Сервер не ответил."
				p.quit = true
			}
		}
	}
	if p.kick != "" {
		p.a.message("Отключено", p.kick)
	}
}

func (p *play) apply(m *proto.ServerMsg) {
	if m.Kick != "" {
		p.kick = m.Kick
		p.quit = true
		return
	}
	if m.Welcome != nil {
		p.welcome = m.Welcome
		if p.srv == nil {
			if db, err := content.FromJSON(m.Welcome.Content); err == nil {
				content.Use(db)
			}
		}
		if m.Welcome.NeedClass {
			p.mode, p.sel = modeClass, 0
		} else if p.mode == modeClass {
			p.mode = modeGame
		}
	}
	if m.Level != nil {
		p.setLevel(m.Level)
	}
	if p.level != nil && len(m.Tiles) > 0 {
		for _, t := range m.Tiles {
			p.level.Set(t.X, t.Y, t.T)
		}
		p.levelVer++
	}
	if m.Snap != nil {
		p.snap = m.Snap
		if r := m.Snap.Self.Region; r != p.region {
			p.region = r
			if r != "" {
				p.regionAt = time.Now()
			}
		}
		if p.a.gfx != nil {
			if len(m.Snap.FX) > 0 {
				p.a.gfx.Effects(m.Snap.FX)
			}
		} else {
			now := time.Now()
			for _, f := range m.Snap.FX {
				p.fxs = append(p.fxs, fxEntry{FX: f, born: now})
			}
		}
	}
	if m.Sheet != nil {
		p.sheet = m.Sheet
	}
	for _, l := range m.Logs {
		p.addLog(l.Text, l.Color)
	}
	if d := m.Dialogue; d != nil {
		if d.Close {
			p.dialogue = nil
			if p.mode == modeDialogue || p.mode == modeTrade {
				p.mode = modeGame
			}
		} else {
			wasOpen := p.dialogue != nil && p.dialogue.NPC == d.NPC
			p.dialogue = d
			if len(d.Trade) > 0 {
				if p.mode != modeTrade {
					p.mode, p.sel, p.tradeCol = modeTrade, 0, 0
				}
			} else if p.mode != modeDialogue {
				p.mode = modeDialogue
				if !wasOpen {
					p.sel = 0
					p.talk.Set("")
				}
			}
		}
	}
}

func (p *play) addLog(text, color string) {
	p.logs = append(p.logs, logEntry{text, color})
	if len(p.logs) > 300 {
		p.logs = p.logs[len(p.logs)-300:]
	}
}

func (p *play) setLevel(ld *proto.LevelData) {
	tiles, err := proto.Decompress(ld.Tiles)
	if err != nil || len(tiles) != ld.W*ld.H {
		p.addLog("Повреждённые данные уровня", "#ff4a4a")
		return
	}
	p.level = &world.Level{ID: ld.ID, Name: ld.Name, W: ld.W, H: ld.H, Tiles: tiles, Lit: ld.Lit, Depth: ld.Depth}
	p.explored = world.NewBitset(ld.W * ld.H)
	copy(p.explored, ld.Explored)
	p.visible = make([]bool, ld.W*ld.H)
	p.fxs = nil
	p.levelVer++
}

func (p *play) setNotice(s string) {
	p.notice = s
	p.noticeAt = time.Now()
}

// ---- input ----

func dirFromKey(ev *tcell.EventKey) world.Dir {
	switch ev.Key() {
	case tcell.KeyUp:
		return world.DirUp
	case tcell.KeyDown:
		return world.DirDown
	case tcell.KeyLeft:
		return world.DirLeft
	case tcell.KeyRight:
		return world.DirRight
	case tcell.KeyRune:
		switch normKey(ev.Rune()) {
		case 'w':
			return world.DirUp
		case 's':
			return world.DirDown
		case 'a':
			return world.DirLeft
		case 'd':
			return world.DirRight
		}
	}
	return world.DirNone
}

func (p *play) setPause(on bool) {
	if !p.isHost() || p.srv.Listening() != "" {
		return
	}
	idx := 0
	if on {
		idx = 1
	}
	p.paused = on
	p.cmd("pause", "", idx)
}

func (p *play) key(ev *tcell.EventKey) {
	switch p.mode {
	case modeGame:
		p.keyGame(ev)
	case modeChat:
		switch ev.Key() {
		case tcell.KeyEscape:
			p.mode = modeGame
		case tcell.KeyEnter:
			if t := strings.TrimSpace(p.chat.String()); t != "" {
				p.send(proto.ClientMsg{Cmd: &proto.Command{Kind: "chat", Text: t}})
			}
			p.chat.Set("")
			p.mode = modeGame
		default:
			p.chat.handle(ev)
		}
	case modePause:
		p.keyPause(ev)
	case modeClass:
		p.keyClass(ev)
	case modeDialogue:
		p.keyDialogue(ev)
	case modeTrade:
		p.keyTrade(ev)
	case modeInventory:
		p.keyInventory(ev)
	case modeSkills:
		p.keySkills(ev)
	case modeChar:
		p.keyChar(ev)
	case modeParty:
		p.keyParty(ev)
	default: // help, map, journal
		if ev.Key() == tcell.KeyEscape || ev.Key() == tcell.KeyEnter || ev.Key() == tcell.KeyRune {
			p.mode = modeGame
		}
	}
}

func (p *play) keyGame(ev *tcell.EventKey) {
	if p.snap != nil && p.snap.Self.Dead {
		if ev.Key() == tcell.KeyEnter || (ev.Key() == tcell.KeyRune && normKey(ev.Rune()) == 'e') {
			p.cmd("respawn", "", 0)
			return
		}
	}
	if d := dirFromKey(ev); d != world.DirNone {
		p.send(proto.ClientMsg{Input: &proto.Input{Move: uint8(d)}})
		return
	}
	switch ev.Key() {
	case tcell.KeyEscape:
		p.mode, p.sel = modePause, 0
		p.setPause(true)
		return
	case tcell.KeyEnter:
		p.send(proto.ClientMsg{Input: &proto.Input{Interact: true}})
		return
	case tcell.KeyF1:
		p.mode = modeHelp
		return
	case tcell.KeyF5:
		p.quickSave()
		return
	case tcell.KeyTab:
		p.mode = modeMap
		return
	}
	if ev.Key() != tcell.KeyRune {
		return
	}
	r := ev.Rune()
	if r >= '1' && r <= '6' {
		p.send(proto.ClientMsg{Input: p.aimed(proto.Input{Ability: int8(r - '0')})})
		return
	}
	switch r {
	case ' ':
		p.send(proto.ClientMsg{Input: p.aimed(proto.Input{Attack: true})})
		return
	case '?':
		p.mode = modeHelp
		return
	}
	switch normKey(r) {
	case 'e', 'f':
		p.send(proto.ClientMsg{Input: &proto.Input{Interact: true}})
	case 'q':
		p.cmd("potion", "health", 0)
	case 'r':
		p.cmd("potion", "mana", 0)
	case 'i':
		p.mode, p.sel = modeInventory, 0
	case 'k':
		p.mode, p.sel = modeSkills, 0
	case 'c':
		p.mode, p.sel = modeChar, 0
	case 'j':
		p.mode = modeJournal
	case 'g':
		p.mode, p.sel = modeParty, 0
	case 'm':
		p.mode = modeMap
	case 't':
		p.mode = modeChat
		p.chat.Set("")
	case 'h':
		p.mode = modeHelp
	}
}

// aimed points an attack or ability at the tile under the mouse cursor
// (graphics mode); without a mouse the server picks the nearest enemy.
func (p *play) aimed(in proto.Input) *proto.Input {
	if am, ok := p.a.gfx.(Aimer); ok {
		if at, ok := am.Aim(); ok {
			in.Aim, in.AimX, in.AimY = true, int32(at.X), int32(at.Y)
		}
	}
	return &in
}

// mouse: on the map the left button attacks and the right button uses the
// first ability, both aimed at the cursor.
func (p *play) mouse(ev *tcell.EventMouse) {
	if p.mode != modeGame || p.snap == nil || p.snap.Self.Dead {
		return
	}
	switch {
	case ev.Buttons()&tcell.Button1 != 0:
		p.send(proto.ClientMsg{Input: p.aimed(proto.Input{Attack: true})})
	case ev.Buttons()&tcell.Button2 != 0:
		p.send(proto.ClientMsg{Input: p.aimed(proto.Input{Ability: 1})})
	}
}

func (p *play) quickSave() {
	if !p.isHost() || p.slotPath == "" {
		p.addLog("Сохранять мир может только хозяин.", "#ff8080")
		return
	}
	var err error
	p.srv.Call(func(g *game.Game) { err = g.Save(p.slotPath) })
	if err != nil {
		p.addLog("Ошибка сохранения: "+err.Error(), "#ff4a4a")
	} else {
		p.addLog("Мир сохранён.", "#a0ffa0")
		p.setNotice("Сохранено")
	}
}

type pauseItem struct {
	key, label string
}

func (p *play) pauseItems() []pauseItem {
	items := []pauseItem{{"resume", "Продолжить"}}
	if p.isHost() {
		items = append(items, pauseItem{"save", "Сохранить мир (F5)"})
		if p.srv.Listening() == "" {
			items = append(items, pauseItem{"open", fmt.Sprintf("Открыть мир для сети (порт %d)", p.a.cfg.Port)})
		}
	}
	return append(items, pauseItem{"help", "Помощь"}, pauseItem{"exit", "Выйти в главное меню"})
}

func (p *play) keyPause(ev *tcell.EventKey) {
	items := p.pauseItems()
	switch ev.Key() {
	case tcell.KeyEscape:
		p.mode = modeGame
		p.setPause(false)
	case tcell.KeyUp:
		p.sel = (p.sel + len(items) - 1) % len(items)
	case tcell.KeyDown, tcell.KeyTab:
		p.sel = (p.sel + 1) % len(items)
	case tcell.KeyEnter:
		switch items[min(p.sel, len(items)-1)].key {
		case "resume":
			p.mode = modeGame
			p.setPause(false)
		case "save":
			p.quickSave()
		case "open":
			p.setPause(false)
			if err := p.srv.Listen(fmt.Sprintf(":%d", p.a.cfg.Port)); err != nil {
				p.addLog("Не удалось открыть порт: "+err.Error(), "#ff4a4a")
			} else {
				addrs := server.LANAddresses()
				p.addLog(fmt.Sprintf("Мир открыт для сети! Адрес для друзей: %s (порт %d)", strings.Join(addrs, ", "), p.a.cfg.Port), "#80ff80")
			}
			p.mode = modeGame
		case "help":
			p.mode = modeHelp
		case "exit":
			p.quit = true
		}
	}
}

func (p *play) keyClass(ev *tcell.EventKey) {
	classes := content.Classes()
	switch ev.Key() {
	case tcell.KeyUp:
		p.sel = (p.sel + len(classes) - 1) % len(classes)
	case tcell.KeyDown, tcell.KeyTab:
		p.sel = (p.sel + 1) % len(classes)
	case tcell.KeyEnter:
		p.cmd("choose_class", classes[p.sel].Key, 0)
	case tcell.KeyEscape:
		p.quit = true
	}
}

func (p *play) keyDialogue(ev *tcell.EventKey) {
	d := p.dialogue
	if d == nil {
		p.mode = modeGame
		return
	}
	n := len(d.Options)
	switch ev.Key() {
	case tcell.KeyEscape:
		p.cmd("talk_close", "", 0)
		p.dialogue = nil
		p.mode = modeGame
		return
	case tcell.KeyUp:
		p.sel = (p.sel + n - 1) % max(1, n)
		return
	case tcell.KeyDown:
		p.sel = (p.sel + 1) % max(1, n)
		return
	case tcell.KeyEnter:
		if d.AI && strings.TrimSpace(p.talk.String()) != "" {
			if !d.Waiting {
				text := strings.TrimSpace(p.talk.String())
				p.addLog("Вы: "+text, "#a0c0ff")
				p.send(proto.ClientMsg{Cmd: &proto.Command{Kind: "talk", Text: text}})
				p.talk.Set("")
			}
			return
		}
		p.chooseOption(p.sel)
		return
	}
	if ev.Key() == tcell.KeyRune && p.talk.String() == "" && ev.Rune() >= '1' && ev.Rune() <= '9' {
		p.chooseOption(int(ev.Rune() - '1'))
		return
	}
	if d.AI {
		p.talk.handle(ev)
	}
}

func (p *play) chooseOption(i int) {
	d := p.dialogue
	if d == nil || i < 0 || i >= len(d.Options) {
		return
	}
	if len(d.Options) == 1 && d.Options[0] == "Уйти" {
		p.dialogue = nil
		p.mode = modeGame
		return
	}
	p.cmd("talk_option", "", i)
}

func (p *play) keyTrade(ev *tcell.EventKey) {
	d := p.dialogue
	if d == nil {
		p.mode = modeGame
		return
	}
	inv := 0
	if p.sheet != nil {
		inv = len(p.sheet.Inventory)
	}
	count := len(d.Trade)
	if p.tradeCol == 1 {
		count = inv
	}
	switch ev.Key() {
	case tcell.KeyEscape:
		p.dialogue.Trade = nil
		p.mode = modeDialogue
	case tcell.KeyTab, tcell.KeyLeft, tcell.KeyRight:
		p.tradeCol = 1 - p.tradeCol
		p.sel = 0
	case tcell.KeyUp:
		if count > 0 {
			p.sel = (p.sel + count - 1) % count
		}
	case tcell.KeyDown:
		if count > 0 {
			p.sel = (p.sel + 1) % count
		}
	case tcell.KeyEnter:
		if p.tradeCol == 0 && p.sel < len(d.Trade) {
			p.cmd("buy", d.Trade[p.sel].Item.Key, 0)
		} else if p.tradeCol == 1 && p.sel < inv {
			p.cmd("sell", "", p.sel)
			if p.sel == inv-1 && p.sel > 0 && p.sheet.Inventory[p.sel].Qty <= 1 {
				p.sel--
			}
		}
	}
}

// inventory rows: equipment slots first, then the backpack
func (p *play) invRows() (slots []string, items []proto.ItemView) {
	if p.sheet == nil {
		return nil, nil
	}
	return game.EquipSlots, p.sheet.Inventory
}

func (p *play) keyInventory(ev *tcell.EventKey) {
	slots, items := p.invRows()
	n := len(slots) + len(items)
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
		if p.sel < len(slots) {
			p.cmd("unequip", slots[p.sel], 0)
		} else if i := p.sel - len(slots); i < len(items) {
			p.cmd("use", "", i)
		}
		return
	}
	if ev.Key() != tcell.KeyRune {
		return
	}
	switch normKey(ev.Rune()) {
	case 'i':
		p.mode = modeGame
	case 'l':
		if i := p.sel - len(slots); i >= 0 && i < len(items) {
			p.cmd("equip_left", "", i)
		}
	case 'x', 'g':
		if i := p.sel - len(slots); i >= 0 && i < len(items) {
			p.cmd("drop", "", i)
		}
	case 'o':
		p.cmd("sort", "", 0)
	case 'q':
		p.cmd("potion", "health", 0)
	case 'r':
		p.cmd("potion", "mana", 0)
	case 'e':
		if p.sel < len(slots) {
			p.cmd("unequip", slots[p.sel], 0)
		} else if i := p.sel - len(slots); i < len(items) {
			p.cmd("use", "", i)
		}
	}
}

var attrKeys = []string{"str", "dex", "int", "vit"}

func (p *play) keyChar(ev *tcell.EventKey) {
	switch ev.Key() {
	case tcell.KeyEscape:
		p.mode = modeGame
	case tcell.KeyUp:
		p.sel = (p.sel + 3) % 4
	case tcell.KeyDown:
		p.sel = (p.sel + 1) % 4
	case tcell.KeyEnter:
		p.cmd("alloc_attr", attrKeys[p.sel], 0)
	case tcell.KeyTab, tcell.KeyLeft, tcell.KeyRight:
		p.tab = (p.tab + 1) % 2
	case tcell.KeyRune:
		switch ev.Rune() {
		case '+', '=':
			p.cmd("alloc_attr", attrKeys[p.sel], 0)
		default:
			if normKey(ev.Rune()) == 'c' {
				p.mode = modeGame
			}
		}
	}
}
