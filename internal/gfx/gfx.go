// Package gfx is the windowed front-end with enhanced graphics.
//
// The whole text interface of the client (menus, HUD, dialogues) runs
// unchanged on an in-memory terminal; this package renders that terminal with
// a real font and translucent panels, and draws the world underneath with
// procedural pixel-art tiles, depth-sorted sprites, smooth movement, dynamic
// lighting, particles and speech bubbles.
package gfx

import (
	"bytes"
	"image"
	"image/color"
	"math"
	"sync"
	"time"

	"github.com/gdamore/tcell/v2"
	"github.com/hajimehoshi/ebiten/v2"
	"github.com/hajimehoshi/ebiten/v2/inpututil"
	"github.com/hajimehoshi/ebiten/v2/text/v2"
	"golang.org/x/image/font/gofont/gobold"
	"golang.org/x/image/font/gofont/gomono"
	"golang.org/x/image/font/gofont/gomonobold"
	"golang.org/x/image/font/gofont/goregular"

	"ratas/internal/client"
	"ratas/internal/config"
	"ratas/internal/proto"
)

// cell is one captured character of the text interface.
type cell struct {
	r      rune
	fg, bg color.RGBA
	clear  bool // background shows the world
}

// shared is the state exchanged between the client goroutine and the window.
type shared struct {
	mu    sync.Mutex
	scene *client.Scene
	fx    []proto.FX
	grid  []cell
	gw    int
	gh    int
}

func (s *shared) Publish(sc *client.Scene) {
	s.mu.Lock()
	s.scene = sc
	s.mu.Unlock()
}

func (s *shared) Effects(fx []proto.FX) {
	s.mu.Lock()
	s.fx = append(s.fx, fx...)
	s.mu.Unlock()
}

// gridScreen is the client's terminal: a tcell simulation screen whose frames
// are captured on every Show.
type gridScreen struct {
	tcell.SimulationScreen
	st   *shared
	once sync.Once
	err  error
}

func (g *gridScreen) Init() error {
	g.once.Do(func() { g.err = g.SimulationScreen.Init() })
	return g.err
}

func (g *gridScreen) Show() {
	g.SimulationScreen.Show()
	g.capture()
}

func (g *gridScreen) Sync() {
	g.SimulationScreen.Sync()
	g.capture()
}

func toRGBA(c tcell.Color, def color.RGBA) (color.RGBA, bool) {
	if c == tcell.ColorDefault {
		return def, true
	}
	r, gg, b := c.TrueColor().RGB()
	if r < 0 {
		return def, true
	}
	return color.RGBA{uint8(r), uint8(gg), uint8(b), 255}, false
}

func (g *gridScreen) capture() {
	cells, w, h := g.SimulationScreen.GetContents()
	buf := make([]cell, w*h)
	white := color.RGBA{216, 216, 224, 255}
	for i := range buf {
		if i >= len(cells) {
			break
		}
		c := cells[i]
		r := ' '
		if len(c.Runes) > 0 {
			r = c.Runes[0]
		}
		fg, bg, _ := c.Style.Decompose()
		f, _ := toRGBA(fg, white)
		b, def := toRGBA(bg, color.RGBA{})
		buf[i] = cell{r: r, fg: f, bg: b, clear: def || (b.R == 0 && b.G == 0 && b.B == 0)}
	}
	g.st.mu.Lock()
	g.st.grid, g.st.gw, g.st.gh = buf, w, h
	g.st.mu.Unlock()
}

type fonts struct {
	mono, monoBold, regular, bold *text.GoTextFaceSource
}

func loadFonts() (*fonts, error) {
	f := &fonts{}
	for _, x := range []struct {
		dst  **text.GoTextFaceSource
		data []byte
	}{{&f.mono, gomono.TTF}, {&f.monoBold, gomonobold.TTF}, {&f.regular, goregular.TTF}, {&f.bold, gobold.TTF}} {
		s, err := text.NewGoTextFaceSource(bytes.NewReader(x.data))
		if err != nil {
			return nil, err
		}
		*x.dst = s
	}
	return f, nil
}

// Game implements ebiten.Game.
type Game struct {
	st    *shared
	scr   *gridScreen
	done  chan error
	err   error
	fonts *fonts

	scale        float64
	w, h         int // physical pixels
	gridFace     *text.GoTextFace
	cellW, cellH float64
	cols, rows   int

	input   *inputState
	world   *worldRenderer
	menu    *menuBackdrop
	last    time.Time
	closing time.Time

	scene  *client.Scene
	grid   []cell
	gw, gh int

	shotReq         chan string // screenshot requests from other goroutines
	shotPath        string      // save the next frame here
	shotDone        chan error  // notified after a requested screenshot
	saveShot        func(img image.Image, path string) error
	shotFull        bool // keep full resolution (tests crop details)
	screenshotSaved time.Time
}

const baseFontSize = 15.0

// Run opens the game window. It must be called from the main goroutine.
func Run(cfg *config.Config, mods []string, opts client.StartOptions) error {
	f, err := loadFonts()
	if err != nil {
		return err
	}
	st := &shared{}
	scr := &gridScreen{SimulationScreen: tcell.NewSimulationScreen("UTF-8"), st: st}
	if err := scr.Init(); err != nil {
		return err
	}
	g := &Game{st: st, scr: scr, done: make(chan error, 1), fonts: f, input: newInput(), last: time.Now()}
	g.world = newWorldRenderer(f)
	g.menu = newMenuBackdrop(g.world)

	ebiten.SetWindowTitle("Ратас")
	ebiten.SetWindowResizingMode(ebiten.WindowResizingModeEnabled)
	ebiten.SetWindowSizeLimits(800, 500, -1, -1)
	ww, wh := 1440, 900
	mw, mh := 0, 0
	if m := ebiten.Monitor(); m != nil {
		mw, mh = m.Size()
	}
	if mw > 0 && mh > 0 {
		ww, wh = min(ww, mw*9/10), min(wh, mh*9/10)
	}
	ebiten.SetWindowSize(ww, wh)
	ebiten.SetWindowClosingHandled(true)
	ebiten.SetTPS(60)
	ebiten.SetWindowIcon([]image.Image{appIcon(64), appIcon(32), appIcon(16)})

	go func() {
		g.done <- client.RunWith(scr, st, cfg, mods, opts)
	}()
	if err := ebiten.RunGame(g); err != nil && err != ebiten.Termination {
		return err
	}
	return g.err
}

func (g *Game) Update() error {
	select {
	case err := <-g.done:
		g.err = err
		return ebiten.Termination
	default:
	}
	if ebiten.IsWindowBeingClosed() && g.closing.IsZero() {
		g.closing = time.Now()
		client.RequestClose(g.scr)
	}
	if !g.closing.IsZero() && time.Since(g.closing) > 8*time.Second {
		return ebiten.Termination
	}
	now := time.Now()
	dt := math.Min(0.1, now.Sub(g.last).Seconds())
	g.last = now

	select {
	case p := <-g.shotReq:
		g.shotPath = p
	default:
	}
	if inpututil.IsKeyJustPressed(ebiten.KeyF12) && g.shotPath == "" {
		g.shotPath = screenshotPath()
	}
	if inpututil.IsKeyJustPressed(ebiten.KeyF11) ||
		(inpututil.IsKeyJustPressed(ebiten.KeyEnter) && ebiten.IsKeyPressed(ebiten.KeyAlt)) {
		ebiten.SetFullscreen(!ebiten.IsFullscreen())
	} else {
		g.input.update(g.scr)
	}
	if _, wy := ebiten.Wheel(); wy != 0 {
		g.world.zoom(wy)
	}
	if ebiten.IsKeyPressed(ebiten.KeyControl) || ebiten.IsKeyPressed(ebiten.KeyMeta) {
		if inpututil.IsKeyJustPressed(ebiten.KeyEqual) {
			g.world.zoom(1)
		}
		if inpututil.IsKeyJustPressed(ebiten.KeyMinus) {
			g.world.zoom(-1)
		}
	}

	g.st.mu.Lock()
	g.scene = g.st.scene
	fx := g.st.fx
	g.st.fx = nil
	g.grid, g.gw, g.gh = g.st.grid, g.st.gw, g.st.gh
	g.st.mu.Unlock()

	if g.scene != nil {
		g.world.update(dt, g.scene, fx)
	} else {
		g.menu.update(dt)
	}
	return nil
}

func (g *Game) Draw(screen *ebiten.Image) {
	screen.Fill(color.RGBA{6, 6, 12, 255})
	switch {
	case g.scene != nil && g.scene.MapOpen:
		// the world map covers everything but the bottom line with the keys
		g.world.drawWorldMap(screen, g.scene, image.Rect(0, 0, g.w, g.h-int(g.cellH)))
	case g.scene != nil:
		g.world.draw(screen, g.scene, g.mapRect(g.scene), g.scale)
	default:
		g.menu.draw(screen, g.scale)
	}
	g.drawGrid(screen)
	if g.shotPath != "" {
		sc := g.scale
		if g.shotFull {
			sc = 1
		}
		img, path := captureImage(screen, sc), g.shotPath
		g.shotPath = ""
		save := g.saveShot
		if save == nil {
			save = writePNG
		}
		if g.shotDone != nil {
			go func() { g.shotDone <- save(img, path) }()
		} else {
			g.screenshotSaved = time.Now()
			go save(img, path)
		}
	}
	if time.Since(g.screenshotSaved) < 1500*time.Millisecond {
		drawCentered(screen, "Скриншот сохранён в ~/.ratas/screenshots", g.world.face(g.fonts.bold, "shot", 16*g.scale),
			float64(g.w)/2, float64(g.h)-60*g.scale, color.RGBA{160, 255, 160, 255}, true)
	}
}

// mapRect is the area of the text map in physical pixels.
func (g *Game) mapRect(s *client.Scene) image.Rectangle {
	x0 := int(float64(s.MapX) * g.cellW)
	y0 := int(float64(s.MapY) * g.cellH)
	return image.Rect(x0, y0, x0+int(float64(s.MapW)*g.cellW), y0+int(float64(s.MapH)*g.cellH))
}

func (g *Game) LayoutF(outW, outH float64) (float64, float64) {
	s := 1.0
	if m := ebiten.Monitor(); m != nil {
		s = m.DeviceScaleFactor()
	}
	if s <= 0 {
		s = 1
	}
	w, h := int(outW*s), int(outH*s)
	if s != g.scale || g.gridFace == nil {
		g.scale = s
		g.gridFace = &text.GoTextFace{Source: g.fonts.mono, Size: baseFontSize * s}
		g.cellW = text.Advance("M", g.gridFace)
		g.cellH = math.Ceil(baseFontSize * s * 1.32)
		g.world.setScale(s)
	}
	if w != g.w || h != g.h || g.cols == 0 {
		g.w, g.h = w, h
		cols := max(40, int(float64(w)/g.cellW))
		rows := max(16, int(float64(h)/g.cellH))
		if cols != g.cols || rows != g.rows {
			g.cols, g.rows = cols, rows
			g.scr.SetSize(cols, rows)
			g.scr.PostEvent(tcell.NewEventResize(cols, rows))
		}
	}
	return float64(w), float64(h)
}

func (g *Game) Layout(int, int) (int, int) { panic("LayoutF is used") }
