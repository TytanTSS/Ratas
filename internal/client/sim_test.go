package client

import (
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/gdamore/tcell/v2"

	"ratas/internal/config"
	"ratas/internal/content"
	"ratas/internal/proto"
)

// capScreen captures a copy of every shown frame, so tests can read the
// screen from another goroutine without racing the renderer.
type capScreen struct {
	tcell.SimulationScreen
	mu   sync.Mutex
	grid [][]rune
}

func newCapScreen() *capScreen {
	return &capScreen{SimulationScreen: tcell.NewSimulationScreen("UTF-8")}
}

func (c *capScreen) Show() { c.SimulationScreen.Show(); c.capture() }
func (c *capScreen) Sync() { c.SimulationScreen.Sync(); c.capture() }

func (c *capScreen) capture() {
	cells, w, h := c.SimulationScreen.GetContents()
	grid := make([][]rune, h)
	for y := 0; y < h; y++ {
		grid[y] = make([]rune, w)
		for x := 0; x < w; x++ {
			grid[y][x] = ' '
			if i := y*w + x; i < len(cells) && len(cells[i].Runes) > 0 {
				grid[y][x] = cells[i].Runes[0]
			}
		}
	}
	c.mu.Lock()
	c.grid = grid
	c.mu.Unlock()
}

func (c *capScreen) frame() [][]rune {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.grid
}

type sim struct {
	t   *testing.T
	scr *capScreen
	n   int
}

func (s *sim) text() string {
	var b strings.Builder
	for _, row := range s.scr.frame() {
		b.WriteString(string(row))
		b.WriteByte('\n')
	}
	return b.String()
}

func (s *sim) waitFor(sub string, d time.Duration) string {
	deadline := time.Now().Add(d)
	for time.Now().Before(deadline) {
		if txt := s.text(); strings.Contains(txt, sub) {
			return txt
		}
		time.Sleep(50 * time.Millisecond)
	}
	s.dump("timeout")
	s.t.Fatalf("timed out waiting for %q", sub)
	return ""
}

func (s *sim) dump(name string) {
	dir := os.Getenv("RATAS_SIM_DUMP")
	if dir == "" {
		return
	}
	s.n++
	os.WriteFile(filepath.Join(dir, strings.ReplaceAll(name, " ", "_")+".txt"), []byte(s.text()), 0o644)
}

func (s *sim) key(k tcell.Key) {
	s.scr.InjectKey(k, 0, tcell.ModNone)
	time.Sleep(60 * time.Millisecond)
}
func (s *sim) rune(r rune) {
	s.scr.InjectKey(tcell.KeyRune, r, tcell.ModNone)
	time.Sleep(60 * time.Millisecond)
}

// TestUISmoke drives the whole client against a local server on a simulated terminal.
func TestUISmoke(t *testing.T) {
	t.Setenv("RATAS_HOME", t.TempDir())
	db, _, err := content.LoadDefault("")
	if err != nil {
		t.Fatal(err)
	}
	content.Use(db)
	scr := newCapScreen()
	s := &sim{t: t, scr: scr}
	cfg := config.Default()
	cfg.AIEnabled = false
	done := make(chan error, 1)
	go func() {
		done <- RunOn(scr, cfg, nil, StartOptions{New: true, Seed: 42, Name: "Тест", Class: "warrior"})
	}()
	time.Sleep(200 * time.Millisecond)
	scr.SetSize(120, 40)
	s.waitFor("Здоровье", 10*time.Second)
	time.Sleep(300 * time.Millisecond)
	s.dump("01 game")

	for i := 0; i < 6; i++ {
		s.key(tcell.KeyDown)
		time.Sleep(150 * time.Millisecond)
	}
	s.dump("02 moved")

	screens := []struct {
		key  rune
		want string
	}{
		{'i', "Рюкзак"}, {'k', "Очки навыков"}, {'c', "Свободные очки"}, {'j', "Журнал"}, {'m', "Карта"}, {'?', "Помощь"},
	}
	for _, sc := range screens {
		s.rune(sc.key)
		s.waitFor(sc.want, 3*time.Second)
		s.dump("03 screen " + string(sc.key))
		s.key(tcell.KeyEscape)
	}
	// skill tree: switch to the warrior branch
	s.rune('k')
	s.key(tcell.KeyRight)
	s.waitFor("Закалка", 3*time.Second)
	s.dump("04 skills warrior")
	s.key(tcell.KeyEscape)

	// Russian keyboard layout: 'ш' is 'i'
	s.rune('ш')
	s.waitFor("Рюкзак", 3*time.Second)
	s.key(tcell.KeyEscape)

	// pause menu and exit to main menu
	s.key(tcell.KeyEscape)
	s.waitFor("Продолжить", 3*time.Second)
	s.dump("05 pause")
	s.key(tcell.KeyUp) // wraps to the last item: exit to menu
	s.key(tcell.KeyEnter)
	s.waitFor("Главное меню", 10*time.Second)
	s.dump("06 main menu")
	saves, _ := filepath.Glob(filepath.Join(config.SavesDir(), "*.sav"))
	if len(saves) != 1 {
		t.Fatalf("expected an autosave on exit, got %v", saves)
	}
	s.key(tcell.KeyEscape)
	s.waitFor("Выйти из игры?", 3*time.Second)
	s.key(tcell.KeyDown)
	s.key(tcell.KeyEnter)
	select {
	case err := <-done:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("app did not exit")
	}
}

type fakeSink struct {
	mu     sync.Mutex
	scenes int
	last   *Scene
	fx     int
	nils   int
}

func (f *fakeSink) Publish(s *Scene) {
	f.mu.Lock()
	defer f.mu.Unlock()
	if s == nil {
		f.nils++
		return
	}
	f.scenes++
	f.last = s
}

func (f *fakeSink) Effects(fx []proto.FX) {
	f.mu.Lock()
	f.fx += len(fx)
	f.mu.Unlock()
}

// TestGraphicalSink runs the client in window mode against a fake renderer:
// the world must be published as scenes instead of drawn as text.
func TestGraphicalSink(t *testing.T) {
	t.Setenv("RATAS_HOME", t.TempDir())
	db, _, _ := content.LoadDefault("")
	content.Use(db)
	scr := newCapScreen()
	sink := &fakeSink{}
	s := &sim{t: t, scr: scr}
	cfg := config.Default()
	cfg.AIEnabled = false
	done := make(chan error, 1)
	go func() {
		done <- RunWith(scr, sink, cfg, nil, StartOptions{New: true, Seed: 9, Name: "Окно", Class: "ranger"})
	}()
	time.Sleep(200 * time.Millisecond)
	scr.SetSize(120, 40)
	s.waitFor("Здоровье", 10*time.Second)
	time.Sleep(500 * time.Millisecond)
	sink.mu.Lock()
	sc := sink.last
	n := sink.scenes
	sink.mu.Unlock()
	if n == 0 || sc == nil || sc.Level == nil || len(sc.Entities) == 0 || sc.MapW == 0 {
		t.Fatalf("no usable scene published: %d %+v", n, sc)
	}
	if sc.Level.ID != "overworld" || !sc.Explored.Get(sc.Self.Y*sc.Level.W+sc.Self.X) {
		t.Fatal("scene lacks level or explored data")
	}
	// the text map must not be drawn under the renderer: no '@' glyph in the map area
	frame := scr.frame()
	for y := sc.MapY; y < sc.MapY+sc.MapH && y < len(frame); y++ {
		for x := sc.MapX; x < sc.MapX+sc.MapW && x < len(frame[y]); x++ {
			if frame[y][x] == '@' {
				t.Fatalf("text map drawn at %d,%d in graphical mode", x, y)
			}
		}
	}
	RequestClose(scr)
	select {
	case <-done:
	case <-time.After(10 * time.Second):
		t.Fatal("window close request did not stop the app")
	}
	sink.mu.Lock()
	defer sink.mu.Unlock()
	if sink.nils == 0 {
		t.Fatal("scene was not cleared after leaving the world")
	}
	saves, _ := filepath.Glob(filepath.Join(config.SavesDir(), "*.sav"))
	if len(saves) != 1 {
		t.Fatalf("closing the window must save the world, got %v", saves)
	}
}
