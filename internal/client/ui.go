// Package client is the terminal front-end: menus, the ASCII renderer and
// input handling. It only talks to the game through the protocol, so it works
// the same for local and remote worlds.
package client

import (
	"strings"
	"unicode/utf8"

	"github.com/gdamore/tcell/v2"

	"ratas/internal/i18n"
)

var colorCache = map[string]tcell.Color{}

// col converts "#rrggbb" (or a tcell color name) to a tcell color.
func col(s string) tcell.Color {
	if s == "" {
		return tcell.ColorDefault
	}
	if c, ok := colorCache[s]; ok {
		return c
	}
	c := tcell.GetColor(s)
	colorCache[s] = c
	return c
}

func rgb(c tcell.Color) (int32, int32, int32) {
	if c == tcell.ColorDefault {
		return 0, 0, 0
	}
	return c.TrueColor().RGB()
}

func clamp8(v float64) int32 {
	if v < 0 {
		return 0
	}
	if v > 255 {
		return 255
	}
	return int32(v)
}

// shade scales a color by k and blends it toward tint by t.
func shade(c tcell.Color, k float64, tint [3]float64, t float64) tcell.Color {
	r, g, b := rgb(c)
	fr := float64(r)*k*(1-t) + tint[0]*t
	fg := float64(g)*k*(1-t) + tint[1]*t
	fb := float64(b)*k*(1-t) + tint[2]*t
	return tcell.NewRGBColor(clamp8(fr), clamp8(fg), clamp8(fb))
}

// Theme colors for the interface.
var (
	cText   = col("#d8d8e0")
	cDim    = col("#707080")
	cAccent = col("#ffd24a")
	cBorder = col("#5a5a7a")
	cPanel  = col("#10101a")
	cSelBG  = col("#2a3a6a")
	cBlack  = col("#000000")
	cHP     = col("#e04040")
	cMP     = col("#4070e0")
	cXP     = col("#c0a020")
	cGood   = col("#60e060")
	cBad    = col("#ff6060")
	cTitle  = col("#ff9a4a")
)

// asciiUI switches interface frames and bars to plain ASCII.
var asciiUI bool

func uiRune(unicode, ascii rune) rune {
	if asciiUI {
		return ascii
	}
	return unicode
}

type canvas struct {
	s    tcell.Screen
	w, h int
}

func (c *canvas) put(x, y int, r rune, fg, bg tcell.Color) {
	if x < 0 || y < 0 || x >= c.w || y >= c.h {
		return
	}
	c.s.SetContent(x, y, r, nil, tcell.StyleDefault.Foreground(fg).Background(bg))
}

// text draws a string in the player's language and returns the x after it.
func (c *canvas) text(x, y int, s string, fg, bg tcell.Color) int {
	return c.raw(x, y, i18n.T(s), fg, bg)
}

// raw draws a string as is (typed text, translated text).
func (c *canvas) raw(x, y int, s string, fg, bg tcell.Color) int {
	for _, r := range s {
		if x >= c.w {
			break
		}
		c.put(x, y, r, fg, bg)
		x++
	}
	return x
}

func (c *canvas) textClip(x, y, maxW int, s string, fg, bg tcell.Color) {
	if maxW <= 0 {
		return
	}
	rs := []rune(i18n.T(s))
	if len(rs) > maxW {
		rs = append(rs[:max(0, maxW-1)], '…')
	}
	c.raw(x, y, string(rs), fg, bg)
}

func (c *canvas) fill(x, y, w, h int, r rune, fg, bg tcell.Color) {
	for yy := y; yy < y+h; yy++ {
		for xx := x; xx < x+w; xx++ {
			c.put(xx, yy, r, fg, bg)
		}
	}
}

// box draws a framed panel with an optional title and clears the inside.
func (c *canvas) box(x, y, w, h int, title string, border tcell.Color) {
	c.fill(x, y, w, h, ' ', cText, cPanel)
	hz, vt := uiRune('─', '-'), uiRune('│', '|')
	for xx := x + 1; xx < x+w-1; xx++ {
		c.put(xx, y, hz, border, cPanel)
		c.put(xx, y+h-1, hz, border, cPanel)
	}
	for yy := y + 1; yy < y+h-1; yy++ {
		c.put(x, yy, vt, border, cPanel)
		c.put(x+w-1, yy, vt, border, cPanel)
	}
	c.put(x, y, uiRune('┌', '+'), border, cPanel)
	c.put(x+w-1, y, uiRune('┐', '+'), border, cPanel)
	c.put(x, y+h-1, uiRune('└', '+'), border, cPanel)
	c.put(x+w-1, y+h-1, uiRune('┘', '+'), border, cPanel)
	if title != "" {
		c.raw(x+2, y, " "+i18n.T(title)+" ", cAccent, cPanel)
	}
}

// bar draws a progress bar of width w.
func (c *canvas) bar(x, y, w int, frac float64, fg, bg tcell.Color) {
	frac = max(0, min(1, frac))
	full := int(frac*float64(w) + 0.5)
	for i := 0; i < w; i++ {
		if i < full {
			c.put(x+i, y, uiRune('█', '#'), fg, cPanel)
		} else {
			c.put(x+i, y, uiRune('░', '.'), bg, cPanel)
		}
	}
}

// wrap translates text and splits it into lines of at most w runes.
func wrap(s string, w int) []string {
	if w <= 0 {
		return nil
	}
	s = i18n.T(s)
	var out []string
	for _, para := range strings.Split(s, "\n") {
		words := strings.Fields(para)
		line := ""
		for _, word := range words {
			for utf8.RuneCountInString(word) > w {
				rs := []rune(word)
				if line != "" {
					out = append(out, line)
					line = ""
				}
				out = append(out, string(rs[:w]))
				word = string(rs[w:])
			}
			switch {
			case line == "":
				line = word
			case utf8.RuneCountInString(line)+1+utf8.RuneCountInString(word) <= w:
				line += " " + word
			default:
				out = append(out, line)
				line = word
			}
		}
		out = append(out, line)
	}
	return out
}

// runeLen is the width of a text as it will be shown (translated).
func runeLen(s string) int { return utf8.RuneCountInString(i18n.T(s)) }

// layoutKey maps Cyrillic (ЙЦУКЕН) keys to their QWERTY positions so hotkeys
// work with a Russian keyboard layout active.
var layoutKey = map[rune]rune{
	'й': 'q', 'ц': 'w', 'у': 'e', 'к': 'r', 'е': 't', 'н': 'y', 'г': 'u', 'ш': 'i', 'щ': 'o', 'з': 'p',
	'ф': 'a', 'ы': 's', 'в': 'd', 'а': 'f', 'п': 'g', 'р': 'h', 'о': 'j', 'л': 'k', 'д': 'l',
	'я': 'z', 'ч': 'x', 'с': 'c', 'м': 'v', 'и': 'b', 'т': 'n', 'ь': 'm', 'ё': '`', '.': '/',
}

func normKey(r rune) rune {
	if r >= 'A' && r <= 'Z' {
		r += 'a' - 'A'
	}
	lr := []rune(strings.ToLower(string(r)))[0]
	if m, ok := layoutKey[lr]; ok {
		return m
	}
	return lr
}

// textInput is a single-line editable field.
type textInput struct {
	value  []rune
	maxLen int
	mask   bool
}

// handle processes a key; it returns true if the key was consumed.
func (t *textInput) handle(ev *tcell.EventKey) bool {
	if isPasteKey(ev) {
		t.paste(readClipboard())
		return true
	}
	switch ev.Key() {
	case tcell.KeyBackspace, tcell.KeyBackspace2:
		if len(t.value) > 0 {
			t.value = t.value[:len(t.value)-1]
		}
		return true
	case tcell.KeyCtrlU:
		t.value = nil
		return true
	case tcell.KeyRune:
		if t.maxLen == 0 || len(t.value) < t.maxLen {
			t.value = append(t.value, ev.Rune())
		}
		return true
	}
	return false
}

func (t *textInput) String() string { return string(t.value) }
func (t *textInput) Set(s string)   { t.value = []rune(s) }

func (t *textInput) draw(c *canvas, x, y, w int, focused bool) {
	bg := col("#1a1a2a")
	if focused {
		bg = col("#24304a")
	}
	c.fill(x, y, w, 1, ' ', cText, bg)
	s := t.value
	if t.mask && len(s) > 0 {
		shown := []rune(strings.Repeat("•", len(s)))
		if len(s) > 8 {
			shown = append([]rune(string(s[:4])), []rune(strings.Repeat("•", len(s)-8))...)
			shown = append(shown, s[len(s)-4:]...)
		}
		s = shown
	}
	if len(s) > w-1 {
		s = s[len(s)-(w-1):]
	}
	c.raw(x, y, string(s), cText, bg)
	if focused {
		c.put(x+len(s), y, '▏', cAccent, bg)
	}
}
