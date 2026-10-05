//! A small immediate-mode interface: the input of a frame, panels, buttons,
//! list rows, bars and text fields. Everything scales with the interface
//! scale and translates its text into the player's language.

use macroquad::prelude::*;

use crate::gfx::{col, round_rect, round_rect_lines, tr, with_a, Gfx};

// ---- theme ----

pub fn c_text() -> Color {
    col("#e8e4d8")
}
pub fn c_dim() -> Color {
    col("#9a96a8")
}
pub fn c_accent() -> Color {
    col("#ffd27a")
}
pub fn c_good() -> Color {
    col("#8ae08a")
}
pub fn c_bad() -> Color {
    col("#ff7a6a")
}
pub fn c_panel() -> Color {
    Color::from_rgba(18, 17, 28, 236)
}
pub fn c_panel2() -> Color {
    Color::from_rgba(30, 28, 44, 240)
}
pub fn c_border() -> Color {
    col("#6a5a3a")
}
pub fn c_sel() -> Color {
    Color::from_rgba(70, 60, 30, 200)
}
pub fn c_hover() -> Color {
    Color::from_rgba(60, 56, 80, 160)
}
pub fn c_hp() -> Color {
    col("#d23c3c")
}
pub fn c_mp() -> Color {
    col("#3c6ed2")
}
pub fn c_xp() -> Color {
    col("#d2b43c")
}
pub fn c_gold() -> Color {
    col("#ffd700")
}

// ---- input ----

/// Keys that repeat while held.
const REPEAT: &[KeyCode] = &[
    KeyCode::Backspace,
    KeyCode::Delete,
    KeyCode::Left,
    KeyCode::Right,
    KeyCode::Up,
    KeyCode::Down,
    KeyCode::PageUp,
    KeyCode::PageDown,
];

/// What happened during one frame.
#[derive(Default)]
pub struct UiInput {
    pub mouse: Vec2,
    pub clicked: bool,
    pub rclicked: bool,
    pub down: bool,
    pub rdown: bool,
    pub wheel: f32,
    /// keys pressed this frame, with auto-repeat for editing keys
    pub keys: Vec<KeyCode>,
    pub chars: Vec<char>,
    pub ctrl: bool,
    pub shift: bool,
    /// a widget took the mouse click
    pub used: bool,
    /// a widget took the keyboard
    pub key_used: bool,
    held: Vec<(KeyCode, f32)>,
    pub dt: f32,
}

impl UiInput {
    pub fn poll(&mut self) {
        let dt = get_frame_time().min(0.1);
        self.dt = dt;
        let (mx, my) = mouse_position();
        self.mouse = vec2(mx, my);
        self.clicked = is_mouse_button_pressed(MouseButton::Left);
        self.rclicked = is_mouse_button_pressed(MouseButton::Right);
        self.down = is_mouse_button_down(MouseButton::Left);
        self.rdown = is_mouse_button_down(MouseButton::Right);
        self.wheel = mouse_wheel().1;
        self.ctrl = is_key_down(KeyCode::LeftControl)
            || is_key_down(KeyCode::RightControl)
            || is_key_down(KeyCode::LeftSuper)
            || is_key_down(KeyCode::RightSuper);
        self.shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        self.used = false;
        self.key_used = false;
        self.keys = get_keys_pressed().into_iter().collect();
        self.keys.sort_by_key(|k| *k as u32);
        // auto-repeat
        for k in REPEAT {
            if is_key_down(*k) {
                match self.held.iter_mut().find(|h| h.0 == *k) {
                    Some(h) => {
                        h.1 += dt;
                        if h.1 > 0.42 {
                            h.1 -= 0.045;
                            if !self.keys.contains(k) {
                                self.keys.push(*k);
                            }
                        }
                    }
                    None => self.held.push((*k, 0.0)),
                }
            } else {
                self.held.retain(|h| h.0 != *k);
            }
        }
        self.chars.clear();
        while let Some(c) = get_char_pressed() {
            if typed(c) && !self.ctrl {
                self.chars.push(c);
            }
        }
    }

    pub fn pressed(&self, k: KeyCode) -> bool {
        !self.key_used && self.keys.contains(&k)
    }

    /// Takes a key press so that nothing else reacts to it.
    pub fn take(&mut self, k: KeyCode) -> bool {
        if self.pressed(k) {
            self.keys.retain(|x| *x != k);
            true
        } else {
            false
        }
    }

    pub fn hover(&self, r: Rect) -> bool {
        r.contains(self.mouse)
    }

    /// A left click inside a rectangle (taken by the first widget).
    pub fn click(&mut self, r: Rect) -> bool {
        if self.clicked && !self.used && r.contains(self.mouse) {
            self.used = true;
            return true;
        }
        false
    }

    pub fn rclick(&mut self, r: Rect) -> bool {
        if self.rclicked && !self.used && r.contains(self.mouse) {
            self.used = true;
            return true;
        }
        false
    }

    /// The wheel over a rectangle.
    pub fn scroll(&mut self, r: Rect) -> f32 {
        if self.wheel != 0.0 && r.contains(self.mouse) {
            let w = self.wheel;
            self.wheel = 0.0;
            return w.signum();
        }
        0.0
    }
}

// ---- widgets ----

/// A panel with a title; returns the inner area.
pub fn panel(g: &Gfx, r: Rect, title: &str) -> Rect {
    let s = g.s;
    round_rect(
        r.x + 4.0 * s,
        r.y + 6.0 * s,
        r.w,
        r.h,
        8.0 * s,
        Color::new(0.0, 0.0, 0.0, 0.45),
    );
    round_rect(r.x, r.y, r.w, r.h, 8.0 * s, c_panel());
    round_rect_lines(r.x, r.y, r.w, r.h, 8.0 * s, 1.5 * s, c_border());
    round_rect_lines(
        r.x + 3.0 * s,
        r.y + 3.0 * s,
        r.w - 6.0 * s,
        r.h - 6.0 * s,
        6.0 * s,
        1.0 * s,
        with_a(c_border(), 0.35),
    );
    let mut top = r.y + 12.0 * s;
    if !title.is_empty() {
        let fs = 18.0 * s;
        let t = tr(title);
        let tw = g.measure(&t, fs, true);
        let ty = r.y + 10.0 * s;
        draw_line(
            r.x + 16.0 * s,
            ty + fs * 0.6,
            r.x + r.w / 2.0 - tw / 2.0 - 12.0 * s,
            ty + fs * 0.6,
            1.0 * s,
            with_a(c_border(), 0.7),
        );
        draw_line(
            r.x + r.w / 2.0 + tw / 2.0 + 12.0 * s,
            ty + fs * 0.6,
            r.x + r.w - 16.0 * s,
            ty + fs * 0.6,
            1.0 * s,
            with_a(c_border(), 0.7),
        );
        g.text_raw(&t, r.x + r.w / 2.0 - tw / 2.0, ty, fs, c_accent(), true);
        top = ty + fs + 12.0 * s;
    }
    Rect::new(
        r.x + 16.0 * s,
        top,
        r.w - 32.0 * s,
        r.y + r.h - top - 12.0 * s,
    )
}

/// A centred rectangle of a wanted size that fits the screen.
pub fn centered(w: f32, h: f32) -> Rect {
    let (sw, sh) = (screen_width(), screen_height());
    let (w, h) = (w.min(sw - 16.0), h.min(sh - 16.0));
    Rect::new(((sw - w) / 2.0).round(), ((sh - h) / 2.0).round(), w, h)
}

/// A button; returns true when clicked.
pub fn button(g: &Gfx, inp: &mut UiInput, r: Rect, label: &str, selected: bool) -> bool {
    let s = g.s;
    let hover = inp.hover(r);
    let bg = if selected {
        Color::from_rgba(96, 78, 36, 235)
    } else if hover {
        Color::from_rgba(64, 58, 84, 235)
    } else {
        Color::from_rgba(38, 35, 54, 235)
    };
    round_rect(r.x, r.y, r.w, r.h, 6.0 * s, bg);
    round_rect_lines(
        r.x,
        r.y,
        r.w,
        r.h,
        6.0 * s,
        1.0 * s,
        if selected || hover {
            c_accent()
        } else {
            c_border()
        },
    );
    let fs = (r.h * 0.45).min(18.0 * s);
    g.text_center(
        label,
        r.x + r.w / 2.0,
        r.y + r.h / 2.0,
        fs,
        if selected { c_accent() } else { c_text() },
        selected,
        false,
    );
    inp.click(r)
}

/// A selectable row background; returns (hovered, clicked).
pub fn row(g: &Gfx, inp: &mut UiInput, r: Rect, selected: bool) -> (bool, bool) {
    let s = g.s;
    let hover = inp.hover(r);
    if selected {
        round_rect(r.x, r.y, r.w, r.h, 4.0 * s, c_sel());
        draw_rectangle(r.x, r.y + 2.0 * s, 3.0 * s, r.h - 4.0 * s, c_accent());
    } else if hover {
        round_rect(r.x, r.y, r.w, r.h, 4.0 * s, c_hover());
    }
    (hover, inp.click(r))
}

/// A progress bar with a label in it.
pub fn bar(g: &Gfx, r: Rect, frac: f32, fill: Color, label: &str) {
    let s = g.s;
    round_rect(
        r.x,
        r.y,
        r.w,
        r.h,
        r.h * 0.3,
        Color::new(0.0, 0.0, 0.0, 0.6),
    );
    let f = frac.clamp(0.0, 1.0);
    if f > 0.0 {
        round_rect(
            r.x + 1.0,
            r.y + 1.0,
            (r.w - 2.0) * f,
            r.h - 2.0,
            r.h * 0.3,
            fill,
        );
        draw_rectangle(
            r.x + 2.0,
            r.y + 2.0,
            ((r.w - 4.0) * f).max(0.0),
            (r.h * 0.3).max(1.0),
            Color::new(1.0, 1.0, 1.0, 0.18),
        );
    }
    round_rect_lines(
        r.x,
        r.y,
        r.w,
        r.h,
        r.h * 0.3,
        1.0 * s,
        Color::new(0.0, 0.0, 0.0, 0.8),
    );
    if !label.is_empty() {
        let fs = (r.h * 0.72).min(15.0 * s);
        g.text_center(
            label,
            r.x + r.w / 2.0,
            r.y + r.h / 2.0,
            fs,
            WHITE,
            true,
            true,
        );
    }
}

/// A footer line of hints at the bottom of a panel.
pub fn footer(g: &Gfx, r: Rect, text: &str) {
    let s = g.s;
    g.text_clip(
        text,
        r.x + 16.0 * s,
        r.y + r.h - 24.0 * s,
        r.w - 32.0 * s,
        13.0 * s,
        c_dim(),
        false,
    );
}

/// Whether a character typed on the keyboard is text. macOS reports arrows,
/// Home, F1 and other function keys as characters of the Unicode private
/// use area (U+F700 and on); they must not end up in text fields.
pub fn typed(c: char) -> bool {
    !c.is_control() && !('\u{E000}'..='\u{F8FF}').contains(&c)
}

/// An editable line of text.
#[derive(Clone, Default)]
pub struct TextInput {
    pub text: String,
    /// cursor position in characters
    pub cursor: usize,
    pub max: usize,
    pub mask: bool,
    blink: f32,
}

impl TextInput {
    pub fn new(max: usize) -> TextInput {
        TextInput {
            max,
            ..Default::default()
        }
    }

    pub fn set(&mut self, s: &str) {
        self.text = s.chars().take(self.max.max(1)).collect();
        self.cursor = self.text.chars().count();
    }

    pub fn value(&self) -> String {
        self.text.trim().to_string()
    }

    fn insert(&mut self, s: &str) {
        for c in s.chars() {
            if !typed(c) {
                continue;
            }
            if self.text.chars().count() >= self.max {
                break;
            }
            let at = self
                .text
                .char_indices()
                .nth(self.cursor)
                .map(|(i, _)| i)
                .unwrap_or(self.text.len());
            self.text.insert(at, c);
            self.cursor += 1;
        }
    }

    /// Takes keys and typed characters; Ctrl+V pastes from the clipboard.
    pub fn handle(&mut self, inp: &mut UiInput) {
        self.blink += inp.dt;
        let chars: Vec<char> = inp.chars.drain(..).collect();
        if !chars.is_empty() {
            self.insert(&chars.iter().collect::<String>());
            self.blink = 0.0;
        }
        let n = self.text.chars().count();
        if inp.ctrl && inp.take(KeyCode::V) {
            if let Some(clip) = miniquad::window::clipboard_get() {
                self.insert(clip.lines().next().unwrap_or(""));
            }
        }
        if inp.ctrl && inp.take(KeyCode::C) && !self.mask {
            miniquad::window::clipboard_set(&self.text);
        }
        if inp.ctrl && inp.take(KeyCode::A) {
            self.text.clear();
            self.cursor = 0;
        }
        if inp.take(KeyCode::Backspace) && self.cursor > 0 {
            let at = self
                .text
                .char_indices()
                .nth(self.cursor - 1)
                .map(|(i, _)| i)
                .unwrap();
            self.text.remove(at);
            self.cursor -= 1;
            self.blink = 0.0;
        }
        if inp.take(KeyCode::Delete) && self.cursor < n {
            let at = self
                .text
                .char_indices()
                .nth(self.cursor)
                .map(|(i, _)| i)
                .unwrap();
            self.text.remove(at);
        }
        if inp.take(KeyCode::Left) {
            self.cursor = self.cursor.saturating_sub(1);
            self.blink = 0.0;
        }
        if inp.take(KeyCode::Right) {
            self.cursor = (self.cursor + 1).min(self.text.chars().count());
            self.blink = 0.0;
        }
        if inp.take(KeyCode::Home) {
            self.cursor = 0;
        }
        if inp.take(KeyCode::End) {
            self.cursor = self.text.chars().count();
        }
    }

    /// Draws the field; focused fields show a blinking cursor.
    pub fn draw(&self, g: &Gfx, r: Rect, focused: bool, placeholder: &str) {
        let s = g.s;
        round_rect(
            r.x,
            r.y,
            r.w,
            r.h,
            4.0 * s,
            if focused {
                Color::from_rgba(36, 48, 74, 240)
            } else {
                Color::from_rgba(26, 26, 42, 240)
            },
        );
        round_rect_lines(
            r.x,
            r.y,
            r.w,
            r.h,
            4.0 * s,
            1.0 * s,
            if focused { c_accent() } else { c_border() },
        );
        let fs = (r.h * 0.52).min(17.0 * s);
        let shown: String = if self.mask {
            "•".repeat(self.text.chars().count())
        } else {
            self.text.clone()
        };
        let pad = 8.0 * s;
        let avail = r.w - pad * 2.0;
        // scroll so that the cursor stays visible
        let before: String = shown.chars().take(self.cursor).collect();
        let cw = g.measure(&before, fs, false);
        let shift = (cw - avail + 4.0 * s).max(0.0);
        let ty = r.y + (r.h - fs) / 2.0 - 1.0;
        if shown.is_empty() && !placeholder.is_empty() && !focused {
            g.text_clip(placeholder, r.x + pad, ty, avail, fs, c_dim(), false);
        }
        // draw only the visible part
        let mut x = r.x + pad - shift;
        for ch in shown.chars() {
            let w = g.measure(&ch.to_string(), fs, false);
            if x + w > r.x + pad - 1.0 && x < r.x + r.w - pad {
                g.text_raw(&ch.to_string(), x, ty, fs, c_text(), false);
            }
            x += w;
        }
        if focused && (self.blink % 1.0) < 0.6 {
            let cx = r.x + pad + cw - shift;
            draw_line(
                cx,
                r.y + 5.0 * s,
                cx,
                r.y + r.h - 5.0 * s,
                1.5 * s,
                c_accent(),
            );
        }
    }
}

/// Wraps and draws a paragraph; returns the y after it.
pub fn paragraph(g: &Gfx, s: &str, x: f32, y: f32, w: f32, size: f32, c: Color) -> f32 {
    let lh = size * 1.3;
    let mut y = y;
    for line in g.wrap(s, w, size, false) {
        g.text_raw(&line, x, y, size, c, false);
        y += lh;
    }
    y
}
