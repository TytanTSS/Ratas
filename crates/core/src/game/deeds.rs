//! Deeds are counters of what a hero has done: kills of every kind, damage of
//! every type, crits, dodges, steps, deaths and more. A hidden skill (a skill
//! with a deed) opens by itself once its deed is done, for a hero of its class.
//!
//! Deed keys: kills, kill:<monster>, theme:<dungeon theme or biome>, bosses,
//! elites, night (kills at night), lowhp (kills below 25% health), deaths,
//! gold, crits, dmg:<damage type>, heal, dodge, block, casts, summons,
//! potions, steps, depth (deepest dungeon floor), uniques, quests, landmarks,
//! level, classlevel (of the skill's class).

use super::*;
use crate::content::{MonsterDef, SkillDef};

/// Describes a deed for players: "Убить 100: нежить склепов".
pub fn deed_text(deed: &str, count: i32) -> String {
    let (kind, arg) = deed.split_once(':').unwrap_or((deed, ""));
    match kind {
        "kills" => format!("Убить {count} врагов"),
        "kill" => {
            let name = db()
                .monster(arg)
                .map(|m| m.name.clone())
                .unwrap_or(arg.into());
            format!("Убить {count}: {name}")
        }
        "theme" => format!("Убить {count}: {}", theme_foes(arg)),
        "bosses" => format!("Сразить {count} боссов подземелий"),
        "elites" => format!("Убить {count} элитных врагов или чемпионов"),
        "night" => format!("Убить {count} врагов ночью"),
        "lowhp" => format!("Убить {count} врагов, когда у вас меньше четверти здоровья"),
        "deaths" => format!("Погибнуть {count} раз и вернуться"),
        "gold" => format!("Собрать {count} золота"),
        "crits" => format!("Нанести {count} критических ударов"),
        "dmg" => format!("Нанести {count} урона: {}", damage_type_name(arg)),
        "heal" => format!("Восстановить умениями {count} здоровья"),
        "dodge" => format!("Увернуться от {count} ударов"),
        "block" => format!("Заблокировать {count} ударов"),
        "casts" => format!("Применить умения {count} раз"),
        "summons" => format!("Призвать союзников {count} раз"),
        "potions" => format!("Выпить {count} зелий"),
        "steps" => format!("Пройти {count} шагов"),
        "depth" => format!("Спуститься на {count}-й ярус подземелья"),
        "uniques" => format!("Выполнить {count} заданий уникальных персонажей"),
        "quests" => format!("Выполнить {count} поручений"),
        "landmarks" => format!("Найти {count} достопримечательностей"),
        "level" => format!("Достичь {count} уровня"),
        "classlevel" => format!("Достичь {count} уровня класса"),
        _ => format!("{deed} ×{count}"),
    }
}

/// Names the creatures of a dungeon theme or a biome.
pub fn theme_foes(theme: &str) -> String {
    match theme {
        "crypt" => "нежить склепов",
        "cave" => "обитатели пещер",
        "ice" => "твари льдов",
        "volcano" | "ash" => "огненные твари",
        "temple" | "desert" => "твари песков и гробниц",
        "fortress" => "демоны и культисты",
        "forest" => "лесные звери",
        "plains" | "hills" => "звери и разбойники равнин",
        "swamp" => "болотные твари",
        "tundra" => "твари севера",
        "cursed" => "порождения проклятых земель",
        "sand" => "твари побережий",
        _ => theme,
    }
    .to_string()
}

/// Whether a deed key is valid (for content validation).
pub fn deed_known(deed: &str) -> bool {
    let (kind, arg) = deed.split_once(':').unwrap_or((deed, ""));
    match kind {
        "kills" | "bosses" | "elites" | "night" | "lowhp" | "deaths" | "gold" | "crits"
        | "heal" | "dodge" | "block" | "casts" | "summons" | "potions" | "steps" | "depth"
        | "uniques" | "quests" | "landmarks" | "level" | "classlevel" => arg.is_empty(),
        "kill" => db().monster(arg).is_some(),
        "theme" => !arg.is_empty(),
        "dmg" => db().damage_type(arg).is_some(),
        _ => false,
    }
}

/// How far a hero is in the deed of a hidden skill.
pub fn deed_progress(p: &PlayerState, sd: &SkillDef) -> i32 {
    match sd.deed.as_str() {
        "level" => p.level,
        "classlevel" => db()
            .branch(&sd.branch)
            .map(|b| class_level(p, &b.class))
            .unwrap_or(0),
        "landmarks" => p.found.len() as i32,
        d => p.deeds.get(d).copied().unwrap_or(0),
    }
}

/// Whether a hidden skill belongs to a hero: its class is one the hero
/// develops (or it belongs to no class).
pub fn hidden_for(p: &PlayerState, sd: &SkillDef) -> bool {
    if sd.deed.is_empty() {
        return false;
    }
    db().branch(&sd.branch)
        .map(|b| b.class.is_empty() || p.has_class(&b.class))
        .unwrap_or(false)
}

impl Game {
    /// Adds to a counter of a hero (projectiles and summons count for their owner).
    pub(crate) fn deed(&mut self, id: Id, key: &str, n: i32) {
        let mut id = id;
        if let Some(e) = self.ents.get(&id) {
            if e.kind == Kind::Projectile {
                match e.proj.as_ref() {
                    Some(p) => id = p.owner,
                    None => return,
                }
            }
        }
        if n <= 0 {
            return;
        }
        let Some(p) = self.ents.get_mut(&id).and_then(|e| e.player.as_mut()) else {
            return;
        };
        *p.deeds.entry(key.into()).or_insert(0) += n;
        p.deed_check = true;
    }

    /// Adds a fractional amount (steps walked).
    pub(crate) fn deed_f(&mut self, id: Id, key: &str, v: f32) {
        let Some(p) = self.ents.get_mut(&id).and_then(|e| e.player.as_mut()) else {
            return;
        };
        p.step_acc += v;
        if p.step_acc >= 1.0 {
            let n = p.step_acc.floor();
            p.step_acc -= n;
            self.deed(id, key, n as i32);
        }
    }

    /// Raises a counter that keeps the best value (the deepest floor).
    pub(crate) fn deed_max(&mut self, id: Id, key: &str, v: i32) {
        let Some(p) = self.ents.get_mut(&id).and_then(|e| e.player.as_mut()) else {
            return;
        };
        if v > p.deeds.get(key).copied().unwrap_or(0) {
            p.deeds.insert(key.into(), v);
            p.deed_check = true;
        }
    }

    /// Opens the hidden skills whose deeds are done.
    pub(crate) fn check_deeds(&mut self, id: Id) {
        self.ents.get_mut(&id).unwrap().pm().deed_check = false;
        for sd in &db().b.skills {
            let p = self.ents[&id].p();
            if !hidden_for(p, sd) || p.skill(&sd.key) > 0 || deed_progress(p, sd) < sd.deed_count {
                continue;
            }
            let e = self.ents.get_mut(&id).unwrap();
            e.pm().skills.insert(sd.key.clone(), sd.max_rank);
            if !sd.grants.is_empty() {
                player::unlock_ability_on(e, &sd.grants);
            }
            e.recalc();
            e.pm().dirty = true;
            let (level, pos) = (e.level.clone(), e.pos);
            self.log(
                id,
                "#ff80ff",
                format!(
                    "Скрытый навык открыт: {}! ({})",
                    sd.name,
                    deed_text(&sd.deed, sd.deed_count)
                ),
            );
            self.fx(&level, pos, &format!("{}!", sd.name), '\0', "#ff80ff", 2200);
        }
    }

    /// Counts a slain monster for a hero who shared in the kill; slayer is
    /// the one who struck the blow.
    pub(crate) fn kill_deeds(&mut self, p: Id, m: &Entity, def: Option<&MonsterDef>, slayer: bool) {
        let ms = m.monster.as_ref().unwrap();
        self.deed(p, "kills", 1);
        self.deed(p, &format!("kill:{}", ms.def), 1);
        if let Some(def) = def {
            for t in &def.themes {
                self.deed(p, &format!("theme:{t}"), 1);
            }
            if def.boss {
                self.deed(p, "bosses", 1);
            }
            if def.elite || !ms.champion.is_empty() {
                self.deed(p, "elites", 1);
            }
        }
        if self.levels.get(&m.level).map(|l| l.lit).unwrap_or(false) && self.is_night() {
            self.deed(p, "night", 1);
        }
        let pe = &self.ents[&p];
        if slayer && pe.max_hp > 0.0 && pe.hp < pe.max_hp / 4.0 {
            self.deed(p, "lowhp", 1);
        }
    }
}
