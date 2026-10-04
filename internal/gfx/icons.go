package gfx

import (
	"image/color"
	"strconv"

	"github.com/hajimehoshi/ebiten/v2"

	"ratas/internal/content"
	"ratas/internal/proto"
)

// model is a creature model ready to draw (two animation frames).
type model struct {
	frames [2]*ebiten.Image
	float  bool
}

func (r *worldRenderer) modelOf(e proto.EntityView) *model {
	name, d := resolveModel(e.Model, e.Def, e.Glyph, e.Kind)
	variant := 0
	if d.variants > 1 {
		variant = int(e.ID) % d.variants
	}
	key := name + "|" + e.Color + "|" + strconv.Itoa(variant)
	if len(e.Gear) > 0 && d.hum != nil {
		// heroes wear what they have equipped
		base := d.hum
		gear := e.Gear
		d.hum = func(c color.RGBA, v int) hum { return dress(base(c, v), gear) }
		key += "|" + gearKey(gear)
	}
	if m, ok := r.models[key]; ok {
		return m
	}
	c := hex(e.Color)
	m := &model{float: d.float}
	for f := range m.frames {
		m.frames[f] = paintModel(d, c, variant, f).image()
	}
	r.models[key] = m
	return m
}

// icon is the picture of an item lying on the ground: chosen by the kind of
// item, or by its glyph for items the client does not know.
func (r *worldRenderer) icon(g rune, c color.RGBA, def string) *ebiten.Image {
	shape := iconShape(content.Item(def), g)
	key := shape + hexKey(c)
	if img, ok := r.icons[key]; ok {
		return img
	}
	var img *ebiten.Image
	if p := itemIcon(shape, c); p != nil {
		img = outlined(p).image()
	} else {
		img = r.glyphTile(g, c)
	}
	r.icons[key] = img
	return img
}

// iconShape names the picture of an item.
func iconShape(d *content.ItemDef, g rune) string {
	if d == nil {
		return string(g)
	}
	switch d.Kind {
	case "weapon":
		switch d.Weapon {
		case "sword", "dagger":
			return "|"
		case "axe":
			return "P"
		case "mace", "hammer":
			return "T"
		case "bow":
			return ")"
		case "crossbow":
			return "crossbow"
		}
		return "/"
	case "shield":
		return "shield"
	case "offhand":
		switch d.Look {
		case "book", "orb", "symbol":
			return d.Look
		}
		return "orb"
	case "head":
		switch d.Look {
		case "helmet", "horned":
			return "helmet"
		case "crown", "circlet":
			return "^"
		case "hood", "wizard", "mitre":
			return "hat"
		}
		return "cap"
	case "chest":
		return "["
	case "legs":
		return "legs"
	case "belt":
		return "belt"
	case "back":
		return "cloak"
	case "ring":
		return "="
	case "quest":
		return "relic"
	}
	return string(g)
}

func itemIcon(shape string, c color.RGBA) *pc {
	p := newPC(14, 14, "icon"+shape)
	switch shape {
	case "crossbow":
		p.rect(1, 6, 12, 2, cWood) // stock
		p.rect(1, 7, 3, 2, mul(cWood, 0.8))
		for y := 1; y <= 12; y++ { // the prod across the stock
			x := 10
			if y <= 2 || y >= 11 {
				x = 9
			}
			p.set(x, y, mul(c, 1.1-0.03*float64(y)))
			p.set(x+1, y, mul(c, 0.75))
		}
		p.line(9, 1, 5, 6, alpha(cWhite, 160))
		p.line(5, 7, 9, 12, alpha(cWhite, 160))
		p.set(12, 6, cSteel)
	case "shield":
		for y := 1; y < 13; y++ {
			half := 5
			if y > 8 {
				half = 5 - (y-8)*5/4
			}
			for x := 7 - half; x < 7+half; x++ {
				k := 1.0
				if x >= 7+half-1 {
					k = 0.72
				}
				if y == 1 || x == 7-half {
					k = 1.15
				}
				p.set(x, y, mul(c, k))
			}
		}
		p.rect(6, 3, 2, 7, cGold)
		p.rect(4, 5, 6, 2, cGold)
	case "book":
		p.rect(2, 2, 10, 10, mul(c, 0.9))
		p.rect(3, 3, 8, 8, c)
		p.rect(10, 3, 2, 9, hex("#f0e8d0"))
		p.rect(5, 5, 3, 3, cGold)
	case "orb":
		p.ball(7, 6.5, 4.6, c, 0)
		p.set(5, 4, cWhite)
		p.set(6, 4, alpha(cWhite, 180))
		p.rect(4, 11, 6, 2, cWood)
	case "symbol":
		p.rect(6, 1, 2, 12, c)
		p.rect(3, 4, 8, 2, c)
		p.rect(7, 1, 1, 12, mul(c, 0.75))
		p.set(6, 1, cWhite)
	case "helmet":
		for y := 2; y < 11; y++ {
			for x := 2; x < 12; x++ {
				dx, dy := float64(x)-6.5, float64(y)-7
				if dx*dx/25+dy*dy/25 > 1 || y > 10 {
					continue
				}
				k := 1.1 - 0.06*float64(x-2)
				p.set(x, y, mul(c, k))
			}
		}
		p.rect(3, 7, 8, 1, cDarkK)
		p.rect(6, 7, 2, 4, mul(c, 0.7))
	case "hat":
		for y := 1; y < 10; y++ {
			half := 1 + y/2
			for x := 7 - half; x < 7+half; x++ {
				p.set(x, y, mul(c, 1.1-0.08*float64(x-(7-half))))
			}
		}
		p.rect(1, 10, 12, 2, mul(c, 0.8))
	case "cap":
		p.blob(7, 7, 5, 3.6, c)
		p.rect(2, 9, 10, 2, mul(c, 0.75))
		p.set(7, 3, cWhite)
	case "legs":
		p.rect(3, 1, 8, 3, mul(c, 1.05))
		p.rect(3, 4, 3, 8, c)
		p.rect(8, 4, 3, 8, mul(c, 0.82))
		p.rect(2, 11, 4, 2, mul(cLeather, 0.8))
		p.rect(8, 11, 4, 2, mul(cLeather, 0.7))
	case "belt":
		p.rect(1, 5, 12, 4, c)
		p.rect(1, 5, 12, 1, mix(c, cWhite, 0.3))
		p.rect(5, 4, 4, 6, cGold)
		p.rect(6, 5, 2, 4, mul(c, 0.6))
	case "cloak":
		for y := 1; y < 13; y++ {
			half := 3 + y/3
			for x := 7 - half; x < 7+half; x++ {
				k := 1.0
				if (x+y)%4 == 0 {
					k = 0.85
				}
				p.set(x, y, mul(c, k))
			}
		}
		p.rect(5, 1, 4, 2, cGold)
	case "relic":
		for y := 1; y < 13; y++ {
			half := 5 - abs(y-6)*5/6
			for x := 7 - half; x <= 7+half; x++ {
				p.set(x, y, mul(c, 1.15-0.07*float64(x-(7-half))))
			}
		}
		p.set(6, 4, cWhite)
		p.set(5, 5, alpha(cWhite, 180))
	case "!": // potion
		p.rect(6, 1, 2, 2, cWood)
		p.rect(6, 3, 2, 2, alpha(cWhite, 200))
		p.blob(7, 9, 4.6, 4.2, mix(c, cWhite, 0.15))
		p.set(5, 7, cWhite)
		p.set(5, 8, alpha(cWhite, 180))
	case "?": // scroll
		paper := hex("#e8dcb0")
		p.rect(3, 3, 8, 8, paper)
		p.rect(2, 2, 10, 2, mul(paper, 0.8))
		p.rect(2, 10, 10, 2, mul(paper, 0.8))
		for y := 5; y <= 8; y++ {
			p.rect(4, y, 6-(y%2)*2, 1, mul(c, 0.7))
		}
		p.set(11, 11, hex("#c03030"))
	case "%": // food
		p.blob(7, 8, 5, 3.6, c)
		p.blob(5.5, 7, 2, 1.4, mix(c, cWhite, 0.35))
	case "|": // sword or dagger
		for i := 0; i < 8; i++ {
			p.set(3+i, 10-i, mul(c, 1.1))
			p.set(4+i, 10-i, mul(c, 0.75))
		}
		p.thick(1, 9, 5, 13, 1, cGold)
		p.set(2, 12, cWood)
		p.set(1, 13, cWood)
	case "/": // staff, wand or spear
		p.thick(2, 13, 10, 3, 1.2, cWood)
		p.blob(11, 2.5, 2, 2, c)
		p.set(10, 2, cWhite)
	case ")": // bow
		for y := 1; y <= 12; y++ {
			x := 4
			if y <= 2 || y >= 11 {
				x = 7
			} else if y <= 4 || y >= 9 {
				x = 5
			}
			p.set(x, y, c)
			p.set(8, y, alpha(cWhite, 170))
		}
	case "P": // axe
		p.thick(3, 13, 9, 3, 1.2, cWood)
		p.blob(10, 4, 3, 3, mul(c, 1.05))
		p.set(9, 2, cWhite)
	case "T": // hammer or mace
		p.thick(3, 13, 8, 5, 1.2, cWood)
		p.rect(6, 1, 6, 5, c)
		p.rect(6, 1, 6, 1, mix(c, cWhite, 0.4))
		p.rect(11, 1, 1, 5, mul(c, 0.7))
	case "[": // armor
		p.rect(3, 3, 8, 9, c)
		p.rect(1, 3, 3, 4, mul(c, 1.1))
		p.rect(10, 3, 3, 4, mul(c, 0.8))
		p.rect(6, 3, 2, 2, color.RGBA{})
		p.rect(4, 4, 1, 6, mix(c, cWhite, 0.4))
		p.rect(3, 9, 8, 1, mul(c, 0.6))
	case "=": // ring
		for y := 0; y < 14; y++ {
			for x := 0; x < 14; x++ {
				dx, dy := float64(x)-6.5, float64(y)-7.5
				if d := dx*dx + dy*dy; d < 16 && d > 6 {
					p.set(x, y, mul(cGold, 1.1-0.05*dx))
				}
			}
		}
		p.blob(6.5, 3.5, 1.8, 1.6, c)
	case "\"": // amulet
		for i := 0; i < 5; i++ {
			p.set(2+i, 2+i, mul(cGold, 0.85))
			p.set(11-i, 2+i, mul(cGold, 0.7))
		}
		p.blob(6.5, 9.5, 3, 3, c)
		p.set(5, 8, cWhite)
	case "^": // crown or mask
		p.rect(2, 7, 10, 4, cGold)
		for x := 2; x <= 11; x += 3 {
			p.rect(x, 4, 1, 3, cGold)
		}
		p.blob(6.5, 8.5, 1.5, 1.4, c)
	case "$": // coins
		for _, xy := range [][2]float64{{4, 9}, {9, 9}, {6.5, 6.5}, {6.5, 10.5}} {
			p.blob(xy[0], xy[1], 2.6, 2, cGold)
			p.set(int(xy[0])-1, int(xy[1])-1, hex("#fff2a0"))
		}
	default:
		return nil
	}
	return p
}
