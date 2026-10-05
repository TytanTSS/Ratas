//! Procedural generation: Perlin-noise overworld with biomes, rivers, villages,
//! walled cities and roads; BSP crypts and cellular-automata caves. All
//! generation is deterministic for a given seed.

mod city;
mod dungeon;
mod names;
mod noise;
mod overworld;
mod regions;

pub use city::*;
pub use dungeon::*;
pub use names::*;
pub use noise::*;
pub use overworld::*;
pub use regions::*;

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
