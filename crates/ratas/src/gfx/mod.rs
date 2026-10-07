//! Drawing helpers on top of macroquad: textures painted by `art`, fonts,
//! additive glows, shapes and text with outlines.

pub mod atlas;
pub mod world;

use std::cell::RefCell;
use std::collections::HashMap;

use macroquad::prelude::*;
use macroquad::text::{load_ttf_font_from_bytes, Font};
use miniquad::{BlendFactor, BlendState, BlendValue, Equation, PipelineParams};

use crate::art::{self, Pc, Rgba};
use ratas_core::content::{self, TileDef};
use ratas_core::i18n;

static FONT_REGULAR: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans.ttf");
static FONT_BOLD: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans-Bold.ttf");

/// Parses "#rrggbb" into a colour (grey when malformed).
pub fn col(s: &str) -> Color {
    rgba(art::hex(s))
}

pub fn rgba(c: Rgba) -> Color {
    Color::from_rgba(c.r, c.g, c.b, c.a)
}

/// The colour with alpha a.
pub fn with_a(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a.clamp(0.0, 1.0))
}

pub fn mul_c(c: Color, k: f32) -> Color {
    Color::new(
        (c.r * k).min(1.0),
        (c.g * k).min(1.0),
        (c.b * k).min(1.0),
        c.a,
    )
}

pub fn mix_c(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

thread_local! {
    static TR: RefCell<(String, HashMap<String, String>)> = RefCell::new((String::new(), HashMap::new()));
}

/// Translates text into the player's language, with a cache: interface
/// text is drawn every frame.
pub fn tr(s: &str) -> String {
    let lang = i18n::lang();
    if lang == i18n::RU || !i18n::has_cyrillic(s) {
        return s.to_string();
    }
    TR.with(|c| {
        let mut c = c.borrow_mut();
        if c.0 != lang {
            c.0 = lang.to_string();
            c.1.clear();
        }
        if let Some(v) = c.1.get(s) {
            return v.clone();
        }
        if c.1.len() > 20000 {
            c.1.clear();
        }
        let v = i18n::t(s);
        c.1.insert(s.to_string(), v.clone());
        v
    })
}

/// A texture from a pixel canvas, drawn with crisp pixels.
pub fn texture(p: &Pc) -> Texture2D {
    let t = Texture2D::from_rgba8(p.w.max(1) as u16, p.h.max(1) as u16, &p.bytes());
    t.set_filter(FilterMode::Nearest);
    t
}

/// The art of one tile type, as textures.
pub struct TileTex {
    /// [variant][frame]
    pub ground: Vec<Vec<Texture2D>>,
    pub object: Vec<Texture2D>,
    pub tall: bool,
    pub wall: bool,
}

/// A creature model: two animation frames.
pub struct ModelTex {
    pub frames: [Texture2D; 2],
    pub float: bool,
}

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    color = color0 / 255.0;
    uv = texcoord;
}"#;

const FRAGMENT: &str = r#"#version 100
varying lowp vec4 color;
varying lowp vec2 uv;
uniform sampler2D Texture;
void main() {
    gl_FragColor = color * texture2D(Texture, uv);
}"#;

pub struct Gfx {
    pub font: Font,
    pub bold: Font,
    pub dot: Texture2D,
    pub vignette: Texture2D,
    /// additive blending for light
    pub add: Material,
    /// interface scale (1 = 1080p-ish at 100%)
    pub s: f32,
    tiles: HashMap<String, TileTex>,
    models: HashMap<String, ModelTex>,
    icons: HashMap<String, Texture2D>,
    ability_icons: HashMap<String, Texture2D>,
    /// identifies the content set the caches were built for
    content_id: usize,
}

impl Gfx {
    pub fn new() -> Gfx {
        let font = load_ttf_font_from_bytes(FONT_REGULAR).expect("font");
        let bold = load_ttf_font_from_bytes(FONT_BOLD).expect("font");
        let dot = texture(&art::soft_dot(64));
        dot.set_filter(FilterMode::Linear);
        let vignette = texture(&art::vignette(256));
        vignette.set_filter(FilterMode::Linear);
        let add = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX,
                fragment: FRAGMENT,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
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
        .expect("additive material");
        Gfx {
            font,
            bold,
            dot,
            vignette,
            add,
            s: 1.0,
            tiles: HashMap::new(),
            models: HashMap::new(),
            icons: HashMap::new(),
            ability_icons: HashMap::new(),
            content_id: 0,
        }
    }

    /// Drops cached art when the content set changed (joining a server with mods).
    pub fn check_content(&mut self) {
        let id = content::db() as *const _ as usize;
        if id != self.content_id {
            self.content_id = id;
            self.tiles.clear();
            self.models.clear();
            self.icons.clear();
            self.ability_icons.clear();
        }
    }

    // ---- art ----

    pub fn tile(&mut self, def: &TileDef) -> &TileTex {
        if !self.tiles.contains_key(&def.key) {
            let tt = match art::tiles::tile_art(def) {
                Some(a) => TileTex {
                    ground: a
                        .ground
                        .iter()
                        .map(|v| v.iter().map(texture).collect())
                        .collect(),
                    object: a.object.iter().map(texture).collect(),
                    tall: a.tall,
                    wall: a.wall,
                },
                None => {
                    // unknown (modded) tile: a coloured block, the glyph is drawn on top
                    let (fg, bg) = art::tiles::tile_colors(def);
                    TileTex {
                        ground: vec![vec![texture(&art::tiles::ground_art("", fg, bg, 0, 0))]],
                        object: vec![],
                        tall: false,
                        wall: false,
                    }
                }
            };
            self.tiles.insert(def.key.clone(), tt);
        }
        &self.tiles[&def.key]
    }

    pub fn model(&mut self, name: &str, color: &str, variant: i32, gear: &[String]) -> &ModelTex {
        let key = format!("{name}|{color}|{variant}|{}", gear.join(","));
        if !self.models.contains_key(&key) {
            let c = art::hex(color);
            let frames = [
                texture(&art::specs::paint_model(name, c, variant, 0, gear)),
                texture(&art::specs::paint_model(name, c, variant, 1, gear)),
            ];
            self.models.insert(
                key.clone(),
                ModelTex {
                    frames,
                    float: art::specs::floats(name),
                },
            );
        }
        &self.models[&key]
    }

    /// The picture of an item (outlined), None when only its glyph can be shown.
    pub fn icon(&mut self, def: &str, glyph: char, color: &str) -> Option<Texture2D> {
        let d = content::db().item(def);
        let shape = art::icons::icon_shape(d, glyph);
        let key = format!("{shape}|{color}");
        if let Some(t) = self.icons.get(&key) {
            return Some(t.clone());
        }
        let p = art::icons::item_icon(&shape, art::hex(color))?;
        let t = texture(&p.outlined());
        self.icons.insert(key, t.clone());
        Some(t)
    }

    /// A round rune for an ability: its colour and a sign by its kind.
    pub fn ability_icon(&mut self, key: &str) -> Texture2D {
        if let Some(t) = self.ability_icons.get(key) {
            return t.clone();
        }
        let d = content::db().ability(key);
        let c = d.map_or(art::rgb(160, 160, 160), |a| art::hex(&a.color));
        let kind = d.map_or("", |a| a.kind.as_str());
        let p = match d.map(|a| a.parts.as_slice()) {
            // a fusion: the runes of both parts split by a golden seam
            Some([x, y, ..]) => paint_fusion(
                &paint_ability(art::hex(&x.color), &x.kind, &x.key),
                &paint_ability(art::hex(&y.color), &y.kind, &y.key),
                key,
            ),
            _ => paint_ability(c, kind, key),
        };
        let t = texture(&p);
        self.ability_icons.insert(key.to_string(), t.clone());
        t
    }

    // ---- text ----

    pub fn font(&self, bold: bool) -> &Font {
        if bold {
            &self.bold
        } else {
            &self.font
        }
    }

    /// The width of text (already translated) at a pixel size.
    pub fn measure(&self, s: &str, size: f32, bold: bool) -> f32 {
        let fs = size.round().max(6.0) as u16;
        measure_text(s, Some(self.font(bold)), fs, 1.0).width
    }

    /// Draws text with its top-left corner at (x, y); returns the right edge.
    pub fn text_raw(&self, s: &str, x: f32, y: f32, size: f32, c: Color, bold: bool) -> f32 {
        let fs = size.round().max(6.0) as u16;
        let base = (y + size * 0.8).round();
        let d = draw_text_ex(
            s,
            x.round(),
            base,
            TextParams {
                font: Some(self.font(bold)),
                font_size: fs,
                font_scale: 1.0,
                color: c,
                ..Default::default()
            },
        );
        x + d.width
    }

    /// Translates and draws text; returns the right edge.
    pub fn text(&self, s: &str, x: f32, y: f32, size: f32, c: Color, bold: bool) -> f32 {
        self.text_raw(&tr(s), x, y, size, c, bold)
    }

    /// Text cut to a width with an ellipsis.
    pub fn text_clip(
        &self,
        s: &str,
        x: f32,
        y: f32,
        w: f32,
        size: f32,
        c: Color,
        bold: bool,
    ) -> f32 {
        let s = tr(s);
        if self.measure(&s, size, bold) <= w {
            return self.text_raw(&s, x, y, size, c, bold);
        }
        let mut out = String::new();
        for ch in s.chars() {
            out.push(ch);
            if self.measure(&format!("{out}…"), size, bold) > w {
                out.pop();
                break;
            }
        }
        out.push('…');
        self.text_raw(&out, x, y, size, c, bold)
    }

    /// Text centred on (x, y), optionally with a dark outline.
    pub fn text_center(
        &self,
        s: &str,
        x: f32,
        y: f32,
        size: f32,
        c: Color,
        bold: bool,
        outline: bool,
    ) {
        let s = tr(s);
        let w = self.measure(&s, size, bold);
        let (tx, ty) = (x - w / 2.0, y - size * 0.55);
        if outline {
            let o = (size / 14.0).max(1.0);
            let oc = Color::new(0.0, 0.0, 0.0, c.a * 0.85);
            for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o)] {
                self.text_raw(&s, tx + dx, ty + dy, size, oc, bold);
            }
        }
        self.text_raw(&s, tx, ty, size, c, bold);
    }

    /// Splits translated text into lines that fit a width.
    pub fn wrap(&self, s: &str, w: f32, size: f32, bold: bool) -> Vec<String> {
        let s = tr(s);
        let mut out = Vec::new();
        for para in s.split('\n') {
            let mut line = String::new();
            for word in para.split_whitespace() {
                let cand = if line.is_empty() {
                    word.to_string()
                } else {
                    format!("{line} {word}")
                };
                if self.measure(&cand, size, bold) <= w || line.is_empty() {
                    line = cand;
                    // a single word longer than the line is cut by characters
                    while self.measure(&line, size, bold) > w && line.chars().count() > 1 {
                        let mut head = String::new();
                        for ch in line.chars() {
                            head.push(ch);
                            if self.measure(&head, size, bold) > w {
                                head.pop();
                                break;
                            }
                        }
                        if head.is_empty() {
                            break;
                        }
                        let rest: String = line.chars().skip(head.chars().count()).collect();
                        out.push(head);
                        line = rest;
                    }
                } else {
                    out.push(std::mem::take(&mut line));
                    line = word.to_string();
                }
            }
            out.push(line);
        }
        while out.last().is_some_and(String::is_empty) && out.len() > 1 {
            out.pop();
        }
        out
    }

    // ---- shapes ----

    /// An additive soft light.
    pub fn glow(&self, x: f32, y: f32, radius: f32, c: Color, a: f32) {
        if a <= 0.0 || radius <= 0.5 {
            return;
        }
        gl_use_material(&self.add);
        draw_texture_ex(
            &self.dot,
            x - radius,
            y - radius,
            Color::new(c.r, c.g, c.b, a.min(1.0)),
            DrawTextureParams {
                dest_size: Some(vec2(radius * 2.0, radius * 2.0)),
                ..Default::default()
            },
        );
        gl_use_default_material();
    }

    /// A soft dark ellipse under a creature.
    pub fn shadow(&self, x: f32, y: f32, w: f32, h: f32, a: f32) {
        draw_texture_ex(
            &self.dot,
            x - w / 2.0,
            y - h / 2.0,
            Color::new(0.0, 0.0, 0.0, a.clamp(0.0, 1.0)),
            DrawTextureParams {
                dest_size: Some(vec2(w, h)),
                ..Default::default()
            },
        );
    }

    pub fn vignette(&self, w: f32, h: f32, a: f32) {
        draw_texture_ex(
            &self.vignette,
            0.0,
            0.0,
            Color::new(1.0, 1.0, 1.0, a),
            DrawTextureParams {
                dest_size: Some(vec2(w, h)),
                ..Default::default()
            },
        );
    }
}

/// Paints a round ability rune: a ring in the ability colour with a sign of
/// its kind (a blade, an arrow, a flame, a cross, a star...).
fn paint_ability(c: Rgba, kind: &str, key: &str) -> Pc {
    use art::{alpha, mix, mul, WHITE};
    let mut p = Pc::new(20, 20, key);
    p.circle(10.0, 10.0, 9.6, mul(c, 0.35));
    p.ball(10.0, 10.0, 8.6, mul(c, 0.7), 0.05);
    p.circle(
        10.0,
        10.0,
        7.2,
        mix(mul(c, 0.25), art::rgb(10, 10, 16), 0.5),
    );
    let fg = mix(c, WHITE, 0.25);
    match kind {
        "strike" | "cleave" | "dash" => {
            p.thick(5.0, 15.0, 14.5, 5.5, 2.0, fg);
            p.thick(4.0, 12.0, 8.0, 16.0, 1.0, art::models::C_GOLD);
        }
        "projectile" | "chain" => {
            p.thick(4.0, 16.0, 15.0, 5.0, 1.0, fg);
            p.thick(15.0, 5.0, 11.0, 5.5, 1.0, WHITE);
            p.thick(15.0, 5.0, 14.5, 9.0, 1.0, WHITE);
        }
        "heal" | "revive" => {
            p.rect(9, 4, 3, 12, fg);
            p.rect(4, 9, 12, 3, fg);
        }
        "buff" | "taunt" | "decoy" => {
            p.circle(10.0, 10.0, 4.6, alpha(fg, 200));
            p.circle(10.0, 10.0, 2.6, mul(c, 0.4));
        }
        "summon" => {
            p.ball(10.0, 8.0, 3.0, fg, 0.0);
            p.rect(7, 11, 7, 5, fg);
        }
        "nova" | "echo" | "death_sentence" => {
            for i in 0..8 {
                let a = i as f64 * std::f64::consts::PI / 4.0;
                p.thick(
                    10.0,
                    10.0,
                    10.0 + a.cos() * 6.0,
                    10.0 + a.sin() * 6.0,
                    1.0,
                    fg,
                );
            }
            p.circle(10.0, 10.0, 2.0, WHITE);
        }
        _ => {
            // a four-pointed spark
            p.thick(10.0, 3.5, 10.0, 16.5, 1.0, fg);
            p.thick(3.5, 10.0, 16.5, 10.0, 1.0, fg);
            p.ball(10.0, 10.0, 2.8, fg, 0.0);
            p.set(9, 9, WHITE);
        }
    }
    p
}

/// The rune of a fused ability: the upper left of one part's rune, the
/// lower right of the other's, a golden seam between them.
fn paint_fusion(a: &Pc, b: &Pc, key: &str) -> Pc {
    let mut p = Pc::new(a.w, a.h, key);
    let n = a.w + a.h - 2;
    for y in 0..a.h {
        for x in 0..a.w {
            let d = x + y - n / 2;
            let c = if d < 0 { a.get(x, y) } else { b.get(x, y) };
            if !c.visible() {
                continue;
            }
            let seam = d == 0 || d == -1;
            p.set(x, y, if seam { art::models::C_GOLD } else { c });
        }
    }
    p
}

// ---- free shape helpers ----

/// A filled rectangle with rounded corners.
pub fn round_rect(x: f32, y: f32, w: f32, h: f32, r: f32, c: Color) {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    if r < 1.0 {
        draw_rectangle(x, y, w, h, c);
        return;
    }
    draw_rectangle(x + r, y, w - 2.0 * r, h, c);
    draw_rectangle(x, y + r, r, h - 2.0 * r, c);
    draw_rectangle(x + w - r, y + r, r, h - 2.0 * r, c);
    for (cx, cy, a0) in [
        (x + r, y + r, 180.0f32),
        (x + w - r, y + r, 270.0),
        (x + w - r, y + h - r, 0.0),
        (x + r, y + h - r, 90.0),
    ] {
        let n = 6;
        for i in 0..n {
            let a1 = (a0 + 90.0 * i as f32 / n as f32).to_radians();
            let a2 = (a0 + 90.0 * (i + 1) as f32 / n as f32).to_radians();
            draw_triangle(
                vec2(cx, cy),
                vec2(cx + a1.cos() * r, cy + a1.sin() * r),
                vec2(cx + a2.cos() * r, cy + a2.sin() * r),
                c,
            );
        }
    }
}

/// The outline of a rounded rectangle.
pub fn round_rect_lines(x: f32, y: f32, w: f32, h: f32, r: f32, t: f32, c: Color) {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    draw_line(x + r, y, x + w - r, y, t, c);
    draw_line(x + r, y + h, x + w - r, y + h, t, c);
    draw_line(x, y + r, x, y + h - r, t, c);
    draw_line(x + w, y + r, x + w, y + h - r, t, c);
    if r >= 1.0 {
        for (cx, cy, a0) in [
            (x + r, y + r, 180.0f32),
            (x + w - r, y + r, 270.0),
            (x + w - r, y + h - r, 0.0),
            (x + r, y + h - r, 90.0),
        ] {
            let n = 5;
            for i in 0..n {
                let a1 = (a0 + 90.0 * i as f32 / n as f32).to_radians();
                let a2 = (a0 + 90.0 * (i + 1) as f32 / n as f32).to_radians();
                draw_line(
                    cx + a1.cos() * r,
                    cy + a1.sin() * r,
                    cx + a2.cos() * r,
                    cy + a2.sin() * r,
                    t,
                    c,
                );
            }
        }
    }
}

/// An ellipse outline.
pub fn ellipse_lines(cx: f32, cy: f32, rx: f32, ry: f32, t: f32, c: Color) {
    let n = 28;
    for i in 0..n {
        let a1 = i as f32 / n as f32 * std::f32::consts::TAU;
        let a2 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
        draw_line(
            cx + a1.cos() * rx,
            cy + a1.sin() * ry,
            cx + a2.cos() * rx,
            cy + a2.sin() * ry,
            t,
            c,
        );
    }
}

/// Fills a polygon that is star-shaped around (cx, cy).
pub fn fill_fan(cx: f32, cy: f32, pts: &[Vec2], c: Color) {
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        draw_triangle(vec2(cx, cy), a, b, c);
    }
}

/// Strokes a closed polygon.
pub fn stroke_poly(pts: &[Vec2], t: f32, c: Color) {
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        draw_line(a.x, a.y, b.x, b.y, t, c);
    }
}

/// A four-pointed sparkle with a dark outline.
pub fn star(cx: f32, cy: f32, rad: f32, c: Color) {
    let pts: Vec<Vec2> = (0..8)
        .map(|i| {
            let a = i as f32 * std::f32::consts::FRAC_PI_4 - std::f32::consts::FRAC_PI_2;
            let d = if i % 2 == 1 { rad * 0.36 } else { rad };
            vec2(cx + a.cos() * d, cy + a.sin() * d)
        })
        .collect();
    stroke_poly(
        &pts,
        (rad * 0.3).max(1.5),
        Color::new(0.08, 0.05, 0.02, 0.8),
    );
    fill_fan(cx, cy, &pts, c);
}

/// Draws a texture into a quad given by four corners (top-left, top-right,
/// bottom-right, bottom-left): used for trees bending in the wind.
pub fn draw_quad(tex: &Texture2D, p: [Vec2; 4], c: Color) {
    let col = [
        (c.r * 255.0) as u8,
        (c.g * 255.0) as u8,
        (c.b * 255.0) as u8,
        (c.a * 255.0) as u8,
    ];
    let v = |q: Vec2, u: f32, w: f32| {
        macroquad::models::Vertex::new(
            q.x,
            q.y,
            0.0,
            u,
            w,
            Color::from_rgba(col[0], col[1], col[2], col[3]),
        )
    };
    let mesh = macroquad::models::Mesh {
        vertices: vec![
            v(p[0], 0.0, 0.0),
            v(p[1], 1.0, 0.0),
            v(p[2], 1.0, 1.0),
            v(p[3], 0.0, 1.0),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: Some(tex.clone()),
    };
    draw_mesh(&mesh);
}

/// Day light 0..1 from the time of day 0..1.
pub fn daylight(t: f64) -> f64 {
    if (0.3..=0.7).contains(&t) {
        1.0
    } else if !(0.15..0.85).contains(&t) {
        0.0
    } else if t < 0.3 {
        (t - 0.15) / 0.15
    } else {
        (0.85 - t) / 0.15
    }
}

/// A stable pseudo-random number for a tile.
pub fn tile_hash(x: i32, y: i32) -> usize {
    let mut h: u32 = 2166136261;
    for b in [x as u8, (x >> 8) as u8, y as u8, (y >> 8) as u8] {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    (h >> 1) as usize
}
