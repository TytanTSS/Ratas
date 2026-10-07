//! Conversations: options, greetings, rumours, quests, trade and the AI voice.

use super::*;
use crate::content::UniqueDef;
use crate::llm::{NpcReply, NpcRequest, Option_, Turn as LTurn};
use crate::proto::Dialogue;

pub(crate) struct DialogueOption {
    pub label: String,
    pub action: &'static str,
}

fn opt(label: &str, action: &'static str) -> DialogueOption {
    DialogueOption {
        label: label.into(),
        action,
    }
}

pub(crate) fn compass(from: Pos, to: Pos) -> &'static str {
    let (dx, dy) = ((to.x - from.x) as f64, (to.y - from.y) as f64);
    let mut ang = (-dy).atan2(dx).to_degrees();
    if ang < 0.0 {
        ang += 360.0;
    }
    const DIRS: [&str; 8] = [
        "east",
        "north-east",
        "north",
        "north-west",
        "west",
        "south-west",
        "south",
        "south-east",
    ];
    DIRS[((ang + 22.5) / 45.0) as usize % 8]
}

pub(crate) fn compass_ru(from: Pos, to: Pos) -> &'static str {
    match compass(from, to) {
        "east" => "на востоке",
        "north-east" => "на северо-востоке",
        "north" => "на севере",
        "north-west" => "на северо-западе",
        "west" => "на западе",
        "south-west" => "на юго-западе",
        "south" => "на юге",
        _ => "на юго-востоке",
    }
}

fn round_to(v: i32, k: i32) -> i32 {
    ((v + k / 2) / k * k).max(k)
}

/// A distance in steps as people would say it: rougher the further it is.
fn round_steps(d: i32) -> i32 {
    if d <= 200 {
        round_to(d, 10)
    } else if d <= 1000 {
        round_to(d, 50)
    } else {
        round_to(d, 100)
    }
}

/// How far away the dangerous lands people warn about may lie.
const NEAR_LANDS: i32 = 500;

/// What a villager tells the model about the world: only the nearest of
/// each kind. A big world has hundreds of places, and every line of the
/// prompt costs a local model time before it says a word.
const FACT_VILLAGES: usize = 6;
const FACT_LANDMARKS: usize = 5;
const FACT_UNIQUES: usize = 3;
const FACT_LANDS: usize = 3;
const FACT_NEWS: usize = 5;

fn theme_word(theme: &str) -> &'static str {
    match theme {
        "cave" => "пещера",
        "crypt" => "старый склеп",
        "ice" => "ледяная пещера",
        "volcano" => "огненный провал",
        "temple" => "песчаная гробница",
        "fortress" => "чёрная цитадель",
        _ => "подземелье",
    }
}

fn landmark_word(kind: &str) -> &'static str {
    match kind {
        "shrine" | "circle" => "Помолишься там — и сил прибудет.",
        "ruins" => "Камни помнят старую войну, а в развалинах, бывает, находят клады.",
        "graveyard" => "Ночью туда лучше не соваться — мертвецы встают.",
        "camp" => "Там засели разбойники. Обходи стороной.",
        "oasis" => "Вода там чистая, а пальмы — как в сказке.",
        _ => "Странное место.",
    }
}

pub fn sell_price(st: &ItemStack) -> i32 {
    (st.value() * 2 / 5).max(1)
}

impl Game {
    pub(crate) fn dialogue_options(&self, p: Id, npc: Id) -> Vec<DialogueOption> {
        let n = self.ents[&npc].npc.as_ref().unwrap();
        if let Some(u) = db().unique(&n.unique) {
            let mut opts = vec![opt("Кто ты?", "about")];
            let q = self
                .unique_quest(p, &u.key)
                .map(|i| &self.ents[&p].p().quests[i]);
            if self.unique_done(p, &u.key) {
                opts.push(opt("Спасибо за всё", "thanks"));
            } else {
                match q {
                    None => opts.push(opt("Чем я могу помочь?", "uoffer")),
                    Some(q) if q.done || q.kind == "relics" => {
                        opts.push(opt("Я выполнил твою просьбу", "ufinish"))
                    }
                    Some(_) => opts.push(opt("Напомни, что нужно сделать", "ufinish")),
                }
            }
            opts.push(opt("Что слышно в мире?", "rumor"));
            opts.push(opt("Прощай", "bye"));
            return opts;
        }
        let role = db().npc_role(&n.role);
        let mut opts = vec![
            opt("Как жизнь?", "mood"),
            opt("Что слышно в округе?", "rumor"),
        ];
        if let Some(r) = role {
            if !r.stories.is_empty() {
                opts.push(opt("Расскажи о себе", "story"));
            }
            if r.quest_giver {
                opts.push(opt("Есть работа?", "quest"));
            }
            if r.trader {
                opts.push(opt("Покажи товары", "trade"));
            }
        }
        opts.extend(self.service_options(p, npc));
        opts.push(opt("Прощай", "bye"));
        opts
    }

    pub(crate) fn npc_title(&self, npc: Id) -> (String, String) {
        let n = self.ents[&npc].npc.as_ref().unwrap();
        if let Some(u) = db().unique(&n.unique) {
            return (u.name.clone(), u.title.clone());
        }
        let rn = db()
            .npc_role(&n.role)
            .map(|r| r.name.clone())
            .unwrap_or(n.role.clone());
        (n.pname.clone(), rn)
    }

    pub(crate) fn send_dialogue(&mut self, p: Id, npc: Id, text: &str, trade: bool, waiting: bool) {
        let (name, role) = self.npc_title(npc);
        let ai = self.brain.as_ref().is_some_and(|b| b.enabled());
        let mut d = Dialogue {
            npc,
            name,
            role,
            text: text.into(),
            options: self
                .dialogue_options(p, npc)
                .into_iter()
                .map(|o| o.label)
                .collect(),
            ai,
            waiting,
            ..Default::default()
        };
        if trade {
            d.trade = self.trade_list(npc);
        }
        self.box_(p).dialogue = Some(d);
    }

    pub(crate) fn open_dialogue(&mut self, p: Id, npc: Id) {
        if self.ents.get(&npc).is_none_or(|n| n.npc.is_none())
            || self.ents[&p].dead
            || self.hostile(p, npc)
        {
            return;
        }
        let pp = self.ents[&p].pos;
        self.ents.get_mut(&p).unwrap().pm().talking = npc;
        let ne = self.ents.get_mut(&npc).unwrap();
        ne.face_to(pp);
        ne.goal = None;
        let mut text = self.greeting(p, npc);
        let reward = self.turn_in_quests(p, npc);
        if !reward.is_empty() {
            text = reward;
        }
        self.send_dialogue(p, npc, &text, false, false);
    }

    /// The greeting depends on who greets whom, when, and what the hero has done.
    pub(crate) fn greeting(&mut self, p: Id, npc: Id) -> String {
        let pname = self.ents[&p].name.clone();
        let n = self.ents.get_mut(&npc).unwrap().npc.as_mut().unwrap();
        let met = n.met.get(&pname).copied().unwrap_or(0);
        *n.met.entry(pname.clone()).or_insert(0) += 1;
        let village = n.village.clone();
        let rep = |s: &str| s.replace("{village}", &village).replace("{player}", &pname);
        if let Some(u) = db().unique(&n.unique) {
            if met > 0 {
                return rep(&self.pick(
                    npc,
                    &[
                        "Снова ты, {player}. Я ждал.",
                        "Вернулся? Хорошо.",
                        "А, {player}. Говори.",
                    ],
                ));
            }
            return rep(&u.greeting);
        }
        let role = db().npc_role(&n.role);
        let mut parts = Vec::new();
        let pe = &self.ents[&p];
        let lit = self.levels[&self.ents[&npc].level].lit;
        if pe.hp < pe.max_hp * 0.4 {
            parts.push(self.pick(
                npc,
                &[
                    "Да ты весь в крови!",
                    "Эк тебя потрепало!",
                    "Тебе бы к знахарке, путник.",
                ],
            ));
        } else if lit && self.is_night() && !village.is_empty() {
            parts.push(self.pick(
                npc,
                &[
                    "Поздно ты гуляешь.",
                    "Ночь на дворе, а ты всё бродишь.",
                    "В такую темень добрые люди по домам сидят.",
                ],
            ));
        }
        if met > 0 && self.chance(60.0) {
            parts.push(rep(&self.pick(
                npc,
                &[
                    "Снова ты, {player}!",
                    "А, {player}, рад видеть.",
                    "Опять ты? Ну, заходи.",
                    "{player}! Как дорога?",
                ],
            )));
        } else if let Some(r) = role.filter(|r| !r.greetings.is_empty()) {
            parts.push(rep(self.rng.pick(&r.greetings)));
        }
        let bosses = self.ents[&p].p().bosses.clone();
        if !bosses.is_empty() && self.chance(30.0) {
            let b = self.rng.pick(&bosses).clone();
            if let Some(d) = db().monster(&b) {
                parts.push(format!("Говорят, это ты одолел {}? Уважаю.", d.name));
            }
        }
        if parts.is_empty() {
            "...".into()
        } else {
            parts.join(" ")
        }
    }

    /// A line not yet said by this NPC (lines repeat only when all were told).
    pub(crate) fn pick<S: AsRef<str>>(&mut self, npc: Id, lines: &[S]) -> String {
        if lines.is_empty() {
            return "...".into();
        }
        let n = self.ents.get_mut(&npc).unwrap().npc.as_mut().unwrap();
        let mut fresh: Vec<&str> = lines
            .iter()
            .map(|s| s.as_ref())
            .filter(|l| !n.said.contains(*l))
            .collect();
        if fresh.is_empty() {
            for l in lines {
                n.said.remove(l.as_ref());
            }
            fresh = lines.iter().map(|s| s.as_ref()).collect();
        }
        let l = fresh[self.rng.usize_n(fresh.len())].to_string();
        self.ents
            .get_mut(&npc)
            .unwrap()
            .npc
            .as_mut()
            .unwrap()
            .said
            .insert(l.clone());
        l
    }

    pub(crate) fn close_dialogue(&mut self, p: Id, notify: bool) {
        let Some(pl) = self.ents.get_mut(&p).and_then(|e| e.player.as_mut()) else {
            return;
        };
        if pl.talking == 0 {
            return;
        }
        pl.talking = 0;
        if notify {
            self.box_(p).dialogue = Some(Dialogue {
                close: true,
                ..Default::default()
            });
        }
    }

    pub(crate) fn talking_to(&self, p: Id) -> Option<Id> {
        let npc = self.ents[&p].p().talking;
        let ne = self.ents.get(&npc)?;
        if ne.npc.is_none() || self.hostile(p, npc) {
            return None;
        }
        Some(npc)
    }

    pub(crate) fn talk_option(&mut self, p: Id, idx: i32) {
        let Some(npc) = self.talking_to(p) else {
            self.close_dialogue(p, true);
            return;
        };
        let opts = self.dialogue_options(p, npc);
        if idx < 0 || idx as usize >= opts.len() {
            return;
        }
        let n = self.ents[&npc].npc.as_ref().unwrap();
        let role = db().npc_role(&n.role);
        let u = db().unique(&n.unique);
        let text = match opts[idx as usize].action {
            "mood" => self.mood_line(npc),
            "rumor" => self.rumor(p, npc),
            "story" => self.pick(npc, &role.map(|r| r.stories.clone()).unwrap_or_default()),
            "quest" => self.canned_quest(p, npc),
            "trade" => {
                self.send_dialogue(p, npc, "Смотри, выбирай. Цены честные!", true, false);
                return;
            }
            "about" => self.pick(npc, &u.map(|u| u.about.clone()).unwrap_or_default()),
            "uoffer" => self.offer_unique_quest(p, npc, u.unwrap()),
            "ufinish" => self.finish_unique_quest(p, u.unwrap()),
            "thanks" => {
                let name = self.ents[&p].name.clone();
                self.pick(
                    npc,
                    &[
                        "Это я должен благодарить тебя.".to_string(),
                        "Ступай со светом. Наши пути ещё пересекутся.".to_string(),
                        format!("Мир тесен, {name}. Ещё увидимся."),
                    ],
                )
            }
            a @ ("rest" | "upgrade" | "song" | "bless") => self.service(p, npc, a),
            "bye" => {
                self.close_dialogue(p, true);
                return;
            }
            _ => "...".into(),
        };
        self.send_dialogue(p, npc, &text, false, false);
    }

    /// How the NPC feels, depending on what goes on around.
    pub(crate) fn mood_line(&mut self, npc: Id) -> String {
        let ne = &self.ents[&npc];
        let threats = self.near(&ne.level, ne.pos, 25.0).into_iter().filter(|o| {
            let oe = &self.ents[o];
            oe.faction == Faction::Monster && oe.kind == Kind::Monster && oe.dist(ne) <= 25.0
        });
        let threats = threats.count();
        let mut lines: Vec<String> = Vec::new();
        if threats >= 3 {
            lines.extend(
                [
                    "Чудища бродят у самой околицы. Страшно мне, честно скажу.",
                    "Слышишь? Опять что-то воет за частоколом.",
                ]
                .map(String::from),
            );
        } else if threats == 0 {
            lines.extend(
                [
                    "Тихо нынче, хвала небесам.",
                    "Спокойный денёк выдался. Редкость по нынешним временам.",
                ]
                .map(String::from),
            );
        }
        if self.is_night() {
            lines.extend(
                [
                    "Ночью я двери на засов запираю. И тебе советую.",
                    "Не люблю ночь. Мёртвые не спят.",
                ]
                .map(String::from),
            );
        } else {
            lines.extend(
                [
                    "Работы невпроворот, а день короткий.",
                    "Солнце греет — уже хорошо.",
                ]
                .map(String::from),
            );
        }
        let r = self.region_index(ne.cell());
        if r != 0 && self.regions[r - 1].danger >= 2 {
            lines.push("Места у нас гиблые. Каждый день как последний.".into());
        }
        if let Some(role) = db().npc_role(&ne.npc.as_ref().unwrap().role) {
            lines.extend(role.lines.iter().cloned());
        }
        self.pick(npc, &lines)
    }

    /// A piece of real news about the world.
    pub(crate) fn rumor(&mut self, _p: Id, npc: Id) -> String {
        let ne = &self.ents[&npc];
        let at = ne.cell();
        let mut lines = Vec::new();
        for c in self.chronicle.iter().rev().take(4) {
            lines.push(format!("Слыхал новость? {c}!"));
        }
        for e in &self.entrances {
            if e.pos.dist(at) < 120 {
                lines.push(format!(
                    "{} отсюда, шагах в {}, есть {} — «{}». Там, говорят, {} ярусов, и на дне сидит кто-то страшный.",
                    upper_first(compass_ru(at, e.pos)),
                    round_to(at.dist(e.pos), 10),
                    theme_word(&e.theme),
                    e.name,
                    e.max_depth
                ));
            }
        }
        for lm in &self.landmarks {
            if lm.pos.dist(at) < 90 {
                lines.push(format!(
                    "{} есть {}. {}",
                    upper_first(compass_ru(at, lm.pos)),
                    lm.name,
                    landmark_word(&lm.kind)
                ));
            }
        }
        // unique characters may live far away in a big world: the distance helps
        for (o, oe) in &self.ents {
            let Some(on) = &oe.npc else { continue };
            if on.unique.is_empty() || *o == npc || oe.level != "overworld" {
                continue;
            }
            if let Some(u) = db().unique(&on.unique) {
                lines.push(format!(
                    "Ходят слухи, что {}, шагах в {}, живёт {} — {}. Говорят, награда у него за помощь такая, что и не снилась.",
                    compass_ru(at, oe.cell()),
                    round_steps(at.dist(oe.cell())),
                    u.name,
                    lower(&u.title)
                ));
            }
        }
        for r in &self.regions {
            if r.danger >= 2 && r.at.dist(at) <= NEAR_LANDS {
                lines.push(format!(
                    "Держись подальше от земель «{}». Оттуда мало кто возвращается.",
                    r.name
                ));
            }
        }
        for m in &db().b.monsters {
            if m.boss {
                lines.push(format!(
                    "Старики шепчут, что {} ещё жив и копит силы.",
                    m.name
                ));
            }
        }
        if lines.is_empty() {
            return "Ничего нового, путник.".into();
        }
        self.pick(npc, &lines)
    }

    // ---- quests ----

    pub(crate) fn turn_in_quests(&mut self, p: Id, npc: Id) -> String {
        let pm = self.ents.get_mut(&p).unwrap().pm();
        let (done, kept): (Vec<Quest>, Vec<Quest>) = std::mem::take(&mut pm.quests)
            .into_iter()
            .partition(|q| q.done && q.giver_id == npc && q.unique.is_empty());
        pm.quests = kept;
        if done.is_empty() {
            return String::new();
        }
        pm.dirty = true;
        let mut parts = Vec::new();
        for q in done {
            self.ents.get_mut(&p).unwrap().pm().gold += q.gold;
            self.give_xp(p, q.xp);
            self.deed(p, "quests", 1);
            parts.push(format!("{} золота", q.gold));
            self.log(
                p,
                "#ffd24a",
                format!("Награда за задание: {} золота, {} опыта.", q.gold, q.xp),
            );
        }
        format!(
            "Ты справился! Деревня тебе благодарна. Держи награду: {}.",
            parts.join(", ")
        )
    }

    pub(crate) fn quest_monsters(&self) -> Vec<&'static crate::content::MonsterDef> {
        db().b
            .monsters
            .iter()
            .filter(|m| m.boss || (m.depth == [0, 0] && m.weight > 0 && !m.elite))
            .collect()
    }

    pub(crate) fn add_quest(
        &mut self,
        p: Id,
        npc: Id,
        monster: &str,
        count: i32,
    ) -> Result<usize, String> {
        let pm = self.ents[&p].p();
        if pm.quests.iter().any(|q| q.giver_id == npc && !q.done) {
            return Err("уже есть задание от этого персонажа".into());
        }
        if pm.quests.len() >= 5 {
            return Err("слишком много заданий".into());
        }
        let Some(def) = db().monster(monster) else {
            return Err("неизвестный монстр".into());
        };
        let count = if def.boss { 1 } else { count.clamp(1, 10) };
        self.quest_seq += 1;
        let n = self.ents[&npc].npc.as_ref().unwrap();
        let mut q = Quest {
            id: self.quest_seq,
            giver: n.pname.clone(),
            giver_id: npc,
            village: n.village.clone(),
            monster: monster.into(),
            need: count,
            gold: 15 + count * def.xp / 2,
            xp: count * def.xp * 3 / 2,
            ..Default::default()
        };
        if def.boss {
            q.gold = 150;
            q.xp = def.xp;
        }
        let gold = q.gold;
        let pm = self.ents.get_mut(&p).unwrap().pm();
        pm.quests.push(q);
        pm.dirty = true;
        let i = pm.quests.len() - 1;
        self.log(
            p,
            "#ffd24a",
            format!(
                "Новое задание — охота: {} ×{count} (награда {gold} золота). Журнал — J.",
                def.name
            ),
        );
        Ok(i)
    }

    pub(crate) fn canned_quest(&mut self, p: Id, npc: Id) -> String {
        if let Some(q) = self.ents[&p]
            .p()
            .quests
            .iter()
            .find(|q| q.giver_id == npc && !q.done)
        {
            let name = db()
                .monster(&q.monster)
                .map(|m| m.name.clone())
                .unwrap_or(q.monster.clone());
            return format!("Ты ещё не закончил: {name} — {} из {}.", q.have, q.need);
        }
        let lvl = self.ents[&p].p().level;
        let pool: Vec<_> = self
            .quest_monsters()
            .into_iter()
            .filter(|m| !m.boss || lvl >= 6)
            .collect();
        if pool.is_empty() {
            return "Пока работы нет.".into();
        }
        let m = pool[self.rng.usize_n(pool.len())];
        let count = 3 + self.rng.int_n(5);
        match self.add_quest(p, npc, &m.key, count) {
            Err(why) => format!("Сначала разберись с другими делами ({why})."),
            Ok(i) => {
                let q = &self.ents[&p].p().quests[i];
                if m.boss {
                    format!(
                        "Говорят, в подземелье правит {}. Уничтожь это зло — и {} золота твои.",
                        m.name, q.gold
                    )
                } else {
                    format!(
                        "Нас одолевают твари. Убей {}: {} — и получишь {} золота.",
                        q.need, m.name, q.gold
                    )
                }
            }
        }
    }

    // ---- trade ----

    pub(crate) fn sell(&mut self, p: Id, idx: i32) {
        let Some(npc) = self.talking_to(p) else {
            return;
        };
        let trader = db()
            .npc_role(&self.ents[&npc].npc.as_ref().unwrap().role)
            .is_some_and(|r| r.trader);
        if !trader {
            return;
        }
        let pm = self.ents.get_mut(&p).unwrap().pm();
        if idx < 0 || idx as usize >= pm.inventory.len() {
            return;
        }
        let i = idx as usize;
        let st = pm.inventory[i].clone();
        let price = sell_price(&st);
        if st.qty > 1 {
            pm.inventory[i].qty -= 1;
        } else {
            pm.inventory.remove(i);
        }
        pm.gold += price;
        pm.dirty = true;
        self.log(
            p,
            "#ffd700",
            format!("Продано: {} за {price} золота.", st.name()),
        );
    }

    // ---- AI conversation ----

    pub(crate) fn world_facts(&self, npc: Id) -> Vec<String> {
        let at = self.ents[&npc].cell();
        let village = self.ents[&npc].npc.as_ref().unwrap().village.clone();
        let mut facts = Vec::new();
        let mut ds: Vec<(usize, i32)> = self
            .entrances
            .iter()
            .enumerate()
            .map(|(i, e)| (i, e.pos.manhattan(at)))
            .collect();
        ds.sort_by_key(|d| d.1);
        for (i, d) in ds.into_iter().take(4) {
            let e = &self.entrances[i];
            facts.push(format!(
                "The dungeon \"{}\" ({}, {} levels deep) lies to the {}, about {} steps away.",
                e.name,
                theme_word(&e.theme),
                e.max_depth,
                compass(at, e.pos),
                d
            ));
        }
        // the nearest settlements (a big world has hundreds)
        let mut vs: Vec<&VillageInfo> =
            self.villages.iter().filter(|v| v.name != village).collect();
        vs.sort_by_key(|v| v.center.dist_sq(at));
        for v in vs.into_iter().take(FACT_VILLAGES) {
            facts.push(format!(
                "The {} {} lies to the {}, about {} steps away.",
                if v.city { "city" } else { "village" },
                v.name,
                compass(at, v.center),
                round_steps(at.dist(v.center))
            ));
        }
        let mut ls: Vec<&Landmark> = self
            .landmarks
            .iter()
            .filter(|lm| lm.pos.dist(at) < 100)
            .collect();
        ls.sort_by_key(|lm| lm.pos.dist_sq(at));
        for lm in ls.into_iter().take(FACT_LANDMARKS) {
            facts.push(format!(
                "Landmark: {} ({}) to the {}.",
                lm.name,
                lm.kind,
                compass(at, lm.pos)
            ));
        }
        let mut us: Vec<(i32, Pos, &UniqueDef)> = self
            .ents
            .iter()
            .filter(|(o, oe)| **o != npc && oe.level == "overworld")
            .filter_map(|(_, oe)| {
                let u = db().unique(&oe.npc.as_ref()?.unique)?;
                Some((at.dist(oe.cell()), oe.cell(), u))
            })
            .collect();
        us.sort_by(|a, b| (a.0, &a.2.key).cmp(&(b.0, &b.2.key)));
        for (d, p, u) in us.into_iter().take(FACT_UNIQUES) {
            facts.push(format!(
                "Rumour: {}, {}, lives to the {}, about {} steps away, and rewards those who help with something extraordinary.",
                u.name,
                u.title,
                compass(at, p),
                round_steps(d)
            ));
        }
        let mut rs: Vec<&Region> = self
            .regions
            .iter()
            .filter(|r| r.danger >= 2 && r.at.dist(at) <= NEAR_LANDS)
            .collect();
        rs.sort_by_key(|r| r.at.dist_sq(at));
        for r in rs.into_iter().take(FACT_LANDS) {
            facts.push(format!(
                "The lands called {} ({}) are deadly.",
                r.name, r.kind
            ));
        }
        let lords: Vec<String> = db()
            .b
            .monsters
            .iter()
            .filter(|m| m.boss)
            .map(|m| format!("{} ({})", m.name, m.themes.join("/")))
            .collect();
        if !lords.is_empty() {
            facts.push(format!(
                "Rumour: the deepest level of each kind of dungeon has its lord: {}.",
                lords.join(", ")
            ));
        }
        facts.extend(self.lore_facts());
        let news = self.chronicle.len().saturating_sub(FACT_NEWS);
        for c in &self.chronicle[news..] {
            facts.push(format!("Recent news people talk about: {c}."));
        }
        facts
    }

    pub(crate) fn talk_ai(&mut self, p: Id, text: &str) {
        let Some(npc) = self.talking_to(p) else {
            return;
        };
        let text: String = text.trim().chars().take(300).collect();
        if text.is_empty() {
            return;
        }
        let n = self.ents[&npc].npc.as_ref().unwrap();
        let role = db().npc_role(&n.role);
        let enabled = self.brain.as_ref().is_some_and(|b| b.enabled());
        if !enabled {
            let line = match role.filter(|r| !r.lines.is_empty()) {
                Some(r) => self.rng.pick(&r.lines).clone(),
                None => "Хм...".into(),
            };
            self.send_dialogue(p, npc, &line, false, false);
            return;
        }
        if n.busy {
            let name = n.pname.clone();
            self.log(p, "#808080", format!("{name} ещё обдумывает ответ..."));
            return;
        }
        let pe = &self.ents[&p];
        let pl = pe.p();
        let mut req = NpcRequest {
            npc_name: n.pname.clone(),
            village: if n.village.is_empty() {
                "the wilds".into()
            } else {
                n.village.clone()
            },
            world: self.world_name.clone(),
            time_of_day: self.time_name().into(),
            player_name: pe.name.clone(),
            player_level: pl.level,
            message: text.clone(),
            purse: n.gold,
            facts: self.world_facts(npc),
            times_met: (n.met.get(&pe.name).copied().unwrap_or(0) - 1).max(0),
            lang: pl.lang.clone(),
            ..Default::default()
        };
        let r = self.region_index(self.ents[&npc].cell());
        if r != 0 {
            let reg = &self.regions[r - 1];
            req.region = format!("{} ({}, danger {} of 3)", reg.name, reg.kind, reg.danger);
        }
        if let Some(role) = role {
            req.role = role.name.clone();
            req.persona = role.persona.clone();
            req.trader = role.trader;
            req.can_give_quest = role.quest_giver;
        }
        if let Some(u) = db().unique(&n.unique) {
            req.role = u.title.clone();
            req.persona = u.persona.clone();
            let state = if let Some(qi) = self.unique_quest(p, &u.key) {
                if pl.quests[qi].done {
                    "the player has done it and can claim the reward"
                } else {
                    "the player is working on it"
                }
            } else if self.unique_done(p, &u.key) {
                "already fulfilled by this player"
            } else {
                "not given yet: the player can ask you about it"
            };
            req.own_quest = format!(
                "{} Reward: {}. State: {state}.",
                u.offer,
                reward_name(&u.reward)
            );
        }
        let d = db();
        req.player_class = pl
            .classes
            .iter()
            .filter_map(|ck| d.class(ck).map(|c| (ck, c)))
            .map(
                |(ck, c)| match pl.subclasses.get(ck).and_then(|s| d.subclass(s)) {
                    Some(sc) => format!("{} ({})", c.name, sc.name),
                    None => c.name.clone(),
                },
            )
            .collect::<Vec<_>>()
            .join(", ");
        for b in &pl.bosses {
            if let Some(m) = d.monster(b) {
                req.player_deeds.push(format!("slew {}", m.name));
            }
        }
        if pl.kills > 0 {
            req.player_deeds
                .push(format!("has killed {} monsters", pl.kills));
        }
        if pe.hp < pe.max_hp * 0.4 {
            req.player_deeds.push("is badly wounded right now".into());
        }
        for q in &pl.quests {
            let name = d
                .monster(&q.monster)
                .map(|m| m.name.clone())
                .unwrap_or(q.monster.clone());
            let status = if q.done {
                "done, reward not yet collected".to_string()
            } else {
                format!("{}/{}", q.have, q.need)
            };
            req.player_quests
                .push(format!("slay {name} for {} ({status})", q.giver));
        }
        for t in n.memory.get(&pe.name).cloned().unwrap_or_default() {
            req.history.push(LTurn {
                who: t.who,
                text: t.text,
            });
        }
        for key in self.gift_keys(npc) {
            req.gifts.push(Option_ {
                name: d.item(&key).unwrap().name.clone(),
                key,
            });
        }
        for m in self.quest_monsters() {
            req.monsters.push(Option_ {
                key: m.key.clone(),
                name: m.name.clone(),
            });
        }
        let mood = self.mood_line(npc);
        req.mood = mood;
        self.ents.get_mut(&npc).unwrap().npc.as_mut().unwrap().busy = true;
        self.send_dialogue(p, npc, "", false, true);
        let account = self.ents[&p].name.clone();
        let tx = self.task_sender();
        let heard_tx = tx.clone();
        let listener = account.clone();
        self.brain.as_ref().unwrap().npc_talk(
            req,
            Box::new(move |words: String| {
                let listener = listener.clone();
                let _ = heard_tx.send(Box::new(move |g: &mut Game| {
                    g.npc_heard(&listener, npc, &words)
                }));
            }),
            Box::new(move |r: Result<NpcReply, String>| {
                let _ = tx.send(Box::new(move |g: &mut Game| {
                    g.apply_npc_reply(&account, npc, &text, r)
                }));
            }),
        );
    }

    /// Shows the words of a reply the model is still writing (the box
    /// stays waiting until the reply is done).
    pub(crate) fn npc_heard(&mut self, account: &str, npc: Id, words: &str) {
        let Some(&p) = self.online.get(account) else {
            return;
        };
        let busy = self
            .ents
            .get(&npc)
            .and_then(|e| e.npc.as_ref())
            .is_some_and(|n| n.busy);
        if busy && self.ents[&p].p().talking == npc {
            let words: String = words.chars().take(400).collect();
            self.send_dialogue(p, npc, &words, false, true);
        }
    }

    pub(crate) fn gift_keys(&self, npc: Id) -> Vec<String> {
        let mut keys: Vec<String> = db()
            .b
            .items
            .iter()
            .filter(|it| it.kind == "consumable" && it.value <= 20)
            .map(|it| it.key.clone())
            .collect();
        if let Some(role) = db().npc_role(&self.ents[&npc].npc.as_ref().unwrap().role) {
            for k in &role.goods {
                if db().item(k).is_some_and(|d| d.value <= 30) && !keys.contains(k) {
                    keys.push(k.clone());
                }
            }
        }
        keys
    }

    pub(crate) fn apply_npc_reply(
        &mut self,
        account: &str,
        npc: Id,
        said: &str,
        r: Result<NpcReply, String>,
    ) {
        let now = self.now;
        let Some(ne) = self.ents.get_mut(&npc) else {
            return;
        };
        let Some(n) = ne.npc.as_mut() else { return };
        n.busy = false;
        let p = self.online.get(account).copied();
        let r = match r {
            Err(err) => {
                if let Some(p) = p {
                    if self.ents[&p].p().talking == npc {
                        let role = db().npc_role(&self.ents[&npc].npc.as_ref().unwrap().role);
                        let line = match role.filter(|r| !r.lines.is_empty()) {
                            Some(r) => self.rng.pick(&r.lines).clone(),
                            None => "Хм... Прости, задумался.".into(),
                        };
                        self.send_dialogue(p, npc, &line, false, false);
                        self.log(p, "#806060", format!("(ИИ недоступен: {err})"));
                    }
                }
                return;
            }
            Ok(r) => r,
        };
        let mut say = r.say.trim().to_string();
        if say.chars().count() > 400 {
            say = say.chars().take(400).collect::<String>() + "…";
        }
        let n = self.ents.get_mut(&npc).unwrap().npc.as_mut().unwrap();
        let mem = n.memory.entry(account.to_string()).or_default();
        mem.push(Turn {
            who: "player".into(),
            text: said.into(),
        });
        mem.push(Turn {
            who: "npc".into(),
            text: say.clone(),
        });
        if mem.len() > 16 {
            let k = mem.len() - 16;
            mem.drain(..k);
        }
        let Some(p) = p else { return };
        self.say(npc, &say, 5000.0);
        let n = self.ents[&npc].npc.as_ref().unwrap();
        let role = db().npc_role(&n.role);
        let pname = n.pname.clone();
        let mut trade = false;
        match r.action.as_str() {
            "give_gold" => {
                let amt = r.gold.min(n.gold).clamp(0, 200);
                if amt > 0 {
                    self.ents.get_mut(&npc).unwrap().npc.as_mut().unwrap().gold -= amt;
                    let pm = self.ents.get_mut(&p).unwrap().pm();
                    pm.gold += amt;
                    pm.dirty = true;
                    self.log(p, "#ffd700", format!("{pname} даёт вам {amt} золота."));
                }
            }
            "give_item" => {
                if self.gift_keys(npc).contains(&r.item) {
                    let last = n.gifts.get(account).copied().unwrap_or(0.0);
                    if last == 0.0 || now - last > 5.0 * 60.0 * 1000.0 {
                        let st = ItemStack::new(&r.item);
                        if self.add_item(p, st.clone()) {
                            self.ents
                                .get_mut(&npc)
                                .unwrap()
                                .npc
                                .as_mut()
                                .unwrap()
                                .gifts
                                .insert(account.into(), now);
                            self.log(p, "#c0c0ff", format!("{pname} дарит вам: {}.", st.name()));
                        }
                    }
                }
            }
            "heal" => {
                let k = format!("heal:{account}");
                let last = n.gifts.get(&k).copied().unwrap_or(0.0);
                if last == 0.0 || now - last > 60.0 * 1000.0 {
                    self.ents
                        .get_mut(&npc)
                        .unwrap()
                        .npc
                        .as_mut()
                        .unwrap()
                        .gifts
                        .insert(k, now);
                    let pe = &self.ents[&p];
                    let missing = pe.max_hp - pe.hp;
                    self.heal(p, missing);
                    self.log(p, "#80ff80", format!("{pname} лечит ваши раны."));
                }
            }
            "offer_quest" => {
                if role.is_some_and(|r| r.quest_giver)
                    && self
                        .quest_monsters()
                        .iter()
                        .any(|m| m.key == r.quest_monster)
                {
                    let _ = self.add_quest(p, npc, &r.quest_monster, r.quest_count);
                }
            }
            "trade" => trade = role.is_some_and(|r| r.trader),
            "hostile" => {
                if self.ents[&p].p().talking == npc {
                    self.send_dialogue(p, npc, &say, false, false);
                }
                self.npc_turn_hostile(npc, p);
                return;
            }
            "end" => {
                if self.ents[&p].p().talking == npc {
                    let (name, role_name) = self.npc_title(npc);
                    self.box_(p).dialogue = Some(Dialogue {
                        npc,
                        name,
                        role: role_name,
                        text: say,
                        options: vec!["Уйти".into()],
                        ai: true,
                        ..Default::default()
                    });
                    self.ents.get_mut(&p).unwrap().pm().talking = 0;
                }
                return;
            }
            _ => {}
        }
        if self.ents[&p].p().talking == npc {
            self.send_dialogue(p, npc, &say, trade, false);
        }
    }

    /// Makes an offended NPC attack the player.
    pub(crate) fn npc_turn_hostile(&mut self, npc: Id, p: Id) {
        self.close_dialogue(p, true);
        let now = self.now;
        let (plvl, ppos) = (self.ents[&p].p().level, self.ents[&p].pos);
        let ne = self.ents.get_mut(&npc).unwrap();
        let n = ne.npc.take().unwrap();
        let (hp, dmg) = if n.role == "guard" {
            (80.0, [6.0, 11.0])
        } else {
            (45.0, [4.0, 8.0])
        };
        ne.kind = Kind::Monster;
        ne.faction = Faction::Monster;
        ne.color = "#ff5050".into();
        ne.hp = hp;
        ne.max_hp = hp;
        ne.pace = 1.0;
        ne.monster = Some(Box::new(MonsterState {
            def: "bandit".into(),
            lvl: 1 + plvl / 2,
            home: n.home,
            target: p,
            last_seen: ppos,
            last_seen_at: now,
            state: "chase".into(),
            damage: dmg,
            armor: 2.0,
            move_ms: 200.0,
            attack_ms: 1000.0,
            xp: 20,
            persistent: true,
            ..Default::default()
        }));
        ne.recalc();
        let name = ne.name.clone();
        self.log(p, "#ff4a4a", format!("{name} нападает на вас!"));
    }
}
