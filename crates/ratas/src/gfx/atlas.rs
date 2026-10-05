//! The world map, drawn as an old atlas: the explored land painted on
//! parchment (coasts inked, forests dotted, mountains as little peaks), the
//! unknown left blank, and villages, dungeons, sights, quest goals and
//! companions marked on top. The same picture serves the minimap.

use macroquad::prelude::*;

use ratas_core::content::{self, TileDef};
use ratas_core::proto::{Place, Snapshot};
use ratas_core::world::{Bitset, Level};

use super::*;
use crate::art::{self, hex, mix, mul, Rgba};

/// Pixels per tile of the painted map.
const MAP_PX: i32 = 4;

const PARCHMENT: Rgba = art::rgb(0xe9, 0xd9, 0xb2);
const INK: Rgba = art::rgb(0x3b, 0x2a, 0x1a);
const INK_SOFT: Rgba = art::rgb(0x6a, 0x52, 0x36);

fn ink() -> Color {
    rgba(INK)
}

/// The atlas colour of a tile.
fn map_color(def: &TileDef) -> Rgba {
    let k = def.key.as_str();
    let h = match k {
        "deep_water" => "#3d6c98",
        "water" => "#6496c0",
        "lava" | "magma_crack" => "#d8642c",
        "ice" | "ice_floor" => "#c4dcea",
        "snow_pine" => "#5f8270",
        "sand" | "desert_sand" | "dunes" | "sandstone_floor" => "#e4c88c",
        "cactus" | "palm" => "#a8b060",
        "tree" | "pine" | "bush" => "#4f7a42",
        "forest_floor" => "#78a058",
        "grass" | "grass2" | "tall_grass" | "flowers" | "garden" => "#98b872",
        "swamp" | "reeds" => "#7a8650",
        "dead_tree" | "twisted_tree" | "blight_grass" | "mushrooms" => "#7e7860",
        "gravestone" | "bones" => "#8e8a7e",
        "hill" => "#b0a072",
        "mountain" | "ice_rock" => "#9a8e78",
        "ash" | "basalt" | "charred_tree" | "basalt_floor" => "#6e6460",
        "road" | "bridge" | "cobblestone" | "city_gate" => "#c4a476",
        "house_wall" => "#9a4e36",
        "city_wall" => "#7a7068",
        "house_floor" | "door" | "door_open" | "carpet" => "#c8a478",
        "dungeon" | "stairs_down" | "stairs_up" => "#2a1a14",
        "void" => "#1c1612",
        _ if k.contains("snow") => "#f2f2ee",
        _ => "",
    };
    if !h.is_empty() {
        return hex(h);
    }
    if !def.walkable && !def.transparent {
        return hex("#5e5448");
    }
    let (fg, bg) = art::tiles::tile_colors(def);
    mix(mix(bg, fg, 0.55), PARCHMENT, 0.25)
}

fn is_water(def: &TileDef) -> bool {
    def.key == "water" || def.key == "deep_water"
}

/// Paints the explored land of a level on parchment.
pub fn paint_map(l: &Level, explored: Option<&Bitset>) -> (i32, i32, Vec<u8>) {
    let (w, h) = (l.w * MAP_PX, l.h * MAP_PX);
    let mut img = vec![0u8; (w * h * 4) as usize];
    let set = |img: &mut Vec<u8>, x: i32, y: i32, c: Rgba| {
        if x >= 0 && y >= 0 && x < w && y < h {
            let o = ((y * w + x) * 4) as usize;
            img[o..o + 4].copy_from_slice(&[c.r, c.g, c.b, 255]);
        }
    };
    let seen =
        |x: i32, y: i32| l.inside(x, y) && explored.map_or(true, |e| e.get((y * l.w + x) as usize));
    for y in 0..h {
        for x in 0..w {
            let n = (tile_hash(x * 7, y * 13) % 100) as f64 / 100.0 - 0.5;
            set(&mut img, x, y, mul(PARCHMENT, 1.0 + n * 0.05));
        }
    }
    let db = content::db();
    let at = |x: i32, y: i32| db.tile(l.at(x, y));
    for ty in 0..l.h {
        for tx in 0..l.w {
            if !seen(tx, ty) {
                continue;
            }
            let def = at(tx, ty);
            let mut base = map_color(def);
            let (mut edge, mut coast) = (false, false);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (tx + dx, ty + dy);
                if !l.inside(nx, ny) {
                    continue;
                }
                if !seen(nx, ny) {
                    edge = true;
                } else if is_water(at(nx, ny)) != is_water(def) {
                    coast = true;
                }
            }
            if is_water(def) && coast {
                base = mix(base, hex("#a8cce0"), 0.45);
            }
            for py in 0..MAP_PX {
                for px in 0..MAP_PX {
                    let (x, y) = (tx * MAP_PX + px, ty * MAP_PX + py);
                    let n = (tile_hash(x * 3, y * 5) % 100) as f64 / 100.0 - 0.5;
                    let mut c = mix(mul(base, 1.0 + n * 0.06), PARCHMENT, 0.14);
                    if edge {
                        c = mix(c, PARCHMENT, 0.5);
                    }
                    set(&mut img, x, y, c);
                }
            }
            let (ox, oy) = (tx * MAP_PX, ty * MAP_PX);
            let hh = tile_hash(tx, ty) & 0xffff;
            match def.key.as_str() {
                "water" | "deep_water" if hh % 7 == 0 && !coast => {
                    let wc = mix(base, art::models::C_WHITE, 0.35);
                    set(&mut img, ox, oy + 2, wc);
                    set(&mut img, ox + 1, oy + 1, wc);
                    set(&mut img, ox + 2, oy + 2, wc);
                }
                "tree" | "pine" | "snow_pine" | "bush" | "dead_tree" | "twisted_tree"
                | "charred_tree" | "palm" | "cactus"
                    if hh % 2 == 0 =>
                {
                    let c = mul(base, 0.7);
                    set(&mut img, ox + 1, oy + 1, c);
                    set(&mut img, ox + 2, oy + 1, c);
                    set(&mut img, ox + 1, oy + 2, mul(c, 0.85));
                    set(&mut img, ox + 2, oy + 2, mul(c, 0.7));
                    set(&mut img, ox + 1, oy + 3, INK_SOFT);
                }
                "mountain" | "ice_rock" if hh % 3 == 0 => {
                    for i in 0..4 {
                        set(&mut img, ox - i + 1, oy + 1 + i, mul(INK, 1.2));
                        set(&mut img, ox + i + 2, oy + 1 + i, mul(base, 0.75));
                    }
                    set(&mut img, ox + 1, oy, art::models::C_WHITE);
                    set(&mut img, ox + 2, oy, mix(art::models::C_WHITE, base, 0.4));
                }
                "hill" if hh % 4 == 0 => {
                    let c = mul(base, 0.72);
                    set(&mut img, ox, oy + 2, c);
                    set(&mut img, ox + 1, oy + 1, c);
                    set(&mut img, ox + 2, oy + 1, c);
                    set(&mut img, ox + 3, oy + 2, c);
                }
                "swamp" | "reeds" if hh % 3 == 0 => {
                    set(&mut img, ox + 1, oy + 1, hex("#56603a"));
                    set(&mut img, ox + 1, oy + 2, hex("#56603a"));
                    set(&mut img, ox + 3, oy + 3, hex("#6a8aa0"));
                }
                "gravestone" => {
                    set(&mut img, ox + 1, oy + 1, INK_SOFT);
                    set(&mut img, ox + 1, oy + 2, INK_SOFT);
                    set(&mut img, ox, oy + 1, INK_SOFT);
                    set(&mut img, ox + 2, oy + 1, INK_SOFT);
                }
                _ => {}
            }
            if coast && !is_water(def) {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (tx + dx, ty + dy);
                    if !l.inside(nx, ny) || !seen(nx, ny) || !is_water(at(nx, ny)) {
                        continue;
                    }
                    for i in 0..MAP_PX {
                        let (x, y) = match (dx, dy) {
                            (1, _) => (ox + MAP_PX - 1, oy + i),
                            (-1, _) => (ox, oy + i),
                            (_, 1) => (ox + i, oy + MAP_PX - 1),
                            _ => (ox + i, oy),
                        };
                        set(&mut img, x, y, mix(INK, base, 0.35));
                    }
                }
            }
        }
    }
    (w, h, img)
}

/// The painted map of the current level, repainted when the land or the
/// explored area changes.
#[derive(Default)]
pub struct Atlas {
    tex: Option<Texture2D>,
    level: String,
    ver: u64,
    seen: usize,
    at: f32,
}

impl Atlas {
    pub fn texture(&mut self, l: &Level, ver: u64, explored: &Bitset, now: f32) -> Texture2D {
        let seen: usize = explored.0.iter().map(|b| b.count_ones() as usize).sum();
        if let Some(t) = &self.tex {
            if self.level == l.id && self.ver == ver && (self.seen == seen || now - self.at < 1.0) {
                return t.clone();
            }
        }
        let (w, h, img) = paint_map(l, Some(explored));
        let t = Texture2D::from_rgba8(w as u16, h as u16, &img);
        t.set_filter(FilterMode::Linear);
        self.tex = Some(t.clone());
        self.level = l.id.clone();
        self.ver = ver;
        self.seen = seen;
        self.at = now;
        t
    }
}

// ---- markers ----

fn poly(pts: &[(f32, f32)]) -> Vec<Vec2> {
    pts.iter().map(|p| vec2(p.0, p.1)).collect()
}

fn fill_poly(pts: &[Vec2], c: Color) {
    let n = pts.len() as f32;
    let cx = pts.iter().map(|p| p.x).sum::<f32>() / n;
    let cy = pts.iter().map(|p| p.y).sum::<f32>() / n;
    fill_fan(cx, cy, pts, c);
}

pub fn mark_village(x: f32, y: f32, s: f32) {
    let walls = poly(&[
        (x - s * 0.5, y + s * 0.45),
        (x - s * 0.5, y - s * 0.05),
        (x + s * 0.5, y - s * 0.05),
        (x + s * 0.5, y + s * 0.45),
    ]);
    let roof = poly(&[(x - s * 0.65, y), (x, y - s * 0.6), (x + s * 0.65, y)]);
    fill_poly(&walls, col("#f4e8c8"));
    stroke_poly(&walls, s * 0.12, ink());
    fill_poly(&roof, col("#b04a32"));
    stroke_poly(&roof, s * 0.12, ink());
    draw_rectangle(x - s * 0.1, y + s * 0.15, s * 0.2, s * 0.3, ink());
}

pub fn mark_dungeon(x: f32, y: f32, s: f32, glow: Color) {
    let rock = poly(&[
        (x - s * 0.6, y + s * 0.4),
        (x - s * 0.45, y - s * 0.2),
        (x, y - s * 0.55),
        (x + s * 0.45, y - s * 0.2),
        (x + s * 0.6, y + s * 0.4),
    ]);
    fill_poly(&rock, col("#8a7a64"));
    stroke_poly(&rock, s * 0.12, ink());
    let door = mix_c(col("#1a0e0a"), glow, 0.35);
    draw_rectangle(x - s * 0.25, y, s * 0.5, s * 0.4, door);
    draw_circle(x, y, s * 0.25, door);
}

pub fn mark_sight(x: f32, y: f32, s: f32, kind: &str) {
    let c = match kind {
        "shrine" => col("#e0b030"),
        "graveyard" => col("#8a8a90"),
        "camp" => col("#c0502a"),
        "oasis" => col("#3aa0a0"),
        "circle" => col("#9a7ac0"),
        _ => col("#c8962a"),
    };
    let d = poly(&[
        (x, y - s * 0.5),
        (x + s * 0.38, y),
        (x, y + s * 0.5),
        (x - s * 0.38, y),
    ]);
    fill_poly(&d, c);
    stroke_poly(&d, s * 0.12, ink());
    draw_circle(x, y, s * 0.1, WHITE);
}

pub fn mark_quest(x: f32, y: f32, s: f32, pulse: f32) {
    draw_circle_lines(
        x,
        y,
        s * (0.55 + 0.25 * pulse),
        s * 0.1,
        with_a(col("#d02020"), 0.9 - 0.5 * pulse),
    );
    draw_triangle(
        vec2(x, y),
        vec2(x - s * 0.32, y - s * 0.55),
        vec2(x + s * 0.32, y - s * 0.55),
        col("#c82020"),
    );
    draw_circle(x, y - s * 0.7, s * 0.34, col("#e03030"));
    draw_circle_lines(x, y - s * 0.7, s * 0.34, s * 0.08, ink());
    draw_circle(x - s * 0.1, y - s * 0.8, s * 0.09, WHITE);
}

pub fn mark_hero(x: f32, y: f32, s: f32, c: Color, angle: f32) {
    let (ca, sa) = (angle.cos(), angle.sin());
    let pt = |fx: f32, fy: f32| vec2(x + (fx * ca - fy * sa) * s, y + (fx * sa + fy * ca) * s);
    let arrow = vec![pt(0.7, 0.0), pt(-0.45, -0.5), pt(-0.2, 0.0), pt(-0.45, 0.5)];
    let tip = pt(-0.2, 0.0);
    fill_fan(tip.x, tip.y, &arrow, c);
    stroke_poly(&arrow, s * 0.14, ink());
}

pub fn theme_glow(theme: &str) -> Color {
    col(match theme {
        "ice" => "#7ad0ff",
        "crypt" => "#8aff7a",
        "volcano" => "#ff6a1a",
        "fortress" => "#c05aff",
        "temple" => "#ffe08a",
        _ => "#ff9a3a",
    })
}

/// Text with a parchment halo so it reads over the land.
fn ink_text(g: &Gfx, s: &str, x: f32, y: f32, size: f32, fg: Color, halo: Color, bold: bool) {
    let w = g.measure(s, size, bold);
    let (tx, ty) = (x - w / 2.0, y - size * 0.55);
    let o = (size / 11.0).max(1.0);
    for (dx, dy) in [
        (-o, 0.0),
        (o, 0.0),
        (0.0, -o),
        (0.0, o),
        (-o, -o),
        (o, o),
        (-o, o),
        (o, -o),
    ] {
        g.text_raw(s, tx + dx, ty + dy, size, halo, bold);
    }
    g.text_raw(s, tx, ty, size, fg, bold);
}

fn kind_of(p: &Place) -> &'static str {
    if p.kind == "village" || p.kind == "city" {
        "village"
    } else if p.kind == "quest" {
        "quest"
    } else if p.kind.starts_with("dungeon") {
        "dungeon"
    } else if p.kind.starts_with("region:") {
        "region"
    } else {
        "sight"
    }
}

struct Labels {
    bounds: Rect,
    taken: Vec<Rect>,
}

impl Labels {
    fn place(
        &mut self,
        g: &Gfx,
        s: &str,
        size: f32,
        bold: bool,
        x: f32,
        y: f32,
    ) -> Option<(f32, f32)> {
        let w = g.measure(s, size, bold) + 4.0;
        let h = size * 1.2;
        let mut r = Rect::new(x - w / 2.0, y - h / 2.0, w, h);
        let b = self.bounds;
        if r.x < b.x + 2.0 {
            r.x = b.x + 2.0;
        }
        if r.right() > b.right() - 2.0 {
            r.x = b.right() - 2.0 - r.w;
        }
        if r.y < b.y || r.bottom() > b.bottom() || r.x < b.x {
            return None;
        }
        if self.taken.iter().any(|o| o.overlaps(&r)) {
            return None;
        }
        self.taken.push(r);
        Some((r.x + r.w / 2.0, r.y + r.h / 2.0))
    }
}

/// Draws the atlas of the current level over an area.
pub fn draw_world_map(
    g: &Gfx,
    tex: &Texture2D,
    l: &Level,
    snap: &Snapshot,
    places: &[Place],
    you_facing: f32,
    area: Rect,
    t: f32,
) {
    let s = g.s;
    draw_rectangle(area.x, area.y, area.w, area.h, col("#2a1e14"));
    let pad = 22.0 * s;
    let title_h = 40.0 * s;
    let k = ((area.w - 4.0 * pad) / l.w as f32)
        .min((area.h - title_h - 3.0 * pad) / l.h as f32)
        .clamp(0.5, 16.0 * s);
    let (mw, mh) = (l.w as f32 * k, l.h as f32 * k);
    let ox = area.x + (area.w - mw) / 2.0;
    let oy = area.y + title_h + pad + (area.h - title_h - 3.0 * pad - mh) / 2.0;
    let at = |x: f32, y: f32| (ox + x * k, oy + y * k);
    let sheet = Rect::new(
        ox - pad * 1.2,
        oy - title_h - pad * 0.4,
        mw + pad * 2.4,
        mh + title_h + pad * 1.6,
    );
    round_rect(
        sheet.x + 3.0,
        sheet.y + 5.0,
        sheet.w,
        sheet.h,
        6.0 * s,
        Color::new(0.0, 0.0, 0.0, 0.47),
    );
    round_rect(sheet.x, sheet.y, sheet.w, sheet.h, 6.0 * s, rgba(PARCHMENT));
    draw_texture_ex(
        tex,
        ox,
        oy,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(mw, mh)),
            ..Default::default()
        },
    );
    // a double frame with corner ornaments
    let (fx0, fy0, fw, fh) = (ox - 6.0 * s, oy - 6.0 * s, mw + 12.0 * s, mh + 12.0 * s);
    draw_rectangle_lines(fx0, fy0, fw, fh, 2.0 * s, ink());
    draw_rectangle_lines(
        fx0 + 4.0 * s,
        fy0 + 4.0 * s,
        fw - 8.0 * s,
        fh - 8.0 * s,
        s,
        rgba(INK_SOFT),
    );
    for (cx, cy) in [
        (fx0, fy0),
        (fx0 + fw, fy0),
        (fx0, fy0 + fh),
        (fx0 + fw, fy0 + fh),
    ] {
        let d = poly(&[
            (cx, cy - 6.0 * s),
            (cx + 6.0 * s, cy),
            (cx, cy + 6.0 * s),
            (cx - 6.0 * s, cy),
        ]);
        fill_poly(&d, col("#a8402a"));
        stroke_poly(&d, s, ink());
    }
    // the title ribbon
    let name = if l.id == "overworld" {
        tr(&format!("Карта мира — {}", l.name))
    } else {
        tr(&l.name)
    };
    let fs = 20.0 * s;
    let tw = g.measure(&name, fs, true) + 40.0 * s;
    let (tx, ty) = (sheet.x + sheet.w / 2.0, sheet.y + title_h * 0.62);
    let rib = poly(&[
        (tx - tw / 2.0 - 14.0 * s, ty - 13.0 * s),
        (tx + tw / 2.0 + 14.0 * s, ty - 13.0 * s),
        (tx + tw / 2.0, ty),
        (tx + tw / 2.0 + 14.0 * s, ty + 13.0 * s),
        (tx - tw / 2.0 - 14.0 * s, ty + 13.0 * s),
        (tx - tw / 2.0, ty),
    ]);
    draw_rectangle(tx - tw / 2.0, ty - 13.0 * s, tw, 26.0 * s, col("#8a2e22"));
    draw_triangle(
        rib[0],
        vec2(tx - tw / 2.0, ty - 13.0 * s),
        rib[5],
        col("#8a2e22"),
    );
    draw_triangle(
        rib[4],
        vec2(tx - tw / 2.0, ty + 13.0 * s),
        rib[5],
        col("#8a2e22"),
    );
    draw_triangle(
        rib[1],
        vec2(tx + tw / 2.0, ty - 13.0 * s),
        rib[2],
        col("#8a2e22"),
    );
    draw_triangle(
        rib[3],
        vec2(tx + tw / 2.0, ty + 13.0 * s),
        rib[2],
        col("#8a2e22"),
    );
    stroke_poly(&rib, 1.5 * s, ink());
    g.text_center(&name, tx, ty, fs, col("#f6e8c8"), true, true);

    let small = 11.0 * s;
    let label = 13.0 * s;
    let ms = (k * 3.0).clamp(12.0 * s, 20.0 * s);
    let halo = with_a(rgba(PARCHMENT), 0.9);
    let mut labels = Labels {
        bounds: Rect::new(ox, oy, mw, mh),
        taken: vec![],
    };
    let me = &snap.you;
    let (sx, sy) = at(me.x, me.y);
    let pulse = 0.5 + 0.5 * (t * 4.0).sin();
    let places: Vec<&Place> = if l.id == "overworld" {
        places.iter().collect()
    } else {
        vec![]
    };
    struct Person {
        name: String,
        x: f32,
        y: f32,
        c: Color,
    }
    let mut people = vec![];
    for m in &me.party {
        if m.level_id == l.id && ((m.x - me.x).abs() + (m.y - me.y).abs()) > 0.5 {
            people.push(Person {
                name: m.name.clone(),
                x: m.x,
                y: m.y,
                c: col("#5ad06a"),
            });
        }
    }
    for e in &snap.entities {
        if e.kind == crate::art::specs::KIND_PLAYER
            && !e.ally
            && !e.dead
            && ((e.x - me.x).abs() + (e.y - me.y).abs()) > 0.5
        {
            people.push(Person {
                name: e.name.clone(),
                x: e.x,
                y: e.y,
                c: col(&e.color),
            });
        }
    }
    let boxr = |x: f32, y: f32, s: f32| Rect::new(x - s * 0.6, y - s * 0.7, s * 1.2, s * 1.2);
    labels.taken.push(boxr(sx, sy, ms * 1.4));
    for p in &places {
        if kind_of(p) != "region" {
            let (x, y) = at(p.x as f32 + 0.5, p.y as f32 + 0.5);
            labels.taken.push(boxr(x, y, ms));
        }
    }
    for p in &people {
        let (x, y) = at(p.x, p.y);
        labels.taken.push(boxr(x, y, ms * 0.8));
    }
    let overworld = l.id == "overworld";
    let legend = legend_rect(g, ox + 12.0 * s, oy + mh - 12.0 * s, small, ms, overworld);
    labels.taken.push(legend);
    let (ccx, ccy) = (ox + mw - 30.0 * s, oy + mh - 34.0 * s);
    labels.taken.push(Rect::new(
        ccx - 26.0 * s,
        ccy - 34.0 * s,
        52.0 * s,
        60.0 * s,
    ));

    let above = |y: f32, f: f32| y - ms * 0.75 - f * 0.65 - 1.0;
    let below = |y: f32, f: f32| y + ms * 0.55 + f * 0.65 + 1.0;
    let mut texts: Vec<(String, f32, f32, f32, Color, bool)> = vec![];
    let mut regions: Vec<(String, f32, f32)> = vec![];
    let put = |labels: &mut Labels,
               texts: &mut Vec<_>,
               s: String,
               size: f32,
               x: f32,
               y: f32,
               c: Color|
     -> bool {
        if let Some((lx, ly)) = labels.place(g, &s, size, true, x, y) {
            texts.push((s, lx, ly, size, c, true));
            true
        } else {
            false
        }
    };
    for p in places.iter().filter(|p| kind_of(p) == "village") {
        let (x, y) = at(p.x as f32 + 0.5, p.y as f32 + 0.5);
        let n = tr(&p.name);
        if !put(
            &mut labels,
            &mut texts,
            n.clone(),
            label,
            x,
            above(y, label),
            ink(),
        ) {
            put(&mut labels, &mut texts, n, label, x, below(y, label), ink());
        }
    }
    for p in places.iter().filter(|p| kind_of(p) == "quest") {
        let (x, y) = at(p.x as f32 + 0.5, p.y as f32 + 0.5);
        put(
            &mut labels,
            &mut texts,
            tr(&p.name),
            small,
            x,
            below(y, small),
            col("#8a1414"),
        );
    }
    for p in &people {
        let (x, y) = at(p.x, p.y);
        put(
            &mut labels,
            &mut texts,
            p.name.clone(),
            small,
            x,
            above(y, small),
            mul_c(p.c, 0.55),
        );
    }
    for kind in ["dungeon", "sight"] {
        for p in places.iter().filter(|p| kind_of(p) == kind) {
            let (x, y) = at(p.x as f32 + 0.5, p.y as f32 + 0.5);
            let fg = if kind == "dungeon" {
                col("#5a1e14")
            } else {
                rgba(INK_SOFT)
            };
            let n = tr(&p.name);
            if !put(
                &mut labels,
                &mut texts,
                n.clone(),
                small,
                x,
                below(y, small),
                fg,
            ) {
                put(&mut labels, &mut texts, n, small, x, above(y, small), fg);
            }
        }
    }
    for p in places.iter().filter(|p| kind_of(p) == "region") {
        let (x, y) = at(p.x as f32 + 0.5, p.y as f32 + 0.5);
        let mut s2 = tr(&p.name).to_uppercase();
        if s2.chars().count() <= 10 {
            s2 = s2
                .chars()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ");
        }
        for dy in [0.0, -ms, ms, -2.0 * ms, 2.0 * ms] {
            if let Some((lx, ly)) = labels.place(g, &s2, label, false, x, y + dy) {
                regions.push((s2.clone(), lx, ly));
                break;
            }
        }
    }
    // region names lie under everything else, like on old maps
    for (s2, x, y) in &regions {
        ink_text(
            g,
            s2,
            *x,
            *y,
            label,
            with_a(col("#5a3e22"), 0.72),
            with_a(rgba(PARCHMENT), 0.45),
            false,
        );
    }
    for p in &places {
        let (x, y) = at(p.x as f32 + 0.5, p.y as f32 + 0.5);
        match kind_of(p) {
            "sight" => mark_sight(x, y, ms * 0.9, &p.kind),
            "dungeon" => mark_dungeon(x, y, ms, theme_glow(p.kind.trim_start_matches("dungeon:"))),
            _ => {}
        }
    }
    for p in places.iter().filter(|p| kind_of(p) == "village") {
        let (x, y) = at(p.x as f32 + 0.5, p.y as f32 + 0.5);
        mark_village(x, y, if p.kind == "city" { ms * 1.9 } else { ms * 1.2 });
    }
    for p in places.iter().filter(|p| kind_of(p) == "quest") {
        let (x, y) = at(p.x as f32 + 0.5, p.y as f32 + 0.5);
        mark_quest(x, y, ms, pulse);
    }
    for p in &people {
        let (x, y) = at(p.x, p.y);
        draw_circle(x, y, ms * 0.34, p.c);
        draw_circle_lines(x, y, ms * 0.34, (ms * 0.1).max(1.0), ink());
    }
    for (s2, x, y, size, c, bold) in &texts {
        ink_text(g, s2, *x, *y, *size, *c, halo, *bold);
    }
    // you are here
    draw_circle_lines(
        sx,
        sy,
        ms * (0.8 + 0.6 * pulse),
        (ms * 0.14).max(2.0),
        with_a(col("#e03020"), 1.0 - 0.6 * pulse),
    );
    mark_hero(sx, sy, ms * 0.95, col("#ffd24a"), you_facing);
    compass(g, ccx, ccy, 18.0 * s, small);
    map_legend(g, legend, small, ms, overworld);
}

fn compass(g: &Gfx, x: f32, y: f32, s: f32, size: f32) {
    draw_circle_lines(x, y, s * 0.62, (s * 0.04).max(1.0), with_a(ink(), 0.7));
    for i in 0..4 {
        let a = i as f32 * std::f32::consts::FRAC_PI_2 - std::f32::consts::FRAC_PI_2;
        let (ca, sa) = (a.cos(), a.sin());
        let tip = vec2(x + ca * s * 0.9, y + sa * s * 0.9);
        let w = s * 0.16;
        draw_triangle(vec2(x, y), tip, vec2(x - sa * w, y + ca * w), ink());
        draw_triangle(
            vec2(x, y),
            tip,
            vec2(x + sa * w, y - ca * w),
            col("#c8a868"),
        );
    }
    ink_text(
        g,
        &tr("С"),
        x,
        y - s * 1.15,
        size,
        ink(),
        with_a(rgba(PARCHMENT), 0.9),
        true,
    );
}

fn legend_rows(overworld: bool) -> Vec<&'static str> {
    let mut rows = vec!["Вы", "Группа"];
    if overworld {
        rows.extend(["Деревня", "Подземелье", "Место силы", "Цель задания"]);
    }
    rows
}

fn legend_rect(g: &Gfx, x: f32, bottom: f32, size: f32, ms: f32, overworld: bool) -> Rect {
    let rows = legend_rows(overworld);
    let w = rows
        .iter()
        .map(|r| g.measure(&tr(r), size, true))
        .fold(0.0, f32::max);
    let lh = (ms * 1.25).max(18.0 * g.s);
    let (bw, bh) = (
        w + ms * 1.6 + 20.0 * g.s,
        rows.len() as f32 * lh + 12.0 * g.s,
    );
    Rect::new(x, bottom - bh, bw, bh)
}

fn map_legend(g: &Gfx, r: Rect, size: f32, ms: f32, overworld: bool) {
    let s = g.s;
    round_rect(r.x, r.y, r.w, r.h, 5.0 * s, with_a(rgba(PARCHMENT), 0.92));
    draw_rectangle_lines(r.x, r.y, r.w, r.h, s, rgba(INK_SOFT));
    let lh = (ms * 1.25).max(18.0 * s);
    for (i, name) in legend_rows(overworld).iter().enumerate() {
        let cy = r.y + 6.0 * s + (i as f32 + 0.5) * lh;
        let x = r.x + 8.0 * s + ms * 0.6;
        match *name {
            "Вы" => mark_hero(x, cy, ms * 0.6, col("#ffd24a"), 0.0),
            "Группа" => {
                draw_circle(x, cy, ms * 0.3, col("#5ad06a"));
                draw_circle_lines(x, cy, ms * 0.3, (ms * 0.1).max(1.0), ink());
            }
            "Деревня" => mark_village(x, cy + ms * 0.1, ms * 0.9),
            "Подземелье" => mark_dungeon(x, cy, ms * 0.85, col("#ff7a3a")),
            "Место силы" => mark_sight(x, cy, ms * 0.8, "shrine"),
            _ => mark_quest(x, cy + ms * 0.35, ms * 0.75, 0.0),
        }
        g.text(
            name,
            r.x + 12.0 * s + ms * 1.3,
            cy - size * 0.55,
            size,
            ink(),
            true,
        );
    }
}

/// A minimap of the land around the hero, cut from the atlas.
pub fn draw_minimap(
    g: &Gfx,
    tex: &Texture2D,
    l: &Level,
    snap: &Snapshot,
    places: &[Place],
    cx: f32,
    cy: f32,
    half: f32,
    facing: f32,
) {
    let s = g.s;
    let me = &snap.you;
    let span = 48.0f32.min(l.w.max(l.h) as f32);
    let k = half * 2.0 / span; // screen pixels per tile
    round_rect(
        cx - half - 5.0 * s,
        cy - half - 5.0 * s,
        half * 2.0 + 10.0 * s,
        half * 2.0 + 10.0 * s,
        6.0 * s,
        Color::new(0.05, 0.04, 0.03, 0.9),
    );
    draw_rectangle(
        cx - half,
        cy - half,
        half * 2.0,
        half * 2.0,
        rgba(PARCHMENT),
    );
    let tiles = half / k;
    // the part of the map around the hero, clipped at the level's edges
    let (x0, y0) = ((me.x - tiles).max(0.0), (me.y - tiles).max(0.0));
    let (x1, y1) = (
        (me.x + tiles).min(l.w as f32),
        (me.y + tiles).min(l.h as f32),
    );
    if x1 > x0 && y1 > y0 {
        let src = Rect::new(
            x0 * MAP_PX as f32,
            y0 * MAP_PX as f32,
            (x1 - x0) * MAP_PX as f32,
            (y1 - y0) * MAP_PX as f32,
        );
        let (dx, dy) = (cx + (x0 - me.x) * k, cy + (y0 - me.y) * k);
        draw_texture_ex(
            tex,
            dx,
            dy,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2((x1 - x0) * k, (y1 - y0) * k)),
                source: Some(src),
                ..Default::default()
            },
        );
    }
    let at = |x: f32, y: f32| (cx + (x - me.x) * k, cy + (y - me.y) * k);
    let inside =
        |x: f32, y: f32| (x - cx).abs() < half - 3.0 * s && (y - cy).abs() < half - 3.0 * s;
    if l.id == "overworld" {
        for p in places {
            let (x, y) = at(p.x as f32 + 0.5, p.y as f32 + 0.5);
            if !inside(x, y) {
                continue;
            }
            match kind_of(p) {
                "village" => mark_village(x, y, 9.0 * s),
                "dungeon" => mark_dungeon(
                    x,
                    y,
                    8.0 * s,
                    theme_glow(p.kind.trim_start_matches("dungeon:")),
                ),
                "quest" => mark_quest(x, y, 7.0 * s, 0.0),
                "sight" => mark_sight(x, y, 7.0 * s, &p.kind),
                _ => {}
            }
        }
    }
    for e in &snap.entities {
        let (x, y) = at(e.x, e.y);
        if !inside(x, y) || e.dead {
            continue;
        }
        let c = if e.ally {
            col("#5ad06a")
        } else if e.kind == crate::art::specs::KIND_PLAYER {
            col(&e.color)
        } else if e.hostile && e.kind == super::world::KIND_MONSTER {
            col("#d03030")
        } else {
            continue;
        };
        draw_circle(x, y, 2.5 * s, c);
    }
    mark_hero(cx, cy, 7.0 * s, col("#ffd24a"), facing);
    draw_rectangle_lines(
        cx - half - 2.0 * s,
        cy - half - 2.0 * s,
        half * 2.0 + 4.0 * s,
        half * 2.0 + 4.0 * s,
        2.0 * s,
        col("#8a6a3a"),
    );
}
