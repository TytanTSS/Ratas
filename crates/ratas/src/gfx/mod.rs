//! Drawing helpers on top of macroquad: textures painted by `art`, fonts,
//! additive glows, shapes and text with outlines.

pub mod atlas;
pub mod world;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use macroquad::prelude::*;
use macroquad::text::{load_ttf_font_from_bytes, Font};
use miniquad::{BlendFactor, BlendState, BlendValue, Equation, PipelineParams};

use crate::art::runes::{Frame, Look};
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

/// Pixel art keeps its square pixels instead of being smoothed.
static CRISP: AtomicBool = AtomicBool::new(false);

/// How many times finer than it is painted pixel art is drawn: a picture
/// of art is ART times wider than its canvas.
pub const ART: f32 = art::smooth::SCALE as f32;

/// Pixel art is smoothed (false) or keeps its square pixels (true).
pub fn crisp() -> bool {
    CRISP.load(Ordering::Relaxed)
}

/// A texture of pixel art, drawn ART times finer than its canvas: its
/// edges smoothed (or its pixels kept square when crisp), filtered
/// smoothly, with smaller copies for when it is drawn small. `wrap`: it is
/// a tile that repeats.
pub fn art_texture(p: &Pc, wrap: bool) -> Texture2D {
    let f = art::smooth::finer(p, wrap, crisp());
    let t = Texture2D::from_rgba8(f.w.max(1) as u16, f.h.max(1) as u16, &f.bytes());
    let id = t.raw_miniquad_id();
    let ctx = unsafe { get_internal_gl() }.quad_context;
    ctx.texture_generate_mipmaps(id);
    ctx.texture_set_filter(id, FilterMode::Linear, miniquad::MipmapFilterMode::Linear);
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
    skill_icons: HashMap<String, Texture2D>,
    /// which picture each ability and skill of the content set shows
    book: Option<art::picks::Book>,
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
            skill_icons: HashMap::new(),
            book: None,
            content_id: 0,
        }
    }

    /// Sets whether pixel art keeps its square pixels; the art is painted
    /// anew when that changes.
    pub fn set_crisp(&mut self, on: bool) {
        if CRISP.swap(on, Ordering::Relaxed) != on {
            self.tiles.clear();
            self.models.clear();
            self.icons.clear();
            self.ability_icons.clear();
            self.skill_icons.clear();
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
            self.skill_icons.clear();
            self.book = None;
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
                        .map(|v| v.iter().map(|p| art_texture(p, true)).collect())
                        .collect(),
                    object: a.object.iter().map(|p| art_texture(p, false)).collect(),
                    tall: a.tall,
                    wall: a.wall,
                },
                None => {
                    // unknown (modded) tile: a coloured block, the glyph is drawn on top
                    let (fg, bg) = art::tiles::tile_colors(def);
                    TileTex {
                        ground: vec![vec![art_texture(
                            &art::tiles::ground_art("", fg, bg, 0, 0),
                            true,
                        )]],
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
                art_texture(&art::specs::paint_model(name, c, variant, 0, gear), false),
                art_texture(&art::specs::paint_model(name, c, variant, 1, gear), false),
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
        let t = art_texture(&p.outlined(), false);
        self.icons.insert(key, t.clone());
        Some(t)
    }

    fn book(&mut self) -> &art::picks::Book {
        self.book
            .get_or_insert_with(|| art::picks::Book::new(content::db()))
    }

    /// The round rune of an ability: a pictogram of what it does in its
    /// colour, a badge for its element; no two abilities share one.
    pub fn ability_icon(&mut self, key: &str) -> Texture2D {
        if let Some(t) = self.ability_icons.get(key) {
            return t.clone();
        }
        let p = match content::db().ability(key) {
            // a fusion: the runes of both parts split by a golden seam
            Some(a) if a.parts.len() >= 2 => {
                let (x, y) = (&a.parts[0], &a.parts[1]);
                let (lx, ly) = (self.book().ability(x), self.book().ability(y));
                art::runes::fusion(
                    &art::runes::paint(lx, art::hex(&x.color), Frame::Rune),
                    &art::runes::paint(ly, art::hex(&y.color), Frame::Rune),
                    key,
                )
            }
            Some(a) => {
                let look = self.book().ability(a);
                art::runes::paint(look, art::hex(&a.color), Frame::Rune)
            }
            None => art::runes::paint(
                Look {
                    glyph: "unknown",
                    badge: "",
                },
                art::rgb(160, 160, 160),
                Frame::Rune,
            ),
        };
        let t = art_texture(&p, false);
        self.ability_icons.insert(key.to_string(), t.clone());
        t
    }

    /// The picture of a skill: the rune of the ability it grants, or a square
    /// plaque in its branch's colour; no two skills share one.
    pub fn skill_icon(&mut self, key: &str) -> Texture2D {
        let db = content::db();
        let Some(sd) = db.skill(key) else {
            return self.ability_icon("");
        };
        if db.ability(&sd.grants).is_some() {
            return self.ability_icon(&sd.grants);
        }
        if let Some(t) = self.skill_icons.get(key) {
            return t.clone();
        }
        let c = db
            .branch(&sd.branch)
            .map_or(art::rgb(160, 160, 160), |b| art::hex(&b.color));
        let look = self.book().skill(sd);
        let t = art_texture(&art::runes::paint(look, c, Frame::Plaque), false);
        self.skill_icons.insert(key.to_string(), t.clone());
        t
    }

    /// The plaque of a hidden skill nobody has opened yet.
    pub fn secret_icon(&mut self) -> Texture2D {
        if let Some(t) = self.skill_icons.get("?") {
            return t.clone();
        }
        let look = Look {
            glyph: "unknown",
            badge: "",
        };
        let t = art_texture(
            &art::runes::paint(look, art::rgb(150, 110, 170), Frame::Plaque),
            false,
        );
        self.skill_icons.insert("?".into(), t.clone());
        t
    }

    /// The shield of a class: its pictogram on a golden rim.
    pub fn class_icon(&mut self, key: &str) -> Texture2D {
        self.shield(key, true)
    }

    /// The shield of a subclass, its class's sign in the corner.
    pub fn subclass_icon(&mut self, key: &str) -> Texture2D {
        self.shield(key, false)
    }

    /// The shield of a secret class or subclass nobody has unlocked yet.
    pub fn secret_shield(&mut self, class: bool) -> Texture2D {
        let id = format!("shield?{class}");
        if let Some(t) = self.skill_icons.get(&id) {
            return t.clone();
        }
        let look = Look {
            glyph: "unknown",
            badge: "",
        };
        let frame = if class { Frame::Class } else { Frame::Subclass };
        let t = art_texture(
            &art::runes::paint(look, art::rgb(150, 110, 170), frame),
            false,
        );
        self.skill_icons.insert(id, t.clone());
        t
    }

    fn shield(&mut self, key: &str, class: bool) -> Texture2D {
        let id = format!("shield:{class}:{key}");
        if let Some(t) = self.skill_icons.get(&id) {
            return t.clone();
        }
        let db = content::db();
        let (look, c, frame) = if class {
            let Some(cl) = db.class(key) else {
                return self.secret_shield(true);
            };
            (self.book().class(cl), art::hex(&cl.color), Frame::Class)
        } else {
            let Some(sc) = db.subclass(key) else {
                return self.secret_shield(false);
            };
            (
                self.book().subclass(sc),
                art::hex(&sc.color),
                Frame::Subclass,
            )
        };
        let t = art_texture(&art::runes::paint(look, c, frame), false);
        self.skill_icons.insert(id, t.clone());
        t
    }

    /// Draws an icon into a square.
    pub fn icon_at(&self, t: &Texture2D, x: f32, y: f32, size: f32, tint: Color) {
        draw_texture_ex(
            t,
            x,
            y,
            tint,
            DrawTextureParams {
                dest_size: Some(vec2(size, size)),
                ..Default::default()
            },
        );
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
