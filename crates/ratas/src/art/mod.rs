//! Procedural pixel art. Everything the game shows is painted in code from
//! the colours in the content files, so modded tiles and creatures get art
//! too: tiles are 16x16 ("tall" objects and walls 16x24, seen in 3/4 view),
//! creatures are assembled humanoids or hand-drawn beasts.

pub mod icons;
pub mod models;
pub mod specs;
pub mod tiles;

use ratas_core::rng::{fnv64, Rng};

/// Size of a tile in art pixels.
pub const SPX: i32 = 16;

/// A straight (non-premultiplied) RGBA colour.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub const NONE: Rgba = Rgba {
    r: 0,
    g: 0,
    b: 0,
    a: 0,
};
pub const BLACK: Rgba = rgb(0, 0, 0);
pub const WHITE: Rgba = rgb(255, 255, 255);
pub const BARK: Rgba = rgb(96, 64, 38);
pub const STONE: Rgba = rgb(128, 128, 140);

pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    Rgba { r, g, b, a: 255 }
}

impl Rgba {
    pub fn visible(self) -> bool {
        self.a > 0
    }
}

/// Parses "#rrggbb"; anything else is grey.
pub fn hex(s: &str) -> Rgba {
    let s = s.trim();
    if s.len() == 7 && s.starts_with('#') {
        if let Ok(v) = u32::from_str_radix(&s[1..], 16) {
            return rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
        }
    }
    rgb(128, 128, 128)
}

fn clamp_u8(v: f64) -> u8 {
    v.clamp(0.0, 255.0) as u8
}

pub fn mul(c: Rgba, k: f64) -> Rgba {
    Rgba {
        r: clamp_u8(c.r as f64 * k),
        g: clamp_u8(c.g as f64 * k),
        b: clamp_u8(c.b as f64 * k),
        a: c.a,
    }
}

pub fn mix(a: Rgba, b: Rgba, t: f64) -> Rgba {
    let m = |x: u8, y: u8| clamp_u8(x as f64 * (1.0 - t) + y as f64 * t);
    Rgba {
        r: m(a.r, b.r),
        g: m(a.g, b.g),
        b: m(a.b, b.b),
        a: m(a.a, b.a),
    }
}

pub fn alpha(c: Rgba, a: u8) -> Rgba {
    Rgba { a, ..c }
}

/// A pixel canvas with its own random stream.
pub struct Pc {
    pub w: i32,
    pub h: i32,
    pub px: Vec<Rgba>,
    pub r: Rng,
}

impl Pc {
    pub fn new(w: i32, h: i32, seed: &str) -> Pc {
        Pc {
            w,
            h,
            px: vec![NONE; (w * h).max(0) as usize],
            r: Rng::new(fnv64(seed), 99),
        }
    }

    /// A random integer in 0..n.
    pub fn ri(&mut self, n: i32) -> i32 {
        self.r.int_n(n.max(1))
    }

    /// A random number in 0..1.
    pub fn rf(&mut self) -> f64 {
        self.r.f64()
    }

    fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }

    /// Paints a pixel, blending translucent colours over what is there.
    pub fn set(&mut self, x: i32, y: i32, c: Rgba) {
        if !self.inside(x, y) || c.a == 0 {
            return;
        }
        let i = (y * self.w + x) as usize;
        if c.a == 255 {
            self.px[i] = c;
            return;
        }
        let o = self.px[i];
        let t = c.a as f64 / 255.0;
        let mut n = mix(o, Rgba { a: 255, ..c }, t);
        n.a = clamp_u8(o.a as f64 + c.a as f64 * (1.0 - o.a as f64 / 255.0));
        self.px[i] = n;
    }

    /// Writes a pixel as is (no blending).
    pub fn put(&mut self, x: i32, y: i32, c: Rgba) {
        if self.inside(x, y) {
            let i = (y * self.w + x) as usize;
            self.px[i] = c;
        }
    }

    pub fn get(&self, x: i32, y: i32) -> Rgba {
        if self.inside(x, y) {
            self.px[(y * self.w + x) as usize]
        } else {
            NONE
        }
    }

    pub fn clear(&mut self, x: i32, y: i32) {
        self.put(x, y, NONE);
    }

    pub fn px(&mut self, x: i32, y: i32, c: Rgba) {
        if c.a > 0 {
            self.set(x, y, c);
        }
    }

    pub fn fill(&mut self, c: Rgba) {
        let (w, h) = (self.w, self.h);
        self.rect(0, 0, w, h, c);
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }

    pub fn circle(&mut self, cx: f64, cy: f64, r: f64, c: Rgba) {
        for y in 0..self.h {
            for x in 0..self.w {
                let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
                if dx * dx + dy * dy <= r * r {
                    self.set(x, y, c);
                }
            }
        }
    }

    /// A shaded sphere lit from the top-left.
    pub fn ball(&mut self, cx: f64, cy: f64, r: f64, c: Rgba, jitter: f64) {
        for y in 0..self.h {
            for x in 0..self.w {
                let (dx, dy) = ((x as f64 + 0.5 - cx) / r, (y as f64 + 0.5 - cy) / r);
                if dx * dx + dy * dy > 1.0 {
                    continue;
                }
                let l = 0.95 - 0.38 * (dx + dy) + (self.r.f64() - 0.5) * jitter;
                self.set(x, y, mul(c, l));
            }
        }
    }

    /// A shaded ellipse lit from the top-left.
    pub fn blob(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, c: Rgba) {
        for y in (cy - ry - 1.0) as i32..=(cy + ry + 1.0) as i32 {
            for x in (cx - rx - 1.0) as i32..=(cx + rx + 1.0) as i32 {
                let (dx, dy) = ((x as f64 + 0.5 - cx) / rx, (y as f64 + 0.5 - cy) / ry);
                if dx * dx + dy * dy > 1.0 {
                    continue;
                }
                let l = 1.02 - 0.22 * dx - 0.3 * dy;
                self.set(x, y, mul(c, l));
            }
        }
    }

    /// A line of a given width.
    pub fn thick(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, w: f64, c: Rgba) {
        let n = ((x1 - x0).abs().max((y1 - y0).abs()) * 2.0) as i32 + 1;
        for i in 0..=n {
            let t = i as f64 / n as f64;
            let (x, y) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
            for yy in (y - w / 2.0 + 0.5) as i32..(y + w / 2.0 + 0.5) as i32 {
                for xx in (x - w / 2.0 + 0.5) as i32..(x + w / 2.0 + 0.5) as i32 {
                    self.set(xx, yy, c);
                }
            }
            if w <= 1.0 {
                self.set(x as i32, y as i32, c);
            }
        }
    }

    /// Jitters the brightness of opaque pixels.
    pub fn noise(&mut self, amount: f64) {
        for i in 0..self.px.len() {
            let c = self.px[i];
            if c.a == 0 {
                continue;
            }
            let k = 1.0 + (self.r.f64() - 0.5) * amount;
            self.px[i] = mul(c, k);
        }
    }

    pub fn speckle(&mut self, c: Rgba, density: f64) {
        for y in 0..self.h {
            for x in 0..self.w {
                if self.r.f64() < density {
                    self.set(x, y, c);
                }
            }
        }
    }

    pub fn line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, c: Rgba) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let sx = if x0 > x1 { -1 } else { 1 };
        let sy = if y0 > y1 { -1 } else { 1 };
        let mut err = dx + dy;
        loop {
            self.set(x0, y0, c);
            if x0 == x1 && y0 == y1 {
                return;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// Fills a rectangle (x0..x1, y0..y1 exclusive) with simple top-left lighting.
    pub fn bx(&mut self, r: Rect, c: Rgba) {
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                let mut k = if x == r.x1 - 1 && r.dx() > 1 {
                    0.74
                } else if y == r.y0 {
                    1.12
                } else if x == r.x0 && r.dx() > 2 {
                    1.05
                } else {
                    1.0
                };
                if y == r.y1 - 1 && r.dy() > 2 {
                    k *= 0.88;
                }
                self.set(x, y, mul(c, k));
            }
        }
    }

    /// Removes fully transparent rows above the picture.
    pub fn crop_top(&mut self) {
        let mut y0 = 0;
        while y0 < self.h - 1 && (0..self.w).all(|x| self.get(x, y0).a == 0) {
            y0 += 1;
        }
        if y0 > 0 {
            self.px.drain(0..(y0 * self.w) as usize);
            self.h -= y0;
        }
    }

    /// Adds a one pixel dark outline around the opaque pixels.
    pub fn outlined(&self) -> Pc {
        let mut o = Pc::new(self.w + 2, self.h + 2, "outline");
        for y in 0..self.h {
            for x in 0..self.w {
                let c = self.get(x, y);
                if c.a > 0 {
                    o.put(x + 1, y + 1, c);
                }
            }
        }
        for y in 0..o.h {
            for x in 0..o.w {
                if o.get(x, y).a > 0 {
                    continue;
                }
                let mut sum = [0.0f64; 3];
                let mut n = 0;
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let c = self.get(x - 1 + dx, y - 1 + dy);
                    if c.a > 120 {
                        sum[0] += c.r as f64;
                        sum[1] += c.g as f64;
                        sum[2] += c.b as f64;
                        n += 1;
                    }
                }
                if n > 0 {
                    let k = 0.22 / n as f64;
                    o.put(
                        x,
                        y,
                        Rgba {
                            r: (sum[0] * k) as u8,
                            g: (sum[1] * k) as u8,
                            b: (sum[2] * k) as u8,
                            a: 235,
                        },
                    );
                }
            }
        }
        o
    }

    /// RGBA bytes, row by row.
    pub fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.px.len() * 4);
        for c in &self.px {
            out.extend_from_slice(&[c.r, c.g, c.b, c.a]);
        }
        out
    }
}

/// An integer rectangle with exclusive max edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Rect {
    pub const fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Rect {
        Rect { x0, y0, x1, y1 }
    }
    /// Inclusive corners, like the hand-written builds.
    pub const fn rc(x0: i32, y0: i32, x1: i32, y1: i32) -> Rect {
        Rect {
            x0,
            y0,
            x1: x1 + 1,
            y1: y1 + 1,
        }
    }
    pub fn dx(&self) -> i32 {
        self.x1 - self.x0
    }
    pub fn dy(&self) -> i32 {
        self.y1 - self.y0
    }
    pub fn shift(&self, dx: i32, dy: i32) -> Rect {
        Rect {
            x0: self.x0 + dx,
            y0: self.y0 + dy,
            x1: self.x1 + dx,
            y1: self.y1 + dy,
        }
    }
}

/// A white radial gradient used for glows, particles and shadows.
pub fn soft_dot(size: i32) -> Pc {
    let mut p = Pc::new(size, size, "dot");
    let c = size as f64 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let d = ((x as f64 + 0.5 - c).hypot(y as f64 + 0.5 - c)) / c;
            let mut a = (1.0 - d).max(0.0);
            a = a * a * (3.0 - 2.0 * a);
            p.put(
                x,
                y,
                Rgba {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: (255.0 * a) as u8,
                },
            );
        }
    }
    p
}

/// Darkens the corners of the screen.
pub fn vignette(size: i32) -> Pc {
    let mut p = Pc::new(size, size, "vignette");
    let c = size as f64 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let d = ((x as f64 + 0.5 - c).hypot(y as f64 + 0.5 - c)) / c;
            let a = ((d - 0.55) / 0.75).clamp(0.0, 1.0);
            p.put(
                x,
                y,
                Rgba {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: (255.0 * a * a) as u8,
                },
            );
        }
    }
    p
}

/// The window icon: a golden ring with a tree, scaled to size.
pub fn app_icon(size: i32) -> Vec<u8> {
    let mut p = Pc::new(16, 16, "icon");
    p.circle(8.0, 8.0, 7.8, rgb(230, 180, 70));
    p.circle(8.0, 8.0, 6.6, rgb(18, 20, 36));
    p.rect(7, 9, 2, 4, BARK);
    p.ball(8.0, 7.0, 4.0, rgb(60, 160, 70), 0.2);
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let c = p.get(x * 16 / size, y * 16 / size);
            out.extend_from_slice(&[c.r, c.g, c.b, c.a]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors() {
        assert_eq!(hex("#ff8000"), rgb(255, 128, 0));
        assert_eq!(hex("bad"), rgb(128, 128, 128));
        assert_eq!(mul(rgb(100, 200, 50), 2.0), rgb(200, 255, 100));
    }

    #[test]
    fn crop_and_outline() {
        let mut p = Pc::new(4, 6, "t");
        p.set(1, 3, WHITE);
        p.crop_top();
        assert_eq!(p.h, 3);
        assert_eq!(p.get(1, 0), WHITE);
        let o = p.outlined();
        assert_eq!((o.w, o.h), (6, 5));
        assert!(o.get(2, 0).a > 0);
    }
}
