//! The world in 3D, the other way to draw what `world.rs` keeps: the same
//! creatures gliding, the same particles and abilities, but the land is
//! ground with walls, rocks, trees and houses standing on it, creatures are
//! built of shapes, and the camera looks down at the hero from behind at an
//! angle. Two looks: low-poly (faceted boxes and prisms) and detailed
//! (smooth shapes of many faces, grass, stones, lit per pixel). The darkness
//! (what the hero does not see, the night, the dark of dungeons) is laid
//! over the land by a shader from the same map of darkness the 2D view uses.
//! Abilities keep their 2D drawings, laid on the ground, held at chest
//! height or turned to the camera (see spells/d3.rs). Names, bars and
//! numbers stay flat on the screen.

mod beings;
mod figures;
mod gpu;
pub(crate) mod lowpoly;
pub(crate) mod materials;
mod roofs;
mod scenery;
mod shapes;
mod terrain;

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;
use miniquad::{BlendFactor, BlendState, BlendValue, Comparison, Equation, PipelineParams};
use ratas_core::content;
use ratas_core::proto::*;

use super::spells::Out;
use super::{Particle, Scene, Vis, WorldRenderer, KIND_ITEM, KIND_PROJECTILE, ZOOM};
use crate::art::specs::{floats, resolve, variants, KIND_PLAYER};
use crate::gfx::{col, daylight, with_a, Gfx};
use beings::{Paint, Pose};
use gpu::{Gpu, Light, Slot};
use lowpoly::{draw_vertices, Batch, Xf};
use ratas_core::game::rarity_color;
pub use terrain::Land;

/// Pixels per tile of the 2D drawings laid into the 3D world.
pub const TS: f32 = 64.0;
/// How far the camera looks down from the horizon (radians).
const PITCH: f32 = 1.05;
/// The camera's field of view (radians, vertical).
const FOV: f32 = 0.66;
/// Tiles drawn around the camera at most.
const REACH: f32 = 46.0;

/// The camera of a frame.
#[derive(Clone, Copy, Debug)]
pub struct Cam {
    pub mat: Mat4,
    pub inv: Mat4,
    pub eye: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub w: f32,
    pub h: f32,
}

impl Cam {
    /// Where a point of the world is on the screen.
    pub fn project(&self, p: Vec3) -> Option<Vec2> {
        let c = self.mat * p.extend(1.0);
        if c.w <= 0.01 {
            return None;
        }
        let n = c.truncate() / c.w;
        Some(vec2((n.x + 1.0) / 2.0 * self.w, (1.0 - n.y) / 2.0 * self.h))
    }

    /// The point at height h under a screen position.
    pub fn ground(&self, sx: f32, sy: f32, h: f32) -> Option<Vec2> {
        let nx = sx / self.w * 2.0 - 1.0;
        let ny = 1.0 - sy / self.h * 2.0;
        let a = self.inv * vec4(nx, ny, -1.0, 1.0);
        let b = self.inv * vec4(nx, ny, 1.0, 1.0);
        let (a, b) = (a.truncate() / a.w, b.truncate() / b.w);
        let d = b - a;
        if d.y.abs() < 1e-6 {
            return None;
        }
        let t = (h - a.y) / d.y;
        if t < 0.0 {
            return None;
        }
        let p = a + d * t;
        Some(vec2(p.x, p.z))
    }

    /// Screen pixels per tile at a point.
    pub fn scale(&self, p: Vec3) -> f32 {
        match (self.project(p), self.project(p + self.right)) {
            (Some(a), Some(b)) => a.distance(b),
            _ => 32.0,
        }
    }
}

/// A drawing plane: the pixels of a 2D drawing (x right, y down, TS per
/// tile) put into the world at o, along ex and ey (one tile each).
pub fn plane(o: Vec3, ex: Vec3, ey: Vec3) -> Mat4 {
    Mat4::from_cols(
        (ex / TS).extend(0.0),
        (ey / TS).extend(0.0),
        ex.cross(ey).normalize_or_zero().extend(0.0),
        o.extend(1.0),
    )
}

/// Draws with a model matrix.
pub fn with_plane(m: Mat4, f: impl FnOnce()) {
    unsafe { get_internal_gl() }.quad_gl.push_model_matrix(m);
    f();
    unsafe { get_internal_gl() }.quad_gl.pop_model_matrix();
}

/// A picture in the world: its centre and half extents along its width
/// and down its height.
pub fn sprite(tex: Option<&Texture2D>, c: Vec3, ex: Vec3, ey: Vec3, color: Color) {
    let v = [
        Vertex::new2(c - ex - ey, vec2(0.0, 0.0), color),
        Vertex::new2(c + ex - ey, vec2(1.0, 0.0), color),
        Vertex::new2(c + ex + ey, vec2(1.0, 1.0), color),
        Vertex::new2(c - ex + ey, vec2(0.0, 1.0), color),
    ];
    draw_mesh(&Mesh {
        vertices: v.to_vec(),
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: tex.cloned(),
    });
}

/// A soft light turned to the camera.
pub fn glow(g: &Gfx, cam: &Cam, c: Vec3, r: f32, col: Color, a: f32) {
    if a <= 0.004 {
        return;
    }
    sprite(Some(&g.dot), c, cam.right * r, -cam.up * r, with_a(col, a));
}

/// A band of light between two points, turned to the camera.
pub fn band(cam: &Cam, a: Vec3, b: Vec3, w: f32, col: Color) {
    let d = b - a;
    let side = d.cross(cam.eye - a).normalize_or_zero() * w / 2.0;
    let v = [
        Vertex::new2(a - side, vec2(0.0, 0.5), col),
        Vertex::new2(b - side, vec2(1.0, 0.5), col),
        Vertex::new2(b + side, vec2(1.0, 0.5), col),
        Vertex::new2(a + side, vec2(0.0, 0.5), col),
    ];
    draw_mesh(&Mesh {
        vertices: v.to_vec(),
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: None,
    });
}

/// The 3D state kept between frames.
#[derive(Default)]
pub struct D3 {
    /// the last frame was drawn in 3D
    pub on: bool,
    /// the detailed looks
    pub hd: bool,
    /// the high-poly look (with `hd`)
    pub fine: bool,
    gpu: Option<Gpu>,
    add: Option<Material>,
    land: Land,
    batch: Batch,
    pub cam: Option<Cam>,
    yaw: HashMap<u32, f32>,
    /// draws only where something stands in front
    xray: Option<Material>,
    /// the hero's triangles, coloured for the x-ray
    hero: Vec<Vertex>,
    /// the darkening at the edges of the screen
    shade: Option<Texture2D>,
    /// houses under roofs this frame: x, y, w, h (their insides darken as
    /// their walls do)
    pub covers: Vec<(i32, i32, i32, i32)>,
    /// the abilities' bodies this frame: solid things and light
    fx_solid: Batch,
    fx_light: Batch,
}

/// Turns the depth test of the default material on or off.
fn set_depth_test(on: bool) {
    unsafe { get_internal_gl() }.quad_gl.depth_test(on);
}

/// The hero's faces pulled a little toward the camera, so the hero never
/// hides itself in the x-ray.
const XRAY_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    vec4 w = Model * vec4(position, 1.0);
    gl_Position = Projection * w;
    gl_Position.z -= 0.004 * gl_Position.w;
    color = color0 / 255.0;
    uv = texcoord;
}"#;

impl D3 {
    /// Darkens the edges of the screen a little, as a painting's.
    fn vignette(&mut self) {
        let t = self.shade.get_or_insert_with(|| {
            let n = 128u16;
            let mut img = Image::gen_image_color(n, n, Color::new(0.0, 0.0, 0.0, 0.0));
            for y in 0..n as u32 {
                for x in 0..n as u32 {
                    let (u, v) = (
                        x as f32 / (n - 1) as f32 * 2.0 - 1.0,
                        y as f32 / (n - 1) as f32 * 2.0 - 1.0,
                    );
                    let r = (u * u + v * v).sqrt();
                    let a = ((r - 0.6) / 0.85).clamp(0.0, 1.0);
                    img.set_pixel(x, y, Color::new(0.06, 0.05, 0.1, a * a * 0.55));
                }
            }
            let t = Texture2D::from_image(&img);
            t.set_filter(FilterMode::Linear);
            t
        });
        draw_texture_ex(
            t,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(screen_width(), screen_height())),
                ..Default::default()
            },
        );
    }

    fn materials(&mut self) {
        if self.add.is_some() {
            return;
        }
        self.gpu = Some(Gpu::new());
        let add = load_material(
            ShaderSource::Glsl {
                vertex: crate::gfx::VERTEX,
                fragment: crate::gfx::FRAGMENT,
            },
            MaterialParams {
                // (without depth writes miniquad does not test the depth
                // either: lights shine over everything, as in 2D)
                pipeline_params: PipelineParams {
                    depth_test: Comparison::LessOrEqual,
                    depth_write: false,
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::One,
                    )),
                    alpha_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Zero,
                        BlendFactor::One,
                    )),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .expect("additive 3d material");
        let xray = load_material(
            ShaderSource::Glsl {
                vertex: XRAY_VERTEX,
                fragment: crate::gfx::FRAGMENT,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    // only the faces turned to the camera, and only where
                    // something else stands in front of them
                    cull_face: miniquad::CullFace::Back,
                    depth_test: Comparison::Greater,
                    // miniquad tests the depth only for pipelines that write
                    // it; the x-ray is the last thing drawn in 3D anyway
                    depth_write: true,
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    )),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .expect("x-ray material");
        self.add = Some(add);
        self.xray = Some(xray);
    }
}

/// The ground under a creature: its height.
fn ground(l: &ratas_core::world::Level, x: f32, y: f32, hd: bool) -> f32 {
    Land::height(l, x, y, hd)
}

impl WorldRenderer {
    /// The world point under a screen position, in either view.
    pub fn world_at(&self, sx: f32, sy: f32) -> (f32, f32) {
        if self.d3.on {
            if let Some(p) = self.d3.cam.and_then(|c| c.ground(sx, sy, 0.0)) {
                return (p.x, p.y);
            }
        }
        self.view.world(sx, sy)
    }

    /// Draws the world of a scene in 3D.
    pub fn draw3d(&mut self, g: &mut Gfx, sc: &Scene, area: Rect) {
        let hd = self.d3.hd;
        let fine = hd && self.d3.fine;
        self.d3.on = true;
        let l = sc.level;
        self.tops.clear();
        self.spells.prepare(g);
        let (w, h) = (screen_width(), screen_height());
        // the flat view stays valid for whoever asks it
        self.view = self.make_view(l, self.cam, area, g.s);
        let zoom = if self.zoom_cur == 0.0 {
            ZOOM[self.zoom_idx]
        } else {
            self.zoom_cur
        };
        // the detailed look sees from lower and narrower, nearly isometric
        let (pitch, fov, far) = if hd {
            (0.92, 0.5, 1.33)
        } else {
            (PITCH, FOV, 1.0)
        };
        let dist = 860.0 / zoom * far;
        let (kx, ky) = self.spells.shake_offset(1.0);
        let (gx, gy) = (self.cam.0 + kx, self.cam.1 + ky);
        let target = vec3(gx, ground(l, gx, gy, hd) + 0.5, gy);
        let eye = target + vec3(0.0, pitch.sin() * dist, pitch.cos() * dist);
        let cam3 = Camera3D {
            position: eye,
            target,
            up: Vec3::Y,
            fovy: fov,
            // nothing comes nearer than a few tiles: the depth stays fine
            // enough to part the water from its shore
            z_near: (dist * 0.25).max(1.0),
            z_far: dist * 3.0 + REACH * 2.0,
            ..Default::default()
        };
        let mat = cam3.matrix();
        let fwd = (target - eye).normalize();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        let cam = Cam {
            mat,
            inv: mat.inverse(),
            eye,
            right,
            up,
            w,
            h,
        };
        self.d3.cam = Some(cam);
        // the tiles in sight: the corners of the screen on the ground
        let (mut x0, mut y0, mut x1, mut y1) = (gx - 8.0, gy - 8.0, gx + 8.0, gy + 8.0);
        for (px, py) in [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)] {
            if let Some(p) = cam.ground(px, py, -0.4) {
                x0 = x0.min(p.x);
                y0 = y0.min(p.y);
                x1 = x1.max(p.x);
                y1 = y1.max(p.y);
            }
        }
        let tx0 = (x0.max(gx - REACH) - 2.0).floor().max(0.0) as i32;
        let ty0 = (y0.max(gy - REACH) - 3.0).floor().max(0.0) as i32;
        let tx1 = (x1.min(gx + REACH) + 2.0).ceil().min(l.w as f32 - 1.0) as i32;
        let ty1 = (y1.min(gy + REACH) + 2.0).ceil().min(l.h as f32 - 1.0) as i32;
        let vis = self.visible(sc).clone();
        let (hx, hy) = match self.ents.get(&sc.you) {
            Some(me) => (me.x, me.y),
            None => (sc.snap.you.x, sc.snap.you.y),
        };
        self.d3
            .land
            .prepare(l, sc.level_ver, sc.explored, (tx0, ty0, tx1, ty1), hd, fine);
        self.d3.covers = self
            .d3
            .land
            .roofs((hx, hy))
            .1
            .iter()
            .map(|h| (h.x, h.y, h.w, h.h))
            .collect();
        let (dw, dh) = (tx1 - tx0 + 3, ty1 - ty0 + 3);
        let dark = self.dark_map(sc, &vis, tx0 - 1, ty0 - 1, dw, dh);
        self.d3.materials();

        set_camera(&cam3);
        // the land and the creatures, lit and darkened by the shader
        let tod = sc.snap.time_of_day;
        let (tint, torch, torch_c) = light_of(sc, l.lit, tod, self.t);
        let haze = if hd && l.lit { 0.55 } else { 0.0 };
        let light = Light {
            mvp: mat,
            dark_rect: vec4(
                (tx0 - 1) as f32,
                (ty0 - 1) as f32,
                1.0 / dw as f32,
                1.0 / dh as f32,
            ),
            tint,
            torch: vec4(hx, hy, torch.x, torch.y),
            torch_color: vec4(torch_c.x, torch_c.y, torch_c.z, ground(l, hx, hy, hd) + 1.3),
            eye: eye.extend(if fine {
                2.0
            } else if hd {
                1.0
            } else {
                0.0
            }),
            sun: if hd {
                lowpoly::sun_hd()
            } else {
                lowpoly::sun()
            }
            .extend(self.t),
            fog: vec4(0.66, 0.74, 0.86, haze),
            fog_range: vec4(dist * 1.25, dist * 2.7, 0.0, 0.0),
        };
        let none = Batch::default();
        if let Some(gpu) = self.d3.gpu.as_mut() {
            gpu.draw(&light, &dark, &self.d3.land.grounds(), &none, Slot::Things);
        }
        // what lies on the ground: shadows, rings, marks of abilities
        set_depth_test(false);
        self.draw_marks3d(sc);
        self.spells
            .draw3d_ground(g, &|x, y| Land::height(l, x, y, hd));
        set_depth_test(true);
        // what stands on it
        let tops = self.draw_beings3d(sc, &cam);
        if let Some(gpu) = self.d3.gpu.as_mut() {
            let mut meshes = self.d3.land.standing();
            meshes.extend(self.d3.land.roofs((hx, hy)).0);
            gpu.draw(&light, &dark, &meshes, &self.d3.batch, Slot::Creatures);
        }
        self.tops = tops;
        // the abilities' bodies: solid things lit, then light added
        let mut solid = std::mem::take(&mut self.d3.fx_solid);
        let mut shine = std::mem::take(&mut self.d3.fx_light);
        for b in [&mut solid, &mut shine] {
            b.clear();
            b.hd = hd;
            b.fine = fine;
        }
        self.spells.forms3d(
            &mut Out {
                solid: &mut solid,
                light: &mut shine,
            },
            &cam,
            &self.ents,
            &|x, y| Land::height(l, x, y, hd),
        );
        if let Some(gpu) = self.d3.gpu.as_mut() {
            gpu.draw(&light, &dark, &[], &solid, Slot::Things);
            gpu.draw_light(&light, &dark, &shine);
        }
        self.d3.fx_solid = solid;
        self.d3.fx_light = shine;
        // things lying about
        self.draw_items3d(g, &cam, sc);
        // abilities, sparks and lights
        let add = self.d3.add.clone().unwrap();
        self.spells
            .draw3d(g, &cam, &add, &self.ents, &|x, y| Land::height(l, x, y, hd));
        self.draw_lights3d(g, sc, &vis, &cam, &add);
        // the hero seen through what hides them
        if !self.d3.hero.is_empty() && !sc.snap.you.dead {
            let xray = self.d3.xray.clone().unwrap();
            gl_use_material(&xray);
            draw_vertices(&self.d3.hero);
            gl_use_default_material();
        }
        set_default_camera();

        if hd {
            self.d3.vignette();
        }
        let scale = cam.scale(target - vec3(0.0, 0.5, 0.0));
        let pr = |x: f32, y: f32, hh: f32| -> (f32, f32) {
            let p = vec3(x, ground(l, x, y, hd) + hh, y);
            cam.project(p).map_or((-1000.0, -1000.0), |v| (v.x, v.y))
        };
        self.draw_overlays(g, sc, area, &pr, scale, true);
    }

    /// Builds and draws the creatures; returns the screen height of their heads.
    fn draw_beings3d(&mut self, sc: &Scene, cam: &Cam) -> HashMap<u32, f32> {
        let hd = self.d3.hd;
        let l = sc.level;
        let t = self.t;
        let dt = self.dt;
        let mut tops = HashMap::new();
        let mut b = std::mem::take(&mut self.d3.batch);
        b.clear();
        b.hd = hd;
        b.fine = hd && self.d3.fine;
        self.d3.hero.clear();
        let ids: Vec<u32> = self.ents.keys().copied().collect();
        for id in ids {
            let st = &self.ents[&id];
            let e = &st.view;
            if st.alpha < 0.02 || e.kind == KIND_PROJECTILE || e.kind == KIND_ITEM {
                continue;
            }
            let name = resolve(&e.model, &e.def, e.glyph, e.kind);
            let nv = variants(&name);
            let variant = if nv > 1 {
                (e.id as i32).rem_euclid(nv)
            } else {
                0
            };
            let moving = st.vx.abs() + st.vy.abs() > 0.05;
            // turn toward the way it walks, or where it looks
            let want = if moving && !e.dead {
                st.vy.atan2(st.vx)
            } else {
                st.facing
            };
            let yaw = self.d3.yaw.entry(id).or_insert(want);
            let mut d = (want - *yaw).rem_euclid(TAU);
            if d > PI {
                d -= TAU;
            }
            *yaw += d * (1.0 - (-dt * 12.0).exp());
            let yaw = *yaw;
            let l_off = st.lunge * (1.0 - st.lunge) * 4.0 * 0.22;
            let (x, y) = (
                st.x + st.facing.cos() * l_off,
                st.y + st.facing.sin() * l_off,
            );
            let gh = ground(l, x, y, hd);
            let mut k = 1.05;
            if e.boss {
                k *= 1.5;
            } else if e.radius >= 0.4 {
                k *= 1.3;
            }
            let lift = if floats(&name) && !e.dead {
                0.2 + (t * 2.6 + e.id as f32).sin() * 0.06
            } else {
                0.0
            };
            let mut tint = Color::new(1.0, 1.0, 1.0, 1.0);
            if e.status & STATUS_CHILLED != 0 {
                tint = Color::new(0.72, 0.88, 1.0, 1.0);
            } else if e.status & STATUS_POISONED != 0 {
                tint = Color::new(0.8, 1.0, 0.72, 1.0);
            }
            if e.status & STATUS_STEALTH != 0 {
                tint = Color::new(0.35, 0.35, 0.42, 1.0);
            } else if e.status & STATUS_ILLUSION != 0 {
                tint = Color::new(0.85, 0.75, 1.0, 1.0);
            }
            if e.dead {
                tint = Color::new(0.6, 0.55, 0.55, 1.0);
            }
            if hd {
                // in the shade of trees and walls
                let k = 1.0 - 0.32 * scenery::shade_at(l, x, y);
                tint = Color::new(tint.r * k, tint.g * k, tint.b * (0.3 + 0.7 * k), 1.0);
            }
            // fading in and out of sight: into the dark
            let fade = st.alpha.clamp(0.0, 1.0);
            tint = Color::new(tint.r * fade, tint.g * fade, tint.b * fade, 1.0);
            let paint = Paint {
                tint,
                flash: (st.flash / 0.16).clamp(0.0, 1.0) * 0.8,
            };
            let pose = Pose {
                walk: st.gait,
                run: st.run,
                moving,
                lunge: st.lunge,
                t,
                seed: id,
            };
            let base = vec3(x, gh + lift, y);
            let mut xf = Xf::yaw(base, yaw).scaled(k);
            if e.dead {
                // lying on the side
                xf = Xf::yaw(base + vec3(0.0, 0.12 * k, 0.0), yaw)
                    .scaled(k)
                    .then(Vec3::ZERO, Mat3::from_rotation_x(FRAC_PI_2_F));
            }
            let c = crate::art::hex(&e.color);
            let from = b.i.len();
            // its materials go with it
            b.frame = Some((base, yaw));
            b.mat = 0;
            let height = beings::being(&mut b, &name, c, variant, &e.gear, xf, &pose, &paint);
            b.frame = None;
            if id == sc.you {
                self.d3.hero = b.plain(from);
                for v in &mut self.d3.hero {
                    v.color = [255, 226, 150, 110];
                }
            }
            let head = if e.dead { 0.3 } else { height * k + lift };
            if let Some(p) = cam.project(vec3(x, gh + head + 0.06, y)) {
                tops.insert(id, p.y);
            }
            if b.v.len() > 1_500_000 {
                break;
            }
        }
        self.d3.batch = b;
        tops
    }

    /// Shadows under creatures and the rings that tell heroes apart.
    fn draw_marks3d(&mut self, sc: &Scene) {
        let hd = self.d3.hd;
        let l = sc.level;
        let mut b = std::mem::take(&mut self.d3.batch);
        b.clear();
        for st in self.ents.values() {
            let e = &st.view;
            if st.alpha < 0.05 || e.kind == KIND_PROJECTILE {
                continue;
            }
            let gh = ground(l, st.x, st.y, hd) + 0.025;
            let c = vec3(st.x, gh, st.y);
            // in the detailed look the shadow falls away from the sun
            let away = if hd {
                let s = lowpoly::sun_hd();
                -vec3(s.x, 0.0, s.z).normalize() * 0.12
            } else {
                Vec3::ZERO
            };
            let mut r = 0.3;
            if e.boss {
                r *= 1.5;
            } else if e.radius >= 0.4 {
                r *= 1.3;
            }
            if e.status & STATUS_STEALTH == 0 {
                let shade = Color::new(0.0, 0.0, 0.0, 0.32 * st.alpha);
                if hd {
                    b.soft_disc(
                        c + away,
                        r * 1.35,
                        r * 1.05,
                        20,
                        with_a(shade, 0.55 * st.alpha),
                    );
                } else {
                    b.disc(c, r, r * 0.8, 10, shade, 0.0);
                }
            }
            let ring = if e.id == sc.you {
                Some(Color::new(1.0, 0.84, 0.43, 0.85))
            } else if e.ally {
                Some(Color::new(0.43, 0.9, 0.47, 0.85))
            } else if e.kind == KIND_PLAYER && e.hostile {
                Some(Color::new(0.94, 0.31, 0.27, 0.85))
            } else if e.kind == KIND_PLAYER {
                Some(with_a(col(&e.color), 0.85))
            } else {
                None
            };
            if let (Some(rc), false) = (ring, e.dead) {
                b.ring(
                    c + vec3(0.0, 0.005, 0.0),
                    r + 0.08,
                    0.05,
                    20,
                    with_a(rc, rc.a * st.alpha),
                    0.0,
                );
            }
        }
        b.flush_plain();
        self.d3.batch = b;
    }

    /// Loot on the ground: its picture standing up, a glow of its rarity.
    fn draw_items3d(&self, g: &mut Gfx, cam: &Cam, sc: &Scene) {
        let hd = self.d3.hd;
        let l = sc.level;
        let add = self.d3.add.as_ref().unwrap();
        for st in self.ents.values() {
            let e = &st.view;
            if e.kind != KIND_ITEM || st.alpha < 0.05 {
                continue;
            }
            let al = st.alpha;
            let bob = (self.t * 3.0 + e.id as f32).sin() * 0.05;
            let p = vec3(st.x, ground(l, st.x, st.y, hd) + 0.32 + bob, st.y);
            let mut c = col(&e.color);
            if e.rarity >= 1 {
                if let Some(d) = content::db().item(&e.def) {
                    c = col(&d.color);
                }
            }
            let ccol = format!(
                "#{:02x}{:02x}{:02x}",
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8
            );
            gl_use_material(add);
            glow(g, cam, p, 0.42, c, 0.3 * al);
            if e.rarity >= 1 {
                let rc = col(rarity_color(e.rarity as i32));
                let pulse = 0.75 + 0.25 * (self.t * 3.0 + e.id as f32).sin();
                glow(
                    g,
                    cam,
                    p,
                    0.45 + 0.12 * e.rarity as f32,
                    rc,
                    (0.18 + 0.1 * e.rarity as f32) * pulse * al,
                );
                if e.rarity >= 3 {
                    for i in 1..=4 {
                        glow(
                            g,
                            cam,
                            p + vec3(0.0, 0.35 * i as f32, 0.0),
                            0.3,
                            rc,
                            0.16 * pulse / i as f32 * al,
                        );
                    }
                }
            }
            gl_use_default_material();
            if let Some(icon) = g.icon(&e.def, e.glyph, &ccol) {
                let k = 0.024 / crate::gfx::ART;
                let (hw, hh) = (icon.width() * k / 2.0, icon.height() * k / 2.0);
                sprite(
                    Some(&icon),
                    p,
                    cam.right * hw,
                    -Vec3::Y * hh,
                    Color::new(1.0, 1.0, 1.0, al),
                );
            }
        }
    }

    /// The lights of tiles (fires, lava, lamps, stairs), flashes, beams,
    /// sparks and the motes of the air.
    fn draw_lights3d(&mut self, g: &Gfx, sc: &Scene, vis: &Vis, cam: &Cam, add: &Material) {
        let hd = self.d3.hd;
        let l = sc.level;
        let db = content::db();
        gl_use_material(add);
        let mut sparks = vec![];
        let mut lava = 0;
        for (x, y) in vis.cells() {
            if !l.inside(x, y) {
                continue;
            }
            let def = db.tile(l.at(x, y));
            let gh = ground(l, x as f32 + 0.5, y as f32 + 0.5, hd);
            let p = vec3(x as f32 + 0.5, gh + 0.35, y as f32 + 0.5);
            if def.damage > 0.0 && lava < 220 {
                lava += 1;
                glow(
                    g,
                    cam,
                    p - vec3(0.0, 0.2, 0.0),
                    1.3,
                    col(&def.fg),
                    0.13 * (0.85 + 0.15 * (self.t * 2.0 + x as f32).sin()),
                );
            } else if matches!(
                def.interact.as_str(),
                "stairs_down" | "stairs_up" | "dungeon"
            ) {
                glow(
                    g,
                    cam,
                    p,
                    1.2,
                    col(&def.fg),
                    0.14 + 0.05 * (self.t * 2.0).sin(),
                );
            } else if def.key == "altar" {
                glow(g, cam, p + vec3(0.0, 0.3, 0.0), 1.6, col(&def.fg), 0.22);
            } else if !def.light.is_empty() {
                let lc = col(&def.light);
                if def.key == "campfire" || def.key == "brazier" {
                    let fl = 0.85
                        + 0.15 * (self.t * 9.0 + x as f32).sin() * (self.t * 6.3 + y as f32).sin();
                    let hf = if def.key == "brazier" { 0.75 } else { 0.3 };
                    let fp = vec3(p.x, gh + hf, p.z);
                    glow(g, cam, fp, 2.6, lc, 0.3 * fl);
                    glow(
                        g,
                        cam,
                        fp,
                        0.55,
                        Color::from_rgba(255, 230, 140, 255),
                        0.55 * fl,
                    );
                    sparks.push((x, y, hf));
                } else {
                    let hl = if def.key == "lamp_post" { 1.28 } else { 0.4 };
                    glow(
                        g,
                        cam,
                        vec3(p.x, gh + hl, p.z),
                        1.5,
                        lc,
                        0.2 + 0.05 * (self.t * 2.0 + (x * 7 + y) as f32).sin(),
                    );
                }
            }
        }
        for (x, y, hf) in sparks {
            if self.rf() < 0.3 {
                let p = Particle {
                    x: x as f32 + 0.5 + (self.rf() - 0.5) * 0.3,
                    y: y as f32 + 0.5,
                    vx: (self.rf() - 0.5) * 0.3,
                    vy: -1.2 - self.rf(),
                    t: 0.0,
                    life: 0.5 + self.rf() * 0.5,
                    size: 0.07,
                    grav: -0.4 - hf,
                    col: Color::from_rgba(255, 150, 50, 255),
                    glow: true,
                };
                self.parts.push(p);
            }
        }
        // flashes: rings over the ground, glows
        let mut b = std::mem::take(&mut self.d3.batch);
        b.clear();
        for fl in &self.lights {
            let k = fl.t / fl.life;
            let gh = ground(l, fl.x, fl.y, hd);
            let c = vec3(fl.x, gh + 0.06, fl.y);
            if fl.ring {
                let r = fl.radius * (0.3 + 0.7 * k.sqrt());
                b.ring(c, r, 0.12, 40, with_a(fl.col, 0.8 * (1.0 - k)), 0.0);
                b.ring(c, r * 0.96, 0.04, 40, with_a(WHITE, 0.6 * (1.0 - k)), 0.0);
                b.disc(
                    c,
                    r * 1.05,
                    r * 1.05,
                    24,
                    with_a(fl.col, 0.12 * (1.0 - k)),
                    0.0,
                );
            } else {
                glow(
                    g,
                    cam,
                    c + vec3(0.0, 0.5, 0.0),
                    fl.radius,
                    fl.col,
                    0.6 * (1.0 - k),
                );
            }
        }
        b.flush_plain();
        self.d3.batch = b;
        // beams
        for bm in &self.beams {
            let a = 1.0 - bm.t / bm.life;
            let p = vec3(bm.x, ground(l, bm.x, bm.y, hd) + 0.6, bm.y);
            let q = vec3(bm.x2, ground(l, bm.x2, bm.y2, hd) + 0.6, bm.y2);
            band(cam, p, q, 0.14, with_a(bm.col, 0.5 * a));
            band(cam, p, q, 0.05, with_a(WHITE, 0.8 * a));
            let n = (p.distance(q) / 0.5).ceil().max(1.0) as i32;
            for i in 0..=n {
                let k = i as f32 / n as f32;
                glow(g, cam, p.lerp(q, k), 0.35, bm.col, 0.35 * a);
            }
        }
        // sparks and bursts: the flat particles fly up from the ground
        for p in &self.parts {
            let a = 1.0 - p.t / p.life;
            // the 2D particles rise on the screen: here they rise in the air
            let rise = (p.t * 1.2).min(1.5);
            let pos = vec3(p.x, ground(l, p.x, p.y, hd) + 0.5 + rise * 0.5, p.y);
            if p.glow {
                glow(g, cam, pos, p.size * 1.6, p.col, a);
            } else {
                glow(g, cam, pos, p.size * 0.8, p.col, a * 0.8);
            }
        }
        for p in &self.ambient {
            let blink = 0.5 + 0.5 * (p.t * 3.0 + p.x).sin();
            let pos = vec3(
                p.x,
                ground(l, p.x, p.y, hd) + 0.6 + (p.t + p.x).sin() * 0.1,
                p.y,
            );
            if p.glow {
                glow(
                    g,
                    cam,
                    pos,
                    0.22,
                    p.col,
                    0.8 * blink * (p.life - p.t).min(1.0),
                );
            } else {
                glow(g, cam, pos, 0.08, p.col, 0.25 * blink);
            }
        }
        // the glows of creatures: bosses, shields, fire on them
        for st in self.ents.values() {
            let e = &st.view;
            if st.alpha < 0.3 || e.dead || e.kind == KIND_ITEM || e.kind == KIND_PROJECTILE {
                continue;
            }
            let p = vec3(st.x, ground(l, st.x, st.y, hd) + 0.55, st.y);
            if e.boss {
                glow(
                    g,
                    cam,
                    p,
                    1.4,
                    col(&e.color),
                    0.16 + 0.06 * (self.t * 3.0).sin(),
                );
            }
            if e.status & STATUS_SHIELDED != 0 {
                glow(
                    g,
                    cam,
                    p,
                    0.9,
                    Color::from_rgba(255, 230, 140, 255),
                    0.22 + 0.05 * (self.t * 5.0).sin(),
                );
            }
            if e.status & STATUS_BURNING != 0 {
                glow(
                    g,
                    cam,
                    p,
                    0.7,
                    Color::from_rgba(255, 120, 30, 255),
                    0.3 + 0.1 * (self.t * 13.0).sin(),
                );
            }
        }
        gl_use_default_material();
    }
}

const FRAC_PI_2_F: f32 = std::f32::consts::FRAC_PI_2;

/// The light of the moment: a tint over the land (day, dusk, night), the
/// hero's torch (radius, strength) and its colour.
fn light_of(sc: &Scene, lit: bool, tod: f64, t: f32) -> (Vec4, Vec2, Vec4) {
    let flicker = 0.92 + 0.08 * (t * 11.0).sin() * (t * 7.3).sin();
    let warm = vec4(1.0, 0.68, 0.4, 1.0);
    let vision = (sc.snap.you.vision as f32).max(1.0);
    if !lit {
        // dungeons: the dark map darkens the far, the torch warms the near
        return (
            vec4(0.92, 0.92, 0.98, 1.0),
            vec2(vision * 0.9, 0.55 * flicker),
            warm,
        );
    }
    let day = daylight(tod) as f32;
    let night = 1.0 - day;
    let mut tint = vec3(1.0, 1.0, 1.0).lerp(vec3(0.32, 0.4, 0.68), night * 0.85);
    if day > 0.0 && day < 1.0 {
        let dusk = 1.0 - (2.0 * day - 1.0).abs();
        tint = tint.lerp(vec3(1.05, 0.72, 0.5), 0.3 * dusk);
    }
    let torch = if sc.snap.you.dead {
        0.0
    } else {
        0.7 * night * flicker
    };
    (tint.extend(1.0), vec2(6.0, torch), warm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratas_core::world::{Bitset, Level};

    fn finite(b: &Batch) -> bool {
        b.v.iter().all(|v| v.pos().is_finite()) && b.i.iter().all(|&k| (k as usize) < b.v.len())
    }

    /// Every creature (and every class) is built of some triangles.
    #[test]
    fn every_creature_has_a_3d_model() {
        let db = content::db();
        let pose = Pose {
            walk: 0.7,
            run: 0.5,
            moving: true,
            lunge: 0.5,
            t: 1.0,
            seed: 3,
        };
        let paint = Paint {
            tint: WHITE,
            flash: 0.0,
        };
        let mut names: Vec<String> =
            db.b.monsters
                .iter()
                .map(|m| resolve(&m.model, &m.key, m.glyph.chars().next().unwrap_or('?'), 2))
                .collect();
        names.extend(
            db.b.classes
                .iter()
                .map(|c| resolve(&c.model, &c.key, '@', KIND_PLAYER)),
        );
        for (hd, fine) in [(false, false), (true, false), (true, true)] {
            for n in &names {
                let mut b = Batch::new(hd);
                b.fine = fine;
                b.frame = Some((Vec3::ZERO, 0.7));
                let c = crate::art::rgb(200, 120, 80);
                let h = beings::being(&mut b, n, c, 0, &[], Xf::at(Vec3::ZERO), &pose, &paint);
                assert!(b.v.len() > 30 && h > 0.1, "{n}: {} vertices", b.v.len());
                assert!(finite(&b), "{n}");
                // the detailed figures stay light enough to draw dozens
                assert!(b.i.len() / 3 < 40_000, "{n}: {} faces", b.i.len() / 3);
                if hd && !matches!(n.as_str(), "slime" | "ice_elemental") {
                    // made of something, and it moves with the creature
                    let made = b.v.iter().filter(|v| v.m[2] != 0).count();
                    assert!(made * 3 > b.v.len() * 2, "{n}: {made} of {}", b.v.len());
                    assert!(b.v.iter().all(|v| v.t[3] != 0), "{n}");
                }
            }
        }
    }

    /// Every tile builds next to every other, on a little field of all of them.
    #[test]
    fn the_land_builds_from_every_tile() {
        let n = content::db().b.tiles.len();
        let (w, h) = (terrain::CHUNK, terrain::CHUNK);
        let mut l = Level::new("test", "test", w, h, 0);
        l.lit = true;
        for (i, t) in l.tiles.iter_mut().enumerate() {
            *t = ((i * 7 + i / w as usize) % n) as u8;
        }
        let mut seen = Bitset::new((w * h) as usize);
        for i in 0..(w * h) as usize {
            seen.set(i);
        }
        for (hd, fine) in [(false, false), (true, false), (true, true)] {
            let (mut ground, mut props) = (Batch::new(hd), Batch::new(hd));
            ground.fine = fine;
            props.fine = fine;
            terrain::build_chunk(&mut ground, &mut props, &l, &seen, 0, 0);
            assert!(ground.v.len() > (w * h) as usize, "{}", ground.v.len());
            assert!(!props.v.is_empty());
            assert!(finite(&ground) && finite(&props));
        }
        assert!(Land::height(&l, 3.5, 4.5, false).is_finite());
        assert!(Land::height(&l, 3.5, 4.5, true).is_finite());
    }
}
