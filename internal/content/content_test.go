package content

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestBuiltinContent(t *testing.T) {
	db, mods, err := LoadDefault("")
	if err != nil {
		t.Fatal(err)
	}
	if len(mods) != 0 || len(db.Tiles) < 20 || len(db.Skills) < 10 || len(db.Classes) < 3 {
		t.Fatalf("unexpected builtin content: %d tiles, %d skills", len(db.Tiles), len(db.Skills))
	}
}

func TestExampleMod(t *testing.T) {
	db, mods, err := LoadDefault(filepath.Join("..", "..", "mods_example"))
	if err != nil {
		t.Fatal(err)
	}
	Use(db)
	if len(mods) != 1 || Skill("plague") == nil || Class("necromancer") == nil || Monster("bone_golem") == nil || Branch("necromancy") == nil {
		t.Fatalf("mod not merged: %v", mods)
	}
	if Skill("toughness") == nil {
		t.Fatal("builtin content lost after merge")
	}
}

func TestModOverrideAndValidation(t *testing.T) {
	dir := t.TempDir()
	// override an existing skill
	os.WriteFile(filepath.Join(dir, "a.toml"), []byte(`
[[skills]]
key = "toughness"
name = "Сталь"
branch = "warrior"
tier = 1
max_rank = 10
level = 1
stats = { max_hp = 20 }
`), 0o644)
	db, _, err := LoadDefault(dir)
	if err != nil {
		t.Fatal(err)
	}
	Use(db)
	if s := Skill("toughness"); s.Name != "Сталь" || s.MaxRank != 10 || s.Stats["max_hp"] != 20 {
		t.Fatalf("override failed: %+v", s)
	}
	// broken references are reported
	os.WriteFile(filepath.Join(dir, "b.toml"), []byte(`
[[skills]]
key = "x"
name = "X"
branch = "nope"
tier = 1
requires = ["missing"]
`), 0o644)
	_, _, err = LoadDefault(dir)
	if err == nil || !strings.Contains(err.Error(), "unknown branch") || !strings.Contains(err.Error(), "unknown requirement") {
		t.Fatalf("expected validation errors, got %v", err)
	}
	os.Remove(filepath.Join(dir, "b.toml"))

	// a typo in a field name is an error, not a silently ignored key
	os.WriteFile(filepath.Join(dir, "c.toml"), []byte(`
[[monsters]]
key = "wolf"
resit = { fire = 10 }
`), 0o644)
	_, _, err = LoadDefault(dir)
	if err == nil || !strings.Contains(err.Error(), "c.toml") || !strings.Contains(err.Error(), "monsters.resit") {
		t.Fatalf("expected an unknown key error, got %v", err)
	}
	os.Remove(filepath.Join(dir, "c.toml"))

	// mods in the old JSON format still load
	os.WriteFile(filepath.Join(dir, "d.json"), []byte(`{"items":[{"key":"old_ring","name":"Старое кольцо","glyph":"=","color":"#ffffff","kind":"trinket","value":5,"depth":1,"weight":1}]}`), 0o644)
	db, mods, err := LoadDefault(dir)
	if err != nil {
		t.Fatal(err)
	}
	Use(db)
	if Item("old_ring") == nil || len(mods) != 2 {
		t.Fatalf("json mod not loaded: %v", mods)
	}
}

// Built-in files must not reuse keys: a later file would silently replace
// an entry of an earlier one (that is only meant for mods).
func TestBuiltinKeysUnique(t *testing.T) {
	entries, err := builtin.ReadDir("data")
	if err != nil {
		t.Fatal(err)
	}
	where := map[string]string{}
	for _, e := range entries {
		data, _ := builtin.ReadFile("data/" + e.Name())
		b, err := decodeTOML(data)
		if err != nil {
			t.Fatal(err)
		}
		check := func(kind, key string) {
			id := kind + " " + key
			if f, ok := where[id]; ok {
				t.Errorf("%s is defined in both %s and %s", id, f, e.Name())
			}
			where[id] = e.Name()
		}
		for _, x := range b.Items {
			check("item", x.Key)
		}
		for _, x := range b.Skills {
			check("skill", x.Key)
		}
		for _, x := range b.Abilities {
			check("ability", x.Key)
		}
		for _, x := range b.Monsters {
			check("monster", x.Key)
		}
		for _, x := range b.Classes {
			check("class", x.Key)
		}
		for _, x := range b.Subclasses {
			check("subclass", x.Key)
		}
		for _, x := range b.Branches {
			check("branch", x.Key)
		}
		for _, x := range b.Uniques {
			check("unique", x.Key)
		}
		for _, x := range b.NPCs {
			check("npc", x.Key)
		}
		for _, x := range b.Tiles {
			check("tile", x.Key)
		}
		for _, x := range b.Squads {
			check("squad", x.Key)
		}
	}
}
