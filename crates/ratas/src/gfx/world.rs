//! The world view: tiles in 3/4 perspective, creatures gliding with their
//! real-time positions, light and darkness, particles, floating numbers,
//! speech bubbles and the boss bar.

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use macroquad::prelude::*;

use ratas_core::content::{self, TileDef};
use ratas_core::game::rarity_color;
use ratas_core::proto::*;
use ratas_core::rng::Rng;
use ratas_core::world::{fov, Bitset, Level, Vec2 as WV};

use super::*;
use crate::art::specs::{resolve, variants, KIND_NPC, KIND_PLAYER};
use crate::art::SPX;

pub const KIND_MONSTER: u8 = 2;
pub const KIND_ITEM: u8 = 4;
pub const KIND_PROJECTILE: u8 = 5;

/// Tile sizes in logical pixels; multiples of the 16px art keep pixels crisp.
const ZOOM: [f32; 4] = [32.0, 48.0, 64.0, 80.0];

/// What to draw this frame.
pub struct Scene<'a> {
    pub level: &'a Level,
    /// grows when tiles change
    pub level_ver: u64,
    pub explored: &'a Bitset,
    pub snap: &'a Snapshot,
    pub you: u32,
    pub paused: bool,
    /// the direction the player holds (for the own hero's prediction)
    pub input: [i8; 2],
}

#[derive(Default)]
struct EntState {
    /// drawn centre (tiles)
    x: f32,
    y: f32,
    /// the server's position and velocity at the last snapshot
    sx: f32,
    sy: f32,
    vx: f32,
    vy: f32,
    /// distance walked: drives walking frames and bobbing
    walk: f32,
    /// fades in and out of sight
    alpha: f32,
    seen: bool,
    left: bool,
    swing: u8,
    lunge: f32,
    flash: f32,
    trail: Vec<(f32, f32)>,
    facing: f32,
    hp: u8,
    /// the last view of the entity (kept while it fades out)
    view: EntityView,
    gone: bool,
}

struct FloatText {
    x: f32,
    y: f32,
    t: f32,
    life: f32,
    text: String,
    col: Color,
    big: bool,
}

#[derive(Clone)]
struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    t: f32,
    life: f32,
    size: f32,
    grav: f32,
    col: Color,
    glow: bool,
}

struct Flash {
    x: f32,
    y: f32,
    t: f32,
    life: f32,
    radius: f32,
    col: Color,
    /// a ring that grows to radius (area effects)
    ring: bool,
}

struct Beam {
    x: f32,
    y: f32,
    x2: f32,
    y2: f32,
    t: f32,
    life: f32,
    col: Color,
}

/// How the world maps to the screen.
#[derive(Clone, Copy, Debug, Default)]
pub struct View {
    pub ox: f32,
    pub oy: f32,
    pub ts: f32,
    pub tx0: i32,
    pub ty0: i32,
    pub tx1: i32,
    pub ty1: i32,
}

impl View {
    pub fn px(&self, x: f32, y: f32) -> (f32, f32) {
        (self.ox + x * self.ts, self.oy + y * self.ts)
    }
    /// The world point under a screen position.
    pub fn world(&self, sx: f32, sy: f32) -> (f32, f32) {
        ((sx - self.ox) / self.ts, (sy - self.oy) / self.ts)
    }
}

/// Swaying trees bend in the wind by this much.
fn sway(key: &str) -> f32 {
    match key {
        "tree" => 0.035,
        "pine" => 0.025,
        "snow_pine" => 0.02,
        "palm" => 0.05,
        "dead_tree" => 0.015,
        "twisted_tree" => 0.02,
        "charred_tree" => 0.01,
        _ => 0.0,
    }
}

/// Ground tiles that are details rather than the floor itself.
fn not_base(key: &str) -> bool {
    matches!(
        key,
        "road"
            | "bridge"
            | "carpet"
            | "web"
            | "rubble"
            | "bones"
            | "magma_crack"
            | "mushrooms"
            | "flowers"
            | "lava"
            | "water"
            | "deep_water"
            | "dungeon"
            | "stairs_up"
            | "stairs_down"
            | "door_open"
            | "void"
    )
}

/// The cells the hero sees: a window around them, so a big level is not
/// copied cell by cell every frame.
#[derive(Clone, Default)]
pub struct Vis {
    x0: i32,
    y0: i32,
    w: i32,
    h: i32,
    cells: Vec<bool>,
}

impl Vis {
    fn set(&mut self, x: i32, y: i32) {
        let (dx, dy) = (x - self.x0, y - self.y0);
        if dx >= 0 && dy >= 0 && dx < self.w && dy < self.h {
            self.cells[(dy * self.w + dx) as usize] = true;
        }
    }

    pub fn get(&self, x: i32, y: i32) -> bool {
        let (dx, dy) = (x - self.x0, y - self.y0);
        dx >= 0 && dy >= 0 && dx < self.w && dy < self.h && self.cells[(dy * self.w + dx) as usize]
    }

    /// The visible cells.
    pub fn cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.cells
            .iter()
            .enumerate()
            .filter(|(_, v)| **v)
            .map(|(i, _)| (self.x0 + i as i32 % self.w, self.y0 + i as i32 / self.w))
    }
}

pub struct WorldRenderer {
    zoom_idx: usize,
    zoom_cur: f32,
    ents: HashMap<u32, EntState>,
    level_id: String,
    cam: (f32, f32),
    cam_ok: bool,
    pub t: f32,
    dt: f32,
    texts: Vec<FloatText>,
    parts: Vec<Particle>,
    lights: Vec<Flash>,
    beams: Vec<Beam>,
    ambient: Vec<Particle>,
    rng: Rng,
    vis: Vis,
    vis_key: (u64, i32, i32, i32, bool, usize),
    dark_cur: Vec<f32>,
    dark_id: String,
    dark_tex: Option<Texture2D>,
    dark_buf: Vec<u8>,
    tops: HashMap<u32, f32>,
    last_tick: u64,
    snap_at: f32,
    pub view: View,
    pub atlas: super::atlas::Atlas,
}

impl Default for WorldRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldRenderer {
    pub fn new() -> WorldRenderer {
        WorldRenderer {
            zoom_idx: 1,
            zoom_cur: 0.0,
            ents: HashMap::new(),
            level_id: String::new(),
            cam: (0.0, 0.0),
            cam_ok: false,
            t: 0.0,
            dt: 0.0,
            texts: vec![],
            parts: vec![],
            lights: vec![],
            beams: vec![],
            ambient: vec![],
            rng: Rng::new(1, 2),
            vis: Vis::default(),
            vis_key: (u64::MAX, 0, 0, 0, false, 0),
            dark_cur: vec![],
            dark_id: String::new(),
            dark_tex: None,
            dark_buf: vec![],
            tops: HashMap::new(),
            last_tick: u64::MAX,
            snap_at: 0.0,
            view: View::default(),
            atlas: super::atlas::Atlas::default(),
        }
    }

    pub fn zoom(&mut self, dir: f32) {
        if dir > 0.0 && self.zoom_idx < ZOOM.len() - 1 {
            self.zoom_idx += 1;
        } else if dir < 0.0 && self.zoom_idx > 0 {
            self.zoom_idx -= 1;
        }
    }

    fn tile_size(&self, s: f32) -> f32 {
        let z = if self.zoom_cur == 0.0 {
            ZOOM[self.zoom_idx]
        } else {
            self.zoom_cur
        };
        (z * s).round().max(8.0)
    }

    /// The visible cells (field of view from the hero), cached.
    pub fn visible(&mut self, sc: &Scene) -> &Vis {
        let l = sc.level;
        let me = &sc.snap.you;
        let (hx, hy) = (me.x.floor() as i32, me.y.floor() as i32);
        let key = (
            sc.level_ver,
            hx,
            hy,
            me.vision,
            me.dead,
            l as *const _ as usize,
        );
        if self.vis_key != key {
            let r = me.vision.max(0) + 1;
            let mut vis = Vis {
                x0: hx - r,
                y0: hy - r,
                w: 2 * r + 1,
                h: 2 * r + 1,
                cells: vec![false; ((2 * r + 1) * (2 * r + 1)) as usize],
            };
            if !me.dead {
                fov(l, hx, hy, me.vision, &mut |x, y| vis.set(x, y));
            }
            self.vis = vis;
            self.vis_key = key;
        }
        &self.vis
    }

    fn rf(&mut self) -> f32 {
        self.rng.f32()
    }

    // ---- simulation of visuals ----

    pub fn update(&mut self, dt: f32, sc: &Scene) {
        self.t += dt;
        self.dt = dt;
        if sc.level.id != self.level_id {
            self.level_id = sc.level.id.clone();
            self.ents.clear();
            self.cam_ok = false;
            self.texts.clear();
            self.parts.clear();
            self.lights.clear();
            self.beams.clear();
            self.ambient.clear();
            self.last_tick = u64::MAX;
        }
        if sc.snap.tick != self.last_tick {
            self.last_tick = sc.snap.tick;
            self.sync(sc);
            for f in &sc.snap.fx {
                self.spawn_fx(f);
            }
        }
        self.animate(dt, sc);
        let want = ZOOM[self.zoom_idx];
        if self.zoom_cur == 0.0 || (want - self.zoom_cur).abs() < 0.25 {
            self.zoom_cur = want;
        } else {
            self.zoom_cur += (want - self.zoom_cur) * (1.0 - (-dt * 12.0).exp());
        }
        let (tx, ty) = match self.ents.get(&sc.you) {
            Some(me) => (me.x, me.y),
            None => (sc.snap.you.x, sc.snap.you.y),
        };
        if !self.cam_ok || (tx - self.cam.0).abs() + (ty - self.cam.1).abs() > 12.0 {
            self.cam = (tx, ty);
            self.cam_ok = true;
        } else {
            let k = 1.0 - (-dt * 9.0).exp();
            self.cam.0 += (tx - self.cam.0) * k;
            self.cam.1 += (ty - self.cam.1) * k;
        }
        self.status_particles(dt, sc);
        self.update_effects(dt);
        self.update_ambient(dt, sc.level.lit, sc.snap.time_of_day);
    }

    /// Takes in a new snapshot.
    fn sync(&mut self, sc: &Scene) {
        self.snap_at = self.t;
        let first = self.ents.is_empty();
        for st in self.ents.values_mut() {
            st.gone = true;
        }
        for e in &sc.snap.entities {
            let st = self.ents.entry(e.id).or_insert_with(|| EntState {
                x: e.x,
                y: e.y,
                alpha: if first { 1.0 } else { 0.0 },
                swing: e.swing,
                facing: e.facing,
                left: e.facing.cos() < -0.2,
                hp: e.hp,
                ..Default::default()
            });
            st.gone = false;
            st.sx = e.x;
            st.sy = e.y;
            st.vx = e.vx;
            st.vy = e.vy;
            if e.swing != st.swing {
                st.swing = e.swing;
                st.lunge = 1.0;
            }
            if e.hp < st.hp {
                st.flash = 0.16;
            }
            st.hp = e.hp;
            st.facing = e.facing;
            if e.kind == KIND_PROJECTILE {
                st.trail.push((st.x, st.y));
                if st.trail.len() > 6 {
                    st.trail.remove(0);
                }
            }
            st.view = e.clone();
            // a jump (teleport, stairs) is not animated
            if (st.x - e.x).abs() + (st.y - e.y).abs() > 3.0 {
                st.x = e.x;
                st.y = e.y;
                st.trail.clear();
            }
        }
    }

    fn animate(&mut self, dt: f32, sc: &Scene) {
        let age = (self.t - self.snap_at).min(0.25);
        let l = sc.level;
        let you = sc.you;
        let speed = sc.snap.you.speed;
        let input = sc.input;
        let visible = self.visible(sc).clone();
        let follow = 1.0 - (-dt * 16.0).exp();
        let mut dead = vec![];
        for (id, st) in self.ents.iter_mut() {
            let e = &st.view;
            let (mut px, mut py) = (st.sx + st.vx * age, st.sy + st.vy * age);
            if *id == you && !e.dead {
                // the own hero answers the keys at once: walk the held
                // direction along the walls until the server confirms
                let (ix, iy) = (input[0] as f32, input[1] as f32);
                if ix != 0.0 || iy != 0.0 {
                    let n = (ix * ix + iy * iy).sqrt();
                    let d = WV::new(ix / n * speed * age, iy / n * speed * age);
                    let blocked = |x: i32, y: i32| !l.walkable(x, y);
                    let p = l.slide(WV::new(st.sx, st.sy), d, e.radius.max(0.2), &blocked);
                    px = p.x;
                    py = p.y;
                }
            }
            let (ox, oy) = (st.x, st.y);
            if e.kind == KIND_PROJECTILE {
                st.x = px;
                st.y = py;
            } else {
                st.x += (px - st.x) * follow;
                st.y += (py - st.y) * follow;
            }
            let moved = ((st.x - ox).powi(2) + (st.y - oy).powi(2)).sqrt();
            st.walk += moved;
            if moved > 0.002 && e.kind != KIND_PROJECTILE {
                let dxm = st.x - ox;
                if dxm.abs() > 0.004 {
                    st.left = dxm < 0.0;
                }
            } else if st.facing.cos().abs() > 0.35 {
                st.left = st.facing.cos() < 0.0;
            }
            st.lunge = (st.lunge - dt * 5.0).max(0.0);
            st.flash = (st.flash - dt).max(0.0);
            let (cx, cy) = (st.x.floor() as i32, st.y.floor() as i32);
            let seen = !st.gone && l.inside(cx, cy) && visible.get(cx, cy);
            st.seen = seen;
            let target = if seen { 1.0 } else { 0.0 };
            st.alpha += (target - st.alpha) * (1.0 - (-dt * 8.0).exp());
            if st.gone && st.alpha < 0.03 {
                dead.push(*id);
            }
        }
        for id in dead {
            self.ents.remove(&id);
        }
    }

    fn burst(
        &mut self,
        x: f32,
        y: f32,
        n: usize,
        c: Color,
        speed: f32,
        grav: f32,
        size: f32,
        glow: bool,
    ) {
        for _ in 0..n {
            let a = self.rf() * TAU;
            let s = speed * (0.4 + self.rf() * 0.8);
            let life = 0.35 + self.rf() * 0.45;
            let size = size * (0.7 + self.rf() * 0.6);
            self.parts.push(Particle {
                x,
                y,
                vx: a.cos() * s,
                vy: a.sin() * s - grav * 0.08,
                t: 0.0,
                life,
                size,
                grav,
                col: c,
                glow,
            });
        }
    }

    fn spawn_fx(&mut self, f: &Fx) {
        let (cx, cy) = (f.x, f.y);
        let c = col(&f.color);
        if !f.text.is_empty() {
            let big = f.text.contains("УРОВЕНЬ");
            let jitter = (self.rf() - 0.5) * 0.4;
            self.texts.push(FloatText {
                x: cx + jitter,
                y: cy - 0.6,
                t: 0.0,
                life: (f.ms as f32 / 1000.0).max(0.8),
                text: tr(&f.text),
                col: c,
                big,
            });
            if big {
                self.burst(
                    cx,
                    cy,
                    40,
                    Color::from_rgba(255, 230, 90, 255),
                    4.0,
                    0.0,
                    0.16,
                    true,
                );
                self.lights.push(Flash {
                    x: cx,
                    y: cy,
                    t: 0.0,
                    life: 0.9,
                    radius: 4.0,
                    col: Color::from_rgba(255, 220, 100, 255),
                    ring: false,
                });
            }
            return;
        }
        if f.radius > 0.0 {
            // an area: a ring sweeping out and sparks over it
            self.lights.push(Flash {
                x: cx,
                y: cy,
                t: 0.0,
                life: (f.ms as f32 / 1000.0).max(0.3),
                radius: f.radius,
                col: c,
                ring: true,
            });
            let n = (f.radius * f.radius * 4.0).clamp(8.0, 60.0) as usize;
            for _ in 0..n {
                let a = self.rf() * TAU;
                let r = self.rf().sqrt() * f.radius;
                let (x, y) = (cx + a.cos() * r, cy + a.sin() * r);
                let life = 0.3 + self.rf() * 0.4;
                self.parts.push(Particle {
                    x,
                    y,
                    vx: 0.0,
                    vy: -0.6,
                    t: 0.0,
                    life,
                    size: 0.1,
                    grav: -0.5,
                    col: c,
                    glow: true,
                });
            }
            return;
        }
        if (f.x2 - f.x).abs() + (f.y2 - f.y).abs() > 0.01 {
            self.beams.push(Beam {
                x: f.x,
                y: f.y,
                x2: f.x2,
                y2: f.y2,
                t: 0.0,
                life: (f.ms as f32 / 1000.0).max(0.2),
                col: c,
            });
            self.burst(f.x2, f.y2, 6, c, 2.5, 0.0, 0.1, true);
            return;
        }
        match f.glyph {
            '%' => self.burst(cx, cy, 18, mix_c(c, BLACK, 0.2), 3.0, 7.0, 0.13, false),
            '~' => self.burst(cx, cy, 4, c, 1.0, 0.0, 0.18, true),
            _ => {
                self.burst(cx, cy, 7, c, 3.2, 0.0, 0.11, true);
                self.lights.push(Flash {
                    x: cx,
                    y: cy,
                    t: 0.0,
                    life: 0.28,
                    radius: 1.6,
                    col: c,
                    ring: false,
                });
            }
        }
    }

    fn update_effects(&mut self, dt: f32) {
        self.parts.retain_mut(|p| {
            p.t += dt;
            p.vy += p.grav * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.vx *= (-dt * 3.0).exp();
            p.t < p.life
        });
        self.texts.retain_mut(|t| {
            t.t += dt;
            t.t < t.life
        });
        self.lights.retain_mut(|l| {
            l.t += dt;
            l.t < l.life
        });
        self.beams.retain_mut(|b| {
            b.t += dt;
            b.t < b.life
        });
    }

    /// Fireflies at night, dust motes underground.
    fn update_ambient(&mut self, dt: f32, lit: bool, tod: f64) {
        let (want, c) = if lit && daylight(tod) < 0.4 {
            (26, Color::from_rgba(190, 255, 120, 255))
        } else if !lit {
            (30, Color::from_rgba(200, 190, 170, 255))
        } else {
            (0, WHITE)
        };
        let cam = self.cam;
        let mut keep = Vec::with_capacity(self.ambient.len());
        for mut p in std::mem::take(&mut self.ambient) {
            p.t += dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.vx += (self.rf() - 0.5) * dt * 2.0;
            p.vy += (self.rf() - 0.5) * dt * 2.0;
            if p.t < p.life
                && (p.x - cam.0).abs() < 16.0
                && (p.y - cam.1).abs() < 11.0
                && keep.len() < want
            {
                keep.push(p);
            }
        }
        self.ambient = keep;
        while self.ambient.len() < want {
            let p = Particle {
                x: cam.0 + (self.rf() - 0.5) * 30.0,
                y: cam.1 + (self.rf() - 0.5) * 20.0,
                vx: (self.rf() - 0.5) * 0.6,
                vy: (self.rf() - 0.5) * 0.6,
                t: 0.0,
                life: 4.0 + self.rf() * 6.0,
                size: 0.12,
                grav: 0.0,
                col: c,
                glow: lit,
            };
            self.ambient.push(p);
        }
    }

    /// Flames, poison bubbles, blood, curses and frost around creatures.
    fn status_particles(&mut self, dt: f32, sc: &Scene) {
        let mut emit = vec![];
        for e in &sc.snap.entities {
            if e.status == 0 {
                continue;
            }
            let Some(st) = self.ents.get(&e.id) else {
                continue;
            };
            let (x, y) = (st.x, st.y - 0.25);
            let list: [(u16, f32, [u8; 3], f32, f32, f32, bool); 6] = [
                (STATUS_BURNING, 16.0, [255, 140, 40], -1.4, -0.6, 0.09, true),
                (STATUS_POISONED, 7.0, [150, 230, 70], -0.7, 0.0, 0.07, true),
                (STATUS_BLEEDING, 7.0, [190, 20, 20], 0.4, 5.0, 0.1, false),
                (STATUS_CURSED, 6.0, [180, 90, 255], -0.5, 0.0, 0.07, true),
                (STATUS_CHILLED, 5.0, [200, 240, 255], 0.35, 0.0, 0.06, true),
                (STATUS_HOLY, 10.0, [255, 236, 150], -1.0, -0.3, 0.07, true),
            ];
            for (bit, rate, c, vy, grav, size, glow) in list {
                if e.status & bit != 0 {
                    emit.push((x, y, rate, c, vy, grav, size, glow));
                }
            }
        }
        for (x, y, rate, c, vy, grav, size, glow) in emit {
            if self.rf() < rate * dt {
                let p = Particle {
                    x: x + (self.rf() - 0.5) * 0.55,
                    y: y + (self.rf() - 0.5) * 0.4,
                    vx: (self.rf() - 0.5) * 0.4,
                    vy: vy * (0.6 + self.rf() * 0.8),
                    t: 0.0,
                    life: 0.45 + self.rf() * 0.45,
                    size,
                    grav,
                    col: Color::from_rgba(c[0], c[1], c[2], 255),
                    glow,
                };
                self.parts.push(p);
            }
        }
    }

    // ---- drawing ----

    /// A view centred on a camera point inside an area of the screen.
    pub fn make_view(&self, l: &Level, cam: (f32, f32), area: Rect, s: f32) -> View {
        let ts = self.tile_size(s);
        let (hx, hy) = (area.w / 2.0 / ts, area.h / 2.0 / ts);
        let (mut cx, mut cy) = cam;
        if l.w as f32 > 2.0 * hx {
            cx = cx.clamp(hx, l.w as f32 - hx);
        } else {
            cx = l.w as f32 / 2.0;
        }
        if l.h as f32 > 2.0 * hy {
            cy = cy.clamp(hy - 0.5, l.h as f32 - hy + 0.5);
        } else {
            cy = l.h as f32 / 2.0;
        }
        let focus = (area.x + area.w / 2.0, area.y + area.h / 2.0);
        let ox = (focus.0 - cx * ts).floor();
        let oy = (focus.1 - cy * ts).floor();
        View {
            ox,
            oy,
            ts,
            tx0: ((-ox / ts).floor() as i32 - 1).max(0),
            ty0: ((-oy / ts).floor() as i32 - 1).max(0),
            tx1: (((screen_width() - ox) / ts).ceil() as i32 + 1).min(l.w - 1),
            ty1: (((screen_height() - oy) / ts).ceil() as i32 + 2).min(l.h - 1),
        }
    }

    /// The ground under an object: by its biome, or the floor next to it.
    fn ground_under<'a>(l: &Level, x: i32, y: i32, def: &TileDef) -> Option<&'a TileDef> {
        let db = content::db();
        if def.biome.is_empty() {
            for (dx, dy) in [
                (0, 1),
                (1, 0),
                (-1, 0),
                (0, -1),
                (1, 1),
                (-1, 1),
                (1, -1),
                (-1, -1),
            ] {
                if !l.inside(x + dx, y + dy) {
                    continue;
                }
                let n = db.tile(l.at(x + dx, y + dy));
                if crate::art::tiles::is_known_ground(&n.key) && !not_base(&n.key) {
                    return Some(n);
                }
            }
        }
        db.tile_by_key(crate::art::tiles::base_ground(def, &l.theme))
    }

    /// Draws the ground pass, then tall objects and creatures row by row so
    /// that canopies and walls hide what stands behind them.
    pub fn draw_tiles(
        &self,
        g: &mut Gfx,
        l: &Level,
        v: &View,
        vis: Option<&Vis>,
        explored: Option<&Bitset>,
        mut row_entities: impl FnMut(&mut Gfx, i32),
    ) {
        let db = content::db();
        let ts = v.ts;
        let frame = (self.t * 3.0) as usize % 4;
        let memory = Color::new(0.86, 0.9, 1.0, 1.0);
        let seen = |i: usize| explored.is_none_or(|e| e.get(i));
        let lit = |x: i32, y: i32| vis.is_none_or(|vv| vv.get(x, y));
        for y in v.ty0..=v.ty1 {
            for x in v.tx0..=v.tx1 {
                let i = (y * l.w + x) as usize;
                if !seen(i) {
                    continue;
                }
                let def = db.tile(l.tiles[i]);
                let c = if lit(x, y) { WHITE } else { memory };
                let (px, py) = v.px(x as f32, y as f32);
                let h = tile_hash(x, y);
                let (has_ground, wall) = {
                    let tt = g.tile(def);
                    (!tt.ground.is_empty(), tt.wall)
                };
                if has_ground {
                    let tt = g.tile(def);
                    let gr = &tt.ground[h % tt.ground.len()];
                    draw_texture_ex(
                        &gr[frame % gr.len()],
                        px,
                        py,
                        c,
                        DrawTextureParams {
                            dest_size: Some(vec2(ts, ts)),
                            ..Default::default()
                        },
                    );
                } else if !wall {
                    if let Some(base) = Self::ground_under(l, x, y, def) {
                        let tt = g.tile(base);
                        if !tt.ground.is_empty() {
                            let gr = &tt.ground[h % tt.ground.len()];
                            draw_texture_ex(
                                &gr[frame % gr.len()],
                                px,
                                py,
                                c,
                                DrawTextureParams {
                                    dest_size: Some(vec2(ts, ts)),
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }
                let tt = g.tile(def);
                if !tt.object.is_empty() && !tt.tall {
                    let obj = &tt.object[h % tt.object.len()];
                    draw_texture_ex(
                        obj,
                        px,
                        py,
                        c,
                        DrawTextureParams {
                            dest_size: Some(vec2(ts, ts)),
                            ..Default::default()
                        },
                    );
                }
                if tt.ground.len() == 1
                    && tt.object.is_empty()
                    && !crate::art::tiles::is_known_ground(&def.key)
                {
                    // a modded tile without art shows its glyph
                    g.text_center(
                        &def.glyph,
                        px + ts / 2.0,
                        py + ts / 2.0,
                        ts * 0.7,
                        col(&def.fg),
                        true,
                        false,
                    );
                }
            }
        }
        for y in v.ty0..=v.ty1 {
            for x in v.tx0..=v.tx1 {
                let i = (y * l.w + x) as usize;
                if !seen(i) {
                    continue;
                }
                let def = db.tile(l.tiles[i]);
                let tt = g.tile(def);
                if tt.object.is_empty() || !tt.tall {
                    continue;
                }
                let c = if lit(x, y) { WHITE } else { memory };
                let (px, py) = v.px(x as f32, y as f32);
                let img = &tt.object[tile_hash(x, y) % tt.object.len()];
                let h = img.height() / SPX as f32 * ts;
                let top = py + ts - h;
                let amp = sway(&def.key);
                if amp > 0.0 && l.lit {
                    let w = (self.t * 1.1 - x as f32 * 0.32 - y as f32 * 0.18).sin()
                        + 0.35 * (self.t * 2.9 + (tile_hash(x, y) % 17) as f32).sin();
                    let dx = amp * w * h;
                    draw_quad(
                        img,
                        [
                            vec2(px + dx, top),
                            vec2(px + ts + dx, top),
                            vec2(px + ts, top + h),
                            vec2(px, top + h),
                        ],
                        c,
                    );
                } else {
                    draw_texture_ex(
                        img,
                        px,
                        top,
                        c,
                        DrawTextureParams {
                            dest_size: Some(vec2(ts, h)),
                            ..Default::default()
                        },
                    );
                }
            }
            row_entities(g, y);
        }
    }

    /// Draws the world of a scene into an area of the screen.
    pub fn draw(&mut self, g: &mut Gfx, sc: &Scene, area: Rect) {
        let l = sc.level;
        self.tops.clear();
        let v = self.make_view(l, self.cam, area, g.s);
        self.view = v;
        let vis = self.visible(sc).clone();
        // creatures by row for depth sorting
        let mut rows: HashMap<i32, Vec<u32>> = HashMap::new();
        let mut projectiles = vec![];
        for (id, st) in &self.ents {
            if st.alpha < 0.02 {
                continue;
            }
            if st.view.kind == KIND_PROJECTILE {
                projectiles.push(*id);
                continue;
            }
            rows.entry(st.y.floor() as i32).or_default().push(*id);
        }
        for list in rows.values_mut() {
            list.sort_by(|a, b| {
                let (ea, eb) = (&self.ents[a], &self.ents[b]);
                let ka = (
                    ea.view.kind != KIND_ITEM,
                    !ea.view.dead,
                    (ea.y * 1000.0) as i32,
                    (ea.x * 1000.0) as i32,
                );
                let kb = (
                    eb.view.kind != KIND_ITEM,
                    !eb.view.dead,
                    (eb.y * 1000.0) as i32,
                    (eb.x * 1000.0) as i32,
                );
                ka.cmp(&kb)
            });
        }
        let ents = std::mem::take(&mut self.ents);
        let mut tops = HashMap::new();
        let t = self.t;
        self.draw_tiles(g, l, &v, Some(&vis), Some(sc.explored), |g, row| {
            if let Some(list) = rows.get(&row) {
                for id in list {
                    if let Some(top) = draw_entity(g, &v, sc, &ents[id], t) {
                        tops.insert(*id, top);
                    }
                }
            }
        });
        self.ents = ents;
        self.tops = tops;
        let ts = v.ts;
        // projectiles with glowing trails
        for id in projectiles {
            let st = &self.ents[&id];
            let c = col(&st.view.color);
            let n = st.trail.len();
            for (i, p) in st.trail.iter().enumerate() {
                let k = (i + 1) as f32 / (n + 1) as f32;
                let (x, y) = v.px(p.0, p.1);
                g.glow(x, y, ts * 0.18 * k, c, 0.5 * k * st.alpha);
            }
            let (x, y) = v.px(st.x, st.y);
            g.glow(x, y, ts * 0.32, c, 0.9 * st.alpha);
            g.glow(x, y, ts * 0.12, WHITE, 0.9 * st.alpha);
        }
        // beams
        for b in &self.beams {
            let a = 1.0 - b.t / b.life;
            let (x1, y1) = v.px(b.x, b.y);
            let (x2, y2) = v.px(b.x2, b.y2);
            draw_line(x1, y1, x2, y2, (ts * 0.12).max(2.0), with_a(b.col, 0.5 * a));
            draw_line(x1, y1, x2, y2, (ts * 0.04).max(1.0), with_a(WHITE, 0.8 * a));
            let n = (((x2 - x1).hypot(y2 - y1)) / (ts * 0.5)).ceil().max(1.0) as i32;
            for i in 0..=n {
                let k = i as f32 / n as f32;
                g.glow(
                    x1 + (x2 - x1) * k,
                    y1 + (y2 - y1) * k,
                    ts * 0.35,
                    b.col,
                    0.35 * a,
                );
            }
        }
        // particles
        for p in &self.parts {
            let (x, y) = v.px(p.x, p.y);
            let a = 1.0 - p.t / p.life;
            if p.glow {
                g.glow(x, y, p.size * ts * 1.6, p.col, a);
            } else {
                let s = p.size * ts * 0.6;
                draw_rectangle(x, y, s, s, with_a(p.col, a));
            }
        }
        self.draw_lighting(g, &v, sc, &vis);
        self.draw_overlays(g, &v, sc, area);
    }

    fn darkness(
        &self,
        l: &Level,
        vis: &Vis,
        explored: &Bitset,
        sc: &Scene,
        x: i32,
        y: i32,
        tod: f64,
    ) -> f32 {
        if !l.inside(x, y) {
            return 1.0;
        }
        let i = (y * l.w + x) as usize;
        if !explored.get(i) {
            return 1.0;
        }
        if !vis.get(x, y) {
            return 0.62;
        }
        let vision = (sc.snap.you.vision as f32).max(1.0);
        let (sx, sy) = match self.ents.get(&sc.you) {
            Some(me) => (me.x, me.y),
            None => (sc.snap.you.x, sc.snap.you.y),
        };
        let d = ((x as f32 + 0.5 - sx).hypot(y as f32 + 0.5 - sy)) / vision;
        if l.lit {
            let night = 1.0 - daylight(tod) as f32;
            return night * (0.18 + 0.6 * d * d);
        }
        (0.04 + 0.82 * d.powf(1.5)).min(0.86)
    }

    fn draw_lighting(&mut self, g: &Gfx, v: &View, sc: &Scene, vis: &Vis) {
        let l = sc.level;
        let ts = v.ts;
        let (w, h) = (screen_width(), screen_height());
        let tod = sc.snap.time_of_day;
        let mut day = 1.0f32;
        if l.lit {
            day = daylight(tod) as f32;
            if day > 0.2 {
                // cloud shadows drifting over the land: nine to a field the
                // size of the classic world, repeated over a bigger one
                let (pw, ph) = (l.w.min(300) as f32, l.h.min(200) as f32);
                let (fx, fy) = ((v.tx0 as f32 / pw).floor(), (v.ty0 as f32 / ph).floor());
                let mut cr = Rng::new(l.id.len() as u64, 5);
                for _ in 0..9 {
                    let bx = (cr.f32() * pw + self.t * 0.35).rem_euclid(pw);
                    let by = (cr.f32() * ph + self.t * 0.12).rem_euclid(ph);
                    let s = (6.0 + cr.f32() * 6.0) * ts;
                    for (i, j) in [
                        (-1.0, -1.0),
                        (0.0, -1.0),
                        (-1.0, 0.0),
                        (0.0, 0.0),
                        (1.0, 0.0),
                        (0.0, 1.0),
                        (1.0, 1.0),
                        (1.0, -1.0),
                        (-1.0, 1.0),
                    ] {
                        let (cx, cy) = (bx + (fx + i) * pw, by + (fy + j) * ph);
                        if cx < 0.0 || cy < 0.0 || cx > l.w as f32 || cy > l.h as f32 {
                            continue;
                        }
                        let (x, y) = v.px(cx, cy);
                        g.shadow(x, y, s * 1.6, s, 0.13 * day);
                    }
                }
            }
            let night = 1.0 - day;
            if night > 0.0 {
                draw_rectangle(
                    0.0,
                    0.0,
                    w,
                    h,
                    Color::from_rgba(4, 8, 26, (130.0 * night) as u8),
                );
            }
            if day > 0.0 && day < 1.0 {
                let warm = 1.0 - (2.0 * day - 1.0).abs();
                draw_rectangle(0.0, 0.0, w, h, Color::new(1.0, 0.43, 0.08, 0.13 * warm));
            }
        }
        // darkness map: one pixel per tile, scaled up with linear filtering
        let (x0, y0) = (v.tx0 - 1, v.ty0 - 1);
        let (dw, dh) = (v.tx1 - x0 + 2, v.ty1 - y0 + 2);
        if self.dark_id != l.id || self.dark_cur.len() != (l.w * l.h) as usize {
            self.dark_id = l.id.clone();
            self.dark_cur = vec![-1.0; (l.w * l.h) as usize];
        }
        self.dark_buf.resize((dw * dh * 4) as usize, 0);
        let ease = 1.0 - (-self.dt * 7.0).exp();
        for yy in 0..dh {
            for xx in 0..dw {
                let (x, y) = (x0 + xx, y0 + yy);
                let mut a = self.darkness(l, vis, sc.explored, sc, x, y, tod);
                if l.inside(x, y) {
                    let i = (y * l.w + x) as usize;
                    let cur = self.dark_cur[i];
                    if cur >= 0.0 {
                        a = cur + (a - cur) * ease;
                    }
                    self.dark_cur[i] = a;
                }
                let o = ((yy * dw + xx) * 4) as usize;
                self.dark_buf[o..o + 4].copy_from_slice(&[
                    0,
                    0,
                    0,
                    (255.0 * a.clamp(0.0, 1.0)) as u8,
                ]);
            }
        }
        let tex = match &self.dark_tex {
            Some(t) if t.width() as i32 == dw && t.height() as i32 == dh => {
                t.update_from_bytes(dw as u32, dh as u32, &self.dark_buf);
                t.clone()
            }
            _ => {
                let t = Texture2D::from_rgba8(dw as u16, dh as u16, &self.dark_buf);
                t.set_filter(FilterMode::Linear);
                self.dark_tex = Some(t.clone());
                t
            }
        };
        // texels are tile centres: shift by half a tile
        let (ox, oy) = v.px(x0 as f32, y0 as f32);
        draw_texture_ex(
            &tex,
            ox,
            oy,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(dw as f32 * ts, dh as f32 * ts)),
                ..Default::default()
            },
        );

        // coloured light sources (additive)
        let (sx, sy) = match self.ents.get(&sc.you) {
            Some(me) => v.px(me.x, me.y),
            None => v.px(sc.snap.you.x, sc.snap.you.y),
        };
        let flicker = 0.9 + 0.1 * (self.t * 11.0).sin() * (self.t * 7.3).sin();
        if !sc.snap.you.dead {
            if !l.lit {
                g.glow(
                    sx,
                    sy,
                    sc.snap.you.vision as f32 * ts * 0.8,
                    Color::from_rgba(255, 160, 80, 255),
                    0.16 * flicker,
                );
            } else if day < 0.6 {
                g.glow(
                    sx,
                    sy,
                    6.0 * ts,
                    Color::from_rgba(255, 170, 90, 255),
                    0.16 * (1.0 - day) * flicker,
                );
            }
        }
        let db = content::db();
        let mut lava = 0;
        let mut sparks = vec![];
        for y in v.ty0..=v.ty1 {
            for x in v.tx0..=v.tx1 {
                if !vis.get(x, y) {
                    continue;
                }
                let def = db.tile(l.at(x, y));
                let (px, py) = v.px(x as f32 + 0.5, y as f32 + 0.5);
                if def.damage > 0.0 && lava < 220 {
                    lava += 1;
                    g.glow(
                        px,
                        py,
                        ts * 1.5,
                        col(&def.fg),
                        0.13 * (0.85 + 0.15 * (self.t * 2.0 + x as f32).sin()),
                    );
                } else if matches!(
                    def.interact.as_str(),
                    "stairs_down" | "stairs_up" | "dungeon"
                ) {
                    g.glow(
                        px,
                        py,
                        ts * 1.4,
                        col(&def.fg),
                        0.14 + 0.05 * (self.t * 2.0).sin(),
                    );
                } else if def.key == "altar" {
                    g.glow(px, py, ts * 2.0, col(&def.fg), 0.2);
                } else if !def.light.is_empty() {
                    let lc = col(&def.light);
                    let py = py - ts * 0.1;
                    if def.key == "campfire" || def.key == "brazier" {
                        let fl = 0.85
                            + 0.15
                                * (self.t * 9.0 + x as f32).sin()
                                * (self.t * 6.3 + y as f32).sin();
                        g.glow(px, py, ts * 3.4, lc, 0.32 * fl);
                        g.glow(
                            px,
                            py - ts * 0.1,
                            ts * 0.7,
                            Color::from_rgba(255, 230, 140, 255),
                            0.5 * fl,
                        );
                        sparks.push((x, y));
                    } else {
                        g.glow(
                            px,
                            py,
                            ts * 1.8,
                            lc,
                            0.18 + 0.05 * (self.t * 2.0 + (x * 7 + y) as f32).sin(),
                        );
                    }
                }
            }
        }
        for (x, y) in sparks {
            if self.rf() < 0.3 {
                let p = Particle {
                    x: x as f32 + 0.5 + (self.rf() - 0.5) * 0.3,
                    y: y as f32 + 0.35,
                    vx: (self.rf() - 0.5) * 0.3,
                    vy: -1.2 - self.rf(),
                    t: 0.0,
                    life: 0.5 + self.rf() * 0.5,
                    size: 0.07,
                    grav: -0.4,
                    col: Color::from_rgba(255, 150, 50, 255),
                    glow: true,
                };
                self.parts.push(p);
            }
        }
        for st in self.ents.values() {
            if st.view.kind == KIND_PROJECTILE && st.alpha > 0.1 {
                let (x, y) = v.px(st.x, st.y);
                g.glow(x, y, ts * 2.2, col(&st.view.color), 0.45 * st.alpha);
            }
        }
        for fl in &self.lights {
            let (x, y) = v.px(fl.x, fl.y);
            let k = fl.t / fl.life;
            if fl.ring {
                let r = fl.radius * ts * (0.3 + 0.7 * k.sqrt());
                g.glow(x, y, r * 1.1, fl.col, 0.35 * (1.0 - k));
                draw_circle_lines(
                    x,
                    y,
                    r,
                    (ts * 0.08).max(2.0),
                    with_a(fl.col, 0.8 * (1.0 - k)),
                );
                draw_circle_lines(
                    x,
                    y,
                    r * 0.96,
                    (ts * 0.03).max(1.0),
                    with_a(WHITE, 0.6 * (1.0 - k)),
                );
            } else {
                g.glow(x, y, fl.radius * ts, fl.col, 0.6 * (1.0 - k));
            }
        }
        for p in &self.ambient {
            let (x, y) = v.px(p.x, p.y);
            let blink = 0.5 + 0.5 * (p.t * 3.0 + p.x).sin();
            if p.glow {
                g.glow(
                    x,
                    y,
                    ts * 0.22,
                    p.col,
                    0.8 * blink * (p.life - p.t).min(1.0),
                );
            } else {
                g.glow(x, y, ts * 0.08, p.col, 0.25 * blink);
            }
        }
    }

    fn draw_overlays(&self, g: &Gfx, v: &View, sc: &Scene, area: Rect) {
        let ts = v.ts;
        let label = (ts * 0.3).max(10.0 * g.s);
        let me = &sc.snap.you;
        for (id, st) in &self.ents {
            let e = &st.view;
            if st.alpha < 0.5 || e.kind == KIND_ITEM || e.kind == KIND_PROJECTILE {
                continue;
            }
            let (cx, cy) = v.px(st.x, st.y);
            let mut top = self
                .tops
                .get(id)
                .map(|t| t - ts * 0.06)
                .unwrap_or(cy - ts * 0.62);
            if e.dead {
                if e.kind == KIND_PLAYER {
                    g.text_center(
                        &format!("† {}", e.name),
                        cx,
                        cy - ts * 0.2,
                        label,
                        Color::new(0.78, 0.78, 0.82, 0.85),
                        true,
                        true,
                    );
                }
                continue;
            }
            let show_bar = (e.hostile && e.hp < 100)
                || (e.kind == KIND_PLAYER && *id != sc.you)
                || (e.ally && e.hp < 100)
                || (e.kind == KIND_NPC && e.hp < 100);
            if show_bar {
                let (bw, bh) = (ts * 0.8, (ts * 0.08).max(3.0));
                let (bx, by) = (cx - bw / 2.0, top - bh);
                draw_rectangle(
                    bx - 1.0,
                    by - 1.0,
                    bw + 2.0,
                    bh + 2.0,
                    Color::new(0.0, 0.0, 0.0, 0.7),
                );
                let fc = if e.hostile {
                    Color::from_rgba(220, 60, 60, 255)
                } else {
                    Color::from_rgba(90, 210, 90, 255)
                };
                draw_rectangle(bx, by, bw * e.hp as f32 / 100.0, bh, fc);
                top -= bh + 2.0;
            }
            let near = (st.x - me.x).abs() + (st.y - me.y).abs() <= 4.5;
            let unique = e.def.starts_with("unique:");
            if unique {
                let pulse = 0.8 + 0.2 * (self.t * 3.0).sin();
                star(
                    cx,
                    top - ts * (0.2 + 0.04 * (self.t * 2.4).sin()),
                    ts * 0.16,
                    Color::new(1.0, 0.82, 0.35, pulse),
                );
                top -= ts * 0.36;
            }
            if (e.kind == KIND_PLAYER && *id != sc.you)
                || (e.kind == KIND_NPC && (near || unique))
                || (e.ally && near)
            {
                let c = if e.ally {
                    Color::from_rgba(140, 240, 140, 242)
                } else if e.kind == KIND_PLAYER && e.hostile {
                    Color::from_rgba(255, 140, 130, 242)
                } else if unique {
                    Color::from_rgba(255, 170, 255, 242)
                } else if e.kind == KIND_NPC {
                    Color::from_rgba(255, 225, 150, 235)
                } else {
                    Color::new(1.0, 1.0, 1.0, 0.92)
                };
                g.text_center(&e.name, cx, top - ts * 0.16, label, c, true, true);
            }
        }
        // "press E" hint on stairs
        if matches!(
            me.standing_on.as_str(),
            "dungeon" | "stairs_down" | "stairs_up"
        ) {
            if let Some(st) = self.ents.get(&sc.you) {
                let (cx, cy) = v.px(st.x, st.y);
                let pulse = 0.75 + 0.25 * (self.t * 5.0).sin();
                let s = ts * 0.42;
                let (bx, by) = (cx - s / 2.0, cy - ts * 1.25 - s / 2.0);
                round_rect(
                    bx,
                    by,
                    s,
                    s,
                    s * 0.25,
                    Color::new(0.16 * pulse, 0.13 * pulse, 0.08 * pulse, 0.9 * pulse),
                );
                g.text_center(
                    "E",
                    cx,
                    by + s / 2.0,
                    ts * 0.3,
                    Color::from_rgba(255, 220, 120, 255),
                    true,
                    false,
                );
            }
        }
        // speech bubbles
        let bub = (ts * 0.3).max(12.0 * g.s);
        for st in self.ents.values() {
            let e = &st.view;
            if e.speech.is_empty() || st.alpha < 0.5 {
                continue;
            }
            let mut lines = g.wrap(&e.speech, bub * 16.0, bub, false);
            lines.truncate(4);
            let lh = bub * 1.25;
            let w = lines
                .iter()
                .map(|l| g.measure(l, bub, false))
                .fold(0.0, f32::max);
            let pad = bub * 0.5;
            let (bw, bh) = (w + pad * 2.0, lines.len() as f32 * lh + pad * 1.4);
            let (cx, cy) = v.px(st.x, st.y);
            let bx = (cx - bw / 2.0).clamp(4.0, (screen_width() - bw - 4.0).max(4.0));
            let head = self.tops.get(&e.id).copied().unwrap_or(cy - ts * 0.75);
            let by = head - bh - ts * 0.3;
            let (bg, fg) = if e.hostile {
                (
                    Color::from_rgba(70, 18, 18, 235),
                    Color::from_rgba(255, 210, 200, 255),
                )
            } else {
                (
                    Color::new(0.965, 0.94, 0.88, 0.94),
                    Color::from_rgba(40, 34, 30, 255),
                )
            };
            round_rect(bx, by, bw, bh, pad, bg);
            draw_triangle(
                vec2(cx - pad * 0.6, by + bh - 1.0),
                vec2(cx + pad * 0.6, by + bh - 1.0),
                vec2(cx, by + bh + pad * 0.9),
                bg,
            );
            for (i, ln) in lines.iter().enumerate() {
                g.text_raw(ln, bx + pad, by + pad * 0.7 + i as f32 * lh, bub, fg, false);
            }
        }
        // floating numbers
        for t in &self.texts {
            let k = t.t / t.life;
            let mut size = ts * 0.42;
            if t.big {
                size = ts * 0.7;
            } else if t.text.ends_with('!') {
                size = ts * 0.55;
            }
            let pop = 1.0 + 0.35 * (1.0 - t.t / 0.12).max(0.0);
            let (x, y) = v.px(t.x, t.y - k * 1.1);
            let a = if k > 0.6 { 1.0 - (k - 0.6) / 0.4 } else { 1.0 };
            g.text_center(
                &t.text,
                x,
                y,
                (size * pop).round(),
                with_a(t.col, a),
                true,
                true,
            );
        }
        // boss health bar
        if let Some(st) = self
            .ents
            .values()
            .find(|st| st.view.boss && st.alpha > 0.5 && !st.view.dead)
        {
            let e = &st.view;
            let s = g.s;
            let bw = (area.w * 0.5).min(520.0 * s);
            let bh = 12.0 * s;
            let bx = area.x + area.w / 2.0 - bw / 2.0;
            let by = area.y + 40.0 * s;
            g.text_center(
                &e.name,
                bx + bw / 2.0,
                by - 12.0 * s,
                15.0 * s,
                Color::from_rgba(255, 140, 255, 255),
                true,
                true,
            );
            round_rect(
                bx - 2.0,
                by - 2.0,
                bw + 4.0,
                bh + 4.0,
                bh / 2.0 + 2.0,
                Color::new(0.0, 0.0, 0.0, 0.8),
            );
            if e.hp > 0 {
                round_rect(
                    bx,
                    by,
                    bw * e.hp as f32 / 100.0,
                    bh,
                    bh / 2.0,
                    Color::from_rgba(200, 40, 140, 255),
                );
            }
        }
        let (w, h) = (screen_width(), screen_height());
        g.vignette(w, h, 0.7);
        if me.dead {
            draw_rectangle(0.0, 0.0, w, h, Color::from_rgba(60, 0, 0, 90));
        }
        if sc.paused {
            draw_rectangle(0.0, 0.0, w, h, Color::from_rgba(0, 0, 0, 90));
        }
    }

    /// The menu backdrop: a slowly drifting landscape.
    pub fn draw_backdrop(&mut self, g: &mut Gfx, l: &Level, t: f32) {
        self.t = t;
        let (w, h) = (screen_width(), screen_height());
        let cam = (
            l.w as f32 / 2.0 + (t * 0.021).sin() * l.w as f32 * 0.3,
            l.h as f32 / 2.0 + (t * 0.033 + 1.0).sin() * l.h as f32 * 0.25,
        );
        let v = self.make_view(l, cam, Rect::new(0.0, 0.0, w, h), g.s);
        self.draw_tiles(g, l, &v, None, None, |_, _| {});
        let day = daylight(((0.32 + t as f64 * 0.006) % 1.0).abs());
        draw_rectangle(
            0.0,
            0.0,
            w,
            h,
            Color::from_rgba(4, 8, 26, (150.0 * (1.0 - day)) as u8),
        );
        draw_rectangle(0.0, 0.0, w, h, Color::from_rgba(0, 0, 0, 80));
        g.vignette(w, h, 1.0);
    }
}

/// Draws one creature or item; returns the screen y of its top.
fn draw_entity(g: &mut Gfx, v: &View, sc: &Scene, st: &EntState, t: f32) -> Option<f32> {
    let e = &st.view;
    let ts = v.ts;
    let mut c = col(&e.color);
    let al = st.alpha;
    let walking = st.vx.abs() + st.vy.abs() > 0.05;
    let frame = if walking {
        ((st.walk * 2.6) as i32 % 2) as usize
    } else {
        0
    };
    let bob = if walking {
        (st.walk * PI * 2.6).sin().abs()
    } else {
        0.0
    };
    // the attack lunge moves the body toward where it faces
    let l = st.lunge * (1.0 - st.lunge) * 4.0 * 0.22;
    let (lx, ly) = (st.facing.cos() * l, st.facing.sin() * l);
    let (cx, cy) = v.px(st.x + lx, st.y + ly);
    if e.kind == KIND_ITEM {
        let bobi = (t * 3.0 + e.id as f32).sin() * ts * 0.05;
        g.shadow(cx, cy + ts * 0.3, ts * 0.5, ts * 0.18, 0.35 * al);
        g.glow(cx, cy + bobi, ts * 0.42, c, 0.3 * al);
        if e.rarity >= 1 {
            // rare loot shines in its rarity colour; epic and legendary with a beam
            let rc = col(rarity_color(e.rarity as i32));
            let pulse = 0.75 + 0.25 * (t * 3.0 + e.id as f32).sin();
            g.glow(
                cx,
                cy + bobi,
                ts * (0.45 + 0.12 * e.rarity as f32),
                rc,
                (0.18 + 0.1 * e.rarity as f32) * pulse * al,
            );
            if e.rarity >= 3 {
                for i in 1..=4 {
                    g.glow(
                        cx,
                        cy - ts * 0.35 * i as f32,
                        ts * 0.3,
                        rc,
                        0.16 * pulse / i as f32 * al,
                    );
                }
            }
            if let Some(d) = content::db().item(&e.def) {
                c = col(&d.color);
            }
        }
        let k = ts / SPX as f32 * 0.85;
        let ccol = format!(
            "#{:02x}{:02x}{:02x}",
            (c.r * 255.0) as u8,
            (c.g * 255.0) as u8,
            (c.b * 255.0) as u8
        );
        match g.icon(&e.def, e.glyph, &ccol) {
            Some(icon) => {
                let (w, h) = (icon.width() * k, icon.height() * k);
                draw_texture_ex(
                    &icon,
                    (cx - w / 2.0).round(),
                    (cy + bobi - h / 2.0).round(),
                    Color::new(1.0, 1.0, 1.0, al),
                    DrawTextureParams {
                        dest_size: Some(vec2(w, h)),
                        ..Default::default()
                    },
                );
            }
            None => g.text_center(
                &e.glyph.to_string(),
                cx,
                cy + bobi,
                ts * 0.6,
                with_a(c, al),
                true,
                true,
            ),
        }
        return Some(cy - ts * 0.5);
    }
    let name = resolve(&e.model, &e.def, e.glyph, e.kind);
    let nv = variants(&name);
    let variant = if nv > 1 {
        (e.id as i32).rem_euclid(nv)
    } else {
        0
    };
    let (img, float) = {
        let m = g.model(&name, &e.color, variant, &e.gear);
        (m.frames[if e.dead { 0 } else { frame }].clone(), m.float)
    };
    let mut k = ts / SPX as f32;
    if e.boss {
        k *= 1.5;
    }
    let feet = cy + ts * 0.44;
    let (w, h) = (img.width() * k, img.height() * k);
    if e.dead {
        // lying on the side, greyed out
        g.shadow(cx, feet - ts * 0.1, h * 0.8, ts * 0.2, 0.4 * al);
        draw_texture_ex(
            &img,
            (cx - w / 2.0).round(),
            (feet - w / 2.0 - h / 2.0).round(),
            Color::new(0.62, 0.56, 0.56, al),
            DrawTextureParams {
                dest_size: Some(vec2(w, h)),
                rotation: -PI / 2.0,
                ..Default::default()
            },
        );
        return Some(feet - w);
    }
    let mut lift = bob * ts * 0.07;
    if float {
        lift = ts * 0.16 + (t * 2.6 + e.id as f32).sin() * ts * 0.06;
    }
    let hidden = e.status & STATUS_STEALTH != 0;
    let illusion = e.status & STATUS_ILLUSION != 0;
    if !hidden {
        g.shadow(
            cx,
            feet - ts * 0.04,
            (ts * 0.55).max(w * 0.8) * (1.0 - bob * 0.12),
            ts * 0.24,
            0.5 * al,
        );
    }
    let ring = (ts * 0.035).max(1.0);
    if e.id == sc.you {
        ellipse_lines(
            cx,
            feet - ts * 0.04,
            ts * 0.36,
            ts * 0.12,
            (ts * 0.04).max(1.0),
            Color::new(1.0, 0.84, 0.43, 0.8 * al),
        );
    } else if e.ally {
        ellipse_lines(
            cx,
            feet - ts * 0.04,
            ts * 0.36,
            ts * 0.12,
            ring,
            Color::new(0.43, 0.9, 0.47, 0.85 * al),
        );
    } else if e.kind == KIND_PLAYER && e.hostile && !illusion {
        ellipse_lines(
            cx,
            feet - ts * 0.04,
            ts * 0.36,
            ts * 0.12,
            ring,
            Color::new(0.94, 0.31, 0.27, 0.8 * al),
        );
    } else if e.kind == KIND_PLAYER {
        ellipse_lines(
            cx,
            feet - ts * 0.04,
            ts * 0.36,
            ts * 0.12,
            ring,
            with_a(c, 0.8 * al),
        );
    }
    if e.boss {
        g.glow(
            cx,
            feet - h * 0.45,
            w.max(h) * 0.9,
            c,
            0.16 + 0.06 * (t * 3.0).sin(),
        );
    }
    if e.status & STATUS_SHIELDED != 0 {
        g.glow(
            cx,
            feet - h * 0.5,
            w.max(h) * 0.75,
            Color::from_rgba(255, 230, 140, 255),
            0.22 + 0.05 * (t * 5.0).sin(),
        );
    }
    let top = (feet - h - lift).round();
    let mut tint = WHITE;
    if e.status & STATUS_CHILLED != 0 {
        tint = Color::new(0.72, 0.88, 1.0, 1.0);
    } else if e.status & STATUS_POISONED != 0 {
        tint = Color::new(0.8, 1.0, 0.72, 1.0);
    }
    tint.a = al;
    if hidden {
        tint.a *= 0.32 + 0.08 * (t * 4.0 + e.id as f32).sin();
    } else if illusion {
        tint = Color::new(
            0.9,
            0.82,
            1.0,
            al * (0.72 + 0.12 * (t * 9.0 + e.id as f32).sin()),
        );
    }
    let params = DrawTextureParams {
        dest_size: Some(vec2(w, h)),
        flip_x: st.left,
        ..Default::default()
    };
    let x = (cx - w / 2.0).round();
    draw_texture_ex(&img, x, top, tint, params.clone());
    if illusion && (t * 3.0 + e.id as f32).sin() > 0.85 {
        draw_texture_ex(
            &img,
            x + (ts * 0.05).round(),
            top,
            with_a(tint, tint.a * 0.35),
            params.clone(),
        );
    }
    if st.flash > 0.0 {
        let a = st.flash / 0.16;
        gl_use_material(&g.add);
        draw_texture_ex(&img, x, top, Color::new(1.0, 1.0, 1.0, a * al), params);
        gl_use_default_material();
    }
    if e.status & STATUS_BURNING != 0 {
        g.glow(
            cx,
            feet - h * 0.4,
            w * 0.8,
            Color::from_rgba(255, 120, 30, 255),
            0.3 + 0.1 * (t * 13.0).sin(),
        );
    }
    if e.status & STATUS_SILENCED != 0 {
        let (sx, sy) = (cx + ts * 0.24, top + ts * 0.04);
        let rad = (ts * 0.09).max(3.0);
        let lw = (ts * 0.025).max(1.0);
        draw_circle(sx, sy, rad, Color::from_rgba(40, 10, 60, 200));
        draw_circle_lines(sx, sy, rad, lw, Color::from_rgba(200, 120, 255, 255));
        draw_line(
            sx - rad * 0.7,
            sy + rad * 0.7,
            sx + rad * 0.7,
            sy - rad * 0.7,
            lw,
            Color::from_rgba(200, 120, 255, 255),
        );
    }
    if e.status & STATUS_STUNNED != 0 {
        for i in 0..3 {
            let a = t * 4.0 + i as f32 * TAU / 3.0;
            let (sx, sy) = (
                cx + a.cos() * ts * 0.24,
                top - ts * 0.06 + a.sin() * ts * 0.07,
            );
            draw_circle(
                sx,
                sy,
                (ts * 0.045).max(1.5),
                Color::from_rgba(255, 230, 90, 255),
            );
        }
    }
    Some(top)
}
