package client

import (
	"fmt"
	"math/rand/v2"
	"net"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"time"

	"github.com/gdamore/tcell/v2"

	"ratas/internal/config"
	"ratas/internal/content"
	"ratas/internal/game"
	"ratas/internal/i18n"
	"ratas/internal/llm"
	"ratas/internal/proto"
	"ratas/internal/server"
)

// StartOptions lets command-line flags skip the main menu.
type StartOptions struct {
	Join  string // address to join
	Host  bool   // open the new/loaded world for network play
	Load  string // save slot to load
	New   bool   // start a new world immediately
	Seed  int64
	NoPvP bool // new worlds: players cannot hurt each other
	Name  string
	Class string
}

type App struct {
	scr     tcell.Screen
	cv      *canvas
	cfg     *config.Config
	events  chan tcell.Event
	mods    []string
	status  string
	gfx     Sink // graphical renderer, nil in the terminal
	closing bool // the window is being closed: leave every screen
}

// closeRequest is posted as an interrupt event to make the app exit cleanly.
const closeRequest = "ratas:close"

// RequestClose asks a running app (on scr) to save and exit.
func RequestClose(scr tcell.Screen) { scr.PostEvent(tcell.NewEventInterrupt(closeRequest)) }

// Run starts the terminal UI and blocks until the player quits.
func Run(cfg *config.Config, mods []string, opts StartOptions) error {
	scr, err := tcell.NewScreen()
	if err != nil {
		return err
	}
	return RunOn(scr, cfg, mods, opts)
}

// RunOn runs the UI on a given screen (a simulation screen in tests).
func RunOn(scr tcell.Screen, cfg *config.Config, mods []string, opts StartOptions) error {
	return RunWith(scr, nil, cfg, mods, opts)
}

// RunWith runs the UI and hands the world to a graphical renderer (sink).
func RunWith(scr tcell.Screen, sink Sink, cfg *config.Config, mods []string, opts StartOptions) error {
	if err := scr.Init(); err != nil {
		return err
	}
	defer scr.Fini()
	scr.SetStyle(tcell.StyleDefault.Background(cBlack).Foreground(cText))
	scr.HideCursor()
	i18n.SetLang(cfg.Language)
	asciiUI = cfg.ASCIIOnly && sink == nil
	a := &App{scr: scr, cv: &canvas{s: scr}, cfg: cfg, events: make(chan tcell.Event, 256), mods: mods, gfx: sink}
	go func() {
		for {
			ev := scr.PollEvent()
			if ev == nil {
				close(a.events)
				return
			}
			a.events <- ev
		}
	}()
	if opts.Name != "" {
		cfg.Name = opts.Name
	}
	switch {
	case opts.Join != "":
		a.joinGame(opts.Join, cfg.Name)
	case opts.Load != "":
		a.loadSlot(filepath.Join(config.SavesDir(), opts.Load+".sav"), opts.Load, opts.Host, cfg.Name)
	case opts.New:
		seed := opts.Seed
		if seed == 0 {
			seed = rand.Int64N(1_000_000)
		}
		a.newGame(seed, cfg.Name, opts.Class, opts.Host, !opts.NoPvP)
	}
	a.mainMenu()
	return nil
}

func (a *App) size() (int, int) {
	w, h := a.scr.Size()
	a.cv.w, a.cv.h = w, h
	return w, h
}

// nextKey waits for the next key event, handling resizes. ok=false on shutdown.
func (a *App) nextKey(redraw func()) (*tcell.EventKey, bool) {
	for {
		redraw()
		a.scr.Show()
		ev, ok := <-a.events
		if !ok {
			return nil, false
		}
		switch ev := ev.(type) {
		case *tcell.EventKey:
			if a.closing {
				return nil, false
			}
			return ev, true
		case *tcell.EventResize:
			a.scr.Sync()
		case *tcell.EventInterrupt:
			if ev.Data() == closeRequest {
				a.closing = true
				return nil, false
			}
		}
		if a.closing {
			return nil, false
		}
	}
}

var logoLetters = map[rune][5]string{
	'R': {" ____  ", "|  _ \\ ", "| |_) |", "|  _ < ", "|_| \\_\\"},
	'A': {"    _    ", "   / \\   ", "  / _ \\  ", " / ___ \\ ", "/_/   \\_\\"},
	'T': {" _____ ", "|_   _|", "  | |  ", "  | |  ", "  |_|  "},
	'S': {" ____  ", "/ ___| ", "\\___ \\ ", " ___) |", "|____/ "},
}

func logo() []string {
	lines := make([]string, 5)
	for _, r := range "RATAS" {
		for i := range lines {
			lines[i] += logoLetters[r][i] + " "
		}
	}
	return lines
}

func (a *App) drawBackdrop() int {
	w, h := a.size()
	a.scr.Clear()
	a.cv.fill(0, 0, w, h, ' ', cText, cBlack)
	// a little starfield for atmosphere (the window draws a living world instead)
	if a.gfx == nil {
		r := rand.New(rand.NewPCG(42, 7))
		for i := 0; i < w*h/40; i++ {
			x, y := r.IntN(max(1, w)), r.IntN(max(1, h))
			g := []rune{'.', '\'', '`', '*'}[r.IntN(4)]
			a.cv.put(x, y, g, col("#2a2a40"), cBlack)
		}
	}
	y := max(1, h/2-11)
	bg := cBlack
	if a.gfx != nil {
		// a dark band keeps the logo readable over the animated world
		bg = col("#0a0a12")
		a.cv.fill(0, y-1, w, 9, ' ', cText, bg)
	}
	for i, l := range logo() {
		c := shade(cTitle, 1-float64(i)*0.08, [3]float64{}, 0)
		a.cv.text((w-runeLen(l))/2, y+i, l, c, bg)
	}
	sub := "фэнтези ASCII RPG • реальное время • кооператив • ИИ-персонажи"
	a.cv.text((w-runeLen(sub))/2, y+6, sub, cDim, bg)
	return y + 8
}

type menuItem struct {
	key   string
	label string
	desc  string
}

// menu shows a vertical list; returns the chosen key or "" on Esc.
func (a *App) menu(title string, items []menuItem, sel int, footer func(y int)) string {
	for {
		ev, ok := a.nextKey(func() {
			y := a.drawBackdrop()
			w, h := a.size()
			bw := 46
			for _, it := range items {
				bw = max(bw, runeLen(it.label)+8)
			}
			bw = min(bw, w-2)
			bh := len(items) + 4
			bx := (w - bw) / 2
			a.cv.box(bx, y, bw, bh, title, cBorder)
			for i, it := range items {
				fg, bg := cText, cPanel
				if i == sel {
					fg, bg = cAccent, cSelBG
					a.cv.fill(bx+1, y+2+i, bw-2, 1, ' ', fg, bg)
				}
				a.cv.textClip(bx+3, y+2+i, bw-5, it.label, fg, bg)
			}
			if sel < len(items) && items[sel].desc != "" {
				for i, l := range wrap(items[sel].desc, min(70, w-4)) {
					if y+bh+1+i < h-2 {
						a.cv.text((w-runeLen(l))/2, y+bh+1+i, l, cDim, cBlack)
					}
				}
			}
			if footer != nil {
				footer(h - 2)
			}
		})
		if !ok {
			return ""
		}
		switch ev.Key() {
		case tcell.KeyUp:
			sel = (sel + len(items) - 1) % len(items)
		case tcell.KeyDown, tcell.KeyTab:
			sel = (sel + 1) % len(items)
		case tcell.KeyEnter:
			return items[sel].key
		case tcell.KeyEscape:
			return ""
		case tcell.KeyRune:
			switch normKey(ev.Rune()) {
			case 'w':
				sel = (sel + len(items) - 1) % len(items)
			case 's':
				sel = (sel + 1) % len(items)
			}
		}
	}
}

func (a *App) message(title, text string) {
	a.nextKey(func() {
		y := a.drawBackdrop()
		w, _ := a.size()
		lines := wrap(text, min(66, w-6))
		bw := min(70, w-2)
		a.cv.box((w-bw)/2, y, bw, len(lines)+4, title, cBorder)
		for i, l := range lines {
			a.cv.text((w-bw)/2+2, y+2+i, l, cText, cPanel)
		}
		a.cv.text((w-bw)/2+2, y+len(lines)+3, " Нажмите любую клавишу ", cDim, cPanel)
	})
}

func (a *App) aiStatus() string {
	switch {
	case !a.cfg.AIEnabled:
		return "ИИ: выключен (простой ИИ)"
	case !llm.HasCredentials(a.cfg.APIKey):
		return "ИИ: нет API ключа — простой ИИ (Настройки)"
	default:
		return "ИИ: " + a.cfg.Model
	}
}

func (a *App) mainMenu() {
	items := []menuItem{
		{"new", "Новая игра", "Создать новый случайный мир и героя."},
		{"load", "Загрузить", "Продолжить сохранённый мир."},
		{"join", "Присоединиться к миру", "Подключиться к миру другого игрока по сети."},
		{"settings", "Настройки", "Имя, сеть, ключ API Anthropic для ИИ-персонажей."},
		{"quit", "Выход", ""},
	}
	sel := 0
	for !a.closing {
		choice := a.menu("Главное меню", items, sel, func(y int) {
			w, _ := a.size()
			info := a.aiStatus()
			if len(a.mods) > 0 {
				info += fmt.Sprintf(" • модов: %d", len(a.mods))
			}
			a.cv.text((w-runeLen(info))/2, y, info, cDim, cBlack)
			if a.status != "" {
				a.cv.text((w-runeLen(a.status))/2, y-1, a.status, cBad, cBlack)
			}
		})
		for i, it := range items {
			if it.key == choice {
				sel = i
			}
		}
		a.status = ""
		switch choice {
		case "new":
			a.newGameForm()
		case "load":
			a.loadMenu()
		case "join":
			a.joinForm()
		case "settings":
			a.settingsForm()
		case "quit", "":
			if a.closing {
				return
			}
			if choice == "" && a.menu("Выйти из игры?", []menuItem{{"no", "Нет", ""}, {"yes", "Да", ""}}, 0, nil) != "yes" {
				continue
			}
			return
		}
	}
}

// ---- forms ----

type field struct {
	label   string
	input   *textInput
	choices []string
	descs   []string
	choice  int
	button  bool
	hint    string
}

// form edits fields; returns false on Esc.
func (a *App) form(title string, fields []*field) bool {
	sel := 0
	for {
		ev, ok := a.nextKey(func() {
			y := a.drawBackdrop()
			w, h := a.size()
			bw := min(72, w-2)
			bx := (w - bw) / 2
			bh := len(fields)*2 + 3
			a.cv.box(bx, y, bw, bh, title, cBorder)
			for i, f := range fields {
				fy := y + 2 + i*2
				lf := cText
				if i == sel {
					lf = cAccent
				}
				if f.button {
					label := "[ " + f.label + " ]"
					bg := cPanel
					if i == sel {
						bg = cSelBG
					}
					a.cv.text(bx+(bw-runeLen(label))/2, fy, label, lf, bg)
					continue
				}
				a.cv.text(bx+2, fy, f.label, lf, cPanel)
				vx := bx + 24
				vw := bw - 26
				if f.input != nil {
					f.input.draw(a.cv, vx, fy, vw, i == sel)
				} else if len(f.choices) > 0 {
					s := "◀ " + f.choices[f.choice] + " ▶"
					if asciiUI {
						s = "< " + f.choices[f.choice] + " >"
					}
					bg := col("#1a1a2a")
					if i == sel {
						bg = col("#24304a")
					}
					a.cv.fill(vx, fy, vw, 1, ' ', cText, bg)
					a.cv.textClip(vx, fy, vw, s, cText, bg)
				}
			}
			f := fields[sel]
			desc := f.hint
			if len(f.descs) > f.choice && f.descs[f.choice] != "" {
				desc = f.descs[f.choice]
			}
			for i, l := range wrap(desc, min(70, w-4)) {
				if y+bh+1+i < h-1 {
					a.cv.text((w-runeLen(l))/2, y+bh+1+i, l, cDim, cBlack)
				}
			}
			help := "↑↓ поле • ←→ выбор • Enter далее • Esc назад"
			a.cv.text((w-runeLen(help))/2, h-1, help, cDim, cBlack)
		})
		if !ok {
			return false
		}
		f := fields[sel]
		switch ev.Key() {
		case tcell.KeyEscape:
			return false
		case tcell.KeyUp, tcell.KeyBacktab:
			sel = (sel + len(fields) - 1) % len(fields)
			continue
		case tcell.KeyDown, tcell.KeyTab:
			sel = (sel + 1) % len(fields)
			continue
		case tcell.KeyEnter:
			if f.button {
				return true
			}
			sel = min(len(fields)-1, sel+1)
			continue
		case tcell.KeyLeft:
			if len(f.choices) > 0 {
				f.choice = (f.choice + len(f.choices) - 1) % len(f.choices)
			}
			continue
		case tcell.KeyRight:
			if len(f.choices) > 0 {
				f.choice = (f.choice + 1) % len(f.choices)
			}
			continue
		}
		if f.input != nil {
			f.input.handle(ev)
		} else if len(f.choices) > 0 && ev.Key() == tcell.KeyRune && ev.Rune() == ' ' {
			f.choice = (f.choice + 1) % len(f.choices)
		}
	}
}

// startingClasses are the classes a new hero can pick (secret ones are earned).
func startingClasses() []content.ClassDef {
	var out []content.ClassDef
	for _, c := range content.Classes() {
		if !c.Secret {
			out = append(out, c)
		}
	}
	return out
}

func classField() *field {
	f := &field{label: "Класс"}
	for _, c := range startingClasses() {
		f.choices = append(f.choices, c.Name)
		f.descs = append(f.descs, c.Desc+" Старт: "+attrLine(c.Attrs))
	}
	return f
}

func attrLine(m map[string]float64) string {
	return fmt.Sprintf("СИЛ %.0f ЛОВ %.0f ИНТ %.0f ВЫН %.0f", m["str"], m["dex"], m["int"], m["vit"])
}

func (a *App) newGameForm() {
	name := &field{label: "Имя героя", input: &textInput{maxLen: 16}, hint: "Имя персонажа. По нему же вас узнает мир при повторном входе."}
	name.input.Set(a.cfg.Name)
	class := classField()
	seed := &field{label: "Зерно мира", input: &textInput{maxLen: 12}, hint: "Одинаковое зерно — одинаковый мир. Оставьте как есть для случайного."}
	seed.input.Set(strconv.Itoa(rand.IntN(1_000_000)))
	mode := &field{label: "Режим", choices: []string{"Одиночная игра", fmt.Sprintf("Открыть для сети (порт %d)", a.cfg.Port)},
		descs: []string{"Мир только для вас. Esc ставит игру на паузу.", "Друзья смогут подключиться через «Присоединиться к миру» по вашему IP."}}
	pvp := &field{label: "Бой между игроками", choices: []string{"Включён", "Выключен"},
		descs: []string{"Вне деревень игроки могут сражаться друг с другом; группа (G) защищает своих.", "Игроки никогда не ранят друг друга."}}
	start := &field{label: "Начать приключение", button: true}
	if !a.form("Новая игра", []*field{name, class, seed, mode, pvp, start}) {
		return
	}
	n := strings.TrimSpace(name.input.String())
	if n == "" {
		n = "Странник"
	}
	a.cfg.Name = n
	a.cfg.Save()
	s, err := strconv.ParseInt(strings.TrimSpace(seed.input.String()), 10, 64)
	if err != nil {
		s = int64(hashString(seed.input.String()))
	}
	a.newGame(s, n, startingClasses()[class.choice].Key, mode.choice == 1, pvp.choice == 0)
}

func hashString(s string) uint32 {
	var h uint32 = 2166136261
	for _, b := range []byte(s) {
		h ^= uint32(b)
		h *= 16777619
	}
	return h
}

func (a *App) brain() *llm.Brain {
	if !a.cfg.AIEnabled {
		return nil
	}
	return llm.New(a.cfg.APIKey, a.cfg.Model)
}

func (a *App) loading(text string) {
	y := a.drawBackdrop()
	w, _ := a.size()
	a.cv.text((w-runeLen(text))/2, y+2, text, cAccent, cBlack)
	a.scr.Show()
}

func (a *App) newGame(seed int64, name, class string, host, pvp bool) {
	a.loading(fmt.Sprintf("Генерация мира (зерно %d)...", seed))
	g := game.New(seed, a.brain())
	g.PvP = pvp
	slot := fmt.Sprintf("%s-%d", slugify(g.WorldName), seed)
	a.runLocal(g, slot, host, name, class)
}

var slugRe = regexp.MustCompile(`[^\p{L}\p{N}]+`)

func slugify(s string) string {
	return strings.Trim(slugRe.ReplaceAllString(strings.ToLower(s), "_"), "_")
}

// LocalServerHook, if set, is told about every local server the app starts
// (used by scripted visual tests to control the world).
var LocalServerHook func(*server.Server)

func (a *App) runLocal(g *game.Game, slot string, host bool, name, class string) {
	srv := server.New(g)
	srv.SavePath = filepath.Join(config.SavesDir(), slot+".sav")
	go srv.Run()
	if LocalServerHook != nil {
		LocalServerHook(srv)
	}
	if host {
		if err := srv.Listen(fmt.Sprintf(":%d", a.cfg.Port)); err != nil {
			a.status = "Не удалось открыть порт: " + err.Error()
		}
	}
	p := newPlay(a, srv.ConnectLocal(), srv, name, class)
	p.slotPath = srv.SavePath
	p.run()
	srv.Call(func(g *game.Game) {
		if err := g.Save(srv.SavePath); err != nil {
			a.status = "Ошибка сохранения: " + err.Error()
		}
	})
	srv.Stop()
}

func (a *App) loadMenu() {
	saves := game.ListSaves(config.SavesDir())
	if len(saves) == 0 {
		a.message("Загрузка", "Сохранений пока нет. Начните новую игру — мир сохраняется автоматически каждые 3 минуты, при выходе и по F5.")
		return
	}
	var items []menuItem
	for _, s := range saves {
		items = append(items, menuItem{
			key:   s.Path,
			label: fmt.Sprintf("%-22s %s", i18n.T(s.WorldName)+" #"+strconv.FormatInt(s.Seed, 10), s.SavedAt.Format("02.01 15:04")),
			desc:  "Герои: " + strings.Join(s.Characters, ", "),
		})
	}
	choice := a.menu("Загрузить мир", items, 0, nil)
	if choice == "" {
		return
	}
	var info game.SaveInfo
	for _, s := range saves {
		if s.Path == choice {
			info = s
		}
	}
	var who []menuItem
	for i, n := range info.Names {
		who = append(who, menuItem{key: n, label: info.Characters[i]})
	}
	who = append(who, menuItem{key: "\x00new", label: "Новый герой..."})
	name := a.menu("Играть за", who, 0, nil)
	if name == "" {
		return
	}
	if name == "\x00new" {
		f := &field{label: "Имя героя", input: &textInput{maxLen: 16}}
		if !a.form("Новый герой", []*field{f, {label: "Далее", button: true}}) {
			return
		}
		name = strings.TrimSpace(f.input.String())
		if name == "" {
			return
		}
	}
	mode := a.menu("Режим", []menuItem{{"solo", "Одиночная игра", ""}, {"host", fmt.Sprintf("Открыть для сети (порт %d)", a.cfg.Port), ""}}, 0, nil)
	if mode == "" {
		return
	}
	a.loadSlot(info.Path, info.Slot, mode == "host", name)
}

func (a *App) loadSlot(path, slot string, host bool, name string) {
	a.loading("Загрузка мира...")
	g, err := game.Load(path, a.brain())
	if err != nil {
		a.message("Ошибка загрузки", err.Error())
		return
	}
	a.runLocal(g, slot, host, name, "")
}

func (a *App) joinForm() {
	addr := &field{label: "Адрес сервера", input: &textInput{maxLen: 64}, hint: "IP или имя хоста и порт, например 192.168.1.20:7777"}
	addr.input.Set(a.cfg.LastServer)
	name := &field{label: "Имя героя", input: &textInput{maxLen: 16}, hint: "С этим именем ваш герой сохранится в чужом мире."}
	name.input.Set(a.cfg.Name)
	if !a.form("Присоединиться", []*field{addr, name, {label: "Подключиться", button: true}}) {
		return
	}
	a.cfg.LastServer = strings.TrimSpace(addr.input.String())
	a.cfg.Name = strings.TrimSpace(name.input.String())
	a.cfg.Save()
	a.joinGame(a.cfg.LastServer, a.cfg.Name)
}

func (a *App) joinGame(addr, name string) {
	if !strings.Contains(addr, ":") {
		addr = fmt.Sprintf("%s:%d", addr, a.cfg.Port)
	}
	a.loading("Подключение к " + addr + "...")
	c, err := net.DialTimeout("tcp", addr, 8*time.Second)
	if err != nil {
		a.message("Нет соединения", err.Error())
		return
	}
	if tc, ok := c.(*net.TCPConn); ok {
		tc.SetNoDelay(true)
	}
	builtin := content.Get()
	p := newPlay(a, proto.NewConn(c), nil, name, "")
	p.run()
	content.Use(builtin) // the server may have sent different content
}

func (a *App) settingsForm() {
	lang := &field{label: "Язык", hint: "Язык интерфейса, предметов, заданий и разговоров. ИИ-персонажи отвечают на нём же."}
	for i, l := range i18n.Langs {
		lang.choices = append(lang.choices, l.Name)
		if l.Code == i18n.Lang() {
			lang.choice = i
		}
	}
	name := &field{label: "Имя по умолчанию", input: &textInput{maxLen: 16}}
	name.input.Set(a.cfg.Name)
	port := &field{label: "Порт сервера", input: &textInput{maxLen: 5}, hint: "TCP-порт для режима «Открыть для сети»."}
	port.input.Set(strconv.Itoa(a.cfg.Port))
	ai := &field{label: "ИИ-персонажи", choices: []string{"Включены", "Выключены"},
		descs: []string{"NPC отвечают и действуют через Claude, элитные враги выбирают тактику. Нужен ключ API.", "Только встроенный простой ИИ."}}
	if !a.cfg.AIEnabled {
		ai.choice = 1
	}
	key := &field{label: "API ключ Anthropic", input: &textInput{maxLen: 200, mask: true},
		hint: "Ключ хранится в " + filepath.Join(config.Home(), "config.json") + " (права 600). Пусто — берётся из переменной ANTHROPIC_API_KEY."}
	key.input.Set(a.cfg.APIKey)
	model := &field{label: "Модель Claude", input: &textInput{maxLen: 40}, hint: "По умолчанию claude-opus-5-5. Для более быстрых и дешёвых ответов можно указать claude-haiku-4-5 или claude-sonnet-5-5."}
	model.input.Set(a.cfg.Model)
	ascii := &field{label: "Рамки интерфейса", choices: []string{"Псевдографика", "Только ASCII"},
		descs: []string{"Красивые рамки ┌─┐ и полосы █░.", "Для терминалов без Unicode: рамки +-| и полосы #."}}
	if a.cfg.ASCIIOnly {
		ascii.choice = 1
	}
	if !a.form("Настройки", []*field{lang, name, port, ai, key, model, ascii, {label: "Сохранить", button: true}}) {
		return
	}
	a.cfg.Language = i18n.Langs[lang.choice].Code
	i18n.SetLang(a.cfg.Language)
	a.cfg.Name = strings.TrimSpace(name.input.String())
	if p, err := strconv.Atoi(strings.TrimSpace(port.input.String())); err == nil && p > 0 && p < 65536 {
		a.cfg.Port = p
	}
	a.cfg.AIEnabled = ai.choice == 0
	a.cfg.APIKey = strings.TrimSpace(key.input.String())
	a.cfg.Model = strings.TrimSpace(model.input.String())
	if a.cfg.Model == "" {
		a.cfg.Model = llm.DefaultModel
	}
	a.cfg.ASCIIOnly = ascii.choice == 1
	asciiUI = a.cfg.ASCIIOnly
	if err := a.cfg.Save(); err != nil {
		a.status = "Не удалось сохранить настройки: " + err.Error()
	}
}
