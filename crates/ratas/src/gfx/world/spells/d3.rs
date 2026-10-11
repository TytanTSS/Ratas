//! The looks of abilities in the 3D world. Their shapes, missiles and the
//! little things they scatter are bodies (see forms.rs); what stays a
//! picture is soft light: glows, sparks and streaks turned to the camera,
//! and the marks lying on the ground.

use super::*;
use crate::gfx::world::d3::{band, glow, plane, sprite, with_plane, Cam, TS};

/// A view that puts a world point (ax, ay) at the origin of a drawing.
fn anchored(ax: f32, ay: f32) -> View {
    View {
        ox: -ax * TS,
        oy: -ay * TS,
        ts: TS,
        tx0: 0,
        ty0: 0,
        tx1: 0,
        ty1: 0,
    }
}

impl Spells {
    /// The marks on the ground (scorches, frost, cracks, puddles, glyphs,
    /// webs). Drawn after the ground and before what stands on it, without
    /// the depth test, so the bumps of the land do not cut through them.
    pub fn draw3d_ground(&self, g: &Gfx, height: &dyn Fn(f32, f32) -> f32) {
        for d in &self.decals {
            let o = vec3(d.x, height(d.x, d.y) + 0.03, d.y);
            with_plane(plane(o, Vec3::X, Vec3::Z), || {
                self.draw_decal(g, &anchored(d.x, d.y), d);
            });
        }
    }

    /// The soft light of the abilities: glows, sparks and streaks, and the
    /// light every missile sheds around it.
    pub fn draw3d(
        &self,
        g: &Gfx,
        cam: &Cam,
        add: &Material,
        ents: &HashMap<u32, EntState>,
        height: &dyn Fn(f32, f32) -> f32,
    ) {
        gl_use_material(&g.add);
        for s in &self.shapes {
            if s.delay > 0.0 || s.k != ShK::Glow {
                continue;
            }
            let (ax, ay) = (s.ax, s.ay);
            let gh = height(ax + s.x, ay + s.y);
            let m = plane(vec3(ax, gh + 0.05, ay), cam.right, -Vec3::Y);
            with_plane(m, || self.draw_shape(g, &anchored(ax, ay), s));
        }
        gl_use_default_material();
        self.parts3d(cam, false, height);
        gl_use_material(add);
        self.parts3d(cam, true, height);
        for (id, st) in ents {
            if st.view.kind != KIND_PROJECTILE || st.alpha < 0.02 {
                continue;
            }
            if let Some(pj) = self.proj.get(id) {
                let p = vec3(st.x, height(st.x, st.y) + 0.65, st.y);
                glow(g, cam, p, 0.9 * pj.lk.big, pj.lk.c, 0.35 * st.alpha);
            }
        }
        gl_use_default_material();
    }

    fn parts3d(&self, cam: &Cam, add: bool, height: &dyn Fn(f32, f32) -> f32) {
        let k = 1.0 / SPX as f32;
        for p in &self.parts {
            if p.add != add || p.delay > 0.0 || Self::bodily(p.spr) {
                continue;
            }
            let u = (p.t / p.life).clamp(0.0, 1.0);
            let mut a = p.alpha
                * if p.fin > 0.0 {
                    (u / p.fin).min(1.0)
                } else {
                    1.0
                };
            a *= match p.spr {
                Spr::Dot | Spr::Streak => 1.0 - u,
                _ => 1.0 - u * u * u,
            };
            if a <= 0.01 {
                continue;
            }
            let c = mix_c(p.c0, p.c1, u);
            let s = p.s0 + (p.s1 - p.s0) * u;
            let wx = if p.wob != 0.0 {
                (p.t * 6.0 + p.rot * 3.0 + p.ax).sin() * p.wob
            } else {
                0.0
            };
            let (x, y) = (p.ax + p.x + wx, p.ay + p.y);
            let pos = vec3(x, height(p.ax, p.ay) + p.z.max(0.0), y);
            match p.spr {
                Spr::Dot => {
                    sprite(
                        Some(&self.tex[Spr::Dot as usize]),
                        pos,
                        cam.right * s,
                        -cam.up * s,
                        with_a(c, a),
                    );
                }
                Spr::Streak => {
                    let d = vec3(p.vx, p.vz, p.vy);
                    let l = d.length().max(0.001);
                    band(cam, pos, pos - d / l * s, 0.04, with_a(c, a));
                }
                spr => {
                    let t = &self.tex[spr as usize];
                    let (w, h) = (
                        t.width() / ART * k * s / 2.0,
                        t.height() / ART * k * s / 2.0,
                    );
                    // turned along the flight as seen on the screen
                    let rot = if p.face {
                        let d = vec3(p.vx, p.vz, p.vy);
                        let (sx, sy) = (d.dot(cam.right), -d.dot(cam.up));
                        sy.atan2(sx) + p.rot
                    } else {
                        p.rot
                    };
                    // the picture's axes on the screen, turned by rot
                    let (cs, sn) = (rot.cos(), rot.sin());
                    let ex = (cam.right * cs - cam.up * sn) * w;
                    let ey = (cam.right * -sn - cam.up * cs) * h;
                    sprite(Some(t), pos, ex, ey, with_a(c, a));
                }
            }
        }
    }
}
