//! The skills window: a tab for known abilities, one per class the hero
//! develops (its base branch, subclass and hidden skills), the common
//! branches, secret knowledge, and a tab to start another class.

use macroquad::prelude::*;

use ratas_core::content::{self, AbilityDef, ClassDef, SkillDef, SubclassDef};
use ratas_core::game::{
    buff_desc, can_choose_subclass, can_learn, can_start_class, damage_type_name, deed_progress,
    deed_text, gear_need_name, stat_name, PlayerState,
};

use super::Session;
use crate::gfx::{col, mul_c, round_rect, tr, Gfx};
use crate::ui::*;

#[derive(Clone)]
struct Tab {
    kind: &'static str,
    key: String,
    name: String,
    color: String,
}

#[derive(Clone)]
enum Row {
    Header(String),
    Skill(&'static SkillDef),
    Sub(&'static SubclassDef),
    Class(&'static ClassDef),
}

/// The part of the character the client needs for rule checks.
fn hero_state(p: &Session) -> PlayerState {
    let sh = p.sheet.as_ref().unwrap();
    let mut ps = PlayerState {
        level: sh.level,
        skills: sh.skills.clone(),
        skill_points: sh.skill_points.max(1),
        class: sh.class.clone(),
        unlocks: sh.unlocks.clone(),
        deeds: sh.deeds.clone(),
        found: vec![0; sh.found.max(0) as usize],
        ..Default::default()
    };
    for c in &sh.classes {
        ps.classes.push(c.key.clone());
        if !c.subclass.is_empty() {
            ps.subclasses.insert(c.key.clone(), c.subclass.clone());
        }
    }
    ps
}

fn tabs(p: &Session) -> Vec<Tab> {
    let db = content::db();
    let mut out = vec![Tab {
        kind: "abilities",
        key: String::new(),
        name: tr("Умения"),
        color: "#ffffff".into(),
    }];
    let Some(sh) = &p.sheet else { return out };
    for cv in &sh.classes {
        if let Some(c) = db.class(&cv.key) {
            out.push(Tab {
                kind: "class",
                key: c.key.clone(),
                name: format!("{} {}", tr(&c.name), cv.level),
                color: c.color.clone(),
            });
        }
    }
    for b in &db.b.branches {
        if !b.class.is_empty() {
            continue;
        }
        if b.secret
            && !sh
                .skills
                .keys()
                .any(|k| db.skill(k).is_some_and(|sd| sd.branch == b.key))
        {
            continue;
        }
        out.push(Tab {
            kind: "branch",
            key: b.key.clone(),
            name: tr(&b.name),
            color: b.color.clone(),
        });
    }
    out.push(Tab {
        kind: "new",
        key: String::new(),
        name: tr("+ Класс"),
        color: "#a0ffa0".into(),
    });
    out
}

fn branch_skills(branch: &str) -> Vec<&'static SkillDef> {
    let mut v: Vec<&'static SkillDef> = content::db()
        .b
        .skills
        .iter()
        .filter(|s| s.branch == branch)
        .collect();
    v.sort_by_key(|s| s.tier);
    v
}

fn class_branch(class: &str, sub: &str) -> String {
    content::db()
        .b
        .branches
        .iter()
        .find(|b| b.class == class && b.subclass == sub && !b.hidden)
        .map(|b| b.key.clone())
        .unwrap_or_default()
}

fn rows_for(p: &Session, t: &Tab) -> Vec<Row> {
    let db = content::db();
    let sh = p.sheet.as_ref().unwrap();
    let mut rows = vec![];
    match t.kind {
        "class" => {
            let Some(c) = db.class(&t.key) else {
                return rows;
            };
            rows.push(Row::Header(format!(
                "{} «{}»",
                tr("Ветка класса"),
                tr(&c.name)
            )));
            for s in branch_skills(&class_branch(&t.key, "")) {
                rows.push(Row::Skill(s));
            }
            let sub = sh
                .classes
                .iter()
                .find(|cv| cv.key == t.key)
                .map(|cv| cv.subclass.clone())
                .unwrap_or_default();
            if let Some(sc) = db.subclass(&sub) {
                rows.push(Row::Header(format!(
                    "{} «{}»",
                    tr("Подкласс"),
                    tr(&sc.name)
                )));
                for s in branch_skills(&class_branch(&t.key, &sub)) {
                    rows.push(Row::Skill(s));
                }
            } else {
                rows.push(Row::Header(tr("Выбор подкласса")));
                for sc in db.subclasses_of(&t.key) {
                    rows.push(Row::Sub(sc));
                }
            }
            let hidden: Vec<&'static SkillDef> =
                db.b.branches
                    .iter()
                    .filter(|b| b.class == t.key && b.hidden)
                    .flat_map(|b| branch_skills(&b.key))
                    .collect();
            if !hidden.is_empty() {
                let opened = hidden
                    .iter()
                    .filter(|s| sh.skills.get(&s.key).copied().unwrap_or(0) > 0)
                    .count();
                rows.push(Row::Header(tr(&format!(
                    "Скрытые навыки: открыто {} из {}",
                    opened,
                    hidden.len()
                ))));
                rows.extend(hidden.into_iter().map(Row::Skill));
            }
        }
        "branch" => rows.extend(branch_skills(&t.key).into_iter().map(Row::Skill)),
        "new" => {
            for c in &db.b.classes {
                if !sh.classes.iter().any(|cv| cv.key == c.key) {
                    rows.push(Row::Class(c));
                }
            }
        }
        _ => {}
    }
    rows
}

fn selectable(r: &Row) -> bool {
    !matches!(r, Row::Header(_))
}

pub fn window(p: &mut Session, g: &mut Gfx, inp: &mut UiInput) {
    let s = g.s;
    let r = centered(1060.0 * s, 640.0 * s);
    let inner = panel(g, r, "Навыки и классы");
    if p.sheet.is_none() {
        return;
    }
    let tabs = tabs(p);
    let nt = tabs.len();
    if inp.take(KeyCode::Left) || (inp.shift && inp.take(KeyCode::Tab)) {
        p.tab = (p.tab + nt - 1) % nt;
        p.sel = 0;
        p.scroll = 0;
    }
    if inp.take(KeyCode::Right) || inp.take(KeyCode::Tab) {
        p.tab = (p.tab + 1) % nt;
        p.sel = 0;
        p.scroll = 0;
    }
    p.tab %= nt;
    // tabs as buttons, wrapping onto several lines when needed
    let mut tx = inner.x;
    let mut ty = inner.y;
    for (i, t) in tabs.iter().enumerate() {
        let tw = g.measure(&t.name, 14.0 * s, true) + 20.0 * s;
        if tx + tw > inner.x + inner.w {
            tx = inner.x;
            ty += 32.0 * s;
        }
        let tr_ = Rect::new(tx, ty, tw, 28.0 * s);
        let active = i == p.tab;
        let hov = inp.hover(tr_);
        round_rect(
            tr_.x,
            tr_.y,
            tr_.w,
            tr_.h,
            5.0 * s,
            if active {
                c_sel()
            } else if hov {
                c_hover()
            } else {
                c_panel2()
            },
        );
        let c = col(&t.color);
        g.text_raw(
            &t.name,
            tr_.x + 10.0 * s,
            tr_.y + 6.0 * s,
            14.0 * s,
            if active { c } else { mul_c(c, 0.6) },
            true,
        );
        if inp.click(tr_) {
            p.tab = i;
            p.sel = 0;
            p.scroll = 0;
        }
        tx += tw + 6.0 * s;
    }
    let top = ty + 40.0 * s;
    let list_w = 440.0 * s;
    let dx = inner.x + list_w + 24.0 * s;
    let dw = inner.x + inner.w - dx;
    let sheet = p.sheet.clone().unwrap();
    let pts = format!("{} {}", tr("Очки навыков:"), sheet.skill_points);
    let pw = g.measure(&pts, 15.0 * s, true);
    g.text_raw(
        &pts,
        inner.x + inner.w - pw,
        r.y + r.h - 50.0 * s,
        15.0 * s,
        c_good(),
        true,
    );
    let t = tabs[p.tab].clone();
    let row_h = 26.0 * s;
    let max_rows = ((inner.y + inner.h - 30.0 * s - top) / row_h).max(1.0) as usize;
    if t.kind == "abilities" {
        let n = sheet.abilities.len();
        if n > 0 {
            if inp.take(KeyCode::Up) {
                p.sel = (p.sel + n - 1) % n;
            }
            if inp.take(KeyCode::Down) {
                p.sel = (p.sel + 1) % n;
            }
            p.sel %= n;
        }
        for (i, key) in sheet.abilities.iter().enumerate().take(max_rows) {
            let Some(a) = content::db().ability(key) else {
                continue;
            };
            let rr = Rect::new(inner.x, top + i as f32 * row_h, list_w, row_h - 2.0 * s);
            let (_, clicked) = row(g, inp, rr, p.sel == i);
            if clicked {
                p.sel = i;
            }
            let slot = sheet
                .hotbar
                .iter()
                .position(|h| h == key)
                .map(|s| (s + 1).to_string())
                .unwrap_or_else(|| " ".into());
            g.text_raw(
                &format!("[{slot}]"),
                rr.x + 6.0 * s,
                rr.y + 4.0 * s,
                14.0 * s,
                c_accent(),
                true,
            );
            let icon = g.ability_icon(key);
            draw_texture_ex(
                &icon,
                rr.x + 36.0 * s,
                rr.y + 1.0 * s,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(row_h - 4.0 * s, row_h - 4.0 * s)),
                    ..Default::default()
                },
            );
            let name = super::hud::ability_tip(p, key)
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            g.text_clip(
                &name,
                rr.x + 64.0 * s,
                rr.y + 4.0 * s,
                list_w - 70.0 * s,
                14.0 * s,
                col(&a.color),
                false,
            );
            if p.sel == i {
                ability_details(g, a, dx, top, dw);
            }
        }
        if n == 0 {
            paragraph(
                g,
                "Пока нет умений. Изучайте навыки в ветках.",
                inner.x,
                top,
                list_w,
                14.0 * s,
                c_dim(),
            );
        }
        for (i, k) in [
            KeyCode::Key1,
            KeyCode::Key2,
            KeyCode::Key3,
            KeyCode::Key4,
            KeyCode::Key5,
            KeyCode::Key6,
        ]
        .iter()
        .enumerate()
        {
            if inp.take(*k) {
                if let Some(a) = sheet.abilities.get(p.sel) {
                    p.cmd("hotbar", a, i as i32);
                }
            }
        }
        footer(
            g,
            r,
            "←→ / щелчок — вкладка • ↑↓ выбор • 1-6 назначить на панель • Esc",
        );
        return;
    }
    let rows = rows_for(p, &t);
    let ps = hero_state(p);
    let n = rows.len();
    if n > 0 {
        let step = |p: &mut Session, d: isize| {
            for _ in 0..n {
                p.sel = ((p.sel as isize + d).rem_euclid(n as isize)) as usize;
                if selectable(&rows[p.sel]) {
                    break;
                }
            }
        };
        if !selectable(&rows[p.sel.min(n - 1)]) {
            step(p, 1);
        }
        if inp.take(KeyCode::Up) {
            step(p, -1);
        }
        if inp.take(KeyCode::Down) {
            step(p, 1);
        }
        p.sel = p.sel.min(n - 1);
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
    let act = |p: &Session, r: &Row| match r {
        Row::Skill(sd) => p.cmd("learn", &sd.key, 0),
        Row::Sub(sc) => p.cmd("choose_subclass", &sc.key, 0),
        Row::Class(c) => p.cmd("start_class", &c.key, 0),
        Row::Header(_) => {}
    };
    if inp.take(KeyCode::Enter) && p.sel < n {
        act(p, &rows[p.sel]);
    }
    for (i, rw) in rows.iter().enumerate().skip(p.scroll).take(max_rows) {
        let y = top + (i - p.scroll) as f32 * row_h;
        let rr = Rect::new(inner.x, y, list_w, row_h - 2.0 * s);
        if let Row::Header(text) = rw {
            g.text_clip(
                text,
                rr.x + 4.0 * s,
                rr.y + 5.0 * s,
                rr.w,
                14.0 * s,
                c_accent(),
                true,
            );
            continue;
        }
        let (_, clicked) = row(g, inp, rr, p.sel == i);
        if clicked {
            if p.double(1000 + i) {
                act(p, rw);
            }
            p.sel = i;
        }
        match rw {
            Row::Skill(sd) => {
                let rank = sheet.skills.get(&sd.key).copied().unwrap_or(0);
                let why = can_learn(&ps, Some(sd));
                let mut fg = if rank >= sd.max_rank {
                    c_good()
                } else if rank > 0 {
                    col("#a0e0a0")
                } else if !why.is_empty() {
                    c_dim()
                } else {
                    c_text()
                };
                let mut name = tr(&sd.name);
                if !sd.deed.is_empty() && rank == 0 {
                    // a hidden skill shows only its deed until it opens
                    let done = deed_progress(&ps, sd);
                    name = format!("??? {}%", (done * 100 / sd.deed_count.max(1)).min(99));
                    fg = c_dim();
                }
                g.text_raw(
                    &format!("T{}", sd.tier),
                    rr.x + 8.0 * s,
                    rr.y + 5.0 * s,
                    12.0 * s,
                    c_dim(),
                    false,
                );
                g.text_raw(
                    if rank > 0 { "●" } else { "○" },
                    rr.x + 34.0 * s,
                    rr.y + 4.0 * s,
                    14.0 * s,
                    fg,
                    false,
                );
                g.text_clip(
                    &name,
                    rr.x + 54.0 * s,
                    rr.y + 4.0 * s,
                    list_w - 110.0 * s,
                    14.0 * s,
                    fg,
                    false,
                );
                let rk = format!("{rank}/{}", sd.max_rank);
                let rkw = g.measure(&rk, 13.0 * s, false);
                g.text_raw(
                    &rk,
                    rr.x + rr.w - rkw - 8.0 * s,
                    rr.y + 5.0 * s,
                    13.0 * s,
                    fg,
                    false,
                );
            }
            Row::Sub(sc) => {
                let why = can_choose_subclass(&ps, &sc.key);
                let (name, fg) = if sc.secret && !ps.unlocked("subclass", &sc.key) {
                    (tr("??? (секретный подкласс)"), c_dim())
                } else if !why.is_empty() {
                    (tr(&sc.name), mul_c(col(&sc.color), 0.6))
                } else {
                    (tr(&sc.name), col(&sc.color))
                };
                g.text_raw("♦", rr.x + 8.0 * s, rr.y + 4.0 * s, 14.0 * s, fg, false);
                g.text_clip(
                    &name,
                    rr.x + 28.0 * s,
                    rr.y + 4.0 * s,
                    list_w - 36.0 * s,
                    14.0 * s,
                    fg,
                    false,
                );
            }
            Row::Class(cl) => {
                let (name, fg) = if cl.secret && !ps.unlocked("class", &cl.key) {
                    (tr("??? (секретный класс)"), c_dim())
                } else if !can_start_class(&ps, &cl.key).is_empty() {
                    (tr(&cl.name), mul_c(col(&cl.color), 0.6))
                } else {
                    (tr(&cl.name), col(&cl.color))
                };
                g.text_raw("♦", rr.x + 8.0 * s, rr.y + 4.0 * s, 14.0 * s, fg, false);
                g.text_clip(
                    &name,
                    rr.x + 28.0 * s,
                    rr.y + 4.0 * s,
                    list_w - 36.0 * s,
                    14.0 * s,
                    fg,
                    false,
                );
            }
            Row::Header(_) => {}
        }
    }
    if let Some(rw) = rows.get(p.sel) {
        row_details(p, g, rw, &ps, dx, top, dw);
    }
    if n == 0 {
        g.text(
            "Все классы уже начаты.",
            inner.x,
            top,
            14.0 * s,
            c_dim(),
            false,
        );
    }
    for (i, k) in [
        KeyCode::Key1,
        KeyCode::Key2,
        KeyCode::Key3,
        KeyCode::Key4,
        KeyCode::Key5,
        KeyCode::Key6,
    ]
    .iter()
    .enumerate()
    {
        if inp.take(*k) {
            if let Some(Row::Skill(sd)) = rows.get(p.sel) {
                if sheet.skills.get(&sd.key).copied().unwrap_or(0) > 0 && !sd.grants.is_empty() {
                    p.cmd("hotbar", &sd.grants, i as i32);
                }
            }
        }
    }
    footer(
        g,
        r,
        "←→ вкладка • ↑↓ выбор • Enter / двойной щелчок — изучить, выбрать • 1-6 на панель • Esc",
    );
}

fn row_details(p: &Session, g: &Gfx, rw: &Row, ps: &PlayerState, x: f32, y: f32, w: f32) {
    let s = g.s;
    let sheet = p.sheet.as_ref().unwrap();
    match rw {
        Row::Skill(sd) => {
            let rank = sheet.skills.get(&sd.key).copied().unwrap_or(0);
            if !sd.deed.is_empty() && rank == 0 {
                g.text("Скрытый навык", x, y, 16.0 * s, col("#ff80ff"), true);
                let text = format!(
                    "{} {}.",
                    tr("Откроется сам, когда вы совершите деяние:"),
                    tr(&deed_text(&sd.deed, sd.deed_count))
                );
                let yy = paragraph(g, &text, x, y + 28.0 * s, w, 14.0 * s, c_text());
                let done = deed_progress(ps, sd);
                g.text_raw(
                    &tr(&format!(
                        "Прогресс: {} из {}",
                        done.min(sd.deed_count),
                        sd.deed_count
                    )),
                    x,
                    yy + 8.0 * s,
                    14.0 * s,
                    c_good(),
                    false,
                );
                bar(
                    g,
                    Rect::new(x, yy + 30.0 * s, w.min(300.0 * s), 12.0 * s),
                    done as f32 / sd.deed_count.max(1) as f32,
                    col("#ff80ff"),
                    "",
                );
                return;
            }
            let why = can_learn(ps, Some(sd));
            skill_details(p, g, sd, rank, &why, x, y, w);
        }
        Row::Sub(sc) => {
            if sc.secret && !ps.unlocked("subclass", &sc.key) {
                g.text("Секретный подкласс", x, y, 16.0 * s, c_accent(), true);
                paragraph(g, "Его открывает задание одного из уникальных персонажей мира. Говорите со странниками и ищите слухи.", x, y + 28.0 * s, w, 14.0 * s, c_dim());
                return;
            }
            let why = can_choose_subclass(ps, &sc.key);
            g.text(&sc.name, x, y, 16.0 * s, col(&sc.color), true);
            let mut yy = paragraph(g, &sc.desc, x, y + 28.0 * s, w, 14.0 * s, c_text()) + 8.0 * s;
            let names: Vec<String> = branch_skills(&class_branch(&sc.class, &sc.key))
                .iter()
                .map(|s| tr(&s.name))
                .collect();
            yy = paragraph(
                g,
                &format!("{} {}", tr("Навыки:"), names.join(", ")),
                x,
                yy,
                w,
                13.0 * s,
                c_dim(),
            ) + 8.0 * s;
            if why.is_empty() {
                paragraph(
                    g,
                    "Enter — выбрать (у класса может быть только один подкласс).",
                    x,
                    yy,
                    w,
                    14.0 * s,
                    c_accent(),
                );
            } else {
                paragraph(
                    g,
                    &format!("{} {}", tr("Недоступно:"), tr(&why)),
                    x,
                    yy,
                    w,
                    14.0 * s,
                    c_bad(),
                );
            }
        }
        Row::Class(cl) => {
            if cl.secret && !ps.unlocked("class", &cl.key) {
                g.text("Секретный класс", x, y, 16.0 * s, c_accent(), true);
                paragraph(
                    g,
                    "Его открывает задание одного из уникальных персонажей мира.",
                    x,
                    y + 28.0 * s,
                    w,
                    14.0 * s,
                    c_dim(),
                );
                return;
            }
            let why = can_start_class(ps, &cl.key);
            g.text(&cl.name, x, y, 16.0 * s, col(&cl.color), true);
            let yy = paragraph(g, &cl.desc, x, y + 28.0 * s, w, 14.0 * s, c_text()) + 8.0 * s;
            if why.is_empty() {
                paragraph(
                    g,
                    "Enter — начать путь этого класса.",
                    x,
                    yy,
                    w,
                    14.0 * s,
                    c_accent(),
                );
            } else {
                paragraph(
                    g,
                    &format!("{} {}", tr("Недоступно:"), tr(&why)),
                    x,
                    yy,
                    w,
                    14.0 * s,
                    c_bad(),
                );
            }
        }
        Row::Header(_) => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn skill_details(
    p: &Session,
    g: &Gfx,
    sd: &SkillDef,
    rank: i32,
    why: &str,
    x: f32,
    y: f32,
    w: f32,
) {
    let s = g.s;
    let mut y = y;
    g.text(&sd.name, x, y, 16.0 * s, c_accent(), true);
    y += 24.0 * s;
    g.text(
        &format!("Ранг {} из {} • с уровня {}", rank, sd.max_rank, sd.level),
        x,
        y,
        13.0 * s,
        c_dim(),
        false,
    );
    y += 20.0 * s;
    if !sd.equip.is_empty() {
        g.text(
            &format!(
                "{} {}",
                tr("Работает, если есть:"),
                tr(&gear_need_name(&sd.equip))
            ),
            x,
            y,
            13.0 * s,
            col("#e0c080"),
            false,
        );
        y += 20.0 * s;
    }
    y = paragraph(g, &sd.desc, x, y + 4.0 * s, w, 14.0 * s, c_text()) + 6.0 * s;
    if !sd.stats.is_empty() {
        g.text("За каждый ранг:", x, y, 13.0 * s, c_dim(), false);
        y += 18.0 * s;
        for (k, v) in &sd.stats {
            g.text_raw(
                &format!(
                    "{} {:+} ({} {:+})",
                    tr(&stat_name(k)),
                    v,
                    tr("сейчас"),
                    v * rank as f64
                ),
                x + 8.0 * s,
                y,
                13.0 * s,
                c_good(),
                false,
            );
            y += 18.0 * s;
        }
        y += 4.0 * s;
    }
    let db = content::db();
    if !sd.requires.is_empty() {
        let names: Vec<String> = sd
            .requires
            .iter()
            .filter_map(|r| db.skill(r))
            .map(|r| tr(&r.name))
            .collect();
        y = paragraph(
            g,
            &format!("{} {}", tr("Требует:"), names.join(", ")),
            x,
            y,
            w,
            13.0 * s,
            c_dim(),
        );
    }
    if let Some(a) = db.ability(&sd.grants) {
        y += 6.0 * s;
        g.text("Даёт умение:", x, y, 13.0 * s, c_dim(), false);
        y = ability_details(g, a, x, y + 20.0 * s, w);
    }
    y += 6.0 * s;
    let (msg, c) = if rank >= sd.max_rank {
        (tr("Изучено полностью."), c_good())
    } else if why.is_empty() && p.sheet.as_ref().is_some_and(|sh| sh.skill_points > 0) {
        (tr("Enter — изучить."), c_accent())
    } else if why.is_empty() {
        (tr("Нет очков навыков."), c_bad())
    } else {
        (format!("{} {}", tr("Недоступно:"), tr(why)), c_bad())
    };
    paragraph(g, &msg, x, y, w, 14.0 * s, c);
}

/// What an ability does; returns the y below it.
pub fn ability_details(g: &Gfx, a: &AbilityDef, x: f32, y: f32, w: f32) -> f32 {
    let s = g.s;
    let mut y = y;
    g.text(&a.name, x, y, 15.0 * s, col(&a.color), true);
    y += 22.0 * s;
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
    let db = content::db();
    if !a.equip.is_empty() {
        g.text(
            &format!("{} {}", tr("Нужно:"), tr(&gear_need_name(&a.equip))),
            x,
            y,
            13.0 * s,
            col("#e0c080"),
            false,
        );
        y += 18.0 * s;
    }
    if a.damage[1] > 0.0 {
        if a.kind == "heal" {
            g.text(
                &format!("Лечение {:.0}-{:.0}", a.damage[0], a.damage[1]),
                x,
                y,
                13.0 * s,
                c_good(),
                false,
            );
        } else {
            let (t, tc) = if !a.split.is_empty() {
                (
                    a.split
                        .iter()
                        .map(|k| tr(&damage_type_name(k)))
                        .collect::<Vec<_>>()
                        .join(" + "),
                    col(&a.color),
                )
            } else if let Some(d) = db.damage_type(&a.dmg_type) {
                (tr(&d.name).to_lowercase(), col(&d.color))
            } else if a.dmg_type.is_empty()
                && !matches!(a.kind.as_str(), "strike" | "cleave" | "dash")
            {
                (tr("тайная магия"), col("#d08aff"))
            } else {
                (tr("оружие"), c_text())
            };
            let nx = g.text(
                &format!("Урон {:.0}-{:.0}:", a.damage[0], a.damage[1]),
                x,
                y,
                13.0 * s,
                c_text(),
                false,
            );
            let nx = g.text_raw(&t, nx + 5.0 * s, y, 13.0 * s, tc, false);
            if matches!(a.kind.as_str(), "strike" | "cleave") {
                g.text(" + удар оружием", nx, y, 13.0 * s, c_dim(), false);
            }
        }
        y += 18.0 * s;
    }
    if a.leech > 0.0 {
        g.text(
            &format!("Вампиризм {:.0}%", a.leech),
            x,
            y,
            13.0 * s,
            col("#ff7a9a"),
            false,
        );
        y += 18.0 * s;
    }
    for b in [&a.on_hit, &a.buff].into_iter().flatten() {
        y = paragraph(g, &buff_desc(b), x, y, w, 13.0 * s, col(&b.color));
    }
    paragraph(g, &a.desc, x, y + 2.0 * s, w, 13.0 * s, c_text())
}
