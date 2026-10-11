//! Roofs over the houses of the detailed look: rows of rough tiles (or
//! shingles, slate, thatch) stepping up to a ridge, gables of plaster and
//! timber, boards along the eaves, a chimney on some. A roof is lifted off
//! while the hero is inside its house.

use macroquad::prelude::*;
use ratas_core::content;
use ratas_core::world::{Bitset, Level};

use super::lowpoly::{hash2, hashf, Batch, Xf};
use super::materials::{PLASTER, STONE, STRAW, WOOD};
use super::terrain::{mix, mul, CHUNK};
use crate::gfx::col;

/// A house: tiles x..x+w, y..y+h, walls round the edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct House {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl House {
    /// The point is inside its walls.
    pub fn holds(&self, px: f32, py: f32) -> bool {
        px > self.x as f32
            && px < (self.x + self.w) as f32
            && py > self.y as f32
            && py < (self.y + self.h) as f32
    }
}

fn wall_like(l: &Level, x: i32, y: i32) -> bool {
    l.inside(x, y)
        && matches!(
            content::db().tile(l.at(x, y)).key.as_str(),
            "house_wall" | "door" | "door_open"
        )
}

/// The houses that reach into a chunk and have been seen (a wall of them).
pub fn houses(l: &Level, explored: &Bitset, cx: i32, cy: i32) -> Vec<House> {
    let db = content::db();
    let mut out = vec![];
    // a house reaching into the chunk has its corner up to 24 tiles before
    let (x0, y0) = ((cx * CHUNK - 24).max(0), (cy * CHUNK - 24).max(0));
    let (x1, y1) = (((cx + 1) * CHUNK).min(l.w), ((cy + 1) * CHUNK).min(l.h));
    for y in y0..y1 {
        for x in x0..x1 {
            if db.tile(l.at(x, y)).key != "house_wall"
                || wall_like(l, x - 1, y)
                || wall_like(l, x, y - 1)
            {
                continue;
            }
            let mut w = 1;
            while wall_like(l, x + w, y) {
                w += 1;
            }
            let mut h = 1;
            while wall_like(l, x, y + h) {
                h += 1;
            }
            if w < 3 || h < 3 || w > 24 || h > 24 {
                continue;
            }
            // it reaches into this chunk
            if x + w <= cx * CHUNK || y + h <= cy * CHUNK {
                continue;
            }
            let closed = (y..y + h).all(|yy| wall_like(l, x + w - 1, yy))
                && (x..x + w).all(|xx| wall_like(l, xx, y + h - 1))
                && !wall_like(l, x + 1, y + 1);
            let seen = |xx: i32, yy: i32| explored.get((yy * l.w + xx) as usize);
            let any_seen = (x..x + w).any(|xx| seen(xx, y) || seen(xx, y + h - 1))
                || (y..y + h).any(|yy| seen(x, yy) || seen(x + w - 1, yy));
            if closed && any_seen {
                out.push(House { x, y, w, h });
            }
        }
    }
    out
}

/// The height of the eaves.
const EAVE: f32 = 1.1;
/// How far the roof reaches past the walls.
const OVER: f32 = 0.26;
/// The rise of the roof per tile across.
const PITCH: f32 = 0.62;

/// Builds the roof of a house.
pub fn roof(b: &mut Batch, house: House) {
    let s = hash2(house.x, house.y);
    let kind = s % 4;
    let base = match kind {
        0 => Color::new(0.64, 0.3, 0.2, 1.0),
        1 => Color::new(0.34, 0.38, 0.47, 1.0),
        2 => Color::new(0.72, 0.58, 0.3, 1.0),
        _ => Color::new(0.47, 0.32, 0.22, 1.0),
    };
    let thatch = kind == 2;
    // clay, slate, straw or shingles of wood
    let stuff = [PLASTER, STONE, STRAW, WOOD][kind as usize];
    let (x0, z0) = (house.x as f32, house.y as f32);
    let (x1, z1) = ((house.x + house.w) as f32, (house.y + house.h) as f32);
    // u runs along the ridge, v across it
    let along_x = house.w >= house.h;
    let (u0, u1, v0, v1) = if along_x {
        (x0, x1, z0, z1)
    } else {
        (z0, z1, x0, x1)
    };
    let at = |u: f32, y: f32, v: f32| {
        if along_x {
            vec3(u, y, v)
        } else {
            vec3(v, y, u)
        }
    };
    let vc = (v0 + v1) / 2.0;
    let span = (v1 - v0) / 2.0 + OVER;
    let top = EAVE + span * PITCH;
    let (ua, ub) = (u0 - OVER, u1 + OVER);
    let row_h = if thatch { 0.3 } else { 0.21 };
    let slope = (span * span + (top - EAVE).powi(2)).sqrt();
    let rows = (slope / row_h).ceil() as i32;
    let mut k = s;
    let mut rnd = || {
        k = k.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        hashf(k)
    };
    for side in [-1.0f32, 1.0] {
        b.mat = stuff;
        let eave = |u: f32| at(u, EAVE, vc + side * span);
        let ridge = |u: f32| at(u, top, vc);
        let n = at(0.0, span, side * (top - EAVE)).normalize();
        let p = |t: f32, u: f32, lift: f32| eave(u).lerp(ridge(u), t) + n * lift;
        for r in 0..rows {
            let t0 = (r as f32 - 0.35) / rows as f32;
            let t1 = (r + 1) as f32 / rows as f32;
            let mut u = ua - rnd() * 0.15;
            while u < ub {
                let w = if thatch {
                    0.45 + rnd() * 0.35
                } else {
                    0.22 + rnd() * 0.14
                };
                let (pa, pb) = (u.max(ua), (u + w).min(ub));
                u += w;
                if pb - pa < 0.05 {
                    continue;
                }
                let g = 0.012;
                let (pa, pb) = (pa + g, pb - g);
                let lift = if thatch { 0.06 } else { 0.035 } + rnd() * 0.015;
                let mut j = |v: f32| v + (rnd() - 0.5) * 0.02;
                let a = p(t0.max(-0.05), pa, j(lift));
                let bb = p(t0.max(-0.05), pb, j(lift));
                let c = p(t1, pb, j(0.004));
                let d = p(t1, pa, j(0.004));
                let mut cc = mul(base, 0.84 + 0.26 * rnd());
                if rnd() < 0.07 {
                    // moss
                    cc = mix(cc, Color::new(0.38, 0.48, 0.24, 1.0), 0.5);
                }
                b.quad(a, bb, c, d, cc, 0.0);
                let (a2, b2) = (p(t0.max(-0.05), pa, 0.0), p(t0.max(-0.05), pb, 0.0));
                b.quad(a2, b2, bb, a, mul(cc, 0.62), 0.0);
            }
        }
        // a board along the eave
        b.mat = WOOD;
        let f0 = eave(ua) - n * 0.02 - Vec3::Y * 0.03;
        let f1 = eave(ub) - n * 0.02 - Vec3::Y * 0.03;
        b.capsule(f0, f1, 0.035, 0.035, col("#4a3222"), 0.0);
    }
    // the ridge
    b.mat = stuff;
    b.capsule(
        at(ua - 0.02, top + 0.04, vc),
        at(ub + 0.02, top + 0.04, vc),
        0.075,
        0.075,
        mul(base, 0.7),
        0.0,
    );
    // gables: plaster in a frame of timber
    let plaster = col("#e2d6bc");
    let wood = col("#4e3624");
    for u in [u0 + 0.02, u1 - 0.02] {
        let (a, bb, c) = (at(u, EAVE, v0), at(u, EAVE, v1), at(u, top - 0.05, vc));
        b.mat = PLASTER;
        b.tri(a, bb, c, plaster, 0.0);
        b.tri(a, c, bb, plaster, 0.0);
        b.mat = WOOD;
        let out = if u < (u0 + u1) / 2.0 { -0.03 } else { 0.03 };
        let o = at(out, 0.0, 0.0);
        b.capsule(at(u, EAVE, vc) + o, c + o, 0.035, 0.035, wood, 0.0);
        b.capsule(
            at(u, EAVE + 0.02, v0 + 0.15) + o,
            at(u, EAVE + 0.02, v1 - 0.15) + o,
            0.035,
            0.035,
            wood,
            0.0,
        );
        let mid = (top - EAVE) * 0.5 + EAVE;
        b.capsule(
            at(u, EAVE, vc - span * 0.35) + o,
            at(u, mid, vc) + o,
            0.025,
            0.025,
            wood,
            0.0,
        );
        b.capsule(
            at(u, EAVE, vc + span * 0.35) + o,
            at(u, mid, vc) + o,
            0.025,
            0.025,
            wood,
            0.0,
        );
    }
    // a chimney of stones on some
    if hashf(s.wrapping_add(3)) < 0.6 {
        let u = u0 + 1.0 + (u1 - u0 - 2.0).max(0.0) * hashf(s.wrapping_add(4));
        let side = if hashf(s.wrapping_add(5)) < 0.5 {
            -1.0
        } else {
            1.0
        };
        let v = vc + side * span * 0.42;
        let y_roof = EAVE + (span - span * 0.42) * PITCH;
        let hgt = top + 0.4 - y_roof + 0.2;
        let o = at(u, y_roof - 0.2, v);
        let stone = Color::new(0.52, 0.5, 0.47, 1.0);
        b.mat = STONE;
        b.cuboid(
            Xf::at(o),
            vec3(0.0, hgt / 2.0, 0.0),
            vec3(0.17, hgt / 2.0, 0.17),
            stone,
            0.0,
        );
        b.cuboid(
            Xf::at(o),
            vec3(0.0, hgt + 0.03, 0.0),
            vec3(0.21, 0.05, 0.21),
            mul(stone, 0.8),
            0.0,
        );
        b.cuboid(
            Xf::at(o),
            vec3(0.0, hgt + 0.08, 0.0),
            vec3(0.1, 0.01, 0.1),
            Color::new(0.06, 0.05, 0.05, 1.0),
            0.0,
        );
    }
    b.mat = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_house_gets_a_roof() {
        let db = content::db();
        let id = |k: &str| db.b.tiles.iter().position(|t| t.key == k).unwrap() as u8;
        let mut l = Level::new("t", "t", 16, 16, 0);
        l.lit = true;
        for t in l.tiles.iter_mut() {
            *t = id("grass");
        }
        for y in 3..9 {
            for x in 2..10 {
                let edge = x == 2 || y == 3 || x == 9 || y == 8;
                l.tiles[(y * l.w + x) as usize] =
                    id(if edge { "house_wall" } else { "house_floor" });
            }
        }
        l.tiles[(8 * l.w + 5) as usize] = id("door");
        let mut seen = Bitset::new((l.w * l.h) as usize);
        for i in 0..(l.w * l.h) as usize {
            seen.set(i);
        }
        let hs = houses(&l, &seen, 0, 0);
        assert_eq!(
            hs,
            vec![House {
                x: 2,
                y: 3,
                w: 8,
                h: 6
            }]
        );
        assert!(hs[0].holds(5.5, 5.5) && !hs[0].holds(5.5, 9.5));
        let mut b = Batch::new(true);
        roof(&mut b, hs[0]);
        assert!(b.i.len() > 3000, "{}", b.i.len());
        assert!(b.v.iter().all(|v| v.pos().is_finite()));
    }
}
