//! Pictures of abilities, skills, classes and subclasses. An ability is a
//! round rune, a passive skill a square plaque, a class or a subclass a
//! heraldic shield; all carry a 16x16 pictogram (a sword, a flame, a wolf's
//! head...) and a small badge in the corner for an element, the kind of bonus
//! or the class a subclass belongs to. Which pictogram and badge each gets is
//! decided in `picks`.

use std::f64::consts::PI;

use super::models::{C_BONE, C_GOLD};
use super::*;

/// The side of an icon in art pixels.
pub const SIDE: i32 = 24;
/// Where the 16x16 pictogram sits in the icon.
const O: f64 = 4.0;

type P = (f64, f64);

const STEEL: Rgba = rgb(0xd0, 0xd6, 0xe2);
const STEEL_HI: Rgba = rgb(0xf8, 0xfa, 0xff);
const STEEL_LO: Rgba = rgb(0x84, 0x8c, 0xa0);
const IRON: Rgba = rgb(0x5a, 0x5c, 0x6e);
const WOOD: Rgba = rgb(0xa8, 0x70, 0x3c);
const GOLD_LO: Rgba = rgb(0xa8, 0x78, 0x20);
const STONE_C: Rgba = rgb(0xa4, 0xa4, 0xb4);
const ROCK: Rgba = rgb(0x7a, 0x5c, 0x4a);
const EARTH: Rgba = rgb(0x8a, 0x64, 0x40);
const GLASS: Rgba = Rgba {
    r: 0xd4,
    g: 0xe4,
    b: 0xf0,
    a: 0xd8,
};
const PAGE: Rgba = rgb(0xef, 0xe6, 0xcf);
const FANG: Rgba = rgb(0xf4, 0xf0, 0xe4);
const HORN: Rgba = rgb(0xe8, 0xd8, 0xb8);
const FOAM: Rgba = rgb(0xf6, 0xf0, 0xdc);
const LEATHER: Rgba = rgb(0x5a, 0x4a, 0x3e);
const YELLOW: Rgba = rgb(0xff, 0xe0, 0x50);
const INK: Rgba = rgb(0x18, 0x12, 0x20);
const BONE_C: Rgba = rgb(0xe8, 0xdc, 0xc0);
const INK_A: Rgba = Rgba {
    r: 0x30,
    g: 0x24,
    b: 0x20,
    a: 0x90,
};

/// What an icon shows: a pictogram and a badge (empty: none).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Look {
    pub glyph: &'static str,
    pub badge: &'static str,
}

/// A pictogram.
pub struct Glyph {
    pub name: &'static str,
    /// its own colour, used on skill plaques (empty: the plaque's colour)
    pub color: &'static str,
    /// the element or bonus it already shows, so no badge repeats it
    pub elem: &'static str,
    draw: fn(&mut Gl),
}

pub fn glyph(name: &str) -> Option<&'static Glyph> {
    GLYPHS.iter().find(|g| g.name == name)
}

pub fn badge(name: &str) -> Option<&'static Badge> {
    BADGES.iter().find(|b| b.name == name)
}

/// The shape of an icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    /// an ability: a round rune
    Rune,
    /// a passive skill: a square plaque
    Plaque,
    /// a class: a shield with a golden rim
    Class,
    /// a subclass: a shield in its colour
    Subclass,
}

/// Paints an icon in a frame; `c` is the colour of the ability, of the
/// skill's branch or of the class.
pub fn paint(look: Look, c: Rgba, frame: Frame) -> Pc {
    let g = glyph(look.glyph).unwrap_or(&GLYPHS[0]);
    let mut p = match frame {
        Frame::Rune => rune_frame(c),
        Frame::Plaque => plaque_frame(c),
        Frame::Class => crest_frame(c, true),
        Frame::Subclass => crest_frame(c, false),
    };
    let main = if frame != Frame::Plaque || g.color.is_empty() {
        c
    } else {
        hex(g.color)
    };
    // a dark colour would vanish in the dark well
    let luma = 0.3 * main.r as f64 + 0.59 * main.g as f64 + 0.11 * main.b as f64;
    let main = if luma < 130.0 {
        mix(main, WHITE, ((130.0 - luma) / 160.0).min(0.6))
    } else {
        main
    };
    let mut gl = Gl::new(main);
    (g.draw)(&mut gl);
    gl.bevel();
    over(&mut p, &gl.p.outlined_in_place());
    if let Some(b) = badge(look.badge) {
        paint_badge(&mut p, b);
    }
    p
}

/// The rune of a fused ability: the upper left of one part's rune, the
/// lower right of the other's, a golden seam between them.
pub fn fusion(a: &Pc, b: &Pc, key: &str) -> Pc {
    let mut p = Pc::new(a.w, a.h, key);
    let n = a.w + a.h - 2;
    for y in 0..a.h {
        for x in 0..a.w {
            let d = x + y - n / 2;
            let c = if d < 0 { a.get(x, y) } else { b.get(x, y) };
            if !c.visible() {
                continue;
            }
            let seam = d == 0 || d == -1;
            p.set(x, y, if seam { C_GOLD } else { c });
        }
    }
    p
}

/// Draws b over a.
fn over(a: &mut Pc, b: &Pc) {
    for y in 0..b.h {
        for x in 0..b.w {
            a.px(x, y, b.get(x, y));
        }
    }
}

/// The dark well inside a frame, a little lighter in the middle.
fn well(p: &mut Pc, c: Rgba, dist: impl Fn(f64, f64) -> f64) {
    let deep = mix(mul(c, 0.16), rgb(8, 7, 14), 0.55);
    let glow = mix(mul(c, 0.42), rgb(8, 7, 14), 0.35);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let t = dist(x as f64 + 0.5 - 12.0, y as f64 + 0.5 - 12.0);
            if t <= 1.0 {
                p.put(x, y, mix(glow, deep, t.powf(1.3)));
            }
        }
    }
}

fn rune_frame(c: Rgba) -> Pc {
    let mut p = Pc::new(SIDE, SIDE, "rune");
    p.circle(12.0, 12.0, 11.95, mul(c, 0.3));
    p.ball(12.0, 12.0, 11.0, mul(c, 0.85), 0.0);
    well(&mut p, c, |dx, dy| dx.hypot(dy) / 9.4);
    p
}

/// Whether (x, y) is inside a rectangle with rounded corners.
fn in_round(x: f64, y: f64, (x0, y0, x1, y1): (f64, f64, f64, f64), r: f64) -> bool {
    let cx = x.clamp(x0 + r, x1 - r);
    let cy = y.clamp(y0 + r, y1 - r);
    (x0..=x1).contains(&x) && (y0..=y1).contains(&y) && (x - cx).hypot(y - cy) <= r
}

fn plaque_frame(c: Rgba) -> Pc {
    let mut p = Pc::new(SIDE, SIDE, "plaque");
    let s = SIDE as f64;
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (u, v) = (x as f64 + 0.5, y as f64 + 0.5);
            if !in_round(u, v, (0.0, 0.0, s, s), 4.0) {
                continue;
            }
            let col = if !in_round(u, v, (1.0, 1.0, s - 1.0, s - 1.0), 3.2) {
                mul(c, 0.3)
            } else if u + v < s {
                mix(c, WHITE, 0.12)
            } else {
                mul(c, 0.55)
            };
            p.put(x, y, col);
        }
    }
    // gold studs in the corners
    for (x, y) in [(2, 2), (21, 2), (2, 21), (21, 21)] {
        p.put(x, y, mix(C_GOLD, c, 0.3));
    }
    well(&mut p, c, |dx, dy| {
        if dx.abs() > 9.0 || dy.abs() > 9.0 {
            2.0
        } else {
            (dx.abs().max(dy.abs()) * 0.6 + dx.hypot(dy) * 0.4) / 10.5
        }
    });
    p
}

/// A heraldic shield: a golden rim for a class, one in its colour for a
/// subclass.
fn crest_frame(c: Rgba, gold: bool) -> Pc {
    let mut p = Pc::new(SIDE, SIDE, "crest");
    let shape = |k: f64| -> Vec<P> {
        // the outline shrunk by k pixels
        vec![
            (0.6 + k, 0.6 + k),
            (23.4 - k, 0.6 + k),
            (23.4 - k, 12.6 - k * 0.3),
            (20.8 - k * 0.8, 18.0 - k * 0.5),
            (12.0, 23.6 - k * 1.3),
            (3.2 + k * 0.8, 18.0 - k * 0.5),
            (0.6 + k, 12.6 - k * 0.3),
        ]
    };
    let (outer, rim, inner) = (shape(0.0), shape(1.0), shape(2.6));
    let rim_c = if gold { C_GOLD } else { c };
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (u, v) = (x as f64 + 0.5, y as f64 + 0.5);
            if !inside(&outer, u, v) {
                continue;
            }
            let col = if !inside(&rim, u, v) {
                mul(c, 0.3)
            } else if !inside(&inner, u, v) {
                if u + v * 0.8 < 21.0 {
                    mix(rim_c, WHITE, 0.2)
                } else {
                    mul(rim_c, 0.62)
                }
            } else {
                continue;
            };
            p.put(x, y, col);
        }
    }
    if gold {
        // studs on the rim
        for (x, y) in [(2, 2), (21, 2), (12, 21)] {
            p.put(x, y, mix(C_GOLD, WHITE, 0.6));
        }
    }
    let deep = mix(mul(c, 0.16), rgb(8, 7, 14), 0.55);
    let glow = mix(mul(c, 0.42), rgb(8, 7, 14), 0.35);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (u, v) = (x as f64 + 0.5, y as f64 + 0.5);
            if inside(&inner, u, v) {
                let t = ((u - 12.0).hypot((v - 10.0) * 1.1) / 11.0).min(1.0);
                p.put(x, y, mix(glow, deep, t.powf(1.3)));
            }
        }
    }
    p
}

// ---- the pictogram canvas ----

pub(crate) struct Gl {
    p: Pc,
    /// pixels painted as details keep their colour when the body is shaded
    flat: Vec<bool>,
    detail: bool,
    c: Rgba,
}

impl Gl {
    fn new(c: Rgba) -> Gl {
        Gl {
            p: Pc::new(SIDE, SIDE, "glyph"),
            flat: vec![false; (SIDE * SIDE) as usize],
            detail: false,
            c,
        }
    }

    fn hi(&self) -> Rgba {
        mix(self.c, WHITE, 0.5)
    }

    fn lo(&self) -> Rgba {
        mul(self.c, 0.6)
    }

    /// Paints every pixel whose centre (in pictogram coordinates) is inside.
    fn each(&mut self, c: Rgba, inside: impl Fn(f64, f64) -> bool) {
        for y in 0..SIDE {
            for x in 0..SIDE {
                if inside(x as f64 + 0.5 - O, y as f64 + 0.5 - O) {
                    self.p.set(x, y, c);
                    self.flat[(y * SIDE + x) as usize] = self.detail;
                }
            }
        }
    }

    fn erase(&mut self, inside: impl Fn(f64, f64) -> bool) {
        for y in 0..SIDE {
            for x in 0..SIDE {
                if inside(x as f64 + 0.5 - O, y as f64 + 0.5 - O) {
                    self.p.clear(x, y);
                }
            }
        }
    }

    /// Draws details: they are not shaded with the body.
    fn flat(&mut self, f: impl FnOnce(&mut Gl)) {
        let was = self.detail;
        self.detail = true;
        f(self);
        self.detail = was;
    }

    fn seg(&mut self, a: P, b: P, w: f64, c: Rgba) {
        self.each(c, |u, v| seg_dist((u, v), a, b).0 <= w / 2.0);
    }

    fn path(&mut self, pts: &[P], w: f64, c: Rgba) {
        self.each(c, |u, v| path_dist(pts, (u, v)).0 <= w / 2.0);
    }

    /// A stroke thick in the middle and thin at the ends.
    fn taper(&mut self, pts: &[P], w: f64, c: Rgba) {
        let len = path_len(pts).max(0.01);
        self.each(c, |u, v| {
            let (d, s) = path_dist(pts, (u, v));
            d <= w / 2.0 * (PI * s / len).sin().max(0.2)
        });
    }

    /// A stroke thick at its start and pointed at its end.
    fn spike(&mut self, pts: &[P], w: f64, c: Rgba) {
        let len = path_len(pts).max(0.01);
        self.each(c, |u, v| {
            let (d, s) = path_dist(pts, (u, v));
            d <= w / 2.0 * (1.0 - s / len).max(0.15)
        });
    }

    fn disc(&mut self, o: P, r: f64, c: Rgba) {
        self.each(c, |u, v| (u - o.0).hypot(v - o.1) <= r);
    }

    fn oval(&mut self, o: P, rx: f64, ry: f64, c: Rgba) {
        self.each(c, |u, v| {
            ((u - o.0) / rx).powi(2) + ((v - o.1) / ry).powi(2) <= 1.0
        });
    }

    fn ring(&mut self, o: P, r: f64, w: f64, c: Rgba) {
        self.each(c, |u, v| ((u - o.0).hypot(v - o.1) - r).abs() <= w / 2.0);
    }

    /// An elliptic ring turned by `rot` degrees.
    fn oval_ring(&mut self, o: P, (rx, ry): P, rot: f64, w: f64, c: Rgba) {
        let (s, k) = rot.to_radians().sin_cos();
        self.each(c, |u, v| {
            let (dx, dy) = (u - o.0, v - o.1);
            let (x, y) = (dx * k + dy * s, -dx * s + dy * k);
            let q = ((x / rx).powi(2) + (y / ry).powi(2)).sqrt().max(1e-6);
            let grad = (x / (rx * rx) / q).hypot(y / (ry * ry) / q).max(1e-6);
            (q - 1.0).abs() / grad <= w / 2.0
        });
    }

    /// Part of a ring from angle a0 to a1 (degrees, 0 east, 90 south).
    fn arc(&mut self, o: P, r: f64, w: f64, (a0, a1): P, c: Rgba) {
        self.each(c, |u, v| {
            let (dx, dy) = (u - o.0, v - o.1);
            (dx.hypot(dy) - r).abs() <= w / 2.0 && in_arc(dy.atan2(dx).to_degrees(), a0, a1)
        });
    }

    fn poly(&mut self, pts: &[P], c: Rgba) {
        self.each(c, |u, v| inside(pts, u, v));
    }

    fn rect(&mut self, a: P, b: P, c: Rgba) {
        self.each(c, |u, v| (a.0..b.0).contains(&u) && (a.1..b.1).contains(&v));
    }

    /// A hand-drawn 16x16 picture: X/x/H the main colour and its shades,
    /// W white, K ink, y yellow, R dark red, f/F bone, g gold.
    fn sprite(&mut self, rows: &[&str; 16]) {
        debug_assert!(rows.iter().all(|r| r.len() == 16), "{rows:?}");
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let (c, body) = match ch {
                    'X' => (self.c, true),
                    'x' => (self.lo(), false),
                    'H' => (self.hi(), false),
                    'W' => (WHITE, false),
                    'K' => (INK, false),
                    'y' => (YELLOW, false),
                    'R' => (rgb(0x8a, 0x1a, 0x22), false),
                    'f' => (C_BONE, true),
                    'F' => (rgb(0xb8, 0xac, 0x8a), false),
                    'g' => (C_GOLD, false),
                    _ => continue,
                };
                let (px, py) = (x as i32 + O as i32, y as i32 + O as i32);
                self.p.set(px, py, c);
                self.flat[(py * SIDE + px) as usize] = !body;
            }
        }
    }

    /// Lights the top-left edges of the body and darkens its bottom-right ones.
    fn bevel(&mut self) {
        let src = self.p.px.clone();
        let full = |x: i32, y: i32| {
            (0..SIDE).contains(&x) && (0..SIDE).contains(&y) && src[(y * SIDE + x) as usize].a > 0
        };
        for y in 0..SIDE {
            for x in 0..SIDE {
                let i = (y * SIDE + x) as usize;
                let c = src[i];
                if c.a == 0 || self.flat[i] {
                    continue;
                }
                let lit = !full(x - 1, y) || !full(x, y - 1);
                let shade = !full(x + 1, y) || !full(x, y + 1);
                self.p.px[i] = match (lit, shade) {
                    (true, false) => Rgba {
                        a: c.a,
                        ..mix(c, WHITE, 0.3)
                    },
                    (false, true) => mul(c, 0.72),
                    _ => c,
                };
            }
        }
    }
}

impl Pc {
    /// A dark one pixel outline around the opaque pixels, on the same canvas.
    fn outlined_in_place(&self) -> Pc {
        let mut o = Pc::new(self.w, self.h, "outline");
        for y in 0..self.h {
            for x in 0..self.w {
                let c = self.get(x, y);
                if c.a > 0 {
                    o.put(x, y, c);
                    continue;
                }
                let near = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .any(|(dx, dy)| self.get(x + dx, y + dy).a > 120);
                if near {
                    o.put(x, y, Rgba { a: 235, ..INK });
                }
            }
        }
        o
    }
}

// ---- geometry ----

fn in_arc(a: f64, a0: f64, a1: f64) -> bool {
    a1 - a0 >= 360.0 || (a - a0).rem_euclid(360.0) <= a1 - a0
}

/// The distance from p to the segment ab and how far along it the nearest point is.
fn seg_dist(p: P, a: P, b: P) -> (f64, f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (
        (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy),
        t * l2.sqrt(),
    )
}

/// The distance from p to a polyline and the length along it to the nearest point.
fn path_dist(pts: &[P], p: P) -> (f64, f64) {
    let mut best = (f64::MAX, 0.0);
    let mut run = 0.0;
    for w in pts.windows(2) {
        let (d, s) = seg_dist(p, w[0], w[1]);
        if d < best.0 {
            best = (d, run + s);
        }
        run += dist(w[0], w[1]);
    }
    best
}

fn path_len(pts: &[P]) -> f64 {
    pts.windows(2).map(|w| dist(w[0], w[1])).sum()
}

fn dist(a: P, b: P) -> f64 {
    (b.0 - a.0).hypot(b.1 - a.1)
}

fn inside(pts: &[P], u: f64, v: f64) -> bool {
    let mut c = false;
    let mut j = pts.len() - 1;
    for (i, &(xi, yi)) in pts.iter().enumerate() {
        let (xj, yj) = pts[j];
        if (yi > v) != (yj > v) && u < (xj - xi) * (v - yi) / (yj - yi) + xi {
            c = !c;
        }
        j = i;
    }
    c
}

/// The unit vector from a to b.
fn unit(a: P, b: P) -> P {
    let l = dist(a, b).max(1e-9);
    ((b.0 - a.0) / l, (b.1 - a.1) / l)
}

/// The point `along` units in direction d from o and `across` to its right.
fn fr(o: P, d: P, along: f64, across: f64) -> P {
    (
        o.0 + d.0 * along - d.1 * across,
        o.1 + d.1 * along + d.0 * across,
    )
}

fn polar(o: P, r: f64, deg: f64) -> P {
    let a = deg.to_radians();
    (o.0 + r * a.cos(), o.1 + r * a.sin())
}

fn arc_pts(o: P, r: f64, a0: f64, a1: f64, n: usize) -> Vec<P> {
    (0..=n)
        .map(|i| polar(o, r, a0 + (a1 - a0) * i as f64 / n as f64))
        .collect()
}

fn bezier(a: P, c: P, b: P, n: usize) -> Vec<P> {
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let (k0, k1, k2) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t), t * t);
            (
                k0 * a.0 + k1 * c.0 + k2 * b.0,
                k0 * a.1 + k1 * c.1 + k2 * b.1,
            )
        })
        .collect()
}

fn star_pts(o: P, r0: f64, r1: f64, n: usize, a0: f64) -> Vec<P> {
    (0..n * 2)
        .map(|i| {
            let r = if i % 2 == 0 { r0 } else { r1 };
            polar(o, r, a0 + i as f64 * 180.0 / n as f64)
        })
        .collect()
}

fn spiral_pts(o: P, r0: f64, r1: f64, turns: f64) -> Vec<P> {
    let n = (turns * 28.0) as usize;
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            polar(o, r0 + (r1 - r0) * t, 360.0 * turns * t)
        })
        .collect()
}

// ---- shared parts ----

/// A blade with a cross guard, a grip and a pommel.
fn blade(g: &mut Gl, base: P, tip: P, hw: f64, guard: f64, grip: f64) {
    let d = unit(base, tip);
    let near = fr(tip, d, -hw * 1.8, 0.0);
    g.poly(
        &[
            fr(base, d, 0.0, hw),
            fr(near, d, 0.0, hw),
            tip,
            fr(near, d, 0.0, -hw),
            fr(base, d, 0.0, -hw),
        ],
        STEEL,
    );
    g.seg(
        fr(base, d, 0.0, guard),
        fr(base, d, 0.0, -guard),
        1.5,
        C_GOLD,
    );
    let end = fr(base, d, -grip, 0.0);
    g.seg(base, end, 1.2, WOOD);
    g.disc(fr(end, d, -0.4, 0.0), 1.0, C_GOLD);
    g.flat(|g| g.seg(fr(base, d, 1.2, 0.0), fr(near, d, -0.2, 0.0), 0.6, STEEL_HI));
}

fn arrowhead(g: &mut Gl, tip: P, d: P, len: f64, hw: f64, c: Rgba) {
    g.poly(
        &[
            tip,
            fr(tip, d, -len, hw),
            fr(tip, d, -len * 0.62, 0.0),
            fr(tip, d, -len, -hw),
        ],
        c,
    );
}

fn arrow(g: &mut Gl, tail: P, tip: P, k: f64) {
    let d = unit(tail, tip);
    g.seg(tail, fr(tip, d, -1.0, 0.0), 1.1, WOOD);
    arrowhead(g, tip, d, 3.2 * k, 1.9 * k, STEEL);
    let c = g.c;
    for s in [0.0, 1.3 * k] {
        g.seg(fr(tail, d, s, 0.0), fr(tail, d, s - 1.4, 1.5 * k), 0.9, c);
        g.seg(fr(tail, d, s, 0.0), fr(tail, d, s - 1.4, -1.5 * k), 0.9, c);
    }
}

/// A heater shield: a rim and a face.
fn heater(g: &mut Gl, o: P, w: f64, h: f64, face: Rgba) {
    let pts = |i: f64| {
        let (x0, x1, y0) = (o.0 - w / 2.0 + i, o.0 + w / 2.0 - i, o.1 + i);
        vec![
            (x0, y0),
            (x1, y0),
            (x1, o.1 + h * 0.45),
            (x1 - w * 0.15, o.1 + h * 0.72),
            (o.0, o.1 + h - i * 1.3),
            (x0 + w * 0.15, o.1 + h * 0.72),
            (x0, o.1 + h * 0.45),
        ]
    };
    g.poly(&pts(0.0), STEEL);
    g.poly(&pts(1.3), face);
}

fn droplet(g: &mut Gl, o: P, r: f64, c: Rgba) {
    g.disc(o, r, c);
    g.poly(
        &[
            (o.0 - r * 0.95, o.1 - r * 0.3),
            (o.0 + r * 0.95, o.1 - r * 0.3),
            (o.0, o.1 - r * 2.1),
        ],
        c,
    );
}

fn flame(g: &mut Gl, o: P, s: f64, c: Rgba) {
    let f = |x: f64, y: f64| (o.0 + x * s, o.1 + y * s);
    g.disc(f(0.0, 2.6), 4.6 * s, c);
    g.poly(
        &[
            f(-4.5, 2.0),
            f(4.5, 2.0),
            f(1.6, -7.6),
            f(0.4, -3.6),
            f(-2.2, -5.8),
        ],
        c,
    );
    g.flat(|g| {
        let h = mix(c, rgb(255, 250, 200), 0.6);
        g.disc(f(0.0, 3.6), 2.6 * s, h);
        g.poly(&[f(-2.5, 3.2), f(2.5, 3.2), f(0.6, -2.4)], h);
        g.disc(f(0.0, 4.4), 1.2 * s, WHITE);
    });
}

fn cloud(g: &mut Gl, o: P, s: f64, c: Rgba) {
    let f = |x: f64, y: f64| (o.0 + x * s, o.1 + y * s);
    g.disc(f(-3.0, 0.8), 3.2 * s, c);
    g.disc(f(0.6, -1.4), 4.2 * s, c);
    g.disc(f(4.0, 1.0), 3.2 * s, c);
    g.rect(f(-3.0, 0.8), f(4.0, 4.2), c);
}

fn leaf(g: &mut Gl, a: P, b: P, w: f64, c: Rgba) {
    let d = unit(a, b);
    let len = dist(a, b);
    let side = |i: usize, k: f64| {
        let t = i as f64 / 12.0;
        fr(a, d, len * t, k * w * (PI * t).sin().powf(0.8))
    };
    let pts: Vec<P> = (0..=12)
        .map(|i| side(i, 1.0))
        .chain((1..12).rev().map(|i| side(i, -0.9)))
        .collect();
    g.poly(&pts, c);
}

fn sparkle(g: &mut Gl, o: P, r: f64, c: Rgba) {
    g.poly(&star_pts(o, r, r * 0.3, 4, -90.0), c);
}

/// Whether (u, v) is inside a person's silhouette centred at o, s times
/// the full size.
fn in_figure(o: P, s: f64, u: f64, v: f64) -> bool {
    let (x, y) = ((u - o.0) / s, (v - o.1) / s);
    x.hypot(y + 4.2) <= 3.0 || (x / 5.6).powi(2) + ((y - 5.2) / 5.4).powi(2) <= 1.0 && y <= 7.6
}

fn figure(g: &mut Gl, o: P, s: f64, c: Rgba) {
    g.each(c, |u, v| in_figure(o, s, u, v));
}

fn zee(g: &mut Gl, o: P, s: f64, c: Rgba) {
    let w = (1.3 * s / 5.0).max(0.95);
    g.path(
        &[
            (o.0, o.1),
            (o.0 + s, o.1),
            (o.0, o.1 + s),
            (o.0 + s, o.1 + s),
        ],
        w,
        c,
    );
}

fn wing(g: &mut Gl, mirror: bool, c: Rgba) {
    let m = |p: P| if mirror { (16.0 - p.0, p.1) } else { p };
    let pts: Vec<P> = [
        (7.4, 7.6),
        (0.6, 1.6),
        (0.2, 5.2),
        (1.0, 8.4),
        (2.8, 11.0),
        (5.4, 12.6),
        (7.4, 11.0),
    ]
    .into_iter()
    .map(m)
    .collect();
    g.poly(&pts, c);
    g.flat(|g| {
        let lo = g.lo();
        g.seg(m((1.2, 5.6)), m((6.2, 9.0)), 0.7, lo);
        g.seg(m((2.2, 8.6)), m((6.4, 10.6)), 0.7, lo);
    });
}

// ---- the pictograms ----

fn g_sword(g: &mut Gl) {
    blade(g, (4.5, 11.5), (14.8, 1.2), 1.25, 2.8, 3.2);
}

fn g_dagger(g: &mut Gl) {
    blade(g, (6.5, 9.5), (13.8, 2.2), 1.25, 2.2, 3.4);
}

fn g_swords(g: &mut Gl) {
    blade(g, (4.5, 11.5), (14.5, 1.5), 1.0, 2.3, 2.8);
    blade(g, (11.5, 11.5), (1.5, 1.5), 1.0, 2.3, 2.8);
}

fn g_axe(g: &mut Gl) {
    let (a, b) = ((3.0, 14.8), (11.8, 2.6));
    g.seg(a, b, 1.5, WOOD);
    let d = unit(a, b);
    let h = fr(b, d, -2.0, 0.0);
    let f = |x, y| fr(h, d, x, y);
    g.poly(
        &[
            f(-1.4, 0.4),
            f(1.4, 0.4),
            f(3.6, -4.4),
            f(0.0, -5.8),
            f(-3.6, -4.4),
        ],
        STEEL,
    );
    g.poly(&[f(-1.0, 0.0), f(1.0, 0.0), f(0.0, 2.6)], STEEL_LO);
    g.flat(|g| g.path(&[f(3.6, -4.4), f(0.0, -5.8), f(-3.6, -4.4)], 0.8, STEEL_HI));
}

fn g_hammer(g: &mut Gl) {
    let (a, b) = ((3.0, 14.8), (10.6, 4.4));
    g.seg(a, b, 1.5, WOOD);
    let d = unit(a, b);
    let f = |x, y| fr(b, d, x, y);
    g.poly(
        &[f(-2.2, -4.0), f(2.2, -4.0), f(2.2, 4.0), f(-2.2, 4.0)],
        STEEL,
    );
    g.flat(|g| g.seg(f(-2.2, 0.0), f(2.2, 0.0), 1.0, C_GOLD));
}

fn g_spear(g: &mut Gl) {
    let (a, b) = ((1.2, 14.8), (10.4, 5.6));
    g.seg(a, b, 1.3, WOOD);
    let d = unit(a, b);
    let f = |x, y| fr(b, d, x, y);
    g.poly(
        &[f(-0.6, 0.0), f(2.0, 1.9), f(6.2, 0.0), f(2.0, -1.9)],
        STEEL,
    );
    g.seg(f(-0.6, -1.5), f(-0.6, 1.5), 1.2, C_GOLD);
    g.flat(|g| g.seg(f(0.4, 0.0), f(5.0, 0.0), 0.6, STEEL_HI));
}

fn g_bow(g: &mut Gl) {
    let (a, b) = ((1.6, 4.4), (11.6, 14.4));
    g.taper(&bezier(a, (15.6, 0.4), b, 16), 2.4, WOOD);
    g.flat(|g| g.seg(a, b, 0.7, STEEL_HI));
    arrow(g, (3.4, 12.6), (13.6, 2.4), 0.85);
}

fn g_arrow(g: &mut Gl) {
    arrow(g, (2.2, 13.8), (14.6, 1.4), 1.0);
}

fn g_volley(g: &mut Gl) {
    let d = unit((0.0, 16.0), (16.0, 0.0));
    for k in [-3.8, 0.0, 3.8] {
        let o = fr((8.0, 8.0), d, 0.0, k);
        arrow(g, fr(o, d, -5.4, 0.0), fr(o, d, 5.2, 0.0), 0.75);
    }
}

fn g_arrow_rain(g: &mut Gl) {
    for (x, y) in [(3.0, 0.4), (8.4, 4.0), (13.6, 0.4)] {
        arrow(g, (x + 1.4, y), (x - 0.6, y + 11.4), 0.85);
    }
}

fn g_shield(g: &mut Gl) {
    let c = g.c;
    heater(g, (8.0, 1.2), 11.8, 14.0, c);
    g.flat(|g| g.disc((8.0, 6.6), 1.3, C_GOLD));
}

fn g_shields(g: &mut Gl) {
    let c = g.c;
    for x in [3.0, 13.0] {
        heater(g, (x, 2.6), 6.4, 10.0, mul(c, 0.8));
    }
    heater(g, (8.0, 4.0), 7.8, 11.6, c);
}

fn g_broken(g: &mut Gl) {
    g_shield(g);
    let crack = [(9.0, 0.0), (7.0, 4.6), (9.6, 7.6), (6.8, 11.0), (8.4, 16.0)];
    g.erase(|u, v| path_dist(&crack, (u, v)).0 <= 0.75);
}

fn g_helm(g: &mut Gl) {
    let c = g.c;
    g.poly(&[(6.4, 2.6), (9.6, 2.6), (11.6, 0.0), (7.2, 0.2)], c);
    g.disc((8.0, 7.6), 5.8, STEEL);
    g.rect((2.2, 7.6), (13.8, 14.8), STEEL);
    g.flat(|g| {
        g.rect((3.4, 8.0), (12.6, 9.4), INK);
        g.rect((7.4, 9.4), (8.6, 13.4), INK);
    });
}

fn g_armor(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[
            (1.2, 3.2),
            (5.4, 1.8),
            (8.0, 4.0),
            (10.6, 1.8),
            (14.8, 3.2),
            (13.8, 7.6),
            (12.2, 8.4),
            (12.2, 14.8),
            (3.8, 14.8),
            (3.8, 8.4),
            (2.2, 7.6),
        ],
        STEEL,
    );
    g.flat(|g| {
        g.rect((3.8, 11.0), (12.2, 12.6), c);
        g.seg((8.0, 4.8), (8.0, 10.6), 0.8, STEEL_LO);
        g.disc((8.0, 11.8), 1.0, C_GOLD);
    });
}

fn g_banner(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[
            (4.4, 2.0),
            (14.8, 2.0),
            (12.2, 6.0),
            (14.8, 10.0),
            (4.4, 10.0),
        ],
        c,
    );
    g.seg((3.6, 1.0), (3.6, 15.6), 1.4, WOOD);
    g.disc((3.6, 1.0), 1.2, C_GOLD);
    g.flat(|g| g.poly(&star_pts((8.4, 6.0), 2.6, 1.1, 5, -90.0), C_GOLD));
}

fn g_crown(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[
            (1.6, 13.4),
            (1.6, 4.6),
            (4.8, 8.6),
            (8.0, 2.8),
            (11.2, 8.6),
            (14.4, 4.6),
            (14.4, 13.4),
        ],
        C_GOLD,
    );
    for o in [(1.6, 4.0), (8.0, 2.2), (14.4, 4.0)] {
        g.disc(o, 1.3, C_GOLD);
    }
    g.flat(|g| {
        g.rect((1.6, 12.0), (14.4, 13.4), GOLD_LO);
        for x in [4.6, 8.0, 11.4] {
            g.disc((x, 10.0), 1.1, c);
        }
    });
}

fn g_fist(g: &mut Gl) {
    let c = g.c;
    for x in [2.6, 5.4, 8.2, 11.0] {
        g.rect((x, 4.6), (x + 2.6, 9.0), c);
        g.disc((x + 1.3, 4.6), 1.3, c);
    }
    g.rect((2.6, 7.0), (13.6, 12.6), c);
    g.oval((8.1, 12.4), 5.5, 2.6, c);
    g.poly(
        &[(1.2, 8.6), (4.4, 7.4), (9.6, 9.0), (9.6, 10.8), (4.2, 11.2)],
        c,
    );
    g.flat(|g| {
        let lo = g.lo();
        for x in [5.4, 8.2, 11.0] {
            g.seg((x, 3.6), (x, 7.2), 0.7, lo);
        }
        g.path(&[(1.2, 8.6), (4.4, 7.4), (9.6, 9.0)], 0.7, lo);
    });
}

fn g_palm(g: &mut Gl) {
    let c = g.c;
    for (x, top) in [(4.6, 3.4), (7.2, 1.6), (9.8, 1.8), (12.2, 3.8)] {
        g.seg((x, 9.0), (x, top), 2.1, c);
    }
    g.seg((4.4, 11.6), (1.8, 7.8), 2.1, c);
    g.oval((8.4, 10.6), 4.4, 4.0, c);
    g.flat(|g| {
        let h = g.hi();
        g.disc((8.4, 10.6), 1.7, h);
        g.disc((8.4, 10.6), 0.8, WHITE);
    });
}

fn g_claw(g: &mut Gl) {
    let c = g.c;
    for k in [0.0, 3.8, 7.6] {
        g.taper(&[(5.8 + k, 0.6), (4.6 + k, 7.6), (1.6 + k, 15.2)], 3.0, c);
    }
}

fn g_slash(g: &mut Gl) {
    let c = g.c;
    g.taper(&arc_pts((1.0, 15.0), 12.6, -88.0, -2.0, 16), 3.8, c);
    g.flat(|g| g.taper(&arc_pts((1.0, 15.0), 11.8, -80.0, -10.0, 12), 1.4, WHITE));
}

fn g_whirl(g: &mut Gl) {
    let c = g.c;
    for a in [0.0, 120.0, 240.0] {
        g.taper(&arc_pts((8.0, 8.0), 6.0, a, a + 110.0, 10), 2.8, c);
        g.taper(&arc_pts((8.0, 8.0), 2.8, a + 60.0, a + 150.0, 6), 1.6, c);
    }
    g.flat(|g| g.disc((8.0, 8.0), 0.9, WHITE));
}

fn g_heart(g: &mut Gl) {
    let c = g.c;
    g.disc((5.3, 5.8), 3.4, c);
    g.disc((10.7, 5.8), 3.4, c);
    g.poly(&[(2.0, 6.8), (14.0, 6.8), (8.0, 14.4)], c);
    g.flat(|g| g.oval((4.6, 4.8), 1.0, 0.8, WHITE));
}

fn g_drop(g: &mut Gl) {
    let c = g.c;
    droplet(g, (8.0, 10.2), 4.4, c);
    g.flat(|g| g.oval((6.2, 9.6), 0.8, 1.6, alpha(WHITE, 200)));
}

fn g_poison(g: &mut Gl) {
    let c = g.c;
    droplet(g, (6.4, 10.6), 4.2, c);
    g.ring((12.6, 3.6), 1.7, 1.0, c);
    g.ring((13.2, 8.8), 1.2, 0.9, c);
    g.flat(|g| {
        g.oval((6.0, 10.4), 2.0, 1.6, INK_A);
        g.disc((5.2, 10.2), 0.6, g.hi());
        g.disc((6.8, 10.2), 0.6, g.hi());
    });
}

fn g_mana(g: &mut Gl) {
    let c = g.c;
    g.poly(&[(8.0, 0.6), (13.4, 6.2), (8.0, 15.4), (2.6, 6.2)], c);
    g.flat(|g| {
        let h = g.hi();
        g.path(&[(2.8, 6.2), (13.2, 6.2)], 0.7, h);
        g.path(&[(8.0, 1.0), (6.0, 6.2), (8.0, 15.0)], 0.7, h);
    });
}

fn g_plus(g: &mut Gl) {
    let c = g.c;
    g.rect((5.8, 1.8), (10.2, 14.2), c);
    g.rect((1.8, 5.8), (14.2, 10.2), c);
    g.flat(|g| {
        let h = g.hi();
        g.rect((7.2, 3.2), (8.8, 12.8), h);
        g.rect((3.2, 7.2), (12.8, 8.8), h);
    });
}

fn g_eye(g: &mut Gl) {
    let c = g.c;
    let (k, r) = (4.1, 8.1);
    g.each(rgb(0xec, 0xec, 0xf4), |u, v| {
        (u - 8.0).hypot(v - 8.0 - k) <= r && (u - 8.0).hypot(v - 8.0 + k) <= r
    });
    g.flat(|g| {
        g.disc((8.0, 8.0), 3.1, c);
        g.disc((8.0, 8.0), 1.4, INK);
        g.disc((7.0, 7.0), 0.6, WHITE);
    });
}

fn g_skull(g: &mut Gl) {
    let c = g.c;
    g.disc((8.0, 6.6), 5.7, C_BONE);
    g.rect((4.8, 9.6), (11.2, 14.6), C_BONE);
    g.flat(|g| {
        for x in [5.6, 10.4] {
            g.disc((x, 7.4), 1.8, INK);
            g.disc((x, 7.4), 0.8, c);
        }
        g.poly(&[(8.0, 9.4), (6.9, 11.4), (9.1, 11.4)], INK);
        for x in [6.4, 8.0, 9.6] {
            g.seg((x, 12.8), (x, 14.6), 0.7, INK);
        }
    });
}

fn g_bones(g: &mut Gl) {
    for (a, b) in [((3.4, 3.4), (12.6, 12.6)), ((12.6, 3.4), (3.4, 12.6))] {
        g.seg(a, b, 2.0, C_BONE);
        for (e, o) in [(a, b), (b, a)] {
            let d = unit(o, e);
            g.disc(fr(e, d, 0.0, 1.1), 1.3, C_BONE);
            g.disc(fr(e, d, 0.0, -1.1), 1.3, C_BONE);
        }
    }
}

fn g_scythe(g: &mut Gl) {
    g.seg((4.6, 15.4), (10.8, 1.2), 1.4, WOOD);
    g.each(STEEL, |u, v| {
        (u - 5.0).hypot(v - 5.6) <= 5.8 && (u - 4.0).hypot(v - 8.2) > 5.8 && u < 10.6
    });
}

fn g_flask(g: &mut Gl) {
    let c = g.c;
    g.disc((8.0, 10.4), 4.9, GLASS);
    g.rect((6.5, 3.0), (9.5, 7.0), GLASS);
    g.rect((6.0, 0.8), (10.0, 3.2), WOOD);
    g.flat(|g| {
        g.each(c, |u, v| (u - 8.0).hypot(v - 10.4) <= 4.1 && v > 9.0);
        let h = g.hi();
        g.disc((9.4, 11.6), 0.7, h);
        g.disc((7.4, 13.2), 0.6, h);
        g.oval((5.8, 9.2), 0.7, 1.4, WHITE);
    });
}

fn g_bomb(g: &mut Gl) {
    let c = g.c;
    g.disc((7.2, 9.8), 5.4, IRON);
    g.rect((8.6, 3.4), (11.4, 5.8), STEEL_LO);
    g.flat(|g| {
        g.path(&[(10.2, 3.6), (11.4, 2.2), (13.0, 1.8)], 0.9, WOOD);
        sparkle(g, (13.6, 1.6), 2.6, c);
        g.disc((13.6, 1.6), 0.7, WHITE);
        g.oval((5.2, 7.6), 1.4, 1.0, alpha(WHITE, 150));
    });
}

fn g_chalice(g: &mut Gl) {
    let c = g.c;
    g.each(C_GOLD, |u, v| v >= 2.8 && (u - 8.0).hypot(v - 2.8) <= 5.8);
    g.rect((1.8, 1.8), (14.2, 3.4), C_GOLD);
    g.rect((7.2, 8.0), (8.8, 12.6), C_GOLD);
    g.disc((8.0, 10.2), 1.3, C_GOLD);
    g.oval((8.0, 13.6), 4.2, 1.4, C_GOLD);
    g.flat(|g| {
        g.rect((2.8, 2.0), (13.2, 3.0), c);
        g.disc((8.0, 5.6), 1.1, c);
    });
}

fn g_horn(g: &mut Gl) {
    let pts = bezier((1.6, 13.0), (5.0, 2.0), (13.6, 5.6), 16);
    let n = pts.len() - 1;
    for (i, &p) in pts.iter().enumerate() {
        let t = i as f64 / n as f64;
        g.disc(p, 0.7 + 2.6 * t * t, HORN);
    }
    g.flat(|g| {
        for &i in &[8, 12] {
            g.disc(pts[i], 0.9 + 2.6 * (i as f64 / n as f64).powi(2), C_GOLD);
        }
        g.oval((14.0, 6.4), 1.2, 2.2, INK_A);
    });
}

fn g_note(g: &mut Gl) {
    let c = g.c;
    g.oval((4.4, 12.6), 2.4, 1.8, c);
    g.oval((11.6, 11.0), 2.4, 1.8, c);
    g.seg((6.4, 12.4), (6.4, 3.6), 1.3, c);
    g.seg((13.6, 10.8), (13.6, 2.0), 1.3, c);
    g.poly(&[(5.8, 3.0), (14.2, 1.2), (14.2, 4.0), (5.8, 5.8)], c);
}

fn g_book(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[
            (0.4, 4.6),
            (8.0, 6.2),
            (15.6, 4.6),
            (15.6, 14.0),
            (8.0, 15.6),
            (0.4, 14.0),
        ],
        c,
    );
    g.poly(&[(1.2, 3.4), (7.6, 4.8), (7.6, 14.2), (1.2, 12.8)], PAGE);
    g.poly(&[(8.4, 4.8), (14.8, 3.4), (14.8, 12.8), (8.4, 14.2)], PAGE);
    g.flat(|g| {
        for k in 0..3 {
            let y = 6.8 + k as f64 * 2.2;
            g.seg((2.4, y - 0.6), (6.6, y + 0.4), 0.7, INK_A);
            g.seg((9.4, y + 0.4), (13.6, y - 0.6), 0.7, INK_A);
        }
    });
}

fn g_rune(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[
            (4.0, 2.2),
            (11.6, 1.2),
            (13.8, 12.6),
            (9.0, 15.2),
            (2.8, 13.6),
            (2.2, 6.0),
        ],
        rgb(0x6a, 0x6a, 0x7a),
    );
    g.flat(|g| {
        let h = mix(c, WHITE, 0.15);
        g.seg((8.0, 4.0), (8.0, 12.8), 1.6, h);
        g.seg((8.0, 8.0), (4.8, 4.4), 1.5, h);
        g.seg((8.0, 8.0), (11.2, 4.4), 1.5, h);
    });
}

fn g_seal(g: &mut Gl) {
    let c = g.c;
    g.ring((8.0, 8.0), 6.8, 1.4, c);
    g.flat(|g| {
        let tri = |a0: f64| -> Vec<P> {
            (0..4)
                .map(|i| polar((8.0, 8.0), 5.6, a0 + i as f64 * 120.0))
                .collect()
        };
        g.path(&tri(-90.0), 0.9, c);
        g.path(&tri(90.0), 0.9, c);
        g.disc((8.0, 8.0), 1.3, WHITE);
    });
}

fn g_chain(g: &mut Gl) {
    g.oval_ring((4.0, 12.0), (3.6, 2.2), -45.0, 1.4, STEEL);
    g.oval_ring((12.0, 4.0), (3.6, 2.2), -45.0, 1.4, STEEL);
    g.seg((5.4, 10.6), (10.6, 5.4), 1.9, STEEL);
}

fn g_trap(g: &mut Gl) {
    let o = (7.4, 7.4);
    g.path(&[(11.4, 11.4), (13.0, 13.6), (15.4, 14.0)], 1.2, STEEL_LO);
    g.ring(o, 6.2, 1.8, STEEL);
    for i in 0..10 {
        let a = i as f64 * 36.0;
        g.poly(
            &[
                polar(o, 5.4, a - 10.0),
                polar(o, 5.4, a + 10.0),
                polar(o, 2.8, a),
            ],
            STEEL,
        );
    }
    g.disc(o, 1.8, IRON);
}

fn g_web(g: &mut Gl) {
    let c = g.c;
    for i in 0..8 {
        let a = i as f64 * 45.0 + 22.5;
        g.seg((8.0, 8.0), polar((8.0, 8.0), 7.8, a), 1.0, c);
    }
    for r in [2.8, 5.6] {
        let pts: Vec<P> = (0..=8)
            .map(|i| polar((8.0, 8.0), r, i as f64 * 45.0 + 22.5))
            .collect();
        g.path(&pts, 1.0, c);
    }
}

fn g_coin(g: &mut Gl) {
    g.disc((8.0, 8.0), 6.4, C_GOLD);
    g.flat(|g| {
        g.ring((8.0, 8.0), 4.6, 0.9, GOLD_LO);
        g.rect((7.2, 5.4), (8.8, 10.6), GOLD_LO);
    });
}

fn g_gem(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[
            (4.6, 2.6),
            (11.4, 2.6),
            (14.6, 6.4),
            (8.0, 14.8),
            (1.4, 6.4),
        ],
        c,
    );
    g.flat(|g| {
        let h = g.hi();
        g.path(&[(1.8, 6.4), (14.2, 6.4)], 0.7, h);
        g.path(&[(4.6, 2.6), (6.2, 6.4), (8.0, 14.4)], 0.6, h);
        g.path(&[(11.4, 2.6), (9.8, 6.4), (8.0, 14.4)], 0.6, h);
    });
}

fn g_hourglass(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[
            (3.8, 2.4),
            (12.2, 2.4),
            (8.9, 8.0),
            (12.2, 13.6),
            (3.8, 13.6),
            (7.1, 8.0),
        ],
        GLASS,
    );
    g.rect((2.2, 0.8), (13.8, 2.6), WOOD);
    g.rect((2.2, 13.4), (13.8, 15.2), WOOD);
    g.seg((2.8, 2.0), (2.8, 14.0), 1.0, WOOD);
    g.seg((13.2, 2.0), (13.2, 14.0), 1.0, WOOD);
    g.flat(|g| {
        g.poly(&[(5.4, 4.6), (10.6, 4.6), (8.0, 7.8)], c);
        g.poly(&[(4.6, 13.4), (11.4, 13.4), (8.0, 10.4)], c);
        g.seg((8.0, 7.4), (8.0, 11.0), 0.7, c);
    });
}

fn g_clock(g: &mut Gl) {
    let c = g.c;
    g.disc((8.0, 8.0), 7.0, c);
    g.flat(|g| {
        g.disc((8.0, 8.0), 5.6, rgb(0xf0, 0xea, 0xd8));
        for a in [0.0, 90.0, 180.0, 270.0] {
            g.disc(polar((8.0, 8.0), 4.6, a), 0.6, INK);
        }
        g.seg((8.0, 8.0), (8.0, 3.8), 1.1, INK);
        g.seg((8.0, 8.0), (11.0, 9.6), 1.1, INK);
        g.disc((8.0, 8.0), 0.8, c);
    });
}

fn g_rewind(g: &mut Gl) {
    let c = g.c;
    let (o, r, head) = ((8.4, 8.4), 5.4, 200.0f64);
    g.arc(o, r, 2.0, (head, head + 270.0), c);
    let a = head.to_radians();
    let p = polar(o, r, head);
    let (t, n) = ((a.sin(), -a.cos()), (a.cos(), a.sin()));
    g.poly(
        &[
            (p.0 + t.0 * 3.6, p.1 + t.1 * 3.6),
            (p.0 + n.0 * 3.0, p.1 + n.1 * 3.0),
            (p.0 - n.0 * 3.0, p.1 - n.1 * 3.0),
        ],
        c,
    );
}

fn g_tower(g: &mut Gl) {
    g.rect((3.6, 5.0), (12.4, 15.4), STONE_C);
    g.rect((2.6, 4.0), (13.4, 6.0), STONE_C);
    for x in [2.6, 6.6, 10.6] {
        g.rect((x, 1.2), (x + 2.8, 4.4), STONE_C);
    }
    g.flat(|g| {
        g.disc((8.0, 12.2), 1.9, INK);
        g.rect((6.1, 12.2), (9.9, 15.4), INK);
        g.rect((7.4, 7.2), (8.6, 9.6), INK);
    });
}

fn g_gate(g: &mut Gl) {
    let c = g.c;
    g.each(c, |u, v| {
        (u - 8.0).hypot(v - 7.2) <= 4.4 || (3.6..12.4).contains(&u) && (7.2..15.2).contains(&v)
    });
    g.flat(|g| {
        g.path(
            &spiral_pts((8.0, 9.6), 0.4, 3.4, 1.8),
            0.9,
            mix(c, WHITE, 0.45),
        )
    });
    g.arc((8.0, 7.2), 5.4, 2.2, (180.0, 360.0), STONE_C);
    g.rect((1.6, 7.0), (4.4, 15.6), STONE_C);
    g.rect((11.6, 7.0), (14.4, 15.6), STONE_C);
}

fn g_spiral(g: &mut Gl) {
    let c = g.c;
    g.path(&spiral_pts((8.0, 8.0), 0.6, 7.0, 2.3), 1.8, c);
}

fn g_flame(g: &mut Gl) {
    let c = g.c;
    flame(g, (8.0, 8.0), 1.0, c);
}

fn g_fireball(g: &mut Gl) {
    let c = g.c;
    g.poly(&[(6.4, 3.2), (12.8, 9.6), (0.8, 15.2)], c);
    g.disc((10.2, 5.8), 4.4, c);
    g.flat(|g| {
        let h = mix(c, rgb(255, 250, 200), 0.6);
        g.disc((10.6, 5.4), 2.6, h);
        g.disc((11.0, 5.0), 1.1, WHITE);
        g.seg((7.4, 8.6), (3.0, 13.0), 0.9, h);
    });
}

fn g_meteor(g: &mut Gl) {
    let c = g.c;
    g.poly(&[(8.0, 3.6), (12.4, 8.0), (0.6, 15.4)], c);
    g.spike(&[(12.4, 8.6), (5.4, 15.6)], 2.0, c);
    g.spike(&[(7.4, 3.6), (0.4, 9.6)], 2.0, c);
    g.disc((11.2, 4.8), 3.8, ROCK);
    g.flat(|g| {
        let lo = mul(ROCK, 0.7);
        g.disc((10.2, 3.8), 0.8, lo);
        g.disc((12.4, 6.0), 0.6, lo);
        g.arc((11.2, 4.8), 3.6, 1.0, (100.0, 250.0), mix(c, WHITE, 0.4));
    });
}

fn g_snowflake(g: &mut Gl) {
    let c = g.c;
    let o = (8.0, 8.0);
    for i in 0..6 {
        let a = i as f64 * 60.0 - 90.0;
        g.seg(o, polar(o, 7.4, a), 1.5, c);
        let m = polar(o, 4.4, a);
        g.seg(m, polar(m, 2.5, a - 50.0), 1.1, c);
        g.seg(m, polar(m, 2.5, a + 50.0), 1.1, c);
    }
    g.flat(|g| g.disc(o, 1.3, WHITE));
}

fn g_icicle(g: &mut Gl) {
    let c = g.c;
    let d = unit((0.0, 16.0), (16.0, 0.0));
    let f = |x, y| fr((8.0, 8.0), d, x, y);
    g.poly(&[f(-9.4, 0.0), f(2.6, 2.6), f(9.8, 0.0), f(2.6, -2.6)], c);
    g.flat(|g| {
        let h = mix(c, WHITE, 0.6);
        g.path(&[f(-7.6, 0.0), f(2.6, -1.2), f(8.4, 0.0)], 0.8, h);
    });
}

fn g_bolt(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[
            (10.4, 0.4),
            (3.0, 9.2),
            (7.4, 9.2),
            (5.0, 15.6),
            (13.2, 6.2),
            (8.8, 6.2),
            (12.0, 0.4),
        ],
        c,
    );
    g.flat(|g| {
        g.path(
            &[(10.6, 1.6), (5.6, 8.2), (8.4, 8.2), (7.0, 12.6)],
            0.6,
            WHITE,
        )
    });
}

fn g_forked(g: &mut Gl) {
    let c = g.c;
    let paths: [&[P]; 3] = [
        &[(2.0, 0.8), (6.4, 5.8), (4.4, 8.0), (9.0, 10.0), (15.0, 8.4)],
        &[(9.0, 10.0), (7.6, 12.4), (11.0, 15.4)],
        &[(6.4, 5.8), (10.6, 4.4), (12.8, 0.8)],
    ];
    for p in paths {
        g.path(p, 1.9, c);
    }
    g.flat(|g| {
        for p in paths {
            g.path(p, 0.6, WHITE);
        }
    });
}

fn g_cloud(g: &mut Gl) {
    let c = g.c;
    cloud(g, (8.0, 8.0), 1.0, c);
}

fn g_storm(g: &mut Gl) {
    let c = g.c;
    cloud(g, (8.0, 5.0), 1.0, c);
    g.poly(
        &[
            (9.4, 8.2),
            (5.8, 12.2),
            (8.0, 12.2),
            (6.6, 15.6),
            (11.4, 10.6),
            (9.0, 10.6),
            (10.8, 8.2),
        ],
        YELLOW,
    );
}

fn g_rain(g: &mut Gl) {
    let c = g.c;
    cloud(g, (8.0, 4.6), 0.95, c);
    g.flat(|g| {
        for x in [4.2, 7.8, 11.4] {
            g.seg((x, 10.4), (x - 1.2, 14.8), 1.2, mix(c, WHITE, 0.45));
        }
    });
}

fn g_wind(g: &mut Gl) {
    let c = g.c;
    g.path(
        &[
            (0.8, 5.4),
            (10.0, 5.4),
            (12.2, 4.4),
            (12.4, 2.6),
            (11.0, 1.8),
            (9.6, 2.6),
        ],
        1.4,
        c,
    );
    g.path(
        &[
            (2.4, 9.0),
            (12.6, 9.0),
            (14.6, 10.2),
            (14.4, 12.2),
            (12.8, 12.8),
            (11.6, 11.8),
        ],
        1.4,
        c,
    );
    g.path(&[(0.8, 12.8), (8.0, 12.8)], 1.4, c);
}

fn g_tornado(g: &mut Gl) {
    let c = g.c;
    let rows = [
        (1.0, 15.0, 0.0),
        (2.6, 13.6, 0.6),
        (4.2, 12.4, 0.2),
        (5.6, 11.4, -0.6),
        (6.6, 10.4, -0.2),
        (7.4, 9.8, 0.4),
    ];
    for (i, (x0, x1, sway)) in rows.into_iter().enumerate() {
        let y = 1.6 + i as f64 * 2.6;
        g.seg((x0 + sway, y), (x1 + sway, y), 1.8, c);
    }
}

fn g_wave(g: &mut Gl) {
    let c = g.c;
    for (i, y) in [3.4, 8.0, 12.6].into_iter().enumerate() {
        let pts: Vec<P> = (0..=32)
            .map(|k| {
                let x = k as f64 / 2.0;
                (x, y + 1.5 * (x * 0.85 + i as f64 * 0.9).sin())
            })
            .collect();
        g.path(&pts, 1.9, c);
    }
}

fn g_leaf(g: &mut Gl) {
    let c = g.c;
    leaf(g, (2.4, 13.6), (14.6, 1.4), 4.0, c);
    g.seg((0.8, 15.2), (2.8, 13.2), 1.3, c);
    g.flat(|g| {
        let lo = g.lo();
        g.seg((2.8, 13.2), (13.0, 3.0), 0.8, lo);
        for t in [0.35, 0.55, 0.75] {
            let p = (2.8 + 10.2 * t, 13.2 - 10.2 * t);
            g.seg(p, (p.0 - 0.6, p.1 - 2.4), 0.6, lo);
        }
    });
}

fn g_tree(g: &mut Gl) {
    let c = g.c;
    g.rect((7.0, 9.0), (9.0, 15.6), WOOD);
    g.poly(&[(4.8, 15.6), (7.0, 13.4), (9.0, 13.4), (11.2, 15.6)], WOOD);
    g.disc((8.0, 5.0), 4.4, c);
    g.disc((4.6, 8.0), 3.3, c);
    g.disc((11.4, 8.0), 3.3, c);
    g.disc((8.0, 9.0), 3.0, c);
}

fn g_treant(g: &mut Gl) {
    g_tree(g);
    g.flat(|g| {
        g.disc((6.0, 6.8), 1.0, YELLOW);
        g.disc((10.0, 6.8), 1.0, YELLOW);
        g.arc((8.0, 8.2), 2.2, 0.9, (30.0, 150.0), INK);
    });
}

fn g_roots(g: &mut Gl) {
    let c = g.c;
    for p in [
        [(2.6, 15.0), (1.6, 10.4), (4.0, 7.0), (2.6, 2.4)],
        [(8.0, 15.0), (9.6, 10.0), (6.6, 6.4), (8.4, 0.8)],
        [(13.4, 15.0), (14.6, 10.6), (12.2, 7.4), (13.6, 3.4)],
    ] {
        g.spike(&p, 3.0, c);
    }
    g.rect((0.4, 13.4), (15.6, 15.6), EARTH);
}

fn g_thorns(g: &mut Gl) {
    let c = g.c;
    let o = (8.0, 8.0);
    g.ring(o, 4.4, 2.2, c);
    for i in 0..9 {
        let a = i as f64 * 40.0 - 90.0;
        g.poly(
            &[
                polar(o, 4.6, a - 13.0),
                polar(o, 4.6, a + 13.0),
                polar(o, 7.9, a + 6.0),
            ],
            c,
        );
    }
}

fn g_rock(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[
            (1.4, 12.6),
            (2.4, 7.0),
            (6.0, 3.4),
            (10.6, 2.8),
            (14.2, 6.6),
            (14.6, 12.2),
            (10.2, 14.8),
            (4.4, 14.6),
        ],
        c,
    );
    g.flat(|g| {
        let lo = g.lo();
        g.path(&[(6.4, 4.0), (7.6, 8.0), (5.8, 11.4)], 0.9, lo);
        g.path(&[(7.6, 8.0), (11.4, 9.4)], 0.9, lo);
    });
}

fn g_quake(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[(0.0, 10.6), (16.0, 9.6), (16.0, 16.0), (0.0, 16.0)],
        EARTH,
    );
    g.flat(|g| {
        let cracks: [&[P]; 3] = [
            &[(8.0, 10.0), (7.0, 12.4), (8.6, 14.0), (7.6, 16.0)],
            &[(8.0, 10.0), (11.0, 12.0), (13.4, 15.2)],
            &[(8.0, 10.0), (4.6, 12.4), (2.6, 15.4)],
        ];
        for p in cracks {
            g.path(p, 1.0, INK);
        }
    });
    for (o, r) in [
        ((3.6, 6.0), 1.4),
        ((11.8, 4.6), 1.7),
        ((7.6, 2.2), 1.2),
        ((14.2, 1.2), 0.9),
        ((1.4, 2.0), 0.9),
    ] {
        g.disc(o, r, c);
    }
}

fn g_volcano(g: &mut Gl) {
    let c = g.c;
    g.poly(&[(0.2, 15.6), (5.4, 7.6), (10.6, 7.6), (15.8, 15.6)], ROCK);
    for (o, r) in [
        ((8.0, 4.4), 2.2),
        ((5.0, 2.4), 1.4),
        ((11.0, 2.0), 1.5),
        ((8.0, 0.8), 1.0),
    ] {
        g.disc(o, r, c);
    }
    g.flat(|g| {
        g.rect((5.4, 7.6), (10.6, 8.8), c);
        g.path(&[(6.4, 8.6), (5.0, 12.0), (5.6, 14.8)], 1.1, c);
        g.path(&[(9.8, 8.6), (11.4, 13.2)], 1.1, c);
    });
}

fn g_sun(g: &mut Gl) {
    let c = g.c;
    let o = (8.0, 8.0);
    for i in 0..8 {
        let r1 = if i % 2 == 0 { 7.8 } else { 7.0 };
        g.seg(
            polar(o, 5.0, i as f64 * 45.0),
            polar(o, r1, i as f64 * 45.0),
            1.6,
            c,
        );
    }
    g.disc(o, 4.0, c);
    g.flat(|g| g.disc((7.4, 7.4), 2.0, mix(c, WHITE, 0.55)));
}

fn g_dawn(g: &mut Gl) {
    let c = g.c;
    let o = (8.0, 12.2);
    for i in 0..7 {
        let a = 180.0 + i as f64 * 30.0;
        g.seg(polar(o, 6.0, a), polar(o, 8.4, a), 1.5, c);
    }
    g.each(c, |u, v| (u - 8.0).hypot(v - 12.2) <= 4.8 && v <= 12.2);
    g.flat(|g| g.rect((0.2, 12.6), (15.8, 13.8), mul(c, 0.7)));
}

fn g_moon(g: &mut Gl) {
    let c = g.c;
    g.each(c, |u, v| {
        (u - 8.0).hypot(v - 8.0) <= 6.6 && (u - 11.4).hypot(v - 5.8) > 5.4
    });
    g.flat(|g| {
        let lo = g.lo();
        g.disc((4.4, 9.4), 0.9, lo);
        g.disc((6.6, 12.4), 0.7, lo);
    });
}

fn g_star(g: &mut Gl) {
    let c = g.c;
    g.poly(&star_pts((8.0, 8.6), 7.6, 3.2, 5, -90.0), c);
    g.flat(|g| {
        g.poly(
            &star_pts((8.0, 8.6), 3.4, 1.4, 5, -90.0),
            mix(c, WHITE, 0.6),
        )
    });
}

fn g_stars(g: &mut Gl) {
    let c = g.c;
    let pts = [
        (2.6, 4.4),
        (8.4, 2.0),
        (13.2, 6.8),
        (10.0, 12.6),
        (4.2, 12.0),
    ];
    g.flat(|g| g.path(&pts, 0.8, alpha(mix(c, WHITE, 0.3), 170)));
    for (i, &p) in pts.iter().enumerate() {
        sparkle(g, p, if i % 2 == 0 { 3.2 } else { 2.4 }, c);
    }
}

fn g_burst(g: &mut Gl) {
    let c = g.c;
    let o = (8.0, 8.0);
    let pts: Vec<P> = (0..16)
        .map(|i| {
            let r = if i % 2 == 0 {
                [7.8, 6.6, 7.4, 6.2][i / 2 % 4]
            } else {
                3.6
            };
            polar(o, r, i as f64 * 22.5 - 80.0)
        })
        .collect();
    g.poly(&pts, c);
    g.flat(|g| {
        g.poly(
            &star_pts(o, 4.4, 2.0, 8, -70.0),
            mix(c, rgb(255, 250, 200), 0.6),
        );
        g.disc(o, 1.4, WHITE);
    });
}

fn g_nova(g: &mut Gl) {
    let c = g.c;
    g.ring((8.0, 8.0), 6.6, 1.5, c);
    g.ring((8.0, 8.0), 3.8, 1.3, c);
    g.flat(|g| {
        let h = g.hi();
        for i in 0..8 {
            let a = i as f64 * 45.0 + 22.5;
            g.seg(polar((8.0, 8.0), 4.6, a), polar((8.0, 8.0), 5.8, a), 0.9, h);
        }
        g.disc((8.0, 8.0), 1.6, WHITE);
    });
}

fn g_pillar(g: &mut Gl) {
    let c = g.c;
    g.oval((8.0, 13.4), 6.8, 2.0, c);
    g.rect((5.2, 0.0), (10.8, 13.0), c);
    g.flat(|g| {
        let h = g.hi();
        g.rect((7.0, 0.0), (9.0, 13.4), alpha(WHITE, 220));
        sparkle(g, (2.6, 4.0), 2.0, h);
        sparkle(g, (13.4, 7.0), 1.8, h);
    });
}

fn g_halo(g: &mut Gl) {
    let c = g.c;
    g.disc((8.0, 9.4), 2.8, c);
    g.each(c, |u, v| {
        ((u - 8.0) / 5.6).powi(2) + ((v - 16.6) / 4.4).powi(2) <= 1.0
    });
    g.oval_ring((8.0, 4.0), (4.8, 1.6), 0.0, 1.4, C_GOLD);
}

fn g_cross(g: &mut Gl) {
    let c = g.c;
    g.rect((6.4, 0.8), (9.6, 15.2), c);
    g.rect((2.4, 4.2), (13.6, 7.4), c);
    g.flat(|g| g.disc((8.0, 5.8), 1.1, WHITE));
}

fn g_ankh(g: &mut Gl) {
    let c = g.c;
    g.oval_ring((8.0, 4.4), (2.8, 3.4), 0.0, 1.9, c);
    g.rect((7.0, 7.4), (9.0, 15.4), c);
    g.rect((2.6, 7.8), (13.4, 9.8), c);
}

fn g_wings(g: &mut Gl) {
    let c = g.c;
    wing(g, false, c);
    wing(g, true, c);
    g.disc((8.0, 9.0), 1.6, c);
}

fn g_feather(g: &mut Gl) {
    let c = g.c;
    leaf(g, (3.0, 13.0), (14.4, 1.6), 3.0, c);
    g.erase(|u, v| {
        seg_dist((u, v), (5.0, 7.2), (6.8, 9.0)).0 < 0.6
            || seg_dist((u, v), (9.6, 9.0), (11.2, 10.6)).0 < 0.6
    });
    g.seg((1.0, 15.0), (3.6, 12.4), 1.1, c);
    g.flat(|g| g.seg((3.0, 13.0), (13.2, 2.8), 0.8, mix(c, WHITE, 0.6)));
}

fn g_boot(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[
            (4.6, 1.6),
            (10.2, 1.6),
            (10.2, 9.4),
            (13.6, 10.4),
            (15.0, 12.2),
            (15.0, 14.8),
            (4.6, 14.8),
        ],
        c,
    );
    g.poly(
        &[(4.4, 4.6), (0.2, 1.2), (0.4, 4.4), (1.4, 6.6), (4.4, 8.0)],
        rgb(0xf0, 0xf4, 0xff),
    );
    g.flat(|g| {
        g.rect((4.6, 1.6), (10.2, 3.2), g.hi());
        g.rect((4.6, 13.6), (15.0, 14.8), g.lo());
    });
}

fn g_steps(g: &mut Gl) {
    let c = g.c;
    for o in [(4.4, 10.8), (11.6, 4.6)] {
        g.oval((o.0, o.1 + 1.0), 2.2, 3.0, c);
        g.oval((o.0 + 0.2, o.1 - 3.4), 2.0, 1.1, c);
    }
}

fn g_rush(g: &mut Gl) {
    let c = g.c;
    for x in [0.6, 6.2] {
        g.poly(
            &[
                (x, 2.0),
                (x + 3.4, 2.0),
                (x + 9.2, 8.0),
                (x + 3.4, 14.0),
                (x, 14.0),
                (x + 5.8, 8.0),
            ],
            c,
        );
    }
}

fn g_target(g: &mut Gl) {
    let c = g.c;
    g.ring((8.0, 8.0), 5.6, 1.5, c);
    g.ring((8.0, 8.0), 2.6, 1.3, c);
    for (a, b) in [
        ((8.0, 0.4), (8.0, 4.0)),
        ((8.0, 12.0), (8.0, 15.6)),
        ((0.4, 8.0), (4.0, 8.0)),
        ((12.0, 8.0), (15.6, 8.0)),
    ] {
        g.seg(a, b, 1.4, c);
    }
    g.flat(|g| g.disc((8.0, 8.0), 0.9, WHITE));
}

fn g_scales(g: &mut Gl) {
    g.seg((8.0, 2.4), (8.0, 13.6), 1.4, C_GOLD);
    g.seg((1.8, 3.8), (14.2, 3.8), 1.2, C_GOLD);
    g.poly(
        &[(4.4, 15.4), (11.6, 15.4), (9.6, 13.2), (6.4, 13.2)],
        C_GOLD,
    );
    g.disc((8.0, 2.2), 1.2, C_GOLD);
    for x in [2.8, 13.2] {
        g.flat(|g| {
            g.seg((x, 3.8), (x - 2.2, 9.6), 0.6, GOLD_LO);
            g.seg((x, 3.8), (x + 2.2, 9.6), 0.6, GOLD_LO);
        });
        g.each(C_GOLD, |u, v| v >= 9.6 && (u - x).hypot(v - 9.6) <= 2.6);
    }
}

fn g_tomb(g: &mut Gl) {
    g.rect((0.4, 14.0), (15.6, 15.8), rgb(0x4a, 0x6a, 0x3a));
    g.disc((8.0, 6.4), 5.2, STONE_C);
    g.rect((2.8, 6.4), (13.2, 14.2), STONE_C);
    g.flat(|g| {
        g.rect((7.3, 3.8), (8.7, 11.0), INK_A);
        g.rect((5.4, 5.6), (10.6, 7.0), INK_A);
    });
}

fn g_mug(g: &mut Gl) {
    let c = g.c;
    g.oval_ring((11.8, 9.6), (2.4, 2.9), 0.0, 1.6, c);
    g.rect((2.6, 4.6), (11.0, 15.2), c);
    for (o, r) in [((3.8, 4.2), 1.8), ((6.6, 3.2), 2.3), ((9.6, 3.8), 2.0)] {
        g.disc(o, r, FOAM);
    }
    g.flat(|g| {
        let lo = g.lo();
        for y in [7.0, 12.4] {
            g.rect((2.6, y), (11.0, y + 1.0), lo);
        }
    });
}

fn g_totem(g: &mut Gl) {
    let c = g.c;
    g.poly(&[(5.0, 3.6), (0.4, 1.2), (1.0, 4.6), (5.0, 6.4)], c);
    g.poly(&[(11.0, 3.6), (15.6, 1.2), (15.0, 4.6), (11.0, 6.4)], c);
    g.rect((4.8, 1.0), (11.2, 15.6), WOOD);
    g.flat(|g| {
        for y in [3.4, 9.6] {
            g.rect((5.8, y), (7.4, y + 1.2), INK);
            g.rect((8.6, y), (10.2, y + 1.2), INK);
            g.rect((6.6, y + 2.4), (9.4, y + 3.4), c);
        }
        g.rect((4.8, 7.8), (11.2, 8.6), c);
    });
}

fn g_pyre(g: &mut Gl) {
    let c = g.c;
    flame(g, (8.0, 6.4), 0.8, c);
    g.seg((1.6, 14.6), (14.4, 11.4), 2.0, WOOD);
    g.seg((1.6, 11.4), (14.4, 14.6), 2.0, WOOD);
}

fn g_lotus(g: &mut Gl) {
    let c = g.c;
    let lo = g.lo();
    g.poly(&[(0.4, 9.4), (5.0, 9.6), (8.0, 12.6), (3.2, 13.0)], lo);
    g.poly(&[(15.6, 9.4), (11.0, 9.6), (8.0, 12.6), (12.8, 13.0)], lo);
    leaf(g, (8.0, 13.0), (1.4, 4.2), 2.2, c);
    leaf(g, (8.0, 13.0), (14.6, 4.2), 2.2, c);
    leaf(g, (8.0, 13.6), (8.0, 0.8), 2.8, c);
    g.flat(|g| g.seg((2.0, 14.8), (14.0, 14.8), 0.9, g.hi()));
}

fn g_yinyang(g: &mut Gl) {
    let c = g.c;
    let (light, dark) = (mix(c, WHITE, 0.75), mul(c, 0.3));
    let r = 6.8;
    let (top, bottom) = ((8.0, 8.0 - r / 2.0), (8.0, 8.0 + r / 2.0));
    g.disc((8.0, 8.0), r, light);
    g.each(dark, |u, v| {
        (u - 8.0).hypot(v - 8.0) <= r
            && ((u >= 8.0 && (u - top.0).hypot(v - top.1) > r / 2.0)
                || (u - bottom.0).hypot(v - bottom.1) <= r / 2.0)
    });
    g.flat(|g| {
        g.disc(top, 1.1, dark);
        g.disc(bottom, 1.1, light);
    });
}

fn g_sound(g: &mut Gl) {
    let c = g.c;
    g.disc((2.4, 8.0), 1.7, c);
    for r in [4.4, 8.0, 11.6] {
        g.arc((1.6, 8.0), r, 1.6, (-42.0, 42.0), c);
    }
}

fn g_zzz(g: &mut Gl) {
    let c = g.c;
    zee(g, (1.0, 8.6), 6.4, c);
    zee(g, (8.6, 3.6), 4.4, c);
    zee(g, (12.4, 0.4), 3.0, c);
}

fn g_orb(g: &mut Gl) {
    let c = g.c;
    g.disc((8.0, 8.0), 6.4, c);
    g.flat(|g| {
        let h = g.hi();
        g.arc((8.0, 8.0), 3.8, 1.0, (160.0, 300.0), h);
        g.arc((8.0, 8.0), 2.0, 0.9, (330.0, 470.0), h);
        g.disc((5.4, 5.2), 1.2, WHITE);
    });
}

fn g_missile(g: &mut Gl) {
    let c = g.c;
    for (o, r) in [((11.6, 4.4), 3.2), ((6.8, 9.2), 2.0), ((3.4, 12.6), 1.3)] {
        g.disc(o, r, c);
    }
    g.flat(|g| {
        g.disc((12.0, 4.0), 1.4, WHITE);
        g.disc((7.0, 9.0), 0.7, WHITE);
    });
}

fn g_spark(g: &mut Gl) {
    let c = g.c;
    g.poly(&star_pts((8.0, 8.0), 8.4, 2.8, 4, -90.0), c);
    g.poly(&star_pts((8.0, 8.0), 5.4, 2.6, 4, -45.0), c);
    g.flat(|g| g.disc((8.0, 8.0), 1.6, mix(c, WHITE, 0.75)));
}

fn g_bubble(g: &mut Gl) {
    let c = g.c;
    g.flat(|g| g.disc((8.0, 8.0), 6.4, alpha(c, 90)));
    g.ring((8.0, 8.0), 6.6, 1.5, c);
    g.flat(|g| {
        g.arc((8.0, 8.0), 4.6, 1.2, (200.0, 260.0), WHITE);
        g.disc((10.6, 10.6), 0.8, alpha(WHITE, 200));
    });
}

fn g_eclipse(g: &mut Gl) {
    let c = g.c;
    g.disc((8.0, 8.0), 7.2, c);
    g.flat(|g| {
        g.disc((8.0, 8.0), 5.4, rgb(0x16, 0x10, 0x26));
        g.each(mix(c, WHITE, 0.7), |u, v| {
            (u - 8.0).hypot(v - 8.0) <= 5.6 && (u - 7.0).hypot(v - 7.0) > 5.2
        });
    });
}

fn g_orbit(g: &mut Gl) {
    let c = g.c;
    g.flat(|g| g.oval_ring((8.0, 8.0), (7.0, 3.2), -25.0, 0.9, alpha(g.hi(), 210)));
    g.disc((8.0, 8.0), 2.6, c);
    let (s, k) = (-25f64).to_radians().sin_cos();
    for a in [30.0f64, 150.0, 260.0] {
        let (x, y) = (7.0 * a.to_radians().cos(), 3.2 * a.to_radians().sin());
        g.disc((8.0 + x * k - y * s, 8.0 + x * s + y * k), 1.4, C_BONE);
    }
}

fn g_crystal(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[(3.6, 15.2), (12.4, 15.2), (10.6, 11.6), (5.4, 11.6)],
        C_GOLD,
    );
    g.disc((8.0, 6.8), 5.6, c);
    g.flat(|g| {
        let h = g.hi();
        g.arc((8.0, 6.8), 3.0, 1.0, (180.0, 330.0), h);
        g.disc((5.8, 4.6), 1.1, WHITE);
        sparkle(g, (9.6, 8.0), 1.8, WHITE);
    });
}

fn g_figure(g: &mut Gl) {
    let c = g.c;
    figure(g, (8.0, 7.6), 1.0, c);
}

fn g_crowd(g: &mut Gl) {
    let c = g.c;
    let lo = g.lo();
    figure(g, (3.4, 6.2), 0.7, lo);
    figure(g, (12.6, 6.2), 0.7, lo);
    // a gap around the one in front keeps the three apart
    let front = (8.0, 8.8);
    g.erase(|u, v| {
        [(0.0, 0.0), (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)]
            .iter()
            .any(|(dx, dy)| in_figure(front, 0.9, u + dx, v + dy))
    });
    figure(g, front, 0.9, c);
}

fn g_mirror(g: &mut Gl) {
    let c = g.c;
    figure(g, (10.8, 8.0), 0.85, alpha(mix(c, WHITE, 0.4), 160));
    figure(g, (5.2, 8.0), 0.85, c);
}

fn g_fade(g: &mut Gl) {
    let c = g.c;
    figure(g, (8.0, 7.6), 1.0, c);
    // dissolves from the feet up
    g.erase(|u, v| {
        let (x, y) = (u.floor() as i32, v.floor() as i32);
        (v > 8.0 && (x + y) % 2 == 0) || (v > 12.5 && x % 2 == 0)
    });
}

fn g_smoke(g: &mut Gl) {
    let c = g.c;
    let puffs = [
        ((4.6, 10.6), 3.4),
        ((9.6, 9.6), 3.8),
        ((12.6, 12.8), 2.6),
        ((7.4, 5.2), 2.8),
        ((11.8, 4.2), 1.9),
        ((3.0, 4.0), 1.3),
    ];
    for (o, r) in puffs {
        g.disc(o, r, c);
    }
    g.flat(|g| {
        let h = g.hi();
        for (o, r) in puffs {
            g.arc(o, r - 1.0, 0.8, (190.0, 260.0), h);
        }
    });
}

fn g_hand(g: &mut Gl) {
    let c = g.c;
    g.oval((8.0, 12.4), 3.8, 2.8, c);
    for p in [
        [(5.0, 11.4), (3.0, 6.4), (3.6, 3.4)],
        [(7.0, 10.6), (6.4, 4.4), (7.2, 1.6)],
        [(9.2, 10.6), (10.0, 4.6), (9.6, 1.8)],
        [(11.2, 11.4), (13.2, 6.6), (12.6, 3.8)],
        [(4.8, 13.0), (1.8, 11.0), (1.2, 8.8)],
    ] {
        g.path(&p, 1.7, c);
    }
}

fn g_fang(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[(1.0, 1.6), (15.0, 1.6), (13.8, 5.6), (2.2, 5.6)],
        mul(c, 0.7),
    );
    g.poly(&[(3.0, 5.0), (6.8, 5.0), (4.9, 13.4)], FANG);
    g.poly(&[(9.2, 5.0), (13.0, 5.0), (11.1, 13.4)], FANG);
    g.poly(&[(6.8, 5.0), (9.2, 5.0), (8.0, 8.4)], FANG);
    g.flat(|g| g.disc((4.9, 14.6), 1.0, c));
}

fn g_muscle(g: &mut Gl) {
    let c = g.c;
    g.seg((1.0, 14.6), (7.0, 11.4), 3.6, c);
    g.seg((7.4, 11.4), (10.2, 5.0), 3.2, c);
    g.disc((6.4, 9.6), 3.2, c);
    g.disc((10.6, 3.8), 2.6, c);
    g.flat(|g| g.arc((6.4, 9.6), 2.0, 0.9, (200.0, 290.0), g.hi()));
}

fn g_cauldron(g: &mut Gl) {
    let c = g.c;
    for (o, r) in [((5.6, 3.4), 1.6), ((9.8, 2.2), 1.3), ((7.6, 0.4), 0.9)] {
        g.ring(o, r, 0.9, c);
    }
    g.each(IRON, |u, v| v >= 6.6 && (u - 8.0).hypot(v - 7.0) <= 6.6);
    g.rect((0.8, 5.6), (15.2, 7.4), IRON);
    g.rect((2.8, 12.6), (4.6, 15.6), IRON);
    g.rect((11.4, 12.6), (13.2, 15.6), IRON);
    g.flat(|g| g.rect((2.0, 5.8), (14.0, 6.8), c));
}

fn g_beacon(g: &mut Gl) {
    let c = g.c;
    g.flat(|g| {
        for i in 0..8 {
            let a = i as f64 * 45.0;
            g.seg(polar((8.0, 4.6), 3.6, a), polar((8.0, 4.6), 5.6, a), 1.0, c);
        }
    });
    g.poly(
        &[(5.0, 15.6), (11.0, 15.6), (9.8, 7.4), (6.2, 7.4)],
        STONE_C,
    );
    g.disc((8.0, 4.6), 2.6, c);
    g.flat(|g| {
        g.disc((8.0, 4.6), 1.2, WHITE);
        g.rect((5.6, 7.0), (10.4, 8.2), C_GOLD);
    });
}

fn g_golem(g: &mut Gl) {
    let c = g.c;
    g.rect((5.8, 0.8), (10.2, 4.8), c);
    g.rect((3.4, 4.6), (12.6, 11.0), c);
    g.rect((0.8, 5.0), (3.4, 12.0), c);
    g.rect((12.6, 5.0), (15.2, 12.0), c);
    g.rect((0.6, 11.4), (3.6, 14.0), c);
    g.rect((12.4, 11.4), (15.4, 14.0), c);
    g.rect((4.2, 11.0), (7.4, 15.6), c);
    g.rect((8.6, 11.0), (11.8, 15.6), c);
    g.flat(|g| {
        let lo = g.lo();
        g.rect((6.6, 2.2), (7.6, 3.2), YELLOW);
        g.rect((8.4, 2.2), (9.4, 3.2), YELLOW);
        g.seg((3.4, 5.0), (3.4, 11.4), 0.7, lo);
        g.seg((12.6, 5.0), (12.6, 11.4), 0.7, lo);
        g.seg((5.0, 8.0), (11.0, 8.0), 0.7, lo);
    });
}

fn g_paw(g: &mut Gl) {
    let c = g.c;
    g.oval((8.0, 11.8), 4.4, 3.4, c);
    for (o, rx, ry) in [
        ((2.4, 6.6), 1.5, 1.9),
        ((5.9, 3.6), 1.6, 2.1),
        ((10.1, 3.6), 1.6, 2.1),
        ((13.6, 6.6), 1.5, 1.9),
    ] {
        g.oval(o, rx, ry, c);
    }
}

fn g_hawk(g: &mut Gl) {
    let c = g.c;
    let wing = [
        (7.4, 6.6),
        (0.4, 1.2),
        (0.6, 4.0),
        (1.8, 6.8),
        (3.8, 9.0),
        (7.0, 10.4),
    ];
    g.poly(&wing, c);
    g.poly(&wing.map(|(x, y)| (16.0 - x, y)), c);
    g.poly(&[(6.4, 11.0), (9.6, 11.0), (10.8, 15.0), (5.2, 15.0)], c);
    g.oval((8.0, 8.6), 2.2, 3.8, c);
    g.disc((8.0, 4.2), 1.9, c);
    g.flat(|g| {
        let lo = g.lo();
        g.poly(&[(7.4, 5.0), (8.6, 5.0), (8.0, 6.6)], C_GOLD);
        g.disc((7.3, 3.8), 0.5, INK);
        g.disc((8.7, 3.8), 0.5, INK);
        for m in [false, true] {
            let f = |p: P| if m { (16.0 - p.0, p.1) } else { p };
            g.seg(f((1.4, 4.0)), f((5.6, 7.6)), 0.7, lo);
            g.seg(f((2.4, 6.8)), f((6.0, 9.0)), 0.7, lo);
        }
    });
}

fn g_demon(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[(4.2, 7.0), (0.8, 3.4), (0.8, 0.4), (3.2, 2.8), (6.4, 5.0)],
        HORN,
    );
    g.poly(
        &[
            (11.8, 7.0),
            (15.2, 3.4),
            (15.2, 0.4),
            (12.8, 2.8),
            (9.6, 5.0),
        ],
        HORN,
    );
    g.disc((8.0, 9.6), 5.4, c);
    g.flat(|g| {
        g.poly(&[(4.2, 8.0), (7.2, 9.4), (4.8, 10.4)], YELLOW);
        g.poly(&[(11.8, 8.0), (8.8, 9.4), (11.2, 10.4)], YELLOW);
        g.arc((8.0, 10.6), 2.8, 1.0, (30.0, 150.0), INK);
        g.poly(&[(5.8, 12.4), (7.0, 12.8), (6.4, 14.2)], FANG);
        g.poly(&[(10.2, 12.4), (9.0, 12.8), (9.6, 14.2)], FANG);
    });
}

fn g_ghost(g: &mut Gl) {
    let c = g.c;
    g.disc((8.0, 6.8), 5.6, c);
    g.rect((2.4, 6.8), (13.6, 13.0), c);
    for x in [3.6, 8.0, 12.4] {
        g.disc((x, 13.0), 1.6, c);
    }
    g.flat(|g| {
        g.oval((5.8, 6.8), 1.0, 1.6, INK);
        g.oval((10.2, 6.8), 1.0, 1.6, INK);
        g.oval((8.0, 10.4), 1.2, 1.6, INK);
    });
}

fn g_soul(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[(3.4, 9.6), (11.6, 9.0), (13.4, 0.6), (9.8, 4.6), (7.0, 3.0)],
        c,
    );
    g.disc((7.4, 10.4), 4.6, c);
    g.flat(|g| {
        g.oval((5.8, 10.2), 0.8, 1.2, INK);
        g.oval((8.8, 10.2), 0.8, 1.2, INK);
    });
}

fn g_mask(g: &mut Gl) {
    let c = g.c;
    g.oval((8.0, 8.0), 5.8, 6.8, c);
    g.flat(|g| {
        g.poly(&[(3.6, 6.0), (7.0, 7.0), (6.6, 8.8), (4.0, 8.2)], INK);
        g.poly(&[(12.4, 6.0), (9.0, 7.0), (9.4, 8.8), (12.0, 8.2)], INK);
        g.arc((8.0, 9.4), 2.8, 1.1, (25.0, 155.0), INK);
        let lo = g.lo();
        g.seg((3.6, 4.4), (6.8, 5.2), 0.8, lo);
        g.seg((12.4, 4.4), (9.2, 5.2), 0.8, lo);
    });
}

fn g_beast(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[
            (1.6, 8.6),
            (2.6, 3.6),
            (5.8, 1.6),
            (9.0, 3.0),
            (12.6, 1.4),
            (14.8, 5.0),
            (14.4, 10.2),
            (11.6, 14.4),
            (5.0, 14.8),
            (1.4, 12.4),
        ],
        c,
    );
    g.flat(|g| {
        for (o, r) in [((5.0, 6.2), 1.6), ((9.6, 5.4), 1.3), ((12.0, 8.2), 1.1)] {
            g.disc(o, r, YELLOW);
            g.disc(o, r * 0.45, INK);
        }
        g.poly(&[(3.6, 10.6), (12.2, 10.0), (10.6, 13.0), (5.0, 13.2)], INK);
        for x in [5.0, 7.0, 9.0, 11.0] {
            g.poly(&[(x, 10.4), (x + 1.2, 10.4), (x + 0.6, 11.8)], FANG);
        }
    });
}

fn g_plague(g: &mut Gl) {
    let c = g.c;
    g.rect((2.4, 0.2), (9.6, 2.4), LEATHER);
    g.rect((0.4, 2.0), (12.0, 3.4), LEATHER);
    g.disc((6.0, 7.0), 4.6, LEATHER);
    g.poly(
        &[(8.2, 6.0), (15.6, 14.6), (13.6, 15.4), (6.6, 10.6)],
        rgb(0xd8, 0xc8, 0xa0),
    );
    g.flat(|g| {
        for (o, r) in [((4.2, 6.6), 1.7), ((7.8, 6.0), 1.5)] {
            g.disc(o, r, C_GOLD);
            g.disc(o, r - 0.6, c);
        }
    });
}

fn g_rage(g: &mut Gl) {
    let c = g.c;
    g.disc((8.0, 8.6), 6.6, c);
    g.flat(|g| {
        g.seg((2.8, 4.8), (7.0, 7.2), 1.5, INK);
        g.seg((13.2, 4.8), (9.0, 7.2), 1.5, INK);
        g.disc((5.4, 8.2), 1.0, YELLOW);
        g.disc((10.6, 8.2), 1.0, YELLOW);
        g.rect((4.6, 10.6), (11.4, 13.6), INK);
        g.rect((5.0, 10.6), (11.0, 11.4), FANG);
        g.rect((5.0, 12.8), (11.0, 13.6), FANG);
    });
}

fn g_unknown(g: &mut Gl) {
    let c = g.c;
    g.arc((8.0, 5.6), 3.6, 2.2, (180.0, 400.0), c);
    g.path(&[(10.6, 8.2), (8.0, 9.6), (8.0, 11.0)], 2.2, c);
    g.disc((8.0, 14.0), 1.3, c);
}

fn g_staff(g: &mut Gl) {
    let c = g.c;
    g.seg((2.4, 15.4), (10.4, 5.4), 1.6, WOOD);
    g.arc((11.8, 4.2), 3.2, 1.1, (80.0, 210.0), C_GOLD);
    g.disc((11.8, 4.2), 2.5, c);
    g.flat(|g| {
        g.disc((11.1, 3.5), 0.8, WHITE);
        g.seg((4.4, 12.2), (5.6, 13.2), 1.0, C_GOLD);
    });
}

fn g_lyre(g: &mut Gl) {
    for m in [false, true] {
        let f = |p: P| if m { (16.0 - p.0, p.1) } else { p };
        let arm: Vec<P> = [(5.0, 13.4), (2.6, 9.6), (3.4, 5.4), (2.0, 1.8), (3.2, 0.8)]
            .into_iter()
            .map(f)
            .collect();
        g.path(&arm, 1.8, C_GOLD);
    }
    g.seg((3.0, 4.2), (13.0, 4.2), 1.3, C_GOLD);
    g.rect((4.0, 12.6), (12.0, 15.0), C_GOLD);
    g.flat(|g| {
        let c = mix(g.c, WHITE, 0.4);
        for x in [6.2, 8.0, 9.8] {
            g.seg((x, 4.8), (x, 12.6), 0.6, c);
        }
    });
}

fn g_drum(g: &mut Gl) {
    let c = g.c;
    g.seg((1.0, 0.8), (6.4, 5.2), 1.2, WOOD);
    g.seg((15.0, 0.8), (9.6, 5.2), 1.2, WOOD);
    g.rect((2.4, 6.4), (13.6, 13.6), c);
    g.oval((8.0, 13.6), 5.6, 1.6, c);
    g.oval((8.0, 6.4), 5.6, 1.9, rgb(0xec, 0xdc, 0xbc));
    g.flat(|g| {
        g.path(
            &[
                (2.8, 8.6),
                (4.6, 12.6),
                (6.6, 8.8),
                (8.6, 13.0),
                (10.6, 8.8),
                (12.6, 12.6),
                (13.4, 9.6),
            ],
            0.7,
            C_BONE,
        );
    });
}

fn g_candle(g: &mut Gl) {
    let c = g.c;
    g.rect((5.6, 6.6), (10.4, 14.0), rgb(0xf0, 0xe8, 0xd0));
    g.oval((8.0, 14.4), 5.2, 1.3, C_GOLD);
    flame(g, (8.0, 3.2), 0.48, c);
    g.flat(|g| {
        g.seg((8.0, 5.6), (8.0, 6.8), 0.7, INK);
        g.seg((9.6, 6.8), (9.6, 9.0), 0.8, rgb(0xd8, 0xcc, 0xb0));
    });
}

/// Two cupped hands holding a light.
fn g_hands(g: &mut Gl) {
    let c = g.c;
    for m in [false, true] {
        let f = |p: P| if m { (16.0 - p.0, p.1) } else { p };
        g.poly(
            &[
                (0.4, 8.2),
                (2.6, 7.4),
                (5.4, 9.6),
                (8.0, 10.4),
                (8.0, 13.6),
                (4.4, 14.2),
                (1.6, 12.0),
            ]
            .map(f),
            c,
        );
    }
    g.flat(|g| {
        let lo = g.lo();
        g.seg((8.0, 10.6), (8.0, 13.6), 0.7, lo);
        for (a, b) in [((2.2, 9.6), (4.6, 11.0)), ((13.8, 9.6), (11.4, 11.0))] {
            g.seg(a, b, 0.6, lo);
        }
        let glow = rgb(0xff, 0xf4, 0xc0);
        g.disc((8.0, 6.0), 2.6, glow);
        g.disc((8.0, 6.0), 1.2, WHITE);
        for a in [-90.0, -135.0, -45.0, 180.0, 0.0] {
            g.seg(
                polar((8.0, 6.0), 3.6, a),
                polar((8.0, 6.0), 5.2, a),
                0.9,
                glow,
            );
        }
    });
}

fn g_aura(g: &mut Gl) {
    let c = g.c;
    g.ring((8.0, 8.0), 7.0, 1.3, c);
    figure(g, (8.0, 8.8), 0.72, mix(c, WHITE, 0.25));
    g.flat(|g| {
        for i in 0..8 {
            let a = i as f64 * 45.0 + 22.5;
            g.seg(
                polar((8.0, 8.0), 5.2, a),
                polar((8.0, 8.0), 6.0, a),
                0.8,
                g.hi(),
            );
        }
    });
}

fn g_snake(g: &mut Gl) {
    let c = g.c;
    g.spike(
        &[
            (11.0, 3.4),
            (8.0, 5.0),
            (5.0, 7.4),
            (6.4, 9.6),
            (10.6, 10.4),
            (11.4, 12.8),
            (7.6, 14.6),
            (2.0, 14.0),
        ],
        3.0,
        c,
    );
    g.oval((11.8, 3.0), 2.4, 1.8, c);
    g.flat(|g| {
        g.disc((12.4, 2.4), 0.6, YELLOW);
        g.path(
            &[(14.0, 3.4), (15.6, 3.6), (16.0, 2.8)],
            0.6,
            rgb(0xe0, 0x30, 0x40),
        );
    });
}

fn g_spider(g: &mut Gl) {
    let c = g.c;
    for m in [false, true] {
        let f = |p: P| if m { (16.0 - p.0, p.1) } else { p };
        for leg in [
            [(6.8, 6.6), (3.6, 4.0), (2.2, 0.8)],
            [(6.4, 8.0), (2.6, 7.0), (0.6, 4.8)],
            [(6.4, 9.6), (2.6, 10.6), (0.8, 13.2)],
            [(6.8, 11.0), (4.2, 13.6), (3.4, 15.6)],
        ] {
            g.path(&leg.map(f), 1.0, c);
        }
    }
    g.disc((8.0, 10.2), 3.2, c);
    g.disc((8.0, 6.0), 2.0, c);
    g.flat(|g| {
        g.disc((7.2, 5.6), 0.5, rgb(0xff, 0x40, 0x40));
        g.disc((8.8, 5.6), 0.5, rgb(0xff, 0x40, 0x40));
        g.poly(
            &[(8.0, 8.8), (9.0, 10.4), (8.0, 12.0), (7.0, 10.4)],
            rgb(0xe0, 0x30, 0x40),
        );
    });
}

fn g_bat(g: &mut Gl) {
    let c = g.c;
    let wing = [
        (7.0, 6.2),
        (0.4, 3.4),
        (1.4, 6.2),
        (0.4, 9.6),
        (2.6, 8.8),
        (3.6, 11.0),
        (5.2, 9.6),
        (6.8, 11.4),
    ];
    g.poly(&wing, c);
    g.poly(&wing.map(|(x, y)| (16.0 - x, y)), c);
    g.oval((8.0, 8.4), 1.9, 3.2, c);
    g.poly(&[(6.6, 5.4), (7.0, 2.8), (7.8, 5.2)], c);
    g.poly(&[(9.4, 5.4), (9.0, 2.8), (8.2, 5.2)], c);
    g.flat(|g| {
        g.disc((7.3, 6.6), 0.5, rgb(0xff, 0x50, 0x50));
        g.disc((8.7, 6.6), 0.5, rgb(0xff, 0x50, 0x50));
    });
}

fn g_raven(g: &mut Gl) {
    let c = g.c;
    g.poly(&[(4.0, 9.6), (0.2, 12.6), (1.4, 14.2), (6.2, 12.0)], c);
    g.oval((7.6, 9.2), 4.8, 3.4, c);
    g.disc((11.6, 5.4), 2.6, c);
    g.poly(
        &[(13.4, 4.2), (16.0, 6.0), (13.4, 6.8)],
        rgb(0x40, 0x40, 0x48),
    );
    g.flat(|g| {
        g.disc((12.2, 4.8), 0.6, YELLOW);
        g.path(&[(4.4, 8.2), (7.4, 10.0), (10.6, 9.4)], 0.7, g.lo());
        g.seg((7.0, 12.4), (6.4, 15.2), 0.8, rgb(0x30, 0x30, 0x38));
        g.seg((9.0, 12.4), (9.4, 15.2), 0.8, rgb(0x30, 0x30, 0x38));
    });
}

fn g_owl(g: &mut Gl) {
    let c = g.c;
    g.poly(&[(2.6, 1.0), (6.0, 4.6), (3.0, 5.6)], c);
    g.poly(&[(13.4, 1.0), (10.0, 4.6), (13.0, 5.6)], c);
    g.oval((8.0, 9.4), 5.6, 5.8, c);
    g.flat(|g| {
        g.oval((8.0, 11.6), 3.2, 3.0, mix(c, WHITE, 0.45));
        for x in [5.6, 10.4] {
            g.disc((x, 7.4), 2.2, FANG);
            g.disc((x, 7.4), 1.0, INK);
        }
        g.poly(&[(7.2, 8.8), (8.8, 8.8), (8.0, 10.6)], C_GOLD);
    });
}

fn g_mushroom(g: &mut Gl) {
    let c = g.c;
    g.rect((6.0, 7.0), (10.0, 14.8), rgb(0xef, 0xe6, 0xcf));
    g.each(c, |u, v| v <= 7.6 && (u - 8.0).hypot(v - 7.6) <= 6.8);
    g.flat(|g| {
        for (o, r) in [
            ((5.0, 4.8), 1.1),
            ((9.4, 3.0), 1.2),
            ((11.6, 6.0), 0.9),
            ((7.4, 6.4), 0.7),
        ] {
            g.disc(o, r, FANG);
        }
    });
}

fn g_antler(g: &mut Gl) {
    let c = g.c;
    for m in [false, true] {
        let f = |p: P| if m { (16.0 - p.0, p.1) } else { p };
        g.path(&[f((6.6, 6.4)), f((4.4, 3.8)), f((1.4, 3.0))], 1.3, BONE_C);
        g.seg(f((4.6, 4.0)), f((4.2, 0.6)), 1.1, BONE_C);
        g.seg(f((3.0, 3.4)), f((1.0, 5.6)), 1.0, BONE_C);
        g.poly(&[f((6.2, 5.6)), f((4.6, 4.6)), f((6.6, 7.6))], c);
    }
    g.poly(
        &[
            (5.8, 6.0),
            (10.2, 6.0),
            (9.6, 12.4),
            (8.0, 15.0),
            (6.4, 12.4),
        ],
        c,
    );
    g.flat(|g| {
        g.disc((6.8, 8.4), 0.6, INK);
        g.disc((9.2, 8.4), 0.6, INK);
        g.disc((8.0, 14.0), 0.9, INK);
    });
}

fn g_bell(g: &mut Gl) {
    let c = g.c;
    g.ring((8.0, 1.6), 1.3, 0.9, c);
    g.disc((8.0, 5.2), 3.0, c);
    g.each(c, |u, v| {
        (5.2..12.6).contains(&v) && (u - 8.0).abs() <= 3.0 + (v - 5.2) * 0.42
    });
    g.rect((1.4, 12.2), (14.6, 13.8), c);
    g.disc((8.0, 14.8), 1.3, c);
}

fn g_mountain(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[
            (0.2, 15.0),
            (6.0, 3.6),
            (8.6, 8.0),
            (11.0, 5.0),
            (15.8, 15.0),
        ],
        c,
    );
    g.flat(|g| {
        g.poly(
            &[(6.0, 3.6), (7.8, 7.0), (6.6, 6.4), (5.4, 7.4), (4.4, 6.8)],
            WHITE,
        );
        g.poly(
            &[
                (11.0, 5.0),
                (12.4, 7.6),
                (11.2, 7.2),
                (10.0, 7.8),
                (9.6, 7.0),
            ],
            WHITE,
        );
    });
}

fn g_anvil(g: &mut Gl) {
    let c = g.c;
    g.rect((4.0, 5.6), (14.8, 8.6), STEEL_LO);
    g.poly(&[(4.2, 5.6), (0.2, 6.2), (4.2, 8.6)], STEEL_LO);
    g.rect((6.4, 8.6), (11.6, 11.8), STEEL_LO);
    g.rect((3.6, 11.8), (14.2, 15.0), STEEL_LO);
    g.flat(|g| {
        sparkle(g, (9.4, 2.4), 2.4, c);
        sparkle(g, (13.4, 1.4), 1.4, c);
        g.rect((4.0, 5.6), (14.8, 6.4), STEEL);
    });
}

fn g_cloak(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[
            (8.0, 0.6),
            (12.4, 4.0),
            (13.0, 8.6),
            (14.4, 15.4),
            (1.6, 15.4),
            (3.0, 8.6),
            (3.6, 4.0),
        ],
        c,
    );
    g.flat(|g| {
        g.oval((8.0, 6.4), 2.6, 3.0, INK);
        g.disc((7.0, 6.6), 0.55, YELLOW);
        g.disc((9.0, 6.6), 0.55, YELLOW);
        g.seg((8.0, 10.4), (8.0, 15.2), 0.7, g.lo());
    });
}

fn g_beetle(g: &mut Gl) {
    let c = g.c;
    for m in [false, true] {
        let f = |p: P| if m { (16.0 - p.0, p.1) } else { p };
        for leg in [
            [(5.0, 7.0), (2.0, 5.0)],
            [(4.4, 10.0), (1.0, 10.4)],
            [(5.0, 12.6), (2.4, 15.0)],
        ] {
            g.path(&leg.map(f), 1.0, c);
        }
    }
    g.oval((8.0, 3.6), 2.4, 1.8, c);
    g.oval((8.0, 9.8), 4.4, 5.0, c);
    g.flat(|g| {
        g.seg((8.0, 5.4), (8.0, 14.6), 0.7, g.lo());
        g.seg((6.0, 1.8), (5.2, 0.4), 0.7, c);
        g.seg((10.0, 1.8), (10.8, 0.4), 0.7, c);
    });
}

fn g_torch(g: &mut Gl) {
    let c = g.c;
    g.seg((6.6, 15.4), (9.0, 7.2), 1.8, WOOD);
    g.poly(&[(6.6, 6.0), (11.4, 6.0), (10.4, 8.4), (7.6, 8.4)], IRON);
    flame(g, (9.0, 3.0), 0.55, c);
}

fn g_dice(g: &mut Gl) {
    let c = g.c;
    g.poly(
        &[(8.0, 1.4), (14.4, 4.8), (8.0, 8.2), (1.6, 4.8)],
        mix(c, WHITE, 0.45),
    );
    g.poly(&[(1.6, 4.8), (8.0, 8.2), (8.0, 15.2), (1.6, 11.8)], c);
    g.poly(
        &[(8.0, 8.2), (14.4, 4.8), (14.4, 11.8), (8.0, 15.2)],
        mul(c, 0.7),
    );
    g.flat(|g| {
        g.disc((8.0, 4.8), 0.8, INK);
        for o in [(3.4, 7.4), (6.2, 12.0), (4.8, 9.8)] {
            g.disc(o, 0.7, INK);
        }
        for o in [(10.0, 9.6), (12.6, 10.4), (10.0, 12.6), (12.6, 7.8)] {
            g.disc(o, 0.7, INK);
        }
    });
}

fn g_wolf(g: &mut Gl) {
    g.sprite(&[
        "................",
        ".x............x.",
        ".Xx..........xX.",
        ".XXx........xXX.",
        ".XxXx......xXxX.",
        ".XXXXXXXXXXXXXX.",
        "XXXXXXXXXXXXXXXX",
        ".XXyKXXXXXXKyXX.",
        "..XXXXXHHXXXXX..",
        "...XXXHHHHXXX...",
        "..XXXXHHHHXXXX..",
        "...XXxHHHHxXX...",
        "....XxHKKHxX....",
        ".....xHHHHx.....",
        "......xHHx......",
        "................",
    ]);
}

fn g_bear(g: &mut Gl) {
    g.sprite(&[
        "................",
        "..xx........xx..",
        ".xXXx......xXXx.",
        ".XxXXXXXXXXXXxX.",
        ".XXXXXXXXXXXXXX.",
        "XXXXXXXXXXXXXXXX",
        "XXXKXXXXXXXXKXXX",
        "XXXXXXXXXXXXXXXX",
        "XXXXXHHHHHHXXXXX",
        ".XXXHHHKKHHHXXX.",
        ".XXXHHHKKHHHXXX.",
        "..XXHHHHHHHHXX..",
        "..XXXHHxxHHXXX..",
        "...XXXHHHHXXX...",
        ".....XXXXXX.....",
        "................",
    ]);
}

fn g_dragon(g: &mut Gl) {
    g.sprite(&[
        "................",
        "...........xX...",
        "..........xXx...",
        "......xXXXXXx...",
        "....xXXXXXXXXx..",
        "..xXXXXXXyKXXXx.",
        ".xKXXXXXXXXXXXXx",
        "xXHHHHHXXXXXXXXx",
        "xWRWRWRRRXXXXXX.",
        "..RRRRRRRxXXXXx.",
        ".WRWRWRRXXXXXx..",
        "xXHHHHXXXXXXXx..",
        ".xxXXXXXXXXXXx..",
        "....xxXXXXXXx...",
        "......xXXXXXX...",
        ".......xXXXXXx..",
    ]);
}

fn g_skeleton(g: &mut Gl) {
    g.sprite(&[
        "................",
        "......ffff......",
        ".....ffffff.....",
        ".....fXffXf.....",
        ".....ffffff.....",
        "......fKKf......",
        ".......ff.......",
        "...ffffffffff...",
        "..f...fKKf...f..",
        "..f..ffffff..f..",
        ".f....fKKf....f.",
        ".f...ffffff...f.",
        "......ffff......",
        "......f..f......",
        ".....ff..ff.....",
        ".....f....f.....",
    ]);
}

const fn gl(
    name: &'static str,
    color: &'static str,
    elem: &'static str,
    draw: fn(&mut Gl),
) -> Glyph {
    Glyph {
        name,
        color,
        elem,
        draw,
    }
}

/// Every pictogram; the first one is drawn for unknown names.
pub static GLYPHS: &[Glyph] = &[
    gl("spark", "#e0a0ff", "arcane", g_spark),
    gl("sword", "", "steel", g_sword),
    gl("dagger", "", "steel", g_dagger),
    gl("swords", "", "steel", g_swords),
    gl("axe", "", "steel", g_axe),
    gl("hammer", "", "steel", g_hammer),
    gl("spear", "", "steel", g_spear),
    gl("bow", "#e0d0a0", "arrow", g_bow),
    gl("arrow", "#e0d0a0", "arrow", g_arrow),
    gl("volley", "#e0d0a0", "arrow", g_volley),
    gl("arrow_rain", "#e0d0a0", "arrow", g_arrow_rain),
    gl("shield", "", "shield", g_shield),
    gl("shields", "", "shield", g_shields),
    gl("broken", "", "", g_broken),
    gl("helm", "", "", g_helm),
    gl("armor", "", "shield", g_armor),
    gl("banner", "", "", g_banner),
    gl("crown", "#ff5a6a", "", g_crown),
    gl("fist", "#f0c49c", "", g_fist),
    gl("palm", "#f0d0a0", "", g_palm),
    gl("claw", "#e04040", "", g_claw),
    gl("slash", "#f0f0ff", "", g_slash),
    gl("whirl", "#e0e8ff", "", g_whirl),
    gl("heart", "#e8404a", "heart", g_heart),
    gl("drop", "#e03848", "blood", g_drop),
    gl("poison", "#8ae04a", "poison", g_poison),
    gl("mana", "#5a9aff", "mana", g_mana),
    gl("plus", "#70f070", "plus", g_plus),
    gl("eye", "", "eye", g_eye),
    gl("skull", "", "bone", g_skull),
    gl("bones", "", "bone", g_bones),
    gl("skeleton", "", "bone", g_skeleton),
    gl("scythe", "", "", g_scythe),
    gl("flask", "", "alchemy", g_flask),
    gl("bomb", "#ffb040", "", g_bomb),
    gl("chalice", "", "", g_chalice),
    gl("horn", "", "sound", g_horn),
    gl("note", "#ff9ad0", "sound", g_note),
    gl("sound", "#ff9ad0", "sound", g_sound),
    gl("book", "", "", g_book),
    gl("rune", "", "arcane", g_rune),
    gl("seal", "#d0a0ff", "arcane", g_seal),
    gl("chain", "", "", g_chain),
    gl("trap", "", "", g_trap),
    gl("web", "#e8e8e8", "", g_web),
    gl("coin", "", "gold", g_coin),
    gl("gem", "#80e0ff", "", g_gem),
    gl("hourglass", "#e8c060", "time", g_hourglass),
    gl("clock", "#a0d8ff", "time", g_clock),
    gl("rewind", "#a0e0ff", "time", g_rewind),
    gl("tower", "", "", g_tower),
    gl("gate", "#8a4aff", "void", g_gate),
    gl("spiral", "#a070ff", "void", g_spiral),
    gl("flame", "#ff7a2a", "fire", g_flame),
    gl("fireball", "#ff8a3a", "fire", g_fireball),
    gl("meteor", "#ff7a3a", "fire", g_meteor),
    gl("pyre", "#ff7a2a", "fire", g_pyre),
    gl("volcano", "#ff5a1a", "fire", g_volcano),
    gl("burst", "#ffa040", "", g_burst),
    gl("nova", "", "", g_nova),
    gl("snowflake", "#a8e4ff", "frost", g_snowflake),
    gl("icicle", "#a0e8ff", "frost", g_icicle),
    gl("bolt", "#ffe85a", "lightning", g_bolt),
    gl("forked", "#fff06a", "lightning", g_forked),
    gl("storm", "#8898b0", "lightning", g_storm),
    gl("cloud", "#c8d0e0", "", g_cloud),
    gl("rain", "#80c0ff", "water", g_rain),
    gl("wind", "#c8f0ff", "wind", g_wind),
    gl("tornado", "#c0d0e0", "wind", g_tornado),
    gl("wave", "#5ab0ff", "water", g_wave),
    gl("leaf", "#6ad04a", "nature", g_leaf),
    gl("tree", "#5ec05e", "nature", g_tree),
    gl("treant", "#6a9a3a", "nature", g_treant),
    gl("roots", "#7a9a40", "nature", g_roots),
    gl("thorns", "#7ac04a", "thorns", g_thorns),
    gl("rock", "#b09070", "earth", g_rock),
    gl("quake", "#c8a070", "earth", g_quake),
    gl("sun", "#ffd040", "sun", g_sun),
    gl("dawn", "#ffc050", "sun", g_dawn),
    gl("moon", "#d8e0ff", "", g_moon),
    gl("star", "#e8ecff", "star", g_star),
    gl("stars", "#c0d0ff", "star", g_stars),
    gl("eclipse", "#8a6aff", "shadow", g_eclipse),
    gl("pillar", "#fff0a0", "holy", g_pillar),
    gl("halo", "#fff4c0", "holy", g_halo),
    gl("cross", "#ffe080", "holy", g_cross),
    gl("ankh", "#f0c040", "holy", g_ankh),
    gl("wings", "#f0f0ff", "holy", g_wings),
    gl("feather", "#e8f0ff", "dodge", g_feather),
    gl("boot", "#a0703a", "speed", g_boot),
    gl("steps", "#c0e0ff", "speed", g_steps),
    gl("rush", "#ffd080", "speed", g_rush),
    gl("target", "#ff5a4a", "crit", g_target),
    gl("scales", "", "", g_scales),
    gl("tomb", "", "", g_tomb),
    gl("mug", "#e0a040", "", g_mug),
    gl("totem", "#e05040", "", g_totem),
    gl("lotus", "#ffb0e0", "chi", g_lotus),
    gl("yinyang", "#f0d8a0", "chi", g_yinyang),
    gl("zzz", "#b0d0ff", "", g_zzz),
    gl("orb", "#c080ff", "arcane", g_orb),
    gl("missile", "#c080ff", "arcane", g_missile),
    gl("bubble", "#a0c0ff", "", g_bubble),
    gl("orbit", "", "", g_orbit),
    gl("crystal", "#a0c0ff", "", g_crystal),
    gl("figure", "", "", g_figure),
    gl("crowd", "", "", g_crowd),
    gl("mirror", "#d0a0ff", "illusion", g_mirror),
    gl("fade", "#9090b0", "", g_fade),
    gl("smoke", "#b0b0c0", "smoke", g_smoke),
    gl("hand", "", "", g_hand),
    gl("fang", "#e03050", "blood", g_fang),
    gl("muscle", "#f0c49c", "", g_muscle),
    gl("cauldron", "", "alchemy", g_cauldron),
    gl("beacon", "#fff4a0", "holy", g_beacon),
    gl("golem", "#a0a0b0", "", g_golem),
    gl("paw", "#a0703a", "beast", g_paw),
    gl("hawk", "#b08050", "beast", g_hawk),
    gl("wolf", "#b0b0b8", "beast", g_wolf),
    gl("bear", "#8a5a3a", "beast", g_bear),
    gl("dragon", "#e05030", "", g_dragon),
    gl("demon", "#d84a3a", "demon", g_demon),
    gl("ghost", "#c8e4f0", "spirit", g_ghost),
    gl("soul", "#a8e0ff", "spirit", g_soul),
    gl("mask", "#ff8ad8", "illusion", g_mask),
    gl("beast", "#70c048", "", g_beast),
    gl("plague", "", "", g_plague),
    gl("rage", "#e83a2a", "", g_rage),
    gl("staff", "#c080ff", "arcane", g_staff),
    gl("lyre", "#ffb0e0", "sound", g_lyre),
    gl("drum", "#c05040", "sound", g_drum),
    gl("candle", "#ffc060", "", g_candle),
    gl("hands", "#f0d0a8", "", g_hands),
    gl("aura", "#ffe080", "", g_aura),
    gl("snake", "#7ac04a", "", g_snake),
    gl("spider", "#7a6a8a", "", g_spider),
    gl("bat", "#7a6a9a", "", g_bat),
    gl("raven", "#5a5a7a", "", g_raven),
    gl("owl", "#a07850", "", g_owl),
    gl("mushroom", "#e04a3a", "", g_mushroom),
    gl("antler", "#c09060", "beast", g_antler),
    gl("bell", "#e0b040", "sound", g_bell),
    gl("mountain", "#a0a0b4", "earth", g_mountain),
    gl("anvil", "#ffb040", "", g_anvil),
    gl("cloak", "#5a5a7a", "", g_cloak),
    gl("beetle", "#c0a040", "", g_beetle),
    gl("torch", "#ff8a2a", "fire", g_torch),
    gl("dice", "#f0f0f0", "", g_dice),
    gl("unknown", "", "", g_unknown),
];

// ---- badges ----

/// A small sign in the corner of an icon: an element or the kind of bonus.
pub struct Badge {
    pub name: &'static str,
    pub color: &'static str,
    /// 7 rows of 7 pixels, by spaces: X/x/H the colour and its shades,
    /// W white, K ink
    rows: &'static str,
}

fn paint_badge(p: &mut Pc, b: &Badge) {
    let c = hex(b.color);
    p.circle(19.5, 19.5, 5.0, rgb(14, 12, 20));
    for y in 14..SIDE {
        for x in 14..SIDE {
            let d = (x as f64 + 0.5 - 19.5).hypot(y as f64 + 0.5 - 19.5);
            if (d - 4.5).abs() <= 0.5 {
                p.put(x, y, mul(c, 0.55));
            }
        }
    }
    for (y, row) in b.rows.split_whitespace().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let col = match ch {
                'X' => c,
                'x' => mul(c, 0.62),
                'H' => mix(c, WHITE, 0.5),
                'W' => WHITE,
                'K' => INK,
                _ => continue,
            };
            p.put(16 + x as i32, 16 + y as i32, col);
        }
    }
}

const fn bd(name: &'static str, color: &'static str, rows: &'static str) -> Badge {
    Badge { name, color, rows }
}

pub static BADGES: &[Badge] = &[
    bd(
        "fire",
        "#ff7a2a",
        "...X... ..XX.X. .XXHXX. .XHWHXX XXHWHXX .XHHHX. ..XXX..",
    ),
    bd(
        "frost",
        "#a8e4ff",
        "...H... .H.X.H. ..XXX.. HXXWXXH ..XXX.. .H.X.H. ...H...",
    ),
    bd(
        "lightning",
        "#ffe85a",
        "....XX. ...XX.. ..XXXX. ....XX. ...XX.. ..XX... .X.....",
    ),
    bd(
        "poison",
        "#8ae04a",
        "..X.... .....X. ...X... ..XXX.. .XHXXX. .XHXXX. ..XXX..",
    ),
    bd(
        "holy",
        "#ffe080",
        "..XXX.. XXXWXXX XWWWWWX XXXWXXX ..XWX.. ..XWX.. ..XXX..",
    ),
    bd(
        "shadow",
        "#a070ff",
        "..XXX.. .XKKKX. XKKKKKX XKKHKKX XKKKKKX .XKKKX. ..XXX..",
    ),
    bd(
        "arcane",
        "#e0a0ff",
        "...X... ...X... ..XHX.. XXHWHXX ..XHX.. ...X... ...X...",
    ),
    bd(
        "nature",
        "#6ad04a",
        "....XXX ..XXXHX .XXXHXX .XXHXX. .XHXX.. XHX.... X......",
    ),
    bd(
        "earth",
        "#c8a070",
        "....... ..XXX.. .XHXXX. XHXXXXX XXXXXxX XXXxxxX .xxxxx.",
    ),
    bd(
        "wind",
        "#c8f0ff",
        "XXXXXX. ......X XXXX.X. ....... XXXXXX. ......X ..XXXX.",
    ),
    bd(
        "blood",
        "#e03848",
        "...X... ..XXX.. ..XXX.. .XHXXX. .XHXXX. .XXXXX. ..XXX..",
    ),
    bd(
        "bone",
        "#efe6cf",
        "X.....X XX...XX ..X.X.. ...H... ..X.X.. XX...XX X.....X",
    ),
    bd(
        "spirit",
        "#a8e0ff",
        "....XX. ...XX.. ..XXX.. .XXXXX. .XKXKX. .XXXXX. ..XXX..",
    ),
    bd(
        "sound",
        "#ff9ad0",
        "...XX.. ...XXX. ...X.XX ...X... .XXX... XXXX... .XX....",
    ),
    bd(
        "time",
        "#a0d8ff",
        "XXXXXXX .X...X. ..XHX.. ...X... ..X.X.. .XHHHX. XXXXXXX",
    ),
    bd(
        "void",
        "#8a4aff",
        "XXXXXXX ......X .XXXX.X .X..X.X .X.XX.X .X....X .XXXXXX",
    ),
    bd(
        "star",
        "#e8ecff",
        "...X... ...X... XXXHXXX .XHWHX. ..XXX.. .XX.XX. X.....X",
    ),
    bd(
        "sun",
        "#ffd040",
        "...X... .X...X. ..XXX.. X.XWX.X ..XXX.. .X...X. ...X...",
    ),
    bd(
        "gold",
        "#ffd040",
        "..XXX.. .XHHXX. XHXXXxX XHXXXxX XXXXxxX .XxxxX. ..XXX..",
    ),
    bd(
        "smoke",
        "#b0b0c0",
        "....... ...XX.. ..XHHX. .XHXXXX XHXXXXX XXXXXXx .xxxxx.",
    ),
    bd(
        "beast",
        "#c09060",
        ".X.X.X. .X.X.X. X.....X X.XXX.X ..XXXX. .XXXXX. ..XXX..",
    ),
    bd(
        "illusion",
        "#ff8ad8",
        ".XXXXX. XXXXXXX XKKXKKX XXXXXXX XXKKKXX .XXXXX. ..XXX..",
    ),
    bd(
        "water",
        "#5ab0ff",
        "...X... ..XXX.. ..XXX.. .XHXXX. .XHXXX. .XXXXX. ..XXX..",
    ),
    bd(
        "chi",
        "#ffe0a0",
        "..WWX.. .WWWXX. WWKWXXX WWWXXXX WWWXWXX .WWXXX. ..WXX..",
    ),
    bd(
        "demon",
        "#ff5a4a",
        "X.....X X.....X XX...XX .XXXXX. .XKXKX. .XXXXX. ..XXX..",
    ),
    bd(
        "alchemy",
        "#80e0a0",
        "..XXX.. ...X... ...X... ..XXX.. .XHXXX. XHXXXXX .XXXXX.",
    ),
    bd(
        "steel",
        "#d0d8e8",
        "......X .....XW ....XW. X..XW.. .XXW... ..XX... .X..X..",
    ),
    bd(
        "arrow",
        "#e0d0a0",
        "...XXXX .....XX ....X.X ...X..X ..X.... XX..... XX.....",
    ),
    bd(
        "up",
        "#a0ff80",
        "...X... ..XXX.. .XXXXX. XXXXXXX ..XXX.. ..XXX.. ..XXX..",
    ),
    bd(
        "shield",
        "#a0c0ff",
        "XXXXXXX XHHHHXX XHXXXXX XHXXXXX .XXXXX. ..XXX.. ...X...",
    ),
    bd(
        "plus",
        "#70f070",
        "..XXX.. ..XWX.. XXXWXXX XWWWWWX XXXWXXX ..XWX.. ..XXX..",
    ),
    bd(
        "heart",
        "#ff6070",
        ".XX.XX. XHXXXXX XHXXXXX XXXXXXX .XXXXX. ..XXX.. ...X...",
    ),
    bd(
        "mana",
        "#5a9aff",
        "...X... ..XHX.. .XHXXX. XHXXXXX .XXXXX. ..XXX.. ...X...",
    ),
    bd(
        "eye",
        "#e8e8ff",
        "....... ..XXX.. .XWWWX. XWWKWWX .XWWWX. ..XXX.. .......",
    ),
    bd(
        "speed",
        "#c0f0ff",
        "X..X... XX.XX.. .XX.XX. ..XX.XX .XX.XX. XX.XX.. X..X...",
    ),
    bd(
        "crit",
        "#ff6a3a",
        "...X... .XXXXX. .X...X. XX.X.XX .X...X. .XXXXX. ...X...",
    ),
    bd(
        "dodge",
        "#e8f0ff",
        "....XXX ...XHXX ..XHXX. .XHXX.. .HXX... H...... H......",
    ),
    bd(
        "thorns",
        "#7ac04a",
        "...X... X.XXX.X .XXHXX. XXHXXXX .XXXXX. X.XXX.X ...X...",
    ),
    bd(
        "crown",
        "#ffd040",
        "....... X..X..X XX.X.XX XXXXXXX XHXHXHX XXXXXXX .......",
    ),
    bd(
        "skull",
        "#efe6cf",
        ".XXXXX. XXXXXXX XKKXKKX XXXKXXX .XXXXX. .X.X.X. .......",
    ),
    bd(
        "moon",
        "#d8e0ff",
        "..XXXX. .XXH... XXH.... XXH.... XXH.... .XXH... ..XXXX.",
    ),
    bd(
        "impact",
        "#ffd080",
        "X..X..X .X.X.X. ..XWX.. XXWWWXX ..XWX.. .X.X.X. X..X..X",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_are_well_formed() {
        let c = rgb(200, 120, 60);
        let bare = rune_frame(c);
        let mut names = std::collections::HashSet::new();
        for g in GLYPHS {
            assert!(names.insert(g.name), "two pictograms named {}", g.name);
            let p = paint(
                Look {
                    glyph: g.name,
                    badge: "",
                },
                c,
                Frame::Rune,
            );
            let drawn = (0..SIDE * SIDE)
                .filter(|&i| p.px[i as usize] != bare.px[i as usize])
                .count();
            assert!(drawn > 20, "{} draws almost nothing", g.name);
        }
        let mut names = std::collections::HashSet::new();
        for b in BADGES {
            assert!(names.insert(b.name), "two badges named {}", b.name);
            let rows: Vec<&str> = b.rows.split_whitespace().collect();
            assert!(
                rows.len() == 7 && rows.iter().all(|r| r.len() == 7),
                "badge {}",
                b.name
            );
        }
    }
}

#[cfg(test)]
mod sheet {
    use super::*;

    /// Writes every pictogram and badge to RATAS_SHEET (a png), 8x scaled:
    /// `RATAS_SHEET=/tmp/s.png cargo test -p ratas glyph_sheet -- --ignored`
    #[test]
    #[ignore]
    fn glyph_sheet() {
        let Ok(path) = std::env::var("RATAS_SHEET") else {
            return;
        };
        let only: Vec<String> = std::env::var("RATAS_SHEET_ONLY")
            .map(|v| v.split(',').map(String::from).collect())
            .unwrap_or_default();
        let cols = 10;
        let k: i32 = std::env::var("RATAS_SHEET_K")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);
        let items: Vec<Pc> = GLYPHS
            .iter()
            .filter(|g| only.is_empty() || only.iter().any(|o| o == g.name))
            .map(|g| {
                let c = if g.color.is_empty() {
                    "#c08a50"
                } else {
                    g.color
                };
                paint(
                    Look {
                        glyph: g.name,
                        badge: "",
                    },
                    hex(c),
                    Frame::Rune,
                )
            })
            .chain(BADGES.iter().filter(|_| only.is_empty()).map(|b| {
                paint(
                    Look {
                        glyph: "nova",
                        badge: b.name,
                    },
                    rgb(90, 90, 110),
                    Frame::Plaque,
                )
            }))
            .collect();
        write_sheet(&path, &items, cols, k);
    }
}

/// Writes icons to a png, `cols` to a row, each pixel k×k: a way to look at
/// the art (`RATAS_SHEET=... cargo test -p ratas sheet -- --ignored`).
#[cfg(test)]
pub(crate) fn write_sheet(path: &str, items: &[Pc], cols: usize, k: i32) {
    let cell = SIDE + 4;
    let rows = items.len().div_ceil(cols) as i32;
    let (w, h) = (cols as i32 * cell * k, rows * cell * k);
    let mut bytes = vec![40u8; (w * h * 4) as usize];
    for (i, p) in items.iter().enumerate() {
        let (cx, cy) = ((i % cols) as i32 * cell, (i / cols) as i32 * cell);
        for y in 0..SIDE * k {
            for x in 0..SIDE * k {
                let c = p.get(x / k, y / k);
                // the png is written bottom up
                let row = h - 1 - ((cy + 2) * k + y);
                let o = (row * w + (cx + 2) * k + x) as usize * 4;
                let a = c.a as u32;
                for (j, v) in [c.r, c.g, c.b].into_iter().enumerate() {
                    bytes[o + j] = ((v as u32 * a + 40 * (255 - a)) / 255) as u8;
                }
                bytes[o + 3] = 255;
            }
        }
    }
    macroquad::texture::Image {
        bytes,
        width: w as u16,
        height: h as u16,
    }
    .export_png(path);
}
