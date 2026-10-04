package game

import (
	"compress/gzip"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"ratas/internal/content"
	"ratas/internal/gen"
	"ratas/internal/llm"
	"ratas/internal/world"
)

const SaveVersion = 2

// SaveData is the on-disk format: gzip-compressed JSON.
type SaveData struct {
	Version    int                 `json:"version"`
	Seed       int64               `json:"seed"`
	WorldName  string              `json:"world_name"`
	Now        float64             `json:"now"`
	NextID     EntityID            `json:"next_id"`
	QuestSeq   int                 `json:"quest_seq"`
	Start      world.Pos           `json:"start"`
	Villages   []VillageInfo       `json:"villages"`
	Entrances  []gen.Entrance      `json:"entrances"`
	Regions    []gen.Region        `json:"regions,omitempty"`
	RegionMap  []uint8             `json:"region_map,omitempty"`
	Landmarks  []gen.Landmark      `json:"landmarks,omitempty"`
	NoPvP      bool                `json:"no_pvp,omitempty"`
	Champions  map[string]EntityID `json:"champions,omitempty"`
	Chronicle  []string            `json:"chronicle,omitempty"`
	Levels     []*world.Level      `json:"levels"`
	Entities   []*Entity           `json:"entities"`
	Characters []*Entity           `json:"characters"`
	SavedAt    time.Time           `json:"saved_at"`
}

// Save writes the whole world (including every character ever seen) atomically.
func (g *Game) Save(path string) error {
	sd := SaveData{
		Version: SaveVersion, Seed: g.Seed, WorldName: g.WorldName, Now: g.Now, NextID: g.NextID,
		QuestSeq: g.questSeq, Start: g.Start, Villages: g.Villages, Entrances: g.Entrances, SavedAt: time.Now(),
		Regions: g.Regions, RegionMap: g.RegionMap, Landmarks: g.Landmarks,
		NoPvP: !g.PvP, Champions: g.Champions, Chronicle: g.Chronicle,
	}
	ids := make([]string, 0, len(g.Levels))
	for id := range g.Levels {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	for _, id := range ids {
		sd.Levels = append(sd.Levels, g.Levels[id])
	}
	for _, e := range g.Entities {
		if e.Kind == KProjectile || e.Kind == KPlayer {
			continue
		}
		sd.Entities = append(sd.Entities, e)
	}
	for _, e := range g.Online {
		sd.Characters = append(sd.Characters, e)
	}
	for _, e := range g.Offline {
		sd.Characters = append(sd.Characters, e)
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	tmp := path + ".tmp"
	f, err := os.Create(tmp)
	if err != nil {
		return err
	}
	zw := gzip.NewWriter(f)
	if err := json.NewEncoder(zw).Encode(&sd); err != nil {
		f.Close()
		return err
	}
	if err := zw.Close(); err != nil {
		f.Close()
		return err
	}
	if err := f.Close(); err != nil {
		return err
	}
	return os.Rename(tmp, path)
}

func readSave(path string) (*SaveData, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	zr, err := gzip.NewReader(f)
	if err != nil {
		return nil, err
	}
	var sd SaveData
	if err := json.NewDecoder(zr).Decode(&sd); err != nil {
		return nil, err
	}
	if sd.Version > SaveVersion {
		return nil, fmt.Errorf("сохранение из более новой версии игры (%d)", sd.Version)
	}
	return &sd, nil
}

// Load restores a world. All characters start offline and come back on Join.
func Load(path string, brain *llm.Brain) (*Game, error) {
	sd, err := readSave(path)
	if err != nil {
		return nil, err
	}
	g := newEmpty(brain)
	g.Seed, g.WorldName, g.Now, g.questSeq = sd.Seed, sd.WorldName, sd.Now, sd.QuestSeq
	g.Start, g.Villages, g.Entrances = sd.Start, sd.Villages, sd.Entrances
	g.Regions, g.RegionMap, g.Landmarks = sd.Regions, sd.RegionMap, sd.Landmarks
	g.PvP, g.Chronicle = !sd.NoPvP, sd.Chronicle
	if sd.Champions != nil {
		g.Champions = sd.Champions
	}
	for _, l := range sd.Levels {
		if len(l.Tiles) != l.W*l.H {
			return nil, fmt.Errorf("повреждён уровень %s", l.ID)
		}
		g.Levels[l.ID] = l
	}
	if g.Levels["overworld"] == nil {
		return nil, fmt.Errorf("в сохранении нет поверхности")
	}
	for _, e := range sd.Entities {
		if g.Levels[e.Level] == nil {
			continue
		}
		g.Spawn(e)
	}
	for _, e := range sd.Characters {
		if e.Player == nil {
			continue
		}
		if e.Player.Explored == nil {
			e.Player.Explored = map[string]world.Bitset{}
		}
		if e.Cooldowns == nil {
			e.Cooldowns = map[string]float64{}
		}
		if sd.Version < 2 {
			g.migrateHero(e)
		}
		if e.Player.Subclasses == nil {
			e.Player.Subclasses = map[string]string{}
		}
		e.Recalc()
		g.Offline[e.Player.Account] = e
	}
	g.NextID = max(g.NextID, sd.NextID)
	return g, nil
}

// oldClasses maps classes of the first version to the current ones.
var oldClasses = map[string]string{"ranger": "rogue", "paladin": "priest"}

// migrateHero converts a character from the first save version: new
// equipment slots, the new classes, and all skill points returned (the skill
// trees were rebuilt).
func (g *Game) migrateHero(e *Entity) {
	p := e.Player
	migrateEquip(p)
	if c, ok := oldClasses[p.Class]; ok {
		p.Class = c
	}
	if c := content.Class(p.Class); c != nil && c.Secret {
		p.Unlocks = append(p.Unlocks, "class:"+p.Class)
	}
	if content.Class(p.Class) == nil {
		p.Class = content.Classes()[0].Key
	}
	p.Classes = []string{p.Class}
	for _, r := range p.Skills {
		p.SkillPoints += r
	}
	p.Skills = map[string]int{}
	p.Abilities = nil
	p.Hotbar = [HotbarSize]string{}
	if c := content.Class(p.Class); c != nil {
		for _, a := range c.Abilities {
			g.unlockAbility(e, a)
		}
	}
}

type SaveInfo struct {
	Slot       string
	Path       string
	WorldName  string
	Seed       int64
	SavedAt    time.Time
	Characters []string // "Name (ур.N)"
	Names      []string
}

// ListSaves returns save files in dir, newest first.
func ListSaves(dir string) []SaveInfo {
	files, _ := filepath.Glob(filepath.Join(dir, "*.sav"))
	var out []SaveInfo
	for _, f := range files {
		sd, err := readSave(f)
		if err != nil {
			continue
		}
		info := SaveInfo{
			Slot: strings.TrimSuffix(filepath.Base(f), ".sav"), Path: f,
			WorldName: sd.WorldName, Seed: sd.Seed, SavedAt: sd.SavedAt,
		}
		for _, c := range sd.Characters {
			if c.Player != nil {
				info.Characters = append(info.Characters, fmt.Sprintf("%s (ур.%d)", c.Name, c.Player.Level))
				info.Names = append(info.Names, c.Player.Account)
			}
		}
		out = append(out, info)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].SavedAt.After(out[j].SavedAt) })
	return out
}
