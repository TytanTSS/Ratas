use super::*;
use crate::content::db;
use crate::world::{dir_towards, Level, Pos};
use std::collections::{HashMap, HashSet};

/// City size in cells.
pub const CITY_W: i32 = 50;
pub const CITY_H: i32 = 40;

/// Spreads walled cities over the main landmass, about one or two per
/// classic world of land: any dry land will do (the city is paved over), with
/// little water, mountains or wasteland.
pub(super) fn place_cities(
    r: &mut Rng,
    l: &mut Level,
    main: &[bool],
    land: usize,
    used: &mut HashSet<String>,
) -> Vec<Village> {
    let want = per_land(land, 1.0 + r.f64(), 1);
    let (w, h) = (l.w, l.h);
    // how bad each cell is for a city
    let bad_t = tile_table(|def| {
        if matches!(def.biome.as_str(), "ash" | "cursed") {
            3
        } else if def.biome == "water"
            || def.biome == "snow"
            || def.key == "mountain"
            || def.damage > 0.0
        {
            1
        } else {
            0
        }
    });
    let walk_t = tile_table(|def| def.walkable);
    let bad = Sat::new(l, |i| {
        let tt = l.tiles[i] as usize;
        if !main[i] && !walk_t[tt] {
            2
        } else {
            bad_t[tt]
        }
    });
    let mut cs: Vec<Village> = Vec::new();
    let mut near = Buckets::new(128);
    let step = ((w as f64 * h as f64 / (want as f64 * 2.5)).sqrt() as i32).max(CITY_W + 30);
    let cells = scatter_cells(r, w, h, step);
    let tries = (3000 / cells.len().max(1)).max(8);
    for max_bad in [6u32, 14, 25] {
        for c in &cells {
            if cs.len() >= want {
                break;
            }
            for _ in 0..tries {
                let p = in_cell(r, c, w, h, 0);
                let cx = p.x.clamp(CITY_W / 2 + 6, w - CITY_W / 2 - 7);
                let cy = p.y.clamp(CITY_H / 2 + 6, h - CITY_H / 2 - 7);
                let area = Rect::new(cx - CITY_W / 2, cy - CITY_H / 2, CITY_W, CITY_H);
                let (x0, y0) = (area.x - 2, area.y - 2);
                let (x1, y1) = (area.x + area.w + 2, area.y + area.h + 2);
                let total = ((x1 - x0) * (y1 - y0)) as u32;
                if bad.sum(x0, y0, x1, y1) * 100 > max_bad * total {
                    continue;
                }
                if near
                    .near(area.center(), CITY_W + 40)
                    .into_iter()
                    .any(|i| cs[i].area.overlaps(&area, 30))
                {
                    continue;
                }
                let name = city_name(r, used);
                near.insert(area.center(), cs.len());
                cs.push(build_city(r, l, area, name));
                break;
            }
        }
        if !cs.is_empty() && cs.len() * 2 >= want {
            break;
        }
    }
    cs
}

/// A house of a city with a purpose ("" for a plain home).
#[derive(Clone)]
struct Building {
    rect: Rect,
    kind: &'static str,
    door: Pos,
}

/// City buildings with a purpose, the most important first (they get the
/// largest houses); NPC roles name the one they work in.
const CITY_BUILDINGS: &[&str] = &[
    "temple", "townhall", "tavern", "armory", "smithy", "magic", "alchemy", "jewelry", "barracks",
];

/// Stamps a walled stone city: gates on every side, two main streets crossing
/// at a plaza with a fountain and a market, stone houses in four quarters,
/// lamps along the streets.
fn build_city(r: &mut Rng, l: &mut Level, area: Rect, name: String) -> Village {
    let mut v = Village {
        name,
        center: area.center(),
        area,
        city: true,
        ..Default::default()
    };
    let c = v.center;
    let (x0, y0, x1, y1) = (area.x, area.y, area.x + area.w - 1, area.y + area.h - 1);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let wall = x == x0 || x == x1 || y == y0 || y == y1;
            l.set(
                x,
                y,
                if wall {
                    t("city_wall")
                } else {
                    t("cobblestone")
                },
            );
        }
    }
    // a ring of grass outside the walls so the gates are never blocked
    for y in y0 - 2..=y1 + 2 {
        for x in x0 - 2..=x1 + 2 {
            if l.inside(x, y) && !area.contains(x, y) && !l.walkable(x, y) {
                l.set(x, y, t("grass"));
            }
        }
    }
    // corner towers
    for p in [
        Pos::new(x0, y0),
        Pos::new(x1 - 1, y0),
        Pos::new(x0, y1 - 1),
        Pos::new(x1 - 1, y1 - 1),
    ] {
        for dy in 0..2 {
            for dx in 0..2 {
                l.set(p.x + dx, p.y + dy, t("city_wall"));
            }
        }
    }
    // gates
    for d in -1..=1 {
        l.set(c.x + d, y0, t("city_gate"));
        l.set(c.x + d, y1, t("city_gate"));
        l.set(x0, c.y + d, t("city_gate"));
        l.set(x1, c.y + d, t("city_gate"));
    }

    // free ground: streets and the plaza stay open
    let street_h = Rect::new(x0 + 1, c.y - 1, area.w - 2, 3);
    let street_v = Rect::new(c.x - 1, y0 + 1, 3, area.h - 2);
    let plaza = Rect::new(c.x - 7, c.y - 5, 15, 11);
    let reserved = [street_h, street_v, plaza];

    // fountain, statues, lamps and market stalls on the plaza
    for dx in -1..=1 {
        l.set(c.x + dx, c.y, t("fountain"));
    }
    for p in [
        Pos::new(plaza.x, plaza.y),
        Pos::new(plaza.x + plaza.w - 1, plaza.y),
        Pos::new(plaza.x, plaza.y + plaza.h - 1),
        Pos::new(plaza.x + plaza.w - 1, plaza.y + plaza.h - 1),
    ] {
        l.set(p.x, p.y, t("lamp_post"));
    }
    l.set(plaza.x + 1, c.y - 4, t("statue"));
    l.set(plaza.x + plaza.w - 2, c.y + 4, t("statue"));
    for row in [c.y - 3, c.y + 3] {
        let mut x = plaza.x + 3;
        while x < plaza.x + plaza.w - 3 {
            if !(x >= street_v.x - 1 && x <= street_v.x + street_v.w) {
                l.set(x, row, t("market_stall"));
            }
            x += 3;
        }
    }
    // lamps along the main streets, just off the pavement
    let mut x = x0 + 4;
    while x < x1 - 3 {
        if !plaza.contains(x, c.y - 2) {
            l.set(x, c.y - 2, t("lamp_post"));
            l.set(x, c.y + 2, t("lamp_post"));
        }
        x += 8;
    }
    let mut y = y0 + 4;
    while y < y1 - 3 {
        if !plaza.contains(c.x - 2, y) && !street_h.contains(c.x - 2, y) {
            l.set(c.x - 2, y, t("lamp_post"));
            l.set(c.x + 2, y, t("lamp_post"));
        }
        y += 8;
    }

    // houses in the four quarters
    let quarters = [
        Rect::new(x0 + 2, y0 + 2, street_v.x - x0 - 3, street_h.y - y0 - 3),
        Rect::new(
            street_v.x + street_v.w + 1,
            y0 + 2,
            x1 - street_v.x - street_v.w - 2,
            street_h.y - y0 - 3,
        ),
        Rect::new(
            x0 + 2,
            street_h.y + street_h.h + 1,
            street_v.x - x0 - 3,
            y1 - street_h.y - street_h.h - 2,
        ),
        Rect::new(
            street_v.x + street_v.w + 1,
            street_h.y + street_h.h + 1,
            x1 - street_v.x - street_v.w - 2,
            y1 - street_h.y - street_h.h - 2,
        ),
    ];
    // rows of houses with one-cell alleys between them; a few lots stay
    // empty as yards
    let mut houses: Vec<Rect> = Vec::new();
    for q in quarters {
        let mut y = q.y;
        while y + 5 <= q.y + q.h {
            let row_h = r.range(5, 6).min(q.y + q.h - y);
            let mut w = 0;
            let mut x = q.x;
            while x + 6 <= q.x + q.w {
                if w == 0 {
                    w = r.range(6, 9);
                }
                w = w.min(q.x + q.w - x);
                let hr = Rect::new(x, y, w, row_h);
                if reserved.iter().any(|o| hr.overlaps(o, 1)) {
                    // a narrower house, or slide past the plaza
                    if w > 6 {
                        w -= 1;
                    } else {
                        x += 1;
                    }
                    continue;
                }
                if r.int_n(12) > 0 {
                    houses.push(hr);
                }
                x += w + 1;
                w = 0;
            }
            y += row_h + 1;
        }
    }
    // the biggest houses become the important buildings
    houses.sort_by_key(|h| -(h.w * h.h));
    let bs: Vec<Building> = houses
        .iter()
        .enumerate()
        .map(|(i, h)| stamp_house(l, *h, CITY_BUILDINGS.get(i).copied().unwrap_or(""), c))
        .collect();
    // little gardens in the alleys
    for _ in 0..30 {
        let (x, y) = (r.range(x0 + 2, x1 - 2), r.range(y0 + 2, y1 - 2));
        if l.def(x, y).key != "cobblestone" {
            continue;
        }
        let free = !reserved.iter().any(|o| o.contains(x, y))
            && !bs.iter().any(|b| b.door.dist(Pos::new(x, y)) <= 2);
        if free {
            l.set(x, y, t("garden"));
        }
    }

    // people
    let by_kind: HashMap<&str, Building> = bs
        .iter()
        .filter(|b| !b.kind.is_empty())
        .map(|b| (b.kind, b.clone()))
        .collect();
    let inside = |l: &Level, b: &Building, k: i32| {
        let ctr = b.rect.center();
        let p = Pos::new(ctr.x - 1 + k % 3, ctr.y);
        if l.walkable(p.x, p.y) {
            p
        } else {
            ctr
        }
    };
    let street_spot = |r: &mut Rng, l: &Level, near: Rect| {
        for _ in 0..300 {
            let (x, y) = (
                r.range(near.x, near.x + near.w - 1),
                r.range(near.y, near.y + near.h - 1),
            );
            let key = &l.def(x, y).key;
            if l.walkable(x, y) && key != "stone_floor" && key != "carpet" && key != "house_floor" {
                return Pos::new(x, y);
            }
        }
        Pos::new(c.x, c.y + 2)
    };
    let gates = [
        Pos::new(c.x, y0 + 1),
        Pos::new(c.x, y1 - 1),
        Pos::new(x0 + 1, c.y),
        Pos::new(x1 - 1, c.y),
    ];
    let night = by_kind
        .get("tavern")
        .map_or(Pos::new(c.x, c.y + 2), |b| b.door);
    let mut guards = 0;
    for role in &db().b.npcs {
        let cnt = r.range(role.city_count[0], role.city_count[1]);
        let b = by_kind.get(role.building.as_str());
        for k in 0..cnt {
            let mut s = NpcSpawn {
                role: role.key.clone(),
                name: person_name(r),
                ..Default::default()
            };
            if let Some(b) = b {
                s.pos = inside(l, b, k);
            } else if !role.combat.is_empty() {
                // guards keep the gates, the rest walk the plaza
                s.pos = if guards < gates.len() {
                    gates[guards]
                } else {
                    street_spot(r, l, plaza)
                };
                guards += 1;
            } else if role.trader {
                s.pos = street_spot(r, l, plaza);
            } else {
                s.pos = street_spot(r, l, Rect::new(x0 + 2, y0 + 2, area.w - 4, area.h - 4));
                s.night = night;
            }
            v.npcs.push(s);
        }
    }
    v.houses = bs.iter().map(|b| b.rect).collect();
    v
}

/// Builds a stone house with its door toward the city center and furnishes
/// it by purpose.
fn stamp_house(l: &mut Level, h: Rect, kind: &'static str, c: Pos) -> Building {
    let floor = match kind {
        "tavern" | "" => t("house_floor"),
        "temple" | "townhall" | "magic" | "jewelry" => t("carpet"),
        _ => t("stone_floor"),
    };
    for y in h.y..h.y + h.h {
        for x in h.x..h.x + h.w {
            let wall = x == h.x || y == h.y || x == h.x + h.w - 1 || y == h.y + h.h - 1;
            l.set(x, y, if wall { t("stone_wall") } else { floor });
        }
    }
    let hc = h.center();
    let d = dir_towards(hc, c);
    let door = match (d.x, d.y) {
        (0, -1) => Pos::new(hc.x, h.y),
        (0, 1) => Pos::new(hc.x, h.y + h.h - 1),
        (-1, 0) => Pos::new(h.x, hc.y),
        _ => Pos::new(h.x + h.w - 1, hc.y),
    };
    l.set(door.x, door.y, t("door"));
    // furniture against the wall opposite the door
    let back = if door.y == h.y {
        Pos::new(hc.x, h.y + h.h - 2)
    } else {
        Pos::new(hc.x, h.y + 1)
    };
    match kind {
        "temple" => {
            l.set(back.x, back.y, t("altar"));
            l.set(h.x + 1, h.y + 1, t("brazier"));
            l.set(h.x + h.w - 2, h.y + 1, t("brazier"));
        }
        "smithy" => l.set(h.x + 1, back.y, t("brazier")),
        "magic" => l.set(back.x, back.y, t("crystal")),
        "jewelry" | "townhall" => l.set(h.x + 1, back.y, t("chest_open")),
        _ => {}
    }
    Building {
        rect: h,
        kind,
        door,
    }
}
