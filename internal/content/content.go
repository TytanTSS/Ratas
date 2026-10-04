// Package content holds all data-driven game definitions: tiles, monsters,
// items, abilities, the skill tree, classes and NPC roles.
//
// Built-in definitions are embedded from data/*.toml. Additional TOML files
// in a mods directory are merged on top: an entry with an existing key
// replaces the built-in one, a new key is appended. This is how the skill
// tree and everything else can be extended without touching Go code. Mods in
// the older JSON format (*.json) are still accepted.
package content

import (
	"embed"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"sort"
	"strings"
	"sync/atomic"
	"unicode/utf8"

	"github.com/BurntSushi/toml"
)

//go:embed data/*.toml
var builtin embed.FS

type TileDef struct {
	Key         string  `json:"key" toml:"key"`
	Name        string  `json:"name" toml:"name"`
	Glyph       string  `json:"glyph" toml:"glyph"`
	FG          string  `json:"fg" toml:"fg"`
	BG          string  `json:"bg,omitempty" toml:"bg,omitempty"`
	Walkable    bool    `json:"walkable" toml:"walkable"`
	Transparent bool    `json:"transparent" toml:"transparent"`
	MoveCost    float64 `json:"move_cost,omitempty" toml:"move_cost,omitempty"` // movement time multiplier, 0 means 1
	Biome       string  `json:"biome,omitempty" toml:"biome,omitempty"`         // used by the overworld spawner
	Interact    string  `json:"interact,omitempty" toml:"interact,omitempty"`   // door, chest, stairs_down, stairs_up, dungeon
	Becomes     string  `json:"becomes,omitempty" toml:"becomes,omitempty"`     // tile key after interaction
	Damage      float64 `json:"damage,omitempty" toml:"damage,omitempty"`       // damage per second while standing here
	DmgType     string  `json:"dmg_type,omitempty" toml:"dmg_type,omitempty"`   // type of that damage, fire by default
	Light       string  `json:"light,omitempty" toml:"light,omitempty"`         // colour of the light the tile gives off (graphics)

	ID   uint8 `json:"-" toml:"-"`
	Rune rune  `json:"-" toml:"-"`
}

// DamageTypeDef is one kind of damage. Every type has a resistance stat
// "res_<key>", a damage bonus "<key>_pct" and a flat weapon bonus "add_<key>".
type DamageTypeDef struct {
	Key     string `json:"key" toml:"key"`
	Name    string `json:"name" toml:"name"`   // "Огонь"
	Short   string `json:"short" toml:"short"` // compact label for the HUD
	Group   string `json:"group" toml:"group"` // physical, elemental or magic
	Color   string `json:"color" toml:"color"`
	ResName string `json:"res_name" toml:"res_name"` // "Сопр. огню"
	PctName string `json:"pct_name" toml:"pct_name"` // "Урон огнём %"
	AddName string `json:"add_name" toml:"add_name"` // "Урон огнём"
}

// DamageGroups name the groups usable in resistances: res_physical etc.
var DamageGroups = map[string]string{
	"physical":  "Сопр. физическому урону",
	"elemental": "Сопр. стихиям",
	"magic":     "Сопр. магии",
	"all":       "Сопр. всему урону",
}

type BuffDef struct {
	Key        string             `json:"key" toml:"key"`
	Name       string             `json:"name" toml:"name"`
	DurationMs int                `json:"duration_ms" toml:"duration_ms"`
	Stats      map[string]float64 `json:"stats,omitempty" toml:"stats,omitempty"`
	DotPerSec  float64            `json:"dot_per_sec,omitempty" toml:"dot_per_sec,omitempty"` // negative heals
	DmgType    string             `json:"dmg_type,omitempty" toml:"dmg_type,omitempty"`       // type of the damage over time
	Stun       bool               `json:"stun,omitempty" toml:"stun,omitempty"`               // the target cannot act
	Silence    bool               `json:"silence,omitempty" toml:"silence,omitempty"`         // the target cannot use abilities
	Stealth    bool               `json:"stealth,omitempty" toml:"stealth,omitempty"`         // enemies cannot see the bearer; ends on attack
	Taunt      bool               `json:"taunt,omitempty" toml:"taunt,omitempty"`             // the target must attack the source
	Color      string             `json:"color,omitempty" toml:"color,omitempty"`
}

type AbilityDef struct {
	Key        string     `json:"key" toml:"key"`
	Name       string     `json:"name" toml:"name"`
	Desc       string     `json:"desc" toml:"desc"`
	Kind       string     `json:"kind" toml:"kind"` // projectile, nova, heal, dash, cleave, strike, buff, chain
	Mana       float64    `json:"mana" toml:"mana"`
	CooldownMs int        `json:"cooldown_ms" toml:"cooldown_ms"`
	Damage     [2]float64 `json:"damage" toml:"damage"`
	DmgType    string     `json:"dmg_type,omitempty" toml:"dmg_type,omitempty"` // damage type; "weapon" (or empty for melee kinds) uses the weapon's
	Leech      float64    `json:"leech,omitempty" toml:"leech,omitempty"`       // percent of the damage dealt that heals the caster
	Scale      string     `json:"scale,omitempty" toml:"scale,omitempty"`       // str, dex, int
	ScaleK     float64    `json:"scale_k,omitempty" toml:"scale_k,omitempty"`
	Range      int        `json:"range,omitempty" toml:"range,omitempty"`
	Radius     int        `json:"radius,omitempty" toml:"radius,omitempty"`
	Count      int        `json:"count,omitempty" toml:"count,omitempty"` // projectiles per cast, jumps of a chain
	Glyph      string     `json:"glyph,omitempty" toml:"glyph,omitempty"`
	Color      string     `json:"color,omitempty" toml:"color,omitempty"`
	SpeedMs    int        `json:"speed_ms,omitempty" toml:"speed_ms,omitempty"` // projectile ms per tile
	Buff       *BuffDef   `json:"buff,omitempty" toml:"buff,omitempty"`         // applied to self (buff kind), to allies within radius
	OnHit      *BuffDef   `json:"on_hit,omitempty" toml:"on_hit,omitempty"`     // applied to targets hit
	Equip      string     `json:"equip,omitempty" toml:"equip,omitempty"`       // required gear: weapon, shield, twohand_dual, bow
	Split      []string   `json:"split,omitempty" toml:"split,omitempty"`       // the damage is shared equally between these types
	Execute    float64    `json:"execute,omitempty" toml:"execute,omitempty"`   // +% damage per % of health the target is missing
	Backstab   float64    `json:"backstab,omitempty" toml:"backstab,omitempty"` // damage multiplier from stealth or behind a distracted target
	Summon     string     `json:"summon,omitempty" toml:"summon,omitempty"`     // monster definition of a summoned ally (summon, decoy)
	Duration   int        `json:"duration_ms,omitempty" toml:"duration_ms,omitempty"`
}

type MonsterDef struct {
	Key       string             `json:"key" toml:"key"`
	Name      string             `json:"name" toml:"name"`
	Glyph     string             `json:"glyph" toml:"glyph"`
	Color     string             `json:"color" toml:"color"`
	HP        float64            `json:"hp" toml:"hp"`
	Damage    [2]float64         `json:"damage" toml:"damage"`
	Armor     float64            `json:"armor" toml:"armor"`
	MoveMs    int                `json:"move_ms" toml:"move_ms"`
	AttackMs  int                `json:"attack_ms" toml:"attack_ms"`
	XP        int                `json:"xp" toml:"xp"`
	Sight     int                `json:"sight" toml:"sight"`
	Behavior  string             `json:"behavior" toml:"behavior"`                     // melee, ranged, coward
	DmgType   string             `json:"dmg_type,omitempty" toml:"dmg_type,omitempty"` // type of its melee damage
	Resist    map[string]float64 `json:"resist,omitempty" toml:"resist,omitempty"`     // damage type, group or "all" -> percent
	OnHit     *BuffDef           `json:"on_hit,omitempty" toml:"on_hit,omitempty"`     // applied by its melee hits
	OnHitPct  float64            `json:"on_hit_pct,omitempty" toml:"on_hit_pct,omitempty"`
	Model     string             `json:"model,omitempty" toml:"model,omitempty"` // creature model in graphics mode
	Role      string             `json:"role,omitempty" toml:"role,omitempty"`   // frontline, skirmisher, ranged, caster, support, leader
	Ability   string             `json:"ability,omitempty" toml:"ability,omitempty"`
	Abilities []string           `json:"abilities,omitempty" toml:"abilities,omitempty"` // more abilities (heals, war cries...)
	Ally      bool               `json:"ally,omitempty" toml:"ally,omitempty"`           // fights on the players' side (NPC fighters, summons)
	Depth     [2]int             `json:"depth" toml:"depth"`                             // dungeon depth range, [0,0] = overworld
	Themes    []string           `json:"themes" toml:"themes"`                           // dungeon themes or overworld biomes
	Gold      [2]int             `json:"gold" toml:"gold"`
	Weight    int                `json:"weight" toml:"weight"`
	Group     [2]int             `json:"group" toml:"group"`
	Night     bool               `json:"night,omitempty" toml:"night,omitempty"` // overworld: only at night
	Elite     bool               `json:"elite,omitempty" toml:"elite,omitempty"` // may use the LLM tactician
	Boss      bool               `json:"boss,omitempty" toml:"boss,omitempty"`
	Persona   string             `json:"persona,omitempty" toml:"persona,omitempty"`
	Drops     []string           `json:"drops,omitempty" toml:"drops,omitempty"` // guaranteed item drops
}

type ItemDef struct {
	Key      string             `json:"key" toml:"key"`
	Name     string             `json:"name" toml:"name"`
	Glyph    string             `json:"glyph" toml:"glyph"`
	Color    string             `json:"color" toml:"color"`
	Kind     string             `json:"kind" toml:"kind"` // weapon, shield, offhand, head, chest, belt, legs, back, ring, consumable, quest, gold
	Stats    map[string]float64 `json:"stats,omitempty" toml:"stats,omitempty"`
	Damage   [2]float64         `json:"damage,omitempty" toml:"damage,omitempty"`
	Weapon   string             `json:"weapon,omitempty" toml:"weapon,omitempty"`     // sword, axe, mace, hammer, dagger, spear, staff, wand, bow, crossbow, scythe
	Hands    int                `json:"hands,omitempty" toml:"hands,omitempty"`       // 2: two-handed weapon
	Look     string             `json:"look,omitempty" toml:"look,omitempty"`         // how worn gear looks in graphics mode
	Unique   bool               `json:"unique,omitempty" toml:"unique,omitempty"`     // a named artifact
	Rarity   string             `json:"rarity,omitempty" toml:"rarity,omitempty"`     // common, uncommon, rare, epic, legendary (artifacts default to legendary)
	DmgType  string             `json:"dmg_type,omitempty" toml:"dmg_type,omitempty"` // weapon damage type
	OnHit    *BuffDef           `json:"on_hit,omitempty" toml:"on_hit,omitempty"`     // weapon: applied to targets hit
	OnHitPct float64            `json:"on_hit_pct,omitempty" toml:"on_hit_pct,omitempty"`
	Heal     float64            `json:"heal,omitempty" toml:"heal,omitempty"`
	Mana     float64            `json:"mana,omitempty" toml:"mana,omitempty"`
	Buff     *BuffDef           `json:"buff,omitempty" toml:"buff,omitempty"`     // consumable: applied to the user
	Effect   string             `json:"effect,omitempty" toml:"effect,omitempty"` // consumable: cleanse, return, respec, xp
	Amount   float64            `json:"amount,omitempty" toml:"amount,omitempty"` // effect strength
	Value    int                `json:"value" toml:"value"`
	Depth    int                `json:"depth" toml:"depth"`   // minimal dungeon depth to drop
	Weight   int                `json:"weight" toml:"weight"` // drop weight, 0 = never random
	Desc     string             `json:"desc,omitempty" toml:"desc,omitempty"`
	// DropFrom lists monsters (usually bosses) that drop this item with
	// DropChance percent (35 by default).
	DropFrom   []string `json:"drop_from,omitempty" toml:"drop_from,omitempty"`
	DropChance float64  `json:"drop_chance,omitempty" toml:"drop_chance,omitempty"`
}

type BranchDef struct {
	Key      string `json:"key" toml:"key"`
	Name     string `json:"name" toml:"name"`
	Color    string `json:"color" toml:"color"`
	Desc     string `json:"desc" toml:"desc"`
	Class    string `json:"class,omitempty" toml:"class,omitempty"`       // the class tree this branch belongs to ("" = common)
	Subclass string `json:"subclass,omitempty" toml:"subclass,omitempty"` // the subclass this branch belongs to
	Secret   bool   `json:"secret,omitempty" toml:"secret,omitempty"`     // skills are only granted by unique quests
	Hidden   bool   `json:"hidden,omitempty" toml:"hidden,omitempty"`     // hidden skills of a class, opened by deeds
}

// SubclassDef is a specialisation chosen inside a class once its class
// level is high enough; a class can have only one subclass.
type SubclassDef struct {
	Key    string `json:"key" toml:"key"`
	Name   string `json:"name" toml:"name"`
	Class  string `json:"class" toml:"class"`
	Desc   string `json:"desc" toml:"desc"`
	Color  string `json:"color" toml:"color"`
	Level  int    `json:"level,omitempty" toml:"level,omitempty"`   // class level needed, default 5
	Secret bool   `json:"secret,omitempty" toml:"secret,omitempty"` // must be unlocked by a unique quest
}

type SkillDef struct {
	Key      string             `json:"key" toml:"key"`
	Name     string             `json:"name" toml:"name"`
	Desc     string             `json:"desc" toml:"desc"`
	Branch   string             `json:"branch" toml:"branch"`
	Tier     int                `json:"tier" toml:"tier"`
	MaxRank  int                `json:"max_rank" toml:"max_rank"`
	Level    int                `json:"level" toml:"level"`                           // required character level
	Requires []string           `json:"requires,omitempty" toml:"requires,omitempty"` // skills that need rank >= 1
	Stats    map[string]float64 `json:"stats,omitempty" toml:"stats,omitempty"`       // per rank
	Grants   string             `json:"grants,omitempty" toml:"grants,omitempty"`     // ability unlocked at rank 1
	Equip    string             `json:"equip,omitempty" toml:"equip,omitempty"`       // the stats work only with this gear
	// A hidden skill opens by itself when its deed is done DeedCount times
	// (see game/deeds.go); its branch is hidden.
	Deed      string `json:"deed,omitempty" toml:"deed,omitempty"`
	DeedCount int    `json:"deed_count,omitempty" toml:"deed_count,omitempty"`
}

type ClassDef struct {
	Key       string             `json:"key" toml:"key"`
	Name      string             `json:"name" toml:"name"`
	Desc      string             `json:"desc" toml:"desc"`
	Color     string             `json:"color" toml:"color"`
	Attrs     map[string]float64 `json:"attrs" toml:"attrs"`
	Abilities []string           `json:"abilities,omitempty" toml:"abilities,omitempty"`
	Items     []string           `json:"items,omitempty" toml:"items,omitempty"`
	Model     string             `json:"model,omitempty" toml:"model,omitempty"`   // hero model in graphics mode
	Secret    bool               `json:"secret,omitempty" toml:"secret,omitempty"` // must be unlocked by a unique quest
}

type NPCRoleDef struct {
	Key        string   `json:"key" toml:"key"`
	Name       string   `json:"name" toml:"name"`
	Glyph      string   `json:"glyph" toml:"glyph"`
	Color      string   `json:"color" toml:"color"`
	Greetings  []string `json:"greetings" toml:"greetings"`
	Lines      []string `json:"lines" toml:"lines"`
	Trader     bool     `json:"trader,omitempty" toml:"trader,omitempty"`
	Goods      []string `json:"goods,omitempty" toml:"goods,omitempty"`
	QuestGiver bool     `json:"quest_giver,omitempty" toml:"quest_giver,omitempty"`
	Wander     int      `json:"wander" toml:"wander"`
	Persona    string   `json:"persona" toml:"persona"`
	Count      [2]int   `json:"count" toml:"count"` // how many per village (per world for wanderers)
	Model      string   `json:"model,omitempty" toml:"model,omitempty"`
	Combat     string   `json:"combat,omitempty" toml:"combat,omitempty"`   // ally monster definition: the NPC fights monsters
	World      bool     `json:"world,omitempty" toml:"world,omitempty"`     // wanders the open world instead of living in a village
	Stories    []string `json:"stories,omitempty" toml:"stories,omitempty"` // "tell me about yourself"
	// Cities: how many live in a city, the building they work in (temple,
	// townhall, tavern, armory, smithy, magic, alchemy, jewelry, barracks).
	CityCount [2]int `json:"city_count,omitempty" toml:"city_count,omitempty"`
	Building  string `json:"building,omitempty" toml:"building,omitempty"`
	// Stock is how many random items of StockKinds (item kinds or weapon
	// types) a trader also sells, rolled with rarities and renewed every day.
	Stock      int      `json:"stock,omitempty" toml:"stock,omitempty"`
	StockKinds []string `json:"stock_kinds,omitempty" toml:"stock_kinds,omitempty"`
	// Services offered in conversation: rest, upgrade, song, bless.
	Services []string `json:"services,omitempty" toml:"services,omitempty"`
}

// SquadDef is a mixed group of monsters that fight together: the frontline
// holds the enemy while archers and casters stay behind.
type SquadDef struct {
	Key     string        `json:"key" toml:"key"`
	Name    string        `json:"name" toml:"name"`
	Themes  []string      `json:"themes" toml:"themes"`
	Depth   [2]int        `json:"depth" toml:"depth"`
	Weight  int           `json:"weight" toml:"weight"`
	Night   bool          `json:"night,omitempty" toml:"night,omitempty"`
	Members []SquadMember `json:"members" toml:"members"`
}

type SquadMember struct {
	Monster string `json:"monster" toml:"monster"`
	Count   [2]int `json:"count" toml:"count"`
}

// UniqueDef is a one of a kind NPC somewhere in the wild with a quest whose
// reward is an artifact, a secret class or subclass, or a secret skill.
type UniqueDef struct {
	Key      string   `json:"key" toml:"key"`
	Name     string   `json:"name" toml:"name"`
	Title    string   `json:"title" toml:"title"`
	Color    string   `json:"color" toml:"color"`
	Model    string   `json:"model,omitempty" toml:"model,omitempty"`
	Persona  string   `json:"persona" toml:"persona"`
	Greeting string   `json:"greeting" toml:"greeting"`
	About    []string `json:"about" toml:"about"`
	Biomes   []string `json:"biomes" toml:"biomes"`
	Quest    string   `json:"quest" toml:"quest"` // slay, boss, relics
	Offer    string   `json:"offer" toml:"offer"`
	Done     string   `json:"done" toml:"done"`
	Target   string   `json:"target" toml:"target"`                         // slay: monster to empower; boss: boss key; relics: item key
	Champion string   `json:"champion,omitempty" toml:"champion,omitempty"` // slay: the champion's name
	Count    int      `json:"count,omitempty" toml:"count,omitempty"`       // relics to bring
	Sources  []string `json:"sources,omitempty" toml:"sources,omitempty"`   // relics: biomes and dungeon themes whose monsters carry them
	Reward   string   `json:"reward" toml:"reward"`                         // item:<key>, class:<key>, subclass:<key>, skill:<key>
}

// Bundle is the serializable set of all definitions. It is also what a
// server sends to joining clients so both sides agree on content.
type Bundle struct {
	DamageTypes []DamageTypeDef `json:"damage_types,omitempty" toml:"damage_types,omitempty"`
	Subclasses  []SubclassDef   `json:"subclasses,omitempty" toml:"subclasses,omitempty"`
	Squads      []SquadDef      `json:"squads,omitempty" toml:"squads,omitempty"`
	Uniques     []UniqueDef     `json:"uniques,omitempty" toml:"uniques,omitempty"`
	Tiles       []TileDef       `json:"tiles,omitempty" toml:"tiles,omitempty"`
	Monsters    []MonsterDef    `json:"monsters,omitempty" toml:"monsters,omitempty"`
	Items       []ItemDef       `json:"items,omitempty" toml:"items,omitempty"`
	Abilities   []AbilityDef    `json:"abilities,omitempty" toml:"abilities,omitempty"`
	Branches    []BranchDef     `json:"branches,omitempty" toml:"branches,omitempty"`
	Skills      []SkillDef      `json:"skills,omitempty" toml:"skills,omitempty"`
	Classes     []ClassDef      `json:"classes,omitempty" toml:"classes,omitempty"`
	NPCs        []NPCRoleDef    `json:"npcs,omitempty" toml:"npcs,omitempty"`
}

// DB is an indexed Bundle.
type DB struct {
	Bundle
	tileByKey map[string]*TileDef
	monsters  map[string]*MonsterDef
	items     map[string]*ItemDef
	abilities map[string]*AbilityDef
	skills    map[string]*SkillDef
	classes   map[string]*ClassDef
	npcs      map[string]*NPCRoleDef
	branches  map[string]*BranchDef
	dmgTypes  map[string]*DamageTypeDef
	subs      map[string]*SubclassDef
	squads    map[string]*SquadDef
	uniques   map[string]*UniqueDef
}

// active is swapped atomically: a network client installs the server's
// content while a graphical renderer may be reading it.
var active atomic.Pointer[DB]

// Use installs db as the active content set.
func Use(d *DB) { active.Store(d) }

// Get returns the active content set.
func Get() *DB { return active.Load() }

// LoadDefault loads built-in content and merges mod files from modsDir (if it exists).
func LoadDefault(modsDir string) (*DB, []string, error) {
	var b Bundle
	entries, err := builtin.ReadDir("data")
	if err != nil {
		return nil, nil, err
	}
	for _, e := range entries {
		data, err := builtin.ReadFile("data/" + e.Name())
		if err != nil {
			return nil, nil, err
		}
		part, err := decodeTOML(data)
		if err != nil {
			return nil, nil, fmt.Errorf("builtin %s: %w", e.Name(), err)
		}
		b.Merge(part)
	}
	var loaded []string
	if modsDir != "" {
		tomls, _ := filepath.Glob(filepath.Join(modsDir, "*.toml"))
		jsons, _ := filepath.Glob(filepath.Join(modsDir, "*.json"))
		files := append(tomls, jsons...)
		sort.Strings(files)
		for _, f := range files {
			data, err := os.ReadFile(f)
			if err != nil {
				return nil, nil, err
			}
			var part Bundle
			if filepath.Ext(f) == ".toml" {
				part, err = decodeTOML(data)
			} else {
				err = json.Unmarshal(data, &part)
			}
			if err != nil {
				return nil, nil, fmt.Errorf("mod %s: %w", filepath.Base(f), err)
			}
			b.Merge(part)
			loaded = append(loaded, filepath.Base(f))
		}
	}
	d, err := Index(b)
	return d, loaded, err
}

// decodeTOML parses a content file; unknown keys are errors, so that typos in
// mods do not silently do nothing.
func decodeTOML(data []byte) (Bundle, error) {
	var b Bundle
	md, err := toml.Decode(string(data), &b)
	if err != nil {
		return b, err
	}
	if und := md.Undecoded(); len(und) > 0 {
		keys := make([]string, len(und))
		for i, k := range und {
			keys[i] = k.String()
		}
		return b, fmt.Errorf("unknown keys: %s", strings.Join(keys, ", "))
	}
	return b, nil
}

// FromJSON builds a DB from a serialized Bundle (used by network clients).
func FromJSON(data []byte) (*DB, error) {
	var b Bundle
	if err := json.Unmarshal(data, &b); err != nil {
		return nil, err
	}
	return Index(b)
}

func (d *DB) JSON() []byte {
	data, _ := json.Marshal(d.Bundle)
	return data
}

func mergeByKey[T any](dst []T, src []T, key func(*T) string) []T {
	for i := range src {
		k := key(&src[i])
		replaced := false
		for j := range dst {
			if key(&dst[j]) == k {
				dst[j] = src[i]
				replaced = true
				break
			}
		}
		if !replaced {
			dst = append(dst, src[i])
		}
	}
	return dst
}

// Merge overlays o on b (same key = replace, new key = append).
func (b *Bundle) Merge(o Bundle) {
	b.DamageTypes = mergeByKey(b.DamageTypes, o.DamageTypes, func(t *DamageTypeDef) string { return t.Key })
	b.Subclasses = mergeByKey(b.Subclasses, o.Subclasses, func(t *SubclassDef) string { return t.Key })
	b.Squads = mergeByKey(b.Squads, o.Squads, func(t *SquadDef) string { return t.Key })
	b.Uniques = mergeByKey(b.Uniques, o.Uniques, func(t *UniqueDef) string { return t.Key })
	b.Tiles = mergeByKey(b.Tiles, o.Tiles, func(t *TileDef) string { return t.Key })
	b.Monsters = mergeByKey(b.Monsters, o.Monsters, func(t *MonsterDef) string { return t.Key })
	b.Items = mergeByKey(b.Items, o.Items, func(t *ItemDef) string { return t.Key })
	b.Abilities = mergeByKey(b.Abilities, o.Abilities, func(t *AbilityDef) string { return t.Key })
	b.Branches = mergeByKey(b.Branches, o.Branches, func(t *BranchDef) string { return t.Key })
	b.Skills = mergeByKey(b.Skills, o.Skills, func(t *SkillDef) string { return t.Key })
	b.Classes = mergeByKey(b.Classes, o.Classes, func(t *ClassDef) string { return t.Key })
	b.NPCs = mergeByKey(b.NPCs, o.NPCs, func(t *NPCRoleDef) string { return t.Key })
}

func firstRune(s string) rune {
	if s == "" {
		return '?'
	}
	r, _ := utf8.DecodeRuneInString(s)
	return r
}

// Index validates a bundle and builds lookup tables.
func Index(b Bundle) (*DB, error) {
	if len(b.Tiles) > 255 {
		return nil, fmt.Errorf("too many tiles: %d (max 255)", len(b.Tiles))
	}
	d := &DB{
		Bundle:    b,
		tileByKey: map[string]*TileDef{},
		monsters:  map[string]*MonsterDef{},
		items:     map[string]*ItemDef{},
		abilities: map[string]*AbilityDef{},
		skills:    map[string]*SkillDef{},
		classes:   map[string]*ClassDef{},
		npcs:      map[string]*NPCRoleDef{},
		branches:  map[string]*BranchDef{},
		dmgTypes:  map[string]*DamageTypeDef{},
	}
	d.subs, d.squads, d.uniques = map[string]*SubclassDef{}, map[string]*SquadDef{}, map[string]*UniqueDef{}
	for i := range d.Subclasses {
		sc := &d.Subclasses[i]
		if sc.Level <= 0 {
			sc.Level = 5
		}
		d.subs[sc.Key] = sc
	}
	for i := range d.Squads {
		d.squads[d.Squads[i].Key] = &d.Squads[i]
	}
	for i := range d.Uniques {
		d.uniques[d.Uniques[i].Key] = &d.Uniques[i]
	}
	for i := range d.Items {
		it := &d.Items[i]
		switch it.Kind { // older content
		case "armor":
			it.Kind = "chest"
		case "trinket":
			it.Kind = "ring"
		}
		if it.Kind == "weapon" && it.Hands == 0 {
			it.Hands = 1
		}
	}
	for i := range d.DamageTypes {
		d.dmgTypes[d.DamageTypes[i].Key] = &d.DamageTypes[i]
	}
	for i := range d.Tiles {
		t := &d.Tiles[i]
		t.ID = uint8(i)
		t.Rune = firstRune(t.Glyph)
		if t.MoveCost == 0 {
			t.MoveCost = 1
		}
		d.tileByKey[t.Key] = t
	}
	for i := range d.Monsters {
		d.monsters[d.Monsters[i].Key] = &d.Monsters[i]
	}
	for i := range d.Items {
		d.items[d.Items[i].Key] = &d.Items[i]
	}
	for i := range d.Abilities {
		d.abilities[d.Abilities[i].Key] = &d.Abilities[i]
	}
	for i := range d.Skills {
		s := &d.Skills[i]
		if s.MaxRank <= 0 {
			s.MaxRank = 1
		}
		d.skills[s.Key] = s
	}
	for i := range d.Classes {
		d.classes[d.Classes[i].Key] = &d.Classes[i]
	}
	for i := range d.NPCs {
		d.npcs[d.NPCs[i].Key] = &d.NPCs[i]
	}
	for i := range d.Branches {
		d.branches[d.Branches[i].Key] = &d.Branches[i]
	}
	var problems []string
	for _, s := range d.Skills {
		if _, ok := d.branches[s.Branch]; !ok {
			problems = append(problems, fmt.Sprintf("skill %q: unknown branch %q", s.Key, s.Branch))
		}
		for _, r := range s.Requires {
			if _, ok := d.skills[r]; !ok {
				problems = append(problems, fmt.Sprintf("skill %q: unknown requirement %q", s.Key, r))
			}
		}
		if s.Grants != "" {
			if _, ok := d.abilities[s.Grants]; !ok {
				problems = append(problems, fmt.Sprintf("skill %q: unknown ability %q", s.Key, s.Grants))
			}
		}
	}
	for _, t := range d.Tiles {
		if t.Becomes != "" {
			if _, ok := d.tileByKey[t.Becomes]; !ok {
				problems = append(problems, fmt.Sprintf("tile %q: unknown 'becomes' %q", t.Key, t.Becomes))
			}
		}
	}
	for _, m := range d.Monsters {
		if m.Ability != "" {
			if _, ok := d.abilities[m.Ability]; !ok {
				problems = append(problems, fmt.Sprintf("monster %q: unknown ability %q", m.Key, m.Ability))
			}
		}
		for _, it := range m.Drops {
			if _, ok := d.items[it]; !ok {
				problems = append(problems, fmt.Sprintf("monster %q: unknown drop %q", m.Key, it))
			}
		}
	}
	for _, it := range d.Items {
		for _, m := range it.DropFrom {
			if _, ok := d.monsters[m]; !ok {
				problems = append(problems, fmt.Sprintf("item %q: unknown monster in drop_from %q", it.Key, m))
			}
		}
		if it.Rarity != "" && !slices.Contains(Rarities, it.Rarity) {
			problems = append(problems, fmt.Sprintf("item %q: unknown rarity %q", it.Key, it.Rarity))
		}
	}
	for _, n := range d.NPCs {
		for _, g := range n.Goods {
			if _, ok := d.items[g]; !ok {
				problems = append(problems, fmt.Sprintf("npc %q: unknown good %q", n.Key, g))
			}
		}
	}
	for _, c := range d.Classes {
		for _, a := range c.Abilities {
			if _, ok := d.abilities[a]; !ok {
				problems = append(problems, fmt.Sprintf("class %q: unknown ability %q", c.Key, a))
			}
		}
		for _, it := range c.Items {
			if _, ok := d.items[it]; !ok {
				problems = append(problems, fmt.Sprintf("class %q: unknown item %q", c.Key, it))
			}
		}
	}
	problems = append(problems, d.validateEffects()...)
	if len(problems) > 0 {
		return nil, fmt.Errorf("content errors:\n  %s", strings.Join(problems, "\n  "))
	}
	return d, nil
}

// --- lookups on the active DB ---

func Tile(id uint8) *TileDef {
	d := active.Load()
	if int(id) < len(d.Tiles) {
		return &d.Tiles[id]
	}
	return &d.Tiles[0]
}

// TileID returns the id of a tile key; it panics on unknown keys because
// generators rely on the built-in tile set.
func TileID(key string) uint8 {
	t, ok := active.Load().tileByKey[key]
	if !ok {
		panic("unknown tile " + key)
	}
	return t.ID
}

func HasTile(key string) bool        { _, ok := active.Load().tileByKey[key]; return ok }
func Monster(key string) *MonsterDef { return active.Load().monsters[key] }
func Item(key string) *ItemDef       { return active.Load().items[key] }
func Ability(key string) *AbilityDef { return active.Load().abilities[key] }
func Skill(key string) *SkillDef     { return active.Load().skills[key] }
func Class(key string) *ClassDef     { return active.Load().classes[key] }
func NPCRole(key string) *NPCRoleDef { return active.Load().npcs[key] }
func Branch(key string) *BranchDef   { return active.Load().branches[key] }
func Classes() []ClassDef            { return active.Load().Classes }
func Skills() []SkillDef             { return active.Load().Skills }
func Branches() []BranchDef          { return active.Load().Branches }
func Monsters() []MonsterDef         { return active.Load().Monsters }
func Items() []ItemDef               { return active.Load().Items }
func NPCRoles() []NPCRoleDef         { return active.Load().NPCs }

func Subclass(key string) *SubclassDef { return active.Load().subs[key] }
func Subclasses() []SubclassDef        { return active.Load().Subclasses }
func Squads() []SquadDef               { return active.Load().Squads }
func Unique(key string) *UniqueDef     { return active.Load().uniques[key] }
func Uniques() []UniqueDef             { return active.Load().Uniques }

// SubclassesOf lists the subclasses of a class in definition order.
func SubclassesOf(class string) []*SubclassDef {
	var out []*SubclassDef
	d := active.Load()
	for i := range d.Subclasses {
		if d.Subclasses[i].Class == class {
			out = append(out, &d.Subclasses[i])
		}
	}
	return out
}

// DamageType returns a damage type definition or nil.
func DamageType(key string) *DamageTypeDef { return active.Load().dmgTypes[key] }

// DamageTypes lists all damage types in definition order.
func DamageTypes() []DamageTypeDef { return active.Load().DamageTypes }
