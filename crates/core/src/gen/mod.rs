//! Procedural generation: Perlin-noise overworld with biomes, rivers, villages,
//! walled cities and roads; BSP crypts and cellular-automata caves. All
//! generation is deterministic for a given seed.

mod city;
mod dungeon;
mod names;
mod noise;
mod overworld;
mod regions;
mod spatial;
mod zones;

pub use city::*;
pub use dungeon::*;
pub use names::*;
pub use noise::*;
pub use overworld::*;
pub use regions::*;
pub use spatial::*;
pub use zones::*;

pub(crate) use crate::rng::Rng;
pub(crate) use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::db;

    #[test]
    fn overworld_is_deterministic_and_connected() {
        let a = generate_overworld(7, 300, 200);
        let b = generate_overworld(7, 300, 200);
        assert_eq!(a.level.tiles, b.level.tiles);
        assert!(a.villages.len() >= 3, "villages: {}", a.villages.len());
        assert!(a.villages.iter().any(|v| v.city), "a city");
        assert!(a.entrances.len() >= 4, "entrances: {}", a.entrances.len());
        assert!(!a.regions.is_empty());
        assert!(!a.landmarks.is_empty());
        assert!(a.level.walkable(a.start.x, a.start.y));
        // every village is reachable from the start
        let dist = bfs(&a.level, a.start);
        for v in &a.villages {
            let near = find_free(&a.level, v.center);
            assert!(
                dist[a.level.idx(near)] >= 0,
                "village {} unreachable",
                v.name
            );
        }
        for e in &a.entrances {
            assert!(
                dist[a.level.idx(e.pos)] >= 0,
                "entrance {} unreachable",
                e.name
            );
        }
    }

    #[test]
    fn dungeons_have_stairs_and_bosses() {
        for theme in themes() {
            let f = generate_dungeon(3, &format!("d0-1-{theme}"), "Тест", theme, 0, 1, 2);
            let l = &f.level;
            assert_eq!(db().tile(l.at(l.up.x, l.up.y)).key, "stairs_up", "{theme}");
            let dist = bfs(l, l.up);
            assert!(dist[l.idx(l.down)] >= 0, "{theme}: stairs down unreachable");
            assert!(!f.monsters.is_empty());
            let last = generate_dungeon(3, &format!("d0-2-{theme}"), "Тест", theme, 0, 2, 2);
            let b = last.boss.expect("boss");
            assert!(
                bfs(&last.level, last.level.up)[last.level.idx(b)] >= 0,
                "{theme}: boss unreachable"
            );
        }
    }

    #[test]
    fn cities_have_gates_and_people() {
        let mut found = 0;
        for seed in 1..6 {
            let ow = generate_overworld(seed, 300, 200);
            for v in ow.villages.iter().filter(|v| v.city) {
                found += 1;
                assert!(
                    v.npcs.len() >= 10,
                    "{} has {} citizens",
                    v.name,
                    v.npcs.len()
                );
                let dist = bfs(&ow.level, ow.start);
                let gate = Pos::new(v.center.x, v.area.y);
                assert_eq!(ow.level.def(gate.x, gate.y).key, "city_gate");
                for n in &v.npcs {
                    assert!(
                        dist[ow.level.idx(n.pos)] >= 0
                            || ow.level.def(n.pos.x, n.pos.y).key == "door",
                        "{} at {:?} unreachable",
                        n.role,
                        n.pos
                    );
                }
            }
        }
        assert!(found >= 3, "cities: {found}");
    }
    use crate::world::Pos;
}

#[cfg(test)]
mod scale_tests {
    use super::*;
    use crate::world::Pos;
    use std::collections::HashSet;

    /// A world nine times the classic area keeps the classic density of
    /// settlements, dungeons and sights, and every settlement has its own name.
    #[test]
    fn bigger_maps_keep_the_classic_density() {
        let ow = generate_overworld(11, 900, 600);
        let land = largest_region(&ow.level).iter().filter(|&&b| b).count();
        let k = land as f64 / BASE_LAND;
        assert!(k > 6.0, "land {land}");
        let villages = ow.villages.iter().filter(|v| !v.city).count() as f64;
        let cities = ow.villages.iter().filter(|v| v.city).count() as f64;
        assert!(
            villages >= 3.0 * k,
            "villages {villages} for {k:.1} worlds of land"
        );
        assert!(cities >= 0.5 * k, "cities {cities}");
        assert!(
            ow.entrances.len() as f64 >= 5.0 * k,
            "dungeons {}",
            ow.entrances.len()
        );
        assert!(
            ow.landmarks.len() as f64 >= 12.0 * k,
            "sights {}",
            ow.landmarks.len()
        );
        let names: HashSet<&str> = ow.villages.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names.len(), ow.villages.len(), "settlement names repeat");
        // the journey starts near the heart of the world
        let c = Pos::new(450, 300);
        assert!(ow.start.dist(c) < 200, "start {:?}", ow.start);
        // regions are labelled on their own land
        for (i, r) in ow.regions.iter().enumerate() {
            assert_eq!(
                ow.region_map[ow.level.idx(r.at)] as usize,
                i + 1,
                "{}",
                r.name
            );
        }
        let dist = bfs(&ow.level, ow.start);
        for v in &ow.villages {
            let near = find_free(&ow.level, v.center);
            assert!(dist[ow.level.idx(near)] >= 0, "{} unreachable", v.name);
        }
    }

    #[test]
    fn generated_names_do_not_repeat() {
        let mut r = Rng::new(3, 4);
        let mut used = HashSet::new();
        for _ in 0..500 {
            let n = village_name(&mut r, &mut used);
            assert!(!n.is_empty());
        }
        for _ in 0..150 {
            city_name(&mut r, &mut used);
        }
        assert_eq!(used.len(), 650);
        // namesakes of places are told apart by a settlement nearby
        let mut used = HashSet::new();
        let list = ["Пещера Эха"];
        assert_eq!(
            place_name(&mut r, &list, "Тихий Брод", &mut used),
            "Пещера Эха"
        );
        assert_eq!(
            place_name(&mut r, &list, "Тихий Брод", &mut used),
            "Пещера Эха (Тихий Брод)"
        );
    }
}

#[cfg(test)]
mod calib {
    use super::*;
    use crate::content::db;

    /// Prints what worlds of a size hold
    /// (`CALIB_SIZE=3000x2000 cargo test --release -p ratas-core world_stats -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn world_stats() {
        let (w, h): (i32, i32) = std::env::var("CALIB_SIZE")
            .ok()
            .and_then(|s| {
                let (a, b) = s.split_once('x')?;
                Some((a.parse().ok()?, b.parse().ok()?))
            })
            .unwrap_or((300, 200));
        for seed in 1..4 {
            let t0 = std::time::Instant::now();
            let ow = generate_overworld(seed, w, h);
            let dt = t0.elapsed();
            let main = largest_region(&ow.level);
            let land = main.iter().filter(|&&b| b).count();
            let mut biomes = std::collections::BTreeMap::new();
            for &t in &ow.level.tiles {
                *biomes.entry(db().tile(t).biome.clone()).or_insert(0) += 1;
            }
            let mut themes = std::collections::BTreeMap::new();
            for e in &ow.entrances {
                *themes.entry(e.theme.clone()).or_insert(0) += 1;
            }
            println!(
                "seed {seed} {dt:?}: land {land} villages {} cities {} entr {:?} regions {} landmarks {} start {:?}\n  biomes {:?}",
                ow.villages.iter().filter(|v| !v.city).count(),
                ow.villages.iter().filter(|v| v.city).count(),
                themes,
                ow.regions.len(),
                ow.landmarks.len(),
                ow.start,
                biomes
            );
        }
    }
}
