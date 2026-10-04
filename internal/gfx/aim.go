package gfx

import (
	"image"
	"image/color"
	"math"
	"time"

	"github.com/gdamore/tcell/v2"
	"github.com/hajimehoshi/ebiten/v2"
	"github.com/hajimehoshi/ebiten/v2/inpututil"
	"github.com/hajimehoshi/ebiten/v2/vector"

	"ratas/internal/world"
)

// While a mouse button is held the click repeats, so holding the left button
// keeps attacking (the server paces the attacks).
const mouseRepeat = 150 * time.Millisecond

// tileAt maps a screen point to the world tile drawn there.
func (r *worldRenderer) tileAt(x, y int) (world.Pos, bool) {
	v := r.lastView
	if v == nil || v.ts <= 0 {
		return world.Pos{}, false
	}
	tx := int(math.Floor((float64(x) - v.ox) / float64(v.ts)))
	ty := int(math.Floor((float64(y) - v.oy) / float64(v.ts)))
	if !v.l.In(tx, ty) {
		return world.Pos{}, false
	}
	return world.Pos{X: tx, Y: ty}, true
}

// overUI: the text interface covers this point (a panel, the HUD, a window).
func (g *Game) overUI(x, y int) bool {
	if g.cellW <= 0 || g.cellH <= 0 || g.gw == 0 {
		return false
	}
	cx, cy := int(float64(x)/g.cellW), int(float64(y)/g.cellH)
	if cx < 0 || cy < 0 || cx >= g.gw || cy >= g.gh || cy*g.gw+cx >= len(g.grid) {
		return true
	}
	return !g.grid[cy*g.gw+cx].clear
}

// updateMouse publishes the tile under the cursor and turns clicks on the map
// into mouse events of the client: left button attacks, right button uses
// the first ability, both aimed at that tile.
func (g *Game) updateMouse() {
	x, y := ebiten.CursorPosition()
	var aim world.Pos
	ok := false
	if g.scene != nil && !g.scene.MapOpen && image.Pt(x, y).In(g.mapRect(g.scene)) && !g.overUI(x, y) {
		aim, ok = g.world.tileAt(x, y)
	}
	g.st.mu.Lock()
	g.st.aim, g.st.aimOK = aim, ok
	g.st.mu.Unlock()
	g.aimOK, g.aimTile = ok, aim
	if !ok {
		return
	}
	now := time.Now()
	if g.mouseAt == nil {
		g.mouseAt = map[ebiten.MouseButton]time.Time{}
	}
	for _, b := range []struct {
		eb ebiten.MouseButton
		tb tcell.ButtonMask
	}{{ebiten.MouseButtonLeft, tcell.Button1}, {ebiten.MouseButtonRight, tcell.Button2}} {
		just := inpututil.IsMouseButtonJustPressed(b.eb)
		if just || (ebiten.IsMouseButtonPressed(b.eb) && now.Sub(g.mouseAt[b.eb]) > mouseRepeat) {
			g.mouseAt[b.eb] = now
			g.scr.InjectMouse(int(float64(x)/g.cellW), int(float64(y)/g.cellH), b.tb, tcell.ModNone)
		}
	}
}

// drawAim marks the tile under the cursor.
func (g *Game) drawAim(dst *ebiten.Image) {
	v := g.world.lastView
	if !g.aimOK || v == nil {
		return
	}
	ts := float32(v.ts)
	px, py := v.px(float64(g.aimTile.X), float64(g.aimTile.Y))
	col := color.RGBA{255, 230, 140, 200}
	for _, e := range g.scene.Entities {
		if e.X == g.aimTile.X && e.Y == g.aimTile.Y && e.Hostile && !e.Dead {
			col = color.RGBA{255, 90, 70, 230}
			break
		}
	}
	pulse := float32(0.75 + 0.25*math.Sin(g.world.t*6))
	col.A = uint8(float32(col.A) * pulse)
	w := float32(math.Max(1.5, 2*g.scale))
	c := ts / 4 // corner length
	x0, y0, x1, y1 := float32(px)+w/2, float32(py)+w/2, float32(px)+ts-w/2, float32(py)+ts-w/2
	for _, s := range [][4]float32{
		{x0, y0, x0 + c, y0}, {x0, y0, x0, y0 + c},
		{x1, y0, x1 - c, y0}, {x1, y0, x1, y0 + c},
		{x0, y1, x0 + c, y1}, {x0, y1, x0, y1 - c},
		{x1, y1, x1 - c, y1}, {x1, y1, x1, y1 - c},
	} {
		vector.StrokeLine(dst, s[0], s[1], s[2], s[3], w, col, true)
	}
}
