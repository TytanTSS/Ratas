//! The balance of the belts of danger: seasoned heroes against the
//! creatures of their lands, alone and in a party.

use super::tests::{run, setup, wild};
use super::*;
use crate::proto::Input;
use crate::world::Vec2;

/// How a fight ended.
#[derive(Debug)]
pub(crate) struct Outcome {
    /// the creature fell
    pub won: bool,
    /// heroes who fell
    pub fallen: usize,
    pub secs: f64,
    /// the heroes' health left (0..1 of their total)
    pub left: f64,
}

/// A party of heroes of a level and classes on open land with nothing
/// around and no new monsters coming.
pub(crate) fn party(classes: &[&str], lvl: i32) -> (Game, Vec<Id>) {
    let mut g = setup();
    g.next_spawn = f64::MAX;
    // the same gear and blows every time
    g.rng = Rng::new(17, 3);
    let lead = g.join_for_test("Veteran", classes[0]);
    let at = wild(&mut g, lead);
    let mut out = vec![lead];
    for (i, c) in classes.iter().enumerate().skip(1) {
        let id = g.join_for_test(&format!("Ally{i}"), c);
        g.place_for_test(
            id,
            "overworld",
            at.add(Pos::new(i as i32 % 3 - 1, -1 - i as i32 / 3)),
        );
        g.party_for_test(lead, id);
        out.push(id);
    }
    for &id in &out {
        g.veteran_for_test(id, lvl);
    }
    g.index_levels();
    (g, out)
}

/// Heroes fight a creature as players would: the melee ones close in, the
/// archers and casters keep their distance; everyone strikes the nearest
/// foe (servants first) and uses the abilities of the quick-access bar in
/// turn (mana permitting).
pub(crate) fn fight(g: &mut Game, heroes: &[Id], m: Id, max_secs: f64) -> Outcome {
    let t0 = g.now;
    let mut turn = 0;
    while g.now - t0 < max_secs * 1000.0 && g.ents.contains_key(&m) {
        let mut standing = 0;
        for &h in heroes {
            let e = &g.ents[&h];
            if !e.alive() {
                continue;
            }
            standing += 1;
            let foe = g
                .near(&e.level, e.pos, 6.0)
                .into_iter()
                .filter(|&o| {
                    o != m
                        && g.ents[&o].alive()
                        && g.ents[&o].monster.is_some()
                        && g.ents[&o].faction == Faction::Monster
                })
                .min_by(|a, b| {
                    let (da, db) = (g.ents[a].pos.dist(e.pos), g.ents[b].pos.dist(e.pos));
                    da.partial_cmp(&db).unwrap()
                })
                .unwrap_or(m);
            let mp = g.ents[&foe].pos;
            let e = &g.ents[&h];
            let p = e.p();
            let far = p.class != "warrior" && (e.stats.gear.ranged || e.stats.int > e.stats.str_);
            let d = mp - e.pos;
            let dist = d.len();
            let reach = g.melee_reach(h) - 0.1;
            let toward = |v: Vec2| -> [i8; 2] {
                let s = |x: f32| {
                    if x > 0.3 {
                        1
                    } else if x < -0.3 {
                        -1
                    } else {
                        0
                    }
                };
                let n = v.norm();
                [s(n.x), s(n.y)]
            };
            let mv = if far {
                if dist > 6.0 {
                    toward(d)
                } else if dist < 3.0 {
                    toward(d * -1.0)
                } else {
                    [0, 0]
                }
            } else if dist > reach {
                toward(d)
            } else {
                [0, 0]
            };
            let cells = p.hotbar.len().max(1) as i32;
            let ability = 1 + (turn + h as i32) % cells;
            g.set_input(
                h,
                &Input {
                    mv,
                    aim: Some([mp.x, mp.y]),
                    attack: true,
                    ability: ability as i8,
                    ..Default::default()
                },
            );
        }
        if standing == 0 {
            break;
        }
        turn += 1;
        run(g, 3);
    }
    let (hp, max): (f64, f64) = heroes
        .iter()
        .map(|h| (g.ents[h].hp.max(0.0), g.ents[h].max_hp))
        .fold((0.0, 0.0), |a, b| (a.0 + b.0, a.1 + b.1));
    Outcome {
        won: !g.ents.contains_key(&m),
        fallen: heroes.iter().filter(|h| !g.ents[h].alive()).count(),
        secs: (g.now - t0) / 1000.0,
        left: hp / max.max(1.0),
    }
}

/// A creature of a level a few steps from the heroes, already angry.
pub(crate) fn foe(g: &mut Game, key: &str, near: Id, lvl: i32) -> Id {
    let at = g.ents[&near].cell().add(Pos::new(4, 0));
    let id = g.spawn_for_test(key, "overworld", at, lvl).unwrap();
    g.ents
        .get_mut(&id)
        .unwrap()
        .monster
        .as_mut()
        .unwrap()
        .target = near;
    id
}

/// A creature for a party is too much for one seasoned hero of its level
/// but falls to three, while the common beasts of its land are fair game
/// alone.
#[test]
fn party_creatures_need_a_party() {
    let (mut g, hs) = party(&["warrior"], 24);
    let m = foe(&mut g, "dire_wolf", hs[0], 24);
    assert!(
        fight(&mut g, &hs, m, 60.0).won,
        "a lone hero loses to a dire wolf"
    );
    let (mut g, hs) = party(&["warrior"], 24);
    let m = foe(&mut g, "stone_giant", hs[0], 24);
    let o = fight(&mut g, &hs, m, 90.0);
    assert!(
        !o.won && o.fallen == 1,
        "a lone hero against a stone giant: {o:?}"
    );
    let (mut g, hs) = party(&["warrior", "mage", "priest"], 24);
    let m = foe(&mut g, "stone_giant", hs[0], 24);
    let o = fight(&mut g, &hs, m, 90.0);
    assert!(o.won, "three heroes against a stone giant: {o:?}");
}

/// Prints how seasoned heroes fare against creatures
/// (`cargo test --release -p ratas-core balance_table -- --ignored --nocapture`,
/// BALANCE="ogre:28,frost_giant:25" for other creatures).
#[test]
#[ignore]
fn balance_table() {
    let spec = std::env::var("BALANCE").unwrap_or("orc:25,cave_troll:25,frost_giant:26".into());
    let foes: Vec<(String, i32)> = spec
        .split(',')
        .filter_map(|s| {
            let (k, l) = s.split_once(':')?;
            Some((k.to_string(), l.parse().ok()?))
        })
        .collect();
    let lineups: &[&[&str]] = &[
        &["warrior"],
        &["mage"],
        &["ranger"],
        &["warrior", "priest"],
        &["warrior", "mage", "priest"],
        &["warrior", "paladin", "mage", "priest"],
        &["warrior", "paladin", "mage", "ranger", "priest"],
    ];
    let hero_lvl: i32 = std::env::var("HERO")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(25);
    for c in ["warrior", "mage", "ranger", "priest", "paladin"] {
        let (g, hs) = party(&[c], hero_lvl);
        let e = &g.ents[&hs[0]];
        println!(
            "{c} {hero_lvl}: hp {:.0} armor {:.0} str {:.0} dex {:.0} int {:.0} weapon {:?} every {:.0}ms, skills {:?}, bar {:?}",
            e.max_hp,
            e.stats.armor,
            e.stats.str_,
            e.stats.dex,
            e.stats.int,
            e.stats.weapon_dmg,
            e.stats.attack_ms,
            e.p().skills.len(),
            e.p().hotbar
        );
    }
    for (key, lvl) in &foes {
        let def = db().monster(key).unwrap();
        let (ms, hp) = allies::monster_state(def, *lvl, Vec2::ZERO);
        println!(
            "{key} lvl {lvl}: hp {hp:.0} damage {:?} armor {:.0}",
            ms.damage, ms.armor
        );
        for lineup in lineups {
            let mut wins = 0;
            let mut secs = 0.0;
            let mut fallen = 0;
            let tries = 3;
            for _ in 0..tries {
                let (mut g, hs) = party(lineup, hero_lvl);
                let p = hs[0];
                let m = foe(&mut g, key, p, *lvl);
                let o = fight(&mut g, &hs, m, 180.0);
                wins += o.won as i32;
                secs += o.secs;
                fallen += o.fallen;
            }
            println!(
                "  {:<40} wins {wins}/{tries}, {:.0} s, fallen {fallen}",
                lineup.join("+"),
                secs / tries as f64
            );
        }
    }
}

/// Damage per second of seasoned heroes of each class and level against a
/// harmless target of their level
/// (`cargo test --release -p ratas-core balance_dps -- --ignored --nocapture`).
#[test]
#[ignore]
fn balance_dps() {
    for lvl in [10, 15, 20, 25, 30] {
        let mut row = format!("lvl {lvl:>2}:");
        for c in [
            "warrior", "paladin", "rogue", "ranger", "mage", "priest", "monk", "skald",
        ] {
            let (mut g, hs) = party(&[c], lvl);
            let m = foe(&mut g, "cave_troll", hs[0], lvl);
            {
                let e = g.ents.get_mut(&m).unwrap();
                e.max_hp = 1e6;
                e.hp = 1e6;
                e.monster.as_mut().unwrap().damage = [0.0, 0.0];
            }
            fight(&mut g, &hs, m, 30.0);
            let dealt = 1e6 - g.ents[&m].hp;
            row += &format!(" {c} {:.0}", dealt / 30.0);
        }
        println!("{row}");
    }
}

/// Seasoned heroes alone against a creature of their own level
/// (`cargo test --release -p ratas-core balance_duels -- --ignored --nocapture`,
/// FOES="orc,cave_troll", LEVELS="10,20,30").
#[test]
#[ignore]
fn balance_duels() {
    let foes = std::env::var("FOES").unwrap_or("orc,cave_troll".into());
    let levels = std::env::var("LEVELS").unwrap_or("10,15,20,25,30".into());
    let classes = ["warrior", "paladin", "rogue", "ranger", "mage", "priest"];
    for key in foes.split(',') {
        for lvl in levels.split(',').filter_map(|s| s.parse::<i32>().ok()) {
            let mut row = format!("{key} {lvl:>2}:");
            for c in classes {
                let (mut won, mut secs, mut left) = (0, 0.0, 0.0);
                for _ in 0..3 {
                    let (mut g, hs) = party(&[c], lvl);
                    let m = foe(&mut g, key, hs[0], lvl);
                    let o = fight(&mut g, &hs, m, 120.0);
                    won += o.won as i32;
                    secs += o.secs / 3.0;
                    left += o.left / 3.0;
                }
                row += &format!(" | {c} {won}/3 {secs:.0}s {:.0}%", left * 100.0);
            }
            println!("{row}");
        }
    }
}

/// What the belts of danger of a full-size world hold
/// (`cargo test --release -p ratas-core zone_stats -- --ignored --nocapture`).
#[test]
#[ignore]
fn zone_stats() {
    let t0 = std::time::Instant::now();
    let g = Game::new(7, None);
    let made = t0.elapsed();
    let ow = &g.levels["overworld"];
    let mut land = [0usize; 5];
    for y in (0..ow.h).step_by(4) {
        for x in (0..ow.w).step_by(4) {
            if ow.walkable(x, y) {
                land[g.zone_tier_at(Pos::new(x, y))] += 1;
            }
        }
    }
    let total: usize = land.iter().sum();
    let mut villages = [0; 5];
    for v in &g.villages {
        villages[g.zone_tier_at(v.center)] += 1;
    }
    let mut dungeons = [0; 5];
    let mut depth = [0; 5];
    let mut top = [0; 5];
    for (i, e) in g.entrances.iter().enumerate() {
        let t = gen::tier_of(g.dungeon_levels(i)[0]);
        dungeons[t] += 1;
        depth[t] = depth[t].max(e.max_depth);
        top[t] = top[t].max(g.dungeon_levels(i)[1]);
    }
    let mut elites = [0; 5];
    let mut lairs = [0; 5];
    let mut lvl = [(99, 0); 5];
    for e in g.ents.values() {
        let Some(m) = &e.monster else { continue };
        let Some(d) = db().monster(&m.def) else {
            continue;
        };
        if e.faction != Faction::Monster || !d.elite {
            continue;
        }
        let t = g.zone_tier_at(e.cell());
        if d.party > 1 {
            lairs[t] += 1;
        } else {
            elites[t] += 1;
        }
        lvl[t] = (lvl[t].0.min(m.lvl), lvl[t].1.max(m.lvl));
    }
    println!("made in {made:?}, start {:?}", g.start);
    for (t, tier) in gen::TIERS.iter().enumerate() {
        println!(
            "{:<14} land {:>4.1}%  villages {:>3}  dungeons {:>3} (floors ≤{}, lvl ≤{})  elites {:>3}  lairs {:>3}  lvl {:?}",
            tier.name,
            100.0 * land[t] as f64 / total as f64,
            villages[t],
            dungeons[t],
            depth[t],
            top[t],
            elites[t],
            lairs[t],
            lvl[t]
        );
    }
}
