//! The stuff things are made of in the detailed looks: woven cloth, leather,
//! metal, wood, bark, leaves, stone, skin, fur, scales, bone, straw and the
//! ground's grass, soil and sand. Each is a seamless tile painted here once
//! (how light or dark it is, how its surface rises and falls, how glossy it
//! is) and gathered with the others into one picture for the video card;
//! the shader lays them over the models from three sides and lights their
//! relief.

use std::f32::consts::{PI, TAU};

/// Woven cloth: shirts, robes, capes, hoods.
pub const CLOTH: u8 = 1;
pub const LEATHER: u8 = 2;
/// Metal: blades, armour, buckles, crowns (brushed, scratched).
pub const METAL: u8 = 3;
/// Worked wood: handles, staffs, beams, planks.
pub const WOOD: u8 = 4;
pub const BARK: u8 = 5;
pub const LEAVES: u8 = 6;
pub const STONE: u8 = 7;
pub const SKIN: u8 = 8;
pub const FUR: u8 = 9;
pub const SCALES: u8 = 10;
/// Bone, horn, tusks, chitin.
pub const BONE: u8 = 11;
/// Straw and thatch.
pub const STRAW: u8 = 12;
pub const GRASS: u8 = 13;
pub const SOIL: u8 = 14;
/// Sand (and snow, glittering).
pub const SAND: u8 = 15;
pub const PLASTER: u8 = 16;
/// The ground: grass, soil and sand, chosen by its colour.
pub const GROUND: u8 = 17;

/// Pixels of a material's tile.
pub const TILE: usize = 256;
/// The border around each tile (repeating it), so the smaller copies of the
/// picture do not bleed into their neighbours.
pub const GUTTER: usize = 16;
/// A tile with its border.
pub const SLOT: usize = TILE + 2 * GUTTER;
/// Tiles along a side of the picture.
pub const COLS: usize = 4;
/// The side of the whole picture.
pub const SIZE: usize = SLOT * COLS;

/// How many times a material repeats along a tile of the world.
pub fn scale_of(m: u8) -> f32 {
    match m {
        CLOTH => 2.2,
        LEATHER => 2.0,
        METAL => 1.6,
        WOOD => 1.5,
        BARK => 1.3,
        LEAVES => 1.5,
        STONE => 0.9,
        SKIN => 2.4,
        FUR => 2.4,
        SCALES => 3.0,
        BONE => 2.0,
        STRAW => 1.3,
        GRASS | GROUND => 0.7,
        SOIL => 0.6,
        SAND => 0.6,
        PLASTER => 0.9,
        _ => 1.0,
    }
}

// ---- seamless noise ----

fn h2(x: i32, y: i32, s: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ s.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0xFF_FFFF) as f32 / 16_777_215.0
}

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Smooth noise in 0..1 at (u, v) of a tile, `px` cells across and `py`
/// down, the same on all its edges.
fn vn(u: f32, v: f32, px: i32, py: i32, s: u32) -> f32 {
    let (x, y) = (u * px as f32, v * py as f32);
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (fu, fv) = (fade(x - ix as f32), fade(y - iy as f32));
    let c = |dx: i32, dy: i32| h2((ix + dx).rem_euclid(px), (iy + dy).rem_euclid(py), s);
    lerp(lerp(c(0, 0), c(1, 0), fu), lerp(c(0, 1), c(1, 1), fu), fv)
}

/// Smooth gradient noise in about -1..1 (its zero lines wind like cracks
/// and furrows), seamless like `vn`.
fn pn(u: f32, v: f32, px: i32, py: i32, s: u32) -> f32 {
    let (x, y) = (u * px as f32, v * py as f32);
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - ix as f32, y - iy as f32);
    let dot = |dx: i32, dy: i32| {
        let a = h2((ix + dx).rem_euclid(px), (iy + dy).rem_euclid(py), s) * TAU;
        a.cos() * (fx - dx as f32) + a.sin() * (fy - dy as f32)
    };
    let (fu, fv) = (fade(fx), fade(fy));
    lerp(
        lerp(dot(0, 0), dot(1, 0), fu),
        lerp(dot(0, 1), dot(1, 1), fu),
        fv,
    ) * 1.45
}

/// Noise of several octaves in about 0..1, `p` cells at the coarsest.
fn fbm(u: f32, v: f32, p: i32, oct: u32, s: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut f) = (0.0, 1.0, 0.0, p);
    for k in 0..oct {
        sum += vn(u, v, f, f, s.wrapping_add(k * 101)) * amp;
        norm += amp;
        amp *= 0.5;
        f *= 2;
    }
    sum / norm
}

/// Cells around scattered points, `p` across: the distances to the
/// nearest and the next point (in cells) and which cell is nearest.
fn worley(u: f32, v: f32, p: i32, s: u32) -> (f32, f32, u32) {
    let (x, y) = (u * p as f32, v * p as f32);
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (mut f1, mut f2, mut id) = (9.0f32, 9.0f32, 0u32);
    for dy in -2..=2 {
        for dx in -2..=2 {
            let (cx, cy) = (ix + dx, iy + dy);
            let (wx, wy) = (cx.rem_euclid(p), cy.rem_euclid(p));
            let qx = cx as f32 + 0.1 + 0.8 * h2(wx, wy, s);
            let qy = cy as f32 + 0.1 + 0.8 * h2(wx, wy, s + 1);
            let d = ((qx - x).powi(2) + (qy - y).powi(2)).sqrt();
            if d < f1 {
                f2 = f1;
                f1 = d;
                id = (wy * p + wx) as u32;
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    (f1, f2, id)
}

// ---- a tile being painted ----

/// A material's tile: how light it is (0.5 is its colour as it is), its
/// height, its gloss.
struct Tile {
    alb: Vec<f32>,
    hgt: Vec<f32>,
    gloss: Vec<f32>,
}

impl Tile {
    fn new() -> Tile {
        Tile {
            alb: vec![0.5; TILE * TILE],
            hgt: vec![0.5; TILE * TILE],
            gloss: vec![0.0; TILE * TILE],
        }
    }

    /// Paints every pixel by its place (u, v in 0..1).
    fn each(&mut self, f: impl Fn(f32, f32) -> (f32, f32, f32)) {
        for y in 0..TILE {
            for x in 0..TILE {
                let (u, v) = (x as f32 / TILE as f32, y as f32 / TILE as f32);
                let (a, h, g) = f(u, v);
                let k = y * TILE + x;
                self.alb[k] = a;
                self.hgt[k] = h;
                self.gloss[k] = g;
            }
        }
    }

    fn at(x: i32, y: i32) -> usize {
        (y.rem_euclid(TILE as i32) as usize) * TILE + x.rem_euclid(TILE as i32) as usize
    }

    /// A soft stroke (a hair, a straw, a blade, a scratch) from (x, y) along
    /// angle `a`: it raises the surface by `h` and sets its lightness
    /// toward `lum` (fading to the tip by `taper`).
    #[allow(clippy::too_many_arguments)]
    fn stroke(&mut self, x: f32, y: f32, a: f32, len: f32, w: f32, h: f32, lum: f32, taper: f32) {
        let (dx, dy) = (a.cos(), a.sin());
        let steps = (len * 2.0).ceil() as i32;
        let r = w.ceil() as i32 + 1;
        for i in 0..=steps {
            let t = i as f32 / steps.max(1) as f32;
            let (cx, cy) = (x + dx * len * t, y + dy * len * t);
            let ww = w * (1.0 - t * taper).max(0.15);
            for oy in -r..=r {
                for ox in -r..=r {
                    let (px, py) = (cx.round() as i32 + ox, cy.round() as i32 + oy);
                    let d = (((px as f32 - cx).powi(2) + (py as f32 - cy).powi(2)).sqrt()) / ww;
                    if d >= 1.0 {
                        continue;
                    }
                    let k = Tile::at(px, py);
                    let prof = (1.0 - d * d).sqrt();
                    let top = 0.5 + h * prof * (1.0 - t * 0.3);
                    if top > self.hgt[k] {
                        self.hgt[k] = top;
                        self.alb[k] = lerp(self.alb[k], lum, prof.min(1.0) * 0.85);
                    }
                }
            }
        }
    }

    /// Softens the height a little (strokes and edges become paint, not
    /// pixels).
    fn blur_height(&mut self, times: usize) {
        for _ in 0..times {
            let src = self.hgt.clone();
            for y in 0..TILE as i32 {
                for x in 0..TILE as i32 {
                    let mut s = src[Tile::at(x, y)] * 4.0;
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        s += src[Tile::at(x + dx, y + dy)];
                    }
                    self.hgt[Tile::at(x, y)] = s / 8.0;
                }
            }
        }
    }

    /// Centres the lightness on 0.5 and sets its spread.
    fn level(&mut self, spread: f32) {
        let n = self.alb.len() as f32;
        let mean = self.alb.iter().sum::<f32>() / n;
        let var = self.alb.iter().map(|a| (a - mean).powi(2)).sum::<f32>() / n;
        let k = spread / var.sqrt().max(1e-4);
        for a in &mut self.alb {
            *a = (0.5 + (*a - mean) * k).clamp(0.0, 1.0);
        }
    }

    /// The tile as the shader reads it: lightness, the slope of the surface
    /// across and down, gloss. The slopes are scaled so that most of them
    /// stay below `bump` (0..1): every material as rough as it should look,
    /// however its heights were painted.
    fn pixels(&self, bump: f32) -> Vec<[u8; 4]> {
        let b8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        let slopes: Vec<(f32, f32)> = (0..TILE * TILE)
            .map(|k| {
                let (x, y) = ((k % TILE) as i32, (k / TILE) as i32);
                (
                    (self.hgt[Tile::at(x + 1, y)] - self.hgt[Tile::at(x - 1, y)]) * 0.5,
                    (self.hgt[Tile::at(x, y + 1)] - self.hgt[Tile::at(x, y - 1)]) * 0.5,
                )
            })
            .collect();
        let mut steep: Vec<f32> = slopes.iter().map(|(a, b)| a.abs().max(b.abs())).collect();
        steep.sort_by(f32::total_cmp);
        let k = bump / steep[steep.len() * 9 / 10].max(1e-5);
        slopes
            .iter()
            .enumerate()
            .map(|(i, (gu, gv))| {
                [
                    b8(self.alb[i]),
                    b8(0.5 + 0.5 * (gu * k).clamp(-1.0, 1.0)),
                    b8(0.5 + 0.5 * (gv * k).clamp(-1.0, 1.0)),
                    b8(self.gloss[i]),
                ]
            })
            .collect()
    }
}

/// A random number generator for scattering.
struct Rnd(u32);

impl Rnd {
    fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 & 0xFF_FFFF) as f32 / 16_777_215.0
    }
}

// ---- the materials ----

fn cloth() -> (Tile, f32) {
    let mut t = Tile::new();
    let threads = 28.0;
    t.each(|u, v| {
        let (x, y) = (u * threads, v * threads);
        let (ix, iy) = (x.floor(), y.floor());
        let (fx, fy) = (x - ix, y - iy);
        let warp = (ix as i32 + iy as i32) % 2 == 0;
        let pw = (PI * fx).sin();
        let pf = (PI * fy).sin();
        let weave = if warp {
            0.45 + 0.55 * pw * (0.75 + 0.25 * pf)
        } else {
            0.45 + 0.55 * pf * (0.75 + 0.25 * pw)
        };
        // the yarn of each thread a little different, slubs, folds, wear
        let yarn = if warp {
            h2(ix as i32, 0, 5)
        } else {
            h2(0, iy as i32, 6)
        };
        let fold = (TAU * (u * 3.0 + fbm(u, v, 2, 2, 9) * 0.9)).sin();
        let wear = fbm(u, v, 3, 4, 11);
        let a = 0.42 + 0.22 * weave + 0.1 * yarn - 0.12 * wear + 0.06 * fold;
        let h = weave * 0.35 + fold * 0.55 + wear * 0.1;
        (a, h, 0.05 + 0.05 * weave)
    });
    t.level(0.12);
    (t, 0.55)
}

fn leather() -> (Tile, f32) {
    let mut t = Tile::new();
    t.each(|u, v| {
        // a fine pebbled grain, thin creases, worn patches
        let (f1, f2, id) = worley(u, v, 46, 3);
        let grain = smooth(0.0, 0.12, f2 - f1) * 0.6 + (1.0 - f1).max(0.0) * 0.4;
        let crease = (1.0 - smooth(0.0, 0.06, pn(u, v, 4, 6, 8).abs()))
            * smooth(0.4, 0.6, fbm(u, v, 2, 2, 9));
        let patch = fbm(u, v, 3, 4, 13);
        let a = 0.42 + 0.06 * grain + 0.04 * h2(id as i32, 1, 2) + 0.22 * patch - 0.12 * crease;
        let h = 0.2 * grain + 0.25 * patch - 0.25 * crease;
        (a, h, 0.18 + 0.25 * patch * (1.0 - crease))
    });
    t.level(0.1);
    (t, 0.45)
}

fn metal() -> (Tile, f32) {
    let mut t = Tile::new();
    t.each(|u, v| {
        let brush = vn(u, v, 3, 160, 21) * 0.6 + vn(u, v, 6, 90, 22) * 0.4;
        let tarnish = fbm(u, v, 3, 4, 23);
        let a = 0.5 + 0.1 * brush - 0.16 * smooth(0.55, 0.85, tarnish);
        (
            a,
            0.5 + 0.06 * brush,
            0.75 + 0.2 * brush - 0.5 * smooth(0.55, 0.9, tarnish),
        )
    });
    let mut r = Rnd(0x51A5_7C3D);
    // dents
    for _ in 0..9 {
        let (cx, cy, rad) = (r.f() * TILE as f32, r.f() * TILE as f32, 8.0 + r.f() * 18.0);
        let ri = rad as i32 + 1;
        for oy in -ri..=ri {
            for ox in -ri..=ri {
                let d = ((ox * ox + oy * oy) as f32).sqrt() / rad;
                if d < 1.0 {
                    let k = Tile::at(cx as i32 + ox, cy as i32 + oy);
                    t.hgt[k] -= 0.25 * (1.0 - d * d).powi(2);
                }
            }
        }
    }
    // scratches
    for _ in 0..70 {
        let (x, y) = (r.f() * TILE as f32, r.f() * TILE as f32);
        let a = r.f() * TAU;
        let len = 10.0 + r.f() * 50.0;
        let (dx, dy) = (a.cos(), a.sin());
        for i in 0..len as i32 {
            let k = Tile::at((x + dx * i as f32) as i32, (y + dy * i as f32) as i32);
            t.hgt[k] -= 0.05;
            t.alb[k] = (t.alb[k] + 0.18).min(1.0);
            t.gloss[k] = 1.0;
        }
    }
    t.level(0.07);
    (t, 0.25)
}

fn wood() -> (Tile, f32) {
    let mut t = Tile::new();
    t.each(|u, v| {
        // the grain runs down the tile, waving a little
        let warp = fbm(u, v, 2, 3, 31) * 0.5 + 0.05 * (TAU * v).sin();
        let ring = (u * 11.0 + warp).fract();
        let late = smooth(0.0, 0.1, ring) * (1.0 - smooth(0.45, 1.0, ring));
        let fibre = vn(u, v, 110, 3, 32) * 0.6 + vn(u, v, 170, 5, 33) * 0.4;
        let a = 0.36 + 0.26 * late + 0.16 * fibre;
        let h = 0.3 * late + 0.3 * fibre + 0.3 * (1.0 - smooth(0.0, 0.06, ring));
        (a, h, 0.12 + 0.08 * late)
    });
    // a knot or two
    let mut r = Rnd(0x77A1_0F3B);
    for _ in 0..2 {
        let (cx, cy) = (r.f() * TILE as f32, r.f() * TILE as f32);
        let (rx, ry) = (6.0 + r.f() * 6.0, 10.0 + r.f() * 10.0);
        for oy in -(ry as i32) * 2..=(ry as i32) * 2 {
            for ox in -(rx as i32) * 2..=(rx as i32) * 2 {
                let d = ((ox as f32 / rx).powi(2) + (oy as f32 / ry).powi(2)).sqrt();
                if d < 2.0 {
                    let k = Tile::at(cx as i32 + ox, cy as i32 + oy);
                    let ring = (d * 3.0).fract();
                    let w = (1.0 - d / 2.0).powi(2);
                    t.alb[k] = lerp(
                        t.alb[k],
                        0.15 + 0.3 * ring,
                        w * if d < 1.0 { 1.0 } else { 0.5 },
                    );
                    t.hgt[k] -= 0.15 * w;
                }
            }
        }
    }
    t.level(0.13);
    (t, 0.4)
}

fn bark() -> (Tile, f32) {
    let mut t = Tile::new();
    t.each(|u, v| {
        // plates running up the trunk between deep, narrow furrows
        let warp = (fbm(u, v, 3, 3, 41) - 0.5) * 0.06;
        let n = pn(u + warp, v, 10, 1, 42) * 0.75 + pn(u + warp, v, 20, 2, 45) * 0.25;
        let plate = smooth(0.0, 0.3, n.abs());
        let split = smooth(0.0, 0.1, pn(u, v, 5, 12, 43).abs());
        let rough = fbm(u, v, 16, 3, 46);
        let h = plate * (0.8 + 0.2 * split) * 0.8 + 0.2 * rough;
        let lichen = smooth(0.64, 0.74, fbm(u, v, 4, 4, 44)) * plate;
        let a = 0.16 + 0.5 * plate * (0.85 + 0.15 * split) + 0.12 * rough + 0.3 * lichen;
        (a, h + 0.06 * lichen, 0.0)
    });
    t.level(0.15);
    (t, 0.7)
}

fn leaves() -> (Tile, f32) {
    let mut t = Tile::new();
    // dark gaps between the leaves
    t.each(|u, v| (0.12 + 0.1 * fbm(u, v, 4, 2, 51), 0.0, 0.0));
    let mut r = Rnd(0x3C6E_F372);
    let n = 260;
    for i in 0..n {
        let (cx, cy) = (r.f() * TILE as f32, r.f() * TILE as f32);
        let a = r.f() * TAU;
        let len = 14.0 + r.f() * 12.0;
        let wid = len * (0.32 + r.f() * 0.12);
        let lum = 0.35 + 0.45 * r.f();
        let layer = i as f32 / n as f32;
        let (ca, sa) = (a.cos(), a.sin());
        let ri = len as i32 + 2;
        for oy in -ri..=ri {
            for ox in -ri..=ri {
                // along the leaf and across it
                let (lx, ly) = (
                    ox as f32 * ca + oy as f32 * sa,
                    -ox as f32 * sa + oy as f32 * ca,
                );
                let t01 = lx / len * 0.5 + 0.5;
                if !(0.0..=1.0).contains(&t01) {
                    continue;
                }
                let half = wid * (PI * t01).sin().powf(0.8) * 0.5;
                if ly.abs() >= half {
                    continue;
                }
                let k = Tile::at(cx as i32 + ox, cy as i32 + oy);
                let across = ly.abs() / half.max(0.01);
                let dome = (1.0 - across * across).sqrt();
                let rib = 1.0 - smooth(0.0, 0.14, across);
                t.hgt[k] = 0.25 + 0.35 * layer + 0.3 * dome * (1.0 - t01 * 0.3);
                t.alb[k] = lum * (0.8 + 0.3 * t01) + 0.12 * rib - 0.1 * across;
                t.gloss[k] = 0.25 + 0.2 * dome;
            }
        }
    }
    t.blur_height(2);
    t.level(0.16);
    (t, 0.5)
}

fn stone() -> (Tile, f32) {
    let mut t = Tile::new();
    t.each(|u, v| {
        let base = fbm(u, v, 4, 5, 61);
        let broad = fbm(u, v, 2, 3, 65);
        // a few thin cracks, only where the rock is weak
        let (f1, f2, _) = worley(u, v, 4, 62);
        let crack = (1.0 - smooth(0.0, 0.035, f2 - f1)) * smooth(0.45, 0.6, broad);
        let grain = h2((u * TILE as f32) as i32, (v * TILE as f32) as i32, 63);
        let speck = if grain > 0.975 {
            0.22
        } else if grain < 0.025 {
            -0.18
        } else {
            0.0
        };
        let a = 0.32 + 0.3 * base + 0.2 * broad + speck - 0.18 * crack;
        let h = base * 0.6 + broad * 0.3 + 0.06 * grain - 0.22 * crack;
        (a, h, 0.06 + 0.14 * base)
    });
    t.level(0.12);
    (t, 0.55)
}

fn skin() -> (Tile, f32) {
    let mut t = Tile::new();
    t.each(|u, v| {
        let mottle = fbm(u, v, 5, 4, 71);
        let pores = vn(u, v, 64, 64, 72);
        let a = 0.5 + 0.1 * mottle + 0.03 * pores;
        (a, 0.4 * mottle + 0.15 * pores, 0.2 + 0.15 * mottle)
    });
    t.level(0.05);
    (t, 0.2)
}

fn fur() -> (Tile, f32) {
    let mut t = Tile::new();
    t.each(|u, v| (0.3 + 0.2 * fbm(u, v, 4, 3, 81), 0.0, 0.0));
    let mut r = Rnd(0x9E37_79B9);
    for _ in 0..5200 {
        let (x, y) = (r.f() * TILE as f32, r.f() * TILE as f32);
        let (u, v) = (x / TILE as f32, y / TILE as f32);
        // the hairs lie down the tile, in clumps turning a little
        let turn = (fbm(u, v, 3, 2, 82) - 0.5) * 1.2;
        let a = PI / 2.0 + turn + (r.f() - 0.5) * 0.3;
        let clump = fbm(u, v, 6, 2, 83);
        let lum = 0.25 + 0.5 * r.f() * 0.6 + 0.35 * clump;
        t.stroke(
            x,
            y,
            a,
            9.0 + r.f() * 10.0,
            1.3,
            0.35 + 0.2 * r.f(),
            lum,
            0.8,
        );
    }
    for g in &mut t.gloss {
        *g = 0.12;
    }
    t.blur_height(1);
    t.level(0.13);
    (t, 0.45)
}

fn scales() -> (Tile, f32) {
    let mut t = Tile::new();
    let rows = 8.0;
    t.each(|u, v| {
        let (x, y) = (u * rows, v * rows);
        // the lowest scale over a point lies on top
        let mut best: Option<(f32, f32, f32)> = None;
        for j in (y.floor() as i32 - 1..=y.floor() as i32 + 1).rev() {
            let off = if j.rem_euclid(2) == 1 { 0.5 } else { 0.0 };
            for i in x.floor() as i32 - 1..=x.floor() as i32 + 1 {
                let cx = i as f32 + off + 0.5;
                let cy = j as f32;
                let (dx, dy) = (x - cx, (y - cy) * 1.15);
                let d = (dx * dx + dy * dy).sqrt() / 0.68;
                if d < 1.0 && y >= cy - 0.6 {
                    let id = h2(i.rem_euclid(rows as i32), j.rem_euclid(rows as i32), 91);
                    best = Some((d, id, y - cy));
                    break;
                }
            }
            if best.is_some() {
                break;
            }
        }
        match best {
            Some((d, id, below)) => {
                let dome = (1.0 - d * d).sqrt();
                let edge = smooth(0.82, 1.0, d);
                let h = 0.3 + 0.5 * dome * smooth(-0.6, 0.4, below);
                (
                    0.32 + 0.3 * dome + 0.12 * id - 0.25 * edge,
                    h,
                    0.35 + 0.3 * dome,
                )
            }
            None => (0.15, 0.1, 0.1),
        }
    });
    t.blur_height(1);
    t.level(0.12);
    (t, 0.6)
}

fn bone() -> (Tile, f32) {
    let mut t = Tile::new();
    t.each(|u, v| {
        let base = fbm(u, v, 3, 4, 101);
        let lines = (TAU * (v * 14.0 + fbm(u, v, 2, 3, 102) * 2.0)).sin();
        let (f1, _, id) = worley(u, v, 40, 104);
        let pit = if h2(id as i32, 7, 105) < 0.35 {
            smooth(0.22, 0.08, f1)
        } else {
            0.0
        };
        let a = 0.5 + 0.12 * base + 0.03 * lines - 0.1 * pit;
        let h = 0.4 * base + 0.06 * lines - 0.12 * pit;
        (a, h, 0.3 + 0.15 * base)
    });
    t.level(0.07);
    (t, 0.25)
}

fn straw() -> (Tile, f32) {
    let mut t = Tile::new();
    t.each(|u, v| (0.2 + 0.1 * fbm(u, v, 4, 2, 111), 0.0, 0.0));
    let mut r = Rnd(0x2545_F491);
    for _ in 0..1700 {
        let (x, y) = (r.f() * TILE as f32, r.f() * TILE as f32);
        let a = PI / 2.0 + (r.f() - 0.5) * 0.35;
        let lum = 0.35 + 0.55 * r.f();
        t.stroke(
            x,
            y,
            a,
            30.0 + r.f() * 50.0,
            1.4 + r.f(),
            0.3 + 0.2 * r.f(),
            lum,
            0.3,
        );
    }
    for g in &mut t.gloss {
        *g = 0.1;
    }
    t.blur_height(1);
    t.level(0.15);
    (t, 0.5)
}

fn grass() -> (Tile, f32) {
    let mut t = Tile::new();
    t.each(|u, v| (0.3 + 0.3 * fbm(u, v, 3, 4, 121), 0.0, 0.0));
    let mut r = Rnd(0x6A09_E667);
    for _ in 0..4200 {
        let (x, y) = (r.f() * TILE as f32, r.f() * TILE as f32);
        let (u, v) = (x / TILE as f32, y / TILE as f32);
        let clump = fbm(u, v, 5, 2, 122);
        let a = r.f() * TAU;
        let lum = 0.3 + 0.45 * r.f() * 0.7 + 0.3 * clump;
        t.stroke(x, y, a, 5.0 + r.f() * 8.0, 1.2, 0.3, lum, 0.9);
    }
    for g in &mut t.gloss {
        *g = 0.08;
    }
    t.blur_height(1);
    t.level(0.1);
    (t, 0.45)
}

fn soil() -> (Tile, f32) {
    let mut t = Tile::new();
    t.each(|u, v| {
        let clods = fbm(u, v, 6, 5, 131);
        let (f1, _, id) = worley(u, v, 24, 132);
        let size = 0.18 + 0.2 * h2(id as i32, 5, 135);
        let pebble = if h2(id as i32, 3, 133) < 0.22 {
            smooth(size, size * 0.4, f1)
        } else {
            0.0
        };
        let a = 0.36 + 0.3 * clods + pebble * (0.1 * h2(id as i32, 4, 134) - 0.02);
        (a, clods * 0.7 + pebble * 0.35, 0.03 + 0.1 * pebble)
    });
    t.level(0.1);
    (t, 0.5)
}

fn sand() -> (Tile, f32) {
    let mut t = Tile::new();
    t.each(|u, v| {
        let grain = h2((u * TILE as f32) as i32, (v * TILE as f32) as i32, 141);
        let ripple = (TAU * (v * 7.0 + fbm(u, v, 2, 3, 142) * 2.2)).sin();
        let a = 0.48 + 0.1 * grain + 0.05 * ripple + 0.08 * fbm(u, v, 3, 3, 143);
        // a grain here and there glitters (snow, quartz)
        let glint = if grain > 0.985 { 1.0 } else { 0.06 };
        (a, 0.3 * ripple + 0.1 * grain, glint)
    });
    t.level(0.07);
    (t, 0.35)
}

fn plaster() -> (Tile, f32) {
    let mut t = Tile::new();
    t.each(|u, v| {
        let lumps = fbm(u, v, 3, 5, 151);
        let fine = vn(u, v, 80, 80, 152);
        let (f1, f2, _) = worley(u, v, 3, 153);
        let crack = (1.0 - smooth(0.0, 0.02, f2 - f1)) * smooth(0.5, 0.6, fbm(u, v, 2, 2, 154));
        let stain = smooth(0.6, 0.85, fbm(u, v, 2, 3, 155));
        let a = 0.55 + 0.12 * lumps + 0.03 * fine - 0.3 * crack - 0.12 * stain;
        (a, 0.5 * lumps + 0.08 * fine - 0.2 * crack, 0.02)
    });
    t.level(0.08);
    (t, 0.3)
}

/// The tiles of all the materials, one after another (CLOTH first), each
/// TILE × TILE pixels.
fn tiles() -> Vec<Vec<[u8; 4]>> {
    let makers: [fn() -> (Tile, f32); 16] = [
        cloth, leather, metal, wood, bark, leaves, stone, skin, fur, scales, bone, straw, grass,
        soil, sand, plaster,
    ];
    makers
        .iter()
        .map(|f| {
            let (t, bump) = f();
            t.pixels(bump)
        })
        .collect()
}

/// The picture of all the materials: SIZE × SIZE pixels (RGBA), each tile
/// in its slot with its border.
pub fn atlas() -> Vec<u8> {
    let mut out = vec![0u8; SIZE * SIZE * 4];
    for (i, tile) in tiles().iter().enumerate() {
        let (sx, sy) = ((i % COLS) * SLOT, (i / COLS) * SLOT);
        for y in 0..SLOT {
            for x in 0..SLOT {
                let tx = (x + TILE - GUTTER) % TILE;
                let ty = (y + TILE - GUTTER) % TILE;
                let k = ((sy + y) * SIZE + sx + x) * 4;
                out[k..k + 4].copy_from_slice(&tile[ty * TILE + tx]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_are_seamless_and_balanced() {
        for (i, tile) in tiles().iter().enumerate() {
            assert_eq!(tile.len(), TILE * TILE);
            let mean = tile.iter().map(|p| p[0] as f32).sum::<f32>() / tile.len() as f32;
            assert!((90.0..166.0).contains(&mean), "material {}: {mean}", i + 1);
            // the slope stays mostly gentle
            let flat = tile
                .iter()
                .filter(|p| (p[1] as i32 - 128).abs() < 120 && (p[2] as i32 - 128).abs() < 120)
                .count();
            assert!(flat * 10 > tile.len() * 8, "material {}", i + 1);
        }
        let a = atlas();
        assert_eq!(a.len(), SIZE * SIZE * 4);
        // the border repeats the far edge of the tile
        let px = |x: usize, y: usize| &a[(y * SIZE + x) * 4..(y * SIZE + x) * 4 + 4];
        assert_eq!(
            px(GUTTER - 1, GUTTER + 5),
            px(GUTTER + TILE - 1, GUTTER + 5)
        );
    }

    /// Writes the materials lit from the upper left to RATAS_SHEET.
    #[test]
    #[ignore]
    fn material_sheet() {
        let path = std::env::var("RATAS_SHEET").unwrap_or("materials.png".into());
        let ts = tiles();
        let (w, h) = (TILE * 4, TILE * 4);
        let mut img =
            macroquad::texture::Image::gen_image_color(w as u16, h as u16, macroquad::color::BLACK);
        for (i, tile) in ts.iter().enumerate() {
            let (ox, oy) = ((i % 4) * TILE, (i / 4) * TILE);
            for y in 0..TILE {
                for x in 0..TILE {
                    let p = tile[y * TILE + x];
                    let (gu, gv) = (p[1] as f32 / 127.5 - 1.0, p[2] as f32 / 127.5 - 1.0);
                    let n = macroquad::math::vec3(-gu, -gv, 1.0).normalize();
                    let l = macroquad::math::vec3(-0.5, -0.6, 0.62).normalize();
                    let k = 0.35 + 0.75 * n.dot(l).max(0.0);
                    let a = p[0] as f32 / 255.0 * 2.0 * k;
                    img.set_pixel(
                        (ox + x) as u32,
                        (oy + y) as u32,
                        macroquad::color::Color::new(a * 0.8, a * 0.75, a * 0.65, 1.0),
                    );
                }
            }
        }
        img.export_png(&path);
    }
}
