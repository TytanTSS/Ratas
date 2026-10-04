package gfx

import (
	"image/color"
	"unicode"
)

// modelDef paints one model: either a humanoid description or a beast painter.
type modelDef struct {
	hum      func(c color.RGBA, variant int) hum
	beast    func(c color.RGBA, frame int) *pc
	float    bool // hovers above the ground
	variants int  // looks that differ between individuals (villagers)
}

func humanoid(f func(c color.RGBA, v int) hum) modelDef { return modelDef{hum: f} }
func beast(f func(c color.RGBA, frame int) *pc) modelDef {
	return modelDef{beast: f}
}

var (
	hairBrown = hex("#6a4224")
	hairBlack = hex("#22180f")
	hairBlond = hex("#e0c070")
	hairGrey  = hex("#b8b8b8")
)

func hero(f func(c color.RGBA) hum) modelDef {
	return humanoid(func(c color.RGBA, _ int) hum { return f(c) })
}

var modelDefs = map[string]modelDef{
	// ---- heroes (class keys) ----
	"warrior": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, outfit: "plate", top: cSteel, trim: mul(c, 0.85), pants: cIron, boots: hex("#4a4048"),
			head: "helmet", headCol: cSteel, weapon: "sword", shield: mul(c, 0.8), shieldMark: cGold}
	}),
	"ranger": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, hair: hairBrown, outfit: "leather", top: hex("#4a7a3a"), pants: hex("#5a4030"), boots: cLeather,
			head: "hood", headCol: hex("#3e6a32"), cape: hex("#35582c"), weapon: "bow"}
	}),
	"mage": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hairBlond, outfit: "robe", top: hex("#3a56b0"), trim: cGold, boots: hex("#26346a"),
			head: "wizard", headCol: hex("#3a56b0"), weapon: "staff", weaponCol: hex("#7ac0ff")}
	}),
	"paladin": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hairBlond, outfit: "plate", top: hex("#e4e4ee"), trim: cGold, pants: hex("#b0b0c0"),
			boots: hex("#8a8a9a"), cape: hex("#3a56b0"), weapon: "hammer", weaponCol: hex("#c8a040"), shield: hex("#f0f0f8"), shieldMark: cGold}
	}),
	"rogue": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, outfit: "leather", top: hex("#3c3c4a"), pants: hex("#2a2a34"), boots: hex("#1e1e26"),
			head: "hood", headCol: hex("#30303c"), mask: hex("#24242c"), weapon: "dagger", weaponCol: hex("#c8d0e0"), cape: hex("#26262e")}
	}),
	"shaman": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, hair: hairBlack, hairStyle: "long", outfit: "fur", top: hex("#8a6a48"), trim: c, pants: hex("#6a4a30"),
			head: "feathers", headCol: c, weapon: "totem", weaponCol: c}
	}),
	"warlock": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#d8c4d4"), outfit: "robe", top: hex("#3a1a4a"), trim: c, boots: hex("#24102e"),
			head: "hood", headCol: hex("#2c1238"), glow: true, eyes: hex("#e08aff"), weapon: "wand", weaponCol: hex("#e08aff")}
	}),
	"druid_hero": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, hair: hex("#6a4a2a"), hairStyle: "long", beard: hex("#6a4a2a"), outfit: "robe", top: hex("#4a6a32"),
			trim: c, boots: hex("#3a2a1a"), head: "horns", headCol: hex("#8a6a4a"), weapon: "staff", weaponCol: hex("#8ae05a"), cape: hex("#5a4a2a")}
	}),
	"necro_hero": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#d0d0c4"), hair: hairBlack, hairStyle: "long", outfit: "robe", top: hex("#20262a"), trim: c, boots: hex("#121614"),
			glow: true, eyes: c, weapon: "skullstaff", weaponCol: c, offhand: "book", offCol: hex("#2a3a2a"), cape: hex("#141a16")}
	}),
	"monk": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, outfit: "robe", top: hex("#e08a30"), trim: hex("#8a3a1a"), boots: hex("#6a3a1a"),
			weapon: "staff", weaponCol: hex("#a07a4a")}
	}),
	"skald": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hairBlond, hairStyle: "long", beard: hairBlond, outfit: "fur", top: hex("#7a5a8a"), trim: c,
			pants: hex("#4a3a2a"), head: "horned", headCol: cSteel, cape: hex("#5a2a4a"), weapon: "axe", shield: hex("#8a5a3a"), shieldForm: "round"}
	}),
	"brewer_hero": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hex("#5a3a2a"), hairStyle: "wild", outfit: "leather", top: hex("#4a6a4a"), trim: c,
			pants: hex("#3a3a2a"), mask: hex("#5a5a4a"), weapon: "dagger", offhand: "orb", offCol: c}
	}),
	"necromancer": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#c8c0b0"), outfit: "robe", top: hex("#28262e"), trim: cBone, boots: hex("#1a181e"),
			head: "hood", headCol: hex("#1c1a20"), glow: true, eyes: hex("#8aff7a"), weapon: "skullstaff", weaponCol: hex("#8aff7a")}
	}),

	// ---- villagers (NPC roles) ----
	"elder": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, beard: hex("#ececec"), outfit: "robe", top: hex("#9a825e"), trim: hex("#e0c890"), boots: hex("#6a5236"),
			weapon: "staff", weaponCol: hex("#8ae08a")}
	}),
	"merchant": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hairBrown, beard: hairBrown, outfit: "tunic", top: hex("#d0a040"), trim: hex("#8a3a2a"),
			pants: hex("#5a3a2a"), head: "cap", headCol: hex("#8a3a2a")}
	}),
	"smith": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, beard: hex("#3a2614"), outfit: "leather", top: hex("#6a4a2a"), pants: hex("#3a3a40"),
			weapon: "hammer", weaponCol: cIron}
	}),
	"healer": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hex("#8a5a3a"), hairStyle: "long", outfit: "robe", top: hex("#5a9a5a"), trim: cWhite,
			boots: hex("#3a6a3a")}
	}),
	"priest": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hex("#8a6a4a"), outfit: "robe", top: hex("#ece8dc"), trim: cGold, boots: hex("#b0a890"),
			weapon: "staff", weaponCol: cGold}
	}),
	"guard": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, outfit: "plate", top: hex("#9aa0b0"), trim: hex("#4a6aaa"), pants: cIron,
			head: "helmet", headCol: hex("#9aa0b0"), weapon: "spear", shield: hex("#4a6aaa"), shieldMark: cWhite}
	}),
	"mayor": hero(func(c color.RGBA) hum {
		return hum{build: "big", skin: cSkin, hair: hairGrey, beard: hairGrey, outfit: "robe", top: hex("#7a1e2a"), trim: cGold,
			boots: hex("#3a1a1a"), head: "cap", headCol: hex("#2a1a3a"), cape: hex("#e8e0d0")}
	}),
	"captain": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, beard: hex("#5a3a2a"), outfit: "plate", top: hex("#b0b8c8"), trim: cGold, pants: cIron,
			head: "helmet", headCol: hex("#c8d0e0"), cape: hex("#2a4a9a"), weapon: "sword", shield: hex("#2a4a9a"), shieldMark: cGold}
	}),
	"innkeeper": hero(func(c color.RGBA) hum {
		return hum{build: "big", skin: cSkin, hair: hex("#8a4a2a"), beard: hex("#8a4a2a"), outfit: "tunic", top: hex("#e8e0d0"),
			trim: hex("#8a5a3a"), pants: hex("#4a3a2a")}
	}),
	"bard": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hairBlond, hairStyle: "long", outfit: "tunic", top: hex("#c03a8a"), trim: cGold,
			pants: hex("#3a2a5a"), head: "feathers", headCol: hex("#ff80c0"), cape: hex("#5a2a7a")}
	}),
	"armorer": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, hair: hairBlack, beard: hairBlack, outfit: "plate", top: hex("#8a909a"), pants: hex("#3a3a40"),
			weapon: "hammer", weaponCol: cIron, shield: hex("#6a7080"), shieldForm: "round"}
	}),
	"enchanter": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hex("#e0e0f0"), hairStyle: "long", outfit: "robe", top: hex("#4a2a8a"), trim: hex("#c0a0ff"),
			boots: hex("#2a1a4a"), head: "circlet", headCol: hex("#c0a0ff"), weapon: "wand", weaponCol: hex("#e0c0ff"), offhand: "orb", offCol: hex("#b090ff")}
	}),
	"alchemist": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hex("#4a3a2a"), hairStyle: "wild", outfit: "leather", top: hex("#5a6a4a"), trim: hex("#80e0a0"),
			pants: hex("#3a3a2a"), mask: hex("#6a6a5a"), offhand: "orb", offCol: hex("#80e0a0")}
	}),
	"jeweler": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hairGrey, beard: hairGrey, outfit: "robe", top: hex("#3a2a5a"), trim: cGold, boots: hex("#2a1a3a"),
			head: "cap", headCol: hex("#8a2a4a")}
	}),
	"villager": {variants: 6, hum: func(c color.RGBA, v int) hum {
		tops := []string{"#8a6a4a", "#6a8a5a", "#a05a4a", "#5a6a9a", "#9a8a5a", "#7a5a7a"}
		hairs := []color.RGBA{hairBrown, hairBlond, hairBlack, hex("#8a4a2a"), hairGrey, hairBrown}
		h := hum{skin: []color.RGBA{cSkin, cSkinTan, cSkin, cSkinDark, cSkin, cSkinTan}[v%6], hair: hairs[v%6],
			outfit: "tunic", top: hex(tops[v%6]), pants: hex("#4a3a2a")}
		if v%2 == 1 {
			h.hairStyle = "long"
			h.outfit = "robe"
			h.boots = mul(h.top, 0.7)
		}
		return h
	}},

	// ---- humanoid monsters ----
	"bandit": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: cSkinTan, outfit: "leather", top: mix(hex("#6a4a2a"), c, 0.25), pants: hex("#4a3a2a"),
			head: "hood", headCol: hex("#8a2a24"), mask: hex("#3a2a24"), weapon: "sword"}
	}),
	"bandit_archer": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: cSkinTan, outfit: "leather", top: mix(hex("#6a5a2a"), c, 0.25), pants: hex("#4a3a2a"),
			head: "hood", headCol: hex("#7a5a2a"), mask: hex("#3a2a24"), weapon: "bow"}
	}),
	"bandit_chief": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: cSkinTan, hair: hairBlack, hairStyle: "wild", beard: hairBlack, outfit: "leather", top: hex("#5a3a2a"),
			trim: c, pants: hex("#3a2a20"), cape: hex("#9a2a24"), weapon: "axe"}
	}),
	"nomad": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: cSkinDark, outfit: "tunic", top: c, trim: hex("#a04a2a"), pants: hex("#b89060"),
			head: "turban", headCol: hex("#f0e4c8"), mask: hex("#c8a060"), weapon: "sword", weaponCol: hex("#e0e0e8")}
	}),
	"cultist": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: mul(cSkin, 0.55), outfit: "robe", top: mul(c, 0.7), trim: hex("#2a0a10"), boots: hex("#2a0a10"),
			head: "hood", headCol: mul(c, 0.6), glow: true, eyes: hex("#ff6a4a"), weapon: "dagger"}
	}),
	"dark_knight": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: hex("#3a3040"), outfit: "plate", top: hex("#4a3a5a"), trim: c, pants: hex("#2e2638"), boots: hex("#221a2a"),
			head: "horned", headCol: hex("#3a3048"), glow: true, eyes: hex("#d08aff"), weapon: "sword", weaponCol: hex("#b8a0e8"), cape: hex("#2a1a3a")}
	}),
	"goblin": humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "small", skin: c, outfit: "rags", top: hex("#6a4a2a"), pants: hex("#4a3a2a"), head: "ears",
			eyes: hex("#ff3a2a"), weapon: "dagger"}
	}),
	"goblin_archer": humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "small", skin: mix(c, hex("#7ac04a"), 0.5), outfit: "rags", top: hex("#5a5a2a"), head: "ears",
			eyes: hex("#ff3a2a"), weapon: "bow"}
	}),
	"goblin_shaman": humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "small", skin: hex("#7ac04a"), outfit: "rags", top: mul(c, 0.6), head: "feathers", headCol: c,
			eyes: hex("#ffd84a"), weapon: "skullstaff", weaponCol: hex("#fff06a")}
	}),
	"orc": humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "big", skin: c, hair: hairBlack, hairStyle: "pony", tusks: true, outfit: "leather", top: hex("#5a4030"),
			pants: hex("#3a3028"), weapon: "axe"}
	}),
	"orc_warlord": humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "big", skin: hex("#6a8a3a"), tusks: true, outfit: "plate", top: hex("#5a5a64"), trim: c,
			pants: hex("#3a3a40"), head: "horned", headCol: hex("#4a4a54"), weapon: "axe", weaponCol: hex("#d0d0dc"), cape: hex("#8a1a1a")}
	}),
	"cave_troll": humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "big", skin: c, tusks: true, outfit: "loin", pants: hex("#5a4030"), eyes: hex("#ffd84a"), weapon: "club"}
	}),
	"skeleton": humanoid(func(c color.RGBA, _ int) hum {
		return hum{outfit: "bones", head: "skull", weapon: "sword", weaponCol: hex("#a8a8a8"), shield: hex("#6a5440")}
	}),
	"wandering_skeleton": humanoid(func(c color.RGBA, _ int) hum {
		return hum{outfit: "bones", head: "skull", glow: true, eyes: hex("#7ad0ff"), weapon: "sword", weaponCol: hex("#9a9a9a")}
	}),
	"skeleton_archer": humanoid(func(c color.RGBA, _ int) hum {
		return hum{outfit: "bones", head: "skull", weapon: "bow"}
	}),
	"zombie": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: c, hair: hex("#2a2a1a"), hairStyle: "wild", outfit: "rags", top: hex("#4a5a6a"), pants: hex("#3a3a4a"),
			eyes: hex("#f0f0b0"), weapon: "claws"}
	}),
	"ghoul": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: c, outfit: "rags", top: hex("#4a3a3a"), pants: hex("#3a2a2a"), glow: true, eyes: hex("#ff4a4a"),
			tusks: true, weapon: "claws"}
	}),
	"mummy": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: mul(c, 0.9), outfit: "wrap", top: c, pants: mul(c, 0.85), boots: mul(c, 0.7), glow: true,
			eyes: hex("#7ad0ff"), mask: mul(c, 0.8)}
	}),
	"lich": humanoid(func(c color.RGBA, _ int) hum {
		return hum{outfit: "robe", top: hex("#4a1a5a"), trim: cGold, boots: hex("#2a0e34"), head: "skull", crown: true,
			glow: true, eyes: c, weapon: "skullstaff", weaponCol: c}
	}),
	"bog_witch": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: hex("#9ab070"), hair: hairGrey, hairStyle: "long", outfit: "robe", top: hex("#4a5a2a"), trim: c,
			head: "wizard", headCol: hex("#2a2a1a"), weapon: "staff", weaponCol: c}
	}),
	"lizardman": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: c, head: "snout", outfit: "leather", top: hex("#6a5a3a"), pants: mul(c, 0.8), boots: mul(c, 0.6),
			tail: c, weapon: "spear"}
	}),
	"harpy": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: hex("#f0c8b0"), hair: c, hairStyle: "long", outfit: "fur", top: mul(c, 0.85), wings: mul(c, 0.72), legs: "talons",
			eyes: hex("#ff3a6a"), weapon: "claws"}
	}),
	"pharaoh": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: hex("#d8b050"), outfit: "robe", top: hex("#f0e8d0"), trim: cGold, boots: hex("#3a5aaa"),
			head: "nemes", headCol: hex("#3a5aaa"), glow: true, eyes: hex("#7ad0ff"), weapon: "staff", weaponCol: hex("#7ad0ff")}
	}),
	"frost_giant": humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "huge", skin: mix(c, hex("#a8c8e0"), 0.5), beard: hex("#f4f8ff"), outfit: "fur", top: hex("#7a8aa0"),
			trim: hex("#c8d8e8"), pants: hex("#5a6a80"), head: "horned", headCol: hex("#6a7a90"), eyes: hex("#2a6aaa"), weapon: "hammer", weaponCol: hex("#c8e8ff")}
	}),
	"yeti": humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "big", skin: hex("#a8b8c8"), outfit: "fur", top: c, head: "hood", headCol: c, eyes: hex("#2a4a6a"),
			weapon: "claws"}
	}),
	"demon": humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "big", skin: c, outfit: "loin", pants: hex("#2a1a1a"), head: "horns", headCol: hex("#2a1a1a"),
			wings: mul(c, 0.5), tail: c, glow: true, eyes: hex("#ffd84a"), weapon: "claws"}
	}),
	"demon_lord": humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "huge", skin: mul(c, 0.85), outfit: "plate", top: hex("#2e1a26"), trim: c, pants: hex("#2a1a1a"),
			head: "horns", headCol: hex("#1a1014"), wings: hex("#4a0a1a"), tail: mul(c, 0.85), glow: true, eyes: hex("#ffd84a"),
			weapon: "sword", weaponCol: hex("#ff6a2a")}
	}),
	"fire_imp": humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "small", skin: c, outfit: "loin", pants: hex("#3a1a0a"), head: "horns", headCol: hex("#4a1a0a"),
			wings: hex("#a03a1a"), tail: c, glow: true, eyes: hex("#ffff8a"), weapon: "claws"}
	}),
	"efreet": humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "big", skin: c, hair: hex("#ffd84a"), hairStyle: "wild", outfit: "loin", top: mul(c, 0.75),
			pants: cGold, legs: "smoke", glow: true, eyes: hex("#fff8c0"), weapon: "sword", weaponCol: cGold}
	}),
	"golem": humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "big", skin: c, outfit: "golem", glow: true, eyes: hex("#7ad0ff")}
	}),
	"magma_golem": humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "big", skin: hex("#4a3a38"), outfit: "golem", trim: c, glow: true, eyes: hex("#ffd84a")}
	}),

	"bandit_thief": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: cSkinTan, outfit: "leather", top: mix(hex("#3a3444"), c, 0.3), pants: hex("#2a2630"), boots: hex("#1e1a22"),
			head: "hood", headCol: hex("#2e2a38"), mask: hex("#1e1a22"), weapon: "dagger", offhand: "dagger"}
	}),
	"bandit_mage": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: cSkinTan, outfit: "robe", top: hex("#5a2a2a"), trim: c, boots: hex("#3a1a1a"),
			head: "hood", headCol: hex("#8a2a24"), mask: hex("#3a2a24"), glow: true, eyes: c, weapon: "wand", weaponCol: c}
	}),
	"orc_shaman": humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "big", skin: hex("#6a9a3a"), tusks: true, outfit: "fur", top: hex("#6a5038"), trim: c,
			head: "feathers", headCol: hex("#8a3a2a"), eyes: hex("#ffd84a"), weapon: "totem", weaponCol: c}
	}),
	"skeleton_mage": humanoid(func(c color.RGBA, _ int) hum {
		return hum{outfit: "robe", top: hex("#2a3048"), trim: c, boots: hex("#1a1e30"), head: "skull", glow: true, eyes: c,
			weapon: "skullstaff", weaponCol: c}
	}),
	"illusion": humanoid(func(c color.RGBA, _ int) hum {
		return hum{skin: mix(cSkin, c, 0.4), hair: c, outfit: "tunic", top: c, pants: mul(c, 0.6), weapon: "dagger"}
	}),

	// ---- wanderers and warriors of the open world ----
	"knight_errant": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, outfit: "plate", top: hex("#c8ccd8"), trim: hex("#3a5aaa"), pants: cIron,
			head: "helmet", headCol: hex("#c8ccd8"), cape: hex("#2a4a9a"), weapon: "sword", weaponCol: hex("#e8eef8"),
			shield: hex("#3a5aaa"), shieldMark: cGold}
	}),
	"hunter": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, hair: hairBrown, beard: hairBrown, outfit: "leather", top: hex("#5a7a3a"), pants: hex("#5a4030"),
			boots: cLeather, head: "fur_hat", headCol: hex("#8a6a48"), cape: hex("#4a3a28"), weapon: "bow"}
	}),
	"pilgrim": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hairGrey, beard: hairGrey, outfit: "robe", top: hex("#a89878"), trim: hex("#e8d8a8"),
			boots: hex("#6a5a40"), head: "hood", headCol: hex("#8a7a5a"), weapon: "staff", weaponCol: hex("#fff0c0"), offhand: "symbol"}
	}),
	"mercenary": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, hair: hairBlack, hairStyle: "pony", beard: hairBlack, outfit: "leather", top: hex("#6a5440"),
			trim: hex("#a07a48"), pants: hex("#3a3a40"), weapon: "axe", offhand: "axe"}
	}),
	"battle_mage": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hairBlond, outfit: "plate", top: hex("#4a5a9a"), trim: hex("#8ab0ff"), pants: hex("#2a3460"),
			head: "circlet", headCol: cSteel, cape: hex("#2a3a7a"), weapon: "sword", weaponCol: hex("#a8d0ff"), offhand: "orb", offCol: hex("#8ab0ff")}
	}),

	// ---- unique characters ----
	"sun_hermit": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, beard: hex("#f4f0e0"), outfit: "robe", top: hex("#e8d8a8"), trim: cGold, boots: hex("#a08050"),
			head: "hood", headCol: hex("#d8b870"), weapon: "staff", weaponCol: hex("#ffd84a")}
	}),
	"faceless": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#e0e0e8"), outfit: "leather", top: hex("#5a5a6a"), pants: hex("#3a3a46"), head: "hood",
			headCol: hex("#6a6a7a"), mask: hex("#f0f0f4"), glow: true, eyes: c, cape: hex("#4a4a58")}
	}),
	"seal_keeper": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, beard: hairGrey, outfit: "robe", top: hex("#2a3a5a"), trim: hex("#7ae0ff"), boots: hex("#1a2438"),
			head: "wizard", headCol: hex("#1e2a44"), weapon: "staff", weaponCol: hex("#7ae0ff"), offhand: "book", offCol: hex("#2a4a6a")}
	}),
	"death_herald": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#d8d0d8"), outfit: "robe", top: hex("#1e1a24"), trim: c, boots: hex("#141018"),
			head: "hood", headCol: hex("#18141e"), glow: true, eyes: hex("#c08aff"), weapon: "scythe", weaponCol: hex("#d8d0e8")}
	}),
	"old_shaman": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, hair: hairGrey, hairStyle: "long", beard: hairGrey, outfit: "fur", top: hex("#7a5a3a"), trim: c,
			head: "feathers", headCol: c, weapon: "totem", weaponCol: c}
	}),
	"dark_pilgrim": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#c8b0b8"), outfit: "robe", top: hex("#4a1a24"), trim: c, boots: hex("#2a0e14"),
			head: "hood", headCol: hex("#3a1018"), glow: true, eyes: hex("#ff6a4a"), weapon: "wand", weaponCol: c, offhand: "book", offCol: hex("#3a0a10")}
	}),
	"stargazer": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hairGrey, hairStyle: "long", beard: hex("#e8e8f0"), outfit: "robe", top: hex("#1e2a5a"), trim: cGold,
			boots: hex("#141a3a"), head: "wizard", headCol: hex("#1a2450"), weapon: "staff", weaponCol: hex("#f0f4ff"), offhand: "orb", offCol: c}
	}),
	"giant_healer": hero(func(c color.RGBA) hum {
		return hum{build: "big", skin: cSkin, hair: hex("#c05a2a"), hairStyle: "wild", beard: hex("#c05a2a"), outfit: "fur",
			top: hex("#8a7a5a"), trim: hex("#5aa05a"), weapon: "staff", weaponCol: hex("#8ae08a")}
	}),
	"night_sister": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#e8d8e0"), hair: hairBlack, hairStyle: "long", outfit: "robe", top: hex("#2a1a3a"), trim: c,
			boots: hex("#1a1024"), mask: hex("#1a1024"), weapon: "dagger", weaponCol: c, offhand: "dagger", offCol: c}
	}),
	"word_master": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, beard: hex("#f4f4f4"), outfit: "robe", top: hex("#7a5a3a"), trim: cGold, boots: hex("#4a3420"),
			head: "cap", headCol: hex("#5a3a2a"), weapon: "staff", weaponCol: c, offhand: "book", offCol: hex("#8a2a2a")}
	}),
	"runner": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, hair: hairBlond, hairStyle: "wild", outfit: "leather", top: hex("#a08a5a"), pants: hex("#6a5a3a"),
			boots: hex("#5a3a20"), cape: mul(c, 0.8), weapon: "dagger"}
	}),
	"hermit_smith": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, beard: hairGrey, outfit: "leather", top: hex("#5a4030"), pants: hex("#3a3a40"),
			head: "cap", headCol: hex("#4a3a2a"), weapon: "hammer", weaponCol: cIron}
	}),
	"ice_keeper": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#e8f0f8"), hair: hex("#f4f8ff"), hairStyle: "long", outfit: "robe", top: hex("#a8c8e8"), trim: hex("#e8f4ff"),
			boots: hex("#6a8aa8"), head: "circlet", headCol: hex("#d0e8ff"), weapon: "staff", weaponCol: hex("#c8f0ff")}
	}),
	"mirror_smith": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hairBlack, beard: hairBlack, outfit: "plate", top: hex("#e0e4f0"), trim: c, pants: cIron,
			weapon: "hammer", weaponCol: hex("#f0f4ff"), shield: hex("#f0f4ff"), shieldForm: "round"}
	}),
	"dragon_slayer": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, beard: hex("#8a4a2a"), outfit: "plate", top: hex("#7a2a24"), trim: cGold, pants: hex("#3a2a2a"),
			head: "horned", headCol: hex("#5a4a4a"), cape: hex("#4a1010"), weapon: "sword", weaponCol: hex("#ffb070")}
	}),
	"grove_warden": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#a08a6a"), hair: hex("#4a7a3a"), hairStyle: "long", outfit: "robe", top: hex("#3a5a2a"), trim: c,
			boots: hex("#4a3a24"), head: "horns", headCol: hex("#7a5a3a"), weapon: "staff", weaponCol: hex("#a0f07a")}
	}),
	"koschei": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#c8c8b8"), beard: hex("#d8d8c8"), outfit: "robe", top: hex("#1e2a1e"), trim: c, boots: hex("#101410"),
			head: "crown", headCol: hex("#8a8a80"), glow: true, eyes: c, weapon: "skullstaff", weaponCol: c, cape: hex("#0e140e")}
	}),
	"berserker_old": hero(func(c color.RGBA) hum {
		return hum{build: "big", skin: cSkin, hair: hairGrey, hairStyle: "wild", beard: hairGrey, outfit: "fur", top: hex("#8a8a8a"),
			trim: c, pants: hex("#4a3a2a"), head: "fur_hat", headCol: hex("#a0a0a0"), weapon: "axe", offhand: "axe"}
	}),
	"monster_huntress": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hex("#a03a1a"), hairStyle: "pony", outfit: "leather", top: hex("#4a3a2a"), trim: c,
			pants: hex("#2a2420"), boots: hex("#1e1a16"), cape: hex("#3a3024"), weapon: "sword", weaponCol: hex("#e8f0ff"), offhand: "crossbow"}
	}),
	"clockmaker": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hairGrey, hairStyle: "wild", beard: hairGrey, outfit: "robe", top: hex("#5a4a3a"), trim: cGold,
			boots: hex("#3a2a1a"), head: "cap", headCol: hex("#3a3a5a"), weapon: "staff", weaponCol: c, offhand: "orb", offCol: c}
	}),
	"inquisitor_npc": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, outfit: "plate", top: hex("#8a1e1a"), trim: cGold, pants: hex("#4a1a18"), head: "hood",
			headCol: hex("#7a1a16"), cape: hex("#a0261e"), weapon: "mace", weaponCol: hex("#ffb050"), offhand: "symbol", offCol: cGold}
	}),
	"stone_hermit": hero(func(c color.RGBA) hum {
		return hum{build: "big", skin: hex("#9a9a9a"), beard: hex("#7a7a7a"), outfit: "loin", pants: hex("#5a5048"),
			eyes: hex("#ffd84a"), weapon: "club", weaponCol: hex("#8a8a8a")}
	}),
	"tracker": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, hair: hairBrown, beard: hairBrown, outfit: "leather", top: hex("#6a5a3a"), pants: hex("#4a3a2a"),
			head: "hood", headCol: hex("#5a6a3a"), cape: hex("#4a5a32"), weapon: "bow"}
	}),
	"sand_wanderer": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinDark, outfit: "robe", top: hex("#f0e8d8"), trim: c, boots: hex("#c8b898"), head: "turban",
			headCol: hex("#f8f4ea"), mask: hex("#e8dcc4"), weapon: "staff", weaponCol: c}
	}),
	"old_voivode": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hairGrey, beard: hairGrey, outfit: "plate", top: hex("#8a8a90"), trim: c, pants: cIron,
			head: "helmet", headCol: hex("#a0a0a8"), cape: hex("#8a2a1a"), weapon: "sword", shield: hex("#8a2a1a"), shieldMark: cGold}
	}),
	"assassin_master": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#d8c8c0"), outfit: "leather", top: hex("#141418"), trim: c, pants: hex("#101014"), boots: hex("#0a0a0c"),
			head: "hood", headCol: hex("#18181c"), mask: hex("#0c0c10"), cape: hex("#1a1a20"), weapon: "dagger", weaponCol: c, offhand: "dagger", offCol: c}
	}),
	"archmage_npc": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hex("#f0f0f0"), hairStyle: "long", beard: hex("#f4f4f4"), outfit: "robe", top: hex("#2a3a8a"), trim: c,
			boots: hex("#1a2450"), head: "wizard", headCol: hex("#2a3a8a"), weapon: "staff", weaponCol: c, offhand: "book", offCol: hex("#4a2a6a")}
	}),
	"oracle_npc": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, hair: hex("#e0e0e8"), hairStyle: "long", outfit: "robe", top: hex("#e8eef8"), trim: c,
			boots: hex("#a0a8b8"), mask: hex("#8a9ab0"), offhand: "orb", offCol: c}
	}),
	"fire_keeper": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#8a5a3a"), hair: hairBlack, hairStyle: "wild", outfit: "fur", top: hex("#5a3a2a"), trim: c,
			head: "horns", headCol: hex("#3a2a20"), glow: true, eyes: hex("#ffa040"), weapon: "totem", weaponCol: c}
	}),
	"nameless": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#b8a8b0"), outfit: "robe", top: hex("#20102a"), trim: c, boots: hex("#140a1a"),
			head: "hood", headCol: hex("#1a0a24"), glow: true, eyes: c, weapon: "skullstaff", weaponCol: c}
	}),
	"moon_druid": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#e8e8f4"), hair: hex("#c0c8f0"), hairStyle: "long", outfit: "robe", top: hex("#2a3060"), trim: c,
			boots: hex("#1a1e40"), head: "circlet", headCol: hex("#e0e8ff"), weapon: "staff", weaponCol: c}
	}),
	"mourner": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#d8d8d8"), hair: hairGrey, hairStyle: "long", outfit: "robe", top: hex("#2a2e34"), trim: c,
			boots: hex("#1a1c20"), head: "hood", headCol: hex("#30343a"), offhand: "symbol", offCol: c}
	}),
	"star_archer": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hex("#e0e4f0"), hairStyle: "long", outfit: "leather", top: hex("#2a3460"), trim: c,
			pants: hex("#1a2040"), cape: hex("#d0d8ff"), weapon: "bow", weaponCol: c}
	}),
	"wild_huntsman": hero(func(c color.RGBA) hum {
		return hum{build: "big", skin: hex("#a8b8a0"), outfit: "fur", top: hex("#3a4a32"), trim: c, head: "horns", headCol: hex("#6a5a3a"),
			glow: true, eyes: hex("#c0ffc0"), cape: hex("#2a3424"), weapon: "spear", weaponCol: c}
	}),
	"grail_keeper": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hairGrey, beard: hairGrey, outfit: "plate", top: hex("#f0f0f8"), trim: cGold, pants: hex("#c8c8d8"),
			cape: hex("#e8d8a0"), weapon: "spear", weaponCol: hex("#fff4c0"), offhand: "orb", offCol: c}
	}),
	"fallen_paladin": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#c8b8c0"), outfit: "plate", top: hex("#2a2030"), trim: c, pants: hex("#1a141e"), head: "helmet",
			headCol: hex("#2a2030"), glow: true, eyes: hex("#c060ff"), cape: hex("#3a1a4a"), weapon: "sword", weaponCol: c, shield: hex("#2a2030"), shieldMark: c}
	}),
	"drunken_npc": hero(func(c color.RGBA) hum {
		return hum{build: "big", skin: hex("#e8b090"), hair: hairGrey, beard: hairGrey, outfit: "robe", top: hex("#c08a40"), trim: hex("#6a3a1a"),
			boots: hex("#5a3a1a"), weapon: "staff", weaponCol: hex("#8a6a4a"), offhand: "orb", offCol: hex("#a06a30")}
	}),
	"dragon_monk": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, outfit: "robe", top: hex("#b02a1a"), trim: cGold, boots: hex("#4a1a10"), weapon: "staff", weaponCol: c}
	}),
	"dirge_singer": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#d8d4dc"), hair: hairBlack, hairStyle: "long", outfit: "robe", top: hex("#1a1820"), trim: c, boots: hex("#121016"),
			head: "feathers", headCol: hex("#2a2a30"), cape: hex("#1e1c24"), weapon: "staff", weaponCol: c}
	}),
	"rune_carver": hero(func(c color.RGBA) hum {
		return hum{build: "big", skin: cSkin, hair: hex("#c87a3a"), hairStyle: "long", beard: hex("#c87a3a"), outfit: "fur", top: hex("#5a5a6a"),
			trim: c, head: "horned", headCol: cSteel, weapon: "hammer", weaponCol: c}
	}),
	"plague_doc": hero(func(c color.RGBA) hum {
		return hum{skin: hex("#c8c0b0"), outfit: "robe", top: hex("#1a1a1a"), trim: c, boots: hex("#0e0e0e"), head: "wizard",
			headCol: hex("#141414"), mask: hex("#e8e0c8"), weapon: "staff", weaponCol: c}
	}),
	"old_alchemist": hero(func(c color.RGBA) hum {
		return hum{skin: cSkin, hair: hex("#f0f0f0"), beard: hex("#f4f4f4"), outfit: "robe", top: hex("#5a2a6a"), trim: cGold,
			boots: hex("#3a1a4a"), head: "cap", headCol: hex("#3a1a4a"), offhand: "orb", offCol: c}
	}),
	"sun_smith": hero(func(c color.RGBA) hum {
		return hum{skin: cSkinTan, beard: hex("#c08a3a"), outfit: "leather", top: hex("#8a5a2a"), trim: cGold, pants: hex("#4a3a30"),
			weapon: "hammer", weaponCol: cGold}
	}),

	// ---- beasts ----
	"wolf": beast(func(c color.RGBA, f int) *pc {
		return paintQuad(quad{w: 20, h: 14, body: [4]float64{9, 8, 6.5, 3}, head: [3]float64{15, 5, 2.7}, snout: [4]float64{17.8, 6.3, 1.9, 1.1},
			ear: "pointy", tail: "bushy", legs: []float64{4, 6, 11, 13}, legW: 2, legTop: 9, fur: c, belly: mix(c, cWhite, 0.4)}, f)
	}),
	"boar": beast(func(c color.RGBA, f int) *pc {
		return paintQuad(quad{w: 18, h: 12, body: [4]float64{8, 6.5, 6.5, 3.6}, head: [3]float64{13.6, 6.2, 3}, snout: [4]float64{16.5, 7.4, 1.6, 1.3},
			ear: "small", tail: "short", legs: []float64{3, 5, 10, 12}, legW: 2, legTop: 8, fur: c, belly: mul(c, 0.85), tusk: true, mane: true}, f)
	}),
	"bear": beast(func(c color.RGBA, f int) *pc {
		return paintQuad(quad{w: 22, h: 15, body: [4]float64{9, 8, 7.8, 4.4}, head: [3]float64{16.5, 5.4, 3}, snout: [4]float64{19.6, 6.6, 1.8, 1.3},
			ear: "round", tail: "", legs: []float64{3, 6, 11, 14}, legW: 3, legTop: 10, fur: c, belly: mul(c, 0.85),
			accent: mul(c, 0.55), muzzle: mix(c, hex("#f0d8b0"), 0.55)}, f)
	}),
	"rat": beast(func(c color.RGBA, f int) *pc {
		return paintQuad(quad{w: 15, h: 9, body: [4]float64{6.5, 5, 4.2, 2.4}, head: [3]float64{10.5, 4.6, 2}, snout: [4]float64{12.8, 5.2, 1.3, 0.9},
			ear: "round", tail: "thin", legs: []float64{4, 5, 8, 9}, legW: 1, legTop: 6, fur: c, belly: mix(c, cWhite, 0.3), accent: hex("#f0a0a8")}, f)
	}),
	"salamander": beast(func(c color.RGBA, f int) *pc {
		return paintQuad(quad{w: 22, h: 10, body: [4]float64{11, 5.5, 5.8, 2}, head: [3]float64{17, 5, 2.1}, snout: [4]float64{19.4, 5.4, 1.7, 1},
			tail: "long", legs: []float64{7, 8, 13, 14}, legW: 1.5, legTop: 6, fur: c, belly: hex("#ffd84a"), accent: hex("#ffe86a"), spots: true, eye: hex("#ffd84a")}, f)
	}),
	"fire_drake": beast(func(c color.RGBA, f int) *pc {
		return paintQuad(quad{w: 26, h: 20, body: [4]float64{12, 12, 7, 4}, head: [3]float64{20.5, 5, 2.8}, snout: [4]float64{23.4, 5.9, 2, 1.2},
			neck: true, ear: "horns", tail: "spiked", legs: []float64{7, 9, 14, 16}, legW: 2.4, legTop: 14, fur: c, belly: hex("#ffd070"),
			accent: hex("#ffb040"), wing: true, eye: hex("#fff06a")}, f)
	}),
	"spider":            beast(func(c color.RGBA, f int) *pc { return paintSpider(c, color.RGBA{}, f) }),
	"cave_spider_queen": beast(func(c color.RGBA, f int) *pc { return paintSpider(c, hex("#ff3a3a"), f) }),
	"scorpion":          beast(paintScorpion),
	"scarab":            beast(paintBeetle),
	"giant_bat":         {beast: paintBat, float: true},
	"slime":             beast(paintSlime),
	"ghost":             {beast: func(c color.RGBA, f int) *pc { return paintGhost(c, f, color.RGBA{}, false) }, float: true},
	"banshee":           {beast: func(c color.RGBA, f int) *pc { return paintGhost(c, f, hex("#3a3a58"), false) }, float: true},
	"wraith":            {beast: func(c color.RGBA, f int) *pc { return paintGhost(c, f, color.RGBA{}, true) }, float: true},
	"ice_elemental":     {beast: paintElemental, float: true},
	"sand_worm":         beast(paintWorm),
	"treant":            beast(paintTreant),
}

// aliases: monsters that share a model under another name
var modelAliases = map[string]string{
	"risen": "zombie", "tomb_guardian": "golem", "bone_golem": "golem", "magma_lord": "magma_golem",
	"frost_jarl": "frost_giant", "ice_wolf": "wolf", "shadow_wolf": "wolf",
	"hunter_npc": "hunter", "wandering_mage": "battle_mage", "town_guard": "guard",
	"imp_minion": "fire_imp", "felguard": "demon", "spirit_wolf": "wolf",
	"skeleton_minion": "skeleton", "skeleton_mage_minion": "skeleton_mage", "bone_golem_minion": "golem", "treant_minion": "treant",
	"rng_wolf_companion": "wolf", "rng_hawk": "giant_bat", "rng_bear_companion": "bear", "wl_soldier": "guard", "wr_ghost": "ghost",
	"wh_hound": "wolf", "wh_huntsman": "hunter", "tm_golem": "golem",
}

// glyphModels guess a model for modded creatures without one.
var glyphModels = map[rune]string{
	'w': "wolf", 'b': "boar", 'B': "bear", 'r': "rat", 's': "spider", 'S': "spider", 'z': "skeleton", 'Z': "zombie",
	'g': "goblin", 'o': "orc", 'O': "orc", 'T': "cave_troll", 'G': "ghost", 'W': "wraith", 'v': "giant_bat", 'V': "banshee",
	'l': "lizardman", 'h': "bandit", 'H': "bandit_chief", 'N': "necromancer", 'L': "lich", 'u': "ghoul", 'M': "mummy",
	'j': "slime", 'x': "scorpion", 'e': "ice_elemental", 'i': "fire_imp", 'D': "fire_drake", 'c': "cultist",
	'K': "dark_knight", 'Y': "yeti", 'J': "frost_giant", 'Q': "golem", 'E': "efreet", 'U': "demon", 'a': "scarab",
	'P': "pharaoh", 'A': "demon_lord",
}

// resolveModel finds the model definition for a creature.
func resolveModel(model, def string, glyph rune, kind uint8) (string, modelDef) {
	for _, name := range []string{model, def} {
		if name == "" {
			continue
		}
		if a, ok := modelAliases[name]; ok {
			name = a
		}
		if d, ok := modelDefs[name]; ok {
			return name, d
		}
	}
	switch kind {
	case kindPlayer:
		return "adventurer", humanoid(func(c color.RGBA, _ int) hum {
			return hum{skin: cSkin, hair: hairBrown, outfit: "tunic", top: c, pants: hex("#4a3a2a"), weapon: "sword"}
		})
	case kindNPC:
		return "villager", modelDefs["villager"]
	}
	if name, ok := glyphModels[glyph]; ok {
		return name, modelDefs[name]
	}
	if name, ok := glyphModels[unicode.ToLower(glyph)]; ok {
		return name, modelDefs[name]
	}
	return "brute", humanoid(func(c color.RGBA, _ int) hum {
		return hum{build: "big", skin: c, outfit: "loin", pants: hex("#4a3a2a"), eyes: hex("#ffd84a"), weapon: "club"}
	})
}

// paintModel renders one frame of a model.
func paintModel(d modelDef, c color.RGBA, variant, frame int) *pc {
	if d.beast != nil {
		return outlined(d.beast(c, frame))
	}
	return outlined(paintHumanoid(d.hum(c, variant), frame))
}
