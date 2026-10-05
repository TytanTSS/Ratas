//! The AI game master. Now and then (and whenever an admin types /gm with a
//! wish) the model looks at the heroes and the world around them and answers
//! with commands: ambushes, rewards, blessings, rumours, omens, the time of
//! day. The game checks every command against its content and limits before
//! carrying it out, so a confused model cannot break the world.

use super::admin::find_def;
use super::*;
use crate::content::BuffDef;
use crate::llm::{local, GmCommand, GmReply, GmRequest, Option_, GM_ACTIONS};

pub(crate) const GM_COLOR: &str = "#c9a0ff";
/// How often the master acts on its own (game time) after its first look.
const GM_EVERY_MS: f64 = 150_000.0;
const GM_FIRST_MS: f64 = 60_000.0;
/// Commands carried out from one reply.
const GM_MAX_COMMANDS: usize = 3;

#[derive(Default)]
pub(crate) struct Director {
    /// the master stages events on its own
    pub on: bool,
    /// when it looks at the world next (None: not planned yet)
    pub next_at: Option<f64>,
    busy: bool,
    /// its recent commands, so it does not repeat itself
    recent: Vec<String>,
    /// the model state last told to the players
    seen: &'static str,
}

fn clip(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}

impl Game {
    /// Lets the AI game master stage events on its own (admins can always
    /// ask it with /gm).
    pub fn set_director(&mut self, on: bool) {
        self.gm.on = on;
    }

    pub fn director_on(&self) -> bool {
        self.gm.on
    }

    /// Lines for the dedicated server's console since the last call.
    pub fn take_server_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.server_log)
    }

    /// Called every couple of seconds: tells the players when a local model
    /// becomes ready or fails, and lets the master act on its own.
    pub(crate) fn director_tick(&mut self) {
        let Some(b) = self.brain.clone() else {
            return;
        };
        if let Some(st) = b.local_state() {
            let key = match &st {
                local::State::Ready => "ready",
                local::State::Failed(_) => "failed",
                _ => "wait",
            };
            if key != self.gm.seen {
                self.gm.seen = key;
                let line = match st {
                    local::State::Ready => Some((
                        "#8fd18f",
                        format!(
                            "Нейросеть {} готова: жители, враги и сам мир теперь думают с её помощью.",
                            b.title()
                        ),
                    )),
                    local::State::Failed(e) => Some((
                        "#ff8080",
                        format!(
                            "Локальная нейросеть недоступна ({e}). Работает простой ИИ."
                        ),
                    )),
                    _ => None,
                };
                if let Some((color, text)) = line {
                    self.server_log.push(text.clone());
                    self.log_all(color, text);
                }
            }
        }
        if !self.gm.on || self.gm.busy || self.online.is_empty() || !b.enabled() {
            return;
        }
        let next = *self.gm.next_at.get_or_insert(self.now + GM_FIRST_MS);
        if self.now < next {
            return;
        }
        self.gm.next_at = Some(self.now + GM_EVERY_MS * self.roll(0.8, 1.3));
        self.ask_master(None, "");
    }

    /// Sends the world to the master; asker is the admin with a wish.
    pub(crate) fn ask_master(&mut self, asker: Option<Id>, wish: &str) -> bool {
        let Some(b) = self.brain.clone() else {
            return false;
        };
        if !b.enabled() || self.gm.busy {
            return false;
        }
        let req = self.master_request(asker, wish);
        let tx = self.task_sender();
        let sent = b.director(
            req,
            Box::new(move |r: Result<GmReply, String>| {
                let _ = tx.send(Box::new(move |g: &mut Game| g.apply_master(asker, r)));
            }),
        );
        self.gm.busy = sent;
        sent
    }

    /// One line about a hero for the master.
    fn hero_line(&self, id: Id) -> String {
        let e = &self.ents[&id];
        let p = e.p();
        let d = db();
        let class = d
            .class(&p.class)
            .map(|c| c.name.clone())
            .unwrap_or(p.class.clone());
        let mut s = format!(
            "{}: level {} {}, health {}%, {} gold",
            e.name,
            p.level,
            class,
            (100.0 * e.hp / e.max_hp.max(1.0)).round() as i32,
            p.gold
        );
        if e.dead {
            s += ", dead (waiting to rise again)";
        }
        let cell = e.cell();
        if e.level == "overworld" {
            let r = self.region_index(cell);
            if r != 0 {
                let reg = &self.regions[r - 1];
                s += &format!(
                    ", in the lands of {} ({}, danger {} of 3)",
                    reg.name, reg.kind, reg.danger
                );
            }
            if self.in_village(cell, 2) {
                s += ", resting in a village";
            } else if let Some(v) = self.villages.iter().min_by_key(|v| v.center.dist_sq(cell)) {
                s += &format!(
                    ", {} steps from the {} {}",
                    v.center.dist(cell),
                    if v.city { "city" } else { "village" },
                    v.name
                );
            }
        } else if let Some(l) = self.levels.get(&e.level) {
            s += &format!(", in the dungeon {} (floor {})", l.name, l.depth);
        }
        let foes = self
            .on_level(&e.level)
            .into_iter()
            .filter(|&o| {
                self.ents
                    .get(&o)
                    .is_some_and(|oe| oe.monster.is_some() && oe.alive() && oe.dist(e) <= 12.0)
                    && self.hostile(id, o)
            })
            .count();
        if foes > 0 {
            s += &format!(", fighting {foes} enemies nearby");
        }
        s
    }

    /// Monsters the master may send (bosses and allies never; elites only
    /// when an admin asks).
    fn master_monsters(admin: bool) -> Vec<&'static crate::content::MonsterDef> {
        db().b
            .monsters
            .iter()
            .filter(|m| !m.boss && !m.ally && (admin || (m.weight > 0 && !m.elite)))
            .collect()
    }

    /// Items the master may give on its own: supplies and simple gear.
    fn master_gift(it: &crate::content::ItemDef) -> bool {
        it.kind != "gold" && !it.unique && (it.kind == "consumable" || it.value <= 60)
    }

    fn master_request(&self, asker: Option<Id>, wish: &str) -> GmRequest {
        let admin = asker.is_some();
        let d = db();
        let mut req = GmRequest {
            world: self.world_name.clone(),
            time_of_day: self.time_name().into(),
            wish: wish.trim().chars().take(400).collect(),
            news: self.chronicle.iter().rev().take(6).cloned().collect(),
            history: self.lore_facts(),
            recent: self.gm.recent.clone(),
            ..Default::default()
        };
        let mut en = 0;
        for &id in self.online.values() {
            if self.ents.contains_key(&id) {
                req.players.push(self.hero_line(id));
                if self.ents[&id].p().lang == "en" {
                    en += 1;
                }
            }
        }
        req.lang = match asker
            .and_then(|a| self.ents.get(&a))
            .and_then(|e| e.player.as_ref())
        {
            Some(p) => p.lang.clone(),
            None if en * 2 > req.players.len() => "en".into(),
            None => "ru".into(),
        };
        for m in Game::master_monsters(admin) {
            req.monsters.push(Option_ {
                key: m.key.clone(),
                name: m.name.clone(),
            });
        }
        for it in
            d.b.items
                .iter()
                .filter(|it| it.kind != "gold" && (admin || Game::master_gift(it)))
        {
            req.items.push(Option_ {
                key: it.key.clone(),
                name: it.name.clone(),
            });
        }
        if admin {
            for u in &d.b.uniques {
                if self.unique_in_world(&u.key).is_none() {
                    req.uniques.push(Option_ {
                        key: u.key.clone(),
                        name: format!("{}, {}", u.name, lower(&u.title)),
                    });
                }
            }
        }
        req
    }

    pub(crate) fn unique_in_world(&self, key: &str) -> Option<Id> {
        self.ents
            .iter()
            .find(|(_, o)| o.npc.as_ref().is_some_and(|n| n.unique == key))
            .map(|(i, _)| *i)
    }

    /// The hero a command names: by name, else the asking admin, else the
    /// only hero online.
    fn master_target(&self, name: &str, asker: Option<Id>) -> Option<Id> {
        let n = name.trim().to_lowercase();
        if !n.is_empty() {
            let found = self
                .online
                .iter()
                .find(|(k, _)| k.to_lowercase() == n)
                .or_else(|| {
                    self.online
                        .iter()
                        .find(|(k, _)| n.contains(&k.to_lowercase()))
                })
                .map(|(_, id)| *id);
            if found.is_some() {
                return found;
            }
        }
        asker
            .filter(|a| self.ents.contains_key(a))
            .or_else(|| (self.online.len() == 1).then(|| *self.online.values().next().unwrap()))
    }

    pub(crate) fn apply_master(&mut self, asker: Option<Id>, r: Result<GmReply, String>) {
        self.gm.busy = false;
        let asker = asker.filter(|a| self.ents.contains_key(a));
        let reply = match r {
            Ok(r) => r,
            Err(e) => {
                if let Some(a) = asker {
                    self.say_admin(a, format!("ИИ-мастер не ответил: {e}."));
                }
                return;
            }
        };
        let mut done = Vec::new();
        for c in reply.commands.iter().take(GM_MAX_COMMANDS) {
            match self.master_command(c, asker) {
                Ok(what) => {
                    let note = format!("{} {} {} {}", c.action, c.key, c.amount, c.player);
                    self.gm.recent.push(clip(&note, 80));
                    done.push(what);
                }
                Err(why) => {
                    if let Some(a) = asker {
                        self.say_admin(a, format!("ИИ-мастер: {} пропущено — {why}.", c.action));
                    }
                }
            }
        }
        let n = self.gm.recent.len();
        if n > 8 {
            self.gm.recent.drain(..n - 8);
        }
        let announce = clip(&reply.announce, 220);
        if !announce.is_empty() {
            self.log_all(GM_COLOR, announce.clone());
            self.server_log
                .push(format!("ИИ-мастер объявляет: {announce}"));
        }
        for what in &done {
            self.server_log.push(format!("ИИ-мастер: {what}"));
            if let Some(a) = asker {
                self.say_admin(a, format!("ИИ-мастер: {what}."));
            }
        }
        if let (Some(a), true) = (asker, done.is_empty() && announce.is_empty()) {
            self.say_admin(a, "ИИ-мастер решил ничего не менять.".into());
        }
    }

    /// Checks and carries out one command; returns what happened (for the
    /// admin and the server log) or why it was dropped.
    fn master_command(&mut self, c: &GmCommand, asker: Option<Id>) -> Result<String, String> {
        if !GM_ACTIONS.contains(&c.action.as_str()) {
            return Err("неизвестная команда".into());
        }
        let admin = asker.is_some();
        let p = self
            .master_target(&c.player, asker)
            .ok_or_else(|| "нет такого героя".to_string())?;
        let pname = self.ents[&p].name.clone();
        let text = clip(&c.text, 220);
        let tell = |g: &mut Game, default: String| {
            let line = if text.is_empty() {
                default
            } else {
                text.clone()
            };
            if !line.is_empty() {
                g.log(p, GM_COLOR, line);
            }
        };
        match c.action.as_str() {
            "spawn_monsters" => {
                let d = find_def(
                    &Game::master_monsters(admin),
                    |t| &t.key,
                    |t| &t.name,
                    &c.key,
                )
                .copied()
                .ok_or_else(|| format!("нет такого монстра {:?}", c.key))?;
                let e = &self.ents[&p];
                if !e.alive() {
                    return Err("герой мёртв".into());
                }
                if !admin && self.safe_zone(p) {
                    return Err("герой в деревне".into());
                }
                let n = c.amount.clamp(1, if admin { 10 } else { 5 });
                let (level, lvl, pos) = (e.level.clone(), e.p().level.max(1), e.pos);
                let ang = self.roll(0.0, std::f64::consts::TAU) as f32;
                let dist = self.roll(6.0, 9.0) as f32;
                let at = (pos + Vec2::new(ang.cos(), ang.sin()) * dist).cell();
                let at = self.free_spot(&level, at);
                for _ in 0..n {
                    let q = self.free_spot(&level, at);
                    self.new_monster(d, &level, q, lvl);
                }
                self.index_levels();
                tell(self, String::new());
                Ok(format!("{} ×{n} рядом с героем {pname}", d.name))
            }
            "give_item" => {
                let all: Vec<&crate::content::ItemDef> = db()
                    .b
                    .items
                    .iter()
                    .filter(|it| it.kind != "gold" && (admin || Game::master_gift(it)))
                    .collect();
                let d = find_def(&all, |t| &t.key, |t| &t.name, &c.key)
                    .copied()
                    .ok_or_else(|| format!("нет такого предмета {:?}", c.key))?;
                let st = ItemStack::new(&d.key);
                if !self.add_item(p, st.clone()) {
                    let (level, cell) = (self.ents[&p].level.clone(), self.ents[&p].cell());
                    self.drop_item(&level, cell, st);
                }
                tell(self, format!("Вы получаете: {}.", d.name));
                Ok(format!("{} для героя {pname}", d.name))
            }
            "give_gold" => {
                let lvl = self.ents[&p].p().level.max(1);
                let cap = if admin { 100_000 } else { (30 * lvl).min(600) };
                let n = c.amount.clamp(1, cap);
                self.ents.get_mut(&p).unwrap().pm().gold += n;
                tell(self, format!("Вы находите {n} золота."));
                Ok(format!("{n} золота для героя {pname}"))
            }
            "heal" => {
                let e = self.ents.get_mut(&p).unwrap();
                if e.dead {
                    return Err("герой мёртв".into());
                }
                e.hp = e.max_hp;
                e.mp = e.max_mp;
                tell(self, "Раны затягиваются сами собой.".into());
                Ok(format!("исцеление героя {pname}"))
            }
            "bless" => {
                if self.ents[&p].dead {
                    return Err("герой мёртв".into());
                }
                let e = self.ents.get_mut(&p).unwrap();
                e.buffs.retain(|b| b.def.key != "gm_blessing");
                e.recalc();
                let b = BuffDef {
                    key: "gm_blessing".into(),
                    name: "Благословение мира".into(),
                    duration_ms: 120_000,
                    stats: [
                        ("melee_pct", 15.0),
                        ("ranged_pct", 15.0),
                        ("spell_pct", 15.0),
                        ("hp_regen", 2.0),
                    ]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect(),
                    color: GM_COLOR.into(),
                    ..Default::default()
                };
                self.apply_buff(p, &b, p);
                tell(self, "На вас нисходит благословение.".into());
                Ok(format!("благословение героя {pname}"))
            }
            "rumor" => {
                if text.is_empty() {
                    return Err("пустой слух".into());
                }
                self.chronicle(text.clone());
                Ok(format!("слух: {text}"))
            }
            "message" => {
                if text.is_empty() {
                    return Err("пустое послание".into());
                }
                self.log(p, GM_COLOR, text.clone());
                Ok(format!("послание герою {pname}"))
            }
            "set_time" => {
                let target = match c.key.trim().to_lowercase().as_str() {
                    "day" | "noon" => 0.5,
                    "night" | "midnight" => 0.0,
                    "dawn" | "morning" => 0.24,
                    "dusk" | "evening" => 0.78,
                    _ => return Err(format!("неизвестное время {:?}", c.key)),
                };
                self.set_clock(target);
                tell(self, String::new());
                Ok(format!("время суток: {}", self.time_name()))
            }
            "summon_unique" => {
                if !admin {
                    return Err("только по просьбе администратора".into());
                }
                let u = find_def(&db().b.uniques, |t| &t.key, |t| &t.name, &c.key)
                    .ok_or_else(|| format!("нет такого персонажа {:?}", c.key))?;
                if self.unique_in_world(&u.key).is_some() {
                    return Err(format!("{} уже в мире", u.name));
                }
                let e = &self.ents[&p];
                if e.level != "overworld" {
                    return Err("уникальные персонажи живут на поверхности".into());
                }
                let at = self.free_spot("overworld", (e.pos + e.facing_vec() * 2.5).cell());
                self.spawn_unique(u, at, 60);
                self.index_levels();
                tell(self, String::new());
                Ok(format!("{} рядом с героем {pname}", u.name))
            }
            _ => Err("неизвестная команда".into()),
        }
    }

    /// The admin's /gm: a wish for the master, or on/off for its own events.
    pub(crate) fn admin_master(&mut self, id: Id, wish: &str) {
        let wish = wish.trim();
        match wish.to_lowercase().as_str() {
            "on" | "вкл" => {
                self.gm.on = true;
                self.gm.next_at = None;
                self.say_admin(id, "ИИ-мастер сам устраивает события: вкл.".into());
                return;
            }
            "off" | "выкл" => {
                self.gm.on = false;
                self.say_admin(id, "ИИ-мастер сам устраивает события: выкл.".into());
                return;
            }
            "" => {
                self.admin_ai(id);
                self.say_admin(id, "Использование: /gm просьба, /gm on или /gm off.".into());
                return;
            }
            _ => {}
        }
        let Some(b) = self.brain.clone() else {
            self.say_admin(id, "Нейросеть выключена (Настройки → Нейросеть).".into());
            return;
        };
        if !b.enabled() {
            self.admin_ai(id);
            return;
        }
        if self.ask_master(Some(id), wish) {
            self.say_admin(id, "ИИ-мастер обдумывает просьбу...".into());
        } else {
            self.say_admin(id, "ИИ-мастер занят, попробуйте чуть позже.".into());
        }
    }

    /// The admin's /ai: which model thinks for the world and how it is.
    pub(crate) fn admin_ai(&mut self, id: Id) {
        let Some(b) = self.brain.clone() else {
            self.say_admin(id, "Нейросеть выключена: работает простой ИИ.".into());
            return;
        };
        let state = match b.local_state() {
            Some(st) => st.describe(),
            None if b.usable() => "подключена".into(),
            None => "отключена".into(),
        };
        self.say_admin(id, format!("Нейросеть: {} — {state}.", b.title()));
        let on = if self.gm.on { "вкл" } else { "выкл" };
        self.say_admin(id, format!("ИИ-мастер сам устраивает события: {on}."));
        let err = b.last_error();
        if !err.is_empty() {
            self.say_admin(id, format!("Последняя ошибка: {err}."));
        }
    }
}
