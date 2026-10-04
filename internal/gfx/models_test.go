package gfx

import (
	"image"
	"image/color"
	"image/draw"
	"image/png"
	"os"
	"path/filepath"
	"testing"

	"ratas/internal/content"
)

// TestModelSheet paints every model; with RATAS_MODEL_SHEET=dir it also
// writes a contact sheet (both animation frames) for looking at the art.
func TestModelSheet(t *testing.T) {
	db, _, err := content.LoadDefault("")
	if err != nil {
		t.Fatal(err)
	}
	content.Use(db)
	type entry struct {
		def   modelDef
		color color.RGBA
	}
	var list []entry
	seen := map[string]bool{}
	add := func(model, def, col string, glyph rune, kind uint8) {
		key, d := resolveModel(model, def, glyph, kind)
		if seen[key+col] {
			return
		}
		seen[key+col] = true
		for f := 0; f < 2; f++ {
			if p := paintModel(d, hex(col), 0, f); p.w < 8 || p.h < 8 {
				t.Errorf("%s: tiny model", key)
			}
		}
		list = append(list, entry{d, hex(col)})
	}
	for _, c := range content.Classes() {
		add(c.Model, c.Key, c.Color, '@', kindPlayer)
	}
	for _, n := range content.NPCRoles() {
		add(n.Model, n.Key, n.Color, '@', kindNPC)
	}
	for _, m := range content.Monsters() {
		if key, _ := resolveModel(m.Model, m.Key, []rune(m.Glyph)[0], kindMonster); key == "brute" {
			t.Errorf("monster %s has no model of its own", m.Key)
		}
		add(m.Model, m.Key, m.Color, []rune(m.Glyph)[0], kindMonster)
	}
	for _, u := range content.Uniques() {
		if key, _ := resolveModel(u.Model, "unique:"+u.Key, '@', kindNPC); key != u.Model {
			t.Errorf("unique %s: model %q not found (got %s)", u.Key, u.Model, key)
		}
		add(u.Model, "unique:"+u.Key, u.Color, '@', kindNPC)
	}
	// heroes in their starting gear and in a few other outfits
	dressed := func(class string, gear ...string) {
		c := content.Class(class)
		if c == nil {
			t.Fatalf("no class %s", class)
		}
		for _, k := range gear {
			if k != "" && content.Item(k) == nil {
				t.Fatalf("no item %s", k)
			}
		}
		_, d := resolveModel(c.Model, c.Key, '@', kindPlayer)
		base := d.hum
		d.hum = func(col color.RGBA, v int) hum { return dress(base(col, v), gear) }
		for f := 0; f < 2; f++ {
			paintModel(d, hex(c.Color), 0, f)
		}
		list = append(list, entry{d, hex(c.Color)})
	}
	for _, c := range content.Classes() {
		gear := make([]string, 5)
		for _, k := range c.Items {
			d := content.Item(k)
			switch {
			case d == nil:
			case d.Kind == "weapon" && gear[gearMain] == "":
				gear[gearMain] = k
			case d.Kind == "weapon" || d.Kind == "shield" || d.Kind == "offhand":
				gear[gearOff] = k
			case d.Kind == "head":
				gear[gearHead] = k
			case d.Kind == "chest":
				gear[gearChest] = k
			case d.Kind == "back":
				gear[gearBack] = k
			}
		}
		dressed(c.Key, gear...)
	}
	for _, g := range sampleOutfits() {
		dressed(g[0], g[1:]...)
	}
	dir := os.Getenv("RATAS_MODEL_SHEET")
	if dir == "" {
		return
	}
	cw, ch, scale, cols := 56, 32, 4, 8
	if from := os.Getenv("RATAS_MODEL_FROM"); from != "" {
		n := 0
		for _, r := range from {
			n = n*10 + int(r-'0')
		}
		list = list[min(n, len(list)):min(n+12, len(list))]
		scale, cols = 7, 4
	}
	rows := (len(list) + cols - 1) / cols
	sheet := image.NewRGBA(image.Rect(0, 0, cols*cw*scale, rows*ch*scale))
	draw.Draw(sheet, sheet.Bounds(), &image.Uniform{color.RGBA{70, 104, 58, 255}}, image.Point{}, draw.Src)
	for i, e := range list {
		for f := 0; f < 2; f++ {
			p := paintModel(e.def, e.color, i, f)
			ox := (i%cols)*cw*scale + f*cw/2*scale + (cw/2-p.w)/2*scale
			oy := (i/cols)*ch*scale + (ch-1-p.h)*scale
			for y := 0; y < p.h; y++ {
				for x := 0; x < p.w; x++ {
					c := p.get(x, y)
					if c.A == 0 {
						continue
					}
					r := image.Rect(ox+x*scale, oy+y*scale, ox+(x+1)*scale, oy+(y+1)*scale)
					for yy := r.Min.Y; yy < r.Max.Y; yy++ {
						for xx := r.Min.X; xx < r.Max.X; xx++ {
							sheet.SetRGBA(xx, yy, blend(sheet.RGBAAt(xx, yy), c))
						}
					}
				}
			}
		}
	}
	f, err := os.Create(filepath.Join(dir, "models.png"))
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	if err := png.Encode(f, sheet); err != nil {
		t.Fatal(err)
	}
	t.Logf("%d models", len(list))
}

func blend(dst, src color.RGBA) color.RGBA {
	a := float64(src.A) / 255
	return color.RGBA{uint8(float64(dst.R)*(1-a) + float64(src.R)*a), uint8(float64(dst.G)*(1-a) + float64(src.G)*a), uint8(float64(dst.B)*(1-a) + float64(src.B)*a), 255}
}

// TestTileSheet paints every tile (RATAS_MODEL_SHEET=dir writes tiles.png).
func TestTileSheet(t *testing.T) {
	db, _, err := content.LoadDefault("")
	if err != nil {
		t.Fatal(err)
	}
	content.Use(db)
	dir := os.Getenv("RATAS_MODEL_SHEET")
	tiles := content.Get().Tiles
	const cell, scale, cols = 18, 4, 12
	rows := (len(tiles) + cols - 1) / cols
	sheet := image.NewRGBA(image.Rect(0, 0, cols*cell*scale, rows*(cell+8)*scale))
	draw.Draw(sheet, sheet.Bounds(), &image.Uniform{color.RGBA{20, 20, 28, 255}}, image.Point{}, draw.Src)
	put := func(p *pc, ox, oy int) {
		for y := 0; y < p.h; y++ {
			for x := 0; x < p.w; x++ {
				c := p.get(x, y)
				if c.A == 0 {
					continue
				}
				for yy := 0; yy < scale; yy++ {
					for xx := 0; xx < scale; xx++ {
						px, py := ox+x*scale+xx, oy+y*scale+yy
						sheet.SetRGBA(px, py, blend(sheet.RGBAAt(px, py), c))
					}
				}
			}
		}
	}
	for i := range tiles {
		def := &tiles[i]
		fg, bg := tileColors(def)
		ox, oy := (i%cols)*cell*scale, (i/cols)*(cell+8)*scale+8*scale
		if obj, tall, _ := objectArt(def.Key, fg, bg, 1); obj != nil {
			if base := groundTile(baseGround(def, "overworld")); base != nil && !tall {
				bfg, bbg := tileColors(base)
				put(groundArt(base.Key, bfg, bbg, 0, 0), ox, oy)
			}
			if tall {
				put(obj, ox, oy-8*scale)
			} else {
				put(obj, ox, oy)
			}
			continue
		}
		if !isKnownGround(def.Key) {
			t.Errorf("tile %s has no art", def.Key)
		}
		put(groundArt(def.Key, fg, bg, 1, 0), ox, oy)
	}
	if dir == "" {
		return
	}
	f, err := os.Create(filepath.Join(dir, "tiles.png"))
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	png.Encode(f, sheet)
}

// sampleOutfits: class followed by right hand, left hand, head, chest, back.
func sampleOutfits() [][]string {
	var out [][]string
	pick := func(kind, look, weapon string) string {
		for _, it := range content.Items() {
			if it.Kind == kind && (look == "" || it.Look == look) && (weapon == "" || it.Weapon == weapon) {
				return it.Key
			}
		}
		return ""
	}
	out = append(out,
		[]string{"warrior", pick("weapon", "", "sword"), pick("shield", "tower", ""), pick("head", "horned", ""), pick("chest", "plate", ""), pick("back", "", "")},
		[]string{"rogue", pick("weapon", "", "dagger"), pick("weapon", "", "dagger"), pick("head", "hood", ""), pick("chest", "leather", ""), ""},
		[]string{"rogue", pick("weapon", "", "crossbow"), "", pick("head", "fur_hat", ""), pick("chest", "chain", ""), pick("back", "", "")},
		[]string{"mage", pick("weapon", "", "wand"), pick("offhand", "orb", ""), pick("head", "circlet", ""), pick("chest", "robe", ""), ""},
		[]string{"priest", pick("weapon", "", "mace"), pick("offhand", "symbol", ""), pick("head", "mitre", ""), pick("chest", "robe", ""), pick("back", "", "")},
		[]string{"mage", pick("weapon", "", "sword"), pick("offhand", "book", ""), pick("head", "crown", ""), pick("chest", "plate", ""), ""},
		[]string{"warrior", pick("weapon", "", "axe"), pick("weapon", "", "mace"), "", "", ""},
	)
	return out
}

// TestIconSheet paints the ground icon of every item (RATAS_MODEL_SHEET=dir
// writes icons.png).
func TestIconSheet(t *testing.T) {
	db, _, err := content.LoadDefault("")
	if err != nil {
		t.Fatal(err)
	}
	content.Use(db)
	items := content.Items()
	const cell, scale, cols = 18, 4, 16
	rows := (len(items) + cols - 1) / cols
	sheet := image.NewRGBA(image.Rect(0, 0, cols*cell*scale, rows*cell*scale))
	draw.Draw(sheet, sheet.Bounds(), &image.Uniform{color.RGBA{40, 44, 36, 255}}, image.Point{}, draw.Src)
	for i, it := range items {
		shape := iconShape(&it, []rune(it.Glyph)[0])
		p := itemIcon(shape, hex(it.Color))
		if p == nil {
			t.Errorf("item %s (%s) has no icon", it.Key, it.Kind)
			continue
		}
		p = outlined(p)
		ox, oy := (i%cols)*cell*scale+2*scale, (i/cols)*cell*scale+2*scale
		for y := 0; y < p.h; y++ {
			for x := 0; x < p.w; x++ {
				c := p.get(x, y)
				if c.A == 0 {
					continue
				}
				for yy := 0; yy < scale; yy++ {
					for xx := 0; xx < scale; xx++ {
						px, py := ox+x*scale+xx, oy+y*scale+yy
						sheet.SetRGBA(px, py, blend(sheet.RGBAAt(px, py), c))
					}
				}
			}
		}
	}
	dir := os.Getenv("RATAS_MODEL_SHEET")
	if dir == "" {
		return
	}
	f, err := os.Create(filepath.Join(dir, "icons.png"))
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	png.Encode(f, sheet)
}
