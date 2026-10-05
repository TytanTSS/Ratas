//! Unique characters of the world and their quests.

use super::dialogue::compass_ru;
use super::spawn::pick_monster;
use super::*;
use crate::content::UniqueDef;
use crate::world::Vec2;

/// How many unique characters a world gets.
pub const MAX_UNIQUES: usize = 40;

/// Describes the reward of a unique quest.
pub fn reward_name(reward: &str) -> String {
    let d = db();
    let (kind, key) = reward.split_once(':').unwrap_or((reward, ""));
    match kind {
        "item" => d.item(key).map(|i| format!("артефакт «{}»", i.name)),
        "class" => d
            .class(key)
            .map(|c| format!("секретный класс «{}»", c.name)),
        "subclass" => d
            .subclass(key)
            .map(|s| format!("секретный подкласс «{}»", s.name)),
        "skill" => d.skill(key).map(|s| format!("тайный навык «{}»", s.name)),
        _ => None,
    }
    .unwrap_or_else(|| reward.to_string())
}

pub(crate) fn sources_text(src: &[String]) -> String {
    src.iter()
        .map(|s| {
            match s.as_str() {
                "desert" => "пустыня",
                "ash" => "пепельные пустоши",
                "volcano" => "огненные недра",
                "temple" => "гробницы",
                "cursed" => "проклятые земли",
                "fortress" => "цитадели",
                "crypt" => "склепы",
                "swamp" => "болота",
                "ice" => "ледяные пещеры",
                "tundra" => "тундра",
                "hills" => "холмы",
                "cave" => "пещеры",
                "forest" => "леса",
                "plains" => "равнины",
                other => other,
            }
            .to_string()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Describes a quest for the journal.
pub fn quest_text(q: &Quest) -> String {
    let d = db();
    let name = d
        .monster(&q.monster)
        .map(|m| m.name.clone())
        .unwrap_or(q.monster.clone());
    match q.kind.as_str() {
        "slay" => format!("сразить чемпиона «{}»", q.where_),
        "boss" if !q.where_.is_empty() => format!("сразить: {name} ({})", q.where_),
        "boss" => format!("сразить: {name}"),
        "relics" => {
            let item = d
                .item(&q.item)
                .map(|i| i.name.clone())
                .unwrap_or(q.item.clone());
            format!("собрать: {item} ×{} ({})", q.need, sources_text(&q.sources))
        }
        _ => format!("убить: {name} ×{}", q.need),
    }
}

impl Game {
    /// Settles unique characters in the wild. Those who teach secret classes
    /// and subclasses come first, the rest are picked at random.
    pub(crate) fn place_uniques(&mut self, r: &mut Rng) {
        let all = &db().b.uniques;
        let (mut first, mut rest): (Vec<usize>, Vec<usize>) = (0..all.len()).partition(|&i| {
            all[i].reward.starts_with("class:") || all[i].reward.starts_with("subclass:")
        });
        r.shuffle(&mut rest);
        first.extend(rest);
        let mut placed: Vec<Pos> = Vec::new();
        // spread over the whole world: further apart on a bigger one
        let k = {
            let l = &self.levels["overworld"];
            gen::MapScale::of(l.w, l.h).lin
        };
        let gaps = [25.0, 16.0, 10.0].map(|g: f64| (g * k) as i32);
        for i in first {
            if placed.len() >= MAX_UNIQUES {
                break;
            }
            let u = &all[i];
            // their own lands far from the others first; closer and anywhere
            // when the world is crowded
            let mut spot = None;
            for gap in gaps {
                spot = self.unique_spot(r, &u.biomes, &placed, gap);
                if spot.is_some() {
                    break;
                }
            }
            if spot.is_none() {
                spot = self.unique_spot(r, &[], &placed, gaps[2]);
            }
            let Some(p) = spot else { continue };
            placed.push(p);
            let gold = 40 + r.int_n(60);
            self.spawn_unique(u, p, gold);
        }
    }

    /// Puts a unique character on the overworld.
    pub(crate) fn spawn_unique(&mut self, u: &UniqueDef, p: Pos, gold: i32) -> Id {
        self.spawn(Entity {
            kind: Kind::Npc,
            name: format!("{}, {}", u.name, lower(&u.title)),
            glyph: "@".into(),
            color: u.color.clone(),
            level: "overworld".into(),
            pos: p.center(),
            faction: Faction::Neutral,
            hp: 100.0,
            max_hp: 100.0,
            facing: std::f32::consts::FRAC_PI_2,
            npc: Some(Box::new(NpcState {
                role: "unique".into(),
                pname: u.name.clone(),
                home: p.center(),
                unique: u.key.clone(),
                gold,
                ..Default::default()
            })),
            ..Default::default()
        })
    }

    pub(crate) fn unique_spot(
        &self,
        r: &mut Rng,
        biomes: &[String],
        taken: &[Pos],
        gap: i32,
    ) -> Option<Pos> {
        let l = &self.levels["overworld"];
        for _ in 0..4000 {
            let p = Pos::new(4 + r.int_n(l.w - 8), 4 + r.int_n(l.h - 8));
            let def = l.def_at(p);
            if !l.walkable(p.x, p.y)
                || !def.interact.is_empty()
                || def.damage > 0.0
                || self.in_village(p, 10)
                || p.manhattan(self.start) < 35
                || self.cell_taken("overworld", p)
            {
                continue;
            }
            if !biomes.is_empty() && !biomes.contains(&def.biome) {
                continue;
            }
            if taken.iter().all(|o| o.dist(p) >= gap) {
                return Some(p);
            }
        }
        None
    }

    pub(crate) fn unique_quest(&self, p: Id, key: &str) -> Option<usize> {
        self.ents[&p]
            .p()
            .quests
            .iter()
            .position(|q| q.unique == key)
    }

    pub(crate) fn unique_done(&self, p: Id, key: &str) -> bool {
        let k = format!("quest:{key}");
        self.ents[&p].p().unlocks.contains(&k)
    }

    /// Gives the quest of a unique character.
    pub(crate) fn offer_unique_quest(&mut self, p: Id, npc: Id, u: &UniqueDef) -> String {
        let pl = self.ents[&p].p();
        if pl.quests.len() >= 6 {
            return "Сперва разберись со своими делами — у тебя и так полный журнал.".into();
        }
        let lvl = pl.level;
        self.quest_seq += 1;
        let mut q = Quest {
            id: self.quest_seq,
            giver: u.name.clone(),
            giver_id: npc,
            village: lower(&u.title),
            kind: u.quest.clone(),
            unique: u.key.clone(),
            need: 1,
            reward: u.reward.clone(),
            gold: 150 + 20 * lvl,
            xp: xp_for_level(lvl) / 2,
            ..Default::default()
        };
        match u.quest.as_str() {
            "slay" => {
                let near = self.ents[&npc].cell();
                let Some(c) = self.champion_of(u, near, lvl) else {
                    return "Твоя цель ускользнула... Приходи позже.".into();
                };
                let ce = &self.ents[&c];
                q.monster = ce.monster.as_ref().unwrap().def.clone();
                q.at = Some(ce.cell());
                q.where_ = ce.name.clone();
            }
            "boss" => {
                q.monster = u.target.clone();
                if let Some(def) = db().monster(&u.target) {
                    if let Some(e) = self
                        .entrances
                        .iter()
                        .find(|e| def.themes.contains(&e.theme))
                    {
                        q.at = Some(e.pos);
                        q.where_ = e.name.clone();
                    }
                }
            }
            "relics" => {
                q.item = u.target.clone();
                q.need = u.count.max(1);
                q.sources = u.sources.clone();
            }
            _ => {}
        }
        let text = format!(
            "Задание {}: {}. Награда — {}. Журнал — J.",
            u.name,
            quest_text(&q),
            reward_name(&u.reward)
        );
        let pm = self.ents.get_mut(&p).unwrap().pm();
        pm.quests.push(q);
        pm.dirty = true;
        self.log(p, "#ff80ff", text);
        u.offer.clone()
    }

    /// The champion of a slay quest, called into the wild (far from the quest
    /// giver) if there is none.
    pub(crate) fn champion_of(&mut self, u: &UniqueDef, near: Pos, lvl: i32) -> Option<Id> {
        if let Some(&id) = self.champions.get(&u.key) {
            if self.alive(id) {
                return Some(id);
            }
        }
        let def = db().monster(&u.target)?;
        let mut spot = None;
        for tries in 0..3000 {
            let d = 35.0 + self.rng.int_n(55) as f32;
            let a = self.rng.f32() * std::f32::consts::TAU;
            let p = (near.center() + Vec2::from_angle(a) * d).cell();
            let l = &self.levels["overworld"];
            let d2 = l.def_at(p);
            if l.walkable(p.x, p.y)
                && d2.interact.is_empty()
                && d2.damage == 0.0
                && !self.in_village(p, 15)
                && !self.cell_taken("overworld", p)
                && (def.themes.contains(&d2.biome) || tries > 2000)
            {
                spot = Some(p);
                break;
            }
        }
        let spot = spot?;
        let c = self.new_monster(def, "overworld", spot, lvl + 2);
        let ce = self.ents.get_mut(&c).unwrap();
        ce.name = u.champion.clone();
        ce.max_hp *= 3.5;
        ce.hp = ce.max_hp;
        let ms = ce.monster.as_mut().unwrap();
        ms.damage[0] *= 1.4;
        ms.damage[1] *= 1.4;
        ms.armor += 3.0;
        ms.xp *= 4;
        ms.champion = u.key.clone();
        ms.persistent = true;
        ce.recalc();
        self.champions.insert(u.key.clone(), c);
        // a champion has a bodyguard
        let biome = self.levels["overworld"].def_at(spot).biome.clone();
        let mut rng = std::mem::replace(&mut self.rng, Rng::new(0, 0));
        if let Some(minion) = pick_monster(&mut rng, &biome, 0, false, false) {
            self.spawn_group(&mut rng, minion, "overworld", spot, lvl);
        }
        self.rng = rng;
        Some(c)
    }

    /// Completes slay quests when a champion falls.
    pub(crate) fn champion_slain(&mut self, p: Id, m: &Entity) {
        let key = m.monster.as_ref().unwrap().champion.clone();
        if key.is_empty() {
            return;
        }
        self.champions.remove(&key);
        if let Some(qi) = self.unique_quest(p, &key) {
            let pm = self.ents.get_mut(&p).unwrap().pm();
            let q = &mut pm.quests[qi];
            if q.kind == "slay" && !q.done {
                q.have = 1;
                q.done = true;
                let giver = q.giver.clone();
                pm.dirty = true;
                self.log(
                    p,
                    "#ff80ff",
                    format!("Чемпион повержен! Вернитесь к: {giver}."),
                );
            }
        }
    }

    /// Monsters of the right lands carry relics for active quests.
    pub(crate) fn relic_drop(&mut self, p: Id, m: &Entity) {
        let l = &self.levels[&m.level];
        let where_ = if l.id == "overworld" {
            l.def_at(m.cell()).biome.clone()
        } else {
            l.theme.clone()
        };
        let items: Vec<String> = self.ents[&p]
            .p()
            .quests
            .iter()
            .filter(|q| q.kind == "relics" && !q.done && q.sources.contains(&where_))
            .map(|q| q.item.clone())
            .collect();
        for it in items {
            if self.chance(35.0) {
                self.drop_item(&m.level, m.cell(), ItemStack::new(&it));
            }
        }
    }

    /// Recounts relics in the backpack.
    pub(crate) fn update_relics(&mut self, p: Id) {
        let pm = self.ents.get_mut(&p).unwrap().pm();
        let inv = pm.inventory.clone();
        let mut msgs = Vec::new();
        for q in pm.quests.iter_mut() {
            if q.kind != "relics" {
                continue;
            }
            let have: i32 = inv
                .iter()
                .filter(|st| st.key == q.item)
                .map(|st| st.qty.max(1))
                .sum();
            let was = q.done;
            q.have = have.min(q.need);
            q.done = have >= q.need;
            if q.done && !was {
                msgs.push(format!("Собрано достаточно! Вернитесь к: {}.", q.giver));
            }
        }
        for m in msgs {
            self.log(p, "#ff80ff", m);
        }
    }

    /// Hands out the reward of a completed unique quest.
    pub(crate) fn finish_unique_quest(&mut self, p: Id, u: &UniqueDef) -> String {
        let Some(_) = self.unique_quest(p, &u.key) else {
            return String::new();
        };
        self.update_relics(p);
        let qi = self.unique_quest(p, &u.key).unwrap();
        let pe = &self.ents[&p];
        let q = pe.p().quests[qi].clone();
        if !q.done {
            return match q.kind.as_str() {
                "relics" => format!(
                    "Пока лишь {} из {}. Ищи у тварей: {}.",
                    q.have,
                    q.need,
                    sources_text(&q.sources)
                ),
                "slay" => {
                    let at = q.at.unwrap_or_default();
                    format!(
                        "{} всё ещё жив. Ищи {}, примерно в {} шагах.",
                        q.where_,
                        compass_ru(pe.cell(), at),
                        pe.cell().dist(at)
                    )
                }
                _ => "Дело не сделано. Возвращайся с победой.".into(),
            };
        }
        let pm = self.ents.get_mut(&p).unwrap().pm();
        if q.kind == "relics" {
            let mut left = q.need;
            let mut kept = Vec::new();
            for mut st in std::mem::take(&mut pm.inventory) {
                if st.key == q.item && left > 0 {
                    let take = left.min(st.qty.max(1));
                    left -= take;
                    st.qty -= take;
                    if st.qty <= 0 {
                        continue;
                    }
                }
                kept.push(st);
            }
            pm.inventory = kept;
        }
        pm.quests.retain(|x| x.unique != u.key);
        pm.unlocks.push(format!("quest:{}", u.key));
        pm.gold += q.gold;
        pm.dirty = true;
        self.deed(p, "uniques", 1);
        self.give_xp(p, q.xp);
        let what = self.unlock(p, &u.reward);
        let e = &self.ents[&p];
        let (level, pos, name) = (e.level.clone(), e.pos, e.name.clone());
        self.fx(&level, pos, "Награда!", '\0', "#ff80ff", 1800);
        self.chronicle(format!(
            "{name} исполнил(а) просьбу {} и получил(а) {what}",
            u.name
        ));
        u.done.clone()
    }
}
