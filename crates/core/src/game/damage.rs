//! Damage: types, resistances, armor, dodge, block, crits and leech.

use super::*;
use crate::content::{BuffDef, TileDef};

/// An amount of one damage type.
#[derive(Clone, Debug, Default)]
pub struct DmgPart {
    pub kind: String,
    pub amount: f64,
}

/// One hit. A hit may carry several damage types (a flaming sword deals
/// slashing and fire damage); each part is reduced by the target's resistance
/// to its type, physical parts also by armor.
#[derive(Clone, Debug, Default)]
pub struct Damage {
    pub parts: Vec<DmgPart>,
    pub crit: bool,
    /// percent of the damage dealt that heals the attacker
    pub leech: f64,
    /// damage over time, hazards, thorns: no dodge, no armor
    pub dot: bool,
    /// multiplier when attacking from stealth or a distracted target
    pub backstab: f64,
}

impl Damage {
    pub fn single(t: &str, v: f64) -> Damage {
        Damage {
            parts: vec![DmgPart {
                kind: t.into(),
                amount: v,
            }],
            ..Default::default()
        }
    }
    pub fn add(&mut self, t: &str, v: f64) {
        if v <= 0.0 {
            return;
        }
        match self.parts.iter_mut().find(|p| p.kind == t) {
            Some(p) => p.amount += v,
            None => self.parts.push(DmgPart {
                kind: t.into(),
                amount: v,
            }),
        }
    }
    pub fn merge(&mut self, o: &Damage, k: f64) {
        for p in &o.parts {
            self.add(&p.kind, p.amount * k);
        }
    }
    pub fn scale(&mut self, k: f64) {
        for p in &mut self.parts {
            p.amount *= k;
        }
    }
    /// The raw amount before resistances.
    pub fn total(&self) -> f64 {
        self.parts.iter().map(|p| p.amount).sum()
    }
    /// The type of the largest part.
    pub fn main(&self) -> String {
        let mut best = (String::new(), -1.0);
        for p in &self.parts {
            if p.amount > best.1 {
                best = (p.kind.clone(), p.amount);
            }
        }
        best.0
    }
}

fn is_physical(t: &str) -> bool {
    db().damage_type(t)
        .map(|d| d.group == "physical")
        .unwrap_or(true)
}

/// Positive armor has diminishing returns, broken (negative) armor makes
/// physical hits 3% stronger per point.
pub fn armor_factor(armor: f64) -> f64 {
    if armor >= 0.0 {
        1.0 - armor / (armor + 15.0)
    } else {
        1.0 + (-armor).min(60.0) * 0.03
    }
}

/// The damage type of a dangerous tile.
pub fn hazard_type(def: &TileDef) -> &str {
    if def.dmg_type.is_empty() {
        "fire"
    } else {
        &def.dmg_type
    }
}

/// Whether an effect is harmful (removed by cleansing).
pub fn is_debuff(b: &BuffDef) -> bool {
    b.stun || b.dot_per_sec > 0.0 || b.stats.values().any(|v| *v < 0.0)
}

pub fn monster_scale(lvl: i32) -> f64 {
    1.0 + 0.18 * (lvl - 1) as f64
}

impl Game {
    /// Applies the attacker's per-type damage bonuses.
    pub(crate) fn type_bonus(&self, src: Id, d: &mut Damage) {
        let s = &self.ents[&src].stats;
        for p in &mut d.parts {
            p.amount *= (1.0 + s.type_bonus(&p.kind) / 100.0).max(0.0);
        }
    }

    pub(crate) fn roll_crit(&mut self, id: Id, d: &mut Damage) {
        let (crit, mult) = {
            let s = &self.ents[&id].stats;
            (s.crit, s.crit_mult)
        };
        if self.chance(crit) {
            d.crit = true;
            d.scale(mult);
            self.deed(id, "crits", 1);
        }
    }

    /// A weapon hit without the critical roll.
    pub(crate) fn melee_damage(&mut self, a: Id) -> Damage {
        let e = &self.ents[&a];
        let s = e.stats.clone();
        let is_player = e.player.is_some();
        let mut dmg = self.roll(s.weapon_dmg[0], s.weapon_dmg[1]);
        if is_player {
            dmg += s.str_ * 0.6;
        }
        dmg *= (1.0 + s.melee_pct / 100.0).max(0.0);
        let mut d = Damage {
            leech: s.life_leech,
            ..Default::default()
        };
        d.add(&s.weapon_type, dmg);
        if s.gear.dual {
            // the second weapon strikes along at half strength
            let v =
                self.roll(s.off_dmg[0], s.off_dmg[1]) * 0.5 * (1.0 + s.melee_pct / 100.0).max(0.0);
            d.add(&s.off_type, v);
        }
        for t in &db().b.damage_types {
            let v = s.m(&format!("add_{}", t.key));
            if v > 0.0 {
                let k = self.roll(0.8, 1.2);
                d.add(&t.key, v * k);
            }
        }
        self.type_bonus(a, &mut d);
        d
    }

    pub(crate) fn ability_type(&self, c: Id, a: &AbilityDef) -> String {
        match a.dmg_type.as_str() {
            "weapon" => self.ents[&c].stats.weapon_type.clone(),
            "" => match a.kind.as_str() {
                "strike" | "cleave" | "dash" => self.ents[&c].stats.weapon_type.clone(),
                _ => "arcane".into(),
            },
            t => t.into(),
        }
    }

    /// The rolled and scaled strength of an ability (damage or healing).
    pub(crate) fn ability_power(&mut self, c: Id, a: &AbilityDef) -> f64 {
        let v = self.roll(a.damage[0], a.damage[1]);
        let e = &self.ents[&c];
        if e.player.is_some() {
            let s = &e.stats;
            let (attr, pct) = match a.scale.as_str() {
                "str" => (s.str_, s.melee_pct),
                "dex" => (s.dex, s.ranged_pct),
                "int" => (s.int, s.spell_pct),
                "vit" => (s.vit, 0.0),
                _ => (0.0, 0.0),
            };
            return (v + attr * a.scale_k) * (1.0 + pct / 100.0).max(0.0);
        }
        if let Some(m) = &e.monster {
            return v * monster_scale(m.lvl);
        }
        v
    }

    /// The damage of an ability without the critical roll.
    pub(crate) fn ability_damage(&mut self, c: Id, a: &AbilityDef) -> Damage {
        let s = self.ents[&c].stats.clone();
        let mut d = Damage {
            leech: a.leech + s.life_leech,
            backstab: a.backstab,
            ..Default::default()
        };
        if a.damage[1] > 0.0 {
            let v = self.ability_power(c, a);
            if !a.split.is_empty() {
                for t in &a.split {
                    d.add(t, v / a.split.len() as f64);
                }
            } else {
                let t = self.ability_type(c, a);
                d.add(&t, v);
            }
        }
        // bows and crossbows put their own strength into every arrow
        if (a.equip == "bow" || a.key == "bow_shot") && s.gear.ranged {
            let mut v = self.roll(s.weapon_dmg[0], s.weapon_dmg[1]);
            if self.ents[&c].player.is_some() {
                v += s.dex * 0.5;
            }
            d.add(&s.weapon_type, v * (1.0 + s.ranged_pct / 100.0).max(0.0));
        }
        self.type_bonus(c, &mut d);
        d
    }

    /// Applies a hit after dodge, armor and resistances; returns the health
    /// actually taken.
    pub(crate) fn damage(&mut self, src: Option<Id>, dst: Id, d: Damage) -> f64 {
        let src = src.filter(|s| self.ents.contains_key(s));
        let Some(de) = self.ents.get(&dst) else {
            return 0.0;
        };
        if !de.alive() || d.parts.is_empty() {
            return 0.0;
        }
        if de.player.as_ref().map(|p| p.god).unwrap_or(false) {
            self.provoke(src, dst);
            return 0.0;
        }
        let (level, pos) = (de.level.clone(), de.pos);
        let dodge = de.stats.dodge;
        if !d.dot && src.is_some() && dodge > 0.0 && self.chance(dodge) {
            self.fx(&level, pos, "уклон", '\0', "#a0a0ff", 600);
            self.deed(dst, "dodge", 1);
            return 0.0;
        }
        let mut mult = 1.0;
        if let (false, Some(s)) = (d.dot, src) {
            let as_ = self.ents[&s].stats.clone();
            if self.ambush(s, dst) {
                let k = (1.0 + as_.ambush_pct / 100.0) * d.backstab.max(1.0);
                if k > 1.05 {
                    mult *= k;
                    self.fx(&level, pos, "в спину!", '\0', "#c08aff", 700);
                }
            }
            if as_.stealthed {
                self.break_stealth(s);
            }
            if as_.duel_pct > 0.0 && self.alone(s, dst) {
                mult *= 1.0 + as_.duel_pct / 100.0;
            }
            let se = &self.ents[&s];
            if as_.fury > 0.0 && se.max_hp > 0.0 {
                mult *= 1.0 + as_.fury / 100.0 * (1.0 - se.hp / se.max_hp).max(0.0);
            }
        }
        let block = self.ents[&dst].stats.block;
        let blocked = !d.dot && block > 0.0 && self.chance(block);
        if blocked {
            self.fx(&level, pos, "блок", '\0', "#a0c0ff", 600);
            self.deed(dst, "block", 1);
        }
        let ds = self.ents[&dst].stats.clone();
        let mut total = 0.0;
        let mut immune = true;
        let mut dealt_by_type = Vec::new();
        for p in &d.parts {
            let mut v = p.amount * mult;
            if v <= 0.0 {
                continue;
            }
            if !d.dot && is_physical(&p.kind) {
                v *= armor_factor(ds.armor);
                if blocked {
                    v *= 0.25;
                }
            }
            let res = ds.resist(&p.kind);
            if res < 100.0 {
                immune = false;
            }
            total += v * (1.0 - res / 100.0);
            let dealt = (v * (1.0 - res / 100.0)).round() as i32;
            if dealt > 0 {
                dealt_by_type.push((p.kind.clone(), dealt));
            }
        }
        if let Some(s) = src {
            if s != dst {
                for (t, n) in dealt_by_type {
                    self.deed(s, &format!("dmg:{t}"), n);
                }
            }
        }
        self.provoke(src, dst);
        if immune {
            if !d.dot {
                self.fx(&level, pos, "иммунитет", '\0', "#9090a0", 700);
            }
            return 0.0;
        }
        let total = total.max(1.0);
        let de = self.ents.get_mut(&dst).unwrap();
        de.hp -= total;
        let is_player = de.player.is_some();
        let mut color = "#ffffff".to_string();
        if let Some(t) = db().damage_type(&d.main()) {
            if t.group != "physical" {
                color = t.color.clone();
            }
        }
        if is_player {
            color = "#ff4a4a".into();
        } else if d.crit {
            color = "#ffff4a".into();
        }
        let mut text = (total.round() as i64).to_string();
        if d.crit {
            text.push('!');
        }
        self.fx(&level, pos, &text, '\0', &color, 700);
        if let Some(s) = src {
            if d.leech > 0.0 && s != dst && self.alive(s) {
                self.leech(s, total * d.leech / 100.0);
            }
        }
        if self.ents[&dst].hp <= 0.0 {
            self.kill(dst, src);
        }
        total
    }

    /// Makes a monster fight back and remembers the player's target.
    pub(crate) fn provoke(&mut self, src: Option<Id>, dst: Id) {
        let Some(mut s) = src else { return };
        if let Some(se) = self.ents.get(&s) {
            if se.kind == Kind::Projectile {
                match se
                    .proj
                    .as_ref()
                    .map(|p| p.owner)
                    .filter(|o| self.ents.contains_key(o))
                {
                    Some(o) => s = o,
                    None => return,
                }
            }
        } else {
            return;
        }
        let now = self.now;
        if s != dst && self.hostile(dst, s) {
            let spos = self.ents[&s].pos;
            let dpos = self.ents[&dst].pos;
            let cur = self.ents[&dst].monster.as_ref().map(|m| m.target);
            if let Some(cur) = cur {
                let switch = match self.ents.get(&cur) {
                    None => true,
                    Some(t) => cur == 0 || !t.alive() || dpos.dist(t.pos) > dpos.dist(spos) + 3.0,
                };
                let ms = self.ents.get_mut(&dst).unwrap().monster.as_mut().unwrap();
                if switch {
                    ms.target = s;
                }
                if ms.target == s {
                    ms.last_seen = spos;
                    ms.last_seen_at = now;
                }
            }
        }
        if let Some(pc) = self.controller(s) {
            if pc != dst {
                if let Some(p) = self.ents.get_mut(&pc).and_then(|e| e.player.as_mut()) {
                    p.target = dst;
                    p.target_at = now;
                }
            }
        }
        let keep = self.ents[&dst].player.as_ref().map(|p| {
            let t = p.target;
            self.ents.get(&t).map(|t| t.alive()).unwrap_or(false) && now - p.target_at <= 4000.0
        });
        if keep == Some(false) {
            let p = self.ents.get_mut(&dst).unwrap().pm();
            p.target = s;
            p.target_at = now;
        }
    }

    /// Attacking unseen (from stealth) or a target busy with someone else.
    pub(crate) fn ambush(&self, src: Id, dst: Id) -> bool {
        if self.ents[&src].stats.stealthed {
            return true;
        }
        let Some(m) = self.ents[&dst].monster.as_ref() else {
            return false;
        };
        if m.target == 0 || !self.ents.contains_key(&m.target) {
            return false;
        }
        self.controller(m.target) != self.controller(src)
    }

    /// No other enemy of src stands near it (duels).
    pub(crate) fn alone(&self, src: Id, dst: Id) -> bool {
        let se = &self.ents[&src];
        !self.on_level(&se.level).into_iter().any(|o| {
            o != dst
                && self
                    .ents
                    .get(&o)
                    .map(|oe| oe.dist(se) <= 4.0)
                    .unwrap_or(false)
                && self.hostile(src, o)
        })
    }

    pub(crate) fn break_stealth(&mut self, id: Id) {
        let e = self.ents.get_mut(&id).unwrap();
        e.buffs.retain(|b| !b.def.stealth);
        e.recalc();
        if let Some(p) = e.player.as_mut() {
            p.dirty = true;
        }
    }

    pub(crate) fn leech(&mut self, id: Id, amount: f64) {
        let e = self.ents.get_mut(&id).unwrap();
        if amount <= 0.0 || e.hp >= e.max_hp {
            return;
        }
        e.hp = (e.hp + amount).min(e.max_hp);
        let (level, pos) = (e.level.clone(), e.pos);
        if amount >= 2.0 {
            self.fx(
                &level,
                pos,
                &format!("+{}", amount.round() as i64),
                '\0',
                "#60ff90",
                600,
            );
        }
    }

    /// Finishes a melee hit: weapon effects and thorns.
    pub(crate) fn weapon_hit(&mut self, a: Id, d: Id, dealt: f64) {
        if dealt <= 0.0 || !self.ents.contains_key(&a) {
            return;
        }
        let s = self.ents[&a].stats.clone();
        if let Some(b) = &s.weapon_on_hit {
            if self.alive(d) && self.chance(s.on_hit_pct) {
                self.apply_buff(d, b, a);
            }
        }
        if let Some(b) = &s.off_on_hit {
            if self.alive(d) && self.chance(s.off_pct) {
                self.apply_buff(d, b, a);
            }
        }
        let thorns = self.ents.get(&d).map(|e| e.stats.thorns).unwrap_or(0.0);
        if thorns > 0.0 && self.alive(a) {
            let mut th = Damage::single("pierce", thorns);
            th.dot = true;
            self.damage(Some(d), a, th);
        }
    }

    pub(crate) fn heal(&mut self, id: Id, amount: f64) {
        let Some(e) = self.ents.get_mut(&id) else {
            return;
        };
        if !e.alive() {
            return;
        }
        e.hp = (e.hp + amount).min(e.max_hp);
        let (level, pos) = (e.level.clone(), e.pos);
        self.fx(
            &level,
            pos,
            &format!("+{}", amount.round() as i64),
            '\0',
            "#60ff60",
            800,
        );
    }
}

use crate::content::AbilityDef;
