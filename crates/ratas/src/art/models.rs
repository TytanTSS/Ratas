//! Creature models painted in code: humanoids are assembled from a build
//! (proportions), an outfit, headgear and weapons; beasts are drawn by
//! their own painters. Colours come from the creature definition, so a
//! modded "ice wolf" with model "wolf" gets a recoloured wolf.

use super::*;

pub const C_SKIN: Rgba = rgb(0xf0, 0xc4, 0x9c);
pub const C_SKIN_TAN: Rgba = rgb(0xc8, 0x8e, 0x62);
pub const C_SKIN_DARK: Rgba = rgb(0x8a, 0x5a, 0x3a);
pub const C_BONE: Rgba = rgb(0xe8, 0xe4, 0xd0);
pub const C_STEEL: Rgba = rgb(0xa8, 0xb0, 0xc0);
pub const C_IRON: Rgba = rgb(0x6a, 0x70, 0x7e);
pub const C_GOLD: Rgba = rgb(0xf0, 0xc0, 0x40);
pub const C_WOOD: Rgba = rgb(0x8a, 0x5a, 0x32);
pub const C_LEATHER: Rgba = rgb(0x7a, 0x52, 0x32);
pub const C_DARK: Rgba = rgb(0x16, 0x12, 0x1c);
pub const C_WHITE: Rgba = rgb(0xf4, 0xf4, 0xf4);

/// Proportions of a humanoid.
#[derive(Clone, Copy)]
struct Geom {
    w: i32,
    h: i32,
    head: Rect,
    torso: Rect,
    arm_l: Rect,
    arm_r: Rect,
    leg_l: Rect,
    leg_r: Rect,
    /// top row of the boots (two rows)
    feet: i32,
}

fn build(name: &str) -> Geom {
    let rc = Rect::rc;
    match name {
        "small" => Geom {
            w: 16,
            h: 20,
            head: rc(4, 5, 11, 11),
            torso: rc(5, 12, 10, 15),
            arm_l: rc(3, 12, 4, 15),
            arm_r: rc(11, 12, 12, 15),
            leg_l: rc(5, 16, 7, 17),
            leg_r: rc(8, 16, 10, 17),
            feet: 18,
        },
        "big" => Geom {
            w: 20,
            h: 24,
            head: rc(6, 1, 13, 7),
            torso: rc(4, 8, 15, 16),
            arm_l: rc(1, 9, 3, 16),
            arm_r: rc(16, 9, 18, 16),
            leg_l: rc(5, 17, 8, 21),
            leg_r: rc(11, 17, 14, 21),
            feet: 22,
        },
        "huge" => Geom {
            w: 24,
            h: 28,
            head: rc(8, 1, 15, 8),
            torso: rc(5, 9, 18, 19),
            arm_l: rc(1, 10, 4, 19),
            arm_r: rc(19, 10, 22, 19),
            leg_l: rc(6, 20, 10, 25),
            leg_r: rc(13, 20, 17, 25),
            feet: 26,
        },
        _ => Geom {
            w: 16,
            h: 20,
            head: rc(4, 1, 11, 7),
            torso: rc(4, 8, 11, 13),
            arm_l: rc(2, 9, 3, 13),
            arm_r: rc(12, 9, 13, 13),
            leg_l: rc(5, 14, 7, 17),
            leg_r: rc(8, 14, 10, 17),
            feet: 18,
        },
    }
}

/// A humanoid description.
#[derive(Clone, Default, Debug)]
pub struct Hum {
    pub build: &'static str,
    pub skin: Rgba,
    /// a == 0: bald
    pub hair: Rgba,
    /// short, long, wild, pony
    pub hair_style: &'static str,
    pub beard: Rgba,
    pub eyes: Rgba,
    /// eyes glow (undead, cultists)
    pub glow: bool,
    pub outfit: &'static str,
    pub top: Rgba,
    pub trim: Rgba,
    pub pants: Rgba,
    pub boots: Rgba,
    /// hood, helmet, horned, wizard, crown, horns, feathers, nemes, turban, cap, skull, snout, ears, ...
    pub head: &'static str,
    pub head_col: Rgba,
    /// cloth over the mouth
    pub mask: Rgba,
    pub weapon: &'static str,
    pub weapon_col: Rgba,
    pub shield: Rgba,
    pub shield_mark: Rgba,
    /// kite (default), round, tower
    pub shield_form: &'static str,
    /// left hand: a second weapon (dual wield), book, orb, symbol
    pub offhand: &'static str,
    pub off_col: Rgba,
    pub cape: Rgba,
    pub wings: Rgba,
    pub tail: Rgba,
    pub tusks: bool,
    pub crown: bool,
    /// "", smoke, talons
    pub legs: &'static str,
}

/// Draws a humanoid; frame 1 is the walking step.
pub fn paint_humanoid(h: &Hum, frame: i32) -> Pc {
    let mut g = build(h.build);
    // head room for hats, horns and wings; empty rows are cropped later
    const TOP: i32 = 6;
    g.h += TOP;
    g.feet += TOP;
    for r in [
        &mut g.head,
        &mut g.torso,
        &mut g.arm_l,
        &mut g.arm_r,
        &mut g.leg_l,
        &mut g.leg_r,
    ] {
        *r = r.shift(0, TOP);
    }
    let mut p = Pc::new(g.w, g.h, "hum");
    let cx = g.w / 2;
    let t = g.torso;
    let (mut leg_l, mut leg_r) = (g.leg_l, g.leg_r);
    let (mut arm_l, mut arm_r) = (g.arm_l, g.arm_r);
    if frame == 1 {
        leg_l = leg_l.shift(0, -1);
        arm_l = arm_l.shift(0, 1);
        arm_r = arm_r.shift(0, -1);
    }
    // behind the body: wings, cape, tail
    if h.wings.visible() {
        paint_wings(&mut p, &g, h.wings, frame);
    }
    if h.cape.visible() {
        p.bx(
            Rect::new(t.x0, t.y0, t.x1, g.h.min(g.feet + 1)),
            mul(h.cape, 0.8),
        );
    }
    if h.tail.visible() {
        for i in 0..5 {
            p.px(
                t.x1 + i / 2,
                t.y1 - 1 + i / 2 - 1,
                mul(h.tail, 0.9 - 0.06 * i as f64),
            );
        }
        p.px(t.x1 + 3, t.y1, mul(h.tail, 0.7));
    }

    // legs and feet
    if h.legs == "smoke" {
        for y in t.y1..g.h {
            let k = (y - t.y1) as f64 / (g.h - t.y1) as f64;
            let half = ((t.dx() / 2) as f64 * (1.0 - k * 0.8)) as i32;
            let off = (k * 2.5) as i32;
            for x in cx - half + off..cx + half + off {
                p.set(
                    x,
                    y,
                    alpha(mul(h.top, 1.0 - 0.3 * k), (255.0 * (1.0 - k * 0.6)) as u8),
                );
            }
        }
    } else if h.outfit != "robe" && h.outfit != "wrapdress" {
        let mut pants = if h.pants.visible() {
            h.pants
        } else {
            mul(h.top, 0.7)
        };
        if h.outfit == "bones" {
            pants = C_BONE;
            leg_l = Rect::new(leg_l.x0 + 1, leg_l.y0, leg_l.x1 - 1, leg_l.y1);
            leg_r = Rect::new(leg_r.x0 + 1, leg_r.y0, leg_r.x1 - 1, leg_r.y1);
        }
        if matches!(h.outfit, "fur" | "loin" | "golem") {
            pants = if h.outfit == "fur" { h.top } else { h.skin };
        }
        p.bx(leg_l, pants);
        p.bx(leg_r, mul(pants, 0.92));
        let mut boots = if h.boots.visible() {
            h.boots
        } else {
            mul(C_LEATHER, 0.8)
        };
        if matches!(h.outfit, "bones" | "loin" | "golem" | "fur") {
            boots = mul(pants, 0.85);
        }
        if h.legs == "talons" {
            boots = C_GOLD;
        }
        let fy = g.feet;
        let mut bl = Rect::new(leg_l.x0 - 1, fy, leg_l.x1, fy + 2);
        if frame == 1 {
            bl = bl.shift(0, -1);
        }
        p.bx(bl, boots);
        p.bx(
            Rect::new(leg_r.x0, fy, leg_r.x1 + 1, fy + 2),
            mul(boots, 0.9),
        );
    }

    paint_outfit(&mut p, &g, h, t);

    // arms and hands
    let sleeve = match h.outfit {
        "bones" => C_BONE,
        "loin" | "golem" => h.skin,
        "plate" => mul(h.top, 0.95),
        _ => h.top,
    };
    if h.outfit == "bones" {
        arm_l = Rect::new(arm_l.x1 - 1, arm_l.y0, arm_l.x1, arm_l.y1);
        arm_r = Rect::new(arm_r.x0, arm_r.y0, arm_r.x0 + 1, arm_r.y1);
    }
    p.bx(arm_l, sleeve);
    p.bx(arm_r, mul(sleeve, 0.9));
    let hand = match h.outfit {
        "bones" => C_BONE,
        "golem" => mul(h.skin, 0.85),
        _ => h.skin,
    };
    p.bx(Rect::new(arm_l.x0, arm_l.y1, arm_l.x1, arm_l.y1 + 1), hand);
    p.bx(
        Rect::new(arm_r.x0, arm_r.y1, arm_r.x1, arm_r.y1 + 1),
        mul(hand, 0.9),
    );
    if h.outfit == "plate" {
        p.bx(
            Rect::new(arm_l.x0 - 1, arm_l.y0 - 1, arm_l.x1 + 1, arm_l.y0 + 1),
            mul(h.top, 1.1),
        );
        p.bx(
            Rect::new(arm_r.x0 - 1, arm_r.y0 - 1, arm_r.x1 + 1, arm_r.y0 + 1),
            h.top,
        );
    }

    paint_head(&mut p, h, g.head);

    if h.shield.visible() {
        paint_shield(&mut p, &g, arm_l, h.shield, h.shield_mark, h.shield_form);
    } else if !h.offhand.is_empty() {
        paint_offhand(&mut p, &g, arm_l, h.offhand, h.off_col);
    }
    if !h.weapon.is_empty() {
        paint_weapon(&mut p, &g, arm_r, h.weapon, h.weapon_col);
    }
    p.crop_top();
    p
}

fn paint_outfit(p: &mut Pc, g: &Geom, h: &Hum, t: Rect) {
    let top = h.top;
    match h.outfit {
        "bones" => {
            for y in t.y0..t.y1 - 1 {
                p.px(t.x0 + t.dx() / 2 - 1, y, C_BONE);
                p.px(t.x0 + t.dx() / 2, y, mul(C_BONE, 0.8));
                if (y - t.y0) % 2 == 0 && y < t.y1 - 2 {
                    for x in t.x0 + 1..t.x1 - 1 {
                        p.px(x, y, mul(C_BONE, 0.95 - 0.04 * (x - t.x0) as f64));
                    }
                }
            }
            p.bx(
                Rect::new(t.x0 + 1, t.y1 - 1, t.x1 - 1, t.y1),
                mul(C_BONE, 0.85),
            );
        }
        "loin" | "golem" => {
            p.bx(t, h.skin);
            if h.outfit == "golem" {
                for _ in 0..t.dx() * t.dy() / 6 {
                    let (x, y) = (t.x0 + p.ri(t.dx()), t.y0 + p.ri(t.dy()));
                    p.px(x, y, mul(h.skin, 0.72));
                }
                if h.trim.visible() {
                    let mut y = t.y0 + 1;
                    while y < t.y1 - 1 {
                        let x = t.x0 + 1 + p.ri((t.dx() - 2).max(1));
                        p.px(x, y, h.trim);
                        p.px(x + 1, y + 1, h.trim);
                        y += 2;
                    }
                }
            } else {
                p.px(t.x0 + t.dx() / 2 - 1, t.y0 + 2, mul(h.skin, 0.8));
                p.px(t.x0 + t.dx() / 2, t.y0 + 2, mul(h.skin, 0.8));
                let loin = if h.pants.visible() {
                    h.pants
                } else {
                    C_LEATHER
                };
                p.bx(Rect::new(t.x0, t.y1 - 2, t.x1, t.y1), loin);
            }
        }
        "fur" => {
            p.bx(t, top);
            for _ in 0..t.dx() * t.dy() / 4 {
                let (x, y) = (t.x0 + p.ri(t.dx()), t.y0 + p.ri(t.dy()));
                let k = 0.82 + 0.3 * p.rf();
                p.px(x, y, mul(top, k));
            }
            if h.trim.visible() {
                p.bx(Rect::new(t.x0, t.y1 - 2, t.x1, t.y1 - 1), h.trim);
            }
        }
        "robe" | "wrapdress" => {
            let bottom = g.h;
            for y in t.y0..bottom {
                let k = (y - t.y0) as f64 / (bottom - t.y0) as f64;
                let grow = if y < t.y1 { 0 } else { (k * 2.4 + 0.2) as i32 };
                p.bx(
                    Rect::new(t.x0 - grow, y, t.x1 + grow, y + 1),
                    mul(top, 1.0 - 0.15 * k),
                );
            }
            if h.trim.visible() {
                for y in t.y0 + 1..bottom - 1 {
                    p.px(t.x0 + t.dx() / 2 - 1, y, h.trim);
                }
                for x in t.x0 - 2..t.x1 + 2 {
                    p.px(x, bottom - 2, h.trim);
                }
            }
            if h.outfit == "wrapdress" {
                let mut y = t.y0;
                while y < bottom {
                    for x in t.x0 - 2..t.x1 + 2 {
                        if p.get(x, y).a > 0 {
                            p.px(x, y, mul(top, 0.78));
                        }
                    }
                    y += 2;
                }
            }
            p.px(t.x0 + 1, bottom - 1, mul(C_LEATHER, 0.7));
            p.px(t.x1 - 2, bottom - 1, mul(C_LEATHER, 0.6));
            let belt = if h.boots.visible() {
                h.boots
            } else {
                mul(top, 0.6)
            };
            p.bx(Rect::new(t.x0, t.y1 - 2, t.x1, t.y1 - 1), belt);
        }
        "plate" => {
            p.bx(t, top);
            for y in t.y0 + 1..t.y1 - 2 {
                p.px(t.x0 + 1, y, mul(top, 1.25));
            }
            if h.trim.visible() {
                let mid = t.x0 + t.dx() / 2;
                p.bx(Rect::new(mid - 1, t.y0 + 1, mid + 1, t.y1 + 1), h.trim);
            }
            p.bx(
                Rect::new(t.x0, t.y1 - 2, t.x1, t.y1 - 1),
                mul(C_LEATHER, 0.8),
            );
            p.px(t.x0 + t.dx() / 2, t.y1 - 2, C_GOLD);
        }
        "wrap" => {
            p.bx(t, top);
            let mut y = t.y0;
            while y < t.y1 {
                for x in t.x0..t.x1 {
                    p.px(x, y, mul(top, 0.8));
                }
                y += 2;
            }
        }
        _ => {
            // tunic, leather, rags
            p.bx(t, top);
            if h.outfit == "leather" {
                for i in 0..t.dy() - 1 {
                    p.px(
                        t.x0 + 1 + i * (t.dx() - 2) / (t.dy() - 1).max(1),
                        t.y0 + i,
                        mul(C_LEATHER, 0.75),
                    );
                }
            }
            if h.trim.visible() {
                p.bx(Rect::new(t.x0, t.y0, t.x1, t.y0 + 1), h.trim);
            }
            let belt = if h.boots.visible() {
                h.boots
            } else {
                C_LEATHER
            };
            p.bx(Rect::new(t.x0, t.y1 - 2, t.x1, t.y1 - 1), mul(belt, 0.85));
            p.px(t.x0 + t.dx() / 2, t.y1 - 2, C_GOLD);
            if h.outfit == "rags" {
                let mut x = t.x0;
                while x < t.x1 {
                    p.px(x, t.y1, mul(top, 0.7));
                    x += 2;
                }
                p.px(t.x0 + 2, t.y0 + 2, h.skin);
                p.px(t.x1 - 2, t.y0 + 3, mul(h.skin, 0.9));
            }
        }
    }
}

fn paint_head(p: &mut Pc, h: &Hum, hd: Rect) {
    let mut skin = h.skin;
    let (x0, y0, x1, y1) = (hd.x0, hd.y0, hd.x1 - 1, hd.y1 - 1);
    let face = |p: &mut Pc, skin: Rgba| {
        for y in y0..=y1 {
            for x in x0..=x1 {
                if (y == y0 || y == y1) && (x == x0 || x == x1) {
                    continue;
                }
                let mut k = if x == x1 {
                    0.78
                } else if x == x0 {
                    1.06
                } else {
                    1.0
                };
                if y == y1 {
                    k *= 0.9;
                }
                p.set(x, y, mul(skin, k));
            }
        }
    };
    let eyes_y = y0 + (y1 - y0) * 3 / 5;
    let (ex1, ex2) = (x0 + 2, x1 - 2);
    let eye = if h.eyes.visible() { h.eyes } else { C_DARK };
    if h.head == "skull" {
        skin = C_BONE;
        face(p, skin);
        for ex in [ex1 - 1, ex2] {
            for dy in -1..=0 {
                p.set(ex, eyes_y + dy, C_DARK);
                p.set(ex + 1, eyes_y + dy, mul(C_DARK, 1.6));
            }
            if h.glow {
                p.set(ex, eyes_y, eye);
            }
        }
        for x in ex1..=ex2 {
            if (x - ex1) % 2 == 0 {
                p.set(x, y1, mul(C_BONE, 0.6));
            }
        }
        p.set((x0 + x1) / 2, eyes_y + 1, mul(C_BONE, 0.55));
    } else {
        face(p, skin);
        p.set(ex1, eyes_y, eye);
        p.set(ex2, eyes_y, eye);
        if h.glow {
            p.set(ex1, eyes_y - 1, alpha(eye, 110));
            p.set(ex2, eyes_y - 1, alpha(eye, 110));
        }
        if h.tusks {
            p.set(ex1, y1, C_WHITE);
            p.set(ex2, y1, C_WHITE);
            p.set(ex1, y1 - 1, mul(C_WHITE, 0.9));
            p.set(ex2, y1 - 1, mul(C_WHITE, 0.9));
        }
    }
    if h.mask.visible() {
        for y in eyes_y + 1..=y1 {
            for x in x0..=x1 {
                if p.get(x, y).a > 0 {
                    p.set(
                        x,
                        y,
                        mul(h.mask, 1.0 - 0.15 * (x - x0) as f64 / (x1 - x0) as f64),
                    );
                }
            }
        }
    }
    // hair
    if h.hair.visible()
        && !matches!(
            h.head,
            "hood" | "helmet" | "horned" | "nemes" | "skull" | "fur_hat"
        )
    {
        let hc = h.hair;
        for x in x0 + 1..x1 {
            p.set(x, y0, mul(hc, 1.1));
        }
        for x in x0..=x1 {
            p.set(x, y0 + 1, hc);
        }
        p.set(x0, y0 + 2, mul(hc, 0.9));
        p.set(x1, y0 + 2, mul(hc, 0.75));
        p.set(x0 + 1, y0 + 2, mul(hc, 0.95));
        match h.hair_style {
            "long" => {
                for y in y0 + 2..=y1 + 3 {
                    p.set(x0, y, mul(hc, 0.92));
                    p.set(x1, y, mul(hc, 0.72));
                }
                p.set(x0 - 1, y1 + 1, mul(hc, 0.85));
                p.set(x1 + 1, y1 + 1, mul(hc, 0.7));
            }
            "wild" => {
                let mut x = x0 - 1;
                while x <= x1 + 1 {
                    p.set(x, y0, hc);
                    x += 2;
                }
                p.set(x0 - 1, y0 + 2, hc);
                p.set(x1 + 1, y0 + 2, mul(hc, 0.75));
            }
            "pony" => {
                p.set(x1 + 1, y0 + 2, hc);
                p.set(x1 + 1, y0 + 3, mul(hc, 0.8));
                p.set(x1 + 2, y0 + 4, mul(hc, 0.7));
            }
            _ => {}
        }
    }
    if h.beard.visible() {
        let bc = h.beard;
        for y in eyes_y + 1..=y1 + 2 {
            let mut w = (x1 - x0 + 1) / 2;
            if y > y1 {
                w -= (y - y1) * 2;
            }
            let mid = (x0 + x1 + 1) / 2;
            for x in mid - w..mid + w {
                if y == eyes_y + 1 && x > mid - 2 && x < mid + 1 {
                    continue; // mouth gap
                }
                p.set(
                    x,
                    y,
                    mul(
                        bc,
                        1.0 - 0.18 * (x - (mid - w)) as f64 / (2 * w).max(1) as f64,
                    ),
                );
            }
        }
    }
    // headgear
    let hc = h.head_col;
    match h.head {
        "hood" => {
            for y in y0 - 1..=y1 {
                for x in x0 - 1..=x1 + 1 {
                    let inner = x > x0 && x < x1 && y > y0 + 1;
                    let edge = (y == y0 - 1 && (x <= x0 || x >= x1))
                        || (y == y0 && (x == x0 - 1 || x == x1 + 1));
                    if inner || edge {
                        continue;
                    }
                    let k = if x >= x1 { 0.75 } else { 1.0 };
                    p.set(x, y, mul(hc, k));
                }
            }
            for x in x0 + 1..x1 {
                p.set(x, y0 + 2, mul(skin, 0.55));
            }
            if h.glow {
                p.set(ex1, eyes_y, eye);
                p.set(ex2, eyes_y, eye);
            }
            p.set((x0 + x1) / 2, y0 - 2, mul(hc, 0.9));
        }
        "helmet" | "horned" => {
            for y in y0..=y0 + 3 {
                for x in x0..=x1 {
                    if y == y0 && (x == x0 || x == x1) {
                        continue;
                    }
                    let mut k = if x >= x1 - 1 { 0.75 } else { 1.0 };
                    if y == y0 {
                        k *= 1.2;
                    }
                    p.set(x, y, mul(hc, k));
                }
            }
            for y in y0 + 4..=y1 {
                p.set(x0, y, mul(hc, 0.95));
                p.set(x1, y, mul(hc, 0.7));
            }
            for x in x0 + 1..x1 {
                p.set(x, y0 + 3, C_DARK);
            }
            if h.glow {
                p.set(ex1, y0 + 3, eye);
                p.set(ex2, y0 + 3, eye);
            }
            p.set((x0 + x1) / 2, y0 - 1, mul(hc, 1.1));
            if h.head == "horned" {
                let horn = hex("#e8dcc0");
                p.set(x0 - 1, y0 + 1, horn);
                p.set(x0 - 2, y0, horn);
                p.set(x0 - 2, y0 - 1, mul(horn, 0.9));
                p.set(x1 + 1, y0 + 1, mul(horn, 0.85));
                p.set(x1 + 2, y0, mul(horn, 0.8));
                p.set(x1 + 2, y0 - 1, mul(horn, 0.75));
            }
        }
        "wizard" => {
            let top = y0 - 1;
            for y in top - 5..=y0 + 1 {
                let k = (y - (top - 5)) as f64 / 7.0;
                let half = (k * 3.5 + 0.5) as i32;
                let mid = (x0 + x1 + 1) / 2;
                for x in mid - half..mid + half {
                    let c = if x >= mid + half - 1 {
                        mul(hc, 0.72)
                    } else {
                        hc
                    };
                    p.set(x + ((1.0 - k) * 1.5) as i32, y, c);
                }
            }
            for x in x0 - 2..=x1 + 2 {
                p.set(x, y0 + 2, mul(hc, 0.85));
            }
            if h.trim.visible() {
                for x in x0..=x1 {
                    p.set(x, y0 + 1, h.trim);
                }
            }
        }
        "crown" => {
            for x in x0..=x1 {
                p.set(x, y0, C_GOLD);
                p.set(x, y0 + 1, mul(C_GOLD, 0.8));
            }
            let mut x = x0;
            while x <= x1 {
                p.set(x, y0 - 1, C_GOLD);
                x += 2;
            }
            p.set((x0 + x1) / 2, y0, hex("#e03050"));
        }
        "horns" => {
            let horn = if hc.visible() { hc } else { hex("#2a2026") };
            p.set(x0, y0, horn);
            p.set(x0 - 1, y0 - 1, horn);
            p.set(x0 - 1, y0 - 2, mul(horn, 0.8));
            p.set(x1, y0, mul(horn, 0.8));
            p.set(x1 + 1, y0 - 1, mul(horn, 0.75));
            p.set(x1 + 1, y0 - 2, mul(horn, 0.65));
        }
        "feathers" => {
            for x in x0..=x1 {
                p.set(x, y0 + 1, hc);
            }
            p.set(x0 + 1, y0 - 1, hex("#e04a3a"));
            p.set(x0 + 1, y0, hex("#e04a3a"));
            p.set(x0 + 3, y0 - 2, C_WHITE);
            p.set(x0 + 3, y0 - 1, C_WHITE);
            p.set(x0 + 3, y0, mul(C_WHITE, 0.85));
            p.set(x1 - 1, y0 - 1, hex("#4a8ae0"));
            p.set(x1 - 1, y0, hex("#4a8ae0"));
        }
        "nemes" => {
            for y in y0..=y1 + 3 {
                for x in x0 - 1..=x1 + 1 {
                    let inner = x > x0 && x < x1 && y >= y0 + 2 && y <= y1;
                    if inner || (y > y1 && x > x0 && x < x1) {
                        continue;
                    }
                    let mut c = if (y - y0) % 2 == 1 { hc } else { C_GOLD };
                    if x > x1 - 1 {
                        c = mul(c, 0.75);
                    }
                    p.set(x, y, c);
                }
            }
            p.set((x0 + x1) / 2, y0 - 1, C_GOLD);
        }
        "turban" => {
            for y in y0 - 1..=y0 + 2 {
                for x in x0 - 1..=x1 + 1 {
                    if y == y0 - 1 && (x < x0 + 1 || x > x1 - 1) {
                        continue;
                    }
                    let mut c = if (x + y) % 3 == 0 { mul(hc, 0.82) } else { hc };
                    if x > x1 {
                        c = mul(c, 0.75);
                    }
                    p.set(x, y, c);
                }
            }
            p.set((x0 + x1) / 2, y0, hex("#e03050"));
        }
        "cap" => {
            for x in x0 - 1..=x1 + 1 {
                p.set(x, y0 + 1, mul(hc, 0.85));
            }
            for y in y0 - 1..=y0 {
                for x in x0..=x1 {
                    p.set(
                        x,
                        y,
                        mul(hc, 1.0 - 0.25 * (x - x0) as f64 / (x1 - x0) as f64),
                    );
                }
            }
        }
        "snout" => {
            for x in x0..=x1 {
                p.set(x, y0, mul(skin, 0.75));
                p.set(x, y0 + 1, mul(skin, 0.85));
            }
            p.set(ex1, eyes_y, hex("#f0e040"));
            p.set(ex2, eyes_y, hex("#f0e040"));
            p.set((x0 + x1) / 2, y1, mul(skin, 0.6));
            p.set((x0 + x1) / 2 + 1, y1, mul(skin, 0.6));
        }
        "circlet" => {
            for x in x0..=x1 {
                p.set(
                    x,
                    y0 + 1,
                    mul(hc, 1.0 - 0.2 * (x - x0) as f64 / (x1 - x0) as f64),
                );
            }
            p.set((x0 + x1) / 2, y0 + 1, hex("#7ad0ff"));
            p.set((x0 + x1) / 2 + 1, y0 + 1, mul(hex("#7ad0ff"), 0.8));
        }
        "mitre" => {
            let mid = (x0 + x1 + 1) / 2;
            for y in y0 - 4..=y0 + 1 {
                let half = 2 + (y - (y0 - 4)) / 2;
                for x in mid - half..mid + half {
                    let c = if x >= mid + half - 1 {
                        mul(hc, 0.75)
                    } else {
                        hc
                    };
                    p.set(x, y, c);
                }
            }
            for y in y0 - 3..=y0 + 1 {
                p.set(mid - 1, y, C_GOLD);
            }
            for x in mid - 3..mid + 3 {
                p.set(x, y0, mul(C_GOLD, 0.9));
            }
        }
        "fur_hat" => {
            for y in y0 - 1..=y0 + 2 {
                for x in x0 - 1..=x1 + 1 {
                    if y == y0 - 1 && (x < x0 || x > x1) {
                        continue;
                    }
                    let mut c = if (x * 7 + y * 3).rem_euclid(4) == 0 {
                        mul(hc, 1.15)
                    } else {
                        hc
                    };
                    if x >= x1 {
                        c = mul(c, 0.78);
                    }
                    p.set(x, y, c);
                }
            }
        }
        "ears" => {
            p.set(x0 - 1, y0 + 2, skin);
            p.set(x0 - 2, y0 + 1, mul(skin, 0.9));
            p.set(x1 + 1, y0 + 2, mul(skin, 0.8));
            p.set(x1 + 2, y0 + 1, mul(skin, 0.7));
        }
        _ => {}
    }
    if h.crown {
        for x in x0..=x1 {
            p.set(x, y0, C_GOLD);
        }
        let mut x = x0;
        while x <= x1 {
            p.set(x, y0 - 1, mul(C_GOLD, 1.1));
            x += 2;
        }
        p.set((x0 + x1) / 2, y0, hex("#e03050"));
    }
}

fn paint_wings(p: &mut Pc, g: &Geom, c: Rgba, frame: i32) {
    let t = g.torso;
    let lift = if frame == 1 { -1 } else { 0 };
    let span = 5 + (g.w - 16) / 3;
    for i in 0..span {
        let y = t.y0 - 3 + i + lift;
        let reach = span - i;
        for x in t.x0 - 1 - reach..t.x0 {
            let k = if x == t.x0 - 1 - reach {
                1.15
            } else {
                0.85 + 0.04 * i as f64
            };
            p.set(x, y, mul(c, k));
        }
        for x in t.x1..t.x1 + 1 + reach {
            let k = if x == t.x1 + reach {
                1.0
            } else {
                0.7 + 0.04 * i as f64
            };
            p.set(x, y, mul(c, k));
        }
    }
    let mut i = 0;
    while i < span {
        p.clear(t.x0 - span + i, t.y0 - 3 + span - i / 2 + lift);
        p.clear(t.x1 + span - 1 - i, t.y0 - 3 + span - i / 2 + lift);
        i += 2;
    }
}

fn paint_shield(p: &mut Pc, g: &Geom, arm: Rect, c: Rgba, mark: Rgba, form: &str) {
    let x0 = arm.x0 - 2;
    let mut y0 = arm.y0 + 1;
    let (mut w, mut hh) = if g.w >= 20 { (6, 8) } else { (5, 6) };
    match form {
        "tower" => {
            y0 -= 2;
            hh += 3;
        }
        "round" => {
            w += 1;
            hh = w;
        }
        _ => {}
    }
    for y in y0..y0 + hh {
        for x in x0..x0 + w {
            match form {
                "tower" => {
                    if (y == y0 || y == y0 + hh - 1) && (x == x0 || x == x0 + w - 1) {
                        continue;
                    }
                }
                "round" => {
                    let (dx, dy) = (
                        (x - x0) as f64 - (w - 1) as f64 / 2.0,
                        (y - y0) as f64 - (hh - 1) as f64 / 2.0,
                    );
                    if dx * dx + dy * dy > (w * w) as f64 / 4.0 + 0.3 {
                        continue;
                    }
                }
                _ => {
                    let bottom = y >= y0 + hh - 2 && (x == x0 || x == x0 + w - 1);
                    if bottom || (y == y0 + hh - 1 && (x == x0 + 1 || x == x0 + w - 2)) {
                        continue;
                    }
                }
            }
            let mut k = if x == x0 + w - 1 { 0.75 } else { 1.0 };
            if x == x0 || y == y0 {
                k = 1.15;
            }
            p.set(x, y, mul(c, k));
        }
    }
    if form == "round" {
        p.set(x0 + w / 2, y0 + hh / 2, mul(C_STEEL, 1.1));
    }
    if mark.visible() {
        let (mx, my) = (x0 + w / 2, y0 + hh / 2 - 1);
        for (dx, dy) in [(0, -1), (0, 0), (0, 1), (-1, 0), (1, 0)] {
            p.set(mx + dx, my + dy, mark);
        }
    }
}

fn paint_weapon(p: &mut Pc, g: &Geom, arm: Rect, kind: &str, tint: Rgba) {
    let (hx, hy) = (arm.x1, arm.y1);
    let blade = if tint.visible() { tint } else { C_STEEL };
    match kind {
        "sword" => {
            for y in hy - 8..hy {
                p.set(hx, y, mul(blade, 1.1));
                p.set(hx + 1, y, mul(blade, 0.8));
            }
            p.set(hx, hy - 9, mul(blade, 1.2));
            p.set(hx - 1, hy, C_GOLD);
            p.set(hx, hy, C_GOLD);
            p.set(hx + 1, hy, C_GOLD);
            p.set(hx + 2, hy, mul(C_GOLD, 0.8));
            p.set(hx, hy + 1, C_WOOD);
        }
        "dagger" => {
            for y in hy - 4..hy {
                p.set(hx, y, mul(blade, 1.05));
            }
            p.set(hx - 1, hy, C_IRON);
            p.set(hx + 1, hy, C_IRON);
            p.set(hx, hy + 1, C_WOOD);
        }
        "axe" => {
            for y in hy - 8..=hy + 1 {
                p.set(hx, y, C_WOOD);
            }
            for y in hy - 8..=hy - 5 {
                p.set(hx + 1, y, mul(blade, 1.05));
                p.set(hx + 2, y, mul(blade, 0.85));
            }
            p.set(hx + 3, hy - 7, mul(blade, 0.75));
            p.set(hx + 3, hy - 6, mul(blade, 0.75));
            p.set(hx - 1, hy - 7, mul(blade, 0.9));
        }
        "mace" | "hammer" | "club" => {
            for y in hy - 6..=hy + 1 {
                p.set(hx, y, C_WOOD);
            }
            let head = if kind == "club" {
                mul(C_WOOD, 1.1)
            } else {
                blade
            };
            for y in hy - 9..=hy - 6 {
                for x in hx - 1..=hx + 1 {
                    p.set(x, y, mul(head, 1.1 - 0.15 * (x - hx + 1) as f64));
                }
            }
            if kind == "hammer" {
                p.set(hx + 2, hy - 8, mul(head, 0.8));
                p.set(hx + 2, hy - 7, mul(head, 0.8));
                p.set(hx - 2, hy - 8, head);
                p.set(hx - 2, hy - 7, head);
            }
        }
        "spear" => {
            for y in hy - 12..=hy + 3 {
                p.set(hx, y, C_WOOD);
            }
            p.set(hx, hy - 13, blade);
            p.set(hx, hy - 14, mul(blade, 1.2));
            p.set(hx - 1, hy - 12, mul(blade, 0.8));
            p.set(hx + 1, hy - 12, mul(blade, 0.8));
        }
        "staff" | "skullstaff" | "totem" => {
            let top = (hy - 13).max(1);
            for y in top..=hy + 3 {
                p.set(hx, y, mul(C_WOOD, 1.1));
                p.set(hx + 1, y, mul(C_WOOD, 0.75));
            }
            let gem = if tint.visible() { tint } else { hex("#6ab0ff") };
            match kind {
                "skullstaff" => {
                    p.set(hx + 1, top - 1, mul(C_BONE, 0.8));
                    p.set(hx, top - 2, C_BONE);
                    p.set(hx + 1, top - 2, mul(C_BONE, 0.8));
                    p.set(hx, top - 1, gem);
                }
                "totem" => {
                    p.set(hx - 1, top, gem);
                    p.set(hx + 2, top, gem);
                    p.set(hx - 1, top + 1, C_WHITE);
                    p.set(hx + 2, top + 2, hex("#e04a3a"));
                }
                _ => {
                    p.set(hx, top - 1, mul(gem, 1.2));
                    p.set(hx + 1, top - 1, gem);
                    p.set(hx, top - 2, gem);
                    p.set(hx + 1, top - 2, mul(gem, 0.7));
                }
            }
        }
        "wand" => {
            for y in hy - 4..=hy + 1 {
                p.set(hx, y, C_WOOD);
            }
            let gem = if tint.visible() { tint } else { hex("#c080ff") };
            p.set(hx, hy - 5, gem);
            p.set(hx, hy - 6, mul(gem, 0.8));
        }
        "scythe" => {
            for y in hy - 12..=hy + 3 {
                p.set(hx, y, mul(C_WOOD, 0.8));
            }
            for x in hx - 5..=hx {
                p.set(x, hy - 12, mul(blade, 1.1));
            }
            p.set(hx - 5, hy - 11, blade);
            p.set(hx - 6, hy - 10, mul(blade, 0.8));
        }
        "bow" => {
            // held in the other hand (viewer's left)
            let bx = if g.w >= 20 { 0 } else { 1 };
            let (top, bot) = (hy - 8, hy + 2);
            for y in top..=bot {
                let x = if y == top || y == bot {
                    bx + 2
                } else if y == top + 1 || y == bot - 1 {
                    bx + 1
                } else {
                    bx
                };
                p.set(x, y, mul(C_WOOD, 1.1));
                p.set(bx + 2, y, alpha(C_WHITE, 150));
            }
        }
        "crossbow" => {
            for x in hx - 1..=hx + 3 {
                p.set(x, hy - 1, mul(C_WOOD, 1.1 - 0.08 * (x - hx) as f64));
            }
            p.set(hx - 1, hy, mul(C_WOOD, 0.8));
            for y in hy - 4..=hy + 2 {
                let end = y == hy - 4 || y == hy + 2;
                p.set(
                    if end { hx + 2 } else { hx + 3 },
                    y,
                    mul(blade, if end { 0.8 } else { 1.0 }),
                );
            }
            p.set(hx + 1, hy - 3, alpha(C_WHITE, 150));
            p.set(hx + 1, hy + 1, alpha(C_WHITE, 150));
            p.set(hx + 4, hy - 1, mul(C_STEEL, 1.2));
        }
        "claws" => {
            let claw = hex("#e8e0d0");
            let left = arm.x0 - (g.torso.dx() + 2 * arm.dx());
            for x in [arm.x0, arm.x1 - 1, left, left + arm.dx() - 1] {
                p.set(x, hy + 1, claw);
            }
        }
        _ => {}
    }
}

/// What is held in the left hand: a second weapon (the right-hand painter
/// mirrored) or a book, orb or holy symbol.
fn paint_offhand(p: &mut Pc, g: &Geom, arm_l: Rect, kind: &str, c: Rgba) {
    let (hx, hy) = (arm_l.x0 - 1, arm_l.y1);
    match kind {
        "book" => {
            let cover = if c.visible() { c } else { hex("#7a3a2a") };
            p.bx(Rect::new(hx - 2, hy - 2, hx + 2, hy + 2), cover);
            for y in hy - 1..=hy {
                p.set(hx + 1, y, hex("#f0e8d0"));
            }
            p.set(hx - 1, hy - 1, C_GOLD);
        }
        "orb" => {
            let orb = if c.visible() { c } else { hex("#8ac0ff") };
            p.ball(hx as f64 - 0.5, hy as f64 - 1.5, 1.8, orb, 0.0);
            p.set(hx - 1, hy - 2, C_WHITE);
        }
        "symbol" => {
            let sym = if c.visible() { c } else { C_GOLD };
            for y in hy - 3..=hy + 1 {
                p.set(hx, y, sym);
            }
            p.set(hx - 1, hy - 2, sym);
            p.set(hx + 1, hy - 2, mul(sym, 0.8));
            p.set(hx, hy - 4, alpha(C_WHITE, 180));
        }
        _ => {
            let mut tmp = Pc::new(p.w, p.h, "off");
            let mirror = Rect::new(p.w - arm_l.x1, arm_l.y0, p.w - arm_l.x0, arm_l.y1);
            paint_weapon(&mut tmp, g, mirror, kind, c);
            for y in 0..p.h {
                for x in 0..p.w {
                    let px = tmp.get(x, y);
                    if px.a > 0 {
                        p.set(p.w - 1 - x, y, mul(px, 0.9));
                    }
                }
            }
        }
    }
}

// ---- quadrupeds (side view, facing right) ----

#[derive(Clone, Default)]
pub struct Quad {
    pub w: i32,
    pub h: i32,
    /// cx cy rx ry
    pub body: [f64; 4],
    /// cx cy r
    pub head: [f64; 3],
    pub snout: [f64; 4],
    pub neck: bool,
    /// pointy, round, small, horns
    pub ear: &'static str,
    /// bushy, thin, short, long, spiked
    pub tail: &'static str,
    pub legs: &'static [f64],
    pub leg_w: f64,
    pub leg_top: f64,
    pub fur: Rgba,
    pub belly: Rgba,
    pub accent: Rgba,
    pub muzzle: Rgba,
    pub tusk: bool,
    pub mane: bool,
    pub spots: bool,
    pub wing: bool,
    pub eye: Rgba,
}

pub fn paint_quad(q: &Quad, frame: i32) -> Pc {
    let mut p = Pc::new(q.w, q.h, "quad");
    let fur = q.fur;
    let dark = mul(fur, 0.6);
    let b = q.body;
    let (tx, ty) = (b[0] - b[2] + 0.5, b[1] - b[3] * 0.3);
    match q.tail {
        "bushy" => {
            p.blob(tx - 1.8, ty - 1.2, 2.0, 1.4, mul(fur, 0.9));
            p.blob(tx - 3.2, ty - 0.2, 1.4, 1.1, mix(fur, C_WHITE, 0.25));
        }
        "thin" => {
            for i in 0..6 {
                p.set(
                    tx as i32 - i,
                    (ty + i as f64 * 0.4) as i32,
                    mul(q.accent, 0.95),
                );
            }
        }
        "short" => {
            p.set(tx as i32 - 1, ty as i32 - 1, dark);
            p.set(tx as i32 - 1, ty as i32, fur);
        }
        "long" | "spiked" => {
            for i in 0..9 {
                let fi = i as f64;
                let w = 2.2 - fi * 0.22;
                p.blob(
                    tx - fi * 0.9,
                    ty + 1.0 + fi * 0.35,
                    w.max(0.6),
                    (w * 0.7).max(0.5),
                    mul(fur, 0.95 - 0.03 * fi),
                );
            }
            if q.tail == "spiked" {
                p.set((tx - 8.5) as i32, (ty + 3.0) as i32, q.accent);
                p.set((tx - 9.0) as i32, (ty + 2.6) as i32, q.accent);
            }
        }
        _ => {}
    }
    // legs: far ones darker, step frame alternates
    let bottom = q.h as f64 - 1.0;
    for (i, &lx) in q.legs.iter().enumerate() {
        let lift = if frame == 1 && i % 2 == 1 { 1.0 } else { 0.0 };
        let c = if i % 2 == 1 {
            mul(fur, 0.68)
        } else {
            mul(fur, 0.82)
        };
        for y in q.leg_top as i32..=(bottom - lift) as i32 {
            for x in lx as i32..(lx + q.leg_w) as i32 {
                p.set(x, y, c);
            }
        }
        for x in lx as i32..(lx + q.leg_w) as i32 {
            p.set(x, (bottom - lift) as i32, mul(fur, 0.4));
        }
    }
    if q.wing {
        let wc = mix(q.accent, fur, 0.4);
        let lift = if frame == 1 { -1.5 } else { 0.0 };
        for i in 0..7 {
            let fi = i as f64;
            let x0 = b[0] - 2.0 + fi;
            p.thick(
                b[0] + 1.0,
                b[1] - b[3] + 1.0,
                x0 - 3.0,
                b[1] - b[3] - 6.0 + lift + fi * 0.5,
                1.0,
                mul(wc, 0.8 + 0.03 * fi),
            );
        }
        p.thick(
            b[0] + 1.0,
            b[1] - b[3] + 1.0,
            b[0] - 4.0,
            b[1] - b[3] - 6.0 + lift,
            1.0,
            mul(fur, 0.5),
        );
    }
    p.blob(b[0], b[1], b[2], b[3], fur);
    for y in b[1] as i32..=(b[1] + b[3]) as i32 {
        for x in (b[0] - b[2]) as i32..=(b[0] + b[2]) as i32 {
            let (dx, dy) = (
                (x as f64 + 0.5 - b[0]) / b[2],
                (y as f64 + 0.5 - b[1]) / b[3],
            );
            if dx * dx + dy * dy <= 1.0 && dy > 0.45 {
                p.set(x, y, mul(q.belly, 1.0 - 0.2 * dy));
            }
        }
    }
    if q.spots {
        for i in 0..5 {
            let x = b[0] - b[2] * 0.6 + i as f64 * b[2] * 0.3;
            p.set(
                x as i32,
                (b[1] - b[3] * 0.3 + (i % 2) as f64) as i32,
                q.accent,
            );
        }
    }
    if q.mane {
        let mut x = b[0] - b[2] * 0.6;
        while x < b[0] + b[2] * 0.8 {
            p.set(x as i32, (b[1] - b[3] - 0.2) as i32, mul(fur, 0.5));
            if (x as i32) % 2 == 0 {
                p.set(x as i32, (b[1] - b[3] - 1.2) as i32, mul(fur, 0.45));
            }
            x += 1.0;
        }
    }
    let h = q.head;
    if q.neck {
        p.thick(
            b[0] + b[2] * 0.6,
            b[1] - b[3] * 0.4,
            h[0] - 0.5,
            h[1] + 0.8,
            3.0,
            mul(fur, 0.95),
        );
    }
    match q.ear {
        "pointy" => {
            p.set((h[0] - 1.0) as i32, (h[1] - h[2] - 0.6) as i32, dark);
            p.set((h[0] - 1.0) as i32, (h[1] - h[2] + 0.4) as i32, fur);
            p.set(
                (h[0] + 0.6) as i32,
                (h[1] - h[2] - 0.6) as i32,
                mul(fur, 0.8),
            );
            p.set((h[0] + 0.6) as i32, (h[1] - h[2] + 0.4) as i32, fur);
        }
        "round" => {
            p.blob(h[0] - 1.2, h[1] - h[2] + 0.2, 1.1, 1.1, mul(fur, 0.85));
            p.set((h[0] - 1.2) as i32, (h[1] - h[2] + 0.2) as i32, q.accent);
        }
        "small" => p.set((h[0] - 0.5) as i32, (h[1] - h[2]) as i32, dark),
        "horns" => {
            p.thick(
                h[0] - 1.0,
                h[1] - h[2] + 0.5,
                h[0] - 3.5,
                h[1] - h[2] - 2.0,
                1.0,
                C_BONE,
            );
            p.thick(
                h[0] + 0.5,
                h[1] - h[2] + 0.5,
                h[0] - 1.5,
                h[1] - h[2] - 2.5,
                1.0,
                mul(C_BONE, 0.85),
            );
        }
        _ => {}
    }
    p.blob(h[0], h[1], h[2], h[2] * 0.92, fur);
    let s = q.snout;
    let muzzle = if q.muzzle.visible() {
        q.muzzle
    } else {
        mix(fur, q.belly, 0.35)
    };
    p.blob(s[0], s[1], s[2], s[3], muzzle);
    p.set(
        (s[0] + s[2] - 0.5) as i32,
        (s[1] - s[3] * 0.4) as i32,
        C_DARK,
    );
    let eye = if q.eye.visible() { q.eye } else { C_DARK };
    p.set(
        (h[0] + h[2] * 0.35) as i32,
        (h[1] - h[2] * 0.25) as i32,
        eye,
    );
    if q.tusk {
        p.set(
            (s[0] + s[2] * 0.2) as i32,
            (s[1] + s[3] + 0.2) as i32,
            C_WHITE,
        );
        p.set(
            (s[0] + s[2] * 0.2) as i32 + 1,
            (s[1] + s[3] - 0.6) as i32,
            C_WHITE,
        );
    }
    p
}

// ---- other beasts ----

pub fn paint_spider(c: Rgba, mark: Rgba, frame: i32) -> Pc {
    let mut p = Pc::new(20, 14, "spider");
    let cx = 10.0;
    let leg = mul(c, 0.55);
    for i in 0..4 {
        let fi = i as f64;
        let lift = if frame == 1 && i % 2 == 0 { -1.0 } else { 0.0 };
        let y0 = 6.5 + fi * 0.7;
        let (kx, ky) = (4.2 + fi * 0.4, 2.0 + fi * 2.2 + lift);
        let fy = 6.0 + fi * 2.2;
        p.thick(cx - 1.0, y0, cx - kx, ky, 1.0, leg);
        p.thick(
            cx - kx,
            ky,
            cx - kx - 2.5 + fi * 0.3,
            fy,
            1.0,
            mul(leg, 0.85),
        );
        p.thick(cx, y0, cx + kx - 1.0, ky, 1.0, mul(leg, 0.8));
        p.thick(
            cx + kx - 1.0,
            ky,
            cx + kx + 1.5 - fi * 0.3,
            fy,
            1.0,
            mul(leg, 0.7),
        );
    }
    p.blob(cx - 0.5, 5.0, 4.6, 4.0, c);
    if mark.visible() {
        p.set(cx as i32 - 1, 4, mark);
        p.set(cx as i32, 4, mark);
        p.set(cx as i32 - 1, 5, mark);
        p.set(cx as i32 - 1, 3, mul(mark, 0.8));
    }
    p.blob(cx - 0.5, 9.5, 2.6, 2.2, mul(c, 0.85));
    let red = hex("#ff3a3a");
    p.set(cx as i32 - 2, 9, red);
    p.set(cx as i32, 9, red);
    p.set(cx as i32 - 1, 10, mul(red, 0.8));
    p.set(cx as i32 - 2, 12, C_WHITE);
    p.set(cx as i32, 12, C_WHITE);
    p
}

/// Side view facing right, tail curled over the back.
pub fn paint_scorpion(c: Rgba, frame: i32) -> Pc {
    let mut p = Pc::new(20, 15, "scorpion");
    let leg = mul(c, 0.55);
    for i in 0..4 {
        let fi = i as f64;
        let lift = if frame == 1 && i % 2 == 1 { -1.0 } else { 0.0 };
        let x = 6.0 + fi * 2.2;
        p.thick(x, 10.0, x - 1.2, 12.5 + lift, 1.0, leg);
        p.thick(
            x - 1.2,
            12.5 + lift,
            x - 1.8,
            14.0 + lift,
            1.0,
            mul(leg, 0.8),
        );
    }
    let segs = [
        [4.5, 9.5, 1.7],
        [3.0, 7.5, 1.6],
        [2.6, 5.3, 1.5],
        [3.4, 3.2, 1.4],
        [5.2, 1.8, 1.3],
        [7.3, 1.6, 1.2],
    ];
    for (i, sg) in segs.iter().enumerate() {
        p.blob(sg[0], sg[1], sg[2], sg[2], mul(c, 0.95 - 0.03 * i as f64));
    }
    p.set(9, 2, hex("#3a1a10"));
    p.set(9, 3, hex("#3a1a10"));
    p.set(8, 4, hex("#5a2a10"));
    p.blob(10.0, 9.5, 5.5, 2.4, c);
    let mut x = 7;
    while x <= 13 {
        p.set(x, 8, mul(c, 0.7));
        x += 2;
    }
    p.blob(15.5, 9.8, 1.8, 1.7, mul(c, 1.05));
    p.thick(16.0, 10.5, 18.0, 8.5, 1.2, mul(c, 0.9));
    p.blob(18.3, 7.6, 1.5, 1.2, mul(c, 1.1));
    p.set(19, 8, C_DARK);
    p.thick(16.0, 11.0, 18.0, 12.0, 1.2, mul(c, 0.8));
    p.blob(18.5, 12.3, 1.3, 1.0, mul(c, 0.95));
    p.set(16, 9, C_DARK);
    p
}

pub fn paint_beetle(c: Rgba, frame: i32) -> Pc {
    let mut p = Pc::new(12, 11, "beetle");
    for i in 0..3 {
        let fi = i as f64;
        let lift = if frame == 1 && i == 1 { -1.0 } else { 0.0 };
        p.thick(
            5.0,
            4.0 + fi * 2.0,
            1.0,
            3.0 + fi * 2.5 + lift,
            1.0,
            mul(c, 0.4),
        );
        p.thick(
            6.0,
            4.0 + fi * 2.0,
            10.0,
            3.0 + fi * 2.5 + lift,
            1.0,
            mul(c, 0.35),
        );
    }
    p.blob(5.5, 5.0, 3.6, 3.8, c);
    for y in 2..9 {
        p.set(5, y, mul(c, 0.5));
    }
    p.set(4, 3, mix(c, C_WHITE, 0.7));
    p.blob(5.5, 9.3, 1.8, 1.2, mul(c, 0.6));
    p
}

pub fn paint_bat(c: Rgba, frame: i32) -> Pc {
    let mut p = Pc::new(20, 12, "bat");
    let cx = 10.0;
    let up = if frame == 1 { -2.5 } else { 0.0 };
    let wing = mul(c, 0.75);
    for s in [-1.0, 1.0] {
        for i in 0..8 {
            let fi = i as f64;
            let top = 4.0 + up * (fi / 8.0) + fi * 0.15;
            let mut bot = 6.0 + fi * 0.35;
            if i % 3 == 2 {
                bot -= 1.0;
            }
            for y in top as i32..=bot as i32 {
                p.set((cx + s * (2.0 + fi)) as i32, y, mul(wing, 1.0 - 0.05 * fi));
            }
        }
        p.thick(cx + s * 2.0, 4.0, cx + s * 9.0, 3.0 + up, 1.0, mul(c, 0.5));
    }
    p.blob(cx - 0.5, 6.0, 2.4, 2.8, c);
    p.set(cx as i32 - 2, 3, c);
    p.set(cx as i32 + 1, 3, mul(c, 0.8));
    p.set(cx as i32 - 2, 5, hex("#ff4a4a"));
    p.set(cx as i32, 5, hex("#ff4a4a"));
    p.set(cx as i32 - 1, 7, C_WHITE);
    p
}

pub fn paint_slime(c: Rgba, frame: i32) -> Pc {
    let mut p = Pc::new(16, 12, "slime");
    let sq = if frame == 1 { 0.8 } else { 0.0 };
    let (cx, cy) = (7.5, 7.5 + sq * 0.5);
    for y in 0..12 {
        for x in 0..16 {
            let (dx, dy) = (
                (x as f64 + 0.5 - cx) / (6.5 + sq),
                (y as f64 + 0.5 - cy) / (4.5 - sq),
            );
            if dy > 0.75 || dx * dx + dy * dy > 1.0 {
                continue;
            }
            let l = 1.05 - 0.25 * dx - 0.35 * dy;
            p.set(x, y, alpha(mul(c, l), 225));
        }
    }
    p.set(5, 5, alpha(C_WHITE, 230));
    p.set(4, 6, alpha(C_WHITE, 150));
    p.set(6, 7, C_DARK);
    p.set(9, 7, C_DARK);
    p.set(7, 9, mul(c, 0.4));
    p.set(8, 9, mul(c, 0.4));
    p
}

pub fn paint_ghost(c: Rgba, frame: i32, hair: Rgba, hood: bool) -> Pc {
    let mut p = Pc::new(16, 20, "ghost");
    let cx = 7.5;
    let body = alpha(c, 205);
    for y in 2..20 {
        let mut half = 5.2;
        if y < 7 {
            let dy = (7 - y) as f64 / 5.0;
            half = 5.2 * (1.0 - dy * dy).max(0.0).sqrt();
        }
        if y > 13 {
            half -= (y - 13) as f64 * 0.35;
        }
        let wave = (y as f64 * 0.9 + frame as f64 * 2.0).sin() * 0.8;
        for x in (cx - half + wave) as i32..=(cx + half + wave) as i32 {
            let l = 1.08 - 0.3 * (x as f64 - cx) / 6.0;
            let a = if y > 14 {
                (205 - (y - 14) * 30) as u8
            } else {
                205
            };
            p.set(x, y, alpha(mul(body, l), a));
        }
    }
    if hood {
        let hc = mul(c, 0.35);
        for y in 2..9 {
            for x in 2..14 {
                if p.get(x, y).a > 0 && (!(5..=10).contains(&x) || y < 4) {
                    p.set(x, y, hc);
                }
            }
        }
        for x in 5..=10 {
            for y in 4..9 {
                p.set(x, y, C_DARK);
            }
        }
        let e = hex("#7ad0ff");
        p.set(6, 6, e);
        p.set(9, 6, e);
    } else {
        p.set(5, 6, C_DARK);
        p.set(5, 7, C_DARK);
        p.set(9, 6, C_DARK);
        p.set(9, 7, C_DARK);
        p.set(7, 10, mul(C_DARK, 1.5));
        p.set(7, 11, mul(C_DARK, 1.5));
    }
    if hair.visible() {
        for y in 2..15 {
            p.set(2 + (y % 2), y, alpha(hair, 220));
            p.set(13 - (y % 2), y, alpha(mul(hair, 0.8), 220));
        }
        for x in 3..13 {
            p.set(x, 2, hair);
        }
    }
    p.set(2, 10 + frame, alpha(c, 180));
    p.set(1, 11 + frame, alpha(c, 150));
    p.set(13, 10 - frame + 1, alpha(c, 180));
    p.set(14, 11 - frame + 1, alpha(c, 150));
    p
}

pub fn paint_elemental(c: Rgba, frame: i32) -> Pc {
    let mut p = Pc::new(16, 20, "elemental");
    let shard = |p: &mut Pc, cx: f64, top: f64, bot: f64, half: f64, col: Rgba| {
        for y in top as i32..=bot as i32 {
            let k = (y as f64 - top) / (bot - top);
            let w = half * (1.0 - (k - 0.4).abs() * 1.4);
            for x in (cx - w) as i32..=(cx + w) as i32 {
                let l = 1.15 - 0.4 * (x as f64 - (cx - w)) / (2.0 * w).max(1.0);
                p.set(x, y, alpha(mul(col, l), 235));
            }
        }
    };
    let bob = frame as f64;
    shard(&mut p, 3.5, 6.0 + bob, 13.0 + bob, 2.0, mul(c, 0.85));
    shard(&mut p, 12.0, 5.0 - bob, 12.0 - bob, 2.0, mul(c, 0.8));
    shard(&mut p, 7.5, 2.0 + bob * 0.5, 17.0 + bob * 0.5, 3.6, c);
    shard(&mut p, 7.5, 13.0, 19.0, 1.5, mul(c, 0.7));
    p.set(6, 7 + frame, C_WHITE);
    p.set(9, 7 + frame, C_WHITE);
    p.set(5, 4, alpha(C_WHITE, 200));
    p
}

pub fn paint_worm(c: Rgba, frame: i32) -> Pc {
    let mut p = Pc::new(16, 22, "worm");
    let sway = frame as f64 * 0.8 - 0.4;
    p.blob(7.5, 20.0, 7.0, 2.0, hex("#c8a060"));
    for i in 0..7 {
        let fi = i as f64;
        let x = 7.5 + (fi * 0.7).sin() * sway * 1.5;
        let y = 18.0 - fi * 2.3;
        let r = 3.6 - fi * 0.15;
        p.blob(x, y, r, 1.8, mul(c, 0.92 + 0.04 * (i % 2) as f64));
        p.set((x - r + 1.0) as i32, y as i32, mul(c, 0.6));
    }
    p.blob(7.5 + sway, 3.0, 3.2, 1.8, hex("#4a1010"));
    let mut x = 5;
    while x <= 10 {
        p.set(x + sway as i32, 2, C_WHITE);
        p.set(x + sway as i32 + 1, 4, mul(C_WHITE, 0.8));
        x += 2;
    }
    p
}

pub fn paint_treant(c: Rgba, frame: i32) -> Pc {
    let mut p = Pc::new(24, 28, "treant");
    let bark = mix(hex("#6a4a2a"), c, 0.15);
    let lift = frame as f64;
    p.thick(9.0, 20.0, 6.0, 27.0 - lift, 2.4, mul(bark, 0.85));
    p.thick(14.0, 20.0, 17.0, 27.0, 2.4, mul(bark, 0.75));
    for y in 8..23 {
        for x in 8..16 {
            let mut l = 1.12 - 0.05 * (x - 8) as f64;
            if (x + y / 3) % 4 == 0 {
                l *= 0.75;
            }
            p.set(x, y, mul(bark, l));
        }
    }
    p.thick(8.0, 11.0, 3.0, 15.0 + lift, 2.0, bark);
    p.thick(3.0, 15.0 + lift, 1.0, 12.0 + lift, 1.0, mul(bark, 0.9));
    p.thick(15.0, 11.0, 20.0, 15.0 - lift, 2.0, mul(bark, 0.8));
    p.thick(20.0, 15.0 - lift, 22.0, 12.0 - lift, 1.0, mul(bark, 0.7));
    p.set(10, 13, hex("#ffd84a"));
    p.set(13, 13, hex("#ffd84a"));
    for x in 10..=13 {
        p.set(x, 16, mul(bark, 0.35));
    }
    p.blob(12.0, 6.0, 9.0, 5.5, c);
    p.blob(7.0, 7.0, 4.5, 3.5, mul(c, 1.08));
    p.blob(16.5, 5.0, 4.5, 3.5, mul(c, 0.92));
    for _ in 0..14 {
        let (x, y) = (4 + p.ri(16), 2 + p.ri(8));
        if p.get(x, y).a > 0 {
            p.set(x, y, mul(c, 1.3));
        }
    }
    p
}
