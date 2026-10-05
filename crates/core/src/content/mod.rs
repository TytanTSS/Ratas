//! All data-driven game definitions: tiles, monsters, items, abilities, the
//! skill tree, classes and NPC roles.
//!
//! Built-in definitions are embedded from data/content/*.toml. Additional TOML
//! (or JSON) files in a mods directory are merged on top: an entry with an
//! existing key replaces the built-in one, a new key is appended. This is how
//! the skill tree and everything else can be extended without touching code.

mod validate;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::atomic::{AtomicPtr, Ordering};

pub use validate::*;

pub type Stats = BTreeMap<String, f64>;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TileDef {
    pub key: String,
    pub name: String,
    pub glyph: String,
    pub fg: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub bg: String,
    pub walkable: bool,
    pub transparent: bool,
    /// movement time multiplier, 0 means 1
    #[serde(skip_serializing_if = "is_zero")]
    pub move_cost: f64,
    /// used by the overworld spawner
    #[serde(skip_serializing_if = "String::is_empty")]
    pub biome: String,
    /// door, chest, stairs_down, stairs_up, dungeon, shrine
    #[serde(skip_serializing_if = "String::is_empty")]
    pub interact: String,
    /// tile key after interaction
    #[serde(skip_serializing_if = "String::is_empty")]
    pub becomes: String,
    /// damage per second while standing here
    #[serde(skip_serializing_if = "is_zero")]
    pub damage: f64,
    /// type of that damage, fire by default
    #[serde(skip_serializing_if = "String::is_empty")]
    pub dmg_type: String,
    /// colour of the light the tile gives off
    #[serde(skip_serializing_if = "String::is_empty")]
    pub light: String,
    #[serde(skip)]
    pub id: u8,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// One kind of damage. Every type has a resistance stat "res_<key>", a damage
/// bonus "<key>_pct" and a flat weapon bonus "add_<key>".
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DamageTypeDef {
    pub key: String,
    pub name: String,
    pub short: String,
    pub group: String,
    pub color: String,
    pub res_name: String,
    pub pct_name: String,
    pub add_name: String,
}

/// The groups usable in resistances: res_physical etc.
pub const DAMAGE_GROUPS: &[(&str, &str)] = &[
    ("physical", "Сопр. физическому урону"),
    ("elemental", "Сопр. стихиям"),
    ("magic", "Сопр. магии"),
    ("all", "Сопр. всему урону"),
];

pub fn damage_group(key: &str) -> Option<&'static str> {
    DAMAGE_GROUPS.iter().find(|(k, _)| *k == key).map(|(_, n)| *n)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct BuffDef {
    pub key: String,
    pub name: String,
    pub duration_ms: i64,
    pub stats: Stats,
    /// negative heals
    pub dot_per_sec: f64,
    pub dmg_type: String,
    pub stun: bool,
    pub silence: bool,
    pub stealth: bool,
    pub taunt: bool,
    pub color: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AbilityDef {
    pub key: String,
    pub name: String,
    pub desc: String,
    /// projectile, nova, heal, dash, cleave, strike, buff, chain, taunt...
    pub kind: String,
    pub mana: f64,
    pub cooldown_ms: i64,
    pub damage: [f64; 2],
    pub dmg_type: String,
    pub leech: f64,
    pub scale: String,
    pub scale_k: f64,
    pub range: i32,
    pub radius: i32,
    pub count: i32,
    pub glyph: String,
    pub color: String,
    /// projectile ms per tile
    pub speed_ms: i32,
    pub buff: Option<BuffDef>,
    pub on_hit: Option<BuffDef>,
    pub equip: String,
    pub split: Vec<String>,
    pub execute: f64,
    pub backstab: f64,
    pub summon: String,
    #[serde(rename = "duration_ms")]
    pub duration: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MonsterDef {
    pub key: String,
    pub name: String,
    pub glyph: String,
    pub color: String,
    pub hp: f64,
    pub damage: [f64; 2],
    pub armor: f64,
    pub move_ms: i32,
    pub attack_ms: i32,
    pub xp: i32,
    pub sight: i32,
    pub behavior: String,
    pub dmg_type: String,
    pub resist: Stats,
    pub on_hit: Option<BuffDef>,
    pub on_hit_pct: f64,
    pub model: String,
    pub role: String,
    pub ability: String,
    pub abilities: Vec<String>,
    pub ally: bool,
    pub depth: [i32; 2],
    pub themes: Vec<String>,
    pub gold: [i32; 2],
    pub weight: i32,
    pub group: [i32; 2],
    pub night: bool,
    pub elite: bool,
    pub boss: bool,
    pub persona: String,
    pub drops: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ItemDef {
    pub key: String,
    pub name: String,
    pub glyph: String,
    pub color: String,
    pub kind: String,
    pub stats: Stats,
    pub damage: [f64; 2],
    pub weapon: String,
    pub hands: i32,
    pub look: String,
    pub unique: bool,
    pub rarity: String,
    pub dmg_type: String,
    pub on_hit: Option<BuffDef>,
    pub on_hit_pct: f64,
    pub heal: f64,
    pub mana: f64,
    pub buff: Option<BuffDef>,
    pub effect: String,
    pub amount: f64,
    pub value: i32,
    pub depth: i32,
    pub weight: i32,
    pub desc: String,
    pub drop_from: Vec<String>,
    pub drop_chance: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BranchDef {
    pub key: String,
    pub name: String,
    pub color: String,
    pub desc: String,
    pub class: String,
    pub subclass: String,
    pub secret: bool,
    pub hidden: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SubclassDef {
    pub key: String,
    pub name: String,
    pub class: String,
    pub desc: String,
    pub color: String,
    pub level: i32,
    pub secret: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SkillDef {
    pub key: String,
    pub name: String,
    pub desc: String,
    pub branch: String,
    pub tier: i32,
    pub max_rank: i32,
    pub level: i32,
    pub requires: Vec<String>,
    pub stats: Stats,
    pub grants: String,
    pub equip: String,
    pub deed: String,
    pub deed_count: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClassDef {
    pub key: String,
    pub name: String,
    pub desc: String,
    pub color: String,
    pub attrs: Stats,
    pub abilities: Vec<String>,
    pub items: Vec<String>,
    pub model: String,
    pub secret: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NpcRoleDef {
    pub key: String,
    pub name: String,
    pub glyph: String,
    pub color: String,
    pub greetings: Vec<String>,
    pub lines: Vec<String>,
    pub trader: bool,
    pub goods: Vec<String>,
    pub quest_giver: bool,
    pub wander: i32,
    pub persona: String,
    pub count: [i32; 2],
    pub model: String,
    pub combat: String,
    pub world: bool,
    pub stories: Vec<String>,
    pub city_count: [i32; 2],
    pub building: String,
    pub stock: i32,
    pub stock_kinds: Vec<String>,
    pub services: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SquadMember {
    pub monster: String,
    pub count: [i32; 2],
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SquadDef {
    pub key: String,
    pub name: String,
    pub themes: Vec<String>,
    pub depth: [i32; 2],
    pub weight: i32,
    pub night: bool,
    pub members: Vec<SquadMember>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UniqueDef {
    pub key: String,
    pub name: String,
    pub title: String,
    pub color: String,
    pub model: String,
    pub persona: String,
    pub greeting: String,
    pub about: Vec<String>,
    pub biomes: Vec<String>,
    /// slay, boss, relics
    pub quest: String,
    pub offer: String,
    pub done: String,
    pub target: String,
    pub champion: String,
    pub count: i32,
    pub sources: Vec<String>,
    /// item:<key>, class:<key>, subclass:<key>, skill:<key>
    pub reward: String,
}

/// The serializable set of all definitions. It is also what a server sends to
/// joining clients so both sides agree on content.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Bundle {
    pub damage_types: Vec<DamageTypeDef>,
    pub subclasses: Vec<SubclassDef>,
    pub squads: Vec<SquadDef>,
    pub uniques: Vec<UniqueDef>,
    pub tiles: Vec<TileDef>,
    pub monsters: Vec<MonsterDef>,
    pub items: Vec<ItemDef>,
    pub abilities: Vec<AbilityDef>,
    pub branches: Vec<BranchDef>,
    pub skills: Vec<SkillDef>,
    pub classes: Vec<ClassDef>,
    pub npcs: Vec<NpcRoleDef>,
    /// Translations of a mod's texts: language → Russian text → translation.
    pub translations: BTreeMap<String, HashMap<String, String>>,
}

trait Keyed {
    fn key(&self) -> &str;
}

macro_rules! keyed {
    ($($t:ty),*) => { $(impl Keyed for $t { fn key(&self) -> &str { &self.key } })* };
}
keyed!(DamageTypeDef, SubclassDef, SquadDef, UniqueDef, TileDef, MonsterDef, ItemDef, AbilityDef, BranchDef, SkillDef, ClassDef, NpcRoleDef);

fn merge_by_key<T: Keyed + Clone>(dst: &mut Vec<T>, src: &[T]) {
    for s in src {
        match dst.iter_mut().find(|d| d.key() == s.key()) {
            Some(d) => *d = s.clone(),
            None => dst.push(s.clone()),
        }
    }
}

impl Bundle {
    /// Overlays o on self (same key = replace, new key = append).
    pub fn merge(&mut self, o: &Bundle) {
        for (l, m) in &o.translations {
            let e = self.translations.entry(l.clone()).or_default();
            for (k, v) in m {
                e.insert(k.clone(), v.clone());
            }
        }
        merge_by_key(&mut self.damage_types, &o.damage_types);
        merge_by_key(&mut self.subclasses, &o.subclasses);
        merge_by_key(&mut self.squads, &o.squads);
        merge_by_key(&mut self.uniques, &o.uniques);
        merge_by_key(&mut self.tiles, &o.tiles);
        merge_by_key(&mut self.monsters, &o.monsters);
        merge_by_key(&mut self.items, &o.items);
        merge_by_key(&mut self.abilities, &o.abilities);
        merge_by_key(&mut self.branches, &o.branches);
        merge_by_key(&mut self.skills, &o.skills);
        merge_by_key(&mut self.classes, &o.classes);
        merge_by_key(&mut self.npcs, &o.npcs);
    }
}

/// An indexed Bundle.
pub struct Db {
    pub b: Bundle,
    tile_by_key: HashMap<String, usize>,
    monsters: HashMap<String, usize>,
    items: HashMap<String, usize>,
    abilities: HashMap<String, usize>,
    skills: HashMap<String, usize>,
    classes: HashMap<String, usize>,
    npcs: HashMap<String, usize>,
    branches: HashMap<String, usize>,
    dmg_types: HashMap<String, usize>,
    subs: HashMap<String, usize>,
    squads: HashMap<String, usize>,
    uniques: HashMap<String, usize>,
    pub json: Vec<u8>,
}

static ACTIVE: AtomicPtr<Db> = AtomicPtr::new(std::ptr::null_mut());

/// Installs db as the active content set. The previous set is leaked on
/// purpose: references to it may still be held by a renderer, and content
/// changes only a few times per run (start, joining another server).
pub fn install(d: Db) -> &'static Db {
    for (l, m) in &d.b.translations {
        crate::i18n::add(l, m);
    }
    let p = Box::into_raw(Box::new(d));
    ACTIVE.store(p, Ordering::Release);
    unsafe { &*p }
}

/// The active content set; the built-in content is installed on first use.
pub fn db() -> &'static Db {
    let p = ACTIVE.load(Ordering::Acquire);
    if p.is_null() {
        let d = load_default(None).expect("built-in content").0;
        return install(d);
    }
    unsafe { &*p }
}

/// Parses a content file; unknown keys are errors, so that typos in mods do
/// not silently do nothing.
pub fn decode_toml(text: &str) -> Result<Bundle, String> {
    toml::from_str(text).map_err(|e| e.to_string())
}

/// Loads built-in content and merges mod files from mods_dir (if it exists).
/// Returns the db and the names of loaded mod files.
pub fn load_default(mods_dir: Option<&Path>) -> Result<(Db, Vec<String>), String> {
    let mut b = Bundle::default();
    for (name, text) in crate::embedded::CONTENT {
        let part = decode_toml(text).map_err(|e| format!("builtin {name}: {e}"))?;
        b.merge(&part);
    }
    let mut loaded = Vec::new();
    if let Some(dir) = mods_dir {
        if let Ok(rd) = std::fs::read_dir(dir) {
            let mut files: Vec<_> = rd
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| matches!(p.extension().and_then(|x| x.to_str()), Some("toml") | Some("json")))
                .collect();
            files.sort();
            for f in files {
                let text = std::fs::read_to_string(&f).map_err(|e| e.to_string())?;
                let name = f.file_name().unwrap().to_string_lossy().to_string();
                let part = if f.extension().and_then(|x| x.to_str()) == Some("toml") {
                    decode_toml(&text)
                } else {
                    serde_json::from_str(&text).map_err(|e| e.to_string())
                }
                .map_err(|e| format!("mod {name}: {e}"))?;
                b.merge(&part);
                loaded.push(name);
            }
        }
    }
    Ok((index(b)?, loaded))
}

/// Builds a DB from a serialized Bundle (used by network clients).
pub fn from_json(data: &[u8]) -> Result<Db, String> {
    let b: Bundle = serde_json::from_slice(data).map_err(|e| e.to_string())?;
    index(b)
}

fn idx<T: Keyed>(v: &[T]) -> HashMap<String, usize> {
    v.iter().enumerate().map(|(i, t)| (t.key().to_string(), i)).collect()
}

/// Validates a bundle and builds lookup tables.
pub fn index(mut b: Bundle) -> Result<Db, String> {
    if b.tiles.len() > 255 {
        return Err(format!("too many tiles: {} (max 255)", b.tiles.len()));
    }
    for sc in &mut b.subclasses {
        if sc.level <= 0 {
            sc.level = 5;
        }
    }
    for it in &mut b.items {
        match it.kind.as_str() {
            "armor" => it.kind = "chest".into(),
            "trinket" => it.kind = "ring".into(),
            _ => {}
        }
        if it.kind == "weapon" && it.hands == 0 {
            it.hands = 1;
        }
    }
    for (i, t) in b.tiles.iter_mut().enumerate() {
        t.id = i as u8;
        if t.move_cost == 0.0 {
            t.move_cost = 1.0;
        }
    }
    for s in &mut b.skills {
        if s.max_rank <= 0 {
            s.max_rank = 1;
        }
    }
    let json = serde_json::to_vec(&b).unwrap_or_default();
    let d = Db {
        tile_by_key: idx(&b.tiles),
        monsters: idx(&b.monsters),
        items: idx(&b.items),
        abilities: idx(&b.abilities),
        skills: idx(&b.skills),
        classes: idx(&b.classes),
        npcs: idx(&b.npcs),
        branches: idx(&b.branches),
        dmg_types: idx(&b.damage_types),
        subs: idx(&b.subclasses),
        squads: idx(&b.squads),
        uniques: idx(&b.uniques),
        json,
        b,
    };
    let problems = d.validate();
    if !problems.is_empty() {
        return Err(format!("content errors:\n  {}", problems.join("\n  ")));
    }
    Ok(d)
}

impl Db {
    pub fn tile(&self, id: u8) -> &TileDef {
        self.b.tiles.get(id as usize).unwrap_or(&self.b.tiles[0])
    }
    /// The id of a tile key; it panics on unknown keys because generators
    /// rely on the built-in tile set.
    pub fn tile_id(&self, key: &str) -> u8 {
        match self.tile_by_key.get(key) {
            Some(&i) => i as u8,
            None => panic!("unknown tile {key}"),
        }
    }
    pub fn has_tile(&self, key: &str) -> bool {
        self.tile_by_key.contains_key(key)
    }
    pub fn tile_by_key(&self, key: &str) -> Option<&TileDef> {
        self.tile_by_key.get(key).map(|&i| &self.b.tiles[i])
    }
    pub fn monster(&self, key: &str) -> Option<&MonsterDef> {
        self.monsters.get(key).map(|&i| &self.b.monsters[i])
    }
    pub fn item(&self, key: &str) -> Option<&ItemDef> {
        self.items.get(key).map(|&i| &self.b.items[i])
    }
    pub fn ability(&self, key: &str) -> Option<&AbilityDef> {
        self.abilities.get(key).map(|&i| &self.b.abilities[i])
    }
    pub fn skill(&self, key: &str) -> Option<&SkillDef> {
        self.skills.get(key).map(|&i| &self.b.skills[i])
    }
    pub fn class(&self, key: &str) -> Option<&ClassDef> {
        self.classes.get(key).map(|&i| &self.b.classes[i])
    }
    pub fn npc_role(&self, key: &str) -> Option<&NpcRoleDef> {
        self.npcs.get(key).map(|&i| &self.b.npcs[i])
    }
    pub fn branch(&self, key: &str) -> Option<&BranchDef> {
        self.branches.get(key).map(|&i| &self.b.branches[i])
    }
    pub fn damage_type(&self, key: &str) -> Option<&DamageTypeDef> {
        self.dmg_types.get(key).map(|&i| &self.b.damage_types[i])
    }
    pub fn subclass(&self, key: &str) -> Option<&SubclassDef> {
        self.subs.get(key).map(|&i| &self.b.subclasses[i])
    }
    pub fn squad(&self, key: &str) -> Option<&SquadDef> {
        self.squads.get(key).map(|&i| &self.b.squads[i])
    }
    pub fn unique(&self, key: &str) -> Option<&UniqueDef> {
        self.uniques.get(key).map(|&i| &self.b.uniques[i])
    }
    /// The subclasses of a class in definition order.
    pub fn subclasses_of(&self, class: &str) -> Vec<&SubclassDef> {
        self.b.subclasses.iter().filter(|s| s.class == class).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_content_loads() {
        let (d, _) = load_default(None).unwrap();
        assert!(d.b.tiles.len() > 50);
        assert!(d.monster("skeleton").is_some() || d.b.monsters.len() > 20);
        assert_eq!(d.tile(d.tile_id("grass")).key, "grass");
    }

    /// Keys must not repeat across built-in files: a repeat silently replaces
    /// an earlier definition.
    #[test]
    fn builtin_keys_unique() {
        let mut seen: HashMap<String, String> = HashMap::new();
        let mut dups = Vec::new();
        for (name, text) in crate::embedded::CONTENT {
            let b = decode_toml(text).unwrap();
            let mut keys: Vec<String> = Vec::new();
            macro_rules! add {
                ($v:expr, $kind:literal) => {
                    for x in &$v {
                        keys.push(format!("{}:{}", $kind, x.key()));
                    }
                };
            }
            add!(b.damage_types, "damage");
            add!(b.subclasses, "subclass");
            add!(b.squads, "squad");
            add!(b.uniques, "unique");
            add!(b.tiles, "tile");
            add!(b.monsters, "monster");
            add!(b.items, "item");
            add!(b.abilities, "ability");
            add!(b.branches, "branch");
            add!(b.skills, "skill");
            add!(b.classes, "class");
            add!(b.npcs, "npc");
            for k in keys {
                if let Some(prev) = seen.insert(k.clone(), name.to_string()) {
                    dups.push(format!("{k} in {prev} and {name}"));
                }
            }
        }
        assert!(dups.is_empty(), "duplicate keys: {dups:?}");
    }

    #[test]
    fn unknown_keys_are_errors() {
        assert!(decode_toml("[[tiles]]\nkey='x'\nwalkabel=true\n").is_err());
    }
}
