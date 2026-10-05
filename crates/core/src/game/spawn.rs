//! Spawning monsters, squads and the people of the world.

use super::allies::monster_state;
use super::*;
use crate::content::{MonsterDef, SquadDef};
use crate::gen::NpcSpawn;
use crate::world::Vec2;

impl Game {
    pub(crate) fn new_monster(&mut self, def: &MonsterDef, level: &str, p: Pos, lvl: i32) -> Id {
        let (ms, hp) = monster_state(def, lvl, p.center());
        let e = Entity {
            kind: Kind::Monster,
            name: def.name.clone(),
            glyph: def.glyph.clone(),
            color: def.color.clone(),
            level: level.into(),
            pos: p.center(),
            faction: if def.ally {
                Faction::Player
            } else {
                Faction::Monster
            },
            hp,
            max_hp: hp,
            facing: std::f32::consts::FRAC_PI_2,
            monster: Some(Box::new(ms)),
            ..Default::default()
        };
        self.spawn(e)
    }

    pub(crate) fn spawn_npc(&mut self, n: &NpcSpawn, village: &str, r: &mut Rng) {
        let Some(role) = db().npc_role(&n.role) else {
            return;
        };
        let l = &self.levels["overworld"];
        let mut p = n.pos;
        if !l.walkable(p.x, p.y) || self.cell_taken("overworld", p) {
            p = self.free_spot("overworld", p);
        }
        let lvl = self.overworld_level_at(p, false) + 1;
        let gold = 30 + r.int_n(70);
        let id = self.make_npc(role, &n.name, village, p, lvl, gold);
        if !n.night.is_zero() {
            self.ents.get_mut(&id).unwrap().npc.as_mut().unwrap().night = Some(n.night.center());
        }
    }

    pub(crate) fn overworld_level_at(&self, p: Pos, night: bool) -> i32 {
        let mut lvl = 1 + p.manhattan(self.start) / 70;
        if night {
            lvl += 1;
        }
        if let Some(l) = self.levels.get("overworld") {
            lvl += effects::biome_danger(&l.def_at(p).biome);
        }
        lvl
    }

    /// Places a mixed squad around p: the frontline in front, the archers and
    /// casters a little behind.
    pub(crate) fn spawn_squad(
        &mut self,
        r: &mut Rng,
        sq: &SquadDef,
        level: &str,
        p: Pos,
        lvl: i32,
    ) -> Vec<Id> {
        self.squad_seq += 1;
        let squad = self.squad_seq;
        let mut out = Vec::new();
        for mem in &sq.members {
            let Some(def) = db().monster(&mem.monster) else {
                continue;
            };
            let n = r.range(mem.count[0], mem.count[1]);
            for _ in 0..n {
                let at = if ai::squishy(def) {
                    p.add(Pos::new(r.int_n(5) - 2, 2))
                } else {
                    p
                };
                let q = self.free_spot(level, at);
                if q.dist(p) > 6 {
                    continue;
                }
                let m = self.new_monster(def, level, q, lvl);
                let ms = self.ents.get_mut(&m).unwrap().monster.as_mut().unwrap();
                ms.squad = squad;
                ms.home = p.center();
                out.push(m);
            }
        }
        out
    }

    /// Places a pack around p.
    pub(crate) fn spawn_group(
        &mut self,
        r: &mut Rng,
        def: &MonsterDef,
        level: &str,
        p: Pos,
        lvl: i32,
    ) -> Vec<Id> {
        let n = r.range(def.group[0].max(1), def.group[1].max(1));
        let mut out = Vec::new();
        for _ in 0..n {
            let q = self.free_spot(level, p);
            if q.dist(p) > 4 {
                continue;
            }
            out.push(self.new_monster(def, level, q, lvl));
        }
        out
    }

    /// Places persistent elite enemies across the map: every kind of elite
    /// at least once in a fitting biome, more if there is room.
    pub(crate) fn populate_overworld(&mut self, r: &mut Rng) {
        let elites: Vec<&MonsterDef> = db()
            .b
            .monsters
            .iter()
            .filter(|m| m.elite && !m.boss && m.depth[0] == 0)
            .collect();
        let (w, h) = {
            let l = &self.levels["overworld"];
            (l.w, l.h)
        };
        let mut placed = 0;
        for _ in 0..2 {
            if placed >= 8 {
                break;
            }
            for def in &elites {
                for _ in 0..1500 {
                    let p = Pos::new(r.int_n(w), r.int_n(h));
                    let l = &self.levels["overworld"];
                    let biome = l.def_at(p).biome.clone();
                    if !l.walkable(p.x, p.y)
                        || !def.themes.contains(&biome)
                        || self.in_village(p, 25)
                        || p.manhattan(self.start) < 60
                        || self.cell_taken("overworld", p)
                    {
                        continue;
                    }
                    let lvl = self.overworld_level_at(p, false);
                    self.new_monster(def, "overworld", p, lvl + 1);
                    // guards
                    if let Some(minion) = pick_monster(r, &biome, 0, false, false) {
                        self.spawn_group(r, minion, "overworld", p, lvl);
                    }
                    placed += 1;
                    break;
                }
            }
        }
    }

    /// Puts guards into bandit camps and the dead into graveyards.
    pub(crate) fn populate_landmarks(&mut self, r: &mut Rng) {
        for lm in self.landmarks.clone() {
            let keys: &[&str] = match lm.kind.as_str() {
                "camp" => &["bandit_chief", "bandit", "bandit_archer"],
                "graveyard" => &["wandering_skeleton", "ghost"],
                _ => continue,
            };
            let lvl = self.overworld_level_at(lm.pos, false);
            for k in keys {
                let Some(def) = db().monster(k) else { continue };
                let at = lm.pos.add(Pos::new(r.int_n(5) - 2, 1));
                for m in self.spawn_group(r, def, "overworld", at, lvl) {
                    self.ents
                        .get_mut(&m)
                        .unwrap()
                        .monster
                        .as_mut()
                        .unwrap()
                        .persistent = true;
                }
            }
        }
    }

    pub(crate) fn populate_dungeon(
        &mut self,
        level: &str,
        monsters: &[Pos],
        items: &[Pos],
        boss: Option<Pos>,
        ent: &crate::gen::Entrance,
    ) {
        let mut r = Rng::labeled(self.seed, &format!("pop-{level}"));
        let depth = self.levels[level].depth;
        let lvl = depth + (ent.max_depth - 3).max(0);
        for &p in monsters {
            if r.int_n(100) < 30 {
                if let Some(sq) = pick_squad(&mut r, &ent.theme, depth, false) {
                    self.spawn_squad(&mut r, sq, level, p, lvl);
                    continue;
                }
            }
            let Some(def) = pick_monster(&mut r, &ent.theme, depth, false, true) else {
                continue;
            };
            self.spawn_group(&mut r, def, level, p, lvl);
        }
        if let Some(bp) = boss {
            let bosses: Vec<&MonsterDef> = db()
                .b
                .monsters
                .iter()
                .filter(|m| m.boss && m.themes.contains(&ent.theme))
                .collect();
            if !bosses.is_empty() {
                let def = bosses[r.usize_n(bosses.len())];
                let l = &self.levels[level];
                let bp = if l.walkable(bp.x, bp.y) && !self.cell_taken(level, bp) {
                    bp
                } else {
                    self.free_spot(level, bp)
                };
                self.new_monster(def, level, bp, lvl + 1);
                if let Some(minion) = pick_monster(&mut r, &ent.theme, depth, false, false) {
                    self.spawn_group(&mut r, minion, level, bp.add(Pos::new(0, 2)), lvl);
                }
            }
        }
        for &p in items {
            if r.int_n(3) == 0 {
                let n = 5 + r.int_n(10 * depth + 5);
                self.drop_gold(level, p, n);
                continue;
            }
            if let Some(st) = self.random_item(depth, 20.0) {
                self.drop_item(level, p, st);
            }
        }
        self.index_levels();
    }

    /// Keeps the overworld around players alive and removes far monsters.
    pub(crate) fn run_spawner(&mut self) {
        if !self.levels.contains_key("overworld") {
            return;
        }
        let players: Vec<Vec2> = self
            .online
            .values()
            .filter_map(|id| self.ents.get(id))
            .filter(|p| p.level == "overworld" && !p.dead)
            .map(|p| p.pos)
            .collect();
        if players.is_empty() {
            return;
        }
        let night = self.is_night();
        // despawn
        for id in self.on_level("overworld") {
            let e = &self.ents[&id];
            let Some(m) = &e.monster else { continue };
            if e.faction != Faction::Monster || m.persistent || m.target != 0 {
                continue;
            }
            if players.iter().all(|p| p.dist(e.pos) >= 70.0) {
                self.remove(id);
            }
        }
        let want = if night { 10 } else { 6 };
        let mut rng = std::mem::replace(&mut self.rng, Rng::new(0, 0));
        for &pp in &players {
            let near = self.on_level("overworld").into_iter().filter(|id| {
                let e = &self.ents[id];
                e.monster.is_some() && e.faction == Faction::Monster && e.pos.dist(pp) <= 32.0
            });
            if near.count() >= want {
                continue;
            }
            for _ in 0..25 {
                let ang = rng.f32() * std::f32::consts::TAU;
                let dist = 22.0 + rng.f32() * 12.0;
                let q = (pp + Vec2::from_angle(ang) * dist).cell();
                let ow = &self.levels["overworld"];
                if !ow.walkable(q.x, q.y)
                    || !ow.def_at(q).interact.is_empty()
                    || self.in_village(q, 6)
                    || self.cell_taken("overworld", q)
                {
                    continue;
                }
                if players.iter().any(|o| o.dist(q.center()) < 16.0) {
                    continue;
                }
                let biome = ow.def_at(q).biome.clone();
                let lvl = self.overworld_level_at(q, night);
                if rng.chance(30.0) {
                    if let Some(sq) = pick_squad(&mut rng, &biome, 0, night) {
                        self.spawn_squad(&mut rng, sq, "overworld", q, lvl);
                        break;
                    }
                }
                let Some(def) = pick_monster(&mut rng, &biome, 0, night, false) else {
                    continue;
                };
                self.spawn_group(&mut rng, def, "overworld", q, lvl);
                break;
            }
        }
        self.rng = rng;
        self.index_levels();
    }
}

/// A weighted random non-boss monster matching theme and depth.
pub(crate) fn pick_monster(
    r: &mut Rng,
    theme: &str,
    depth: i32,
    night: bool,
    allow_elite: bool,
) -> Option<&'static MonsterDef> {
    let mut pool = Vec::new();
    let mut total = 0;
    for m in &db().b.monsters {
        if m.boss
            || m.ally
            || m.weight <= 0
            || (m.elite && !allow_elite)
            || !m.themes.iter().any(|t| t == theme)
        {
            continue;
        }
        // depth [0,0]: overworld only; [0,n]: overworld and dungeon floors 1..n
        if depth == 0 {
            if m.depth[0] != 0 || (m.night && !night) {
                continue;
            }
        } else if depth < m.depth[0].max(1) || depth > m.depth[1] {
            continue;
        }
        pool.push(m);
        total += m.weight;
    }
    if total == 0 {
        return None;
    }
    let mut n = r.int_n(total);
    for m in &pool {
        n -= m.weight;
        if n < 0 {
            return Some(m);
        }
    }
    pool.last().copied()
}

/// A weighted random squad for a theme and depth.
pub(crate) fn pick_squad(
    r: &mut Rng,
    theme: &str,
    depth: i32,
    night: bool,
) -> Option<&'static SquadDef> {
    let mut pool = Vec::new();
    let mut total = 0;
    for sq in &db().b.squads {
        if sq.weight <= 0 || !sq.themes.iter().any(|t| t == theme) {
            continue;
        }
        if depth == 0 {
            if sq.depth[0] != 0 || (sq.night && !night) {
                continue;
            }
        } else if depth < sq.depth[0].max(1) || depth > sq.depth[1] {
            continue;
        }
        pool.push(sq);
        total += sq.weight;
    }
    if total == 0 {
        return None;
    }
    let mut n = r.int_n(total);
    for sq in &pool {
        n -= sq.weight;
        if n < 0 {
            return Some(sq);
        }
    }
    pool.last().copied()
}
