// Package proto defines the client/server protocol. The same messages are
// used for single player (in-process pipe) and network play (TCP), so solo
// and multiplayer share one code path.
package proto

import (
	"bufio"
	"bytes"
	"compress/gzip"
	"encoding/gob"
	"io"
	"net"
	"sync"
	"time"
)

const Version = 4

// ---- client -> server ----

type Hello struct {
	Name    string
	Class   string
	Version int
	Lang    string // the player's language: AI characters answer in it
}

// Input is a real-time control event. Move is a world.Dir value.
type Input struct {
	Move     uint8
	Attack   bool
	Interact bool
	Ability  int8 // hotbar slot 1..6, 0 = none
	// Aim is the tile under the mouse cursor (graphics mode): attacks and
	// abilities go there instead of the nearest enemy.
	Aim        bool
	AimX, AimY int32
}

// Command is a discrete, non-real-time action (menus, dialogue, chat...).
type Command struct {
	Kind   string
	Key    string
	Index  int
	Text   string
	Target uint32
}

type ClientMsg struct {
	Hello *Hello
	Input *Input
	Cmd   *Command
}

// ---- server -> client ----

type Welcome struct {
	YouID     uint32
	NeedClass bool
	Content   []byte // JSON content bundle
	WorldName string
	Seed      int64
	AI        bool
	Host      bool
	Admin     bool // admin commands are allowed (testing mode)
}

type LevelData struct {
	ID       string
	Name     string
	W, H     int
	Tiles    []byte // gzip-compressed tile ids
	Explored []byte // bitset
	Lit      bool
	Depth    int
}

type TileChange struct {
	X, Y int
	T    uint8
}

type EntityView struct {
	ID      uint32
	X, Y    int
	Glyph   rune
	Color   string
	Kind    uint8
	Name    string
	HP      uint8 // percent
	Speech  string
	Hostile bool
	Boss    bool
	Def     string // monster, NPC role, class or item key (graphics models)
	Model   string // explicit model name from content, if any
	Status  uint16 // Status* bits
	Facing  uint8
	Dead    bool     // a fallen hero
	Ally    bool     // a party member or one of your summons
	Gear    []string // heroes: right hand, left hand, head, chest, back
	Step    uint16   // milliseconds one step takes at the current pace (smooth movement)
	Swing   uint8    // grows with every attack (swing animation)
	Rarity  uint8    // items on the ground: 0 common .. 4 legendary
}

// Status bits of an entity (visual effects).
const (
	StatusBurning uint16 = 1 << iota
	StatusPoisoned
	StatusChilled
	StatusStunned
	StatusCursed // a resistance lowered by a curse or mark
	StatusBleeding
	StatusShielded
	StatusHoly // burning with holy light
	StatusSilenced
	StatusStealth
	StatusIllusion
)

// PartyMember is what party members see of each other.
type PartyMember struct {
	Name      string
	Class     string
	Level     int
	HP, MaxHP float64
	MP, MaxMP float64
	Dead      bool
	Leader    bool
	Where     string // level name when not on your level
	X, Y      int
	LevelID   string
	Buffs     []BuffView
}

// Place is a named point for the world map.
type Place struct {
	Name string
	Kind string // village, dungeon, landmark kinds, region, quest
	X, Y int
}

// ClassView is the progress in one class.
type ClassView struct {
	Key      string
	Level    int
	Subclass string
}

// TargetView describes the enemy the player is fighting.
type TargetView struct {
	ID      uint32
	Name    string
	Color   string
	HP      uint8
	Level   int
	Boss    bool
	Res     map[string]int // effective resistances that are not zero
	Effects []BuffView
}

type FX struct {
	X, Y  int
	Text  string
	Glyph rune
	Color string
	Ms    int
}

type LogLine struct {
	Text  string
	Color string
}

type BuffView struct {
	Name  string
	Color string
	Left  int // ms
}

type SelfView struct {
	X, Y        int
	HP, MaxHP   float64
	MP, MaxMP   float64
	XP, XPNext  int
	Level       int
	Gold        int
	Cooldown    [6]float32 // remaining fraction per hotbar slot
	Buffs       []BuffView
	Dead        bool
	RespawnIn   int
	Vision      int
	Facing      uint8
	StandingOn  string // interaction hint
	AttrPoints  int
	SkillPoints int
	Region      string // overworld region name
	Target      *TargetView
	CanRise     bool // dead: the respawn button works
	Party       []PartyMember
	Invites     []string
	Copied      string // mimic: the copied ability
	CopiedLeft  int    // ms
	Safe        bool   // in a village: no fighting between players
	PvP         bool
}

type ItemView struct {
	Key    string
	Name   string
	Glyph  rune
	Color  string
	Kind   string
	Qty    int
	Value  int
	Desc   string
	Hands  int
	Rarity int8 // 0 common .. 4 legendary
}

type QuestView struct {
	Text   string
	Have   int
	Need   int
	Done   bool
	Giver  string
	Reward string
	Where  string // hint where to go
	X, Y   int
	Unique bool
}

// PlayerSheet is the full character state, sent when it changes.
type PlayerSheet struct {
	Name        string
	Class       string
	Level       int
	AttrPoints  int
	SkillPoints int
	Attrs       map[string]float64
	Stats       map[string]float64
	Skills      map[string]int
	Abilities   []string
	Hotbar      [6]string
	Inventory   []ItemView
	Equip       map[string]ItemView
	Quests      []QuestView
	Classes     []ClassView
	Unlocks     []string
	Places      []Place
	Deeds       map[string]int // deed counters (hidden skills)
	Found       int            // landmarks found
}

type Snapshot struct {
	Tick      uint64
	TimeOfDay float64
	Self      SelfView
	Entities  []EntityView
	FX        []FX
	Online    []string
}

type TradeItem struct {
	Item  ItemView
	Price int
}

type Dialogue struct {
	NPC     uint32
	Name    string
	Role    string
	Text    string
	Options []string
	Trade   []TradeItem
	AI      bool
	Waiting bool
	Close   bool
}

type ServerMsg struct {
	Welcome  *Welcome
	Level    *LevelData
	Tiles    []TileChange
	Snap     *Snapshot
	Sheet    *PlayerSheet
	Logs     []LogLine
	Dialogue *Dialogue
	Kick     string
}

// ---- transport ----

// Conn is a framed, gob-encoded bidirectional message stream.
type Conn struct {
	raw  net.Conn
	bw   *bufio.Writer
	enc  *gob.Encoder
	dec  *gob.Decoder
	wmu  sync.Mutex
	once sync.Once
}

func NewConn(c net.Conn) *Conn {
	bw := bufio.NewWriterSize(c, 64*1024)
	return &Conn{raw: c, bw: bw, enc: gob.NewEncoder(bw), dec: gob.NewDecoder(bufio.NewReaderSize(c, 64*1024))}
}

func (c *Conn) Send(m any) error {
	c.wmu.Lock()
	defer c.wmu.Unlock()
	if tc, ok := c.raw.(*net.TCPConn); ok {
		tc.SetWriteDeadline(time.Now().Add(10 * time.Second))
	}
	if err := c.enc.Encode(m); err != nil {
		return err
	}
	return c.bw.Flush()
}

func (c *Conn) RecvServer() (*ServerMsg, error) {
	var m ServerMsg
	err := c.dec.Decode(&m)
	return &m, err
}

func (c *Conn) RecvClient() (*ClientMsg, error) {
	var m ClientMsg
	err := c.dec.Decode(&m)
	return &m, err
}

func (c *Conn) Close() error {
	var err error
	c.once.Do(func() { err = c.raw.Close() })
	return err
}

func (c *Conn) RemoteAddr() string { return c.raw.RemoteAddr().String() }

// Compress gzips a byte slice.
func Compress(b []byte) []byte {
	var buf bytes.Buffer
	w, _ := gzip.NewWriterLevel(&buf, gzip.BestSpeed)
	w.Write(b)
	w.Close()
	return buf.Bytes()
}

// Decompress reverses Compress.
func Decompress(b []byte) ([]byte, error) {
	r, err := gzip.NewReader(bytes.NewReader(b))
	if err != nil {
		return nil, err
	}
	defer r.Close()
	return io.ReadAll(r)
}
