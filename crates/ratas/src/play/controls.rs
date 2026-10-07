//! The controls window (Esc → «Управление», or the button on the hotbar):
//! how many cells the quick-access bar has and which keys fire each cell.

use macroquad::prelude::*;

use ratas_core::config::Config;
use ratas_core::content;
use ratas_core::game::HOTBAR_MAX;

use super::keys::{capture, cell_name, label, Capture, Keys, BINDS, POTION_HEALTH, POTION_MANA};
use super::{Mode, Session};
use crate::gfx::{col, round_rect, round_rect_lines, tr, with_a, Gfx};
use crate::ui::*;

/// The state of the controls window.
#[derive(Default)]
pub struct ControlsUi {
    /// the row under the cursor (0 is the number of cells) and the column
    /// of the binding in it
    pub sel: usize,
    pub col: usize,
    scroll: usize,
    /// a binding waits for a key
    pub capture: bool,
    /// the last change or refusal, and whether it went well
    note: (String, bool),
    /// the number of cells asked of the server and not in the sheet yet
    pending: Option<usize>,
    /// the window to return to
    pub back: Option<Mode>,
}

/// The cell a row shows (row 0 is the number of cells).
fn row_cell(row: usize, n: usize) -> Option<usize> {
    match row {
        0 => None,
        r if r <= n => Some(r - 1),
        r if r == n + 1 => Some(POTION_HEALTH),
        r if r == n + 2 => Some(POTION_MANA),
        _ => None,
    }
}

impl Session {
    pub(super) fn open_controls(&mut self, back: Mode) {
        self.controls = ControlsUi {
            back: Some(back),
            ..Default::default()
        };
        self.mode = Mode::Controls;
    }

    pub(super) fn close_controls(&mut self) {
        self.controls.capture = false;
        self.mode = self.controls.back.take().unwrap_or(Mode::Game);
    }
}

pub fn window(p: &mut Session, g: &mut Gfx, inp: &mut UiInput, cfg: &mut Config) {
    let s = g.s;
    let r = centered(840.0 * s, 700.0 * s);
    let inner = panel(g, r, "Управление");
    let Some(sheet) = p.sheet.clone() else { return };
    let mut ui = std::mem::take(&mut p.controls);
    let have = sheet.hotbar.len();
    if ui.pending == Some(have) {
        ui.pending = None;
    }
    let n = ui.pending.unwrap_or(have);
    let rows = n + 3;
    ui.sel = ui.sel.min(rows - 1);
    let mut keys = Keys::load(cfg);
    let mut changed = false;
    let mut count = n;
    let mut close = false;
    let was_capturing = ui.capture;

    if ui.capture {
        match capture(inp) {
            Capture::Waiting => {}
            Capture::Cancel => ui.capture = false,
            Capture::Refused(why) => ui.note = (why, false),
            Capture::Set(b) => {
                if let Some(cell) = row_cell(ui.sel, n) {
                    let from = keys.set(cell, ui.col, b);
                    ui.note = match (b, from) {
                        (Some(b), Some(f)) => (
                            tr(&format!(
                                "Клавиша {} перенесена: «{}» → «{}».",
                                label(b),
                                cell_name(f),
                                cell_name(cell)
                            )),
                            true,
                        ),
                        (Some(b), None) => (format!("{} — {}", cell_name(cell), label(b)), true),
                        (None, _) => (
                            format!("{} — {}", cell_name(cell), tr("клавиша снята")),
                            true,
                        ),
                    };
                    changed = true;
                }
                ui.capture = false;
            }
        }
    } else {
        if inp.take(KeyCode::Up) {
            ui.sel = (ui.sel + rows - 1) % rows;
        }
        if inp.take(KeyCode::Down) {
            ui.sel = (ui.sel + 1) % rows;
        }
        let (left, right) = (inp.take(KeyCode::Left), inp.take(KeyCode::Right));
        if ui.sel == 0 {
            if left {
                count = count.saturating_sub(1);
            }
            if right {
                count += 1;
            }
        } else if left || right {
            ui.col = (ui.col + 1) % BINDS;
        }
        if inp.take(KeyCode::Enter) && ui.sel > 0 {
            ui.capture = true;
            ui.note = (String::new(), true);
        }
        if inp.take(KeyCode::Backspace) || inp.take(KeyCode::Delete) {
            if let Some(cell) = row_cell(ui.sel, n) {
                keys.set(cell, ui.col, None);
                ui.note = (
                    format!("{} — {}", cell_name(cell), tr("клавиша снята")),
                    true,
                );
                changed = true;
            }
        }
    }

    // ---- the number of cells ----
    let mut y = paragraph(
        g,
        "Сколько ячеек на панели быстрого доступа и какие клавиши их нажимают. Ряд ячеек, который не помещается по ширине экрана, переносится выше.",
        inner.x,
        inner.y,
        inner.w,
        13.0 * s,
        c_dim(),
    ) + 8.0 * s;
    let row_h = 32.0 * s;
    let cr = Rect::new(inner.x, y, inner.w, row_h + 4.0 * s);
    let (_, clicked) = row(g, inp, cr, ui.sel == 0);
    if clicked {
        ui.sel = 0;
    }
    g.text(
        "Ячеек на панели",
        cr.x + 10.0 * s,
        cr.y + 9.0 * s,
        15.0 * s,
        c_text(),
        true,
    );
    let bx = cr.x + 260.0 * s;
    let bw = 34.0 * s;
    if button(
        g,
        inp,
        Rect::new(bx, cr.y + 3.0 * s, bw, row_h - 2.0 * s),
        "−",
        false,
    ) {
        count = count.saturating_sub(1);
        ui.sel = 0;
    }
    g.text_center(
        &count.clamp(1, HOTBAR_MAX).to_string(),
        bx + bw + 30.0 * s,
        cr.y + cr.h / 2.0,
        18.0 * s,
        c_accent(),
        true,
        false,
    );
    if button(
        g,
        inp,
        Rect::new(bx + bw + 60.0 * s, cr.y + 3.0 * s, bw, row_h - 2.0 * s),
        "+",
        false,
    ) {
        count += 1;
        ui.sel = 0;
    }
    g.text(
        &format!("1–{HOTBAR_MAX} • ←→"),
        bx + 2.0 * bw + 76.0 * s,
        cr.y + 10.0 * s,
        13.0 * s,
        c_dim(),
        false,
    );
    y += cr.h + 12.0 * s;

    // ---- the cells and their keys ----
    let name_w = 170.0 * s;
    let box_w = 150.0 * s;
    let box_x = [
        inner.x + inner.w - 2.0 * box_w - 12.0 * s,
        inner.x + inner.w - box_w,
    ];
    for (text, x) in [
        ("Ячейка", inner.x + 10.0 * s),
        ("Умение", inner.x + name_w),
        ("Клавиша", box_x[0]),
        ("Вторая клавиша", box_x[1]),
    ] {
        g.text(text, x, y, 13.0 * s, c_dim(), true);
    }
    y += 22.0 * s;
    let list_top = y;
    let list_bottom = r.y + r.h - 108.0 * s;
    let visible = (((list_bottom - list_top) / row_h) as usize).max(1);
    let total = rows - 1;
    let wheel = inp.scroll(Rect::new(
        inner.x,
        list_top,
        inner.w,
        list_bottom - list_top,
    ));
    if wheel < 0.0 {
        ui.scroll = (ui.scroll + 3).min(total.saturating_sub(visible));
    } else if wheel > 0.0 {
        ui.scroll = ui.scroll.saturating_sub(3);
    }
    if ui.sel > 0 {
        let i = ui.sel - 1;
        if i < ui.scroll {
            ui.scroll = i;
        }
        if i >= ui.scroll + visible {
            ui.scroll = i + 1 - visible;
        }
    }
    ui.scroll = ui.scroll.min(total.saturating_sub(visible));
    let db = content::db();
    for i in ui.scroll..(ui.scroll + visible).min(total) {
        let row_i = i + 1;
        let Some(cell) = row_cell(row_i, n) else {
            continue;
        };
        let ry = list_top + (i - ui.scroll) as f32 * row_h;
        let rr = Rect::new(inner.x, ry, inner.w, row_h - 2.0 * s);
        let selected = ui.sel == row_i;
        if selected {
            round_rect(rr.x, rr.y, rr.w, rr.h, 4.0 * s, with_a(c_sel(), 0.6));
        }
        let potion = cell >= POTION_HEALTH;
        g.text_raw(
            &cell_name(cell),
            rr.x + 10.0 * s,
            rr.y + 7.0 * s,
            14.0 * s,
            if potion { col("#e08080") } else { c_text() },
            potion,
        );
        // what the cell holds
        let ability = sheet.hotbar.get(cell).filter(|k| !k.is_empty());
        match ability.and_then(|k| db.ability(k).map(|a| (k, a))) {
            Some((k, a)) => {
                let icon = g.ability_icon(k);
                draw_texture_ex(
                    &icon,
                    rr.x + name_w,
                    rr.y + 2.0 * s,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(rr.h - 4.0 * s, rr.h - 4.0 * s)),
                        ..Default::default()
                    },
                );
                g.text_clip(
                    &a.name,
                    rr.x + name_w + rr.h + 4.0 * s,
                    rr.y + 7.0 * s,
                    box_x[0] - (rr.x + name_w + rr.h + 16.0 * s),
                    14.0 * s,
                    col(&a.color),
                    false,
                );
            }
            None if !potion => {
                g.text(
                    "пусто",
                    rr.x + name_w,
                    rr.y + 7.0 * s,
                    13.0 * s,
                    c_dim(),
                    false,
                );
            }
            None => {}
        }
        for (j, &x) in box_x.iter().enumerate() {
            let br = Rect::new(x, rr.y + 2.0 * s, box_w, rr.h - 4.0 * s);
            let active = selected && ui.col == j;
            let waiting = active && ui.capture;
            let hover = inp.hover(br);
            round_rect(
                br.x,
                br.y,
                br.w,
                br.h,
                5.0 * s,
                if waiting {
                    Color::from_rgba(96, 78, 36, 235)
                } else if hover {
                    c_hover()
                } else {
                    c_panel2()
                },
            );
            round_rect_lines(
                br.x,
                br.y,
                br.w,
                br.h,
                5.0 * s,
                1.0 * s,
                if active { c_accent() } else { c_border() },
            );
            let (text, c) = if waiting {
                let blink = (p.t * 2.5) as i32 % 2 == 0;
                (
                    tr("нажмите клавишу…"),
                    if blink { c_accent() } else { c_dim() },
                )
            } else {
                match keys.cells[cell][j] {
                    Some(b) => (label(b), c_text()),
                    None => ("—".to_string(), c_dim()),
                }
            };
            g.text_center(
                &text,
                br.x + br.w / 2.0,
                br.y + br.h / 2.0,
                14.0 * s,
                c,
                !waiting && text != "—",
                false,
            );
            if inp.click(br) {
                ui.sel = row_i;
                ui.col = j;
                ui.capture = true;
                ui.note = (String::new(), true);
            }
        }
        if inp.click(rr) {
            ui.sel = row_i;
        }
    }
    if total > visible {
        // where the list is scrolled to
        let track = list_bottom - list_top;
        let k = visible as f32 / total as f32;
        let at = ui.scroll as f32 / total as f32;
        draw_rectangle(
            inner.x + inner.w + 6.0 * s,
            list_top + track * at,
            3.0 * s,
            track * k,
            with_a(c_accent(), 0.6),
        );
    }
    // a click anywhere else ends the waiting for a key
    if was_capturing && ui.capture && inp.clicked && !inp.used {
        ui.capture = false;
    }

    // ---- the last change, the buttons ----
    let ny = list_bottom + 6.0 * s;
    if !ui.note.0.is_empty() {
        g.text_clip(
            &ui.note.0,
            inner.x,
            ny,
            inner.w,
            14.0 * s,
            if ui.note.1 { c_good() } else { c_bad() },
            false,
        );
    } else if ui.capture {
        g.text_clip(
            "Нажмите клавишу или кнопку мыши (ПКМ, СКМ). Delete — убрать, Esc — отмена.",
            inner.x,
            ny,
            inner.w,
            14.0 * s,
            c_accent(),
            false,
        );
    }
    let by = ny + 26.0 * s;
    if button(
        g,
        inp,
        Rect::new(inner.x, by, 260.0 * s, 34.0 * s),
        "Клавиши по умолчанию",
        false,
    ) {
        keys = Keys::default();
        ui.note = (tr("Все клавиши — как по умолчанию."), true);
        changed = true;
    }
    if button(
        g,
        inp,
        Rect::new(inner.x + inner.w - 180.0 * s, by, 180.0 * s, 34.0 * s),
        "Готово",
        true,
    ) {
        close = true;
    }
    footer(
        g,
        r,
        "↑↓ выбор • ←→ число ячеек / столбец • Enter / щелчок — назначить клавишу • Delete — убрать • Esc — закрыть",
    );

    let count = count.clamp(1, HOTBAR_MAX);
    if count != n {
        ui.pending = Some(count);
        p.cmd("hotbar_size", "", count as i32);
    }
    if changed {
        keys.store(cfg);
        if let Err(e) = cfg.save() {
            ui.note = (
                format!("{} {e}", tr("Не удалось сохранить настройки:")),
                false,
            );
        }
    }
    p.controls = ui;
    if close {
        p.close_controls();
    }
}
