//! The heads-up display over the world: the top bar, the hero frame, the
//! ability bar, the minimap, the target and party frames, the message log
//! and banners.

use macroquad::prelude::*;

use ratas_core::content;
use ratas_core::llm::{local, Provider};
use ratas_core::proto::*;

use super::{Mode, Session};
use crate::gfx::{atlas, col, daylight, round_rect, round_rect_lines, tr, with_a, Gfx};
use crate::ui::*;

/// Draws the HUD; returns true when the mouse is over it.
pub fn draw(
    p: &mut Session,
    g: &mut Gfx,
    inp: &mut UiInput,
    cfg: &ratas_core::config::Config,
) -> bool {
    let s = g.s;
    let (w, h) = (screen_width(), screen_height());
    let mut over = false;
    // the host's own local model may still be downloading or loading
    let local_ai = (p.srv.is_some() && cfg.provider() == Provider::Local)
        .then(local::current)
        .flatten()
        .map(|rt| rt.state())
        .filter(|st| *st != local::State::Ready);
    top_bar(p, g, w, s, local_ai);
    over |= hero_frame(p, g, inp, s);
    let mm = 92.0 * s;
    let (mx, my) = (w - mm - 14.0 * s, 34.0 * s + mm);
    if p.mode != Mode::Map {
        let level = p.level.as_ref().unwrap();
        let snap = p.snap.as_ref().unwrap();
        let places = p
            .sheet
            .as_ref()
            .map(|sh| sh.places.as_slice())
            .unwrap_or(&[]);
        let facing = me_facing(p);
        atlas::draw_minimap(
            g,
            &mut p.wr.atlas,
            level,
            &p.explored,
            snap,
            places,
            (mx, my, mm),
            facing,
            p.t,
        );
        let r = Rect::new(mx - mm, my - mm, mm * 2.0, mm * 2.0);
        if inp.hover(r) {
            over = true;
            if inp.click(r) {
                p.toggle(Mode::Map);
            }
        }
    }
    let mut y = my + mm + 16.0 * s;
    y = target_frame(p, g, w - 240.0 * s - 14.0 * s, y, 240.0 * s, s);
    party_frames(p, g, w - 240.0 * s - 14.0 * s, y, 240.0 * s, s);
    over |= hotbar(p, g, inp, w, h, s);
    log_box(p, g, s);
    banners(p, g, w, h, s);
    let _ = cfg;
    over
}

fn me_facing(p: &Session) -> f32 {
    let you = p.welcome.as_ref().map_or(0, |w| w.you_id);
    p.snap
        .as_ref()
        .and_then(|s| s.entities.iter().find(|e| e.id == you))
        .map_or(0.0, |e| e.facing)
}

fn top_bar(p: &Session, g: &Gfx, w: f32, s: f32, local_ai: Option<local::State>) {
    let snap = p.snap.as_ref().unwrap();
    let level = p.level.as_ref().unwrap();
    let bh = 24.0 * s;
    draw_rectangle(0.0, 0.0, w, bh, Color::new(0.06, 0.06, 0.1, 0.82));
    draw_line(0.0, bh, w, bh, 1.0, with_a(c_border(), 0.6));
    let fs = 14.0 * s;
    let ty = (bh - fs) / 2.0 - 1.0;
    let mut x = g.text(&level.name, 10.0 * s, ty, fs, c_accent(), true);
    if !snap.you.region.is_empty() {
        x = g.text_raw(" • ", x, ty, fs, c_dim(), false);
        x = g.text(&snap.you.region, x, ty, fs, col("#d8c890"), false);
    }
    if level.lit {
        let t = snap.time_of_day;
        let hh = (t * 24.0) as i32;
        let mm = ((t * 24.0 - hh as f64) * 60.0) as i32;
        let icon = if daylight(t) < 0.35 { "☾" } else { "☀" };
        g.text_raw(
            &format!("{icon} {hh:02}:{mm:02}"),
            x + 16.0 * s,
            ty,
            fs,
            c_text(),
            false,
        );
    }
    let mut right = format!("{} {}", tr("Игроков:"), snap.online.len());
    if snap.you.pvp && snap.online.len() > 1 {
        right = if snap.you.safe {
            format!("{} • {right}", tr("мирная зона"))
        } else {
            format!("{} • {right}", tr("бой между игроками"))
        };
    }
    if p.welcome.as_ref().is_some_and(|w| w.ai) {
        match &local_ai {
            Some(st) => right += &format!(" • {} {}", tr("ИИ:"), tr(&st.describe())),
            None => right += &format!(" • {}", tr("ИИ вкл")),
        }
    }
    if p.admin() {
        right = format!("{} • {right}", tr("АДМИН (F9)"));
    }
    match &p.srv {
        Some(srv) => {
            let a = srv.listening();
            if !a.is_empty() {
                right += &format!(" • {} {a}", tr("сервер"));
            }
        }
        None => right += &format!(" • {}", tr("онлайн")),
    }
    right += &format!(" • F1 {}", tr("помощь"));
    let rw = g.measure(&right, fs, false);
    g.text_raw(&right, w - rw - 10.0 * s, ty, fs, c_dim(), false);
}

/// The hero portrait, bars, effects and unspent points.
fn hero_frame(p: &mut Session, g: &mut Gfx, inp: &mut UiInput, s: f32) -> bool {
    let snap = p.snap.as_ref().unwrap();
    let me = &snap.you;
    let x = 12.0 * s;
    let y = 34.0 * s;
    let (fw, fh) = (300.0 * s, 92.0 * s);
    round_rect(x, y, fw, fh, 8.0 * s, Color::new(0.05, 0.05, 0.09, 0.78));
    round_rect_lines(x, y, fw, fh, 8.0 * s, 1.0 * s, with_a(c_border(), 0.8));
    // portrait
    let pr = Rect::new(x + 8.0 * s, y + 8.0 * s, 76.0 * s, 76.0 * s);
    round_rect(
        pr.x,
        pr.y,
        pr.w,
        pr.h,
        6.0 * s,
        Color::from_rgba(30, 28, 44, 255),
    );
    let you = p.welcome.as_ref().map_or(0, |w| w.you_id);
    if let Some(e) = snap.entities.iter().find(|e| e.id == you) {
        let name = crate::art::specs::resolve(&e.model, &e.def, e.glyph, e.kind);
        let tex = g.model(&name, &e.color, 0, &e.gear).frames[0].clone();
        let k = (pr.h * 0.9 / tex.height())
            .min(pr.w * 0.9 / tex.width())
            .floor()
            .max(1.0);
        let (tw, th) = (tex.width() * k, tex.height() * k);
        draw_texture_ex(
            &tex,
            pr.x + (pr.w - tw) / 2.0,
            pr.y + pr.h - th - 3.0 * s,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(tw, th)),
                ..Default::default()
            },
        );
    }
    let lv = format!("{}", me.level);
    let lr = Rect::new(
        pr.x + pr.w - 22.0 * s,
        pr.y + pr.h - 20.0 * s,
        24.0 * s,
        22.0 * s,
    );
    round_rect(
        lr.x,
        lr.y,
        lr.w,
        lr.h,
        5.0 * s,
        Color::from_rgba(90, 70, 30, 255),
    );
    g.text_center(
        &lv,
        lr.x + lr.w / 2.0,
        lr.y + lr.h / 2.0,
        13.0 * s,
        WHITE,
        true,
        false,
    );
    let tx = pr.x + pr.w + 10.0 * s;
    let bw = x + fw - tx - 10.0 * s;
    let class_name = p
        .sheet
        .as_ref()
        .and_then(|sh| content::db().class(&sh.class))
        .map(|c| tr(&c.name))
        .unwrap_or_default();
    let nx = g.text_clip(
        &p.name,
        tx,
        y + 8.0 * s,
        bw * 0.55,
        15.0 * s,
        c_accent(),
        true,
    );
    g.text_clip(
        &class_name,
        nx + 8.0 * s,
        y + 10.0 * s,
        (x + fw - nx - 18.0 * s).max(0.0),
        12.0 * s,
        c_dim(),
        false,
    );
    bar(
        g,
        Rect::new(tx, y + 30.0 * s, bw, 16.0 * s),
        (me.hp / me.max_hp.max(1.0)) as f32,
        c_hp(),
        &format!("{:.0}/{:.0}", me.hp.ceil(), me.max_hp),
    );
    bar(
        g,
        Rect::new(tx, y + 50.0 * s, bw, 14.0 * s),
        (me.mp / me.max_mp.max(1.0)) as f32,
        c_mp(),
        &format!("{:.0}/{:.0}", me.mp, me.max_mp),
    );
    bar(
        g,
        Rect::new(tx, y + 68.0 * s, bw, 8.0 * s),
        me.xp as f32 / me.xp_next.max(1) as f32,
        c_xp(),
        "",
    );
    g.text_raw(
        &format!("◆ {}", me.gold),
        tx,
        y + 77.0 * s,
        11.0 * s,
        c_gold(),
        true,
    );
    let mut over = inp.hover(Rect::new(x, y, fw, fh));
    // effects
    let mut ey = y + fh + 6.0 * s;
    let mut ex = x;
    for b in &me.buffs {
        let label = format!("{} {}", tr(&b.name), (b.left + 999) / 1000);
        let bw = g.measure(&label, 12.0 * s, false) + 12.0 * s;
        if ex + bw > x + fw + 60.0 * s {
            ex = x;
            ey += 22.0 * s;
        }
        round_rect(
            ex,
            ey,
            bw,
            19.0 * s,
            9.0 * s,
            Color::new(0.05, 0.05, 0.09, 0.78),
        );
        round_rect_lines(
            ex,
            ey,
            bw,
            19.0 * s,
            9.0 * s,
            1.0,
            with_a(col(&b.color), 0.8),
        );
        g.text_raw(
            &label,
            ex + 6.0 * s,
            ey + 3.0 * s,
            12.0 * s,
            col(&b.color),
            false,
        );
        ex += bw + 4.0 * s;
    }
    if !me.buffs.is_empty() {
        ey += 26.0 * s;
    }
    // unspent points and invitations: clickable reminders
    let mut notes: Vec<(String, super::Mode)> = vec![];
    if me.attr_points > 0 {
        notes.push((
            format!("+{} {} [C]", me.attr_points, tr("очк. характеристик")),
            Mode::Char,
        ));
    }
    if me.skill_points > 0 {
        notes.push((
            format!("+{} {} [K]", me.skill_points, tr("очк. навыков")),
            Mode::Skills,
        ));
    }
    if let Some(inv) = me.invites.first() {
        notes.push((format!("{} {inv} [G]", tr("Приглашение:")), Mode::Party));
    }
    let mut open = None;
    for (text, mode) in notes {
        let tw = g.measure(&text, 13.0 * s, true) + 16.0 * s;
        let r = Rect::new(x, ey, tw, 22.0 * s);
        let hov = inp.hover(r);
        round_rect(
            r.x,
            r.y,
            r.w,
            r.h,
            5.0 * s,
            if hov {
                Color::from_rgba(50, 80, 40, 230)
            } else {
                Color::from_rgba(30, 50, 26, 210)
            },
        );
        g.text_raw(
            &text,
            r.x + 8.0 * s,
            r.y + 4.0 * s,
            13.0 * s,
            c_good(),
            true,
        );
        if inp.click(r) {
            open = Some(mode);
        }
        over |= hov;
        ey += 26.0 * s;
    }
    if let Some(m) = open {
        p.toggle(m);
    }
    over
}

/// The enemy we fight with its resistances.
fn target_frame(p: &Session, g: &Gfx, x: f32, y: f32, w: f32, s: f32) -> f32 {
    let Some(t) = p.snap.as_ref().and_then(|s| s.you.target.as_ref()) else {
        return y;
    };
    let db = content::db();
    let mut lines: Vec<(String, Color)> = vec![];
    for good in [true, false] {
        let items: Vec<String> =
            db.b.damage_types
                .iter()
                .filter_map(|dt| {
                    t.res
                        .get(&dt.key)
                        .filter(|v| (**v > 0) == good)
                        .map(|v| format!("{} {}%", tr(&dt.short), v))
                })
                .collect();
        if !items.is_empty() {
            let label = if good {
                tr("Стойк.")
            } else {
                tr("Уязв.")
            };
            lines.push((
                format!("{label} {}", items.join(", ")),
                if good { c_good() } else { c_bad() },
            ));
        }
    }
    for e in &t.effects {
        lines.push((
            format!("• {} {}", tr(&e.name), (e.left + 999) / 1000),
            col(&e.color),
        ));
    }
    let wrapped: Vec<(String, Color)> = lines
        .iter()
        .flat_map(|(l, c)| {
            g.wrap(l, w - 20.0 * s, 12.0 * s, false)
                .into_iter()
                .map(move |x| (x, *c))
        })
        .collect();
    let h = 52.0 * s + wrapped.len() as f32 * 16.0 * s;
    round_rect(x, y, w, h, 8.0 * s, Color::new(0.05, 0.05, 0.09, 0.8));
    round_rect_lines(
        x,
        y,
        w,
        h,
        8.0 * s,
        1.0 * s,
        if t.boss {
            col("#c02080")
        } else {
            with_a(c_border(), 0.8)
        },
    );
    g.text_clip(
        &t.name,
        x + 10.0 * s,
        y + 6.0 * s,
        w - 70.0 * s,
        14.0 * s,
        col(&t.color),
        true,
    );
    if t.level > 0 {
        let lv = format!("{} {}", tr("ур."), t.level);
        let lw = g.measure(&lv, 12.0 * s, false);
        g.text_raw(
            &lv,
            x + w - lw - 10.0 * s,
            y + 8.0 * s,
            12.0 * s,
            c_dim(),
            false,
        );
    }
    bar(
        g,
        Rect::new(x + 10.0 * s, y + 26.0 * s, w - 20.0 * s, 12.0 * s),
        t.hp as f32 / 100.0,
        c_hp(),
        "",
    );
    let mut ly = y + 44.0 * s;
    for (l, c) in wrapped {
        g.text_raw(&l, x + 10.0 * s, ly, 12.0 * s, c, false);
        ly += 16.0 * s;
    }
    y + h + 8.0 * s
}

fn party_frames(p: &Session, g: &Gfx, x: f32, y: f32, w: f32, s: f32) {
    let me = &p.snap.as_ref().unwrap().you;
    let mut y = y;
    for m in &me.party {
        if m.name == p.name {
            continue;
        }
        let h = 44.0 * s;
        if y + h > screen_height() - 140.0 * s {
            break;
        }
        round_rect(x, y, w, h, 6.0 * s, Color::new(0.05, 0.05, 0.09, 0.75));
        let mut name = m.name.clone();
        if m.leader {
            name = format!("♛ {name}");
        }
        let c = if m.dead { c_bad() } else { c_text() };
        g.text_clip(
            &name,
            x + 8.0 * s,
            y + 4.0 * s,
            w - 60.0 * s,
            13.0 * s,
            c,
            true,
        );
        let lv = format!("{} {}", tr("ур."), m.level);
        let lw = g.measure(&lv, 11.0 * s, false);
        g.text_raw(
            &lv,
            x + w - lw - 8.0 * s,
            y + 5.0 * s,
            11.0 * s,
            c_dim(),
            false,
        );
        if !m.where_.is_empty() {
            g.text_clip(
                &m.where_,
                x + 8.0 * s,
                y + 24.0 * s,
                w - 16.0 * s,
                11.0 * s,
                c_dim(),
                false,
            );
        } else {
            bar(
                g,
                Rect::new(x + 8.0 * s, y + 23.0 * s, w * 0.62, 9.0 * s),
                (m.hp / m.max_hp.max(1.0)) as f32,
                c_hp(),
                "",
            );
            bar(
                g,
                Rect::new(
                    x + 12.0 * s + w * 0.62,
                    y + 23.0 * s,
                    w * 0.38 - 20.0 * s,
                    9.0 * s,
                ),
                (m.mp / m.max_mp.max(1.0)) as f32,
                c_mp(),
                "",
            );
        }
        if m.dead {
            g.text_raw(
                &tr("— пал"),
                x + 8.0 * s,
                y + 33.0 * s,
                10.0 * s,
                c_bad(),
                false,
            );
        }
        y += h + 6.0 * s;
    }
}

/// Where the quick-access bar lies: its ability cells fill blocks of one
/// row each, as wide as the screen allows; a block that does not fit makes
/// a new one above. The potions and the controls button end the bottom
/// block.
pub struct BarLayout {
    cell: f32,
    gap: f32,
    pad: f32,
    sep: f32,
    gear: f32,
    n: usize,
    per_row: usize,
    /// from one block to the next
    step: f32,
    grid_w: f32,
    /// the left edge of the cells and the top of the bottom block's cells
    x0: f32,
    y0: f32,
    /// around all the blocks
    pub frame: Rect,
}

impl BarLayout {
    pub fn new(n: usize, w: f32, h: f32, s: f32) -> BarLayout {
        let (cell, gap, pad, sep, gear) = (60.0 * s, 6.0 * s, 10.0 * s, 14.0 * s, 28.0 * s);
        // the potions and the button after the cells
        let extra = sep + 2.0 * cell + 2.0 * gap + gear;
        let margin = 16.0 * s + pad;
        let fit = ((w - 2.0 * margin - extra + gap) / (cell + gap))
            .floor()
            .max(1.0) as usize;
        let n = n.max(1);
        let per_row = fit.min(n);
        let rows = n.div_ceil(per_row);
        let grid_w = per_row as f32 * (cell + gap) - gap;
        let total = grid_w + extra;
        let x0 = ((w - total) / 2.0).max(margin);
        let y0 = h - cell - 18.0 * s;
        let step = cell + 2.0 * pad + 6.0 * s;
        let top = y0 - (rows - 1) as f32 * step;
        BarLayout {
            cell,
            gap,
            pad,
            sep,
            gear,
            n,
            per_row,
            step,
            grid_w,
            x0,
            y0,
            frame: Rect::new(
                x0 - pad,
                top - pad,
                total + 2.0 * pad,
                y0 + cell - top + 2.0 * pad,
            ),
        }
    }

    /// The bar of the hero of a session.
    pub fn of(p: &Session, w: f32, h: f32, s: f32) -> BarLayout {
        let n = p.sheet.as_ref().map_or(1, |sh| sh.hotbar.len());
        BarLayout::new(n, w, h, s)
    }

    /// Ability cell i: block by block from the bottom.
    pub fn cell_rect(&self, i: usize) -> Rect {
        let (row, c) = (i / self.per_row, i % self.per_row);
        Rect::new(
            self.x0 + c as f32 * (self.cell + self.gap),
            self.y0 - row as f32 * self.step,
            self.cell,
            self.cell,
        )
    }

    /// The frames of the blocks; the bottom one holds the potions too.
    fn blocks(&self) -> Vec<Rect> {
        let rows = self.n.div_ceil(self.per_row);
        (0..rows)
            .map(|row| {
                let cells = (self.n - row * self.per_row).min(self.per_row);
                let w = if row == 0 {
                    self.frame.w - 2.0 * self.pad
                } else {
                    cells as f32 * (self.cell + self.gap) - self.gap
                };
                Rect::new(
                    self.x0 - self.pad,
                    self.y0 - row as f32 * self.step - self.pad,
                    w + 2.0 * self.pad,
                    self.cell + 2.0 * self.pad,
                )
            })
            .collect()
    }

    /// Potion j (health, mana) at the end of the bottom row.
    fn potion_rect(&self, j: usize) -> Rect {
        let x = self.x0 + self.grid_w + self.sep + j as f32 * (self.cell + self.gap);
        Rect::new(x, self.y0, self.cell, self.cell)
    }

    fn gear_rect(&self) -> Rect {
        let p = self.potion_rect(1);
        Rect::new(
            p.x + p.w + self.gap,
            self.y0 + (self.cell - self.gear) / 2.0,
            self.gear,
            self.gear,
        )
    }
}

/// Where the message log and the chat line go: left of the bar while there
/// is room, above it otherwise. Returns (x, width, bottom).
pub fn log_area(p: &Session, s: f32) -> (f32, f32, f32) {
    let (w, h) = (screen_width(), screen_height());
    let bar = BarLayout::of(p, w, h, s);
    let x = 12.0 * s;
    let room = bar.frame.x - x - 10.0 * s;
    if room >= 300.0 * s {
        (x, room.min(520.0 * s), h - 14.0 * s)
    } else {
        (x, (520.0 * s).min(w - 2.0 * x), bar.frame.y - 8.0 * s)
    }
}

/// A key or a count in the corner of a cell, cut to fit.
fn corner_text(g: &Gfx, text: &str, r: Rect, s: f32, top: bool, c: Color) {
    if text.is_empty() {
        return;
    }
    let mut fs = 13.0 * s;
    while fs > 8.0 * s && g.measure(text, fs, true) > r.w - 8.0 * s {
        fs -= 1.0 * s;
    }
    let tw = g.measure(text, fs, true);
    let (x, y) = if top {
        (r.x + 4.0 * s, r.y + 2.0 * s)
    } else {
        (r.x + r.w - tw - 4.0 * s, r.y + r.h - fs - 4.0 * s)
    };
    // a dark backing keeps the key readable over bright icons
    round_rect(
        x - 2.0 * s,
        y,
        tw + 4.0 * s,
        fs + 3.0 * s,
        3.0 * s,
        Color::new(0.0, 0.0, 0.0, 0.55),
    );
    g.text_raw(text, x, y, fs, c, true);
}

/// The quick-access bar: the ability cells (as many as the hero chose, in
/// rows), the two potions and the controls button.
fn hotbar(p: &mut Session, g: &mut Gfx, inp: &mut UiInput, w: f32, h: f32, s: f32) -> bool {
    let Some(sheet) = p.sheet.clone() else {
        return false;
    };
    let me = p.snap.as_ref().unwrap().you.clone();
    let db = content::db();
    let l = BarLayout::new(sheet.hotbar.len(), w, h, s);
    let mut over = false;
    for f in l.blocks() {
        round_rect(
            f.x,
            f.y,
            f.w,
            f.h,
            10.0 * s,
            Color::new(0.04, 0.04, 0.07, 0.78),
        );
        round_rect_lines(
            f.x,
            f.y,
            f.w,
            f.h,
            10.0 * s,
            1.0 * s,
            with_a(c_border(), 0.8),
        );
        over |= inp.hover(f);
    }
    let mut tip: Option<(String, Rect)> = None;
    for (i, key) in sheet.hotbar.iter().enumerate() {
        let r = l.cell_rect(i);
        round_rect(
            r.x,
            r.y,
            r.w,
            r.h,
            6.0 * s,
            Color::from_rgba(24, 22, 34, 255),
        );
        let keys_line = {
            let k = p.keys.labels(i);
            if k.is_empty() {
                tr("Клавиша не назначена (Esc → Управление).")
            } else {
                format!("{} {k}", tr("Клавиши:"))
            }
        };
        match db.ability(key) {
            Some(a) => {
                let icon = g.ability_icon(key);
                let no_mana = a.mana > me.mp;
                let c = if no_mana {
                    Color::new(0.45, 0.45, 0.6, 1.0)
                } else {
                    WHITE
                };
                draw_texture_ex(
                    &icon,
                    r.x + 3.0 * s,
                    r.y + 3.0 * s,
                    c,
                    DrawTextureParams {
                        dest_size: Some(vec2(r.w - 6.0 * s, r.h - 6.0 * s)),
                        ..Default::default()
                    },
                );
                let cd = me.cooldown.get(i).copied().unwrap_or(0.0);
                if cd > 0.0 {
                    let ch = (r.h - 4.0 * s) * cd.clamp(0.0, 1.0);
                    draw_rectangle(
                        r.x + 2.0 * s,
                        r.y + 2.0 * s,
                        r.w - 4.0 * s,
                        ch,
                        Color::new(0.0, 0.0, 0.0, 0.62),
                    );
                    let left = cd as f64 * a.cooldown_ms as f64 / 1000.0;
                    let text = if left >= 1.0 {
                        format!("{}", left.ceil() as i64)
                    } else {
                        format!("{left:.1}")
                    };
                    g.text_center(
                        &text,
                        r.x + r.w / 2.0,
                        r.y + r.h / 2.0,
                        20.0 * s,
                        WHITE,
                        true,
                        true,
                    );
                }
                if inp.hover(r) {
                    tip = Some((format!("{}\n{keys_line}", ability_tip(p, key)), r));
                }
                if inp.click(r) {
                    p.conn.send(ClientMsg::Input(Input {
                        ability: (i + 1).min(i8::MAX as usize) as i8,
                        aim: p.sent.aim,
                        ..Default::default()
                    }));
                }
            }
            None => {
                if inp.hover(r) {
                    tip = Some((
                        format!(
                            "{}\n{}\n{keys_line}",
                            crate::play::keys::cell_name(i),
                            tr("Пусто: в окне навыков (K) выберите умение и нажмите клавишу этой ячейки.")
                        ),
                        r,
                    ));
                }
            }
        }
        round_rect_lines(r.x, r.y, r.w, r.h, 6.0 * s, 1.0 * s, c_border());
        corner_text(g, &p.keys.label(i), r, s, true, c_accent());
    }
    // potions
    let (mut hp, mut mp) = (0, 0);
    for it in &sheet.inventory {
        if let Some(d) = db.item(&it.key).filter(|d| d.kind == "consumable") {
            if d.heal > 0.0 {
                hp += it.qty;
            }
            if d.mana > 0.0 {
                mp += it.qty;
            }
        }
    }
    for (j, (cell, count, color, kind)) in [
        (super::keys::POTION_HEALTH, hp, "#e04040", "health"),
        (super::keys::POTION_MANA, mp, "#4060e0", "mana"),
    ]
    .into_iter()
    .enumerate()
    {
        let r = l.potion_rect(j);
        round_rect(
            r.x,
            r.y,
            r.w,
            r.h,
            6.0 * s,
            Color::from_rgba(24, 22, 34, 255),
        );
        if let Some(icon) = g.icon("", '!', color) {
            let k = (r.w * 0.7 / icon.width()).floor().max(1.0);
            let (iw, ih) = (icon.width() * k, icon.height() * k);
            let c = if count > 0 {
                WHITE
            } else {
                Color::new(0.4, 0.4, 0.4, 0.8)
            };
            draw_texture_ex(
                &icon,
                r.x + (r.w - iw) / 2.0,
                r.y + (r.h - ih) / 2.0,
                c,
                DrawTextureParams {
                    dest_size: Some(vec2(iw, ih)),
                    ..Default::default()
                },
            );
        }
        round_rect_lines(r.x, r.y, r.w, r.h, 6.0 * s, 1.0 * s, c_border());
        corner_text(g, &p.keys.label(cell), r, s, true, c_accent());
        corner_text(g, &format!("×{count}"), r, s, false, WHITE);
        if inp.hover(r) {
            let keys = p.keys.labels(cell);
            tip = Some((
                format!(
                    "{}\n{}",
                    crate::play::keys::cell_name(cell),
                    if keys.is_empty() {
                        tr("Клавиша не назначена (Esc → Управление).")
                    } else {
                        format!("{} {keys}", tr("Клавиши:"))
                    }
                ),
                r,
            ));
        }
        if inp.click(r) {
            p.cmd("potion", kind, 0);
        }
    }
    // the controls: how many cells and which keys
    let gr = l.gear_rect();
    let hov = inp.hover(gr);
    round_rect(
        gr.x,
        gr.y,
        gr.w,
        gr.h,
        5.0 * s,
        if hov { c_hover() } else { c_panel2() },
    );
    round_rect_lines(gr.x, gr.y, gr.w, gr.h, 5.0 * s, 1.0 * s, c_border());
    g.text_center(
        "⚙",
        gr.x + gr.w / 2.0,
        gr.y + gr.h / 2.0,
        gr.h * 0.7,
        if hov { c_accent() } else { c_dim() },
        false,
        false,
    );
    if hov {
        tip = Some((tr("Управление\nЧисло ячеек панели и их клавиши."), gr));
    }
    if inp.click(gr) {
        p.open_controls(Mode::Game);
    }
    if let Some((text, r)) = tip {
        tooltip(g, &text, r.x, r.y - 6.0 * s, s);
        over = true;
    }
    over
}

pub fn ability_tip(p: &Session, key: &str) -> String {
    let db = content::db();
    let Some(a) = db.ability(key) else {
        return key.to_string();
    };
    let mut name = tr(&a.name);
    if key == "copied" {
        if let Some(orig) = p.snap.as_ref().and_then(|s| db.ability(&s.you.copied)) {
            name = format!("{} {}", tr("Копия:"), tr(&orig.name));
        }
    }
    let mut out = vec![name];
    if !a.parts.is_empty() {
        out.extend(super::fusion::tip_lines(a));
        return out.join("\n");
    }
    out.push(tr(&format!(
        "Мана {:.0} • перезарядка {:.1}с",
        a.mana,
        a.cooldown_ms as f64 / 1000.0
    )));
    if a.damage[1] > 0.0 {
        if a.kind == "heal" {
            out.push(tr(&format!(
                "Лечение {:.0}-{:.0}",
                a.damage[0], a.damage[1]
            )));
        } else {
            out.push(tr(&format!("Урон {:.0}-{:.0}", a.damage[0], a.damage[1])));
        }
    }
    out.push(tr(&a.desc));
    out.join("\n")
}

/// A tooltip box whose bottom-left corner is at (x, y).
pub fn tooltip(g: &Gfx, text: &str, x: f32, y: f32, s: f32) {
    let w = 300.0 * s;
    let lines: Vec<String> = text
        .lines()
        .flat_map(|l| g.wrap(l, w - 20.0 * s, 13.0 * s, false))
        .collect();
    let h = lines.len() as f32 * 17.0 * s + 14.0 * s;
    let x = x.clamp(4.0, screen_width() - w - 4.0);
    let y = (y - h).max(4.0);
    round_rect(x, y, w, h, 6.0 * s, Color::from_rgba(14, 13, 22, 245));
    round_rect_lines(x, y, w, h, 6.0 * s, 1.0 * s, c_border());
    for (i, l) in lines.iter().enumerate() {
        let c = if i == 0 { c_accent() } else { c_text() };
        g.text_raw(
            l,
            x + 10.0 * s,
            y + 7.0 * s + i as f32 * 17.0 * s,
            13.0 * s,
            c,
            i == 0,
        );
    }
}

/// The message log at the bottom left: recent lines, fading with age.
fn log_box(p: &Session, g: &Gfx, s: f32) {
    // left of the quick-access bar, or above it when it is wide
    let (x, w, bottom) = log_area(p, s);
    let fs = 13.0 * s;
    let lh = 17.0 * s;
    let bottom = bottom - if p.mode == Mode::Chat { 34.0 * s } else { 0.0 };
    let max_lines = 9;
    let chatting = p.mode == Mode::Chat;
    let mut lines: Vec<(String, Color, f32)> = vec![];
    for l in p.logs.iter().rev() {
        let age = p.t - l.at;
        if !chatting && age > 14.0 {
            break;
        }
        let a = if chatting {
            1.0
        } else {
            (1.0 - (age - 10.0) / 4.0).clamp(0.0, 1.0)
        };
        let c = if l.color.is_empty() {
            c_text()
        } else {
            col(&l.color)
        };
        let wrapped = g.wrap(&l.text, w - 16.0 * s, fs, false);
        for wl in wrapped.into_iter().rev() {
            lines.push((wl, c, a));
        }
        if lines.len() >= max_lines {
            break;
        }
    }
    lines.truncate(max_lines);
    if lines.is_empty() {
        return;
    }
    let bh = lines.len() as f32 * lh + 10.0 * s;
    let amax = lines.iter().map(|l| l.2).fold(0.0, f32::max);
    round_rect(
        x,
        bottom - bh,
        w,
        bh,
        6.0 * s,
        Color::new(0.03, 0.03, 0.06, 0.55 * amax),
    );
    for (i, (l, c, a)) in lines.iter().enumerate() {
        let y = bottom - 5.0 * s - (i + 1) as f32 * lh;
        g.text_raw(l, x + 8.0 * s, y, fs, with_a(*c, *a), false);
    }
}

fn banners(p: &Session, g: &Gfx, w: f32, h: f32, s: f32) {
    let me = &p.snap.as_ref().unwrap().you;
    if p.paused {
        let t = tr("ПАУЗА");
        let tw = g.measure(&t, 18.0 * s, true) + 24.0 * s;
        round_rect(
            w / 2.0 - tw / 2.0,
            40.0 * s,
            tw,
            30.0 * s,
            6.0 * s,
            c_accent(),
        );
        g.text_center(&t, w / 2.0, 55.0 * s, 18.0 * s, BLACK, true, false);
    }
    if !p.notice.0.is_empty() && p.t - p.notice.1 < 1.5 {
        let t = tr(&p.notice.0);
        let tw = g.measure(&t, 16.0 * s, true) + 24.0 * s;
        round_rect(
            w / 2.0 - tw / 2.0,
            76.0 * s,
            tw,
            28.0 * s,
            6.0 * s,
            c_good(),
        );
        g.text_center(&t, w / 2.0, 90.0 * s, 16.0 * s, BLACK, true, false);
    }
    if !p.region.0.is_empty() && p.mode == Mode::Game && p.t - p.region.1 < 3.5 {
        let a = ((3.5 - (p.t - p.region.1)) / 0.8).clamp(0.0, 1.0)
            * ((p.t - p.region.1) / 0.4).clamp(0.0, 1.0);
        let t = tr(&p.region.0);
        let fs = 30.0 * s;
        let tw = g.measure(&t, fs, true);
        let y = h * 0.22;
        draw_rectangle(
            w / 2.0 - tw / 2.0 - 60.0 * s,
            y - 10.0 * s,
            tw + 120.0 * s,
            fs + 22.0 * s,
            Color::new(0.05, 0.05, 0.08, 0.6 * a),
        );
        draw_line(
            w / 2.0 - tw / 2.0 - 40.0 * s,
            y - 4.0 * s,
            w / 2.0 + tw / 2.0 + 40.0 * s,
            y - 4.0 * s,
            1.0 * s,
            Color::new(0.42, 0.35, 0.23, a),
        );
        draw_line(
            w / 2.0 - tw / 2.0 - 40.0 * s,
            y + fs + 6.0 * s,
            w / 2.0 + tw / 2.0 + 40.0 * s,
            y + fs + 6.0 * s,
            1.0 * s,
            Color::new(0.42, 0.35, 0.23, a),
        );
        g.text_center(
            &t,
            w / 2.0,
            y + fs / 2.0,
            fs,
            Color::new(0.94, 0.86, 0.63, a),
            true,
            true,
        );
    }
    if me.dead && p.mode != Mode::Map {
        let msg = tr(&format!(
            "ВЫ ПОГИБЛИ • возрождение в деревне через {} с",
            (me.respawn_in + 999) / 1000
        ));
        let hint = if me.can_rise {
            tr("Enter — возродиться сейчас • союзник может воскресить вас")
        } else {
            tr("Союзник может воскресить вас, пока вы здесь")
        };
        let fs = 22.0 * s;
        let tw = g
            .measure(&msg, fs, true)
            .max(g.measure(&hint, 15.0 * s, false))
            + 40.0 * s;
        let y = h * 0.4;
        round_rect(
            w / 2.0 - tw / 2.0,
            y,
            tw,
            70.0 * s,
            8.0 * s,
            Color::from_rgba(90, 14, 14, 220),
        );
        g.text_center(&msg, w / 2.0, y + 22.0 * s, fs, WHITE, true, true);
        g.text_center(
            &hint,
            w / 2.0,
            y + 50.0 * s,
            15.0 * s,
            col("#ffd0d0"),
            false,
            false,
        );
    }
}
