package i18n

import (
	"fmt"
	"regexp"
	"strconv"
	"strings"
	"sync"
	"unicode"
	"unicode/utf8"
)

// catalog holds the translations into one language.
type catalog struct {
	lang    string
	mu      sync.RWMutex
	exact   map[string]string  // trimmed source → translation
	tmplSrc map[string]string  // format source → format translation
	buckets map[string][]*tmpl // templates by the first word of their text
	loose   []*tmpl            // templates that start with an argument
	cache   map[string]string  // finished translations
	misses  map[string]bool    // texts that were transliterated
}

func newCatalog(l string) *catalog {
	return &catalog{lang: l, exact: map[string]string{}, tmplSrc: map[string]string{},
		buckets: map[string][]*tmpl{}, cache: map[string]string{}, misses: map[string]bool{}}
}

// addLocked registers one entry; formats with verbs become templates.
func (c *catalog) addLocked(k, v string) {
	k = strings.TrimSpace(k)
	v = strings.TrimSpace(v)
	if k == "" || v == "" {
		return
	}
	k, v = placeholders(k, v)
	if hasVerbs(k) {
		c.tmplSrc[k] = v
		return
	}
	c.exact[k] = v
	// "• выдал:" = "• given by:" also teaches "выдал" = "given by"
	kl, kc, kt := splitCore(k)
	vl, vc, vt := splitCore(v)
	if kc != k && kc != "" && vc != "" && kl == vl && kt == vt {
		if _, ok := c.exact[kc]; !ok {
			c.exact[kc] = vc
		}
	}
}

// rebuild compiles the templates.
func (c *catalog) rebuild() {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.buckets, c.loose = map[string][]*tmpl{}, nil
	for src, dst := range c.tmplSrc {
		t, err := compile(src, dst)
		if err != nil {
			continue
		}
		if t.word != "" {
			c.buckets[t.word] = append(c.buckets[t.word], t)
		} else {
			c.loose = append(c.loose, t)
		}
	}
}

func (c *catalog) translate(s string) string {
	c.mu.RLock()
	v, ok := c.cache[s]
	c.mu.RUnlock()
	if ok {
		return v
	}
	out := c.tr(s, 0)
	c.mu.Lock()
	if len(c.cache) > 50000 {
		c.cache = map[string]string{}
	}
	c.cache[s] = out
	c.mu.Unlock()
	return out
}

func (c *catalog) lookup(s string) (string, bool) {
	c.mu.RLock()
	defer c.mu.RUnlock()
	if v, ok := c.exact[s]; ok {
		return v, true
	}
	// the same phrase at the start of a sentence or inside one
	if isUpperFirst(s) {
		if v, ok := c.exact[lowerFirst(s)]; ok {
			return upperFirst(v), true
		}
	} else if v, ok := c.exact[upperFirst(s)]; ok {
		return lowerFirst(v), true
	}
	return "", false
}

// tr translates a piece of text; depth limits the splitting.
func (c *catalog) tr(s string, depth int) string {
	if !hasCyrillic(s) {
		return s
	}
	lead, body, trail := splitSpace(s)
	if v, ok := c.lookup(body); ok {
		return lead + v + trail
	}
	pl, core, pt := splitCore(body)
	if core != body && core != "" {
		if v, ok := c.lookup(core); ok {
			return lead + pl + v + pt + trail
		}
	}
	if v, ok := c.matchTemplate(body, depth); ok {
		return lead + v + trail
	}
	if core != body && core != "" {
		if v, ok := c.matchTemplate(core, depth); ok {
			return lead + pl + v + pt + trail
		}
	}
	if depth < 10 {
		if v, ok := c.segment(body, depth); ok {
			return lead + v + trail
		}
	}
	return lead + c.fallback(body) + trail
}

var sentenceEnd = regexp.MustCompile(`([.!?…]+["»)]?)\s+`)

// separators that split composite lines, the strongest first
var separators = []string{"\n", " • ", " — ", " – ", " | ", "; ", ", ", ": ", " / ", " («", "» ", "«", "»", " (", ")"}

func (c *catalog) segment(s string, depth int) (string, bool) {
	// sentences
	if locs := sentenceEnd.FindAllStringIndex(s, -1); len(locs) > 0 {
		var b strings.Builder
		prev := 0
		for _, l := range locs {
			if l[1] >= len(s) {
				break
			}
			// keep the punctuation with its sentence, the spaces as they are
			punct := strings.TrimRight(s[l[0]:l[1]], " \t\n")
			b.WriteString(c.tr(s[prev:l[0]]+punct, depth+1))
			b.WriteString(s[l[0]+len(punct) : l[1]])
			prev = l[1]
		}
		if prev > 0 {
			b.WriteString(c.tr(s[prev:], depth+1))
			return b.String(), true
		}
	}
	for _, sep := range separators {
		if !strings.Contains(s, sep) {
			continue
		}
		parts := strings.Split(s, sep)
		for i, p := range parts {
			parts[i] = c.tr(p, depth+1)
		}
		return strings.Join(parts, sep), true
	}
	return "", false
}

// fallback handles text without a translation: proper names (every word
// capitalised: people, villages) are transliterated, free text (chat, lines
// of AI characters) is left as written.
func (c *catalog) fallback(s string) string {
	c.mu.Lock()
	if len(c.misses) < 5000 {
		c.misses[s] = true
	}
	c.mu.Unlock()
	if !isName(s) {
		return s
	}
	return Translit(s)
}

func isName(s string) bool {
	words := 0
	for _, w := range strings.FieldsFunc(s, func(r rune) bool { return !unicode.IsLetter(r) && r != '-' }) {
		if !hasCyrillic(w) {
			continue
		}
		words++
		if !isUpperFirst(w) || words > 4 {
			return false
		}
	}
	return words > 0
}

// splitSpace separates leading and trailing white space.
func splitSpace(s string) (lead, body, trail string) {
	body = strings.TrimLeft(s, " \t\n")
	lead = s[:len(s)-len(body)]
	b2 := strings.TrimRight(body, " \t\n")
	trail = body[len(b2):]
	return lead, b2, trail
}

const edge = " \t\n•:—–-|/()[]«»\"'.,;!?…*>"

// splitCore separates punctuation around a phrase.
func splitCore(s string) (lead, core, trail string) {
	core = strings.TrimLeft(s, edge)
	lead = s[:len(s)-len(core)]
	c2 := strings.TrimRight(core, edge)
	trail = core[len(c2):]
	return lead, c2, trail
}

// ---- templates ----

type verb struct {
	flags, width string
	kind         byte
	index        int // explicit argument index (1-based), 0 = next
}

type piece struct {
	lit string
	v   *verb
}

type tmpl struct {
	re     *regexp.Regexp
	src    []*verb // verbs of the source in order
	out    []piece // the translation
	weight int     // length of the literal text: more specific wins
	word   string  // first word, for quick candidate selection
}

// parseFormat splits a fmt format into literal text and verbs.
func parseFormat(f string) []piece {
	var out []piece
	var lit strings.Builder
	for i := 0; i < len(f); i++ {
		if f[i] != '%' {
			lit.WriteByte(f[i])
			continue
		}
		j := i + 1
		if j < len(f) && f[j] == '%' {
			lit.WriteByte('%')
			i = j
			continue
		}
		v := &verb{}
		for j < len(f) && strings.IndexByte("-+# 0", f[j]) >= 0 {
			v.flags += string(f[j])
			j++
		}
		if j < len(f) && f[j] == '[' {
			k := strings.IndexByte(f[j:], ']')
			if k > 0 {
				v.index, _ = strconv.Atoi(f[j+1 : j+k])
				j += k + 1
			}
		}
		for j < len(f) && (f[j] >= '0' && f[j] <= '9') {
			v.width += string(f[j])
			j++
		}
		if j < len(f) && f[j] == '.' {
			j++
			for j < len(f) && f[j] >= '0' && f[j] <= '9' {
				j++
			}
		}
		if j < len(f) && strings.IndexByte("svdqfgecxXt", f[j]) >= 0 {
			v.kind = f[j]
			if lit.Len() > 0 {
				out = append(out, piece{lit: lit.String()})
				lit.Reset()
			}
			out = append(out, piece{v: v})
			i = j
			continue
		}
		lit.WriteByte('%') // a plain percent sign
	}
	if lit.Len() > 0 {
		out = append(out, piece{lit: lit.String()})
	}
	return out
}

func hasVerbs(s string) bool {
	for _, p := range parseFormat(s) {
		if p.v != nil {
			return true
		}
	}
	return false
}

func compile(src, dst string) (*tmpl, error) {
	t := &tmpl{out: parseFormat(dst)}
	var re strings.Builder
	re.WriteString(`(?s)^`)
	ps := parseFormat(src)
	for i, p := range ps {
		if p.v == nil {
			re.WriteString(regexp.QuoteMeta(p.lit))
			t.weight += utf8.RuneCountInString(strings.TrimSpace(p.lit))
			if i == 0 && strings.Contains(p.lit, " ") {
				t.word = firstWord(p.lit)
			}
			continue
		}
		t.src = append(t.src, p.v)
		switch p.v.kind {
		case 'd':
			re.WriteString(`\s*([-+]?\d+)`)
		case 'f', 'g', 'e':
			re.WriteString(`\s*([-+]?\d+(?:\.\d+)?)`)
		case 'x', 'X':
			re.WriteString(`([0-9a-fA-F]+)`)
		case 'c':
			re.WriteString(`(.)`)
		default:
			if p.v.width != "" {
				re.WriteString(`\s*(.+?)\s*`)
			} else {
				re.WriteString(`(.+?)`)
			}
		}
	}
	re.WriteString(`$`)
	if t.weight == 0 {
		return nil, fmt.Errorf("template without text")
	}
	r, err := regexp.Compile(re.String())
	if err != nil {
		return nil, err
	}
	t.re = r
	return t, nil
}

func firstWord(s string) string {
	s = strings.TrimSpace(s)
	if i := strings.IndexAny(s, " \t\n"); i >= 0 {
		s = s[:i]
	}
	return strings.ToLower(s)
}

func (c *catalog) matchTemplate(s string, depth int) (string, bool) {
	c.mu.RLock()
	cands := append(append([]*tmpl(nil), c.buckets[firstWord(s)]...), c.loose...)
	c.mu.RUnlock()
	var best *tmpl
	var bestArgs []string
	for _, t := range cands {
		if best != nil && t.weight <= best.weight {
			continue
		}
		m := t.re.FindStringSubmatch(s)
		if m == nil {
			continue
		}
		ok := true
		for _, a := range m[1:] {
			if strings.TrimSpace(a) == "" {
				ok = false
			}
		}
		if ok {
			best, bestArgs = t, m[1:]
		}
	}
	if best == nil {
		return "", false
	}
	// captures are in source order; verbs may refer to arguments by index
	args := map[int]string{}
	kinds := map[int]byte{}
	next := 1
	for i, v := range best.src {
		idx := next
		if v.index > 0 {
			idx = v.index
		}
		next = idx + 1
		args[idx], kinds[idx] = strings.TrimSpace(bestArgs[i]), v.kind
	}
	var b strings.Builder
	next = 1
	for _, p := range best.out {
		if p.v == nil {
			b.WriteString(p.lit)
			continue
		}
		idx := next
		if p.v.index > 0 {
			idx = p.v.index
		}
		next = idx + 1
		arg, ok := args[idx]
		if !ok {
			continue
		}
		switch kinds[idx] {
		case 's', 'v', 'q':
			arg = c.tr(arg, depth+1)
		}
		spec := "%"
		if strings.Contains(p.v.flags, "-") {
			spec += "-"
		}
		b.WriteString(fmt.Sprintf(spec+p.v.width+"s", arg))
	}
	return b.String(), true
}

// ---- transliteration ----

var translit = map[rune]string{
	'а': "a", 'б': "b", 'в': "v", 'г': "g", 'д': "d", 'е': "e", 'ё': "yo", 'ж': "zh", 'з': "z",
	'и': "i", 'й': "y", 'к': "k", 'л': "l", 'м': "m", 'н': "n", 'о': "o", 'п': "p", 'р': "r",
	'с': "s", 'т': "t", 'у': "u", 'ф': "f", 'х': "kh", 'ц': "ts", 'ч': "ch", 'ш': "sh", 'щ': "shch",
	'ъ': "", 'ы': "y", 'ь': "", 'э': "e", 'ю': "yu", 'я': "ya", 'і': "i", 'ї': "yi", 'є': "ye",
}

// Translit writes Russian letters in Latin ones: names of people and
// villages read naturally ("Мирослава" → "Miroslava").
func Translit(s string) string {
	rs := []rune(s)
	var b strings.Builder
	for i, r := range rs {
		lo := unicode.ToLower(r)
		t, ok := translit[lo]
		if !ok {
			b.WriteRune(r)
			continue
		}
		if r == lo || t == "" {
			b.WriteString(t)
			continue
		}
		// a capital letter: whole word in capitals or just the first one
		caps := (i+1 < len(rs) && unicode.IsUpper(rs[i+1])) || (i > 0 && unicode.IsUpper(rs[i-1]))
		if caps {
			b.WriteString(strings.ToUpper(t))
		} else {
			b.WriteString(upperFirst(t))
		}
	}
	return b.String()
}

var placeholder = regexp.MustCompile(`\{[a-z_]+\}`)

// placeholders turns {player}-style names into indexed verbs, so that a line
// with the name already filled in matches its template.
func placeholders(k, v string) (string, string) {
	names := placeholder.FindAllString(k, -1)
	if len(names) == 0 {
		return k, v
	}
	k = strings.ReplaceAll(k, "%", "%%")
	v = strings.ReplaceAll(v, "%", "%%")
	idx := map[string]int{}
	for _, n := range names {
		if _, ok := idx[n]; !ok {
			idx[n] = len(idx) + 1
		}
	}
	for n, i := range idx {
		verb := fmt.Sprintf("%%[%d]s", i)
		k = strings.ReplaceAll(k, n, verb)
		v = strings.ReplaceAll(v, n, verb)
	}
	return k, v
}
