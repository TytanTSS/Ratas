// Package i18n translates the game's text.
//
// The game is written in Russian: the server, the saves and the content keep
// Russian text, and every client translates what it shows into its player's
// language. So players with different languages share one world.
//
// A catalog maps Russian source text to a translation. Besides exact phrases
// the translator understands:
//   - templates: a source text with fmt verbs ("Регион: %s — %s.") matches a
//     finished line, and its arguments are translated in turn;
//   - composite lines: text glued from pieces is split into sentences and
//     segments (" • ", ", ", "(…)") that are translated one by one;
//   - proper names: unknown words (people, villages) are transliterated.
//
// Catalogs are embedded TOML files (en/*.toml) and can be extended by mods.
package i18n

import (
	"embed"
	"fmt"
	"io/fs"
	"strings"
	"sync"
	"sync/atomic"
	"unicode"

	"github.com/BurntSushi/toml"
)

// Languages.
const (
	RU = "ru" // the source language
	EN = "en"
)

// Langs lists the supported languages with their own names.
var Langs = []struct{ Code, Name string }{{RU, "Русский"}, {EN, "English"}}

var lang atomic.Value

// SetLang chooses the language of this process ("ru" or "en").
func SetLang(l string) {
	l = Normalize(l)
	lang.Store(l)
}

// Lang is the current language.
func Lang() string {
	if l, ok := lang.Load().(string); ok {
		return l
	}
	return RU
}

// Normalize turns "en_US.UTF-8", "EN" or "english" into a supported code.
func Normalize(l string) string {
	l = strings.ToLower(strings.TrimSpace(l))
	switch {
	case strings.HasPrefix(l, "en"):
		return EN
	}
	return RU
}

// T translates text into the current language.
func T(s string) string { return In(Lang(), s) }

// Tf formats a Russian format string and translates the result.
func Tf(format string, args ...any) string { return T(fmt.Sprintf(format, args...)) }

// In translates text into a language.
func In(l, s string) string {
	if l == RU || !hasCyrillic(s) {
		return s
	}
	c := catalogFor(l)
	if c == nil {
		return s
	}
	return c.translate(s)
}

// Add merges translations (Russian → translation) into a language, for mods.
func Add(l string, entries map[string]string) {
	l = Normalize(l)
	if l == RU || len(entries) == 0 {
		return
	}
	c := catalogFor(l)
	c.mu.Lock()
	for k, v := range entries {
		c.addLocked(k, v)
	}
	c.cache = map[string]string{}
	c.mu.Unlock()
	c.rebuild()
}

// Has reports whether a language has a translation of exactly this text
// (after trimming), used by tests that check the catalog is complete.
func Has(l, s string) bool {
	c := catalogFor(l)
	if c == nil {
		return false
	}
	c.mu.RLock()
	defer c.mu.RUnlock()
	s = strings.TrimSpace(s)
	if _, ok := c.exact[s]; ok {
		return true
	}
	if k, _ := placeholders(s, ""); c.tmplSrc[k] != "" {
		return true
	}
	_, k, _ := splitCore(s)
	_, ok := c.exact[k]
	return ok
}

// Misses returns texts that had no translation and were transliterated
// (for finding gaps in the catalog).
func Misses(l string) []string {
	c := catalogFor(l)
	if c == nil {
		return nil
	}
	c.mu.RLock()
	defer c.mu.RUnlock()
	out := make([]string, 0, len(c.misses))
	for s := range c.misses {
		out = append(out, s)
	}
	return out
}

//go:embed en/*.toml
var files embed.FS

var (
	catalogsMu sync.Mutex
	catalogs   = map[string]*catalog{}
)

func catalogFor(l string) *catalog {
	l = Normalize(l)
	if l == RU {
		return nil
	}
	catalogsMu.Lock()
	defer catalogsMu.Unlock()
	if c, ok := catalogs[l]; ok {
		return c
	}
	c := newCatalog(l)
	paths, _ := fs.Glob(files, l+"/*.toml")
	for _, p := range paths {
		data, err := files.ReadFile(p)
		if err != nil {
			continue
		}
		var m map[string]string
		if _, err := toml.Decode(string(data), &m); err != nil {
			panic(fmt.Sprintf("i18n: %s: %v", p, err))
		}
		for k, v := range m {
			c.addLocked(k, v)
		}
	}
	c.rebuild()
	catalogs[l] = c
	return c
}

func hasCyrillic(s string) bool {
	for _, r := range s {
		if r >= 0x400 && r <= 0x4ff {
			return true
		}
	}
	return false
}

func upperFirst(s string) string {
	for i, r := range s {
		return string(unicode.ToUpper(r)) + s[i+len(string(r)):]
	}
	return s
}

func lowerFirst(s string) string {
	for i, r := range s {
		return string(unicode.ToLower(r)) + s[i+len(string(r)):]
	}
	return s
}

func isUpperFirst(s string) bool {
	for _, r := range s {
		return unicode.IsUpper(r)
	}
	return false
}
