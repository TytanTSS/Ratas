package gfx

import (
	"os"
	"path/filepath"
	"testing"

	"golang.org/x/image/font/gofont/gobold"
	"golang.org/x/image/font/gofont/gomono"
	"golang.org/x/image/font/sfnt"
)

// TestUIGlyphs: every symbol the text interface prints must exist in the
// window font or be drawn by the grid itself, otherwise it shows up as an
// empty box.
func TestUIGlyphs(t *testing.T) {
	mono, err := sfnt.Parse(gomono.TTF)
	if err != nil {
		t.Fatal(err)
	}
	bold, err := sfnt.Parse(gobold.TTF)
	if err != nil {
		t.Fatal(err)
	}
	var buf sfnt.Buffer
	has := func(f *sfnt.Font, r rune) bool {
		i, _ := f.GlyphIndex(&buf, r)
		return i != 0
	}
	files, _ := filepath.Glob("../client/*.go")
	more, _ := filepath.Glob("../content/data/*.toml")
	files = append(files, more...)
	seen := map[rune]bool{}
	for _, name := range files {
		data, err := os.ReadFile(name)
		if err != nil {
			t.Fatal(err)
		}
		for _, r := range string(data) {
			if r < 0x80 || seen[r] || isSpecial(r) {
				continue
			}
			seen[r] = true
			if !has(mono, r) {
				t.Errorf("%s: %q (U+%04X) is not in the mono font", filepath.Base(name), r, r)
			}
		}
	}
	for _, r := range "†" { // world labels
		if !has(bold, r) {
			t.Errorf("%q is not in the bold font", r)
		}
	}
}
