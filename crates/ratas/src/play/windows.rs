//! Windows over the game: inventory, character, journal, map, help, the
//! pause menu, party, admin tools, class choice and chat.

use macroquad::prelude::*;

use ratas_core::content;
use ratas_core::game::{rarity_color, slot_name, ADMIN_COMMANDS, EQUIP_SLOTS, INVENTORY_SIZE};
use ratas_core::proto::*;

use super::{Mode, Session};
use crate::gfx::{atlas, col, round_rect, round_rect_lines, tr, with_a, Gfx};
use crate::ui::*;

/// Colours an item name by rarity; common items stay plain.
pub fn rarity_col(it: &ItemView) -> Color {
    if it.rarity <= 0 {
        c_text()
    } else {
        col(rarity_color(it.rarity as i32))
    }
}

/// Draws an item icon (or its glyph) into a square.
pub fn item_icon(g: &mut Gfx, it: &ItemView, r: Rect) {
    let color = content::db()
        .item(&it.key)
        .map(|d| d.color.clone())
        .unwrap_or_else(|| it.color.clone());
    if it.rarity > 0 {
        g.glow(
            r.x + r.w / 2.0,
            r.y + r.h / 2.0,
            r.w * 0.55,
            col(rarity_color(it.rarity as i32)),
            0.35,
        );
    }
    match g.icon(&it.key, it.glyph, &color) {
        Some(t) => {
            let k = (r.w * 0.86 / t.width()).min(r.h * 0.86 / t.height());
            let (w, h) = (t.width() * k, t.height() * k);
            draw_texture_ex(
                &t,
                r.x + (r.w - w) / 2.0,
                r.y + (r.h - h) / 2.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(w, h)),
                    ..Default::default()
                },
            );
        }
        None => g.text_center(
            &it.glyph.to_string(),
            r.x + r.w / 2.0,
            r.y + r.h / 2.0,
            r.h * 0.6,
            col(&it.color),
            true,
            true,
        ),
    }
    if it.qty > 1 {
        let q = it.qty.to_string();
        let fs = (r.h * 0.3).max(9.0);
        let qw = g.measure(&q, fs, true);
        g.text_raw(
            &q,
            r.x + r.w - qw - 3.0,
            r.y + r.h - fs - 3.0,
            fs,
            WHITE,
            true,
        );
    }
}

fn kind_name(it: &ItemView) -> String {
    let k = match it.kind.as_str() {
        "weapon" => {
            if it.hands >= 2 {
                "двуручное оружие"
            } else {
                "одноручное оружие"
            }
        }
        "shield" => "щит, левая рука",
        "offhand" => "левая рука",
        "head" => "голова",
        "chest" => "грудь",
        "belt" => "пояс",
        "legs" => "ноги",
        "back" => "спина",
        "ring" => "кольцо",
        "consumable" => "расходуемое",
        "quest" => "предмет задания",
        _ => "",
    };
    tr(k)
}

/// The description of an item in a column; returns the y below it.
fn item_details(
    g: &mut Gfx,
    it: &ItemView,
    x: f32,
    y: f32,
    w: f32,
    price: Option<(&str, i32)>,
) -> f32 {
    let s = g.s;
    let ir = Rect::new(x, y, 48.0 * s, 48.0 * s);
    round_rect(
        ir.x,
        ir.y,
        ir.w,
        ir.h,
        6.0 * s,
        Color::from_rgba(30, 28, 44, 255),
    );
    item_icon(g, it, ir);
    let tx = x + 58.0 * s;
    let mut name = tr(&it.name);
    if it.qty > 1 {
        name = format!("{name} ×{}", it.qty);
    }
    let lines = g.wrap(&name, w - 58.0 * s, 16.0 * s, true);
    let mut ty = y;
    for l in &lines {
        g.text_raw(l, tx, ty, 16.0 * s, rarity_col(it), true);
        ty += 20.0 * s;
    }
    g.text_raw(
        &format!("{} • {} {}", kind_name(it), tr("цена"), it.value),
        tx,
        ty,
        13.0 * s,
        c_dim(),
        false,
    );
    let mut y = (ty + 20.0 * s).max(y + 56.0 * s);
    for part in it.desc.split(", ") {
        for l in g.wrap(part, w, 14.0 * s, false) {
            g.text_raw(&l, x, y, 14.0 * s, c_text(), false);
            y += 18.0 * s;
        }
    }
    if let Some((verb, p)) = price {
        y += 6.0 * s;
        g.text_raw(
            &tr(&format!("{verb} за {p} золота (Enter)")),
            x,
            y,
            14.0 * s,
            c_accent(),
            true,
        );
        y += 20.0 * s;
    }
    y
}

// ---- inventory ----

pub fn inventory(p: &mut Session, g: &mut Gfx, inp: &mut UiInput) {
    let s = g.s;
    let r = centered(900.0 * s, 600.0 * s);
    let inner = panel(g, r, "Инвентарь");
    let Some(sheet) = p.sheet.clone() else { return };
    let slots = EQUIP_SLOTS;
    let items = &sheet.inventory;
    let n = slots.len() + items.len();
    // keyboard: the equipment column, then the backpack grid
    let cols = 6usize;
    if inp.take(KeyCode::Up) {
        p.sel = if p.sel >= slots.len() + cols {
            p.sel - cols
        } else {
            p.sel.saturating_sub(1)
        };
    }
    if inp.take(KeyCode::Down) {
        p.sel = if p.sel >= slots.len() {
            (p.sel + cols).min(n.max(1) - 1)
        } else {
            (p.sel + 1).min(n.max(1) - 1)
        };
    }
    if inp.take(KeyCode::Left) {
        p.sel = p.sel.saturating_sub(1);
    }
    if inp.take(KeyCode::Right) {
        p.sel = (p.sel + 1).min(n.max(1) - 1);
    }
    p.sel = p.sel.min(n.max(1) - 1);
    let use_sel = |p: &Session, sel: usize| {
        if sel < slots.len() {
            p.cmd("unequip", slots[sel], 0);
        } else if sel - slots.len() < items.len() {
            p.cmd("use", "", (sel - slots.len()) as i32);
        }
    };
    if inp.take(KeyCode::Enter) || inp.take(KeyCode::E) {
        use_sel(p, p.sel);
    }
    let bi = p.sel.checked_sub(slots.len()).filter(|i| *i < items.len());
    if inp.take(KeyCode::L) {
        if let Some(i) = bi {
            p.cmd("equip_left", "", i as i32);
        }
    }
    if inp.take(KeyCode::X) || inp.take(KeyCode::Delete) {
        if let Some(i) = bi {
            p.cmd("drop", "", i as i32);
        }
    }
    if inp.take(KeyCode::O) {
        p.cmd("sort", "", 0);
    }

    // equipment column
    let row_h = 38.0 * s;
    let ew = 300.0 * s;
    g.text("Надето", inner.x, inner.y, 15.0 * s, c_accent(), true);
    let mut hovered: Option<usize> = None;
    for (i, slot) in slots.iter().enumerate() {
        let rr = Rect::new(
            inner.x,
            inner.y + 24.0 * s + i as f32 * row_h,
            ew,
            row_h - 4.0 * s,
        );
        let (hov, clicked) = row(g, inp, rr, p.sel == i);
        let it = sheet.equip.get(*slot);
        let ir = Rect::new(
            rr.x + 4.0 * s,
            rr.y + 2.0 * s,
            rr.h - 4.0 * s,
            rr.h - 4.0 * s,
        );
        round_rect(
            ir.x,
            ir.y,
            ir.w,
            ir.h,
            4.0 * s,
            Color::from_rgba(30, 28, 44, 255),
        );
        g.text(
            slot_name(slot),
            ir.x + ir.w + 8.0 * s,
            rr.y + 2.0 * s,
            11.0 * s,
            c_dim(),
            false,
        );
        match it {
            Some(it) => {
                item_icon(g, it, ir);
                g.text_clip(
                    &it.name,
                    ir.x + ir.w + 8.0 * s,
                    rr.y + 15.0 * s,
                    rr.w - ir.w - 16.0 * s,
                    14.0 * s,
                    rarity_col(it),
                    false,
                );
            }
            None => {
                g.text(
                    "(пусто)",
                    ir.x + ir.w + 8.0 * s,
                    rr.y + 15.0 * s,
                    13.0 * s,
                    with_a(c_dim(), 0.6),
                    false,
                );
            }
        }
        if hov {
            hovered = Some(i);
        }
        if clicked {
            if p.double(i) {
                use_sel(p, i);
            }
            p.sel = i;
        }
        if inp.rclick(rr) && it.is_some() {
            use_sel(p, i);
        }
    }
    // backpack grid
    let gx = inner.x + ew + 20.0 * s;
    let cell = 54.0 * s;
    g.text(
        &format!("Рюкзак {}/{}", items.len(), INVENTORY_SIZE),
        gx,
        inner.y,
        15.0 * s,
        c_accent(),
        true,
    );
    for i in 0..INVENTORY_SIZE {
        let (cx, cy) = ((i % cols) as f32, (i / cols) as f32);
        let cr = Rect::new(
            gx + cx * (cell + 4.0 * s),
            inner.y + 24.0 * s + cy * (cell + 4.0 * s),
            cell,
            cell,
        );
        let idx = slots.len() + i;
        let selected = p.sel == idx;
        let hov = inp.hover(cr);
        round_rect(
            cr.x,
            cr.y,
            cr.w,
            cr.h,
            5.0 * s,
            if selected {
                c_sel()
            } else if hov {
                c_hover()
            } else {
                Color::from_rgba(26, 24, 38, 255)
            },
        );
        round_rect_lines(
            cr.x,
            cr.y,
            cr.w,
            cr.h,
            5.0 * s,
            1.0 * s,
            if selected {
                c_accent()
            } else {
                with_a(c_border(), 0.6)
            },
        );
        if let Some(it) = items.get(i) {
            item_icon(
                g,
                it,
                Rect::new(
                    cr.x + 3.0 * s,
                    cr.y + 3.0 * s,
                    cr.w - 6.0 * s,
                    cr.h - 6.0 * s,
                ),
            );
            if hov {
                hovered = Some(idx);
            }
            if inp.click(cr) {
                if p.double(idx) {
                    use_sel(p, idx);
                }
                p.sel = idx;
            }
            if inp.rclick(cr) {
                use_sel(p, idx);
            }
        }
    }
    // details of the hovered or selected item
    let show = hovered.unwrap_or(p.sel);
    let it = if show < slots.len() {
        sheet.equip.get(slots[show]).cloned()
    } else {
        items.get(show - slots.len()).cloned()
    };
    let dx = gx;
    let dy = inner.y + 24.0 * s + 4.0 * (cell + 4.0 * s) + 10.0 * s;
    let dw = inner.x + inner.w - dx;
    if let Some(it) = it {
        item_details(g, &it, dx, dy, dw, None);
    }
    let gold = p.snap.as_ref().map(|s| s.you.gold).unwrap_or(0);
    g.text(
        &format!("Золото: {gold}"),
        inner.x,
        inner.y + inner.h - 40.0 * s,
        15.0 * s,
        c_gold(),
        true,
    );
    footer(g, r, "Двойной щелчок / ПКМ / Enter — надеть, использовать, снять • L — в левую руку • X — выбросить • O — сортировать • Esc");
}

// ---- character ----

const ATTRS: [(&str, &str, &str); 4] = [
    ("str", "Сила", "урон в ближнем бою, сила воинских умений"),
    (
        "dex",
        "Ловкость",
        "крит, уклонение, скорость атаки, урон стрел",
    ),
    (
        "int",
        "Интеллект",
        "мана, её восстановление, сила заклинаний",
    ),
    ("vit", "Выносливость", "здоровье и его восстановление"),
];

pub fn character(p: &mut Session, g: &mut Gfx, inp: &mut UiInput) {
    let s = g.s;
    let r = centered(1000.0 * s, 600.0 * s);
    let inner = panel(g, r, "Персонаж");
    let Some(sh) = p.sheet.clone() else { return };
    let db = content::db();
    if inp.take(KeyCode::Up) {
        p.sel = (p.sel + 3) % 4;
    }
    if inp.take(KeyCode::Down) {
        p.sel = (p.sel + 1) % 4;
    }
    p.sel %= 4;
    if inp.take(KeyCode::Enter) || inp.take(KeyCode::Equal) || inp.take(KeyCode::KpAdd) {
        p.cmd("alloc_attr", ATTRS[p.sel].0, 0);
    }
    // the header: name, level and classes
    let mut x = g.text_raw(
        &format!("{} — {} {}", sh.name, sh.level, tr("уровень")),
        inner.x,
        inner.y,
        18.0 * s,
        c_accent(),
        true,
    ) + 16.0 * s;
    for cv in &sh.classes {
        if let Some(cl) = db.class(&cv.key) {
            let mut label = format!("{} {}", tr(&cl.name), cv.level);
            if let Some(sc) = db.subclass(&cv.subclass) {
                label += &format!(" ({})", tr(&sc.name));
            }
            x = g.text_raw(
                &label,
                x,
                inner.y + 2.0 * s,
                15.0 * s,
                col(&cl.color),
                false,
            ) + 14.0 * s;
        }
    }
    let y0 = inner.y + 34.0 * s;
    g.text(
        &format!("Свободные очки: {}", sh.attr_points),
        inner.x,
        y0,
        15.0 * s,
        c_good(),
        true,
    );
    let aw = 330.0 * s;
    for (i, (k, name, desc)) in ATTRS.iter().enumerate() {
        let rr = Rect::new(inner.x, y0 + 28.0 * s + i as f32 * 62.0 * s, aw, 56.0 * s);
        let (_, clicked) = row(g, inp, rr, p.sel == i);
        if clicked {
            p.sel = i;
        }
        g.text(
            name,
            rr.x + 10.0 * s,
            rr.y + 6.0 * s,
            16.0 * s,
            c_text(),
            true,
        );
        let v = format!(
            "{:.0} ({:.0})",
            sh.attrs.get(*k).copied().unwrap_or(0.0),
            sh.stats.get(*k).copied().unwrap_or(0.0)
        );
        g.text_raw(
            &v,
            rr.x + 170.0 * s,
            rr.y + 6.0 * s,
            16.0 * s,
            c_accent(),
            true,
        );
        g.text_clip(
            desc,
            rr.x + 10.0 * s,
            rr.y + 30.0 * s,
            rr.w - 20.0 * s,
            12.0 * s,
            c_dim(),
            false,
        );
        if sh.attr_points > 0 {
            let br = Rect::new(rr.x + rr.w - 44.0 * s, rr.y + 6.0 * s, 34.0 * s, 24.0 * s);
            if button(g, inp, br, "+", false) {
                p.cmd("alloc_attr", k, 0);
            }
        }
    }
    let st = |k: &str| sh.stats.get(k).copied().unwrap_or(0.0);
    let mut lines = vec![
        format!(
            "Здоровье      {:.0}  (+{:.1}/с)",
            st("max_hp"),
            st("hp_regen")
        ),
        format!(
            "Мана          {:.0}  (+{:.1}/с)",
            st("max_mp"),
            st("mp_regen")
        ),
        format!("Урон оружия   {:.0}-{:.0}", st("dmg_min"), st("dmg_max")),
        format!(
            "Броня         {:.0}  (-{:.0}% физ.)",
            st("armor"),
            100.0 * st("armor") / (st("armor").max(0.0) + 15.0)
        ),
        format!("Уклонение     {:.0}%", st("dodge")),
        format!("Крит          {:.0}% ×{:.1}", st("crit"), st("crit_mult")),
        format!("Ближний бой   {:+.0}%", st("melee_pct")),
        format!("Дальний бой   {:+.0}%", st("ranged_pct")),
        format!("Магия         {:+.0}%", st("spell_pct")),
        format!("Атака раз в   {:.2}с", st("attack_ms") / 1000.0),
        format!(
            "Скорость      {:.1} клеток/с",
            1000.0 / st("move_ms").max(1.0)
        ),
        format!("Обзор         {:+.0}", st("sight")),
    ];
    if st("life_leech") != 0.0 {
        lines.push(format!("Вампиризм     {:.1}%", st("life_leech")));
    }
    if st("thorns") != 0.0 {
        lines.push(format!("Шипы          {:.0}", st("thorns")));
    }
    for (k, n) in [
        ("block", "Блок"),
        ("fury", "Ярость ран."),
        ("duel_pct", "Один на один"),
        ("ambush_pct", "Из засады"),
        ("heal_pct", "Сила лечения"),
        ("mimic_pct", "Сила копий"),
    ] {
        if st(k) != 0.0 {
            lines.push(format!("{} +{:.0}%", tr(n), st(k)));
        }
    }
    lines.push(format!("Убито врагов  {:.0}", st("kills")));
    let sx = inner.x + aw + 30.0 * s;
    g.text("Характеристики", sx, y0, 15.0 * s, c_accent(), true);
    for (i, l) in lines.iter().enumerate() {
        let t = tr(l);
        // label and value in two columns
        let (a, b) = match t.find("  ") {
            Some(pos) => (t[..pos].trim().to_string(), t[pos..].trim().to_string()),
            None => match t.rfind(' ') {
                Some(pos) => (t[..pos].to_string(), t[pos + 1..].to_string()),
                None => (t.clone(), String::new()),
            },
        };
        let ly = y0 + 28.0 * s + i as f32 * 21.0 * s;
        g.text_raw(&a, sx, ly, 14.0 * s, c_dim(), false);
        g.text_raw(&b, sx + 150.0 * s, ly, 14.0 * s, c_text(), false);
    }
    let rx = sx + 320.0 * s;
    let rw = inner.x + inner.w - rx;
    g.text("Сопротивления", rx, y0, 15.0 * s, c_accent(), true);
    let mut ry = y0 + 28.0 * s;
    for dt in &db.b.damage_types {
        let v = st(&format!("res_{}", dt.key));
        let fg = if v > 0.0 {
            c_good()
        } else if v < 0.0 {
            c_bad()
        } else {
            c_dim()
        };
        g.text_clip(
            &dt.name,
            rx,
            ry,
            rw - 60.0 * s,
            14.0 * s,
            col(&dt.color),
            false,
        );
        let vs = format!("{v:.0}%");
        let vw = g.measure(&vs, 14.0 * s, false);
        g.text_raw(&vs, rx + rw - vw, ry, 14.0 * s, fg, false);
        ry += 20.0 * s;
    }
    let mut bonus = vec![];
    for dt in &db.b.damage_types {
        let v = st(&format!("{}_pct", dt.key));
        if v != 0.0 {
            bonus.push(format!("{} {v:+.0}%", tr(&dt.short)));
        }
        let v = st(&format!("add_{}", dt.key));
        if v != 0.0 {
            bonus.push(format!("+{v:.0} {}", tr(&dt.short)));
        }
    }
    if !bonus.is_empty() {
        ry += 10.0 * s;
        g.text("Урон по типам", rx, ry, 15.0 * s, c_accent(), true);
        paragraph(
            g,
            &bonus.join(", "),
            rx,
            ry + 24.0 * s,
            rw,
            13.0 * s,
            c_text(),
        );
    }
    footer(g, r, "↑↓ выбор • Enter / + вложить очко • Esc");
}

// ---- journal ----

pub fn journal(p: &mut Session, g: &mut Gfx, inp: &mut UiInput) {
    let s = g.s;
    let r = centered(820.0 * s, 580.0 * s);
    let inner = panel(g, r, "Журнал заданий");
    let quests = p
        .sheet
        .as_ref()
        .map(|sh| sh.quests.clone())
        .unwrap_or_default();
    if quests.is_empty() {
        g.text(
            "Заданий нет. Поговорите со старостой деревни.",
            inner.x,
            inner.y,
            15.0 * s,
            c_dim(),
            false,
        );
        footer(g, r, "Esc");
        return;
    }
    let n = quests.len();
    p.scroll = p.scroll.min(n.saturating_sub(1));
    let wheel = inp.scroll(r);
    if wheel < 0.0 || inp.take(KeyCode::Down) {
        p.scroll = (p.scroll + 1).min(n - 1);
    }
    if wheel > 0.0 || inp.take(KeyCode::Up) {
        p.scroll = p.scroll.saturating_sub(1);
    }
    let mut y = inner.y;
    for q in quests.iter().skip(p.scroll) {
        if y > inner.y + inner.h - 60.0 * s {
            break;
        }
        let fg = if q.done {
            c_good()
        } else if q.unique {
            col("#ff9aff")
        } else {
            c_text()
        };
        for l in g.wrap(&q.text, inner.w, 15.0 * s, true) {
            g.text_raw(&l, inner.x, y, 15.0 * s, fg, true);
            y += 19.0 * s;
        }
        let status = if q.done {
            tr("выполнено — вернитесь за наградой")
        } else {
            format!("{}/{}", q.have, q.need)
        };
        g.text_clip(
            &format!(
                "{} {status} • {} {}",
                tr("Прогресс:"),
                tr("выдал:"),
                tr(&q.giver)
            ),
            inner.x + 16.0 * s,
            y,
            inner.w - 16.0 * s,
            13.0 * s,
            c_dim(),
            false,
        );
        y += 17.0 * s;
        g.text_clip(
            &format!("{} {}", tr("Награда:"), tr(&q.reward)),
            inner.x + 16.0 * s,
            y,
            inner.w - 16.0 * s,
            13.0 * s,
            col("#ffd24a"),
            false,
        );
        y += 17.0 * s;
        if !q.where_.is_empty() && !q.done {
            g.text_clip(
                &format!("{} {}", tr("Где:"), tr(&q.where_)),
                inner.x + 16.0 * s,
                y,
                inner.w - 16.0 * s,
                13.0 * s,
                c_dim(),
                false,
            );
            y += 17.0 * s;
        }
        y += 12.0 * s;
    }
    footer(g, r, "Колесо / ↑↓ — прокрутка • Esc");
}

// ---- world map ----

pub fn world_map(p: &mut Session, g: &mut Gfx, inp: &mut UiInput) {
    let (Some(level), Some(snap)) = (&p.level, &p.snap) else {
        return;
    };
    let tex = p.wr.atlas.texture(level, p.level_ver, &p.explored, p.t);
    let places = p
        .sheet
        .as_ref()
        .map(|sh| sh.places.as_slice())
        .unwrap_or(&[]);
    let you = p.welcome.as_ref().map(|w| w.you_id).unwrap_or(0);
    let facing = snap
        .entities
        .iter()
        .find(|e| e.id == you)
        .map(|e| e.facing)
        .unwrap_or(0.0);
    let area = Rect::new(0.0, 0.0, screen_width(), screen_height());
    atlas::draw_world_map(g, &tex, level, snap, places, facing, area, p.t);
    g.text_center(
        "M / Tab / Esc — закрыть карту",
        screen_width() / 2.0,
        screen_height() - 14.0 * g.s,
        13.0 * g.s,
        col("#c8b890"),
        false,
        true,
    );
    if inp.clicked {
        inp.used = true;
    }
}

// ---- help ----

pub fn help(_p: &mut Session, g: &mut Gfx, inp: &mut UiInput) {
    let s = g.s;
    let r = centered(900.0 * s, 660.0 * s);
    let inner = panel(g, r, "Помощь");
    let keys: &[(&str, &str)] = &[
        (
            "WASD / стрелки",
            "ходьба в 8 направлениях, в том числе по диагонали",
        ),
        (
            "ЛКМ / Пробел",
            "атака туда, куда указывает курсор (удерживайте)",
        ),
        ("ПКМ", "умение 1 в сторону курсора"),
        ("1-6", "умения с панели в сторону курсора"),
        ("E", "говорить, открыть дверь/сундук, пройти по лестнице"),
        ("Q / R", "выпить зелье здоровья / маны"),
        ("I", "инвентарь: 11 слотов, L — в левую руку"),
        ("K", "классы, подклассы, навыки и панель умений"),
        ("C", "персонаж и характеристики"),
        ("J", "журнал заданий"),
        ("G", "группа: пригласить, принять, покинуть"),
        ("M / Tab", "карта уровня"),
        ("T / Enter", "чат с другими игроками"),
        ("Колесо, + / -", "приблизить или отдалить"),
        ("F5", "сохранить мир (хозяин)"),
        ("F9", "окно администратора (запуск с --admin)"),
        ("F11 / F12", "полный экран / снимок экрана"),
        ("Ctrl+V", "вставить текст из буфера обмена в поле ввода"),
        ("Esc", "меню (пауза в одиночной игре)"),
    ];
    let mut y = inner.y;
    for (k, d) in keys {
        g.text(k, inner.x, y, 14.0 * s, c_accent(), true);
        g.text_clip(
            d,
            inner.x + 170.0 * s,
            y,
            inner.w - 170.0 * s,
            14.0 * s,
            c_text(),
            false,
        );
        y += 20.0 * s;
    }
    y += 8.0 * s;
    let notes = [
        "Урон: 10 типов — рубящий, колющий, дробящий, огонь, холод, молния, яд, тайная магия, свет, тьма. Броня гасит только физический урон, остальное — сопротивления (C). Справа видно, к чему цель стойка и уязвима.",
        "Классы: на 5-м уровне класса выбирается подкласс; с 5-го уровня героя можно начать второй класс (K → «+ Класс»). Секретные классы и подклассы дают уникальные персонажи.",
        "Мир: у каждого региона свой нрав — пустыня, тундра, пепел, проклятые земли опаснее. Вне деревень игроки могут напасть друг на друга — группа (G) защищает своих. Павшего героя можно воскресить в течение минуты.",
        "Диалоги: варианты ответа — мышью или цифрами; при включённом ИИ можно писать персонажу что угодно и нажать Enter.",
    ];
    for n in notes {
        y = paragraph(g, n, inner.x, y, inner.w, 13.0 * s, c_dim()) + 6.0 * s;
    }
    footer(g, r, "F1 / Esc — закрыть");
    let _ = inp;
}

// ---- pause ----

pub fn pause(
    p: &mut Session,
    g: &mut Gfx,
    inp: &mut UiInput,
    cfg: &mut ratas_core::config::Config,
) {
    let s = g.s;
    let mut items: Vec<(&str, String)> = vec![("resume", tr("Продолжить"))];
    if let Some(srv) = &p.srv {
        items.push(("save", tr("Сохранить мир (F5)")));
        if srv.listening().is_empty() {
            items.push((
                "open",
                format!(
                    "{} ({} {})",
                    tr("Открыть мир для сети"),
                    tr("порт"),
                    cfg.port
                ),
            ));
        }
    }
    items.push(("help", tr("Помощь")));
    items.push(("exit", tr("Выйти в главное меню")));
    let n = items.len();
    let row_h = 46.0 * s;
    let r = centered(420.0 * s, n as f32 * row_h + 70.0 * s);
    let inner = panel(g, r, "Меню");
    if inp.take(KeyCode::Up) {
        p.sel = (p.sel + n - 1) % n;
    }
    if inp.take(KeyCode::Down) {
        p.sel = (p.sel + 1) % n;
    }
    p.sel %= n;
    let mut chosen = None;
    if inp.take(KeyCode::Enter) {
        chosen = Some(items[p.sel].0);
    }
    for (i, (k, label)) in items.iter().enumerate() {
        let br = Rect::new(
            inner.x,
            inner.y + i as f32 * row_h,
            inner.w,
            row_h - 8.0 * s,
        );
        if button(g, inp, br, label, i == p.sel) {
            chosen = Some(k);
        }
    }
    match chosen {
        Some("resume") => {
            p.mode = Mode::Game;
            p.set_pause(false);
        }
        Some("save") => p.quick_save(),
        Some("open") => {
            p.open_for_network(cfg.port);
            p.mode = Mode::Game;
        }
        Some("help") => p.mode = Mode::Help,
        Some("exit") => p.quit = true,
        _ => {}
    }
}

// ---- party ----

pub fn party(p: &mut Session, g: &mut Gfx, inp: &mut UiInput) {
    let s = g.s;
    let r = centered(700.0 * s, 520.0 * s);
    let inner = panel(g, r, "Группа");
    let Some(snap) = p.snap.clone() else { return };
    let me = &snap.you;
    let mut rows: Vec<(&str, String)> = vec![];
    for n in &me.invites {
        rows.push(("invite", n.clone()));
    }
    for m in &me.party {
        rows.push(("member", m.name.clone()));
    }
    for n in &snap.online {
        if *n != p.name && !me.party.iter().any(|m| m.name == *n) {
            rows.push(("player", n.clone()));
        }
    }
    let n = rows.len();
    if n > 0 {
        if inp.take(KeyCode::Up) {
            p.sel = (p.sel + n - 1) % n;
        }
        if inp.take(KeyCode::Down) {
            p.sel = (p.sel + 1) % n;
        }
        p.sel %= n;
    }
    let leader = me
        .party
        .iter()
        .find(|m| m.leader)
        .map(|m| m.name.clone())
        .unwrap_or_default();
    let mut y = inner.y;
    let mut section = "";
    for (i, (kind, name)) in rows.iter().enumerate() {
        if *kind != section {
            if !section.is_empty() {
                y += 8.0 * s;
            }
            section = kind;
            let mut title = tr(match *kind {
                "invite" => "Приглашения",
                "member" => "Ваша группа",
                _ => "Игроки в мире",
            });
            if *kind == "member" && !leader.is_empty() {
                title += &format!(" ({} {leader})", tr("лидер:"));
            }
            g.text_raw(&title, inner.x, y, 15.0 * s, c_accent(), true);
            y += 24.0 * s;
        }
        let rr = Rect::new(inner.x, y, inner.w, 34.0 * s);
        let (_, clicked) = row(g, inp, rr, p.sel == i);
        if clicked {
            p.sel = i;
        }
        let bw = 130.0 * s;
        let br = Rect::new(
            rr.x + rr.w - bw - 4.0 * s,
            rr.y + 3.0 * s,
            bw,
            rr.h - 6.0 * s,
        );
        match *kind {
            "invite" => {
                g.text_clip(
                    &format!("{name} {}", tr("зовёт вас в группу")),
                    rr.x + 10.0 * s,
                    rr.y + 8.0 * s,
                    rr.w - 2.0 * bw - 20.0 * s,
                    14.0 * s,
                    c_good(),
                    false,
                );
                if button(g, inp, br, "Принять", false) {
                    p.cmd("party_accept", name, 0);
                }
                let br2 = Rect::new(br.x - bw - 6.0 * s, br.y, bw, br.h);
                if button(g, inp, br2, "Отказаться", false) {
                    p.cmd("party_decline", name, 0);
                }
            }
            "member" => {
                if let Some(m) = me.party.iter().find(|m| m.name == *name) {
                    let c = if m.dead { c_bad() } else { c_text() };
                    g.text_clip(
                        &m.name,
                        rr.x + 10.0 * s,
                        rr.y + 8.0 * s,
                        160.0 * s,
                        14.0 * s,
                        c,
                        true,
                    );
                    g.text_raw(
                        &format!("{} {}", tr("ур."), m.level),
                        rr.x + 180.0 * s,
                        rr.y + 9.0 * s,
                        13.0 * s,
                        c_dim(),
                        false,
                    );
                    if !m.where_.is_empty() {
                        g.text_clip(
                            &m.where_,
                            rr.x + 240.0 * s,
                            rr.y + 9.0 * s,
                            200.0 * s,
                            13.0 * s,
                            c_dim(),
                            false,
                        );
                    } else {
                        bar(
                            g,
                            Rect::new(rr.x + 240.0 * s, rr.y + 10.0 * s, 120.0 * s, 12.0 * s),
                            (m.hp / m.max_hp.max(1.0)) as f32,
                            c_hp(),
                            "",
                        );
                        bar(
                            g,
                            Rect::new(rr.x + 366.0 * s, rr.y + 10.0 * s, 70.0 * s, 12.0 * s),
                            (m.mp / m.max_mp.max(1.0)) as f32,
                            c_mp(),
                            "",
                        );
                    }
                    if m.name == p.name {
                        if button(g, inp, br, "Покинуть", false) {
                            p.cmd("party_leave", "", 0);
                        }
                    } else if leader == p.name && button(g, inp, br, "Исключить", false) {
                        p.cmd("party_kick", name, 0);
                    }
                }
            }
            _ => {
                g.text_clip(
                    name,
                    rr.x + 10.0 * s,
                    rr.y + 8.0 * s,
                    rr.w - bw - 20.0 * s,
                    14.0 * s,
                    c_text(),
                    false,
                );
                if button(g, inp, br, "Пригласить", false) {
                    p.cmd("party_invite", name, 0);
                }
            }
        }
        y += 38.0 * s;
    }
    if rows.is_empty() {
        paragraph(
            g,
            "В мире никого, кроме вас. Откройте мир для сети, чтобы позвать друзей.",
            inner.x,
            inner.y,
            inner.w,
            15.0 * s,
            c_dim(),
        );
    }
    let mut info = tr("Члены группы не ранят друг друга и видят здоровье друг друга.");
    if me.pvp {
        info += " ";
        info += &tr("Остальные игроки вне деревень могут на вас напасть.");
    }
    paragraph(
        g,
        &info,
        inner.x,
        inner.y + inner.h - 50.0 * s,
        inner.w,
        13.0 * s,
        c_dim(),
    );
    if inp.take(KeyCode::Enter) && p.sel < n {
        match rows[p.sel].0 {
            "invite" => p.cmd("party_accept", &rows[p.sel].1, 0),
            "player" => p.cmd("party_invite", &rows[p.sel].1, 0),
            _ => {}
        }
    }
    footer(g, r, "↑↓ выбор • Enter пригласить/принять • Esc");
}

// ---- admin ----

const ADMIN_ACTIONS: &[(&str, &str)] = &[
    ("Бессмертие вкл/выкл", "/god"),
    ("Умения без маны и перезарядки вкл/выкл", "/nocd"),
    ("Полное здоровье и мана", "/heal"),
    ("+1 уровень", "/level"),
    ("+10 очков навыков и характеристик", "/points 10"),
    ("+1000 золота", "/gold 1000"),
    ("Убить врагов вокруг", "/kill"),
    ("Открыть карту уровня", "/reveal"),
    ("Сделать день", "/time day"),
    ("Сделать ночь", "/time night"),
    (
        "Открыть все секретные классы, подклассы и навыки",
        "/unlock all",
    ),
    ("Легендарное оружие и доспех", "/give long_sword legendary"),
    ("Список уникальных персонажей", "/uniques"),
    ("Список уровней мира", "/levels"),
    ("Все команды (/help)", "/help"),
    ("Ввести команду…", ""),
];

pub fn admin(p: &mut Session, g: &mut Gfx, inp: &mut UiInput) {
    let s = g.s;
    let n = ADMIN_ACTIONS.len();
    let row_h = 28.0 * s;
    let r = centered(640.0 * s, n as f32 * row_h + 190.0 * s);
    let inner = panel(g, r, "Администратор (F9)");
    if inp.take(KeyCode::Up) {
        p.sel = (p.sel + n - 1) % n;
    }
    if inp.take(KeyCode::Down) {
        p.sel = (p.sel + 1) % n;
    }
    p.sel %= n;
    let mut run = None;
    if inp.take(KeyCode::Enter) {
        run = Some(p.sel);
    }
    for (i, (label, _)) in ADMIN_ACTIONS.iter().enumerate() {
        let rr = Rect::new(
            inner.x,
            inner.y + i as f32 * row_h,
            inner.w,
            row_h - 2.0 * s,
        );
        let (_, clicked) = row(g, inp, rr, p.sel == i);
        g.text(
            label,
            rr.x + 10.0 * s,
            rr.y + 5.0 * s,
            14.0 * s,
            if p.sel == i { c_accent() } else { c_text() },
            false,
        );
        if clicked {
            p.sel = i;
            run = Some(i);
        }
    }
    if let Some(i) = run {
        let c = ADMIN_ACTIONS[i].1;
        if c.is_empty() {
            p.mode = Mode::Chat;
            p.chat.set("/");
            return;
        }
        p.cmd_text("admin", c);
        if c == "/give long_sword legendary" {
            p.cmd_text("admin", "/give plate_armor legendary");
        }
    }
    let y = inner.y + n as f32 * row_h + 10.0 * s;
    g.text(
        "Команды в чате (T):",
        inner.x,
        y,
        14.0 * s,
        c_accent(),
        true,
    );
    let names: Vec<&str> = ADMIN_COMMANDS.iter().map(|c| c.name).collect();
    paragraph(
        g,
        &names.join(" "),
        inner.x,
        y + 20.0 * s,
        inner.w,
        13.0 * s,
        c_dim(),
    );
    footer(
        g,
        r,
        "Enter / щелчок — выполнить • / — ввести команду • Esc — закрыть",
    );
}

// ---- class choice ----

pub fn class_window(p: &mut Session, g: &mut Gfx, inp: &mut UiInput) {
    let s = g.s;
    let classes = crate::app::starting_classes();
    let db = content::db();
    let n = classes.len().max(1);
    let cols = 4usize;
    let rows = n.div_ceil(cols);
    let cw = 230.0 * s;
    let ch = 150.0 * s;
    let r = centered(
        cols as f32 * (cw + 10.0 * s) + 32.0 * s,
        rows as f32 * (ch + 10.0 * s) + 220.0 * s,
    );
    let inner = panel(g, r, "Выберите класс героя");
    if inp.take(KeyCode::Left) {
        p.sel = (p.sel + n - 1) % n;
    }
    if inp.take(KeyCode::Right) || inp.take(KeyCode::Tab) {
        p.sel = (p.sel + 1) % n;
    }
    if inp.take(KeyCode::Up) {
        p.sel = (p.sel + n - cols.min(n)) % n;
    }
    if inp.take(KeyCode::Down) {
        p.sel = (p.sel + cols) % n;
    }
    p.sel %= n;
    let mut choose = inp.take(KeyCode::Enter);
    for (i, key) in classes.iter().enumerate() {
        let Some(cl) = db.class(key) else { continue };
        let (cx, cy) = ((i % cols) as f32, (i / cols) as f32);
        let cr = Rect::new(
            inner.x + cx * (cw + 10.0 * s),
            inner.y + cy * (ch + 10.0 * s),
            cw,
            ch,
        );
        let selected = p.sel == i;
        let hov = inp.hover(cr);
        round_rect(
            cr.x,
            cr.y,
            cr.w,
            cr.h,
            8.0 * s,
            if selected {
                c_sel()
            } else if hov {
                c_hover()
            } else {
                c_panel2()
            },
        );
        round_rect_lines(
            cr.x,
            cr.y,
            cr.w,
            cr.h,
            8.0 * s,
            1.5 * s,
            if selected {
                col(&cl.color)
            } else {
                with_a(c_border(), 0.6)
            },
        );
        // the class model
        let name =
            crate::art::specs::resolve(&cl.model, &cl.key, '@', crate::art::specs::KIND_PLAYER);
        let tex = g.model(&name, &cl.color, 0, &[]).frames
            [((p.t * 2.0) as usize + i) % 2 * selected as usize]
            .clone();
        let k = ((ch - 40.0 * s) / tex.height())
            .min(90.0 * s / tex.width())
            .floor()
            .max(1.0);
        let (tw, th) = (tex.width() * k, tex.height() * k);
        g.shadow(
            cr.x + 60.0 * s,
            cr.y + ch - 18.0 * s,
            tw * 0.9,
            12.0 * s,
            0.5,
        );
        draw_texture_ex(
            &tex,
            cr.x + 60.0 * s - tw / 2.0,
            cr.y + ch - 18.0 * s - th,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(tw, th)),
                ..Default::default()
            },
        );
        g.text_clip(
            &cl.name,
            cr.x + 116.0 * s,
            cr.y + 12.0 * s,
            cw - 124.0 * s,
            16.0 * s,
            col(&cl.color),
            true,
        );
        let attrs = crate::app::attr_line(&cl.attrs);
        let mut ay = cr.y + 40.0 * s;
        for part in attrs.split("  ").flat_map(|p| {
            p.split(' ')
                .collect::<Vec<_>>()
                .chunks(2)
                .map(|c| c.join(" "))
                .collect::<Vec<_>>()
        }) {
            g.text_raw(&part, cr.x + 116.0 * s, ay, 12.0 * s, c_dim(), false);
            ay += 16.0 * s;
        }
        if inp.click(cr) {
            if p.double(i) {
                choose = true;
            }
            p.sel = i;
        }
    }
    if let Some(cl) = classes.get(p.sel).and_then(|k| db.class(k)) {
        let y = inner.y + rows as f32 * (ch + 10.0 * s) + 6.0 * s;
        g.text(&cl.name, inner.x, y, 17.0 * s, col(&cl.color), true);
        paragraph(
            g,
            &cl.desc,
            inner.x,
            y + 24.0 * s,
            inner.w,
            14.0 * s,
            c_text(),
        );
        let br = Rect::new(
            inner.x + inner.w - 220.0 * s,
            inner.y + inner.h - 40.0 * s,
            220.0 * s,
            36.0 * s,
        );
        if button(g, inp, br, "Выбрать", true) {
            choose = true;
        }
        if choose {
            p.cmd("choose_class", &cl.key, 0);
        }
    }
    footer(
        g,
        r,
        "Стрелки — выбор • Enter / двойной щелчок — подтвердить • Esc — выйти",
    );
}

// ---- chat ----

pub fn chat(p: &mut Session, g: &mut Gfx, inp: &mut UiInput) {
    let s = g.s;
    let w = (520.0 * s).min(screen_width() * 0.4);
    let r = Rect::new(12.0 * s, screen_height() - 44.0 * s, w, 32.0 * s);
    let label = tr("Сказать:");
    let lw = g.measure(&label, 14.0 * s, true) + 10.0 * s;
    round_rect(
        r.x,
        r.y,
        r.w,
        r.h,
        6.0 * s,
        Color::new(0.03, 0.03, 0.06, 0.85),
    );
    g.text_raw(
        &label,
        r.x + 8.0 * s,
        r.y + 8.0 * s,
        14.0 * s,
        c_accent(),
        true,
    );
    if inp.take(KeyCode::Enter) || inp.take(KeyCode::KpEnter) {
        let t = p.chat.value();
        if !t.is_empty() {
            if t.starts_with('/') && p.admin() {
                p.cmd_text("admin", &t);
            } else {
                p.cmd_text("chat", &t);
            }
        }
        p.chat.set("");
        p.mode = Mode::Game;
        return;
    }
    p.chat.handle(inp);
    p.chat.draw(
        g,
        Rect::new(r.x + lw, r.y + 3.0 * s, r.w - lw - 4.0 * s, r.h - 6.0 * s),
        true,
        "",
    );
}
