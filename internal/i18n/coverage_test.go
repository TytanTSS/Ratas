package i18n

import (
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"testing"

	"github.com/BurntSushi/toml"
)

// Names of people and villages are transliterated, not translated; the
// keyboard layout table is not text.
var properNames = map[string]bool{"villageNames": true, "cityNames": true, "maleNames": true, "femaleNames": true, "moveLetters": true}

// sourceTexts collects the Russian text of the game: string literals in Go
// code (outside tests) and string values of the content and mod files.
func sourceTexts(t *testing.T) map[string]string {
	t.Helper()
	out := map[string]string{} // text → where
	root := filepath.Join("..", "..")
	add := func(s, where string) {
		s = strings.TrimSpace(s)
		if hasCyrillic(s) {
			if _, ok := out[s]; !ok {
				out[s] = where
			}
		}
	}
	err := filepath.Walk(root, func(p string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if info.IsDir() {
			switch info.Name() {
			case "i18n", ".git", "dist", "node_modules":
				return filepath.SkipDir
			}
			return nil
		}
		rel, _ := filepath.Rel(root, p)
		switch {
		case strings.HasSuffix(p, ".go") && !strings.HasSuffix(p, "_test.go"):
			fs := token.NewFileSet()
			f, err := parser.ParseFile(fs, p, nil, 0)
			if err != nil {
				return err
			}
			skip := map[ast.Node]bool{}
			for _, d := range f.Decls {
				if g, ok := d.(*ast.GenDecl); ok {
					for _, sp := range g.Specs {
						if vs, ok := sp.(*ast.ValueSpec); ok && len(vs.Names) > 0 && properNames[vs.Names[0].Name] {
							skip[vs] = true
						}
					}
				}
			}
			ast.Inspect(f, func(n ast.Node) bool {
				if skip[n] {
					return false
				}
				if bl, ok := n.(*ast.BasicLit); ok && bl.Kind == token.STRING {
					if s, err := strconv.Unquote(bl.Value); err == nil {
						add(s, fmt.Sprintf("%s:%d", rel, fs.Position(bl.Pos()).Line))
					}
				}
				return true
			})
		case strings.HasSuffix(p, ".toml") && (strings.Contains(rel, "content/data") || strings.Contains(rel, "mods_example")):
			var m map[string]any
			if _, err := toml.DecodeFile(p, &m); err != nil {
				return err
			}
			var walk func(v any)
			walk = func(v any) {
				switch x := v.(type) {
				case string:
					add(x, rel)
				case []any:
					for _, e := range x {
						walk(e)
					}
				case []map[string]any:
					for _, e := range x {
						walk(e)
					}
				case map[string]any:
					for _, e := range x {
						walk(e)
					}
				}
			}
			walk(m)
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	return out
}

// TestCatalogComplete: every Russian text of the game has an English
// translation. RATAS_I18N_MISSING=file writes the missing ones as TOML.
func TestCatalogComplete(t *testing.T) {
	texts := sourceTexts(t)
	var missing []string
	for s := range texts {
		if !Has(EN, s) {
			missing = append(missing, s)
		}
	}
	sort.Slice(missing, func(i, j int) bool { return texts[missing[i]] < texts[missing[j]] })
	if out := os.Getenv("RATAS_I18N_MISSING"); out != "" {
		var b strings.Builder
		last := ""
		for _, s := range missing {
			where := texts[s]
			if file, _, _ := strings.Cut(where, ":"); file != last {
				fmt.Fprintf(&b, "\n# %s\n", file)
				last = file
			}
			fmt.Fprintf(&b, "%s = \"\"\n", strconv.Quote(s))
		}
		os.WriteFile(out, []byte(b.String()), 0o644)
	}
	if len(missing) > 0 {
		n := min(len(missing), 30)
		for _, s := range missing[:n] {
			t.Errorf("no English for %q (%s)", s, texts[s])
		}
		t.Errorf("%d of %d texts have no English translation", len(missing), len(texts))
	}
}
