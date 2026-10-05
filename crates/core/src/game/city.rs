//! Big cities: their traders also sell a daily stock of rare things, their
//! craftsmen offer services, and townsfolk keep a day and night schedule.

use super::dialogue::DialogueOption;
use super::*;
use crate::content::{BuffDef, ItemDef};
use crate::proto::TradeItem;
use crate::world::{find_path, Vec2};

/// City traders sell their rare stock dearer than its worth.
const STOCK_MARKUP: f64 = 1.25;
const SONG_PRICE: i32 = 15;
const BLESS_PRICE: i32 = 30;

fn stock_fits(d: &ItemDef, kinds: &[String]) -> bool {
    kinds.contains(&d.kind) || (d.kind == "weapon" && kinds.contains(&d.weapon))
}

fn stock_price(st: &ItemStack) -> i32 {
    (st.value() as f64 * STOCK_MARKUP) as i32
}

fn rest_price(level: i32) -> i32 {
    10 + 2 * level
}

/// Each step of rarity costs more; artifacts and legendary items cannot be
/// improved.
fn upgrade_price(st: &ItemStack) -> Option<i32> {
    let d = st.def()?;
    let r = st.item_rarity();
    if d.unique || !d.rarity.is_empty() || r >= LEGENDARY {
        return None;
    }
    Some(60 * (1 << r) + d.value * (r + 1))
}

fn buff(key: &str, name: &str, ms: i64, color: &str, stats: &[(&str, f64)]) -> BuffDef {
    BuffDef {
        key: key.into(),
        name: name.into(),
        duration_ms: ms,
        stats: stats.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        color: color.into(),
        ..Default::default()
    }
}

impl Game {
    pub(crate) fn day(&self) -> i32 {
        (self.now / DAY_MS) as i32
    }

    pub(crate) fn is_city(&self, village: &str) -> bool {
        self.villages.iter().any(|v| v.name == village && v.city)
    }

    /// A citizen of a big city (services and daily stock work only there).
    pub(crate) fn city_npc(&self, npc: Id) -> bool {
        self.ents[&npc]
            .npc
            .as_ref()
            .map(|n| !n.village.is_empty() && self.is_city(&n.village))
            .unwrap_or(false)
    }

    /// Renews the daily stock of a city trader: random equipment of its
    /// kinds, from uncommon to (rarely) legendary.
    pub(crate) fn refresh_stock(&mut self, npc: Id) {
        let day = self.day();
        let n = self.ents[&npc].npc.as_ref().unwrap();
        let role = db().npc_role(&n.role);
        let Some(role) = role.filter(|r| r.stock > 0) else {
            self.ents.get_mut(&npc).unwrap().npc.as_mut().unwrap().stock = None;
            return;
        };
        if !self.city_npc(npc) {
            self.ents.get_mut(&npc).unwrap().npc.as_mut().unwrap().stock = None;
            return;
        }
        if n.stock.is_some() && n.stock_day == day {
            return;
        }
        let pool: Vec<&ItemDef> = db()
            .b
            .items
            .iter()
            .filter(|d| {
                d.weight > 0
                    && !d.unique
                    && d.rarity.is_empty()
                    && !slot_for(Some(d)).is_empty()
                    && stock_fits(d, &role.stock_kinds)
            })
            .collect();
        let mut stock = Vec::new();
        if !pool.is_empty() {
            let lvl = self
                .online
                .values()
                .filter_map(|id| self.ents.get(id))
                .map(|p| p.p().level)
                .max()
                .unwrap_or(3)
                .max(3);
            for _ in 0..role.stock {
                let d = pool[self.rng.usize_n(pool.len())];
                let x = self.rng.f64() * 100.0;
                let r = if x < 3.0 {
                    LEGENDARY
                } else if x < 20.0 {
                    EPIC
                } else if x < 60.0 {
                    RARE
                } else {
                    UNCOMMON
                };
                let st = self.roll_rarity(ItemStack::new(&d.key), r, lvl);
                stock.push(st);
            }
        }
        let n = self.ents.get_mut(&npc).unwrap().npc.as_mut().unwrap();
        n.stock_day = day;
        n.stock = Some(stock);
    }

    /// The fixed goods first, then the daily stock.
    pub(crate) fn trade_list(&mut self, npc: Id) -> Vec<TradeItem> {
        let role = db().npc_role(&self.ents[&npc].npc.as_ref().unwrap().role);
        let Some(role) = role.filter(|r| r.trader) else {
            return Vec::new();
        };
        let mut out: Vec<TradeItem> = role
            .goods
            .iter()
            .map(|k| {
                let st = ItemStack::new(k);
                TradeItem {
                    item: item_view(&st),
                    price: st.value(),
                }
            })
            .collect();
        self.refresh_stock(npc);
        for st in self.ents[&npc].npc.as_ref().unwrap().stock.iter().flatten() {
            out.push(TradeItem {
                item: item_view(st),
                price: stock_price(st),
            });
        }
        out
    }

    /// Buys item `key` at row idx of the trade list.
    pub(crate) fn buy(&mut self, p: Id, key: &str, idx: i32) {
        let Some(npc) = self.talking_to(p) else {
            return;
        };
        let n = self.ents[&npc].npc.as_ref().unwrap();
        let Some(role) = db().npc_role(&n.role).filter(|r| r.trader) else {
            return;
        };
        let si = idx - role.goods.len() as i32;
        let stock = n.stock.clone().unwrap_or_default();
        let (st, price, from_stock) =
            if si >= 0 && (si as usize) < stock.len() && stock[si as usize].key == key {
                let st = stock[si as usize].clone();
                let pr = stock_price(&st);
                (st, pr, Some(si as usize))
            } else if role.goods.iter().any(|g| g == key) {
                let st = ItemStack::new(key);
                let pr = st.value();
                (st, pr, None)
            } else {
                return;
            };
        if self.ents[&p].p().gold < price {
            self.log(p, "#ff8080", format!("Не хватает золота (нужно {price})."));
            return;
        }
        if !self.add_item(p, st.clone()) {
            self.log(p, "#ff8080", "Инвентарь полон!".into());
            return;
        }
        self.ents.get_mut(&p).unwrap().pm().gold -= price;
        let n = self.ents.get_mut(&npc).unwrap().npc.as_mut().unwrap();
        n.gold += price / 4;
        if let Some(i) = from_stock {
            n.stock.as_mut().unwrap().remove(i);
            self.send_dialogue(
                p,
                npc,
                "Отличный выбор! Такой вещи больше ни у кого нет.",
                true,
                false,
            );
        }
        self.log(
            p,
            "#ffd700",
            format!("Куплено: {} за {price} золота.", st.name()),
        );
    }

    // ---- services ----

    /// The extra conversation lines of a city craftsman.
    pub(crate) fn service_options(&self, p: Id, npc: Id) -> Vec<DialogueOption> {
        let role = db().npc_role(&self.ents[&npc].npc.as_ref().unwrap().role);
        let Some(role) = role else { return Vec::new() };
        if !self.city_npc(npc) {
            return Vec::new();
        }
        let pl = self.ents[&p].p();
        let mut opts = Vec::new();
        for s in &role.services {
            match s.as_str() {
                "rest" => opts.push(DialogueOption {
                    label: format!(
                        "Снять комнату и поужинать ({} золота)",
                        rest_price(pl.level)
                    ),
                    action: "rest",
                }),
                "upgrade" => {
                    if let Some(price) = pl.equip.get(SLOT_MAIN).and_then(upgrade_price) {
                        opts.push(DialogueOption {
                            label: format!("Улучшить оружие в руке ({price} золота)"),
                            action: "upgrade",
                        });
                    }
                }
                "song" => opts.push(DialogueOption {
                    label: format!("Спой мне песню ({SONG_PRICE} золота)"),
                    action: "song",
                }),
                "bless" => opts.push(DialogueOption {
                    label: format!("Благослови меня ({BLESS_PRICE} золота)"),
                    action: "bless",
                }),
                _ => {}
            }
        }
        opts
    }

    pub(crate) fn pay(&mut self, p: Id, npc: Id, price: i32) -> bool {
        let pm = self.ents.get_mut(&p).unwrap().pm();
        if pm.gold < price {
            return false;
        }
        pm.gold -= price;
        pm.dirty = true;
        self.ents.get_mut(&npc).unwrap().npc.as_mut().unwrap().gold += price / 2;
        true
    }

    /// Performs a paid service and returns the NPC's answer.
    pub(crate) fn service(&mut self, p: Id, npc: Id, kind: &str) -> String {
        let (level, pos, plvl, pname) = {
            let e = &self.ents[&p];
            (e.level.clone(), e.pos, e.p().level, e.name.clone())
        };
        match kind {
            "rest" => {
                if !self.pay(p, npc, rest_price(plvl)) {
                    return "Комната стоит денег, друг. Приходи, когда разбогатеешь.".into();
                }
                let b = buff(
                    "rested",
                    "Отдых",
                    600000,
                    "#ffd8a0",
                    &[("max_hp", 20.0), ("hp_regen", 1.0), ("mp_regen", 0.5)],
                );
                self.apply_buff(p, &b, npc);
                let e = self.ents.get_mut(&p).unwrap();
                e.hp = e.max_hp;
                e.mp = e.max_mp;
                self.fx(&level, pos, "Отдых", '\0', &b.color, 1200);
                self.pick(
                    npc,
                    &[
                        "Мягкая постель, горячий ужин — и ты как новенький!",
                        "Выспался? Вот и славно. Дорога ждёт.",
                        "Ужин за счёт заведения. Шучу — уже оплачен.",
                    ],
                )
            }
            "upgrade" => {
                let Some(st) = self.ents[&p].p().equip.get(SLOT_MAIN).cloned() else {
                    return "С этим я ничего не сделаю.".into();
                };
                let Some(price) = upgrade_price(&st) else {
                    return "С этим я ничего не сделаю.".into();
                };
                if !self.pay(p, npc, price) {
                    return format!("Работа тонкая — {price} золота, не меньше.");
                }
                let r0 = st.item_rarity();
                let st = self.roll_rarity(st, r0 + 1, plvl.max(1));
                let e = self.ents.get_mut(&p).unwrap();
                e.pm().equip.insert(SLOT_MAIN.into(), st.clone());
                e.recalc();
                let r = st.item_rarity();
                self.log(
                    p,
                    rarity_color(r),
                    format!("Улучшено: {} ({}).", st.name(), lower(rarity_name(r))),
                );
                self.fx(
                    &level,
                    pos,
                    &format!("{}!", rarity_name(r)),
                    '\0',
                    rarity_color(r),
                    1500,
                );
                format!("Держи. Перековал, заточил, закалил — теперь это {} клинок, не хуже королевского.", lower(rarity_name(r)))
            }
            "song" => {
                if !self.pay(p, npc, SONG_PRICE) {
                    return "Песня стоит монету, а у тебя и той нет. Ладно, напою бесплатно: ля-ля-ля.".into();
                }
                let b = buff(
                    "inspired",
                    "Вдохновение",
                    300000,
                    "#ff80c0",
                    &[
                        ("melee_pct", 10.0),
                        ("spell_pct", 10.0),
                        ("ranged_pct", 10.0),
                    ],
                );
                self.apply_buff(p, &b, npc);
                self.say(
                    npc,
                    &format!("Ла-ла! О герое по имени {pname} сложат песни!"),
                    4000.0,
                );
                self.pick(
                    npc,
                    &[
                        "Эта баллада — о тебе! Иди и сделай её правдой.",
                        "Песня о храбреце, что не знал страха. Узнаёшь?",
                        "Пусть мелодия ведёт твой клинок!",
                    ],
                )
            }
            "bless" => {
                if !self.pay(p, npc, BLESS_PRICE) {
                    return "Свет не торгует, но храм нуждается в пожертвованиях.".into();
                }
                let b = buff(
                    "blessed",
                    "Благословение",
                    300000,
                    "#fff0b0",
                    &[("res_all", 10.0), ("res_shadow", 10.0)],
                );
                self.apply_buff(p, &b, npc);
                self.fx(&level, pos, "Благословение", '\0', &b.color, 1200);
                "Да хранит тебя Свет от тьмы и от дурной стали.".into()
            }
            _ => "...".into(),
        }
    }

    // ---- schedule ----

    /// Where a citizen wants to be now: around the tavern at night, at home
    /// by day.
    pub(crate) fn schedule_spot(&self, id: Id) -> Vec2 {
        let n = self.ents[&id].npc.as_ref().unwrap();
        match n.night {
            Some(night) if self.is_night() => night,
            _ => n.home,
        }
    }

    /// Takes the next step along a path to the target.
    pub(crate) fn walk_toward(&mut self, id: Id, to: Vec2) -> bool {
        let e = &self.ents[&id];
        let l = &self.levels[&e.level];
        let (from, goal) = (e.cell(), to.cell());
        if from == goal {
            self.walk_to(id, to);
            return true;
        }
        let path = find_path(l.w, l.h, from, goal, 4000, true, &mut |x, y| {
            if x == goal.x && y == goal.y {
                return 1.0; // the spot itself may be a door
            }
            let def = l.def(x, y);
            if !def.walkable || !def.interact.is_empty() || def.damage > 0.0 {
                return -1.0;
            }
            1.0
        });
        let Some(&next) = path.first() else {
            return false;
        };
        let target = if next == goal && l.walkable(goal.x, goal.y) {
            to
        } else if next == goal {
            return false; // a closed door: stand next to it
        } else {
            next.center()
        };
        let e = self.ents.get_mut(&id).unwrap();
        e.pace = 0.65;
        e.goal = Some(target);
        true
    }
}
