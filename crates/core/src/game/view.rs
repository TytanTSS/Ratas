//! What clients see: snapshots, the character sheet, item descriptions.

use super::*;
use crate::content::BuffDef;
use crate::proto::*;
use crate::world::los;

fn first_char(s: &str) -> char {
    s.chars().next().unwrap_or('?')
}

/// Describes an item for the client.
pub fn item_view(st: &ItemStack) -> ItemView {
    let Some(d) = st.def() else {
        return ItemView {
            key: st.key.clone(),
            name: st.key.clone(),
            glyph: '?',
            qty: st.qty,
            ..Default::default()
        };
    };
    let mut desc = Vec::new();
    let rarity = st.item_rarity();
    if rarity > COMMON || !slot_for(Some(d)).is_empty() {
        desc.push(rarity_name(rarity).to_string());
    }
    if d.kind == "weapon" {
        let t = if d.dmg_type.is_empty() {
            "blunt"
        } else {
            &d.dmg_type
        };
        let hands = if d.hands >= 2 {
            "двуручное"
        } else {
            "одноручное"
        };
        let k = st.stat_k();
        desc.push(format!(
            "Урон {:.0}-{:.0} ({}), {hands}",
            (d.damage[0] * k).round(),
            (d.damage[1] * k).round(),
            damage_type_name(t)
        ));
    }
    if d.heal > 0.0 {
        desc.push(format!("+{:.0} здоровья", d.heal));
    }
    if d.mana > 0.0 {
        desc.push(format!("+{:.0} маны", d.mana));
    }
    desc.extend(stat_lines(&scaled(&d.stats, st.stat_k())));
    desc.extend(stat_lines(&st.bonus));
    if let Some(b) = &d.on_hit {
        desc.push(format!("При ударе {:.0}%: {}", d.on_hit_pct, buff_desc(b)));
    }
    if let (Some(b), true) = (&d.buff, d.desc.is_empty()) {
        desc.push(buff_desc(b));
    }
    if !d.desc.is_empty() && (d.kind != "consumable" || d.buff.is_some() || !d.effect.is_empty()) {
        desc.push(d.desc.clone());
    }
    ItemView {
        key: st.key.clone(),
        name: st.name(),
        glyph: first_char(&d.glyph),
        color: d.color.clone(),
        kind: d.kind.clone(),
        qty: st.qty.max(1),
        value: st.value(),
        desc: desc.join(", "),
        hands: d.hands,
        rarity: rarity as i8,
    }
}

/// Multiplies stats (a rarer copy of an item is stronger).
fn scaled(m: &crate::content::Stats, k: f64) -> crate::content::Stats {
    if k == 1.0 {
        return m.clone();
    }
    m.iter()
        .map(|(key, v)| {
            (
                key.clone(),
                if v.abs() >= 3.0 {
                    (v * k).round()
                } else {
                    (v * k * 10.0).round() / 10.0
                },
            )
        })
        .collect()
}

/// How far from the start villages are common knowledge (about the span of
/// the classic world).
const HOME_LANDS: i32 = 350;

/// Formats the small values of stats: "3", "0.5", "1.25".
pub fn num(v: f64) -> String {
    if v == v.trunc() {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn signed(v: f64) -> String {
    if v >= 0.0 {
        format!("+{}", num(v))
    } else {
        num(v)
    }
}

/// Describes an effect: "Горение (огонь 3/с, 3 с)".
pub fn buff_desc(b: &BuffDef) -> String {
    let mut parts = Vec::new();
    if b.stun {
        parts.push("оглушение".to_string());
    }
    if b.dot_per_sec > 0.0 {
        let t = if b.dmg_type.is_empty() {
            "poison"
        } else {
            &b.dmg_type
        };
        parts.push(format!("{} {}/с", damage_type_name(t), num(b.dot_per_sec)));
    } else if b.dot_per_sec < 0.0 {
        parts.push(format!("лечение {}/с", num(-b.dot_per_sec)));
    }
    parts.extend(stat_lines(&b.stats));
    parts.push(format!("{} с", num(b.duration_ms as f64 / 1000.0)));
    format!("{} ({})", b.name, parts.join("; "))
}

pub fn stat_lines(m: &crate::content::Stats) -> Vec<String> {
    m.iter()
        .map(|(k, v)| format!("{} {}", stat_name(k), signed(*v)))
        .collect()
}

/// The visible effects on an entity.
fn status_of(e: &Entity) -> u16 {
    let mut st = 0;
    for b in &e.buffs {
        let d = &b.def;
        if d.dot_per_sec > 0.0 {
            st |= match d.dmg_type.as_str() {
                "fire" => STATUS_BURNING,
                "slash" | "pierce" | "blunt" => STATUS_BLEEDING,
                "shadow" | "arcane" => STATUS_CURSED,
                "holy" | "lightning" => STATUS_HOLY,
                "cold" => STATUS_CHILLED,
                _ => STATUS_POISONED,
            };
        }
        if d.stun {
            st |= STATUS_STUNNED;
        }
        if d.silence {
            st |= STATUS_SILENCED;
        }
        if d.stealth {
            st |= STATUS_STEALTH;
        }
        if d.stats.get("move_speed").copied().unwrap_or(0.0) < 0.0 {
            st |= STATUS_CHILLED;
        }
        for (k, v) in &d.stats {
            if *v < 0.0 && k.starts_with("res_") {
                st |= STATUS_CURSED;
            }
            if (*v > 0.0 && k.starts_with("res_") && b.source == e.id)
                || (*v > 0.0 && k == "res_all")
            {
                st |= STATUS_SHIELDED;
            }
        }
    }
    st
}

fn buff_views(e: &Entity, now: f64) -> Vec<BuffView> {
    e.buffs
        .iter()
        .map(|b| BuffView {
            name: b.def.name.clone(),
            color: b.def.color.clone(),
            left: (b.until - now) as i32,
        })
        .collect()
}

impl Game {
    /// The enemy shown in a player's HUD: the one they fight, or the nearest
    /// visible one.
    pub(crate) fn target_for(&self, id: Id) -> Option<Id> {
        let e = &self.ents[&id];
        let p = e.p();
        let vision = self.vision(e) as f32;
        if let Some(t) = self.ents.get(&p.target) {
            if t.alive()
                && t.level == e.level
                && self.hostile(id, p.target)
                && self.can_see(id, p.target)
                && self.now - p.target_at < 12000.0
                && e.dist(t) <= vision + 2.0
            {
                return Some(p.target);
            }
        }
        let l = &self.levels[&e.level];
        let mut best = None;
        let mut bd = f32::MAX;
        for o in self.near(&e.level, e.pos, vision) {
            let oe = &self.ents[&o];
            if oe.monster.is_none() || !self.hostile(id, o) || !self.can_see(id, o) {
                continue;
            }
            let d = e.dist(oe);
            if d < bd && d <= vision {
                best = Some(o);
                bd = d;
            }
        }
        best.filter(|b| los(l, e.cell(), self.ents[b].cell()))
    }

    pub(crate) fn target_view(&self, t: Id) -> TargetView {
        let te = &self.ents[&t];
        let mut v = TargetView {
            id: t,
            name: te.name.clone(),
            color: te.color.clone(),
            ..Default::default()
        };
        if te.max_hp > 0.0 {
            v.hp = (100.0 * te.hp / te.max_hp).clamp(0.0, 100.0) as u8;
        }
        if let Some(m) = &te.monster {
            v.level = m.lvl;
            v.boss = db().monster(&m.def).is_some_and(|d| d.boss);
        }
        for dt in &db().b.damage_types {
            let r = te.stats.resist(&dt.key).round() as i32;
            if r != 0 {
                v.res.insert(dt.key.clone(), r);
            }
        }
        v.effects = buff_views(te, self.now);
        v
    }

    /// The per-tick view for one player.
    pub fn snapshot(&mut self, id: Id) -> Snapshot {
        let now = self.now;
        let e = &self.ents[&id];
        let p = e.p();
        let level = self.levels.get(&e.level);
        let mut you = SelfView {
            x: e.pos.x,
            y: e.pos.y,
            hp: e.hp,
            max_hp: e.max_hp,
            mp: e.mp,
            max_mp: e.max_mp,
            xp: p.xp,
            xp_next: xp_for_level(p.level),
            level: p.level,
            gold: p.gold,
            dead: e.dead,
            vision: self.vision(e),
            attr_points: p.attr_points,
            skill_points: p.skill_points,
            speed: e.speed(level.map_or(1.0, |l| l.def_at(e.cell()).move_cost)),
            ..Default::default()
        };
        if e.dead {
            you.respawn_in = (p.respawn_at - now).max(0.0) as i32;
        }
        for (i, key) in p.hotbar.iter().enumerate() {
            if let Some(a) = db().ability(key) {
                if a.cooldown_ms > 0 {
                    let left = e.cooldowns.get(key).copied().unwrap_or(0.0) - now;
                    if left > 0.0 {
                        you.cooldown[i] = (left / a.cooldown_ms as f64) as f32;
                    }
                }
            }
        }
        you.buffs = buff_views(e, now);
        if let Some(l) = level {
            you.standing_on = l.def_at(e.cell()).interact.clone();
        }
        if e.level == "overworld" {
            you.region = self.region_name(e.cell());
        }
        if let Some(t) = self.target_for(id) {
            you.target = Some(self.target_view(t));
        }
        you.can_rise = e.dead && now >= p.can_rise;
        you.safe = self.safe_zone(id);
        you.pvp = self.pvp;
        if !p.copied.is_empty() {
            you.copied = p.copied.clone();
            you.copied_left = (p.copied_end - now) as i32;
        }
        you.invites = p
            .invites
            .iter()
            .filter(|(from, at)| now - **at < 120000.0 && self.online.contains_key(*from))
            .map(|(f, _)| f.clone())
            .collect();
        you.invites.sort();
        if let Some(pt) = self.party_of(id) {
            for n in &pt.members {
                let Some(&mid) = self.online.get(n) else {
                    continue;
                };
                if mid == id {
                    continue;
                }
                let m = &self.ents[&mid];
                let mut pm = PartyMember {
                    name: m.name.clone(),
                    class: m.p().class.clone(),
                    level: m.p().level,
                    hp: m.hp,
                    max_hp: m.max_hp,
                    mp: m.mp,
                    max_mp: m.max_mp,
                    dead: m.dead,
                    leader: pt.leader == *n,
                    x: m.pos.x,
                    y: m.pos.y,
                    level_id: m.level.clone(),
                    buffs: buff_views(m, now),
                    ..Default::default()
                };
                if m.level != e.level {
                    if let Some(l) = self.levels.get(&m.level) {
                        pm.where_ = l.name.clone();
                    }
                }
                you.party.push(pm);
            }
        }

        let mut ents = Vec::new();
        let me_pos = e.pos;
        // what the client shows: 60 steps to the sides, 40 up and down
        for oid in self.near(&e.level, me_pos, 60.0) {
            let o = &self.ents[&oid];
            let (dx, dy) = (o.pos.x - me_pos.x, o.pos.y - me_pos.y);
            if dx.abs() > 60.0 || dy.abs() > 40.0 {
                continue;
            }
            if o.kind == Kind::Player && o.dead && now - o.p().died_at > REVIVE_WINDOW_MS {
                continue;
            }
            let hostile = self.hostile(id, oid);
            if hostile && !self.can_see(id, oid) {
                continue; // stealthed enemy
            }
            let mut v = EntityView {
                id: oid,
                x: o.pos.x,
                y: o.pos.y,
                vx: o.vel.x,
                vy: o.vel.y,
                facing: o.facing,
                radius: o.radius(),
                glyph: first_char(&o.glyph),
                color: o.color.clone(),
                kind: o.kind as u8,
                name: o.name.clone(),
                hostile: hostile || (o.faction == Faction::Monster && o.kind != Kind::Projectile),
                status: status_of(o),
                dead: o.dead,
                ally: oid != id && (self.same_party(id, oid) || o.owner == id),
                swing: o.swings,
                ..Default::default()
            };
            let d = db();
            let illusion = o
                .monster
                .as_ref()
                .is_some_and(|m| m.def == "illusion" && self.ents.contains_key(&o.owner));
            if illusion {
                // illusions look like their maker
                let own = &self.ents[&o.owner];
                v.kind = own.kind as u8;
                v.status |= STATUS_ILLUSION;
                if let Some(op) = &own.player {
                    v.def = op.class.clone();
                    v.gear = gear_of(own);
                }
            } else if let Some(u) = o.npc.as_ref().filter(|n| !n.unique.is_empty()) {
                v.def = format!("unique:{}", u.unique);
                if let Some(ud) = d.unique(&u.unique) {
                    v.model = ud.model.clone();
                }
            } else if let Some(n) = &o.npc {
                v.def = n.role.clone();
                if let Some(r) = d.npc_role(&n.role) {
                    v.model = r.model.clone();
                }
            } else if let Some(m) = &o.monster {
                v.def = m.def.clone();
                if let Some(md) = d.monster(&m.def) {
                    v.model = md.model.clone();
                }
            } else if let Some(op) = &o.player {
                v.def = op.class.clone();
                if let Some(c) = d.class(&op.class) {
                    v.model = c.model.clone();
                }
                v.gear = gear_of(o);
            } else if let Some(it) = &o.item {
                v.def = it.key.clone();
                v.rarity = it.item_rarity() as u8;
                if v.rarity >= RARE as u8 {
                    v.color = rarity_color(v.rarity as i32).into(); // rare loot stands out
                }
            } else if let Some(pr) = &o.proj {
                v.def = pr.ability.clone();
            }
            if o.max_hp > 0.0 && o.kind != Kind::Item {
                v.hp = (100.0 * o.hp / o.max_hp).clamp(0.0, 100.0) as u8;
            }
            if let Some(m) = &o.monster {
                v.boss = d.monster(&m.def).is_some_and(|md| md.boss);
            }
            if o.speech_until > now {
                v.speech = o.speech.clone();
            }
            ents.push(v);
        }
        Snapshot {
            tick: self.tick_n,
            time_of_day: self.time_of_day(),
            you,
            entities: ents,
            fx: self
                .fx
                .get(&self.ents[&id].level)
                .cloned()
                .unwrap_or_default(),
            online: self.online.keys().cloned().collect(),
        }
    }

    /// What a player knows on the world map.
    ///
    /// Villages within HOME_LANDS steps of the start are known from the first
    /// day, the rest once seen.
    pub(crate) fn places(&self, id: Id) -> Vec<Place> {
        let e = &self.ents[&id];
        let p = e.p();
        let mut out = Vec::new();
        let ow = self.levels.get("overworld");
        let empty = crate::world::Bitset::default();
        let bs = p.explored.get("overworld").unwrap_or(&empty);
        let seen = |q: Pos| ow.is_some_and(|l| l.inside(q.x, q.y) && bs.get(l.idx(q)));
        // cities are famous; villages are known around home and once seen
        for v in &self.villages {
            if !v.city && v.center.manhattan(self.start) > HOME_LANDS && !seen(v.center) {
                continue;
            }
            out.push(Place {
                name: v.name.clone(),
                kind: if v.city { "city" } else { "village" }.into(),
                x: v.center.x,
                y: v.center.y,
            });
        }
        for en in &self.entrances {
            if seen(en.pos) {
                out.push(Place {
                    name: en.name.clone(),
                    kind: format!("dungeon:{}", en.theme),
                    x: en.pos.x,
                    y: en.pos.y,
                });
            }
        }
        for &i in &p.found {
            if let Some(lm) = self.landmarks.get(i) {
                out.push(Place {
                    name: lm.name.clone(),
                    kind: lm.kind.clone(),
                    x: lm.pos.x,
                    y: lm.pos.y,
                });
            }
        }
        // a region is labelled at its most central cell
        for r in &self.regions {
            if seen(r.at) {
                out.push(Place {
                    name: r.name.clone(),
                    kind: format!("region:{}", r.kind),
                    x: r.at.x,
                    y: r.at.y,
                });
            }
        }
        for q in &p.quests {
            if let (Some(at), false) = (q.at, q.done) {
                out.push(Place {
                    name: q.where_.clone(),
                    kind: "quest".into(),
                    x: at.x,
                    y: at.y,
                });
            }
        }
        out
    }

    /// Works out where region names go for saves made before regions kept
    /// it: the region's own cell nearest to its centroid, in two passes over
    /// the map.
    pub(crate) fn anchor_regions(&mut self) {
        if self.regions.iter().all(|r| !r.at.is_zero()) {
            return;
        }
        let Some(l) = self.levels.get("overworld") else {
            return;
        };
        if self.region_map.len() != (l.w * l.h) as usize {
            return;
        }
        let n = self.regions.len();
        let (mut sx, mut sy, mut cnt) = (vec![0i64; n], vec![0i64; n], vec![0i64; n]);
        for (i, &r) in self.region_map.iter().enumerate() {
            if r == 0 || r as usize > n {
                continue;
            }
            let k = r as usize - 1;
            sx[k] += i as i64 % l.w as i64;
            sy[k] += i as i64 / l.w as i64;
            cnt[k] += 1;
        }
        let centre: Vec<Pos> = (0..n)
            .map(|k| {
                let c = cnt[k].max(1);
                Pos::new((sx[k] / c) as i32, (sy[k] / c) as i32)
            })
            .collect();
        let mut best = vec![(i32::MAX, Pos::default()); n];
        for (i, &r) in self.region_map.iter().enumerate() {
            if r == 0 || r as usize > n {
                continue;
            }
            let k = r as usize - 1;
            let p = Pos::new(i as i32 % l.w, i as i32 / l.w);
            let d = p.dist_sq(centre[k]);
            if d < best[k].0 {
                best[k] = (d, p);
            }
        }
        for (k, reg) in self.regions.iter_mut().enumerate() {
            if reg.at.is_zero() {
                reg.at = best[k].1;
            }
        }
    }

    /// The full character sheet.
    pub fn sheet(&mut self, id: Id) -> PlayerSheet {
        self.update_relics(id);
        let places = self.places(id);
        let e = &self.ents[&id];
        let p = e.p();
        let mut sh = PlayerSheet {
            name: e.name.clone(),
            class: p.class.clone(),
            level: p.level,
            attr_points: p.attr_points,
            skill_points: p.skill_points,
            attrs: p.attrs.clone(),
            stats: e.stats.map(),
            skills: p.skills.clone(),
            abilities: p.abilities.clone(),
            hotbar: p.hotbar.clone(),
            unlocks: p.unlocks.clone(),
            places,
            deeds: p.deeds.clone(),
            found: p.found.len() as i32,
            ..Default::default()
        };
        sh.stats.insert("move_ms".into(), e.stats.move_ms);
        sh.stats.insert("attack_ms".into(), e.stats.attack_ms);
        sh.stats.insert("kills".into(), p.kills as f64);
        sh.inventory = p.inventory.iter().map(item_view).collect();
        for (slot, st) in &p.equip {
            sh.equip.insert(slot.clone(), item_view(st));
        }
        for q in &p.quests {
            let mut qv = QuestView {
                text: upper_first(&quest_text(q)),
                have: q.have,
                need: q.need,
                done: q.done,
                giver: format!("{}, {}", q.giver, q.village),
                unique: !q.unique.is_empty(),
                where_: q.where_.clone(),
                ..Default::default()
            };
            if let Some(at) = q.at {
                qv.x = at.x;
                qv.y = at.y;
                if e.level == "overworld" && !q.done {
                    qv.where_ = format!(
                        "{} — {}, ~{} шагов",
                        q.where_,
                        dialogue::compass_ru(e.cell(), at),
                        e.cell().dist(at)
                    );
                }
            }
            qv.reward = if q.reward.is_empty() {
                format!("{} золота", q.gold)
            } else {
                reward_name(&q.reward)
            };
            sh.quests.push(qv);
        }
        for c in &p.classes {
            sh.classes.push(ClassView {
                key: c.clone(),
                level: class_level(p, c),
                subclass: p.subclasses.get(c).cloned().unwrap_or_default(),
            });
        }
        sh
    }

    /// The current level of a player for transmission. The packed tiles of
    /// a level are kept until a tile changes: the overworld takes a while.
    pub fn level_data(&mut self, id: Id) -> LevelData {
        let e = &self.ents[&id];
        let l = &self.levels[&e.level];
        let tiles = match self.packed_tiles.get(&l.id) {
            Some((ver, data)) if *ver == l.ver => data.clone(),
            _ => {
                let data = compress(&l.tiles);
                self.packed_tiles
                    .insert(l.id.clone(), (l.ver, data.clone()));
                data
            }
        };
        LevelData {
            id: l.id.clone(),
            name: l.name.clone(),
            w: l.w,
            h: l.h,
            tiles,
            explored: e
                .p()
                .explored
                .get(&l.id)
                .map(|b| compress(&b.0))
                .unwrap_or_default(),
            lit: l.lit,
            depth: l.depth,
            theme: l.theme.clone(),
        }
    }
}

/// What a hero visibly wears: right hand, left hand, head, chest, back.
fn gear_of(e: &Entity) -> Vec<String> {
    let eq = &e.p().equip;
    [SLOT_MAIN, SLOT_OFF, SLOT_HEAD, SLOT_CHEST, SLOT_BACK]
        .iter()
        .map(|s| eq.get(*s).map(|i| i.key.clone()).unwrap_or_default())
        .collect()
}
