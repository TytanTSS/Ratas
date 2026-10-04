package gfx

import (
	"image/color"
	"strings"

	"ratas/internal/content"
)

// Heroes are drawn in what they wear: the class model gives the face, hair
// and colours, the equipment (right hand, left hand, head, chest, back)
// replaces weapons, armour, headgear and cloak.

const (
	gearMain = iota
	gearOff
	gearHead
	gearChest
	gearBack
)

func gearItem(gear []string, i int) *content.ItemDef {
	if i >= len(gear) || gear[i] == "" {
		return nil
	}
	return content.Item(gear[i])
}

// gearKey is the part of the model cache key that depends on equipment.
func gearKey(gear []string) string { return strings.Join(gear, ",") }

// dress puts a hero's equipment on the class model.
func dress(h hum, gear []string) hum {
	if len(gear) == 0 {
		return h
	}
	// weapons
	h.weapon, h.weaponCol = "", color.RGBA{}
	h.shield, h.shieldMark, h.shieldForm = color.RGBA{}, color.RGBA{}, ""
	h.offhand, h.offCol = "", color.RGBA{}
	if d := gearItem(gear, gearMain); d != nil && d.Weapon != "" {
		h.weapon, h.weaponCol = d.Weapon, weaponTint(d)
	}
	if d := gearItem(gear, gearOff); d != nil {
		switch {
		case d.Kind == "shield":
			h.shield, h.shieldForm = mul(hex(d.Color), 0.85), d.Look
			h.shieldMark = cGold
			if d.Look == "round" {
				h.shieldMark = color.RGBA{}
			}
		case d.Kind == "weapon" && d.Weapon != "":
			h.offhand, h.offCol = d.Weapon, weaponTint(d)
		case d.Kind == "offhand":
			h.offhand, h.offCol = d.Look, hex(d.Color)
			if h.offhand == "" {
				h.offhand = "orb"
			}
		}
	}
	// headgear: without it the hair shows
	mask := h.mask
	h.head, h.headCol, h.mask = "", color.RGBA{}, color.RGBA{}
	if d := gearItem(gear, gearHead); d != nil {
		h.head, h.headCol = d.Look, hex(d.Color)
		if h.head == "" {
			h.head = "cap"
		}
		if h.head == "hood" {
			h.mask = mask
		}
	}
	if h.hair.A == 0 && h.head != "skull" {
		h.hair = hairBrown
	}
	// body armour
	if d := gearItem(gear, gearChest); d != nil {
		c := hex(d.Color)
		switch d.Look {
		case "plate":
			h.outfit, h.top = "plate", c
			h.pants = mul(c, 0.7)
		case "chain":
			h.outfit, h.top = "plate", mix(c, cIron, 0.35)
			h.pants = mul(cIron, 0.9)
		case "robe":
			h.outfit, h.top = "robe", c
			h.boots = mul(c, 0.6)
		case "leather":
			h.outfit, h.top = "leather", c
			if h.pants.A == 0 {
				h.pants = mul(c, 0.65)
			}
		default:
			h.outfit, h.top = "tunic", c
		}
	} else {
		h.outfit, h.top, h.trim = "tunic", hex("#c8b898"), color.RGBA{}
		h.pants = hex("#6a5a48")
	}
	// cloak
	h.cape = color.RGBA{}
	if d := gearItem(gear, gearBack); d != nil {
		h.cape = hex(d.Color)
	}
	return h
}

// weaponTint colours blades with a hint of the item colour and gives
// staves and wands their gem.
func weaponTint(d *content.ItemDef) color.RGBA {
	c := hex(d.Color)
	switch d.Weapon {
	case "staff", "wand":
		return c
	case "bow":
		return color.RGBA{}
	}
	return mix(cSteel, c, 0.45)
}
