//! Danger zones: belts of land from the quiet cradle around the start,
//! where newcomers meet wolves and bandits, to the deadly edges of the
//! world, whose creatures and strongholds only a party of seasoned heroes
//! overcomes.
//!
//! The level of the land grows with the distance from the start, measured
//! in the shape of the island (half the map's width and height count the
//! same), so every map size has the same belts. Slow noise makes the borders
//! ragged; special lands (swamps, deserts, tundra, ash, cursed lands) are
//! more dangerous than the belt they lie in.

use super::*;
use crate::world::{Level, Pos};

/// A belt of danger.
pub struct Tier {
    pub name: &'static str,
    /// monster levels of the land: from, to
    pub levels: [i32; 2],
    /// how many heroes it takes
    pub party: i32,
    /// shown on the map and in the HUD
    pub color: &'static str,
    /// what the heroes are told when they come in
    pub hint: &'static str,
}

pub const TIERS: [Tier; 5] = [
    Tier {
        name: "Мирный край",
        levels: [1, 5],
        party: 1,
        color: "#8ad07a",
        hint: "Земли для новичков: слабые звери и разбойники, элитных врагов нет.",
    },
    Tier {
        name: "Пограничье",
        levels: [6, 12],
        party: 1,
        color: "#d8d070",
        hint: "Здесь хозяйничают отряды и элитные враги со свитой.",
    },
    Tier {
        name: "Дикие земли",
        levels: [13, 20],
        party: 1,
        color: "#e8a050",
        hint: "Дикие земли: сильные звери, глубокие подземелья, одиночке надо быть начеку.",
    },
    Tier {
        name: "Опасные земли",
        levels: [21, 27],
        party: 3,
        color: "#e8603a",
        hint: "Здесь бродят чудища, которых не одолеть в одиночку: ходите группой.",
    },
    Tier {
        name: "Гиблые земли",
        levels: [28, MAX_ZONE_LEVEL],
        party: 4,
        color: "#d04ab0",
        hint: "Твердыни и владыки этих мест по силам лишь группе героев 20–30 уровня.",
    },
];

impl Tier {
    /// Its levels as shown: "21–27", the last one "28+".
    pub fn range(&self) -> String {
        let [lo, hi] = self.levels;
        if hi >= MAX_ZONE_LEVEL {
            format!("{lo}+")
        } else {
            format!("{lo}–{hi}")
        }
    }
}

/// The level of the deadliest land (with its biome and the night).
pub const MAX_ZONE_LEVEL: i32 = 40;

/// The first belt where creatures for a party live.
pub const PARTY_TIER: usize = 3;

/// The belt of a land level.
pub fn tier_of(level: i32) -> usize {
    TIERS
        .iter()
        .rposition(|t| level >= t.levels[0])
        .unwrap_or(0)
}

/// The share of the land (cumulative) every belt ends at, from the start
/// outwards: the cradle is small, the deadly lands are the far rim.
const SHARES: [f64; 5] = [0.08, 0.26, 0.54, 0.84, 1.0];
/// The level (with fractions) at the start and at the end of every belt.
const LEVELS: [f64; 6] = [1.0, 5.99, 12.99, 20.99, 27.99, 35.0];

/// The level by the distance from the start, between the knots (the
/// distances the belts end at).
fn curve(knots: &[f64], r: f64) -> f64 {
    let mut r0 = 0.0;
    for (i, &r1) in knots.iter().enumerate() {
        if r <= r1 {
            let k = if r1 > r0 { (r - r0) / (r1 - r0) } else { 1.0 };
            return LEVELS[i] + (LEVELS[i + 1] - LEVELS[i]) * k.max(0.0);
        }
        r0 = r1;
    }
    LEVELS[LEVELS.len() - 1]
}

/// The level of the land without its biome, in blocks of cells. It grows
/// with the distance from the start, measured in the shape of the island
/// (half the map's width and height count the same); the distances where
/// the belts end are the quantiles of the land, so every belt holds the
/// same share of land on any map. Worlds keep the knots in their saves.
#[derive(Clone, Debug, Default)]
pub struct ZoneMap {
    /// cells per block side
    pub cell: i32,
    pub gw: i32,
    pub gh: i32,
    base: Vec<u8>,
    /// the distances the belts end at
    pub knots: Vec<f64>,
}

impl ZoneMap {
    /// Works out the belts of the land of a surface.
    pub fn new(seed: i64, l: &Level, start: Pos) -> ZoneMap {
        ZoneMap::build(seed, l, start, None)
    }

    /// The belts of a world with its knots known (from a save).
    pub fn with_knots(seed: i64, l: &Level, start: Pos, knots: &[f64]) -> ZoneMap {
        ZoneMap::build(
            seed,
            l,
            start,
            Some(knots).filter(|k| k.len() == SHARES.len()),
        )
    }

    fn build(seed: i64, l: &Level, start: Pos, knots: Option<&[f64]>) -> ZoneMap {
        let (w, h) = (l.w, l.h);
        let mut r = Rng::labeled(seed, "zones");
        let noise = Perlin::new(&mut r);
        // about 190 blocks across any map: 16 cells on the big one
        let cell = ((w as f64 / 190.0).round() as i32).clamp(1, 16);
        let (gw, gh) = ((w + cell - 1) / cell, (h + cell - 1) / cell);
        let (hw, hh) = (w as f64 / 2.0, h as f64 / 2.0);
        let mut dist = Vec::with_capacity((gw * gh) as usize);
        let mut land = Vec::new();
        for by in 0..gh {
            for bx in 0..gw {
                let (cx, cy) = (
                    (bx * cell + cell / 2).min(w - 1),
                    (by * cell + cell / 2).min(h - 1),
                );
                let (x, y) = (cx as f64, cy as f64);
                let dx = ((x - start.x as f64) / hw).clamp(-1.0, 1.0);
                let dy = ((y - start.y as f64) / hh).clamp(-1.0, 1.0);
                // the island's own shape: 1 - (1 - dx²)(1 - dy²)
                let r = (dx * dx + dy * dy - dx * dx * dy * dy).sqrt();
                // ragged borders, but a calm cradle around the start
                let calm = ((r - 0.08) / 0.17).clamp(0.0, 1.0);
                let warp = noise.fbm(x / w as f64 * 2.6, y / h as f64 * 2.6, 3) * 0.08 * calm;
                let r = (r + warp).max(0.0);
                dist.push(r);
                if l.def(cx, cy).biome != "water" {
                    land.push(r);
                }
            }
        }
        let knots = match knots {
            Some(k) => k.to_vec(),
            None => {
                land.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let mut last = 0.0;
                SHARES
                    .iter()
                    .map(|q| {
                        let v = if land.is_empty() {
                            *q
                        } else {
                            land[((q * land.len() as f64) as usize).min(land.len() - 1)]
                        };
                        last = v.max(last + 1e-6);
                        last
                    })
                    .collect()
            }
        };
        let base = dist
            .iter()
            .map(|&r| curve(&knots, r).floor().clamp(1.0, 255.0) as u8)
            .collect();
        ZoneMap {
            cell,
            gw,
            gh,
            base,
            knots,
        }
    }

    /// The level of the land at a cell, without its biome.
    pub fn base_at(&self, p: Pos) -> i32 {
        if self.base.is_empty() {
            return 1;
        }
        let bx = (p.x / self.cell).clamp(0, self.gw - 1);
        let by = (p.y / self.cell).clamp(0, self.gh - 1);
        self.base[(by * self.gw + bx) as usize] as i32
    }

    /// The level of the land with its biome (the creatures there by day).
    pub fn level_at(&self, l: &Level, p: Pos) -> i32 {
        let biome = if l.inside(p.x, p.y) {
            region_danger(&l.def_at(p).biome)
        } else {
            0
        };
        (self.base_at(p) + biome).min(MAX_ZONE_LEVEL)
    }

    /// The level of the land around a dungeon entrance: its belt and the
    /// most dangerous biome three steps around (the entrance itself and the
    /// rubble at it are no land of any kind).
    pub fn entrance_level(&self, l: &Level, p: Pos) -> i32 {
        let mut biome = 0;
        for dy in -3..=3 {
            for dx in -3..=3 {
                let q = Pos::new(p.x + dx, p.y + dy);
                if dx.abs().max(dy.abs()) == 3 && l.inside(q.x, q.y) {
                    biome = biome.max(region_danger(&l.def_at(q).biome));
                }
            }
        }
        (self.base_at(p) + biome).min(MAX_ZONE_LEVEL)
    }

    /// The belt of every block for the map: by the land at its centre,
    /// 255 for water.
    pub fn tiers(&self, l: &Level) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.base.len());
        for by in 0..self.gh {
            for bx in 0..self.gw {
                let p = Pos::new(
                    (bx * self.cell + self.cell / 2).min(l.w - 1),
                    (by * self.cell + self.cell / 2).min(l.h - 1),
                );
                let d = l.def_at(p);
                out.push(if d.biome == "water" {
                    255
                } else {
                    tier_of(self.level_at(l, p)) as u8
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_cover_their_levels() {
        assert_eq!(tier_of(1), 0);
        assert_eq!(tier_of(5), 0);
        assert_eq!(tier_of(6), 1);
        assert_eq!(tier_of(20), 2);
        assert_eq!(tier_of(21), 3);
        assert_eq!(tier_of(27), 3);
        assert_eq!(tier_of(28), 4);
        assert_eq!(tier_of(99), 4);
        for w in TIERS.windows(2) {
            assert_eq!(w[0].levels[1] + 1, w[1].levels[0], "{}", w[1].name);
        }
        // the curve keeps to the belts between its knots
        let knots = [0.1, 0.3, 0.5, 0.7, 0.9];
        assert_eq!(curve(&knots, 0.0).floor() as i32, 1);
        for (t, &k) in knots.iter().enumerate() {
            assert_eq!(tier_of(curve(&knots, k - 0.01) as i32), t, "{k}");
        }
        let mut last = 0.0;
        for i in 0..=120 {
            let v = curve(&knots, i as f64 / 100.0);
            assert!(v >= last);
            last = v;
        }
    }

    #[test]
    fn zones_grow_from_the_start() {
        let ow = generate_overworld(7, 300, 200);
        let (l, start) = (&ow.level, ow.start);
        let z = &ow.zones;
        assert_eq!(z.base_at(start), 1);
        // every belt holds its share of the land, the far ones far away
        let mut land = [0usize; 5];
        for by in 0..z.gh {
            for bx in 0..z.gw {
                let p = Pos::new(bx * z.cell + z.cell / 2, by * z.cell + z.cell / 2);
                if !l.inside(p.x, p.y) || l.def(p.x, p.y).biome == "water" {
                    continue;
                }
                let t = tier_of(z.base_at(p));
                land[t] += 1;
                if p.manhattan(start) < 10 {
                    assert_eq!(t, 0, "{p:?} near the start");
                }
            }
        }
        let total: usize = land.iter().sum();
        for (t, &n) in land.iter().enumerate() {
            let want = SHARES[t] - if t == 0 { 0.0 } else { SHARES[t - 1] };
            let got = n as f64 / total as f64;
            assert!(
                (got - want).abs() < 0.04,
                "belt {t}: {got:.3} of the land, not {want}"
            );
        }
        // the same for the same world, and from its knots
        let z2 = ZoneMap::new(7, l, start);
        assert_eq!(z.base, z2.base);
        let z3 = ZoneMap::with_knots(7, l, start, &z.knots);
        assert_eq!(z.base, z3.base);
    }
}
