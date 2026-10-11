//! Shapes of the detailed look: smooth limbs and tubes, parts of balls
//! (hair, helmets, hoods), rings, cloth and leaves, blades and flat pieces.

use std::f32::consts::{FRAC_PI_2, TAU};

use macroquad::prelude::*;

use super::lowpoly::{ico, Batch, Xf};

impl Batch {
    /// A smooth ellipsoid.
    pub fn ellipsoid(&mut self, xf: Xf, c: Vec3, r: Vec3, col: Color, glow: f32) {
        self.blob(xf, c, r, 0.0, 0, col, glow);
    }

    /// A smooth limb between two points, round at both ends, its radius
    /// going from `ra` to `rb`.
    pub fn capsule(&mut self, a: Vec3, b: Vec3, ra: f32, rb: f32, col: Color, glow: f32) {
        let len = (b - a).length();
        if len < 1e-5 {
            self.ellipsoid(Xf::at(a), Vec3::ZERO, Vec3::splat(ra), col, glow);
            return;
        }
        let xf = Xf::along(a, b);
        let n = round_sides(ra.max(rb));
        // rings over each round end
        let m = if self.fine { 4 } else { 2 };
        let mut rings = Vec::with_capacity(2 * m + 2);
        for i in 0..=m {
            let t = -FRAC_PI_2 + i as f32 / m as f32 * FRAC_PI_2;
            let r = ra * t.cos();
            rings.push((ra * t.sin(), r, r));
        }
        for i in 0..=m {
            let t = i as f32 / m as f32 * FRAC_PI_2;
            let r = rb * t.cos();
            rings.push((len + rb * t.sin(), r, r));
        }
        self.lathe(xf, Vec3::ZERO, &rings, n, false, col, glow);
    }

    /// A smooth tube through points, of a radius at each (0 makes a
    /// point); `round` closes open ends with a dome.
    pub fn tube(&mut self, pts: &[Vec3], radii: &[f32], round: bool, col: Color, glow: f32) {
        if pts.len() < 2 || radii.len() != pts.len() {
            return;
        }
        let (mut pts, mut radii) = if self.fine {
            // a smooth curve through the points
            let r: Vec<Vec3> = radii.iter().map(|&r| Vec3::splat(r)).collect();
            (
                refine(pts, false),
                refine(&r, false).iter().map(|r| r.x.max(0.0)).collect(),
            )
        } else {
            (pts.to_vec(), radii.to_vec())
        };
        if round {
            let k = pts.len() - 1;
            if radii[k] > 1e-5 {
                let d = (pts[k] - pts[k - 1]).normalize_or_zero();
                let (p, r) = (pts[k], radii[k]);
                pts.push(p + d * r * 0.7);
                radii.push(r * 0.7);
                pts.push(p + d * r);
                radii.push(0.0);
            }
            if radii[0] > 1e-5 {
                let d = (pts[0] - pts[1]).normalize_or_zero();
                let (p, r) = (pts[0], radii[0]);
                pts.insert(0, p + d * r * 0.7);
                radii.insert(0, r * 0.7);
                pts.insert(0, p + d * r);
                radii.insert(0, 0.0);
            }
        }
        let m = pts.len();
        let n =
            round_sides(radii.iter().cloned().fold(0.0, f32::max)) * if self.fine { 2 } else { 1 };
        let tan = |i: usize| -> Vec3 {
            let a = pts[i.saturating_sub(1)];
            let b = pts[(i + 1).min(m - 1)];
            (b - a).normalize_or(Vec3::Y)
        };
        let t0 = tan(0);
        let mut u = if t0.y.abs() < 0.9 {
            Vec3::Y.cross(t0).normalize()
        } else {
            Vec3::X.cross(t0).normalize()
        };
        let (v0, i0) = (self.v.len(), self.i.len());
        let mut ids: Vec<Vec<u32>> = Vec::with_capacity(m);
        for i in 0..m {
            let t = tan(i);
            // carried along the curve without twisting
            u = (u - t * u.dot(t)).normalize_or(u);
            let w = u.cross(t);
            if radii[i] < 1e-5 {
                let k = self.vert(pts[i], t, col, glow);
                ids.push(vec![k; n]);
                continue;
            }
            let row = (0..n)
                .map(|k| {
                    let a = k as f32 / n as f32 * TAU;
                    let d = u * a.cos() + w * a.sin();
                    self.vert(pts[i] + d * radii[i], d, col, glow)
                })
                .collect();
            ids.push(row);
        }
        for r in 0..m - 1 {
            for k in 0..n {
                let j = (k + 1) % n;
                let (a, b, c, d) = (ids[r][k], ids[r][j], ids[r + 1][j], ids[r + 1][k]);
                if a != b {
                    self.index(a, d, b);
                }
                if d != c {
                    self.index(b, d, c);
                }
            }
        }
        self.smooth(v0, i0);
    }

    /// The part of an ellipsoid whose directions from the centre `keep`
    /// likes (hair, helmets, hoods, caps).
    pub fn shell(
        &mut self,
        xf: Xf,
        c: Vec3,
        r: Vec3,
        keep: impl Fn(Vec3) -> bool,
        col: Color,
        glow: f32,
    ) {
        let ico = ico(if self.fine || r.max_element() * xf.size() > 0.12 {
            3
        } else {
            2
        });
        let base = self.v.len() as u32;
        let mut used = vec![u32::MAX; ico.v.len()];
        for t in &ico.t {
            let mid = (ico.v[t[0] as usize] + ico.v[t[1] as usize] + ico.v[t[2] as usize]) / 3.0;
            if !keep(mid.normalize()) {
                continue;
            }
            let mut k = [0u32; 3];
            for (j, &i) in t.iter().enumerate() {
                if used[i as usize] == u32::MAX {
                    let d = ico.v[i as usize];
                    let n = xf.n(d / r.max(Vec3::splat(1e-5)));
                    used[i as usize] = self.vert(xf.p(c + d * r), n, col, glow);
                }
                k[j] = used[i as usize];
            }
            self.index(k[0], k[1], k[2]);
        }
        let _ = base;
    }

    /// A ring lying around the y axis of `xf`: its radius and the radius
    /// of its tube.
    pub fn torus(&mut self, xf: Xf, c: Vec3, big: f32, small: f32, col: Color, glow: f32) {
        let (n, m) = if self.fine {
            (round_sides(big).max(12) * 2, 12)
        } else {
            (round_sides(big).max(12), 8)
        };
        let pts: Vec<Vec<Vec3>> = (0..n)
            .map(|i| {
                let u = i as f32 / n as f32 * TAU;
                (0..m)
                    .map(|j| {
                        let v = j as f32 / m as f32 * TAU;
                        let rr = big + small * v.cos();
                        xf.p(c + vec3(rr * u.cos(), small * v.sin(), rr * u.sin()))
                    })
                    .collect()
            })
            .collect();
        self.mesh(&pts, true, true, &|_, _| col, glow);
    }

    /// A surface through rows of points (all rows as long), smoothed;
    /// `wrap_rows` joins the last row to the first, `wrap_cols` the last
    /// point of each row to its first. `col` colours a point by (row, col).
    /// The high-poly look bends it through twice as many points.
    pub fn grid(
        &mut self,
        pts: &[Vec<Vec3>],
        wrap_rows: bool,
        wrap_cols: bool,
        col: &dyn Fn(usize, usize) -> Color,
        glow: f32,
    ) {
        if !self.fine || pts.len() < 2 {
            self.mesh(pts, wrap_rows, wrap_cols, col, glow);
            return;
        }
        let along: Vec<Vec<Vec3>> = pts.iter().map(|r| refine(r, wrap_cols)).collect();
        let cols = along[0].len();
        let columns: Vec<Vec<Vec3>> = (0..cols)
            .map(|k| {
                let c: Vec<Vec3> = along.iter().map(|r| r[k]).collect();
                refine(&c, wrap_rows)
            })
            .collect();
        let rows = columns[0].len();
        let fine: Vec<Vec<Vec3>> = (0..rows)
            .map(|r| columns.iter().map(|c| c[r]).collect())
            .collect();
        self.mesh(&fine, wrap_rows, wrap_cols, &|r, k| col(r / 2, k / 2), glow);
    }

    /// A surface through rows of points, as given (see `grid`).
    fn mesh(
        &mut self,
        pts: &[Vec<Vec3>],
        wrap_rows: bool,
        wrap_cols: bool,
        col: &dyn Fn(usize, usize) -> Color,
        glow: f32,
    ) {
        let rows = pts.len();
        if rows < 2 {
            return;
        }
        let cols = pts[0].len();
        let (v0, i0) = (self.v.len(), self.i.len());
        for (r, row) in pts.iter().enumerate() {
            for (k, p) in row.iter().enumerate() {
                self.vert(*p, Vec3::Y, col(r, k), glow);
            }
        }
        let id = |r: usize, k: usize| (v0 + (r % rows) * cols + (k % cols)) as u32;
        let rn = if wrap_rows { rows } else { rows - 1 };
        let kn = if wrap_cols { cols } else { cols - 1 };
        for r in 0..rn {
            for k in 0..kn {
                let (a, b, c, d) = (id(r, k), id(r, k + 1), id(r + 1, k + 1), id(r + 1, k));
                self.index(a, b, c);
                self.index(a, c, d);
            }
        }
        self.smooth(v0, i0);
    }

    /// A blade from `base` to `tip`: `side` is the way its edges face,
    /// `w` its half width at the base, `th` its half thickness. Faceted
    /// (sharp edges catch the light).
    #[allow(clippy::too_many_arguments)]
    pub fn blade(&mut self, base: Vec3, tip: Vec3, side: Vec3, w: f32, th: f32, col: Color) {
        let d = tip - base;
        let len = d.length();
        if len < 1e-5 {
            return;
        }
        let f = d / len;
        let s = (side - f * side.dot(f)).normalize_or(Vec3::X);
        let flat = f.cross(s);
        let ring = |at: f32, k: f32| {
            let c = base + f * len * at;
            [
                c + s * w * k,
                c + flat * th * k,
                c - s * w * k,
                c - flat * th * k,
            ]
        };
        let a = ring(0.0, 1.0);
        let m = ring(0.78, 0.92);
        for i in 0..4 {
            let j = (i + 1) % 4;
            self.quad(a[i], a[j], m[j], m[i], col, 0.0);
            self.tri(m[i], m[j], tip, col, 0.0);
        }
        self.tri(a[0], a[2], a[1], col, 0.0);
        self.tri(a[0], a[3], a[2], col, 0.0);
    }

    /// A flat piece cut to a convex outline (in the x-y plane of `xf`),
    /// `th` thick along z.
    pub fn slab(&mut self, xf: Xf, outline: &[Vec2], th: f32, col: Color, glow: f32) {
        let n = outline.len();
        if n < 3 {
            return;
        }
        let area: f32 = (0..n)
            .map(|i| {
                let (a, b) = (outline[i], outline[(i + 1) % n]);
                a.x * b.y - b.x * a.y
            })
            .sum();
        let pts: Vec<Vec2> = if area < 0.0 {
            outline.iter().rev().copied().collect()
        } else {
            outline.to_vec()
        };
        let mid = pts.iter().copied().sum::<Vec2>() / n as f32;
        let at = |p: Vec2, z: f32| xf.p(vec3(p.x, p.y, z));
        let (f, bk) = (th / 2.0, -th / 2.0);
        for i in 0..n {
            let j = (i + 1) % n;
            self.tri(at(mid, f), at(pts[i], f), at(pts[j], f), col, glow);
            self.tri(at(mid, bk), at(pts[j], bk), at(pts[i], bk), col, glow);
            self.quad(
                at(pts[i], bk),
                at(pts[j], bk),
                at(pts[j], f),
                at(pts[i], f),
                col,
                glow,
            );
        }
    }

    /// A thin leaf or feather between two points: an ellipse `w` wide,
    /// bent a little along `up`.
    pub fn leaf(&mut self, a: Vec3, b: Vec3, side: Vec3, up: Vec3, w: f32, col: Color, glow: f32) {
        let steps = 5;
        let rows: Vec<Vec<Vec3>> = (0..=steps)
            .map(|i| {
                let t = i as f32 / steps as f32;
                let c = a.lerp(b, t) + up * (t * (1.0 - t) * 0.6 * (b - a).length() * 0.3);
                let half = w * (t * std::f32::consts::PI).sin().max(0.02);
                vec![c - side * half, c, c + side * half]
            })
            .collect();
        self.grid(&rows, false, false, &|_, _| col, glow);
    }
}

/// A line through points made smooth: a point between every two, on the
/// curve through their neighbours.
fn refine(pts: &[Vec3], wrap: bool) -> Vec<Vec3> {
    let n = pts.len() as isize;
    if n < 2 {
        return pts.to_vec();
    }
    let at = |i: isize| {
        if wrap {
            pts[i.rem_euclid(n) as usize]
        } else {
            pts[i.clamp(0, n - 1) as usize]
        }
    };
    let segs = if wrap { n } else { n - 1 };
    let mut out = Vec::with_capacity(pts.len() * 2);
    for i in 0..segs {
        out.push(at(i));
        out.push((at(i) * 9.0 + at(i + 1) * 9.0 - at(i - 1) - at(i + 2)) / 16.0);
    }
    if !wrap {
        out.push(at(n - 1));
    }
    out
}

/// Sides of a round thing by its size.
fn round_sides(r: f32) -> usize {
    if r > 0.2 {
        10
    } else if r > 0.07 {
        8
    } else if r > 0.03 {
        7
    } else {
        6
    }
}
