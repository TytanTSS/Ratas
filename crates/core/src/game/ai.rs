//! The tactical AI of monsters and the creatures on the players' side
//! (guards, wanderers, summons). Every creature has a role:
//!   frontline  — engages and protects its archers and casters;
//!   skirmisher — flanks, strikes the weakest from behind, retreats when hurt;
//!   ranged/caster — keeps its distance, steps back from melee, shoots;
//!   support    — stays back, heals wounded allies, then shoots;
//!   leader     — fights in front and rallies the squad; its death breaks morale.

use super::combat::ability_range;
use super::*;
use crate::content::{AbilityDef, MonsterDef};
use crate::llm::{TacticReply, TacticRequest, TACTICS};
use crate::world::{los, Vec2};

pub(crate) fn role_of(def: &MonsterDef) -> &str {
    if !def.role.is_empty() {
        &def.role
    } else if def.behavior == "ranged" {
        "ranged"
    } else {
        "frontline"
    }
}

pub(crate) fn squishy(def: &MonsterDef) -> bool {
    matches!(role_of(def), "ranged" | "caster" | "support")
}

/// The monster's main special ability.
pub(crate) fn monster_ability(d: &MonsterDef) -> Option<&'static AbilityDef> {
    if d.ability.is_empty() {
        None
    } else {
        db().ability(&d.ability)
    }
}

fn monster_abilities(d: &MonsterDef) -> Vec<&'static AbilityDef> {
    let mut out: Vec<&'static AbilityDef> = monster_ability(d).into_iter().collect();
    out.extend(d.abilities.iter().filter_map(|k| db().ability(k)));
    out
}

impl Game {
    pub(crate) fn def_of(&self, id: Id) -> &'static MonsterDef {
        let key = self.ents[&id].monster.as_ref().map(|m| m.def.as_str()).unwrap_or("bandit");
        db().monster(key).or_else(|| db().monster("bandit")).unwrap_or(&db().b.monsters[0])
    }

    pub(crate) fn ms(&mut self, id: Id) -> &mut MonsterState {
        self.ents.get_mut(&id).unwrap().monster.as_mut().unwrap()
    }

    /// Runs one creature's AI; returns true while it is busy fighting (NPC
    /// fighters only wander when it is not).
    pub(crate) fn update_monster(&mut self, id: Id) -> bool {
        if self.summon_expired(id) {
            return true;
        }
        let now = self.now;
        let e = &self.ents[&id];
        let ms = e.monster.as_ref().unwrap();
        if now < ms.next_think {
            return ms.target != 0;
        }
        let think = 80.0 + self.rng.f64() * 60.0;
        self.ms(id).next_think = now + think;
        let def = self.def_of(id);
        let e = &self.ents[&id];
        let level = e.level.clone();
        let hz = self.levels[&level].def_at(e.cell());
        if hz.damage > 0.0 && self.tick_n % HALF_SEC < 4 {
            let mut d = Damage::single(hazard_type(hz), hz.damage / 2.0 * think / 500.0);
            d.dot = true;
            self.damage(None, id, d);
            if !self.alive(id) {
                return false;
            }
        }
        let e = &self.ents[&id];
        if e.stats.stunned || def.behavior == "decoy" {
            self.stop(id);
            return true;
        }

        let Some(target) = self.choose_target(id, def) else {
            self.ms(id).target = 0;
            self.idle(id);
            return false;
        };
        // leashes: summons stay by their owner, guards by their post
        let e = &self.ents[&id];
        if e.owner != 0 {
            if let Some(o) = self.ents.get(&e.owner) {
                if e.dist(o) > 12.0 {
                    let op = o.pos;
                    self.ms(id).target = 0;
                    self.step_toward(id, op);
                    return true;
                }
            }
        }
        let e = &self.ents[&id];
        if let Some(n) = &e.npc {
            if n.travel.is_none() && e.pos.dist(e.monster.as_ref().unwrap().home) > 14.0 {
                let home = e.monster.as_ref().unwrap().home;
                let ms = self.ms(id);
                ms.target = 0;
                ms.state = "return".into();
                self.step_toward(id, home);
                return true;
            }
        }
        if e.faction == Faction::Monster {
            self.maybe_ask_tactic(id, def, target);
        }

        let e = &self.ents[&id];
        let ms = e.monster.as_ref().unwrap();
        let tactic = if now < ms.tactic_until { ms.tactic.clone() } else { String::new() };
        let te = &self.ents[&target];
        let tpos = te.pos;
        let dist = e.dist(te);
        let hp = e.hp / e.max_hp;
        let mut role = role_of(def).to_string();

        // fleeing: badly hurt (once per fight), broken morale or ordered to retreat
        if hp < 0.2 && !def.boss && ms.state != "fled" && role != "leader" && e.faction == Faction::Monster && tactic != "aggressive" && self.chance(40.0) {
            let ms = self.ms(id);
            ms.state = "fled".into();
            ms.flee_until = now + 3500.0;
        }
        if tactic == "retreat" && self.ms(id).flee_until < now {
            self.ms(id).flee_until = now + 2500.0;
        }
        if now < self.ms(id).flee_until {
            if !self.step_away(id, tpos) && self.in_reach(id, target) && now >= self.ents[&id].next_attack {
                self.melee_attack(id, target);
            }
            return true;
        }

        let abilities = monster_abilities(def);
        let ready = |g: &Game, a: &AbilityDef| {
            let e = &g.ents[&id];
            !e.stats.silenced && now >= e.cooldowns.get(&a.key).copied().unwrap_or(0.0) && e.mp >= a.mana
        };
        // healers mend wounded allies first
        for a in &abilities {
            if a.kind == "heal" && ready(self, a) && self.wounded_ally(id, a.radius.max(1) as f32) {
                self.stop(id);
                self.use_ability(id, &a.key, None);
                return true;
            }
        }
        // war cries and blessings when the fight starts
        for a in &abilities {
            if a.kind == "buff" && ready(self, a) && now - self.ms(id).buffed_at > 12000.0 {
                self.ms(id).buffed_at = now;
                self.use_ability(id, &a.key, None);
                if a.buff.is_some() {
                    self.say(id, "В бой!", 1500.0);
                }
                return true;
            }
        }
        // stomps and auras when enemies are close
        for a in &abilities {
            if a.kind == "nova" && ready(self, a) && dist <= a.radius.max(1) as f32 + 0.5 && self.chance(45.0) {
                self.use_ability(id, &a.key, Some(target));
                return true;
            }
        }
        let attack = abilities.iter().copied().find(|a| matches!(a.kind.as_str(), "projectile" | "chain" | "strike") && ready(self, a));
        let level_ref = &self.levels[&level];
        let in_range = match attack {
            Some(a) if a.kind == "strike" => self.in_reach(id, target),
            Some(a) => dist <= ability_range(a) + 0.5 && los(level_ref, self.ents[&id].cell(), self.ents[&target].cell()),
            None => false,
        };
        if tactic == "use_ability" && attack.is_some() && in_range {
            self.stop(id);
            self.use_ability(id, &attack.unwrap().key, Some(target));
            return true;
        }
        if tactic == "defensive" && role != "support" {
            role = "ranged".into();
        }
        if tactic == "aggressive" && (role == "ranged" || role == "caster") && dist <= 2.0 {
            role = "frontline".into();
        }

        match role.as_str() {
            "ranged" | "caster" | "support" => self.keep_distance(id, target, attack, in_range),
            "skirmisher" => self.skirmish(id, target, attack, in_range),
            _ => {
                if self.in_reach(id, target) {
                    self.stop(id);
                    if let (Some(a), true, true) = (attack, in_range, attack.map(|a| a.kind == "strike").unwrap_or(false)) {
                        if self.chance(50.0) {
                            self.use_ability(id, &a.key, Some(target));
                            return true;
                        }
                    }
                    if now >= self.ents[&id].next_attack {
                        self.melee_attack(id, target);
                    } else {
                        self.ents.get_mut(&id).unwrap().face_to(tpos);
                    }
                    return true;
                }
                if let Some(a) = attack {
                    if in_range && a.kind != "strike" && dist > 2.5 && self.chance(30.0) {
                        self.stop(id);
                        self.use_ability(id, &a.key, Some(target));
                        return true;
                    }
                }
                let goal = if tactic == "flank" && dist > 2.5 { self.flank_goal(id, target) } else { tpos };
                self.step_toward(id, goal);
            }
        }
        true
    }

    /// Archers and casters stay a few steps away, step back from melee and
    /// shoot when they can.
    pub(crate) fn keep_distance(&mut self, id: Id, t: Id, attack: Option<&AbilityDef>, in_range: bool) {
        let now = self.now;
        let (e, te) = (&self.ents[&id], &self.ents[&t]);
        let dist = e.dist(te);
        let tpos = te.pos;
        let min_d = 3.0;
        let mut max_d = 5.0f32;
        if let Some(a) = attack.filter(|a| a.kind != "strike") {
            max_d = (ability_range(a) - 1.0).clamp(min_d, 7.0);
        } else if let Some(a) = monster_ability(self.def_of(id)) {
            max_d = (ability_range(a) - 1.0).clamp(min_d, 7.0);
        }
        if dist < min_d {
            if self.step_away(id, tpos) {
                return;
            }
            if self.in_reach(id, t) && now >= self.ents[&id].next_attack {
                self.melee_attack(id, t);
            }
            return;
        }
        if let (Some(a), true) = (attack, in_range) {
            self.stop(id);
            self.use_ability(id, &a.key, Some(t));
            return;
        }
        let l = &self.levels[&e.level];
        let sight = los(l, e.cell(), te.cell());
        if dist > max_d || !sight {
            self.step_toward(id, tpos);
            return;
        }
        if now >= self.ms(id).strafe_until && self.chance(15.0) {
            self.strafe(id, t, min_d, max_d);
        }
    }

    /// Gets behind the enemy, strikes and slips away when hurt.
    pub(crate) fn skirmish(&mut self, id: Id, t: Id, attack: Option<&AbilityDef>, in_range: bool) {
        let now = self.now;
        let e = &self.ents[&id];
        if e.hp / e.max_hp < 0.35 && now - e.monster.as_ref().unwrap().retreat_at > 9000.0 && e.faction == Faction::Monster {
            let ms = self.ms(id);
            ms.retreat_at = now;
            ms.flee_until = now + 1800.0;
            return;
        }
        let dist = e.dist(&self.ents[&t]);
        if self.in_reach(id, t) {
            self.stop(id);
            if let Some(a) = attack {
                if a.kind == "strike" && in_range && self.chance(50.0) {
                    self.use_ability(id, &a.key, Some(t));
                    return;
                }
            }
            if now >= self.ents[&id].next_attack {
                self.melee_attack(id, t);
            }
            return;
        }
        if let Some(a) = attack {
            if in_range && a.kind == "projectile" && dist > 2.5 && self.chance(25.0) {
                self.stop(id);
                self.use_ability(id, &a.key, Some(t));
                return;
            }
        }
        let g = self.flank_goal(id, t);
        self.step_toward(id, g);
    }

    /// A point next to the target, preferably behind it.
    pub(crate) fn flank_goal(&self, id: Id, t: Id) -> Vec2 {
        let te = &self.ents[&t];
        let e = &self.ents[&id];
        let mut behind = te.facing_vec() * -1.0;
        if let Some(m) = &te.monster {
            if let Some(tt) = self.ents.get(&m.target) {
                behind = (te.pos - tt.pos).norm();
            }
        }
        let d = te.radius() + e.radius() + 0.25;
        let l = &self.levels[&te.level];
        let blocked = |x: i32, y: i32| !l.walkable(x, y);
        for turn in [0.0f32, 0.9, -0.9, 1.6, -1.6] {
            let dir = Vec2::from_angle(behind.angle() + turn);
            let p = te.pos + dir * d;
            if l.circle_fits(p, e.radius(), &blocked) {
                return p;
            }
        }
        te.pos
    }

    /// Steps sideways while staying in the wanted distance band and in sight.
    pub(crate) fn strafe(&mut self, id: Id, t: Id, min_d: f32, max_d: f32) {
        let now = self.now;
        let (e, te) = (&self.ents[&id], &self.ents[&t]);
        let l = &self.levels[&e.level];
        let base = (e.pos - te.pos).angle();
        let side = if self.rng.int_n(2) == 0 { 1.0 } else { -1.0 };
        for k in [0.5f32, 0.8, -0.5, -0.8] {
            let a = base + k * side;
            let d = e.dist(te).clamp(min_d, max_d);
            let p = te.pos + Vec2::from_angle(a) * d;
            let c = p.cell();
            if l.walkable(c.x, c.y) && l.def_at(c).damage == 0.0 && los(l, c, te.cell()) && self.straight_walk(&e.level, e.pos, p, e.radius()) {
                self.walk_to(id, p);
                self.ms(id).strafe_until = now + 900.0;
                return;
            }
        }
    }

    /// Whether some ally (or the creature itself) within r needs healing.
    pub(crate) fn wounded_ally(&self, id: Id, r: f32) -> bool {
        let e = &self.ents[&id];
        self.on_level(&e.level).into_iter().any(|o| {
            let oe = &self.ents[&o];
            oe.alive() && oe.blocks() && oe.max_hp > 0.0 && oe.hp / oe.max_hp < 0.65 && e.dist(oe) <= r + 0.5 && self.friendly(id, o)
        })
    }

    /// Keeps or picks the enemy to fight.
    pub(crate) fn choose_target(&mut self, id: Id, def: &MonsterDef) -> Option<Id> {
        let now = self.now;
        let tt = self.ents[&id].stats.taunter;
        if tt != 0 {
            if let Some(t) = self.ents.get(&tt) {
                if t.level == self.ents[&id].level && self.hostile(id, tt) {
                    let tp = t.pos;
                    let ms = self.ms(id);
                    ms.target = tt;
                    ms.last_seen = tp;
                    ms.last_seen_at = now;
                    return Some(tt);
                }
            }
        }
        if let Some(t) = self.valid_target(id, def) {
            if now >= self.ms(id).retarget {
                let wait = 1500.0 + self.rng.f64() * 1000.0;
                self.ms(id).retarget = now + wait;
                if let Some(b) = self.better_target(id, def, t) {
                    if b != t {
                        let bp = self.ents[&b].pos;
                        let ms = self.ms(id);
                        ms.target = b;
                        ms.last_seen = bp;
                        ms.last_seen_at = now;
                        return Some(b);
                    }
                }
            }
            return Some(t);
        }
        if self.ms(id).target != 0 {
            return None; // still chasing the last known position
        }
        self.acquire_target(id, def)
    }

    pub(crate) fn valid_target(&mut self, id: Id, def: &MonsterDef) -> Option<Id> {
        let now = self.now;
        let t = self.ms(id).target;
        if t == 0 {
            return None;
        }
        let ok = self.ents.get(&t).map(|te| te.level == self.ents[&id].level).unwrap_or(false) && self.hostile(id, t);
        if !ok {
            self.ms(id).target = 0;
            return None;
        }
        let (e, te) = (&self.ents[&id], &self.ents[&t]);
        let l = &self.levels[&e.level];
        let d = e.dist(te);
        if d <= (def.sight + 4) as f32 && los(l, e.cell(), te.cell()) && self.can_see(id, t) {
            let tp = te.pos;
            let ms = self.ms(id);
            ms.last_seen = tp;
            ms.last_seen_at = now;
            return Some(t);
        }
        // lost sight: chase the last known position for a while
        let stealthed = te.stats.stealthed;
        let ms = e.monster.as_ref().unwrap();
        if now - ms.last_seen_at > 5000.0 || d > (def.sight * 3) as f32 || stealthed {
            let ms = self.ms(id);
            ms.target = 0;
            ms.state = "return".into();
            ms.tactic.clear();
            return None;
        }
        let ls = ms.last_seen;
        if e.pos.dist(ls) < 0.5 {
            self.ms(id).last_seen_at -= 1000.0;
            return None;
        }
        self.step_toward(id, ls);
        None
    }

    /// Lower is better. Skirmishers and shooters hunt the weakest, the
    /// frontline takes whoever is closest.
    pub(crate) fn target_score(&self, id: Id, def: &MonsterDef, o: Id) -> f32 {
        let (e, oe) = (&self.ents[&id], &self.ents[&o]);
        let d = e.dist(oe);
        match role_of(def) {
            "skirmisher" | "ranged" | "caster" => d * 0.4 + 10.0 * (oe.hp / oe.max_hp.max(1.0)) as f32,
            _ => d,
        }
    }

    pub(crate) fn better_target(&self, id: Id, def: &MonsterDef, cur: Id) -> Option<Id> {
        let e = &self.ents[&id];
        let l = &self.levels[&e.level];
        let others = self.on_level(&e.level);
        match role_of(def) {
            "frontline" | "leader" => {
                // protect our archers and casters from enemies who reached them
                for &o in &others {
                    if o == cur || !self.hostile(id, o) || e.dist(&self.ents[&o]) > 6.0 {
                        continue;
                    }
                    for &a in &others {
                        let ae = &self.ents[&a];
                        if ae.monster.is_some() && a != id && self.friendly(id, a) && ae.gap(&self.ents[&o]) < 0.6 && squishy(self.def_of(a)) {
                            return Some(o);
                        }
                    }
                }
                None
            }
            "skirmisher" | "ranged" | "caster" => {
                let mut best = cur;
                let mut bs = self.target_score(id, def, cur) - 2.0;
                for &o in &others {
                    let oe = &self.ents[&o];
                    if !self.hostile(id, o) || e.dist(oe) > def.sight as f32 || !self.can_see(id, o) || !los(l, e.cell(), oe.cell()) {
                        continue;
                    }
                    let s = self.target_score(id, def, o);
                    if s < bs {
                        best = o;
                        bs = s;
                    }
                }
                Some(best)
            }
            _ => None,
        }
    }

    pub(crate) fn acquire_target(&mut self, id: Id, def: &MonsterDef) -> Option<Id> {
        let now = self.now;
        let e = &self.ents[&id];
        let l = &self.levels[&e.level];
        let mut sight = def.sight;
        if l.lit && self.is_night() && !def.night && e.faction == Faction::Monster {
            sight = (sight - 2).max(3);
        }
        let mut best = None;
        let mut bs = f32::MAX;
        for o in self.on_level(&e.level) {
            if !self.hostile(id, o) || !self.can_see(id, o) {
                continue;
            }
            let oe = &self.ents[&o];
            let d = e.dist(oe);
            if d > sight as f32 + 0.5 || (d > 2.5 && !los(l, e.cell(), oe.cell())) {
                continue;
            }
            let s = self.target_score(id, def, o);
            if s < bs {
                best = Some(o);
                bs = s;
            }
        }
        let best = best?;
        let bp = self.ents[&best].pos;
        let (my_pos, squad, faction) = (e.pos, e.monster.as_ref().unwrap().squad, e.faction);
        let ms = self.ms(id);
        ms.target = best;
        ms.last_seen = bp;
        ms.last_seen_at = now;
        ms.state = "chase".into();
        ms.path.clear();
        // packs and squads alert each other
        if faction == Faction::Monster {
            let level = self.ents[&id].level.clone();
            for o in self.on_level(&level) {
                if o == id || !self.friendly(id, o) {
                    continue;
                }
                let oe = &self.ents[&o];
                let Some(om) = &oe.monster else { continue };
                if om.target != 0 || oe.owner != 0 {
                    continue;
                }
                let d = oe.pos.dist(my_pos);
                let near = d <= 6.0 || (squad != 0 && om.squad == squad && d <= 14.0);
                if near {
                    let om = self.ms(o);
                    om.target = best;
                    om.last_seen = bp;
                    om.last_seen_at = now;
                    om.state = "chase".into();
                }
            }
        }
        Some(best)
    }

    /// Summons follow their owner, monsters roam near home.
    pub(crate) fn idle(&mut self, id: Id) {
        let e = &self.ents[&id];
        if e.owner != 0 {
            if let Some(o) = self.ents.get(&e.owner) {
                if e.dist(o) > 2.2 {
                    let op = o.pos;
                    self.step_toward(id, op);
                } else {
                    self.stop(id);
                }
            }
            return;
        }
        if e.npc.is_some() {
            return;
        }
        self.monster_idle(id);
    }

    pub(crate) fn monster_idle(&mut self, id: Id) {
        let e = self.ents.get_mut(&id).unwrap();
        if e.hp < e.max_hp {
            e.hp = (e.hp + e.max_hp * 0.002).min(e.max_hp);
        }
        let ms = e.monster.as_ref().unwrap();
        let home = ms.home;
        if ms.state == "return" || ms.state == "fled" {
            if e.pos.dist(home) > 2.0 {
                self.step_toward(id, home);
                return;
            }
            self.ms(id).state = "idle".into();
        }
        let e = &self.ents[&id];
        if e.goal.is_none() && self.rng.chance(6.0) {
            let a = self.rng.f32() * std::f32::consts::TAU;
            let d = 1.0 + self.rng.f32() * 2.5;
            let to = e.pos + Vec2::from_angle(a) * d;
            let l = &self.levels[&e.level];
            let c = to.cell();
            let def = l.def_at(c);
            if l.walkable(c.x, c.y) && to.dist(home) <= 6.0 && def.damage == 0.0 && def.interact.is_empty() && self.straight_walk(&e.level, e.pos, to, e.radius()) {
                self.walk_to(id, to);
            }
        }
    }

    // ---- LLM tactics for elite enemies ----

    pub(crate) fn maybe_ask_tactic(&mut self, id: Id, def: &MonsterDef, target: Id) {
        let now = self.now;
        let enabled = self.brain.as_ref().map(|b| b.enabled()).unwrap_or(false);
        let e = &self.ents[&id];
        let ms = e.monster.as_ref().unwrap();
        if !def.elite || !enabled || ms.llm_pending || now < ms.next_llm {
            return;
        }
        let mut allies: Vec<String> = self
            .on_level(&e.level)
            .into_iter()
            .filter(|&o| o != id)
            .filter_map(|o| self.ents.get(&o))
            .filter(|o| o.monster.is_some() && o.faction == e.faction && o.dist(e) <= 10.0)
            .map(|o| o.name.clone())
            .collect();
        if allies.len() > 8 {
            allies.truncate(8);
            allies.push("...".into());
        }
        let te = &self.ents[&target];
        let mut req = TacticRequest {
            name: e.name.clone(),
            persona: def.persona.clone(),
            boss: def.boss,
            hp_pct: (100.0 * e.hp / e.max_hp) as i32,
            allies,
            enemy: te.name.clone(),
            enemy_hp_pct: (100.0 * te.hp / te.max_hp.max(1.0)) as i32,
            distance: e.dist(te).round() as i32,
            events: ms.events.clone(),
            ..Default::default()
        };
        if let Some(p) = &te.player {
            req.enemy_level = p.level;
            req.lang = p.lang.clone();
            if let Some(c) = db().class(&p.class) {
                req.enemy_class = c.name.clone();
            }
        }
        if let Some(a) = monster_ability(def) {
            req.ability_name = a.name.clone();
        }
        let ms = self.ms(id);
        ms.events.clear();
        ms.next_llm = now + 7000.0;
        let tx = self.task_sender();
        let brain = self.brain.as_ref().unwrap();
        let sent = brain.tactic(
            req,
            Box::new(move |r: Result<TacticReply, String>| {
                let _ = tx.send(Box::new(move |g: &mut Game| {
                    let Some(m) = g.ents.get_mut(&id).and_then(|e| e.monster.as_mut()) else { return };
                    m.llm_pending = false;
                    if let Ok(r) = r {
                        g.apply_tactic(id, r);
                    }
                }));
            }),
        );
        if sent {
            self.ms(id).llm_pending = true;
        }
    }

    pub(crate) fn apply_tactic(&mut self, id: Id, r: TacticReply) {
        if !TACTICS.contains(&r.tactic.as_str()) {
            return;
        }
        let now = self.now;
        let ms = self.ms(id);
        ms.tactic = r.tactic.clone();
        ms.tactic_until = now + 8000.0;
        let (target, last_seen) = (ms.target, ms.last_seen);
        let e = &self.ents[&id];
        let (level, pos, name, faction) = (e.level.clone(), e.pos, e.name.clone(), e.faction);
        let mut say = r.say.trim().to_string();
        if !say.is_empty() {
            if say.chars().count() > 90 {
                say = say.chars().take(90).collect::<String>() + "…";
            }
            self.say(id, &say, 4500.0);
            for p in self.players_on(&level) {
                if self.ents[&p].pos.dist(pos) <= 18.0 {
                    self.log(p, "#ff9a6a", format!("{name}: «{say}»"));
                }
            }
        }
        if r.tactic == "call_allies" {
            for o in self.on_level(&level) {
                let Some(oe) = self.ents.get(&o) else { continue };
                if o == id || oe.monster.is_none() || oe.faction != faction || oe.pos.dist(pos) > 16.0 {
                    continue;
                }
                let op = oe.pos;
                let om = self.ms(o);
                om.target = target;
                om.last_seen = last_seen;
                om.last_seen_at = now;
                om.state = "chase".into();
                self.fx(&level, op, "!", '\0', "#ff4a4a", 800);
            }
        }
    }

    // ---- NPCs ----

    pub(crate) fn update_npc(&mut self, id: Id) {
        let now = self.now;
        let talking_to: Option<Vec2> = self.online.values().filter_map(|p| self.ents.get(p)).find(|p| p.p().talking == id).map(|p| p.pos);
        if let Some(pp) = talking_to {
            let e = self.ents.get_mut(&id).unwrap();
            e.face_to(pp);
            e.goal = None;
        }
        let talking = talking_to.is_some();
        if self.ents[&id].monster.is_some() && !talking && self.update_monster(id) {
            return; // fighting
        }
        let Some(e) = self.ents.get(&id) else { return };
        let n = e.npc.as_ref().unwrap();
        let role = db().npc_role(&n.role);
        if role.map(|r| r.world).unwrap_or(false) && !talking {
            self.travel(id);
        }
        let n = self.ents[&id].npc.as_ref().unwrap();
        if now < n.next_think || talking {
            return;
        }
        let wait = 1200.0 + self.rng.f64() * 2500.0;
        let e = self.ents.get_mut(&id).unwrap();
        e.npc.as_mut().unwrap().next_think = now + wait;
        let Some(role) = role else { return };
        let n = e.npc.as_mut().unwrap();
        if n.next_chat == 0.0 {
            n.next_chat = now + 5000.0 + self.rng.f64() * 40000.0;
        }
        let (level, pos) = (e.level.clone(), e.pos);
        if now >= self.ents[&id].npc.as_ref().unwrap().next_chat && !role.lines.is_empty() {
            let near = self.players_on(&level).into_iter().any(|p| self.ents[&p].pos.dist(pos) <= 6.0);
            if near {
                let line = self.rng.pick(&role.lines).clone();
                self.say(id, &line, 4000.0);
                let next = now + 30000.0 + self.rng.f64() * 30000.0;
                self.ents.get_mut(&id).unwrap().npc.as_mut().unwrap().next_chat = next;
            }
        }
        if role.world || role.wander <= 0 || self.ents[&id].goal.is_some() {
            return;
        }
        // citizens keep a schedule: back to their spot when far from it
        let spot = self.schedule_spot(id);
        let pos = self.ents[&id].pos;
        if pos.dist(spot) > role.wander as f32 + 1.0 {
            self.ents.get_mut(&id).unwrap().npc.as_mut().unwrap().next_think = now + 300.0;
            if self.walk_toward(id, spot) {
                return;
            }
        }
        if !self.chance(55.0) {
            return;
        }
        let a = self.rng.f32() * std::f32::consts::TAU;
        let d = 0.8 + self.rng.f32() * 1.5;
        let to = pos + Vec2::from_angle(a) * d;
        let e = &self.ents[&id];
        let l = &self.levels[&e.level];
        let c = to.cell();
        let def = l.def_at(c);
        if l.walkable(c.x, c.y) && def.interact.is_empty() && to.dist(spot) <= role.wander as f32 + 0.5 && self.straight_walk(&e.level, pos, to, e.radius()) {
            let e = self.ents.get_mut(&id).unwrap();
            e.pace = 0.55;
            e.goal = Some(to);
        }
    }
}
