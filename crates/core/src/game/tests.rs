//! Tests of the simulation: movement, combat, progression, the world.

use super::*;
use crate::proto::{Command, Input};
use crate::world::{Vec2, DIRS4};

/// A world of the classic size: quick to make, and every kind of content in it.
pub(crate) fn setup() -> Game {
    Game::with_size(7, 300, 200, None)
}

pub(crate) fn run(g: &mut Game, ticks: usize) {
    for _ in 0..ticks {
        g.tick();
        let ids: Vec<Id> = g.online.values().copied().collect();
        for p in ids {
            g.snapshot(p);
            g.take_outbox(p);
        }
        g.end_frame();
    }
}

fn input(mv: [i8; 2]) -> Input {
    Input {
        mv,
        ..Default::default()
    }
}

fn cmd(kind: &str, key: &str, index: i32) -> Command {
    Command::new(kind, key, index)
}

/// Puts a hero on open land outside villages, with no monsters around.
pub(crate) fn wild(g: &mut Game, p: Id) -> Pos {
    let (w, h) = (g.levels["overworld"].w, g.levels["overworld"].h);
    let start = g.ents[&p].cell();
    for y in 6..h - 6 {
        for x in 6..w - 6 {
            let q = Pos::new(x, y);
            let l = &g.levels["overworld"];
            let open = (-3..=3).all(|dy| {
                (-3..=3).all(|dx| {
                    let d = l.def(x + dx, y + dy);
                    l.walkable(x + dx, y + dy)
                        && d.damage == 0.0
                        && d.interact.is_empty()
                        && d.move_cost <= 1.0
                })
            });
            if open && !g.in_village(q, 10) && q.dist(start) > 20 && !g.cell_taken("overworld", q) {
                g.place_for_test(p, "overworld", q);
                g.index_levels();
                for e in g.on_level("overworld") {
                    if e != p
                        && g.ents[&e].monster.is_some()
                        && g.ents[&e].pos.dist(q.center()) < 15.0
                    {
                        g.remove(e);
                    }
                }
                g.index_levels();
                return q;
            }
        }
    }
    panic!("no wild land");
}

pub(crate) fn spawn_at(g: &mut Game, key: &str, p: Id, offset: Vec2) -> Id {
    let e = &g.ents[&p];
    let (level, pos) = (e.level.clone(), e.pos + offset);
    let id = g.new_monster(db().monster(key).unwrap(), &level, pos.cell(), 1);
    g.place(id, pos);
    let ms = g.ents.get_mut(&id).unwrap().monster.as_mut().unwrap();
    ms.target = 0;
    g.index_levels();
    id
}

fn give(g: &mut Game, p: Id, key: &str) -> usize {
    g.add_item(p, ItemStack::new(key));
    g.ents[&p]
        .p()
        .inventory
        .iter()
        .position(|s| s.key == key)
        .unwrap()
}

#[test]
fn join_move_and_explore() {
    let mut g = setup();
    let r = g.join("Тест", "").unwrap();
    assert_eq!(r, (None, true), "a new hero must choose a class");
    let p = g.join_for_test("Тест", "warrior");
    assert_eq!(g.join("Тест", "warrior").unwrap_err(), ERR_NAME_TAKEN);
    let e = &g.ents[&p];
    assert!(e.hp > 0.0 && e.max_hp >= 50.0);
    let start = e.pos;
    let mut moved = false;
    for d in DIRS4 {
        g.set_input(p, &input([d.x as i8, d.y as i8]));
        run(&mut g, 5);
        if g.ents[&p].pos.dist(start) > 0.3 {
            moved = true;
            break;
        }
    }
    assert!(moved, "the hero could not move in any direction");
    assert!(!g.ents[&p].p().explored["overworld"].0.is_empty());
    run(&mut g, 200);
    assert!(
        g.ents.values().any(|e| e.kind == Kind::Monster),
        "no monsters spawned"
    );
}

/// Heroes walk smoothly in eight directions at their own pace.
#[test]
fn continuous_diagonal_movement() {
    let mut g = setup();
    let p = g.join_for_test("Тест", "warrior");
    wild(&mut g, p);
    let start = g.ents[&p].pos;
    let speed = g.ents[&p].speed(1.0);
    for _ in 0..TPS {
        g.set_input(p, &input([1, 1]));
        run(&mut g, 1);
    }
    let end = g.ents[&p].pos;
    let d = end - start;
    assert!((d.x - d.y).abs() < 0.05, "not diagonal: {d:?}");
    assert!(
        (d.len() - speed).abs() < speed * 0.1,
        "walked {} in a second at speed {speed}",
        d.len()
    );
    // positions between cells
    assert!(end.x.fract() != 0.5 || end.y.fract() != 0.5 || d.len() > 0.0);
    // releasing the keys stops the hero
    run(&mut g, 15);
    let still = g.ents[&p].pos;
    run(&mut g, 5);
    assert_eq!(still, g.ents[&p].pos);
}

/// Walls stop heroes, and a hero slides along a wall when walking diagonally into it.
#[test]
fn walls_block_and_slide() {
    let mut g = setup();
    let p = g.join_for_test("Тест", "warrior");
    assert!(g.teleport_for_test(p, &dungeon_level_id(0, 1)));
    let lid = g.ents[&p].level.clone();
    for _ in 0..200 {
        g.set_input(p, &input([-1, 0]));
        run(&mut g, 1);
        let e = &g.ents[&p];
        let l = &g.levels[&lid];
        assert!(
            l.circle_fits(e.pos, e.radius() * 0.99, &|x, y| !l.walkable(x, y)),
            "inside a wall at {:?}",
            e.pos
        );
    }
}

#[test]
fn dungeon_combat_and_save() {
    let mut g = setup();
    let p = g.join_for_test("Герой", "mage");
    let lid = dungeon_level_id(0, 1);
    assert!(g.teleport_for_test(p, &lid));
    assert_eq!(g.ents[&p].level, lid);
    g.tough_for_test(p);
    let mut kills = 0;
    for _ in 0..300 {
        let pos = g.ents[&p].pos;
        let target = g
            .on_level(&lid)
            .into_iter()
            .filter(|o| g.ents[o].kind == Kind::Monster)
            .min_by(|a, b| {
                g.ents[a]
                    .pos
                    .dist(pos)
                    .partial_cmp(&g.ents[b].pos.dist(pos))
                    .unwrap()
            });
        let Some(t) = target else { break };
        g.move_next_to(p, t);
        let tp = g.ents[&t].pos;
        g.ents.get_mut(&p).unwrap().mp = 1000.0;
        g.set_input(
            p,
            &Input {
                ability: 1,
                attack: true,
                aim: Some([tp.x, tp.y]),
                ..Default::default()
            },
        );
        let before = g.ents[&p].p().kills;
        run(&mut g, 10);
        kills += g.ents[&p].p().kills - before;
    }
    assert!(kills > 0, "no kills in the dungeon");
    let dir = std::env::temp_dir().join(format!("ratas-test-{}", std::process::id()));
    let path = dir.join("w.sav");
    g.save(&path).unwrap();
    let mut g2 = Game::load(&path, None).unwrap();
    let p2 = g2.join("Герой", "").unwrap().0.unwrap();
    let (a, b) = (g.ents[&p].p(), g2.ents[&p2].p());
    assert_eq!((a.level, a.kills), (b.level, b.kills));
    assert_eq!(g2.ents[&p2].level, lid);
    run(&mut g2, 20);
    let infos = list_saves(&dir);
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].world_name, g.world_name);
    std::fs::remove_dir_all(dir).ok();
}

/// Projectiles fly over time instead of hitting at once.
#[test]
fn projectiles_fly() {
    let mut g = setup();
    let p = g.join_for_test("Тест", "mage");
    wild(&mut g, p);
    let wolf = spawn_at(&mut g, "wolf", p, Vec2::new(6.0, 0.0));
    let wp = g.ents[&wolf].pos;
    g.ents.get_mut(&p).unwrap().pm().hotbar[0] = "magic_missile".into();
    g.unlock_ability(p, "magic_missile");
    g.ents.get_mut(&p).unwrap().mp = 100.0;
    g.set_input(
        p,
        &Input {
            ability: 1,
            aim: Some([wp.x, wp.y]),
            ..Default::default()
        },
    );
    run(&mut g, 1);
    let flying: Vec<&Entity> = g
        .ents
        .values()
        .filter(|e| e.kind == Kind::Projectile)
        .collect();
    assert!(!flying.is_empty(), "no projectile in flight");
    assert!(g.ents[&wolf].hp >= g.ents[&wolf].max_hp, "hit at once");
    run(&mut g, 30);
    assert!(
        !g.ents.contains_key(&wolf) || g.ents[&wolf].hp < g.ents[&wolf].max_hp,
        "the missile never hit"
    );
}

/// A mouse-aimed ability goes to the enemy under the cursor, not the nearest.
#[test]
fn mouse_aim() {
    let mut g = setup();
    let p = g.join_for_test("Тест", "mage");
    wild(&mut g, p);
    let near = spawn_at(&mut g, "wolf", p, Vec2::new(2.0, 0.0));
    let far = spawn_at(&mut g, "wolf", p, Vec2::new(-4.0, 0.0));
    // keep them still
    for w in [near, far] {
        g.apply_buff(
            w,
            &crate::content::BuffDef {
                key: "freeze".into(),
                stun: true,
                duration_ms: 60000,
                ..Default::default()
            },
            0,
        );
    }
    g.unlock_ability(p, "magic_missile");
    g.ents.get_mut(&p).unwrap().pm().hotbar[0] = "magic_missile".into();
    g.ents.get_mut(&p).unwrap().mp = 100.0;
    let fp = g.ents[&far].pos;
    g.set_input(
        p,
        &Input {
            ability: 1,
            aim: Some([fp.x, fp.y]),
            ..Default::default()
        },
    );
    run(&mut g, 30);
    assert!(
        g.ents.get(&far).is_none_or(|f| f.hp < f.max_hp),
        "the aimed wolf was not hit"
    );
    assert!(
        g.ents[&near].hp >= g.ents[&near].max_hp,
        "the nearest wolf was hit instead"
    );
    // aimed at empty ground, a projectile flies there anyway
    g.remove(near);
    g.remove(far);
    let e = g.ents.get_mut(&p).unwrap();
    e.cooldowns.clear();
    e.mp = 100.0;
    let pos = e.pos;
    g.set_input(
        p,
        &Input {
            ability: 1,
            aim: Some([pos.x, pos.y - 3.0]),
            ..Default::default()
        },
    );
    run(&mut g, 1);
    let up = g
        .ents
        .values()
        .any(|e| e.kind == Kind::Projectile && e.vel.y < 0.0 && e.vel.x.abs() < 0.1);
    assert!(up, "no projectile toward the empty ground");
    let f = g.ents[&p].facing;
    assert!((f + std::f32::consts::FRAC_PI_2).abs() < 0.1, "facing {f}");
}

/// The knight's long reach hits an enemy two tiles away.
#[test]
fn knight_reach() {
    let mut g = setup();
    let p = g.join_for_test("Тест", "warrior");
    wild(&mut g, p);
    let far = spawn_at(&mut g, "wolf", p, Vec2::new(2.0, 0.0));
    g.apply_buff(
        far,
        &crate::content::BuffDef {
            key: "freeze".into(),
            stun: true,
            duration_ms: 60000,
            ..Default::default()
        },
        0,
    );
    g.ents.get_mut(&p).unwrap().facing = 0.0;
    g.attack_facing(p);
    assert!(
        g.ents[&far].hp >= g.ents[&far].max_hp,
        "hit two tiles away without the skill"
    );
    assert!(!g.in_reach(p, far));
    g.ents
        .get_mut(&p)
        .unwrap()
        .pm()
        .skills
        .insert("long_reach".into(), 1);
    g.ents.get_mut(&p).unwrap().recalc();
    assert_eq!(g.ents[&p].stats.reach, 1.0);
    assert!(g.in_reach(p, far));
    for _ in 0..10 {
        if g.ents[&far].hp < g.ents[&far].max_hp {
            break;
        }
        g.ents.get_mut(&p).unwrap().next_attack = 0.0;
        g.attack_facing(p);
    }
    assert!(
        g.ents[&far].hp < g.ents[&far].max_hp,
        "the long reach did not hit"
    );
    let farther = spawn_at(&mut g, "wolf", p, Vec2::new(0.0, 3.4));
    assert!(!g.in_reach(p, farther), "reach too long");
}

#[test]
fn skills_and_items() {
    let mut g = setup();
    let p = g.join_for_test("Учёный", "rogue");
    g.give_xp(p, 700);
    assert!(g.ents[&p].p().level >= 4);
    g.command(p, &cmd("learn", "swift", 0));
    assert_eq!(g.ents[&p].p().skill("swift"), 1);
    g.command(p, &cmd("learn", "smoke_bomb", 0));
    assert_eq!(
        g.ents[&p].p().skill("smoke_bomb"),
        0,
        "smoke bomb requires level 6"
    );
    g.command(p, &cmd("learn", "toughness", 0));
    assert_eq!(
        g.ents[&p].p().skill("toughness"),
        0,
        "a warrior skill without the class"
    );
    let before = g.ents[&p].stats.move_ms;
    g.command(p, &cmd("learn", "swift", 0));
    assert!(
        g.ents[&p].stats.move_ms < before,
        "move speed skill had no effect"
    );
    g.command(p, &cmd("alloc_attr", "dex", 0));
    let base = db().class("rogue").unwrap().attrs["dex"];
    assert_eq!(g.ents[&p].p().attrs["dex"], base + 1.0);
}

#[test]
fn equipment_slots() {
    let mut g = setup();
    let p = g.join_for_test("Тест", "warrior");
    let eq = |g: &Game, s: &str| {
        g.ents[&p]
            .p()
            .equip
            .get(s)
            .map(|i| i.key.clone())
            .unwrap_or_default()
    };
    assert_eq!(eq(&g, SLOT_MAIN), "short_sword");
    assert_eq!(eq(&g, SLOT_OFF), "wooden_shield");
    assert!(g.ents[&p].stats.gear.shield);
    let i = give(&mut g, p, "greatsword");
    g.equip(p, i, false);
    assert_eq!(eq(&g, SLOT_OFF), "");
    assert!(g.ents[&p].stats.gear.two_hand);
    let i = give(&mut g, p, "round_shield");
    g.equip(p, i, false);
    assert_eq!(
        (eq(&g, SLOT_MAIN), eq(&g, SLOT_OFF)),
        ("".into(), "round_shield".into())
    );
    let i = give(&mut g, p, "dagger");
    g.equip(p, i, false);
    let i = give(&mut g, p, "short_sword");
    g.equip(p, i, true);
    assert!(g.ents[&p].stats.gear.dual);
    for r in [
        "ring_vigor",
        "ring_mind",
        "ring_fire",
        "ring_frost",
        "ring_storm",
    ] {
        let i = give(&mut g, p, r);
        g.equip(p, i, false);
    }
    for s in ["ring1", "ring2", "ring3", "ring4"] {
        assert!(!eq(&g, s).is_empty(), "ring slot {s} empty");
    }
    assert!(g.ents[&p].stats.resist("fire") >= 25.0);
}

#[test]
fn classes_and_subclasses() {
    let mut g = setup();
    let p = g.join_for_test("Тест", "mage");
    assert!(can_choose_subclass(g.ents[&p].p(), "pyromancer").contains("уровень класса"));
    assert!(!can_start_class(g.ents[&p].p(), "warrior").is_empty());
    g.give_xp(p, 5000);
    for k in ["wisdom", "wisdom", "meditation", "meditation", "wisdom"] {
        g.learn_skill(p, k);
    }
    assert_eq!(class_level(g.ents[&p].p(), "mage"), 5);
    assert!(can_learn(g.ents[&p].p(), db().skill("fireball")).contains("подкласс"));
    assert!(!can_choose_subclass(g.ents[&p].p(), "sealer").is_empty());
    g.choose_subclass(p, "pyromancer");
    assert!(!can_choose_subclass(g.ents[&p].p(), "cryomancer").is_empty());
    g.learn_skill(p, "fireball");
    assert!(g.ents[&p].p().abilities.contains(&"fireball".to_string()));
    g.start_class(p, "warrior");
    assert!(g.ents[&p].p().has_class("warrior"));
    assert!(!can_start_class(g.ents[&p].p(), "shaman").is_empty());
    g.unlock(p, "class:shaman");
    assert_eq!(can_start_class(g.ents[&p].p(), "shaman"), "");
    g.unlock(p, "skill:titan_blood");
    assert_eq!(g.ents[&p].p().skill("titan_blood"), 1);
    g.respec(p);
    let pl = g.ents[&p].p();
    assert_eq!(pl.skill("titan_blood"), 1);
    assert!(pl.subclasses.is_empty() && pl.classes.len() == 1);
}

#[test]
fn pvp_and_party() {
    let mut g = setup();
    let a = g.join_for_test("Аня", "warrior");
    let b = g.join_for_test("Боря", "rogue");
    assert!(!g.hostile(a, b), "players fight in the village");
    let q = wild(&mut g, a);
    g.place_for_test(b, "overworld", q.add(Pos::new(1, 0)));
    assert!(g.hostile(a, b), "no PvP in the wild");
    let hp = g.ents[&b].hp;
    g.ents.get_mut(&b).unwrap().stats.dodge = 0.0;
    g.damage(Some(a), b, Damage::single("slash", 10.0));
    assert!(g.ents[&b].hp < hp);
    g.command(a, &cmd("party_invite", "Боря", 0));
    g.command(b, &cmd("party_accept", "Аня", 0));
    assert!(g.same_party(a, b) && !g.hostile(a, b));
    let snap = g.snapshot(a);
    assert_eq!(snap.you.party.len(), 1);
    let m = spawn_at(&mut g, "wolf", a, Vec2::new(0.0, 1.0));
    let hp = g.ents[&b].hp;
    let (f, lvl, pos) = (g.ents[&a].faction, g.ents[&a].level.clone(), g.ents[&a].pos);
    g.area_damage(
        Some(a),
        f,
        &lvl,
        pos,
        3.0,
        &Damage::single("fire", 20.0),
        None,
    );
    assert!(g.ents[&b].hp >= hp, "area damage hit the party");
    assert!(
        !g.ents.contains_key(&m) || g.ents[&m].hp < g.ents[&m].max_hp,
        "area damage missed the wolf"
    );
    g.command(b, &cmd("party_leave", "", 0));
    assert!(!g.same_party(a, b) && g.party_of(a).is_none());
    g.pvp = false;
    assert!(!g.hostile(a, b));
}

#[test]
fn death_and_resurrection() {
    let mut g = setup();
    let a = g.join_for_test("Аня", "priest");
    let b = g.join_for_test("Боря", "warrior");
    let q = wild(&mut g, b);
    g.place_for_test(a, "overworld", q.add(Pos::new(1, 0)));
    let pos = g.ents[&b].pos;
    g.kill_for_test(b);
    assert!(g.ents[&b].dead && g.ents[&b].pos == pos);
    let snap = g.snapshot(a);
    assert!(
        snap.entities.iter().any(|e| e.id == b && e.dead),
        "the fallen hero is not visible"
    );
    g.command(b, &cmd("respawn", "", 0));
    assert!(g.ents[&b].dead, "respawned before the button works");
    g.ents
        .get_mut(&a)
        .unwrap()
        .pm()
        .subclasses
        .insert("priest".into(), "saint".into());
    g.unlock_ability(a, "resurrection");
    g.ents.get_mut(&a).unwrap().mp = 100.0;
    g.use_ability(a, "resurrection", None);
    assert!(!g.ents[&b].dead && g.ents[&b].hp > 0.0 && g.ents[&b].pos.dist(pos) <= 1.5);
    g.kill_for_test(b);
    g.now += REVIVE_WINDOW_MS + 100.0;
    assert!(!g.revive(b, a), "resurrected after the window");
}

#[test]
fn summons_and_decoys() {
    let mut g = setup();
    let p = g.join_for_test("Тест", "rogue");
    wild(&mut g, p);
    let wolf = spawn_at(&mut g, "wolf", p, Vec2::new(3.0, 0.0));
    g.ents
        .get_mut(&wolf)
        .unwrap()
        .monster
        .as_mut()
        .unwrap()
        .target = p;
    g.unlock_ability(p, "decoy");
    g.ents.get_mut(&p).unwrap().mp = 100.0;
    g.use_ability(p, "decoy", None);
    let decoy = g.ents[&wolf].monster.as_ref().unwrap().target;
    assert_ne!(decoy, p);
    assert_eq!(g.ents[&decoy].owner, p);
    assert!(g.hostile(wolf, decoy));
    g.unlock_ability(p, "summon_imp");
    g.ents.get_mut(&p).unwrap().mp = 100.0;
    g.use_ability(p, "summon_imp", None);
    let imp = g
        .ents
        .values()
        .find(|e| e.owner == p && e.monster.as_ref().is_some_and(|m| m.def == "imp_minion"))
        .map(|e| e.id)
        .expect("imp");
    assert!(g.ents[&imp].faction == Faction::Player && !g.hostile(p, imp) && g.hostile(imp, wolf));
    run(&mut g, 150);
    assert!(
        !g.ents.contains_key(&wolf) || g.ents[&wolf].hp < g.ents[&wolf].max_hp,
        "the imp did not attack the wolf"
    );
    g.now += 60000.0;
    run(&mut g, 5);
    assert!(!g.ents.contains_key(&imp), "the summon did not expire");
}

#[test]
fn hotbar_size() {
    // saves of six-cell bars keep their six cells
    let old: PlayerState =
        serde_json::from_str(r#"{"hotbar": ["a", "b", "", "", "", "f"], "level": 3}"#).unwrap();
    assert_eq!(old.hotbar.len(), 6);
    let mut g = setup();
    let p = g.join_for_test("Тест", "priest");
    assert_eq!(g.ents[&p].p().hotbar.len(), HOTBAR_DEFAULT);
    for a in ["heal", "blessing", "divine_shield"] {
        g.unlock_ability(p, a);
    }
    // a small bar: the abilities of removed cells move to empty ones
    g.command(p, &cmd("hotbar_size", "", 3));
    let bar = g.ents[&p].p().hotbar.clone();
    assert_eq!(bar.len(), 3);
    assert!(bar.iter().all(|h| !h.is_empty()), "{bar:?}");
    // a big bar: new cells take the abilities that were left off
    g.command(p, &cmd("hotbar_size", "", 12));
    let pl = g.ents[&p].p();
    assert_eq!(pl.hotbar.len(), 12);
    for a in &pl.abilities {
        assert!(pl.hotbar.contains(a), "{a} is not on the bar");
    }
    g.command(p, &cmd("hotbar", "heal", 11));
    assert_eq!(g.ents[&p].p().hotbar[11], "heal");
    g.command(p, &cmd("hotbar_size", "", 1000));
    assert_eq!(g.ents[&p].p().hotbar.len(), HOTBAR_MAX);
    g.command(p, &cmd("hotbar_size", "", 0));
    assert_eq!(g.ents[&p].p().hotbar.len(), 1);
    // the snapshot has a cooldown for every cell
    g.command(p, &cmd("hotbar_size", "", 9));
    assert_eq!(g.snapshot(p).you.cooldown.len(), 9);
}

#[test]
fn ability_fusion() {
    use crate::content::fusion::fusion_key;
    let mut g = setup();
    let p = g.join_for_test("Тест", "mage");
    let pl = g.ents[&p].p();
    assert!(can_fuse(pl, "magic_missile", "power_strike").contains("уровня"));
    g.give_xp(p, 2000);
    g.ents.get_mut(&p).unwrap().pm().level = 5;
    g.start_class(p, "warrior");
    g.unlock_ability(p, "fireball");
    let pl = g.ents[&p].p();
    assert!(pl.abilities.contains(&"power_strike".to_string()));
    assert!(can_fuse(pl, "magic_missile", "fireball").contains("разных классов"));
    assert!(!can_fuse(pl, "magic_missile", "magic_missile").is_empty());
    assert!(
        !can_fuse(pl, "magic_missile", "cleave").is_empty(),
        "not learned"
    );
    assert_eq!(can_fuse(pl, "magic_missile", "power_strike"), "");
    let slot = pl.hotbar.iter().position(|h| h == "magic_missile").unwrap();

    g.command(p, &Command::text("fuse", "power_strike"));
    assert_eq!(g.ents[&p].p().abilities.len(), 3, "no key: nothing fused");
    let mut c = cmd("fuse", "magic_missile", 0);
    c.text = "power_strike".into();
    g.command(p, &c);
    let key = fusion_key("magic_missile", "power_strike");
    let pl = g.ents[&p].p();
    assert!(pl.abilities.contains(&key), "{:?}", pl.abilities);
    assert!(!pl
        .abilities
        .iter()
        .any(|k| k == "magic_missile" || k == "power_strike"));
    assert_eq!(pl.hotbar[slot], key);
    assert!(!pl.hotbar.iter().any(|h| h == "power_strike"));
    // a slot for every five levels: the second fusion waits for level 10
    assert!(can_fuse(pl, "fireball", "power_strike").contains("слоты"));
    // learning a fused ability again does not split it
    g.unlock_ability(p, "magic_missile");
    assert!(!g.ents[&p]
        .p()
        .abilities
        .contains(&"magic_missile".to_string()));

    let def = db().ability(&key).expect("the fusion resolves").clone();
    let (mm, ps) = (
        db().ability("magic_missile").unwrap(),
        db().ability("power_strike").unwrap(),
    );
    assert_eq!(def.kind, "fusion");
    assert_eq!(def.parts.len(), 2);
    assert!(def.mana >= mm.mana + ps.mana + 4.0);
    assert!(def.cooldown_ms >= mm.cooldown_ms.max(ps.cooldown_ms));
    // the strike borrows the missile's element
    assert_eq!(def.parts[1].dmg_type, "arcane");

    wild(&mut g, p);
    let wolf = spawn_at(&mut g, "wolf", p, Vec2::new(1.1, 0.0));
    g.ents.get_mut(&wolf).unwrap().max_hp = 5000.0;
    g.ents.get_mut(&wolf).unwrap().hp = 5000.0;
    let mut hurt = false;
    for _ in 0..6 {
        let e = g.ents.get_mut(&p).unwrap();
        e.cooldowns.clear();
        e.mp = 100.0;
        e.hp = e.max_hp;
        g.use_ability(p, &key, Some(wolf));
        let e = &g.ents[&p];
        assert!(
            (e.mp - (100.0 - def.mana)).abs() < 0.01,
            "the fusion costs its mana"
        );
        if def.backlash > 0.0 {
            assert!(e.hp < e.max_hp, "no backlash");
        }
        if let Some(b) = &def.drawback {
            assert!(e.buffs.iter().any(|x| x.def.key == b.key), "no weakness");
        }
        run(&mut g, 20);
        hurt |= g.ents[&wolf].hp < 5000.0;
    }
    assert!(hurt, "the fused ability never hit");
    assert!(g.ents[&p].cooldowns[&key] > g.now);

    let left = g.ents[&p].cooldowns[&key];
    g.command(p, &cmd("unfuse", &key, 0));
    let e = &g.ents[&p];
    let pl = e.p();
    assert!(!pl.abilities.contains(&key));
    assert!(pl.abilities.contains(&"magic_missile".to_string()));
    assert!(pl.abilities.contains(&"power_strike".to_string()));
    assert_eq!(pl.hotbar[slot], "magic_missile");
    assert!(pl.hotbar.contains(&"power_strike".to_string()));
    assert!(
        e.cooldowns["power_strike"] >= left,
        "splitting skips the cooldown"
    );
}

#[test]
fn archer_keeps_distance_and_healer_heals() {
    let mut g = setup();
    let p = g.join_for_test("Тест", "warrior");
    wild(&mut g, p);
    g.tough_for_test(p);
    let archer = spawn_at(&mut g, "bandit_archer", p, Vec2::new(1.0, 0.0));
    g.ents
        .get_mut(&archer)
        .unwrap()
        .monster
        .as_mut()
        .unwrap()
        .target = p;
    run(&mut g, 45);
    if let Some(a) = g.ents.get(&archer) {
        assert!(
            a.dist(&g.ents[&p]) >= 2.0,
            "the archer stayed in melee ({})",
            a.dist(&g.ents[&p])
        );
    }
    let shaman = spawn_at(&mut g, "orc_shaman", p, Vec2::new(0.0, 5.0));
    let orc = spawn_at(&mut g, "orc", shaman, Vec2::new(1.0, 0.0));
    let e = g.ents.get_mut(&orc).unwrap();
    e.hp = e.max_hp * 0.3;
    let hp = e.hp;
    g.ents
        .get_mut(&shaman)
        .unwrap()
        .monster
        .as_mut()
        .unwrap()
        .target = p;
    run(&mut g, 60);
    assert!(
        g.ents.get(&orc).is_some_and(|o| o.hp > hp),
        "the shaman did not heal the orc"
    );
}

#[test]
fn squads_guards_and_wanderers() {
    let mut g = setup();
    let sq = &db().b.squads[0];
    let start = g.start.add(Pos::new(60, 0));
    let mut r = Rng::new(1, 1);
    let ms = g.spawn_squad(&mut r, sq, "overworld", start, 2);
    let roles: std::collections::HashSet<&str> =
        ms.iter().map(|m| ai::role_of(g.def_of(*m))).collect();
    assert!(
        roles.contains("frontline") && roles.contains("ranged"),
        "{roles:?}"
    );
    let (mut guards, mut wanderers, mut uniques) = (0, 0, 0);
    for e in g.ents.values() {
        let Some(n) = &e.npc else { continue };
        if n.role == "guard" {
            assert!(e.faction == Faction::Player && e.monster.is_some());
            guards += 1;
        } else if !n.unique.is_empty() {
            uniques += 1;
        } else if db().npc_role(&n.role).is_some_and(|r| r.world) {
            wanderers += 1;
        }
    }
    assert!(
        guards > 0 && wanderers >= 4 && uniques >= 6,
        "guards {guards} wanderers {wanderers} uniques {uniques}"
    );
}

#[test]
fn unique_quests() {
    let mut g = setup();
    let p = g.join_for_test("Тест", "warrior");
    let npc = g
        .ents
        .values()
        .find(|e| e.npc.as_ref().is_some_and(|n| n.unique == "morta"))
        .map(|e| e.id);
    let npc = match npc {
        Some(n) => n,
        None => {
            g.admin(p, "/unique morta");
            g.ents
                .values()
                .find(|e| e.npc.as_ref().is_some_and(|n| n.unique == "morta"))
                .unwrap()
                .id
        }
    };
    let u = db().unique("morta").unwrap();
    g.offer_unique_quest(p, npc, u);
    assert!(g.unique_quest(p, "morta").is_some());
    let c = g.champions["morta"];
    assert_eq!(g.ents[&c].name, u.champion);
    assert!(g.ents[&c].max_hp >= db().monster(&u.target).unwrap().hp * 3.0);
    let cc = g.ents[&c].cell();
    g.place_for_test(p, "overworld", cc);
    g.ents.get_mut(&c).unwrap().hp = 0.0;
    g.kill(c, Some(p));
    let qi = g.unique_quest(p, "morta").unwrap();
    assert!(g.ents[&p].p().quests[qi].done, "slay quest not done");
    g.finish_unique_quest(p, u);
    assert!(g.ents[&p].p().unlocked("subclass", "deathbringer"));
    let r = db().unique("ulf").unwrap();
    g.offer_unique_quest(p, npc, r);
    for _ in 0..r.count {
        g.add_item(p, ItemStack::new(&r.target));
    }
    g.update_relics(p);
    let qi = g.unique_quest(p, "ulf").unwrap();
    assert!(g.ents[&p].p().quests[qi].done);
    g.finish_unique_quest(p, r);
    assert!(g.ents[&p].p().unlocked("subclass", "sealer"));
    assert!(!g.ents[&p].p().inventory.iter().any(|s| s.key == r.target));
}

#[test]
fn mimic_and_death_sentence() {
    let mut g = setup();
    let p = g.join_for_test("Тест", "rogue");
    wild(&mut g, p);
    let mage = spawn_at(&mut g, "bandit_mage", p, Vec2::new(3.0, 0.0));
    g.apply_buff(
        mage,
        &crate::content::BuffDef {
            key: "freeze".into(),
            stun: true,
            duration_ms: 60000,
            ..Default::default()
        },
        0,
    );
    g.unlock_ability(p, "mimicry");
    g.ents.get_mut(&p).unwrap().mp = 100.0;
    g.use_ability(p, "mimicry", Some(mage));
    assert_eq!(g.ents[&p].p().copied, "m_firebolt");
    g.use_ability(p, "copied", Some(mage));
    run(&mut g, 30);
    assert!(
        g.ents.get(&mage).is_none_or(|m| m.hp < m.max_hp),
        "the copied firebolt did not hit"
    );
    g.ents.get_mut(&p).unwrap().pm().level = 10;
    let weak = spawn_at(&mut g, "wolf", p, Vec2::new(0.0, 2.0));
    g.unlock_ability(p, "death_sentence");
    g.ents.get_mut(&p).unwrap().mp = 100.0;
    g.use_ability(p, "death_sentence", None);
    assert!(
        !g.ents.contains_key(&weak),
        "a weaker creature survived the sentence"
    );
}

#[test]
fn admin_commands() {
    let mut g = setup();
    let p = g.join_for_test("Тест", "warrior");
    wild(&mut g, p);
    g.admin(p, "/god");
    let hp = g.ents[&p].hp;
    g.damage(None, p, Damage::single("fire", 1000.0));
    assert_eq!(g.ents[&p].hp, hp);
    g.admin(p, "/level 10");
    assert_eq!(g.ents[&p].p().level, 10);
    g.admin(p, "/give long sword 2 legendary");
    let swords: Vec<&ItemStack> = g.ents[&p]
        .p()
        .inventory
        .iter()
        .filter(|s| s.key == "long_sword")
        .collect();
    assert_eq!(swords.len(), 2);
    assert!(swords
        .iter()
        .all(|s| s.item_rarity() == LEGENDARY && s.bonus.len() == 4));
    g.admin(p, "/spawn wolf 3 4");
    let pos = g.ents[&p].pos;
    let wolves: Vec<i32> = g
        .ents
        .values()
        .filter(|o| o.monster.as_ref().is_some_and(|m| m.def == "wolf") && o.pos.dist(pos) < 7.0)
        .map(|o| o.monster.as_ref().unwrap().lvl)
        .collect();
    assert_eq!(wolves.len(), 3);
    assert!(wolves.iter().all(|&l| l == 4));
    g.admin(p, "/kill");
    assert!(!g
        .ents
        .values()
        .any(|o| o.monster.as_ref().is_some_and(|m| m.def == "wolf") && o.pos.dist(pos) < 7.0));
    g.admin(p, "/tp d0-1");
    assert_eq!(g.ents[&p].level, "d0-1");
    g.admin(p, "/time night");
    assert!(g.is_night());
    g.admin(p, "/unlock all");
    assert!(g.ents[&p].p().unlocks.len() > 10);
}

#[test]
fn rarity_rolls() {
    let mut g = setup();
    assert_eq!(rarity_by_name("эпич"), EPIC);
    assert_eq!(rarity_by_name("необ"), UNCOMMON);
    assert_eq!(rarity_by_name("legendary"), LEGENDARY);
    let st = g.roll_rarity(ItemStack::new("long_sword"), RARE, 5);
    assert_eq!(st.item_rarity(), RARE);
    assert_eq!(st.bonus.len(), 2);
    assert!(!st.suffix.is_empty());
    assert!(st.value() > ItemStack::new("long_sword").value());
    let mut counts = [0; 5];
    for _ in 0..2000 {
        counts[g.roll_rarity_tier(10, 100.0, COMMON) as usize] += 1;
    }
    assert!(
        counts[UNCOMMON as usize] > counts[LEGENDARY as usize] && counts[LEGENDARY as usize] > 0,
        "{counts:?}"
    );
}

#[test]
fn city_life() {
    let mut g = setup();
    let city = g.villages.iter().find(|v| v.city).expect("a city").clone();
    let p = g.join_for_test("Тест", "warrior");
    g.place_for_test(p, "overworld", Pos::new(city.center.x, city.center.y + 2));
    g.ents.get_mut(&p).unwrap().pm().gold = 5000;
    let trader = g
        .ents
        .values()
        .find(|e| {
            e.npc.as_ref().is_some_and(|n| {
                n.village == city.name && db().npc_role(&n.role).is_some_and(|r| r.stock > 0)
            })
        })
        .map(|e| e.id)
        .expect("a city trader");
    g.move_next_to(p, trader);
    g.open_dialogue(p, trader);
    let list = g.trade_list(trader);
    let role = db()
        .npc_role(&g.ents[&trader].npc.as_ref().unwrap().role)
        .unwrap();
    assert!(list.len() > role.goods.len(), "no daily stock");
    let idx = role.goods.len();
    let key = list[idx].item.key.clone();
    let n = g.ents[&p].p().inventory.len();
    g.buy(p, &key, idx as i32);
    assert_eq!(g.ents[&p].p().inventory.len(), n + 1);
    // townsfolk walk to the tavern at night
    g.admin(p, "/time night");
    let citizens: Vec<Id> = g
        .ents
        .values()
        .filter(|e| e.npc.as_ref().is_some_and(|n| n.night.is_some()))
        .map(|e| e.id)
        .collect();
    assert!(!citizens.is_empty());
    let dist = |g: &Game| {
        citizens
            .iter()
            .map(|c| g.ents[c].pos.dist(g.schedule_spot(*c)))
            .sum::<f32>()
    };
    let before = dist(&g);
    run(&mut g, 600);
    assert!(dist(&g) < before, "townsfolk did not go to the tavern");
}

#[test]
fn deeds_open_hidden_skills() {
    let mut g = setup();
    let p = g.join_for_test("Тест", "warrior");
    let sd = db()
        .b
        .skills
        .iter()
        .find(|s| s.deed == "kills" && db().branch(&s.branch).is_some_and(|b| b.class == "warrior"))
        .expect("a kills deed");
    assert!(!can_learn(g.ents[&p].p(), Some(sd)).is_empty());
    g.deed(p, "kills", sd.deed_count);
    g.check_deeds(p);
    assert_eq!(g.ents[&p].p().skill(&sd.key), sd.max_rank);
    for s in &db().b.skills {
        if !s.deed.is_empty() {
            assert!(deed_known(&s.deed), "unknown deed {} in {}", s.deed, s.key);
        }
    }
}

/// Plays random inputs for many ticks with several players to catch panics
/// in long sessions (day and night, the spawner, dungeons).
#[test]
fn soak() {
    let mut g = setup();
    let mut players = Vec::new();
    for (i, cls) in ["warrior", "ranger", "mage"].iter().enumerate() {
        let p = g.join_for_test(&format!("Герой{i}"), cls);
        g.give_xp(p, 5000);
        for s in &db().b.skills {
            for _ in 0..3 {
                g.command(p, &cmd("learn", &s.key, 0));
            }
        }
        players.push(p);
    }
    let mut r = Rng::new(3, 3);
    let ticks = if cfg!(debug_assertions) { 4000 } else { 12000 };
    for tick in 0..ticks {
        for (i, &p) in players.iter().enumerate() {
            match r.int_n(12) {
                0..=5 => g.set_input(p, &input([(r.int_n(3) - 1) as i8, (r.int_n(3) - 1) as i8])),
                6 => {
                    let pos = g.ents[&p].pos;
                    g.set_input(
                        p,
                        &Input {
                            ability: (1 + r.int_n(6)) as i8,
                            aim: Some([pos.x + r.f32() * 8.0 - 4.0, pos.y + r.f32() * 8.0 - 4.0]),
                            ..Default::default()
                        },
                    )
                }
                7 => g.set_input(
                    p,
                    &Input {
                        attack: true,
                        ..Default::default()
                    },
                ),
                8 => g.set_input(
                    p,
                    &Input {
                        interact: true,
                        ..Default::default()
                    },
                ),
                9 => {
                    let n = g.ents[&p].p().inventory.len() as i32;
                    if n > 0 {
                        g.command(p, &cmd("use", "", r.int_n(n)));
                    }
                }
                _ => {}
            }
            g.ents.get_mut(&p).unwrap().mp = 1000.0;
            if tick % 1500 == 700 + i * 100 && !g.ents[&p].dead {
                let idx = r.int_n(g.entrances.len() as i32);
                let depth = 1 + r.int_n(g.entrances[idx as usize].max_depth);
                g.teleport_for_test(p, &dungeon_level_id(idx, depth));
            }
        }
        g.tick();
        for &p in &players {
            g.snapshot(p);
            g.sheet(p);
            g.take_outbox(p);
        }
        g.end_frame();
        if tick % 500 == 0 {
            // nobody is stuck inside a wall
            for e in g.ents.values().filter(|e| e.blocks() && e.alive()) {
                let l = &g.levels[&e.level];
                assert!(
                    l.walkable(e.cell().x, e.cell().y) || l.def_at(e.cell()).interact == "door",
                    "{} inside a wall at {:?} ({})",
                    e.name,
                    e.pos,
                    l.def_at(e.cell()).key
                );
            }
        }
    }
    let path = std::env::temp_dir().join(format!("ratas-soak-{}.sav", std::process::id()));
    g.save(&path).unwrap();
    std::fs::remove_file(path).ok();
}

/// A fake Messages API that answers every request with one JSON reply.
fn fake_api(reply: serde_json::Value) -> String {
    use std::io::{BufRead, BufReader, Read, Write};
    let ln = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("http://{}", ln.local_addr().unwrap());
    std::thread::spawn(move || {
        for s in ln.incoming() {
            let Ok(mut s) = s else { continue };
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut len = 0;
            loop {
                let mut line = String::new();
                if r.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some(v) = line.to_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0; len];
            r.read_exact(&mut body).ok();
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            assert_eq!(req["output_config"]["format"]["type"], "json_schema");
            let resp = serde_json::json!({
                "id": "msg_1", "type": "message", "role": "assistant", "model": "claude-opus-5-5",
                "content": [{"type": "text", "text": reply.to_string()}], "stop_reason": "end_turn",
                "usage": {"input_tokens": 1, "output_tokens": 1}
            })
            .to_string();
            let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", resp.len(), resp);
        }
    });
    addr
}

#[test]
fn npc_dialogue_with_claude() {
    let mut g = setup();
    let base = fake_api(
        serde_json::json!({"say": "Волки совсем обнаглели. Помоги нам!", "action": "offer_quest", "item": "", "gold": 0, "quest_monster": "wolf", "quest_count": 3}),
    );
    g.brain = crate::llm::Brain::with_base("sk-test", "", &base);
    let p = g.join_for_test("Герой", "warrior");
    let elder = g
        .ents
        .values()
        .find(|e| e.npc.as_ref().is_some_and(|n| n.role == "elder"))
        .map(|e| e.id)
        .expect("an elder");
    g.move_next_to(p, elder);
    g.open_dialogue(p, elder);
    let d = g.take_outbox(p).and_then(|o| o.dialogue).expect("dialogue");
    assert!(d.ai);
    g.command(p, &Command::text("talk", "Есть работа?"));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut last = None;
    while g.ents[&p].p().quests.is_empty() && std::time::Instant::now() < deadline {
        g.tick();
        if let Some(d) = g.take_outbox(p).and_then(|o| o.dialogue) {
            last = Some(d);
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let q = &g.ents[&p].p().quests;
    assert_eq!(q.len(), 1, "quest from Claude not created");
    assert_eq!((q[0].monster.as_str(), q[0].need), ("wolf", 3));
    if let Some(d) = g.take_outbox(p).and_then(|o| o.dialogue) {
        last = Some(d);
    }
    assert!(
        last.is_some_and(|d| d.text.contains("Помоги нам")),
        "reply not delivered"
    );
    assert_eq!(
        g.ents[&elder].npc.as_ref().unwrap().memory["Герой"].len(),
        2
    );
}

/// A fake Ollama server: the model is missing at first and gets pulled;
/// chats answer by the schema they ask for (an NPC or the game master).
/// Returns the address and the paths requested.
fn fake_ollama(
    npc: serde_json::Value,
    gm: serde_json::Value,
    lore: serde_json::Value,
) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    use std::io::{BufRead, BufReader, Read, Write};
    let ln = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("http://{}", ln.local_addr().unwrap());
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        let mut pulled = false;
        for s in ln.incoming() {
            let Ok(mut s) = s else { continue };
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut first = String::new();
            r.read_line(&mut first).ok();
            let path = first.split(' ').nth(1).unwrap_or("").to_string();
            let mut len = 0;
            loop {
                let mut line = String::new();
                if r.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some(v) = line.to_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0; len];
            r.read_exact(&mut body).ok();
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            log.lock().unwrap().push(path.clone());
            let (code, resp) = match path.as_str() {
                "/api/version" => (200, r#"{"version":"0.18.1"}"#.to_string()),
                "/api/show" if !pulled => (404, r#"{"error":"model not found"}"#.to_string()),
                "/api/show" => (200, "{}".to_string()),
                "/api/pull" => {
                    assert_eq!(req["model"], "mistral-test");
                    pulled = true;
                    let lines = [
                        r#"{"status":"pulling manifest"}"#,
                        r#"{"status":"pulling 1","digest":"sha256:1","total":1000,"completed":500}"#,
                        r#"{"status":"pulling 1","digest":"sha256:1","total":1000,"completed":1000}"#,
                        r#"{"status":"success"}"#,
                    ];
                    (200, lines.join("\n") + "\n")
                }
                "/api/generate" => (200, r#"{"done":true,"done_reason":"load"}"#.into()),
                "/api/chat" => {
                    // constrained decoding: the schema goes as "format"
                    assert_eq!(req["format"]["type"], "object");
                    assert_eq!(req["stream"], false);
                    let content = req["messages"][0]["content"].as_str().unwrap_or("");
                    let props = &req["format"]["properties"];
                    let reply = if props["commands"].is_object() {
                        assert!(content.contains("game master"));
                        &gm
                    } else if props["history"].is_object() {
                        // the one long answer
                        assert_eq!(req["options"]["num_predict"], 4096);
                        assert!(content.contains("chronicler"));
                        &lore
                    } else {
                        &npc
                    };
                    let v = serde_json::json!({
                        "model": "mistral-test", "done": true, "done_reason": "stop",
                        "message": {"role": "assistant", "content": reply.to_string()}
                    });
                    (200, v.to_string())
                }
                _ => (404, "{}".into()),
            };
            let _ = write!(s, "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", resp.len(), resp);
        }
    });
    (addr, seen)
}

fn wolves_near(g: &Game, p: Id) -> usize {
    let at = g.ents[&p].pos;
    g.ents
        .values()
        .filter(|e| {
            e.monster.as_ref().is_some_and(|m| m.def == "wolf")
                && e.level == g.ents[&p].level
                && e.pos.dist(at) < 16.0
        })
        .count()
}

fn logs_of(g: &mut Game, p: Id) -> Vec<String> {
    g.take_outbox(p)
        .map(|o| o.logs.into_iter().map(|l| l.text).collect())
        .unwrap_or_default()
}

/// The local model: pulled into Ollama, warmed up, then NPCs talk and the
/// game master carries out an admin's wish through it.
#[test]
fn local_model_talks_and_masters() {
    use crate::llm::local;
    let (base, seen) = fake_ollama(
        serde_json::json!({"say": "Волки совсем обнаглели!", "action": "offer_quest", "item": "", "gold": 0, "quest_monster": "wolf", "quest_count": 3}),
        serde_json::json!({"announce": "Из чащи доносится вой.", "commands": [
            {"action": "spawn_monsters", "player": "Герой", "key": "wolf", "amount": 3, "text": ""},
            {"action": "give_gold", "player": "герой", "key": "", "amount": 100, "text": ""},
            {"action": "rumor", "player": "", "key": "", "amount": 0, "text": "В лесах снова видели волков"}
        ]}),
        serde_json::Value::Null,
    );
    let rt = local::spawn(local::Settings::new("ollama", "mistral-test", &base, ""));
    let mut g = setup();
    g.brain = Some(crate::llm::Brain::local(rt.clone()));
    let p = g.join_for_test("Герой", "warrior");
    assert!(!g.brain.as_ref().unwrap().enabled() || rt.ready());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !rt.ready() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(rt.state(), local::State::Ready);
    let paths = seen.lock().unwrap().clone();
    assert!(
        paths.contains(&"/api/pull".to_string()),
        "model not pulled: {paths:?}"
    );
    assert!(
        paths.contains(&"/api/generate".to_string()),
        "not warmed up"
    );
    // the players hear that the model is ready
    g.director_tick();
    assert!(logs_of(&mut g, p).iter().any(|l| l.contains("готова")));
    // an NPC answers through the local model
    let elder = g
        .ents
        .values()
        .find(|e| e.npc.as_ref().is_some_and(|n| n.role == "elder"))
        .map(|e| e.id)
        .expect("an elder");
    g.move_next_to(p, elder);
    g.open_dialogue(p, elder);
    g.command(p, &Command::text("talk", "Есть работа?"));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while g.ents[&p].p().quests.is_empty() && std::time::Instant::now() < deadline {
        g.tick();
        g.take_outbox(p);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(g.ents[&p].p().quests.len(), 1, "quest from the local model");
    // the game master carries out an admin's wish
    wild(&mut g, p);
    let gold = g.ents[&p].p().gold;
    g.admin(p, "/gm устрой засаду волков");
    let mut logs = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while g.ents[&p].p().gold == gold && std::time::Instant::now() < deadline {
        g.tick();
        logs.extend(logs_of(&mut g, p));
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    logs.extend(logs_of(&mut g, p));
    assert_eq!(g.ents[&p].p().gold, gold + 100);
    assert_eq!(wolves_near(&g, p), 3);
    assert!(g.chronicle.iter().any(|c| c.contains("видели волков")));
    assert!(logs.iter().any(|l| l.contains("вой")), "announce: {logs:?}");
    assert!(logs.iter().any(|l| l.starts_with("ИИ-мастер:")));
    assert!(g.take_server_log().iter().any(|l| l.contains("Волк ×3")));
    g.admin(p, "/ai");
    let logs = logs_of(&mut g, p);
    assert!(logs
        .iter()
        .any(|l| l.contains("Mistral") || l.contains("mistral-test")));
}

fn gm_cmd(action: &str, key: &str, amount: i32) -> crate::llm::GmCommand {
    crate::llm::GmCommand {
        action: action.into(),
        key: key.into(),
        amount,
        ..Default::default()
    }
}

/// Acting on its own, the master stays within limits: no bosses, no
/// legendary visitors, small rewards, no ambushes in villages.
#[test]
fn game_master_limits() {
    use crate::llm::GmReply;
    let mut g = setup();
    let p = g.join_for_test("Герой", "warrior");
    wild(&mut g, p);
    let gold = g.ents[&p].p().gold;
    let boss = db().b.monsters.iter().find(|m| m.boss).unwrap().key.clone();
    let unique = db().b.uniques[0].key.clone();
    let n = g.ents.len();
    g.apply_master(
        None,
        Ok(GmReply {
            announce: String::new(),
            commands: vec![
                gm_cmd("give_gold", "", 99_999),
                gm_cmd("spawn_monsters", &boss, 1),
                gm_cmd("summon_unique", &unique, 0),
                gm_cmd("spawn_monsters", "wolf", 2), // past the limit of a reply
            ],
        }),
    );
    let lvl = g.ents[&p].p().level;
    assert_eq!(g.ents[&p].p().gold, gold + 30 * lvl, "a modest reward");
    assert_eq!(g.ents.len(), n, "no boss, no unique, no fourth command");
    g.apply_master(
        None,
        Ok(GmReply {
            announce: String::new(),
            commands: vec![
                gm_cmd("spawn_monsters", "wolf", 50),
                gm_cmd("nuke", "", 0),
                gm_cmd("set_time", "night", 0),
            ],
        }),
    );
    assert_eq!(wolves_near(&g, p), 5, "a band is at most five");
    assert!(g.is_night());
    // heroes in a village are safe from its ambushes
    let v = g.villages[0].center;
    g.place_for_test(p, "overworld", v);
    g.index_levels();
    let n = g.ents.len();
    g.apply_master(
        None,
        Ok(GmReply {
            announce: String::new(),
            commands: vec![gm_cmd("spawn_monsters", "wolf", 3)],
        }),
    );
    assert_eq!(g.ents.len(), n);
    // an admin may summon a legendary character
    wild(&mut g, p);
    g.apply_master(
        Some(p),
        Ok(GmReply {
            announce: String::new(),
            commands: vec![gm_cmd("summon_unique", &unique, 0)],
        }),
    );
    // (it may have lived in this world already: then it is not doubled)
    let copies = g
        .ents
        .values()
        .filter(|e| e.npc.as_ref().is_some_and(|n| n.unique == unique))
        .count();
    assert_eq!(copies, 1);
}

/// On its own the master looks at the world now and then (Claude here).
#[test]
fn game_master_acts_on_its_own() {
    let mut g = setup();
    let base = fake_api(serde_json::json!({"announce": "", "commands": [
        {"action": "rumor", "player": "", "key": "", "amount": 0, "text": "Над холмами кружат вороны"}
    ]}));
    g.brain = crate::llm::Brain::with_base("sk-test", "", &base);
    let p = g.join_for_test("Герой", "warrior");
    wild(&mut g, p);
    g.director_tick();
    assert!(g.chronicle.is_empty(), "the master is off by default");
    g.set_director(true);
    g.director_tick();
    assert!(g.gm.next_at > Some(g.now), "its first look waits a minute");
    g.gm.next_at = Some(g.now);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !g.chronicle.iter().any(|c| c.contains("вороны")) && std::time::Instant::now() < deadline
    {
        g.director_tick();
        g.tick();
        g.take_outbox(p);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(g.chronicle.iter().any(|c| c.contains("вороны")));
    assert!(g.gm.next_at > Some(g.now + 60_000.0));
}

/// With Claude at hand a new world gets its own story: characters of the
/// story live in it, their quests lead to its artifacts, and all of it
/// survives a save.
#[test]
fn world_story_from_claude() {
    use std::sync::atomic::AtomicBool;
    let mut g = setup();
    let req = g.lore_request();
    let reply = serde_json::to_value(super::lore::tests::sample(&req)).unwrap();
    let base = fake_api(reply);
    g.brain = crate::llm::Brain::with_base("sk-test", "", &base);
    let stages = std::sync::Mutex::new(Vec::<String>::new());
    let title = g
        .write_lore(&AtomicBool::new(false), &|s| {
            stages.lock().unwrap().push(s.to_string())
        })
        .unwrap();
    assert_eq!(title, "Летопись Пепельной Короны");
    assert!(stages.lock().unwrap().iter().any(|s| s.contains("пишет")));
    let lore = g.lore.clone().unwrap();
    let key = format!("{LORE_PREFIX}{}", g.seed.unsigned_abs());
    // the characters of the story live in the world, not far from the start
    let people: Vec<Id> = g
        .ents
        .values()
        .filter(|e| e.npc.as_ref().is_some_and(|n| n.unique.starts_with(&key)))
        .map(|e| e.id)
        .collect();
    assert_eq!(people.len(), 3);
    for &id in &people {
        assert!(g.ents[&id].cell().dist(g.start) < 400);
    }
    // their content is the game's content now, with English
    let relic_giver = lore
        .content
        .uniques
        .iter()
        .find(|u| u.quest == "relics")
        .unwrap()
        .clone();
    assert!(db().unique(&relic_giver.key).is_some());
    assert_eq!(crate::i18n::tr_in("en", &relic_giver.name), "Name");
    // NPCs know the history
    let elder = g
        .ents
        .values()
        .find(|e| e.npc.as_ref().is_some_and(|n| n.role == "elder"))
        .map(|e| e.id)
        .unwrap();
    assert!(g
        .world_facts(elder)
        .iter()
        .any(|f| f.contains("Пепельной Короны")));
    // a hero sees the chronicle once
    let p = g.join_for_test("Герой", "warrior");
    assert!(g.lore_first_look(p));
    assert!(!g.lore_first_look(p));
    // the quest of the story leads to its artifact
    let npc = *people
        .iter()
        .find(|&&id| g.ents[&id].npc.as_ref().unwrap().unique == relic_giver.key)
        .unwrap();
    g.offer_unique_quest(p, npc, &relic_giver);
    for _ in 0..5 {
        g.give_for_test(p, &relic_giver.target);
    }
    g.finish_unique_quest(p, &relic_giver);
    let art = relic_giver.reward.strip_prefix("item:").unwrap();
    assert!(
        g.ents[&p].p().inventory.iter().any(|st| st.key == art),
        "the artifact of the story"
    );
    // the story and its people survive a save
    let dir = std::env::temp_dir().join(format!("ratas-lore-{}", std::process::id()));
    let path = dir.join("lore.sav");
    g.save(&path).unwrap();
    let l = Game::load(&path, None).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(l.lore.as_ref().unwrap().title, title);
    assert_eq!(
        l.ents
            .values()
            .filter(|e| e.npc.as_ref().is_some_and(|n| n.unique.starts_with(&key)))
            .count(),
        3
    );
    assert!(db().item(art).is_some());
}

/// The local model writes the story too, as its one long answer.
#[test]
fn world_story_from_local_model() {
    use crate::llm::local;
    use std::sync::atomic::AtomicBool;
    let mut g = setup();
    let req = g.lore_request();
    let lore = serde_json::to_value(super::lore::tests::sample(&req)).unwrap();
    let (base, _) = fake_ollama(serde_json::Value::Null, serde_json::Value::Null, lore);
    let rt = local::spawn(local::Settings::new("ollama", "mistral-test", &base, ""));
    g.brain = Some(crate::llm::Brain::local(rt));
    // it waits for the model to load first
    let title = g.write_lore(&AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!(title, "Летопись Пепельной Короны");
    assert_eq!(g.lore.as_ref().unwrap().characters().len(), 3);
}

/// No story without a model, and none while a model is still downloading.
#[test]
fn no_story_without_a_model() {
    use std::sync::atomic::AtomicBool;
    let mut g = setup();
    assert!(g.write_lore(&AtomicBool::new(false), &|_| {}).is_err());
    assert!(g.lore.is_none());
    // a skipped story leaves the world as it is
    let base = fake_api(serde_json::json!({}));
    g.brain = crate::llm::Brain::with_base("sk-test", "", &base);
    assert!(g.write_lore(&AtomicBool::new(true), &|_| {}).is_err());
    assert!(g.lore.is_none());
}

/// Creatures far from every player sleep: they are not simulated and AI
/// queries do not see them, yet cell lookups still do.
#[test]
fn far_creatures_sleep() {
    let mut g = setup();
    let p = g.join_for_test("Герой", "warrior");
    g.tough_for_test(p);
    let at = wild(&mut g, p);
    let near = spawn_at(&mut g, "wolf", p, Vec2::new(6.0, 0.0));
    // the farthest open land from the hero, beyond the awake range
    let l = &g.levels["overworld"];
    let mut far_cell = None;
    'search: for d in [140, 120, 100] {
        for (dx, dy) in [
            (d, 0),
            (-d, 0),
            (0, d),
            (0, -d),
            (d, d),
            (-d, -d),
            (d, -d),
            (-d, d),
        ] {
            let q = Pos::new(at.x + dx, at.y + dy);
            if l.walkable(q.x, q.y) && l.def_at(q).interact.is_empty() && !g.in_village(q, 6) {
                far_cell = Some(q);
                break 'search;
            }
        }
    }
    let far_cell = far_cell.expect("far land");
    let far = g.spawn_for_test("wolf", "overworld", far_cell, 1).unwrap();
    // (the spawner would clear away an ordinary wolf left far behind)
    g.ents
        .get_mut(&far)
        .unwrap()
        .monster
        .as_mut()
        .unwrap()
        .persistent = true;
    let far_pos = g.ents[&far].pos;
    run(&mut g, 60);
    let awake = g.awake_ids();
    assert!(awake.contains(&p) && awake.contains(&near));
    assert!(!awake.contains(&far), "a far wolf is simulated");
    assert!(!g.on_level("overworld").contains(&far));
    assert_eq!(g.ents[&far].pos, far_pos, "a sleeping wolf moved");
    assert!(
        g.cell_taken("overworld", far_pos.cell()),
        "cell lookups see sleepers"
    );
    // walking up wakes it
    g.place_for_test(p, "overworld", far_cell.add(Pos::new(0, 8)));
    run(&mut g, 2);
    assert!(g.awake_ids().contains(&far));
}

#[test]
fn danger_grows_slower_far_away() {
    use super::spawn::ring_level;
    // the classic world as before: a level per 70 steps
    assert_eq!(ring_level(0), 1);
    assert_eq!(ring_level(69), 1);
    assert_eq!(ring_level(70), 2);
    assert_eq!(ring_level(350), 6);
    // beyond it a level per 200 steps, so the far ends of a big world stay
    // within reach
    assert_eq!(ring_level(550), 7);
    assert!(ring_level(2500) < 20);
    let mut last = 0;
    for d in (0..4000).step_by(10) {
        assert!(ring_level(d) >= last);
        last = ring_level(d);
    }
}

/// The full-size world: how long it takes to make, save and load, and how
/// long a tick takes next to a player in a busy city
/// (`cargo test --release -p ratas-core big_world -- --ignored --nocapture`).
#[test]
#[ignore]
fn big_world() {
    use std::time::Instant;
    let t0 = Instant::now();
    let mut g = Game::new(7, None);
    let made = t0.elapsed();
    let mut kinds = std::collections::BTreeMap::new();
    for e in g.ents.values() {
        *kinds.entry(format!("{:?}", e.kind)).or_insert(0) += 1;
    }
    let ow = &g.levels["overworld"];
    println!(
        "made {made:?}: {}×{}, {} settlements ({} cities), {} dungeons, {} sights, {} regions, entities {kinds:?}",
        ow.w,
        ow.h,
        g.villages.len(),
        g.villages.iter().filter(|v| v.city).count(),
        g.entrances.len(),
        g.landmarks.len(),
        g.regions.len()
    );
    let p = g.join_for_test("Герой", "warrior");
    g.tough_for_test(p);
    let city = g.villages.iter().find(|v| v.city).unwrap().center;
    g.place_for_test(p, "overworld", Pos::new(city.x, city.y + 2));
    run(&mut g, 30);
    let t1 = Instant::now();
    run(&mut g, 300);
    let per = t1.elapsed() / 300;
    println!(
        "tick in a city: {per:?}, awake {} of {}",
        g.awake_ids().len(),
        g.ents.len()
    );
    let w = wild(&mut g, p);
    run(&mut g, 30);
    let t1 = Instant::now();
    run(&mut g, 300);
    println!("tick in the wild at {w:?}: {:?}", t1.elapsed() / 300);
    let t1 = Instant::now();
    let ld = g.level_data(p);
    let first = t1.elapsed();
    let t1 = Instant::now();
    g.level_data(p);
    println!(
        "level data {first:?}, again {:?}: tiles {} KB, explored {} KB",
        t1.elapsed(),
        ld.tiles.len() / 1024,
        ld.explored.len() / 1024
    );
    let t1 = Instant::now();
    let sh = g.sheet(p);
    println!("sheet {:?}: {} places", t1.elapsed(), sh.places.len());
    let dir = std::env::temp_dir().join(format!("ratas-big-{}", std::process::id()));
    let path = dir.join("big.sav");
    let t1 = Instant::now();
    g.save(&path).unwrap();
    let saved = t1.elapsed();
    let size = std::fs::metadata(&path).unwrap().len();
    let t1 = Instant::now();
    let listed = list_saves(&dir);
    println!("list of saves {:?}: {}", t1.elapsed(), listed.len());
    let t1 = Instant::now();
    let g2 = Game::load(&path, None).unwrap();
    println!(
        "save {saved:?} ({} KB), load {:?}, {} entities",
        size / 1024,
        t1.elapsed(),
        g2.ents.len()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
