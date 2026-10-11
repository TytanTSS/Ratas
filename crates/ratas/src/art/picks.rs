//! Which pictogram and badge each ability, skill, class and subclass gets.
//! No two skills share a picture, not even across classes, and no two
//! classes or subclasses share a shield: the pictures are handed out over
//! the whole content at once, the ones picked by hand first, then each takes
//! the best free one of its options, guessed from its key, name, the look of
//! its effects (fx), its element and its bonuses. Mods and the monster
//! abilities a mimic copies get pictures the same way. A subclass's shield
//! wears the sign of its class in the corner.

use std::collections::{HashMap, HashSet};

use ratas_core::content::{AbilityDef, ClassDef, Db, SkillDef, SubclassDef};

use super::runes::{self, Look};

mod table;
use table::{ABILITIES, CLASSES, SKILLS, SUBCLASSES};

/// Words of keys and names that name a pictogram, the more telling first.
/// Russian words match by their beginning, English ones as whole words of
/// the key or the fx; "a+b" needs both.
const PICTURE_WORDS: &[(&str, &str)] = &[
    ("chain+lightning цепн+молн", "forked"),
    ("ice+lance лед+копь ледян+копь", "icicle"),
    ("arrow+rain arrows+rain hail ливен дожд+стрел град", "arrow_rain"),
    ("skeleton skeletons скелет", "skeleton"),
    ("treant древн", "treant"),
    ("golem colossus голем колосс", "golem"),
    ("wolf wolves hounds волк волч свор", "wolf"),
    ("bear медвед медвеж", "bear"),
    ("hawk hawks falcon сокол птиц орл", "hawk"),
    ("dragon дракон", "dragon"),
    ("imp imps felguard demon бес демон", "demon"),
    ("ghost ghosts wraith spectral призрак призрач", "ghost"),
    ("soul souls душ", "soul"),
    ("abomination чудовищ", "beast"),
    ("mimicry mimic copy copied echo подраж копи эхо", "mask"),
    ("mirror images decoy зеркал обманк двойник", "mirror"),
    ("legion army rally легион арми сплоч", "crowd"),
    ("wall стен", "shields"),
    ("shield shields barrier щит", "shield"),
    ("dagger knife backstab кинжал нож", "dagger"),
    ("sword blade blades клинок клинк меч", "sword"),
    ("axe секир топор", "axe"),
    ("hammer молот", "hammer"),
    ("spear lance копь копье", "spear"),
    ("volley multishot залп", "volley"),
    ("bow archery лук стрельб", "bow"),
    ("arrow arrows shot стрел выстрел", "arrow"),
    ("bomb бомб", "bomb"),
    ("flask flasks склянк", "flask"),
    (
        "potion elixir draught mutagen зель эликсир глоток мутаген смес",
        "flask",
    ),
    ("grail chalice грал чаш", "chalice"),
    ("horn рог", "horn"),
    ("banner знам", "banner"),
    ("crown king kings корол корон", "crown"),
    ("trap snare капкан", "trap"),
    ("web паутин", "web"),
    ("root roots entangling корн", "roots"),
    ("seal seals печат", "seal"),
    ("rune runes рун", "rune"),
    ("scythe harvest reaper жатв жнец", "scythe"),
    ("chain chains цеп", "chain"),
    (
        "gold coin coins pickpocket scavenger золот монет мародер карман трофе",
        "coin",
    ),
    ("diamond gem алмаз", "gem"),
    ("rewind отмот", "rewind"),
    ("hourglass sand", "hourglass"),
    ("haste time clock slow stop ускор врем", "clock"),
    ("tower fortress bastion крепост башн", "tower"),
    ("gate portal врат", "gate"),
    ("rift void abyss бездн разлом пустот", "spiral"),
    ("totem тотем", "totem"),
    ("pyre костер", "pyre"),
    ("drunken пьян", "mug"),
    ("lotus meditation transcendence медитац просветл покой", "lotus"),
    ("balance равновес", "yinyang"),
    ("lullaby sleep колыбел", "zzz"),
    (
        "song songs hymn ballad verse chant anthem dirge requiem saga tale песн гимн баллад куплет напев сказан реквием саг певец",
        "note",
    ),
    ("shout cry roar wail voice клич рев вопль голос", "sound"),
    ("book lore wisdom знан наук мудр книг", "book"),
    (
        "judgement verdict sentence ruling scales приговор вердикт суд",
        "scales",
    ),
    ("tomb funeral crypt grave погреб склеп могил", "tomb"),
    ("beacon маяк", "beacon"),
    ("prophecy foresight пророч предвид", "crystal"),
    (
        "fate constellation starfall предначерт созвезд звездопад",
        "stars",
    ),
    ("supernova сверхнов", "burst"),
    ("eclipse затмен", "eclipse"),
    ("moon lunar moonbeam лун", "moon"),
    ("star звезд", "star"),
    ("dawn рассвет", "dawn"),
    ("sun solar солн", "sun"),
    ("halo нимб", "halo"),
    ("miracle чудо", "wings"),
    ("resurrection reborn воскрес возрожд", "ankh"),
    ("crusader cross крестонос крест", "cross"),
    ("pillar pillars столп", "pillar"),
    ("meteor метеор", "meteor"),
    ("fireball", "fireball"),
    ("eruption lava magma извержен лав магм", "volcano"),
    ("inferno преиспод", "flame"),
    ("explosion blast burst взрыв", "burst"),
    ("heart сердц", "heart"),
    ("grasp хватк", "hand"),
    ("leech leeches drain vampire пиявк похищ вампир", "fang"),
    ("claw maul savage rend растерз когт кровопуск", "claw"),
    ("palm palms ладон", "palm"),
    ("fist fists кулак", "fist"),
    ("eye eyes sight глаз зорк зрен", "eye"),
    ("mark метк", "target"),
    ("headshot aimed aim прицел голов", "target"),
    ("skull death doom смерт мертв рок", "skull"),
    ("bone bones кост", "bones"),
    ("hex curse порч проклят", "eye"),
    ("blood кров", "drop"),
    ("mana мана маны ману", "mana"),
    ("thorn thorns шип терн", "thorns"),
    ("tree forest grove wild лес рощ древо", "tree"),
    ("herb herbalism leaf трав лист", "leaf"),
    ("tornado cyclone смерч", "tornado"),
    ("whirl whirlwind rampage вихр", "whirl"),
    ("wind ветер ветр порыв", "wind"),
    ("rain дожд", "rain"),
    ("wave water волн вод", "wave"),
    ("thunder thunderstorm storm гроз гром бур", "storm"),
    ("lightning static молни статич", "bolt"),
    (
        "frost ice cold blizzard winter лед ледян холод мороз стуж зим север мерзл метел",
        "snowflake",
    ),
    ("fire flame flames огн огон пламен", "flame"),
    (
        "plague pestilence contagion miasma чум мор зараз миазм",
        "poison",
    ),
    ("poison venom toxic toxin яд ядов токсик отрав", "poison"),
    ("cloud mist облак туман", "cloud"),
    ("smoke дым", "smoke"),
    ("vanish fade исчезн", "fade"),
    (
        "earth ground slam stomp quake земл оземь топот сотряс",
        "quake",
    ),
    ("stone rock boulder камен валун", "rock"),
    ("armor plate доспех брон латы", "armor"),
    ("helm шлем", "helm"),
    ("ward оберег", "bubble"),
    ("orb sphere сфер", "orb"),
    ("missile", "missile"),
    ("arcane magic тайн чародей маги", "spark"),
    (
        "heal mend renew rejuvenation исцел лечен целеб целит обновл омолож",
        "plus",
    ),
    (
        "feather dodge evasion уклон ускольз неуловим бесплот",
        "feather",
    ),
    ("dash charge рывок натиск бросок", "rush"),
    ("blink step steps шаг скачок", "steps"),
    (
        "boots road roads legs ног дорог путь странник паломник первопроход",
        "boot",
    ),
    ("strength mutation сил мощ мутац", "muscle"),
    (
        "rage fury frenzy berserk enrage ярост бешен берсерк неистов",
        "rage",
    ),
    ("taunt challenge провокац вызов", "sound"),
    ("strike удар", "slash"),
    ("cleave рассек", "whirl"),
];

/// Badges tried when a skill's own ones are taken.
const SPARE_BADGES: &[&str] = &[
    "up", "impact", "star", "arcane", "crit", "sun", "shadow", "spirit", "time", "void",
];

/// The words of a key (and fx) and of a name.
struct Words {
    key: Vec<String>,
    name: Vec<String>,
}

impl Words {
    fn new(key: &str, fx: &str, name: &str) -> Words {
        let split = |s: &str| -> Vec<String> {
            s.to_lowercase()
                .chars()
                .map(|c| if c == 'ё' { 'е' } else { c })
                .collect::<String>()
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| !w.is_empty())
                .map(String::from)
                .collect()
        };
        let mut key = split(key);
        key.extend(split(fx));
        Words {
            key,
            name: split(name),
        }
    }

    fn has(&self, alt: &str) -> bool {
        alt.split('+').all(|part| {
            if part.chars().next().is_some_and(|c| c >= '\u{400}') {
                self.name.iter().any(|w| w.starts_with(part))
            } else {
                self.key.iter().any(|w| w == part)
            }
        })
    }

    /// Every pictogram the words name, the more telling first.
    fn glyphs(&self) -> Vec<&'static str> {
        PICTURE_WORDS
            .iter()
            .filter(|(alts, _)| alts.split_whitespace().any(|a| self.has(a)))
            .map(|&(_, g)| g)
            .collect()
    }
}

/// The badge of an element of the effects' look or of a damage type.
fn element(a: &AbilityDef) -> &'static str {
    for w in a.fx.split_whitespace() {
        if w == "storm" {
            return "lightning";
        }
        if let Some(b) = runes::badge(w) {
            return b.name;
        }
    }
    match a.dmg_type.as_str() {
        "fire" => "fire",
        "cold" => "frost",
        "lightning" => "lightning",
        "poison" => "poison",
        "shadow" => "shadow",
        "holy" => "holy",
        "arcane" => "arcane",
        _ => "",
    }
}

/// The pictogram of a signature shape of the effects.
fn shape(a: &AbilityDef) -> Option<&'static str> {
    a.fx.split_whitespace().find_map(|w| {
        Some(match w {
            "meteor" => "meteor",
            "blizzard" => "snowflake",
            "rain" if a.dmg_type == "pierce" => "arrow_rain",
            "rain" => "rain",
            "thunder" => "storm",
            "whirlwind" => "whirl",
            "inferno" => "flame",
            "eruption" => "volcano",
            "pillars" => "pillar",
            "eclipse" => "eclipse",
            "orbit" => "orbit",
            "supernova" => "burst",
            "shield" => "shield",
            "flask" => "flask",
            "web" => "web",
            "hammer" => "hammer",
            "lance" => "spear",
            "rune" => "rune",
            "mark" => "target",
            "sleep" => "zzz",
            _ => return None,
        })
    })
}

/// The pictogram of an element, shaped by the kind of ability.
fn element_glyph(elem: &str, kind: &str) -> Option<&'static str> {
    Some(match (elem, kind) {
        ("fire", "projectile") => "fireball",
        ("fire", "nova") => "burst",
        ("fire", _) => "flame",
        ("frost", "projectile") => "icicle",
        ("frost", _) => "snowflake",
        ("lightning", "chain") => "forked",
        ("lightning", "nova") => "storm",
        ("lightning", _) => "bolt",
        ("poison", "nova") => "cloud",
        ("poison", _) => "poison",
        ("shadow", "projectile") => "missile",
        ("shadow", _) => "eclipse",
        ("holy", "nova") => "pillar",
        ("holy", "heal") => "plus",
        ("holy", _) => "cross",
        ("arcane", "projectile") => "missile",
        ("arcane", "nova") => "nova",
        ("arcane", _) => "spark",
        ("nature", _) => "leaf",
        ("earth", _) => "rock",
        ("wind", _) => "wind",
        ("blood", _) => "drop",
        ("bone", _) => "bones",
        ("spirit", _) => "ghost",
        ("sound", _) => "sound",
        ("time", _) => "hourglass",
        ("void", _) => "spiral",
        ("star", _) => "star",
        ("sun", _) => "sun",
        ("gold", _) => "coin",
        ("smoke", _) => "smoke",
        ("beast", _) => "paw",
        ("illusion", _) => "mask",
        ("water", _) => "wave",
        ("chi", _) => "palm",
        ("demon", _) => "demon",
        ("alchemy", _) => "flask",
        ("steel", _) => "sword",
        ("arrow", _) => "arrow",
        _ => return None,
    })
}

/// The pictogram and the badge of a kind of ability.
fn kind_look(a: &AbilityDef) -> (&'static str, &'static str) {
    match a.kind.as_str() {
        "strike" => ("slash", "steel"),
        "cleave" => ("whirl", "steel"),
        "dash" => ("rush", "speed"),
        "projectile" if a.dmg_type == "pierce" => ("arrow", "arrow"),
        "projectile" => ("missile", "impact"),
        "nova" => ("nova", "impact"),
        "heal" => ("plus", "plus"),
        "buff" => ("bubble", "up"),
        "summon" => ("figure", "spirit"),
        "taunt" | "echo" => ("sound", "sound"),
        "decoy" => ("mirror", "illusion"),
        "revive" => ("ankh", "holy"),
        "chain" => ("forked", "impact"),
        "mimic" | "copied" => ("mask", "illusion"),
        "death_sentence" => ("skull", "shadow"),
        _ => ("spark", "arcane"),
    }
}

/// The pictograms and badges that fit an ability, best first.
fn ability_options(a: &AbilityDef) -> (Vec<&'static str>, Vec<&'static str>) {
    let elem = element(a);
    let (kind_glyph, kind_badge) = kind_look(a);
    let words = Words::new(&a.key, &a.fx, &a.name).glyphs();
    let mut glyphs = words.clone();
    glyphs.extend(shape(a));
    glyphs.extend(element_glyph(elem, &a.kind));
    glyphs.push(kind_glyph);
    // a second word of the name makes a better badge than none
    let mut badges = vec![elem];
    badges.extend(words.iter().skip(1).filter_map(|g| own_badge(g)));
    badges.push(kind_badge);
    (glyphs, badges)
}

/// The badge that says what a pictogram shows, if there is one.
fn own_badge(glyph: &str) -> Option<&'static str> {
    runes::glyph(glyph)
        .map(|g| g.elem)
        .filter(|e| runes::badge(e).is_some())
}

/// The pictogram and the badge a bonus stands for.
fn stat_look(stat: &str) -> (&'static str, &'static str) {
    match stat {
        "max_hp" => ("heart", "heart"),
        "hp_regen" => ("heart", "plus"),
        "heal_pct" => ("plus", "plus"),
        "max_mp" => ("mana", "mana"),
        "mp_regen" => ("mana", "time"),
        "melee_pct" => ("sword", "steel"),
        "ranged_pct" => ("bow", "arrow"),
        "spell_pct" => ("orb", "arcane"),
        "fire_pct" => ("flame", "fire"),
        "cold_pct" => ("snowflake", "frost"),
        "lightning_pct" => ("bolt", "lightning"),
        "poison_pct" | "add_poison" => ("poison", "poison"),
        "shadow_pct" => ("eclipse", "shadow"),
        "holy_pct" => ("cross", "holy"),
        "arcane_pct" => ("spark", "arcane"),
        "res_fire" => ("flame", "shield"),
        "res_cold" => ("snowflake", "shield"),
        "res_lightning" => ("bolt", "shield"),
        "res_poison" => ("poison", "shield"),
        "res_shadow" => ("eclipse", "shield"),
        "res_magic" => ("spark", "shield"),
        "res_elemental" => ("burst", "shield"),
        "res_physical" | "armor" => ("armor", "shield"),
        "res_all" | "block" => ("shield", "shield"),
        "crit" | "crit_mult" => ("target", "crit"),
        "dodge" | "dex" => ("feather", "dodge"),
        "move_speed" => ("boot", "speed"),
        "attack_speed" => ("swords", "speed"),
        "gold_find" => ("coin", "gold"),
        "sight" => ("eye", "eye"),
        "life_leech" => ("fang", "blood"),
        "thorns" => ("thorns", "thorns"),
        "fury" => ("rage", "impact"),
        "str" => ("muscle", "up"),
        "int" => ("book", "up"),
        "reach" => ("spear", "up"),
        "duel_pct" => ("swords", "steel"),
        "ambush_pct" => ("dagger", "shadow"),
        "mimic_pct" => ("mask", "illusion"),
        _ => ("spark", "up"),
    }
}

/// How much a bonus weighs against the others of a skill.
fn stat_unit(stat: &str) -> f64 {
    match stat {
        "max_hp" => 10.0,
        "hp_regen" => 0.5,
        "max_mp" => 8.0,
        "mp_regen" => 0.3,
        "armor" | "crit" | "dodge" | "str" | "dex" | "int" => 2.0,
        "block" | "thorns" | "res_all" => 3.0,
        "crit_mult" => 0.1,
        "move_speed" | "attack_speed" => 4.0,
        "gold_find" => 12.0,
        "sight" | "life_leech" | "reach" => 1.0,
        "fury" => 10.0,
        s if s.starts_with("res_") => 5.0,
        _ => 6.0,
    }
}

/// The pictograms and badges that fit a passive skill, best first: what its
/// name says, then its bonuses by weight.
fn skill_options(s: &SkillDef) -> (Vec<&'static str>, Vec<&'static str>) {
    let mut stats: Vec<(&str, f64)> = s
        .stats
        .iter()
        .map(|(k, v)| (k.as_str(), v.abs() / stat_unit(k)))
        .collect();
    stats.sort_by(|a, b| b.1.total_cmp(&a.1));
    let words = Words::new(&s.key, "", &s.name).glyphs();
    let mut glyphs = words.clone();
    glyphs.extend(stats.iter().map(|(k, _)| stat_look(k).0));
    let mut badges: Vec<&'static str> = stats.iter().map(|(k, _)| stat_look(k).1).collect();
    badges.extend(words.iter().skip(1).filter_map(|g| own_badge(g)));
    badges.push("up");
    (glyphs, badges)
}

/// The first free look: the best pictogram with each of the badges in turn
/// (a badge repeating what the pictogram shows becomes no badge), then the
/// next pictograms, then any picture at all.
fn choose(glyphs: &[&'static str], badges: &[&'static str], taken: &HashSet<Look>) -> Look {
    let tries = glyphs.iter().flat_map(|&glyph| {
        let own = runes::glyph(glyph).map_or("", |g| g.elem);
        badges.iter().chain(SPARE_BADGES).map(move |&b| Look {
            glyph,
            badge: if b == own { "" } else { b },
        })
    });
    let any = runes::GLYPHS.iter().flat_map(|g| {
        std::iter::once("")
            .chain(runes::BADGES.iter().map(|b| b.name))
            .map(move |badge| Look {
                glyph: g.name,
                badge,
            })
    });
    tries
        .chain(any)
        .find(|l| !taken.contains(l))
        .expect("more pictures than skills")
}

/// The pictures of a content set's abilities and passive skills.
pub struct Book {
    abilities: HashMap<String, Look>,
    skills: HashMap<String, Look>,
    classes: HashMap<String, Look>,
    /// the badge of each class's subclasses
    signs: HashMap<String, &'static str>,
    subclasses: HashMap<String, Look>,
}

/// The pictograms that fit a class or a subclass, best first.
fn crest_glyphs(key: &str, name: &str, last: &'static str) -> Vec<&'static str> {
    let mut glyphs = Words::new(key, "", name).glyphs();
    glyphs.push(last);
    glyphs
}

impl Book {
    pub fn new(db: &Db) -> Book {
        let mut book = Book {
            abilities: HashMap::new(),
            skills: HashMap::new(),
            classes: HashMap::new(),
            signs: HashMap::new(),
            subclasses: HashMap::new(),
        };
        let mut taken = HashSet::new();
        let found = |table: &[(&str, &'static str, &'static str)], key: &str| {
            table
                .iter()
                .find(|(k, _, _)| *k == key)
                .map(|&(_, glyph, badge)| Look { glyph, badge })
        };
        // the hand-picked ones first, then the rest in content order
        for a in &db.b.abilities {
            if let Some(l) = found(ABILITIES, &a.key) {
                taken.insert(l);
                book.abilities.insert(a.key.clone(), l);
            }
        }
        for s in &db.b.skills {
            if let Some(l) = found(SKILLS, &s.key) {
                taken.insert(l);
                book.skills.insert(s.key.clone(), l);
            }
        }
        for a in &db.b.abilities {
            if !book.abilities.contains_key(&a.key) {
                let (glyphs, badges) = ability_options(a);
                let l = choose(&glyphs, &badges, &taken);
                taken.insert(l);
                book.abilities.insert(a.key.clone(), l);
            }
        }
        for s in &db.b.skills {
            if book.skills.contains_key(&s.key) || db.ability(&s.grants).is_some() {
                continue;
            }
            let (glyphs, badges) = skill_options(s);
            let l = choose(&glyphs, &badges, &taken);
            taken.insert(l);
            book.skills.insert(s.key.clone(), l);
        }
        // the shields of classes and subclasses, apart from the skills
        let mut shields = HashSet::new();
        for c in &db.b.classes {
            if let Some(&(_, glyph, sign)) = CLASSES.iter().find(|t| t.0 == c.key) {
                let l = Look { glyph, badge: "" };
                shields.insert(l);
                book.classes.insert(c.key.clone(), l);
                book.signs.insert(c.key.clone(), sign);
            }
        }
        for sc in &db.b.subclasses {
            if let Some(&(_, glyph)) = SUBCLASSES.iter().find(|t| t.0 == sc.key) {
                let l = Look {
                    glyph,
                    badge: book.signs.get(&sc.class).copied().unwrap_or(""),
                };
                shields.insert(l);
                book.subclasses.insert(sc.key.clone(), l);
            }
        }
        for c in &db.b.classes {
            if !book.classes.contains_key(&c.key) {
                let l = choose(&crest_glyphs(&c.key, &c.name, "figure"), &[""], &shields);
                shields.insert(l);
                book.classes.insert(c.key.clone(), l);
                let sign = own_badge(l.glyph).unwrap_or("crown");
                book.signs.insert(c.key.clone(), sign);
            }
        }
        for sc in &db.b.subclasses {
            if !book.subclasses.contains_key(&sc.key) {
                let sign = book.signs.get(&sc.class).copied().unwrap_or("crown");
                let l = choose(&crest_glyphs(&sc.key, &sc.name, "spark"), &[sign], &shields);
                shields.insert(l);
                book.subclasses.insert(sc.key.clone(), l);
            }
        }
        book
    }

    /// The shield of a class.
    pub fn class(&self, c: &ClassDef) -> Look {
        self.classes.get(&c.key).copied().unwrap_or(Look {
            glyph: "figure",
            badge: "",
        })
    }

    /// The shield of a subclass: its pictogram, its class's sign.
    pub fn subclass(&self, sc: &SubclassDef) -> Look {
        self.subclasses.get(&sc.key).copied().unwrap_or(Look {
            glyph: "spark",
            badge: self.signs.get(&sc.class).copied().unwrap_or(""),
        })
    }

    /// The picture of an ability (a fusion is painted from its parts).
    pub fn ability(&self, a: &AbilityDef) -> Look {
        self.abilities.get(&a.key).copied().unwrap_or_else(|| {
            let (glyphs, badges) = ability_options(a);
            choose(&glyphs, &badges, &HashSet::new())
        })
    }

    /// The picture of a passive skill (one granting an ability shows the
    /// ability instead).
    pub fn skill(&self, s: &SkillDef) -> Look {
        self.skills.get(&s.key).copied().unwrap_or_else(|| {
            let (glyphs, badges) = skill_options(s);
            choose(&glyphs, &badges, &HashSet::new())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratas_core::content;

    /// Everything the hand-picked tables name exists.
    #[test]
    fn picks_name_real_pictures() {
        let db = content::db();
        for (table, abilities) in [(ABILITIES, true), (SKILLS, false)] {
            for &(key, glyph, badge) in table {
                let known = if abilities {
                    db.ability(key).is_some()
                } else {
                    db.skill(key)
                        .is_some_and(|s| db.ability(&s.grants).is_none())
                };
                assert!(known, "{key} is not a hand-pickable ability or skill");
                assert!(runes::glyph(glyph).is_some(), "{key}: no pictogram {glyph}");
                assert!(
                    badge.is_empty() || runes::badge(badge).is_some(),
                    "{key}: no badge {badge}"
                );
            }
        }
        for w in PICTURE_WORDS {
            assert!(runes::glyph(w.1).is_some(), "no pictogram {}", w.1);
        }
        for &(key, glyph, sign) in CLASSES {
            assert!(db.class(key).is_some(), "no class {key}");
            assert!(runes::glyph(glyph).is_some(), "{key}: no pictogram {glyph}");
            assert!(runes::badge(sign).is_some(), "{key}: no badge {sign}");
        }
        for &(key, glyph) in SUBCLASSES {
            assert!(db.subclass(key).is_some(), "no subclass {key}");
            assert!(runes::glyph(glyph).is_some(), "{key}: no pictogram {glyph}");
        }
    }

    /// No two abilities or passive skills share a picture, and no two skills
    /// grant the same ability (they would show the same picture).
    #[test]
    fn every_picture_is_unique() {
        let db = content::db();
        let book = Book::new(db);
        let mut seen: HashMap<Look, String> = HashMap::new();
        let mut clashes = vec![];
        let mut add = |l: Look, key: &str| {
            if let Some(other) = seen.insert(l, key.to_string()) {
                clashes.push(format!(
                    "{key} and {other} are both {} {}",
                    l.glyph, l.badge
                ));
            }
        };
        for a in &db.b.abilities {
            add(book.ability(a), &a.key);
        }
        let mut grants: HashMap<&str, &str> = HashMap::new();
        let mut twice = vec![];
        for s in &db.b.skills {
            if db.ability(&s.grants).is_some() {
                if let Some(other) = grants.insert(&s.grants, &s.key) {
                    twice.push(format!("{} and {other} both grant {}", s.key, s.grants));
                }
            } else {
                add(book.skill(s), &s.key);
            }
        }
        clashes.extend(twice);
        assert!(clashes.is_empty(), "{}", clashes.join("\n"));
    }

    /// Every class and subclass has a shield of its own, every built-in one
    /// picked by hand, and a subclass wears its class's sign.
    #[test]
    fn every_shield_is_unique() {
        let db = content::db();
        let book = Book::new(db);
        let mut seen: HashMap<Look, String> = HashMap::new();
        for c in &db.b.classes {
            assert!(
                CLASSES.iter().any(|t| t.0 == c.key),
                "class {} is not picked",
                c.key
            );
            let l = book.class(c);
            if let Some(other) = seen.insert(l, c.key.clone()) {
                panic!("{} and {other} are both {} {}", c.key, l.glyph, l.badge);
            }
        }
        for sc in &db.b.subclasses {
            assert!(
                SUBCLASSES.iter().any(|t| t.0 == sc.key),
                "subclass {} is not picked",
                sc.key
            );
            let l = book.subclass(sc);
            assert_eq!(l.badge, book.signs[&sc.class], "{}", sc.key);
            if let Some(other) = seen.insert(l, sc.key.clone()) {
                panic!("{} and {other} are both {} {}", sc.key, l.glyph, l.badge);
            }
        }
        let signs: HashSet<&str> = book.signs.values().copied().collect();
        assert_eq!(signs.len(), db.b.classes.len(), "two classes share a sign");
    }

    /// Lists every picture and how it was chosen:
    /// `cargo test -p ratas list_pictures -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn list_pictures() {
        let db = content::db();
        let book = Book::new(db);
        for a in &db.b.abilities {
            let l = book.ability(a);
            let mark = if ABILITIES.iter().any(|t| t.0 == a.key) {
                " "
            } else {
                "?"
            };
            println!(
                "A{mark} {:<24} {:<28} {} {}",
                a.key, a.name, l.glyph, l.badge
            );
        }
        for s in &db.b.skills {
            if db.ability(&s.grants).is_none() {
                let l = book.skill(s);
                let mark = if SKILLS.iter().any(|t| t.0 == s.key) {
                    " "
                } else {
                    "?"
                };
                println!(
                    "S{mark} {:<24} {:<28} {} {}   {:?}",
                    s.key, s.name, l.glyph, l.badge, s.stats
                );
            }
        }
    }

    /// Paints the pictures of one class's skills (RATAS_SHEET_CLASS, "common"
    /// for the classless branches) to RATAS_SHEET, in the order of the window.
    #[test]
    #[ignore]
    fn class_sheet() {
        let (Ok(path), Ok(class)) = (
            std::env::var("RATAS_SHEET"),
            std::env::var("RATAS_SHEET_CLASS"),
        ) else {
            return;
        };
        let db = content::db();
        let book = Book::new(db);
        let mut items = vec![];
        for s in &db.b.skills {
            let Some(b) = db.branch(&s.branch) else {
                continue;
            };
            let mine = b.class == class || (class == "common" && b.class.is_empty());
            if !mine {
                continue;
            }
            items.push(match db.ability(&s.grants) {
                Some(a) => runes::paint(
                    book.ability(a),
                    super::super::hex(&a.color),
                    runes::Frame::Rune,
                ),
                None => runes::paint(
                    book.skill(s),
                    super::super::hex(&b.color),
                    runes::Frame::Plaque,
                ),
            });
        }
        runes::write_sheet(&path, &items, 10, 4);
    }

    /// Paints every class's shield and its subclasses' to RATAS_SHEET, a
    /// class to a row.
    #[test]
    #[ignore]
    fn shield_sheet() {
        let Ok(path) = std::env::var("RATAS_SHEET") else {
            return;
        };
        let db = content::db();
        let book = Book::new(db);
        let mut items = vec![];
        for c in &db.b.classes {
            let mut row = vec![runes::paint(
                book.class(c),
                super::super::hex(&c.color),
                runes::Frame::Class,
            )];
            for sc in db.subclasses_of(&c.key) {
                row.push(runes::paint(
                    book.subclass(sc),
                    super::super::hex(&sc.color),
                    runes::Frame::Subclass,
                ));
            }
            row.resize_with(7, || super::super::Pc::new(runes::SIDE, runes::SIDE, ""));
            items.extend(row);
        }
        runes::write_sheet(&path, &items, 7, 4);
    }
}
