//! Continuous movement: creatures walk in any direction at their own pace,
//! slide along walls and push against each other.

use super::*;
use crate::world::{find_path, Pos, Vec2, DIRS8};

/// How far apart two bodies must stay (multiplied radii sum).
const BODY_GAP: f32 = 0.98;

impl Game {
    /// Rebuilds the list of entities of every level.
    pub(crate) fn index_levels(&mut self) {
        for v in self.by_level.values_mut() {
            v.clear();
        }
        for (id, e) in &self.ents {
            self.by_level.entry(e.level.clone()).or_default().push(*id);
        }
    }

    /// The entities of a level (as of the last index).
    pub(crate) fn on_level(&self, level: &str) -> Vec<Id> {
        self.by_level
            .get(level)
            .map(|v| {
                v.iter()
                    .copied()
                    .filter(|id| self.ents.get(id).map(|e| e.level == level).unwrap_or(false))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Is a cell taken by a living body?
    pub(crate) fn cell_taken(&self, level: &str, p: Pos) -> bool {
        self.by_level.get(level).map(|v| {
            v.iter().any(|id| {
                self.ents
                    .get(id)
                    .map(|e| e.level == level && e.blocks() && e.alive() && e.cell() == p)
                    .unwrap_or(false)
            })
        }) == Some(true)
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
            if let Some(list) = self.by_level.get(&e.level) {
                for oid in list {
                    let Some(o) = self.ents.get(oid) else {
                        continue;
                    };
                    if o.level != e.level || !self.collides(e, o) {
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
                let taken: Vec<Pos> = self
                    .on_level(&level)
                    .iter()
                    .filter_map(|o| self.ents.get(o))
                    .filter(|o| o.id != id && o.blocks() && o.alive() && o.pos.dist(pos) < 12.0)
                    .map(|o| o.cell())
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
