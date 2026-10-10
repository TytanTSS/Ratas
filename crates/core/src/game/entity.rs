//! Entities and their derived stats.

use super::*;
pub use crate::content::gear_need_name;
use crate::content::{db, BuffDef, ItemDef, Stats as StatMap};
use crate::world::Vec2;
use std::collections::{BTreeMap, HashMap};

pub type Id = u32;

/// The radius of a creature's body in tiles; bosses are bigger.
pub const BODY_RADIUS: f32 = 0.32;
pub const BOSS_RADIUS: f32 = 0.45;
/// creatures for a party are big
pub const BIG_RADIUS: f32 = 0.41;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Kind {
    #[default]
    Player = 1,
    Monster = 2,
    Npc = 3,
    Item = 4,
    Projectile = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Faction {
    #[default]
    Neutral,
    Player,
    Monster,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ItemStack {
    pub key: String,
    pub qty: i32,
    pub bonus: StatMap,
    pub suffix: String,
    /// rolled rarity (see rarity.rs)
    pub rarity: i32,
}

impl ItemStack {
    pub fn new(key: &str) -> ItemStack {
        ItemStack {
            key: key.into(),
            qty: 1,
            ..Default::default()
        }
    }
    pub fn qty(key: &str, qty: i32) -> ItemStack {
        ItemStack {
            key: key.into(),
            qty,
            ..Default::default()
        }
    }
    pub fn def(&self) -> Option<&'static ItemDef> {
        db().item(&self.key)
    }
    pub fn name(&self) -> String {
        match self.def() {
            None => self.key.clone(),
            Some(d) if !self.suffix.is_empty() => format!("{} {}", d.name, self.suffix),
            Some(d) => d.name.clone(),
        }
    }
    pub fn value(&self) -> i32 {
        let Some(d) = self.def() else { return 0 };
        let n = self.bonus.len() as i32;
        let v = (d.value + n * d.value / 2 + 10 * n) as f64;
        if self.rarity == 0 && n > 0 {
            return v as i32; // a magic item of an old save
        }
        (v * RARITY_VALUE[clamp_rarity(self.item_rarity())]
            / RARITY_VALUE[clamp_rarity(def_rarity(Some(d)))]) as i32
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Buff {
    pub def: BuffDef,
    pub until: f64,
    pub source: Id,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Entity {
    pub id: Id,
    pub kind: Kind,
    pub name: String,
    pub glyph: String,
    pub color: String,
    pub level: String,
    pub pos: Vec2,
    /// the direction it looks at, radians (0 = east, π/2 = south)
    pub facing: f32,
    pub faction: Faction,

    pub hp: f64,
    pub max_hp: f64,
    pub mp: f64,
    pub max_mp: f64,
    pub dead: bool,

    pub next_attack: f64,
    #[serde(skip)]
    pub swings: u8,
    pub cooldowns: HashMap<String, f64>,
    pub buffs: Vec<Buff>,
    #[serde(skip)]
    pub speech: String,
    #[serde(skip)]
    pub speech_until: f64,

    pub monster: Option<Box<MonsterState>>,
    pub player: Option<Box<PlayerState>>,
    pub npc: Option<Box<NpcState>>,
    pub item: Option<ItemStack>,
    #[serde(skip)]
    pub proj: Option<Box<ProjState>>,

    /// summoned by this entity (fights on its side)
    pub owner: Id,
    /// summons and illusions vanish at this time
    pub expires: f64,

    /// the direction it wants to move this tick (unit length or zero)
    #[serde(skip)]
    pub want: Vec2,
    /// the point it walks to (AI)
    #[serde(skip)]
    pub goal: Option<Vec2>,
    /// actual velocity of the last tick (tiles per second), for clients
    #[serde(skip)]
    pub vel: Vec2,
    /// slower walk (townsfolk, travellers)
    #[serde(skip)]
    pub pace: f32,

    #[serde(skip)]
    pub stats: Stats,
}

impl Entity {
    pub fn blocks(&self) -> bool {
        matches!(self.kind, Kind::Player | Kind::Monster | Kind::Npc)
    }
    pub fn alive(&self) -> bool {
        !self.dead && self.hp > 0.0
    }
    /// The radius of the body in tiles.
    pub fn radius(&self) -> f32 {
        match self.kind {
            Kind::Item => 0.2,
            Kind::Projectile => 0.12,
            _ => match self.monster.as_ref().and_then(|m| db().monster(&m.def)) {
                Some(d) if d.boss => BOSS_RADIUS,
                Some(d) if d.party > 1 => BIG_RADIUS,
                _ => BODY_RADIUS,
            },
        }
    }
    /// The tile it stands on.
    pub fn cell(&self) -> Pos {
        self.pos.cell()
    }
    pub fn dist(&self, o: &Entity) -> f32 {
        self.pos.dist(o.pos)
    }
    /// Distance between the edges of two bodies.
    pub fn gap(&self, o: &Entity) -> f32 {
        self.pos.dist(o.pos) - self.radius() - o.radius()
    }
    pub fn face_to(&mut self, p: Vec2) {
        let d = p - self.pos;
        if d.len_sq() > 1e-6 {
            self.facing = d.angle();
        }
    }
    pub fn facing_vec(&self) -> Vec2 {
        Vec2::from_angle(self.facing)
    }
    pub fn p(&self) -> &PlayerState {
        self.player.as_ref().expect("player")
    }
    pub fn pm(&mut self) -> &mut PlayerState {
        self.player.as_mut().expect("player")
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MonsterState {
    pub def: String,
    pub lvl: i32,
    pub home: Vec2,
    #[serde(skip)]
    pub target: Id,
    #[serde(skip)]
    pub last_seen: Vec2,
    #[serde(skip)]
    pub last_seen_at: f64,
    #[serde(skip)]
    pub path: Vec<Pos>,
    #[serde(skip)]
    pub path_goal: Pos,
    #[serde(skip)]
    pub path_at: f64,
    #[serde(skip)]
    pub next_think: f64,
    pub state: String,
    #[serde(skip)]
    pub flee_until: f64,
    #[serde(skip)]
    pub tactic: String,
    #[serde(skip)]
    pub tactic_until: f64,
    #[serde(skip)]
    pub next_llm: f64,
    #[serde(skip)]
    pub llm_pending: bool,
    pub damage: [f64; 2],
    pub armor: f64,
    pub move_ms: f64,
    pub attack_ms: f64,
    pub xp: i32,
    /// never despawns
    pub persistent: bool,
    #[serde(skip)]
    pub events: Vec<String>,
    /// unique whose quest this champion is
    pub champion: String,
    /// members of one squad fight together
    pub squad: i32,
    #[serde(skip)]
    pub retreat_at: f64,
    #[serde(skip)]
    pub buffed_at: f64,
    #[serde(skip)]
    pub retarget: f64,
    #[serde(skip)]
    pub strafe_until: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Turn {
    pub who: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NpcState {
    pub role: String,
    pub pname: String,
    pub home: Vec2,
    pub village: String,
    pub gold: i32,
    pub memory: HashMap<String, Vec<Turn>>,
    #[serde(skip)]
    pub next_think: f64,
    #[serde(skip)]
    pub next_chat: f64,
    #[serde(skip)]
    pub busy: bool,
    /// player -> time of last gift
    #[serde(skip)]
    pub gifts: HashMap<String, f64>,
    /// a unique character of the world
    pub unique: String,
    /// conversations with each player
    pub met: HashMap<String, i32>,
    /// lines already told (avoid repeating)
    #[serde(skip)]
    pub said: std::collections::HashSet<String>,
    pub travel: Option<Vec2>,
    #[serde(skip)]
    pub travel_until: f64,
    /// citizens: where to spend the night
    pub night: Option<Vec2>,
    /// city traders: today's rare goods
    pub stock: Option<Vec<ItemStack>>,
    pub stock_day: i32,
}

#[derive(Clone, Debug, Default)]
pub struct ProjState {
    pub owner: Id,
    pub faction: Faction,
    pub vel: Vec2,
    pub left: f32,
    pub damage: Damage,
    pub radius: i32,
    pub on_hit: Option<BuffDef>,
    pub ability: String,
}

/// What a character holds; skills and abilities may need it.
#[derive(Clone, Debug, Default)]
pub struct Gear {
    /// weapon type in the right hand ("" = unarmed)
    pub weapon: String,
    pub two_hand: bool,
    pub dual: bool,
    pub shield: bool,
    /// bow or crossbow
    pub ranged: bool,
    pub has_items: bool,
}

impl Gear {
    /// Reports whether the gear satisfies a requirement.
    pub fn has(&self, need: &str) -> bool {
        match need {
            "" => true,
            "weapon" => !self.weapon.is_empty(),
            "melee" => !self.weapon.is_empty() && !self.ranged,
            "shield" => self.shield,
            "twohand_dual" => (self.two_hand && !self.ranged) || self.dual,
            "bow" => self.ranged,
            _ => false,
        }
    }
}

/// Derived from attributes, equipment, skills and buffs.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub str_: f64,
    pub dex: f64,
    pub int: f64,
    pub vit: f64,
    pub max_hp: f64,
    pub max_mp: f64,
    pub hp_regen: f64,
    pub mp_regen: f64,
    pub armor: f64,
    pub dodge: f64,
    pub crit: f64,
    pub crit_mult: f64,
    pub melee_pct: f64,
    pub spell_pct: f64,
    pub ranged_pct: f64,
    pub attack_speed: f64,
    pub move_speed: f64,
    pub sight: i32,
    pub gold_find: f64,
    pub weapon_dmg: [f64; 2],
    pub move_ms: f64,
    pub attack_ms: f64,

    /// every modifier: attributes, gear, skills, buffs
    pub mods: HashMap<String, f64>,
    /// highest possible resistance
    pub res_cap: f64,
    /// damage type of melee attacks
    pub weapon_type: String,
    pub weapon_on_hit: Option<BuffDef>,
    pub on_hit_pct: f64,
    pub life_leech: f64,
    pub thorns: f64,
    pub stunned: bool,
    pub silenced: bool,
    pub stealthed: bool,
    pub taunter: Id,

    pub block: f64,
    pub fury: f64,
    pub duel_pct: f64,
    pub ambush_pct: f64,
    pub heal_pct: f64,
    pub mimic_pct: f64,
    /// melee attacks reach this many tiles further
    pub reach: f64,

    pub gear: Gear,
    pub off_dmg: [f64; 2],
    pub off_type: String,
    pub off_on_hit: Option<BuffDef>,
    pub off_pct: f64,
}

impl Stats {
    pub fn m(&self, k: &str) -> f64 {
        self.mods.get(k).copied().unwrap_or(0.0)
    }
    /// The effective resistance to a damage type in percent: its own stat plus
    /// its group and "all", clamped to [-100, res_cap].
    pub fn resist(&self, t: &str) -> f64 {
        let mut v = self.m(&format!("res_{t}")) + self.m("res_all");
        if let Some(d) = db().damage_type(t) {
            v += self.m(&format!("res_{}", d.group));
        }
        v.min(self.res_cap).max(-100.0)
    }
    /// The damage bonus in percent for a damage type.
    pub fn type_bonus(&self, t: &str) -> f64 {
        self.m(&format!("{t}_pct"))
    }

    /// Derived stats for the character sheet.
    pub fn map(&self) -> BTreeMap<String, f64> {
        let mut m: BTreeMap<String, f64> = [
            ("str", self.str_),
            ("dex", self.dex),
            ("int", self.int),
            ("vit", self.vit),
            ("max_hp", self.max_hp),
            ("max_mp", self.max_mp),
            ("hp_regen", self.hp_regen),
            ("mp_regen", self.mp_regen),
            ("armor", self.armor),
            ("dodge", self.dodge),
            ("crit", self.crit),
            ("crit_mult", self.crit_mult),
            ("melee_pct", self.melee_pct),
            ("spell_pct", self.spell_pct),
            ("ranged_pct", self.ranged_pct),
            ("attack_speed", self.attack_speed),
            ("move_speed", self.move_speed),
            ("sight", self.sight as f64),
            ("gold_find", self.gold_find),
            ("dmg_min", self.weapon_dmg[0]),
            ("dmg_max", self.weapon_dmg[1]),
            ("life_leech", self.life_leech),
            ("thorns", self.thorns),
            ("block", self.block),
            ("fury", self.fury),
            ("duel_pct", self.duel_pct),
            ("ambush_pct", self.ambush_pct),
            ("heal_pct", self.heal_pct),
            ("mimic_pct", self.mimic_pct),
            ("reach", self.reach),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        for t in &db().b.damage_types {
            m.insert(format!("res_{}", t.key), self.resist(&t.key));
            let v = self.type_bonus(&t.key);
            if v != 0.0 {
                m.insert(format!("{}_pct", t.key), v);
            }
            let v = self.m(&format!("add_{}", t.key));
            if v != 0.0 {
                m.insert(format!("add_{}", t.key), v);
            }
        }
        m
    }
}

fn add_mods(dst: &mut HashMap<String, f64>, src: &StatMap, k: f64) {
    for (key, v) in src {
        *dst.entry(key.clone()).or_insert(0.0) += v * k;
    }
}

impl Entity {
    /// Recomputes derived stats; it keeps the HP/MP fraction when maxima change.
    pub fn recalc(&mut self) {
        let d = db();
        let mut mods: HashMap<String, f64> = HashMap::new();
        let mut s = Stats::default();
        for b in &self.buffs {
            add_mods(&mut mods, &b.def.stats, 1.0);
            s.stunned |= b.def.stun;
            s.silenced |= b.def.silence;
            s.stealthed |= b.def.stealth;
            if b.def.taunt {
                s.taunter = b.source;
            }
        }
        s.res_cap = 75.0;
        s.weapon_type = "blunt".into();
        let get = |m: &HashMap<String, f64>, k: &str| m.get(k).copied().unwrap_or(0.0);
        if let Some(p) = &self.player {
            add_mods(&mut mods, &p.attrs, 1.0);
            s.weapon_dmg = [1.0, 3.0];
            // gear first: some skills only work with certain equipment
            for (slot, it) in &p.equip {
                let Some(def) = it.def() else { continue };
                s.gear.has_items = true;
                let k = it.stat_k();
                add_mods(&mut mods, &def.stats, k);
                add_mods(&mut mods, &it.bonus, 1.0);
                if slot == SLOT_MAIN && def.kind == "weapon" {
                    s.weapon_dmg = [(def.damage[0] * k).round(), (def.damage[1] * k).round()];
                    if !def.dmg_type.is_empty() {
                        s.weapon_type = def.dmg_type.clone();
                    }
                    s.weapon_on_hit = def.on_hit.clone();
                    s.on_hit_pct = def.on_hit_pct;
                    s.gear.weapon = if def.weapon.is_empty() {
                        "sword".into()
                    } else {
                        def.weapon.clone()
                    };
                    s.gear.two_hand = def.hands >= 2;
                    s.gear.ranged = def.weapon == "bow" || def.weapon == "crossbow";
                } else if slot == SLOT_OFF && def.kind == "weapon" {
                    s.off_dmg = [(def.damage[0] * k).round(), (def.damage[1] * k).round()];
                    s.off_type = if def.dmg_type.is_empty() {
                        "blunt".into()
                    } else {
                        def.dmg_type.clone()
                    };
                    s.off_on_hit = def.on_hit.clone();
                    s.off_pct = def.on_hit_pct;
                    s.gear.dual = true;
                } else if slot == SLOT_OFF && def.kind == "shield" {
                    s.gear.shield = true;
                }
            }
            if s.gear.weapon.is_empty() && s.gear.dual {
                // a single weapon in the left hand still counts as a weapon
                s.gear.weapon = "dagger".into();
                s.gear.dual = false;
                s.weapon_dmg = s.off_dmg;
                s.weapon_type = s.off_type.clone();
            }
            for (key, rank) in &p.skills {
                if let Some(sd) = d.skill(key) {
                    if s.gear.has(&sd.equip) {
                        add_mods(&mut mods, &sd.stats, *rank as f64);
                    }
                }
            }
            s.str_ = get(&mods, "str");
            s.dex = get(&mods, "dex");
            s.int = get(&mods, "int");
            s.vit = get(&mods, "vit");
            s.max_hp = 40.0 + s.vit * 10.0 + p.level as f64 * 6.0 + get(&mods, "max_hp");
            s.max_mp = 15.0 + s.int * 6.0 + get(&mods, "max_mp");
            s.hp_regen = 0.25 + s.vit * 0.06 + get(&mods, "hp_regen");
            s.mp_regen = 0.5 + s.int * 0.08 + get(&mods, "mp_regen");
            s.armor = get(&mods, "armor");
            s.dodge = (s.dex * 0.3 + get(&mods, "dodge")).min(50.0);
            s.crit = (5.0 + s.dex * 0.5 + get(&mods, "crit")).min(75.0);
            s.crit_mult = 1.5 + get(&mods, "crit_mult");
            s.melee_pct = get(&mods, "melee_pct");
            s.spell_pct = get(&mods, "spell_pct");
            s.ranged_pct = get(&mods, "ranged_pct");
            s.attack_speed = get(&mods, "attack_speed") + s.dex * 0.8;
            if s.gear.dual {
                s.attack_speed += 10.0;
            }
            s.move_speed = get(&mods, "move_speed");
            s.sight = get(&mods, "sight") as i32;
            s.gold_find = get(&mods, "gold_find");
        } else if let Some(m) = &self.monster {
            if let Some(def) = d.monster(&m.def) {
                for (k, v) in &def.resist {
                    *mods.entry(format!("res_{k}")).or_insert(0.0) += v;
                }
                if !def.dmg_type.is_empty() {
                    s.weapon_type = def.dmg_type.clone();
                }
                s.weapon_on_hit = def.on_hit.clone();
                s.on_hit_pct = def.on_hit_pct;
            }
            s.res_cap = 100.0;
            s.max_hp = self.max_hp;
            s.armor = m.armor + get(&mods, "armor");
            s.weapon_dmg = m.damage;
            s.melee_pct = get(&mods, "melee_pct");
            s.attack_speed = get(&mods, "attack_speed");
            s.move_speed = get(&mods, "move_speed");
            s.crit = (3.0 + get(&mods, "crit")).max(0.0);
            s.crit_mult = 1.5;
            s.dodge = get(&mods, "dodge");
        } else {
            s.max_hp = self.max_hp;
            s.move_speed = get(&mods, "move_speed");
            s.attack_speed = get(&mods, "attack_speed");
        }
        s.life_leech = get(&mods, "life_leech");
        s.thorns = get(&mods, "thorns");
        s.block = get(&mods, "block").min(75.0);
        s.fury = get(&mods, "fury");
        s.duel_pct = get(&mods, "duel_pct");
        s.ambush_pct = get(&mods, "ambush_pct");
        s.heal_pct = get(&mods, "heal_pct");
        s.mimic_pct = get(&mods, "mimic_pct");
        if self.player.is_some() && s.gear.has("melee") {
            s.reach = get(&mods, "reach");
        }
        let (mut base_move, mut base_attack) = (160.0, 650.0);
        if let Some(m) = &self.monster {
            base_move = m.move_ms;
            base_attack = m.attack_ms;
        }
        if self.kind == Kind::Npc && self.monster.is_none() {
            base_move = 300.0;
        }
        s.move_ms = base_move / (1.0 + s.move_speed / 100.0).max(0.25);
        s.attack_ms = base_attack / (1.0 + s.attack_speed / 100.0).max(0.25);
        s.mods = mods;
        if self.player.is_some() {
            let frac_hp = if self.max_hp > 0.0 {
                self.hp / self.max_hp
            } else {
                1.0
            };
            let frac_mp = if self.max_mp > 0.0 {
                self.mp / self.max_mp
            } else {
                1.0
            };
            self.max_hp = s.max_hp;
            self.max_mp = s.max_mp;
            self.hp = (frac_hp * self.max_hp).clamp(0.0, self.max_hp);
            self.mp = (frac_mp * self.max_mp).clamp(0.0, self.max_mp);
        }
        self.stats = s;
    }

    /// Walking speed in tiles per second on ground of the given move cost.
    pub fn speed(&self, move_cost: f64) -> f32 {
        let ms = self.stats.move_ms.max(30.0) * move_cost.max(0.2);
        let pace = if self.pace > 0.0 { self.pace } else { 1.0 };
        (1000.0 / ms) as f32 * pace
    }
}

/// Human-readable names for stat keys (UI and item descriptions).
pub fn stat_name(key: &str) -> String {
    let n = match key {
        "str" => "Сила",
        "dex" => "Ловкость",
        "int" => "Интеллект",
        "vit" => "Выносливость",
        "max_hp" => "Здоровье",
        "max_mp" => "Мана",
        "hp_regen" => "Реген. здоровья",
        "mp_regen" => "Реген. маны",
        "armor" => "Броня",
        "dodge" => "Уклонение %",
        "crit" => "Крит. шанс %",
        "crit_mult" => "Крит. множитель",
        "melee_pct" => "Урон ближн. %",
        "spell_pct" => "Сила магии %",
        "ranged_pct" => "Урон дальн. %",
        "attack_speed" => "Скорость атаки %",
        "move_speed" => "Скорость бега %",
        "sight" => "Обзор",
        "gold_find" => "Находка золота %",
        "life_leech" => "Вампиризм %",
        "thorns" => "Шипы",
        "block" => "Блок %",
        "fury" => "Ярость раненого %",
        "duel_pct" => "Урон один на один %",
        "ambush_pct" => "Урон из засады %",
        "heal_pct" => "Сила лечения %",
        "mimic_pct" => "Сила копий %",
        "reach" => "Дальность удара",
        _ => "",
    };
    if !n.is_empty() {
        return n.to_string();
    }
    let d = db();
    if let Some(t) = key.strip_prefix("res_") {
        if let Some(n) = crate::content::damage_group(t) {
            return format!("{n} %");
        }
        if let Some(dt) = d.damage_type(t) {
            return format!("{} %", dt.res_name);
        }
    } else if let Some(t) = key.strip_prefix("add_") {
        if let Some(dt) = d.damage_type(t) {
            return dt.add_name.clone();
        }
    } else if let Some(t) = key.strip_suffix("_pct") {
        if let Some(dt) = d.damage_type(t) {
            return dt.pct_name.clone();
        }
    }
    key.to_string()
}

/// A damage type name in lower case ("огонь").
pub fn damage_type_name(key: &str) -> String {
    match db().damage_type(key) {
        Some(d) => d.name.to_lowercase(),
        None => key.to_string(),
    }
}
