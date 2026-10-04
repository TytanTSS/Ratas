package game

import (
	"ratas/internal/content"
)

// Equipment slots.
const (
	SlotHead  = "head"
	SlotChest = "chest"
	SlotBelt  = "belt"
	SlotLegs  = "legs"
	SlotBack  = "back"
	SlotMain  = "main" // right hand
	SlotOff   = "off"  // left hand
	SlotRing1 = "ring1"
	SlotRing2 = "ring2"
	SlotRing3 = "ring3"
	SlotRing4 = "ring4"
)

// EquipSlots lists the slots in display order.
var EquipSlots = []string{SlotHead, SlotChest, SlotBelt, SlotLegs, SlotBack, SlotMain, SlotOff, SlotRing1, SlotRing2, SlotRing3, SlotRing4}

var ringSlots = []string{SlotRing1, SlotRing2, SlotRing3, SlotRing4}

// SlotNames are the slot labels shown to players.
var SlotNames = map[string]string{
	SlotHead: "Голова", SlotChest: "Грудь", SlotBelt: "Пояс", SlotLegs: "Ноги", SlotBack: "Спина",
	SlotMain: "Правая рука", SlotOff: "Левая рука",
	SlotRing1: "Кольцо 1", SlotRing2: "Кольцо 2", SlotRing3: "Кольцо 3", SlotRing4: "Кольцо 4",
}

// equippable reports whether an item can be worn at all.
func equippable(d *content.ItemDef) bool {
	if d == nil {
		return false
	}
	switch d.Kind {
	case "weapon", "shield", "offhand", "head", "chest", "belt", "legs", "back", "ring":
		return true
	}
	return false
}

// slotFor is the default slot of an item ("" = not equipment).
func slotFor(d *content.ItemDef) string {
	if !equippable(d) {
		return ""
	}
	switch d.Kind {
	case "weapon":
		return SlotMain
	case "shield", "offhand":
		return SlotOff
	case "ring":
		return SlotRing1
	}
	return d.Kind
}

func twoHanded(st ItemStack) bool {
	d := st.Def()
	return d != nil && d.Kind == "weapon" && d.Hands >= 2
}

// equip puts the item at inventory index idx on; left asks for the left hand
// (a second one-handed weapon or a shield).
func (g *Game) equip(e *Entity, idx int, left bool) {
	p := e.Player
	if idx < 0 || idx >= len(p.Inventory) {
		return
	}
	st := p.Inventory[idx]
	d := st.Def()
	if !equippable(d) {
		return
	}
	slot := slotFor(d)
	switch {
	case d.Kind == "ring":
		slot = ringSlots[0]
		for _, rs := range ringSlots {
			if p.Equip[rs].Key == "" {
				slot = rs
				break
			}
		}
	case d.Kind == "weapon" && left:
		if d.Hands >= 2 {
			g.Log(e, "#ff8080", "Двуручное оружие не взять в одну левую руку.")
			return
		}
		slot = SlotOff
	}
	p.Inventory = append(p.Inventory[:idx], p.Inventory[idx+1:]...)
	var freed []ItemStack
	if old, ok := p.Equip[slot]; ok && old.Key != "" {
		freed = append(freed, old)
	}
	switch {
	case slot == SlotMain && twoHanded(st):
		if off, ok := p.Equip[SlotOff]; ok && off.Key != "" {
			freed = append(freed, off)
			delete(p.Equip, SlotOff)
		}
	case slot == SlotOff && twoHanded(p.Equip[SlotMain]):
		freed = append(freed, p.Equip[SlotMain])
		delete(p.Equip, SlotMain)
	}
	p.Equip[slot] = st
	p.Inventory = append(p.Inventory, freed...)
	e.Recalc()
	p.Dirty = true
	where := SlotNames[slot]
	g.Log(e, "#c0c0ff", "Надето (%s): %s.", lower(where), st.Name())
	if len(p.Inventory) > InventorySize {
		// no room for what we took off: it falls to the ground
		for len(p.Inventory) > InventorySize {
			drop := p.Inventory[len(p.Inventory)-1]
			p.Inventory = p.Inventory[:len(p.Inventory)-1]
			g.dropItem(e.Level, e.Pos, drop)
			g.Log(e, "#ff8080", "Рюкзак полон — %s на земле.", drop.Name())
		}
	}
}

func (g *Game) unequip(e *Entity, slot string) {
	p := e.Player
	st, ok := p.Equip[slot]
	if !ok || st.Key == "" {
		return
	}
	if len(p.Inventory) >= InventorySize {
		g.Log(e, "#ff8080", "Инвентарь полон!")
		return
	}
	delete(p.Equip, slot)
	p.Inventory = append(p.Inventory, st)
	e.Recalc()
	p.Dirty = true
}

// migrateEquip moves items from the old three slots (weapon, armor, trinket)
// to the current ones.
func migrateEquip(p *PlayerState) {
	old := p.Equip
	p.Equip = map[string]ItemStack{}
	var spare []ItemStack
	for slot, st := range old {
		d := st.Def()
		if st.Key == "" {
			continue
		}
		if d == nil {
			spare = append(spare, st)
			continue
		}
		switch slot {
		case SlotHead, SlotChest, SlotBelt, SlotLegs, SlotBack, SlotMain, SlotOff, SlotRing1, SlotRing2, SlotRing3, SlotRing4:
			p.Equip[slot] = st
			continue
		}
		target := slotFor(d)
		if d.Kind == "ring" {
			for _, rs := range ringSlots {
				if p.Equip[rs].Key == "" {
					target = rs
					break
				}
			}
		}
		if target != "" && p.Equip[target].Key == "" {
			p.Equip[target] = st
		} else {
			spare = append(spare, st)
		}
	}
	p.Inventory = append(p.Inventory, spare...)
}

func lower(s string) string {
	r := []rune(s)
	if len(r) > 0 && r[0] >= 'А' && r[0] <= 'Я' {
		r[0] += 'а' - 'А'
	}
	return string(r)
}
