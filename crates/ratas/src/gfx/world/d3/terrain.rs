//! The land in low-poly: the ground of a level cut into chunks of faceted
//! tiles, with walls, rocks, trees and buildings standing on it. A chunk is
//! built once and rebuilt when its tiles change or more of it is explored;
//! tiles nobody has seen are not built at all. The detailed land is built
//! by scenery.rs.

use std::collections::HashMap;

use macroquad::prelude::*;
use ratas_core::content::{self, TileDef};
use ratas_core::world::{Bitset, Level};

use super::gpu::GpuMesh;
use super::lowpoly::{hash2, hashf, Batch, Xf};
use super::roofs::{self, House};
use crate::gfx::col;
use crate::gfx::world::WorldRenderer;

/// The side of a chunk in tiles.
pub const CHUNK: i32 = 16;
/// Chunks kept built (the farthest are dropped): low-poly, detailed.
const KEEP: [usize; 2] = [160, 56];

struct Chunk {
    /// the ground itself, drawn before the marks lying on it
    ground: Option<GpuMesh>,
    /// what stands on it
    props: Option<GpuMesh>,
    ver: u64,
    seen: u32,
    used: u64,
}

#[derive(Default)]
pub struct Land {
    id: String,
    hd: bool,
    fine: bool,
    chunks: HashMap<(i32, i32), Chunk>,
    batch: Batch,
    props: Batch,
    frame: u64,
    /// the chunks drawn this frame
    shown: Vec<(i32, i32)>,
    /// the roofs of the houses seen so far
    roofs: HashMap<(i32, i32, i32, i32), Option<GpuMesh>>,
    /// the tiles in sight
    view: (i32, i32, i32, i32),
}

/// How a ground tile lies: its height and how rough it is.
pub(super) fn ground_form(key: &str) -> (f32, f32) {
    match key {
        "water" => (-0.2, 0.0),
        "deep_water" => (-0.36, 0.0),
        "lava" => (-0.14, 0.0),
        "magma_crack" => (-0.02, 0.05),
        "ice" | "ice_floor" => (-0.02, 0.0),
        "hill" => (0.22, 0.22),
        "dunes" => (0.12, 0.16),
        "snowdrift" => (0.1, 0.12),
        "swamp" | "reeds" => (-0.06, 0.04),
        "road" | "bridge" | "house_floor" | "stone_floor" | "cave_floor" | "door_open"
        | "stairs_down" | "stairs_up" | "dungeon" | "sandstone_floor" | "basalt_floor"
        | "dark_floor" | "carpet" | "cobblestone" | "city_gate" => (0.0, 0.0),
        "void" => (-0.6, 0.0),
        _ => (0.0, 0.07),
    }
}

/// Walls that stand as blocks and join their neighbours.
pub(super) fn wall_height(key: &str) -> Option<f32> {
    Some(match key {
        "stone_wall" | "dark_wall" | "basalt_wall" | "sandstone_wall" => 1.35,
        "cave_wall" => 1.5,
        "ice_wall" => 1.4,
        "house_wall" => 1.15,
        "ruin_wall" => 0.8,
        "city_wall" => 1.7,
        _ => return None,
    })
}

/// Rocky tiles that rise as peaks.
pub(super) fn peak(key: &str) -> bool {
    matches!(
        key,
        "mountain" | "snow" | "sandstone" | "basalt" | "ice_rock"
    )
}

impl Land {
    /// The height of the ground at a corner of tiles.
    fn corner(l: &Level, cx: i32, cy: i32) -> f32 {
        let db = content::db();
        let (mut h, mut rough, mut n) = (0.0, 0.0, 0.0);
        let mut water = false;
        for (dx, dy) in [(-1, -1), (0, -1), (-1, 0), (0, 0)] {
            let (x, y) = (cx + dx, cy + dy);
            if !l.inside(x, y) {
                continue;
            }
            let def = db.tile(l.at(x, y));
            let (hh, rr) = ground_form(&def.key);
            water |= hh < -0.15;
            h += hh;
            rough += rr;
            n += 1.0;
        }
        if n == 0.0 {
            return 0.0;
        }
        let (h, rough) = (h / n, rough / n);
        // the shore of a lake slopes in, so the water stays below the land
        let h = if water { h.min(-0.05) } else { h };
        h + (hashf(hash2(cx, cy)) - 0.5) * rough
    }

    /// The height of the ground under a point (tiles), where things stand
    /// (on water: its surface).
    pub fn height(l: &Level, x: f32, y: f32, hd: bool) -> f32 {
        if hd {
            return super::scenery::height(l, x, y).max(super::scenery::WATER_Y);
        }
        let (tx, ty) = (x.floor() as i32, y.floor() as i32);
        let (fx, fy) = (x - tx as f32, y - ty as f32);
        let h00 = Self::corner(l, tx, ty);
        let h10 = Self::corner(l, tx + 1, ty);
        let h01 = Self::corner(l, tx, ty + 1);
        let h11 = Self::corner(l, tx + 1, ty + 1);
        let top = h00 + (h10 - h00) * fx;
        let bot = h01 + (h11 - h01) * fx;
        (top + (bot - top) * fy).max(-0.1)
    }

    /// Builds the chunks that cover tiles x0..=x1, y0..=y1 and changed
    /// since (`grounds` and `standing` give them to draw).
    pub fn prepare(
        &mut self,
        l: &Level,
        ver: u64,
        explored: &Bitset,
        (x0, y0, x1, y1): (i32, i32, i32, i32),
        hd: bool,
        fine: bool,
    ) {
        if self.id != l.id || self.hd != hd || self.fine != fine {
            self.id = l.id.clone();
            self.hd = hd;
            self.fine = fine;
            self.chunks.clear();
            self.roofs.clear();
        }
        self.view = (x0, y0, x1, y1);
        self.frame += 1;
        self.shown.clear();
        let (c0x, c0y) = (x0.max(0).div_euclid(CHUNK), y0.max(0).div_euclid(CHUNK));
        let (c1x, c1y) = (
            x1.min(l.w - 1).div_euclid(CHUNK),
            y1.min(l.h - 1).div_euclid(CHUNK),
        );
        for cy in c0y..=c1y {
            for cx in c0x..=c1x {
                let seen = Self::seen(l, explored, cx, cy);
                if seen == 0 {
                    continue;
                }
                let fresh = self
                    .chunks
                    .get(&(cx, cy))
                    .is_some_and(|c| c.ver == ver && c.seen == seen);
                if !fresh {
                    self.batch.clear();
                    self.props.clear();
                    self.batch.hd = hd;
                    self.props.hd = hd;
                    self.batch.fine = fine;
                    self.props.fine = fine;
                    build_chunk(&mut self.batch, &mut self.props, l, explored, cx, cy);
                    if hd && l.lit {
                        if self.roofs.len() > 600 {
                            self.roofs.clear();
                        }
                        let mut rb = Batch {
                            hd,
                            fine,
                            ..Default::default()
                        };
                        for h in roofs::houses(l, explored, cx, cy) {
                            self.roofs.entry((h.x, h.y, h.w, h.h)).or_insert_with(|| {
                                rb.clear();
                                roofs::roof(&mut rb, h);
                                GpuMesh::new(&rb)
                            });
                        }
                    }
                    self.chunks.insert(
                        (cx, cy),
                        Chunk {
                            ground: GpuMesh::new(&self.batch),
                            props: GpuMesh::new(&self.props),
                            ver,
                            seen,
                            used: 0,
                        },
                    );
                }
                let c = self.chunks.get_mut(&(cx, cy)).unwrap();
                c.used = self.frame;
                self.shown.push((cx, cy));
            }
        }
        let keep = KEEP[hd as usize];
        if self.chunks.len() > keep {
            let mut ages: Vec<((i32, i32), u64)> =
                self.chunks.iter().map(|(k, c)| (*k, c.used)).collect();
            ages.sort_by_key(|a| a.1);
            for (k, _) in ages.iter().take(self.chunks.len() - keep) {
                self.chunks.remove(k);
            }
        }
    }

    /// The ground of the chunks in sight.
    pub fn grounds(&self) -> Vec<&GpuMesh> {
        self.shown
            .iter()
            .filter_map(|k| self.chunks.get(k).and_then(|c| c.ground.as_ref()))
            .collect()
    }

    /// What stands on the ground of the chunks in sight.
    pub fn standing(&self) -> Vec<&GpuMesh> {
        self.shown
            .iter()
            .filter_map(|k| self.chunks.get(k).and_then(|c| c.props.as_ref()))
            .collect()
    }

    /// The roofs in sight but the one over the hero's head, and the houses
    /// they cover.
    pub fn roofs(&self, hero: (f32, f32)) -> (Vec<&GpuMesh>, Vec<House>) {
        let (mut meshes, mut covered) = (vec![], vec![]);
        let (x0, y0, x1, y1) = self.view;
        for (&(x, y, w, h), m) in &self.roofs {
            let house = House { x, y, w, h };
            let in_sight = x + w + 1 >= x0 && y + h + 1 >= y0 && x - 1 <= x1 && y - 1 <= y1;
            if !in_sight || house.holds(hero.0, hero.1) {
                continue;
            }
            covered.push(house);
            if let Some(m) = m {
                meshes.push(m);
            }
        }
        (meshes, covered)
    }

    /// How many tiles of a chunk are explored.
    fn seen(l: &Level, explored: &Bitset, cx: i32, cy: i32) -> u32 {
        let mut n = 0;
        for y in cy * CHUNK..((cy + 1) * CHUNK).min(l.h) {
            for x in cx * CHUNK..((cx + 1) * CHUNK).min(l.w) {
                if explored.get((y * l.w + x) as usize) {
                    n += 1;
                }
            }
        }
        n
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
    if gb.hd {
        super::scenery::build_chunk(gb, b, l, explored, cx, cy);
        return;
    }
    let db = content::db();
    // the corners of the chunk, shared by its tiles
    let side = CHUNK + 1;
    let mut corners = vec![0.0f32; (side * side) as usize];
    for j in 0..side {
        for i in 0..side {
            corners[(j * side + i) as usize] = Land::corner(l, cx * CHUNK + i, cy * CHUNK + j);
        }
    }
    let ch = |i: i32, j: i32| corners[(j * side + i) as usize];
    for j in 0..CHUNK {
        for i in 0..CHUNK {
            let (x, y) = (cx * CHUNK + i, cy * CHUNK + j);
            if !l.inside(x, y) || !explored.get((y * l.w + x) as usize) {
                continue;
            }
            let def = db.tile(l.at(x, y));
            // what can be walked on (and deep water) is the ground itself,
            // unless it is a small thing lying on another ground
            let flat = def.walkable || matches!(def.key.as_str(), "deep_water" | "void");
            let ground = if flat && !decor(&def.key) {
                def
            } else {
                WorldRenderer::ground_under(l, x, y, def).unwrap_or(def)
            };
            let hs = [ch(i, j), ch(i + 1, j), ch(i + 1, j + 1), ch(i, j + 1)];
            if ground.key != "void" {
                ground_tile(gb, ground, x, y, hs);
            }
            let h0 = (hs[0] + hs[1] + hs[2] + hs[3]) / 4.0;
            if flat {
                ground_detail(b, def, x, y, h0);
            } else {
                prop(b, l, def, x, y, h0.max(0.0));
            }
        }
    }
}

/// Small things lying on another ground.
pub(super) fn decor(key: &str) -> bool {
    matches!(
        key,
        "flowers" | "mushrooms" | "web" | "bones" | "rubble" | "garden"
    )
}

/// The colour of a ground tile, a little different from tile to tile.
fn ground_color(def: &TileDef, x: i32, y: i32, k: u32) -> Color {
    let j = 0.965 + 0.06 * hashf(hash2(x, y).wrapping_add(k));
    mul(base_color(def), j)
}

/// The colour of a ground tile.
pub(super) fn base_color(def: &TileDef) -> Color {
    let c = col(&def.fg);
    match def.key.as_str() {
        // darker water in the deep, the road a bit darker than its glyph
        "deep_water" => mul(c, 0.7),
        "road" | "cobblestone" => mul(c, 0.85),
        // the steps of stairs are drawn apart; their tile is a stone floor
        "stairs_down" | "stairs_up" | "dungeon" => {
            mix(col(&def.bg), Color::new(0.4, 0.4, 0.44, 1.0), 0.6)
        }
        "void" => BLACK,
        _ => c,
    }
}

pub(super) fn mul(c: Color, k: f32) -> Color {
    Color::new(
        (c.r * k).min(1.0),
        (c.g * k).min(1.0),
        (c.b * k).min(1.0),
        c.a,
    )
}

pub(super) fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

/// A tile of ground: two faceted triangles over its four corners.
fn ground_tile(b: &mut Batch, def: &TileDef, x: i32, y: i32, hs: [f32; 4]) {
    let (fx, fy) = (x as f32, y as f32);
    let p = [
        vec3(fx, hs[0], fy),
        vec3(fx + 1.0, hs[1], fy),
        vec3(fx + 1.0, hs[2], fy + 1.0),
        vec3(fx, hs[3], fy + 1.0),
    ];
    let glow = match def.key.as_str() {
        "lava" => 1.0,
        "magma_crack" => 0.5,
        _ => 0.0,
    };
    let (c1, c2) = (ground_color(def, x, y, 1), ground_color(def, x, y, 2));
    // the diagonal alternates, so the facets do not line up in stripes
    if (x + y) % 2 == 0 {
        b.tri(p[0], p[3], p[2], c1, glow);
        b.tri(p[0], p[2], p[1], c2, glow);
    } else {
        b.tri(p[0], p[3], p[1], c1, glow);
        b.tri(p[1], p[3], p[2], c2, glow);
    }
}

/// Small things on plain ground tiles: tufts, flowers, stones.
pub(super) fn ground_detail(b: &mut Batch, def: &TileDef, x: i32, y: i32, h0: f32) {
    let s = hash2(x, y);
    let at = |k: u32| {
        vec3(
            x as f32 + 0.15 + 0.7 * hashf(s.wrapping_add(k)),
            h0,
            y as f32 + 0.15 + 0.7 * hashf(s.wrapping_add(k + 101)),
        )
    };
    let c = col(&def.fg);
    match def.key.as_str() {
        "tall_grass" | "reeds" | "garden" => {
            let n = if def.key == "garden" { 3 } else { 4 };
            for k in 0..n {
                tuft(
                    b,
                    at(k * 7),
                    mul(c, 1.05),
                    0.32 + 0.12 * hashf(s + k),
                    s + k,
                );
            }
            if def.key == "garden" {
                for k in 0..4 {
                    let p = at(40 + k * 3) + vec3(0.0, 0.12, 0.0);
                    let fc = [
                        col("#ff6a8a"),
                        col("#ffd040"),
                        col("#ffffff"),
                        col("#b07aff"),
                    ][(k as usize + s as usize) % 4];
                    b.gem(Xf::at(p), Vec3::ZERO, vec3(0.05, 0.04, 0.05), 0, fc, 0.0);
                }
            }
        }
        "flowers" => {
            for k in 0..5 {
                let p = at(k * 5);
                tuft(b, p, col("#4c8a35"), 0.16, s + k);
                let fc = mul(c, 0.85 + 0.3 * hashf(s + k * 13));
                b.gem(
                    Xf::at(p + vec3(0.0, 0.17, 0.0)),
                    Vec3::ZERO,
                    vec3(0.06, 0.04, 0.06),
                    0,
                    fc,
                    0.0,
                );
            }
        }
        "grass" | "grass2" | "forest_floor" | "blight_grass" | "swamp" => {
            if hashf(s) < 0.35 {
                tuft(b, at(3), mul(c, 1.1), 0.18, s);
            }
        }
        "rubble" => {
            for k in 0..3 {
                b.gem(
                    Xf::at(at(k * 9)),
                    vec3(0.0, 0.04, 0.0),
                    vec3(0.1, 0.07, 0.09),
                    s + k,
                    c,
                    0.0,
                );
            }
        }
        "bones" => {
            for k in 0..3 {
                let p = at(k * 9) + vec3(0.0, 0.03, 0.0);
                let a = hashf(s + k * 3) * 6.3;
                let d = vec3(a.cos(), 0.0, a.sin()) * 0.14;
                b.rod(p - d, p + d, 0.025, 4, 1.0, c, 0.0);
            }
        }
        "mushrooms" => {
            for k in 0..4 {
                let p = at(k * 6);
                let h = 0.1 + 0.08 * hashf(s + k);
                b.prism(Xf::at(p), Vec3::ZERO, 0.025, h, 4, 1.0, col("#e8e0d0"), 0.4);
                b.frustum(
                    Xf::at(p),
                    vec3(0.0, h, 0.0),
                    vec2(0.07, 0.07),
                    vec2(0.02, 0.02),
                    0.05,
                    c,
                    0.85,
                );
            }
        }
        "web" => {
            let o = vec3(x as f32 + 0.5, h0 + 0.03, y as f32 + 0.5);
            let w = Color::new(0.9, 0.9, 0.95, 0.8);
            for k in 0..4 {
                let a = k as f32 * 0.785 + hashf(s) * 0.3;
                let d = vec3(a.cos(), 0.0, a.sin()) * 0.48;
                b.rod(o - d, o + d, 0.012, 3, 1.0, w, 0.2);
            }
        }
        "stairs_down" | "dungeon" => {
            // a dark way down with a few steps
            let o = vec3(x as f32 + 0.5, h0, y as f32 + 0.5);
            b.cuboid(
                Xf::at(o),
                vec3(0.0, 0.005, 0.0),
                vec3(0.42, 0.005, 0.42),
                Color::new(0.03, 0.03, 0.05, 1.0),
                0.0,
            );
            for k in 0..3 {
                let z = -0.3 + k as f32 * 0.2;
                b.cuboid(
                    Xf::at(o),
                    vec3(0.0, -0.02 - k as f32 * 0.06, z),
                    vec3(0.34, 0.03, 0.09),
                    mul(c, 0.35 - k as f32 * 0.08),
                    0.0,
                );
            }
            if def.key == "dungeon" {
                arch(b, o, Color::new(0.5, 0.45, 0.42, 1.0), 0.9);
            }
        }
        "stairs_up" => {
            // steps rising along the north side of the tile
            let o = vec3(x as f32 + 0.5, h0, y as f32 + 0.5);
            for k in 0..4 {
                b.cuboid(
                    Xf::at(o),
                    vec3(0.0, 0.05 + k as f32 * 0.1, -0.12 - k as f32 * 0.1),
                    vec3(0.36, 0.05 + k as f32 * 0.1, 0.05),
                    mul(Color::new(0.62, 0.62, 0.68, 1.0), 1.0 - k as f32 * 0.05),
                    0.0,
                );
            }
        }
        "bridge" => {
            let o = vec3(x as f32 + 0.5, 0.0, y as f32 + 0.5);
            for k in 0..4 {
                b.cuboid(
                    Xf::at(o),
                    vec3(-0.375 + k as f32 * 0.25, 0.04, 0.0),
                    vec3(0.11, 0.035, 0.5),
                    mul(c, 0.9 + 0.1 * hashf(s + k)),
                    0.0,
                );
            }
        }
        "door_open" => {
            frame_door(
                b,
                o_center(x, y, h0),
                Color::new(0.45, 0.32, 0.2, 1.0),
                None,
            );
        }
        "city_gate" => {
            frame_door(b, o_center(x, y, h0), col("#9a948a"), None);
        }
        _ => {}
    }
}

pub(super) fn o_center(x: i32, y: i32, h: f32) -> Vec3 {
    vec3(x as f32 + 0.5, h, y as f32 + 0.5)
}

/// A few blades of grass.
fn tuft(b: &mut Batch, p: Vec3, c: Color, h: f32, seed: u32) {
    for k in 0..3u32 {
        let a = hashf(seed.wrapping_mul(7).wrapping_add(k)) * 6.3;
        let tip = p + vec3(
            a.cos() * 0.06,
            h * (0.7 + 0.3 * hashf(seed + k)),
            a.sin() * 0.06,
        );
        let side = vec3(-a.sin(), 0.0, a.cos()) * 0.035;
        b.tri(p - side, p + side, tip, mul(c, 0.9 + 0.12 * k as f32), 0.0);
    }
}

/// A stone arch over a way down.
fn arch(b: &mut Batch, o: Vec3, c: Color, h: f32) {
    for s in [-1.0f32, 1.0] {
        b.cuboid(
            Xf::at(o),
            vec3(s * 0.42, h / 2.0, -0.36),
            vec3(0.08, h / 2.0, 0.08),
            c,
            0.0,
        );
    }
    b.cuboid(
        Xf::at(o),
        vec3(0.0, h + 0.06, -0.36),
        vec3(0.52, 0.07, 0.1),
        mul(c, 1.1),
        0.0,
    );
}

/// A door frame along the wall it stands in; `panel` is the colour of a
/// closed door.
fn frame_door(b: &mut Batch, o: Vec3, c: Color, panel: Option<Color>) {
    let h = 1.0;
    for s in [-1.0f32, 1.0] {
        b.cuboid(
            Xf::at(o),
            vec3(s * 0.44, h / 2.0, 0.0),
            vec3(0.06, h / 2.0, 0.08),
            c,
            0.0,
        );
    }
    b.cuboid(
        Xf::at(o),
        vec3(0.0, h + 0.05, 0.0),
        vec3(0.5, 0.05, 0.09),
        c,
        0.0,
    );
    if let Some(p) = panel {
        b.cuboid(
            Xf::at(o),
            vec3(0.0, h / 2.0, 0.0),
            vec3(0.38, h / 2.0, 0.04),
            p,
            0.0,
        );
        b.cuboid(
            Xf::at(o),
            vec3(0.24, 0.5, 0.05),
            vec3(0.03, 0.03, 0.02),
            col("#f0c040"),
            0.0,
        );
    }
}

pub(super) fn is_wall_at(l: &Level, x: i32, y: i32) -> bool {
    if !l.inside(x, y) {
        return true;
    }
    let def = content::db().tile(l.at(x, y));
    wall_height(&def.key).is_some() || def.key == "door"
}

/// What a thing on a tile is made of, in the detailed looks.
pub(super) fn material_of(key: &str) -> u8 {
    use super::materials::*;
    match key {
        "door" | "door_open" | "fence" | "chest" | "chest_open" | "bridge" | "market_stall"
        | "city_gate" | "campfire" => WOOD,
        "sarcophagus" | "sarcophagus_open" | "altar" | "shrine" | "shrine_used" | "pillar"
        | "gravestone" | "menhir" | "statue" | "fountain" | "well" | "stairs_down"
        | "stairs_up" | "dungeon" | "rubble" => STONE,
        "brazier" | "lamp_post" => METAL,
        "bones" => BONE,
        "tent" => CLOTH,
        "tall_grass" | "reeds" | "garden" | "flowers" => LEAVES,
        _ => 0,
    }
}

/// What stands on a tile that is not plain ground.
pub(super) fn prop(b: &mut Batch, l: &Level, def: &TileDef, x: i32, y: i32, h0: f32) {
    let s = hash2(x, y);
    let c = col(&def.fg);
    let o = o_center(x, y, h0);
    let key = def.key.as_str();
    if let Some(h) = wall_height(key) {
        wall(b, l, def, x, y, h);
        return;
    }
    if peak(key) {
        let top = match key {
            "snow" => 1.0,
            "mountain" => 0.0,
            _ => -1.0,
        };
        rock_peak(b, o, c, s, top, key == "ice_rock");
        return;
    }
    match key {
        "tree" => {
            let k = 1.15 + 0.35 * hashf(s);
            trunk(b, o, 0.5 * k, s);
            b.ball(
                Xf::at(o),
                vec3(0.0, 0.85 * k, 0.0),
                vec3(0.42, 0.36, 0.42) * k,
                s,
                c,
                0.0,
            );
            b.ball(
                Xf::at(o),
                vec3(0.12, 1.12 * k, -0.06),
                vec3(0.26, 0.24, 0.26) * k,
                s + 9,
                mul(c, 1.12),
                0.0,
            );
        }
        "pine" | "snow_pine" => {
            let k = 1.1 + 0.4 * hashf(s);
            trunk(b, o, 0.3 * k, s);
            for i in 0..3 {
                let y = 0.25 + i as f32 * 0.32 * k;
                let r = (0.46 - i as f32 * 0.11) * k;
                let cc = if key == "snow_pine" && i == 2 {
                    col("#f0f6ff")
                } else {
                    mul(c, 1.0 + i as f32 * 0.06)
                };
                b.cone(Xf::at(o), vec3(0.0, y, 0.0), r, 0.5 * k, 7, cc, 0.0);
                if key == "snow_pine" && i < 2 {
                    b.cone(
                        Xf::at(o),
                        vec3(0.0, y + 0.3 * k, 0.0),
                        r * 0.45,
                        0.2 * k,
                        7,
                        col("#f0f6ff"),
                        0.0,
                    );
                }
            }
        }
        "palm" => {
            let lean = vec3(0.12, 0.0, 0.06);
            let mut p = o;
            for i in 0..4 {
                let q = o
                    + vec3(0.0, 0.32 * (i + 1) as f32, 0.0)
                    + lean * ((i + 1) as f32 / 4.0).powi(2);
                b.rod(p, q, 0.055 - i as f32 * 0.006, 5, 0.9, col("#9a7a4a"), 0.0);
                p = q;
            }
            for k in 0..6 {
                let a = k as f32 / 6.0 * std::f32::consts::TAU + hashf(s);
                let d = vec3(a.cos(), 0.0, a.sin());
                let tip = p + d * 0.55 + vec3(0.0, -0.22, 0.0);
                let mid = p + d * 0.3 + vec3(0.0, 0.06, 0.0);
                let side = vec3(-d.z, 0.0, d.x) * 0.1;
                b.tri(p, mid + side, mid - side, c, 0.0);
                b.tri(mid - side, mid + side, tip, mul(c, 0.9), 0.0);
            }
        }
        "dead_tree" | "charred_tree" | "twisted_tree" => {
            let top = o + vec3(0.04, 0.9, -0.03);
            b.rod(o, top, 0.07, 5, 0.5, c, 0.0);
            for k in 0..4 {
                let a = k as f32 * 1.6 + hashf(s + k) * 1.2;
                let from = o + vec3(0.0, 0.4 + 0.12 * k as f32, 0.0);
                let to = from
                    + vec3(
                        a.cos() * 0.35,
                        0.25 + 0.15 * hashf(s + k * 3),
                        a.sin() * 0.35,
                    );
                b.rod(from, to, 0.03, 4, 0.4, mul(c, 0.9), 0.0);
                if key == "twisted_tree" {
                    b.ball(
                        Xf::at(to),
                        Vec3::ZERO,
                        vec3(0.12, 0.1, 0.12),
                        s + k,
                        mul(c, 1.3),
                        0.0,
                    );
                }
            }
        }
        "bush" => {
            b.ball(
                Xf::at(o),
                vec3(0.0, 0.18, 0.0),
                vec3(0.32, 0.22, 0.32),
                s,
                c,
                0.0,
            );
            b.ball(
                Xf::at(o),
                vec3(0.12, 0.3, 0.05),
                vec3(0.16, 0.12, 0.16),
                s + 3,
                mul(c, 1.15),
                0.0,
            );
        }
        "cactus" => {
            b.prism(Xf::at(o), Vec3::ZERO, 0.11, 0.8, 6, 0.85, c, 0.0);
            for sgn in [-1.0f32, 1.0] {
                let base = o + vec3(sgn * 0.1, 0.35 + 0.1 * sgn.max(0.0), 0.0);
                let elbow = base + vec3(sgn * 0.16, 0.0, 0.0);
                b.rod(base, elbow, 0.05, 5, 1.0, c, 0.0);
                b.rod(elbow, elbow + vec3(0.0, 0.25, 0.0), 0.05, 5, 0.8, c, 0.0);
            }
        }
        "door" => {
            let along_x = is_wall_at(l, x - 1, y) || is_wall_at(l, x + 1, y);
            let xf = if along_x {
                Xf::at(o)
            } else {
                Xf::yaw(o, std::f32::consts::FRAC_PI_2)
            };
            door_in(b, xf, Color::new(0.42, 0.3, 0.2, 1.0), Some(c));
        }
        "well" => {
            b.prism(
                Xf::at(o),
                Vec3::ZERO,
                0.36,
                0.32,
                8,
                1.0,
                col("#8a8a96"),
                0.0,
            );
            b.disc(o + vec3(0.0, 0.33, 0.0), 0.28, 0.28, 8, col("#1a3050"), 0.0);
            for sgn in [-1.0f32, 1.0] {
                b.cuboid(
                    Xf::at(o),
                    vec3(sgn * 0.3, 0.55, 0.0),
                    vec3(0.03, 0.25, 0.03),
                    col("#7a5a3a"),
                    0.0,
                );
            }
            roof(
                b,
                Xf::at(o + vec3(0.0, 0.8, 0.0)),
                0.42,
                0.3,
                0.2,
                col("#9a5a3a"),
            );
        }
        "fence" => {
            let wood = c;
            b.cuboid(
                Xf::at(o),
                vec3(0.0, 0.3, 0.0),
                vec3(0.05, 0.3, 0.05),
                wood,
                0.0,
            );
            for (dx, dy) in [(1, 0), (0, 1)] {
                if l.inside(x + dx, y + dy)
                    && content::db().tile(l.at(x + dx, y + dy)).key == "fence"
                {
                    let to = o + vec3(dx as f32, 0.0, dy as f32);
                    for hy in [0.22f32, 0.44] {
                        b.rod(
                            o + vec3(0.0, hy, 0.0),
                            to + vec3(0.0, hy, 0.0),
                            0.025,
                            4,
                            1.0,
                            mul(wood, 1.1),
                            0.0,
                        );
                    }
                }
            }
        }
        "chest" | "chest_open" | "sarcophagus" | "sarcophagus_open" => {
            let stone = key.starts_with("sarcophagus");
            let (half, body) = if stone {
                (vec3(0.42, 0.2, 0.24), mul(c, 0.75))
            } else {
                (vec3(0.28, 0.16, 0.2), col("#8a5a2a"))
            };
            b.cuboid(Xf::at(o), vec3(0.0, half.y, 0.0), half, body, 0.0);
            let open = key.ends_with("_open");
            let lid = Xf::at(o + vec3(0.0, half.y * 2.0, -half.z)).then(
                Vec3::ZERO,
                Mat3::from_rotation_x(if open { -1.2 } else { 0.0 }),
            );
            b.cuboid(
                lid,
                vec3(0.0, 0.05, half.z),
                vec3(half.x, 0.05, half.z),
                mul(body, 1.1),
                0.0,
            );
            for sgn in [-1.0f32, 1.0] {
                b.cuboid(
                    Xf::at(o),
                    vec3(sgn * half.x * 0.7, half.y, 0.0),
                    vec3(0.03, half.y + 0.01, half.z + 0.01),
                    col("#f0c040"),
                    0.0,
                );
            }
            if open && !stone {
                b.disc(
                    o + vec3(0.0, half.y * 2.0 - 0.02, 0.0),
                    0.2,
                    0.14,
                    6,
                    col("#ffd24a"),
                    0.6,
                );
            }
        }
        "altar" | "shrine" | "shrine_used" => {
            let stone = col("#8a8090");
            b.cuboid(
                Xf::at(o),
                vec3(0.0, 0.18, 0.0),
                vec3(0.4, 0.18, 0.3),
                stone,
                0.0,
            );
            b.cuboid(
                Xf::at(o),
                vec3(0.0, 0.4, 0.0),
                vec3(0.45, 0.04, 0.34),
                mul(stone, 1.15),
                0.0,
            );
            let glow = if key == "shrine_used" { 0.0 } else { 1.0 };
            b.gem(
                Xf::at(o),
                vec3(0.0, 0.62, 0.0),
                vec3(0.14, 0.18, 0.14),
                0,
                c,
                glow,
            );
        }
        "pillar" => {
            let stone = c;
            b.prism(
                Xf::at(o),
                Vec3::ZERO,
                0.24,
                0.12,
                8,
                1.0,
                mul(stone, 0.9),
                0.0,
            );
            b.prism(
                Xf::at(o),
                vec3(0.0, 0.12, 0.0),
                0.18,
                1.3,
                8,
                1.0,
                stone,
                0.0,
            );
            b.prism(
                Xf::at(o),
                vec3(0.0, 1.42, 0.0),
                0.25,
                0.12,
                8,
                1.0,
                mul(stone, 1.1),
                0.0,
            );
        }
        "gravestone" => {
            let xf = Xf::yaw(o, (hashf(s) - 0.5) * 0.3);
            b.cuboid(xf, vec3(0.0, 0.25, 0.0), vec3(0.06, 0.25, 0.2), c, 0.0);
            b.prism(
                xf.then(
                    vec3(0.0, 0.5, 0.0),
                    Mat3::from_rotation_z(std::f32::consts::FRAC_PI_2),
                ),
                vec3(0.0, -0.06, 0.0),
                0.2,
                0.12,
                6,
                1.0,
                c,
                0.0,
            );
            b.cuboid(
                Xf::at(o),
                vec3(0.15, 0.02, 0.0),
                vec3(0.2, 0.02, 0.24),
                col("#4a6a3a"),
                0.0,
            );
        }
        "menhir" => {
            let xf = Xf::yaw(o, hashf(s) * 3.0).then(Vec3::ZERO, Mat3::from_rotation_z(0.08));
            b.frustum(
                xf,
                Vec3::ZERO,
                vec2(0.18, 0.13),
                vec2(0.1, 0.08),
                1.4,
                c,
                0.0,
            );
        }
        "tent" => {
            let xf = Xf::yaw(o, hashf(s) * 6.3);
            b.cone(xf, Vec3::ZERO, 0.5, 0.95, 4, c, 0.0);
            b.cuboid(
                xf,
                vec3(0.45, 0.2, 0.0),
                vec3(0.04, 0.2, 0.12),
                col("#2a1a10"),
                0.0,
            );
        }
        "campfire" => {
            fire(b, o, 0.0);
        }
        "brazier" => {
            for k in 0..3 {
                let a = k as f32 * 2.094;
                b.rod(
                    o + vec3(a.cos() * 0.22, 0.0, a.sin() * 0.22),
                    o + vec3(a.cos() * 0.12, 0.45, a.sin() * 0.12),
                    0.025,
                    4,
                    1.0,
                    col("#3a3030"),
                    0.0,
                );
            }
            b.frustum(
                Xf::at(o),
                vec3(0.0, 0.42, 0.0),
                vec2(0.12, 0.12),
                vec2(0.24, 0.24),
                0.14,
                col("#5a4a40"),
                0.0,
            );
            fire(b, o + vec3(0.0, 0.5, 0.0), 0.6);
        }
        "crystal" => {
            for k in 0..3 {
                let a = k as f32 * 2.1 + hashf(s);
                let base = o + vec3(a.cos() * 0.14, 0.0, a.sin() * 0.14);
                let tip = base + vec3(a.cos() * 0.15, 0.55 + 0.25 * hashf(s + k), a.sin() * 0.15);
                b.spike(base, tip, 0.1, 5, mul(c, 0.9 + 0.1 * k as f32), 0.85);
            }
        }
        "statue" => {
            let stone = c;
            b.cuboid(
                Xf::at(o),
                vec3(0.0, 0.15, 0.0),
                vec3(0.32, 0.15, 0.32),
                mul(stone, 0.8),
                0.0,
            );
            let xf = Xf::yaw(o + vec3(0.0, 0.3, 0.0), hashf(s) * 6.3);
            b.cuboid(xf, vec3(0.0, 0.25, 0.0), vec3(0.1, 0.25, 0.08), stone, 0.0);
            b.frustum(
                xf,
                vec3(0.0, 0.5, 0.0),
                vec2(0.1, 0.07),
                vec2(0.17, 0.08),
                0.35,
                stone,
                0.0,
            );
            b.cuboid(xf, vec3(0.0, 0.95, 0.0), vec3(0.09, 0.1, 0.09), stone, 0.0);
            b.rod(
                xf.p(vec3(0.0, 0.8, 0.18)),
                xf.p(vec3(0.0, 1.25, 0.25)),
                0.03,
                4,
                1.0,
                mul(stone, 1.1),
                0.0,
            );
        }
        "fountain" => {
            let stone = col("#9a9aa8");
            b.prism(Xf::at(o), Vec3::ZERO, 0.46, 0.22, 8, 1.0, stone, 0.0);
            b.disc(o + vec3(0.0, 0.23, 0.0), 0.4, 0.4, 8, c, 0.2);
            b.prism(
                Xf::at(o),
                vec3(0.0, 0.22, 0.0),
                0.07,
                0.45,
                6,
                1.0,
                stone,
                0.0,
            );
            b.prism(
                Xf::at(o),
                vec3(0.0, 0.67, 0.0),
                0.18,
                0.06,
                8,
                1.0,
                stone,
                0.0,
            );
            b.cone(
                Xf::at(o),
                vec3(0.0, 0.73, 0.0),
                0.06,
                0.2,
                5,
                mix(c, WHITE, 0.4),
                0.6,
            );
        }
        "market_stall" => {
            let wood = col("#7a5a3a");
            b.cuboid(
                Xf::at(o),
                vec3(0.0, 0.3, 0.0),
                vec3(0.42, 0.04, 0.28),
                wood,
                0.0,
            );
            for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                b.cuboid(
                    Xf::at(o),
                    vec3(sx * 0.38, 0.45, sz * 0.24),
                    vec3(0.025, 0.45, 0.025),
                    wood,
                    0.0,
                );
            }
            for k in 0..4 {
                let cc = if k % 2 == 0 { c } else { col("#f4ecd8") };
                b.cuboid(
                    Xf::at(o),
                    vec3(-0.33 + k as f32 * 0.22, 0.92, 0.0),
                    vec3(0.11, 0.02, 0.34),
                    cc,
                    0.0,
                );
            }
            for k in 0..3 {
                b.gem(
                    Xf::at(o),
                    vec3(-0.2 + k as f32 * 0.2, 0.39, 0.0),
                    vec3(0.07, 0.06, 0.07),
                    s + k,
                    [col("#e05040"), col("#f0c040"), col("#80c050")][k as usize],
                    0.0,
                );
            }
        }
        "lamp_post" => {
            b.prism(
                Xf::at(o),
                Vec3::ZERO,
                0.04,
                1.2,
                5,
                1.0,
                col("#3a3a40"),
                0.0,
            );
            b.cuboid(
                Xf::at(o),
                vec3(0.0, 1.28, 0.0),
                vec3(0.09, 0.09, 0.09),
                c,
                1.0,
            );
            b.cone(
                Xf::at(o),
                vec3(0.0, 1.37, 0.0),
                0.13,
                0.1,
                4,
                col("#3a3a40"),
                0.0,
            );
        }
        _ => {
            // something modded: a block in its colour
            b.cuboid(Xf::at(o), vec3(0.0, 0.4, 0.0), vec3(0.4, 0.4, 0.4), c, 0.0);
        }
    }
}

fn trunk(b: &mut Batch, o: Vec3, h: f32, s: u32) {
    let lean = vec3(
        (hashf(s + 5) - 0.5) * 0.08,
        0.0,
        (hashf(s + 6) - 0.5) * 0.08,
    );
    b.rod(
        o,
        o + vec3(0.0, h, 0.0) + lean,
        0.075,
        5,
        0.75,
        col("#6a4a2a"),
        0.0,
    );
}

/// A gabled roof over a frame: half width (x), half depth (z), height.
fn roof(b: &mut Batch, xf: Xf, w: f32, d: f32, h: f32, c: Color) {
    let p = |x: f32, y: f32, z: f32| xf.p(vec3(x, y, z));
    let (a, bb, cc, dd) = (p(-w, 0.0, -d), p(w, 0.0, -d), p(w, 0.0, d), p(-w, 0.0, d));
    let (r1, r2) = (p(-w, h, 0.0), p(w, h, 0.0));
    b.quad(a, bb, r2, r1, c, 0.0);
    b.quad(cc, dd, r1, r2, mul(c, 0.9), 0.0);
    b.tri(dd, a, r1, mul(c, 0.8), 0.0);
    b.tri(bb, cc, r2, mul(c, 0.8), 0.0);
}

fn door_in(b: &mut Batch, xf: Xf, frame: Color, panel: Option<Color>) {
    let h = 1.0;
    for s in [-1.0f32, 1.0] {
        b.cuboid(
            xf,
            vec3(s * 0.44, h / 2.0, 0.0),
            vec3(0.06, h / 2.0, 0.1),
            frame,
            0.0,
        );
    }
    b.cuboid(
        xf,
        vec3(0.0, h + 0.05, 0.0),
        vec3(0.5, 0.06, 0.11),
        frame,
        0.0,
    );
    if let Some(p) = panel {
        b.cuboid(
            xf,
            vec3(0.0, h / 2.0, 0.0),
            vec3(0.38, h / 2.0, 0.05),
            p,
            0.0,
        );
        b.cuboid(
            xf,
            vec3(0.24, 0.5, 0.07),
            vec3(0.03, 0.03, 0.02),
            col("#f0c040"),
            0.0,
        );
    }
}

/// A fire: logs, flames that shine, embers.
fn fire(b: &mut Batch, o: Vec3, small: f32) {
    let k = 1.0 - small * 0.5;
    if small == 0.0 {
        for a in [0.4f32, 2.2, 4.0] {
            let d = vec3(a.cos(), 0.0, a.sin()) * 0.28;
            b.rod(
                o - d + vec3(0.0, 0.04, 0.0),
                o + d + vec3(0.0, 0.12, 0.0),
                0.05,
                5,
                1.0,
                col("#5a3a22"),
                0.0,
            );
        }
        for a in 0..6 {
            let a = a as f32 * 1.047;
            b.gem(
                Xf::at(o),
                vec3(a.cos() * 0.34, 0.04, a.sin() * 0.34),
                vec3(0.07, 0.05, 0.07),
                a as u32 + 1,
                col("#6a6a70"),
                0.0,
            );
        }
    }
    b.cone(
        Xf::at(o),
        vec3(0.0, 0.05, 0.0),
        0.2 * k,
        0.5 * k,
        5,
        col("#ff7a2a"),
        1.0,
    );
    b.cone(
        Xf::at(o),
        vec3(0.05, 0.05, 0.03),
        0.11 * k,
        0.62 * k,
        4,
        col("#ffd060"),
        1.0,
    );
}

/// A rocky peak, its snow cap: 1 all white, 0 a cap on the tall ones,
/// below 0 none.
fn rock_peak(b: &mut Batch, o: Vec3, c: Color, s: u32, snow: f32, ice: bool) {
    let h = 0.9 + 0.9 * hashf(s);
    let apex = vec3((hashf(s + 1) - 0.5) * 0.3, h, (hashf(s + 2) - 0.5) * 0.3);
    let n = 5;
    let r = 0.68;
    let a0 = hashf(s + 3) * 6.3;
    let base: Vec<Vec3> = (0..n)
        .map(|i| {
            let a = a0 + i as f32 / n as f32 * std::f32::consts::TAU;
            let rr = r * (0.85 + 0.3 * hashf(s + 10 + i as u32));
            o + vec3(a.cos() * rr, 0.0, a.sin() * rr)
        })
        .collect();
    let top = o + apex;
    let glow = if ice { 0.15 } else { 0.0 };
    for i in 0..n {
        let j = (i + 1) % n;
        let (p, q) = (base[i], base[j]);
        if snow >= 1.0 || (snow == 0.0 && h > 1.4) {
            // a snow cap: the upper part of each face
            let k = if snow >= 1.0 { 0.25 } else { 0.62 };
            let (pm, qm) = (p + (top - p) * k, q + (top - q) * k);
            if k > 0.3 {
                b.quad(q, p, pm, qm, c, glow);
            } else {
                b.quad(q, p, pm, qm, mix(c, WHITE, 0.6), glow);
            }
            b.tri(qm, pm, top, col("#f4f8ff"), glow);
        } else {
            b.tri(q, p, top, mul(c, 0.95 + 0.1 * hashf(s + i as u32)), glow);
        }
    }
    // a smaller rock leaning on it
    b.gem(
        Xf::at(o),
        vec3((hashf(s + 7) - 0.5) * 0.6, 0.1, (hashf(s + 8) - 0.5) * 0.6),
        vec3(0.22, 0.2, 0.2),
        s,
        mul(c, 0.85),
        glow,
    );
}

/// A block of wall; its sides show only toward open tiles.
fn wall(b: &mut Batch, l: &Level, def: &TileDef, x: i32, y: i32, h: f32) {
    let s = hash2(x, y);
    let c = col(&def.fg);
    let key = def.key.as_str();
    let (fx, fy) = (x as f32, y as f32);
    let rough = key == "cave_wall";
    let h = if key == "ruin_wall" {
        h * (0.5 + 0.7 * hashf(s))
    } else {
        h
    };
    let hc = |cx: i32, cy: i32| {
        if rough {
            h + (hashf(hash2(cx, cy) ^ 0x55) - 0.5) * 0.5
        } else {
            h
        }
    };
    let top = [
        vec3(fx, hc(x, y), fy),
        vec3(fx + 1.0, hc(x + 1, y), fy),
        vec3(fx + 1.0, hc(x + 1, y + 1), fy + 1.0),
        vec3(fx, hc(x, y + 1), fy + 1.0),
    ];
    let bot = [
        vec3(fx, -0.1, fy),
        vec3(fx + 1.0, -0.1, fy),
        vec3(fx + 1.0, -0.1, fy + 1.0),
        vec3(fx, -0.1, fy + 1.0),
    ];
    let top_c = mul(c, 1.12);
    b.tri(top[0], top[3], top[2], top_c, 0.0);
    b.tri(top[0], top[2], top[1], mul(top_c, 0.97), 0.0);
    // faces: north (y-1), east, south, west
    let sides = [
        ((0, -1), 1, 0),
        ((1, 0), 2, 1),
        ((0, 1), 3, 2),
        ((-1, 0), 0, 3),
    ];
    for ((dx, dy), i, j) in sides {
        let n = (x + dx, y + dy);
        let open = l.inside(n.0, n.1) && {
            let nd = content::db().tile(l.at(n.0, n.1));
            wall_height(&nd.key).is_none() || (key == "ruin_wall")
        };
        if !open {
            continue;
        }
        let shade = 0.92 + 0.12 * hashf(s.wrapping_add(i as u32));
        b.quad(bot[i], bot[j], top[j], top[i], mul(c, shade), 0.0);
        // a course of stones or logs along the face
        if !rough && key != "ice_wall" {
            let nrm = vec3(dx as f32, 0.0, dy as f32) * 0.012;
            for k in 1..3 {
                let y = h * k as f32 / 3.0;
                let a = bot[i] + vec3(0.0, y + 0.1, 0.0) + nrm;
                let bq = bot[j] + vec3(0.0, y + 0.1, 0.0) + nrm;
                let up = vec3(0.0, 0.03, 0.0);
                b.quad(a - up, bq - up, bq + up, a + up, mul(c, 0.78), 0.0);
            }
        }
    }
    if key == "city_wall" {
        for k in 0..2 {
            b.cuboid(
                Xf::at(vec3(fx + 0.25 + k as f32 * 0.5, h, fy + 0.5)),
                vec3(0.0, 0.12, 0.0),
                vec3(0.14, 0.12, 0.14),
                top_c,
                0.0,
            );
        }
    }
    if key == "house_wall" {
        // timber posts at the corners
        let wood = col("#5a3e26");
        for (cx, cz) in [(0.06f32, 0.06f32), (0.94, 0.06), (0.06, 0.94), (0.94, 0.94)] {
            b.cuboid(
                Xf::at(vec3(fx + cx, 0.0, fy + cz)),
                vec3(0.0, h / 2.0, 0.0),
                vec3(0.07, h / 2.0 + 0.01, 0.07),
                wood,
                0.0,
            );
        }
    }
}
