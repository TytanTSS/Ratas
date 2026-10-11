//! The land of the detailed look: smooth ground of many faces whose colours
//! flow from tile to tile and darken in the corners by walls; lakes with a
//! bed under rippling water; blades of grass, reeds, ferns and flowers that
//! sway; paved floors, cobbles and planks; trees with lumpy crowns, firs in
//! tiers, palms with fronds; mountains of noisy rock with snow on what faces
//! up; walls of stones, rough rock, ice, plaster and timber with windows.

use std::f32::consts::{PI, TAU};

use macroquad::prelude::*;
use ratas_core::content::{self, TileDef};
use ratas_core::world::{Bitset, Level};

use super::lowpoly::{fbm2, hash2, hashf, noise2, noise3, sun_hd, Batch, Xf, SHINY, SMOOTH, WATER};
use super::materials::{BARK, FUR, GROUND, LEAVES, PLASTER, STONE, WOOD};
use super::terrain::{self, base_color, decor, mix, mul, peak, wall_height};
use crate::gfx::col;
use crate::gfx::world::WorldRenderer;

/// The height of the water's surface.
pub const WATER_Y: f32 = -0.15;
/// Faces along a side of a tile of ground.
const SUB: i32 = 4;

/// How a ground tile lies: its height and how rough it is.
fn form(key: &str) -> (f32, f32) {
    match key {
        "water" => (-0.36, 0.0),
        "deep_water" => (-0.55, 0.0),
        "lava" => (-0.12, 0.0),
        "magma_crack" => (-0.02, 0.06),
        "ice" | "ice_floor" => (-0.02, 0.0),
        "hill" => (0.3, 0.2),
        "dunes" => (0.14, 0.16),
        "snowdrift" => (0.12, 0.12),
        "swamp" | "reeds" => (-0.06, 0.05),
        "cave_floor" => (0.0, 0.05),
        "road" | "bridge" | "house_floor" | "stone_floor" | "door_open" | "stairs_down"
        | "stairs_up" | "dungeon" | "sandstone_floor" | "basalt_floor" | "dark_floor"
        | "carpet" | "cobblestone" | "city_gate" => (0.0, 0.0),
        "void" => (-0.8, 0.0),
        _ => (0.0, 0.09),
    }
}

fn is_water(key: &str) -> bool {
    matches!(key, "water" | "deep_water")
}

/// How deep the middle of a tile sinks below its corners.
fn bowl(key: &str) -> f32 {
    match key {
        "water" => -0.26,
        "deep_water" => -0.36,
        "lava" => -0.05,
        _ => 0.0,
    }
}

/// The height and roughness of the ground at a corner of tiles.
fn corner_of(l: &Level, cx: i32, cy: i32) -> (f32, f32) {
    let db = content::db();
    let (mut h, mut r, mut n) = (0.0, 0.0, 0.0);
    let (mut wet, mut dry) = (false, false);
    for (dx, dy) in [(-1, -1), (0, -1), (-1, 0), (0, 0)] {
        let (x, y) = (cx + dx, cy + dy);
        if !l.inside(x, y) {
            continue;
        }
        let key = &db.tile(l.at(x, y)).key;
        let (hh, rr) = form(key);
        if is_water(key) {
            wet = true;
        } else {
            dry = true;
        }
        h += hh;
        r += rr;
        n += 1.0;
    }
    if n == 0.0 {
        return (0.0, 0.0);
    }
    if wet && dry {
        // the shore: just above the water, so it shows its edge
        return (-0.04, 0.0);
    }
    (h / n, r / n)
}

fn bowl_of(l: &Level, x: i32, y: i32) -> f32 {
    if !l.inside(x, y) {
        return 0.0;
    }
    bowl(&content::db().tile(l.at(x, y)).key)
}

fn fade(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The ground's height at a point, from the corners around it.
fn surface(
    corner: &dyn Fn(i32, i32) -> (f32, f32),
    sink: &dyn Fn(i32, i32) -> f32,
    x: f32,
    y: f32,
) -> f32 {
    let (tx, ty) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - tx as f32, y - ty as f32);
    let (a, b, c, d) = (
        corner(tx, ty),
        corner(tx + 1, ty),
        corner(tx, ty + 1),
        corner(tx + 1, ty + 1),
    );
    let (u, v) = (fade(fx), fade(fy));
    let h = lerp(lerp(a.0, b.0, u), lerp(c.0, d.0, u), v);
    let r = lerp(lerp(a.1, b.1, fx), lerp(c.1, d.1, fx), fy);
    let s = sink(tx, ty);
    let dip = if s != 0.0 {
        // round hollows: ponds are round, shores wavy
        let r2 = ((fx - 0.5).powi(2) + (fy - 0.5).powi(2)) * 4.0;
        s * (1.0 - r2).max(0.0).powf(1.5) * (0.8 + 0.4 * (noise2(x * 2.0, y * 2.0) * 0.5 + 0.5))
    } else {
        0.0
    };
    h + r * fbm2(x * 1.4, y * 1.4) + dip
}

/// The height of the detailed ground under a point (lake beds included).
pub fn height(l: &Level, x: f32, y: f32) -> f32 {
    surface(
        &|cx, cy| corner_of(l, cx, cy),
        &|tx, ty| bowl_of(l, tx, ty),
        x,
        y,
    )
}

/// What stands on a tile and throws a shadow.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Caster {
    None,
    /// a block of wall this tall
    Block(f32),
    /// a mountain: a cone this tall
    Cone(f32),
    /// a crown of leaves (or a thing) between two heights, this wide,
    /// narrowing upward (0: a ball, 1: a cone), this dark
    Crown {
        lo: f32,
        hi: f32,
        r: f32,
        cone: f32,
        dark: f32,
    },
}

/// The scale of a broad-leaved tree on a tile.
fn tree_size(s: u32) -> f32 {
    1.3 + 0.45 * hashf(s)
}

fn caster_of(def: &TileDef, x: i32, y: i32) -> Caster {
    let key = def.key.as_str();
    let s = hash2(x, y);
    if let Some(h) = wall_height(key) {
        return Caster::Block(if key == "house_wall" { 1.6 } else { h });
    }
    if key == "house_floor" {
        // under a roof
        return Caster::Block(2.2);
    }
    if peak(key) {
        return Caster::Cone(1.0 + 1.0 * hashf(s));
    }
    let crown = |lo: f32, hi: f32, r: f32, cone: f32, dark: f32| Caster::Crown {
        lo,
        hi,
        r,
        cone,
        dark,
    };
    match key {
        "tree" => {
            let k = tree_size(s);
            crown(0.5 * k, 1.35 * k, 0.62 * k, 0.0, 1.0)
        }
        "pine" | "snow_pine" => {
            let k = 1.1 + 0.4 * hashf(s);
            crown(0.2 * k, 1.6 * k, 0.5 * k, 1.0, 1.0)
        }
        "palm" => crown(1.1, 1.55, 0.7, 0.0, 0.75),
        "bush" => crown(0.0, 0.48, 0.42, 0.0, 0.9),
        "dead_tree" | "charred_tree" | "twisted_tree" => crown(0.3, 1.1, 0.35, 0.0, 0.45),
        "cactus" => crown(0.0, 0.9, 0.22, 0.0, 0.8),
        _ if !def.walkable => crown(0.0, 0.8, 0.38, 0.0, 0.8),
        _ => Caster::None,
    }
}

/// How much a caster standing on tile (cx, cy) hides the sun from a point
/// at height y over the ground.
fn hides(c: Caster, cx: i32, cy: i32, qx: f32, qz: f32, y: f32) -> f32 {
    let (ox, oz) = (cx as f32 + 0.5, cy as f32 + 0.5);
    let d = ((qx - ox).powi(2) + (qz - oz).powi(2)).sqrt();
    match c {
        Caster::None => 0.0,
        Caster::Block(h) => {
            let inside =
                qx >= cx as f32 && qx < cx as f32 + 1.0 && qz >= cy as f32 && qz < cy as f32 + 1.0;
            if inside && y < h {
                1.0
            } else {
                0.0
            }
        }
        Caster::Cone(h) => {
            let top = h * (1.0 - d / 0.85);
            ((top - y) / 0.15).clamp(0.0, 1.0)
        }
        Caster::Crown {
            lo,
            hi,
            r,
            cone,
            dark,
        } => {
            if y < lo || y > hi {
                return 0.0;
            }
            let t = (y - lo) / (hi - lo).max(0.01);
            // a ball is widest in its middle, a cone at its bottom
            let ball = (1.0 - (2.0 * t - 1.0).powi(2)).max(0.0).sqrt();
            let rr = r * (ball * (1.0 - cone) + (1.0 - t) * cone).max(0.05);
            (1.0 - fade(((d - rr * 0.6) / (rr * 0.5)).clamp(0.0, 1.0))) * dark
        }
    }
}

/// How much the sun is hidden from a point on the ground (0 lit, 1 in
/// shadow), by what stands on the tiles `caster` tells.
fn shadow(caster: &dyn Fn(i32, i32) -> Caster, x: f32, z: f32) -> f32 {
    let s = sun_hd();
    let flat = vec2(s.x, s.z);
    let dir = flat.normalize();
    let slope = s.y / flat.length();
    let mut occ: f32 = 0.0;
    let mut t = 0.08;
    while t < 3.2 {
        let (qx, qz) = (x + dir.x * t, z + dir.y * t);
        let y = t * slope;
        let (tx, tz) = (qx.floor() as i32, qz.floor() as i32);
        for (dx, dz) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let c = caster(tx + dx, tz + dz);
            if c == Caster::None || (matches!(c, Caster::Block(_)) && (dx, dz) != (0, 0)) {
                continue;
            }
            occ = occ.max(hides(c, tx + dx, tz + dz, qx, qz, y));
        }
        if occ >= 1.0 {
            break;
        }
        t += 0.12 + t * 0.12;
    }
    occ
}

/// The sun's shadow on the detailed ground at a point (0 lit, 1 dark);
/// none in the dark under the earth.
pub fn shade_at(l: &Level, x: f32, z: f32) -> f32 {
    if !l.lit {
        return 0.0;
    }
    let db = content::db();
    shadow(
        &|cx, cy| {
            if l.inside(cx, cy) {
                caster_of(db.tile(l.at(cx, cy)), cx, cy)
            } else {
                Caster::None
            }
        },
        x,
        z,
    )
}

/// The colour of a shadow laid over a colour: darker and bluer, as the sky
/// lights it.
fn shaded(c: Color, k: f32) -> Color {
    let k = k * 0.72;
    Color::new(
        c.r * (1.0 - k * 0.42),
        c.g * (1.0 - k * 0.36),
        c.b * (1.0 - k * 0.18),
        c.a,
    )
}

/// What a chunk needs to know of its tiles and their neighbours, read once.
struct Field<'a> {
    l: &'a Level,
    ox: i32,
    oy: i32,
    n: i32,
    corners: Vec<(f32, f32)>,
    sinks: Vec<f32>,
    colors: Vec<Color>,
    solid: Vec<bool>,
    casters: Vec<Caster>,
}

impl<'a> Field<'a> {
    fn new(l: &'a Level, cx: i32, cy: i32) -> Field<'a> {
        let db = content::db();
        let (ox, oy) = (cx * terrain::CHUNK - 2, cy * terrain::CHUNK - 2);
        let n = terrain::CHUNK + 5;
        let mut f = Field {
            l,
            ox,
            oy,
            n,
            corners: Vec::with_capacity((n * n) as usize),
            sinks: Vec::with_capacity((n * n) as usize),
            colors: Vec::with_capacity((n * n) as usize),
            solid: Vec::with_capacity((n * n) as usize),
            casters: Vec::with_capacity((n * n) as usize),
        };
        for j in 0..n {
            for i in 0..n {
                let (x, y) = (ox + i, oy + j);
                f.corners.push(corner_of(l, x, y));
                f.sinks.push(bowl_of(l, x, y));
                let (tx, ty) = (x.clamp(0, l.w - 1), y.clamp(0, l.h - 1));
                let def = db.tile(l.at(tx, ty));
                f.colors.push(ground_tint(l, tx, ty, def));
                f.solid
                    .push(wall_height(&def.key).is_some() || peak(&def.key));
                let inside = l.inside(x, y);
                f.casters.push(if inside && l.lit {
                    caster_of(def, x, y)
                } else {
                    Caster::None
                });
            }
        }
        f
    }

    fn at(&self, x: i32, y: i32) -> Option<usize> {
        let (i, j) = (x - self.ox, y - self.oy);
        if i < 0 || j < 0 || i >= self.n || j >= self.n {
            return None;
        }
        Some((j * self.n + i) as usize)
    }

    fn h(&self, x: f32, y: f32) -> f32 {
        surface(
            &|cx, cy| match self.at(cx, cy) {
                Some(k) => self.corners[k],
                None => corner_of(self.l, cx, cy),
            },
            &|tx, ty| match self.at(tx, ty) {
                Some(k) => self.sinks[k],
                None => bowl_of(self.l, tx, ty),
            },
            x,
            y,
        )
    }

    fn normal(&self, x: f32, y: f32) -> Vec3 {
        let e = 0.07;
        vec3(
            self.h(x - e, y) - self.h(x + e, y),
            2.0 * e,
            self.h(x, y - e) - self.h(x, y + e),
        )
        .normalize()
    }

    fn tint(&self, x: i32, y: i32) -> Color {
        self.at(x, y).map_or(GRAY, |k| self.colors[k])
    }

    fn solid(&self, x: i32, y: i32) -> bool {
        self.at(x, y).is_some_and(|k| self.solid[k])
    }

    fn shadow(&self, x: f32, z: f32) -> f32 {
        if !self.l.lit {
            return 0.0;
        }
        shadow(
            &|cx, cy| self.at(cx, cy).map_or(Caster::None, |k| self.casters[k]),
            x,
            z,
        )
    }

    /// The colour of the ground at a point: the tiles' colours flowing into
    /// each other, in patches, darker by walls.
    fn color(&self, x: f32, y: f32) -> Color {
        let (gx, gy) = (x - 0.5, y - 0.5);
        let (i0, j0) = (gx.floor() as i32, gy.floor() as i32);
        let s = |t: f32| fade(((t - 0.28) / 0.44).clamp(0.0, 1.0));
        let (u, v) = (s(gx - i0 as f32), s(gy - j0 as f32));
        let top = mix(self.tint(i0, j0), self.tint(i0 + 1, j0), u);
        let bot = mix(self.tint(i0, j0 + 1), self.tint(i0 + 1, j0 + 1), u);
        let c = mix(top, bot, v);
        let patch = 0.91 + 0.18 * (fbm2(x * 0.33 + 7.0, y * 0.33 - 3.0) * 0.5 + 0.5);
        let fine = 0.97 + 0.06 * hashf(hash2((x * 16.0) as i32, (y * 16.0) as i32));
        // shade in the corners by walls and rocks
        let (tx, ty) = (x.floor() as i32, y.floor() as i32);
        let mut occ = 0.0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (ax, ay) = (tx + dx, ty + dy);
                if !self.solid(ax, ay) {
                    continue;
                }
                let ddx = (ax as f32 - x).max(x - (ax + 1) as f32).max(0.0);
                let ddy = (ay as f32 - y).max(y - (ay + 1) as f32).max(0.0);
                occ += (1.0 - (ddx * ddx + ddy * ddy).sqrt() / 0.6).max(0.0);
            }
        }
        let ao = 1.0 - 0.36 * (occ * 0.75f32).min(1.0);
        // warm and cool patches, as if painted
        let hue = fade((fbm2(x * 0.17 - 11.0, y * 0.17 + 5.0) * 0.8 + 0.5).clamp(0.0, 1.0));
        let warm = vec3(1.08, 1.03, 0.84);
        let cool = vec3(0.9, 1.0, 1.07);
        let k = cool.lerp(warm, hue) * (patch * fine * ao);
        let c = Color::new(c.r * k.x, c.g * k.y, c.b * k.z, c.a);
        shaded(c, self.shadow(x, y))
    }
}

/// Paved floors: their tile is the mortar, the stones lie on it.
fn paved(key: &str) -> bool {
    matches!(
        key,
        "stone_floor"
            | "dark_floor"
            | "sandstone_floor"
            | "basalt_floor"
            | "cobblestone"
            | "house_floor"
    )
}

/// A colour of nature as painted: a little less loud, leaning to olive and
/// warm.
fn painted(c: Color) -> Color {
    let lum = c.r * 0.3 + c.g * 0.55 + c.b * 0.15;
    let k = 0.24;
    Color::new(
        c.r + (lum * 1.08 - c.r) * k,
        c.g + (lum * 1.02 - c.g) * k,
        c.b + (lum * 0.8 - c.b) * k,
        c.a,
    )
}

/// The colour a tile gives the ground under it.
fn ground_tint(l: &Level, x: i32, y: i32, def: &TileDef) -> Color {
    let g = ground_of(l, x, y, def);
    let c = base_color(g);
    match g.key.as_str() {
        // a lake bed: sand and mud seen through the water
        "water" | "deep_water" => mix(c, Color::new(0.34, 0.31, 0.22, 1.0), 0.85),
        k if paved(k) => mul(c, 0.55),
        _ => painted(c),
    }
}

/// The ground a tile lies on (itself, if it is ground).
fn ground_of<'a>(l: &Level, x: i32, y: i32, def: &'a TileDef) -> &'a TileDef {
    let flat = def.walkable || matches!(def.key.as_str(), "deep_water" | "void");
    if flat && !decor(&def.key) {
        def
    } else {
        WorldRenderer::ground_under(l, x, y, def).unwrap_or(def)
    }
}

pub(super) fn build_chunk(
    gb: &mut Batch,
    b: &mut Batch,
    l: &Level,
    explored: &Bitset,
    cx: i32,
    cy: i32,
) {
    let db = content::db();
    let f = Field::new(l, cx, cy);
    let ch = terrain::CHUNK;
    for j in 0..ch {
        for i in 0..ch {
            let (x, y) = (cx * ch + i, cy * ch + j);
            if !l.inside(x, y) || !explored.get((y * l.w + x) as usize) {
                continue;
            }
            let def = db.tile(l.at(x, y));
            let flat = def.walkable || matches!(def.key.as_str(), "deep_water" | "void");
            let g = ground_of(l, x, y, def);
            if g.key != "void" {
                ground_tile(gb, &f, g, x, y);
            }
            if is_water(&g.key) {
                water(b, &f, g, x, y);
            }
            let h0 = f.h(x as f32 + 0.5, y as f32 + 0.5);
            if flat {
                detail(b, &f, def, x, y, h0);
            } else {
                standing(b, l, def, x, y, h0);
            }
        }
    }
}

/// A tile of ground: a grid of faces following the land.
fn ground_tile(b: &mut Batch, f: &Field, def: &TileDef, x: i32, y: i32) {
    let glow = match def.key.as_str() {
        "lava" => 1.0,
        "magma_crack" => 0.5,
        _ => 0.0,
    };
    let old = b.flags;
    b.flags |= SMOOTH;
    if matches!(def.key.as_str(), "ice" | "ice_floor") {
        b.flags |= SHINY;
    }
    b.mat = if glow > 0.0 { 0 } else { GROUND };
    let v0 = b.v.len() as u32;
    let k = 1.0 / SUB as f32;
    for j in 0..=SUB {
        for i in 0..=SUB {
            let (px, py) = (x as f32 + i as f32 * k, y as f32 + j as f32 * k);
            let c = if glow > 0.0 {
                // lava burns brighter in its veins
                let v = noise3(vec3(px * 2.5, py * 2.5, 0.0)) * 0.5 + 0.5;
                mix(
                    col(&def.fg),
                    Color::new(1.0, 0.85, 0.3, 1.0),
                    v * 0.6 * glow,
                )
            } else {
                f.color(px, py)
            };
            b.vert(vec3(px, f.h(px, py), py), f.normal(px, py), c, glow);
        }
    }
    let row = (SUB + 1) as u32;
    for j in 0..SUB as u32 {
        for i in 0..SUB as u32 {
            let a = v0 + j * row + i;
            let (bb, c, d) = (a + 1, a + row + 1, a + row);
            if (i + j) % 2 == 0 {
                b.index(a, d, c);
                b.index(a, c, bb);
            } else {
                b.index(a, d, bb);
                b.index(bb, d, c);
            }
        }
    }
    b.flags = old;
    b.mat = 0;
}

/// The surface of a lake over its tile: turquoise in the shallows, deep
/// blue far from the shore, foam where it laps the land.
fn water(b: &mut Batch, f: &Field, def: &TileDef, x: i32, y: i32) {
    let deep_c = if def.key == "deep_water" {
        vec3(0.08, 0.27, 0.48)
    } else {
        vec3(0.12, 0.42, 0.58)
    };
    let shallow = vec3(0.36, 0.78, 0.74);
    let old = b.flags;
    b.flags = WATER | SMOOTH;
    let v0 = b.v.len() as u32;
    let k = 1.0 / SUB as f32;
    for j in 0..=SUB {
        for i in 0..=SUB {
            let (px, py) = (x as f32 + i as f32 * k, y as f32 + j as f32 * k);
            let depth = WATER_Y - f.h(px, py);
            let t = fade((depth / 0.3).clamp(0.0, 1.0));
            let mut c = shallow.lerp(deep_c, t);
            let foam = 1.0 - fade((depth / 0.05).clamp(0.0, 1.0));
            c = c.lerp(vec3(0.92, 0.97, 1.0), foam * 0.8);
            let a = (0.5 + 0.42 * t).max(foam * 0.9);
            let col = shaded(Color::new(c.x, c.y, c.z, a), f.shadow(px, py));
            b.vert(vec3(px, WATER_Y, py), Vec3::Y, col, 0.0);
        }
    }
    let row = (SUB + 1) as u32;
    for j in 0..SUB as u32 {
        for i in 0..SUB as u32 {
            let a = v0 + j * row + i;
            b.index(a, a + row, a + row + 1);
            b.index(a, a + row + 1, a + 1);
        }
    }
    b.flags = old;
}

/// A point at random on a tile (k picks which).
fn spot(x: i32, y: i32, k: u32) -> (f32, f32) {
    let s = hash2(x, y).wrapping_add(k.wrapping_mul(7919));
    (
        x as f32 + 0.06 + 0.88 * hashf(s),
        y as f32 + 0.06 + 0.88 * hashf(s.wrapping_add(1)),
    )
}

/// One blade of grass (or a reed): it bends with its lean and sways more
/// toward its tip.
fn blade(b: &mut Batch, base: Vec3, h: f32, lean: Vec3, side: Vec3, c: Color) {
    let old = b.sway;
    let (flags, mat) = (b.flags, b.mat);
    // blades are lit softly, as the ground under them (and are too thin
    // for a material)
    b.flags |= SMOOTH;
    b.mat = 0;
    let v0 = b.v.len() as u32;
    let up = (Vec3::Y * 3.0 + side.cross(Vec3::Y).normalize_or_zero()).normalize();
    for t in [0.0f32, 0.4, 0.75] {
        let p = base + Vec3::Y * h * t + lean * t * t;
        let w = 1.0 - t;
        let cc = mix(mul(c, 0.72), mul(c, 1.14), t);
        b.sway = t * t;
        b.vert(p - side * w, up, cc, 0.0);
        b.vert(p + side * w, up, cc, 0.0);
    }
    b.sway = 1.0;
    b.vert(base + Vec3::Y * h + lean, up, mul(c, 1.18), 0.0);
    for k in 0..2u32 {
        let a = v0 + k * 2;
        b.index(a, a + 1, a + 3);
        b.index(a, a + 3, a + 2);
    }
    b.index(v0 + 4, v0 + 5, v0 + 6);
    b.sway = old;
    b.flags = flags;
    b.mat = mat;
}

/// Grass on a tile: `n` blades between `lo` and `hi` tall.
#[allow(clippy::too_many_arguments)]
fn grass(b: &mut Batch, f: &Field, x: i32, y: i32, n: u32, lo: f32, hi: f32, c: Color, w: f32) {
    for k in 0..n {
        let s = hash2(x, y).wrapping_add(k * 131);
        let (px, py) = spot(x, y, k);
        let base = vec3(px, f.h(px, py) - 0.01, py);
        let h = lo + (hi - lo) * hashf(s + 2);
        let a = hashf(s + 3) * TAU;
        let lean = vec3(a.cos(), 0.0, a.sin()) * h * (0.2 + 0.4 * hashf(s + 4));
        let fa = a + PI / 2.0 + (hashf(s + 5) - 0.5);
        let side = vec3(fa.cos(), 0.0, fa.sin()) * w;
        let tip = mix(c, Color::new(0.85, 0.82, 0.35, 1.0), 0.25 * hashf(s + 7));
        let cc = shaded(mul(tip, 0.85 + 0.3 * hashf(s + 6)), f.shadow(px, py));
        blade(b, base, h, lean, side, cc);
    }
}

/// A little flower: a stem and a ring of petals.
fn flower(b: &mut Batch, base: Vec3, h: f32, c: Color, s: u32) {
    let stem = Color::new(0.3, 0.55, 0.22, 1.0);
    blade(
        b,
        base,
        h,
        vec3(0.01, 0.0, 0.0),
        vec3(0.008, 0.0, 0.0),
        stem,
    );
    let top = base + vec3(0.01, h, 0.0);
    let (old, mat) = (b.sway, b.mat);
    b.sway = 1.0;
    b.mat = 0;
    let n = 5;
    let r = 0.045 + 0.02 * hashf(s);
    let a0 = hashf(s + 1) * TAU;
    let mid = b.vert(
        top + vec3(0.0, 0.006, 0.0),
        Vec3::Y,
        mul(Color::new(1.0, 0.85, 0.3, 1.0), 1.0),
        0.0,
    );
    for k in 0..n {
        let a = a0 + k as f32 / n as f32 * TAU;
        let d = vec3(a.cos(), 0.0, a.sin());
        let side = vec3(-d.z, 0.0, d.x) * r * 0.45;
        let tip = top + d * r + vec3(0.0, 0.012, 0.0);
        let half = top + d * r * 0.5;
        let p = [half - side, tip, half + side];
        let i = b.vert(top, Vec3::Y, c, 0.0);
        for q in p {
            b.vert(q, Vec3::Y, mul(c, 1.08), 0.0);
        }
        b.index(i, i + 1, i + 2);
        b.index(i, i + 2, i + 3);
    }
    let _ = mid;
    b.sway = old;
    b.mat = mat;
}

/// A stone slab of a floor: its corners, the height of its top, a bevel.
#[allow(clippy::too_many_arguments)]
fn paver(b: &mut Batch, x0: f32, z0: f32, x1: f32, z1: f32, y: f32, h: f32, bv: f32, c: Color) {
    let top = [
        vec3(x0 + bv, y + h, z0 + bv),
        vec3(x1 - bv, y + h, z0 + bv),
        vec3(x1 - bv, y + h, z1 - bv),
        vec3(x0 + bv, y + h, z1 - bv),
    ];
    let bot = [
        vec3(x0, y, z0),
        vec3(x1, y, z0),
        vec3(x1, y, z1),
        vec3(x0, y, z1),
    ];
    b.quad(top[0], top[3], top[2], top[1], c, 0.0);
    for i in 0..4 {
        let j = (i + 1) % 4;
        b.quad(bot[j], bot[i], top[i], top[j], mul(c, 0.9), 0.0);
    }
}

/// A rounded cobble.
fn cobble(b: &mut Batch, c: Vec3, r: f32, h: f32, col: Color) {
    let (v0, i0) = (b.v.len(), b.i.len());
    let n = 8;
    let top = b.vert(c + vec3(0.0, h, 0.0), Vec3::Y, mul(col, 1.05), 0.0);
    for (k, y) in [(0.72, h * 0.8), (1.0, 0.0)] {
        for i in 0..n {
            let a = i as f32 / n as f32 * TAU;
            b.vert(
                c + vec3(a.cos() * r * k, y, a.sin() * r * k * 0.9),
                Vec3::Y,
                col,
                0.0,
            );
        }
    }
    let n32 = n as u32;
    for i in 0..n32 {
        let j = (i + 1) % n32;
        b.index(top, top + 1 + j, top + 1 + i);
        let (a, bb) = (top + 1 + i, top + 1 + j);
        b.index(a, bb, bb + n32);
        b.index(a, bb + n32, a + n32);
    }
    b.smooth(v0, i0);
}

/// What lies on a tile one can walk on.
fn detail(b: &mut Batch, f: &Field, def: &TileDef, x: i32, y: i32, h0: f32) {
    let s = hash2(x, y);
    let c = painted(col(&def.fg));
    let (fx, fy) = (x as f32, y as f32);
    b.mat = match def.key.as_str() {
        "rubble" | "road" | "cave_floor" | "sand" | "beach" | "stone_floor" | "dark_floor"
        | "sandstone_floor" | "basalt_floor" | "cobblestone" => STONE,
        "house_floor" => WOOD,
        "garden" => LEAVES,
        _ => terrain::material_of(&def.key),
    };
    detail_of(b, f, def, x, y, h0, s, c, fx, fy);
    b.mat = 0;
}

#[allow(clippy::too_many_arguments)]
fn detail_of(
    b: &mut Batch,
    f: &Field,
    def: &TileDef,
    x: i32,
    y: i32,
    h0: f32,
    s: u32,
    c: Color,
    fx: f32,
    fy: f32,
) {
    match def.key.as_str() {
        "grass" | "grass2" | "hill" => {
            grass(b, f, x, y, 9, 0.12, 0.24, mul(c, 1.05), 0.018);
            if hashf(s + 40) < 0.12 {
                let (px, py) = spot(x, y, 50);
                let fc = [col("#ffffff"), col("#ffd040"), col("#b07aff")][(s % 3) as usize];
                flower(b, vec3(px, f.h(px, py), py), 0.14, fc, s);
            }
        }
        "forest_floor" => {
            grass(b, f, x, y, 4, 0.1, 0.2, mul(c, 1.25), 0.016);
            // fallen leaves
            b.mat = LEAVES;
            for k in 0..4 {
                let (px, py) = spot(x, y, 20 + k);
                let p = vec3(px, f.h(px, py) + 0.008, py);
                let a = hashf(s + k * 3) * TAU;
                let d = vec3(a.cos(), 0.0, a.sin()) * 0.05;
                let e = vec3(-d.z, 0.0, d.x) * 0.6;
                let lc =
                    [col("#a0642a"), col("#c08a30"), col("#7a5a28")][(k as usize + s as usize) % 3];
                b.quad(p - d, p - e, p + d, p + e, lc, 0.0);
            }
            if hashf(s + 70) < 0.22 {
                fern(b, f, x, y, mul(c, 1.6), s);
            }
        }
        "blight_grass" => grass(b, f, x, y, 5, 0.1, 0.2, mul(c, 1.1), 0.016),
        "tall_grass" => grass(b, f, x, y, 18, 0.32, 0.52, mul(c, 1.05), 0.022),
        "reeds" | "swamp" => {
            let n = if def.key == "reeds" { 9 } else { 3 };
            grass(b, f, x, y, n, 0.4, 0.68, col("#6a8a3a"), 0.014);
            for k in 0..n / 3 {
                let (px, py) = spot(x, y, k);
                let hh = 0.4 + 0.28 * hashf(hash2(x, y).wrapping_add(k * 131) + 2);
                let p = vec3(px, f.h(px, py) + hh * 0.86, py);
                let old = b.sway;
                b.sway = 0.8;
                b.capsule(p, p + vec3(0.0, 0.1, 0.0), 0.02, 0.02, col("#6a4026"), 0.0);
                b.sway = old;
            }
        }
        "garden" => {
            for k in 0..4 {
                let (px, py) = spot(x, y, k);
                let p = vec3(px, f.h(px, py) + 0.05, py);
                b.blob(
                    Xf::at(p),
                    Vec3::ZERO,
                    vec3(0.09, 0.07, 0.09),
                    0.2,
                    s + k,
                    mul(c, 1.1),
                    0.0,
                );
            }
            grass(b, f, x, y, 5, 0.1, 0.2, mul(c, 1.1), 0.016);
            for k in 0..3 {
                let (px, py) = spot(x, y, 10 + k);
                let fc = [
                    col("#ff6a8a"),
                    col("#ffd040"),
                    col("#ffffff"),
                    col("#b07aff"),
                ][((k + s) % 4) as usize];
                flower(b, vec3(px, f.h(px, py), py), 0.2, fc, s + k);
            }
        }
        "flowers" => {
            grass(b, f, x, y, 5, 0.1, 0.18, col("#4c8a35"), 0.016);
            for k in 0..6 {
                let (px, py) = spot(x, y, 30 + k);
                let fc = mul(c, 0.85 + 0.3 * hashf(s + k * 13));
                flower(
                    b,
                    vec3(px, f.h(px, py), py),
                    0.14 + 0.06 * hashf(s + k),
                    fc,
                    s + k,
                );
            }
        }
        "rubble" => {
            for k in 0..4 {
                let (px, py) = spot(x, y, k);
                let r = 0.06 + 0.06 * hashf(s + k);
                b.stone(
                    Xf::at(vec3(px, f.h(px, py) + r * 0.3, py)),
                    Vec3::ZERO,
                    vec3(r, r * 0.7, r * 0.9),
                    s + k + 1,
                    mul(c, 0.9 + 0.2 * hashf(s + k + 7)),
                    0.0,
                );
            }
        }
        "road" | "cave_floor" | "sand" | "beach" => {
            let n = if def.key == "road" { 3 } else { 2 };
            for k in 0..n {
                if hashf(s + 90 + k) < 0.55 {
                    continue;
                }
                let (px, py) = spot(x, y, 60 + k);
                let r = 0.025 + 0.025 * hashf(s + k + 3);
                b.stone(
                    Xf::at(vec3(px, f.h(px, py) + r * 0.2, py)),
                    Vec3::ZERO,
                    vec3(r, r * 0.6, r),
                    s + k + 1,
                    mul(c, 0.8),
                    0.0,
                );
            }
        }
        "stone_floor" | "dark_floor" | "sandstone_floor" | "basalt_floor" => {
            let shift = if y % 2 == 0 { 0.0 } else { 0.25 };
            for j in 0..2 {
                for i in 0..2 {
                    let k = hashf(s.wrapping_add(i * 5 + j * 11));
                    let x0 = fx + i as f32 * 0.5 + if j == 1 { shift } else { 0.0 };
                    let z0 = fy + j as f32 * 0.5;
                    let x1 = (x0 + 0.5).min(fx + 1.0);
                    paver(
                        b,
                        x0 + 0.012,
                        z0 + 0.012,
                        x1 - 0.012,
                        z0 + 0.488,
                        h0,
                        0.022,
                        0.018,
                        mul(c, 0.9 + 0.18 * k),
                    );
                    if j == 1 && i == 1 && shift > 0.0 {
                        paver(
                            b,
                            fx + 0.012,
                            z0 + 0.012,
                            fx + shift - 0.012,
                            z0 + 0.488,
                            h0,
                            0.022,
                            0.018,
                            mul(c, 0.95),
                        );
                    }
                }
            }
        }
        "cobblestone" => {
            for j in 0..3 {
                for i in 0..3 {
                    let k = s.wrapping_add((i * 3 + j) as u32 * 17);
                    let p = vec3(
                        fx + 0.17 + i as f32 * 0.33 + (hashf(k) - 0.5) * 0.05,
                        h0,
                        fy + 0.17 + j as f32 * 0.33 + (hashf(k + 1) - 0.5) * 0.05,
                    );
                    cobble(
                        b,
                        p,
                        0.14 + 0.02 * hashf(k + 2),
                        0.035,
                        mul(c, 0.85 + 0.25 * hashf(k + 3)),
                    );
                }
            }
        }
        "house_floor" => {
            for k in 0..4 {
                let z0 = fy + k as f32 * 0.25;
                let t = hashf(s + k);
                paver(
                    b,
                    fx + 0.005,
                    z0 + 0.008,
                    fx + 0.995,
                    z0 + 0.242,
                    h0,
                    0.015,
                    0.008,
                    mul(c, 0.85 + 0.25 * t),
                );
            }
        }
        _ => terrain::ground_detail(b, def, x, y, h0),
    }
}

/// A fern: fronds spreading from the middle.
fn fern(b: &mut Batch, f: &Field, x: i32, y: i32, c: Color, s: u32) {
    let (px, py) = spot(x, y, 77);
    let o = vec3(px, f.h(px, py), py);
    let old = b.sway;
    b.sway = 0.5;
    b.mat = LEAVES;
    for k in 0..6 {
        let a = k as f32 / 6.0 * TAU + hashf(s + k);
        let d = vec3(a.cos(), 0.0, a.sin());
        let tip = o + d * 0.32 + vec3(0.0, 0.12, 0.0);
        let side = vec3(-d.z, 0.0, d.x);
        b.leaf(
            o + vec3(0.0, 0.02, 0.0),
            tip,
            side,
            Vec3::Y,
            0.06,
            mul(c, 0.9 + 0.2 * hashf(s + k * 3)),
            0.0,
        );
    }
    b.sway = old;
}

/// What stands on a tile one cannot walk through.
fn standing(b: &mut Batch, l: &Level, def: &TileDef, x: i32, y: i32, h0: f32) {
    b.mat = 0;
    standing_of(b, l, def, x, y, h0);
    b.mat = 0;
}

fn standing_of(b: &mut Batch, l: &Level, def: &TileDef, x: i32, y: i32, h0: f32) {
    let s = hash2(x, y);
    let c = painted(col(&def.fg));
    let key = def.key.as_str();
    let o = vec3(x as f32 + 0.5, h0, y as f32 + 0.5);
    if let Some(h) = wall_height(key) {
        wall(b, l, def, x, y, h);
        return;
    }
    if peak(key) {
        let snow = match key {
            "snow" => 1.0,
            "mountain" => 0.0,
            _ => -1.0,
        };
        mountain(b, o, c, s, snow, key == "ice_rock");
        return;
    }
    match key {
        "tree" => tree(b, o, c, s),
        "pine" | "snow_pine" => fir(b, o, c, s, key == "snow_pine"),
        "palm" => palm(b, o, c, s),
        "bush" => {
            let old = b.sway;
            b.sway = 0.25;
            b.mat = LEAVES;
            for k in 0..4 {
                let a = k as f32 * 1.7 + hashf(s) * 3.0;
                let r = if k == 0 {
                    0.3
                } else {
                    0.18 + 0.05 * hashf(s + k)
                };
                let p = o + vec3(
                    a.cos() * 0.14 * (k.min(1) as f32),
                    0.16 + 0.06 * k as f32,
                    a.sin() * 0.14 * (k.min(1) as f32),
                );
                b.lump(
                    Xf::at(p),
                    Vec3::ZERO,
                    vec3(r, r * 0.8, r),
                    0.22,
                    s + k,
                    if k == 0 { 2 } else { 1 },
                    mul(c, 0.9 + 0.08 * k as f32),
                    0.0,
                );
            }
            b.sway = old;
            b.mat = 0;
            if hashf(s + 3) < 0.4 {
                for k in 0..5 {
                    let a = k as f32 * 1.3 + hashf(s + 9) * 5.0;
                    let p = o + vec3(a.cos() * 0.24, 0.2 + 0.1 * (k % 2) as f32, a.sin() * 0.24);
                    b.ellipsoid(
                        Xf::at(p),
                        Vec3::ZERO,
                        Vec3::splat(0.025),
                        col("#d03040"),
                        0.0,
                    );
                }
            }
        }
        "dead_tree" | "charred_tree" | "twisted_tree" => {
            bare_tree(b, o, c, s, key == "twisted_tree")
        }
        "cactus" => {
            b.tube(
                &[o - Vec3::Y * 0.05, o + Vec3::Y * 0.5, o + Vec3::Y * 0.85],
                &[0.12, 0.11, 0.1],
                true,
                c,
                0.0,
            );
            for sgn in [-1.0f32, 1.0] {
                let base = o + vec3(sgn * 0.08, 0.32 + 0.12 * sgn.max(0.0), 0.0);
                b.tube(
                    &[
                        base,
                        base + vec3(sgn * 0.17, 0.02, 0.0),
                        base + vec3(sgn * 0.2, 0.12, 0.0),
                        base + vec3(sgn * 0.2, 0.3, 0.0),
                    ],
                    &[0.06, 0.06, 0.06, 0.055],
                    true,
                    mul(c, 0.95),
                    0.0,
                );
            }
        }
        _ => {
            b.mat = terrain::material_of(key);
            terrain::prop(b, l, def, x, y, h0.max(0.0))
        }
    }
}

/// A broad-leaved tree: a trunk with roots and branches, a crown of
/// lumps of leaves, lighter on top.
fn tree(b: &mut Batch, o: Vec3, c: Color, s: u32) {
    let k = tree_size(s);
    let bark = col("#5e4128");
    b.mat = BARK;
    let lean = vec3(
        (hashf(s + 5) - 0.5) * 0.12,
        0.0,
        (hashf(s + 6) - 0.5) * 0.12,
    );
    let top = o + vec3(0.0, 0.66 * k, 0.0) + lean;
    b.tube(
        &[
            o - Vec3::Y * 0.05,
            o + Vec3::Y * 0.12 * k + lean * 0.15,
            o + Vec3::Y * 0.4 * k + lean * 0.6,
            top,
        ],
        &[0.12 * k, 0.08 * k, 0.065 * k, 0.05 * k],
        false,
        bark,
        0.0,
    );
    for r in 0..4 {
        let a = r as f32 * 1.57 + hashf(s + 20) * 1.5;
        let d = vec3(a.cos(), 0.0, a.sin());
        b.tube(
            &[
                o + Vec3::Y * 0.12 * k,
                o + d * 0.16 * k + Vec3::Y * 0.02,
                o + d * 0.26 * k - Vec3::Y * 0.03,
            ],
            &[0.045 * k, 0.03 * k, 0.0],
            false,
            bark,
            0.0,
        );
    }
    let crown = top + vec3(0.0, 0.2 * k, 0.0);
    let old = b.sway;
    b.sway = 0.3;
    for i in 0..3 {
        let a = i as f32 * 2.1 + hashf(s + 30) * 2.0;
        let d = vec3(a.cos(), 0.0, a.sin());
        let end = crown + d * 0.3 * k + vec3(0.0, -0.05, 0.0);
        b.tube(
            &[top - Vec3::Y * 0.15 * k, top + d * 0.12 * k, end],
            &[0.04 * k, 0.03 * k, 0.012],
            false,
            bark,
            0.0,
        );
    }
    b.mat = LEAVES;
    b.lump(
        Xf::at(crown),
        Vec3::ZERO,
        vec3(0.44, 0.38, 0.44) * k,
        0.3,
        s,
        1,
        c,
        0.0,
    );
    for i in 0..7 {
        let a = i as f32 / 7.0 * TAU + hashf(s + 40) * 2.0;
        let up = (hashf(s + 50 + i) - 0.4) * 0.3;
        let p = crown + vec3(a.cos() * 0.33, up, a.sin() * 0.33) * k;
        let r = (0.24 + 0.08 * hashf(s + 60 + i)) * k;
        let shade = if up > 0.0 { 1.12 } else { 0.86 };
        let warm = if up > 0.0 {
            mix(c, Color::new(0.72, 0.8, 0.28, 1.0), 0.18)
        } else {
            mix(c, Color::new(0.12, 0.3, 0.32, 1.0), 0.15)
        };
        b.lump(
            Xf::at(p),
            Vec3::ZERO,
            vec3(r, r * 0.85, r),
            0.3,
            s + 70 + i,
            1,
            mul(warm, shade),
            0.0,
        );
    }
    b.lump(
        Xf::at(crown + vec3(0.04, 0.3, -0.02) * k),
        Vec3::ZERO,
        vec3(0.26, 0.2, 0.26) * k,
        0.22,
        s + 9,
        1,
        mul(c, 1.2),
        0.0,
    );
    b.sway = old;
}

/// A fir: a trunk under tiers of drooping, ragged boughs.
fn fir(b: &mut Batch, o: Vec3, c: Color, s: u32, snowy: bool) {
    let k = 1.1 + 0.4 * hashf(s);
    let bark = col("#4e3624");
    b.mat = BARK;
    b.tube(
        &[o - Vec3::Y * 0.05, o + Vec3::Y * 0.5 * k],
        &[0.08 * k, 0.05 * k],
        false,
        bark,
        0.0,
    );
    let tiers = 5;
    let old = b.sway;
    b.sway = 0.18;
    // needles, as a fur of the boughs
    b.mat = FUR;
    for t in 0..tiers {
        let f = t as f32 / (tiers - 1) as f32;
        let y = (0.22 + f * 1.0) * k;
        let r = (0.5 - f * 0.34) * k;
        let hgt = (0.42 - f * 0.12) * k;
        let n = 14;
        let (v0, i0) = (b.v.len(), b.i.len());
        let apex = b.vert(o + vec3(0.0, y + hgt, 0.0), Vec3::Y, mul(c, 1.1), 0.0);
        let a0 = hashf(s + t) * TAU;
        for i in 0..n {
            let a = a0 + i as f32 / n as f32 * TAU;
            let rr = r * if i % 2 == 0 { 1.0 } else { 0.78 };
            let droop = if i % 2 == 0 { 0.06 * k } else { 0.0 };
            let tip_c = if snowy && t + 1 >= tiers - 1 {
                mix(c, WHITE, 0.6)
            } else {
                mul(c, 0.95)
            };
            b.vert(
                o + vec3(a.cos() * rr, y - droop, a.sin() * rr),
                Vec3::Y,
                tip_c,
                0.0,
            );
        }
        // the underside, back to the trunk
        let under = b.vert(
            o + vec3(0.0, y + 0.06 * k, 0.0),
            -Vec3::Y,
            mul(c, 0.55),
            0.0,
        );
        for i in 0..n as u32 {
            let j = (i + 1) % n as u32;
            b.index(apex, apex + 1 + j, apex + 1 + i);
            b.index(under, apex + 1 + i, apex + 1 + j);
        }
        b.smooth(v0, i0);
        if snowy {
            // snow lies on the upper side of the boughs
            for v in &mut b.v[v0..] {
                if v.normal().y > 0.55 && v.c[0] < 200 {
                    v.c = [236, 242, 250, 255];
                }
            }
        }
    }
    b.sway = old;
}

/// A palm: a ringed, bending trunk and drooping fronds.
fn palm(b: &mut Batch, o: Vec3, c: Color, s: u32) {
    let lean = vec3(0.16, 0.0, 0.08) * (0.7 + 0.6 * hashf(s));
    let pts: Vec<Vec3> = (0..=5)
        .map(|i| {
            let t = i as f32 / 5.0;
            o + vec3(0.0, 1.35 * t, 0.0) + lean * t * t
        })
        .collect();
    let bark = col("#9a7a4a");
    b.mat = BARK;
    b.tube(
        &pts,
        &[0.075, 0.065, 0.06, 0.055, 0.05, 0.048],
        false,
        bark,
        0.0,
    );
    for i in 1..5 {
        let t = i as f32 / 5.0;
        let p = o + vec3(0.0, 1.35 * t, 0.0) + lean * t * t;
        b.torus(
            Xf::at(p),
            Vec3::ZERO,
            0.066 - t * 0.02,
            0.01,
            mul(bark, 0.8),
            0.0,
        );
    }
    let top = pts[5];
    b.mat = 0;
    for k in 0..3 {
        let a = k as f32 * 2.1 + hashf(s);
        b.ellipsoid(
            Xf::at(top + vec3(a.cos() * 0.06, -0.06, a.sin() * 0.06)),
            Vec3::ZERO,
            Vec3::splat(0.05),
            col("#5a4020"),
            0.0,
        );
    }
    let old = b.sway;
    b.sway = 0.45;
    b.mat = LEAVES;
    for k in 0..8 {
        let a = k as f32 / 8.0 * TAU + hashf(s + 1);
        let d = vec3(a.cos(), 0.0, a.sin());
        let side = vec3(-d.z, 0.0, d.x);
        let rows: Vec<Vec<Vec3>> = (0..=7)
            .map(|i| {
                let t = i as f32 / 7.0;
                let mid = top + d * 0.7 * t + vec3(0.0, 0.18 * t - 0.5 * t * t, 0.0);
                let w = 0.16 * (t * PI).sin().max(0.02);
                let fold = vec3(0.0, -0.05 * (t * PI).sin(), 0.0);
                vec![mid - side * w + fold, mid, mid + side * w + fold]
            })
            .collect();
        b.grid(
            &rows,
            false,
            false,
            &|_, k| if k == 1 { mul(c, 0.85) } else { c },
            0.0,
        );
    }
    b.sway = old;
}

/// A leafless tree: a gnarled trunk forking twice.
fn bare_tree(b: &mut Batch, o: Vec3, c: Color, s: u32, twisted: bool) {
    let top = o + vec3(0.05, 0.85, -0.03);
    b.mat = BARK;
    b.tube(
        &[o - Vec3::Y * 0.05, o + vec3(0.02, 0.4, 0.01), top],
        &[0.09, 0.065, 0.04],
        false,
        c,
        0.0,
    );
    for k in 0..4u32 {
        let a = k as f32 * 1.6 + hashf(s + k) * 1.2;
        let from = o + vec3(0.0, 0.42 + 0.12 * k as f32, 0.0);
        let d = vec3(a.cos(), 0.0, a.sin());
        let mid = from + d * 0.2 + Vec3::Y * 0.14;
        let end = from + d * 0.38 + Vec3::Y * (0.3 + 0.15 * hashf(s + k * 3));
        b.tube(
            &[from, mid, end],
            &[0.032, 0.022, 0.0],
            false,
            mul(c, 0.92),
            0.0,
        );
        let twig = mid + vec3(-d.z, 0.0, d.x) * 0.14 + Vec3::Y * 0.12;
        b.tube(&[mid, twig], &[0.016, 0.0], false, mul(c, 0.9), 0.0);
        if twisted {
            b.mat = LEAVES;
            b.lump(
                Xf::at(end),
                Vec3::ZERO,
                vec3(0.12, 0.1, 0.12),
                0.25,
                s + k,
                1,
                mul(c, 1.3),
                0.0,
            );
            b.mat = BARK;
        }
    }
}

/// A mountain: a noisy cone of rock, snow on its top and on what faces up.
fn mountain(b: &mut Batch, o: Vec3, c: Color, s: u32, snow: f32, ice: bool) {
    let h = 1.0 + 1.0 * hashf(s);
    // rock in big slabs, not in little tiles
    b.mat = STONE;
    b.tex_k = 0.5;
    let r0 = 0.82;
    let (rings, n) = if b.fine { (12, 30) } else { (6, 16) };
    let apex = vec3((hashf(s + 1) - 0.5) * 0.3, h, (hashf(s + 2) - 0.5) * 0.3);
    let off = vec3(hashf(s) * 40.0, hashf(s + 3) * 40.0, 7.0);
    let (v0, i0) = (b.v.len(), b.i.len());
    let a0 = hashf(s + 4) * TAU;
    for r in 0..rings {
        let t = r as f32 / rings as f32;
        let rad = r0 * (1.0 - t).powf(0.9);
        for i in 0..n {
            let a = a0 + i as f32 / n as f32 * TAU;
            let d = vec3(a.cos(), 0.0, a.sin());
            let p = d * rad + apex * vec3(t, 1.0, t) * vec3(1.0, t, 1.0);
            let mut bump =
                noise3(p * 2.6 + off) * 0.16 * (1.0 - t * 0.6) + noise3(p * 6.0 - off) * 0.05;
            if b.fine {
                // ledges and knobs a finer mesh can hold
                bump += noise3(p * 12.0 + off * 0.5) * 0.025;
            }
            let q = o + p + d * bump + vec3(0.0, bump * 0.4 - if r == 0 { 0.08 } else { 0.0 }, 0.0);
            b.vert(q, Vec3::Y, c, 0.0);
        }
    }
    let top = b.vert(o + apex, Vec3::Y, c, 0.0);
    let n32 = n as u32;
    let base = v0 as u32;
    for r in 0..rings as u32 - 1 {
        for i in 0..n32 {
            let j = (i + 1) % n32;
            let (a, bb) = (base + r * n32 + i, base + r * n32 + j);
            let (cc, d) = (a + n32, bb + n32);
            b.index(a, cc, bb);
            b.index(bb, cc, d);
        }
    }
    let last = base + (rings as u32 - 1) * n32;
    for i in 0..n32 {
        b.index(last + i, top, last + (i + 1) % n32);
    }
    b.smooth(v0, i0);
    // rock darker in the folds, snow where it lies
    let line = if snow >= 1.0 { 0.15 } else { 0.58 };
    for v in &mut b.v[v0..] {
        let p = v.pos();
        let t = ((p.y - o.y) / h).clamp(0.0, 1.0);
        let nrm = v.normal();
        let k = 0.82 + 0.3 * (noise3(p * 4.0 + off) * 0.5 + 0.5);
        let mut cc = mul(c, k);
        if snow == 0.0 && t < 0.4 && nrm.y > 0.6 {
            // grass and moss where the slope is gentle
            cc = mix(
                cc,
                Color::new(0.4, 0.5, 0.26, 1.0),
                ((0.4 - t) * 4.0).clamp(0.0, 0.7),
            );
        }
        let snowy = snow >= 0.0 && (snow >= 1.0 || h > 1.4) && t + nrm.y * 0.25 > line + 0.2;
        if snowy {
            cc = mix(
                cc,
                Color::new(0.95, 0.97, 1.0, 1.0),
                ((t + nrm.y * 0.25 - line - 0.2) * 6.0).clamp(0.0, 1.0),
            );
        }
        let c8 = [cc.r, cc.g, cc.b].map(|x| (x.clamp(0.0, 1.0) * 255.0) as u8);
        v.c = [c8[0], c8[1], c8[2], 255];
    }
    if ice {
        for v in &mut b.v[v0..] {
            v.m[1] |= SHINY;
        }
    }
    b.tex_k = 0.0;
    // boulders at its foot
    for k in 0..2 {
        let a = hashf(s + 7 + k) * TAU;
        let p = o + vec3(a.cos(), 0.0, a.sin()) * 0.55;
        let r = 0.14 + 0.1 * hashf(s + 9 + k);
        b.stone(
            Xf::at(p),
            vec3(0.0, r * 0.4, 0.0),
            vec3(r, r * 0.8, r),
            s + k + 1,
            mul(c, 0.85),
            0.0,
        );
    }
}

/// A block of wall in the detailed look: its faces toward open tiles are
/// built of stones, logs, rock, ice or timber and plaster.
fn wall(b: &mut Batch, l: &Level, def: &TileDef, x: i32, y: i32, h: f32) {
    let s = hash2(x, y);
    let c = col(&def.fg);
    let key = def.key.as_str();
    let (fx, fy) = (x as f32, y as f32);
    let rough = key == "cave_wall";
    let ruin = key == "ruin_wall";
    let h = if ruin { h * (0.5 + 0.7 * hashf(s)) } else { h };
    let top_y = |px: f32, pz: f32| -> f32 {
        if rough {
            h + noise3(vec3(px * 2.0, 3.0, pz * 2.0)) * 0.22
        } else {
            h
        }
    };
    // the top
    let old = b.flags;
    if key == "ice_wall" {
        b.flags |= SHINY;
    }
    b.mat = if key == "house_wall" { WOOD } else { STONE };
    let tc = if key == "house_wall" {
        col("#5a3e28")
    } else {
        mul(c, 1.12)
    };
    if rough {
        let rows: Vec<Vec<Vec3>> = (0..=3)
            .map(|j| {
                (0..=3)
                    .map(|i| {
                        let (px, pz) = (fx + i as f32 / 3.0, fy + j as f32 / 3.0);
                        vec3(px, top_y(px, pz), pz)
                    })
                    .collect()
            })
            .collect();
        b.grid(&rows, false, false, &|_, _| tc, 0.0);
    } else {
        let t = [
            vec3(fx, h, fy),
            vec3(fx + 1.0, h, fy),
            vec3(fx + 1.0, h, fy + 1.0),
            vec3(fx, h, fy + 1.0),
        ];
        b.quad(t[0], t[3], t[2], t[1], tc, 0.0);
    }
    // faces: north (y-1), east, south, west; corners from-to along each
    let corners = [
        vec3(fx, 0.0, fy),
        vec3(fx + 1.0, 0.0, fy),
        vec3(fx + 1.0, 0.0, fy + 1.0),
        vec3(fx, 0.0, fy + 1.0),
    ];
    let sides = [
        ((0, -1), 1, 0),
        ((1, 0), 2, 1),
        ((0, 1), 3, 2),
        ((-1, 0), 0, 3),
    ];
    for (fi, ((dx, dy), i, j)) in sides.into_iter().enumerate() {
        let open = l.inside(x + dx, y + dy) && {
            let nd = content::db().tile(l.at(x + dx, y + dy));
            wall_height(&nd.key).is_none() || ruin
        };
        if !open {
            continue;
        }
        let nrm = vec3(dx as f32, 0.0, dy as f32);
        let (a, bb) = (corners[i], corners[j]);
        let seed = s.wrapping_add(fi as u32 * 977);
        match key {
            "cave_wall" | "ice_wall" => rock_face(b, a, bb, nrm, c, &top_y, key == "ice_wall"),
            "house_wall" => timber_face(b, a, bb, nrm, h, c, seed),
            _ => {
                let big = matches!(key, "city_wall" | "sandstone_wall");
                stone_face(b, a, bb, nrm, h, c, seed, big, ruin);
            }
        }
    }
    b.flags = old;
    b.mat = STONE;
    if key == "city_wall" {
        for k in 0..2 {
            b.cuboid(
                Xf::at(vec3(fx + 0.25 + k as f32 * 0.5, h, fy + 0.5)),
                vec3(0.0, 0.13, 0.0),
                vec3(0.14, 0.13, 0.42),
                tc,
                0.0,
            );
        }
    }
}

/// A face of stones: rows of blocks standing out of dark mortar.
#[allow(clippy::too_many_arguments)]
fn stone_face(
    b: &mut Batch,
    a: Vec3,
    bb: Vec3,
    nrm: Vec3,
    h: f32,
    c: Color,
    s: u32,
    big: bool,
    ruin: bool,
) {
    let along = bb - a;
    let lo = -0.1;
    // the mortar behind
    let p = |u: f32, y: f32, out: f32| a + along * u + Vec3::Y * y + nrm * out;
    b.mat = PLASTER;
    b.quad(
        p(0.0, lo, 0.0),
        p(1.0, lo, 0.0),
        p(1.0, h, 0.0),
        p(0.0, h, 0.0),
        mul(c, 0.5),
        0.0,
    );
    b.mat = STONE;
    let rh = if big { 0.34 } else { 0.24 };
    let rows = ((h - lo) / rh).round().max(1.0) as i32;
    let rh = (h - lo) / rows as f32;
    let g = 0.018;
    for r in 0..rows {
        let y0 = lo + r as f32 * rh;
        let n = if big {
            2
        } else {
            2 + (hashf(s + r as u32) * 1.6) as i32
        };
        let shift = if r % 2 == 0 { 0.0 } else { 0.5 / n as f32 };
        let mut cuts: Vec<f32> = (0..=n).map(|k| k as f32 / n as f32 + shift).collect();
        cuts.retain(|u| *u < 1.0);
        cuts.insert(0, 0.0);
        if shift > 0.0 {
            cuts.dedup();
        }
        cuts.push(1.0);
        for k in 0..cuts.len() - 1 {
            let (u0, u1) = (cuts[k], cuts[k + 1]);
            if u1 - u0 < 0.05 {
                continue;
            }
            let ks = s.wrapping_add((r * 13 + k as i32) as u32);
            if ruin && r == rows - 1 && hashf(ks) < 0.45 {
                continue;
            }
            let out = 0.02 + 0.02 * hashf(ks + 1);
            let cc = mul(c, 0.84 + 0.26 * hashf(ks + 2));
            let (ua, ub) = (u0 + g, u1 - g);
            let (ya, yb) = (y0 + g, (y0 + rh - g).min(h - 0.005));
            let bv = 0.025;
            let f = [
                p(ua + bv, ya + bv, out),
                p(ub - bv, ya + bv, out),
                p(ub - bv, yb - bv, out),
                p(ua + bv, yb - bv, out),
            ];
            let k4 = [
                p(ua, ya, 0.0),
                p(ub, ya, 0.0),
                p(ub, yb, 0.0),
                p(ua, yb, 0.0),
            ];
            b.quad(f[0], f[1], f[2], f[3], cc, 0.0);
            for e in 0..4 {
                let n2 = (e + 1) % 4;
                b.quad(k4[e], k4[n2], f[n2], f[e], mul(cc, 0.9), 0.0);
            }
        }
    }
}

/// A face of rough rock (caves) or of ice, bulging with noise.
#[allow(clippy::too_many_arguments)]
fn rock_face(
    b: &mut Batch,
    a: Vec3,
    bb: Vec3,
    nrm: Vec3,
    c: Color,
    top_y: &dyn Fn(f32, f32) -> f32,
    ice: bool,
) {
    let along = bb - a;
    let (nu, nv) = (4, 6);
    // rock in big slabs
    b.mat = STONE;
    b.tex_k = 0.6;
    let rows: Vec<Vec<Vec3>> = (0..=nv)
        .map(|j| {
            (0..=nu)
                .map(|i| {
                    let u = i as f32 / nu as f32;
                    let base = a + along * u;
                    let t = j as f32 / nv as f32;
                    let hy = top_y(base.x, base.z);
                    let y = -0.1 + (hy + 0.1) * t;
                    let edge = (u * PI).sin() * (t * PI).sin().max(0.0).sqrt();
                    let k = if ice { 0.05 } else { 0.12 };
                    let bump = noise3(vec3(base.x * 2.3, y * 2.3, base.z * 2.3)) * k * edge;
                    base + Vec3::Y * y + nrm * bump.max(-0.02)
                })
                .collect()
        })
        .collect();
    let col_at = |r: usize, k: usize| -> Color {
        let p = rows[r][k];
        let n = noise3(p * 3.1) * 0.5 + 0.5;
        mul(c, 0.78 + 0.32 * n)
    };
    b.grid(&rows, false, false, &col_at, 0.0);
    b.tex_k = 0.0;
}

/// A face of a house: plaster between timber posts and beams, a window
/// lit from within on some.
#[allow(clippy::too_many_arguments)]
fn timber_face(b: &mut Batch, a: Vec3, bb: Vec3, nrm: Vec3, h: f32, c: Color, s: u32) {
    let along = bb - a;
    let p = |u: f32, y: f32, out: f32| a + along * u + Vec3::Y * y + nrm * out;
    let plaster = mix(col("#e4d8be"), c, 0.12);
    b.mat = PLASTER;
    b.quad(
        p(0.0, -0.1, 0.0),
        p(1.0, -0.1, 0.0),
        p(1.0, h, 0.0),
        p(0.0, h, 0.0),
        plaster,
        0.0,
    );
    let wood = col("#5a3e26");
    b.mat = WOOD;
    let beam = |b: &mut Batch, u0: f32, y0: f32, u1: f32, y1: f32, w: f32| {
        let (pa, pb) = (p(u0, y0, 0.03), p(u1, y1, 0.03));
        let d = (pb - pa).normalize_or_zero();
        let side = d.cross(nrm).normalize_or_zero() * w;
        let o = nrm * 0.03;
        b.quad(pa - side, pb - side, pb + side, pa + side, wood, 0.0);
        b.quad(
            pa - side - o,
            pb - side - o,
            pb - side,
            pa - side,
            mul(wood, 0.8),
            0.0,
        );
        b.quad(
            pa + side,
            pb + side,
            pb + side - o,
            pa + side - o,
            mul(wood, 0.8),
            0.0,
        );
    };
    beam(b, 0.04, -0.1, 0.04, h, 0.045);
    beam(b, 0.96, -0.1, 0.96, h, 0.045);
    beam(b, 0.0, h - 0.05, 1.0, h - 0.05, 0.045);
    beam(b, 0.0, 0.05, 1.0, 0.05, 0.04);
    if hashf(s) < 0.4 {
        // a window: a dark pane, warm at night, its frame and cross
        let (u0, u1, y0, y1) = (0.3, 0.7, 0.42, 0.82);
        let glass = Color::new(0.95, 0.72, 0.36, 1.0);
        b.mat = 0;
        b.quad(
            p(u0, y0, 0.012),
            p(u1, y0, 0.012),
            p(u1, y1, 0.012),
            p(u0, y1, 0.012),
            glass,
            0.45,
        );
        b.mat = WOOD;
        beam(b, u0, y0, u1, y0, 0.025);
        beam(b, u0, y1, u1, y1, 0.025);
        beam(b, u0, y0, u0, y1, 0.025);
        beam(b, u1, y0, u1, y1, 0.025);
        beam(b, 0.5, y0, 0.5, y1, 0.012);
        beam(b, u0, (y0 + y1) / 2.0, u1, (y0 + y1) / 2.0, 0.012);
    } else {
        beam(b, 0.04, 0.05, 0.96, h - 0.05, 0.035);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trees_and_walls_cast_shadows() {
        let db = content::db();
        let id = |k: &str| db.b.tiles.iter().position(|t| t.key == k).unwrap() as u8;
        let mut l = Level::new("t", "t", 9, 9, 0);
        l.lit = true;
        for t in l.tiles.iter_mut() {
            *t = id("grass");
        }
        let w = l.w;
        l.tiles[(4 * w + 4) as usize] = id("tree");
        let s = sun_hd();
        let away = -vec2(s.x, s.z).normalize();
        let lit = shade_at(&l, 4.5 - away.x * 1.5, 4.5 - away.y * 1.5);
        let dark = shade_at(&l, 4.5 + away.x * 0.9, 4.5 + away.y * 0.9);
        assert!(lit < 0.05, "toward the sun: {lit}");
        assert!(dark > 0.5, "behind the tree: {dark}");
        l.lit = false;
        assert_eq!(shade_at(&l, 4.5 + away.x * 0.9, 4.5 + away.y * 0.9), 0.0);
    }
}
