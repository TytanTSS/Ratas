package content

import (
	"fmt"
	"slices"
	"strings"
)

// BaseStats are the stat keys understood by the game besides the per damage
// type ones (res_<type|group|all>, <type>_pct, add_<type>).
var BaseStats = []string{
	"str", "dex", "int", "vit", "max_hp", "max_mp", "hp_regen", "mp_regen",
	"armor", "dodge", "crit", "crit_mult", "melee_pct", "spell_pct", "ranged_pct",
	"attack_speed", "move_speed", "sight", "gold_find", "life_leech", "thorns",
	"block", "fury", "duel_pct", "ambush_pct", "heal_pct", "mimic_pct", "reach",
}

// ItemKinds are the valid item kinds; equipment kinds are also slot names.
var ItemKinds = []string{"weapon", "shield", "offhand", "head", "chest", "belt", "legs", "back", "ring", "consumable", "quest", "gold"}

// EquipNeeds are the gear requirements of skills and abilities.
var EquipNeeds = []string{"", "weapon", "melee", "shield", "twohand_dual", "bow"}

// WeaponTypes are the kinds of weapons.
var WeaponTypes = []string{"sword", "axe", "mace", "hammer", "dagger", "spear", "staff", "wand", "bow", "crossbow", "scythe"}

// Effects are the special actions a consumable can perform.
var Effects = []string{"cleanse", "return", "respec", "xp"}

// KnownStat reports whether key is a stat the game understands.
func (d *DB) KnownStat(key string) bool {
	for _, k := range BaseStats {
		if k == key {
			return true
		}
	}
	switch {
	case strings.HasPrefix(key, "res_"):
		t := key[4:]
		_, group := DamageGroups[t]
		return group || d.dmgTypes[t] != nil
	case strings.HasPrefix(key, "add_"):
		return d.dmgTypes[key[4:]] != nil
	case strings.HasSuffix(key, "_pct"):
		return d.dmgTypes[strings.TrimSuffix(key, "_pct")] != nil
	}
	return false
}

// validateEffects checks damage types, stat keys and effects everywhere.
func (d *DB) validateEffects() []string {
	var problems []string
	bad := func(format string, args ...any) { problems = append(problems, fmt.Sprintf(format, args...)) }
	dmg := func(where, t string) {
		if t != "" && t != "weapon" && d.dmgTypes[t] == nil {
			bad("%s: unknown damage type %q", where, t)
		}
	}
	stats := func(where string, m map[string]float64) {
		for k := range m {
			if !d.KnownStat(k) {
				bad("%s: unknown stat %q", where, k)
			}
		}
	}
	buff := func(where string, b *BuffDef) {
		if b == nil {
			return
		}
		stats(where+" buff "+b.Key, b.Stats)
		dmg(where+" buff "+b.Key, b.DmgType)
	}
	for _, t := range d.DamageTypes {
		if _, ok := DamageGroups[t.Group]; !ok || t.Group == "all" {
			bad("damage type %q: unknown group %q", t.Key, t.Group)
		}
	}
	for _, t := range d.Tiles {
		dmg("tile "+t.Key, t.DmgType)
	}
	for _, a := range d.Abilities {
		dmg("ability "+a.Key, a.DmgType)
		buff("ability "+a.Key, a.Buff)
		buff("ability "+a.Key, a.OnHit)
	}
	for _, m := range d.Monsters {
		dmg("monster "+m.Key, m.DmgType)
		buff("monster "+m.Key, m.OnHit)
		for k := range m.Resist {
			if _, group := DamageGroups[k]; !group && d.dmgTypes[k] == nil {
				bad("monster %q: unknown resistance %q", m.Key, k)
			}
		}
	}
	for _, it := range d.Items {
		dmg("item "+it.Key, it.DmgType)
		stats("item "+it.Key, it.Stats)
		buff("item "+it.Key, it.OnHit)
		buff("item "+it.Key, it.Buff)
		if it.Effect != "" {
			ok := false
			for _, e := range Effects {
				ok = ok || e == it.Effect
			}
			if !ok {
				bad("item %q: unknown effect %q", it.Key, it.Effect)
			}
		}
	}
	for _, s := range d.Skills {
		stats("skill "+s.Key, s.Stats)
		if !slices.Contains(EquipNeeds, s.Equip) {
			bad("skill %q: unknown equip %q", s.Key, s.Equip)
		}
	}
	for _, a := range d.Abilities {
		if !slices.Contains(EquipNeeds, a.Equip) {
			bad("ability %q: unknown equip %q", a.Key, a.Equip)
		}
		for _, t := range a.Split {
			dmg("ability "+a.Key, t)
		}
		if a.Summon != "" && d.monsters[a.Summon] == nil {
			bad("ability %q: unknown summon %q", a.Key, a.Summon)
		}
	}
	for _, it := range d.Items {
		if !slices.Contains(ItemKinds, it.Kind) {
			bad("item %q: unknown kind %q", it.Key, it.Kind)
		}
		if it.Weapon != "" && !slices.Contains(WeaponTypes, it.Weapon) {
			bad("item %q: unknown weapon type %q", it.Key, it.Weapon)
		}
	}
	for _, sc := range d.Subclasses {
		if d.classes[sc.Class] == nil {
			bad("subclass %q: unknown class %q", sc.Key, sc.Class)
		}
	}
	for _, b := range d.Branches {
		if b.Class != "" && d.classes[b.Class] == nil {
			bad("branch %q: unknown class %q", b.Key, b.Class)
		}
		if sc := d.subs[b.Subclass]; b.Subclass != "" && (sc == nil || sc.Class != b.Class) {
			bad("branch %q: unknown subclass %q of class %q", b.Key, b.Subclass, b.Class)
		}
	}
	for _, m := range d.Monsters {
		for _, a := range m.Abilities {
			if d.abilities[a] == nil {
				bad("monster %q: unknown ability %q", m.Key, a)
			}
		}
		switch m.Role {
		case "", "frontline", "skirmisher", "ranged", "caster", "support", "leader":
		default:
			bad("monster %q: unknown role %q", m.Key, m.Role)
		}
	}
	for _, sq := range d.Squads {
		for _, mem := range sq.Members {
			if d.monsters[mem.Monster] == nil {
				bad("squad %q: unknown monster %q", sq.Key, mem.Monster)
			}
		}
	}
	for _, n := range d.NPCs {
		if n.Combat != "" && d.monsters[n.Combat] == nil {
			bad("npc %q: unknown combat profile %q", n.Key, n.Combat)
		}
	}
	for _, u := range d.Uniques {
		switch u.Quest {
		case "slay", "boss":
			if d.monsters[u.Target] == nil {
				bad("unique %q: unknown target monster %q", u.Key, u.Target)
			}
		case "relics":
			if d.items[u.Target] == nil {
				bad("unique %q: unknown relic %q", u.Key, u.Target)
			}
		default:
			bad("unique %q: unknown quest %q", u.Key, u.Quest)
		}
		kind, key, _ := strings.Cut(u.Reward, ":")
		ok := false
		switch kind {
		case "item":
			ok = d.items[key] != nil
		case "class":
			ok = d.classes[key] != nil
		case "subclass":
			ok = d.subs[key] != nil
		case "skill":
			ok = d.skills[key] != nil
		}
		if !ok {
			bad("unique %q: unknown reward %q", u.Key, u.Reward)
		}
	}
	for _, c := range d.Classes {
		for k := range c.Attrs {
			switch k {
			case "str", "dex", "int", "vit":
			default:
				bad("class %q: unknown attribute %q", c.Key, k)
			}
		}
	}
	return problems
}
