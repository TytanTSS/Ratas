package game

import (
	"fmt"
	"math"
	"math/rand/v2"
	"strconv"
	"strings"

	"ratas/internal/content"
	"ratas/internal/gen"
	"ratas/internal/llm"
	"ratas/internal/proto"
	"ratas/internal/world"
)

const (
	TickMs     = 50.0
	TPS        = 20
	DayMs      = 12 * 60 * 1000.0
	OverworldW = 300
	OverworldH = 150
)

type VillageInfo struct {
	Name   string    `json:"name"`
	Center world.Pos `json:"center"`
	Area   gen.Rect  `json:"area"`
}

// Outbox collects per-player messages produced during a tick.
type Outbox struct {
	Logs     []proto.LogLine
	Dialogue *proto.Dialogue
}

type Game struct {
	Seed      int64
	WorldName string
	Now       float64 // game time in ms
	TickN     uint64
	Levels    map[string]*world.Level
	Villages  []VillageInfo
	Entrances []gen.Entrance
	Regions   []gen.Region
	RegionMap []uint8 // overworld cell -> 1-based region index
	Landmarks []gen.Landmark
	Start     world.Pos
	Entities  map[EntityID]*Entity
	NextID    EntityID
	Online    map[string]*Entity // account -> player entity in the world
	Offline   map[string]*Entity // characters of players not connected
	Brain     *llm.Brain
	Paused    bool
	PvP       bool                // players outside one party can hurt each other
	Champions map[string]EntityID // unique quest -> its champion
	Chronicle []string            // recent deeds the world talks about

	rng       *rand.Rand
	tasks     chan func()
	outbox    map[EntityID]*Outbox
	fx        map[string][]proto.FX
	tiles     map[string][]proto.TileChange
	nextSpawn float64
	questSeq  int
	parties   map[int]*Party
	partySeq  int
	squadSeq  int
	byLevel   map[string][]*Entity
	revivals  []pendingNPC
	regionAt  []world.Pos
}

func newEmpty(brain *llm.Brain) *Game {
	return &Game{
		Levels:    map[string]*world.Level{},
		Entities:  map[EntityID]*Entity{},
		Online:    map[string]*Entity{},
		Offline:   map[string]*Entity{},
		Brain:     brain,
		NextID:    1,
		PvP:       true,
		Champions: map[string]EntityID{},
		parties:   map[int]*Party{},
		rng:       rand.New(rand.NewPCG(rand.Uint64(), rand.Uint64())),
		tasks:     make(chan func(), 1024),
		outbox:    map[EntityID]*Outbox{},
		fx:        map[string][]proto.FX{},
		tiles:     map[string][]proto.TileChange{},
	}
}

// New generates a fresh world from a seed.
func New(seed int64, brain *llm.Brain) *Game {
	g := newEmpty(brain)
	g.Seed = seed
	ow := gen.GenerateOverworld(seed, OverworldW, OverworldH)
	g.WorldName = ow.Name
	g.Levels[ow.Level.ID] = ow.Level
	g.Start = ow.Start
	g.Entrances = ow.Entrances
	g.Regions, g.RegionMap, g.Landmarks = ow.Regions, ow.RegionMap, ow.Landmarks
	r := gen.RNG(seed, "population")
	for _, v := range ow.Villages {
		g.Villages = append(g.Villages, VillageInfo{Name: v.Name, Center: v.Center, Area: v.Area})
		for _, n := range v.NPCs {
			g.spawnNPC(n, v.Name, r)
		}
	}
	g.populateOverworld(r)
	g.populateLandmarks(r)
	g.populateWanderers(r)
	g.placeUniques(r)
	return g
}

// Post schedules fn to run on the game goroutine (safe from any goroutine).
func (g *Game) Post(fn func()) {
	select {
	case g.tasks <- fn:
	default:
	}
}

func (g *Game) drainTasks() {
	for {
		select {
		case fn := <-g.tasks:
			fn()
		default:
			return
		}
	}
}

// Tick advances the simulation by one fixed step.
func (g *Game) Tick() {
	g.drainTasks()
	if g.Paused {
		return
	}
	g.Now += TickMs
	g.TickN++
	g.indexLevels()
	active := map[string]bool{}
	for _, p := range g.Online {
		active[p.Level] = true
	}
	for _, e := range g.Entities {
		if !active[e.Level] {
			continue
		}
		switch e.Kind {
		case KPlayer:
			g.updatePlayer(e)
		case KMonster:
			g.updateMonster(e)
		case KNPC:
			g.updateNPC(e)
		case KProjectile:
			g.updateProjectile(e)
			continue
		case KItem:
			continue
		}
		if g.Entities[e.ID] == e {
			g.updateBuffs(e)
		}
	}
	if g.Now >= g.nextSpawn {
		g.nextSpawn = g.Now + 2000
		g.runSpawner()
		g.reviveNPCs()
	}
}

// Chronicle remembers a notable deed for NPC gossip.
func (g *Game) chronicle(format string, args ...any) {
	g.Chronicle = append(g.Chronicle, fmt.Sprintf(format, args...))
	if len(g.Chronicle) > 20 {
		g.Chronicle = g.Chronicle[len(g.Chronicle)-20:]
	}
}

// ---- entities ----

func (g *Game) Spawn(e *Entity) *Entity {
	if e.ID == 0 {
		e.ID = g.NextID
		g.NextID++
	} else if e.ID >= g.NextID {
		g.NextID = e.ID + 1
	}
	g.Entities[e.ID] = e
	if e.Blocks() {
		if l := g.Levels[e.Level]; l != nil {
			l.SetOccupant(e.Pos.X, e.Pos.Y, e.ID)
		}
	}
	if e.Cooldowns == nil {
		e.Cooldowns = map[string]float64{}
	}
	e.Recalc()
	return e
}

func (g *Game) Remove(e *Entity) {
	if l := g.Levels[e.Level]; l != nil && e.Blocks() {
		l.ClearOccupant(e.Pos.X, e.Pos.Y, e.ID)
	}
	delete(g.Entities, e.ID)
	delete(g.outbox, e.ID)
}

func (g *Game) moveEntity(e *Entity, to world.Pos) {
	if l := g.Levels[e.Level]; l != nil && e.Blocks() {
		l.ClearOccupant(e.Pos.X, e.Pos.Y, e.ID)
		l.SetOccupant(to.X, to.Y, e.ID)
	}
	e.Pos = to
}

func (g *Game) At(level string, p world.Pos) *Entity {
	l := g.Levels[level]
	if l == nil {
		return nil
	}
	if id := l.Occupant(p.X, p.Y); id != 0 {
		return g.Entities[id]
	}
	return nil
}

func (g *Game) playersOn(level string) []*Entity {
	var out []*Entity
	for _, p := range g.Online {
		if p.Level == level {
			out = append(out, p)
		}
	}
	return out
}

// findFree searches outward from p for a walkable unoccupied cell.
func findFree(l *world.Level, p world.Pos) world.Pos {
	for rad := 0; rad < 40; rad++ {
		for dy := -rad; dy <= rad; dy++ {
			for dx := -rad; dx <= rad; dx++ {
				if max(iabs(dx), iabs(dy)) != rad {
					continue
				}
				q := world.Pos{X: p.X + dx, Y: p.Y + dy}
				if l.Free(q.X, q.Y) && l.Def(q.X, q.Y).Interact == "" {
					return q
				}
			}
		}
	}
	return p
}

func iabs(v int) int {
	if v < 0 {
		return -v
	}
	return v
}

// ---- levels ----

func DungeonLevelID(idx, depth int) string { return fmt.Sprintf("d%d-%d", idx, depth) }

func parseDungeonID(id string) (idx, depth int, ok bool) {
	if !strings.HasPrefix(id, "d") {
		return 0, 0, false
	}
	parts := strings.SplitN(id[1:], "-", 2)
	if len(parts) != 2 {
		return 0, 0, false
	}
	a, err1 := strconv.Atoi(parts[0])
	b, err2 := strconv.Atoi(parts[1])
	return a, b, err1 == nil && err2 == nil
}

// Level returns a level, generating dungeon floors on first visit.
func (g *Game) Level(id string) *world.Level {
	if l, ok := g.Levels[id]; ok {
		return l
	}
	idx, depth, ok := parseDungeonID(id)
	if !ok || idx < 0 || idx >= len(g.Entrances) {
		return nil
	}
	ent := g.Entrances[idx]
	f := gen.GenerateDungeon(g.Seed, id, ent.Name, ent.Theme, idx, depth, ent.MaxDepth)
	g.Levels[id] = f.Level
	g.populateDungeon(f, ent)
	return f.Level
}

func (g *Game) SetTile(l *world.Level, x, y int, t uint8) {
	l.Set(x, y, t)
	g.tiles[l.ID] = append(g.tiles[l.ID], proto.TileChange{X: x, Y: y, T: t})
}

// ---- time ----

func (g *Game) TimeOfDay() float64 { return math.Mod(g.Now/DayMs+0.30, 1) }

// Daylight is 1 at noon and 0 at night with smooth dawn and dusk.
func (g *Game) Daylight() float64 {
	t := g.TimeOfDay()
	switch {
	case t >= 0.3 && t <= 0.7:
		return 1
	case t >= 0.85 || t <= 0.15:
		return 0
	case t < 0.3:
		return (t - 0.15) / 0.15
	default:
		return (0.85 - t) / 0.15
	}
}

func (g *Game) IsNight() bool { return g.Daylight() < 0.35 }

func (g *Game) TimeName() string {
	t := g.TimeOfDay()
	switch {
	case t < 0.2 || t >= 0.85:
		return "ночь"
	case t < 0.3:
		return "рассвет"
	case t < 0.45:
		return "утро"
	case t < 0.6:
		return "день"
	case t < 0.75:
		return "вечер"
	default:
		return "сумерки"
	}
}

func (g *Game) Vision(e *Entity) int {
	l := g.Levels[e.Level]
	r := 8
	if l != nil && l.Lit {
		r = 6 + int(math.Round(11*g.Daylight()))
	}
	return r + e.stats.Sight
}

// ---- messaging ----

func (g *Game) box(e *Entity) *Outbox {
	b := g.outbox[e.ID]
	if b == nil {
		b = &Outbox{}
		g.outbox[e.ID] = b
	}
	return b
}

func (g *Game) Log(e *Entity, color, format string, args ...any) {
	if e == nil || e.Kind != KPlayer {
		return
	}
	b := g.box(e)
	b.Logs = append(b.Logs, proto.LogLine{Text: fmt.Sprintf(format, args...), Color: color})
}

func (g *Game) LogLevel(level, color, format string, args ...any) {
	for _, p := range g.playersOn(level) {
		g.Log(p, color, format, args...)
	}
}

func (g *Game) LogAll(color, format string, args ...any) {
	for _, p := range g.Online {
		g.Log(p, color, format, args...)
	}
}

func (g *Game) FX(level string, p world.Pos, text string, glyph rune, color string, ms int) {
	g.fx[level] = append(g.fx[level], proto.FX{X: p.X, Y: p.Y, Text: text, Glyph: glyph, Color: color, Ms: ms})
}

func (g *Game) Say(e *Entity, text string, ms float64) {
	e.Speech = text
	e.SpeechUntil = g.Now + ms
}

// TakeOutbox returns and clears the queued messages for a player.
func (g *Game) TakeOutbox(e *Entity) *Outbox {
	b := g.outbox[e.ID]
	delete(g.outbox, e.ID)
	return b
}

// FrameFX returns effects and tile changes produced this tick on a level.
func (g *Game) FrameFX(level string) ([]proto.FX, []proto.TileChange) {
	return g.fx[level], g.tiles[level]
}

// EndFrame clears per-tick broadcast buffers.
func (g *Game) EndFrame() {
	clear(g.fx)
	clear(g.tiles)
}

func (g *Game) roll(lo, hi float64) float64 {
	if hi <= lo {
		return lo
	}
	return lo + g.rng.Float64()*(hi-lo)
}

func (g *Game) chance(pct float64) bool { return g.rng.Float64()*100 < pct }

func (g *Game) Rand() *rand.Rand { return g.rng }

func tileKey(l *world.Level, p world.Pos) string { return content.Tile(l.At(p.X, p.Y)).Key }
