//! The world's own story. When a world is made and a model is at hand
//! (Claude, or a local model that is already downloaded), the model writes
//! the history of this world around its real places, and the game turns it
//! into content of this world only: living characters of the story with
//! quests to finish what the story left unfinished, the artifacts they give,
//! the relics they seek, and English translations of all of it. The model
//! only names and tells; the game builds every item on a real one, picks the
//! stats and checks every reference, so the story cannot break the balance.
//!
//! The story and its content live in the save and reach network players
//! with the rest of the content.

use super::dialogue::compass;
use super::*;
use crate::content::{Bundle, ItemDef, UniqueDef};
use crate::llm::{local, LoreReply, LoreRequest, Option_, Text2, LORE_FORMS, LORE_POWERS};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

/// Keys of a world's own content start with this.
pub const LORE_PREFIX: &str = "lore";
/// Characters of a story (at most).
const LORE_CHARACTERS: usize = 3;
/// How long to wait for a local model to start and load.
const LOAD_WAIT: Duration = Duration::from_secs(120);

/// How a character of the story may look: a model and its description.
const LORE_LOOKS: &[(&str, &str)] = &[
    ("elder", "an old village elder"),
    ("priest", "a priest in white robes"),
    ("healer", "a herb-healer woman"),
    ("bard", "a wandering bard"),
    ("knight_errant", "a knight errant in armour"),
    ("hunter", "a hunter with a bow"),
    ("pilgrim", "a pilgrim with a staff"),
    ("mercenary", "a scarred mercenary"),
    ("battle_mage", "a battle mage"),
    ("sun_hermit", "a hermit in a hood"),
    ("stargazer", "a stargazer"),
    ("night_sister", "a sister of a night order"),
    ("rune_carver", "a rune carver"),
    ("grove_warden", "a forest warden"),
    ("oracle_npc", "an oracle"),
    ("mourner", "a mourner in black"),
    ("fallen_paladin", "a fallen paladin"),
    ("plague_doc", "a plague doctor"),
    ("old_alchemist", "an old alchemist"),
    ("wild_huntsman", "a wild huntsman"),
    ("monster_huntress", "a monster huntress"),
    ("archmage_npc", "an archmage"),
    ("dragon_slayer", "a dragon slayer"),
    ("inquisitor_npc", "an inquisitor"),
    ("sand_wanderer", "a desert wanderer"),
    ("old_voivode", "an old warlord"),
    ("mayor", "a town mayor"),
    ("captain", "a guard captain"),
    ("smith", "a smith"),
    ("merchant", "a merchant"),
];

/// Lands a character can live in, with the dungeons where the relics of
/// their quest also turn up.
const LANDS: &[(&str, &str)] = &[
    ("plains", "crypt"),
    ("forest", "cave"),
    ("swamp", "crypt"),
    ("hills", "cave"),
    ("desert", "temple"),
    ("tundra", "ice"),
    ("ash", "volcano"),
    ("cursed", "fortress"),
];

/// The story of a world.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Lore {
    pub title: String,
    pub history: Vec<String>,
    /// the world's own characters, artifacts, relics and their translations
    pub content: Bundle,
}

impl Lore {
    /// The characters of the story.
    pub fn characters(&self) -> &[UniqueDef] {
        &self.content.uniques
    }
}

/// The color and stats an artifact's power gives.
fn power(p: &str) -> (&'static str, &'static [(&'static str, f64)]) {
    match p {
        "fire" => (
            "#ff7a3a",
            &[("fire_pct", 20.0), ("add_fire", 5.0), ("res_fire", 20.0)],
        ),
        "cold" => (
            "#7ad0ff",
            &[("cold_pct", 20.0), ("add_cold", 5.0), ("res_cold", 20.0)],
        ),
        "lightning" => (
            "#e8e05a",
            &[
                ("lightning_pct", 20.0),
                ("add_lightning", 5.0),
                ("res_lightning", 15.0),
            ],
        ),
        "poison" => (
            "#8ad04a",
            &[
                ("poison_pct", 20.0),
                ("add_poison", 4.0),
                ("res_poison", 20.0),
            ],
        ),
        "holy" => (
            "#ffe08a",
            &[
                ("holy_pct", 20.0),
                ("add_holy", 4.0),
                ("res_holy", 15.0),
                ("heal_pct", 10.0),
            ],
        ),
        "shadow" => (
            "#9a6aff",
            &[
                ("shadow_pct", 20.0),
                ("add_shadow", 5.0),
                ("life_leech", 3.0),
            ],
        ),
        "agility" => (
            "#5ae0a0",
            &[("dex", 5.0), ("crit", 6.0), ("attack_speed", 8.0)],
        ),
        "wisdom" => (
            "#6a9aff",
            &[("int", 5.0), ("spell_pct", 15.0), ("max_mp", 25.0)],
        ),
        "vigor" => (
            "#e0a05a",
            &[
                ("vit", 5.0),
                ("max_hp", 30.0),
                ("hp_regen", 1.5),
                ("armor", 4.0),
            ],
        ),
        _ => (
            "#e05a4a",
            &[("str", 5.0), ("melee_pct", 12.0), ("max_hp", 20.0)],
        ),
    }
}

/// The slot (and weapon or look) an artifact form fills.
fn form(f: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match f {
        "sword" => ("weapon", "sword", ""),
        "axe" => ("weapon", "axe", ""),
        "mace" => ("weapon", "mace", ""),
        "hammer" => ("weapon", "hammer", ""),
        "dagger" => ("weapon", "dagger", ""),
        "spear" => ("weapon", "spear", ""),
        "staff" => ("weapon", "staff", ""),
        "wand" => ("weapon", "wand", ""),
        "bow" => ("weapon", "bow", ""),
        "crossbow" => ("weapon", "crossbow", ""),
        "scythe" => ("weapon", "scythe", ""),
        "shield" => ("shield", "", ""),
        "helm" => ("head", "", ""),
        "armor" => ("chest", "", ""),
        "cloak" => ("back", "", ""),
        "belt" => ("belt", "", ""),
        "greaves" => ("legs", "", ""),
        "ring" => ("ring", "", ""),
        "orb" => ("offhand", "", "orb"),
        "tome" => ("offhand", "", "book"),
        "symbol" => ("offhand", "", "symbol"),
        _ => return None,
    })
}

/// Cleans a text of the model: one line, no format characters, at most max
/// characters; "{player}" stays only where allowed.
fn clean(s: &str, max: usize, player: bool) -> String {
    const P: &str = "\u{1}";
    let s = s.replace("{player}", P);
    let s: String = s
        .chars()
        .filter(|&c| c == '\u{1}' || !(c.is_control() || matches!(c, '{' | '}' | '%')))
        .collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s = if s.chars().count() > max {
        let cut: String = s.chars().take(max).collect();
        // end on a word
        match cut.rfind(' ') {
            Some(i) if i > max / 2 => format!("{}…", cut[..i].trim_end_matches([',', ';', ':'])),
            _ => cut + "…",
        }
    } else {
        s
    };
    if player {
        return s.replace(P, "{player}").trim().to_string();
    }
    // "Hello, {player}!" without the name is "Hello!"
    s.replace(&format!(", {P}"), "")
        .replace(&format!("{P}, "), "")
        .replace(P, "")
        .trim()
        .to_string()
}

/// Russian texts of the story with their English translations.
#[derive(Default)]
struct Texts {
    tr: std::collections::HashMap<String, String>,
    /// Russian place names and their English names, longest first
    names: Vec<(String, String)>,
}

impl Texts {
    fn new(names: &[(String, String)]) -> Texts {
        let mut names = names.to_vec();
        names.sort_by_key(|n| std::cmp::Reverse(n.0.chars().count()));
        Texts {
            tr: Default::default(),
            names,
        }
    }

    /// English text in which the model left Russian names: the places get
    /// their English names, other Russian phrases are translated whole.
    fn english(&self, en: String) -> String {
        let mut en = en;
        for (ru, e) in &self.names {
            // the model may copy a name with its hint: "Долина (Vale)"
            en = en
                .replace(&format!("{ru} ({e})"), e)
                .replace(&format!("{e} ({e})"), e)
                .replace(ru.as_str(), e);
        }
        if crate::i18n::has_cyrillic(&en) {
            en = translate_runs(&en);
        }
        en
    }

    /// A Russian text without the English hints the request gave names.
    fn russian(&self, ru: String) -> String {
        let mut ru = ru;
        for (r, e) in &self.names {
            ru = ru.replace(&format!("{r} ({e})"), r);
        }
        ru
    }

    /// The Russian text (English if the model wrote no Russian), its
    /// translation remembered; def when both are empty.
    fn put(&mut self, t: &Text2, max: usize, def: &str) -> String {
        let ru = self.russian(clean(&t.ru, max, true));
        let en = self.english(clean(&t.en, max, ru.contains("{player}")));
        let ru = match (ru.is_empty(), en.is_empty()) {
            (true, true) => return def.to_string(),
            (true, false) => return en,
            _ => ru,
        };
        if crate::i18n::has_cyrillic(&ru) && !en.is_empty() && !crate::i18n::has_cyrillic(&en) {
            self.tr.insert(ru.clone(), en);
        }
        ru
    }
}

/// Translates the Russian phrases inside an English text, each as a whole
/// ("the dungeon Пещера Эха fell" → "the dungeon Cave of Echoes fell").
fn translate_runs(s: &str) -> String {
    let cyr = |c: char| ('\u{400}'..='\u{4ff}').contains(&c);
    let mut out = String::new();
    let mut run = String::new();
    let mut gap = String::new();
    let flush = |out: &mut String, run: &mut String| {
        if !run.is_empty() {
            out.push_str(&crate::i18n::tr_in(crate::i18n::EN, run));
            run.clear();
        }
    };
    for c in s.chars() {
        if cyr(c) {
            if run.is_empty() {
                out.push_str(&gap);
            } else {
                run.push_str(&gap);
            }
            gap.clear();
            run.push(c);
        } else if !run.is_empty() && matches!(c, ' ' | '-') {
            gap.push(c);
        } else {
            flush(&mut out, &mut run);
            out.push_str(&gap);
            gap.clear();
            out.push(c);
        }
    }
    flush(&mut out, &mut run);
    out.push_str(&gap);
    out
}

/// Builds an artifact of the story on the best ordinary item of its slot.
fn artifact(key: &str, f: &str, p: &str, name: String, desc: String) -> Option<ItemDef> {
    let (kind, weapon, look) = form(f)?;
    let base = db()
        .b
        .items
        .iter()
        .filter(|it| {
            it.kind == kind
                && (weapon.is_empty() || it.weapon == weapon)
                && (look.is_empty() || it.look == look)
                && !it.key.starts_with(LORE_PREFIX)
        })
        .max_by_key(|it| (!it.unique, it.value))?;
    let (color, stats) = power(p);
    let mut it = base.clone();
    it.key = key.to_string();
    it.name = name;
    it.desc = desc;
    it.color = color.into();
    it.unique = true;
    it.rarity = "legendary".into();
    it.value = base.value * 2 + 800;
    it.depth = 99;
    it.weight = 0;
    it.drop_from.clear();
    it.drop_chance = 0.0;
    if kind == "weapon" {
        it.damage = it.damage.map(|d| (d * 1.25).round());
    }
    for (k, v) in stats {
        *it.stats.entry(k.to_string()).or_insert(0.0) += v;
    }
    Some(it)
}

/// Turns the model's story into the world's own content. Every reference
/// is checked against the request; what cannot be fixed is left out.
pub(crate) fn build_lore(key: &str, req: &LoreRequest, r: &LoreReply) -> Result<Lore, String> {
    let mut tx = Texts::new(&req.names);
    let history: Vec<String> = r
        .history
        .iter()
        .map(|t| tx.put(t, 450, ""))
        .filter(|s| !s.is_empty())
        .take(5)
        .collect();
    if history.len() < 2 {
        return Err("нейросеть не написала историю".into());
    }
    let title = tx.put(&r.title, 80, "Летопись мира");
    let has = |opts: &[Option_], k: &str| opts.iter().any(|o| o.key == k);
    let mut content = Bundle::default();
    for (i, c) in r.characters.iter().take(LORE_CHARACTERS).enumerate() {
        let name = tx.put(&c.name, 40, "");
        if name.is_empty() {
            continue;
        }
        let ckey = format!("{key}_c{i}");
        let akey = format!("{key}_a{i}");
        let land = if req.lands.contains(&c.land) {
            c.land.clone()
        } else {
            req.lands[i % req.lands.len()].clone()
        };
        let look = if has(&req.looks, &c.look) {
            c.look.clone()
        } else {
            req.looks[i % req.looks.len()].key.clone()
        };
        let pw = if LORE_POWERS.contains(&c.artifact.power.as_str()) {
            c.artifact.power.as_str()
        } else {
            "might"
        };
        let fm = if LORE_FORMS.contains(&c.artifact.form.as_str()) {
            c.artifact.form.as_str()
        } else {
            "sword"
        };
        let art_name = tx.put(&c.artifact.name, 50, "");
        if art_name.is_empty() {
            continue;
        }
        let art_desc = tx.put(&c.artifact.desc, 220, "");
        let Some(art) = artifact(&akey, fm, pw, art_name, art_desc) else {
            continue;
        };
        let mut u = UniqueDef {
            key: ckey,
            title: tx.put(&c.title, 60, "Хранитель летописи"),
            name,
            color: power(pw).0.into(),
            model: look,
            persona: format!(
                "{} A living character of the chronicle \"{}\" of this world.",
                clean(&c.persona, 500, false),
                r.title.en.trim()
            ),
            greeting: tx.put(&c.greeting, 200, "Ты пришёл вовремя, {player}."),
            biomes: vec![land.clone()],
            offer: tx.put(
                &c.offer,
                300,
                "Помоги мне закончить то, что началось много лет назад.",
            ),
            done: tx.put(
                &c.done,
                300,
                "Ты сделал то, что не удалось никому до тебя. Возьми — это твоё по праву.",
            ),
            reward: format!("item:{akey}"),
            ..Default::default()
        };
        let about = tx.put(&c.about, 220, "");
        if !about.is_empty() {
            u.about.push(about);
        }
        let foe = tx.put(&c.foe, 60, "");
        let villain = has(&req.villains, &c.target);
        let boss = has(&req.bosses, &c.target);
        // the model may mix the kinds up: a dungeon lord makes a boss quest,
        // a named villain a slay quest, a named piece a relics quest
        let asked = c.quest.as_str();
        let quest = if boss && asked != "relics" {
            "boss"
        } else if villain && !foe.is_empty() && asked != "relics" {
            "slay"
        } else if !foe.is_empty() || !boss {
            "relics"
        } else {
            "boss"
        };
        match quest {
            "slay" => {
                u.quest = "slay".into();
                u.target = c.target.clone();
                u.champion = foe;
            }
            "boss" => {
                u.quest = "boss".into();
                u.target = c.target.clone();
            }
            _ => {
                let rkey = format!("{key}_r{i}");
                let relic = if foe.is_empty() {
                    "Осколок прошлого".to_string()
                } else {
                    foe
                };
                let dungeon = LANDS
                    .iter()
                    .find(|l| l.0 == land)
                    .map(|l| l.1)
                    .unwrap_or("crypt");
                content.items.push(ItemDef {
                    key: rkey.clone(),
                    name: relic,
                    glyph: "*".into(),
                    color: u.color.clone(),
                    kind: "quest".into(),
                    depth: 99,
                    desc: format!("Частица истории «{title}». Её ждёт {}.", u.name),
                    ..Default::default()
                });
                u.quest = "relics".into();
                u.target = rkey;
                u.count = 5;
                u.sources = vec![land, dungeon.to_string()];
            }
        }
        content.items.push(art);
        content.uniques.push(u);
    }
    if !tx.tr.is_empty() {
        content.translations.insert("en".into(), tx.tr);
    }
    Ok(Lore {
        title,
        history,
        content,
    })
}

impl Game {
    /// What the chronicler is told about this world: its real places.
    pub(crate) fn lore_request(&self) -> LoreRequest {
        let d = db();
        let at = self.start;
        let mut vs: Vec<&VillageInfo> = self.villages.iter().collect();
        vs.sort_by_key(|v| v.center.dist_sq(at));
        let home = vs.first().map(|v| v.name.clone()).unwrap_or_default();
        let mut names: Vec<(String, String)> = Vec::new();
        // a place name with its English name for the English texts
        let mut named = |ru: &str| -> String {
            let en = crate::i18n::tr_in(crate::i18n::EN, ru);
            if en == ru {
                return ru.to_string();
            }
            if !names.iter().any(|n| n.0 == ru) {
                names.push((ru.to_string(), en.clone()));
            }
            format!("{ru} ({en})")
        };
        let mut req = LoreRequest {
            world: named(&self.world_name),
            start: named(&home),
            ..Default::default()
        };
        let place = |what: String, p: Pos| {
            format!(
                "{what}, {} steps to the {} of the start",
                p.dist(at),
                compass(at, p)
            )
        };
        let cities = vs.iter().filter(|v| v.city).take(5);
        let villages = vs.iter().filter(|v| !v.city && v.name != home).take(4);
        for v in cities.chain(villages) {
            let kind = if v.city { "the city" } else { "the village" };
            req.places
                .push(place(format!("{kind} {}", named(&v.name)), v.center));
        }
        let mut rs: Vec<&Region> = self.regions.iter().collect();
        rs.sort_by_key(|r| r.at.dist_sq(at));
        let deadly = rs.iter().skip(6).filter(|r| r.danger >= 3).take(3);
        for r in rs.iter().take(6).chain(deadly) {
            req.places.push(place(
                format!(
                    "the lands of {} ({}, danger {} of 3)",
                    named(&r.name),
                    r.kind,
                    r.danger
                ),
                r.at,
            ));
        }
        let mut es: Vec<&Entrance> = self.entrances.iter().collect();
        es.sort_by_key(|e| e.pos.dist_sq(at));
        let mut deep: Vec<&&Entrance> = es.iter().skip(5).collect();
        deep.sort_by_key(|e| std::cmp::Reverse(e.max_depth));
        for e in es.iter().take(5).chain(deep.into_iter().take(3)) {
            req.places.push(place(
                format!(
                    "the dungeon {} ({}, {} floors deep)",
                    named(&e.name),
                    e.theme,
                    e.max_depth
                ),
                e.pos,
            ));
        }
        let mut ls: Vec<&Landmark> = self.landmarks.iter().collect();
        ls.sort_by_key(|l| l.pos.dist_sq(at));
        for l in ls.into_iter().take(4) {
            req.places
                .push(place(format!("the {} {}", l.kind, named(&l.name)), l.pos));
        }
        for (land, _) in LANDS {
            if self.regions.iter().any(|r| r.kind == *land) {
                req.lands.push(land.to_string());
            }
        }
        if req.lands.is_empty() {
            req.lands = LANDS.iter().map(|l| l.0.to_string()).collect();
        }
        req.looks = LORE_LOOKS
            .iter()
            .map(|(k, n)| Option_ {
                key: k.to_string(),
                name: n.to_string(),
            })
            .collect();
        for m in &d.b.monsters {
            if !m.boss && !m.ally && m.depth == [0, 0] && m.weight > 0 && req.villains.len() < 30 {
                req.villains.push(Option_ {
                    key: m.key.clone(),
                    name: m.name.clone(),
                });
            }
            if m.boss {
                if let Some(e) = self.entrances.iter().find(|e| m.themes.contains(&e.theme)) {
                    req.bosses.push(Option_ {
                        key: m.key.clone(),
                        name: format!(
                            "{}, the lord of dungeons like {}",
                            named(&m.name),
                            named(&e.name)
                        ),
                    });
                }
            }
        }
        req.names = names;
        req
    }

    /// Lets the model write this world's story if one is at hand: Claude, or
    /// a local model that is already downloaded (a download is not waited
    /// for). It blocks until the story is written, the model gives up or
    /// `skip` is set; `stage` hears what is going on. Returns the title.
    pub fn write_lore(
        &mut self,
        skip: &AtomicBool,
        stage: &dyn Fn(&str),
    ) -> Result<String, String> {
        let Some(b) = self.brain.clone() else {
            return Err("нейросеть выключена".into());
        };
        let started = Instant::now();
        let mut told = false;
        loop {
            match b.local_state() {
                None | Some(local::State::Ready) => break,
                Some(local::State::Downloading { .. }) => {
                    return Err("модель ещё скачивается, мир создан без летописи".into())
                }
                Some(local::State::Failed(e)) => return Err(e),
                Some(_) => {
                    if !told {
                        told = true;
                        stage("Нейросеть загружается");
                    }
                    if skip.load(Ordering::Relaxed) {
                        return Err("летопись пропущена".into());
                    }
                    if started.elapsed() > LOAD_WAIT {
                        return Err("нейросеть не загрузилась вовремя".into());
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
        if !b.enabled() {
            return Err("нейросеть недоступна".into());
        }
        stage("Нейросеть пишет историю мира");
        let req = self.lore_request();
        let (tx, rx) = std::sync::mpsc::channel();
        b.lore(
            req.clone(),
            Box::new(move |r| {
                let _ = tx.send(r);
            }),
        );
        let started = Instant::now();
        let reply = loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(r) => break r?,
                Err(RecvTimeoutError::Timeout) => {
                    if skip.load(Ordering::Relaxed) {
                        return Err("летопись пропущена".into());
                    }
                    if started.elapsed() > Duration::from_secs(crate::llm::LORE_SECS + 10) {
                        return Err("нейросеть не успела написать историю".into());
                    }
                }
                Err(RecvTimeoutError::Disconnected) => return Err("нейросеть не ответила".into()),
            }
        };
        stage("Летопись оживает");
        let key = format!("{LORE_PREFIX}{}", self.seed.unsigned_abs());
        let lore = build_lore(&key, &req, &reply)?;
        crate::content::install_world(&lore.content)?;
        let title = lore.title.clone();
        self.lore = Some(lore);
        self.place_lore_characters();
        Ok(title)
    }

    /// Settles the characters of the story: the first one not far from where
    /// heroes begin, the others further out, each in their own land if it
    /// is near enough.
    fn place_lore_characters(&mut self) {
        let Some(lore) = &self.lore else { return };
        let chars = lore.content.uniques.clone();
        let mut r = Rng::labeled(self.seed, "lore");
        let (w, h) = {
            let l = &self.levels["overworld"];
            (l.w, l.h)
        };
        let k = gen::MapScale::of(w, h).lin.clamp(1.0, 3.0);
        let rings = [(30.0, 90.0), (60.0, 180.0), (100.0, 320.0)]
            .map(|(a, b)| ((a * k) as i32, (b * k) as i32));
        let mut placed: Vec<Pos> = Vec::new();
        for (i, u) in chars.iter().enumerate() {
            let (lo, hi) = rings[i.min(rings.len() - 1)];
            let spot = self
                .lore_spot(&mut r, &u.biomes, lo, hi, &placed)
                .or_else(|| self.lore_spot(&mut r, &[], lo, hi * 2, &placed))
                .or_else(|| self.unique_spot(&mut r, &[], &placed, 5));
            if let Some(p) = spot {
                placed.push(p);
                let gold = 60 + r.int_n(60);
                self.spawn_unique(u, p, gold);
            }
        }
        self.index_levels();
    }

    fn lore_spot(
        &self,
        r: &mut Rng,
        biomes: &[String],
        lo: i32,
        hi: i32,
        taken: &[Pos],
    ) -> Option<Pos> {
        let l = &self.levels["overworld"];
        for _ in 0..3000 {
            let a = r.f32() * std::f32::consts::TAU;
            let d = (lo + r.int_n((hi - lo).max(1))) as f32;
            let p = (self.start.center() + Vec2::from_angle(a) * d).cell();
            if !l.inside(p.x, p.y) || !l.walkable(p.x, p.y) {
                continue;
            }
            let def = l.def_at(p);
            if !def.interact.is_empty()
                || def.damage > 0.0
                || self.in_village(p, 8)
                || self.cell_taken("overworld", p)
                || (!biomes.is_empty() && !biomes.contains(&def.biome))
            {
                continue;
            }
            if taken.iter().all(|o| o.dist(p) >= 12) {
                return Some(p);
            }
        }
        None
    }

    /// Restores the world's own content from its save.
    pub(crate) fn install_lore(&self) -> Result<(), String> {
        match &self.lore {
            Some(l) => crate::content::install_world(&l.content).map(|_| ()),
            None => Ok(()),
        }
    }

    /// Facts of the story for NPCs and the game master.
    pub(crate) fn lore_facts(&self) -> Vec<String> {
        let Some(l) = &self.lore else {
            return Vec::new();
        };
        let mut out = vec![format!(
            "The history of this world (\"{}\"): {}",
            l.title,
            l.history.join(" ")
        )];
        for u in l.characters() {
            out.push(format!(
                "{}, {}, is a living character of this history.",
                u.name, u.title
            ));
        }
        out
    }

    /// Whether a hero sees the chronicle for the first time (it opens by
    /// itself then); a note about a story that could not be written goes to
    /// the first hero.
    pub fn lore_first_look(&mut self, id: Id) -> bool {
        if let Some(note) = self.lore_note.take() {
            self.log(id, "#909090", note);
        }
        if self.lore.is_none() {
            return false;
        }
        let Some(p) = self.ents.get_mut(&id).and_then(|e| e.player.as_mut()) else {
            return false;
        };
        if p.unlocks.iter().any(|u| u == "lore:seen") {
            return false;
        }
        p.unlocks.push("lore:seen".into());
        true
    }

    /// Remembers why a world has no story (told to the first hero).
    pub fn note_lore(&mut self, why: &str) {
        self.lore_note = Some(format!("Летопись мира не написана: {why}."));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::llm::{LoreArtifact, LoreCharacter};

    fn t2(ru: &str, en: &str) -> Text2 {
        Text2 {
            ru: ru.into(),
            en: en.into(),
        }
    }

    /// A story as a model might write it, a bit careless in places.
    pub(crate) fn sample(req: &LoreRequest) -> LoreReply {
        let villain = req.villains[0].key.clone();
        let boss = req.bosses[0].key.clone();
        let ch = |name: &str, quest: &str, target: &str, foe: Text2, form: &str| LoreCharacter {
            name: t2(name, "Name"),
            title: t2("Последний страж", "The Last Warden"),
            persona: "A grim old warden who speaks in short sentences.".into(),
            greeting: t2("Здравствуй.", "Hello, {player}!"),
            about: t2("Я помню войну.", "I remember the war."),
            land: "nowhere".into(),
            look: "dragon".into(),
            quest: quest.into(),
            target: target.into(),
            foe,
            offer: t2(
                "Помоги мне, {player}: 100% важно.",
                "Help me, {player}: it matters.",
            ),
            done: t2("Спасибо.", "Thank you."),
            artifact: LoreArtifact {
                name: t2(&format!("Клинок {name}"), &format!("Blade of {name}")),
                desc: t2("Его выковали в {огне}.", "Forged in fire."),
                form: form.into(),
                power: "fire".into(),
            },
        };
        LoreReply {
            title: t2(
                "Летопись Пепельной Короны",
                "The Chronicle of the Ash Crown",
            ),
            history: vec![
                t2(
                    "В начале был лишь пепел.",
                    "In the beginning there was only ash.",
                ),
                t2("Потом пришли короли.", "Then the kings came."),
                t2(
                    "Сейчас мир снова в опасности.",
                    "Now the world is in danger again.",
                ),
            ],
            characters: vec![
                ch(
                    "Ведана",
                    "slay",
                    &villain,
                    t2("Мор Пепельный", "Mor the Ashen"),
                    "sword",
                ),
                ch("Горислав", "boss", &boss, Text2::default(), "ring"),
                ch(
                    "Любава",
                    "relics",
                    "",
                    t2("Осколок Короны", "Shard of the Crown"),
                    "nonsense",
                ),
                ch("Лишний", "slay", &villain, t2("Кто-то", "Someone"), "axe"),
            ],
        }
    }

    #[test]
    fn story_becomes_world_content() {
        let g = crate::game::tests::setup();
        let req = g.lore_request();
        assert!(!req.places.is_empty() && !req.villains.is_empty() && !req.bosses.is_empty());
        assert!(req.places.iter().any(|p| p.contains("the city")));
        let lore = build_lore("lore7", &req, &sample(&req)).unwrap();
        assert_eq!(lore.title, "Летопись Пепельной Короны");
        assert_eq!(lore.history.len(), 3);
        let u = &lore.content.uniques;
        assert_eq!(u.len(), 3, "three characters at most");
        assert_eq!(u[0].quest, "slay");
        assert_eq!(u[0].champion, "Мор Пепельный");
        assert_eq!(u[1].quest, "boss");
        assert_eq!(u[2].quest, "relics");
        assert_eq!(u[2].count, 5);
        assert_eq!(u[2].target, "lore7_r2");
        // unknown lands and looks are replaced with real ones
        assert!(req.lands.contains(&u[0].biomes[0]));
        assert!(req.looks.iter().any(|l| l.key == u[0].model));
        // every reward is an artifact of the story, built on a real item
        let items = &lore.content.items;
        for c in u {
            let key = c.reward.strip_prefix("item:").unwrap();
            let it = items.iter().find(|i| i.key == key).unwrap();
            assert!(it.unique && it.rarity == "legendary");
            assert_eq!(it.stats.get("fire_pct"), Some(&20.0));
        }
        let blade = items.iter().find(|i| i.key == "lore7_a0").unwrap();
        let base = db()
            .b
            .items
            .iter()
            .filter(|i| i.weapon == "sword" && !i.unique)
            .max_by_key(|i| i.value)
            .unwrap();
        assert!(blade.damage[1] > base.damage[1]);
        assert!(!blade.desc.contains('{'), "no format characters");
        // an unknown form falls back to a sword
        assert_eq!(
            items.iter().find(|i| i.key == "lore7_a2").unwrap().weapon,
            "sword"
        );
        // {player} only where the Russian text has it, no % signs
        assert!(!u[0].offer.contains('%') && u[0].offer.contains("{player}"));
        let en = &lore.content.translations["en"];
        assert_eq!(en["Ведана"], "Name");
        assert_eq!(en["Здравствуй."], "Hello!");
        assert_eq!(
            en["Летопись Пепельной Короны"],
            "The Chronicle of the Ash Crown"
        );
        // Russian place names left in English texts get their English names
        let world = g.world_name.clone();
        let world_en = crate::i18n::tr_in("en", &world);
        assert!(req.world.contains(&format!("({world_en})")));
        let mut r2 = sample(&req);
        r2.history[0] = t2(
            &format!("Мир {world} был молод."),
            &format!("The world of {world} was young."),
        );
        let l2 = build_lore("lore8", &req, &r2).unwrap();
        let line = &l2.content.translations["en"][&format!("Мир {world} был молод.")];
        assert_eq!(line, &format!("The world of {world_en} was young."));
        // the content checks out against the game's own
        let mut b = db().b.clone();
        b.merge(&lore.content);
        crate::content::index(b).unwrap();
    }

    #[test]
    fn russian_left_in_english() {
        let line = translate_runs("That night Пещера Эха fell, and Ведана rose.");
        assert_eq!(line, "That night Cave of Echoes fell, and Vedana rose.");
        let tx = Texts::new(&[("Долина Туманов".into(), "Vale of Mists".into())]);
        assert_eq!(
            tx.english("The Долина Туманов (Vale of Mists) slept.".into()),
            "The Vale of Mists slept."
        );
        assert_eq!(
            tx.russian("Долина Туманов (Vale of Mists) спала.".into()),
            "Долина Туманов спала."
        );
    }

    #[test]
    fn clean_texts() {
        assert_eq!(clean("  a\n b  {x} 5% ", 50, false), "a b x 5");
        assert_eq!(clean("Привет, {player}!", 50, true), "Привет, {player}!");
        assert_eq!(clean("Привет, {player}!", 50, false), "Привет!");
        assert_eq!(clean("{player}, привет", 50, false), "привет");
        let long = clean(&"слово ".repeat(40), 30, false);
        assert!(long.ends_with('…') && long.chars().count() <= 31);
    }
}
