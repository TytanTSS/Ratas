//! Heroes: joining, input, interaction, death, progression and inventory.

use super::*;
use crate::proto::{Command, Input};
use crate::world::{Bitset, Vec2};
use std::collections::HashMap;

pub const INVENTORY_SIZE: usize = 24;
pub const HOTBAR_SIZE: usize = 6;
/// How long a fallen hero can be resurrected.
pub const REVIVE_WINDOW_MS: f64 = 60000.0;

/// The real-time control state of a player.
#[derive(Clone, Debug, Default)]
pub struct Intent {
    pub mv: Vec2,
    pub move_at: f64,
    pub attack: bool,
    pub attack_at: f64,
    pub interact: bool,
    pub ability: i32,
    pub ability_at: f64,
    /// the point the hero aims at with the mouse
    pub aim: Option<Vec2>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Quest {
    pub id: i32,
    pub giver: String,
    pub giver_id: Id,
    pub village: String,
    pub monster: String,
    pub need: i32,
    pub have: i32,
    pub gold: i32,
    pub xp: i32,
    pub done: bool,
    /// "" (hunt), slay, boss, relics
    pub kind: String,
    /// quest of a unique character
    pub unique: String,
    /// relics to collect
    pub item: String,
    pub sources: Vec<String>,
    /// where to go (champion lair, dungeon entrance)
    pub at: Option<Pos>,
    /// name of that place
    pub where_: String,
    pub reward: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerState {
    pub account: String,
    /// the player's language (AI characters answer in it)
    pub lang: String,
    pub class: String,
    pub level: i32,
    pub xp: i32,
    pub attrs: crate::content::Stats,
    pub attr_points: i32,
    pub skill_points: i32,
    pub skills: BTreeMap<String, i32>,
    pub abilities: Vec<String>,
    pub hotbar: [String; HOTBAR_SIZE],
    pub inventory: Vec<ItemStack>,
    pub equip: BTreeMap<String, ItemStack>,
    pub gold: i32,
    pub quests: Vec<Quest>,
    pub explored: HashMap<String, Bitset>,
    pub kills: i32,
    /// discovered landmarks
    pub found: Vec<usize>,
    /// classes being developed, the first is the starting one
    pub classes: Vec<String>,
    /// class -> chosen subclass
    pub subclasses: BTreeMap<String, String>,
    /// secret classes and subclasses opened by quests
    pub unlocks: Vec<String>,
    /// bosses slain (for conversations)
    pub bosses: Vec<String>,
    /// what the hero has done (see deeds.rs)
    pub deeds: BTreeMap<String, i32>,

    #[serde(skip)]
    pub intent: Intent,
    #[serde(skip)]
    pub dirty: bool,
    #[serde(skip)]
    pub talking: Id,
    #[serde(skip)]
    pub respawn_at: f64,
    /// enemy shown in the HUD
    #[serde(skip)]
    pub target: Id,
    #[serde(skip)]
    pub target_at: f64,
    #[serde(skip)]
    pub died_at: f64,
    /// the respawn button works from this time
    #[serde(skip)]
    pub can_rise: f64,
    #[serde(skip)]
    pub party_id: i32,
    /// party invitations: inviter -> time
    #[serde(skip)]
    pub invites: HashMap<String, f64>,
    /// ability copied by a mimic
    #[serde(skip)]
    pub copied: String,
    #[serde(skip)]
    pub copied_lvl: i32,
    #[serde(skip)]
    pub copied_end: f64,
    /// admin: takes no damage
    #[serde(skip)]
    pub god: bool,
    /// admin: abilities cost nothing and have no cooldown
    #[serde(skip)]
    pub no_cd: bool,
    /// send the level again (the explored map changed)
    #[serde(skip)]
    pub resync: bool,
    /// the point the current attack or ability is aimed at
    #[serde(skip)]
    pub aim: Option<Vec2>,
    /// a deed counter changed: look for hidden skills
    #[serde(skip)]
    pub deed_check: bool,
    #[serde(skip)]
    pub region: usize,
    #[serde(skip)]
    pub last_hint: String,
    #[serde(skip)]
    pub step_acc: f32,
    #[serde(skip)]
    pub bump_at: f64,
}

pub fn xp_for_level(l: i32) -> i32 {
    (40.0 * (l as f64).powf(1.6)) as i32
}

pub const ERR_NAME_TAKEN: &str = "игрок с таким именем уже в игре";

impl Game {
    pub(crate) fn new_player(&self, name: &str, class_key: &str) -> Entity {
        let d = db();
        let c = match d.class(class_key) {
            Some(c) if !c.secret => c, // secret classes are earned, not chosen
            _ => &d.b.classes[0],
        };
        let mut e = Entity {
            kind: Kind::Player,
            name: name.into(),
            glyph: "@".into(),
            color: c.color.clone(),
            faction: Faction::Player,
            level: "overworld".into(),
            pos: self.start.center(),
            facing: std::f32::consts::FRAC_PI_2,
            ..Default::default()
        };
        let p = PlayerState {
            account: name.into(),
            class: c.key.clone(),
            level: 1,
            gold: 25,
            classes: vec![c.key.clone()],
            attrs: c.attrs.clone(),
            ..Default::default()
        };
        e.player = Some(Box::new(p));
        for a in &c.abilities {
            unlock_ability_on(&mut e, a);
        }
        for key in &c.items {
            let st = ItemStack::new(key);
            let def = st.def();
            let p = e.pm();
            let mut slot = slot_for(def).to_string();
            let main_taken = p.equip.get(SLOT_MAIN).is_some_and(|s| !s.key.is_empty());
            if slot == SLOT_MAIN
                && main_taken
                && !two_handed(&st)
                && !p.equip.get(SLOT_MAIN).is_some_and(two_handed)
            {
                slot = SLOT_OFF.into(); // a second one-handed weapon goes to the left hand
            }
            if slot == SLOT_OFF && p.equip.get(SLOT_MAIN).is_some_and(two_handed) {
                slot.clear();
            }
            if !slot.is_empty() && p.equip.get(&slot).is_none_or(|s| s.key.is_empty()) {
                p.equip.insert(slot, st);
            } else if p.inventory.len() < INVENTORY_SIZE {
                p.inventory.push(st);
            }
        }
        e.recalc();
        e.hp = e.max_hp;
        e.mp = e.max_mp;
        e
    }

    /// Brings a player into the world. need_class is true for a new character
    /// without a chosen class: the client must ask and call join again.
    pub fn join(&mut self, name: &str, class: &str) -> Result<(Option<Id>, bool), String> {
        if self.online.contains_key(name) {
            return Err(ERR_NAME_TAKEN.into());
        }
        let id = if let Some(mut e) = self.offline.remove(name) {
            if !self.ensure_level(&e.level.clone()) {
                e.level = "overworld".into();
                e.pos = self.start.center();
            }
            let fp = self.free_spot(&e.level, e.cell());
            if fp != e.cell() {
                e.pos = fp.center();
            }
            let p = e.pm();
            p.intent = Intent::default();
            p.talking = 0;
            if e.dead || e.hp <= 0.0 {
                e.dead = false;
                e.hp = e.max_hp * 0.5;
            }
            let id = self.spawn(e);
            self.online.insert(name.into(), id);
            let wn = self.world_name.clone();
            self.log(id, "#a0e0ff", format!("С возвращением в {wn}, {name}!"));
            id
        } else {
            if class.is_empty() {
                return Ok((None, true));
            }
            let mut e = self.new_player(name, class);
            e.pos = self.free_spot("overworld", self.start).center();
            let id = self.spawn(e);
            self.online.insert(name.into(), id);
            let wn = self.world_name.clone();
            self.log(
                id,
                "#ffd24a",
                format!("Добро пожаловать в {wn}, {name}! Время приключений."),
            );
            self.log(id, "#a0a0a0", "WASD/стрелки — ходить (можно по диагонали), мышь — целиться, ЛКМ/пробел — атака, 1-6 — умения, E — взаимодействие, F1 — помощь.".into());
            id
        };
        self.log_all("#80c0ff", format!("{name} входит в мир."));
        self.ents.get_mut(&id).unwrap().pm().dirty = true;
        self.update_explored(id);
        Ok((Some(id), false))
    }

    /// Removes a player from the world but keeps the character for later.
    pub fn leave(&mut self, name: &str) {
        let Some(id) = self.online.get(name).copied() else {
            return;
        };
        self.close_dialogue(id, false);
        if self.party_of(id).is_some() {
            self.party_leave(id);
        }
        self.online.remove(name);
        if let Some(mut e) = self.remove(id) {
            e.pm().intent = Intent::default();
            e.want = Vec2::ZERO;
            self.offline.insert(name.into(), e);
        }
        self.log_all("#80c0ff", format!("{name} покидает мир."));
    }

    /// Records a real-time input; it is executed on the next ticks.
    pub fn set_input(&mut self, id: Id, inp: &Input) {
        let now = self.now;
        let Some(e) = self.ents.get_mut(&id) else {
            return;
        };
        let it = &mut e.pm().intent;
        let mv = Vec2::new(inp.mv[0].clamp(-1, 1) as f32, inp.mv[1].clamp(-1, 1) as f32).norm();
        it.mv = mv;
        it.move_at = now;
        it.aim = inp.aim.map(|a| Vec2::new(a[0], a[1]));
        if inp.attack {
            it.attack = true;
            it.attack_at = now;
        }
        if inp.interact {
            it.interact = true;
        }
        if inp.ability > 0 {
            it.ability = inp.ability as i32;
            it.ability_at = now;
        }
    }

    pub(crate) fn update_player(&mut self, id: Id) {
        let now = self.now;
        let tick = self.tick_n;
        let e = self.ents.get_mut(&id).unwrap();
        if e.dead {
            e.want = Vec2::ZERO;
            if now >= e.p().respawn_at {
                self.respawn(id);
            }
            return;
        }
        if !e.p().copied.is_empty() && now >= e.p().copied_end {
            e.pm().copied.clear();
            e.pm().dirty = true;
            self.log(id, "#ff8ad8", "Скопированное умение рассеялось.".into());
        }
        let e = self.ents.get_mut(&id).unwrap();
        let dt = TICK_MS / 1000.0;
        e.hp = (e.hp + e.stats.hp_regen * dt).min(e.max_hp);
        e.mp = (e.mp + e.stats.mp_regen * dt).min(e.max_mp);
        let level = e.level.clone();
        let cell = e.cell();
        let def = self.levels[&level].def_at(cell);
        if def.damage > 0.0 && tick.is_multiple_of(HALF_SEC) {
            let mut hz = Damage::single(hazard_type(def), def.damage / 2.0);
            hz.dot = true;
            self.damage(None, id, hz);
            if !self.alive(id) {
                return;
            }
        }
        if tick.is_multiple_of(HALF_SEC) {
            self.update_explored(id);
            self.check_surroundings(id);
            if self.ents[&id].p().deed_check {
                self.check_deeds(id);
            }
        }
        let e = self.ents.get_mut(&id).unwrap();
        if e.stats.stunned {
            e.pm().intent = Intent::default();
            e.want = Vec2::ZERO;
            return;
        }
        let it = &mut e.player.as_mut().unwrap().intent;
        // a held key is repeated by the client; a silent one has been released
        if now - it.move_at > 300.0 {
            it.mv = Vec2::ZERO;
        }
        if it.ability != 0 && now - it.ability_at > 400.0 {
            it.ability = 0;
        }
        if it.attack && now - it.attack_at > 400.0 {
            it.attack = false;
        }
        let interact = std::mem::take(&mut it.interact);
        let mv = it.mv;
        let aim = it.aim;
        let ability = it.ability;
        let attack = it.attack;
        e.want = mv;
        if let Some(a) = aim {
            e.face_to(a);
        } else if mv.len_sq() > 0.0 {
            e.facing = mv.angle();
        }
        e.pm().aim = aim;
        if interact {
            self.interact(id);
        }
        if ability != 0 && self.use_hotbar(id, ability) {
            if let Some(e) = self.ents.get_mut(&id) {
                e.pm().intent.ability = 0;
            }
        }
        let Some(e) = self.ents.get(&id) else { return };
        if attack && now >= e.next_attack {
            self.ents.get_mut(&id).unwrap().pm().intent.attack = false;
            self.attack_facing(id);
        }
        // walking into an enemy attacks it, into a door opens it
        if mv.len_sq() > 0.0 {
            self.bump(id, mv);
        }
        let Some(e) = self.ents.get(&id) else { return };
        let talking = e.p().talking;
        if talking != 0 {
            let close = match self.ents.get(&talking) {
                Some(npc) => {
                    npc.level != e.level
                        || npc.dist(e) > 3.5
                        || self.hostile(talking, id)
                        || npc.npc.is_none()
                }
                None => true,
            };
            if close {
                self.close_dialogue(id, true);
            }
        }
    }

    /// What lies right ahead of a walking hero: an enemy to strike or a door,
    /// chest or shrine to use.
    pub(crate) fn bump(&mut self, id: Id, mv: Vec2) {
        let now = self.now;
        let e = &self.ents[&id];
        let ahead = e.pos + mv * (e.radius() + 0.35);
        let level = e.level.clone();
        // an enemy in the way
        if now >= e.next_attack {
            let mut target = None;
            for oid in self.near(&level, e.pos, e.radius() + BOSS_RADIUS + 0.12) {
                let o = &self.ents[&oid];
                if !self.hostile(id, oid) || !o.blocks() {
                    continue;
                }
                let to = o.pos - e.pos;
                if e.gap(o) <= 0.12 && to.norm().dot(mv) > 0.5 {
                    target = Some(oid);
                    break;
                }
            }
            if let Some(t) = target {
                self.melee_attack(id, t);
                return;
            }
        }
        let e = &self.ents[&id];
        if now - e.p().bump_at < 350.0 {
            return;
        }
        let cell = ahead.cell();
        let l = &self.levels[&level];
        if l.walkable(cell.x, cell.y) {
            return;
        }
        let interact = l.def_at(cell).interact.as_str();
        if matches!(interact, "door" | "chest" | "shrine") {
            self.ents.get_mut(&id).unwrap().pm().bump_at = now;
            self.use_tile(id, cell);
        }
    }

    pub(crate) fn after_player_move(&mut self, id: Id) {
        self.update_explored(id);
        self.pickup(id);
        let e = &self.ents[&id];
        let l = &self.levels[&e.level];
        let hint = match l.def_at(e.cell()).interact.as_str() {
            "dungeon" => match self.entrance_at(e.cell()) {
                Some(i) => format!(
                    "Вход: {}. Нажмите E, чтобы спуститься.",
                    self.entrances[i].name
                ),
                None => String::new(),
            },
            "stairs_down" => "Лестница вниз. Нажмите E.".into(),
            "stairs_up" => "Лестница вверх. Нажмите E.".into(),
            _ => String::new(),
        };
        if !hint.is_empty() && hint != e.p().last_hint {
            self.log(id, "#ffd24a", hint.clone());
        }
        self.ents.get_mut(&id).unwrap().pm().last_hint = hint;
    }

    pub(crate) fn update_explored(&mut self, id: Id) {
        let e = &self.ents[&id];
        let vision = self.vision(e);
        let Some(l) = self.levels.get(&e.level) else {
            return;
        };
        let n = (l.w * l.h) as usize;
        let c = e.cell();
        let lid = l.id.clone();
        let w = l.w;
        let mut cells = Vec::new();
        crate::world::fov(l, c.x, c.y, vision, &mut |x, y| {
            cells.push((y * w + x) as usize)
        });
        let p = self.ents.get_mut(&id).unwrap().pm();
        let bs = p.explored.entry(lid).or_insert_with(|| Bitset::new(n));
        if !bs.len_for(n) {
            *bs = Bitset::new(n);
        }
        for i in cells {
            bs.set(i);
        }
    }

    pub(crate) fn entrance_at(&self, p: Pos) -> Option<usize> {
        self.entrances.iter().position(|en| en.pos == p)
    }

    pub(crate) fn interact(&mut self, id: Id) {
        let e = &self.ents[&id];
        let level = e.level.clone();
        let cell = e.cell();
        let l = &self.levels[&level];
        let under = l.def_at(cell).interact.clone();
        if matches!(under.as_str(), "dungeon" | "stairs_down" | "stairs_up") {
            self.use_stairs(id, &under);
            return;
        }
        // the nearest friendly NPC in reach, the one looked at first
        let fv = e.facing_vec();
        let mut best: Option<(Id, f32)> = None;
        for oid in self.near(&level, e.pos, 1.9) {
            let o = &self.ents[&oid];
            if o.npc.is_none() || self.hostile(id, oid) || !o.alive() {
                continue;
            }
            let d = o.dist(e);
            if d > 1.9 {
                continue;
            }
            let score = d - (o.pos - e.pos).norm().dot(fv) * 0.6;
            if best.is_none_or(|b| score < b.1) {
                best = Some((oid, score));
            }
        }
        if let Some((npc, _)) = best {
            let np = self.ents[&npc].pos;
            self.ents.get_mut(&id).unwrap().face_to(np);
            self.open_dialogue(id, npc);
            return;
        }
        // a door, chest or shrine next to the hero, the one looked at first
        let ahead = (e.pos + fv * 0.8).cell();
        let mut cells = vec![ahead];
        for d in crate::world::DIRS8 {
            cells.push(cell.add(d));
        }
        for c in cells {
            let i = l.def_at(c).interact.as_str();
            if matches!(i, "door" | "chest" | "shrine") && c.center().dist(e.pos) < 1.6 {
                self.ents.get_mut(&id).unwrap().face_to(c.center());
                self.use_tile(id, c);
                return;
            }
        }
        self.log(id, "#808080", "Рядом нет ничего интересного.".into());
    }

    pub(crate) fn use_tile(&mut self, id: Id, c: Pos) {
        let level = self.ents[&id].level.clone();
        let def = self.levels[&level].def_at(c);
        match def.interact.as_str() {
            "door" => {
                let t = db().tile_id(&def.becomes);
                self.set_tile(&level, c.x, c.y, t);
            }
            "chest" => {
                let t = db().tile_id(&def.becomes);
                self.set_tile(&level, c.x, c.y, t);
                self.log(id, "#ffd24a", "Вы открываете сундук.".into());
                self.fx(&level, c.center(), "", '*', "#ffd24a", 300);
                let depth = self.levels[&level].depth.max(1);
                let gold = self.rng.int_n(10 * depth) + 5 * depth;
                // loot falls in front of the chest, toward the hero
                let spot = self.item_spot(&level, c, self.ents[&id].pos);
                self.drop_gold(&level, spot, gold);
                let n = 1 + self.rng.int_n(2);
                for _ in 0..n {
                    if let Some(st) = self.random_item(depth, 30.0) {
                        self.drop_item(&level, spot, st);
                    }
                }
            }
            "shrine" => self.pray_at_shrine(id, &level, c, def),
            _ => {}
        }
    }

    pub(crate) fn use_stairs(&mut self, id: Id, kind: &str) {
        let e = &self.ents[&id];
        let l = &self.levels[&e.level];
        let (dungeon, depth) = (l.dungeon, l.depth);
        let cell = e.cell();
        match kind {
            "dungeon" => {
                let Some(idx) = self.entrance_at(cell) else {
                    return;
                };
                let lid = dungeon_level_id(idx as i32, 1);
                if self.ensure_level(&lid) {
                    let up = self.levels[&lid].up;
                    self.change_level(id, &lid, up);
                }
            }
            "stairs_down" => {
                let lid = dungeon_level_id(dungeon, depth + 1);
                if self.ensure_level(&lid) {
                    let up = self.levels[&lid].up;
                    self.change_level(id, &lid, up);
                }
            }
            "stairs_up" => {
                if depth <= 1 {
                    let p = self.entrances[dungeon as usize].pos;
                    self.change_level(id, "overworld", p);
                } else {
                    let lid = dungeon_level_id(dungeon, depth - 1);
                    if self.ensure_level(&lid) {
                        let down = self.levels[&lid].down;
                        self.change_level(id, &lid, down);
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn change_level(&mut self, id: Id, target: &str, near: Pos) {
        self.close_dialogue(id, true);
        // arrive on the stairs themselves, where they can be used to go back
        let tl = &self.levels[target];
        let spot = if tl.walkable(near.x, near.y) && !self.cell_taken(target, near) {
            near
        } else {
            self.free_spot(target, near)
        };
        let tl = &self.levels[target];
        let (name, depth, no_down) = (tl.name.clone(), tl.depth, tl.down.x < 0);
        let hint = tl.def_at(spot).key.clone();
        {
            let e = self.ents.get_mut(&id).unwrap();
            e.level = target.to_string();
            e.pos = spot.center();
            e.want = Vec2::ZERO;
            e.goal = None;
            if let Some(p) = e.player.as_mut() {
                p.last_hint = hint;
            }
        }
        self.index_levels();
        if self.ents[&id].player.is_none() {
            return;
        }
        self.update_explored(id);
        self.deed_max(id, "depth", depth);
        self.log(id, "#c0a0ff", format!("Вы входите: {name}."));
        if depth > 0 && no_down {
            self.log(
                id,
                "#ff6a6a",
                "Здесь обитает нечто могущественное...".into(),
            );
        }
    }

    /// The player's own request to respawn in the village.
    pub(crate) fn rise(&mut self, id: Id) {
        let e = &self.ents[&id];
        if e.dead && self.now >= e.p().can_rise {
            self.respawn(id);
        }
    }

    /// Brings a fallen hero back where they fell.
    pub(crate) fn revive(&mut self, id: Id, by: Id) -> bool {
        let e = &self.ents[&id];
        if !e.dead || self.now - e.p().died_at > REVIVE_WINDOW_MS {
            return false;
        }
        let spot = self.free_spot(&e.level, e.cell());
        let e = self.ents.get_mut(&id).unwrap();
        if spot != e.cell() {
            e.pos = spot.center();
        }
        e.dead = false;
        e.recalc();
        e.hp = e.max_hp * 0.4;
        e.mp = e.max_mp * 0.3;
        e.pm().dirty = true;
        let (level, pos) = (e.level.clone(), e.pos);
        let by_name = self.ents[&by].name.clone();
        let name = self.ents[&id].name.clone();
        self.fx(&level, pos, "Воскрешение!", '\0', "#ffffc0", 1500);
        self.log(id, "#ffffc0", format!("{by_name} возвращает вас к жизни!"));
        self.log(by, "#ffffc0", format!("Вы воскресили: {name}."));
        true
    }

    pub(crate) fn respawn(&mut self, id: Id) {
        let spot = self.free_spot("overworld", self.start);
        let e = self.ents.get_mut(&id).unwrap();
        e.dead = false;
        e.buffs.clear();
        e.recalc();
        e.hp = e.max_hp;
        e.mp = e.max_mp * 0.5;
        e.level = "overworld".into();
        e.pos = spot.center();
        e.pm().dirty = true;
        self.index_levels();
        self.update_explored(id);
        self.log(
            id,
            "#a0ffa0",
            "Вы приходите в себя у колодца деревни.".into(),
        );
    }

    pub(crate) fn kill_player(&mut self, id: Id, killer: Option<Id>) {
        self.deed(id, "deaths", 1);
        let now = self.now;
        let e = self.ents.get_mut(&id).unwrap();
        e.dead = true;
        e.hp = 0.0;
        e.want = Vec2::ZERO;
        e.buffs.clear();
        let p = e.pm();
        p.died_at = now;
        p.respawn_at = now + REVIVE_WINDOW_MS;
        p.can_rise = now + 3000.0;
        p.intent = Intent::default();
        let lost = p.gold / 10;
        p.gold -= lost;
        p.dirty = true;
        let (name, level, pos) = (e.name.clone(), e.level.clone(), e.pos);
        self.close_dialogue(id, true);
        let mut who = "окружающий мир".to_string();
        if let Some(k) = killer.and_then(|k| self.controller(k)) {
            if let Some(ke) = self.ents.get(&k) {
                who = ke.name.clone();
                if ke.player.is_some() && k != id && lost > 0 {
                    let km = self.ents.get_mut(&k).unwrap().pm();
                    km.gold += lost;
                    km.dirty = true;
                    self.log(k, "#ffd700", format!("Вы забрали у {name} {lost} золота."));
                }
            }
        }
        self.log(
            id,
            "#ff4a4a",
            format!("Вы погибли! Вас сразил(а) {who}. Потеряно {lost} золота."),
        );
        self.log(
            id,
            "#c0c0c0",
            "Enter — возродиться у колодца деревни. Союзник может воскресить вас в течение минуты."
                .into(),
        );
        let others: Vec<Id> = self.online.values().copied().filter(|&o| o != id).collect();
        for o in others {
            self.log(o, "#ff8a8a", format!("{name} пал(а) в бою ({who})."));
        }
        self.fx(&level, pos, "", '%', "#ff2a2a", 1500);
    }

    // ---- progression ----

    pub fn give_xp(&mut self, id: Id, amount: i32) {
        let Some(e) = self.ents.get_mut(&id) else {
            return;
        };
        if e.player.is_none() || amount <= 0 {
            return;
        }
        e.pm().xp += amount;
        e.pm().dirty = true;
        loop {
            let e = self.ents.get_mut(&id).unwrap();
            let p = e.pm();
            if p.xp < xp_for_level(p.level) {
                break;
            }
            p.xp -= xp_for_level(p.level);
            p.level += 1;
            p.attr_points += 3;
            p.skill_points += 1;
            let lvl = p.level;
            e.recalc();
            e.hp = e.max_hp;
            e.mp = e.max_mp;
            let (name, level, pos) = (e.name.clone(), e.level.clone(), e.pos);
            self.log(
                id,
                "#ffff4a",
                format!("*** Уровень {lvl}! +3 очка характеристик, +1 очко навыков (C и K). ***"),
            );
            self.fx(&level, pos, "УРОВЕНЬ!", '\0', "#ffff4a", 1500);
            let others: Vec<Id> = self.online.values().copied().filter(|&o| o != id).collect();
            for o in others {
                self.log(o, "#c0c080", format!("{name} достигает {lvl} уровня."));
            }
        }
    }

    pub(crate) fn unlock_ability(&mut self, id: Id, key: &str) {
        if let Some(e) = self.ents.get_mut(&id) {
            unlock_ability_on(e, key);
        }
    }

    pub(crate) fn learn_skill(&mut self, id: Id, key: &str) {
        let Some(sd) = db().skill(key) else { return };
        let why = can_learn(self.ents[&id].p(), Some(sd));
        if !why.is_empty() {
            self.log(id, "#ff8080", format!("Нельзя изучить: {why}."));
            return;
        }
        let e = self.ents.get_mut(&id).unwrap();
        let p = e.pm();
        *p.skills.entry(key.into()).or_insert(0) += 1;
        p.skill_points -= 1;
        let rank = p.skills[key];
        p.dirty = true;
        if !sd.grants.is_empty() && rank == 1 {
            unlock_ability_on(e, &sd.grants);
            if let Some(ad) = db().ability(&sd.grants) {
                self.log(id, "#80ffff", format!("Новое умение: {}!", ad.name));
            }
        }
        self.ents.get_mut(&id).unwrap().recalc();
        self.log(
            id,
            "#80ff80",
            format!("Навык «{}» — ранг {}/{}.", sd.name, rank, sd.max_rank),
        );
    }

    pub(crate) fn alloc_attr(&mut self, id: Id, key: &str) {
        if !matches!(key, "str" | "dex" | "int" | "vit") {
            return;
        }
        let e = self.ents.get_mut(&id).unwrap();
        let p = e.pm();
        if p.attr_points <= 0 {
            return;
        }
        p.attr_points -= 1;
        *p.attrs.entry(key.into()).or_insert(0.0) += 1.0;
        p.dirty = true;
        e.recalc();
    }

    // ---- inventory ----

    pub(crate) fn add_item(&mut self, id: Id, mut st: ItemStack) -> bool {
        let e = self.ents.get_mut(&id).unwrap();
        let p = e.pm();
        if st.qty <= 0 {
            st.qty = 1;
        }
        if let Some(d) = st.def() {
            if d.kind == "consumable" && st.bonus.is_empty() {
                if let Some(it) = p.inventory.iter_mut().find(|it| it.key == st.key) {
                    it.qty += st.qty;
                    p.dirty = true;
                    return true;
                }
            }
        }
        if p.inventory.len() >= INVENTORY_SIZE {
            return false;
        }
        p.inventory.push(st);
        p.dirty = true;
        true
    }

    pub(crate) fn remove_inv_item(&mut self, id: Id, idx: usize) {
        let p = self.ents.get_mut(&id).unwrap().pm();
        p.inventory.remove(idx);
        p.dirty = true;
    }

    /// Picks up what lies under the hero's feet.
    pub(crate) fn pickup(&mut self, id: Id) {
        let e = &self.ents[&id];
        let (level, pos) = (e.level.clone(), e.pos);
        let items: Vec<Id> = self
            .near(&level, pos, 0.7)
            .into_iter()
            .filter(|o| {
                self.ents
                    .get(o)
                    .is_some_and(|o| o.kind == Kind::Item && o.pos.dist(pos) < 0.7)
            })
            .collect();
        let mut relics = false;
        for oid in items {
            let st = self.ents[&oid].item.clone().unwrap();
            if st.key == "gold" {
                let p = self.ents.get_mut(&id).unwrap().pm();
                p.gold += st.qty;
                p.dirty = true;
                self.deed(id, "gold", st.qty);
                self.log(id, "#ffd700", format!("+{} золота.", st.qty));
                self.remove(oid);
                continue;
            }
            if self.add_item(id, st.clone()) {
                if st.def().is_some_and(|d| d.kind == "quest") {
                    relics = true;
                }
                let r = st.item_rarity();
                if st.qty > 1 {
                    self.log(
                        id,
                        "#c0c0ff",
                        format!("Подобрано: {} ×{}.", st.name(), st.qty),
                    );
                } else if r > COMMON && !slot_for(st.def()).is_empty() {
                    self.log(
                        id,
                        rarity_color(r),
                        format!("Подобрано: {} ({}).", st.name(), lower(rarity_name(r))),
                    );
                    if r >= EPIC {
                        self.fx(
                            &level,
                            pos,
                            &format!("{}!", rarity_name(r)),
                            '\0',
                            rarity_color(r),
                            1500,
                        );
                    }
                } else {
                    self.log(id, "#c0c0ff", format!("Подобрано: {}.", st.name()));
                }
                self.remove(oid);
            } else {
                self.log(id, "#ff8080", "Инвентарь полон!".into());
            }
        }
        if relics {
            self.update_relics(id);
        }
    }

    pub(crate) fn use_item(&mut self, id: Id, idx: i32) {
        let e = &self.ents[&id];
        let p = e.p();
        if idx < 0 || idx as usize >= p.inventory.len() || e.dead {
            return;
        }
        let idx = idx as usize;
        let st = p.inventory[idx].clone();
        let Some(d) = st.def() else { return };
        if equippable(Some(d)) {
            self.equip(id, idx, false);
            return;
        }
        if d.kind != "consumable" {
            return;
        }
        if d.effect == "return" && !self.can_return(id) {
            return;
        }
        if d.heal > 0.0 || d.mana > 0.0 {
            self.deed(id, "potions", 1);
        }
        let e = self.ents.get_mut(&id).unwrap();
        let (level, pos) = (e.level.clone(), e.pos);
        if d.heal > 0.0 {
            e.hp = (e.hp + d.heal).min(e.max_hp);
        }
        if d.mana > 0.0 {
            e.mp = (e.mp + d.mana).min(e.max_mp);
        }
        // remove the item first: some effects reorganize the inventory
        let p = e.pm();
        p.inventory[idx].qty -= 1;
        p.dirty = true;
        if p.inventory[idx].qty <= 0 {
            self.remove_inv_item(id, idx);
        }
        if d.heal > 0.0 {
            self.fx(
                &level,
                pos,
                &format!("+{}", d.heal as i32),
                '\0',
                "#60ff60",
                900,
            );
        }
        if d.mana > 0.0 {
            self.fx(
                &level,
                pos,
                &format!("+{}", d.mana as i32),
                '\0',
                "#6090ff",
                900,
            );
        }
        self.log(id, "#a0ffa0", format!("Вы используете: {}.", d.name));
        self.item_effect(id, d);
    }

    /// Drinks the first healing or mana potion.
    pub(crate) fn quick_potion(&mut self, id: Id, mana: bool) {
        let best = self.ents[&id].p().inventory.iter().position(|st| {
            st.def().is_some_and(|d| {
                d.kind == "consumable" && ((mana && d.mana > 0.0) || (!mana && d.heal > 0.0))
            })
        });
        match best {
            Some(i) => self.use_item(id, i as i32),
            None => self.log(id, "#ff8080", "Нет подходящих зелий.".into()),
        }
    }

    pub(crate) fn drop_inv(&mut self, id: Id, idx: i32) {
        let e = &self.ents[&id];
        if idx < 0 || idx as usize >= e.p().inventory.len() {
            return;
        }
        let st = e.p().inventory[idx as usize].clone();
        let (level, cell) = (e.level.clone(), e.cell());
        self.remove_inv_item(id, idx as usize);
        self.drop_item(&level, cell, st.clone());
        self.log(id, "#a0a0a0", format!("Выброшено: {}.", st.name()));
    }

    /// Orders items by kind then name.
    pub(crate) fn sort_inventory(&mut self, id: Id) {
        let order = |k: &str| match k {
            "weapon" => 0,
            "shield" | "offhand" => 1,
            "head" => 2,
            "chest" => 3,
            "belt" => 4,
            "legs" => 5,
            "back" => 6,
            "ring" => 7,
            "consumable" => 8,
            "quest" => 9,
            _ => 10,
        };
        let p = self.ents.get_mut(&id).unwrap().pm();
        p.inventory.sort_by(|a, b| {
            let (da, dbb) = (a.def(), b.def());
            match (da, dbb) {
                (Some(x), Some(y)) => order(&x.kind)
                    .cmp(&order(&y.kind))
                    .then_with(|| a.name().cmp(&b.name())),
                _ => std::cmp::Ordering::Equal,
            }
        });
        p.dirty = true;
    }

    // ---- commands ----

    /// Handles discrete client actions.
    pub fn command(&mut self, id: Id, c: &Command) {
        if self.ents.get(&id).and_then(|e| e.player.as_ref()).is_none() {
            return;
        }
        match c.kind.as_str() {
            "alloc_attr" => self.alloc_attr(id, &c.key),
            "learn" => self.learn_skill(id, &c.key),
            "hotbar" => {
                if c.index < 0 || c.index as usize >= HOTBAR_SIZE {
                    return;
                }
                let p = self.ents.get_mut(&id).unwrap().pm();
                if !c.key.is_empty() {
                    if !p.abilities.contains(&c.key) {
                        return;
                    }
                    for h in p.hotbar.iter_mut() {
                        if *h == c.key {
                            h.clear();
                        }
                    }
                }
                p.hotbar[c.index as usize] = c.key.clone();
                p.dirty = true;
            }
            "use" => self.use_item(id, c.index),
            "equip_left" => {
                if c.index >= 0 {
                    self.equip(id, c.index as usize, true)
                }
            }
            "respawn" => self.rise(id),
            "start_class" => self.start_class(id, &c.key),
            "choose_subclass" => self.choose_subclass(id, &c.key),
            "party_invite" => self.party_invite(id, &c.key),
            "party_accept" => self.party_accept(id, &c.key),
            "party_decline" => self.party_decline(id, &c.key),
            "party_leave" => self.party_leave(id),
            "party_kick" => self.party_kick(id, &c.key),
            "potion" => self.quick_potion(id, c.key == "mana"),
            "unequip" => self.unequip(id, &c.key),
            "drop" => self.drop_inv(id, c.index),
            "sort" => self.sort_inventory(id),
            "talk" => self.talk_ai(id, &c.text),
            "talk_option" => self.talk_option(id, c.index),
            "talk_close" => self.close_dialogue(id, false),
            "buy" => self.buy(id, &c.key, c.index),
            "sell" => self.sell(id, c.index),
            "chat" => {
                let text: String = c.text.chars().take(160).collect();
                if text.trim().is_empty() {
                    return;
                }
                self.say(id, &text, 5000.0);
                let name = self.ents[&id].name.clone();
                self.log_all("#ffffff", format!("[{name}] {text}"));
            }
            _ => {}
        }
    }
}

/// Adds an ability to a hero and to the first free hotbar slot.
pub(crate) fn unlock_ability_on(e: &mut Entity, key: &str) {
    let p = e.pm();
    if p.abilities.iter().any(|a| a == key) {
        return;
    }
    p.abilities.push(key.into());
    if let Some(h) = p.hotbar.iter_mut().find(|h| h.is_empty()) {
        *h = key.into();
    }
    p.dirty = true;
}
