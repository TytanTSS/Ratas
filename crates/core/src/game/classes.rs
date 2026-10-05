//! Classes, subclasses, multiclassing and secret unlocks.

use super::*;
use crate::content::SkillDef;

/// The character level needed to start a second class.
pub const MULTICLASS_LEVEL: i32 = 5;

/// The number of skill points invested in a class (its base branch and its
/// subclass branch; hidden skills do not count).
pub fn class_level(p: &PlayerState, class: &str) -> i32 {
    let d = db();
    p.skills
        .iter()
        .filter_map(|(k, r)| {
            d.skill(k)
                .and_then(|s| d.branch(&s.branch))
                .filter(|b| b.class == class && !b.hidden)
                .map(|_| *r)
        })
        .sum()
}

impl PlayerState {
    /// Whether the character develops a class.
    pub fn has_class(&self, class: &str) -> bool {
        self.classes.iter().any(|c| c == class) || (self.classes.is_empty() && self.class == class)
    }
    /// Whether a secret class, subclass or skill has been opened.
    pub fn unlocked(&self, kind: &str, key: &str) -> bool {
        let k = format!("{kind}:{key}");
        self.unlocks.contains(&k)
    }
    pub fn skill(&self, key: &str) -> i32 {
        self.skills.get(key).copied().unwrap_or(0)
    }
}

/// Explains why a skill cannot be learned ("" = it can).
pub fn can_learn(p: &PlayerState, sd: Option<&SkillDef>) -> String {
    let d = db();
    let Some(sd) = sd else {
        return "нет такого навыка".into();
    };
    if p.skill(&sd.key) >= sd.max_rank {
        return "максимальный ранг".into();
    }
    if let Some(b) = d.branch(&sd.branch) {
        if !sd.deed.is_empty() {
            return format!(
                "открывается деянием: {}",
                lower(&deed_text(&sd.deed, sd.deed_count))
            );
        }
        if b.secret {
            return "этому учат лишь уникальные мастера мира".into();
        }
        if !b.class.is_empty() && !p.has_class(&b.class) {
            let name = d
                .class(&b.class)
                .map(|c| c.name.clone())
                .unwrap_or(b.class.clone());
            return format!("класс «{name}» не начат");
        }
        if !b.subclass.is_empty() && p.subclasses.get(&b.class) != Some(&b.subclass) {
            let name = d
                .subclass(&b.subclass)
                .map(|s| s.name.clone())
                .unwrap_or(b.subclass.clone());
            return format!("нужен подкласс «{name}»");
        }
    }
    if p.level < sd.level {
        return format!("нужен уровень персонажа {}", sd.level);
    }
    for r in &sd.requires {
        if p.skill(r) < 1 {
            let name = d.skill(r).map(|s| s.name.clone()).unwrap_or(r.clone());
            return format!("требуется: {name}");
        }
    }
    if p.skill_points <= 0 {
        return "нет очков навыков".into();
    }
    String::new()
}

/// Explains why a class cannot be started ("" = it can).
pub fn can_start_class(p: &PlayerState, class: &str) -> String {
    match db().class(class) {
        None => "нет такого класса".into(),
        Some(_) if p.has_class(class) => "класс уже начат".into(),
        Some(c) if c.secret && !p.unlocked("class", class) => {
            "секретный класс: его открывает задание уникального персонажа".into()
        }
        Some(_) if p.level < MULTICLASS_LEVEL => {
            format!("второй класс можно начать с {MULTICLASS_LEVEL}-го уровня")
        }
        _ => String::new(),
    }
}

/// Explains why a subclass cannot be chosen ("" = it can).
pub fn can_choose_subclass(p: &PlayerState, key: &str) -> String {
    let Some(sc) = db().subclass(key) else {
        return "нет такого подкласса".into();
    };
    if !p.has_class(&sc.class) {
        return "класс не начат".into();
    }
    if p.subclasses
        .get(&sc.class)
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        return "подкласс этого класса уже выбран".into();
    }
    if sc.secret && !p.unlocked("subclass", key) {
        return "секретный подкласс: его открывает задание уникального персонажа".into();
    }
    if class_level(p, &sc.class) < sc.level {
        return format!("нужен {}-й уровень класса", sc.level);
    }
    String::new()
}

impl Game {
    pub(crate) fn start_class(&mut self, id: Id, class: &str) {
        let why = can_start_class(self.ents[&id].p(), class);
        if !why.is_empty() {
            self.log(id, "#ff8080", format!("Нельзя начать класс: {why}."));
            return;
        }
        let c = db().class(class).unwrap();
        let e = self.ents.get_mut(&id).unwrap();
        e.pm().classes.push(class.into());
        for a in &c.abilities {
            player::unlock_ability_on(e, a);
        }
        e.recalc();
        e.pm().dirty = true;
        let (level, pos) = (e.level.clone(), e.pos);
        self.log(
            id,
            "#80ffff",
            format!(
                "Вы начинаете путь класса «{}». Его навыки — в окне K.",
                c.name
            ),
        );
        self.fx(&level, pos, &format!("{}!", c.name), '\0', &c.color, 1500);
    }

    pub(crate) fn choose_subclass(&mut self, id: Id, key: &str) {
        let why = can_choose_subclass(self.ents[&id].p(), key);
        if !why.is_empty() {
            self.log(id, "#ff8080", format!("Нельзя выбрать подкласс: {why}."));
            return;
        }
        let sc = db().subclass(key).unwrap();
        let e = self.ents.get_mut(&id).unwrap();
        e.pm().subclasses.insert(sc.class.clone(), key.into());
        e.pm().dirty = true;
        let (level, pos) = (e.level.clone(), e.pos);
        self.log(
            id,
            "#ffd24a",
            format!("Ваш путь: {}! Его навыки открыты в окне K.", sc.name),
        );
        self.fx(&level, pos, &format!("{}!", sc.name), '\0', &sc.color, 1800);
    }

    /// Opens a secret class, subclass or skill, or gives an artifact.
    pub(crate) fn unlock(&mut self, id: Id, reward: &str) -> String {
        let d = db();
        let (kind, key) = reward.split_once(':').unwrap_or((reward, ""));
        match kind {
            "item" => {
                let st = ItemStack::new(key);
                if !self.add_item(id, st.clone()) {
                    let cell = self.ents[&id].cell();
                    let level = self.ents[&id].level.clone();
                    self.drop_item(&level, cell, st.clone());
                }
                self.log(id, "#ff80ff", format!("Артефакт: {}!", st.name()));
                st.name()
            }
            "class" | "subclass" => {
                let p = self.ents.get_mut(&id).unwrap().pm();
                if !p.unlocks.iter().any(|u| u == reward) {
                    p.unlocks.push(reward.into());
                }
                p.dirty = true;
                let mut name = key.to_string();
                if kind == "class" {
                    if let Some(c) = d.class(key) {
                        name = format!("класс «{}»", c.name);
                    }
                    self.log(id, "#ff80ff", format!("Открыт секретный {name}! Начать его можно в окне навыков (K) с {MULTICLASS_LEVEL}-го уровня."));
                } else {
                    if let Some(sc) = d.subclass(key) {
                        name = format!("подкласс «{}»", sc.name);
                        if !p.has_class(&sc.class) {
                            p.classes.push(sc.class.clone());
                        }
                    }
                    self.log(
                        id,
                        "#ff80ff",
                        format!("Открыт секретный {name}! Выбрать его можно в окне навыков (K)."),
                    );
                }
                name
            }
            "skill" => {
                let Some(sd) = d.skill(key) else {
                    return key.into();
                };
                let e = self.ents.get_mut(&id).unwrap();
                let p = e.pm();
                if p.skill(key) < sd.max_rank {
                    p.skills.insert(key.into(), sd.max_rank);
                }
                p.dirty = true;
                if !sd.grants.is_empty() {
                    player::unlock_ability_on(e, &sd.grants);
                }
                e.recalc();
                self.log(id, "#ff80ff", format!("Тайное знание: {}!", sd.name));
                format!("навык «{}»", sd.name)
            }
            _ => reward.into(),
        }
    }
}
