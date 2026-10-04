package gfx

import (
	"math"
	"strconv"
	"sync"
	"testing"
	"time"

	"github.com/gdamore/tcell/v2"

	"ratas/internal/client"
	"ratas/internal/config"
	"ratas/internal/content"
	"ratas/internal/game"
	"ratas/internal/proto"
	"ratas/internal/server"
	"ratas/internal/world"
)

type sceneSink struct {
	mu sync.Mutex
	sc *client.Scene
	fx []proto.FX
}

func (s *sceneSink) Publish(sc *client.Scene) { s.mu.Lock(); s.sc = sc; s.mu.Unlock() }
func (s *sceneSink) Effects(fx []proto.FX)    { s.mu.Lock(); s.fx = append(s.fx, fx...); s.mu.Unlock() }

// walkWorld runs a real server and client with a hero in a clearing of
// w×h cells cut for the test (the hero at its bottom-left corner, nobody
// else around).
type walkWorld struct {
	t    *testing.T
	srv  *server.Server
	scr  tcell.SimulationScreen
	sink *sceneSink
	r    *worldRenderer
	done chan error
}

func startWalk(t *testing.T, w, h int) *walkWorld {
	t.Helper()
	t.Setenv("RATAS_HOME", t.TempDir())
	db, _, err := content.LoadDefault("")
	if err != nil {
		t.Fatal(err)
	}
	content.Use(db)
	f, err := loadFonts()
	if err != nil {
		t.Fatal(err)
	}
	ww := &walkWorld{t: t, r: newWorldRenderer(f), sink: &sceneSink{}, done: make(chan error, 1)}
	srvCh := make(chan *server.Server, 1)
	client.LocalServerHook = func(s *server.Server) { srvCh <- s }
	t.Cleanup(func() { client.LocalServerHook = nil })
	ww.scr = tcell.NewSimulationScreen("UTF-8")
	cfg := config.Default()
	cfg.AIEnabled = false
	go func() {
		ww.done <- client.RunWith(ww.scr, ww.sink, cfg, nil, client.StartOptions{New: true, Seed: 5, Name: "Ходок", Class: "rogue"})
	}()
	ww.srv = <-srvCh
	time.Sleep(500 * time.Millisecond)
	ww.scr.SetSize(120, 40)
	ww.srv.Call(func(g *game.Game) {
		p := g.Online["Ходок"]
		l := g.Levels["overworld"]
		// the first open spot far from the edges; the clearing is cut there
		for y := 10 + h; y < l.H-10; y++ {
			for x := 10; x < l.W-10-w; x++ {
				if def := l.Def(x, y); !l.Free(x, y) || def.MoveCost > 1 || def.Damage > 0 {
					continue
				}
				for _, e := range g.Entities {
					if e.Kind != game.KPlayer && e.Level == "overworld" && e.Pos.Dist(world.Pos{X: x, Y: y}) < 40+w {
						g.Remove(e)
					}
				}
				ground := l.At(x, y)
				for dy := 0; dy < h; dy++ {
					for dx := 0; dx < w; dx++ {
						l.Set(x+dx, y-dy, ground)
					}
				}
				g.PlaceForTest(p, "overworld", world.Pos{X: x, Y: y})
				return
			}
		}
		ww.t.Fatal("no open land")
	})
	return ww
}

func (ww *walkWorld) stop() {
	client.RequestClose(ww.scr)
	select {
	case <-ww.done:
	case <-time.After(5 * time.Second):
	}
}

// TestWalkPipeline walks the hero over open land through the real server,
// client and snapshots, animating at 60 frames a second: the hero must glide
// at an even pace without stopping between cells.
func TestWalkPipeline(t *testing.T) {
	if testing.Short() {
		t.Skip("real-time test")
	}
	ww := startWalk(t, 16, 1)
	r, sink, scr := ww.r, ww.sink, ww.scr
	var pace float64
	// frames at about 60 per second timed by the clock, as in the window
	prev := time.Now()
	var dts []float64
	frame := func() *client.Scene {
		sink.mu.Lock()
		sc, fx := sink.sc, sink.fx
		sink.fx = nil
		sink.mu.Unlock()
		dt := time.Since(prev).Seconds()
		prev = time.Now()
		dts = append(dts, dt)
		if sc != nil {
			r.update(dt, sc, fx)
		}
		return sc
	}
	deadline := time.Now().Add(700 * time.Millisecond)
	for time.Now().Before(deadline) {
		frame()
		time.Sleep(time.Second / 60)
	}
	var xs []float64
	start := time.Now()
	last := time.Now()
	for time.Since(start) < 2600*time.Millisecond {
		if time.Since(start) < 2*time.Second && time.Since(last) > 50*time.Millisecond {
			scr.InjectKey(tcell.KeyRight, 0, tcell.ModNone)
			last = time.Now()
		}
		if sc := frame(); sc != nil {
			if st := r.ents[sc.YouID]; st != nil {
				xs = append(xs, st.x)
				pace = 1 / st.stepS
			}
		}
		time.Sleep(time.Second / 60)
	}
	ww.stop()
	if len(xs) < 100 {
		t.Fatalf("only %d frames", len(xs))
	}
	moved := xs[len(xs)-1] - xs[0]
	// speeds in the middle of the walk, cells per second
	var speeds []float64
	off := len(dts) - len(xs)
	for i := 1; i < len(xs); i++ {
		speeds = append(speeds, (xs[i]-xs[i-1])/dts[off+i])
	}
	mid := speeds[len(speeds)/5 : len(speeds)*3/5]
	lo, hi, sum := math.Inf(1), 0.0, 0.0
	stalls := 0
	for _, v := range mid {
		lo, hi, sum = math.Min(lo, v), math.Max(hi, v), sum+v
		if v < 0.3*pace {
			stalls++
		}
	}
	t.Logf("moved %.1f cells; walking speed min %.2f avg %.2f max %.2f cells/s at a pace of %.2f; %d stalled frames of %d",
		moved, lo, sum/float64(len(mid)), hi, pace, stalls, len(mid))
	if testing.Verbose() {
		var b []byte
		for _, v := range mid {
			b = append(b, []byte(" "+strconvF(v))...)
		}
		t.Logf("speeds:%s", b)
	}
	if moved < 6 {
		t.Fatalf("the hero hardly moved (%.1f cells)", moved)
	}
	if stalls > 0 {
		t.Errorf("the hero stops between cells in %d frames", stalls)
	}
	if hi > 1.8*pace {
		t.Errorf("the hero darts at %.2f cells/s", hi)
	}
}

func strconvF(v float64) string { return strconv.FormatFloat(v, 'f', 1, 64) }
