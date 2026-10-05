//! Item pictures, chosen by the kind of item or by its glyph for items the
//! client does not know.

use ratas_core::content::ItemDef;

use super::models::*;
use super::*;

/// Names the picture of an item.
pub fn icon_shape(d: Option<&ItemDef>, glyph: char) -> String {
    let Some(d) = d else { return glyph.to_string() };
    let s = match d.kind.as_str() {
        "weapon" => match d.weapon.as_str() {
            "sword" | "dagger" => "|",
            "axe" => "P",
            "mace" | "hammer" => "T",
            "bow" => ")",
            "crossbow" => "crossbow",
            _ => "/",
        },
        "shield" => "shield",
        "offhand" => match d.look.as_str() {
            "book" => "book",
            "symbol" => "symbol",
            _ => "orb",
        },
        "head" => match d.look.as_str() {
            "helmet" | "horned" => "helmet",
            "crown" | "circlet" => "^",
            "hood" | "wizard" | "mitre" => "hat",
            _ => "cap",
        },
        "chest" => "[",
        "legs" => "legs",
        "belt" => "belt",
        "back" => "cloak",
        "ring" => "=",
        "quest" => "relic",
        _ => return glyph.to_string(),
    };
    s.to_string()
}

/// Paints an item picture (14x14, not outlined); None for unknown shapes.
pub fn item_icon(shape: &str, c: Rgba) -> Option<Pc> {
    let mut p = Pc::new(14, 14, &format!("icon{shape}"));
    match shape {
        "crossbow" => {
            p.rect(1, 6, 12, 2, C_WOOD);
            p.rect(1, 7, 3, 2, mul(C_WOOD, 0.8));
            for y in 1..=12 {
                let x = if y <= 2 || y >= 11 { 9 } else { 10 };
                p.set(x, y, mul(c, 1.1 - 0.03 * y as f64));
                p.set(x + 1, y, mul(c, 0.75));
            }
            p.line(9, 1, 5, 6, alpha(C_WHITE, 160));
            p.line(5, 7, 9, 12, alpha(C_WHITE, 160));
            p.set(12, 6, C_STEEL);
        }
        "shield" => {
            for y in 1..13 {
                let half = if y > 8 { 5 - (y - 8) * 5 / 4 } else { 5 };
                for x in 7 - half..7 + half {
                    let mut k = if x >= 7 + half - 1 { 0.72 } else { 1.0 };
                    if y == 1 || x == 7 - half {
                        k = 1.15;
                    }
                    p.set(x, y, mul(c, k));
                }
            }
            p.rect(6, 3, 2, 7, C_GOLD);
            p.rect(4, 5, 6, 2, C_GOLD);
        }
        "book" => {
            p.rect(2, 2, 10, 10, mul(c, 0.9));
            p.rect(3, 3, 8, 8, c);
            p.rect(10, 3, 2, 9, hex("#f0e8d0"));
            p.rect(5, 5, 3, 3, C_GOLD);
        }
        "orb" => {
            p.ball(7.0, 6.5, 4.6, c, 0.0);
            p.set(5, 4, C_WHITE);
            p.set(6, 4, alpha(C_WHITE, 180));
            p.rect(4, 11, 6, 2, C_WOOD);
        }
        "symbol" => {
            p.rect(6, 1, 2, 12, c);
            p.rect(3, 4, 8, 2, c);
            p.rect(7, 1, 1, 12, mul(c, 0.75));
            p.set(6, 1, C_WHITE);
        }
        "helmet" => {
            for y in 2..11 {
                for x in 2..12 {
                    let (dx, dy) = (x as f64 - 6.5, y as f64 - 7.0);
                    if dx * dx / 25.0 + dy * dy / 25.0 > 1.0 {
                        continue;
                    }
                    p.set(x, y, mul(c, 1.1 - 0.06 * (x - 2) as f64));
                }
            }
            p.rect(3, 7, 8, 1, C_DARK);
            p.rect(6, 7, 2, 4, mul(c, 0.7));
        }
        "hat" => {
            for y in 1..10 {
                let half = 1 + y / 2;
                for x in 7 - half..7 + half {
                    p.set(x, y, mul(c, 1.1 - 0.08 * (x - (7 - half)) as f64));
                }
            }
            p.rect(1, 10, 12, 2, mul(c, 0.8));
        }
        "cap" => {
            p.blob(7.0, 7.0, 5.0, 3.6, c);
            p.rect(2, 9, 10, 2, mul(c, 0.75));
            p.set(7, 3, C_WHITE);
        }
        "legs" => {
            p.rect(3, 1, 8, 3, mul(c, 1.05));
            p.rect(3, 4, 3, 8, c);
            p.rect(8, 4, 3, 8, mul(c, 0.82));
            p.rect(2, 11, 4, 2, mul(C_LEATHER, 0.8));
            p.rect(8, 11, 4, 2, mul(C_LEATHER, 0.7));
        }
        "belt" => {
            p.rect(1, 5, 12, 4, c);
            p.rect(1, 5, 12, 1, mix(c, C_WHITE, 0.3));
            p.rect(5, 4, 4, 6, C_GOLD);
            p.rect(6, 5, 2, 4, mul(c, 0.6));
        }
        "cloak" => {
            for y in 1..13 {
                let half = 3 + y / 3;
                for x in 7 - half..7 + half {
                    let k = if (x + y) % 4 == 0 { 0.85 } else { 1.0 };
                    p.set(x, y, mul(c, k));
                }
            }
            p.rect(5, 1, 4, 2, C_GOLD);
        }
        "relic" => {
            for y in 1..13 {
                let half = 5 - (y - 6i32).abs() * 5 / 6;
                for x in 7 - half..=7 + half {
                    p.set(x, y, mul(c, 1.15 - 0.07 * (x - (7 - half)) as f64));
                }
            }
            p.set(6, 4, C_WHITE);
            p.set(5, 5, alpha(C_WHITE, 180));
        }
        "!" => {
            p.rect(6, 1, 2, 2, C_WOOD);
            p.rect(6, 3, 2, 2, alpha(C_WHITE, 200));
            p.blob(7.0, 9.0, 4.6, 4.2, mix(c, C_WHITE, 0.15));
            p.set(5, 7, C_WHITE);
            p.set(5, 8, alpha(C_WHITE, 180));
        }
        "?" => {
            let paper = hex("#e8dcb0");
            p.rect(3, 3, 8, 8, paper);
            p.rect(2, 2, 10, 2, mul(paper, 0.8));
            p.rect(2, 10, 10, 2, mul(paper, 0.8));
            for y in 5..=8 {
                p.rect(4, y, 6 - (y % 2) * 2, 1, mul(c, 0.7));
            }
            p.set(11, 11, hex("#c03030"));
        }
        "%" => {
            p.blob(7.0, 8.0, 5.0, 3.6, c);
            p.blob(5.5, 7.0, 2.0, 1.4, mix(c, C_WHITE, 0.35));
        }
        "|" => {
            for i in 0..8 {
                p.set(3 + i, 10 - i, mul(c, 1.1));
                p.set(4 + i, 10 - i, mul(c, 0.75));
            }
            p.thick(1.0, 9.0, 5.0, 13.0, 1.0, C_GOLD);
            p.set(2, 12, C_WOOD);
            p.set(1, 13, C_WOOD);
        }
        "/" => {
            p.thick(2.0, 13.0, 10.0, 3.0, 1.2, C_WOOD);
            p.blob(11.0, 2.5, 2.0, 2.0, c);
            p.set(10, 2, C_WHITE);
        }
        ")" => {
            for y in 1..=12 {
                let x = if y <= 2 || y >= 11 {
                    7
                } else if y <= 4 || y >= 9 {
                    5
                } else {
                    4
                };
                p.set(x, y, c);
                p.set(8, y, alpha(C_WHITE, 170));
            }
        }
        "P" => {
            p.thick(3.0, 13.0, 9.0, 3.0, 1.2, C_WOOD);
            p.blob(10.0, 4.0, 3.0, 3.0, mul(c, 1.05));
            p.set(9, 2, C_WHITE);
        }
        "T" => {
            p.thick(3.0, 13.0, 8.0, 5.0, 1.2, C_WOOD);
            p.rect(6, 1, 6, 5, c);
            p.rect(6, 1, 6, 1, mix(c, C_WHITE, 0.4));
            p.rect(11, 1, 1, 5, mul(c, 0.7));
        }
        "[" => {
            p.rect(3, 3, 8, 9, c);
            p.rect(1, 3, 3, 4, mul(c, 1.1));
            p.rect(10, 3, 3, 4, mul(c, 0.8));
            for (x, y) in [(6, 3), (7, 3), (6, 4), (7, 4)] {
                p.clear(x, y);
            }
            p.rect(4, 4, 1, 6, mix(c, C_WHITE, 0.4));
            p.rect(3, 9, 8, 1, mul(c, 0.6));
        }
        "=" => {
            for y in 0..14 {
                for x in 0..14 {
                    let (dx, dy) = (x as f64 - 6.5, y as f64 - 7.5);
                    let d = dx * dx + dy * dy;
                    if d < 16.0 && d > 6.0 {
                        p.set(x, y, mul(C_GOLD, 1.1 - 0.05 * dx));
                    }
                }
            }
            p.blob(6.5, 3.5, 1.8, 1.6, c);
        }
        "\"" => {
            for i in 0..5 {
                p.set(2 + i, 2 + i, mul(C_GOLD, 0.85));
                p.set(11 - i, 2 + i, mul(C_GOLD, 0.7));
            }
            p.blob(6.5, 9.5, 3.0, 3.0, c);
            p.set(5, 8, C_WHITE);
        }
        "^" => {
            p.rect(2, 7, 10, 4, C_GOLD);
            let mut x = 2;
            while x <= 11 {
                p.rect(x, 4, 1, 3, C_GOLD);
                x += 3;
            }
            p.blob(6.5, 8.5, 1.5, 1.4, c);
        }
        "$" => {
            for (x, y) in [(4.0, 9.0), (9.0, 9.0), (6.5, 6.5), (6.5, 10.5)] {
                p.blob(x, y, 2.6, 2.0, C_GOLD);
                p.set(x as i32 - 1, y as i32 - 1, hex("#fff2a0"));
            }
        }
        _ => return None,
    }
    Some(p)
}
