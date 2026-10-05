//! Attacks, abilities, projectiles in flight, loot and buffs.

use super::*;
use crate::content::{AbilityDef, BuffDef};
use crate::proto::{FX_ALLY, FX_AREA, FX_BEAM, FX_CAST, FX_HIT, FX_IMPACT};
use crate::world::{los, Vec2, DIRS8};

/// The gap between bodies a melee blow reaches across (tiles).
pub const MELEE_GAP: f32 = 0.42;

/// The basic attack with a bow or crossbow: weapon damage.
pub(crate) fn bow_shot() -> AbilityDef {
    AbilityDef {
        key: "bow_shot".into(),
        name: "Выстрел".into(),
        kind: "projectile".into(),
        range: 9,
        speed_ms: 35,
        glyph: "dir".into(),
        color: "#e0d0a0".into(),
        dmg_type: "weapon".into(),
        ..Default::default()
    }
}

/// The basic melee attack, for aiming.
pub(crate) fn melee_swing() -> AbilityDef {
    AbilityDef {
        key: "melee".into(),
        kind: "strike".into(),
        ..Default::default()
    }
}

pub(crate) fn ability_range(a: &AbilityDef) -> f32 {
    if a.range > 0 {
        a.range as f32
    } else if a.kind == "projectile" {
        8.0
    } else {
        1.0
    }
}

pub(crate) fn glyph_of(a: &AbilityDef, def: char) -> char {
    if a.glyph.is_empty() || a.glyph == "dir" {
        def
    } else {
        a.glyph.chars().next().unwrap_or(def)
    }
}

impl Game {
    pub(crate) fn kill(&mut self, id: Id, killer: Option<Id>) {
        let Some(e) = self.ents.get(&id) else { return };
        match (e.kind, e.faction) {
            (Kind::Player, _) => self.kill_player(id, killer),
            (Kind::Monster, Faction::Monster) => self.kill_monster(id, killer),
            _ => self.kill_ally(id),
        }
    }

    pub(crate) fn kill_monster(&mut self, id: Id, killer: Option<Id>) {
        let Some(m) = self.remove(id) else { return };
        let ms = m.monster.as_ref().unwrap();
        let def = db().monster(&ms.def);
        let (level, pos, cell) = (m.level.clone(), m.pos, m.cell());
        self.fx(&level, pos, "", '%', "#a03030", 1200);
        let killer = killer.and_then(|k| self.controller(k));
        let is_boss = def.is_some_and(|d| d.boss);
        let is_elite = def.is_some_and(|d| d.elite);
        if let Some(def) = def {
            let mut gold =
                self.roll(def.gold[0] as f64, (def.gold[1] + 1) as f64) * monster_scale(ms.lvl);
            if let Some(k) = killer.and_then(|k| self.ents.get(&k)) {
                if k.player.is_some() {
                    gold *= 1.0 + k.stats.gold_find / 100.0;
                }
            }
            self.drop_gold(&level, cell, gold as i32);
            let depth = ms.lvl.max(1);
            let item_chance = if is_boss {
                100.0
            } else if is_elite {
                60.0
            } else {
                10.0
            };
            if self.chance(item_chance) {
                if let Some(st) = self.random_item(depth, 30.0) {
                    self.drop_item(&level, cell, st);
                }
            }
            for key in &def.drops {
                self.drop_item(&level, cell, ItemStack::new(key));
            }
            for it in &db().b.items {
                let chance = if it.drop_chance <= 0.0 {
                    35.0
                } else {
                    it.drop_chance
                };
                if it.drop_from.contains(&def.key) && self.chance(chance) {
                    self.drop_item(&level, cell, ItemStack::new(&it.key));
                    self.fx(
                        &level,
                        pos,
                        &format!("{}!", it.name),
                        '\0',
                        rarity_color(def_rarity(Some(it))),
                        2000,
                    );
                }
            }
            if is_boss {
                if let Some(st) = self.random_item_at_least(depth + 2, 100.0, RARE) {
                    self.drop_item(&level, cell, st);
                }
            }
        }
        let mut heroes = Vec::new();
        let kc = killer;
        for p in self.players_on(&level) {
            let pe = &self.ents[&p];
            if !pe.alive() {
                continue;
            }
            let d = pe.pos.dist(pos);
            let party_near = kc.is_some_and(|k| self.same_party(p, k) && d <= 50.0);
            if d > 25.0 && !party_near {
                continue;
            }
            heroes.push(pe.name.clone());
            self.give_xp(p, ms.xp);
            self.ents.get_mut(&p).unwrap().pm().kills += 1;
            self.kill_deeds(p, &m, def, Some(p) == kc);
            if Some(p) == kc {
                self.log(
                    p,
                    "#e0e0e0",
                    format!("Вы убили: {} (+{} опыта).", m.name, ms.xp),
                );
            } else {
                self.log(
                    p,
                    "#c0c0c0",
                    format!("{} повержен (+{} опыта).", m.name, ms.xp),
                );
            }
            self.quest_progress(p, &ms.def);
            self.champion_slain(p, &m);
            self.relic_drop(p, &m);
            if is_boss {
                let pm = self.ents.get_mut(&p).unwrap().pm();
                if !pm.bosses.contains(&ms.def) {
                    pm.bosses.push(ms.def.clone());
                }
            }
        }
        if is_boss {
            heroes.sort();
            self.log_all(
                "#ff4aff",
                format!(
                    "*** {} повержен! Герои: {} ***",
                    m.name,
                    format!("[{}]", heroes.join(" "))
                ),
            );
            self.chronicle(format!("{} сразил(и) {}", heroes.join(", "), m.name));
        }
        if !ms.champion.is_empty() && !heroes.is_empty() {
            self.chronicle(format!(
                "{} одолел(и) чемпиона «{}»",
                heroes.join(", "),
                m.name
            ));
        }
        if def.is_some_and(|d| d.role == "leader") && ms.squad != 0 {
            // the squad loses heart when its leader falls
            for o in self.on_level(&level) {
                let same = self
                    .ents
                    .get(&o)
                    .and_then(|oe| oe.monster.as_ref())
                    .is_some_and(|om| om.squad == ms.squad);
                if same && self.chance(40.0) {
                    let now = self.now;
                    self.ents
                        .get_mut(&o)
                        .unwrap()
                        .monster
                        .as_mut()
                        .unwrap()
                        .flee_until = now + 4000.0;
                    self.say(o, "Бежим!", 1500.0);
                }
            }
        }
        // let nearby elites know
        for o in self.near(&level, pos, 12.0) {
            if let Some(oe) = self.ents.get_mut(&o) {
                if oe.faction == Faction::Monster && oe.pos.dist(pos) < 12.0 {
                    if let Some(om) = oe.monster.as_mut() {
                        om.events
                            .push(format!("your ally {} was just killed", m.name));
                    }
                }
            }
        }
    }

    pub(crate) fn quest_progress(&mut self, p: Id, monster: &str) {
        let mut done_msgs = Vec::new();
        let pm = self.ents.get_mut(&p).unwrap().pm();
        for q in pm.quests.iter_mut() {
            if q.done || q.monster != monster || (!q.kind.is_empty() && q.kind != "boss") {
                continue;
            }
            q.have += 1;
            if q.have >= q.need {
                q.done = true;
                done_msgs.push(format!(
                    "Задание выполнено! Вернитесь к: {} ({}).",
                    q.giver, q.village
                ));
            }
            pm.dirty = true;
        }
        for m in done_msgs {
            self.log(p, "#ffd24a", m);
        }
    }

    pub(crate) fn melee_attack(&mut self, a: Id, d: Id) {
        let now = self.now;
        let dpos = self.ents[&d].pos;
        let e = self.ents.get_mut(&a).unwrap();
        e.next_attack = now + e.stats.attack_ms;
        e.face_to(dpos);
        e.swings = e.swings.wrapping_add(1);
        let mut dmg = self.melee_damage(a);
        self.roll_crit(a, &mut dmg);
        let dealt = self.damage(Some(a), d, dmg);
        self.weapon_hit(a, d, dealt);
    }

    /// The farthest (centre to centre) a melee blow of e can reach.
    pub(crate) fn melee_reach(&self, e: Id) -> f32 {
        let ee = &self.ents[&e];
        ee.radius() + BOSS_RADIUS + MELEE_GAP + ee.stats.reach as f32
    }

    /// Melee attack from e reaches o: bodies close enough; with a long reach
    /// further, along a line not blocked by walls.
    pub(crate) fn in_reach(&self, e: Id, o: Id) -> bool {
        let (ee, oe) = (&self.ents[&e], &self.ents[&o]);
        if ee.level != oe.level {
            return false;
        }
        let gap = ee.gap(oe);
        if gap <= MELEE_GAP {
            return true;
        }
        let reach = ee.stats.reach as f32;
        if reach <= 0.0 || gap > MELEE_GAP + reach {
            return false;
        }
        los(&self.levels[&ee.level], ee.cell(), oe.cell())
    }

    /// Attacks the enemy aimed at, or the one in reach the hero faces; with a
    /// bow or crossbow it shoots at the aim or the nearest enemy in sight.
    pub(crate) fn attack_facing(&mut self, id: Id) {
        let now = self.now;
        let aim = self.aimed(id);
        if let Some(a) = aim {
            self.ents.get_mut(&id).unwrap().face_to(a);
        }
        if self.ents[&id].stats.gear.ranged {
            let bs = bow_shot();
            let t = match aim {
                Some(a) => self.aim_target(id, &bs, a),
                None => self.auto_target(id, &bs),
            };
            if t.is_some() || aim.is_some() {
                let e = self.ents.get_mut(&id).unwrap();
                e.next_attack = now + e.stats.attack_ms * 1.3;
                e.swings = e.swings.wrapping_add(1);
                cast_projectile(self, id, &bs, t);
                return;
            }
        }
        if let Some(a) = aim {
            if let Some(o) = self.aim_target(id, &melee_swing(), a) {
                self.melee_attack(id, o);
                return;
            }
        }
        // the enemy in reach closest to where the hero looks
        let e = &self.ents[&id];
        let fv = e.facing_vec();
        let mut best: Option<(Id, f32)> = None;
        for o in self.near(&e.level, e.pos, self.melee_reach(id)) {
            if !self.hostile(id, o) || !self.can_see(id, o) || !self.in_reach(id, o) {
                continue;
            }
            let oe = &self.ents[&o];
            let score = -(oe.pos - e.pos).norm().dot(fv) + oe.dist(e) * 0.1;
            if best.is_none_or(|b| score < b.1) {
                best = Some((o, score));
            }
        }
        match best {
            Some((o, _)) => self.melee_attack(id, o),
            None => {
                // a swing at the air
                let e = self.ents.get_mut(&id).unwrap();
                e.next_attack = now + e.stats.attack_ms * 0.5;
                e.swings = e.swings.wrapping_add(1);
            }
        }
    }

    // ---- loot ----

    /// A walkable cell for loot at p (or next to it, toward a point).
    pub(crate) fn item_spot(&self, level: &str, p: Pos, toward: Vec2) -> Pos {
        let l = &self.levels[level];
        if l.walkable(p.x, p.y) {
            return p;
        }
        let mut best = p;
        let mut bd = f32::MAX;
        for d in DIRS8 {
            let q = p.add(d);
            if l.walkable(q.x, q.y) {
                let dd = q.center().dist(toward);
                if dd < bd {
                    best = q;
                    bd = dd;
                }
            }
        }
        best
    }

    pub(crate) fn drop_item(&mut self, level: &str, p: Pos, mut st: ItemStack) {
        let Some(d) = st.def() else { return };
        if st.qty <= 0 {
            st.qty = 1;
        }
        let cell = self.item_spot(level, p, p.center());
        let jitter = Vec2::new(self.rng.f32() - 0.5, self.rng.f32() - 0.5) * 0.5;
        let e = Entity {
            kind: Kind::Item,
            name: st.name(),
            glyph: d.glyph.clone(),
            color: d.color.clone(),
            level: level.into(),
            pos: cell.center() + jitter,
            item: Some(st),
            ..Default::default()
        };
        self.spawn(e);
    }

    pub(crate) fn drop_gold(&mut self, level: &str, p: Pos, n: i32) {
        if n > 0 {
            self.drop_item(level, p, ItemStack::qty("gold", n));
        }
    }

    /// A weighted random item for a depth; equipment is better than common
    /// with magic_chance percent (see roll_rarity_tier).
    pub(crate) fn random_item(&mut self, depth: i32, magic_chance: f64) -> Option<ItemStack> {
        self.random_item_at_least(depth, magic_chance, COMMON)
    }

    /// random_item with a lowest rarity for equipment.
    pub(crate) fn random_item_at_least(
        &mut self,
        depth: i32,
        magic_chance: f64,
        least: i32,
    ) -> Option<ItemStack> {
        let items = &db().b.items;
        let total: i32 = items
            .iter()
            .filter(|it| it.weight > 0 && it.depth <= depth)
            .map(|it| it.weight)
            .sum();
        if total == 0 {
            return None;
        }
        let mut n = self.rng.int_n(total);
        for it in items {
            if it.weight <= 0 || it.depth > depth {
                continue;
            }
            n -= it.weight;
            if n >= 0 {
                continue;
            }
            let mut st = ItemStack::new(&it.key);
            if !slot_for(Some(it)).is_empty() && !it.unique {
                let r = self.roll_rarity_tier(depth, magic_chance, least);
                st = self.roll_rarity(st, r, depth);
            }
            return Some(st);
        }
        None
    }

    // ---- abilities ----

    pub(crate) fn use_hotbar(&mut self, id: Id, slot: i32) -> bool {
        if !(1..=HOTBAR_SIZE as i32).contains(&slot) {
            return true;
        }
        let key = self.ents[&id].p().hotbar[slot as usize - 1].clone();
        if key.is_empty() {
            self.log(
                id,
                "#808080",
                format!("Слот {slot} пуст. Назначьте умение в окне навыков (K)."),
            );
            return true;
        }
        self.use_ability(id, &key, None)
    }

    /// Returns true when the request is finished (cast or failed) and false
    /// when it should wait (cooldown).
    pub(crate) fn use_ability(&mut self, c: Id, key: &str, target: Option<Id>) -> bool {
        let Some(a) = db().ability(key) else {
            return true;
        };
        let now = self.now;
        let Some(ce) = self.ents.get(&c) else {
            return true;
        };
        if !ce.alive() {
            return true;
        }
        let free = ce.player.as_ref().is_some_and(|p| p.no_cd);
        if now < ce.cooldowns.get(key).copied().unwrap_or(0.0) && !free {
            return false;
        }
        if ce.mp < a.mana && !free {
            self.log(c, "#6090ff", format!("Недостаточно маны для «{}».", a.name));
            return true;
        }
        if ce.stats.silenced {
            self.log(
                c,
                "#e0c8ff",
                "Вы под печатью и не можете применять умения.".into(),
            );
            return true;
        }
        if ce.player.is_some() && !ce.stats.gear.has(&a.equip) {
            self.log(
                c,
                "#ff8080",
                format!("Для «{}» нужно: {}.", a.name, gear_need_name(&a.equip)),
            );
            return true;
        }
        let mut target = target;
        if target.is_none() {
            if let Some(aim) = self.aimed(c) {
                self.ents.get_mut(&c).unwrap().face_to(aim);
                target = self.aim_target(c, a, aim);
                if target.is_none() && a.kind != "projectile" {
                    target = self.auto_target(c, a);
                }
            }
        }
        if target.is_none() {
            target = self.auto_target(c, a);
        }
        if !cast(self, c, a, target) {
            return true;
        }
        if !free {
            if let Some(ce) = self.ents.get_mut(&c) {
                ce.mp -= a.mana;
                ce.cooldowns.insert(key.into(), now + a.cooldown_ms as f64);
            }
        }
        self.deed(c, "casts", 1);
        if a.kind == "summon" {
            self.deed(c, "summons", 1);
        }
        true
    }

    /// The point a player aims the current action at with the mouse.
    pub(crate) fn aimed(&self, id: Id) -> Option<Vec2> {
        self.ents
            .get(&id)
            .and_then(|e| e.player.as_ref())
            .and_then(|p| p.aim)
    }

    /// The enemy at the aimed point (or right next to it) that the ability
    /// can reach.
    pub(crate) fn aim_target(&self, c: Id, a: &AbilityDef, aim: Vec2) -> Option<Id> {
        let rng = ability_range(a);
        let ce = &self.ents[&c];
        let l = &self.levels[&ce.level];
        let mut best = None;
        let mut bd = f32::MAX;
        for o in self.near(&ce.level, aim, 1.0 + BOSS_RADIUS) {
            if !self.hostile(c, o) || !self.can_see(c, o) {
                continue;
            }
            let oe = &self.ents[&o];
            let d = oe.pos.dist(aim);
            if d > 1.0 + oe.radius() {
                continue;
            }
            if rng <= 1.0 {
                if !self.in_reach(c, o) {
                    continue;
                }
            } else if ce.dist(oe) > rng + 0.5 || !los(l, ce.cell(), oe.cell()) {
                continue;
            }
            if d < bd {
                best = Some(o);
                bd = d;
            }
        }
        best
    }

    /// The nearest visible enemy in range.
    pub(crate) fn auto_target(&self, c: Id, a: &AbilityDef) -> Option<Id> {
        let ce = &self.ents[&c];
        if let Some(m) = &ce.monster {
            if self.ents.contains_key(&m.target) {
                return Some(m.target);
            }
        }
        let rng = ability_range(a);
        let l = &self.levels[&ce.level];
        let mut best = None;
        let mut bd = f32::MAX;
        let reach = if rng <= 1.0 {
            self.melee_reach(c)
        } else {
            rng + 0.5
        };
        for o in self.near(&ce.level, ce.pos, reach) {
            if !self.hostile(c, o) || !self.can_see(c, o) {
                continue;
            }
            let oe = &self.ents[&o];
            let d = ce.dist(oe);
            if d >= bd {
                continue;
            }
            if rng <= 1.0 {
                if !self.in_reach(c, o) {
                    continue;
                }
            } else if d > rng + 0.5 || !los(l, ce.cell(), oe.cell()) {
                continue;
            }
            best = Some(o);
            bd = d;
        }
        best
    }

    pub(crate) fn update_projectile(&mut self, id: Id) {
        let dt = (TICK_MS / 1000.0) as f32;
        let Some(p) = self.ents.get(&id) else { return };
        let ps = p.proj.as_ref().unwrap();
        let level = p.level.clone();
        let mut pos = p.pos;
        let vel = ps.vel;
        let mut left = ps.left;
        let mut travel = (vel.len() * dt).min(left);
        let l = &self.levels[&level];
        while travel > 0.0 {
            let step = travel.min(0.25);
            travel -= step;
            left -= step;
            let next = pos + vel.norm() * step;
            let c = next.cell();
            if !l.transparent(c.x, c.y) {
                self.projectile_impact(id, pos, None);
                return;
            }
            pos = next;
            // a body in the way
            let mut hit = None;
            for o in self.near(&level, pos, BOSS_RADIUS + 0.15) {
                let oe = &self.ents[&o];
                if !oe.blocks() || !oe.alive() || oe.pos.dist(pos) > oe.radius() + 0.15 {
                    continue;
                }
                if self.projectile_hits(id, o) {
                    hit = Some(o);
                    break;
                }
            }
            if let Some(o) = hit {
                self.ents.get_mut(&id).unwrap().pos = pos;
                self.projectile_impact(id, pos, Some(o));
                return;
            }
        }
        let p = self.ents.get_mut(&id).unwrap();
        p.pos = pos;
        p.vel = vel;
        p.proj.as_mut().unwrap().left = left;
        if left <= 0.0 {
            self.projectile_impact(id, pos, None);
        }
    }

    /// Whether a projectile hurts o.
    pub(crate) fn projectile_hits(&self, pid: Id, o: Id) -> bool {
        let ps = self.ents[&pid].proj.as_ref().unwrap();
        if o == ps.owner {
            return false;
        }
        if self.ents.contains_key(&ps.owner) {
            return self.hostile(ps.owner, o);
        }
        let oe = &self.ents[&o];
        oe.alive() && hostile_factions(ps.faction, oe.faction)
    }

    pub(crate) fn projectile_impact(&mut self, id: Id, at: Vec2, hit: Option<Id>) {
        let Some(p) = self.remove(id) else { return };
        let ps = p.proj.unwrap();
        let owner = Some(ps.owner).filter(|o| self.ents.contains_key(o));
        let fallback;
        let a = match db().ability(&ps.ability) {
            Some(a) => a,
            None => {
                fallback = AbilityDef {
                    key: ps.ability.clone(),
                    color: p.color.clone(),
                    ..Default::default()
                };
                &fallback
            }
        };
        let from = at - ps.vel.norm();
        if ps.radius > 0 {
            let r = ps.radius as f32;
            self.fx_spell(&p.level, a, FX_IMPACT, at, from, r + 0.5);
            self.area_damage(
                owner,
                ps.faction,
                &p.level,
                at,
                r,
                &ps.damage,
                ps.on_hit.as_ref(),
            );
            return;
        }
        if let Some(h) = hit {
            self.fx_spell(&p.level, a, FX_IMPACT, at, from, 0.0);
            self.damage(owner, h, ps.damage.clone());
            if let Some(b) = &ps.on_hit {
                self.apply_buff(h, b, ps.owner);
            }
        }
    }

    /// Hits every enemy of a faction within radius.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn area_damage(
        &mut self,
        src: Option<Id>,
        f: Faction,
        level: &str,
        at: Vec2,
        radius: f32,
        dmg: &Damage,
        on_hit: Option<&BuffDef>,
    ) {
        let mut victims = Vec::new();
        for o in self.near(level, at, radius + 0.5 + BOSS_RADIUS) {
            let oe = &self.ents[&o];
            if !oe.alive() || !oe.blocks() || oe.pos.dist(at) > radius + 0.5 + oe.radius() * 0.5 {
                continue;
            }
            let hostile = match src {
                Some(s) => self.hostile(s, o),
                None => hostile_factions(f, oe.faction),
            };
            if hostile {
                victims.push(o);
            }
        }
        for o in victims {
            if !dmg.parts.is_empty() {
                self.damage(src, o, dmg.clone());
            } else {
                self.provoke(src, o);
            }
            if let Some(b) = on_hit {
                self.apply_buff(o, b, src.unwrap_or(0));
            }
        }
    }

    // ---- buffs ----

    pub(crate) fn apply_buff(&mut self, id: Id, def: &BuffDef, src: Id) {
        let now = self.now;
        let Some(e) = self.ents.get_mut(&id) else {
            return;
        };
        if !e.alive() {
            return;
        }
        let until = now + def.duration_ms as f64;
        if let Some(b) = e.buffs.iter_mut().find(|b| b.def.key == def.key) {
            b.until = until;
            b.source = src;
            return;
        }
        e.buffs.push(Buff {
            def: def.clone(),
            until,
            source: src,
        });
        e.recalc();
        if let Some(p) = e.player.as_mut() {
            p.dirty = true;
        }
        if def.stun {
            let (level, pos) = (e.level.clone(), e.pos);
            self.fx(&level, pos, "оглушён", '\0', "#ffe080", 800);
        }
    }

    pub(crate) fn update_buffs(&mut self, id: Id) {
        let now = self.now;
        let tick_dot = self.tick_n.is_multiple_of(HALF_SEC);
        let Some(e) = self.ents.get(&id) else { return };
        if e.buffs.is_empty() {
            return;
        }
        let buffs = e.buffs.clone();
        let mut expired = false;
        for b in &buffs {
            if tick_dot && b.def.dot_per_sec != 0.0 {
                let mut amt = b.def.dot_per_sec / 2.0;
                if amt > 0.0 {
                    let t = if b.def.dmg_type.is_empty() {
                        "poison"
                    } else {
                        &b.def.dmg_type
                    };
                    let mut dot = Damage::single(t, amt);
                    dot.dot = true;
                    let src = Some(b.source).filter(|s| self.ents.contains_key(s));
                    self.damage(src, id, dot);
                    if !self.alive(id) {
                        return;
                    }
                } else {
                    if let Some(src) = self.ents.get(&b.source) {
                        amt *= 1.0 + src.stats.heal_pct / 100.0;
                    }
                    self.heal(id, -amt);
                }
            }
            if now >= b.until {
                expired = true;
            }
        }
        if expired {
            let e = self.ents.get_mut(&id).unwrap();
            e.buffs.retain(|b| now < b.until);
            e.recalc();
            if let Some(p) = e.player.as_mut() {
                p.dirty = true;
            }
        }
    }
}

pub(crate) fn hostile_factions(a: Faction, b: Faction) -> bool {
    (a == Faction::Player && b == Faction::Monster)
        || (a == Faction::Monster && b == Faction::Player)
}

/// Runs an ability by its kind; returns false if it could not be used.
pub(crate) fn cast(g: &mut Game, c: Id, a: &AbilityDef, target: Option<Id>) -> bool {
    match a.kind.as_str() {
        "projectile" => cast_projectile(g, c, a, target),
        "nova" => cast_nova(g, c, a),
        "cleave" => cast_cleave(g, c, a),
        "strike" => cast_strike(g, c, a, target),
        "heal" => cast_heal(g, c, a),
        "dash" => cast_dash(g, c, a),
        "buff" => cast_buff(g, c, a),
        "chain" => cast_chain(g, c, a, target),
        "taunt" => abilities::cast_taunt(g, c, a),
        "decoy" | "summon" => abilities::cast_summon(g, c, a),
        "mimic" => abilities::cast_mimic(g, c, a, target),
        "copied" => g.cast_copy(c, target, 1.0),
        "echo" => g.cast_copy(c, target, 2.0),
        "revive" => abilities::cast_revive(g, c, a),
        "death_sentence" => abilities::cast_death_sentence(g, c, a),
        _ => {
            g.log(c, "#ff8080", format!("Неизвестный тип умения: {}.", a.kind));
            false
        }
    }
}

pub(crate) fn cast_projectile(g: &mut Game, c: Id, a: &AbilityDef, target: Option<Id>) -> bool {
    let rng = ability_range(a);
    let ce = &g.ents[&c];
    let pos = ce.pos;
    let aim = if let Some(t) = target.and_then(|t| g.ents.get(&t)) {
        // lead a moving target a little
        let flight = t.pos.dist(pos) / (1000.0 / a.speed_ms.max(15).max(1) as f32);
        t.pos + t.vel * flight.min(0.6)
    } else if let Some(at) = g.aimed(c) {
        at
    } else {
        pos + ce.facing_vec() * rng
    };
    if aim.dist(pos) < 0.05 {
        return false;
    }
    let level = ce.level.clone();
    let (faction, r) = (ce.faction, ce.radius());
    let base = (aim - pos).angle();
    g.ents.get_mut(&c).unwrap().facing = base;
    let count = a.count.max(1);
    let speed = 1000.0
        / if a.speed_ms > 0 {
            a.speed_ms as f32
        } else {
            50.0
        };
    for i in 0..count {
        let off = i - count / 2;
        let ang = base + off as f32 * 0.14;
        let dir = Vec2::from_angle(ang);
        let mut dmg = g.ability_damage(c, a);
        g.roll_crit(c, &mut dmg);
        let e = Entity {
            kind: Kind::Projectile,
            name: a.name.clone(),
            glyph: a.glyph.clone(),
            color: a.color.clone(),
            level: level.clone(),
            pos: pos + dir * (r * 0.6),
            facing: ang,
            vel: dir * speed,
            proj: Some(Box::new(ProjState {
                owner: c,
                faction,
                vel: dir * speed,
                left: rng + 0.5,
                damage: dmg,
                radius: a.radius,
                on_hit: a.on_hit.clone(),
                ability: a.key.clone(),
            })),
            ..Default::default()
        };
        g.spawn(e);
    }
    g.fx_spell(&level, a, FX_CAST, pos, aim, 0.0);
    true
}

fn cast_nova(g: &mut Game, c: Id, a: &AbilityDef) -> bool {
    let mut dmg = g.ability_damage(c, a);
    g.roll_crit(c, &mut dmg);
    let ce = &g.ents[&c];
    let (f, level, pos) = (ce.faction, ce.level.clone(), ce.pos);
    let r = a.radius.max(1) as f32;
    g.fx_spell(&level, a, FX_AREA, pos, pos, r + 0.5);
    g.area_damage(Some(c), f, &level, pos, r, &dmg, a.on_hit.as_ref());
    true
}

fn cast_cleave(g: &mut Game, c: Id, a: &AbilityDef) -> bool {
    let r = a.radius.max(1) as f32;
    let ce = &g.ents[&c];
    let (level, pos, ahead) = (ce.level.clone(), ce.pos, ce.pos + ce.facing_vec());
    let victims: Vec<Id> = g
        .near(&level, pos, r + 0.6 + BOSS_RADIUS)
        .into_iter()
        .filter(|&o| g.hostile(c, o) && g.ents[&o].pos.dist(pos) <= r + 0.6 + g.ents[&o].radius())
        .collect();
    g.fx_spell(&level, a, FX_AREA, pos, ahead, r + 0.6);
    for o in victims {
        let mut dmg = g.ability_damage(c, a);
        let md = g.melee_damage(c);
        dmg.merge(&md, 0.5);
        g.roll_crit(c, &mut dmg);
        let dealt = g.damage(Some(c), o, dmg);
        g.weapon_hit(c, o, dealt);
        if let Some(b) = &a.on_hit {
            g.apply_buff(o, b, c);
        }
    }
    let now = g.now;
    if let Some(e) = g.ents.get_mut(&c) {
        e.next_attack = now + e.stats.attack_ms;
        e.swings = e.swings.wrapping_add(1);
    }
    true
}

fn cast_strike(g: &mut Game, c: Id, a: &AbilityDef, target: Option<Id>) -> bool {
    let Some(t) = target.filter(|t| g.ents.contains_key(t) && g.in_reach(c, *t)) else {
        g.log(c, "#808080", format!("Нет врага рядом для «{}».", a.name));
        return false;
    };
    let tpos = g.ents[&t].pos;
    let e = g.ents.get_mut(&c).unwrap();
    e.face_to(tpos);
    e.swings = e.swings.wrapping_add(1);
    let hits = a.count.max(1);
    let weapon_k = if hits > 1 { 0.5 } else { 1.0 };
    let (level, cpos) = (e.level.clone(), e.pos);
    for _ in 0..hits {
        if !g.alive(t) {
            break;
        }
        let mut dmg = g.ability_damage(c, a);
        let md = g.melee_damage(c);
        dmg.merge(&md, weapon_k);
        let te = &g.ents[&t];
        if a.execute > 0.0 && te.max_hp > 0.0 {
            dmg.scale(1.0 + a.execute * (1.0 - te.hp / te.max_hp));
        }
        dmg.backstab = a.backstab;
        g.roll_crit(c, &mut dmg);
        let tp = g.ents[&t].pos;
        g.fx_spell(&level, a, FX_HIT, tp, cpos, 0.0);
        let dealt = g.damage(Some(c), t, dmg);
        g.weapon_hit(c, t, dealt);
        if let Some(b) = &a.on_hit {
            g.apply_buff(t, b, c);
        }
    }
    let now = g.now;
    if let Some(e) = g.ents.get_mut(&c) {
        e.next_attack = now + e.stats.attack_ms;
    }
    true
}

fn cast_heal(g: &mut Game, c: Id, a: &AbilityDef) -> bool {
    let amount = g.ability_power(c, a) * (1.0 + g.ents[&c].stats.heal_pct / 100.0);
    let ce = &g.ents[&c];
    let missing = ce.max_hp - ce.hp;
    let (level, pos) = (ce.level.clone(), ce.pos);
    g.fx_spell(&level, a, FX_CAST, pos, pos, a.radius as f32);
    g.deed(c, "heal", amount.min(missing) as i32);
    g.heal(c, amount);
    if a.radius > 0 {
        for o in g.near(&level, pos, a.radius as f32 + 0.5) {
            let oe = &g.ents[&o];
            if o != c
                && oe.alive()
                && oe.blocks()
                && g.friendly(c, o)
                && oe.pos.dist(pos) <= a.radius as f32 + 0.5
            {
                let (missing, op) = (oe.max_hp - oe.hp, oe.pos);
                g.fx_spell(&level, a, FX_ALLY, op, pos, 0.0);
                g.deed(c, "heal", (amount * 0.7).min(missing) as i32);
                g.heal(o, amount * 0.7);
            }
        }
    }
    if let Some(b) = &a.buff {
        g.apply_buff(c, b, c);
    }
    true
}

fn cast_dash(g: &mut Game, c: Id, a: &AbilityDef) -> bool {
    let ce = &g.ents[&c];
    let (level, start, r) = (ce.level.clone(), ce.pos, ce.radius());
    let dir = match g.aimed(c) {
        Some(aim) if aim.dist(start) > 0.3 => (aim - start).norm(),
        _ => ce.facing_vec(),
    };
    let l = &g.levels[&level];
    let blocked = |x: i32, y: i32| !l.walkable(x, y);
    let range = ability_range(a);
    let mut pos = start;
    let mut hit = None;
    let mut travelled = 0.0;
    while travelled < range {
        let step = 0.2f32.min(range - travelled);
        let next = pos + dir * step;
        if !l.circle_fits(next, r, &blocked) {
            break;
        }
        // the first enemy in the way stops the dash
        let mut stop = false;
        for o in g.near(&level, next, r + BOSS_RADIUS) {
            let oe = &g.ents[&o];
            if o == c || !oe.blocks() || !oe.alive() || oe.pos.dist(next) > oe.radius() + r {
                continue;
            }
            if g.hostile(c, o) {
                hit = Some(o);
            }
            if g.hostile(c, o) || oe.kind != Kind::Player {
                stop = true;
            }
            break;
        }
        if stop {
            break;
        }
        pos = next;
        travelled += step;
    }
    if pos.dist(start) < 0.3 && hit.is_none() {
        g.log(c, "#808080", "Некуда рвануться.".into());
        return false;
    }
    g.fx_spell(&level, a, FX_BEAM, start, pos, 0.0);
    if let Some(e) = g.ents.get_mut(&c) {
        e.pos = pos;
        e.goal = None;
    }
    if let Some(o) = hit {
        if a.damage[1] > 0.0 {
            let op = g.ents[&o].pos;
            g.fx_spell(&level, a, FX_HIT, op, pos, 0.0);
            let mut dmg = g.ability_damage(c, a);
            g.roll_crit(c, &mut dmg);
            g.damage(Some(c), o, dmg);
            if let Some(b) = &a.on_hit {
                g.apply_buff(o, b, c);
            }
        }
    }
    if g.ents.get(&c).is_some_and(|e| e.player.is_some()) {
        g.after_player_move(c);
    }
    true
}

fn cast_buff(g: &mut Game, c: Id, a: &AbilityDef) -> bool {
    let Some(b) = &a.buff else { return false };
    g.apply_buff(c, b, c);
    let ce = &g.ents[&c];
    let (level, pos) = (ce.level.clone(), ce.pos);
    g.fx(&level, pos, &format!("{}!", b.name), '\0', &a.color, 1000);
    g.fx_spell(&level, a, FX_CAST, pos, pos, a.radius as f32);
    if a.radius > 0 {
        // party buff: allies around get it too
        for o in g.near(&level, pos, a.radius as f32 + 0.5) {
            let oe = &g.ents[&o];
            if o != c
                && oe.alive()
                && oe.blocks()
                && g.friendly(c, o)
                && oe.pos.dist(pos) <= a.radius as f32 + 0.5
            {
                let op = oe.pos;
                g.apply_buff(o, b, c);
                g.fx_spell(&level, a, FX_ALLY, op, pos, 0.0);
            }
        }
    }
    true
}

/// Hits the target, then jumps to the nearest enemies around it.
fn cast_chain(g: &mut Game, c: Id, a: &AbilityDef, target: Option<Id>) -> bool {
    let Some(mut t) = target.filter(|t| g.ents.contains_key(t)) else {
        g.log(c, "#808080", format!("Нет цели для «{}».", a.name));
        return false;
    };
    let ce = &g.ents[&c];
    let level = ce.level.clone();
    let mut from = ce.pos;
    let mut hit = std::collections::HashSet::new();
    let mut k = 1.0;
    for _ in 0..=a.count.max(1) {
        let Some(te) = g.ents.get(&t) else { break };
        let tp = te.pos;
        g.fx_spell(&level, a, FX_BEAM, from, tp, 0.0);
        let mut dmg = g.ability_damage(c, a);
        dmg.scale(k);
        g.roll_crit(c, &mut dmg);
        hit.insert(t);
        g.damage(Some(c), t, dmg);
        if let Some(b) = &a.on_hit {
            g.apply_buff(t, b, c);
        }
        from = tp;
        k *= 0.85;
        let l = &g.levels[&level];
        let mut next = None;
        let mut best = f32::MAX;
        for o in g.near(&level, from, 4.5) {
            if hit.contains(&o) || !g.hostile(c, o) {
                continue;
            }
            let op = g.ents[&o].pos;
            let d = op.dist(from);
            if d < best && d <= 4.5 && los(l, from.cell(), op.cell()) {
                next = Some(o);
                best = d;
            }
        }
        match next {
            Some(n) => t = n,
            None => break,
        }
    }
    true
}
