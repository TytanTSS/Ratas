//! Pixel art made finer: a picture is redrawn four times larger with its
//! staircase edges turned into slopes and curves, so the art keeps its
//! shapes and colours but shows no blocks. Each doubling follows the xBR
//! rules (Hyllian's): at every corner of a pixel an edge is traced where
//! the colours along it are closer than the colours across it, and the
//! corner takes the colour beyond the edge, wholly or in part (by how
//! steep the edge runs).

use super::{Pc, Rgba, NONE};

/// A pixel: its colour multiplied by its opacity, and its opacity (0..1).
type P = [f32; 4];

fn load(c: Rgba) -> P {
    let a = c.a as f32 / 255.0;
    [
        c.r as f32 / 255.0 * a,
        c.g as f32 / 255.0 * a,
        c.b as f32 / 255.0 * a,
        a,
    ]
}

fn store(p: P) -> Rgba {
    if p[3] < 0.5 / 255.0 {
        return NONE;
    }
    let k = 255.0 / p[3];
    let c = |v: f32| (v * k).round().clamp(0.0, 255.0) as u8;
    Rgba {
        r: c(p[0]),
        g: c(p[1]),
        b: c(p[2]),
        a: (p[3] * 255.0).round().clamp(0.0, 255.0) as u8,
    }
}

/// How unlike two pixels look: brightness weighs most, then the hues and
/// how opaque they are (black to white is 48).
fn df(a: &P, b: &P) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let y = 0.299 * d[0] + 0.587 * d[1] + 0.114 * d[2];
    let u = -0.169 * d[0] - 0.331 * d[1] + 0.5 * d[2];
    let v = 0.5 * d[0] - 0.419 * d[1] - 0.081 * d[2];
    48.0 * y.abs() + 7.0 * u.abs() + 6.0 * v.abs() + 40.0 * (a[3] - b[3]).abs()
}

fn eq(a: &P, b: &P) -> bool {
    df(a, b) < 1.6
}

fn mix(dst: &mut P, src: &P, w: f32) {
    for k in 0..4 {
        dst[k] += (src[k] - dst[k]) * w;
    }
}

/// The picture twice as large. `wrap`: it repeats (a tile of ground), so
/// its edges continue on the other side; otherwise nothing lies beyond.
fn double(w: usize, h: usize, src: &[P], wrap: bool) -> Vec<P> {
    let (wi, hi) = (w as i32, h as i32);
    let at = |x: i32, y: i32| -> P {
        if wrap {
            src[(y.rem_euclid(hi) * wi + x.rem_euclid(wi)) as usize]
        } else if x < 0 || y < 0 || x >= wi || y >= hi {
            [0.0; 4]
        } else {
            src[(y * wi + x) as usize]
        }
    };
    let mut out = vec![[0.0f32; 4]; w * h * 4];
    for y in 0..hi {
        for x in 0..wi {
            let pe = at(x, y);
            let mut e = [pe; 4];
            // the four corners, each seen as the lower right one of the
            // neighbourhood turned a quarter more
            for k in 0..4 {
                let r = |dx: i32, dy: i32| -> (i32, i32) {
                    let (mut a, mut b) = (dx, dy);
                    for _ in 0..k {
                        (a, b) = (b, -a);
                    }
                    (a, b)
                };
                let n = |dx: i32, dy: i32| {
                    let (a, b) = r(dx, dy);
                    at(x + a, y + b)
                };
                let corner = |sx: i32, sy: i32| {
                    let (a, b) = r(sx, sy);
                    ((b > 0) as usize) * 2 + (a > 0) as usize
                };
                let (pi, ph, pf) = (n(1, 1), n(0, 1), n(1, 0));
                if pe == ph || pe == pf {
                    continue;
                }
                let (pg, pc, pd, pb) = (n(-1, 1), n(1, -1), n(-1, 0), n(0, -1));
                let (f4, i4, h5, i5) = (n(2, 0), n(2, 1), n(0, 2), n(1, 2));
                let ee =
                    df(&pe, &pc) + df(&pe, &pg) + df(&pi, &h5) + df(&pi, &f4) + 4.0 * df(&ph, &pf);
                let ii =
                    df(&ph, &pd) + df(&ph, &i5) + df(&pf, &i4) + df(&pf, &pb) + 4.0 * df(&pe, &pi);
                if ee > ii {
                    continue;
                }
                let px = if df(&pe, &pf) <= df(&pe, &ph) { pf } else { ph };
                let (n3, n2, n1) = (corner(1, 1), corner(-1, 1), corner(1, -1));
                let edge = ee < ii
                    && ((!eq(&pf, &pb) && !eq(&ph, &pd))
                        || (eq(&pe, &pi) && !eq(&pf, &i4) && !eq(&ph, &i5))
                        || eq(&pe, &pg)
                        || eq(&pe, &pc));
                if !edge {
                    mix(&mut e[n3], &px, 0.25);
                    continue;
                }
                let (ke, ki) = (df(&pf, &pg), df(&ph, &pc));
                let ex2 = pe != pc && pb != pc;
                let ex3 = pe != pg && pd != pg;
                let left = 2.0 * ke <= ki && ex3;
                let up = ke >= 2.0 * ki && ex2;
                if left && up {
                    mix(&mut e[n3], &px, 0.875);
                    mix(&mut e[n2], &px, 0.25);
                    e[n1] = e[n2];
                } else if left {
                    mix(&mut e[n3], &px, 0.75);
                    mix(&mut e[n2], &px, 0.25);
                } else if up {
                    mix(&mut e[n3], &px, 0.75);
                    mix(&mut e[n1], &px, 0.25);
                } else {
                    mix(&mut e[n3], &px, 0.5);
                }
            }
            let (ox, oy) = (x as usize * 2, y as usize * 2);
            for (k, p) in e.iter().enumerate() {
                out[(oy + k / 2) * w * 2 + ox + k % 2] = *p;
            }
        }
    }
    out
}

/// How many times larger `finer` draws a picture.
pub const SCALE: i32 = 4;

/// The picture SCALE times larger: smoothed, or (`crisp`) with its pixels
/// kept square. `wrap`: it is a tile that repeats.
pub fn finer(p: &Pc, wrap: bool, crisp: bool) -> Pc {
    let (w, h) = (p.w.max(0) as usize, p.h.max(0) as usize);
    let mut out = Pc::new(p.w * SCALE, p.h * SCALE, "finer");
    if w == 0 || h == 0 {
        return out;
    }
    if crisp {
        for y in 0..out.h {
            for x in 0..out.w {
                out.px[(y * out.w + x) as usize] = p.px[((y / SCALE) * p.w + x / SCALE) as usize];
            }
        }
        return out;
    }
    let src: Vec<P> = p.px.iter().map(|c| load(*c)).collect();
    let twice = double(w, h, &src, wrap);
    let four = double(w * 2, h * 2, &twice, wrap);
    out.px = four.into_iter().map(store).collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::rgb;

    #[test]
    fn a_diagonal_becomes_a_slope() {
        // a dark triangle under the diagonal of a light square
        let mut p = Pc::new(8, 8, "t");
        for y in 0..8 {
            for x in 0..8 {
                p.set(
                    x,
                    y,
                    if x < y {
                        rgb(20, 20, 20)
                    } else {
                        rgb(230, 230, 230)
                    },
                );
            }
        }
        let f = finer(&p, false, false);
        assert_eq!((f.w, f.h), (32, 32));
        // the stair steps get colours between
        let mid =
            f.px.iter()
                .filter(|c| c.r > 40 && c.r < 210 && c.a == 255)
                .count();
        assert!(mid > 20, "{mid}");
        // far from the edge the colours stay as they were
        assert_eq!(f.px[(28 * 32 + 2) as usize].r, 20);
        assert_eq!(f.px[(2 * 32 + 28) as usize].r, 230);
        // crisp keeps the blocks
        let c = finer(&p, false, true);
        assert!(c.px.iter().all(|c| c.r == 20 || c.r == 230));
    }

    #[test]
    fn transparency_stays_clean() {
        // a red dot on nothing: no dark fringe appears around it
        let mut p = Pc::new(6, 6, "t");
        for (x, y) in [(2, 2), (3, 2), (2, 3), (3, 3), (2, 1), (1, 2)] {
            p.set(x, y, rgb(220, 40, 40));
        }
        let f = finer(&p, false, false);
        for c in &f.px {
            if c.a > 0 {
                assert!(c.r > 150, "{c:?}");
            }
        }
    }

    /// Writes art as it was (left) and made finer (right) to RATAS_SHEET.
    #[test]
    #[ignore]
    fn smooth_sheet() {
        use macroquad::color::Color;
        use macroquad::texture::Image;
        let path = std::env::var("RATAS_SHEET").unwrap_or("smooth.png".into());
        let db = ratas_core::content::db();
        let mut pics: Vec<(Pc, bool)> = vec![];
        for n in ["warrior", "mage", "wolf", "skeleton", "dragon", "bear"] {
            pics.push((
                crate::art::specs::paint_model(n, rgb(200, 120, 80), 0, 0, &[]),
                false,
            ));
        }
        for s in ["sword", "potion", "shield", "helmet"] {
            if let Some(i) = crate::art::icons::item_icon(s, rgb(200, 60, 60)) {
                pics.push((i.outlined(), false));
            }
        }
        for k in ["grass", "tree", "water", "stone_wall"] {
            if let Some(t) = db.b.tiles.iter().find(|t| t.key == k) {
                if let Some(a) = crate::art::tiles::tile_art(t) {
                    if let Some(g) = a
                        .ground
                        .into_iter()
                        .next()
                        .and_then(|v| v.into_iter().next())
                    {
                        pics.push((g, true));
                    }
                    if let Some(o) = a.object.into_iter().next() {
                        pics.push((o, false));
                    }
                }
            }
        }
        let k = 6;
        let cell = 32 * k;
        let mut img = Image::gen_image_color(
            (cell * 2) as u16,
            (cell * pics.len() as i32) as u16,
            Color::new(0.2, 0.25, 0.2, 1.0),
        );
        for (i, (p, wrap)) in pics.iter().enumerate() {
            let f = finer(p, *wrap, false);
            for y in 0..p.h * k {
                for x in 0..p.w * k {
                    let put = |img: &mut Image, ox: i32, c: Rgba| {
                        if c.a > 0 {
                            img.set_pixel(
                                (ox + x) as u32,
                                (i as i32 * cell + y) as u32,
                                Color::from_rgba(c.r, c.g, c.b, 255),
                            );
                        }
                    };
                    put(&mut img, 0, p.get(x / k, y / k));
                    let s = f.get(x * SCALE / k, y * SCALE / k);
                    put(&mut img, cell, s);
                }
            }
        }
        img.export_png(&path);
    }
}
