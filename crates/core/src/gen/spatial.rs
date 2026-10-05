//! Helpers that keep generation fast on big maps: how a map relates to the
//! classic world, area sums in constant time, neighbourhood lookups and an
//! even spread of candidate points.

use super::*;
use crate::content::{db, TileDef};
use crate::world::{Level, Pos};
use std::collections::HashMap;

/// The classic world the content densities were tuned on.
const BASE_AREA: f64 = 300.0 * 200.0;
/// The walkable land of the classic world: settlements, dungeons, sights and
/// elite monsters are counted per this much land.
pub const BASE_LAND: f64 = 37000.0;

/// How a map relates to the classic 300×200 world: `area` is the ratio of
/// areas, `lin` its square root (at least 1) and sizes features such as
/// biomes, wastelands and the spacing of unique characters.
#[derive(Clone, Copy, Debug)]
pub struct MapScale {
    pub area: f64,
    pub lin: f64,
}

impl MapScale {
    pub fn of(w: i32, h: i32) -> MapScale {
        let area = w as f64 * h as f64 / BASE_AREA;
        MapScale {
            area,
            lin: area.sqrt().max(1.0),
        }
    }
}

/// How many of something the classic world had `base` of, for this much land
/// (rounded, at least `min`).
pub fn per_land(land: usize, base: f64, min: usize) -> usize {
    ((land as f64 / BASE_LAND * base).round() as usize).max(min)
}

/// A value per tile id (tile definitions are looked up once, not per cell).
pub fn tile_table<T>(f: impl Fn(&TileDef) -> T) -> Vec<T> {
    db().b.tiles.iter().map(f).collect()
}

/// A summed-area table: the sum of a grid over any rectangle in O(1).
pub struct Sat {
    w: i32,
    h: i32,
    s: Vec<u32>,
}

impl Sat {
    pub fn new(l: &Level, f: impl Fn(usize) -> u32) -> Sat {
        let (w, h) = (l.w, l.h);
        let sw = (w + 1) as usize;
        let mut s = vec![0u32; sw * (h + 1) as usize];
        for y in 0..h as usize {
            let mut row = 0u32;
            for x in 0..w as usize {
                row += f(y * w as usize + x);
                s[(y + 1) * sw + x + 1] = s[y * sw + x + 1] + row;
            }
        }
        Sat { w, h, s }
    }

    /// The sum over [x0, x1) × [y0, y1), clipped to the map.
    pub fn sum(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> u32 {
        let (x0, y0) = (x0.clamp(0, self.w) as usize, y0.clamp(0, self.h) as usize);
        let (x1, y1) = (x1.clamp(0, self.w) as usize, y1.clamp(0, self.h) as usize);
        if x1 <= x0 || y1 <= y0 {
            return 0;
        }
        let sw = (self.w + 1) as usize;
        self.s[y1 * sw + x1] + self.s[y0 * sw + x0] - self.s[y0 * sw + x1] - self.s[y1 * sw + x0]
    }
}

/// Points bucketed by area for neighbourhood lookups.
pub struct Buckets {
    size: i32,
    map: HashMap<(i32, i32), Vec<usize>>,
}

impl Buckets {
    pub fn new(size: i32) -> Buckets {
        Buckets {
            size: size.max(1),
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, p: Pos, i: usize) {
        let k = (p.x.div_euclid(self.size), p.y.div_euclid(self.size));
        self.map.entry(k).or_default().push(i);
    }

    /// Everything stored within `r` cells of p along each axis (and maybe a
    /// little further: callers check the exact distance).
    pub fn near(&self, p: Pos, r: i32) -> Vec<usize> {
        let mut out = Vec::new();
        let (x0, x1) = (
            (p.x - r).div_euclid(self.size),
            (p.x + r).div_euclid(self.size),
        );
        let (y0, y1) = (
            (p.y - r).div_euclid(self.size),
            (p.y + r).div_euclid(self.size),
        );
        for by in y0..=y1 {
            for bx in x0..=x1 {
                if let Some(v) = self.map.get(&(bx, by)) {
                    out.extend_from_slice(v);
                }
            }
        }
        out
    }
}

/// Settlement areas for "is this near a village" checks.
pub struct VillageIndex<'a> {
    vs: &'a [Village],
    b: Buckets,
    /// the largest settlement half-size, added to lookups
    reach: i32,
}

impl<'a> VillageIndex<'a> {
    pub fn new(vs: &'a [Village]) -> VillageIndex<'a> {
        let mut b = Buckets::new(64);
        let mut reach = 0;
        for (i, v) in vs.iter().enumerate() {
            b.insert(v.center, i);
            reach = reach.max(v.area.w.max(v.area.h) / 2 + 1);
        }
        VillageIndex { vs, b, reach }
    }

    /// Is (x, y) within `margin` cells of a settlement's area?
    pub fn near(&self, x: i32, y: i32, margin: i32) -> bool {
        self.b
            .near(Pos::new(x, y), margin + self.reach)
            .into_iter()
            .any(|i| {
                let a = self.vs[i].area;
                x >= a.x - margin
                    && y >= a.y - margin
                    && x < a.x + a.w + margin
                    && y < a.y + a.h + margin
            })
    }

    /// Does a rectangle come within `gap` cells of a settlement's area?
    pub fn overlaps(&self, r: &Rect, gap: i32) -> bool {
        self.b
            .near(r.center(), gap + self.reach + r.w.max(r.h))
            .into_iter()
            .any(|i| self.vs[i].area.overlaps(r, gap))
    }

    /// The settlement nearest to p ("" when there are none).
    pub fn nearest_name(&self, p: Pos) -> &'a str {
        let mut r = 64;
        loop {
            let best = self
                .b
                .near(p, r)
                .into_iter()
                .min_by_key(|&i| self.vs[i].center.dist_sq(p));
            if let Some(i) = best {
                return &self.vs[i].name;
            }
            if self.vs.is_empty() || r > 1 << 14 {
                return "";
            }
            r *= 2;
        }
    }
}

/// Cells of a grid with the given step over the map, in random order; the
/// caller tries random points inside each cell (see `in_cell`).
pub fn scatter_cells(r: &mut Rng, w: i32, h: i32, step: i32) -> Vec<Rect> {
    let step = step.max(4);
    let mut cells = Vec::new();
    let mut y = 0;
    while y < h {
        let mut x = 0;
        while x < w {
            cells.push(Rect::new(x, y, step.min(w - x), step.min(h - y)));
            x += step;
        }
        y += step;
    }
    r.shuffle(&mut cells);
    cells
}

/// A random point of a cell, kept `margin` cells off the map's edges.
pub fn in_cell(r: &mut Rng, c: &Rect, w: i32, h: i32, margin: i32) -> Pos {
    let x = (c.x + r.int_n(c.w)).clamp(margin, (w - 1 - margin).max(margin));
    let y = (c.y + r.int_n(c.h)).clamp(margin, (h - 1 - margin).max(margin));
    Pos::new(x, y)
}

/// The walkable cell of the main landmass nearest to the middle of the map:
/// the heart of the world, where the journey starts.
pub fn central_land(l: &Level, main: &[bool]) -> Pos {
    let c = Pos::new(l.w / 2, l.h / 2);
    (0..l.w.max(l.h))
        .flat_map(|rad| c.ring(rad))
        .find(|p| l.inside(p.x, p.y) && main[l.idx(*p)])
        .unwrap_or(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sat_sums_rectangles() {
        let l = Level::new("t", "t", 7, 5, 0);
        let s = Sat::new(&l, |i| (i % 3) as u32);
        let mut want = 0;
        for y in 1..4 {
            for x in 2..6 {
                want += ((y * 7 + x) % 3) as u32;
            }
        }
        assert_eq!(s.sum(2, 1, 6, 4), want);
        assert_eq!(
            s.sum(-5, -5, 100, 100),
            (0..35).map(|i| (i % 3) as u32).sum::<u32>()
        );
        assert_eq!(s.sum(3, 3, 3, 4), 0);
    }

    #[test]
    fn buckets_find_neighbours() {
        let mut b = Buckets::new(16);
        b.insert(Pos::new(5, 5), 0);
        b.insert(Pos::new(100, 100), 1);
        b.insert(Pos::new(-20, 3), 2);
        let n = b.near(Pos::new(0, 0), 25);
        assert!(n.contains(&0) && n.contains(&2) && !n.contains(&1));
    }
}
