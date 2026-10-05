//! Continuous movement: creatures walk in any direction at their own pace,
//! slide along walls and push against each other.

use super::*;
use crate::world::{find_path, Pos, Vec2, DIRS8};
use std::collections::HashSet;

/// How far apart two bodies must stay (multiplied radii sum).
const BODY_GAP: f32 = 0.98;

/// Creatures farther than this from every player (cells along each axis,
/// beyond what clients are shown) sleep: they are not simulated, and on a
/// level with players only cell lookups see them.
pub const AWAKE_RANGE: f32 = 80.0;

/// Side of the cells of the spatial index, in tiles.
const GRID: i32 = 16;

/// The entities of one level by squares of GRID×GRID tiles: lookups of a cell
/// cost the same on a small dungeon floor and on a huge overworld.
#[derive(Default)]
pub(crate) struct Grid {
    cw: i32,
    ch: i32,
    cells: Vec<Vec<Id>>,
}

impl Grid {
    fn new(w: i32, h: i32) -> Grid {
        let (cw, ch) = ((w + GRID - 1) / GRID, (h + GRID - 1) / GRID);
        Grid {
            cw,
            ch,
            cells: vec![Vec::new(); (cw * ch).max(0) as usize],
        }
    }

    fn slot(&self, p: Pos) -> Option<usize> {
        let (x, y) = (p.x.div_euclid(GRID), p.y.div_euclid(GRID));
        (x >= 0 && y >= 0 && x < self.cw && y < self.ch).then(|| (y * self.cw + x) as usize)
    }

    pub(crate) fn add(&mut self, p: Pos, id: Id) {
        if let Some(i) = self.slot(p) {
            self.cells[i].push(id);
        }
    }

    /// The ids stored in the square of a cell.
    fn at(&self, p: Pos) -> &[Id] {
        self.slot(p)
            .map(|i| self.cells[i].as_slice())
            .unwrap_or(&[])
    }

    /// Adds the ids stored in the squares a box of half-size r around p touches.
    fn around(&self, p: Vec2, r: f32, out: &mut Vec<Id>) {
        let span = |a: f32, n: i32| {
            let lo = ((a - r).floor() as i32).div_euclid(GRID).max(0);
            let hi = ((a + r).floor() as i32).div_euclid(GRID).min(n - 1);
            lo..=hi
        };
        for y in span(p.y, self.ch) {
            for x in span(p.x, self.cw) {
                out.extend_from_slice(&self.cells[(y * self.cw + x) as usize]);
            }
        }
    }
}

/// The spatial index is rebuilt once a tick and a creature walks less than
/// this in between, so lookups around a point look this much further.
const NEAR_SLACK: f32 = 1.5;

impl Game {
    /// Rebuilds the indices of entities: the spatial grid of every level and
    /// the lists of entities to simulate and query — on a level with players
    /// only the awake ones near them, elsewhere all.
    pub(crate) fn index_levels(&mut self) {
        let players: Vec<(String, Vec2)> = self
            .online
            .values()
            .filter_map(|id| self.ents.get(id))
            .map(|e| (e.level.clone(), e.pos))
            .collect();
        for v in self.by_level.values_mut() {
            v.clear();
        }
        for g in self.grids.values_mut() {
            for c in &mut g.cells {
                c.clear();
            }
        }
        for (lid, l) in &self.levels {
            if !self.grids.contains_key(lid) {
                self.grids.insert(lid.clone(), Grid::new(l.w, l.h));
            }
        }
        // entities come in runs of one level: look the level up once per run
        let mut last = String::new();
        let mut watched: Vec<Vec2> = Vec::new();
        let mut list: Option<&mut Vec<Id>> = None;
        let mut grid: Option<&mut Grid> = None;
        let (by_level, grids) = (&mut self.by_level, &mut self.grids);
        for (id, e) in &self.ents {
            if e.level != last {
                last.clone_from(&e.level);
                watched = players
                    .iter()
                    .filter(|(lv, _)| *lv == e.level)
                    .map(|(_, p)| *p)
                    .collect();
                if !by_level.contains_key(&e.level) {
                    by_level.insert(e.level.clone(), Vec::new());
                }
                // two disjoint maps: both entries can be held at once
                list = by_level.get_mut(&e.level);
                grid = grids.get_mut(&e.level);
            }
            if let Some(g) = grid.as_deref_mut() {
                g.add(e.cell(), *id);
            }
            let awake = watched.is_empty()
                || watched.iter().any(|p| {
                    (p.x - e.pos.x).abs() <= AWAKE_RANGE && (p.y - e.pos.y).abs() <= AWAKE_RANGE
                });
            if awake {
                if let Some(v) = list.as_deref_mut() {
                    v.push(*id);
                }
            }
        }
    }

    /// Ids of the entities of a level to simulate: the awake ones on a level
    /// with players (see AWAKE_RANGE), all of them elsewhere.
    pub(crate) fn awake_ids(&self) -> Vec<Id> {
        let played: std::collections::HashSet<&str> = self
            .online
            .values()
            .filter_map(|id| self.ents.get(id))
            .map(|e| e.level.as_str())
            .collect();
        let mut ids: Vec<Id> = self
            .by_level
            .iter()
            .filter(|(l, _)| played.contains(l.as_str()))
            .flat_map(|(_, v)| v.iter().copied())
            .collect();
        ids.sort_unstable();
        ids
    }

    /// Every entity of a level, asleep or not (a full scan: for rare needs
    /// such as despawning or looking someone up).
    pub(crate) fn all_on_level(&self, level: &str) -> Vec<Id> {
        self.ents
            .iter()
            .filter(|(_, e)| e.level == level)
            .map(|(id, _)| *id)
            .collect()
    }

    /// The entities of a level as of the last index: on a level with players
    /// only those awake near them.
    pub(crate) fn on_level(&self, level: &str) -> Vec<Id> {
        self.by_level
            .get(level)
            .map(|v| {
                v.iter()
                    .copied()
                    .filter(|id| self.ents.get(id).is_some_and(|e| e.level == level))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The entities of a level that may be within r of p, asleep or not, by
    /// ascending id (callers check the exact distance): a lookup in the
    /// spatial index instead of a walk through the whole level.
    pub(crate) fn near(&self, level: &str, p: Vec2, r: f32) -> Vec<Id> {
        let Some(g) = self.grids.get(level) else {
            return self.on_level(level);
        };
        let mut ids = Vec::new();
        g.around(p, r + NEAR_SLACK, &mut ids);
        ids.sort_unstable();
        ids.dedup();
        ids.retain(|id| self.ents.get(id).is_some_and(|e| e.level == level));
        ids
    }

    /// Is a cell taken by a living body (asleep or not)?
    pub(crate) fn cell_taken(&self, level: &str, p: Pos) -> bool {
        let taken = |id: &Id| {
            self.ents
                .get(id)
                .is_some_and(|e| e.level == level && e.blocks() && e.alive() && e.cell() == p)
        };
        match self.grids.get(level) {
            Some(g) => g.at(p).iter().any(taken),
            None => self
                .by_level
                .get(level)
                .is_some_and(|v| v.iter().any(taken)),
        }
    }

    /// A free walkable cell near p (no body, no interaction).
    pub(crate) fn free_spot(&self, level: &str, p: Pos) -> Pos {
        let Some(l) = self.levels.get(level) else {
            return p;
        };
        find_free(l, p, &|q| self.cell_taken(level, q))
    }

    /// Teleports an entity (no collision checks).
    pub(crate) fn place(&mut self, id: Id, p: Vec2) {
        if let Some(e) = self.ents.get_mut(&id) {
            e.pos = p;
            e.goal = None;
            e.want = Vec2::ZERO;
            if let Some(m) = e.monster.as_mut() {
                m.path.clear();
            }
        }
    }

    /// Whether `a` must not walk through `b`.
    pub(crate) fn collides(&self, a: &Entity, b: &Entity) -> bool {
        if a.id == b.id || !b.blocks() || !b.alive() || b.kind == Kind::Player && b.dead {
            return false;
        }
        if a.owner == b.id || b.owner == a.id {
            return false; // summons and their masters pass through each other
        }
        if a.kind == Kind::Player {
            // heroes are only stopped by enemies; townsfolk step aside
            return self.hostile(a.id, b.id);
        }
        if b.kind == Kind::Player && !self.hostile(a.id, b.id) {
            return false;
        }
        true
    }

    /// Moves every creature that wants to move this tick.
    pub(crate) fn move_all(&mut self, ids: &[Id]) {
        let dt = (TICK_MS / 1000.0) as f32;
        for &id in ids {
            let Some(e) = self.ents.get(&id) else {
                continue;
            };
            if !e.blocks() || !e.alive() {
                if let Some(e) = self.ents.get_mut(&id) {
                    e.vel = Vec2::ZERO;
                }
                continue;
            }
            let Some(l) = self.levels.get(&e.level) else {
                continue;
            };
            // steering toward a goal point
            let mut want = e.want;
            let mut limit = f32::MAX;
            if want.len_sq() < 1e-6 {
                if let Some(goal) = e.goal {
                    let d = goal - e.pos;
                    if d.len() < 0.02 {
                        if let Some(e) = self.ents.get_mut(&id) {
                            e.goal = None;
                            e.vel = Vec2::ZERO;
                        }
                        continue;
                    }
                    want = d.norm();
                    limit = d.len();
                }
            }
            if want.len_sq() < 1e-6 || e.stats.stunned {
                if let Some(e) = self.ents.get_mut(&id) {
                    e.vel = Vec2::ZERO;
                }
                continue;
            }
            let cost = l.def_at(e.pos.cell()).move_cost;
            let step = (e.speed(cost) * dt).min(limit);
            let r = e.radius();
            let from = e.pos;
            let monster_ai = e.kind != Kind::Player;
            let blocked = |x: i32, y: i32| -> bool {
                let d = l.def(x, y);
                if !l.inside(x, y) || !d.walkable {
                    return true;
                }
                // creatures keep off stairs and hazards unless they must
                monster_ai && !d.interact.is_empty() && d.interact != "door"
            };
            let mut to = l.slide(from, want * step, r, &blocked);
            // bodies push each other apart: the mover stops at the other
            for oid in self.near(&e.level, to, r + BOSS_RADIUS) {
                let o = &self.ents[&oid];
                if !self.collides(e, o) {
                    continue;
                }
                let min = (r + o.radius()) * BODY_GAP;
                let d = to - o.pos;
                let dl = d.len();
                if dl >= min {
                    continue;
                }
                // only resolve if the move brought us closer
                if dl >= (from - o.pos).len() - 1e-4 && dl > 0.01 {
                    continue;
                }
                let n = if dl > 1e-4 {
                    d * (1.0 / dl)
                } else {
                    (from - o.pos).norm()
                };
                let pushed = o.pos + n * min;
                if l.circle_fits(pushed, r, &blocked) {
                    to = pushed;
                } else {
                    to = from;
                }
            }
            let moved = to - from;
            let e = self.ents.get_mut(&id).unwrap();
            e.pos = to;
            e.vel = moved * (1.0 / dt);
            if moved.len() > 1e-3 && e.kind != Kind::Player {
                e.facing = moved.angle();
            }
            if e.kind == Kind::Player && moved.len() > 1e-3 {
                let walked = moved.len();
                let pid = e.id;
                let before = from.cell();
                let after = to.cell();
                self.deed_f(pid, "steps", walked);
                if before != after {
                    self.after_player_move(pid);
                }
            }
        }
    }

    /// Sets an AI creature to walk to a point.
    pub(crate) fn walk_to(&mut self, id: Id, p: Vec2) {
        if let Some(e) = self.ents.get_mut(&id) {
            e.goal = Some(p);
            e.want = Vec2::ZERO;
        }
    }

    pub(crate) fn stop(&mut self, id: Id) {
        if let Some(e) = self.ents.get_mut(&id) {
            e.goal = None;
            e.want = Vec2::ZERO;
        }
    }

    /// Walks along an A* path toward goal (recomputed when needed). Returns
    /// false when no step can be made.
    pub(crate) fn step_toward(&mut self, id: Id, goal: Vec2) -> bool {
        let now = self.now;
        let Some(e) = self.ents.get(&id) else {
            return false;
        };
        if e.stats.stunned {
            return false;
        }
        let level = e.level.clone();
        let (pos, cell, gcell) = (e.pos, e.pos.cell(), goal.cell());
        if pos.dist(goal) < 0.15 {
            self.stop(id);
            return false;
        }
        // in the same cell or a straight free line: walk directly
        let Some(l) = self.levels.get(&level) else {
            return false;
        };
        if cell == gcell
            || (pos.dist(goal) < 6.0 && self.straight_walk(&level, pos, goal, e.radius()))
        {
            self.walk_to(id, goal);
            return true;
        }
        let needs_path = {
            let ms = e.monster.as_ref();
            match ms {
                Some(ms) => {
                    ms.path.is_empty() || ms.path_goal != gcell || now - ms.path_at > 1500.0
                }
                None => true,
            }
        };
        let mut path: Vec<Pos> = match e.monster.as_ref() {
            Some(ms) if !needs_path => ms.path.clone(),
            _ => {
                let taken: HashSet<Pos> = self
                    .near(&level, pos, 12.0)
                    .iter()
                    .map(|o| &self.ents[o])
                    .filter(|o| o.id != id && o.blocks() && o.alive() && o.pos.dist(pos) < 12.0)
                    .map(|e| e.cell())
                    .collect();
                let mut cost = |x: i32, y: i32| -> f64 {
                    let def = l.def(x, y);
                    if def.interact == "door" {
                        return 2.0; // creatures can open doors
                    }
                    if !def.walkable
                        || matches!(
                            def.interact.as_str(),
                            "stairs_up" | "stairs_down" | "dungeon"
                        )
                    {
                        return -1.0;
                    }
                    let mut c = def.move_cost;
                    if def.damage > 0.0 {
                        c += 15.0;
                    }
                    if taken.contains(&Pos::new(x, y)) {
                        c += 4.0;
                    }
                    c
                };
                find_path(l.w, l.h, cell, gcell, 600, true, &mut cost)
            }
        };
        // drop waypoints already reached
        while let Some(first) = path.first() {
            if *first == cell || first.center().dist(pos) < 0.2 {
                path.remove(0);
            } else {
                break;
            }
        }
        if let Some(ms) = self.ents.get_mut(&id).and_then(|e| e.monster.as_mut()) {
            if needs_path {
                ms.path_at = now;
                ms.path_goal = gcell;
            }
            ms.path = path.clone();
        }
        let Some(&next) = path.first() else {
            // greedy fallback
            return self.greedy_step(id, goal);
        };
        // open a door on the way
        let l = &self.levels[&level];
        let def = l.def_at(next);
        if def.interact == "door" && !def.becomes.is_empty() && !self.cell_taken(&level, next) {
            let t = db().tile_id(&def.becomes);
            self.set_tile(&level, next.x, next.y, t);
            self.stop(id);
            if let Some(e) = self.ents.get_mut(&id) {
                e.face_to(next.center());
            }
            return true;
        }
        let target = if next == gcell { goal } else { next.center() };
        self.walk_to(id, target);
        true
    }

    /// Whether a body can walk straight from a to b without hitting walls.
    pub(crate) fn straight_walk(&self, level: &str, a: Vec2, b: Vec2, r: f32) -> bool {
        let Some(l) = self.levels.get(level) else {
            return false;
        };
        let d = b - a;
        let n = (d.len() / 0.3).ceil() as i32;
        for i in 1..=n {
            let p = a + d * (i as f32 / n as f32);
            let blocked = |x: i32, y: i32| {
                let def = l.def(x, y);
                !l.inside(x, y)
                    || !def.walkable
                    || def.damage > 0.0
                    || (!def.interact.is_empty() && def.interact != "door")
            };
            if !l.circle_fits(p, r * 0.9, &blocked) {
                return false;
            }
        }
        true
    }

    pub(crate) fn greedy_step(&mut self, id: Id, goal: Vec2) -> bool {
        let Some(e) = self.ents.get(&id) else {
            return false;
        };
        let l = &self.levels[&e.level];
        let cell = e.cell();
        let mut best: Option<Pos> = None;
        let mut bd = goal.dist(e.pos);
        for d in DIRS8 {
            let to = cell.add(d);
            if !self.can_step(&e.level, to) {
                continue;
            }
            let dd = to.center().dist(goal);
            if dd < bd && l.walkable(to.x, to.y) {
                best = Some(to);
                bd = dd;
            }
        }
        match best {
            Some(b) => {
                self.walk_to(id, b.center());
                true
            }
            None => false,
        }
    }

    pub(crate) fn can_step(&self, level: &str, p: Pos) -> bool {
        let Some(l) = self.levels.get(level) else {
            return false;
        };
        if !l.walkable(p.x, p.y) || self.cell_taken(level, p) {
            return false;
        }
        let def = l.def_at(p);
        def.interact.is_empty() || def.interact == "door"
    }

    /// Steps away from a threat: to the free neighbour farthest from it.
    pub(crate) fn step_away(&mut self, id: Id, from: Vec2) -> bool {
        let Some(e) = self.ents.get(&id) else {
            return false;
        };
        let l = &self.levels[&e.level];
        let cell = e.cell();
        let mut best: Option<Pos> = None;
        let mut bd = e.pos.dist(from);
        for d in DIRS8 {
            let to = cell.add(d);
            if !self.can_step(&e.level, to) || l.def_at(to).damage > 0.0 {
                continue;
            }
            // no cutting corners
            if d.x != 0
                && d.y != 0
                && (!l.walkable(cell.x + d.x, cell.y) || !l.walkable(cell.x, cell.y + d.y))
            {
                continue;
            }
            let dd = to.center().dist(from);
            if dd > bd {
                best = Some(to);
                bd = dd;
            }
        }
        match best {
            Some(b) => {
                self.walk_to(id, b.center());
                true
            }
            None => false,
        }
    }
}
