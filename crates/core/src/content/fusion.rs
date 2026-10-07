//! Fusion of abilities: two abilities of different classes melt into one.
//!
//! The fused ability does what both parts do with one press and shares their
//! strengths: elements (a resonance of the same element, a dual element or
//! an enchanted weapon blow), effects on hit, leech, finishing blows. It pays
//! for that: more mana than both parts cast apart, a longer cooldown, weaker
//! parts (unless their elements resonate), shorter self-blessings, the gear
//! of both parts, a clash of opposed elements, and a flaw of its own —
//! backlash, instability, exhaustion, vulnerability or numbness.
//!
//! A fusion is not stored as content: its key `fuse:<first>+<second>` names
//! the parts, and `fuse` makes the same ability from them on the server and
//! on every client (Db::ability resolves such keys), so the fusion window
//! shows exactly what the hero will get.

use super::{gear_need_name, AbilityDef, BuffDef, Db};
use crate::rng::fnv64;

/// Fused keys are "fuse:<first>+<second>".
pub const FUSION_PREFIX: &str = "fuse:";
/// The kind of a fused ability: its parts are cast together.
pub const FUSION_KIND: &str = "fusion";

/// Parts whose elements resonate grow stronger, all others weaker.
const RESONANCE: f64 = 1.1;
const DISPERSION: f64 = 0.85;
/// A fusion costs this much more mana than its parts cast one by one, and at
/// least MANA_MIN more.
const MANA_MULT: f64 = 1.3;
const MANA_MIN: f64 = 4.0;
/// Self-blessings of the parts last this share of their time.
const BUFF_TIME: f64 = 0.75;
/// A healing part makes the striking one heal for this percent of the damage.
const HEAL_LEECH: f64 = 10.0;
/// Opposed elements: more chance to fizzle and a backlash.
const CLASH_FIZZLE: f64 = 15.0;
const CLASH_BACKLASH: f64 = 4.0;

/// Elements that fight each other in one fusion.
const OPPOSED: &[(&str, &str)] = &[("fire", "cold"), ("holy", "shadow")];

/// Gear requirements one hero cannot meet at the same time.
const GEAR_CLASH: &[(&str, &str)] = &[
    ("bow", "melee"),
    ("bow", "shield"),
    ("bow", "twohand_dual"),
    ("shield", "twohand_dual"),
];

/// A fusion: the ability and, in words, what it gives and what it costs.
#[derive(Clone, Debug, Default)]
pub struct Fusion {
    pub def: AbilityDef,
    pub pros: Vec<String>,
    pub cons: Vec<String>,
}

pub fn fusion_key(a: &str, b: &str) -> String {
    format!("{FUSION_PREFIX}{a}+{b}")
}

/// The keys of the two parts of a fused key.
pub fn fusion_parts(key: &str) -> Option<(&str, &str)> {
    key.strip_prefix(FUSION_PREFIX)?.split_once('+')
}

pub fn is_fusion(key: &str) -> bool {
    key.starts_with(FUSION_PREFIX)
}

/// Whether abilities of a kind can be fused: copies of other creatures'
/// abilities last only a while, and a fusion does not fuse again.
pub fn fusable_kind(kind: &str) -> bool {
    !matches!(kind, "mimic" | "copied" | "echo" | FUSION_KIND)
}

/// Whether one hero cannot hold the gear of both requirements at once.
pub fn gear_clash(x: &str, y: &str) -> bool {
    GEAR_CLASH
        .iter()
        .any(|&(a, b)| (x == a && y == b) || (x == b && y == a))
}

/// Everything an ability needs in the hands: a fusion needs what its parts
/// need.
pub fn equip_needs(a: &AbilityDef) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    for e in std::iter::once(&a.equip).chain(a.parts.iter().map(|p| &p.equip)) {
        if !e.is_empty() && !out.contains(&e.as_str()) {
            out.push(e);
        }
    }
    out
}

/// Makes the fusion a fused key names from the content of d.
pub fn fuse_key(d: &Db, key: &str) -> Option<Fusion> {
    let (a, b) = fusion_parts(key)?;
    let (a, b) = (d.ability(a)?, d.ability(b)?);
    if a.key == b.key || !fusable_kind(&a.kind) || !fusable_kind(&b.kind) {
        return None;
    }
    Some(fuse(d, a, b))
}

/// Whether an ability strikes creatures (and so can carry effects on hit).
fn hits(a: &AbilityDef) -> bool {
    match a.kind.as_str() {
        "projectile" | "nova" | "cleave" | "strike" | "chain" | "taunt" => true,
        "dash" => a.damage[1] > 0.0,
        _ => false,
    }
}

fn deals_damage(a: &AbilityDef) -> bool {
    a.damage[1] > 0.0 && a.kind != "heal"
}

/// Blows that take the damage type of the weapon in hand.
fn weapon_based(a: &AbilityDef) -> bool {
    a.dmg_type == "weapon"
        || (a.dmg_type.is_empty() && matches!(a.kind.as_str(), "strike" | "cleave" | "dash"))
}

/// The damage types an ability deals by itself (none for weapon blows).
fn elements(d: &Db, a: &AbilityDef) -> Vec<String> {
    if !deals_damage(a) || weapon_based(a) {
        return vec![];
    }
    if !a.split.is_empty() {
        return a.split.clone();
    }
    if a.dmg_type.is_empty() {
        return vec!["arcane".into()];
    }
    d.damage_type(&a.dmg_type)
        .map(|t| vec![t.key.clone()])
        .unwrap_or_default()
}

fn type_name(d: &Db, t: &str) -> String {
    d.damage_type(t)
        .map_or_else(|| t.to_string(), |x| x.name.to_lowercase())
}

fn opposed(x: &str, y: &str) -> bool {
    OPPOSED
        .iter()
        .any(|&(a, b)| (x == a && y == b) || (x == b && y == a))
}

/// Effects that hold a creature (stun, silence, taunt) keep their own short
/// time and do not melt into another effect.
fn control(b: &BuffDef) -> bool {
    b.stun || b.silence || b.taunt || b.stealth
}

/// One effect that does what both do.
fn merge_buffs(x: &BuffDef, y: &BuffDef) -> BuffDef {
    if x.key == y.key {
        let mut m = x.clone();
        m.duration_ms = x.duration_ms.max(y.duration_ms);
        m.dot_per_sec = x.dot_per_sec.max(y.dot_per_sec);
        return m;
    }
    let mut stats = x.stats.clone();
    for (k, v) in &y.stats {
        *stats.entry(k.clone()).or_insert(0.0) += v;
    }
    let dot = if x.dot_per_sec > 0.0 && y.dot_per_sec > 0.0 {
        x.dot_per_sec + y.dot_per_sec
    } else if x.dot_per_sec != 0.0 {
        x.dot_per_sec
    } else {
        y.dot_per_sec
    };
    // the damage over time burns in the type of the stronger one
    let dmg_type = if y.dot_per_sec.abs() > x.dot_per_sec.abs() {
        y.dmg_type.clone()
    } else {
        x.dmg_type.clone()
    };
    BuffDef {
        key: format!("{}+{}", x.key, y.key),
        name: format!("{} + {}", x.name, y.name),
        duration_ms: x.duration_ms.max(y.duration_ms),
        stats,
        dot_per_sec: dot,
        dmg_type,
        color: x.color.clone(),
        ..Default::default()
    }
}

/// Scales what a part deals or heals.
fn scale_power(a: &mut AbilityDef, k: f64) {
    let r = |v: f64| (v * k * 100.0).round() / 100.0;
    a.damage = [r(a.damage[0]), r(a.damage[1])];
    a.scale_k = r(a.scale_k);
}

/// The distance a part reaches its target from (see combat::ability_range).
fn reach(a: &AbilityDef) -> i32 {
    if a.range > 0 {
        a.range
    } else if a.kind == "projectile" {
        8
    } else {
        1
    }
}

/// "3", "0.5", "7.3".
fn num(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn secs(ms: i64) -> String {
    num(ms as f64 / 1000.0)
}

/// The flaws a fusion may carry besides the costs every fusion pays; which
/// one depends on the pair (the same pair always has the same flaw).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Flaw {
    Backlash,
    Unstable,
    Exhaustion,
    Vulnerable,
    Numbness,
}

const FLAWS: [Flaw; 5] = [
    Flaw::Backlash,
    Flaw::Unstable,
    Flaw::Exhaustion,
    Flaw::Vulnerable,
    Flaw::Numbness,
];

fn flaw_of(a: &str, b: &str) -> Flaw {
    let (x, y) = if a <= b { (a, b) } else { (b, a) };
    FLAWS[(fnv64(&format!("{x}+{y}")) % FLAWS.len() as u64) as usize]
}

/// A weakness laid on the caster after each cast.
fn drawback(key: &str, name: &str, ms: i64, stats: &[(&str, f64)]) -> BuffDef {
    BuffDef {
        key: key.into(),
        name: name.into(),
        duration_ms: ms,
        stats: stats.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        color: "#b08aff".into(),
        ..Default::default()
    }
}

/// Melts a (the first part) and b (the second) into one ability.
pub fn fuse(d: &Db, a: &AbilityDef, b: &AbilityDef) -> Fusion {
    let mut pros = Vec::new();
    let mut cons = Vec::new();
    let (mut pa, mut pb) = (a.clone(), b.clone());
    pros.push(format!(
        "Одно применение — срабатывают оба умения: «{}» и «{}».",
        a.name, b.name
    ));
    // a dash goes first: the other part strikes from where it ends
    let dash_first = b.kind == "dash" && a.kind != "dash";
    match (a.kind == "dash", b.kind == "dash") {
        (true, false) => pros.push(format!(
            "Сначала рывок «{}», затем «{}» — уже с нового места.",
            a.name, b.name
        )),
        (false, true) => pros.push(format!(
            "Сначала рывок «{}», затем «{}» — уже с нового места.",
            b.name, a.name
        )),
        _ => {}
    }

    // elements: resonance, a dual element, an enchanted weapon
    let (ea, eb) = (elements(d, a), elements(d, b));
    let resonance = ea.iter().find(|t| eb.contains(t)).cloned();
    match &resonance {
        Some(t) => {
            scale_power(&mut pa, RESONANCE);
            scale_power(&mut pb, RESONANCE);
            pros.push(format!(
                "Резонанс стихии «{}»: обе части сильнее на {}%.",
                type_name(d, t),
                num((RESONANCE - 1.0) * 100.0)
            ));
        }
        None => {
            scale_power(&mut pa, DISPERSION);
            scale_power(&mut pb, DISPERSION);
            cons.push(format!(
                "Рассеивание силы: урон и лечение каждой части слабее на {}%.",
                num((1.0 - DISPERSION) * 100.0)
            ));
            match (ea.as_slice(), eb.as_slice()) {
                ([x], [y]) => {
                    pa.split = vec![x.clone(), y.clone()];
                    pb.split = vec![y.clone(), x.clone()];
                    pros.push(format!(
                        "Двойная стихия: урон обеих частей делится между «{}» и «{}» — врагу труднее защититься.",
                        type_name(d, x),
                        type_name(d, y)
                    ));
                }
                ([], [y]) if deals_damage(a) => {
                    pa.dmg_type = y.clone();
                    pros.push(format!(
                        "Зачарование: «{}» бьёт стихией «{}».",
                        a.name,
                        type_name(d, y)
                    ));
                }
                ([x], []) if deals_damage(b) => {
                    pb.dmg_type = x.clone();
                    pros.push(format!(
                        "Зачарование: «{}» бьёт стихией «{}».",
                        b.name,
                        type_name(d, x)
                    ));
                }
                _ => {}
            }
        }
    }
    let clash = ea
        .iter()
        .flat_map(|x| eb.iter().map(move |y| (x, y)))
        .find(|(x, y)| opposed(x, y));

    // effects on hit
    match (&a.on_hit, &b.on_hit) {
        (Some(x), Some(y)) if !control(x) && !control(y) => {
            let m = merge_buffs(x, y);
            for p in [&mut pa, &mut pb] {
                if hits(p) {
                    p.on_hit = Some(m.clone());
                }
            }
            if x.key == y.key {
                pros.push(format!(
                    "Общий эффект «{}» берёт лучшее от обеих частей.",
                    x.name
                ));
            } else {
                pros.push(format!(
                    "Попадания обеих частей накладывают сразу «{}» и «{}».",
                    x.name, y.name
                ));
            }
        }
        (Some(x), None) if hits(&pb) => {
            pb.on_hit = Some(x.clone());
            pros.push(format!(
                "«{}» теперь тоже накладывает «{}».",
                b.name, x.name
            ));
        }
        (None, Some(y)) if hits(&pa) => {
            pa.on_hit = Some(y.clone());
            pros.push(format!(
                "«{}» теперь тоже накладывает «{}».",
                a.name, y.name
            ));
        }
        _ => {}
    }

    // a healing part lends its life to the blows of the other
    for (heal, other) in [(&a, &mut pb), (&b, &mut pa)] {
        if heal.kind == "heal" && deals_damage(other) && other.leech < HEAL_LEECH {
            other.leech = HEAL_LEECH;
            pros.push(format!(
                "Живительная сила: «{}» лечит вас на {}% нанесённого урона.",
                other.name,
                num(HEAL_LEECH)
            ));
        }
    }
    // leech, finishing blows, blows in the back
    let leech = pa.leech.max(pb.leech);
    if leech > 0.0 && deals_damage(a) && deals_damage(b) && pa.leech.min(pb.leech) < leech {
        pa.leech = leech;
        pb.leech = leech;
        pros.push(format!("Вампиризм {}% у обеих частей.", num(leech)));
    }
    let execute = a.execute.max(b.execute);
    for p in [&mut pa, &mut pb] {
        if execute > 0.0 && p.kind == "strike" && p.execute < execute {
            p.execute = execute;
            pros.push(format!(
                "Добивание: «{}» сильнее бьёт раненых врагов.",
                p.name
            ));
        }
    }
    let backstab = a.backstab.max(b.backstab);
    if backstab > 0.0 && deals_damage(a) && deals_damage(b) && a.backstab.min(b.backstab) < backstab
    {
        pa.backstab = backstab;
        pb.backstab = backstab;
        pros.push(format!("Удар в спину ×{} у обеих частей.", num(backstab)));
    }

    // what every fusion pays
    let apart = a.mana + b.mana;
    let mana = (apart * MANA_MULT).max(apart + MANA_MIN).round();
    cons.push(format!(
        "Расход маны {} вместо {} за оба умения порознь.",
        num(mana),
        num(apart)
    ));
    let (long, short) = (
        a.cooldown_ms.max(b.cooldown_ms),
        a.cooldown_ms.min(b.cooldown_ms),
    );
    let cooldown = ((long + short / 2 + 50) / 100 * 100).max(1000);
    cons.push(format!(
        "Перезарядка {} с — дольше, чем у каждой части ({} с и {} с).",
        secs(cooldown),
        secs(a.cooldown_ms),
        secs(b.cooldown_ms)
    ));
    for p in [&mut pa, &mut pb] {
        if let Some(bf) = p.buff.as_mut() {
            let was = bf.duration_ms;
            bf.duration_ms = ((was as f64 * BUFF_TIME / 100.0).round() * 100.0) as i64;
            cons.push(format!(
                "Действие «{}» короче: {} с вместо {} с.",
                bf.name,
                secs(bf.duration_ms),
                secs(was)
            ));
        }
    }
    if !a.equip.is_empty() && !b.equip.is_empty() && a.equip != b.equip {
        cons.push(format!(
            "Нужно снаряжение обеих частей: {} и {}.",
            gear_need_name(&a.equip),
            gear_need_name(&b.equip)
        ));
    }
    let (mut fizzle, mut backlash) = (0.0, 0.0);
    if let Some((x, y)) = clash {
        fizzle += CLASH_FIZZLE;
        backlash += CLASH_BACKLASH;
        cons.push(format!(
            "Противоборство стихий «{}» и «{}»: шанс срыва +{}%, отдача {}% здоровья.",
            type_name(d, x),
            type_name(d, y),
            num(CLASH_FIZZLE),
            num(CLASH_BACKLASH)
        ));
    }
    let mut weakness = None;
    match flaw_of(&a.key, &b.key) {
        Flaw::Backlash => {
            backlash += 6.0;
            cons.push("Отдача: каждое применение отнимает 6% здоровья.".into());
        }
        Flaw::Unstable => {
            fizzle += 12.0;
            cons.push(
                "Нестабильность: с шансом 12% слияние срывается — мана и перезарядка тратятся впустую."
                    .into(),
            );
        }
        Flaw::Exhaustion => {
            weakness = Some(drawback(
                "fusion_exhaustion",
                "Истощение",
                4000,
                &[("move_speed", -30.0)],
            ));
            cons.push("Истощение: после применения 4 с вы бегаете на 30% медленнее.".into());
        }
        Flaw::Vulnerable => {
            weakness = Some(drawback(
                "fusion_vulnerable",
                "Уязвимость",
                5000,
                &[("res_all", -15.0), ("armor", -6.0)],
            ));
            cons.push(
                "Уязвимость: после применения 5 с у вас −15% сопротивления всему урону и −6 брони."
                    .into(),
            );
        }
        Flaw::Numbness => {
            weakness = Some(drawback(
                "fusion_numbness",
                "Оцепенение",
                5000,
                &[("attack_speed", -35.0)],
            ));
            cons.push("Оцепенение: после применения 5 с вы атакуете на 35% медленнее.".into());
        }
    }

    let equip = if b.equip.is_empty() || a.equip == b.equip {
        a.equip.clone()
    } else if a.equip.is_empty() {
        b.equip.clone()
    } else {
        String::new() // two needs: both are checked through the parts
    };
    let range = reach(&pa).max(reach(&pb));
    let parts = if dash_first {
        vec![pb, pa]
    } else {
        vec![pa, pb]
    };
    let def = AbilityDef {
        key: fusion_key(&a.key, &b.key),
        name: format!("{} + {}", a.name, b.name),
        desc: format!(
            "Слияние умений «{}» и «{}»: оба срабатывают одним применением.",
            a.name, b.name
        ),
        kind: FUSION_KIND.into(),
        mana,
        cooldown_ms: cooldown,
        range,
        glyph: a.glyph.clone(),
        color: a.color.clone(),
        fx: a.fx.clone(),
        equip,
        parts,
        fizzle,
        backlash,
        drawback: weakness,
        ..Default::default()
    };
    Fusion { def, pros, cons }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{index, load_default};

    /// The class abilities that may fuse, with their classes.
    fn class_abilities(d: &Db) -> Vec<(&AbilityDef, &str)> {
        d.b.abilities
            .iter()
            .filter(|a| fusable_kind(&a.kind))
            .filter_map(|a| d.ability_class(&a.key).map(|c| (a, c)))
            .collect()
    }

    /// Every pair of abilities of different classes fuses into an ability
    /// that costs more than its parts, names its strengths and its flaws,
    /// and passes the content checks; all flaws occur.
    #[test]
    fn every_pair_fuses() {
        let (d, _) = load_default(None).unwrap();
        let all = class_abilities(&d);
        assert!(all.len() > 100, "{}", all.len());
        let mut flaws = std::collections::HashSet::new();
        let mut sample = Vec::new();
        let mut n = 0;
        for (i, &(a, ca)) in all.iter().enumerate() {
            for &(b, cb) in &all[i + 1..] {
                if ca == cb || gear_clash(&a.equip, &b.equip) {
                    continue;
                }
                n += 1;
                let f = fuse(&d, a, b);
                let def = &f.def;
                assert_eq!(def.kind, FUSION_KIND);
                assert_eq!(def.parts.len(), 2);
                assert!(def.mana >= a.mana + b.mana + MANA_MIN, "{}", def.key);
                assert!(def.cooldown_ms >= a.cooldown_ms.max(b.cooldown_ms));
                assert!(!f.pros.is_empty() && f.cons.len() >= 3, "{}", def.key);
                assert!(def.parts.iter().all(|p| p.kind != FUSION_KIND));
                flaws.insert(flaw_of(&a.key, &b.key));
                if n % 97 == 0 {
                    sample.push(def.clone());
                }
            }
        }
        assert!(n > 5000, "{n} pairs");
        assert_eq!(flaws.len(), FLAWS.len());
        // fusions written as content pass the checks (with keys of their own)
        let mut b = d.b.clone();
        for (i, mut def) in sample.into_iter().enumerate() {
            def.key = format!("fused_sample_{i}");
            b.abilities.push(def);
        }
        index(b).unwrap();
    }

    #[test]
    fn fused_keys_resolve() {
        let (d, _) = load_default(None).unwrap();
        let key = fusion_key("fireball", "smite");
        let a = d.ability(&key).expect("fusion");
        assert_eq!(a.key, key);
        assert!(std::ptr::eq(a, d.ability(&key).unwrap()), "made once");
        assert_eq!(fusion_parts(&key), Some(("fireball", "smite")));
        // fire and holy: a dual element
        assert_eq!(a.parts[0].split, vec!["fire", "holy"]);
        assert!(d.ability("fuse:fireball+fireball").is_none());
        assert!(d.ability("fuse:fireball+nothing").is_none());
        assert!(d.ability(&fusion_key(&key, "smite")).is_none());
        // fire and cold fight each other
        let f = fuse_key(&d, &fusion_key("fireball", "frost_nova")).unwrap();
        assert!(f.def.fizzle >= CLASH_FIZZLE && f.def.backlash >= CLASH_BACKLASH);
        assert_eq!(d.ability_class("fireball"), Some("mage"));
        assert_eq!(d.ability_class("power_strike"), Some("warrior"));
    }
}
