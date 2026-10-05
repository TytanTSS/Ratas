//! Creatures on the players' side: villagers, guards, wanderers and summons.

use super::*;
use crate::content::{MonsterDef, NpcRoleDef};
use crate::gen::person_name;
use crate::world::Vec2;

/// The combat state of a creature of a level, and its health.
pub(crate) fn monster_state(def: &MonsterDef, lvl: i32, home: Vec2) -> (MonsterState, f64) {
    let lvl = lvl.max(1);
    let hp_scale = 1.0 + 0.25 * (lvl - 1) as f64;
    let dmg_scale = monster_scale(lvl);
    (
        MonsterState {
            def: def.key.clone(),
            lvl,
            home,
            state: "idle".into(),
            damage: [def.damage[0] * dmg_scale, def.damage[1] * dmg_scale],
            armor: def.armor + (lvl - 1) as f64 / 2.0,
            move_ms: def.move_ms as f64,
            attack_ms: def.attack_ms as f64,
            xp: (def.xp as f64 * (1.0 + 0.3 * (lvl - 1) as f64)) as i32,
            persistent: def.boss || def.elite,
            ..Default::default()
        },
        def.hp * hp_scale,
    )
}

/// How far a wanderer looks for the next place to go.
const TRAVEL_RANGE: f32 = 220.0;

impl Game {
    /// Creates a villager, guard or wanderer; NPCs with a combat profile
    /// fight monsters on the players' side.
    pub(crate) fn make_npc(
        &mut self,
        role: &NpcRoleDef,
        name: &str,
        village: &str,
        p: Pos,
        lvl: i32,
        gold: i32,
    ) -> Id {
        let mut e = Entity {
            kind: Kind::Npc,
            name: format!("{name} ({})", role.name),
            glyph: role.glyph.clone(),
            color: role.color.clone(),
            level: "overworld".into(),
            pos: p.center(),
            faction: Faction::Neutral,
            hp: 50.0,
            max_hp: 50.0,
            facing: std::f32::consts::FRAC_PI_2,
            npc: Some(Box::new(NpcState {
                role: role.key.clone(),
                pname: name.into(),
                home: p.center(),
                village: village.into(),
                gold,
                ..Default::default()
            })),
            ..Default::default()
        };
        if let Some(def) = db().monster(&role.combat) {
            let (mut ms, hp) = monster_state(def, lvl, p.center());
            ms.persistent = true;
            e.monster = Some(Box::new(ms));
            e.faction = Faction::Player;
            e.hp = hp;
            e.max_hp = hp;
        }
        self.spawn(e)
    }

    /// Places knights, hunters, pilgrims and others in the wild.
    pub(crate) fn populate_wanderers(&mut self, r: &mut Rng) {
        // as many per settlement as in the classic world of five
        let per = (self.villages.len() as f64 / 6.0).max(1.0);
        for role in &db().b.npcs {
            if !role.world {
                continue;
            }
            let n = (r.range(role.count[0], role.count[1]) as f64 * per).round() as i32;
            for _ in 0..n {
                let Some(p) = self.wild_spot(r, 12, 60) else {
                    continue;
                };
                let name = person_name(r);
                let lvl = self.overworld_level_at(p, false) + 2;
                let gold = 20 + r.int_n(60);
                self.make_npc(role, &name, "", p, lvl, gold);
            }
        }
    }

    /// Free land outside villages, between min_d and max_d steps from some
    /// village (or anywhere when there are none).
    pub(crate) fn wild_spot(&self, r: &mut Rng, min_d: i32, max_d: i32) -> Option<Pos> {
        let l = &self.levels["overworld"];
        for _ in 0..400 {
            let p = if !self.villages.is_empty() {
                let v = r.pick(&self.villages).center;
                let p = Pos::new(
                    v.x + r.int_n(2 * max_d + 1) - max_d,
                    v.y + r.int_n(2 * max_d + 1) - max_d,
                );
                if p.manhattan(v) < min_d {
                    continue;
                }
                p
            } else {
                Pos::new(r.int_n(l.w), r.int_n(l.h))
            };
            let def = l.def_at(p);
            if l.walkable(p.x, p.y)
                && def.interact.is_empty()
                && def.damage == 0.0
                && !self.in_village(p, 4)
                && !self.cell_taken("overworld", p)
            {
                return Some(p);
            }
        }
        None
    }

    /// Moves a wandering NPC between villages and landmarks.
    pub(crate) fn travel(&mut self, id: Id) {
        let now = self.now;
        let e = &self.ents[&id];
        let n = e.npc.as_ref().unwrap();
        let pos = e.pos;
        let arrived = n.travel.map(|t| pos.dist(t) <= 2.5).unwrap_or(false);
        if n.travel.is_none() || arrived || now > n.travel_until {
            if arrived && self.rng.chance(70.0) {
                let rest = 4000.0 + self.rng.f64() * 8000.0;
                let n = self.ents.get_mut(&id).unwrap().npc.as_mut().unwrap();
                n.travel_until = now + rest; // rest a little
                n.travel = None;
                self.stop(id);
                return;
            }
            if n.travel.is_none() && now < n.travel_until {
                return;
            }
            // the next village or sight on the way, not across the world
            let near = |p: Vec2| p.dist(pos) <= TRAVEL_RANGE;
            let mut goals: Vec<Vec2> = self
                .villages
                .iter()
                .map(|v| Pos::new(v.center.x, v.center.y + 3).center())
                .chain(self.landmarks.iter().map(|l| l.pos.center()))
                .filter(|&g| near(g) && g.dist(pos) > 6.0)
                .collect();
            if goals.is_empty() {
                // lost in the wild: the nearest place will do
                goals.extend(
                    self.villages
                        .iter()
                        .map(|v| Pos::new(v.center.x, v.center.y + 3).center())
                        .min_by(|a, b| a.dist(pos).total_cmp(&b.dist(pos))),
                );
            }
            if goals.is_empty() {
                return;
            }
            let g = *self.rng.pick(&goals);
            let n = self.ents.get_mut(&id).unwrap().npc.as_mut().unwrap();
            n.travel = Some(g);
            n.travel_until = now + 240000.0;
            n.home = g;
            if let Some(m) = self.ents.get_mut(&id).unwrap().monster.as_mut() {
                m.home = g;
            }
        }
        let e = self.ents.get_mut(&id).unwrap();
        if e.monster.is_some() {
            e.pace = 0.7; // a travelling pace, slower than a charge
            let t = e.npc.as_ref().unwrap().travel.unwrap();
            if e.goal.is_none() || e.goal.map(|g| g.dist(e.pos) < 0.3).unwrap_or(true) {
                self.step_toward(id, t);
            }
        }
    }

    /// Removes a fallen ally; guards and wanderers return later.
    pub(crate) fn kill_ally(&mut self, id: Id) {
        let Some(e) = self.remove(id) else { return };
        self.fx(&e.level, e.pos, "", '%', "#a03030", 1200);
        if e.owner != 0 {
            if self.ents.contains_key(&e.owner) && e.expires > self.now + 500.0 {
                self.log(e.owner, "#c0c0c0", format!("{} пал.", e.name));
            }
            return;
        }
        if e.npc.is_some() {
            for p in self.players_on(&e.level) {
                if self.ents[&p].pos.dist(e.pos) <= 20.0 {
                    self.log(p, "#ff8a8a", format!("{} пал в бою.", e.name));
                }
            }
            let at = self.now + 180000.0;
            self.revivals.push(PendingNpc { e, at });
        }
    }

    /// Brings fallen NPCs back to their homes.
    pub(crate) fn revive_npcs(&mut self) {
        let now = self.now;
        let (ready, kept): (Vec<PendingNpc>, Vec<PendingNpc>) = std::mem::take(&mut self.revivals)
            .into_iter()
            .partition(|p| now >= p.at);
        self.revivals = kept;
        for pn in ready {
            let mut e = pn.e;
            if !self.levels.contains_key(&e.level) {
                continue;
            }
            e.dead = false;
            e.hp = e.max_hp;
            e.buffs.clear();
            let home = e.npc.as_ref().unwrap().home.cell();
            e.pos = self.free_spot(&e.level, home).center();
            if let Some(m) = e.monster.as_mut() {
                m.target = 0;
                m.state = "idle".into();
            }
            self.spawn(e);
        }
    }

    /// Removes summons and illusions whose time is up or whose owner is gone.
    pub(crate) fn summon_expired(&mut self, id: Id) -> bool {
        let e = &self.ents[&id];
        if e.owner == 0 {
            return false;
        }
        let gone = match self.ents.get(&e.owner) {
            None => true,
            Some(o) => {
                (e.expires > 0.0 && self.now >= e.expires)
                    || o.level != e.level
                    || (o.player.is_some() && o.dead)
            }
        };
        if gone {
            let (level, pos) = (e.level.clone(), e.pos);
            self.fx(&level, pos, "", '*', "#c0a0ff", 400);
            self.remove(id);
        }
        gone
    }
}
