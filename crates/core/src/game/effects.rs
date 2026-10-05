//! Consumable effects, shrines, regions and landmark discovery.

use super::*;
use crate::content::{BuffDef, ItemDef, TileDef};
use crate::world::los;

const DANGER_NAMES: &[&str] = &[
    "спокойный край",
    "здесь бывает опасно",
    "опасные земли",
    "смертельно опасные земли",
];

pub(crate) fn biome_danger(biome: &str) -> i32 {
    crate::gen::region_danger(biome)
}

fn blessing(i: usize) -> BuffDef {
    let (key, name, color, stats): (&str, &str, &str, &[(&str, f64)]) = match i {
        0 => (
            "bless_stone",
            "Благословение камня",
            "#d0d0ff",
            &[("res_all", 15.0), ("armor", 3.0)],
        ),
        1 => (
            "bless_bear",
            "Благословение медведя",
            "#ff8a5a",
            &[("str", 4.0), ("melee_pct", 15.0)],
        ),
        2 => (
            "bless_owl",
            "Благословение совы",
            "#8ab0ff",
            &[("int", 4.0), ("spell_pct", 15.0), ("mp_regen", 1.0)],
        ),
        3 => (
            "bless_lynx",
            "Благословение рыси",
            "#9ae07a",
            &[("dex", 4.0), ("crit", 5.0), ("move_speed", 10.0)],
        ),
        4 => (
            "bless_oak",
            "Благословение дуба",
            "#7ad08a",
            &[("max_hp", 40.0), ("hp_regen", 1.5)],
        ),
        _ => (
            "bless_sun",
            "Благословение солнца",
            "#ffe08a",
            &[("res_shadow", 30.0), ("add_holy", 5.0), ("holy_pct", 20.0)],
        ),
    };
    BuffDef {
        key: key.into(),
        name: name.into(),
        duration_ms: 180000,
        stats: stats.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        color: color.into(),
        ..Default::default()
    }
}

impl Game {
    /// Applies the buff and special effect of a used consumable.
    pub(crate) fn item_effect(&mut self, id: Id, d: &ItemDef) {
        match d.effect.as_str() {
            "cleanse" => self.cleanse(id),
            "return" => {
                let e = &self.ents[&id];
                let (level, pos) = (e.level.clone(), e.pos);
                self.fx(&level, pos, "", '*', "#e0d0a0", 400);
                let start = self.start;
                self.change_level(id, "overworld", start);
                let pos = self.ents[&id].pos;
                self.fx("overworld", pos, "", '*', "#e0d0a0", 600);
            }
            "respec" => self.respec(id),
            "xp" => {
                let lvl = self.ents[&id].p().level;
                self.give_xp(
                    id,
                    ((xp_for_level(lvl) as f64 * d.amount / 100.0) as i32).max(1),
                );
            }
            _ => {}
        }
        if let Some(b) = &d.buff {
            self.apply_buff(id, b, id);
            let e = &self.ents[&id];
            let (level, pos) = (e.level.clone(), e.pos);
            self.fx(&level, pos, &format!("{}!", b.name), '\0', &b.color, 1000);
        }
    }

    pub(crate) fn can_return(&mut self, id: Id) -> bool {
        let e = &self.ents[&id];
        if e.dead {
            return false;
        }
        if e.level == "overworld" && e.pos.dist(self.start.center()) < 6.0 {
            self.log(id, "#808080", "Вы и так у колодца.".into());
            return false;
        }
        true
    }

    /// Removes harmful effects applied by others.
    pub(crate) fn cleanse(&mut self, id: Id) {
        let e = self.ents.get_mut(&id).unwrap();
        let before = e.buffs.len();
        e.buffs.retain(|b| b.source == id || !is_debuff(&b.def));
        let removed = before - e.buffs.len();
        e.recalc();
        if removed > 0 {
            let (level, pos) = (e.level.clone(), e.pos);
            self.fx(&level, pos, "очищение", '\0', "#c0ffc0", 900);
        }
    }

    /// Returns every skill and attribute point, forgets subclasses and
    /// additional classes. Secret skills taught by unique masters stay.
    pub(crate) fn respec(&mut self, id: Id) {
        let d = db();
        let e = self.ents.get_mut(&id).unwrap();
        let p = e.pm();
        let mut kept = BTreeMap::new();
        for (k, r) in std::mem::take(&mut p.skills) {
            let secret = d
                .skill(&k)
                .and_then(|s| d.branch(&s.branch))
                .map(|b| b.secret)
                .unwrap_or(false);
            if secret {
                kept.insert(k, r);
            } else {
                p.skill_points += r;
            }
        }
        p.skills = kept.clone();
        p.subclasses.clear();
        p.classes = vec![p.class.clone()];
        if let Some(c) = d.class(&p.class) {
            for (k, v) in p.attrs.iter_mut() {
                let base = c.attrs.get(k).copied().unwrap_or(0.0);
                if *v > base {
                    p.attr_points += (*v - base).round() as i32;
                    *v = base;
                }
            }
            p.abilities.clear();
            p.hotbar = Default::default();
            for a in &c.abilities {
                player::unlock_ability_on(e, a);
            }
        }
        for k in kept.keys() {
            if let Some(sd) = d.skill(k) {
                if !sd.grants.is_empty() {
                    player::unlock_ability_on(e, &sd.grants);
                }
            }
        }
        e.recalc();
        let p = e.p();
        let (sp, ap) = (p.skill_points, p.attr_points);
        let (level, pos) = (e.level.clone(), e.pos);
        self.log(id, "#c0c0ff", format!("Память очищена: {sp} очк. навыков и {ap} очк. характеристик можно распределить заново (K и C)."));
        self.fx(&level, pos, "Забвение", '\0', "#c0c0ff", 1200);
    }

    pub(crate) fn pray_at_shrine(&mut self, id: Id, level: &str, c: Pos, def: &TileDef) {
        let b = blessing(self.rng.usize_n(6));
        self.apply_buff(id, &b, id);
        let e = self.ents.get_mut(&id).unwrap();
        e.hp = e.max_hp;
        e.mp = e.max_mp;
        let pos = e.pos;
        if !def.becomes.is_empty() {
            let t = db().tile_id(&def.becomes);
            self.set_tile(level, c.x, c.y, t);
        }
        self.log(
            id,
            "#ffe08a",
            format!(
                "Вы молитесь у святилища. {} на 3 минуты! Силы восстановлены.",
                b.name
            ),
        );
        self.fx(level, pos, &b.name, '\0', &b.color, 1500);
        self.fx(level, c.center(), "", '*', "#ffe08a", 600);
    }

    /// Announces regions and discovers landmarks around a player.
    pub(crate) fn check_surroundings(&mut self, id: Id) {
        let e = &self.ents[&id];
        if e.level != "overworld" {
            self.ents.get_mut(&id).unwrap().pm().region = 0;
            return;
        }
        let cell = e.cell();
        let r = self.region_index(cell);
        if r != 0 && r != e.p().region {
            self.ents.get_mut(&id).unwrap().pm().region = r;
            let reg = &self.regions[r - 1];
            let text = format!(
                "Регион: {} — {}.",
                reg.name,
                DANGER_NAMES[(reg.danger as usize).min(DANGER_NAMES.len() - 1)]
            );
            self.log(id, "#e0d0a0", text);
        }
        let l = &self.levels["overworld"];
        let mut found = Vec::new();
        let p = self.ents[&id].p();
        for (i, lm) in self.landmarks.iter().enumerate() {
            if p.found.contains(&i) || cell.dist(lm.pos) > 7 || !los(l, cell, lm.pos) {
                continue;
            }
            found.push(i);
        }
        for i in found {
            let e = self.ents.get_mut(&id).unwrap();
            let p = e.pm();
            p.found.push(i);
            p.deed_check = true;
            let xp = 10 + 5 * p.level;
            let (level, pos) = (e.level.clone(), e.pos);
            let name = self.landmarks[i].name.clone();
            self.log(id, "#ffe08a", format!("Открытие: {name} (+{xp} опыта)."));
            self.fx(&level, pos, "Открытие!", '\0', "#ffe08a", 1200);
            self.give_xp(id, xp);
        }
    }

    /// The 1-based region of an overworld cell (0 = none).
    pub(crate) fn region_index(&self, p: Pos) -> usize {
        let Some(l) = self.levels.get("overworld") else {
            return 0;
        };
        if self.region_map.len() != (l.w * l.h) as usize || !l.inside(p.x, p.y) {
            return 0;
        }
        let r = self.region_map[l.idx(p)] as usize;
        if r > self.regions.len() {
            0
        } else {
            r
        }
    }

    /// The name of the overworld region at p ("" outside).
    pub fn region_name(&self, p: Pos) -> String {
        match self.region_index(p) {
            0 => String::new(),
            r => self.regions[r - 1].name.clone(),
        }
    }
}
