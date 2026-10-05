//! Which model each creature uses: humanoid descriptions for heroes,
//! villagers and humanoid monsters, painters for beasts, aliases and glyph
//! guesses for modded creatures, and dressing heroes in their equipment.

use ratas_core::content::{self, ItemDef};

use super::models::*;
use super::*;

pub const HAIR_BROWN: Rgba = rgb(0x6a, 0x42, 0x24);
pub const HAIR_BLACK: Rgba = rgb(0x22, 0x18, 0x0f);
pub const HAIR_BLOND: Rgba = rgb(0xe0, 0xc0, 0x70);
pub const HAIR_GREY: Rgba = rgb(0xb8, 0xb8, 0xb8);

pub const KIND_PLAYER: u8 = 1;
pub const KIND_NPC: u8 = 3;

fn h(x: Hum) -> Option<Hum> {
    Some(x)
}

/// The humanoid description of a model, if it is a humanoid.
pub fn humanoid(name: &str, c: Rgba, v: i32) -> Option<Hum> {
    match name {
        "warrior" => h(Hum {
            skin: C_SKIN,
            outfit: "plate",
            top: C_STEEL,
            trim: mul(c, 0.85),
            pants: C_IRON,
            boots: hex("#4a4048"),
            head: "helmet",
            head_col: C_STEEL,
            weapon: "sword",
            shield: mul(c, 0.8),
            shield_mark: C_GOLD,
            ..Hum::default()
        }),
        "ranger" => h(Hum {
            skin: C_SKIN_TAN,
            hair: HAIR_BROWN,
            outfit: "leather",
            top: hex("#4a7a3a"),
            pants: hex("#5a4030"),
            boots: C_LEATHER,
            head: "hood",
            head_col: hex("#3e6a32"),
            cape: hex("#35582c"),
            weapon: "bow",
            ..Hum::default()
        }),
        "mage" => h(Hum {
            skin: C_SKIN,
            hair: HAIR_BLOND,
            outfit: "robe",
            top: hex("#3a56b0"),
            trim: C_GOLD,
            boots: hex("#26346a"),
            head: "wizard",
            head_col: hex("#3a56b0"),
            weapon: "staff",
            weapon_col: hex("#7ac0ff"),
            ..Hum::default()
        }),
        "paladin" => h(Hum {
            skin: C_SKIN,
            hair: HAIR_BLOND,
            outfit: "plate",
            top: hex("#e4e4ee"),
            trim: C_GOLD,
            pants: hex("#b0b0c0"),
            boots: hex("#8a8a9a"),
            cape: hex("#3a56b0"),
            weapon: "hammer",
            weapon_col: hex("#c8a040"),
            shield: hex("#f0f0f8"),
            shield_mark: C_GOLD,
            ..Hum::default()
        }),
        "rogue" => h(Hum {
            skin: C_SKIN,
            outfit: "leather",
            top: hex("#3c3c4a"),
            pants: hex("#2a2a34"),
            boots: hex("#1e1e26"),
            head: "hood",
            head_col: hex("#30303c"),
            mask: hex("#24242c"),
            weapon: "dagger",
            weapon_col: hex("#c8d0e0"),
            cape: hex("#26262e"),
            ..Hum::default()
        }),
        "shaman" => h(Hum {
            skin: C_SKIN_TAN,
            hair: HAIR_BLACK,
            hair_style: "long",
            outfit: "fur",
            top: hex("#8a6a48"),
            trim: c,
            pants: hex("#6a4a30"),
            head: "feathers",
            head_col: c,
            weapon: "totem",
            weapon_col: c,
            ..Hum::default()
        }),
        "warlock" => h(Hum {
            skin: hex("#d8c4d4"),
            outfit: "robe",
            top: hex("#3a1a4a"),
            trim: c,
            boots: hex("#24102e"),
            head: "hood",
            head_col: hex("#2c1238"),
            glow: true,
            eyes: hex("#e08aff"),
            weapon: "wand",
            weapon_col: hex("#e08aff"),
            ..Hum::default()
        }),
        "druid_hero" => h(Hum {
            skin: C_SKIN_TAN,
            hair: hex("#6a4a2a"),
            hair_style: "long",
            beard: hex("#6a4a2a"),
            outfit: "robe",
            top: hex("#4a6a32"),
            trim: c,
            boots: hex("#3a2a1a"),
            head: "horns",
            head_col: hex("#8a6a4a"),
            weapon: "staff",
            weapon_col: hex("#8ae05a"),
            cape: hex("#5a4a2a"),
            ..Hum::default()
        }),
        "necro_hero" => h(Hum {
            skin: hex("#d0d0c4"),
            hair: HAIR_BLACK,
            hair_style: "long",
            outfit: "robe",
            top: hex("#20262a"),
            trim: c,
            boots: hex("#121614"),
            glow: true,
            eyes: c,
            weapon: "skullstaff",
            weapon_col: c,
            offhand: "book",
            off_col: hex("#2a3a2a"),
            cape: hex("#141a16"),
            ..Hum::default()
        }),
        "monk" => h(Hum {
            skin: C_SKIN_TAN,
            outfit: "robe",
            top: hex("#e08a30"),
            trim: hex("#8a3a1a"),
            boots: hex("#6a3a1a"),
            weapon: "staff",
            weapon_col: hex("#a07a4a"),
            ..Hum::default()
        }),
        "skald" => h(Hum {
            skin: C_SKIN,
            hair: HAIR_BLOND,
            hair_style: "long",
            beard: HAIR_BLOND,
            outfit: "fur",
            top: hex("#7a5a8a"),
            trim: c,
            pants: hex("#4a3a2a"),
            head: "horned",
            head_col: C_STEEL,
            cape: hex("#5a2a4a"),
            weapon: "axe",
            shield: hex("#8a5a3a"),
            shield_form: "round",
            ..Hum::default()
        }),
        "brewer_hero" => h(Hum {
            skin: C_SKIN,
            hair: hex("#5a3a2a"),
            hair_style: "wild",
            outfit: "leather",
            top: hex("#4a6a4a"),
            trim: c,
            pants: hex("#3a3a2a"),
            mask: hex("#5a5a4a"),
            weapon: "dagger",
            offhand: "orb",
            off_col: c,
            ..Hum::default()
        }),
        "necromancer" => h(Hum {
            skin: hex("#c8c0b0"),
            outfit: "robe",
            top: hex("#28262e"),
            trim: C_BONE,
            boots: hex("#1a181e"),
            head: "hood",
            head_col: hex("#1c1a20"),
            glow: true,
            eyes: hex("#8aff7a"),
            weapon: "skullstaff",
            weapon_col: hex("#8aff7a"),
            ..Hum::default()
        }),
        "elder" => h(Hum {
            skin: C_SKIN,
            beard: hex("#ececec"),
            outfit: "robe",
            top: hex("#9a825e"),
            trim: hex("#e0c890"),
            boots: hex("#6a5236"),
            weapon: "staff",
            weapon_col: hex("#8ae08a"),
            ..Hum::default()
        }),
        "merchant" => h(Hum {
            skin: C_SKIN,
            hair: HAIR_BROWN,
            beard: HAIR_BROWN,
            outfit: "tunic",
            top: hex("#d0a040"),
            trim: hex("#8a3a2a"),
            pants: hex("#5a3a2a"),
            head: "cap",
            head_col: hex("#8a3a2a"),
            ..Hum::default()
        }),
        "smith" => h(Hum {
            skin: C_SKIN_TAN,
            beard: hex("#3a2614"),
            outfit: "leather",
            top: hex("#6a4a2a"),
            pants: hex("#3a3a40"),
            weapon: "hammer",
            weapon_col: C_IRON,
            ..Hum::default()
        }),
        "healer" => h(Hum {
            skin: C_SKIN,
            hair: hex("#8a5a3a"),
            hair_style: "long",
            outfit: "robe",
            top: hex("#5a9a5a"),
            trim: C_WHITE,
            boots: hex("#3a6a3a"),
            ..Hum::default()
        }),
        "priest" => h(Hum {
            skin: C_SKIN,
            hair: hex("#8a6a4a"),
            outfit: "robe",
            top: hex("#ece8dc"),
            trim: C_GOLD,
            boots: hex("#b0a890"),
            weapon: "staff",
            weapon_col: C_GOLD,
            ..Hum::default()
        }),
        "guard" => h(Hum {
            skin: C_SKIN,
            outfit: "plate",
            top: hex("#9aa0b0"),
            trim: hex("#4a6aaa"),
            pants: C_IRON,
            head: "helmet",
            head_col: hex("#9aa0b0"),
            weapon: "spear",
            shield: hex("#4a6aaa"),
            shield_mark: C_WHITE,
            ..Hum::default()
        }),
        "mayor" => h(Hum {
            build: "big",
            skin: C_SKIN,
            hair: HAIR_GREY,
            beard: HAIR_GREY,
            outfit: "robe",
            top: hex("#7a1e2a"),
            trim: C_GOLD,
            boots: hex("#3a1a1a"),
            head: "cap",
            head_col: hex("#2a1a3a"),
            cape: hex("#e8e0d0"),
            ..Hum::default()
        }),
        "captain" => h(Hum {
            skin: C_SKIN_TAN,
            beard: hex("#5a3a2a"),
            outfit: "plate",
            top: hex("#b0b8c8"),
            trim: C_GOLD,
            pants: C_IRON,
            head: "helmet",
            head_col: hex("#c8d0e0"),
            cape: hex("#2a4a9a"),
            weapon: "sword",
            shield: hex("#2a4a9a"),
            shield_mark: C_GOLD,
            ..Hum::default()
        }),
        "innkeeper" => h(Hum {
            build: "big",
            skin: C_SKIN,
            hair: hex("#8a4a2a"),
            beard: hex("#8a4a2a"),
            outfit: "tunic",
            top: hex("#e8e0d0"),
            trim: hex("#8a5a3a"),
            pants: hex("#4a3a2a"),
            ..Hum::default()
        }),
        "bard" => h(Hum {
            skin: C_SKIN,
            hair: HAIR_BLOND,
            hair_style: "long",
            outfit: "tunic",
            top: hex("#c03a8a"),
            trim: C_GOLD,
            pants: hex("#3a2a5a"),
            head: "feathers",
            head_col: hex("#ff80c0"),
            cape: hex("#5a2a7a"),
            ..Hum::default()
        }),
        "armorer" => h(Hum {
            skin: C_SKIN_TAN,
            hair: HAIR_BLACK,
            beard: HAIR_BLACK,
            outfit: "plate",
            top: hex("#8a909a"),
            pants: hex("#3a3a40"),
            weapon: "hammer",
            weapon_col: C_IRON,
            shield: hex("#6a7080"),
            shield_form: "round",
            ..Hum::default()
        }),
        "enchanter" => h(Hum {
            skin: C_SKIN,
            hair: hex("#e0e0f0"),
            hair_style: "long",
            outfit: "robe",
            top: hex("#4a2a8a"),
            trim: hex("#c0a0ff"),
            boots: hex("#2a1a4a"),
            head: "circlet",
            head_col: hex("#c0a0ff"),
            weapon: "wand",
            weapon_col: hex("#e0c0ff"),
            offhand: "orb",
            off_col: hex("#b090ff"),
            ..Hum::default()
        }),
        "alchemist" => h(Hum {
            skin: C_SKIN,
            hair: hex("#4a3a2a"),
            hair_style: "wild",
            outfit: "leather",
            top: hex("#5a6a4a"),
            trim: hex("#80e0a0"),
            pants: hex("#3a3a2a"),
            mask: hex("#6a6a5a"),
            offhand: "orb",
            off_col: hex("#80e0a0"),
            ..Hum::default()
        }),
        "jeweler" => h(Hum {
            skin: C_SKIN,
            hair: HAIR_GREY,
            beard: HAIR_GREY,
            outfit: "robe",
            top: hex("#3a2a5a"),
            trim: C_GOLD,
            boots: hex("#2a1a3a"),
            head: "cap",
            head_col: hex("#8a2a4a"),
            ..Hum::default()
        }),
        "bandit" => h(Hum {
            skin: C_SKIN_TAN,
            outfit: "leather",
            top: mix(hex("#6a4a2a"), c, 0.25),
            pants: hex("#4a3a2a"),
            head: "hood",
            head_col: hex("#8a2a24"),
            mask: hex("#3a2a24"),
            weapon: "sword",
            ..Hum::default()
        }),
        "bandit_archer" => h(Hum {
            skin: C_SKIN_TAN,
            outfit: "leather",
            top: mix(hex("#6a5a2a"), c, 0.25),
            pants: hex("#4a3a2a"),
            head: "hood",
            head_col: hex("#7a5a2a"),
            mask: hex("#3a2a24"),
            weapon: "bow",
            ..Hum::default()
        }),
        "bandit_chief" => h(Hum {
            skin: C_SKIN_TAN,
            hair: HAIR_BLACK,
            hair_style: "wild",
            beard: HAIR_BLACK,
            outfit: "leather",
            top: hex("#5a3a2a"),
            trim: c,
            pants: hex("#3a2a20"),
            cape: hex("#9a2a24"),
            weapon: "axe",
            ..Hum::default()
        }),
        "nomad" => h(Hum {
            skin: C_SKIN_DARK,
            outfit: "tunic",
            top: c,
            trim: hex("#a04a2a"),
            pants: hex("#b89060"),
            head: "turban",
            head_col: hex("#f0e4c8"),
            mask: hex("#c8a060"),
            weapon: "sword",
            weapon_col: hex("#e0e0e8"),
            ..Hum::default()
        }),
        "cultist" => h(Hum {
            skin: mul(C_SKIN, 0.55),
            outfit: "robe",
            top: mul(c, 0.7),
            trim: hex("#2a0a10"),
            boots: hex("#2a0a10"),
            head: "hood",
            head_col: mul(c, 0.6),
            glow: true,
            eyes: hex("#ff6a4a"),
            weapon: "dagger",
            ..Hum::default()
        }),
        "dark_knight" => h(Hum {
            skin: hex("#3a3040"),
            outfit: "plate",
            top: hex("#4a3a5a"),
            trim: c,
            pants: hex("#2e2638"),
            boots: hex("#221a2a"),
            head: "horned",
            head_col: hex("#3a3048"),
            glow: true,
            eyes: hex("#d08aff"),
            weapon: "sword",
            weapon_col: hex("#b8a0e8"),
            cape: hex("#2a1a3a"),
            ..Hum::default()
        }),
        "goblin" => h(Hum {
            build: "small",
            skin: c,
            outfit: "rags",
            top: hex("#6a4a2a"),
            pants: hex("#4a3a2a"),
            head: "ears",
            eyes: hex("#ff3a2a"),
            weapon: "dagger",
            ..Hum::default()
        }),
        "goblin_archer" => h(Hum {
            build: "small",
            skin: mix(c, hex("#7ac04a"), 0.5),
            outfit: "rags",
            top: hex("#5a5a2a"),
            head: "ears",
            eyes: hex("#ff3a2a"),
            weapon: "bow",
            ..Hum::default()
        }),
        "goblin_shaman" => h(Hum {
            build: "small",
            skin: hex("#7ac04a"),
            outfit: "rags",
            top: mul(c, 0.6),
            head: "feathers",
            head_col: c,
            eyes: hex("#ffd84a"),
            weapon: "skullstaff",
            weapon_col: hex("#fff06a"),
            ..Hum::default()
        }),
        "orc" => h(Hum {
            build: "big",
            skin: c,
            hair: HAIR_BLACK,
            hair_style: "pony",
            tusks: true,
            outfit: "leather",
            top: hex("#5a4030"),
            pants: hex("#3a3028"),
            weapon: "axe",
            ..Hum::default()
        }),
        "orc_warlord" => h(Hum {
            build: "big",
            skin: hex("#6a8a3a"),
            tusks: true,
            outfit: "plate",
            top: hex("#5a5a64"),
            trim: c,
            pants: hex("#3a3a40"),
            head: "horned",
            head_col: hex("#4a4a54"),
            weapon: "axe",
            weapon_col: hex("#d0d0dc"),
            cape: hex("#8a1a1a"),
            ..Hum::default()
        }),
        "cave_troll" => h(Hum {
            build: "big",
            skin: c,
            tusks: true,
            outfit: "loin",
            pants: hex("#5a4030"),
            eyes: hex("#ffd84a"),
            weapon: "club",
            ..Hum::default()
        }),
        "skeleton" => h(Hum {
            outfit: "bones",
            head: "skull",
            weapon: "sword",
            weapon_col: hex("#a8a8a8"),
            shield: hex("#6a5440"),
            ..Hum::default()
        }),
        "wandering_skeleton" => h(Hum {
            outfit: "bones",
            head: "skull",
            glow: true,
            eyes: hex("#7ad0ff"),
            weapon: "sword",
            weapon_col: hex("#9a9a9a"),
            ..Hum::default()
        }),
        "skeleton_archer" => h(Hum {
            outfit: "bones",
            head: "skull",
            weapon: "bow",
            ..Hum::default()
        }),
        "zombie" => h(Hum {
            skin: c,
            hair: hex("#2a2a1a"),
            hair_style: "wild",
            outfit: "rags",
            top: hex("#4a5a6a"),
            pants: hex("#3a3a4a"),
            eyes: hex("#f0f0b0"),
            weapon: "claws",
            ..Hum::default()
        }),
        "ghoul" => h(Hum {
            skin: c,
            outfit: "rags",
            top: hex("#4a3a3a"),
            pants: hex("#3a2a2a"),
            glow: true,
            eyes: hex("#ff4a4a"),
            tusks: true,
            weapon: "claws",
            ..Hum::default()
        }),
        "mummy" => h(Hum {
            skin: mul(c, 0.9),
            outfit: "wrap",
            top: c,
            pants: mul(c, 0.85),
            boots: mul(c, 0.7),
            glow: true,
            eyes: hex("#7ad0ff"),
            mask: mul(c, 0.8),
            ..Hum::default()
        }),
        "lich" => h(Hum {
            outfit: "robe",
            top: hex("#4a1a5a"),
            trim: C_GOLD,
            boots: hex("#2a0e34"),
            head: "skull",
            crown: true,
            glow: true,
            eyes: c,
            weapon: "skullstaff",
            weapon_col: c,
            ..Hum::default()
        }),
        "bog_witch" => h(Hum {
            skin: hex("#9ab070"),
            hair: HAIR_GREY,
            hair_style: "long",
            outfit: "robe",
            top: hex("#4a5a2a"),
            trim: c,
            head: "wizard",
            head_col: hex("#2a2a1a"),
            weapon: "staff",
            weapon_col: c,
            ..Hum::default()
        }),
        "lizardman" => h(Hum {
            skin: c,
            head: "snout",
            outfit: "leather",
            top: hex("#6a5a3a"),
            pants: mul(c, 0.8),
            boots: mul(c, 0.6),
            tail: c,
            weapon: "spear",
            ..Hum::default()
        }),
        "harpy" => h(Hum {
            skin: hex("#f0c8b0"),
            hair: c,
            hair_style: "long",
            outfit: "fur",
            top: mul(c, 0.85),
            wings: mul(c, 0.72),
            legs: "talons",
            eyes: hex("#ff3a6a"),
            weapon: "claws",
            ..Hum::default()
        }),
        "pharaoh" => h(Hum {
            skin: hex("#d8b050"),
            outfit: "robe",
            top: hex("#f0e8d0"),
            trim: C_GOLD,
            boots: hex("#3a5aaa"),
            head: "nemes",
            head_col: hex("#3a5aaa"),
            glow: true,
            eyes: hex("#7ad0ff"),
            weapon: "staff",
            weapon_col: hex("#7ad0ff"),
            ..Hum::default()
        }),
        "frost_giant" => h(Hum {
            build: "huge",
            skin: mix(c, hex("#a8c8e0"), 0.5),
            beard: hex("#f4f8ff"),
            outfit: "fur",
            top: hex("#7a8aa0"),
            trim: hex("#c8d8e8"),
            pants: hex("#5a6a80"),
            head: "horned",
            head_col: hex("#6a7a90"),
            eyes: hex("#2a6aaa"),
            weapon: "hammer",
            weapon_col: hex("#c8e8ff"),
            ..Hum::default()
        }),
        "yeti" => h(Hum {
            build: "big",
            skin: hex("#a8b8c8"),
            outfit: "fur",
            top: c,
            head: "hood",
            head_col: c,
            eyes: hex("#2a4a6a"),
            weapon: "claws",
            ..Hum::default()
        }),
        "demon" => h(Hum {
            build: "big",
            skin: c,
            outfit: "loin",
            pants: hex("#2a1a1a"),
            head: "horns",
            head_col: hex("#2a1a1a"),
            wings: mul(c, 0.5),
            tail: c,
            glow: true,
            eyes: hex("#ffd84a"),
            weapon: "claws",
            ..Hum::default()
        }),
        "demon_lord" => h(Hum {
            build: "huge",
            skin: mul(c, 0.85),
            outfit: "plate",
            top: hex("#2e1a26"),
            trim: c,
            pants: hex("#2a1a1a"),
            head: "horns",
            head_col: hex("#1a1014"),
            wings: hex("#4a0a1a"),
            tail: mul(c, 0.85),
            glow: true,
            eyes: hex("#ffd84a"),
            weapon: "sword",
            weapon_col: hex("#ff6a2a"),
            ..Hum::default()
        }),
        "fire_imp" => h(Hum {
            build: "small",
            skin: c,
            outfit: "loin",
            pants: hex("#3a1a0a"),
            head: "horns",
            head_col: hex("#4a1a0a"),
            wings: hex("#a03a1a"),
            tail: c,
            glow: true,
            eyes: hex("#ffff8a"),
            weapon: "claws",
            ..Hum::default()
        }),
        "efreet" => h(Hum {
            build: "big",
            skin: c,
            hair: hex("#ffd84a"),
            hair_style: "wild",
            outfit: "loin",
            top: mul(c, 0.75),
            pants: C_GOLD,
            legs: "smoke",
            glow: true,
            eyes: hex("#fff8c0"),
            weapon: "sword",
            weapon_col: C_GOLD,
            ..Hum::default()
        }),
        "golem" => h(Hum {
            build: "big",
            skin: c,
            outfit: "golem",
            glow: true,
            eyes: hex("#7ad0ff"),
            ..Hum::default()
        }),
        "magma_golem" => h(Hum {
            build: "big",
            skin: hex("#4a3a38"),
            outfit: "golem",
            trim: c,
            glow: true,
            eyes: hex("#ffd84a"),
            ..Hum::default()
        }),
        "bandit_thief" => h(Hum {
            skin: C_SKIN_TAN,
            outfit: "leather",
            top: mix(hex("#3a3444"), c, 0.3),
            pants: hex("#2a2630"),
            boots: hex("#1e1a22"),
            head: "hood",
            head_col: hex("#2e2a38"),
            mask: hex("#1e1a22"),
            weapon: "dagger",
            offhand: "dagger",
            ..Hum::default()
        }),
        "bandit_mage" => h(Hum {
            skin: C_SKIN_TAN,
            outfit: "robe",
            top: hex("#5a2a2a"),
            trim: c,
            boots: hex("#3a1a1a"),
            head: "hood",
            head_col: hex("#8a2a24"),
            mask: hex("#3a2a24"),
            glow: true,
            eyes: c,
            weapon: "wand",
            weapon_col: c,
            ..Hum::default()
        }),
        "orc_shaman" => h(Hum {
            build: "big",
            skin: hex("#6a9a3a"),
            tusks: true,
            outfit: "fur",
            top: hex("#6a5038"),
            trim: c,
            head: "feathers",
            head_col: hex("#8a3a2a"),
            eyes: hex("#ffd84a"),
            weapon: "totem",
            weapon_col: c,
            ..Hum::default()
        }),
        "skeleton_mage" => h(Hum {
            outfit: "robe",
            top: hex("#2a3048"),
            trim: c,
            boots: hex("#1a1e30"),
            head: "skull",
            glow: true,
            eyes: c,
            weapon: "skullstaff",
            weapon_col: c,
            ..Hum::default()
        }),
        "illusion" => h(Hum {
            skin: mix(C_SKIN, c, 0.4),
            hair: c,
            outfit: "tunic",
            top: c,
            pants: mul(c, 0.6),
            weapon: "dagger",
            ..Hum::default()
        }),
        "knight_errant" => h(Hum {
            skin: C_SKIN,
            outfit: "plate",
            top: hex("#c8ccd8"),
            trim: hex("#3a5aaa"),
            pants: C_IRON,
            head: "helmet",
            head_col: hex("#c8ccd8"),
            cape: hex("#2a4a9a"),
            weapon: "sword",
            weapon_col: hex("#e8eef8"),
            shield: hex("#3a5aaa"),
            shield_mark: C_GOLD,
            ..Hum::default()
        }),
        "hunter" => h(Hum {
            skin: C_SKIN_TAN,
            hair: HAIR_BROWN,
            beard: HAIR_BROWN,
            outfit: "leather",
            top: hex("#5a7a3a"),
            pants: hex("#5a4030"),
            boots: C_LEATHER,
            head: "fur_hat",
            head_col: hex("#8a6a48"),
            cape: hex("#4a3a28"),
            weapon: "bow",
            ..Hum::default()
        }),
        "pilgrim" => h(Hum {
            skin: C_SKIN,
            hair: HAIR_GREY,
            beard: HAIR_GREY,
            outfit: "robe",
            top: hex("#a89878"),
            trim: hex("#e8d8a8"),
            boots: hex("#6a5a40"),
            head: "hood",
            head_col: hex("#8a7a5a"),
            weapon: "staff",
            weapon_col: hex("#fff0c0"),
            offhand: "symbol",
            ..Hum::default()
        }),
        "mercenary" => h(Hum {
            skin: C_SKIN_TAN,
            hair: HAIR_BLACK,
            hair_style: "pony",
            beard: HAIR_BLACK,
            outfit: "leather",
            top: hex("#6a5440"),
            trim: hex("#a07a48"),
            pants: hex("#3a3a40"),
            weapon: "axe",
            offhand: "axe",
            ..Hum::default()
        }),
        "battle_mage" => h(Hum {
            skin: C_SKIN,
            hair: HAIR_BLOND,
            outfit: "plate",
            top: hex("#4a5a9a"),
            trim: hex("#8ab0ff"),
            pants: hex("#2a3460"),
            head: "circlet",
            head_col: C_STEEL,
            cape: hex("#2a3a7a"),
            weapon: "sword",
            weapon_col: hex("#a8d0ff"),
            offhand: "orb",
            off_col: hex("#8ab0ff"),
            ..Hum::default()
        }),
        "sun_hermit" => h(Hum {
            skin: C_SKIN_TAN,
            beard: hex("#f4f0e0"),
            outfit: "robe",
            top: hex("#e8d8a8"),
            trim: C_GOLD,
            boots: hex("#a08050"),
            head: "hood",
            head_col: hex("#d8b870"),
            weapon: "staff",
            weapon_col: hex("#ffd84a"),
            ..Hum::default()
        }),
        "faceless" => h(Hum {
            skin: hex("#e0e0e8"),
            outfit: "leather",
            top: hex("#5a5a6a"),
            pants: hex("#3a3a46"),
            head: "hood",
            head_col: hex("#6a6a7a"),
            mask: hex("#f0f0f4"),
            glow: true,
            eyes: c,
            cape: hex("#4a4a58"),
            ..Hum::default()
        }),
        "seal_keeper" => h(Hum {
            skin: C_SKIN,
            beard: HAIR_GREY,
            outfit: "robe",
            top: hex("#2a3a5a"),
            trim: hex("#7ae0ff"),
            boots: hex("#1a2438"),
            head: "wizard",
            head_col: hex("#1e2a44"),
            weapon: "staff",
            weapon_col: hex("#7ae0ff"),
            offhand: "book",
            off_col: hex("#2a4a6a"),
            ..Hum::default()
        }),
        "death_herald" => h(Hum {
            skin: hex("#d8d0d8"),
            outfit: "robe",
            top: hex("#1e1a24"),
            trim: c,
            boots: hex("#141018"),
            head: "hood",
            head_col: hex("#18141e"),
            glow: true,
            eyes: hex("#c08aff"),
            weapon: "scythe",
            weapon_col: hex("#d8d0e8"),
            ..Hum::default()
        }),
        "old_shaman" => h(Hum {
            skin: C_SKIN_TAN,
            hair: HAIR_GREY,
            hair_style: "long",
            beard: HAIR_GREY,
            outfit: "fur",
            top: hex("#7a5a3a"),
            trim: c,
            head: "feathers",
            head_col: c,
            weapon: "totem",
            weapon_col: c,
            ..Hum::default()
        }),
        "dark_pilgrim" => h(Hum {
            skin: hex("#c8b0b8"),
            outfit: "robe",
            top: hex("#4a1a24"),
            trim: c,
            boots: hex("#2a0e14"),
            head: "hood",
            head_col: hex("#3a1018"),
            glow: true,
            eyes: hex("#ff6a4a"),
            weapon: "wand",
            weapon_col: c,
            offhand: "book",
            off_col: hex("#3a0a10"),
            ..Hum::default()
        }),
        "stargazer" => h(Hum {
            skin: C_SKIN,
            hair: HAIR_GREY,
            hair_style: "long",
            beard: hex("#e8e8f0"),
            outfit: "robe",
            top: hex("#1e2a5a"),
            trim: C_GOLD,
            boots: hex("#141a3a"),
            head: "wizard",
            head_col: hex("#1a2450"),
            weapon: "staff",
            weapon_col: hex("#f0f4ff"),
            offhand: "orb",
            off_col: c,
            ..Hum::default()
        }),
        "giant_healer" => h(Hum {
            build: "big",
            skin: C_SKIN,
            hair: hex("#c05a2a"),
            hair_style: "wild",
            beard: hex("#c05a2a"),
            outfit: "fur",
            top: hex("#8a7a5a"),
            trim: hex("#5aa05a"),
            weapon: "staff",
            weapon_col: hex("#8ae08a"),
            ..Hum::default()
        }),
        "night_sister" => h(Hum {
            skin: hex("#e8d8e0"),
            hair: HAIR_BLACK,
            hair_style: "long",
            outfit: "robe",
            top: hex("#2a1a3a"),
            trim: c,
            boots: hex("#1a1024"),
            mask: hex("#1a1024"),
            weapon: "dagger",
            weapon_col: c,
            offhand: "dagger",
            off_col: c,
            ..Hum::default()
        }),
        "word_master" => h(Hum {
            skin: C_SKIN,
            beard: hex("#f4f4f4"),
            outfit: "robe",
            top: hex("#7a5a3a"),
            trim: C_GOLD,
            boots: hex("#4a3420"),
            head: "cap",
            head_col: hex("#5a3a2a"),
            weapon: "staff",
            weapon_col: c,
            offhand: "book",
            off_col: hex("#8a2a2a"),
            ..Hum::default()
        }),
        "runner" => h(Hum {
            skin: C_SKIN_TAN,
            hair: HAIR_BLOND,
            hair_style: "wild",
            outfit: "leather",
            top: hex("#a08a5a"),
            pants: hex("#6a5a3a"),
            boots: hex("#5a3a20"),
            cape: mul(c, 0.8),
            weapon: "dagger",
            ..Hum::default()
        }),
        "hermit_smith" => h(Hum {
            skin: C_SKIN_TAN,
            beard: HAIR_GREY,
            outfit: "leather",
            top: hex("#5a4030"),
            pants: hex("#3a3a40"),
            head: "cap",
            head_col: hex("#4a3a2a"),
            weapon: "hammer",
            weapon_col: C_IRON,
            ..Hum::default()
        }),
        "ice_keeper" => h(Hum {
            skin: hex("#e8f0f8"),
            hair: hex("#f4f8ff"),
            hair_style: "long",
            outfit: "robe",
            top: hex("#a8c8e8"),
            trim: hex("#e8f4ff"),
            boots: hex("#6a8aa8"),
            head: "circlet",
            head_col: hex("#d0e8ff"),
            weapon: "staff",
            weapon_col: hex("#c8f0ff"),
            ..Hum::default()
        }),
        "mirror_smith" => h(Hum {
            skin: C_SKIN,
            hair: HAIR_BLACK,
            beard: HAIR_BLACK,
            outfit: "plate",
            top: hex("#e0e4f0"),
            trim: c,
            pants: C_IRON,
            weapon: "hammer",
            weapon_col: hex("#f0f4ff"),
            shield: hex("#f0f4ff"),
            shield_form: "round",
            ..Hum::default()
        }),
        "dragon_slayer" => h(Hum {
            skin: C_SKIN_TAN,
            beard: hex("#8a4a2a"),
            outfit: "plate",
            top: hex("#7a2a24"),
            trim: C_GOLD,
            pants: hex("#3a2a2a"),
            head: "horned",
            head_col: hex("#5a4a4a"),
            cape: hex("#4a1010"),
            weapon: "sword",
            weapon_col: hex("#ffb070"),
            ..Hum::default()
        }),
        "grove_warden" => h(Hum {
            skin: hex("#a08a6a"),
            hair: hex("#4a7a3a"),
            hair_style: "long",
            outfit: "robe",
            top: hex("#3a5a2a"),
            trim: c,
            boots: hex("#4a3a24"),
            head: "horns",
            head_col: hex("#7a5a3a"),
            weapon: "staff",
            weapon_col: hex("#a0f07a"),
            ..Hum::default()
        }),
        "koschei" => h(Hum {
            skin: hex("#c8c8b8"),
            beard: hex("#d8d8c8"),
            outfit: "robe",
            top: hex("#1e2a1e"),
            trim: c,
            boots: hex("#101410"),
            head: "crown",
            head_col: hex("#8a8a80"),
            glow: true,
            eyes: c,
            weapon: "skullstaff",
            weapon_col: c,
            cape: hex("#0e140e"),
            ..Hum::default()
        }),
        "berserker_old" => h(Hum {
            build: "big",
            skin: C_SKIN,
            hair: HAIR_GREY,
            hair_style: "wild",
            beard: HAIR_GREY,
            outfit: "fur",
            top: hex("#8a8a8a"),
            trim: c,
            pants: hex("#4a3a2a"),
            head: "fur_hat",
            head_col: hex("#a0a0a0"),
            weapon: "axe",
            offhand: "axe",
            ..Hum::default()
        }),
        "monster_huntress" => h(Hum {
            skin: C_SKIN,
            hair: hex("#a03a1a"),
            hair_style: "pony",
            outfit: "leather",
            top: hex("#4a3a2a"),
            trim: c,
            pants: hex("#2a2420"),
            boots: hex("#1e1a16"),
            cape: hex("#3a3024"),
            weapon: "sword",
            weapon_col: hex("#e8f0ff"),
            offhand: "crossbow",
            ..Hum::default()
        }),
        "clockmaker" => h(Hum {
            skin: C_SKIN,
            hair: HAIR_GREY,
            hair_style: "wild",
            beard: HAIR_GREY,
            outfit: "robe",
            top: hex("#5a4a3a"),
            trim: C_GOLD,
            boots: hex("#3a2a1a"),
            head: "cap",
            head_col: hex("#3a3a5a"),
            weapon: "staff",
            weapon_col: c,
            offhand: "orb",
            off_col: c,
            ..Hum::default()
        }),
        "inquisitor_npc" => h(Hum {
            skin: C_SKIN,
            outfit: "plate",
            top: hex("#8a1e1a"),
            trim: C_GOLD,
            pants: hex("#4a1a18"),
            head: "hood",
            head_col: hex("#7a1a16"),
            cape: hex("#a0261e"),
            weapon: "mace",
            weapon_col: hex("#ffb050"),
            offhand: "symbol",
            off_col: C_GOLD,
            ..Hum::default()
        }),
        "stone_hermit" => h(Hum {
            build: "big",
            skin: hex("#9a9a9a"),
            beard: hex("#7a7a7a"),
            outfit: "loin",
            pants: hex("#5a5048"),
            eyes: hex("#ffd84a"),
            weapon: "club",
            weapon_col: hex("#8a8a8a"),
            ..Hum::default()
        }),
        "tracker" => h(Hum {
            skin: C_SKIN_TAN,
            hair: HAIR_BROWN,
            beard: HAIR_BROWN,
            outfit: "leather",
            top: hex("#6a5a3a"),
            pants: hex("#4a3a2a"),
            head: "hood",
            head_col: hex("#5a6a3a"),
            cape: hex("#4a5a32"),
            weapon: "bow",
            ..Hum::default()
        }),
        "sand_wanderer" => h(Hum {
            skin: C_SKIN_DARK,
            outfit: "robe",
            top: hex("#f0e8d8"),
            trim: c,
            boots: hex("#c8b898"),
            head: "turban",
            head_col: hex("#f8f4ea"),
            mask: hex("#e8dcc4"),
            weapon: "staff",
            weapon_col: c,
            ..Hum::default()
        }),
        "old_voivode" => h(Hum {
            skin: C_SKIN,
            hair: HAIR_GREY,
            beard: HAIR_GREY,
            outfit: "plate",
            top: hex("#8a8a90"),
            trim: c,
            pants: C_IRON,
            head: "helmet",
            head_col: hex("#a0a0a8"),
            cape: hex("#8a2a1a"),
            weapon: "sword",
            shield: hex("#8a2a1a"),
            shield_mark: C_GOLD,
            ..Hum::default()
        }),
        "assassin_master" => h(Hum {
            skin: hex("#d8c8c0"),
            outfit: "leather",
            top: hex("#141418"),
            trim: c,
            pants: hex("#101014"),
            boots: hex("#0a0a0c"),
            head: "hood",
            head_col: hex("#18181c"),
            mask: hex("#0c0c10"),
            cape: hex("#1a1a20"),
            weapon: "dagger",
            weapon_col: c,
            offhand: "dagger",
            off_col: c,
            ..Hum::default()
        }),
        "archmage_npc" => h(Hum {
            skin: C_SKIN,
            hair: hex("#f0f0f0"),
            hair_style: "long",
            beard: hex("#f4f4f4"),
            outfit: "robe",
            top: hex("#2a3a8a"),
            trim: c,
            boots: hex("#1a2450"),
            head: "wizard",
            head_col: hex("#2a3a8a"),
            weapon: "staff",
            weapon_col: c,
            offhand: "book",
            off_col: hex("#4a2a6a"),
            ..Hum::default()
        }),
        "oracle_npc" => h(Hum {
            skin: C_SKIN_TAN,
            hair: hex("#e0e0e8"),
            hair_style: "long",
            outfit: "robe",
            top: hex("#e8eef8"),
            trim: c,
            boots: hex("#a0a8b8"),
            mask: hex("#8a9ab0"),
            offhand: "orb",
            off_col: c,
            ..Hum::default()
        }),
        "fire_keeper" => h(Hum {
            skin: hex("#8a5a3a"),
            hair: HAIR_BLACK,
            hair_style: "wild",
            outfit: "fur",
            top: hex("#5a3a2a"),
            trim: c,
            head: "horns",
            head_col: hex("#3a2a20"),
            glow: true,
            eyes: hex("#ffa040"),
            weapon: "totem",
            weapon_col: c,
            ..Hum::default()
        }),
        "nameless" => h(Hum {
            skin: hex("#b8a8b0"),
            outfit: "robe",
            top: hex("#20102a"),
            trim: c,
            boots: hex("#140a1a"),
            head: "hood",
            head_col: hex("#1a0a24"),
            glow: true,
            eyes: c,
            weapon: "skullstaff",
            weapon_col: c,
            ..Hum::default()
        }),
        "moon_druid" => h(Hum {
            skin: hex("#e8e8f4"),
            hair: hex("#c0c8f0"),
            hair_style: "long",
            outfit: "robe",
            top: hex("#2a3060"),
            trim: c,
            boots: hex("#1a1e40"),
            head: "circlet",
            head_col: hex("#e0e8ff"),
            weapon: "staff",
            weapon_col: c,
            ..Hum::default()
        }),
        "mourner" => h(Hum {
            skin: hex("#d8d8d8"),
            hair: HAIR_GREY,
            hair_style: "long",
            outfit: "robe",
            top: hex("#2a2e34"),
            trim: c,
            boots: hex("#1a1c20"),
            head: "hood",
            head_col: hex("#30343a"),
            offhand: "symbol",
            off_col: c,
            ..Hum::default()
        }),
        "star_archer" => h(Hum {
            skin: C_SKIN,
            hair: hex("#e0e4f0"),
            hair_style: "long",
            outfit: "leather",
            top: hex("#2a3460"),
            trim: c,
            pants: hex("#1a2040"),
            cape: hex("#d0d8ff"),
            weapon: "bow",
            weapon_col: c,
            ..Hum::default()
        }),
        "wild_huntsman" => h(Hum {
            build: "big",
            skin: hex("#a8b8a0"),
            outfit: "fur",
            top: hex("#3a4a32"),
            trim: c,
            head: "horns",
            head_col: hex("#6a5a3a"),
            glow: true,
            eyes: hex("#c0ffc0"),
            cape: hex("#2a3424"),
            weapon: "spear",
            weapon_col: c,
            ..Hum::default()
        }),
        "grail_keeper" => h(Hum {
            skin: C_SKIN,
            hair: HAIR_GREY,
            beard: HAIR_GREY,
            outfit: "plate",
            top: hex("#f0f0f8"),
            trim: C_GOLD,
            pants: hex("#c8c8d8"),
            cape: hex("#e8d8a0"),
            weapon: "spear",
            weapon_col: hex("#fff4c0"),
            offhand: "orb",
            off_col: c,
            ..Hum::default()
        }),
        "fallen_paladin" => h(Hum {
            skin: hex("#c8b8c0"),
            outfit: "plate",
            top: hex("#2a2030"),
            trim: c,
            pants: hex("#1a141e"),
            head: "helmet",
            head_col: hex("#2a2030"),
            glow: true,
            eyes: hex("#c060ff"),
            cape: hex("#3a1a4a"),
            weapon: "sword",
            weapon_col: c,
            shield: hex("#2a2030"),
            shield_mark: c,
            ..Hum::default()
        }),
        "drunken_npc" => h(Hum {
            build: "big",
            skin: hex("#e8b090"),
            hair: HAIR_GREY,
            beard: HAIR_GREY,
            outfit: "robe",
            top: hex("#c08a40"),
            trim: hex("#6a3a1a"),
            boots: hex("#5a3a1a"),
            weapon: "staff",
            weapon_col: hex("#8a6a4a"),
            offhand: "orb",
            off_col: hex("#a06a30"),
            ..Hum::default()
        }),
        "dragon_monk" => h(Hum {
            skin: C_SKIN_TAN,
            outfit: "robe",
            top: hex("#b02a1a"),
            trim: C_GOLD,
            boots: hex("#4a1a10"),
            weapon: "staff",
            weapon_col: c,
            ..Hum::default()
        }),
        "dirge_singer" => h(Hum {
            skin: hex("#d8d4dc"),
            hair: HAIR_BLACK,
            hair_style: "long",
            outfit: "robe",
            top: hex("#1a1820"),
            trim: c,
            boots: hex("#121016"),
            head: "feathers",
            head_col: hex("#2a2a30"),
            cape: hex("#1e1c24"),
            weapon: "staff",
            weapon_col: c,
            ..Hum::default()
        }),
        "rune_carver" => h(Hum {
            build: "big",
            skin: C_SKIN,
            hair: hex("#c87a3a"),
            hair_style: "long",
            beard: hex("#c87a3a"),
            outfit: "fur",
            top: hex("#5a5a6a"),
            trim: c,
            head: "horned",
            head_col: C_STEEL,
            weapon: "hammer",
            weapon_col: c,
            ..Hum::default()
        }),
        "plague_doc" => h(Hum {
            skin: hex("#c8c0b0"),
            outfit: "robe",
            top: hex("#1a1a1a"),
            trim: c,
            boots: hex("#0e0e0e"),
            head: "wizard",
            head_col: hex("#141414"),
            mask: hex("#e8e0c8"),
            weapon: "staff",
            weapon_col: c,
            ..Hum::default()
        }),
        "old_alchemist" => h(Hum {
            skin: C_SKIN,
            hair: hex("#f0f0f0"),
            beard: hex("#f4f4f4"),
            outfit: "robe",
            top: hex("#5a2a6a"),
            trim: C_GOLD,
            boots: hex("#3a1a4a"),
            head: "cap",
            head_col: hex("#3a1a4a"),
            offhand: "orb",
            off_col: c,
            ..Hum::default()
        }),
        "sun_smith" => h(Hum {
            skin: C_SKIN_TAN,
            beard: hex("#c08a3a"),
            outfit: "leather",
            top: hex("#8a5a2a"),
            trim: C_GOLD,
            pants: hex("#4a3a30"),
            weapon: "hammer",
            weapon_col: C_GOLD,
            ..Hum::default()
        }),
        "villager" => {
            let tops = [
                "#8a6a4a", "#6a8a5a", "#a05a4a", "#5a6a9a", "#9a8a5a", "#7a5a7a",
            ];
            let hairs = [
                HAIR_BROWN,
                HAIR_BLOND,
                HAIR_BLACK,
                hex("#8a4a2a"),
                HAIR_GREY,
                HAIR_BROWN,
            ];
            let skins = [C_SKIN, C_SKIN_TAN, C_SKIN, C_SKIN_DARK, C_SKIN, C_SKIN_TAN];
            let i = v.rem_euclid(6) as usize;
            let mut x = Hum {
                skin: skins[i],
                hair: hairs[i],
                outfit: "tunic",
                top: hex(tops[i]),
                pants: hex("#4a3a2a"),
                ..Hum::default()
            };
            if v % 2 == 1 {
                x.hair_style = "long";
                x.outfit = "robe";
                x.boots = mul(x.top, 0.7);
            }
            Some(x)
        }
        "adventurer" => h(Hum {
            skin: C_SKIN,
            hair: HAIR_BROWN,
            outfit: "tunic",
            top: c,
            pants: hex("#4a3a2a"),
            weapon: "sword",
            ..Hum::default()
        }),
        "brute" => h(Hum {
            build: "big",
            skin: c,
            outfit: "loin",
            pants: hex("#4a3a2a"),
            eyes: hex("#ffd84a"),
            weapon: "club",
            ..Hum::default()
        }),
        _ => None,
    }
}

/// Paints a beast model, if the name is a beast.
pub fn beast(name: &str, c: Rgba, f: i32) -> Option<Pc> {
    let q = |q: Quad| Some(paint_quad(&q, f));
    match name {
        "wolf" => q(Quad {
            w: 20,
            h: 14,
            body: [9.0, 8.0, 6.5, 3.0],
            head: [15.0, 5.0, 2.7],
            snout: [17.8, 6.3, 1.9, 1.1],
            ear: "pointy",
            tail: "bushy",
            legs: &[4.0, 6.0, 11.0, 13.0],
            leg_w: 2.0,
            leg_top: 9.0,
            fur: c,
            belly: mix(c, C_WHITE, 0.4),
            ..Quad::default()
        }),
        "boar" => q(Quad {
            w: 18,
            h: 12,
            body: [8.0, 6.5, 6.5, 3.6],
            head: [13.6, 6.2, 3.0],
            snout: [16.5, 7.4, 1.6, 1.3],
            ear: "small",
            tail: "short",
            legs: &[3.0, 5.0, 10.0, 12.0],
            leg_w: 2.0,
            leg_top: 8.0,
            fur: c,
            belly: mul(c, 0.85),
            tusk: true,
            mane: true,
            ..Quad::default()
        }),
        "bear" => q(Quad {
            w: 22,
            h: 15,
            body: [9.0, 8.0, 7.8, 4.4],
            head: [16.5, 5.4, 3.0],
            snout: [19.6, 6.6, 1.8, 1.3],
            ear: "round",
            legs: &[3.0, 6.0, 11.0, 14.0],
            leg_w: 3.0,
            leg_top: 10.0,
            fur: c,
            belly: mul(c, 0.85),
            accent: mul(c, 0.55),
            muzzle: mix(c, hex("#f0d8b0"), 0.55),
            ..Quad::default()
        }),
        "rat" => q(Quad {
            w: 15,
            h: 9,
            body: [6.5, 5.0, 4.2, 2.4],
            head: [10.5, 4.6, 2.0],
            snout: [12.8, 5.2, 1.3, 0.9],
            ear: "round",
            tail: "thin",
            legs: &[4.0, 5.0, 8.0, 9.0],
            leg_w: 1.0,
            leg_top: 6.0,
            fur: c,
            belly: mix(c, C_WHITE, 0.3),
            accent: hex("#f0a0a8"),
            ..Quad::default()
        }),
        "salamander" => q(Quad {
            w: 22,
            h: 10,
            body: [11.0, 5.5, 5.8, 2.0],
            head: [17.0, 5.0, 2.1],
            snout: [19.4, 5.4, 1.7, 1.0],
            tail: "long",
            legs: &[7.0, 8.0, 13.0, 14.0],
            leg_w: 1.5,
            leg_top: 6.0,
            fur: c,
            belly: hex("#ffd84a"),
            accent: hex("#ffe86a"),
            spots: true,
            eye: hex("#ffd84a"),
            ..Quad::default()
        }),
        "fire_drake" => q(Quad {
            w: 26,
            h: 20,
            body: [12.0, 12.0, 7.0, 4.0],
            head: [20.5, 5.0, 2.8],
            snout: [23.4, 5.9, 2.0, 1.2],
            neck: true,
            ear: "horns",
            tail: "spiked",
            legs: &[7.0, 9.0, 14.0, 16.0],
            leg_w: 2.4,
            leg_top: 14.0,
            fur: c,
            belly: hex("#ffd070"),
            accent: hex("#ffb040"),
            wing: true,
            eye: hex("#fff06a"),
            ..Quad::default()
        }),
        "spider" => Some(paint_spider(c, NONE, f)),
        "cave_spider_queen" => Some(paint_spider(c, hex("#ff3a3a"), f)),
        "scorpion" => Some(paint_scorpion(c, f)),
        "scarab" => Some(paint_beetle(c, f)),
        "giant_bat" => Some(paint_bat(c, f)),
        "slime" => Some(paint_slime(c, f)),
        "ghost" => Some(paint_ghost(c, f, NONE, false)),
        "banshee" => Some(paint_ghost(c, f, hex("#3a3a58"), false)),
        "wraith" => Some(paint_ghost(c, f, NONE, true)),
        "ice_elemental" => Some(paint_elemental(c, f)),
        "sand_worm" => Some(paint_worm(c, f)),
        "treant" => Some(paint_treant(c, f)),
        _ => None,
    }
}

/// Models that hover above the ground.
pub fn floats(name: &str) -> bool {
    matches!(
        name,
        "giant_bat" | "ghost" | "banshee" | "wraith" | "ice_elemental"
    )
}

/// Looks that differ between individuals.
pub fn variants(name: &str) -> i32 {
    if name == "villager" {
        6
    } else {
        1
    }
}

const ALIASES: &[(&str, &str)] = &[
    ("risen", "zombie"),
    ("tomb_guardian", "golem"),
    ("bone_golem", "golem"),
    ("magma_lord", "magma_golem"),
    ("frost_jarl", "frost_giant"),
    ("ice_wolf", "wolf"),
    ("shadow_wolf", "wolf"),
    ("hunter_npc", "hunter"),
    ("wandering_mage", "battle_mage"),
    ("town_guard", "guard"),
    ("imp_minion", "fire_imp"),
    ("felguard", "demon"),
    ("spirit_wolf", "wolf"),
    ("skeleton_minion", "skeleton"),
    ("skeleton_mage_minion", "skeleton_mage"),
    ("bone_golem_minion", "golem"),
    ("treant_minion", "treant"),
    ("rng_wolf_companion", "wolf"),
    ("rng_hawk", "giant_bat"),
    ("rng_bear_companion", "bear"),
    ("wl_soldier", "guard"),
    ("wr_ghost", "ghost"),
    ("wh_hound", "wolf"),
    ("wh_huntsman", "hunter"),
    ("tm_golem", "golem"),
];

const GLYPH_MODELS: &[(char, &str)] = &[
    ('w', "wolf"),
    ('b', "boar"),
    ('B', "bear"),
    ('r', "rat"),
    ('s', "spider"),
    ('S', "spider"),
    ('z', "skeleton"),
    ('Z', "zombie"),
    ('g', "goblin"),
    ('o', "orc"),
    ('O', "orc"),
    ('T', "cave_troll"),
    ('G', "ghost"),
    ('W', "wraith"),
    ('v', "giant_bat"),
    ('V', "banshee"),
    ('l', "lizardman"),
    ('h', "bandit"),
    ('H', "bandit_chief"),
    ('N', "necromancer"),
    ('L', "lich"),
    ('u', "ghoul"),
    ('M', "mummy"),
    ('j', "slime"),
    ('x', "scorpion"),
    ('e', "ice_elemental"),
    ('i', "fire_imp"),
    ('D', "fire_drake"),
    ('c', "cultist"),
    ('K', "dark_knight"),
    ('Y', "yeti"),
    ('J', "frost_giant"),
    ('Q', "golem"),
    ('E', "efreet"),
    ('U', "demon"),
    ('a', "scarab"),
    ('P', "pharaoh"),
    ('A', "demon_lord"),
];

fn exists(name: &str) -> bool {
    humanoid(name, WHITE, 0).is_some() || beast(name, WHITE, 0).is_some()
}

/// Finds the model for a creature: its explicit model, its definition key
/// (through aliases), a default for its kind or a guess by its glyph.
pub fn resolve(model: &str, def: &str, glyph: char, kind: u8) -> String {
    for name in [model, def] {
        if name.is_empty() {
            continue;
        }
        let name = ALIASES
            .iter()
            .find(|(a, _)| *a == name)
            .map_or(name, |(_, b)| *b);
        if exists(name) {
            return name.to_string();
        }
    }
    match kind {
        KIND_PLAYER => return "adventurer".into(),
        KIND_NPC => return "villager".into(),
        _ => {}
    }
    let lower = glyph.to_lowercase().next().unwrap_or(glyph);
    for g in [glyph, lower] {
        if let Some((_, n)) = GLYPH_MODELS.iter().find(|(c, _)| *c == g) {
            return n.to_string();
        }
    }
    "brute".into()
}

/// Renders one frame of a model (outlined); heroes wear their gear.
pub fn paint_model(name: &str, c: Rgba, variant: i32, frame: i32, gear: &[String]) -> Pc {
    if let Some(p) = beast(name, c, frame) {
        return p.outlined();
    }
    let mut hum = humanoid(name, c, variant).unwrap_or_default();
    if !gear.is_empty() {
        hum = dress(hum, gear);
    }
    paint_humanoid(&hum, frame).outlined()
}

pub const GEAR_MAIN: usize = 0;
pub const GEAR_OFF: usize = 1;
pub const GEAR_HEAD: usize = 2;
pub const GEAR_CHEST: usize = 3;
pub const GEAR_BACK: usize = 4;

fn gear_item(gear: &[String], i: usize) -> Option<&'static ItemDef> {
    let key = gear.get(i)?;
    if key.is_empty() {
        return None;
    }
    content::db().item(key)
}

fn st(s: &'static str) -> &'static str {
    s
}

/// Puts a hero's equipment (right hand, left hand, head, chest, back) on
/// the class model: the model gives face, hair and colours.
pub fn dress(mut h: Hum, gear: &[String]) -> Hum {
    h.weapon = "";
    h.weapon_col = NONE;
    h.shield = NONE;
    h.shield_mark = NONE;
    h.shield_form = "";
    h.offhand = "";
    h.off_col = NONE;
    if let Some(d) = gear_item(gear, GEAR_MAIN) {
        if !d.weapon.is_empty() {
            h.weapon = st(d.weapon.as_str());
            h.weapon_col = weapon_tint(d);
        }
    }
    if let Some(d) = gear_item(gear, GEAR_OFF) {
        if d.kind == "shield" {
            h.shield = mul(hex(&d.color), 0.85);
            h.shield_form = st(d.look.as_str());
            h.shield_mark = if d.look == "round" { NONE } else { C_GOLD };
        } else if d.kind == "weapon" && !d.weapon.is_empty() {
            h.offhand = st(d.weapon.as_str());
            h.off_col = weapon_tint(d);
        } else if d.kind == "offhand" {
            h.offhand = if d.look.is_empty() {
                "orb"
            } else {
                st(d.look.as_str())
            };
            h.off_col = hex(&d.color);
        }
    }
    // headgear: without it the hair shows
    let mask = h.mask;
    h.head = "";
    h.head_col = NONE;
    h.mask = NONE;
    if let Some(d) = gear_item(gear, GEAR_HEAD) {
        h.head = if d.look.is_empty() {
            "cap"
        } else {
            st(d.look.as_str())
        };
        h.head_col = hex(&d.color);
        if h.head == "hood" {
            h.mask = mask;
        }
    }
    if !h.hair.visible() && h.head != "skull" {
        h.hair = HAIR_BROWN;
    }
    if let Some(d) = gear_item(gear, GEAR_CHEST) {
        let c = hex(&d.color);
        match d.look.as_str() {
            "plate" => {
                h.outfit = "plate";
                h.top = c;
                h.pants = mul(c, 0.7);
            }
            "chain" => {
                h.outfit = "plate";
                h.top = mix(c, C_IRON, 0.35);
                h.pants = mul(C_IRON, 0.9);
            }
            "robe" => {
                h.outfit = "robe";
                h.top = c;
                h.boots = mul(c, 0.6);
            }
            "leather" => {
                h.outfit = "leather";
                h.top = c;
                if !h.pants.visible() {
                    h.pants = mul(c, 0.65);
                }
            }
            _ => {
                h.outfit = "tunic";
                h.top = c;
            }
        }
    } else {
        h.outfit = "tunic";
        h.top = hex("#c8b898");
        h.trim = NONE;
        h.pants = hex("#6a5a48");
    }
    h.cape = gear_item(gear, GEAR_BACK).map_or(NONE, |d| hex(&d.color));
    h
}

/// Blades get a hint of the item colour, staves and wands their gem.
fn weapon_tint(d: &ItemDef) -> Rgba {
    let c = hex(&d.color);
    match d.weapon.as_str() {
        "staff" | "wand" => c,
        "bow" => NONE,
        _ => mix(C_STEEL, c, 0.45),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_creature_has_a_model() {
        let db = content::db();
        for m in &db.b.monsters {
            let name = resolve(&m.model, &m.key, m.glyph.chars().next().unwrap_or('?'), 2);
            let p = paint_model(&name, hex(&m.color), 0, 1, &[]);
            assert!(p.w > 4 && p.h > 4, "{}", m.key);
        }
        for c in &db.b.classes {
            let name = resolve(&c.model, &c.key, '@', KIND_PLAYER);
            let gear = vec!["".to_string(); 5];
            let p = paint_model(&name, WHITE, 0, 0, &gear);
            assert!(p.px.iter().any(|c| c.a > 0), "{}", c.key);
        }
    }

    #[test]
    fn modded_glyph_guess() {
        assert_eq!(resolve("", "ice_dragonling", 'D', 2), "fire_drake");
        assert_eq!(resolve("", "frost_wolf", 'w', 2), "wolf");
        assert_eq!(resolve("", "nobody", '?', 2), "brute");
        assert_eq!(resolve("", "ice_wolf", 'w', 2), "wolf");
    }
}
