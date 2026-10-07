//! Fusing abilities of different classes (the fusion window, U) and casting
//! the fused ones. What a fusion is made of is in content/fusion.rs.

use super::*;
use crate::content::fusion::{
    fusable_kind, fusion_key, fusion_parts, gear_clash, is_fusion, FUSION_KIND,
};
use crate::proto::FX_CAST;

/// The character level from which abilities fuse; every FUSION_LEVEL levels
/// open one more fusion.
pub const FUSION_LEVEL: i32 = 5;

/// How many fusions a hero of a level may hold.
pub fn max_fusions(level: i32) -> usize {
    (level / FUSION_LEVEL).max(0) as usize
}

/// The fused abilities of a hero.
pub fn fusions_of(p: &PlayerState) -> Vec<&String> {
    p.abilities.iter().filter(|k| is_fusion(k)).collect()
}

/// Whether an ability is melted into one of the hero's fusions.
pub fn fused_in(p: &PlayerState, key: &str) -> bool {
    p.abilities
        .iter()
        .filter_map(|f| fusion_parts(f))
        .any(|(a, b)| a == key || b == key)
}

/// Explains why an ability cannot take part in a fusion ("" = it can).
pub fn can_fuse_one(p: &PlayerState, key: &str) -> String {
    let d = db();
    let Some(a) = d.ability(key) else {
        return "нет такого умения".into();
    };
    if !p.abilities.iter().any(|k| k == key) {
        return format!("умение «{}» не изучено", a.name);
    }
    if is_fusion(key) {
        return "слитое умение нельзя слить ещё раз".into();
    }
    if !fusable_kind(&a.kind) {
        return format!("умение «{}» не сливается", a.name);
    }
    if d.ability_class(key).is_none() {
        return format!("умение «{}» не принадлежит ни одному классу", a.name);
    }
    String::new()
}

/// Explains why two abilities cannot be fused ("" = they can).
pub fn can_fuse(p: &PlayerState, a: &str, b: &str) -> String {
    let d = db();
    if p.level < FUSION_LEVEL {
        return format!("слияние доступно с {FUSION_LEVEL}-го уровня");
    }
    let (have, max) = (fusions_of(p).len(), max_fusions(p.level));
    if have >= max {
        let next = (max as i32 + 1) * FUSION_LEVEL;
        return format!(
            "все слоты слияния заняты ({have} из {max}); новый откроется на {next}-м уровне"
        );
    }
    if a == b {
        return "выберите два разных умения".into();
    }
    for k in [a, b] {
        let why = can_fuse_one(p, k);
        if !why.is_empty() {
            return why;
        }
    }
    let (ca, cb) = (d.ability_class(a), d.ability_class(b));
    if ca == cb {
        let name = ca
            .and_then(|c| d.class(c))
            .map_or_else(String::new, |c| c.name.clone());
        return format!("оба умения из класса «{name}» — нужны умения разных классов");
    }
    let (ea, eb) = (&d.ability(a).unwrap().equip, &d.ability(b).unwrap().equip);
    if gear_clash(ea, eb) {
        return format!(
            "несовместимое снаряжение: {} и {}",
            gear_need_name(ea),
            gear_need_name(eb)
        );
    }
    String::new()
}

impl Game {
    /// Melts two abilities into one; it takes the place of the first on the
    /// hotbar.
    pub(crate) fn fuse(&mut self, id: Id, a: &str, b: &str) {
        let why = can_fuse(self.ents[&id].p(), a, b);
        if !why.is_empty() {
            self.log(id, "#ff8080", format!("Слияние невозможно: {why}."));
            return;
        }
        let key = fusion_key(a, b);
        let Some(def) = db().ability(&key) else {
            return;
        };
        let now = self.now;
        let e = self.ents.get_mut(&id).unwrap();
        // the fusion waits for what its parts still wait for
        let ready = [a, b]
            .iter()
            .map(|k| e.cooldowns.get(*k).copied().unwrap_or(0.0))
            .fold(now, f64::max);
        e.cooldowns.insert(key.clone(), ready);
        let p = e.pm();
        let slot = p
            .hotbar
            .iter()
            .position(|h| h == a)
            .or_else(|| p.hotbar.iter().position(|h| h == b));
        for h in p.hotbar.iter_mut() {
            if h == a || h == b {
                h.clear();
            }
        }
        p.abilities.retain(|k| k != a && k != b);
        p.abilities.push(key.clone());
        if let Some(i) = slot.or_else(|| p.hotbar.iter().position(String::is_empty)) {
            p.hotbar[i] = key.clone();
        }
        p.dirty = true;
        let (level, pos) = (e.level.clone(), e.pos);
        let (name, color) = (def.name.clone(), def.color.clone());
        self.log(
            id,
            "#ffd24a",
            format!("Умения слились в одно: «{name}». Разделить их можно в окне слияния (U)."),
        );
        self.fx(&level, pos, "Слияние!", '\0', &color, 1500);
        self.fx_spell(&level, def, FX_CAST, pos, pos, 1.5);
    }

    /// Splits a fusion back into its two abilities.
    pub(crate) fn unfuse(&mut self, id: Id, key: &str) {
        let Some((a, b)) = fusion_parts(key) else {
            return;
        };
        let now = self.now;
        let e = self.ents.get_mut(&id).unwrap();
        if !e.p().abilities.iter().any(|k| k == key) {
            return;
        }
        // the parts wait for what the fusion still waits for
        let ready = e.cooldowns.remove(key).unwrap_or(0.0).max(now);
        for k in [a, b] {
            let cd = e.cooldowns.entry(k.to_string()).or_insert(0.0);
            *cd = cd.max(ready);
        }
        let p = e.pm();
        let slot = p.hotbar.iter().position(|h| h == key);
        p.abilities.retain(|k| k != key);
        if let Some(i) = slot {
            p.hotbar[i].clear();
        }
        for k in [a, b] {
            if !p.abilities.iter().any(|x| x == k) {
                p.abilities.push(k.to_string());
            }
        }
        // the first part takes the fusion's place, the second a free slot
        match slot {
            Some(i) => p.hotbar[i] = a.to_string(),
            None => {
                if let Some(h) = p.hotbar.iter_mut().find(|h| h.is_empty()) {
                    *h = a.to_string();
                }
            }
        }
        if let Some(h) = p.hotbar.iter_mut().find(|h| h.is_empty()) {
            *h = b.to_string();
        }
        p.dirty = true;
        let d = db();
        let name = |k: &str| {
            d.ability(k)
                .map_or_else(|| k.to_string(), |x| x.name.clone())
        };
        self.log(
            id,
            "#c0a0ff",
            format!(
                "Слияние распалось: «{}» и «{}» снова отдельные умения.",
                name(a),
                name(b)
            ),
        );
    }
}

/// Casts a fused ability: its parts one after another (a dash first), each
/// at its own target. The price (fizzle, backlash, weakness) is paid in
/// use_ability, as by any ability that has one.
pub(crate) fn cast_fusion(g: &mut Game, c: Id, a: &AbilityDef, target: Option<Id>) -> bool {
    let mut any = false;
    for part in &a.parts {
        if !g.alive(c) {
            break;
        }
        if part.kind == FUSION_KIND {
            continue;
        }
        // every part looks for its own target: a blow after a dash finds
        // the enemy next to the new place
        let own = match g.aimed(c) {
            Some(aim) => g.aim_target(c, part, aim).or_else(|| {
                (part.kind != "projectile")
                    .then(|| g.auto_target(c, part))
                    .flatten()
            }),
            None => g.auto_target(c, part),
        };
        let t = own.or(target.filter(|t| g.ents.contains_key(t)));
        any |= combat::cast(g, c, part, t);
    }
    any
}

impl Game {
    /// Rolls whether an unstable ability fizzles; a fizzle is announced.
    pub(crate) fn fizzles(&mut self, c: Id, a: &AbilityDef) -> bool {
        if a.fizzle <= 0.0 || !self.chance(a.fizzle) {
            return false;
        }
        let ce = &self.ents[&c];
        let (level, pos) = (ce.level.clone(), ce.pos);
        self.fx(&level, pos, "срыв!", '\0', "#b08aff", 900);
        self.log(
            c,
            "#b08aff",
            format!("«{}» срывается: сила рассеялась впустую.", a.name),
        );
        true
    }

    /// What a cast costs besides mana: a backlash (it hurts but never
    /// kills) and a weakness on the caster.
    pub(crate) fn pay_price(&mut self, c: Id, a: &AbilityDef) {
        let Some(e) = self.ents.get_mut(&c) else {
            return;
        };
        let god = e.player.as_ref().is_some_and(|p| p.god);
        if a.backlash > 0.0 && !god && e.alive() {
            let lost = (e.max_hp * a.backlash / 100.0).min(e.hp - 1.0).max(0.0);
            e.hp -= lost;
            if let Some(p) = e.player.as_mut() {
                p.dirty = true;
            }
            let (level, pos) = (e.level.clone(), e.pos);
            if lost >= 1.0 {
                self.fx(
                    &level,
                    pos,
                    &format!("-{} отдача", lost as i32),
                    '\0',
                    "#ff6a8a",
                    900,
                );
            }
        }
        if let Some(b) = &a.drawback {
            self.apply_buff(c, b, c);
        }
    }
}
