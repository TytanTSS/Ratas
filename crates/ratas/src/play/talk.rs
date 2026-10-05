//! Conversations with characters (options or free text when AI is on) and
//! trading.

use macroquad::prelude::*;

use ratas_core::content;

use super::windows::{item_icon, rarity_col};
use super::{Mode, Session};
use crate::gfx::{round_rect, round_rect_lines, tr, Gfx};
use crate::ui::*;

pub fn dialogue(p: &mut Session, g: &mut Gfx, inp: &mut UiInput) {
    let s = g.s;
    let Some(d) = p.dialogue.clone() else {
        p.mode = Mode::Game;
        return;
    };
    let w = (820.0 * s).min(screen_width() - 20.0);
    let text_w = w - 150.0 * s;
    let lines = if d.waiting {
        vec![tr(&format!("{} обдумывает ответ...", d.name))]
    } else {
        g.wrap(&d.text, text_w, 15.0 * s, false)
    };
    let opt_h = 30.0 * s;
    let h = (lines.len() as f32 * 20.0 * s
        + d.options.len() as f32 * opt_h
        + if d.ai { 60.0 } else { 20.0 } * s
        + 110.0 * s)
        .min(screen_height() - 140.0 * s);
    let r = Rect::new(
        (screen_width() - w) / 2.0,
        screen_height() - h - 100.0 * s,
        w,
        h,
    );
    let title = format!("{} — {}", tr(&d.name), tr(&d.role));
    let inner = panel(g, r, &title);
    // the speaker's portrait
    if let Some(e) = p
        .snap
        .as_ref()
        .and_then(|sn| sn.entities.iter().find(|e| e.id == d.npc))
    {
        let pr = Rect::new(inner.x, inner.y, 110.0 * s, 110.0 * s);
        round_rect(
            pr.x,
            pr.y,
            pr.w,
            pr.h,
            6.0 * s,
            Color::from_rgba(30, 28, 44, 255),
        );
        round_rect_lines(pr.x, pr.y, pr.w, pr.h, 6.0 * s, 1.0 * s, c_border());
        let name = crate::art::specs::resolve(&e.model, &e.def, e.glyph, e.kind);
        let nv = crate::art::specs::variants(&name);
        let variant = if nv > 1 {
            (e.id as i32).rem_euclid(nv)
        } else {
            0
        };
        let tex = g.model(&name, &e.color, variant, &e.gear).frames[0].clone();
        let k = (pr.h * 0.85 / tex.height())
            .min(pr.w * 0.85 / tex.width())
            .floor()
            .max(1.0);
        let (tw, th) = (tex.width() * k, tex.height() * k);
        draw_texture_ex(
            &tex,
            pr.x + (pr.w - tw) / 2.0,
            pr.y + pr.h - th - 4.0 * s,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(tw, th)),
                ..Default::default()
            },
        );
    }
    let tx = inner.x + 126.0 * s;
    let mut y = inner.y;
    for l in &lines {
        g.text_raw(
            l,
            tx,
            y,
            15.0 * s,
            if d.waiting { c_dim() } else { c_text() },
            false,
        );
        y += 20.0 * s;
    }
    y += 10.0 * s;
    let n = d.options.len();
    let typing = d.ai && !p.talk.text.is_empty();
    if n > 0 && !typing {
        if inp.take(KeyCode::Up) {
            p.sel = (p.sel + n - 1) % n;
        }
        if inp.take(KeyCode::Down) {
            p.sel = (p.sel + 1) % n;
        }
    }
    p.sel = p.sel.min(n.max(1) - 1);
    let mut choose = None;
    for (i, o) in d.options.iter().enumerate() {
        let rr = Rect::new(tx, y, inner.x + inner.w - tx, opt_h - 4.0 * s);
        let (_, clicked) = row(g, inp, rr, p.sel == i);
        g.text_clip(
            &format!("{}. {}", i + 1, tr(o)),
            rr.x + 10.0 * s,
            rr.y + 5.0 * s,
            rr.w - 20.0 * s,
            15.0 * s,
            if p.sel == i { c_accent() } else { c_text() },
            false,
        );
        if clicked {
            choose = Some(i);
        }
        y += opt_h;
    }
    // digits choose options while nothing is typed
    if !typing {
        let digits = [
            KeyCode::Key1,
            KeyCode::Key2,
            KeyCode::Key3,
            KeyCode::Key4,
            KeyCode::Key5,
            KeyCode::Key6,
            KeyCode::Key7,
            KeyCode::Key8,
            KeyCode::Key9,
        ];
        for (i, k) in digits.iter().enumerate().take(n) {
            if inp.take(*k) {
                choose = Some(i);
                inp.chars.clear();
            }
        }
    }
    if d.ai {
        y += 8.0 * s;
        g.text("Вы:", tx, y + 7.0 * s, 15.0 * s, c_accent(), true);
        if !d.waiting {
            p.talk.handle(inp);
        }
        p.talk.draw(
            g,
            Rect::new(
                tx + 44.0 * s,
                y,
                inner.x + inner.w - tx - 44.0 * s,
                32.0 * s,
            ),
            true,
            &tr("напишите ответ..."),
        );
    } else {
        inp.chars.clear();
    }
    if inp.take(KeyCode::Enter) || inp.take(KeyCode::KpEnter) {
        if d.ai && !p.talk.value().is_empty() {
            if !d.waiting {
                let text = p.talk.value();
                p.log(&format!("{} {text}", tr("Вы:")), "#a0c0ff");
                p.cmd_text("talk", &text);
                p.talk.set("");
            }
        } else {
            choose = Some(p.sel);
        }
    }
    if let Some(i) = choose {
        if i < n {
            if n == 1 && d.options[0] == "Уйти" {
                p.dialogue = None;
                p.mode = Mode::Game;
            } else {
                p.cmd("talk_option", "", i as i32);
            }
        }
    }
    let hint = if d.ai {
        "пишите свой ответ или выберите вариант • Esc — уйти"
    } else {
        "↑↓ / цифры / щелчок — выбор • Enter — ответить • Esc — уйти"
    };
    footer(g, r, hint);
}

pub fn trade(p: &mut Session, g: &mut Gfx, inp: &mut UiInput) {
    let s = g.s;
    let (Some(d), Some(sheet)) = (p.dialogue.clone(), p.sheet.clone()) else {
        p.mode = Mode::Game;
        return;
    };
    let r = centered(980.0 * s, 620.0 * s);
    let inner = panel(g, r, &format!("{} — {}", tr("Торговля"), tr(&d.name)));
    let gold = p.snap.as_ref().map(|s| s.you.gold).unwrap_or(0);
    if inp.take(KeyCode::Tab) || inp.take(KeyCode::Left) || inp.take(KeyCode::Right) {
        p.trade_col = 1 - p.trade_col;
        p.sel = 0;
    }
    let count = if p.trade_col == 0 {
        d.trade.len()
    } else {
        sheet.inventory.len()
    };
    if count > 0 {
        if inp.take(KeyCode::Up) {
            p.sel = (p.sel + count - 1) % count;
        }
        if inp.take(KeyCode::Down) {
            p.sel = (p.sel + 1) % count;
        }
        p.sel = p.sel.min(count - 1);
    }
    let col_w = (inner.w - 20.0 * s) / 2.0;
    let row_h = 34.0 * s;
    let list_h = inner.h - 150.0 * s;
    let max_rows = (list_h / row_h) as usize;
    let mut act: Option<(usize, usize)> = None;
    for (c, header) in ["Товары (купить)", "Ваш рюкзак (продать)"]
        .iter()
        .enumerate()
    {
        let x = inner.x + c as f32 * (col_w + 20.0 * s);
        let hr = Rect::new(x, inner.y, col_w, 26.0 * s);
        g.text(
            header,
            x + 4.0 * s,
            inner.y + 4.0 * s,
            15.0 * s,
            if p.trade_col == c {
                c_accent()
            } else {
                c_dim()
            },
            true,
        );
        if inp.click(hr) {
            p.trade_col = c;
            p.sel = 0;
        }
        let items: Vec<(ratas_core::proto::ItemView, i32)> = if c == 0 {
            d.trade.iter().map(|t| (t.item.clone(), t.price)).collect()
        } else {
            sheet
                .inventory
                .iter()
                .map(|it| (it.clone(), (it.value * 2 / 5).max(1)))
                .collect()
        };
        let start = if p.trade_col == c && p.sel >= max_rows {
            p.sel + 1 - max_rows
        } else {
            0
        };
        for (i, (it, price)) in items.iter().enumerate().skip(start).take(max_rows) {
            let rr = Rect::new(
                x,
                inner.y + 32.0 * s + (i - start) as f32 * row_h,
                col_w,
                row_h - 3.0 * s,
            );
            let selected = p.trade_col == c && p.sel == i;
            let (_, clicked) = row(g, inp, rr, selected);
            let ir = Rect::new(
                rr.x + 4.0 * s,
                rr.y + 1.0 * s,
                rr.h - 2.0 * s,
                rr.h - 2.0 * s,
            );
            item_icon(g, it, ir);
            let mut name = tr(&it.name);
            if it.qty > 1 {
                name = format!("{name} ×{}", it.qty);
            }
            g.text_clip(
                &name,
                ir.x + ir.w + 8.0 * s,
                rr.y + 7.0 * s,
                rr.w - ir.w - 90.0 * s,
                14.0 * s,
                rarity_col(it),
                false,
            );
            let ps = price.to_string();
            let pw = g.measure(&ps, 14.0 * s, true);
            let pc = if c == 0 && *price > gold {
                c_bad()
            } else {
                c_gold()
            };
            g.text_raw(
                &ps,
                rr.x + rr.w - pw - 10.0 * s,
                rr.y + 7.0 * s,
                14.0 * s,
                pc,
                true,
            );
            if clicked {
                if p.double(2000 + c * 100 + i) {
                    act = Some((c, i));
                }
                p.trade_col = c;
                p.sel = i;
            }
        }
    }
    if inp.take(KeyCode::Enter) {
        act = Some((p.trade_col, p.sel));
    }
    // details of the selected item
    let dy = inner.y + inner.h - 108.0 * s;
    let detail = if p.trade_col == 0 {
        d.trade
            .get(p.sel)
            .map(|t| (t.item.clone(), t.price, "Купить"))
    } else {
        sheet
            .inventory
            .get(p.sel)
            .map(|it| (it.clone(), (it.value * 2 / 5).max(1), "Продать"))
    };
    if let Some((it, price, verb)) = detail {
        draw_line(
            inner.x,
            dy - 8.0 * s,
            inner.x + inner.w,
            dy - 8.0 * s,
            1.0,
            c_border(),
        );
        let x = g.text(&it.name, inner.x, dy, 15.0 * s, rarity_col(&it), true);
        g.text_clip(
            &format!(" — {}", tr(&it.desc)),
            x,
            dy,
            inner.x + inner.w - x,
            14.0 * s,
            c_text(),
            false,
        );
        let br = Rect::new(inner.x, dy + 26.0 * s, 260.0 * s, 32.0 * s);
        if button(
            g,
            inp,
            br,
            &tr(&format!("{verb} за {price} золота (Enter)")),
            true,
        ) {
            act = Some((p.trade_col, p.sel));
        }
    }
    g.text(
        &format!("Ваше золото: {gold}"),
        inner.x + inner.w - 220.0 * s,
        dy + 32.0 * s,
        15.0 * s,
        c_gold(),
        true,
    );
    if let Some((c, i)) = act {
        if c == 0 {
            if let Some(t) = d.trade.get(i) {
                p.cmd("buy", &t.item.key, i as i32);
            }
        } else if i < sheet.inventory.len() {
            p.cmd("sell", "", i as i32);
            if i == sheet.inventory.len() - 1 && i > 0 && sheet.inventory[i].qty <= 1 {
                p.sel = i - 1;
            }
        }
    }
    let _ = content::db();
    footer(
        g,
        r,
        "Tab/←→ сторона • ↑↓ выбор • Enter / двойной щелчок — купить, продать • Esc назад",
    );
}
