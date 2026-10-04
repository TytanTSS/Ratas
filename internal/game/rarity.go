package game

import (
	"math"
	"strings"

	"ratas/internal/content"
)

// Rarity of an item: the more rare, the more random bonuses (affixes), the
// stronger they roll and the stronger the item's own stats.
type Rarity int

const (
	Common Rarity = iota
	Uncommon
	Rare
	Epic
	Legendary
)

// RarityNames are the labels shown to players.
var RarityNames = [...]string{"Обычный", "Необычный", "Редкий", "Эпический", "Легендарный"}

// RarityColors color item names by rarity.
var RarityColors = [...]string{"#d8d8d8", "#5ee05e", "#4aa0ff", "#c070ff", "#ff9a2a"}

// rarityKeys are the names used in content files and admin commands.
var rarityKeys = [...]string{"common", "uncommon", "rare", "epic", "legendary"}

var (
	rarityBase  = [...]float64{1, 1.06, 1.14, 1.24, 1.38} // multiplier of the item's own stats and damage
	rarityPower = [...]float64{1, 1, 1.12, 1.3, 1.55}     // multiplier of the rolled bonuses
	rarityValue = [...]float64{1, 1.6, 2.6, 4.5, 8}       // multiplier of the price
)

// RarityName is the label of a rarity level, clamped to the known ones.
func RarityName(r int) string { return RarityNames[clampRarity(r)] }

// RarityColor is the name color of a rarity level.
func RarityColor(r int) string { return RarityColors[clampRarity(r)] }

func clampRarity(r int) int { return max(0, min(int(Legendary), r)) }

// RarityByName parses a rarity in English or Russian ("epic", "эпич"); -1 if unknown.
func RarityByName(s string) int {
	s = strings.ToLower(s)
	if s == "" {
		return -1
	}
	for i, k := range rarityKeys {
		if k == s {
			return i
		}
	}
	// "необычный" before "обычный": the first contains the second
	for _, i := range []int{1, 0, 2, 3, 4} {
		if n := strings.ToLower(RarityNames[i]); strings.HasPrefix(n, s) && len([]rune(s)) >= 3 {
			return i
		}
	}
	return -1
}

// defRarity is the rarity a content item has by itself: named artifacts are
// legendary unless the file says otherwise.
func defRarity(d *content.ItemDef) Rarity {
	if d == nil {
		return Common
	}
	for i, k := range rarityKeys {
		if d.Rarity == k {
			return Rarity(i)
		}
	}
	if d.Unique {
		return Legendary
	}
	return Common
}

// ItemRarity is the rarity shown for a stack: rolled or the item's own.
func (s ItemStack) ItemRarity() Rarity {
	r := Rarity(s.Rarity)
	if r == Common && len(s.Bonus) > 0 {
		r = Uncommon // a magic item from before rarities
	}
	return max(r, defRarity(s.Def()))
}

// statK is how much a rolled rarity strengthens the item's own stats.
func (s ItemStack) statK() float64 { return rarityBase[clampRarity(s.Rarity)] }

// rollRarityTier picks the rarity of a found item: magic percent of the
// finds are better than common, and deeper places give rarer things.
func (g *Game) rollRarityTier(depth int, magic float64, least Rarity) Rarity {
	r := Common
	if g.chance(magic) {
		d := float64(depth)
		leg := math.Min(8, 1.5+d*0.25)
		epic := math.Min(22, 6+d*0.8)
		rare := math.Min(40, 25+d)
		switch x := g.rng.Float64() * 100; {
		case x < leg:
			r = Legendary
		case x < leg+epic:
			r = Epic
		case x < leg+epic+rare:
			r = Rare
		default:
			r = Uncommon
		}
	}
	return max(r, least)
}

// rollRarity turns an equipment stack into one of the given rarity with as
// many random bonuses as the rarity level. The name gets the suffix of the
// first bonus only ("Long sword of Strength"), so it stays translatable.
func (g *Game) rollRarity(st ItemStack, r Rarity, depth int) ItemStack {
	d := st.Def()
	if d == nil || slotFor(d) == "" || d.Unique || r <= Common {
		return st
	}
	st.Rarity = int(r)
	st.Bonus = map[string]float64{}
	pool := g.rng.Perm(len(affixes))
	for _, i := range pool {
		if len(st.Bonus) >= int(r) {
			break
		}
		af := affixes[i]
		if !affixFits(af.stat, d) {
			continue
		}
		v := af.base * (1 + float64(depth)*0.4) * g.roll(0.8, 1.2) * rarityPower[r]
		if v >= 3 {
			v = math.Round(v)
		} else {
			v = math.Round(v*10) / 10
		}
		st.Bonus[af.stat] = v
		if st.Suffix == "" {
			st.Suffix = af.suffix // the name tells the first bonus, the color the rarity
		}
	}
	return st
}

// affixFits keeps bonuses sensible: weapon damage only on weapons and rings.
func affixFits(stat string, d *content.ItemDef) bool {
	if strings.HasPrefix(stat, "add_") || stat == "life_leech" {
		return d.Kind == "weapon" || d.Kind == "ring" || d.Kind == "offhand"
	}
	if stat == "thorns" {
		return d.Kind != "weapon"
	}
	return true
}
