//! The fusion window (U): two abilities of different classes melt into one
//! that does what both do. The result, its strengths and its flaws are shown
//! before it is made (the client makes the same fusion as the server); a
//! fusion splits back into its abilities here too.

use macroquad::prelude::*;

use ratas_core::content::{self, fusion, AbilityDef};
use ratas_core::game::{
    buff_desc, can_fuse, can_fuse_one, damage_type_name, gear_need_name, max_fusions, PlayerState,
    FUSION_LEVEL,
};
use ratas_core::proto::{Command, PlayerSheet};

use super::Session;
use crate::gfx::{col, mul_c, round_rect, round_rect_lines, tr, with_a, Gfx};
use crate::ui::*;

enum Row {
    Header(String, Color),
    Ability(String),
    Fusion(String),
}

fn selectable(r: &Row) -> bool {
    !matches!(r, Row::Header(..))
}

/// What the rules need to know of the hero.
fn hero(sh: &PlayerSheet) -> PlayerState {
    PlayerState {
        level: sh.level,
        abilities: sh.abilities.clone(),
        ..Default::default()
    }
}

/// The hero's abilities by class (the classes the hero develops first),
/// those of no class, then the fusions.
fn rows(sh: &PlayerSheet) -> Vec<Row> {
    let db = content::db();
    let own: Vec<&String> = sh
        .abilities
        .iter()
        .filter(|k| !fusion::is_fusion(k))
        .collect();
    let mut classes: Vec<String> = sh.classes.iter().map(|c| c.key.clone()).collect();
    for k in &own {
        if let Some(c) = db.ability_class(k) {
            if !classes.iter().any(|x| x == c) {
                classes.push(c.to_string());
            }
        }
    }
    let mut out = Vec::new();
    for c in &classes {
        let mine: Vec<&String> = own
            .iter()
            .copied()
            .filter(|k| db.ability_class(k) == Some(c.as_str()))
            .collect();
        if mine.is_empty() {
            continue;
        }
        let (name, color) = db
            .class(c)
            .map_or((c.clone(), c_accent()), |cl| (tr(&cl.name), col(&cl.color)));
        out.push(Row::Header(name, color));
        out.extend(mine.into_iter().map(|k| Row::Ability(k.clone())));
    }
    let classless: Vec<&String> = own
        .iter()
        .copied()
        .filter(|k| db.ability_class(k).is_none())
        .collect();
    if !classless.is_empty() {
        out.push(Row::Header(tr("Без класса — не сливаются"), c_dim()));
        out.extend(classless.into_iter().map(|k| Row::Ability(k.clone())));
    }
    let fused: Vec<&String> = sh
        .abilities
        .iter()
        .filter(|k| fusion::is_fusion(k))
        .collect();
    out.push(Row::Header(
        tr(&format!(
            "Слияния: {} из {}",
            fused.len(),
            max_fusions(sh.level)
        )),
        col("#ffd24a"),
    ));
    out.extend(fused.into_iter().map(|k| Row::Fusion(k.clone())));
    out
}

/// The damage types of an ability in words.
fn types_text(a: &AbilityDef) -> String {
    if !a.split.is_empty() {
        return a
            .split
            .iter()
            .map(|t| tr(&damage_type_name(t)))
            .collect::<Vec<_>>()
            .join(" + ");
    }
    match a.dmg_type.as_str() {
        "" if !matches!(a.kind.as_str(), "strike" | "cleave" | "dash") => tr("тайная магия"),
        "" | "weapon" => tr("оружие"),
        t => tr(&damage_type_name(t)),
    }
}

/// One part of a fusion in a line: what it deals and lays on.
fn part_text(a: &AbilityDef) -> String {
    let mut bits = Vec::new();
    if a.damage[1] > 0.0 {
        if a.kind == "heal" {
            bits.push(tr(&format!(
                "Лечение {:.0}-{:.0}",
                a.damage[0], a.damage[1]
            )));
        } else {
            bits.push(format!(
                "{} {}",
                tr(&format!("Урон {:.0}-{:.0}:", a.damage[0], a.damage[1])),
                types_text(a)
            ));
        }
    }
    if a.leech > 0.0 {
        bits.push(tr(&format!("Вампиризм {:.0}%", a.leech)));
    }
    for b in [&a.on_hit, &a.buff].into_iter().flatten() {
        bits.push(tr(&buff_desc(b)));
    }
    if bits.is_empty() {
        bits.push(tr(&a.desc));
    }
    bits.join(" • ")
}

/// The price a fused ability takes on every cast, in lines.
fn price_lines(a: &AbilityDef) -> Vec<String> {
    let mut out = Vec::new();
    if a.fizzle > 0.0 {
        out.push(tr(&format!("Шанс срыва {:.0}%", a.fizzle)));
    }
    if a.backlash > 0.0 {
        out.push(tr(&format!("Отдача {:.0}% здоровья", a.backlash)));
    }
    if let Some(b) = &a.drawback {
        out.push(format!("{} {}", tr("После применения:"), tr(&buff_desc(b))));
    }
    out
}

/// The lines of the hotbar tooltip of a fused ability (after its name).
pub fn tip_lines(a: &AbilityDef) -> Vec<String> {
    let mut out = vec![tr(&format!(
        "Мана {:.0} • перезарядка {:.1}с",
        a.mana,
        a.cooldown_ms as f64 / 1000.0
    ))];
    for part in &a.parts {
        out.push(format!("• {}: {}", tr(&part.name), part_text(part)));
    }
    out.extend(price_lines(a));
    out
}

/// What a fused ability does; returns the y below it.
pub fn details(g: &Gfx, a: &AbilityDef, x: f32, y: f32, w: f32) -> f32 {
    let s = g.s;
    let mut y = paragraph(g, &a.name, x, y, w, 15.0 * s, col(&a.color)) + 2.0 * s;
    g.text(
        &format!(
            "Мана {:.0} • перезарядка {:.1}с",
            a.mana,
            a.cooldown_ms as f64 / 1000.0
        ),
        x,
        y,
        13.0 * s,
        c_dim(),
        false,
    );
    y += 18.0 * s;
    let needs = fusion::equip_needs(a);
    if !needs.is_empty() {
        let names: Vec<String> = needs.iter().map(|n| tr(&gear_need_name(n))).collect();
        y = paragraph(
            g,
            &format!("{} {}", tr("Нужно:"), names.join(", ")),
            x,
            y,
            w,
            13.0 * s,
            col("#e0c080"),
        );
    }
    for part in &a.parts {
        g.text_raw("●", x, y, 13.0 * s, col(&part.color), false);
        let nx = g.text_raw(
            &tr(&part.name),
            x + 14.0 * s,
            y,
            13.0 * s,
            col(&part.color),
            true,
        );
        let text = part_text(part);
        let first = w - (nx - x) - 8.0 * s;
        // the rest of the line next to the name, then wrapped under it
        let lines = g.wrap(&text, first.max(60.0 * s), 13.0 * s, false);
        if let Some(l) = lines.first() {
            g.text_raw(l, nx + 6.0 * s, y, 13.0 * s, c_text(), false);
        }
        y += 17.0 * s;
        let rest: Vec<String> = lines.into_iter().skip(1).collect();
        if !rest.is_empty() {
            y = paragraph(
                g,
                &rest.join(" "),
                x + 14.0 * s,
                y,
                w - 14.0 * s,
                13.0 * s,
                c_text(),
            );
        }
    }
    for l in price_lines(a) {
        y = paragraph(g, &l, x, y, w, 13.0 * s, col("#ff9a8a"));
    }
    y
}

/// Two columns: what the fusion gives and what it costs.
fn pros_cons(g: &Gfx, f: &fusion::Fusion, x: f32, y: f32, w: f32) -> f32 {
    let s = g.s;
    let cw = (w - 20.0 * s) / 2.0;
    let mut bottom = y;
    for (i, (title, list, c, mark)) in [
        ("Преимущества", &f.pros, c_good(), "+"),
        ("Недостатки", &f.cons, col("#ff8a7a"), "−"),
    ]
    .into_iter()
    .enumerate()
    {
        let cx = x + i as f32 * (cw + 20.0 * s);
        g.text(title, cx, y, 14.0 * s, c, true);
        let mut yy = y + 22.0 * s;
        for line in list {
            g.text_raw(mark, cx, yy, 13.0 * s, c, true);
            yy = paragraph(
                g,
                line,
                cx + 14.0 * s,
                yy,
                cw - 14.0 * s,
                13.0 * s,
                mul_c(c, 0.95),
            ) + 3.0 * s;
        }
        bottom = bottom.max(yy);
    }
    bottom
}

/// An ability slot of the crucible: its icon, or a number while empty.
fn slot(g: &mut Gfx, r: Rect, key: Option<&str>, label: &str) {
    let s = g.s;
    round_rect(
        r.x,
        r.y,
        r.w,
        r.h,
        8.0 * s,
        Color::from_rgba(20, 18, 30, 255),
    );
    match key {
        Some(k) => {
            let icon = g.ability_icon(k);
            draw_texture_ex(
                &icon,
                r.x + 4.0 * s,
                r.y + 4.0 * s,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(r.w - 8.0 * s, r.h - 8.0 * s)),
                    ..Default::default()
                },
            );
            round_rect_lines(r.x, r.y, r.w, r.h, 8.0 * s, 1.5 * s, c_accent());
        }
        None => {
            round_rect_lines(
                r.x,
                r.y,
                r.w,
                r.h,
                8.0 * s,
                1.0 * s,
                with_a(c_border(), 0.6),
            );
            g.text_center(
                label,
                r.x + r.w / 2.0,
                r.y + r.h / 2.0,
                r.h * 0.4,
                c_dim(),
                true,
                false,
            );
        }
    }
}

pub fn window(p: &mut Session, g: &mut Gfx, inp: &mut UiInput) {
    let s = g.s;
    let r = centered(1180.0 * s, 690.0 * s);
    let inner = panel(g, r, "Слияние умений");
    let Some(sheet) = p.sheet.clone() else { return };
    let db = content::db();
    let ps = hero(&sheet);
    p.fuse_pick
        .retain(|k| sheet.abilities.contains(k) && !fusion::is_fusion(k));
    let rows = rows(&sheet);
    let n = rows.len();

    // the cursor walks over abilities and fusions
    let step = |p: &mut Session, d: isize| {
        for _ in 0..n {
            p.sel = ((p.sel as isize + d).rem_euclid(n as isize)) as usize;
            if selectable(&rows[p.sel]) {
                break;
            }
        }
    };
    if n > 0 {
        p.sel = p.sel.min(n - 1);
        if !selectable(&rows[p.sel]) {
            step(p, 1);
        }
        if inp.take(KeyCode::Up) {
            step(p, -1);
        }
        if inp.take(KeyCode::Down) {
            step(p, 1);
        }
    }
    let toggle = |p: &mut Session, key: &str| {
        if let Some(i) = p.fuse_pick.iter().position(|k| k == key) {
            p.fuse_pick.remove(i);
        } else if p.fuse_pick.len() < 2 {
            p.fuse_pick.push(key.to_string());
        } else {
            p.fuse_pick[1] = key.to_string();
        }
    };
    let current = rows.get(p.sel);
    if inp.take(KeyCode::Enter) {
        if let Some(Row::Ability(k)) = current {
            toggle(p, k);
        }
    }
    let mut split = None;
    if inp.take(KeyCode::Backspace) || inp.take(KeyCode::Delete) {
        match current {
            Some(Row::Fusion(k)) => split = Some(k.clone()),
            _ => p.fuse_pick.clear(),
        }
    }
    let pair = (p.fuse_pick.len() == 2).then(|| (p.fuse_pick[0].clone(), p.fuse_pick[1].clone()));
    let why = pair
        .as_ref()
        .map(|(a, b)| can_fuse(&ps, a, b))
        .unwrap_or_default();
    let mut fuse_now = inp.take(KeyCode::Space) && pair.is_some() && why.is_empty();

    // ---- the list ----
    let list_w = 400.0 * s;
    let max = max_fusions(sheet.level);
    let intro = if sheet.level < FUSION_LEVEL {
        tr(&format!("Слияние откроется на {}-м уровне.", FUSION_LEVEL))
    } else {
        tr(&format!(
            "Свободно слотов: {} — новый каждые {} уровней.",
            max.saturating_sub(
                sheet
                    .abilities
                    .iter()
                    .filter(|k| fusion::is_fusion(k))
                    .count()
            ),
            FUSION_LEVEL
        ))
    };
    g.text_clip(&intro, inner.x, inner.y, list_w, 13.0 * s, c_dim(), false);
    let top = inner.y + 24.0 * s;
    let row_h = 28.0 * s;
    let max_rows = ((inner.y + inner.h - 34.0 * s - top) / row_h).max(1.0) as usize;
    if n > 0 {
        let wheel = inp.scroll(Rect::new(inner.x, top, list_w, max_rows as f32 * row_h));
        if wheel < 0.0 {
            p.scroll = (p.scroll + 3).min(n.saturating_sub(max_rows));
        } else if wheel > 0.0 {
            p.scroll = p.scroll.saturating_sub(3);
        }
        if p.sel < p.scroll {
            p.scroll = p.sel.saturating_sub(1);
        }
        if p.sel >= p.scroll + max_rows {
            p.scroll = p.sel + 1 - max_rows;
        }
    }
    for (i, rw) in rows.iter().enumerate().skip(p.scroll).take(max_rows) {
        let y = top + (i - p.scroll) as f32 * row_h;
        let rr = Rect::new(inner.x, y, list_w, row_h - 2.0 * s);
        let key = match rw {
            Row::Header(text, c) => {
                g.text_clip(
                    text,
                    rr.x + 4.0 * s,
                    rr.y + 7.0 * s,
                    rr.w,
                    14.0 * s,
                    *c,
                    true,
                );
                continue;
            }
            Row::Ability(k) | Row::Fusion(k) => k,
        };
        let (_, clicked) = row(g, inp, rr, p.sel == i);
        if clicked {
            p.sel = i;
            if let Row::Ability(k) = rw {
                toggle(p, k);
            }
        }
        let Some(a) = db.ability(key) else { continue };
        let ok = matches!(rw, Row::Fusion(_)) || can_fuse_one(&ps, key).is_empty();
        let mark = match p.fuse_pick.iter().position(|k| k == key) {
            Some(0) => "①",
            Some(_) => "②",
            None if matches!(rw, Row::Fusion(_)) => "◆",
            None => "○",
        };
        let picked = mark == "①" || mark == "②";
        g.text_raw(
            mark,
            rr.x + 8.0 * s,
            rr.y + 5.0 * s,
            15.0 * s,
            if picked { c_accent() } else { c_dim() },
            true,
        );
        let icon = g.ability_icon(key);
        draw_texture_ex(
            &icon,
            rr.x + 30.0 * s,
            rr.y + 1.0 * s,
            if ok {
                WHITE
            } else {
                Color::new(0.5, 0.5, 0.55, 0.8)
            },
            DrawTextureParams {
                dest_size: Some(vec2(row_h - 4.0 * s, row_h - 4.0 * s)),
                ..Default::default()
            },
        );
        let fg = if ok { col(&a.color) } else { c_dim() };
        let slot_no = sheet
            .hotbar
            .iter()
            .position(|h| h == key)
            .map(|i| format!("[{}]", i + 1))
            .unwrap_or_default();
        let sw = g.measure(&slot_no, 13.0 * s, false);
        g.text_clip(
            &a.name,
            rr.x + 60.0 * s,
            rr.y + 6.0 * s,
            list_w - 72.0 * s - sw,
            14.0 * s,
            fg,
            picked,
        );
        g.text_raw(
            &slot_no,
            rr.x + rr.w - sw - 8.0 * s,
            rr.y + 6.0 * s,
            13.0 * s,
            c_dim(),
            false,
        );
    }

    // ---- the crucible and the result ----
    let dx = inner.x + list_w + 28.0 * s;
    let dw = inner.x + inner.w - dx;
    let sz = 64.0 * s;
    let mut y = inner.y;
    let showing_fusion = match current {
        Some(Row::Fusion(k)) if p.fuse_pick.is_empty() => Some(k.clone()),
        _ => None,
    };
    let (sa, sb, res) = match &showing_fusion {
        Some(k) => {
            let (a, b) = fusion::fusion_parts(k).unwrap_or(("", ""));
            (Some(a.to_string()), Some(b.to_string()), Some(k.clone()))
        }
        None => (
            p.fuse_pick.first().cloned(),
            p.fuse_pick.get(1).cloned(),
            pair.as_ref().map(|(a, b)| fusion::fusion_key(a, b)),
        ),
    };
    let x0 = dx + (dw - (sz * 3.0 + 110.0 * s)) / 2.0;
    slot(g, Rect::new(x0, y, sz, sz), sa.as_deref(), "1");
    g.text_center(
        "+",
        x0 + sz + 27.0 * s,
        y + sz / 2.0,
        28.0 * s,
        c_accent(),
        true,
        false,
    );
    slot(
        g,
        Rect::new(x0 + sz + 55.0 * s, y, sz, sz),
        sb.as_deref(),
        "2",
    );
    g.text_center(
        "=",
        x0 + sz * 2.0 + 82.0 * s,
        y + sz / 2.0,
        28.0 * s,
        c_accent(),
        true,
        false,
    );
    let rr = Rect::new(x0 + sz * 2.0 + 110.0 * s, y, sz, sz);
    if res.is_some() {
        g.glow(
            rr.x + sz / 2.0,
            rr.y + sz / 2.0,
            sz * 0.75,
            col("#ffd24a"),
            0.35,
        );
    }
    slot(g, rr, res.as_deref(), "?");
    y += sz + 16.0 * s;

    let made = res.as_deref().and_then(|k| fusion::fuse_key(db, k));
    match (&made, &showing_fusion) {
        (Some(f), _) => {
            y = details(g, &f.def, dx, y, dw) + 10.0 * s;
            y = pros_cons(g, f, dx, y, dw) + 8.0 * s;
            let br = Rect::new(dx, y.min(r.y + r.h - 82.0 * s), 220.0 * s, 34.0 * s);
            if showing_fusion.is_some() {
                if button(g, inp, br, "Разделить [Delete]", false) {
                    split = showing_fusion.clone();
                }
            } else if why.is_empty() {
                if button(g, inp, br, "Слить [Пробел]", true) {
                    fuse_now = true;
                }
                paragraph(
                    g,
                    "Умения исчезнут с панели и станут одним; разделить их можно в этом окне.",
                    br.x + br.w + 14.0 * s,
                    br.y + 1.0 * s,
                    dw - br.w - 14.0 * s,
                    12.0 * s,
                    c_dim(),
                );
            } else {
                paragraph(
                    g,
                    &format!("{} {}", tr("Недоступно:"), tr(&why)),
                    dx,
                    br.y,
                    dw,
                    14.0 * s,
                    c_bad(),
                );
            }
        }
        (None, _) => {
            if let Some(Row::Ability(k)) = current {
                if let Some(a) = db.ability(k) {
                    y = super::skills::ability_details(g, a, dx, y, dw) + 10.0 * s;
                    let no = can_fuse_one(&ps, k);
                    if !no.is_empty() {
                        y = paragraph(
                            g,
                            &format!("{} {}", tr("Не сливается:"), tr(&no)),
                            dx,
                            y,
                            dw,
                            14.0 * s,
                            c_bad(),
                        ) + 8.0 * s;
                    }
                }
            }
            paragraph(
                g,
                "Выберите два умения разных классов (Enter или щелчок) — здесь появится их слияние: одно умение, которое делает то же, что оба, с их общими преимуществами и платой за них: больше маны, дольше перезарядка, слабее части и свой изъян.",
                dx,
                y,
                dw,
                13.0 * s,
                c_dim(),
            );
        }
    }

    if fuse_now {
        if let Some((a, b)) = pair {
            p.conn.cmd(Command {
                kind: "fuse".into(),
                key: a,
                text: b,
                index: 0,
            });
            p.fuse_pick.clear();
        }
    }
    if let Some(k) = split {
        p.cmd("unfuse", &k, 0);
    }
    footer(
        g,
        r,
        "↑↓ выбор • Enter / щелчок — взять умение • Пробел — слить • Delete — разделить слияние или очистить выбор • Esc",
    );
}
