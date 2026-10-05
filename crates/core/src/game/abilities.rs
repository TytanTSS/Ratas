//! Special abilities: taunts, summons and illusions, mimicry, resurrection.

use super::*;
use crate::content::AbilityDef;
use crate::world::DIRS8;

impl Game {
    /// Stealthed enemies are invisible unless right next to you.
    pub(crate) fn can_see(&self, c: Id, o: Id) -> bool {
        let (ce, oe) = (&self.ents[&c], &self.ents[&o]);
        !oe.stats.stealthed || ce.gap(oe) <= 0.6 || self.friendly(c, o)
    }

    pub(crate) fn copy_power(&self, c: Id) -> f64 {
        let e = &self.ents[&c];
        0.8 * (1.0 + e.stats.mimic_pct / 100.0) * monster_scale(e.p().copied_lvl.max(1))
    }

    pub(crate) fn cast_copy(&mut self, c: Id, target: Option<Id>, k: f64) -> bool {
        let copied = self.ents[&c].player.as_ref().map(|p| p.copied.clone()).unwrap_or_default();
        if copied.is_empty() {
            self.log(c, "#808080", "Сначала скопируйте умение врага.".into());
            return false;
        }
        let Some(orig) = db().ability(&copied) else { return false };
        let mut clone = orig.clone();
        let pw = self.copy_power(c) * k;
        clone.damage = [orig.damage[0] * pw, orig.damage[1] * pw];
        if !copyable(&clone) {
            return false;
        }
        let target = target.or_else(|| self.auto_target(c, &clone));
        combat::cast(self, c, &clone, target)
    }
}

pub(crate) fn copyable(a: &AbilityDef) -> bool {
    !matches!(a.kind.as_str(), "mimic" | "copied" | "echo" | "revive" | "death_sentence")
}

/// Forces enemies around to attack the caster.
pub(crate) fn cast_taunt(g: &mut Game, c: Id, a: &AbilityDef) -> bool {
    let r = a.radius.max(1) as f32;
    let ce = &g.ents[&c];
    let (level, pos) = (ce.level.clone(), ce.pos);
    let now = g.now;
    for o in g.on_level(&level) {
        if !g.hostile(c, o) || g.ents[&o].pos.dist(pos) > r + 0.5 {
            continue;
        }
        if let Some(b) = &a.on_hit {
            g.apply_buff(o, b, c);
        }
        let oe = g.ents.get_mut(&o).unwrap();
        if let Some(m) = oe.monster.as_mut() {
            m.target = c;
            m.last_seen = pos;
            m.last_seen_at = now;
        }
        let op = oe.pos;
        g.fx(&level, op, "!", '\0', &a.color, 800);
    }
    if let Some(b) = &a.buff {
        g.apply_buff(c, b, c);
    }
    g.fx(&level, pos, &format!("{}!", a.name), '\0', &a.color, 1000);
    true
}

/// Calls allies (summon) or illusions (decoy) next to the caster.
pub(crate) fn cast_summon(g: &mut Game, c: Id, a: &AbilityDef) -> bool {
    let Some(def) = db().monster(&a.summon) else { return false };
    let ce = &g.ents[&c];
    let (level, pos, cell, faction) = (ce.level.clone(), ce.pos, ce.cell(), ce.faction);
    let (cname, cglyph, ccolor) = (ce.name.clone(), ce.glyph.clone(), ce.color.clone());
    let lvl = ce.player.as_ref().map(|p| p.level).or(ce.monster.as_ref().map(|m| m.lvl)).unwrap_or(1);
    // a new call replaces the old summons of the same kind
    for o in g.on_level(&level) {
        let mine = g.ents.get(&o).map(|oe| oe.owner == c && oe.monster.as_ref().map(|m| m.def == def.key).unwrap_or(false)).unwrap_or(false);
        if mine {
            g.remove(o);
        }
    }
    let now = g.now;
    let mut made = Vec::new();
    for i in 0..a.count.max(1) {
        let p = g.free_spot(&level, cell.add(DIRS8[i as usize % 8]));
        if p.center().dist(pos) > 4.5 {
            break;
        }
        let m = g.new_monster(def, &level, p, lvl);
        let me = g.ents.get_mut(&m).unwrap();
        me.faction = faction;
        me.owner = c;
        me.expires = now + a.duration.max(1000) as f64;
        me.monster.as_mut().unwrap().persistent = true;
        if a.kind == "decoy" {
            me.name = format!("Иллюзия: {cname}");
            me.glyph = cglyph.clone();
            me.color = ccolor.clone();
            me.hp = 20.0 + lvl as f64 * 8.0;
            me.max_hp = me.hp;
            me.recalc();
        } else {
            me.name = format!("{} ({cname})", def.name);
        }
        let mp = me.pos;
        g.fx(&level, mp, "", '*', &a.color, 400);
        made.push(m);
    }
    if made.is_empty() {
        return false;
    }
    g.index_levels();
    if a.kind == "decoy" {
        // enemies who were after the caster turn to the illusions
        for o in g.on_level(&level) {
            let turn = g.ents.get(&o).and_then(|oe| oe.monster.as_ref().map(|m| (m.target, oe.pos))).map(|(t, op)| (t == c || t == 0) && op.dist(pos) <= 8.0).unwrap_or(false);
            if turn && g.hostile(o, c) {
                let pick = made[g.rng.usize_n(made.len())];
                g.ents.get_mut(&o).unwrap().monster.as_mut().unwrap().target = pick;
            }
        }
    }
    true
}

/// Copies an ability of the nearest enemy.
pub(crate) fn cast_mimic(g: &mut Game, c: Id, target: Option<Id>) -> bool {
    let Some(t) = target.filter(|t| g.ents.contains_key(t)) else {
        g.log(c, "#808080", "Некого копировать.".into());
        return false;
    };
    if g.ents[&c].player.is_none() {
        return false;
    }
    let te = &g.ents[&t];
    let (mut key, mut lvl) = (String::new(), 1);
    if let Some(m) = &te.monster {
        if let Some(def) = db().monster(&m.def) {
            let mut all = vec![def.ability.clone()];
            all.extend(def.abilities.iter().cloned());
            for k in all {
                if db().ability(&k).map(copyable).unwrap_or(false) {
                    key = k;
                    break;
                }
            }
        }
        lvl = m.lvl;
    } else if let Some(p) = &te.player {
        let opts: Vec<String> = p.abilities.iter().filter(|k| db().ability(k).map(copyable).unwrap_or(false)).cloned().collect();
        if !opts.is_empty() {
            key = opts[g.rng.usize_n(opts.len())].clone();
        }
        lvl = p.level;
    }
    let (tname, tlevel, tpos) = (te.name.clone(), te.level.clone(), te.pos);
    if key.is_empty() {
        g.log(c, "#808080", format!("У {tname} нет умения, которое можно скопировать."));
        return false;
    }
    let now = g.now;
    let e = g.ents.get_mut(&c).unwrap();
    let plvl = e.p().level;
    let p = e.pm();
    p.copied = key.clone();
    p.copied_lvl = lvl.min(plvl);
    p.copied_end = now + 120000.0;
    p.dirty = true;
    player::unlock_ability_on(e, "copied");
    let (level, pos) = (e.level.clone(), e.pos);
    let name = db().ability(&key).unwrap().name.clone();
    g.log(c, "#ff8ad8", format!("Скопировано умение: {name} (на 2 минуты, кнопка «Копия»)."));
    g.fx(&level, pos, &format!("Копия: {name}"), '\0', "#ff8ad8", 1500);
    g.fx(&tlevel, tpos, "", '*', "#ff8ad8", 400);
    true
}

/// Brings back a fallen ally next to the caster.
pub(crate) fn cast_revive(g: &mut Game, c: Id, a: &AbilityDef) -> bool {
    let r = a.range.max(1) as f32 + 0.5;
    let ce = &g.ents[&c];
    let (level, pos) = (ce.level.clone(), ce.pos);
    for o in g.on_level(&level) {
        let oe = &g.ents[&o];
        if oe.player.is_some() && oe.dead && o != c && oe.pos.dist(pos) <= r && g.revive(o, c) {
            return true;
        }
    }
    g.log(c, "#808080", "Рядом нет павших союзников, которых ещё можно вернуть.".into());
    false
}

/// Kills every creature around weaker than the caster.
pub(crate) fn cast_death_sentence(g: &mut Game, c: Id, a: &AbilityDef) -> bool {
    let ce = &g.ents[&c];
    let lvl = ce.player.as_ref().map(|p| p.level).unwrap_or(1);
    let r = a.radius.max(1) as f32 + 0.5;
    let (level, pos) = (ce.level.clone(), ce.pos);
    g.fx_area(&level, pos, r, glyph_of(a, '%'), &a.color, 260);
    for o in g.on_level(&level) {
        let Some(oe) = g.ents.get(&o) else { continue };
        let Some(m) = &oe.monster else { continue };
        if !g.hostile(c, o) || oe.pos.dist(pos) > r {
            continue;
        }
        let boss = db().monster(&m.def).map(|d| d.boss).unwrap_or(false);
        let op = oe.pos;
        if boss {
            let oe = g.ents.get_mut(&o).unwrap();
            oe.hp -= oe.max_hp * 0.25;
            let dead = oe.hp <= 0.0;
            g.fx(&level, op, "приговор", '\0', &a.color, 1000);
            g.provoke(Some(c), o);
            if dead {
                g.kill(o, Some(c));
            }
        } else if m.lvl < lvl {
            g.fx(&level, op, "смерть", '\0', &a.color, 1000);
            g.ents.get_mut(&o).unwrap().hp = 0.0;
            g.kill(o, Some(c));
        }
    }
    true
}

use super::combat::glyph_of;
