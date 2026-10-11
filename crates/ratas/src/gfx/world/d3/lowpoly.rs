//! The geometry of the 3D world: triangles gathered into indexed batches
//! with their normals, colours, materials and a few flags; the shader in
//! gpu.rs lights them by the sun and darkens them by the night and the
//! unseen. In the low-poly look the shapes are faceted boxes, prisms, cones
//! and gems; in the detailed looks the same calls build round shapes of many
//! more faces (and shapes.rs adds the ones only those looks use), cut finer
//! still in the high-poly look.

use std::f32::consts::TAU;
use std::sync::OnceLock;

use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;

use super::materials::scale_of;

/// Vertices drawn at once through macroquad (the draw call capacity is set
/// in app.rs).
pub const CALL: usize = 30_000;

/// A flag of a vertex: not lit by the sun (marks, flat colours).
pub const UNLIT: u8 = 1;
/// A flag of a vertex: glossy, with a sharp highlight (metal, ice).
pub const SHINY: u8 = 2;
/// A flag of a vertex: the surface of water, rippling.
pub const WATER: u8 = 4;
/// A flag of a vertex: lit smooth, not carved into flat faces (the ground).
pub const SMOOTH: u8 = 8;
/// A flag of a vertex of light (with UNLIT): a body of light, bright through
/// its depth and fading at its edges.
pub const HALO: u8 = 16;
/// A flag of a vertex of light (with UNLIT): a shell of light, bright at its
/// edges and clear in the middle (a bubble).
pub const RIM: u8 = 32;

/// A vertex as the shader takes it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V {
    pub p: [f32; 3],
    pub c: [u8; 4],
    /// the normal (x, y, z as 1..255, 128 is 0) and how much it glows
    pub n: [u8; 4],
    /// how much it sways in the wind, its flags, its material and how
    /// finely the material repeats (16: once along a tile)
    pub m: [u8; 4],
    /// where it is on its creature, so the material stays on it as it walks
    /// and turns: x, y, z (1/1024 of a tile, 32768 is 0) and the creature's
    /// turn (1 + 65534 per full turn; 0: it lies where it is in the world)
    pub t: [u16; 4],
}

impl V {
    pub fn pos(&self) -> Vec3 {
        Vec3::from(self.p)
    }

    pub fn normal(&self) -> Vec3 {
        vec3(dec(self.n[0]), dec(self.n[1]), dec(self.n[2]))
    }
}

fn enc(v: f32) -> u8 {
    (v.clamp(-1.0, 1.0) * 127.0 + 128.0).round() as u8
}

fn dec(b: u8) -> f32 {
    (b as f32 - 128.0) / 127.0
}

fn unit8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A point turned around the vertical axis (as the shader turns it back).
pub fn turn_y(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    vec3(c * v.x + s * v.z, v.y, -s * v.x + c * v.z)
}

fn fixed(v: f32) -> u16 {
    (v * 1024.0 + 32768.0).round().clamp(0.0, 65535.0) as u16
}

/// The direction the sun shines from (upper left, toward the viewer).
pub fn sun() -> Vec3 {
    vec3(-0.5, 1.0, 0.55).normalize()
}

/// The sun of the detailed look: lower and more from the left, so the
/// shadows lie long to the right, where the camera sees them.
pub fn sun_hd() -> Vec3 {
    vec3(-0.72, 0.85, 0.22).normalize()
}

/// A place and a turn for the parts of a model.
#[derive(Clone, Copy, Debug)]
pub struct Xf {
    pub o: Vec3,
    pub m: Mat3,
}

impl Xf {
    pub fn at(o: Vec3) -> Xf {
        Xf {
            o,
            m: Mat3::IDENTITY,
        }
    }

    /// Turned around the vertical axis (radians; 0 looks along +x).
    pub fn yaw(o: Vec3, yaw: f32) -> Xf {
        Xf {
            o,
            m: Mat3::from_rotation_y(-yaw),
        }
    }

    /// A frame at `a` whose y axis points to `b`.
    pub fn along(a: Vec3, b: Vec3) -> Xf {
        let d = (b - a).normalize_or(Vec3::Y);
        Xf {
            o: a,
            m: Mat3::from_quat(Quat::from_rotation_arc(Vec3::Y, d)),
        }
    }

    pub fn scaled(self, k: f32) -> Xf {
        Xf {
            o: self.o,
            m: self.m * Mat3::from_diagonal(Vec3::splat(k)),
        }
    }

    /// A child part: moved to `p` (in this frame) and turned by `r`.
    pub fn then(self, p: Vec3, r: Mat3) -> Xf {
        Xf {
            o: self.o + self.m * p,
            m: self.m * r,
        }
    }

    pub fn p(&self, v: Vec3) -> Vec3 {
        self.o + self.m * v
    }

    /// A direction of this frame in the world (unit length).
    pub fn n(&self, v: Vec3) -> Vec3 {
        (self.m * v).normalize_or_zero()
    }

    /// How much this frame enlarges.
    pub fn size(&self) -> f32 {
        self.m.x_axis.length()
    }
}

/// A growing list of triangles.
#[derive(Default)]
pub struct Batch {
    pub v: Vec<V>,
    pub i: Vec<u32>,
    /// the detailed looks: rounder shapes of many more faces
    pub hd: bool,
    /// the high-poly look: smoother still, cut finer
    pub fine: bool,
    /// how much what is added now sways in the wind (0..1)
    pub sway: f32,
    /// the flags of what is added now
    pub flags: u8,
    /// the material of what is added now (0: paint only)
    pub mat: u8,
    /// how much finer than usual the material repeats (0 counts as 1)
    pub tex_k: f32,
    /// the creature being built: where it stands and how it is turned (its
    /// materials move with it)
    pub frame: Option<(Vec3, f32)>,
}

fn rgba8(c: Color) -> [u8; 4] {
    [unit8(c.r), unit8(c.g), unit8(c.b), unit8(c.a)]
}

impl Batch {
    #[cfg(test)]
    pub fn new(hd: bool) -> Batch {
        Batch {
            hd,
            ..Default::default()
        }
    }

    pub fn clear(&mut self) {
        self.v.clear();
        self.i.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.i.is_empty()
    }

    /// Adds a vertex; returns its index.
    pub fn vert(&mut self, p: Vec3, n: Vec3, col: Color, glow: f32) -> u32 {
        let k = if self.tex_k > 0.0 { self.tex_k } else { 1.0 };
        let fine = (scale_of(self.mat) * k * 16.0).round().clamp(1.0, 255.0) as u8;
        let t = match self.frame {
            Some((o, yaw)) => {
                let l = turn_y(p - o, yaw);
                let a = yaw.rem_euclid(TAU) / TAU;
                [
                    fixed(l.x),
                    fixed(l.y),
                    fixed(l.z),
                    1 + (a * 65534.0).round() as u16,
                ]
            }
            None => [0; 4],
        };
        self.v.push(V {
            p: p.to_array(),
            c: rgba8(col),
            n: [enc(n.x), enc(n.y), enc(n.z), unit8(glow)],
            m: [unit8(self.sway), self.flags, self.mat, fine],
            t,
        });
        (self.v.len() - 1) as u32
    }

    pub fn index(&mut self, a: u32, b: u32, c: u32) {
        self.i.extend_from_slice(&[a, b, c]);
    }

    /// A flat triangle lit by its normal (a, b, c counter-clockwise seen
    /// from outside). `glow` 0..1 makes it shine through the dark.
    pub fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, col: Color, glow: f32) {
        let n = (b - a).cross(c - a).normalize_or_zero();
        let i = self.vert(a, n, col, glow);
        self.vert(b, n, col, glow);
        self.vert(c, n, col, glow);
        self.index(i, i + 1, i + 2);
    }

    /// A triangle in a colour as it is (no sun).
    pub fn tri_flat(&mut self, a: Vec3, b: Vec3, c: Vec3, col: Color, glow: f32) {
        let f = self.flags;
        self.flags |= UNLIT;
        let i = self.vert(a, Vec3::ZERO, col, glow);
        self.vert(b, Vec3::ZERO, col, glow);
        self.vert(c, Vec3::ZERO, col, glow);
        self.index(i, i + 1, i + 2);
        self.flags = f;
    }

    pub fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: Color, glow: f32) {
        self.tri(a, b, c, col, glow);
        self.tri(a, c, d, col, glow);
    }

    /// Gives the vertices from `v0` on (of the triangles from `i0` on) the
    /// mean normal of the faces around them: a smooth surface.
    pub fn smooth(&mut self, v0: usize, i0: usize) {
        let mut acc = vec![Vec3::ZERO; self.v.len() - v0];
        for t in self.i[i0..].chunks(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            if a < v0 || b < v0 || c < v0 {
                continue;
            }
            let (pa, pb, pc) = (self.v[a].pos(), self.v[b].pos(), self.v[c].pos());
            // weighted by the area: big faces count more
            let n = (pb - pa).cross(pc - pa);
            for k in [a, b, c] {
                acc[k - v0] += n;
            }
        }
        for (k, n) in acc.into_iter().enumerate() {
            let n = n.normalize_or_zero();
            let v = &mut self.v[v0 + k];
            v.n[0] = enc(n.x);
            v.n[1] = enc(n.y);
            v.n[2] = enc(n.z);
        }
    }

    /// A box: centre and half sizes in the frame of `xf`.
    pub fn cuboid(&mut self, xf: Xf, c: Vec3, h: Vec3, col: Color, glow: f32) {
        if self.hd {
            self.rounded(xf, c, h, None, col, glow);
            return;
        }
        self.frustum(
            xf,
            c - vec3(0.0, h.y, 0.0),
            vec2(h.x, h.z),
            vec2(h.x, h.z),
            h.y * 2.0,
            col,
            glow,
        );
    }

    /// A box narrowing (or widening) upward: bottom centre, half sizes of
    /// the bottom and the top (x, z), height.
    #[allow(clippy::too_many_arguments)]
    pub fn frustum(&mut self, xf: Xf, b: Vec3, lo: Vec2, hi: Vec2, h: f32, col: Color, glow: f32) {
        if self.hd {
            let m = lo.max(hi);
            let taper = (lo / m.max(Vec2::splat(1e-5)), hi / m.max(Vec2::splat(1e-5)));
            let c = b + vec3(0.0, h / 2.0, 0.0);
            self.rounded(xf, c, vec3(m.x, h / 2.0, m.y), Some(taper), col, glow);
            return;
        }
        let p = |x: f32, y: f32, z: f32| xf.p(b + vec3(x, y, z));
        let l = [
            p(-lo.x, 0.0, -lo.y),
            p(lo.x, 0.0, -lo.y),
            p(lo.x, 0.0, lo.y),
            p(-lo.x, 0.0, lo.y),
        ];
        let u = [
            p(-hi.x, h, -hi.y),
            p(hi.x, h, -hi.y),
            p(hi.x, h, hi.y),
            p(-hi.x, h, hi.y),
        ];
        self.quad(u[3], u[2], u[1], u[0], col, glow);
        self.quad(l[0], l[1], l[2], l[3], col, glow);
        for i in 0..4 {
            let j = (i + 1) % 4;
            self.quad(l[j], l[i], u[i], u[j], col, glow);
        }
    }

    /// A box with round edges (the detailed box): centre, half sizes, and
    /// how the x and z sizes shrink from the bottom to the top.
    fn rounded(
        &mut self,
        xf: Xf,
        c: Vec3,
        h: Vec3,
        taper: Option<(Vec2, Vec2)>,
        col: Color,
        glow: f32,
    ) {
        let h = h.max(Vec3::splat(1e-4));
        let r = h.min_element() * 0.32;
        // a chamfer along every edge, cut corners (rounded when fine)
        let fine = self.fine;
        let steps = |e: f32| -> Vec<f32> {
            if fine {
                // every 15 degrees round the edge
                let (a, b) = (r * 0.423, r * 0.732);
                vec![-e, -e + a, -e + b, -e + r, e - r, e - b, e - a, e]
            } else {
                vec![-e, -e + r, e - r, e]
            }
        };
        let s = [steps(h.x), steps(h.y), steps(h.z)];
        let inner = h - Vec3::splat(r);
        for axis in 0..3 {
            for sign in [-1.0f32, 1.0] {
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                let v0 = self.v.len() as u32;
                let (nu, nv) = (s[u].len(), s[v].len());
                for j in 0..nv {
                    for i in 0..nu {
                        let mut q = Vec3::ZERO;
                        q[axis] = sign * h[axis];
                        q[u] = s[u][i];
                        q[v] = s[v][j];
                        let core = q.clamp(-inner, inner);
                        let n = (q - core).normalize_or_zero();
                        let mut p = core + n * r;
                        if let Some((lo, hi)) = taper {
                            let t = (p.y + h.y) / (2.0 * h.y);
                            let k = lo.lerp(hi, t.clamp(0.0, 1.0));
                            p.x *= k.x;
                            p.z *= k.y;
                        }
                        self.vert(xf.p(c + p), xf.n(n), col, glow);
                    }
                }
                for j in 0..nv - 1 {
                    for i in 0..nu - 1 {
                        let a = v0 + (j * nu + i) as u32;
                        let b = a + 1;
                        let d = a + nu as u32;
                        let e = d + 1;
                        // outward: u × v points along the axis
                        if sign > 0.0 {
                            self.index(a, b, e);
                            self.index(a, e, d);
                        } else {
                            self.index(a, e, b);
                            self.index(a, d, e);
                        }
                    }
                }
            }
        }
    }

    /// A body turned around the y axis of `xf` from `b`: rings of (height,
    /// radius along x, radius along z). A ring of radius 0 closes it to a
    /// point; `caps` closes open ends with flat lids.
    #[allow(clippy::too_many_arguments)]
    pub fn lathe(
        &mut self,
        xf: Xf,
        b: Vec3,
        rings: &[(f32, f32, f32)],
        n: usize,
        caps: bool,
        col: Color,
        glow: f32,
    ) {
        if rings.len() < 2 {
            return;
        }
        let n = if self.fine { n.max(3) * 2 } else { n.max(3) };
        let (v0, i0) = (self.v.len(), self.i.len());
        let mut ids: Vec<Vec<u32>> = vec![];
        for &(y, rx, rz) in rings {
            if rx.abs() < 1e-6 && rz.abs() < 1e-6 {
                let i = self.vert(xf.p(b + vec3(0.0, y, 0.0)), Vec3::Y, col, glow);
                ids.push(vec![i; n]);
                continue;
            }
            let row = (0..n)
                .map(|k| {
                    let a = k as f32 / n as f32 * TAU;
                    let p = b + vec3(a.cos() * rx, y, a.sin() * rz);
                    self.vert(xf.p(p), Vec3::Y, col, glow)
                })
                .collect();
            ids.push(row);
        }
        for r in 0..rings.len() - 1 {
            let (lo, hi) = (&ids[r], &ids[r + 1]);
            for k in 0..n {
                let j = (k + 1) % n;
                let (a, bb, c, d) = (lo[k], lo[j], hi[j], hi[k]);
                if a != bb {
                    self.index(a, d, bb);
                }
                if d != c {
                    self.index(bb, d, c);
                }
            }
        }
        self.smooth(v0, i0);
        if caps {
            for (end, down) in [(0, true), (rings.len() - 1, false)] {
                let (y, rx, rz) = rings[end];
                if rx.abs() < 1e-6 && rz.abs() < 1e-6 {
                    continue;
                }
                let nn = xf.n(if down { -Vec3::Y } else { Vec3::Y });
                let c0 = self.vert(xf.p(b + vec3(0.0, y, 0.0)), nn, col, glow);
                for k in 0..n {
                    let a = k as f32 / n as f32 * TAU;
                    let p = b + vec3(a.cos() * rx, y, a.sin() * rz);
                    self.vert(xf.p(p), nn, col, glow);
                }
                for k in 0..n as u32 {
                    let j = (k + 1) % n as u32;
                    if down {
                        self.index(c0, c0 + 1 + k, c0 + 1 + j);
                    } else {
                        self.index(c0, c0 + 1 + j, c0 + 1 + k);
                    }
                }
            }
        }
    }

    /// Sides of a round thing in the detailed look, by the size it has.
    fn sides(&self, xf: &Xf, r: f32, n: usize) -> usize {
        let px = r * xf.size();
        let k = if px > 0.2 {
            12
        } else if px > 0.08 {
            9
        } else if px > 0.03 {
            7
        } else {
            6
        };
        k.max(n)
    }

    /// An n-sided column (a prism) from the bottom centre, narrowing to
    /// `top` times its radius.
    #[allow(clippy::too_many_arguments)]
    pub fn prism(
        &mut self,
        xf: Xf,
        b: Vec3,
        r: f32,
        h: f32,
        n: usize,
        top: f32,
        col: Color,
        glow: f32,
    ) {
        if self.hd {
            let n = self.sides(&xf, r, n);
            let rt = r * top;
            self.lathe(xf, b, &[(0.0, r, r), (h, rt, rt)], n, true, col, glow);
            return;
        }
        let n = n.max(3);
        let ring = |y: f32, k: f32| -> Vec<Vec3> {
            (0..n)
                .map(|i| {
                    let a = i as f32 / n as f32 * TAU;
                    xf.p(b + vec3(a.cos() * r * k, y, a.sin() * r * k))
                })
                .collect()
        };
        let (lo, hi) = (ring(0.0, 1.0), ring(h, top));
        let (cb, ct) = (xf.p(b), xf.p(b + vec3(0.0, h, 0.0)));
        for i in 0..n {
            let j = (i + 1) % n;
            self.quad(lo[j], lo[i], hi[i], hi[j], col, glow);
            self.tri(ct, hi[j], hi[i], col, glow);
            self.tri(cb, lo[i], lo[j], col, glow);
        }
    }

    /// An n-sided cone from the bottom centre.
    #[allow(clippy::too_many_arguments)]
    pub fn cone(&mut self, xf: Xf, b: Vec3, r: f32, h: f32, n: usize, col: Color, glow: f32) {
        if self.hd {
            let n = self.sides(&xf, r, n);
            self.lathe(
                xf,
                b,
                &[(0.0, r, r), (h * 0.5, r * 0.5, r * 0.5), (h, 0.0, 0.0)],
                n,
                true,
                col,
                glow,
            );
            return;
        }
        let n = n.max(3);
        let tip = xf.p(b + vec3(0.0, h, 0.0));
        let cb = xf.p(b);
        let pts: Vec<Vec3> = (0..n)
            .map(|i| {
                let a = i as f32 / n as f32 * TAU;
                xf.p(b + vec3(a.cos() * r, 0.0, a.sin() * r))
            })
            .collect();
        for i in 0..n {
            let j = (i + 1) % n;
            self.tri(pts[j], pts[i], tip, col, glow);
            self.tri(cb, pts[i], pts[j], col, glow);
        }
    }

    /// A cone between two points (a spike, a horn, a leg).
    pub fn spike(&mut self, a: Vec3, b: Vec3, r: f32, n: usize, col: Color, glow: f32) {
        let len = (b - a).length();
        if len < 1e-4 {
            return;
        }
        self.cone(Xf::along(a, b), Vec3::ZERO, r, len, n, col, glow);
    }

    /// A column between two points.
    #[allow(clippy::too_many_arguments)]
    pub fn rod(&mut self, a: Vec3, b: Vec3, r: f32, n: usize, top: f32, col: Color, glow: f32) {
        let len = (b - a).length();
        if len < 1e-4 {
            return;
        }
        self.prism(Xf::along(a, b), Vec3::ZERO, r, len, n, top, col, glow);
    }

    /// A rough ball: an octahedron with its corners pushed about by `seed`
    /// (0: a clean diamond). In the detailed look a rough one is a stone of
    /// many facets; a clean one stays a cut gem.
    #[allow(clippy::too_many_arguments)]
    pub fn gem(&mut self, xf: Xf, c: Vec3, r: Vec3, seed: u32, col: Color, glow: f32) {
        if self.hd && seed != 0 {
            self.stone(xf, c, r, seed, col, glow);
            return;
        }
        let j = |i: u32| {
            if seed == 0 {
                1.0
            } else {
                0.8 + 0.4 * hashf(seed.wrapping_mul(31).wrapping_add(i))
            }
        };
        let p = [
            vec3(r.x * j(0), 0.0, 0.0),
            vec3(-r.x * j(1), 0.0, 0.0),
            vec3(0.0, r.y * j(2), 0.0),
            vec3(0.0, -r.y * j(3), 0.0),
            vec3(0.0, 0.0, r.z * j(4)),
            vec3(0.0, 0.0, -r.z * j(5)),
        ]
        .map(|v| xf.p(c + v));
        let faces = [
            (0, 2, 4),
            (4, 2, 1),
            (1, 2, 5),
            (5, 2, 0),
            (4, 3, 0),
            (1, 3, 4),
            (5, 3, 1),
            (0, 3, 5),
        ];
        for (a, b, cc) in faces {
            self.tri(p[a], p[b], p[cc], col, glow);
        }
    }

    /// A stone of many facets (the detailed rough gem).
    #[allow(clippy::too_many_arguments)]
    pub fn stone(&mut self, xf: Xf, c: Vec3, r: Vec3, seed: u32, col: Color, glow: f32) {
        if self.fine {
            self.lump(xf, c, r * 0.95, 0.2, seed, 2, col, glow);
            return;
        }
        let ico = ico(1);
        let off = vec3(
            hashf(seed) * 37.0,
            hashf(seed + 1) * 37.0,
            hashf(seed + 2) * 37.0,
        );
        let pts: Vec<Vec3> = ico
            .v
            .iter()
            .map(|d| {
                let k = 0.82 + 0.3 * (noise3(*d * 1.7 + off) * 0.5 + 0.5);
                xf.p(c + *d * r * k)
            })
            .collect();
        for t in &ico.t {
            self.tri(
                pts[t[0] as usize],
                pts[t[1] as usize],
                pts[t[2] as usize],
                col,
                glow,
            );
        }
    }

    /// A rounder ball: an icosahedron, roughened by `seed`; in the
    /// detailed look a smooth ball of many faces, a little lumpy by `seed`.
    #[allow(clippy::too_many_arguments)]
    pub fn ball(&mut self, xf: Xf, c: Vec3, r: Vec3, seed: u32, col: Color, glow: f32) {
        if self.hd {
            let amp = if seed == 0 { 0.0 } else { 0.12 };
            self.blob(xf, c, r, amp, seed, col, glow);
            return;
        }
        let t = (1.0 + 5f32.sqrt()) / 2.0;
        let raw = [
            vec3(-1.0, t, 0.0),
            vec3(1.0, t, 0.0),
            vec3(-1.0, -t, 0.0),
            vec3(1.0, -t, 0.0),
            vec3(0.0, -1.0, t),
            vec3(0.0, 1.0, t),
            vec3(0.0, -1.0, -t),
            vec3(0.0, 1.0, -t),
            vec3(t, 0.0, -1.0),
            vec3(t, 0.0, 1.0),
            vec3(-t, 0.0, -1.0),
            vec3(-t, 0.0, 1.0),
        ];
        let mut p = [Vec3::ZERO; 12];
        for (i, v) in raw.iter().enumerate() {
            let k = if seed == 0 {
                1.0
            } else {
                0.85 + 0.3 * hashf(seed.wrapping_mul(17).wrapping_add(i as u32))
            };
            p[i] = xf.p(c + v.normalize() * r * k);
        }
        for (a, b, cc) in ICO_FACES {
            self.tri(p[a], p[b], p[cc], col, glow);
        }
    }

    /// A smooth ball of many faces (more for bigger ones), lumpy by `amp`
    /// (0: an ellipsoid).
    #[allow(clippy::too_many_arguments)]
    pub fn blob(&mut self, xf: Xf, c: Vec3, r: Vec3, amp: f32, seed: u32, col: Color, glow: f32) {
        let big = r.max_element() * xf.size();
        let level = if big > 0.1 { 2 } else { 1 };
        self.lump(xf, c, r, amp, seed, level, col, glow);
    }

    /// A smooth ball cut `level` times finer than an icosahedron, lumpy by
    /// `amp`.
    #[allow(clippy::too_many_arguments)]
    pub fn lump(
        &mut self,
        xf: Xf,
        c: Vec3,
        r: Vec3,
        amp: f32,
        seed: u32,
        level: usize,
        col: Color,
        glow: f32,
    ) {
        // finer when fine, but for the smallest bits
        let level = if self.fine && r.max_element() * xf.size() > 0.035 {
            (level + 1).min(3)
        } else {
            level
        };
        let ico = ico(level);
        let (v0, i0) = (self.v.len(), self.i.len());
        let off = vec3(hashf(seed) * 51.0, hashf(seed + 7) * 51.0, 3.0);
        for d in &ico.v {
            let k = if amp > 0.0 {
                1.0 + amp * noise3(*d * 1.9 + off)
            } else {
                1.0
            };
            let n = if amp > 0.0 {
                Vec3::Y
            } else {
                xf.n(*d / r.max(Vec3::splat(1e-5)))
            };
            self.vert(xf.p(c + *d * r * k), n, col, glow);
        }
        let b = v0 as u32;
        for t in &ico.t {
            self.index(b + t[0], b + t[1], b + t[2]);
        }
        if amp > 0.0 {
            self.smooth(v0, i0);
        }
    }

    /// A flat disc lying at height y (shadows, rings of light).
    pub fn disc(&mut self, c: Vec3, rx: f32, rz: f32, n: usize, col: Color, glow: f32) {
        let pts: Vec<Vec3> = (0..=n)
            .map(|i| {
                let a = i as f32 / n as f32 * TAU;
                c + vec3(a.cos() * rx, 0.0, a.sin() * rz)
            })
            .collect();
        for i in 0..n {
            self.tri_flat(c, pts[i + 1], pts[i], col, glow);
        }
    }

    /// A soft round shadow: dark in the middle, fading to nothing.
    pub fn soft_disc(&mut self, c: Vec3, rx: f32, rz: f32, n: usize, col: Color) {
        let f = self.flags;
        self.flags |= UNLIT;
        let mid = self.vert(c, Vec3::ZERO, col, 0.0);
        let half = Color::new(col.r, col.g, col.b, col.a * 0.55);
        let none = Color::new(col.r, col.g, col.b, 0.0);
        for (k, cc) in [(0.6, half), (1.0, none)] {
            for i in 0..n {
                let a = i as f32 / n as f32 * TAU;
                self.vert(
                    c + vec3(a.cos() * rx * k, 0.0, a.sin() * rz * k),
                    Vec3::ZERO,
                    cc,
                    0.0,
                );
            }
        }
        let n32 = n as u32;
        for i in 0..n32 {
            let j = (i + 1) % n32;
            self.index(mid, mid + 1 + j, mid + 1 + i);
            let (a, b) = (mid + 1 + i, mid + 1 + j);
            let (c2, d2) = (a + n32, b + n32);
            self.index(a, b, d2);
            self.index(a, d2, c2);
        }
        self.flags = f;
    }

    /// A flat ring lying at height y.
    #[allow(clippy::too_many_arguments)]
    pub fn ring(&mut self, c: Vec3, r: f32, w: f32, n: usize, col: Color, glow: f32) {
        let p = |i: usize, k: f32| {
            let a = i as f32 / n as f32 * TAU;
            c + vec3(a.cos() * k, 0.0, a.sin() * k)
        };
        for i in 0..n {
            let (a, b) = (p(i, r - w / 2.0), p(i + 1, r - w / 2.0));
            let (cc, d) = (p(i + 1, r + w / 2.0), p(i, r + w / 2.0));
            self.tri_flat(a, b, cc, col, glow);
            self.tri_flat(a, cc, d, col, glow);
        }
    }

    /// The triangles as macroquad draws them (in their colours, unlit).
    pub fn plain(&self, from: usize) -> Vec<Vertex> {
        self.i[from..]
            .iter()
            .map(|&k| {
                let v = &self.v[k as usize];
                Vertex {
                    position: v.pos(),
                    uv: vec2(0.0, 0.0),
                    color: v.c,
                    normal: vec4(0.0, 0.0, 0.0, 0.0),
                }
            })
            .collect()
    }

    /// Draws the triangles through macroquad (in their colours) and empties.
    pub fn flush_plain(&mut self) {
        draw_vertices(&self.plain(0));
        self.clear();
    }
}

/// Draws a list of macroquad triangles in pieces that fit a draw call.
pub fn draw_vertices(v: &[Vertex]) {
    for part in v.chunks(CALL - CALL % 3) {
        let mesh = Mesh {
            vertices: part.to_vec(),
            indices: (0..part.len() as u16).collect(),
            texture: None,
        };
        draw_mesh(&mesh);
    }
}

const ICO_FACES: [(usize, usize, usize); 20] = [
    (0, 11, 5),
    (0, 5, 1),
    (0, 1, 7),
    (0, 7, 10),
    (0, 10, 11),
    (1, 5, 9),
    (5, 11, 4),
    (11, 10, 2),
    (10, 7, 6),
    (7, 1, 8),
    (3, 9, 4),
    (3, 4, 2),
    (3, 2, 6),
    (3, 6, 8),
    (3, 8, 9),
    (4, 9, 5),
    (2, 4, 11),
    (6, 2, 10),
    (8, 6, 7),
    (9, 8, 1),
];

/// A unit ball of triangles (an icosahedron cut finer `level` times).
pub struct Ico {
    pub v: Vec<Vec3>,
    pub t: Vec<[u32; 3]>,
}

pub fn ico(level: usize) -> &'static Ico {
    static BALLS: OnceLock<Vec<Ico>> = OnceLock::new();
    let balls = BALLS.get_or_init(|| {
        let t = (1.0 + 5f32.sqrt()) / 2.0;
        let mut v: Vec<Vec3> = [
            vec3(-1.0, t, 0.0),
            vec3(1.0, t, 0.0),
            vec3(-1.0, -t, 0.0),
            vec3(1.0, -t, 0.0),
            vec3(0.0, -1.0, t),
            vec3(0.0, 1.0, t),
            vec3(0.0, -1.0, -t),
            vec3(0.0, 1.0, -t),
            vec3(t, 0.0, -1.0),
            vec3(t, 0.0, 1.0),
            vec3(-t, 0.0, -1.0),
            vec3(-t, 0.0, 1.0),
        ]
        .iter()
        .map(|p| p.normalize())
        .collect();
        let mut f: Vec<[u32; 3]> = ICO_FACES
            .iter()
            .map(|&(a, b, c)| [a as u32, b as u32, c as u32])
            .collect();
        let mut out = vec![Ico {
            v: v.clone(),
            t: f.clone(),
        }];
        for _ in 0..3 {
            let mut mid = std::collections::HashMap::new();
            let mut half = |a: u32, b: u32, v: &mut Vec<Vec3>| -> u32 {
                let key = (a.min(b), a.max(b));
                *mid.entry(key).or_insert_with(|| {
                    v.push(((v[a as usize] + v[b as usize]) / 2.0).normalize());
                    (v.len() - 1) as u32
                })
            };
            let mut nf = Vec::with_capacity(f.len() * 4);
            for [a, b, c] in f {
                let ab = half(a, b, &mut v);
                let bc = half(b, c, &mut v);
                let ca = half(c, a, &mut v);
                nf.extend_from_slice(&[[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
            }
            f = nf;
            out.push(Ico {
                v: v.clone(),
                t: f.clone(),
            });
        }
        out
    });
    &balls[level.min(3)]
}

/// A number in 0..1 from a seed.
pub fn hashf(x: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

/// A seed from a pair of integers.
pub fn hash2(x: i32, y: i32) -> u32 {
    (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663)
}

fn hash3(x: i32, y: i32, z: i32) -> f32 {
    hashf(hash2(x, y) ^ (z as u32).wrapping_mul(83_492_791))
}

fn fade(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Smooth noise in -1..1 over space.
pub fn noise3(p: Vec3) -> f32 {
    let i = p.floor();
    let f = p - i;
    let (x, y, z) = (i.x as i32, i.y as i32, i.z as i32);
    let (u, v, w) = (fade(f.x), fade(f.y), fade(f.z));
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let c = |dx: i32, dy: i32, dz: i32| hash3(x + dx, y + dy, z + dz);
    let x00 = lerp(c(0, 0, 0), c(1, 0, 0), u);
    let x10 = lerp(c(0, 1, 0), c(1, 1, 0), u);
    let x01 = lerp(c(0, 0, 1), c(1, 0, 1), u);
    let x11 = lerp(c(0, 1, 1), c(1, 1, 1), u);
    let y0 = lerp(x00, x10, v);
    let y1 = lerp(x01, x11, v);
    lerp(y0, y1, w) * 2.0 - 1.0
}

/// Smooth noise in -1..1 over the ground.
pub fn noise2(x: f32, y: f32) -> f32 {
    noise3(vec3(x, y, 0.5))
}

/// Noise of a few octaves over the ground, in about -1..1.
pub fn fbm2(x: f32, y: f32) -> f32 {
    noise2(x, y) * 0.6
        + noise2(x * 2.1 + 17.0, y * 2.1 - 9.0) * 0.28
        + noise2(x * 4.3 - 5.0, y * 4.3 + 31.0) * 0.12
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every shape winds outward, in both looks.
    #[test]
    fn shapes_face_outward() {
        for (hd, fine) in [(false, false), (true, false), (true, true)] {
            let mut b = Batch::new(hd);
            b.fine = fine;
            let c = vec3(1.0, 2.0, 3.0);
            b.cuboid(Xf::at(c), Vec3::ZERO, vec3(0.3, 0.2, 0.25), WHITE, 0.0);
            b.ball(Xf::at(c), Vec3::ZERO, vec3(0.3, 0.2, 0.25), 0, WHITE, 0.0);
            b.prism(
                Xf::at(c),
                vec3(0.0, -0.2, 0.0),
                0.2,
                0.4,
                6,
                0.8,
                WHITE,
                0.0,
            );
            for t in b.i.chunks(3) {
                let [p, q, r] = [t[0], t[1], t[2]].map(|k| b.v[k as usize].pos());
                let n = (q - p).cross(r - p);
                let mid = (p + q + r) / 3.0 - c;
                if n.length() > 1e-7 {
                    assert!(n.dot(mid) > -1e-6, "hd {hd}: an inward face at {mid}");
                }
            }
        }
    }

    #[test]
    fn normals_survive_packing() {
        let mut b = Batch::default();
        let n = vec3(0.3, -0.8, 0.52).normalize();
        b.vert(Vec3::ZERO, n, WHITE, 0.5);
        assert!(b.v[0].normal().distance(n) < 0.02);
        assert_eq!(std::mem::size_of::<V>(), 32);
    }
}
