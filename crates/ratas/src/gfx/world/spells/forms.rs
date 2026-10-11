//! The abilities in the 3D world, made of shapes like everything else there.
//! Every ability has forms of its own (its `Design`): a heart (an orb, a
//! crystal, a star, a cluster, a planet, a helix, a flame, a wheel) drawn in
//! the ways of its element, a crown around it (motes, spikes, petals, rings,
//! blades, wings, ribbons) and an accent of its element (flames, sparks,
//! bubbles, leaves, ice...). Its missiles fly as these forms or as things
//! (arrows, blades, rocks, bones, flasks, hammers); its moments show its sign;
//! and the shapes all abilities share (rings, bolts, pillars, runes, slashes,
//! shields, clocks, holes) and the little things they scatter (snowflakes,
//! shards, leaves, notes, coins, bones) are bodies in the world. Solid things
//! go into one batch and are lit by the sun; light goes into another and is
//! added to the picture.

use super::*;
use crate::gfx::world::d3::lowpoly::{hashf, Batch, Xf, HALO, RIM, SHINY, UNLIT};
use crate::gfx::world::d3::materials::{BONE, FUR, LEAVES, METAL, STONE, WOOD};
use crate::gfx::world::d3::Cam;

/// How high over the ground blows land and missiles fly.
const CHEST: f32 = 0.62;

/// The batches the forms of a frame are built into.
pub struct Out<'a> {
    /// solid things, lit by the sun
    pub solid: &'a mut Batch,
    /// light, added to the picture
    pub light: &'a mut Batch,
}

impl Out<'_> {
    /// The batch of light, building with these flags (HALO, RIM or none).
    fn glow(&mut self, flags: u8) -> &mut Batch {
        self.light.flags = UNLIT | flags;
        self.light.mat = 0;
        self.light.sway = 0.0;
        &mut *self.light
    }

    /// The batch of solid things, building of a material.
    fn thing(&mut self, mat: u8, shiny: bool) -> &mut Batch {
        self.solid.flags = if shiny { SHINY } else { 0 };
        self.solid.mat = mat;
        self.solid.sway = 0.0;
        &mut *self.solid
    }
}

fn ca(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, (c.a * a).clamp(0.0, 1.0))
}

/// A colour of light made richer (pale colours added up turn white).
fn vivid(c: Color) -> Color {
    let l = c.r * 0.3 + c.g * 0.55 + c.b * 0.15;
    let k = 1.7;
    let f = |v: f32| (l + (v - l) * k).clamp(0.0, 1.0) * 0.85;
    Color::new(f(c.r), f(c.g), f(c.b), c.a)
}

fn mixc(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

/// A frame at `o` whose x axis runs along `d`, y up as far as it can, of
/// size `k`, rolled around its x axis by `roll`.
fn along(o: Vec3, d: Vec3, k: f32, roll: f32) -> Xf {
    let x = d.normalize_or(Vec3::X);
    let up = if x.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
    let z = x.cross(up).normalize_or(Vec3::Z);
    let y = z.cross(x);
    Xf {
        o,
        m: Mat3::from_cols(x, y, z)
            * Mat3::from_rotation_x(roll)
            * Mat3::from_diagonal(Vec3::splat(k)),
    }
}

/// A frame standing at `o`, turned around the vertical by `yaw`, its x axis
/// pointing up (signs stand on their tails).
fn upright(o: Vec3, yaw: f32, k: f32) -> Xf {
    Xf {
        o,
        m: Mat3::from_rotation_y(yaw)
            * Mat3::from_cols(Vec3::Y, -Vec3::X, Vec3::Z)
            * Mat3::from_diagonal(Vec3::splat(k)),
    }
}

/// What the bodies of an element are made of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Body {
    Light,
    Ice,
    Stone,
    Metal,
    Bone,
    Leaf,
    Dark,
    Liquid,
    Gold,
}

fn body(e: Elem) -> Body {
    match e {
        Elem::Frost => Body::Ice,
        Elem::Earth => Body::Stone,
        Elem::Steel | Elem::Arrow => Body::Metal,
        Elem::Bone | Elem::Beast => Body::Bone,
        Elem::Nature => Body::Leaf,
        Elem::Shadow | Elem::Void => Body::Dark,
        Elem::Blood | Elem::Water | Elem::Alchemy => Body::Liquid,
        Elem::Gold => Body::Gold,
        _ => Body::Light,
    }
}

/// A random number from a seed and a moment (things that flicker).
fn flick(seed: u32, t: f32, rate: f32) -> f32 {
    hashf(seed.wrapping_mul(7919) ^ (t * rate) as u32)
}

/// Points of a lightning between two points, jagged by `dev`.
fn zigzag(a: Vec3, b: Vec3, dev: f32, seed: u32, n: usize) -> Vec<Vec3> {
    let d = b - a;
    let side = d.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = side.cross(d).normalize_or(Vec3::Y);
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            if i == 0 || i == n {
                return a + d * t;
            }
            let k = (t * PI).sin();
            let s = hashf(seed.wrapping_add(i as u32 * 31)) - 0.5;
            let u = hashf(seed.wrapping_add(i as u32 * 57 + 3)) - 0.5;
            a + d * t + (side * s + up * u) * dev * 2.0 * k
        })
        .collect()
}

/// A lightning bolt of light: a wide glow and a white core.
fn bolt(o: &mut Out, pts: &[Vec3], w: f32, c: Color, a: f32) {
    if pts.len() < 2 {
        return;
    }
    let outer: Vec<f32> = pts.iter().map(|_| w).collect();
    let inner: Vec<f32> = pts.iter().map(|_| w * 0.35).collect();
    o.glow(HALO).tube(pts, &outer, false, ca(c, 0.55 * a), 1.0);
    o.glow(0).tube(pts, &inner, false, ca(WHITE, 0.9 * a), 1.0);
}

// ---- the heart, the crown, the accent ----

/// The colours a body is drawn in: its own and the hot one.
fn tints(lk: &Look, b: Body) -> (Color, Color) {
    match b {
        Body::Ice => (
            Color::new(0.72, 0.9, 1.0, 0.82),
            Color::new(0.9, 0.97, 1.0, 0.9),
        ),
        Body::Stone => (mixc(lk.c, Color::new(0.45, 0.4, 0.36, 1.0), 0.65), lk.hot),
        Body::Metal => (mixc(lk.c, Color::new(0.78, 0.8, 0.86, 1.0), 0.6), lk.hot),
        Body::Bone => (Color::new(0.93, 0.88, 0.76, 1.0), lk.hot),
        Body::Leaf => (mixc(lk.c, Color::new(0.3, 0.6, 0.22, 1.0), 0.4), lk.hot),
        Body::Dark => (Color::new(0.05, 0.02, 0.09, 1.0), lk.c),
        Body::Liquid => (ca(lk.c, 0.88), lk.hot),
        Body::Gold => (
            Color::new(1.0, 0.78, 0.25, 1.0),
            Color::new(1.0, 0.95, 0.6, 1.0),
        ),
        Body::Light => (vivid(lk.c), mixc(vivid(lk.c), WHITE, 0.45)),
    }
}

/// The heart of an ability's forms at `xf` (x along its way, about 1
/// across): one of CORES shapes, of its element's body.
fn heart(o: &mut Out, lk: &Look, xf: Xf, t: f32, a: f32) {
    let bd = body(lk.elem);
    let (c, hot) = tints(lk, bd);
    let d = lk.d;
    let spin = d.spin as f32;
    let solid = bd != Body::Light;
    // a solid heart glows a little, a heart of light is all glow
    let halo_r = if solid { 1.5 } else { 1.3 };
    o.glow(HALO).ellipsoid(
        xf,
        Vec3::ZERO,
        Vec3::splat(halo_r),
        ca(vivid(lk.c), 0.13 * a),
        1.0,
    );
    let roll = t * spin * 1.7;
    let (col, inner) = if solid {
        (ca(c, a.max(0.3)), None)
    } else {
        (ca(c, 0.42 * a), Some(ca(hot, 0.6 * a)))
    };
    let b = if solid {
        let (mat, shiny) = match bd {
            Body::Stone => (STONE, false),
            Body::Metal => (METAL, true),
            Body::Bone => (BONE, false),
            Body::Leaf => (LEAVES, false),
            _ => (0, true),
        };
        o.thing(mat, shiny)
    } else {
        o.glow(HALO)
    };
    let rx = xf.then(Vec3::ZERO, Mat3::from_rotation_x(roll));
    match d.core {
        0 => {
            if solid && bd == Body::Stone {
                b.stone(rx, Vec3::ZERO, Vec3::splat(0.85), d.n as u32 + 1, col, 0.0);
            } else {
                b.blob(
                    rx,
                    Vec3::ZERO,
                    Vec3::splat(0.85),
                    0.08,
                    d.n as u32,
                    col,
                    0.3,
                );
            }
        }
        1 => {
            b.gem(rx, Vec3::ZERO, vec3(1.45, 0.6, 0.6), 0, col, 0.3);
        }
        2 => {
            b.ellipsoid(rx, Vec3::ZERO, Vec3::splat(0.45), col, 0.3);
            let dirs = [
                Vec3::X,
                -Vec3::X,
                Vec3::Y,
                -Vec3::Y,
                Vec3::Z,
                -Vec3::Z,
                vec3(0.0, 0.7, 0.7),
                vec3(0.0, -0.7, -0.7),
            ];
            for dd in dirs {
                b.spike(
                    rx.p(dd * 0.3),
                    rx.p(dd * 1.25),
                    0.2 * xf.size(),
                    6,
                    col,
                    0.3,
                );
            }
        }
        3 => {
            for i in 0..3 {
                let ang = i as f32 * TAU / 3.0;
                let tilt = Xf {
                    o: rx.o,
                    m: rx.m * Mat3::from_rotation_x(ang) * Mat3::from_rotation_z(0.5),
                };
                b.gem(
                    tilt,
                    vec3(0.4, 0.0, 0.0),
                    vec3(0.8, 0.28, 0.28),
                    0,
                    col,
                    0.3,
                );
            }
            b.gem(rx, vec3(0.3, 0.0, 0.0), vec3(1.0, 0.36, 0.36), 0, col, 0.3);
        }
        4 => {
            b.ellipsoid(rx, Vec3::ZERO, Vec3::splat(0.72), col, 0.3);
            let ring = Xf {
                o: xf.o,
                m: xf.m * Mat3::from_rotation_z(0.45) * Mat3::from_rotation_y(t * spin),
            };
            b.torus(ring, Vec3::ZERO, 1.15, 0.07, col, 0.5);
        }
        5 => {
            for s in [0.0, PI] {
                let pts: Vec<Vec3> = (0..10)
                    .map(|i| {
                        let k = i as f32 / 9.0;
                        let ang = s + k * 5.0 + t * spin * 3.0;
                        xf.p(vec3(0.9 - k * 2.4, ang.cos() * 0.45, ang.sin() * 0.45))
                    })
                    .collect();
                let radii: Vec<f32> = (0..10)
                    .map(|i| xf.size() * 0.16 * (1.0 - i as f32 / 12.0))
                    .collect();
                b.tube(&pts, &radii, true, col, 0.3);
            }
            b.ellipsoid(xf, vec3(0.9, 0.0, 0.0), Vec3::splat(0.38), col, 0.3);
        }
        6 => {
            // a flame, its tail streaming back and waving
            let wave = (t * 9.0).sin() * 0.12;
            let tail = Xf {
                o: xf.o,
                m: xf.m * Mat3::from_rotation_z(FRAC_PI_2 + wave),
            };
            b.lathe(
                tail,
                Vec3::ZERO,
                &[
                    (-0.85, 0.0, 0.0),
                    (-0.6, 0.65, 0.65),
                    (0.0, 0.85, 0.85),
                    (0.7, 0.55, 0.55),
                    (1.5, 0.2, 0.2),
                    (2.1, 0.0, 0.0),
                ],
                10,
                false,
                col,
                0.3,
            );
        }
        _ => {
            // a wheel rolling on its way
            let wheel = Xf {
                o: xf.o,
                m: xf.m * Mat3::from_rotation_x(FRAC_PI_2) * Mat3::from_rotation_y(t * spin * 4.0),
            };
            b.lathe(
                wheel,
                vec3(0.0, -0.12, 0.0),
                &[(0.0, 0.95, 0.95), (0.24, 0.95, 0.95)],
                12,
                true,
                col,
                0.3,
            );
            for i in 0..d.n {
                let ang = i as f32 * TAU / d.n as f32;
                let dir = vec3(ang.cos(), 0.0, ang.sin());
                b.spike(
                    wheel.p(dir * 0.8),
                    wheel.p(dir * 1.45),
                    0.18 * xf.size(),
                    5,
                    col,
                    0.3,
                );
            }
        }
    }
    if let Some(core) = inner {
        o.glow(HALO)
            .ellipsoid(xf, Vec3::ZERO, Vec3::splat(0.3), core, 1.0);
    }
}

/// What crowns an ability's forms: one of CROWNS ornaments of `n` parts,
/// around the heart at `xf`.
fn crown(o: &mut Out, lk: &Look, xf: Xf, t: f32, a: f32) {
    let d = lk.d;
    let n = d.n.max(3) as usize;
    let spin = d.spin as f32;
    let v = vivid(lk.c);
    let (c, hot) = (ca(v, 0.55 * a), ca(mixc(v, WHITE, 0.35), 0.6 * a));
    let k = xf.size();
    // the circle around the way (the y-z plane of the frame)
    let around = |i: usize, r: f32, turn: f32| {
        let ang = i as f32 * TAU / n as f32 + turn;
        vec3(0.0, ang.cos() * r, ang.sin() * r)
    };
    match d.crown {
        0 => {
            let ring = Xf {
                o: xf.o,
                m: xf.m * Mat3::from_rotation_z(FRAC_PI_2),
            };
            let pulse = 1.0 + 0.08 * (t * 6.0).sin();
            o.glow(HALO)
                .torus(ring, Vec3::ZERO, 1.6 * pulse, 0.07, c, 1.0);
            for i in 0..n {
                let p = around(i, 1.6 * pulse, t * spin);
                o.glow(HALO).ellipsoid(xf, p, Vec3::splat(0.13), hot, 1.0);
            }
        }
        1 => {
            for i in 0..n {
                let p = around(i, 1.7, t * spin * 2.2);
                let wob = (t * 5.0 + i as f32).sin() * 0.3;
                o.glow(HALO)
                    .ellipsoid(xf, p + vec3(wob, 0.0, 0.0), Vec3::splat(0.24), hot, 1.0);
            }
        }
        2 => {
            for i in 0..n {
                let dir = around(i, 1.0, t * spin * 0.8);
                let b = o.glow(HALO);
                b.spike(
                    xf.p(dir * 0.9),
                    xf.p(dir * 2.1 - vec3(0.3, 0.0, 0.0)),
                    0.16 * k,
                    6,
                    c,
                    1.0,
                );
            }
        }
        3 => {
            for i in 0..n {
                let dir = around(i, 1.0, t * spin * 0.5);
                let base = xf.p(dir * 0.5 - vec3(0.2, 0.0, 0.0));
                let tip = xf.p(dir * 1.9 - vec3(1.0, 0.0, 0.0));
                let side = xf.n(vec3(0.0, -dir.z, dir.y));
                let up = xf.n(dir);
                o.glow(0).leaf(base, tip, side, up, 0.32 * k, c, 1.0);
            }
        }
        4 => {
            for (j, (ax, rate)) in [(0, 1.0f32), (1, -1.4), (2, 0.8)].into_iter().enumerate() {
                let rot = match ax {
                    0 => Mat3::from_rotation_z(FRAC_PI_2) * Mat3::from_rotation_y(t * spin * rate),
                    1 => Mat3::from_rotation_x(t * spin * rate),
                    _ => Mat3::from_rotation_z(t * spin * rate),
                };
                let ring = Xf {
                    o: xf.o,
                    m: xf.m * rot,
                };
                let r = 1.45 + j as f32 * 0.18;
                o.glow(HALO).torus(ring, Vec3::ZERO, r, 0.05, c, 1.0);
            }
            for i in 0..n.min(4) {
                let p = around(i, 1.5, t * spin * 1.4);
                o.glow(HALO).ellipsoid(xf, p, Vec3::splat(0.12), hot, 1.0);
            }
        }
        5 => {
            let metal = body(lk.elem) == Body::Metal;
            for i in 0..n {
                let ang = i as f32 * TAU / n as f32 + t * spin * 3.0;
                let p = vec3(0.0, ang.cos() * 1.6, ang.sin() * 1.6);
                let tangent = vec3(0.0, -ang.sin(), ang.cos());
                let (base, tip) = (xf.p(p - tangent * 0.5), xf.p(p + tangent * 0.6));
                let side = xf.n(p.normalize());
                if metal {
                    o.thing(METAL, true).blade(
                        base,
                        tip,
                        side,
                        0.14 * k,
                        0.03 * k,
                        Color::new(0.85, 0.87, 0.92, a),
                    );
                } else {
                    o.glow(0).blade(base, tip, side, 0.14 * k, 0.03 * k, hot);
                }
            }
        }
        6 => {
            let flap = (t * 7.0).sin() * 0.35;
            for side in [-1.0f32, 1.0] {
                for i in 0..n {
                    let f = i as f32 / (n - 1) as f32;
                    let ang = 0.3 + f * 1.4 + flap;
                    let dir = vec3(-0.35 - f * 0.6, ang.cos(), side * ang.sin());
                    let base = xf.p(vec3(-0.2, 0.0, side * 0.35));
                    let tip = xf.p(vec3(-0.2, 0.0, side * 0.35)
                        + dir.normalize() * (1.4 + 0.5 * (f * PI).sin()));
                    let flat = xf.n(vec3(0.0, -dir.z, dir.y).normalize_or(Vec3::Y));
                    o.glow(0).leaf(
                        base,
                        tip,
                        flat,
                        xf.n(Vec3::X),
                        0.16 * k,
                        mixc(c, hot, f),
                        1.0,
                    );
                }
            }
        }
        _ => {
            for i in 0..n {
                let start = around(i, 0.7, t * spin);
                let pts: Vec<Vec3> = (0..8)
                    .map(|j| {
                        let f = j as f32 / 7.0;
                        let w = (t * 6.0 - f * 5.0 + i as f32).sin() * 0.35 * f;
                        xf.p(start * (1.0 + f * 0.4) + vec3(-f * 3.0, w, w * 0.6))
                    })
                    .collect();
                let radii: Vec<f32> = (0..8).map(|j| 0.09 * k * (1.0 - j as f32 / 8.0)).collect();
                o.glow(HALO).tube(&pts, &radii, false, c, 1.0);
            }
        }
    }
}

/// The touch of an element on an ability's forms at `xf`.
fn accent(o: &mut Out, lk: &Look, xf: Xf, t: f32, a: f32, seed: u32) {
    let k = xf.size();
    let (c, hot) = (vivid(lk.c), mixc(vivid(lk.c), WHITE, 0.4));
    match lk.elem {
        Elem::Fire | Elem::Demon | Elem::Sun => {
            // tongues of flame streaming back, flickering
            for i in 0..5 {
                let ang = i as f32 * TAU / 5.0 + t * 3.0;
                let len = 1.4 + 0.9 * flick(seed + i, t, 14.0);
                let base = vec3(-0.3, ang.cos() * 0.45, ang.sin() * 0.45);
                let tip = base + vec3(-len, ang.cos() * 0.3, ang.sin() * 0.3);
                o.glow(HALO).spike(
                    xf.p(base),
                    xf.p(tip),
                    0.42 * k,
                    7,
                    ca(mixc(c, hot, 0.3), 0.55 * a),
                    1.0,
                );
            }
            if lk.elem == Elem::Demon {
                for side in [-1.0f32, 1.0] {
                    let pts = [
                        xf.p(vec3(0.2, 0.55, side * 0.4)),
                        xf.p(vec3(0.0, 1.1, side * 0.75)),
                        xf.p(vec3(-0.45, 1.45, side * 0.7)),
                    ];
                    o.thing(BONE, false).tube(
                        &pts,
                        &[0.16 * k, 0.1 * k, 0.0],
                        false,
                        Color::new(0.22, 0.08, 0.06, a),
                        0.2,
                    );
                }
            }
        }
        Elem::Storm => {
            for i in 0..3 {
                let s = seed.wrapping_add(i * 977) ^ (t * 18.0) as u32;
                let dir = vec3(hashf(s) - 0.5, hashf(s + 1) - 0.5, hashf(s + 2) - 0.5)
                    .normalize_or(Vec3::Y);
                let pts = zigzag(xf.p(dir * 0.5), xf.p(dir * 2.2), 0.18 * k, s, 4);
                bolt(o, &pts, 0.07 * k, c, a);
            }
        }
        Elem::Shadow | Elem::Void => {
            o.thing(0, true).ellipsoid(
                xf,
                Vec3::ZERO,
                Vec3::splat(0.62),
                Color::new(0.02, 0.0, 0.04, a.max(0.4)),
                0.0,
            );
            o.glow(RIM)
                .ellipsoid(xf, Vec3::ZERO, Vec3::splat(0.95), ca(c, 0.9 * a), 1.0);
        }
        Elem::Poison | Elem::Alchemy | Elem::Water | Elem::Blood => {
            for i in 0..4 {
                let ang = i as f32 * TAU / 4.0 + t * 1.3;
                let r = 1.1 + 0.25 * (t * 3.0 + i as f32).sin();
                let p = vec3(-0.4 - 0.3 * i as f32, ang.cos() * r, ang.sin() * r);
                o.glow(RIM).ellipsoid(
                    xf,
                    p,
                    Vec3::splat(0.2 + 0.05 * i as f32),
                    ca(hot, 0.8 * a),
                    1.0,
                );
            }
        }
        Elem::Holy | Elem::Star => {
            let turn = t * 1.2;
            for i in 0..4 {
                let ang = i as f32 * FRAC_PI_2 + turn;
                let dir = vec3(0.0, ang.cos(), ang.sin());
                o.glow(HALO).spike(
                    xf.p(dir * 0.4),
                    xf.p(dir * 2.3),
                    0.12 * k,
                    5,
                    ca(hot, 0.8 * a),
                    1.0,
                );
            }
        }
        Elem::Nature => {
            for i in 0..4 {
                let ang = i as f32 * TAU / 4.0 + t * 2.0;
                let p = vec3(-0.3, ang.cos() * 1.4, ang.sin() * 1.4);
                let tip = p + vec3(-0.6, ang.cos() * 0.3, ang.sin() * 0.3);
                o.thing(LEAVES, false).leaf(
                    xf.p(p),
                    xf.p(tip),
                    xf.n(vec3(0.0, -ang.sin(), ang.cos())),
                    xf.n(vec3(0.0, ang.cos(), ang.sin())),
                    0.25 * k,
                    Color::new(0.35, 0.65, 0.25, a),
                    0.0,
                );
            }
        }
        Elem::Frost => {
            for i in 0..3 {
                let ang = i as f32 * TAU / 3.0 - t * 1.6;
                let p = vec3(-0.2, ang.cos() * 1.35, ang.sin() * 1.35);
                o.thing(0, true).gem(
                    Xf {
                        o: xf.p(p),
                        m: xf.m * Mat3::from_rotation_x(ang),
                    },
                    Vec3::ZERO,
                    vec3(0.45, 0.16, 0.16),
                    0,
                    Color::new(0.8, 0.95, 1.0, 0.85 * a),
                    0.3,
                );
            }
        }
        Elem::Time => {
            let disc = Xf {
                o: xf.o,
                m: xf.m * Mat3::from_rotation_z(FRAC_PI_2),
            };
            o.glow(0)
                .torus(disc, Vec3::ZERO, 1.25, 0.05, ca(hot, 0.8 * a), 1.0);
            for (len, rate) in [(1.1, 4.0), (0.75, 0.6)] {
                let ang = t * rate;
                let tip = vec3(0.0, ang.cos() * len, ang.sin() * len);
                o.glow(0)
                    .capsule(xf.o, xf.p(tip), 0.06 * k, 0.03 * k, ca(hot, a), 1.0);
            }
        }
        Elem::Sound => {
            for i in 0..3 {
                let f = ((t * 1.5 + i as f32 / 3.0) % 1.0).max(0.01);
                let ring = Xf {
                    o: xf.p(vec3(-f * 1.5, 0.0, 0.0)),
                    m: xf.m * Mat3::from_rotation_z(FRAC_PI_2),
                };
                o.glow(HALO).torus(
                    ring,
                    Vec3::ZERO,
                    0.6 + f * 1.4,
                    0.05,
                    ca(c, (1.0 - f) * a),
                    1.0,
                );
            }
        }
        Elem::Wind | Elem::Smoke | Elem::Spirit => {
            let pts: Vec<Vec3> = (0..14)
                .map(|i| {
                    let f = i as f32 / 13.0;
                    let ang = f * 9.0 + t * 6.0;
                    xf.p(vec3(-f * 2.6, ang.cos() * (0.5 + f), ang.sin() * (0.5 + f)))
                })
                .collect();
            let radii: Vec<f32> = (0..14)
                .map(|i| 0.08 * k * (1.0 - i as f32 / 15.0))
                .collect();
            o.glow(HALO)
                .tube(&pts, &radii, false, ca(hot, 0.5 * a), 1.0);
        }
        Elem::Illusion | Elem::Arcane | Elem::Chi => {
            for i in 0..3 {
                let ang = t * (1.0 + i as f32 * 0.7) + i as f32;
                let ring = Xf {
                    o: xf.o,
                    m: xf.m * Mat3::from_rotation_y(ang) * Mat3::from_rotation_x(ang * 0.7),
                };
                let tint = if lk.elem == Elem::Illusion {
                    [
                        Color::new(1.0, 0.4, 0.6, 1.0),
                        Color::new(0.4, 0.9, 1.0, 1.0),
                        Color::new(0.9, 1.0, 0.4, 1.0),
                    ][i]
                } else {
                    hot
                };
                o.glow(0).torus(
                    ring,
                    Vec3::ZERO,
                    1.05 + i as f32 * 0.12,
                    0.035,
                    ca(tint, 0.7 * a),
                    1.0,
                );
            }
        }
        _ => {}
    }
}

/// An ability's forms whole: its heart, its crown and its element's touch.
fn sign(o: &mut Out, lk: &Look, xf: Xf, t: f32, a: f32, seed: u32) {
    heart(o, lk, xf, t, a);
    crown(o, lk, xf, t, a);
    accent(o, lk, xf, t, a, seed);
}

// ---- things that fly ----

/// An arrow along `d` at `o`, about `len` long: its head, its fletching and
/// the bands of colour on its shaft by the design.
fn arrow(o: &mut Out, lk: &Look, at: Vec3, d: Vec3, len: f32, a: f32, magic: bool) {
    let xf = along(at, d, len, 0.0);
    let k = xf.size();
    let dz = lk.d;
    let wood = Color::new(0.55, 0.38, 0.22, a);
    let steel = Color::new(0.82, 0.84, 0.9, a);
    o.thing(WOOD, false).capsule(
        xf.p(vec3(-0.5, 0.0, 0.0)),
        xf.p(vec3(0.34, 0.0, 0.0)),
        0.022 * k,
        0.022 * k,
        wood,
        0.0,
    );
    // the head
    let side = xf.n(Vec3::Z);
    let (base, tip) = (xf.p(vec3(0.32, 0.0, 0.0)), xf.p(vec3(0.58, 0.0, 0.0)));
    match dz.core % 4 {
        0 => o
            .thing(METAL, true)
            .blade(base, tip, side, 0.055 * k, 0.012 * k, steel),
        1 => o
            .thing(METAL, true)
            .spike(base, tip, 0.03 * k, 5, steel, 0.0),
        2 => o
            .thing(METAL, true)
            .blade(base, tip, side, 0.09 * k, 0.012 * k, steel),
        _ => {
            o.thing(METAL, true)
                .spike(base, tip, 0.032 * k, 5, steel, 0.0);
            for s in [-1.0f32, 1.0] {
                o.thing(METAL, true).spike(
                    xf.p(vec3(0.42, 0.0, 0.0)),
                    xf.p(vec3(0.34, 0.0, s * 0.08)),
                    0.014 * k,
                    4,
                    steel,
                    0.0,
                );
            }
        }
    }
    // the fletching in the ability's colour, cut by its crown
    let fl = 3 + (dz.core / 4) as usize;
    let feather = Color::new(lk.c.r, lk.c.g, lk.c.b, a);
    let (fw, flen) = (
        0.035 + 0.01 * (dz.crown % 4) as f32,
        0.2 + 0.05 * (dz.crown / 4) as f32,
    );
    for i in 0..fl {
        let ang = i as f32 * TAU / fl as f32;
        let out = vec3(0.0, ang.cos(), ang.sin());
        o.thing(FUR, false).leaf(
            xf.p(vec3(-0.5, 0.0, 0.0) + out * 0.02),
            xf.p(vec3(-0.5 + flen, 0.0, 0.0) + out * 0.03),
            xf.n(out),
            xf.n(vec3(0.0, -ang.sin(), ang.cos())),
            fw * k,
            feather,
            0.0,
        );
    }
    // bands round the shaft
    for i in 0..(dz.n as usize).saturating_sub(2) {
        let x = -0.2 + i as f32 * 0.05;
        let ring = Xf {
            o: xf.p(vec3(x, 0.0, 0.0)),
            m: xf.m * Mat3::from_rotation_z(FRAC_PI_2),
        };
        o.thing(0, false).torus(
            ring,
            Vec3::ZERO,
            0.026,
            0.008,
            mixc(feather, BLACK, 0.2 * i as f32),
            0.2,
        );
    }
    if magic {
        // a charmed arrow: its tip burns in its colour
        o.glow(HALO).ellipsoid(
            Xf::at(tip),
            Vec3::ZERO,
            Vec3::splat(0.08 * k),
            ca(lk.hot, 0.9 * a),
            1.0,
        );
        let trail = along(at, d, len * 0.35, 0.0);
        crown(o, lk, trail, 0.0, 0.5 * a);
    }
}

/// A thrown weapon spinning flat on its way.
fn thrown(o: &mut Out, lk: &Look, at: Vec3, d: Vec3, r: f32, t: f32, a: f32) {
    let dz = lk.d;
    let spin = t * 16.0 * dz.spin.signum() as f32;
    let flat = Xf {
        o: at,
        m: Mat3::from_rotation_y(spin) * Mat3::from_diagonal(Vec3::splat(r)),
    };
    let steel = Color::new(0.84, 0.86, 0.92, a);
    let k = flat.size();
    match dz.core % 4 {
        0 => {
            // a dagger
            o.thing(METAL, true).blade(
                flat.p(vec3(0.0, 0.0, 0.0)),
                flat.p(vec3(1.0, 0.0, 0.0)),
                flat.n(Vec3::Y),
                0.12 * k,
                0.02 * k,
                steel,
            );
            o.thing(METAL, true).capsule(
                flat.p(vec3(-0.05, 0.0, -0.18)),
                flat.p(vec3(-0.05, 0.0, 0.18)),
                0.04 * k,
                0.04 * k,
                Color::new(0.8, 0.65, 0.25, a),
                0.0,
            );
            o.thing(0, false).capsule(
                flat.p(vec3(-0.05, 0.0, 0.0)),
                flat.p(vec3(-0.45, 0.0, 0.0)),
                0.045 * k,
                0.04 * k,
                mixc(lk.c, Color::new(0.3, 0.2, 0.15, 1.0), 0.6),
                0.0,
            );
        }
        1 => {
            // a throwing axe
            o.thing(WOOD, false).capsule(
                flat.p(vec3(-0.7, 0.0, 0.0)),
                flat.p(vec3(0.5, 0.0, 0.0)),
                0.05 * k,
                0.05 * k,
                Color::new(0.5, 0.34, 0.2, a),
                0.0,
            );
            let head = Xf {
                o: flat.p(vec3(0.4, 0.0, 0.0)),
                m: flat.m * Mat3::from_cols(Vec3::Z, Vec3::X, Vec3::Y),
            };
            o.thing(METAL, true).slab(
                head,
                &[
                    vec2(-0.12, -0.05),
                    vec2(0.15, -0.25),
                    vec2(0.42, -0.1),
                    vec2(0.42, 0.1),
                    vec2(0.15, 0.25),
                    vec2(-0.12, 0.05),
                ],
                0.05,
                steel,
                0.0,
            );
        }
        2 => {
            // a star of blades
            let n = dz.n.max(3) as usize;
            let outline: Vec<Vec2> = (0..n * 2)
                .map(|i| {
                    let ang = i as f32 * PI / n as f32;
                    let rr = if i % 2 == 0 { 1.0 } else { 0.32 };
                    vec2(ang.cos() * rr, ang.sin() * rr)
                })
                .collect();
            let disc = Xf {
                o: at,
                m: flat.m * Mat3::from_rotation_x(FRAC_PI_2),
            };
            // (a star is not convex: two halves of it, each a fan)
            for half in 0..n {
                let pts = [
                    Vec2::ZERO,
                    outline[(half * 2 + outline.len() - 1) % outline.len()],
                    outline[half * 2],
                    outline[(half * 2 + 1) % outline.len()],
                ];
                o.thing(METAL, true).slab(disc, &pts, 0.06, steel, 0.0);
            }
            o.thing(0, false)
                .ellipsoid(flat, Vec3::ZERO, vec3(0.2, 0.08, 0.2), ca(lk.c, a), 0.2);
        }
        _ => {
            // a ring of steel with spikes round it
            o.thing(METAL, true)
                .torus(flat, Vec3::ZERO, 0.8, 0.09, steel, 0.0);
            let n = dz.n.max(3) as usize;
            for i in 0..n {
                let ang = i as f32 * TAU / n as f32;
                let dir = vec3(ang.cos(), 0.0, ang.sin());
                o.thing(METAL, true).spike(
                    flat.p(dir * 0.8),
                    flat.p(dir * 1.25),
                    0.08 * k,
                    4,
                    steel,
                    0.0,
                );
            }
        }
    }
    let trail = along(at, d, r * 0.6, 0.0);
    crown(o, lk, trail, t, 0.4 * a);
}

/// A flask tumbling through the air, its potion glowing.
fn flask(o: &mut Out, lk: &Look, xf: Xf, a: f32) {
    let glass = Color::new(0.75, 0.88, 1.0, 0.45 * a);
    o.thing(0, true).lathe(
        xf,
        vec3(0.0, -0.5, 0.0),
        &[
            (0.0, 0.0, 0.0),
            (0.05, 0.35, 0.35),
            (0.35, 0.45, 0.45),
            (0.62, 0.3, 0.3),
            (0.8, 0.12, 0.12),
            (1.0, 0.12, 0.12),
        ],
        10,
        true,
        glass,
        0.0,
    );
    o.glow(HALO).ellipsoid(
        xf,
        vec3(0.0, -0.18, 0.0),
        vec3(0.36, 0.3, 0.36),
        ca(lk.c, 0.85 * a),
        1.0,
    );
    o.thing(WOOD, false).lathe(
        xf,
        vec3(0.0, 0.48, 0.0),
        &[(0.0, 0.13, 0.13), (0.14, 0.11, 0.11)],
        8,
        true,
        Color::new(0.55, 0.38, 0.22, a),
        0.0,
    );
}

/// A hammer of light and steel turning over and over.
fn hammer(o: &mut Out, lk: &Look, xf: Xf, a: f32) {
    let steel = Color::new(0.85, 0.86, 0.9, a);
    o.thing(WOOD, false).capsule(
        xf.p(vec3(0.0, -0.9, 0.0)),
        xf.p(vec3(0.0, 0.4, 0.0)),
        0.07 * xf.size(),
        0.07 * xf.size(),
        Color::new(0.5, 0.34, 0.2, a),
        0.0,
    );
    o.thing(METAL, true)
        .cuboid(xf, vec3(0.0, 0.5, 0.0), vec3(0.42, 0.22, 0.22), steel, 0.0);
    o.thing(METAL, true).cuboid(
        xf,
        vec3(0.0, 0.5, 0.0),
        vec3(0.12, 0.26, 0.26),
        Color::new(0.95, 0.8, 0.35, a),
        0.0,
    );
    o.glow(HALO).ellipsoid(
        xf,
        vec3(0.0, 0.5, 0.0),
        vec3(0.8, 0.5, 0.5),
        ca(lk.c, 0.35 * a),
        1.0,
    );
}

/// A web spun of light threads, turning as it flies.
fn web(o: &mut Out, lk: &Look, xf: Xf, a: f32) {
    let silk = ca(mixc(WHITE, lk.c, 0.2), 0.75 * a);
    let k = xf.size();
    let spokes = 8;
    for i in 0..spokes {
        let ang = i as f32 * TAU / spokes as f32;
        let end = vec3(0.0, ang.cos(), ang.sin());
        o.glow(0)
            .capsule(xf.o, xf.p(end), 0.02 * k, 0.012 * k, silk, 1.0);
    }
    for r in [0.35f32, 0.65, 0.95] {
        let ring = Xf {
            o: xf.o,
            m: xf.m * Mat3::from_rotation_z(FRAC_PI_2),
        };
        o.glow(0).torus(ring, Vec3::ZERO, r, 0.012, silk, 1.0);
    }
}

/// A rock flying, rough and turning; a meteor burns with it.
fn rock(o: &mut Out, lk: &Look, at: Vec3, d: Vec3, r: f32, t: f32, a: f32, burning: bool) {
    let tumble = Xf {
        o: at,
        m: Mat3::from_rotation_x(t * 4.0) * Mat3::from_rotation_z(t * 2.6),
    };
    let stone = if burning {
        Color::new(0.3, 0.22, 0.18, a)
    } else {
        Color::new(
            0.45 + lk.c.r * 0.2,
            0.4 + lk.c.g * 0.2,
            0.36 + lk.c.b * 0.2,
            a,
        )
    };
    o.thing(STONE, false).stone(
        tumble,
        Vec3::ZERO,
        Vec3::splat(r),
        lk.d.n as u32 + 3,
        stone,
        if burning { 0.25 } else { 0.0 },
    );
    if burning {
        o.glow(HALO).ellipsoid(
            Xf::at(at),
            Vec3::ZERO,
            Vec3::splat(r * 1.35),
            ca(lk.c, 0.45 * a),
            1.0,
        );
        let tail = along(at, d, r, 0.0);
        accent(o, lk, tail, t, a, lk.d.n as u32 * 13);
    }
}

/// A missile of an ability at `at`, flying along `d`.
fn missile(o: &mut Out, lk: &Look, at: Vec3, d: Vec3, t: f32, a: f32, seed: u32) {
    let big = lk.big;
    match (lk.sig, lk.elem) {
        (Sig::Meteor | Sig::Flask | Sig::Hammer | Sig::Web, _) => {
            match lk.sig {
                Sig::Meteor => rock(o, lk, at, d, 0.3 * big, t, a, true),
                Sig::Flask => flask(o, lk, along(at, d, 0.28, t * 9.0), a),
                Sig::Hammer => hammer(o, lk, along(at, d, 0.4, -t * 14.0), a),
                _ => web(o, lk, along(at, d, 0.45, t * 3.0), a),
            }
            // and its own crown about it
            crown(o, lk, along(at, d, 0.22 * big, 0.0), t, 0.6 * a);
        }
        (Sig::Lance, _) => {
            // a spear of light
            let xf = along(at, d, 0.5, 0.0);
            o.glow(HALO).lathe(
                along(at, d, 1.0, 0.0).then(Vec3::ZERO, Mat3::from_rotation_z(-FRAC_PI_2)),
                vec3(0.0, -1.0, 0.0),
                &[
                    (0.0, 0.03, 0.03),
                    (1.0, 0.07, 0.07),
                    (1.25, 0.14, 0.14),
                    (1.6, 0.0, 0.0),
                ],
                10,
                false,
                ca(lk.c, 0.8 * a),
                1.0,
            );
            sign(o, lk, xf, t, 0.7 * a, seed);
        }
        (_, Elem::Arrow) | (Sig::Rain, _) => {
            let magic = lk.c.r + lk.c.g + lk.c.b < 2.4;
            arrow(o, lk, at, d, 0.9, a, magic);
        }
        (_, Elem::Steel) => thrown(o, lk, at, d, 0.3, t, a),
        (_, Elem::Earth) => {
            rock(o, lk, at, d, 0.24 * big, t, a, false);
            crown(o, lk, along(at, d, 0.2 * big, 0.0), t, 0.7 * a);
        }
        _ => {
            let xf = along(at, d, 0.2 * big, 0.0);
            sign(o, lk, xf, t, a, seed);
        }
    }
}

// ---- the little things scattered ----

/// A snowflake: six arms with little branches.
fn snowflake(b: &mut Batch, xf: Xf, c: Color) {
    let k = xf.size();
    for i in 0..6 {
        let ang = i as f32 * TAU / 6.0;
        let dir = vec3(ang.cos(), ang.sin(), 0.0);
        b.capsule(xf.o, xf.p(dir), 0.07 * k, 0.04 * k, c, 1.0);
        for s in [-1.0f32, 1.0] {
            let br = vec3((ang + s * 0.7).cos(), (ang + s * 0.7).sin(), 0.0);
            b.capsule(
                xf.p(dir * 0.55),
                xf.p(dir * 0.55 + br * 0.3),
                0.04 * k,
                0.02 * k,
                c,
                1.0,
            );
        }
    }
}

/// A star of five points, a little thick.
fn star5(b: &mut Batch, xf: Xf, c: Color, glow: f32) {
    let outline: Vec<Vec2> = (0..10)
        .map(|i| {
            let ang = FRAC_PI_2 + i as f32 * PI / 5.0;
            let r = if i % 2 == 0 { 1.0 } else { 0.42 };
            vec2(ang.cos() * r, ang.sin() * r)
        })
        .collect();
    for i in 0..5 {
        let pts = [
            Vec2::ZERO,
            outline[(i * 2 + 9) % 10],
            outline[i * 2],
            outline[i * 2 + 1],
        ];
        b.slab(xf, &pts, 0.3, c, glow);
    }
}

/// A little skull.
fn skull(b: &mut Batch, xf: Xf, c: Color) {
    b.ellipsoid(xf, vec3(0.0, 0.1, 0.0), vec3(0.75, 0.7, 0.7), c, 0.0);
    b.ellipsoid(xf, vec3(0.25, -0.35, 0.0), vec3(0.4, 0.3, 0.45), c, 0.0);
    let hole = Color::new(0.08, 0.05, 0.08, c.a);
    for s in [-1.0f32, 1.0] {
        b.ellipsoid(
            xf,
            vec3(0.62, 0.05, s * 0.28),
            vec3(0.15, 0.17, 0.15),
            hole,
            0.0,
        );
    }
}

/// A little bone.
fn bone(b: &mut Batch, xf: Xf, c: Color) {
    let k = xf.size();
    b.capsule(
        xf.p(vec3(-0.7, 0.0, 0.0)),
        xf.p(vec3(0.7, 0.0, 0.0)),
        0.16 * k,
        0.16 * k,
        c,
        0.0,
    );
    for x in [-0.8f32, 0.8] {
        for z in [-0.18f32, 0.18] {
            b.ellipsoid(xf, vec3(x, 0.0, z), Vec3::splat(0.22), c, 0.0);
        }
    }
}

/// A note of music.
fn note(b: &mut Batch, xf: Xf, c: Color) {
    let k = xf.size();
    b.ellipsoid(xf, vec3(-0.3, -0.7, 0.0), vec3(0.32, 0.24, 0.24), c, 1.0);
    b.capsule(
        xf.p(vec3(-0.02, -0.65, 0.0)),
        xf.p(vec3(-0.02, 0.75, 0.0)),
        0.06 * k,
        0.06 * k,
        c,
        1.0,
    );
    b.capsule(
        xf.p(vec3(-0.02, 0.75, 0.0)),
        xf.p(vec3(0.45, 0.3, 0.0)),
        0.07 * k,
        0.05 * k,
        c,
        1.0,
    );
}

/// A letter Z (of sleep).
fn zed(b: &mut Batch, xf: Xf, c: Color) {
    let k = xf.size();
    let pts = [
        vec3(-0.6, 0.6, 0.0),
        vec3(0.6, 0.6, 0.0),
        vec3(-0.6, -0.6, 0.0),
        vec3(0.6, -0.6, 0.0),
    ];
    for w in pts.windows(2) {
        b.capsule(xf.p(w[0]), xf.p(w[1]), 0.1 * k, 0.1 * k, c, 1.0);
    }
}

/// A rune tablet with a sign scratched in light.
fn tablet(o: &mut Out, xf: Xf, c: Color, hot: Color, seed: u32) {
    let k = xf.size();
    o.thing(STONE, false).cuboid(
        xf,
        Vec3::ZERO,
        vec3(0.55, 0.8, 0.12),
        Color::new(0.32, 0.3, 0.34, c.a.max(0.3)),
        0.0,
    );
    // three strokes of the rune
    for i in 0..3u32 {
        let s = seed.wrapping_add(i * 71);
        let a = vec3(hashf(s) - 0.5, hashf(s + 1) - 0.5, 0.0) * vec3(0.7, 1.1, 0.0);
        let b = vec3(hashf(s + 2) - 0.5, hashf(s + 3) - 0.5, 0.0) * vec3(0.7, 1.1, 0.0);
        let z = vec3(0.0, 0.0, 0.14);
        o.glow(0)
            .capsule(xf.p(a + z), xf.p(b + z), 0.06 * k, 0.06 * k, hot, 1.0);
        o.glow(0)
            .capsule(xf.p(a - z), xf.p(b - z), 0.06 * k, 0.06 * k, hot, 1.0);
    }
    o.glow(HALO)
        .ellipsoid(xf, Vec3::ZERO, vec3(0.9, 1.1, 0.5), ca(c, 0.3), 1.0);
}

impl Spells {
    /// Builds the abilities' forms of this frame.
    pub fn forms3d(
        &self,
        o: &mut Out,
        cam: &Cam,
        ents: &HashMap<u32, EntState>,
        height: &dyn Fn(f32, f32) -> f32,
    ) {
        // missiles
        for (id, st) in ents {
            if st.view.kind != KIND_PROJECTILE || st.alpha < 0.02 {
                continue;
            }
            let Some(pj) = self.proj.get(id) else {
                continue;
            };
            let d = if st.vx.abs() + st.vy.abs() > 0.01 {
                vec3(st.vx, 0.0, st.vy)
            } else {
                vec3(st.facing.cos(), 0.0, st.facing.sin())
            };
            let at = vec3(st.x, height(st.x, st.y) + CHEST, st.y);
            missile(o, &pj.lk, at, d, pj.age, st.alpha, *id);
        }
        for s in &self.shapes {
            if s.delay <= 0.0 {
                self.shape3d(o, cam, s, height);
            }
        }
        // the forms of abilities on show: a missile, or a sign over it
        let cols = 6;
        for (i, key) in self.gallery.iter().enumerate() {
            let lk = look(
                key,
                &content::db()
                    .ability(key)
                    .map_or("#ffffff".into(), |a| a.color.clone()),
            );
            let (cx, cy) = (
                self.cam.0 + ((i % cols) as f32 - (cols as f32 - 1.0) / 2.0) * 1.7,
                self.cam.1 + ((i / cols) as f32 - 1.5) * 1.7,
            );
            let at = vec3(cx, height(cx, cy) + CHEST + 0.2, cy);
            if lk.kind == Kind::Projectile {
                missile(o, &lk, at, Vec3::X, self.t, 1.0, i as u32);
            } else {
                sign(o, &lk, upright(at, self.t, 0.3), self.t, 1.0, i as u32);
            }
        }
        for p in &self.parts {
            if p.delay <= 0.0 {
                Self::part3d(o, p, height);
            }
        }
    }

    /// Whether a particle is drawn as a body (else as a picture).
    pub(super) fn bodily(spr: Spr) -> bool {
        !matches!(spr, Spr::Dot | Spr::Streak | Spr::Spark | Spr::Liquid)
    }

    /// One of the little things an ability scatters, as a body.
    fn part3d(o: &mut Out, p: &P, height: &dyn Fn(f32, f32) -> f32) {
        if !Self::bodily(p.spr) {
            return;
        }
        let u = (p.t / p.life).clamp(0.0, 1.0);
        let mut a = p.alpha
            * if p.fin > 0.0 {
                (u / p.fin).min(1.0)
            } else {
                1.0
            };
        a *= 1.0 - u * u * u;
        if a <= 0.02 {
            return;
        }
        let c = mixc(p.c0, p.c1, u);
        let s = p.s0 + (p.s1 - p.s0) * u;
        let wx = if p.wob != 0.0 {
            (p.t * 6.0 + p.rot * 3.0 + p.ax).sin() * p.wob
        } else {
            0.0
        };
        let (x, y) = (p.ax + p.x + wx, p.ay + p.y);
        let at = vec3(x, height(p.ax, p.ay) + p.z.max(0.0), y);
        // the size of the 2D picture, in tiles: about half of it across
        let r = s * 0.2;
        let flight = vec3(p.vx, p.vz, p.vy);
        let turned = if p.face && flight.length() > 0.01 {
            along(at, flight, r, p.rot)
        } else {
            Xf {
                o: at,
                m: Mat3::from_rotation_y(p.rot * 0.7 + p.ax)
                    * Mat3::from_rotation_z(p.rot)
                    * Mat3::from_diagonal(Vec3::splat(r)),
            }
        };
        let glowing = p.add;
        match p.spr {
            Spr::Snow => {
                let b = if glowing {
                    o.glow(HALO)
                } else {
                    o.thing(0, true)
                };
                snowflake(b, turned, ca(c, a));
            }
            Spr::Shard => {
                let xf = turned.then(Vec3::ZERO, Mat3::from_rotation_z(FRAC_PI_2));
                if glowing {
                    o.glow(HALO)
                        .gem(xf, Vec3::ZERO, vec3(1.2, 0.4, 0.4), 0, ca(c, a), 1.0);
                } else {
                    o.thing(0, true).gem(
                        xf,
                        Vec3::ZERO,
                        vec3(1.2, 0.4, 0.4),
                        0,
                        ca(c, a.max(0.5) * 0.9),
                        0.3,
                    );
                }
            }
            Spr::Star => star5(o.glow(HALO), turned, ca(c, a), 1.0),
            Spr::Cross => {
                let b = o.glow(HALO);
                b.cuboid(turned, Vec3::ZERO, vec3(0.16, 0.9, 0.16), ca(c, a), 1.0);
                b.cuboid(
                    turned,
                    vec3(0.0, 0.3, 0.0),
                    vec3(0.55, 0.16, 0.16),
                    ca(c, a),
                    1.0,
                );
            }
            Spr::Leaf => {
                let (base, tip) = (
                    turned.p(vec3(-0.9, 0.0, 0.0)),
                    turned.p(vec3(0.9, 0.0, 0.0)),
                );
                let b = if glowing {
                    o.glow(0)
                } else {
                    o.thing(LEAVES, false)
                };
                b.leaf(
                    base,
                    tip,
                    turned.n(Vec3::Z),
                    turned.n(Vec3::Y),
                    0.45 * r,
                    ca(c, a),
                    0.0,
                );
            }
            Spr::Note => note(o.glow(HALO), turned, ca(c, a)),
            Spr::Coin => {
                let spin = Xf {
                    o: at,
                    m: Mat3::from_rotation_y(p.rot * 3.0)
                        * Mat3::from_rotation_x(FRAC_PI_2)
                        * Mat3::from_diagonal(Vec3::splat(r)),
                };
                o.thing(METAL, true).lathe(
                    spin,
                    vec3(0.0, -0.1, 0.0),
                    &[(0.0, 0.8, 0.8), (0.2, 0.8, 0.8)],
                    12,
                    true,
                    Color::new(1.0, 0.8, 0.3, a),
                    0.15,
                );
            }
            Spr::Bone => bone(
                o.thing(BONE, false),
                turned,
                Color::new(0.93, 0.88, 0.76, a),
            ),
            Spr::Skull => skull(
                o.thing(BONE, false),
                turned,
                Color::new(0.93, 0.88, 0.76, a),
            ),
            Spr::Drop => {
                let xf = Xf {
                    o: at,
                    m: Mat3::from_diagonal(Vec3::splat(r)),
                };
                let b = if glowing {
                    o.glow(HALO)
                } else {
                    o.thing(0, true)
                };
                b.lathe(
                    xf,
                    vec3(0.0, -0.5, 0.0),
                    &[
                        (0.0, 0.0, 0.0),
                        (0.15, 0.45, 0.45),
                        (0.45, 0.55, 0.55),
                        (0.85, 0.3, 0.3),
                        (1.4, 0.0, 0.0),
                    ],
                    8,
                    false,
                    ca(c, a * 0.9),
                    0.2,
                );
            }
            Spr::Bubble => {
                o.glow(RIM)
                    .ellipsoid(Xf::at(at), Vec3::ZERO, Vec3::splat(r * 0.8), ca(c, a), 1.0);
            }
            Spr::Rock => {
                o.thing(STONE, false).stone(
                    turned,
                    Vec3::ZERO,
                    vec3(0.7, 0.55, 0.65),
                    (p.ax * 13.0) as u32 | 1,
                    ca(c, a.max(0.6)),
                    0.0,
                );
            }
            Spr::Zed => zed(o.glow(HALO), turned, ca(c, a)),
            Spr::Arrow => {
                let lk = Look {
                    elem: Elem::Arrow,
                    sig: Sig::None,
                    kind: Kind::Projectile,
                    c: Color::new(0.85, 0.3, 0.25, 1.0),
                    hot: WHITE,
                    deep: BLACK,
                    pointy: true,
                    cross: false,
                    big: 1.0,
                    d: Design::default(),
                };
                let d = if flight.length() > 0.01 {
                    flight
                } else {
                    // stuck in the ground, leaning
                    vec3(p.rot.cos() * 0.4, -1.0, p.rot.sin() * 0.4)
                };
                arrow(o, &lk, at, d, s * 0.65, a, false);
            }
            Spr::Flask => flask(
                o,
                &Look {
                    c,
                    hot: c,
                    ..look_blood()
                },
                turned,
                a,
            ),
            Spr::Hammer => hammer(o, &look_blood(), turned, a),
            Spr::Web => web(o, &look_blood(), turned, a),
            Spr::Rune => {
                let seed = (p.ax * 31.0 + p.ay * 17.0) as u32;
                tablet(o, turned, ca(c, a), ca(c, a), seed)
            }
            Spr::Dot | Spr::Streak | Spr::Spark | Spr::Liquid => {}
        }
    }

    /// A shape of an ability as bodies of light (and of things).
    fn shape3d(&self, o: &mut Out, cam: &Cam, s: &Sh, height: &dyn Fn(f32, f32) -> f32) {
        let u = (s.t / s.life).clamp(0.0, 1.0);
        let (ax, ay) = (s.ax, s.ay);
        let gh = height(ax, ay);
        let base = vec3(ax, gh, ay);
        let (c, c2) = (s.c, s.c2);
        let lk = s.lk.unwrap_or_else(look_blood);
        let hot = lk.hot;
        let t = s.t;
        match s.k {
            ShK::Glow => {}
            ShK::Ring | ShK::Implode => {
                let (r, a) = if s.k == ShK::Ring {
                    let grow = if s.follow != 0 { 1.0 } else { u.sqrt() };
                    (
                        (s.r * (0.12 + 0.88 * grow)).max(0.05),
                        if s.follow != 0 {
                            env(u, 0.15, 0.3)
                        } else {
                            1.0 - u
                        },
                    )
                } else {
                    (s.r * (1.0 - u * 0.9), env(u, 0.1, 0.25))
                };
                let lift = if s.follow != 0 { 0.08 } else { 0.06 };
                let ring = Xf::at(base + vec3(0.0, lift, 0.0));
                let w = (s.w * 0.6).max(0.02) * (1.0 - u * 0.5);
                o.glow(HALO)
                    .torus(ring, Vec3::ZERO, r, w, ca(c, 0.8 * a), 1.0);
                o.glow(0)
                    .torus(ring, Vec3::ZERO, r * 0.98, w * 0.3, ca(WHITE, 0.6 * a), 1.0);
                if s.k == ShK::Ring && s.follow == 0 && s.r > 1.0 {
                    self.wave_riders(o, &lk, base, r, a, t, s.seed);
                }
                if s.k == ShK::Implode {
                    for i in 0..8 {
                        let ang = i as f32 * TAU / 8.0 + t;
                        let dir = vec3(ang.cos(), 0.0, ang.sin());
                        let p = base + vec3(0.0, 0.5, 0.0) + dir * r;
                        o.glow(HALO).capsule(
                            p,
                            p - dir * r * 0.35,
                            0.04,
                            0.01,
                            ca(c, 0.7 * a),
                            1.0,
                        );
                    }
                }
            }
            ShK::Shock => {
                let r = s.r * (0.15 + 0.85 * u.sqrt());
                let a = 1.0 - u;
                let ring = Xf::at(base + vec3(0.0, 0.05, 0.0));
                o.glow(HALO)
                    .torus(ring, Vec3::ZERO, r, 0.12 * a + 0.02, ca(c, 0.6 * a), 1.0);
                o.glow(HALO).torus(
                    ring,
                    vec3(0.0, 0.08, 0.0),
                    r * 0.92,
                    0.2 * a,
                    Color::new(0.55, 0.47, 0.38, 0.35 * a),
                    1.0,
                );
                // clods thrown up by the blow
                let n = 10;
                for i in 0..n {
                    let s2 = s.seed.wrapping_add(i * 37);
                    let ang = i as f32 * TAU / n as f32 + hashf(s2) * 0.5;
                    let d = r * (0.6 + 0.4 * hashf(s2 + 1));
                    let up = (u * PI).sin() * (0.25 + 0.4 * hashf(s2 + 2));
                    let p = base + vec3(ang.cos() * d, up, ang.sin() * d);
                    o.thing(STONE, false).stone(
                        Xf {
                            o: p,
                            m: Mat3::from_rotation_x(u * 8.0 + i as f32),
                        },
                        Vec3::ZERO,
                        Vec3::splat(0.05 + 0.05 * hashf(s2 + 3)),
                        s2 | 1,
                        Color::new(0.45, 0.4, 0.34, 1.0),
                        0.0,
                    );
                }
            }
            ShK::Runes => {
                let e = env(u, 0.15, 0.35);
                let r = s.r * (0.85 + 0.15 * (u / 0.15).min(1.0));
                let rot = s.ang + t * 1.4 * lk.d.spin.signum() as f32;
                let ring = Xf::at(base + vec3(0.0, 0.05, 0.0));
                o.glow(HALO)
                    .torus(ring, Vec3::ZERO, r, 0.04, ca(c, 0.9 * e), 1.0);
                o.glow(HALO)
                    .torus(ring, Vec3::ZERO, r * 0.78, 0.025, ca(c, 0.8 * e), 1.0);
                // a hexagram lying in it, turning the other way
                let pts: Vec<Vec3> = (0..6)
                    .map(|i| {
                        let ang = -rot * 0.7 + i as f32 * TAU / 6.0;
                        base + vec3(ang.cos() * r * 0.78, 0.06, ang.sin() * r * 0.78)
                    })
                    .collect();
                for i in 0..6 {
                    o.glow(0)
                        .capsule(pts[i], pts[(i + 2) % 6], 0.015, 0.015, ca(c2, 0.6 * e), 1.0);
                }
                // the ability's own signs standing round it
                let n = lk.d.n.max(3) as usize;
                for i in 0..n {
                    let ang = rot + i as f32 * TAU / n as f32;
                    let p = base
                        + vec3(
                            ang.cos() * r * 0.9,
                            0.22 + 0.04 * (t * 3.0 + i as f32).sin(),
                            ang.sin() * r * 0.9,
                        );
                    let small = upright(p, -ang, 0.11 * (r / 0.6).clamp(0.6, 1.5));
                    heart(o, &lk, small, t, 0.9 * e);
                }
            }
            ShK::Pillar => {
                let e = env(u, 0.08, 0.55);
                let h = s.r;
                let w = s.w * (1.0 - 0.45 * u) * 0.5;
                let fall = (u / 0.12).min(1.0);
                let top = base + vec3(0.0, h, 0.0);
                let bottom = top.lerp(base, fall);
                let col = Xf::at(bottom);
                let len = (top.y - bottom.y).max(0.05);
                o.glow(HALO).lathe(
                    col,
                    Vec3::ZERO,
                    &[(0.0, w, w), (len, w * 0.6, w * 0.6)],
                    14,
                    false,
                    ca(c, 0.55 * e),
                    1.0,
                );
                o.glow(HALO).lathe(
                    col,
                    Vec3::ZERO,
                    &[(0.0, w * 0.35, w * 0.35), (len, w * 0.2, w * 0.2)],
                    10,
                    false,
                    ca(WHITE, 0.8 * e),
                    1.0,
                );
                // ribbons of light winding up it
                let n = (lk.d.n as usize).clamp(2, 5);
                for i in 0..n {
                    let pts: Vec<Vec3> = (0..16)
                        .map(|j| {
                            let f = j as f32 / 15.0;
                            let ang = f * 7.0 + t * 4.0 + i as f32 * TAU / n as f32;
                            bottom + vec3(ang.cos() * w * 1.4, f * len, ang.sin() * w * 1.4)
                        })
                        .collect();
                    let radii = vec![0.025; 16];
                    o.glow(HALO)
                        .tube(&pts, &radii, false, ca(hot, 0.6 * e), 1.0);
                }
                if fall >= 1.0 {
                    o.glow(HALO).ellipsoid(
                        Xf::at(base),
                        Vec3::ZERO,
                        vec3(w * 3.0, w * 0.6, w * 3.0),
                        ca(c, 0.6 * e),
                        1.0,
                    );
                }
            }
            ShK::Bolt => {
                let a = (1.0 - u) * (0.7 + 0.3 * hash(s.seed ^ (s.t * 22.0) as u32));
                let end = vec3(ax + s.x2, 0.0, ay + s.y2);
                let gend = height(end.x, end.z);
                // a bolt from far up the screen falls from the sky
                let (from, to) = if s.y2 > 3.0 && s.x2.abs() < 2.5 {
                    (
                        vec3(end.x + s.x2 * 0.2, gend + s.y2, end.z - 1.0),
                        vec3(end.x, gend, end.z),
                    )
                } else {
                    (
                        base + vec3(0.0, CHEST, 0.0),
                        vec3(end.x, gend + CHEST, end.z),
                    )
                };
                let seed = s.seed ^ ((s.t / 0.05) as u32).wrapping_mul(97);
                let n = ((from.distance(to) / 0.4) as usize).clamp(4, 24);
                let pts = zigzag(from, to, 0.3, seed, n);
                bolt(o, &pts, 0.09, c, a);
                let m = pts[pts.len() / 2];
                let b = (to - from).normalize_or(Vec3::Y);
                let side = b.cross(Vec3::Y).normalize_or(Vec3::X);
                let branch = m + (side * (hashf(seed + 9) - 0.5) * 2.0 + b * 0.5) * 0.8;
                bolt(o, &zigzag(m, branch, 0.12, seed + 5, 4), 0.05, hot, 0.8 * a);
                o.glow(HALO).ellipsoid(
                    Xf::at(to),
                    Vec3::ZERO,
                    Vec3::splat(0.35),
                    ca(c, 0.6 * a),
                    1.0,
                );
            }
            ShK::Ray | ShK::Wave | ShK::Ribbon => {
                let a = 1.0 - u;
                let end = vec3(ax + s.x2, 0.0, ay + s.y2);
                let from = base + vec3(0.0, CHEST, 0.0);
                let to = vec3(end.x, height(end.x, end.z) + CHEST, end.z);
                let len = from.distance(to).max(0.01);
                let n = ((len / 0.2) as usize).clamp(4, 60);
                let dir = (to - from) / len;
                let side = dir.cross(Vec3::Y).normalize_or(Vec3::X);
                let pts: Vec<Vec3> = (0..=n)
                    .map(|i| {
                        let f = i as f32 / n as f32;
                        let p = from.lerp(to, f);
                        if s.k == ShK::Wave {
                            let amp = 0.25 * (f * PI).sin().max(0.25);
                            p + side * (f * PI * 3.0 - t * 12.0).sin() * amp
                                + Vec3::Y * (f * PI * 2.0 - t * 9.0).cos() * amp * 0.5
                        } else {
                            p
                        }
                    })
                    .collect();
                let w = match s.k {
                    ShK::Ray => s.w * (1.0 - u * 0.5) * 0.5,
                    ShK::Ribbon => s.w * 0.35,
                    _ => 0.07,
                }
                .max(0.02);
                let radii: Vec<f32> = (0..=n)
                    .map(|i| {
                        let f = i as f32 / n as f32;
                        if s.k == ShK::Ribbon {
                            w * (0.2 + 0.8 * f)
                        } else {
                            w
                        }
                    })
                    .collect();
                o.glow(HALO).tube(&pts, &radii, false, ca(c, 0.6 * a), 1.0);
                let core: Vec<f32> = radii.iter().map(|r| r * 0.3).collect();
                o.glow(0).tube(&pts, &core, false, ca(WHITE, 0.85 * a), 1.0);
                if s.k == ShK::Ray {
                    // a helix of the ability's colour round the beam
                    let m = lk.d.n.max(2) as usize;
                    for k in 0..m.min(4) {
                        let hel: Vec<Vec3> = (0..=n)
                            .map(|i| {
                                let f = i as f32 / n as f32;
                                let ang = f * len * 4.0 - t * 10.0 + k as f32 * TAU / m as f32;
                                let up = side.cross(dir);
                                pts[i] + (side * ang.cos() + up * ang.sin()) * w * 1.8
                            })
                            .collect();
                        let rr = vec![0.018; n + 1];
                        o.glow(HALO).tube(&hel, &rr, false, ca(hot, 0.6 * a), 1.0);
                    }
                }
            }
            ShK::Slash | ShK::Claw | ShK::Spin => {
                let (a, sweep) = match s.k {
                    ShK::Spin => (env(u, 0.1, 0.4), 1.0),
                    ShK::Claw => (1.0 - ((u - 0.25) / 0.75).max(0.0), (u / 0.25).min(1.0)),
                    _ => (1.0 - ((u - 0.3) / 0.7).max(0.0), (u / 0.3).min(1.0)),
                };
                let centre = base + vec3(0.0, CHEST, 0.0);
                let blades: Vec<(f32, f32, f32)> = match s.k {
                    ShK::Spin => {
                        let n = s.n.max(1) as usize;
                        let r = s.r * (0.6 + 0.4 * (u / 0.2).min(1.0));
                        (0..n)
                            .map(|i| (s.ang + t * 14.0 + i as f32 * TAU / n as f32, 1.6, r))
                            .collect()
                    }
                    ShK::Claw => (0..3)
                        .map(|i| (s.ang + (i as f32 - 1.0) * 0.32 + PI, 1.3 * sweep, 0.5))
                        .collect(),
                    _ => {
                        // it sweeps across the full arc, one way or the other
                        let full = if s.n == 2 { 3.0 } else { 2.3 };
                        let span = full * sweep;
                        let a0 = if s.n == 1 {
                            s.ang + full / 2.0 - span
                        } else {
                            s.ang - full / 2.0
                        };
                        vec![(a0, span, s.r)]
                    }
                };
                // swung on a slant, as a blade is
                let tilt = if s.n == 1 { -0.45 } else { 0.45 };
                for (i, &(a0, span, r)) in blades.iter().enumerate() {
                    let off = if s.k == ShK::Slash {
                        -vec3(s.ang.cos(), 0.0, s.ang.sin()) * r * 0.55
                    } else if s.k == ShK::Claw {
                        vec3(s.ang.cos(), 0.0, s.ang.sin()) * (0.25 - i as f32 * 0.25)
                    } else {
                        Vec3::ZERO
                    };
                    let steps = 16;
                    let w = s.w.max(0.08) * if s.k == ShK::Claw { 0.4 } else { 0.8 };
                    let rows: Vec<Vec<Vec3>> = (0..=steps)
                        .map(|j| {
                            let f = j as f32 / steps as f32;
                            let ang = a0 + span * f;
                            let dir = vec3(ang.cos(), 0.0, ang.sin());
                            let lift = Vec3::Y * ((f - 0.5) * tilt * r);
                            let thick = w * (PI * f).sin().max(0.05);
                            let p = centre + off + dir * r + lift;
                            vec![p - dir * thick * 0.5, p, p + dir * thick * 0.5]
                        })
                        .collect();
                    let col = |_: usize, k: usize| {
                        if k == 1 {
                            ca(c2, 0.95 * a)
                        } else {
                            ca(c, 0.45 * a)
                        }
                    };
                    o.glow(0).grid(&rows, false, false, &col, 1.0);
                    if s.k == ShK::Spin && body(lk.elem) == Body::Metal {
                        // the blades themselves at the head of each sweep
                        let ang = a0 + span;
                        let dir = vec3(ang.cos(), 0.0, ang.sin());
                        let tang = vec3(-ang.sin(), 0.0, ang.cos());
                        let p = centre + dir * r;
                        o.thing(METAL, true).blade(
                            p - tang * 0.3,
                            p + tang * 0.3,
                            Vec3::Y,
                            0.07,
                            0.015,
                            Color::new(0.85, 0.87, 0.92, a),
                        );
                    }
                }
            }
            ShK::Thrust => {
                let a = 1.0 - u;
                let dir = vec3(s.ang.cos(), 0.0, s.ang.sin());
                let from = base + vec3(0.0, CHEST, 0.0) - dir * 0.45;
                let len = 0.9 * (u / 0.25).min(1.0);
                let w = s.w.max(0.1) * 0.5;
                o.glow(HALO)
                    .spike(from, from + dir * len, w, 8, ca(c, 0.7 * a), 1.0);
                o.glow(0).spike(
                    from,
                    from + dir * len * 1.05,
                    w * 0.3,
                    6,
                    ca(WHITE, 0.9 * a),
                    1.0,
                );
            }
            ShK::Waves => {
                let n = 3;
                for i in 0..n {
                    let ui = (u * 1.2 - i as f32 * 0.18).clamp(0.0, 1.0);
                    if ui <= 0.0 || ui >= 1.0 {
                        continue;
                    }
                    let r = s.r * (0.1 + 0.9 * ui.powf(0.7));
                    let a = 1.0 - ui;
                    let ring = Xf::at(base + vec3(0.0, 0.3 + 0.2 * i as f32, 0.0));
                    o.glow(HALO).torus(
                        ring,
                        Vec3::ZERO,
                        r,
                        (s.w * a * 0.5).max(0.02),
                        ca(c, 0.7 * a),
                        1.0,
                    );
                }
            }
            ShK::Clock => {
                let e = env(u, 0.15, 0.3);
                let r = s.r;
                let face = Xf {
                    o: base + vec3(0.0, 1.75 + r, 0.0),
                    m: Mat3::from_cols(cam.right, cam.up, cam.right.cross(cam.up))
                        * Mat3::from_diagonal(Vec3::splat(r)),
                };
                let gold = Color::new(1.0, 0.82, 0.38, e);
                let disc = face.then(Vec3::ZERO, Mat3::from_rotation_x(FRAC_PI_2));
                o.glow(0).lathe(
                    disc,
                    vec3(0.0, -0.02, 0.0),
                    &[(0.0, 0.95, 0.95), (0.04, 0.95, 0.95)],
                    24,
                    true,
                    ca(vivid(c), 0.12 * e),
                    1.0,
                );
                o.thing(METAL, true)
                    .torus(disc, Vec3::ZERO, 1.0, 0.06, gold, 0.3);
                for i in 0..12 {
                    let ang = i as f32 * TAU / 12.0;
                    let d = vec3(ang.cos(), ang.sin(), 0.0);
                    o.thing(METAL, true).capsule(
                        face.p(d * 0.78),
                        face.p(d * 0.9),
                        0.025 * r,
                        0.025 * r,
                        gold,
                        0.3,
                    );
                }
                // the teeth of a gear behind, turning slowly
                let n = (lk.d.n as usize + 6).min(14);
                for i in 0..n {
                    let ang = i as f32 * TAU / n as f32 + t * 0.5;
                    let d = vec3(ang.cos(), ang.sin(), -0.15);
                    o.thing(METAL, true).cuboid(
                        Xf {
                            o: face.p(d * 1.12),
                            m: face.m * Mat3::from_rotation_z(ang),
                        },
                        Vec3::ZERO,
                        vec3(0.1, 0.07, 0.04),
                        gold,
                        0.3,
                    );
                }
                for (len, rate) in [(0.75, 6.0f32), (0.5, 0.5)] {
                    let ang = FRAC_PI_2 - t * rate;
                    o.glow(0).capsule(
                        face.o,
                        face.p(vec3(ang.cos() * len, ang.sin() * len, 0.05)),
                        0.04 * r,
                        0.02 * r,
                        ca(WHITE, e),
                        1.0,
                    );
                }
            }
            ShK::Hole | ShK::Eclipse => {
                let e = env(u, 0.15, 0.3);
                let r =
                    s.r * if s.k == ShK::Hole {
                        0.3 + 0.7 * (u / 0.2).min(1.0)
                    } else {
                        1.0
                    } * 0.6;
                let at = base + vec3(0.0, if s.k == ShK::Hole { 0.35 } else { 1.6 }, 0.0);
                o.thing(0, true).ellipsoid(
                    Xf::at(at),
                    Vec3::ZERO,
                    Vec3::splat(r),
                    Color::new(0.02, 0.0, 0.04, e),
                    0.0,
                );
                o.glow(RIM).ellipsoid(
                    Xf::at(at),
                    Vec3::ZERO,
                    Vec3::splat(r * 1.15),
                    ca(c, 0.9 * e),
                    1.0,
                );
                if s.k == ShK::Hole {
                    for (i, k) in [(0, 1.6f32), (1, 2.1)] {
                        let disc = Xf {
                            o: at,
                            m: Mat3::from_rotation_y(t * (2.0 + i as f32))
                                * Mat3::from_rotation_x(0.25),
                        };
                        o.glow(HALO).torus(
                            disc,
                            Vec3::ZERO,
                            r * k,
                            r * 0.12,
                            ca(hot, 0.6 * e),
                            1.0,
                        );
                    }
                } else {
                    for i in 0..12 {
                        let ang = i as f32 * TAU / 12.0 + t * 0.4;
                        let d = cam.right * ang.cos() + cam.up * ang.sin();
                        let l = 1.25 + 0.35 * flick(s.seed + i, t, 6.0);
                        o.glow(HALO).spike(
                            at + d * r * 1.05,
                            at + d * r * l * 1.4,
                            r * 0.12,
                            5,
                            ca(hot, 0.6 * e),
                            1.0,
                        );
                    }
                }
            }
            ShK::Bubble => {
                let e = env(u, 0.1, 0.3);
                let r =
                    s.r * (0.8 + 0.2 * (u / 0.15).min(1.0)) * (1.0 + 0.03 * (t * 8.0).sin()) * 1.15;
                let at = base + vec3(0.0, 0.55, 0.0);
                o.glow(RIM).ellipsoid(
                    Xf::at(at),
                    Vec3::ZERO,
                    vec3(r, r * 1.1, r),
                    ca(c, 0.9 * e),
                    1.0,
                );
                // the ability's signs circling the shield
                let n = lk.d.n.max(3) as usize;
                for i in 0..n {
                    let ang = t * 1.5 * lk.d.spin as f32 + i as f32 * TAU / n as f32;
                    let p = at
                        + vec3(
                            ang.cos() * r,
                            0.1 * (t * 2.0 + i as f32).sin(),
                            ang.sin() * r,
                        );
                    heart(o, &lk, upright(p, -ang, 0.09), t, 0.8 * e);
                }
            }
            ShK::Sigil => {
                let e = env(u, 0.1, 0.3);
                let pop = 1.0 + 0.6 * (1.0 - (u / 0.15).min(1.0));
                let at = base + vec3(0.0, 1.55, 0.0);
                let xf = upright(at, t * 2.0, 0.17 * pop);
                sign(o, &lk, xf, t, e, s.seed);
            }
            ShK::Rays => {
                let e = env(u, 0.1, 0.4);
                let n = s.n.max(3) as usize;
                let at = base + vec3(0.0, CHEST, 0.0);
                o.glow(HALO).ellipsoid(
                    Xf::at(at),
                    Vec3::ZERO,
                    Vec3::splat(s.r * 0.35),
                    ca(c, 0.4 * e),
                    1.0,
                );
                for i in 0..n {
                    let ang = s.ang + i as f32 * TAU / n as f32 + t * 0.6;
                    let up = ((i * 7) % 5) as f32 * 0.15 - 0.2;
                    let d = vec3(ang.cos(), up, ang.sin()).normalize();
                    let l = s.r * (0.7 + 0.5 * (u / 0.2).min(1.0));
                    o.glow(HALO)
                        .spike(at + d * 0.1, at + d * l, 0.07, 6, ca(c2, 0.7 * e), 1.0);
                }
            }
            ShK::Spike => {
                let e = env(u, 0.0, 0.3);
                let grow = (u / 0.15).min(1.0);
                let ice = Color::new(0.75, 0.92, 1.0, 0.85 * e);
                for i in 0..3u32 {
                    let (tilt, k) = match i {
                        0 => (0.0, 1.0),
                        1 => (-0.5 - hashf(s.seed + 1) * 0.3, 0.6),
                        _ => (0.5 + hashf(s.seed + 2) * 0.3, 0.55),
                    };
                    let h = s.r * 1.3 * grow * k;
                    let dir = vec3(tilt.sin(), tilt.cos(), (i as f32 - 1.0) * 0.3).normalize();
                    let foot = base + vec3((i as f32 - 1.0) * 0.08, 0.0, 0.0);
                    o.thing(0, true).gem(
                        along(foot + dir * h * 0.5, dir, 1.0, i as f32),
                        Vec3::ZERO,
                        vec3(h * 0.55, h * 0.13, h * 0.13),
                        0,
                        ice,
                        0.25,
                    );
                }
                o.glow(HALO).ellipsoid(
                    Xf::at(base),
                    Vec3::ZERO,
                    vec3(s.r, s.r * 0.3, s.r),
                    ca(c, 0.3 * e),
                    1.0,
                );
            }
            ShK::Emblem => {
                // over a creature it rises and turns; elsewhere it bursts
                let over = matches!(s.n, FX_CAST | FX_ALLY);
                let (e, h, k) = if over {
                    (env(u, 0.15, 0.35), 1.45 + 0.25 * u, 1.0)
                } else if s.n == FX_AREA || s.n == FX_SUMMON {
                    (env(u, 0.1, 0.4), 0.9 + 0.3 * u, 1.0 + 0.3 * u)
                } else {
                    (1.0 - u, CHEST + 0.1, 1.0 + 1.2 * u)
                };
                let at = base + vec3(0.0, h, 0.0);
                let xf = upright(at, t * 1.5 * lk.d.spin as f32, s.r * k);
                sign(o, &lk, xf, t, e, s.seed);
            }
        }
    }

    /// What an ability's wave carries round its ring: pieces of its element
    /// (flames, ice, rocks, light) and its own signs.
    #[allow(clippy::too_many_arguments)]
    fn wave_riders(&self, o: &mut Out, lk: &Look, base: Vec3, r: f32, a: f32, t: f32, seed: u32) {
        let n = (lk.d.n as usize * 2).max(6);
        for i in 0..n {
            let ang = i as f32 * TAU / n as f32 + hashf(seed + i as u32) * 0.3;
            let dir = vec3(ang.cos(), 0.0, ang.sin());
            let p = base + dir * r;
            match body(lk.elem) {
                Body::Ice => {
                    o.thing(0, true).gem(
                        along(p + Vec3::Y * 0.18 * a, Vec3::Y + dir * 0.3, 1.0, ang),
                        Vec3::ZERO,
                        vec3(0.25 * a, 0.07, 0.07),
                        0,
                        Color::new(0.78, 0.94, 1.0, 0.85 * a),
                        0.25,
                    );
                }
                Body::Stone => {
                    o.thing(STONE, false).stone(
                        Xf::at(p + Vec3::Y * 0.06),
                        Vec3::ZERO,
                        Vec3::splat(0.08 + 0.06 * a),
                        seed.wrapping_add(i as u32) | 1,
                        Color::new(0.5, 0.44, 0.38, 1.0),
                        0.0,
                    );
                }
                _ => {
                    if i % 2 == 0 {
                        let tip = p + Vec3::Y * (0.35 + 0.3 * flick(seed + i as u32, t, 12.0)) * a;
                        o.glow(HALO).spike(p, tip, 0.09, 6, ca(lk.c, 0.6 * a), 1.0);
                    } else {
                        let xf = upright(p + Vec3::Y * 0.25, ang, 0.07);
                        heart(o, lk, xf, t, 0.8 * a);
                    }
                }
            }
        }
    }
}
