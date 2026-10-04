// Package game is the authoritative real-time simulation. It runs on a single
// goroutine (owned by the server) at a fixed tick rate; asynchronous work
// such as LLM calls reports back through Post.
package game

import (
	"math"
	"sort"
	"strings"

	"ratas/internal/content"
	"ratas/internal/world"
)

type EntityID = uint32

type Kind uint8

const (
	KPlayer Kind = iota + 1
	KMonster
	KNPC
	KItem
	KProjectile
)

type Faction uint8

const (
	FNeutral Faction = iota
	FPlayer
	FMonster
)

type ItemStack struct {
	Key    string             `json:"key"`
	Qty    int                `json:"qty,omitempty"`
	Bonus  map[string]float64 `json:"bonus,omitempty"`
	Suffix string             `json:"suffix,omitempty"`
}

func (s ItemStack) Def() *content.ItemDef { return content.Item(s.Key) }

func (s ItemStack) Name() string {
	d := s.Def()
	if d == nil {
		return s.Key
	}
	if s.Suffix != "" {
		return d.Name + " " + s.Suffix
	}
	return d.Name
}

func (s ItemStack) Value() int {
	d := s.Def()
	if d == nil {
		return 0
	}
	return d.Value + len(s.Bonus)*d.Value/2 + 10*len(s.Bonus)
}

type Buff struct {
	Def    content.BuffDef `json:"def"`
	Until  float64         `json:"until"`
	Source EntityID        `json:"source"`
}

type Entity struct {
	ID      EntityID  `json:"id"`
	Kind    Kind      `json:"kind"`
	Name    string    `json:"name"`
	Glyph   string    `json:"glyph"`
	Color   string    `json:"color"`
	Level   string    `json:"level"`
	Pos     world.Pos `json:"pos"`
	Facing  world.Dir `json:"facing"`
	Faction Faction   `json:"faction"`

	HP    float64 `json:"hp"`
	MaxHP float64 `json:"max_hp"`
	MP    float64 `json:"mp"`
	MaxMP float64 `json:"max_mp"`
	Dead  bool    `json:"dead,omitempty"`

	NextMove    float64            `json:"next_move"`
	NextAttack  float64            `json:"next_attack"`
	Cooldowns   map[string]float64 `json:"cooldowns,omitempty"`
	Buffs       []Buff             `json:"buffs,omitempty"`
	Speech      string             `json:"-"`
	SpeechUntil float64            `json:"-"`

	Monster *MonsterState `json:"monster,omitempty"`
	Player  *PlayerState  `json:"player,omitempty"`
	NPC     *NPCState     `json:"npc,omitempty"`
	Item    *ItemStack    `json:"item,omitempty"`
	Proj    *ProjState    `json:"-"`

	Owner   EntityID `json:"owner,omitempty"`   // summoned by this entity (fights on its side)
	Expires float64  `json:"expires,omitempty"` // summons and illusions vanish at this time

	stats Stats // derived, recomputed by Recalc
}

func (e *Entity) Blocks() bool {
	return e.Kind == KPlayer || e.Kind == KMonster || e.Kind == KNPC
}

func (e *Entity) Alive() bool { return !e.Dead && e.HP > 0 }

type MonsterState struct {
	Def         string      `json:"def"`
	Lvl         int         `json:"lvl"`
	Home        world.Pos   `json:"home"`
	Target      EntityID    `json:"-"`
	LastSeen    world.Pos   `json:"-"`
	LastSeenAt  float64     `json:"-"`
	Path        []world.Pos `json:"-"`
	PathGoal    world.Pos   `json:"-"`
	NextThink   float64     `json:"-"`
	State       string      `json:"state"`
	FleeUntil   float64     `json:"-"`
	Tactic      string      `json:"-"`
	TacticUntil float64     `json:"-"`
	NextLLM     float64     `json:"-"`
	LLMPending  bool        `json:"-"`
	Damage      [2]float64  `json:"damage"`
	Armor       float64     `json:"armor"`
	MoveMs      float64     `json:"move_ms"`
	AttackMs    float64     `json:"attack_ms"`
	XP          int         `json:"xp"`
	Persistent  bool        `json:"persistent,omitempty"` // never despawns
	Events      []string    `json:"-"`
	Champion    string      `json:"champion,omitempty"` // unique whose quest this champion is
	Squad       int         `json:"squad,omitempty"`    // members of one squad fight together
	RetreatAt   float64     `json:"-"`                  // last hit-and-run retreat
	BuffedAt    float64     `json:"-"`
	Retarget    float64     `json:"-"`
}

type Turn struct {
	Who  string `json:"who"`
	Text string `json:"text"`
}

type NPCState struct {
	Role        string             `json:"role"`
	PName       string             `json:"pname"`
	Home        world.Pos          `json:"home"`
	Village     string             `json:"village"`
	Gold        int                `json:"gold"`
	Memory      map[string][]Turn  `json:"memory,omitempty"`
	NextThink   float64            `json:"-"`
	NextChat    float64            `json:"-"`
	Busy        bool               `json:"-"`
	Gifts       map[string]float64 `json:"-"`                // player -> time of last gift
	Unique      string             `json:"unique,omitempty"` // a unique character of the world
	Met         map[string]int     `json:"met,omitempty"`    // conversations with each player
	Said        map[string]bool    `json:"-"`                // lines already told (avoid repeating)
	Travel      world.Pos          `json:"travel,omitempty"`
	TravelUntil float64            `json:"-"`
}

type ProjState struct {
	Owner   EntityID
	Faction Faction
	Path    []world.Pos
	Step    int
	StepMs  float64
	Next    float64
	Damage  Damage
	Radius  int
	OnHit   *content.BuffDef
	Ability string
}

// Stats are derived from attributes, equipment, skills and buffs.
type Stats struct {
	Str, Dex, Int, Vit float64
	MaxHP, MaxMP       float64
	HPRegen, MPRegen   float64
	Armor, Dodge       float64
	Crit, CritMult     float64
	MeleePct, SpellPct float64
	RangedPct          float64
	AttackSpeed        float64
	MoveSpeed          float64
	Sight              int
	GoldFind           float64
	WeaponDmg          [2]float64
	MoveMs, AttackMs   float64

	Mods        map[string]float64 // every modifier: attributes, gear, skills, buffs
	ResCap      float64            // highest possible resistance
	WeaponType  string             // damage type of melee attacks
	WeaponOnHit *content.BuffDef   // effect applied by melee hits
	OnHitPct    float64
	LifeLeech   float64 // percent of direct damage dealt returned as health
	Thorns      float64 // damage dealt back to melee attackers
	Stunned     bool
	Silenced    bool // cannot use abilities
	Stealthed   bool // invisible to enemies until it attacks
	Taunter     EntityID

	Block     float64 // percent chance to block most physical damage
	Fury      float64 // damage bonus at zero health, scaled by missing health
	DuelPct   float64 // damage bonus when only the target is near
	AmbushPct float64 // damage bonus from stealth or on a distracted target
	HealPct   float64 // healing bonus
	MimicPct  float64 // power of copied abilities
	Reach     int     // melee attacks reach this many tiles further

	Gear     Gear
	OffDmg   [2]float64 // second weapon (dual wield)
	OffType  string
	OffOnHit *content.BuffDef
	OffPct   float64
}

// Gear summarizes what a character holds; skills and abilities may need it.
type Gear struct {
	Weapon   string // weapon type in the right hand ("" = unarmed)
	TwoHand  bool
	Dual     bool
	Shield   bool
	Ranged   bool // bow or crossbow
	HasItems bool
}

// Has reports whether the gear satisfies a requirement.
func (g Gear) Has(need string) bool {
	switch need {
	case "":
		return true
	case "weapon":
		return g.Weapon != ""
	case "melee":
		return g.Weapon != "" && !g.Ranged
	case "shield":
		return g.Shield
	case "twohand_dual":
		return (g.TwoHand && !g.Ranged) || g.Dual
	case "bow":
		return g.Ranged
	}
	return false
}

// GearNeedName describes a requirement for messages.
func GearNeedName(need string) string {
	switch need {
	case "weapon":
		return "оружие в руке"
	case "melee":
		return "оружие ближнего боя"
	case "shield":
		return "щит"
	case "twohand_dual":
		return "двуручное оружие или два оружия"
	case "bow":
		return "лук или арбалет"
	}
	return need
}

// Resist is the effective resistance to a damage type in percent: its own
// stat plus its group and "all", clamped to [-100, ResCap].
func (s *Stats) Resist(t string) float64 {
	v := s.Mods["res_"+t] + s.Mods["res_all"]
	if d := content.DamageType(t); d != nil {
		v += s.Mods["res_"+d.Group]
	}
	return math.Max(-100, math.Min(s.ResCap, v))
}

// TypeBonus is the damage bonus in percent for a damage type.
func (s *Stats) TypeBonus(t string) float64 { return s.Mods[t+"_pct"] }

func addMods(dst map[string]float64, src map[string]float64, k float64) {
	for key, v := range src {
		dst[key] += v * k
	}
}

// Recalc recomputes derived stats; it keeps the HP/MP fraction when maxima change.
func (e *Entity) Recalc() {
	mods := map[string]float64{}
	s := &e.stats
	s.Stunned, s.Silenced, s.Stealthed, s.Taunter = false, false, false, 0
	for _, b := range e.Buffs {
		addMods(mods, b.Def.Stats, 1)
		s.Stunned = s.Stunned || b.Def.Stun
		s.Silenced = s.Silenced || b.Def.Silence
		s.Stealthed = s.Stealthed || b.Def.Stealth
		if b.Def.Taunt {
			s.Taunter = b.Source
		}
	}
	s.Mods = mods
	s.ResCap = 75
	s.WeaponType, s.WeaponOnHit, s.OnHitPct = "blunt", nil, 0
	s.Gear, s.OffDmg, s.OffType, s.OffOnHit, s.OffPct = Gear{}, [2]float64{}, "", nil, 0
	switch {
	case e.Player != nil:
		p := e.Player
		addMods(mods, p.Attrs, 1)
		s.WeaponDmg = [2]float64{1, 3}
		// gear first: some skills only work with certain equipment
		slots := make([]string, 0, len(p.Equip))
		for slot := range p.Equip {
			slots = append(slots, slot)
		}
		sort.Strings(slots)
		for _, slot := range slots {
			it := p.Equip[slot]
			d := it.Def()
			if d == nil {
				continue
			}
			s.Gear.HasItems = true
			addMods(mods, d.Stats, 1)
			addMods(mods, it.Bonus, 1)
			switch {
			case slot == SlotMain && d.Kind == "weapon":
				s.WeaponDmg = d.Damage
				if d.DmgType != "" {
					s.WeaponType = d.DmgType
				}
				s.WeaponOnHit, s.OnHitPct = d.OnHit, d.OnHitPct
				s.Gear.Weapon = d.Weapon
				if s.Gear.Weapon == "" {
					s.Gear.Weapon = "sword"
				}
				s.Gear.TwoHand = d.Hands >= 2
				s.Gear.Ranged = d.Weapon == "bow" || d.Weapon == "crossbow"
			case slot == SlotOff && d.Kind == "weapon":
				s.OffDmg, s.OffType = d.Damage, d.DmgType
				if s.OffType == "" {
					s.OffType = "blunt"
				}
				s.OffOnHit, s.OffPct = d.OnHit, d.OnHitPct
				s.Gear.Dual = true
			case slot == SlotOff && d.Kind == "shield":
				s.Gear.Shield = true
			}
		}
		if s.Gear.Weapon == "" && s.Gear.Dual {
			// a single weapon in the left hand still counts as a weapon
			s.Gear.Weapon, s.Gear.Dual = "dagger", false
			s.WeaponDmg, s.WeaponType = s.OffDmg, s.OffType
		}
		for key, rank := range p.Skills {
			if sd := content.Skill(key); sd != nil && s.Gear.Has(sd.Equip) {
				addMods(mods, sd.Stats, float64(rank))
			}
		}
		s.Str, s.Dex, s.Int, s.Vit = mods["str"], mods["dex"], mods["int"], mods["vit"]
		s.MaxHP = 40 + s.Vit*10 + float64(p.Level)*6 + mods["max_hp"]
		s.MaxMP = 15 + s.Int*6 + mods["max_mp"]
		s.HPRegen = 0.25 + s.Vit*0.06 + mods["hp_regen"]
		s.MPRegen = 0.5 + s.Int*0.08 + mods["mp_regen"]
		s.Armor = mods["armor"]
		s.Dodge = math.Min(50, s.Dex*0.3+mods["dodge"])
		s.Crit = math.Min(75, 5+s.Dex*0.5+mods["crit"])
		s.CritMult = 1.5 + mods["crit_mult"]
		s.MeleePct = mods["melee_pct"]
		s.SpellPct = mods["spell_pct"]
		s.RangedPct = mods["ranged_pct"]
		s.AttackSpeed = mods["attack_speed"] + s.Dex*0.8
		if s.Gear.Dual {
			s.AttackSpeed += 10
		}
		s.MoveSpeed = mods["move_speed"]
		s.Sight = int(mods["sight"])
		s.GoldFind = mods["gold_find"]
	case e.Monster != nil:
		m := e.Monster
		if def := content.Monster(m.Def); def != nil {
			for k, v := range def.Resist {
				mods["res_"+k] += v
			}
			if def.DmgType != "" {
				s.WeaponType = def.DmgType
			}
			s.WeaponOnHit, s.OnHitPct = def.OnHit, def.OnHitPct
		}
		s.ResCap = 100
		s.MaxHP = e.MaxHP
		s.Armor = m.Armor + mods["armor"]
		s.WeaponDmg = m.Damage
		s.MeleePct = mods["melee_pct"]
		s.AttackSpeed = mods["attack_speed"]
		s.MoveSpeed = mods["move_speed"]
		s.Crit = math.Max(0, 3+mods["crit"])
		s.CritMult = 1.5
		s.Dodge = mods["dodge"]
	default:
		s.MaxHP = e.MaxHP
		s.MoveSpeed = mods["move_speed"]
		s.AttackSpeed = mods["attack_speed"]
	}
	s.LifeLeech = mods["life_leech"]
	s.Thorns = mods["thorns"]
	s.Block = math.Min(75, mods["block"])
	s.Fury = mods["fury"]
	s.DuelPct = mods["duel_pct"]
	s.AmbushPct = mods["ambush_pct"]
	s.HealPct = mods["heal_pct"]
	s.MimicPct = mods["mimic_pct"]
	s.Reach = 0
	if e.Player != nil && s.Gear.Has("melee") {
		s.Reach = int(mods["reach"])
	}
	baseMove, baseAttack := 160.0, 650.0
	if e.Monster != nil {
		baseMove, baseAttack = e.Monster.MoveMs, e.Monster.AttackMs
	}
	if e.Kind == KNPC && e.Monster == nil {
		baseMove = 300
	}
	s.MoveMs = baseMove / math.Max(0.25, 1+s.MoveSpeed/100)
	s.AttackMs = baseAttack / math.Max(0.25, 1+s.AttackSpeed/100)
	if e.Player != nil {
		fracHP, fracMP := 1.0, 1.0
		if e.MaxHP > 0 {
			fracHP = e.HP / e.MaxHP
		}
		if e.MaxMP > 0 {
			fracMP = e.MP / e.MaxMP
		}
		e.MaxHP, e.MaxMP = s.MaxHP, s.MaxMP
		e.HP = math.Min(e.MaxHP, math.Max(0, fracHP*e.MaxHP))
		e.MP = math.Min(e.MaxMP, math.Max(0, fracMP*e.MaxMP))
	}
}

func (e *Entity) Stats() *Stats { return &e.stats }

// StatsMap exposes derived stats for the character sheet.
func (s *Stats) Map() map[string]float64 {
	m := map[string]float64{
		"str": s.Str, "dex": s.Dex, "int": s.Int, "vit": s.Vit,
		"max_hp": s.MaxHP, "max_mp": s.MaxMP, "hp_regen": s.HPRegen, "mp_regen": s.MPRegen,
		"armor": s.Armor, "dodge": s.Dodge, "crit": s.Crit, "crit_mult": s.CritMult,
		"melee_pct": s.MeleePct, "spell_pct": s.SpellPct, "ranged_pct": s.RangedPct,
		"attack_speed": s.AttackSpeed, "move_speed": s.MoveSpeed, "sight": float64(s.Sight),
		"gold_find": s.GoldFind, "dmg_min": s.WeaponDmg[0], "dmg_max": s.WeaponDmg[1],
		"life_leech": s.LifeLeech, "thorns": s.Thorns, "block": s.Block, "fury": s.Fury,
		"duel_pct": s.DuelPct, "ambush_pct": s.AmbushPct, "heal_pct": s.HealPct, "mimic_pct": s.MimicPct,
		"reach": float64(s.Reach),
	}
	for _, t := range content.DamageTypes() {
		m["res_"+t.Key] = s.Resist(t.Key)
		if v := s.TypeBonus(t.Key); v != 0 {
			m[t.Key+"_pct"] = v
		}
		if v := s.Mods["add_"+t.Key]; v != 0 {
			m["add_"+t.Key] = v
		}
	}
	return m
}

// StatNames gives human-readable names for stat keys (used in UI and item descriptions).
var StatNames = map[string]string{
	"str": "Сила", "dex": "Ловкость", "int": "Интеллект", "vit": "Выносливость",
	"max_hp": "Здоровье", "max_mp": "Мана", "hp_regen": "Реген. здоровья", "mp_regen": "Реген. маны",
	"armor": "Броня", "dodge": "Уклонение %", "crit": "Крит. шанс %", "crit_mult": "Крит. множитель",
	"melee_pct": "Урон ближн. %", "spell_pct": "Сила магии %", "ranged_pct": "Урон дальн. %",
	"attack_speed": "Скорость атаки %", "move_speed": "Скорость бега %", "sight": "Обзор",
	"gold_find": "Находка золота %", "life_leech": "Вампиризм %", "thorns": "Шипы",
	"block": "Блок %", "fury": "Ярость раненого %", "duel_pct": "Урон один на один %",
	"ambush_pct": "Урон из засады %", "heal_pct": "Сила лечения %", "mimic_pct": "Сила копий %",
	"reach": "Дальность удара",
}

// StatName is the human-readable name of any stat key, including the
// per damage type ones.
func StatName(key string) string {
	if n, ok := StatNames[key]; ok {
		return n
	}
	switch {
	case strings.HasPrefix(key, "res_"):
		t := key[4:]
		if n, ok := content.DamageGroups[t]; ok {
			return n + " %"
		}
		if d := content.DamageType(t); d != nil {
			return d.ResName + " %"
		}
	case strings.HasPrefix(key, "add_"):
		if d := content.DamageType(key[4:]); d != nil {
			return d.AddName
		}
	case strings.HasSuffix(key, "_pct"):
		if d := content.DamageType(strings.TrimSuffix(key, "_pct")); d != nil {
			return d.PctName
		}
	}
	return key
}

// DamageTypeName is a damage type name in lower case ("огонь").
func DamageTypeName(key string) string {
	if d := content.DamageType(key); d != nil {
		return strings.ToLower(d.Name)
	}
	return key
}
