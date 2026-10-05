//! The tile map and spatial algorithms (field of view, line of sight, path
//! finding, circle movement with sliding) shared by server and client.

mod fov;
pub mod packed;
mod path;

pub use fov::*;
pub use path::*;

use crate::content::{db, TileDef};
use serde::{Deserialize, Serialize};

/// A tile cell.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord,
)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Pos {
        Pos { x, y }
    }
    pub fn add(self, o: Pos) -> Pos {
        Pos::new(self.x + o.x, self.y + o.y)
    }
    /// The Chebyshev distance.
    pub fn dist(self, o: Pos) -> i32 {
        (self.x - o.x).abs().max((self.y - o.y).abs())
    }
    pub fn manhattan(self, o: Pos) -> i32 {
        (self.x - o.x).abs() + (self.y - o.y).abs()
    }
    pub fn dist_sq(self, o: Pos) -> i32 {
        let (dx, dy) = (self.x - o.x, self.y - o.y);
        dx * dx + dy * dy
    }
    /// The centre of the cell in world coordinates.
    pub fn center(self) -> Vec2 {
        Vec2::new(self.x as f32 + 0.5, self.y as f32 + 0.5)
    }
    pub fn is_zero(&self) -> bool {
        self.x == 0 && self.y == 0
    }
    /// The cells at Chebyshev distance r, row by row: searching rings of
    /// growing r finds the nearest cells first without visiting any twice.
    pub fn ring(self, r: i32) -> impl Iterator<Item = Pos> {
        (-r..=r).flat_map(move |dy| {
            // the top and bottom rows whole, of the others only both ends
            let step = if dy.abs() == r { 1 } else { 2 * r };
            (-r..=r)
                .step_by(step as usize)
                .map(move |dx| Pos::new(self.x + dx, self.y + dy))
        })
    }
}

/// A point in world coordinates: one unit is one tile, the cell (x, y)
/// spans [x, x+1) × [y, y+1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub const fn new(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }
    pub fn from_angle(a: f32) -> Vec2 {
        Vec2::new(a.cos(), a.sin())
    }
    pub fn cell(self) -> Pos {
        Pos::new(self.x.floor() as i32, self.y.floor() as i32)
    }
    pub fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn len_sq(self) -> f32 {
        self.x * self.x + self.y * self.y
    }
    pub fn dist(self, o: Vec2) -> f32 {
        (self - o).len()
    }
    pub fn norm(self) -> Vec2 {
        let l = self.len();
        if l < 1e-6 {
            Vec2::ZERO
        } else {
            self * (1.0 / l)
        }
    }
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }
    pub fn dot(self, o: Vec2) -> f32 {
        self.x * o.x + self.y * o.y
    }
    pub fn lerp(self, o: Vec2, t: f32) -> Vec2 {
        self + (o - self) * t
    }
}

impl std::ops::Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}
impl std::ops::Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}
impl std::ops::Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, k: f32) -> Vec2 {
        Vec2::new(self.x * k, self.y * k)
    }
}
impl std::ops::AddAssign for Vec2 {
    fn add_assign(&mut self, o: Vec2) {
        self.x += o.x;
        self.y += o.y;
    }
}

/// The four cardinal neighbours.
pub const DIRS4: [Pos; 4] = [
    Pos::new(0, -1),
    Pos::new(1, 0),
    Pos::new(0, 1),
    Pos::new(-1, 0),
];
/// All eight neighbours, cardinal first.
pub const DIRS8: [Pos; 8] = [
    Pos::new(0, -1),
    Pos::new(1, 0),
    Pos::new(0, 1),
    Pos::new(-1, 0),
    Pos::new(1, -1),
    Pos::new(1, 1),
    Pos::new(-1, 1),
    Pos::new(-1, -1),
];

/// The dominant cardinal direction from a to b (as a unit cell offset).
pub fn dir_towards(a: Pos, b: Pos) -> Pos {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    if dx == 0 && dy == 0 {
        return Pos::default();
    }
    if dx.abs() >= dy.abs() {
        Pos::new(dx.signum(), 0)
    } else {
        Pos::new(0, dy.signum())
    }
}

/// One map: the overworld or a single dungeon floor.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Level {
    pub id: String,
    pub name: String,
    pub w: i32,
    pub h: i32,
    #[serde(with = "packed")]
    pub tiles: Vec<u8>,
    /// 0 = overworld
    pub depth: i32,
    /// crypt, cave, overworld...
    pub theme: String,
    /// entrance index, -1 for the overworld
    pub dungeon: i32,
    pub up: Pos,
    pub down: Pos,
    /// affected by daylight
    pub lit: bool,
    /// grows with every change of a tile (not saved)
    #[serde(skip)]
    pub ver: u64,
}

impl Level {
    pub fn new(id: &str, name: &str, w: i32, h: i32, fill: u8) -> Level {
        Level {
            id: id.into(),
            name: name.into(),
            w,
            h,
            tiles: vec![fill; (w * h) as usize],
            dungeon: -1,
            ..Default::default()
        }
    }
    pub fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }
    pub fn at(&self, x: i32, y: i32) -> u8 {
        if !self.inside(x, y) {
            return 0;
        }
        self.tiles[(y * self.w + x) as usize]
    }
    pub fn set(&mut self, x: i32, y: i32, t: u8) {
        if self.inside(x, y) {
            self.tiles[(y * self.w + x) as usize] = t;
            self.ver += 1;
        }
    }
    pub fn def(&self, x: i32, y: i32) -> &'static TileDef {
        db().tile(self.at(x, y))
    }
    pub fn def_at(&self, p: Pos) -> &'static TileDef {
        self.def(p.x, p.y)
    }
    pub fn walkable(&self, x: i32, y: i32) -> bool {
        self.inside(x, y) && self.def(x, y).walkable
    }
    pub fn transparent(&self, x: i32, y: i32) -> bool {
        self.inside(x, y) && self.def(x, y).transparent
    }
    pub fn idx(&self, p: Pos) -> usize {
        (p.y * self.w + p.x) as usize
    }

    /// Moves a circle of radius r from `from` by `delta`, sliding along cells
    /// it cannot enter. Each axis moves separately, so walking diagonally
    /// into a wall slides along it. Returns the new position.
    pub fn slide(
        &self,
        from: Vec2,
        delta: Vec2,
        r: f32,
        blocked: &dyn Fn(i32, i32) -> bool,
    ) -> Vec2 {
        // long moves are split so a fast mover cannot tunnel through a wall
        let steps = ((delta.len() / (r.max(0.1) * 0.9)).ceil() as i32).max(1);
        let d = delta * (1.0 / steps as f32);
        let mut p = from;
        for _ in 0..steps {
            let nx = Vec2::new(p.x + d.x, p.y);
            if !circle_hits(nx, r, blocked) {
                p = nx;
            } else {
                p.x = push_axis(p, d.x, r, true, blocked);
            }
            let ny = Vec2::new(p.x, p.y + d.y);
            if !circle_hits(ny, r, blocked) {
                p = ny;
            } else {
                p.y = push_axis(p, d.y, r, false, blocked);
            }
        }
        p
    }

    /// Reports whether a circle fits without touching blocked cells.
    pub fn circle_fits(&self, p: Vec2, r: f32, blocked: &dyn Fn(i32, i32) -> bool) -> bool {
        !circle_hits(p, r, blocked)
    }
}

/// Moves along one axis as far as possible (up to the wall).
fn push_axis(p: Vec2, d: f32, r: f32, x_axis: bool, blocked: &dyn Fn(i32, i32) -> bool) -> f32 {
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..8 {
        let mid = (lo + hi) / 2.0;
        let q = if x_axis {
            Vec2::new(p.x + d * mid, p.y)
        } else {
            Vec2::new(p.x, p.y + d * mid)
        };
        if circle_hits(q, r, blocked) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    if x_axis {
        p.x + d * lo
    } else {
        p.y + d * lo
    }
}

/// Reports whether a circle overlaps any blocked cell.
pub fn circle_hits(p: Vec2, r: f32, blocked: &dyn Fn(i32, i32) -> bool) -> bool {
    let (x0, x1) = ((p.x - r).floor() as i32, (p.x + r).floor() as i32);
    let (y0, y1) = ((p.y - r).floor() as i32, (p.y + r).floor() as i32);
    for y in y0..=y1 {
        for x in x0..=x1 {
            if !blocked(x, y) {
                continue;
            }
            // nearest point of the cell to the circle centre
            let nx = p.x.clamp(x as f32, x as f32 + 1.0);
            let ny = p.y.clamp(y as f32, y as f32 + 1.0);
            let (dx, dy) = (p.x - nx, p.y - ny);
            if dx * dx + dy * dy < r * r - 1e-6 {
                return true;
            }
        }
    }
    false
}

/// A compact boolean grid used for explored-map memory.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Bitset(#[serde(with = "packed")] pub Vec<u8>);

impl Bitset {
    pub fn new(n: usize) -> Bitset {
        Bitset(vec![0; n.div_ceil(8)])
    }
    pub fn get(&self, i: usize) -> bool {
        i / 8 < self.0.len() && self.0[i / 8] & (1 << (i % 8)) != 0
    }
    pub fn set(&mut self, i: usize) {
        if i / 8 < self.0.len() {
            self.0[i / 8] |= 1 << (i % 8);
        }
    }
    pub fn len_for(&self, n: usize) -> bool {
        self.0.len() == n.div_ceil(8)
    }
    pub fn fill(&mut self) {
        for b in &mut self.0 {
            *b = 0xff;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rings_go_round() {
        let c = Pos::new(5, 5);
        assert_eq!(c.ring(0).collect::<Vec<_>>(), vec![c]);
        for r in 1..4 {
            let ring: Vec<Pos> = c.ring(r).collect();
            assert_eq!(ring.len(), 8 * r as usize);
            assert!(ring.iter().all(|p| p.dist(c) == r));
            let mut sorted = ring.clone();
            sorted.sort_by_key(|p| (p.y, p.x));
            assert_eq!(ring, sorted, "row by row");
        }
    }

    #[test]
    fn slide_along_walls() {
        // a wall at x = 3
        let blocked = |x: i32, _y: i32| x == 3;
        let l = Level::new("t", "t", 10, 10, 0);
        let p = l.slide(Vec2::new(2.5, 5.5), Vec2::new(1.0, 1.0), 0.3, &blocked);
        assert!(p.x <= 2.7 + 1e-3, "{p:?}");
        assert!((p.y - 6.5).abs() < 1e-3, "slides down: {p:?}");
    }

    #[test]
    fn no_tunnelling() {
        let blocked = |x: i32, _y: i32| x == 3;
        let l = Level::new("t", "t", 10, 10, 0);
        let p = l.slide(Vec2::new(2.5, 5.5), Vec2::new(5.0, 0.0), 0.3, &blocked);
        assert!(p.x < 3.0, "{p:?}");
    }
}
