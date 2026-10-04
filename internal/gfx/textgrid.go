package gfx

import (
	"image/color"
	"math"
	"strings"
	"time"

	"github.com/hajimehoshi/ebiten/v2"
	"github.com/hajimehoshi/ebiten/v2/text/v2"
	"github.com/hajimehoshi/ebiten/v2/vector"
)

// bgAlpha makes interface panels translucent so the world shows through.
func bgAlpha(c color.RGBA) float64 {
	switch c {
	case color.RGBA{0x10, 0x10, 0x1a, 255}: // panel
		return 0.84
	case color.RGBA{0x18, 0x18, 0x28, 255}: // top bar
		return 0.72
	case color.RGBA{0x0a, 0x0a, 0x12, 255}: // log
		return 0.58
	case color.RGBA{0x2a, 0x3a, 0x6a, 255}: // selection
		return 0.9
	}
	return 0.94
}

func scaleColor(c color.RGBA, a float64) color.RGBA {
	return color.RGBA{uint8(float64(c.R) * a), uint8(float64(c.G) * a), uint8(float64(c.B) * a), uint8(255 * a)}
}

func isSpecial(r rune) bool {
	switch r {
	case '─', '│', '┌', '┐', '└', '┘', '█', '░', '▏', '☀', '☾', '◀', '▶', '♛':
		return true
	}
	return false
}

func (g *Game) drawGrid(dst *ebiten.Image) {
	grid, gw, gh := g.grid, g.gw, g.gh
	if len(grid) < gw*gh || gw == 0 {
		return
	}
	cw, ch := g.cellW, g.cellH
	// backgrounds, merged into runs
	for y := 0; y < gh; y++ {
		row := grid[y*gw : (y+1)*gw]
		for x := 0; x < gw; {
			c := row[x]
			if c.clear {
				x++
				continue
			}
			x2 := x + 1
			for x2 < gw && !row[x2].clear && row[x2].bg == c.bg {
				x2++
			}
			vector.FillRect(dst, float32(float64(x)*cw), float32(float64(y)*ch), float32(float64(x2-x)*cw), float32(ch),
				scaleColor(c.bg, bgAlpha(c.bg)), false)
			x = x2
		}
	}
	// glyphs: runs of the same colour are drawn as one string
	m := g.gridFace.Metrics()
	dy := (ch - (m.HAscent + m.HDescent)) / 2
	var sb strings.Builder
	for y := 0; y < gh; y++ {
		row := grid[y*gw : (y+1)*gw]
		start := -1
		var runFG color.RGBA
		flush := func(end int) {
			if start < 0 {
				return
			}
			s := strings.TrimRight(sb.String(), " ")
			if s != "" {
				op := &text.DrawOptions{}
				op.GeoM.Translate(math.Round(float64(start)*cw), math.Round(float64(y)*ch+dy))
				op.ColorScale.ScaleWithColor(runFG)
				text.Draw(dst, s, g.gridFace, op)
			}
			sb.Reset()
			start = -1
		}
		for x := 0; x < gw; x++ {
			c := row[x]
			switch {
			case isSpecial(c.r):
				flush(x)
				g.drawSpecial(dst, c, float64(x)*cw, float64(y)*ch)
			case c.r == ' ' || c.r == 0:
				if start >= 0 {
					sb.WriteByte(' ')
				}
			default:
				if start >= 0 && c.fg != runFG {
					flush(x)
				}
				if start < 0 {
					start, runFG = x, c.fg
				}
				sb.WriteRune(c.r)
			}
		}
		flush(gw)
	}
}

func (g *Game) drawSpecial(dst *ebiten.Image, c cell, x, y float64) {
	cw, ch := g.cellW, g.cellH
	t := float32(math.Max(1, math.Round(g.scale)))
	cx, cy := float32(x+cw/2), float32(y+ch/2)
	fx, fy, fw, fh := float32(x), float32(y), float32(cw), float32(ch)
	switch c.r {
	case '─':
		vector.FillRect(dst, fx, cy-t/2, fw, t, c.fg, false)
	case '│':
		vector.FillRect(dst, cx-t/2, fy, t, fh, c.fg, false)
	case '┌', '┐', '└', '┘':
		var p vector.Path
		switch c.r {
		case '┌':
			p.MoveTo(fx+fw, cy)
			p.QuadTo(cx, cy, cx, fy+fh)
		case '┐':
			p.MoveTo(fx, cy)
			p.QuadTo(cx, cy, cx, fy+fh)
		case '└':
			p.MoveTo(cx, fy)
			p.QuadTo(cx, cy, fx+fw, cy)
		case '┘':
			p.MoveTo(cx, fy)
			p.QuadTo(cx, cy, fx, cy)
		}
		op := &vector.DrawPathOptions{AntiAlias: true}
		op.ColorScale.ScaleWithColor(c.fg)
		vector.StrokePath(dst, &p, &vector.StrokeOptions{Width: t}, op)
	case '█':
		vector.FillRect(dst, fx, fy+fh*0.3, fw+0.5, fh*0.4, c.fg, false)
		vector.FillRect(dst, fx, fy+fh*0.3, fw+0.5, fh*0.1, pre(white, 0.16), false)
	case '░':
		vector.FillRect(dst, fx, fy+fh*0.3, fw+0.5, fh*0.4, scaleColor(c.fg, 0.55), false)
	case '▏':
		if time.Now().UnixMilli()/500%2 == 0 {
			vector.FillRect(dst, fx+1, fy+fh*0.15, t*1.5, fh*0.7, c.fg, false)
		}
	case '☀':
		r := fw * 0.26
		vector.FillCircle(dst, cx, cy, r, c.fg, true)
		for i := 0; i < 8; i++ {
			a := float64(i) * math.Pi / 4
			ca, sa := float32(math.Cos(a)), float32(math.Sin(a))
			vector.StrokeLine(dst, cx+ca*r*1.4, cy+sa*r*1.4, cx+ca*r*2, cy+sa*r*2, t, c.fg, true)
		}
	case '☾':
		r := fw * 0.36
		vector.FillCircle(dst, cx, cy, r, c.fg, true)
		vector.FillCircle(dst, cx+r*0.55, cy-r*0.25, r*0.85, color.RGBA{0x18, 0x18, 0x28, 255}, true)
	case '◀', '▶':
		var p vector.Path
		w := fw * 0.32
		if c.r == '◀' {
			p.MoveTo(cx+w, cy-w*1.2)
			p.LineTo(cx-w, cy)
			p.LineTo(cx+w, cy+w*1.2)
		} else {
			p.MoveTo(cx-w, cy-w*1.2)
			p.LineTo(cx+w, cy)
			p.LineTo(cx-w, cy+w*1.2)
		}
		p.Close()
		op := &vector.DrawPathOptions{AntiAlias: true}
		op.ColorScale.ScaleWithColor(c.fg)
		vector.FillPath(dst, &p, nil, op)
	case '♛': // a crown: the party leader
		var p vector.Path
		w, h := fw*0.4, fh*0.22
		p.MoveTo(cx-w, cy+h)
		p.LineTo(cx-w, cy-h)
		p.LineTo(cx-w*0.45, cy)
		p.LineTo(cx, cy-h*1.4)
		p.LineTo(cx+w*0.45, cy)
		p.LineTo(cx+w, cy-h)
		p.LineTo(cx+w, cy+h)
		p.Close()
		op := &vector.DrawPathOptions{AntiAlias: true}
		op.ColorScale.ScaleWithColor(c.fg)
		vector.FillPath(dst, &p, nil, op)
	}
}
