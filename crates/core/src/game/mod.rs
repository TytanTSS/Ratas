//! The authoritative real-time simulation. It runs on a single thread (owned
//! by the server) at a fixed tick rate; creatures move continuously in any
//! direction and collide with walls and each other. Asynchronous work such as
//! LLM calls reports back through tasks posted to the game.

mod abilities;
mod admin;
mod ai;
mod allies;
mod city;
mod classes;
mod combat;
mod damage;
mod deeds;
mod dialogue;
mod director;
mod effects;
mod entity;
mod equip;
mod fusion;
mod lore;
mod movement;
mod player;
mod rarity;
mod relations;
mod save;
mod spawn;
mod testhooks;
mod uniques;
mod view;

pub use admin::*;
pub use classes::*;
pub use damage::*;
pub use deeds::*;
pub use entity::*;
pub use equip::*;
pub use fusion::{can_fuse, can_fuse_one, fused_in, fusions_of, max_fusions, FUSION_LEVEL};
pub use lore::{Lore, LORE_PREFIX};
pub use player::*;
pub use rarity::*;
pub use relations::Party;
pub use save::*;
pub use uniques::{quest_text, reward_name};
pub use view::*;

use crate::content::{db, AbilityDef};
use crate::gen::{self, Entrance, Landmark, Rect, Region};
use crate::llm::Brain;
use crate::proto::{Dialogue, Fx, LogLine, TileChange, FX_SUMMON};
use crate::rng::Rng;
use crate::world::{Level, Pos, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::{channel, Receiver, Sender};

/// Simulation steps per second.
pub const TPS: u32 = 30;
pub const TICK_MS: f64 = 1000.0 / TPS as f64;
/// Ticks in half a second (regeneration, hazards, damage over time).
pub const HALF_SEC: u64 = (TPS / 2) as u64;
pub const DAY_MS: f64 = 12.0 * 60.0 * 1000.0;
/// The size of the surface of a new world: ten times the classic 300×200
/// along each side (content is spread over it at the classic density).
pub const OVERWORLD_W: i32 = 3000;
pub const OVERWORLD_H: i32 = 2000;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VillageInfo {
    pub name: String,
    pub center: Pos,
    pub area: Rect,
    /// a big walled stone city
    #[serde(default)]
    pub city: bool,
}

/// Per-player messages produced during a tick.
#[derive(Default)]
pub struct Outbox {
    pub logs: Vec<LogLine>,
    pub dialogue: Option<Dialogue>,
}

pub type Task = Box<dyn FnOnce(&mut Game) + Send>;

pub(crate) struct PendingNpc {
    pub e: Entity,
    pub at: f64,
}

pub struct Game {
    pub seed: i64,
    pub world_name: String,
    /// game time in ms
    pub now: f64,
    pub tick_n: u64,
    pub levels: HashMap<String, Level>,
    pub villages: Vec<VillageInfo>,
    pub entrances: Vec<Entrance>,
    pub regions: Vec<Region>,
    /// overworld cell -> 1-based region index
    pub region_map: Vec<u16>,
    pub landmarks: Vec<Landmark>,
    pub start: Pos,
    /// the belts of danger (rebuilt from the seed on load)
    pub zones: gen::ZoneMap,
    pub ents: BTreeMap<Id, Entity>,
    pub next_id: Id,
    /// account -> player entity in the world
    pub online: BTreeMap<String, Id>,
    /// characters of players not connected
    pub offline: BTreeMap<String, Entity>,
    pub brain: Option<Brain>,
    pub paused: bool,
    /// players outside one party can hurt each other
    pub pvp: bool,
    /// unique quest -> its champion
    pub champions: HashMap<String, Id>,
    /// recent deeds the world talks about
    pub chronicle: Vec<String>,
    /// the world's own story, written by a model when the world was made
    pub lore: Option<Lore>,
    /// why the story could not be written (told to the first hero)
    pub(crate) lore_note: Option<String>,

    pub(crate) rng: Rng,
    tasks_tx: Sender<Task>,
    tasks_rx: Receiver<Task>,
    pub(crate) outbox: HashMap<Id, Outbox>,
    pub(crate) fx: HashMap<String, Vec<Fx>>,
    pub(crate) tiles: HashMap<String, Vec<TileChange>>,
    next_spawn: f64,
    pub(crate) quest_seq: i32,
    pub(crate) parties: HashMap<i32, Party>,
    pub(crate) party_seq: i32,
    pub(crate) squad_seq: i32,
    /// entities to simulate and query per level (see index_levels)
    pub(crate) by_level: HashMap<String, Vec<Id>>,
    /// every entity per level by area, for cell lookups
    pub(crate) grids: HashMap<String, movement::Grid>,
    pub(crate) village_grid: relations::VillageGrid,
    pub(crate) revivals: Vec<PendingNpc>,
    /// the AI game master
    pub(crate) gm: director::Director,
    /// lines for the dedicated server's console
    pub(crate) server_log: Vec<String>,
    /// level -> its tiles packed for clients and the version they were packed at
    pub(crate) packed_tiles: HashMap<String, (u64, Vec<u8>)>,
}

impl Game {
    pub(crate) fn empty(brain: Option<Brain>) -> Game {
        let (tasks_tx, tasks_rx) = channel();
        Game {
            seed: 0,
            world_name: String::new(),
            now: 0.0,
            tick_n: 0,
            levels: HashMap::new(),
            villages: Vec::new(),
            entrances: Vec::new(),
            regions: Vec::new(),
            region_map: Vec::new(),
            landmarks: Vec::new(),
            start: Pos::default(),
            zones: Default::default(),
            ents: BTreeMap::new(),
            next_id: 1,
            online: BTreeMap::new(),
            offline: BTreeMap::new(),
            brain,
            paused: false,
            pvp: true,
            champions: HashMap::new(),
            chronicle: Vec::new(),
            lore: None,
            lore_note: None,
            rng: Rng::from_time(),
            tasks_tx,
            tasks_rx,
            outbox: HashMap::new(),
            fx: HashMap::new(),
            tiles: HashMap::new(),
            next_spawn: 0.0,
            quest_seq: 0,
            parties: HashMap::new(),
            party_seq: 0,
            squad_seq: 0,
            by_level: HashMap::new(),
            grids: HashMap::new(),
            village_grid: Default::default(),
            revivals: Vec::new(),
            gm: Default::default(),
            server_log: Vec::new(),
            packed_tiles: HashMap::new(),
        }
    }

    /// Generates a fresh world from a seed.
    pub fn new(seed: i64, brain: Option<Brain>) -> Game {
        Game::with_size(seed, OVERWORLD_W, OVERWORLD_H, brain)
    }

    /// Generates a fresh world with a surface of the given size (tests use
    /// small ones).
    pub fn with_size(seed: i64, w: i32, h: i32, brain: Option<Brain>) -> Game {
        let mut g = Game::empty(brain);
        g.seed = seed;
        let ow = gen::generate_overworld(seed, w, h);
        g.world_name = ow.name.clone();
        g.start = ow.start;
        g.entrances = ow.entrances;
        g.regions = ow.regions;
        g.region_map = ow.region_map;
        g.landmarks = ow.landmarks;
        g.zones = ow.zones;
        g.levels.insert("overworld".into(), ow.level);
        let mut r = Rng::labeled(seed, "population");
        for v in &ow.villages {
            g.villages.push(VillageInfo {
                name: v.name.clone(),
                center: v.center,
                area: v.area,
                city: v.city,
            });
        }
        g.index_villages();
        g.index_levels();
        for v in &ow.villages {
            for n in &v.npcs {
                g.spawn_npc(n, &v.name, &mut r);
            }
        }
        g.index_levels();
        g.populate_overworld(&mut r);
        g.populate_landmarks(&mut r);
        g.populate_wanderers(&mut r);
        g.index_levels();
        g.place_uniques(&mut r);
        g.index_levels();
        g
    }

    /// A sender of tasks to run on the game thread (safe from any thread).
    pub fn task_sender(&self) -> Sender<Task> {
        self.tasks_tx.clone()
    }

    pub(crate) fn drain_tasks(&mut self) {
        while let Ok(f) = self.tasks_rx.try_recv() {
            f(self);
        }
    }

    /// Advances the simulation by one fixed step.
    pub fn tick(&mut self) {
        self.drain_tasks();
        if self.paused {
            return;
        }
        self.now += TICK_MS;
        self.tick_n += 1;
        self.index_levels();
        // only what is near the players lives: the rest of the world sleeps
        let ids = self.awake_ids();
        for &id in &ids {
            let Some(e) = self.ents.get(&id) else {
                continue;
            };
            match e.kind {
                Kind::Player => self.update_player(id),
                Kind::Monster => {
                    self.update_monster(id);
                }
                Kind::Npc => self.update_npc(id),
                Kind::Projectile => {
                    self.update_projectile(id);
                    continue;
                }
                Kind::Item => continue,
            }
            if self.ents.contains_key(&id) {
                self.update_buffs(id);
            }
        }
        self.move_all(&ids);
        if self.now >= self.next_spawn {
            self.next_spawn = self.now + 2000.0;
            self.run_spawner();
            self.revive_npcs();
            self.director_tick();
        }
    }

    /// Remembers a notable deed for NPC gossip.
    pub(crate) fn chronicle(&mut self, s: String) {
        self.chronicle.push(s);
        if self.chronicle.len() > 20 {
            let n = self.chronicle.len() - 20;
            self.chronicle.drain(..n);
        }
    }

    // ---- entities ----

    pub fn spawn(&mut self, mut e: Entity) -> Id {
        if e.id == 0 {
            e.id = self.next_id;
            self.next_id += 1;
        } else if e.id >= self.next_id {
            self.next_id = e.id + 1;
        }
        e.recalc();
        let id = e.id;
        self.by_level.entry(e.level.clone()).or_default().push(id);
        if let Some(g) = self.grids.get_mut(&e.level) {
            g.add(e.cell(), id);
        }
        self.ents.insert(id, e);
        id
    }

    pub fn remove(&mut self, id: Id) -> Option<Entity> {
        self.outbox.remove(&id);
        self.ents.remove(&id)
    }

    pub fn e(&self, id: Id) -> Option<&Entity> {
        self.ents.get(&id)
    }

    pub fn em(&mut self, id: Id) -> Option<&mut Entity> {
        self.ents.get_mut(&id)
    }

    /// The player entity of an account in the world.
    pub fn player(&self, name: &str) -> Option<&Entity> {
        self.online.get(name).and_then(|id| self.ents.get(id))
    }

    pub fn player_id(&self, name: &str) -> Option<Id> {
        self.online.get(name).copied()
    }

    pub(crate) fn players_on(&self, level: &str) -> Vec<Id> {
        self.online
            .values()
            .copied()
            .filter(|id| self.ents.get(id).is_some_and(|e| e.level == level))
            .collect()
    }

    pub(crate) fn alive(&self, id: Id) -> bool {
        self.ents.get(&id).is_some_and(|e| e.alive())
    }

    // ---- levels ----

    pub fn level(&self, id: &str) -> Option<&Level> {
        self.levels.get(id)
    }

    /// Returns a level id, generating dungeon floors on first visit.
    pub fn ensure_level(&mut self, id: &str) -> bool {
        if self.levels.contains_key(id) {
            return true;
        }
        let Some((idx, depth)) = parse_dungeon_id(id) else {
            return false;
        };
        if idx < 0 || idx as usize >= self.entrances.len() {
            return false;
        }
        let ent = self.entrances[idx as usize].clone();
        if depth < 1 || depth > ent.max_depth {
            return false;
        }
        let f = gen::generate_dungeon(
            self.seed,
            id,
            &ent.name,
            &ent.theme,
            idx,
            depth,
            ent.max_depth,
        );
        let lid = f.level.id.clone();
        let monsters = f.monsters.clone();
        let items = f.items.clone();
        let boss = f.boss;
        self.levels.insert(lid.clone(), f.level);
        self.populate_dungeon(&lid, &monsters, &items, boss, &ent);
        true
    }

    pub(crate) fn set_tile(&mut self, level: &str, x: i32, y: i32, t: u8) {
        if let Some(l) = self.levels.get_mut(level) {
            l.set(x, y, t);
            self.tiles
                .entry(level.to_string())
                .or_default()
                .push(TileChange { x, y, t });
        }
    }

    // ---- time ----

    pub fn time_of_day(&self) -> f64 {
        (self.now / DAY_MS + 0.30).rem_euclid(1.0)
    }

    /// 1 at noon and 0 at night with smooth dawn and dusk.
    pub fn daylight(&self) -> f64 {
        daylight(self.time_of_day())
    }

    pub fn is_night(&self) -> bool {
        self.daylight() < 0.35
    }

    pub fn time_name(&self) -> &'static str {
        let t = self.time_of_day();
        if !(0.2..0.85).contains(&t) {
            "ночь"
        } else if t < 0.3 {
            "рассвет"
        } else if t < 0.45 {
            "утро"
        } else if t < 0.6 {
            "день"
        } else if t < 0.75 {
            "вечер"
        } else {
            "сумерки"
        }
    }

    pub fn vision(&self, e: &Entity) -> i32 {
        let lit = self.levels.get(&e.level).is_some_and(|l| l.lit);
        let r = if lit {
            6 + (11.0 * self.daylight()).round() as i32
        } else {
            8
        };
        r + e.stats.sight
    }

    // ---- messaging ----

    pub(crate) fn box_(&mut self, id: Id) -> &mut Outbox {
        self.outbox.entry(id).or_default()
    }

    pub fn log(&mut self, id: Id, color: &str, text: String) {
        if self.ents.get(&id).is_some_and(|e| e.kind == Kind::Player) {
            self.box_(id).logs.push(LogLine {
                text,
                color: color.into(),
            });
        }
    }

    pub fn log_all(&mut self, color: &str, text: String) {
        let ids: Vec<Id> = self.online.values().copied().collect();
        for p in ids {
            self.log(p, color, text.clone());
        }
    }

    /// A floating text or a burst at a point.
    pub(crate) fn fx(
        &mut self,
        level: &str,
        p: Vec2,
        text: &str,
        glyph: char,
        color: &str,
        ms: i32,
    ) {
        self.push_fx(
            level,
            Fx {
                text: text.into(),
                ..fx_between(p, p, glyph, color, ms)
            },
        );
    }

    /// A moment of an ability (FX_*), drawn by clients in the ability's look;
    /// `other` is the second point the moment names (see FX_*).
    pub(crate) fn fx_spell(
        &mut self,
        level: &str,
        a: &AbilityDef,
        part: u8,
        at: Vec2,
        other: Vec2,
        radius: f32,
    ) {
        let ms = if part == FX_SUMMON { 500 } else { 300 };
        self.push_fx(
            level,
            Fx {
                radius,
                ability: a.key.clone(),
                part,
                ..fx_between(at, other, combat::glyph_of(a, '*'), &a.color, ms)
            },
        );
    }

    fn push_fx(&mut self, level: &str, fx: Fx) {
        match self.fx.get_mut(level) {
            Some(v) => v.push(fx),
            None => {
                self.fx.insert(level.to_string(), vec![fx]);
            }
        }
    }

    pub(crate) fn say(&mut self, id: Id, text: &str, ms: f64) {
        let now = self.now;
        if let Some(e) = self.ents.get_mut(&id) {
            e.speech = text.to_string();
            e.speech_until = now + ms;
        }
    }

    /// Returns and clears the queued messages for a player.
    pub fn take_outbox(&mut self, id: Id) -> Option<Outbox> {
        self.outbox.remove(&id)
    }

    /// Effects and tile changes produced this tick on a level.
    pub fn frame_tiles(&self, level: &str) -> Vec<TileChange> {
        self.tiles.get(level).cloned().unwrap_or_default()
    }

    /// Clears per-tick broadcast buffers.
    pub fn end_frame(&mut self) {
        self.fx.clear();
        self.tiles.clear();
    }

    pub(crate) fn roll(&mut self, lo: f64, hi: f64) -> f64 {
        self.rng.roll(lo, hi)
    }

    pub(crate) fn chance(&mut self, pct: f64) -> bool {
        self.rng.chance(pct)
    }

    pub fn rng(&mut self) -> &mut Rng {
        &mut self.rng
    }
}

pub fn daylight(t: f64) -> f64 {
    if (0.3..=0.7).contains(&t) {
        1.0
    } else if t >= 0.85 || t <= 0.15 {
        0.0
    } else if t < 0.3 {
        (t - 0.15) / 0.15
    } else {
        (0.85 - t) / 0.15
    }
}

fn fx_between(a: Vec2, b: Vec2, glyph: char, color: &str, ms: i32) -> Fx {
    Fx {
        x: a.x,
        y: a.y,
        x2: b.x,
        y2: b.y,
        glyph,
        color: color.into(),
        ms,
        ..Default::default()
    }
}

pub fn dungeon_level_id(idx: i32, depth: i32) -> String {
    format!("d{idx}-{depth}")
}

pub(crate) fn parse_dungeon_id(id: &str) -> Option<(i32, i32)> {
    let rest = id.strip_prefix('d')?;
    let (a, b) = rest.split_once('-')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

/// Searches outward from p for a walkable cell without an interaction that
/// no creature stands on.
pub(crate) fn find_free(l: &Level, p: Pos, taken: &dyn Fn(Pos) -> bool) -> Pos {
    (0..40)
        .flat_map(|rad| p.ring(rad))
        .find(|q| l.walkable(q.x, q.y) && l.def(q.x, q.y).interact.is_empty() && !taken(*q))
        .unwrap_or(p)
}

pub(crate) fn lower(s: &str) -> String {
    crate::i18n::lower_first(s)
}

pub(crate) fn upper_first(s: &str) -> String {
    crate::i18n::upper_first(s)
}

#[cfg(test)]
mod balance_tests;
#[cfg(test)]
mod tests;
