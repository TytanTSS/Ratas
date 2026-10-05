//! Admin mode is for testing: the host started with --admin (or everyone on a
//! dedicated server started with --admin) can type commands into the chat.

use super::*;
use crate::content::BuffDef;
use crate::world::Bitset;

const ADMIN_COLOR: &str = "#ff9a3a";

/// One command for the help and the quick panel.
pub struct AdminCommand {
    pub name: &'static str,
    pub args: &'static str,
    pub desc: &'static str,
}

/// The commands in the order of the help.
pub const ADMIN_COMMANDS: &[AdminCommand] = &[
    AdminCommand {
        name: "/god",
        args: "",
        desc: "бессмертие вкл/выкл",
    },
    AdminCommand {
        name: "/nocd",
        args: "",
        desc: "умения без маны и перезарядки вкл/выкл",
    },
    AdminCommand {
        name: "/heal",
        args: "",
        desc: "полное здоровье и мана, снять эффекты",
    },
    AdminCommand {
        name: "/level",
        args: "N",
        desc: "поднять героя до уровня N",
    },
    AdminCommand {
        name: "/xp",
        args: "N",
        desc: "дать N опыта",
    },
    AdminCommand {
        name: "/gold",
        args: "N",
        desc: "дать N золота",
    },
    AdminCommand {
        name: "/points",
        args: "N",
        desc: "дать N очков навыков и характеристик",
    },
    AdminCommand {
        name: "/give",
        args: "предмет [кол-во] [редкость]",
        desc: "дать предмет по ключу или части названия",
    },
    AdminCommand {
        name: "/items",
        args: "[фильтр]",
        desc: "список предметов",
    },
    AdminCommand {
        name: "/spawn",
        args: "монстр [кол-во] [уровень]",
        desc: "призвать монстров рядом",
    },
    AdminCommand {
        name: "/monsters",
        args: "[фильтр]",
        desc: "список монстров",
    },
    AdminCommand {
        name: "/kill",
        args: "[радиус]",
        desc: "убить врагов вокруг (по умолчанию 12)",
    },
    AdminCommand {
        name: "/tp",
        args: "X Y | уровень",
        desc: "телепорт: в точку или на уровень (d3-2, overworld)",
    },
    AdminCommand {
        name: "/levels",
        args: "",
        desc: "список уровней мира",
    },
    AdminCommand {
        name: "/find",
        args: "имя",
        desc: "телепорт к NPC, уникальному персонажу или монстру",
    },
    AdminCommand {
        name: "/unique",
        args: "ключ",
        desc: "поставить уникального персонажа рядом (даже если его нет в мире)",
    },
    AdminCommand {
        name: "/uniques",
        args: "",
        desc: "список уникальных персонажей",
    },
    AdminCommand {
        name: "/unlock",
        args: "all | class:ключ | subclass:ключ | skill:ключ",
        desc: "открыть секреты",
    },
    AdminCommand {
        name: "/reveal",
        args: "",
        desc: "открыть карту текущего уровня",
    },
    AdminCommand {
        name: "/time",
        args: "day | night | ЧЧ",
        desc: "сменить время суток",
    },
    AdminCommand {
        name: "/speed",
        args: "N",
        desc: "бонус к скорости бега в процентах (0 — снять)",
    },
];

fn on_off(b: bool) -> &'static str {
    if b {
        "вкл"
    } else {
        "выкл"
    }
}

fn is_number(s: &str) -> bool {
    s.parse::<i32>().is_ok()
}

/// Separates trailing numbers and words from a name of several words.
fn split_tail(args: &[String], is_tail: impl Fn(&str) -> bool) -> (String, Vec<String>) {
    let mut i = args.len();
    while i > 1 && is_tail(&args[i - 1]) {
        i -= 1;
    }
    (args[..i].join(" "), args[i..].to_vec())
}

/// Finds a definition by exact key, then by a part of the key or name.
fn find_def<'a, T>(
    all: &'a [T],
    key: impl Fn(&T) -> &str,
    name: impl Fn(&T) -> &str,
    q: &str,
) -> Option<&'a T> {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    let k = q.replace(' ', "_"); // "long sword" is long_sword
    all.iter().find(|t| key(t) == k).or_else(|| {
        all.iter()
            .find(|t| key(t).contains(&k) || name(t).to_lowercase().contains(&q))
    })
}

impl Game {
    pub(crate) fn say_admin(&mut self, id: Id, text: String) {
        self.log(id, ADMIN_COLOR, text);
    }

    /// Runs an admin command line; the caller checks the rights.
    pub fn admin(&mut self, id: Id, line: &str) {
        if self.ents.get(&id).and_then(|e| e.player.as_ref()).is_none() {
            return;
        }
        let f: Vec<String> = line
            .trim()
            .trim_start_matches('/')
            .split_whitespace()
            .map(String::from)
            .collect();
        if f.is_empty() {
            return;
        }
        let cmd = f[0].to_lowercase();
        let args = &f[1..];
        let num = |i: usize, def: i32| {
            args.get(i)
                .and_then(|a| a.parse::<i32>().ok())
                .unwrap_or(def)
        };
        let say = |g: &mut Game, s: String| g.say_admin(id, s);
        match cmd.as_str() {
            "help" | "?" => {
                for c in ADMIN_COMMANDS {
                    say(self, format!("{} {} — {}", c.name, c.args, c.desc));
                }
            }
            "god" => {
                let p = self.ents.get_mut(&id).unwrap().pm();
                p.god = !p.god;
                let v = p.god;
                say(self, format!("Бессмертие: {}.", on_off(v)));
            }
            "nocd" => {
                let p = self.ents.get_mut(&id).unwrap().pm();
                p.no_cd = !p.no_cd;
                let v = p.no_cd;
                say(self, format!("Без маны и перезарядки: {}.", on_off(v)));
            }
            "heal" => {
                let e = self.ents.get_mut(&id).unwrap();
                e.buffs.clear();
                e.recalc();
                e.hp = e.max_hp;
                e.mp = e.max_mp;
                say(self, "Здоровье и мана восстановлены.".into());
            }
            "level" | "lvl" => {
                let cur = self.ents[&id].p().level;
                let n = num(0, cur + 1).min(99);
                while self.ents[&id].p().level < n {
                    let p = self.ents[&id].p();
                    let need = xp_for_level(p.level) - p.xp;
                    self.give_xp(id, need.max(1));
                }
                let lvl = self.ents[&id].p().level;
                say(self, format!("Уровень героя: {lvl}."));
            }
            "xp" => self.give_xp(id, num(0, 1000)),
            "gold" => {
                let n = num(0, 1000);
                self.ents.get_mut(&id).unwrap().pm().gold += n;
                say(self, format!("+{n} золота."));
            }
            "points" => {
                let n = num(0, 10);
                let p = self.ents.get_mut(&id).unwrap().pm();
                p.skill_points += n;
                p.attr_points += n;
                say(self, format!("+{n} очков навыков и характеристик."));
            }
            "give" => self.admin_give(id, args),
            "items" => {
                let all: Vec<String> = db()
                    .b
                    .items
                    .iter()
                    .filter(|i| i.kind != "gold")
                    .map(|i| format!("{} — {}", i.key, i.name))
                    .collect();
                self.admin_list(id, args, &all);
            }
            "spawn" => self.admin_spawn(id, args),
            "monsters" => {
                let all: Vec<String> = db()
                    .b
                    .monsters
                    .iter()
                    .map(|m| format!("{} — {}", m.key, m.name))
                    .collect();
                self.admin_list(id, args, &all);
            }
            "kill" => {
                let r = num(0, 12) as f32;
                let e = &self.ents[&id];
                let (level, pos) = (e.level.clone(), e.pos);
                let mut n = 0;
                for o in self.on_level(&level) {
                    let Some(oe) = self.ents.get(&o) else {
                        continue;
                    };
                    if oe.monster.is_some()
                        && oe.alive()
                        && self.hostile(id, o)
                        && oe.pos.dist(pos) <= r
                    {
                        self.ents.get_mut(&o).unwrap().hp = 0.0;
                        self.kill(o, Some(id));
                        n += 1;
                    }
                }
                say(self, format!("Убито врагов: {n}."));
            }
            "tp" => self.admin_teleport(id, args),
            "levels" => {
                let mut ids: Vec<String> = self.levels.keys().cloned().collect();
                ids.sort();
                for lid in ids {
                    let name = self.levels[&lid].name.clone();
                    say(self, format!("{lid} — {name}"));
                }
                let n = self.entrances.len();
                say(self, format!("Подземелий: {n} (d<номер>-<глубина>, ещё не созданные уровни создаются при входе)."));
            }
            "find" => self.admin_find(id, &args.join(" ")),
            "unique" => self.admin_unique(id, &args.join(" ")),
            "uniques" => {
                for u in &db().b.uniques {
                    let where_ = self
                        .ents
                        .values()
                        .find(|o| o.npc.as_ref().map(|n| n.unique == u.key).unwrap_or(false))
                        .map(|o| format!("({},{})", o.cell().x, o.cell().y))
                        .unwrap_or_else(|| "нет в этом мире".into());
                    say(
                        self,
                        format!(
                            "{} — {}, {}: {} → {}",
                            u.key,
                            u.name,
                            lower(&u.title),
                            where_,
                            u.reward
                        ),
                    );
                }
            }
            "unlock" => self.admin_unlock(id, args),
            "reveal" => {
                let level = self.ents[&id].level.clone();
                let l = &self.levels[&level];
                let mut bs = Bitset::new((l.w * l.h) as usize);
                bs.fill();
                let nlm = self.landmarks.len();
                let p = self.ents.get_mut(&id).unwrap().pm();
                p.explored.insert(level.clone(), bs);
                if level == "overworld" {
                    p.found = (0..nlm).collect();
                }
                p.resync = true;
                p.dirty = true;
                say(self, "Карта уровня открыта.".into());
            }
            "time" => {
                let target = match args.first().map(|s| s.as_str()) {
                    None | Some("day") => 0.5,
                    Some("night") => 0.0,
                    _ => (num(0, 12) % 24) as f64 / 24.0,
                };
                // moving the clock forward keeps every timer consistent
                self.now += (target - self.time_of_day() + 1.0).rem_euclid(1.0) * DAY_MS;
                say(
                    self,
                    format!("Время: {:02}:00.", ((target * 24.0).round() as i32) % 24),
                );
            }
            "speed" => {
                let n = num(0, 100);
                let e = self.ents.get_mut(&id).unwrap();
                e.buffs.retain(|b| b.def.key != "admin_speed");
                e.recalc();
                if n != 0 {
                    let b = BuffDef {
                        key: "admin_speed".into(),
                        name: "Скорость (админ)".into(),
                        duration_ms: 24 * 3600000,
                        stats: [("move_speed".to_string(), n as f64)].into_iter().collect(),
                        ..Default::default()
                    };
                    self.apply_buff(id, &b, id);
                }
                say(self, format!("Бонус к скорости бега: {n}%."));
            }
            _ => say(self, format!("Неизвестная команда /{cmd}. Список: /help.")),
        }
        if let Some(p) = self.ents.get_mut(&id).and_then(|e| e.player.as_mut()) {
            p.dirty = true;
        }
    }

    pub(crate) fn admin_list(&mut self, id: Id, args: &[String], all: &[String]) {
        let filter = args.join(" ").to_lowercase();
        let mut n = 0;
        for s in all {
            if !filter.is_empty() && !s.to_lowercase().contains(&filter) {
                continue;
            }
            if n == 40 {
                self.say_admin(id, "…и ещё. Уточните фильтр.".into());
                return;
            }
            self.say_admin(id, s.clone());
            n += 1;
        }
        if n == 0 {
            self.say_admin(id, "Ничего не найдено.".into());
        }
    }

    pub(crate) fn admin_give(&mut self, id: Id, args: &[String]) {
        let (name, tail) = split_tail(args, |s| is_number(s) || rarity_by_name(s) >= 0);
        let Some(d) = find_def(&db().b.items, |t| &t.key, |t| &t.name, &name) else {
            self.say_admin(
                id,
                format!("Нет такого предмета: {name:?}. Список: /items фильтр."),
            );
            return;
        };
        let (mut qty, mut rarity) = (1, -1);
        for t in &tail {
            match t.parse::<i32>() {
                Ok(v) => qty = v.clamp(1, 999),
                Err(_) => rarity = rarity_by_name(t),
            }
        }
        if d.kind == "gold" {
            self.ents.get_mut(&id).unwrap().pm().gold += qty;
            return;
        }
        let lvl = self.ents[&id].p().level.max(1);
        let mut last = ItemStack::new(&d.key);
        for _ in 0..qty {
            let mut st = ItemStack::new(&d.key);
            if rarity >= 0 && !slot_for(Some(d)).is_empty() {
                st = self.roll_rarity(st, rarity, lvl);
            }
            if !self.add_item(id, st.clone()) {
                let (level, cell) = (self.ents[&id].level.clone(), self.ents[&id].cell());
                self.drop_item(&level, cell, st.clone());
            }
            last = st;
        }
        self.say_admin(
            id,
            format!(
                "Получено: {} ×{qty} ({}).",
                last.name(),
                lower(rarity_name(last.item_rarity()))
            ),
        );
    }

    pub(crate) fn admin_spawn(&mut self, id: Id, args: &[String]) {
        let (name, tail) = split_tail(args, is_number);
        let Some(d) = find_def(&db().b.monsters, |t| &t.key, |t| &t.name, &name) else {
            self.say_admin(
                id,
                format!("Нет такого монстра: {name:?}. Список: /monsters фильтр."),
            );
            return;
        };
        let n = tail
            .first()
            .and_then(|t| t.parse().ok())
            .unwrap_or(1)
            .clamp(1, 30);
        let lvl = tail
            .get(1)
            .and_then(|t| t.parse().ok())
            .unwrap_or(self.ents[&id].p().level.max(1))
            .clamp(1, 99);
        let e = &self.ents[&id];
        let level = e.level.clone();
        let front = (e.pos + e.facing_vec() * 2.5).cell();
        for _ in 0..n {
            let p = self.free_spot(&level, front);
            self.new_monster(d, &level, p, lvl);
        }
        self.index_levels();
        self.say_admin(id, format!("Призвано: {} ×{n} (уровень {lvl}).", d.name));
    }

    pub(crate) fn admin_teleport(&mut self, id: Id, args: &[String]) {
        if args.len() >= 2 && is_number(&args[0]) && is_number(&args[1]) {
            let (x, y) = (args[0].parse().unwrap(), args[1].parse().unwrap());
            let level = self.ents[&id].level.clone();
            let l = &self.levels[&level];
            if !l.inside(x, y) {
                let (w, h) = (l.w, l.h);
                self.say_admin(id, format!("Точка вне уровня ({w}×{h})."));
                return;
            }
            let p = self.free_spot(&level, Pos::new(x, y));
            self.change_level(id, &level, p);
            return;
        }
        if args.len() == 1 {
            if !self.ensure_level(&args[0]) {
                self.say_admin(id, format!("Нет уровня {:?}. Список: /levels.", args[0]));
                return;
            }
            let l = &self.levels[&args[0]];
            let p = if l.depth == 0 { self.start } else { l.up };
            self.change_level(id, &args[0].clone(), p);
            return;
        }
        self.say_admin(id, "Использование: /tp X Y или /tp уровень.".into());
    }

    pub(crate) fn admin_find(&mut self, id: Id, q: &str) {
        let q = q.trim().to_lowercase();
        if q.is_empty() {
            self.say_admin(id, "Использование: /find имя.".into());
            return;
        }
        let me = &self.ents[&id];
        let mut best: Option<Id> = None;
        for (oid, o) in &self.ents {
            if *oid == id || (o.npc.is_none() && o.monster.is_none()) || !o.alive() {
                continue;
            }
            let mut m = o.name.to_lowercase().contains(&q);
            if let Some(n) = &o.npc {
                m |= n.unique == q || n.role == q;
            }
            if let Some(ms) = &o.monster {
                m |= ms.def == q;
            }
            if !m {
                continue;
            }
            let better = match best.map(|b| &self.ents[&b]) {
                None => true,
                Some(b) => o.level == me.level && (b.level != me.level || o.dist(me) < b.dist(me)),
            };
            if better {
                best = Some(*oid);
            }
        }
        let Some(b) = best else {
            self.say_admin(id, format!("Никого не найдено по {q:?}."));
            return;
        };
        self.move_next_to(id, b);
        let be = &self.ents[&b];
        let text = format!(
            "Вы рядом: {} ({}, {},{}).",
            be.name,
            be.level,
            be.cell().x,
            be.cell().y
        );
        self.say_admin(id, text);
    }

    pub(crate) fn admin_unique(&mut self, id: Id, q: &str) {
        let Some(u) = find_def(&db().b.uniques, |t| &t.key, |t| &t.name, q) else {
            self.say_admin(
                id,
                format!("Нет такого персонажа: {q:?}. Список: /uniques."),
            );
            return;
        };
        if let Some(o) = self
            .ents
            .iter()
            .find(|(_, o)| o.npc.as_ref().map(|n| n.unique == u.key).unwrap_or(false))
            .map(|(i, _)| *i)
        {
            self.move_next_to(id, o);
            self.say_admin(id, format!("{} уже в мире — вы рядом.", u.name));
            return;
        }
        let e = &self.ents[&id];
        if e.level != "overworld" {
            self.say_admin(id, "Уникальные персонажи живут на поверхности.".into());
            return;
        }
        let p = self.free_spot("overworld", (e.pos + e.facing_vec() * 2.5).cell());
        self.spawn_unique(u, p, 60);
        self.say_admin(id, format!("{} появляется рядом.", u.name));
    }

    pub(crate) fn admin_unlock(&mut self, id: Id, args: &[String]) {
        if args.is_empty() {
            self.say_admin(
                id,
                "Использование: /unlock all | class:ключ | subclass:ключ | skill:ключ.".into(),
            );
            return;
        }
        let d = db();
        let rewards: Vec<String> = if args[0] == "all" {
            let mut r: Vec<String> =
                d.b.classes
                    .iter()
                    .filter(|c| c.secret)
                    .map(|c| format!("class:{}", c.key))
                    .collect();
            r.extend(
                d.b.subclasses
                    .iter()
                    .filter(|s| s.secret)
                    .map(|s| format!("subclass:{}", s.key)),
            );
            r.extend(
                d.b.skills
                    .iter()
                    .filter(|s| d.branch(&s.branch).map(|b| b.secret).unwrap_or(false))
                    .map(|s| format!("skill:{}", s.key)),
            );
            r
        } else {
            args.to_vec()
        };
        for r in rewards {
            self.unlock(id, &r);
        }
    }

    /// Places e next to target (on its level).
    pub fn move_next_to(&mut self, id: Id, target: Id) {
        let (tl, tc) = (self.ents[&target].level.clone(), self.ents[&target].cell());
        let p = self.free_spot(&tl, tc);
        if self.ents[&id].level != tl {
            self.change_level(id, &tl, p);
        } else {
            self.place(id, p.center());
        }
    }
}
