// Package world contains the tile map representation and spatial algorithms
// (field of view, line of sight, path finding) shared by server and client.
package world

import "ratas/internal/content"

type Pos struct{ X, Y int }

func (p Pos) Add(o Pos) Pos { return Pos{p.X + o.X, p.Y + o.Y} }

// Dist is the Chebyshev distance.
func (p Pos) Dist(o Pos) int { return max(abs(p.X-o.X), abs(p.Y-o.Y)) }

// Manhattan distance.
func (p Pos) Manhattan(o Pos) int { return abs(p.X-o.X) + abs(p.Y-o.Y) }

// DistSq is the squared euclidean distance.
func (p Pos) DistSq(o Pos) int {
	dx, dy := p.X-o.X, p.Y-o.Y
	return dx*dx + dy*dy
}

func abs(v int) int {
	if v < 0 {
		return -v
	}
	return v
}

// Dir is one of the four cardinal directions (movement is 4-directional).
type Dir uint8

const (
	DirNone Dir = iota
	DirUp
	DirRight
	DirDown
	DirLeft
)

var dirDelta = [...]Pos{{0, 0}, {0, -1}, {1, 0}, {0, 1}, {-1, 0}}

func (d Dir) Delta() Pos {
	if int(d) < len(dirDelta) {
		return dirDelta[d]
	}
	return Pos{}
}

func (d Dir) Opposite() Dir {
	switch d {
	case DirUp:
		return DirDown
	case DirDown:
		return DirUp
	case DirLeft:
		return DirRight
	case DirRight:
		return DirLeft
	}
	return DirNone
}

// DirTowards returns the dominant cardinal direction from a to b.
func DirTowards(a, b Pos) Dir {
	dx, dy := b.X-a.X, b.Y-a.Y
	if dx == 0 && dy == 0 {
		return DirNone
	}
	if abs(dx) >= abs(dy) {
		if dx > 0 {
			return DirRight
		}
		return DirLeft
	}
	if dy > 0 {
		return DirDown
	}
	return DirUp
}

var AllDirs = [4]Dir{DirUp, DirRight, DirDown, DirLeft}

// Level is one map: the overworld or a single dungeon floor.
type Level struct {
	ID      string  `json:"id"`
	Name    string  `json:"name"`
	W       int     `json:"w"`
	H       int     `json:"h"`
	Tiles   []uint8 `json:"tiles"`
	Depth   int     `json:"depth"`   // 0 = overworld
	Theme   string  `json:"theme"`   // crypt, cave, overworld
	Dungeon int     `json:"dungeon"` // entrance index, -1 for overworld
	Up      Pos     `json:"up"`
	Down    Pos     `json:"down"`
	Lit     bool    `json:"lit"` // affected by daylight

	occ []uint32 // blocking entity per cell, runtime only
}

func NewLevel(id, name string, w, h int, fill uint8) *Level {
	l := &Level{ID: id, Name: name, W: w, H: h, Tiles: make([]uint8, w*h), Dungeon: -1}
	for i := range l.Tiles {
		l.Tiles[i] = fill
	}
	return l
}

func (l *Level) In(x, y int) bool { return x >= 0 && y >= 0 && x < l.W && y < l.H }

func (l *Level) At(x, y int) uint8 {
	if !l.In(x, y) {
		return 0
	}
	return l.Tiles[y*l.W+x]
}

func (l *Level) Set(x, y int, t uint8) {
	if l.In(x, y) {
		l.Tiles[y*l.W+x] = t
	}
}

func (l *Level) Def(x, y int) *content.TileDef { return content.Tile(l.At(x, y)) }

func (l *Level) Walkable(x, y int) bool {
	return l.In(x, y) && content.Tile(l.Tiles[y*l.W+x]).Walkable
}

func (l *Level) Transparent(x, y int) bool {
	return l.In(x, y) && content.Tile(l.Tiles[y*l.W+x]).Transparent
}

func (l *Level) ensureOcc() {
	if len(l.occ) != l.W*l.H {
		l.occ = make([]uint32, l.W*l.H)
	}
}

// Occupant returns the blocking entity id at a cell (0 = none).
func (l *Level) Occupant(x, y int) uint32 {
	if !l.In(x, y) {
		return 0
	}
	l.ensureOcc()
	return l.occ[y*l.W+x]
}

func (l *Level) SetOccupant(x, y int, id uint32) {
	if !l.In(x, y) {
		return
	}
	l.ensureOcc()
	l.occ[y*l.W+x] = id
}

// ClearOccupant clears a cell only if it currently belongs to id.
func (l *Level) ClearOccupant(x, y int, id uint32) {
	if !l.In(x, y) {
		return
	}
	l.ensureOcc()
	if l.occ[y*l.W+x] == id {
		l.occ[y*l.W+x] = 0
	}
}

// Free reports whether a cell is walkable and has no blocking entity.
func (l *Level) Free(x, y int) bool { return l.Walkable(x, y) && l.Occupant(x, y) == 0 }

// Bitset is a compact boolean grid used for explored-map memory.
type Bitset []byte

func NewBitset(n int) Bitset    { return make(Bitset, (n+7)/8) }
func (b Bitset) Get(i int) bool { return i >= 0 && i/8 < len(b) && b[i/8]&(1<<(i%8)) != 0 }
func (b Bitset) Set(i int) {
	if i >= 0 && i/8 < len(b) {
		b[i/8] |= 1 << (i % 8)
	}
}
