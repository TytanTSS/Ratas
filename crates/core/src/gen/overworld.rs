use super::*;
use crate::content::db;
use crate::world::{dir_towards, find_path, Level, Pos, DIRS4};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn center(&self) -> Pos {
        Pos::new(self.x + self.w / 2, self.y + self.h / 2)
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
    pub fn overlaps(&self, o: &Rect, gap: i32) -> bool {
        self.x - gap < o.x + o.w && o.x - gap < self.x + self.w && self.y - gap < o.y + o.h && o.y - gap < self.y + self.h
    }
}

#[derive(Clone, Debug, Default)]
pub struct NpcSpawn {
    pub role: String,
    pub name: String,
    pub pos: Pos,
    /// where to go at night (cities); zero = stay
    pub night: Pos,
}

#[derive(Clone, Debug, Default)]
pub struct Village {
    pub name: String,
    pub center: Pos,
    pub area: Rect,
    pub houses: Vec<Rect>,
    pub npcs: Vec<NpcSpawn>,
    /// a big walled stone city
    pub city: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Entrance {
    pub pos: Pos,
    pub theme: String,
    pub name: String,
    pub max_depth: i32,
}

pub struct Overworld {
    pub name: String,
    pub level: Level,
    pub villages: Vec<Village>,
    pub entrances: Vec<Entrance>,
    pub start: Pos,
    pub regions: Vec<Region>,
    /// 1-based index into regions per cell, 0 = none
    pub region_map: Vec<u8>,
    pub landmarks: Vec<Landmark>,
}

pub fn t(key: &str) -> u8 {
    db().tile_id(key)
}

/// Builds the surface map.
///
/// 1. Elevation = fractal Perlin noise minus a radial falloff (island shape);
///    moisture and temperature (north is cold, south is hot, highlands are
///    cooler) are independent noise fields, classified by quantiles so the
///    share of water, deserts or tundra is stable for any seed.
/// 2. Biomes come from (elevation, moisture, temperature); local detail noise
///    adds tree density, dunes, snowdrifts and reeds.
/// 3. Rivers flow downhill from highlands to the sea (frozen in the north).
/// 4. Cities and villages are placed on the largest walkable landmass.
/// 5. Far from people the land is scarred: a volcanic waste and cursed lands.
/// 6. Settlements are joined by roads found with weighted A* (bridges over water).
/// 7. Connected areas of one biome become named regions; landmarks are stamped
///    on free land; dungeon entrances are scattered, the deeper ones further
///    from the start.
pub fn generate_overworld(seed: i64, w: i32, h: i32) -> Overworld {
    let mut r = Rng::labeled(seed, "overworld");
    let name = world_name(&mut r);
    let mut l = Level::new("overworld", &name, w, h, t("grass"));
    l.lit = true;
    l.theme = "overworld".into();

    let elev_n = Perlin::new(&mut r);
    let moist_n = Perlin::new(&mut r);
    let detail_n = Perlin::new(&mut r);
    let temp_n = Perlin::new(&mut r);
    let n = (w * h) as usize;
    let mut e_ = vec![0.0; n];
    let mut m_ = vec![0.0; n];
    let mut d_ = vec![0.0; n];
    let mut tm = vec![0.0; n];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let (fx, fy) = (x as f64 / 48.0, y as f64 / 48.0);
            let e = (elev_n.fbm(fx, fy, 6) + 1.0) / 2.0;
            let (nx, ny) = (x as f64 / w as f64 * 2.0 - 1.0, y as f64 / h as f64 * 2.0 - 1.0);
            let d = 1.0 - (1.0 - nx * nx) * (1.0 - ny * ny);
            e_[i] = e - 0.55 * d * d;
            m_[i] = (moist_n.fbm(fx * 0.8 + 100.0, fy * 0.8 + 100.0, 4) + 1.0) / 2.0;
            d_[i] = detail_n.fbm(x as f64 / 6.0, y as f64 / 6.0, 2);
            tm[i] = y as f64 / h as f64 * 1.1 + temp_n.fbm(fx * 0.6 + 50.0, fy * 0.6 + 50.0, 3) * 0.55;
        }
    }
    let eq = quantiles(&e_);
    let mq = quantiles(&m_);
    let tq = quantiles(&tm);
    let e75 = eq(0.75);
    for i in 0..n {
        tm[i] -= (e_[i] - e75).max(0.0) * 1.5;
    }

    let mut cold = vec![false; n];
    for i in 0..n {
        let (e, m, det) = (e_[i], m_[i], d_[i]);
        let is_cold = tm[i] < tq(0.15);
        let is_hot = tm[i] > tq(0.8) && m < mq(0.62);
        cold[i] = is_cold;
        let key = if e < eq(0.20) {
            "deep_water"
        } else if e < eq(0.28) {
            if is_cold {
                "ice"
            } else {
                "water"
            }
        } else if e < eq(0.31) {
            if is_cold {
                "snow_ground"
            } else if is_hot {
                "desert_sand"
            } else {
                "sand"
            }
        } else if e > eq(0.985) {
            "snow"
        } else if e > eq(0.945) || (e > eq(0.87) && det > 0.25) {
            if is_cold {
                if r.int_n(3) == 0 {
                    "snow"
                } else {
                    "ice_rock"
                }
            } else if is_hot {
                "sandstone"
            } else {
                "mountain"
            }
        } else if e > eq(0.87) {
            if is_cold {
                "snowdrift"
            } else if is_hot {
                "dunes"
            } else {
                "hill"
            }
        } else if is_cold {
            let v = r.f64();
            if v < 0.3 + det * 0.4 && m > mq(0.4) {
                "snow_pine"
            } else if v < 0.012 {
                "ice_rock"
            } else if det > 0.2 {
                "snowdrift"
            } else {
                "snow_ground"
            }
        } else if is_hot {
            let v = r.f64();
            if v < 0.018 {
                "cactus"
            } else if v < 0.026 {
                "sandstone"
            } else if v < 0.03 {
                "dead_tree"
            } else if det > 0.12 {
                "dunes"
            } else {
                "desert_sand"
            }
        } else if m > mq(0.86) && e < eq(0.6) {
            let v = r.f64();
            if v < 0.08 {
                "water"
            } else if v < 0.12 {
                "dead_tree"
            } else if v < 0.32 || det > 0.2 {
                "reeds"
            } else {
                "swamp"
            }
        } else if m > mq(0.52) {
            let density = 0.38 + det * 0.5;
            let v = r.f64();
            if v < density {
                if e > eq(0.7) || r.int_n(3) == 0 {
                    "pine"
                } else {
                    "tree"
                }
            } else if v < density + 0.07 {
                "bush"
            } else {
                "forest_floor"
            }
        } else {
            let v = r.f64();
            if v < 0.012 {
                "flowers"
            } else if v < 0.03 {
                "tree"
            } else if det > 0.18 {
                "tall_grass"
            } else if v < 0.5 {
                "grass2"
            } else {
                "grass"
            }
        };
        l.tiles[i] = t(key);
    }

    carve_rivers(&mut r, &mut l, &e_, &eq);
    // rivers freeze in the north
    let (water, ice) = (t("water"), t("ice"));
    for i in 0..n {
        if l.tiles[i] == water && cold[i] {
            l.tiles[i] = ice;
        }
    }

    let mut main = largest_region(&l);
    let mut names = HashSet::new();
    let want = 1 + r.int_n(2);
    let cities = place_cities(&mut r, &mut l, &main, want, &mut names);
    // villages first: the start is in the first one
    let mut villages = place_villages(&mut r, &mut l, &main, &cities, &mut names);
    villages.extend(cities);
    let start = villages.first().map(|v| v.center).unwrap_or_default();
    scar_land(&mut r, &mut l, &main, &villages, start);
    build_roads(&mut l, &villages);
    let (regions, region_map) = name_regions(&mut r, &l);
    main = largest_region(&l);
    let landmarks = place_landmarks(&mut r, &mut l, &main, &villages, start);
    main = largest_region(&l);
    let entrances = place_entrances(&mut r, &mut l, &main, &villages);

    let start = if let Some(v) = villages.first() {
        find_free(&l, Pos::new(v.center.x, v.center.y + 2))
    } else {
        let i = main.iter().position(|&b| b).unwrap_or(0) as i32;
        Pos::new(i % w, i / w)
    };
    Overworld { name, level: l, villages, entrances, start, regions, region_map, landmarks }
}

/// A function mapping q in [0,1] to the value at that quantile.
fn quantiles(v: &[f64]) -> impl Fn(f64) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    move |q: f64| {
        let i = (q * (s.len() - 1) as f64) as usize;
        s[i.min(s.len() - 1)]
    }
}

fn carve_rivers(r: &mut Rng, l: &mut Level, e_: &[f64], eq: &dyn Fn(f64) -> f64) {
    let (water, deep) = (t("water"), t("deep_water"));
    let (lo, hi) = (eq(0.75), eq(0.93));
    let mut rivers = 0;
    let mut attempt = 0;
    while attempt < 400 && rivers < 7 {
        attempt += 1;
        let (mut x, mut y) = (r.int_n(l.w), r.int_n(l.h));
        let e = e_[(y * l.w + x) as usize];
        if e < lo || e > hi {
            continue;
        }
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        let mut reached = false;
        for steps in 0..900 {
            let i = (y * l.w + x) as usize;
            visited.insert(i);
            path.push(Pos::new(x, y));
            let tt = l.tiles[i];
            if steps > 0 && (tt == water || tt == deep) {
                reached = true;
                break;
            }
            let (mut best, mut bx, mut by) = (f64::INFINITY, -1, -1);
            for d in DIRS4 {
                let (nx, ny) = (x + d.x, y + d.y);
                if !l.inside(nx, ny) || visited.contains(&((ny * l.w + nx) as usize)) {
                    continue;
                }
                let v = e_[(ny * l.w + nx) as usize] + r.f64() * 0.004;
                if v < best {
                    best = v;
                    bx = nx;
                    by = ny;
                }
            }
            if bx < 0 {
                break;
            }
            x = bx;
            y = by;
        }
        if !reached || path.len() < 25 {
            continue;
        }
        let half = path.len() / 2;
        for (k, p) in path.iter().enumerate() {
            l.set(p.x, p.y, water);
            // widen the lower course of the river
            if k > half && k % 2 == 0 {
                l.set(p.x + 1, p.y, water);
            }
        }
        rivers += 1;
    }
}

/// Flood-fills walkable cells and returns a mask of the largest component.
pub fn largest_region(l: &Level) -> Vec<bool> {
    let n = (l.w * l.h) as usize;
    let mut region = vec![-1i32; n];
    let (mut best_id, mut best_size) = (-1, 0);
    let mut id = 0;
    let mut stack = Vec::with_capacity(1024);
    let walk: Vec<bool> = db().b.tiles.iter().map(|t| t.walkable).collect();
    for i in 0..n {
        if region[i] >= 0 || !walk[l.tiles[i] as usize] {
            continue;
        }
        let mut size = 0;
        stack.clear();
        stack.push(i);
        region[i] = id;
        while let Some(c) = stack.pop() {
            size += 1;
            let (cx, cy) = ((c as i32) % l.w, (c as i32) / l.w);
            for d in DIRS4 {
                let (nx, ny) = (cx + d.x, cy + d.y);
                if !l.inside(nx, ny) {
                    continue;
                }
                let ni = (ny * l.w + nx) as usize;
                if region[ni] < 0 && walk[l.tiles[ni] as usize] {
                    region[ni] = id;
                    stack.push(ni);
                }
            }
        }
        if size > best_size {
            best_id = id;
            best_size = size;
        }
        id += 1;
    }
    region.iter().map(|&r| r == best_id).collect()
}

pub fn find_free(l: &Level, p: Pos) -> Pos {
    for rad in 0..30i32 {
        for dy in -rad..=rad {
            for dx in -rad..=rad {
                if dx.abs().max(dy.abs()) != rad {
                    continue;
                }
                if l.walkable(p.x + dx, p.y + dy) {
                    return Pos::new(p.x + dx, p.y + dy);
                }
            }
        }
    }
    p
}

pub fn is_ground(tile: u8) -> bool {
    let def = db().tile(tile);
    matches!(def.biome.as_str(), "plains" | "forest" | "sand" | "hills" | "swamp" | "desert" | "tundra" | "ash" | "cursed")
        && def.walkable
        && def.damage == 0.0
}

const VILLAGE_W: i32 = 26;
const VILLAGE_H: i32 = 18;

fn place_villages(r: &mut Rng, l: &mut Level, main: &[bool], cities: &[Village], used: &mut HashSet<String>) -> Vec<Village> {
    let mut vs: Vec<Village> = Vec::new();
    let want = 4 + r.int_n(2) as usize;
    for _ in 0..4000 {
        if vs.len() >= want {
            break;
        }
        let cx = 20 + r.int_n(l.w - 40);
        let cy = 16 + r.int_n(l.h - 32);
        if !main[(cy * l.w + cx) as usize] {
            continue;
        }
        let area = Rect::new(cx - VILLAGE_W / 2, cy - VILLAGE_H / 2, VILLAGE_W, VILLAGE_H);
        // mostly dry flat land
        let (mut good, mut total) = (0, 0);
        for y in area.y..area.y + area.h {
            for x in area.x..area.x + area.w {
                total += 1;
                let def = l.def(x, y);
                if matches!(def.biome.as_str(), "plains" | "forest" | "sand") {
                    good += 1;
                }
            }
        }
        if good * 100 / total < 85 {
            continue;
        }
        let mut far = vs.iter().all(|v| !((v.center.x - cx).abs() < 50 && (v.center.y - cy).abs() < 40));
        if cities.iter().any(|c| c.area.overlaps(&area, 12)) {
            far = false;
        }
        if !far {
            continue;
        }
        let name = pick_unique(r, VILLAGE_NAMES, used);
        vs.push(build_village(r, l, area, name));
    }
    vs
}

fn build_village(r: &mut Rng, l: &mut Level, area: Rect, name: String) -> Village {
    let mut v = Village { name, center: area.center(), area, ..Default::default() };
    // clear an ellipse of grass
    for y in area.y..area.y + area.h {
        for x in area.x..area.x + area.w {
            let dx = (x - v.center.x) as f64 / (area.w / 2) as f64;
            let dy = (y - v.center.y) as f64 / (area.h / 2) as f64;
            if dx * dx + dy * dy <= 1.05 {
                l.set(x, y, if r.int_n(2) == 0 { t("grass") } else { t("grass2") });
            }
        }
    }
    let c = v.center;
    l.set(c.x, c.y, t("well"));
    let plaza = Rect::new(c.x - 4, c.y - 3, 9, 7);
    let nh = 4 + r.int_n(3) as usize;
    for _ in 0..300 {
        if v.houses.len() >= nh {
            break;
        }
        let (hw, hh) = (r.range(6, 8), r.range(5, 7));
        let hx = r.range(area.x + 1, area.x + area.w - hw - 1);
        let hy = r.range(area.y + 1, area.y + area.h - hh - 1);
        let hr = Rect::new(hx, hy, hw, hh);
        if hr.overlaps(&plaza, 1) || v.houses.iter().any(|o| hr.overlaps(o, 2)) {
            continue;
        }
        for y in hy..hy + hh {
            for x in hx..hx + hw {
                let wall = x == hx || y == hy || x == hx + hw - 1 || y == hy + hh - 1;
                l.set(x, y, if wall { t("house_wall") } else { t("house_floor") });
            }
        }
        // door on the wall facing the well
        let hc = hr.center();
        let d = dir_towards(hc, c);
        let door = match (d.x, d.y) {
            (0, -1) => Pos::new(hc.x, hy),
            (0, 1) => Pos::new(hc.x, hy + hh - 1),
            (-1, 0) => Pos::new(hx, hc.y),
            _ => Pos::new(hx + hw - 1, hc.y),
        };
        l.set(door.x, door.y, t("door"));
        v.houses.push(hr);
    }

    // population
    let houses = v.houses.clone();
    let mut house_spot = 0;
    for role in &db().b.npcs {
        if role.world {
            continue; // wanderers live in the open world
        }
        let cnt = r.range(role.count[0], role.count[1]);
        for _ in 0..cnt {
            let p = match role.key.as_str() {
                "elder" | "merchant" | "smith" | "healer" | "priest" if house_spot < houses.len() => {
                    house_spot += 1;
                    houses[house_spot - 1].center()
                }
                _ => village_free(r, l, &area, c),
            };
            v.npcs.push(NpcSpawn { role: role.key.clone(), name: person_name(r), pos: p, night: Pos::default() });
        }
    }
    v
}

fn village_free(r: &mut Rng, l: &Level, area: &Rect, c: Pos) -> Pos {
    for _ in 0..200 {
        let x = r.range(area.x + 2, area.x + area.w - 3);
        let y = r.range(area.y + 2, area.y + area.h - 3);
        if l.walkable(x, y) && l.def(x, y).key != "house_floor" {
            return Pos::new(x, y);
        }
    }
    Pos::new(c.x + 1, c.y + 1)
}

/// The tiles of settlements that roads go around.
pub fn built_up(key: &str) -> bool {
    matches!(
        key,
        "well" | "house_wall" | "door" | "house_floor" | "city_wall" | "stone_wall" | "stone_floor" | "carpet" | "fountain"
            | "market_stall" | "lamp_post" | "statue" | "altar" | "brazier" | "crystal" | "chest_open"
    )
}

fn build_roads(l: &mut Level, vs: &[Village]) {
    if vs.len() < 2 {
        return;
    }
    let (road, bridge) = (t("road"), t("bridge"));
    let (water, deep) = (t("water"), t("deep_water"));
    let mut connected = vec![0usize];
    for i in 1..vs.len() {
        let mut best = 0;
        let mut bd = i32::MAX;
        for &j in &connected {
            let d = vs[i].center.manhattan(vs[j].center);
            if d < bd {
                best = j;
                bd = d;
            }
        }
        let from = Pos::new(vs[i].center.x, vs[i].center.y + 1);
        let to = Pos::new(vs[best].center.x, vs[best].center.y + 1);
        let path = {
            let lv: &Level = l;
            let mut cost = |x: i32, y: i32| -> f64 {
                let tt = lv.at(x, y);
                let def = db().tile(tt);
                if tt == road || tt == bridge {
                    0.3
                } else if tt == deep {
                    25.0
                } else if tt == water {
                    6.0
                } else if def.key == "snow" {
                    -1.0
                } else if def.key == "mountain" {
                    40.0
                } else if def.key == "city_gate" || def.key == "cobblestone" {
                    0.3
                } else if built_up(&def.key) {
                    -1.0
                } else if !def.walkable {
                    4.0
                } else {
                    def.move_cost
                }
            };
            find_path(lv.w, lv.h, from, to, (lv.w * lv.h) as usize, false, &mut cost)
        };
        for p in path {
            let tt = l.at(p.x, p.y);
            let key = &db().tile(tt).key;
            if tt == water || tt == deep {
                l.set(p.x, p.y, bridge);
            } else if built_up(key) || key == "city_gate" || key == "cobblestone" || key == "garden" {
            } else {
                l.set(p.x, p.y, road);
            }
        }
        connected.push(i);
    }
}

fn place_entrances(r: &mut Rng, l: &mut Level, main: &[bool], vs: &[Village]) -> Vec<Entrance> {
    let start = vs.first().map(|v| v.center).unwrap_or_default();
    let mut es: Vec<Entrance> = Vec::new();
    let mut used = HashSet::new();
    let themes = ["cave", "crypt", "cave", "crypt", "crypt", "cave"];
    for attempt in 0..6000 {
        if es.len() >= themes.len() {
            break;
        }
        let (x, y) = (4 + r.int_n(l.w - 8), 4 + r.int_n(l.h - 8));
        if !main[(y * l.w + x) as usize] || !is_ground(l.at(x, y)) {
            continue;
        }
        let p = Pos::new(x, y);
        if (p.x - start.x).abs() < 25 && (p.y - start.y).abs() < 20 {
            continue;
        }
        let mut ok = !vs.iter().any(|v| v.area.overlaps(&Rect::new(x - 3, y - 3, 7, 7), 2));
        if es.iter().any(|e| (e.pos.x - x).abs() < 30 && (e.pos.y - y).abs() < 25) {
            ok = false;
        }
        if !ok {
            continue;
        }
        let theme = themes[es.len()];
        // caves prefer hills, crypts prefer anything but hills
        if theme == "cave" && l.def(x, y).biome != "hills" && attempt < 3000 {
            continue;
        }
        let name = pick_unique(r, dungeon_names(theme), &mut used);
        es.push(Entrance { pos: p, theme: theme.into(), name, max_depth: 0 });
    }
    // deeper dungeons further from the start
    es.sort_by_key(|e| e.pos.dist_sq(start));
    for (i, e) in es.iter_mut().enumerate() {
        e.max_depth = 2 + i as i32 / 2 + (i as i32).min(1);
    }
    // every special region hides its own dungeon deep inside
    for (theme, biome) in REGIONAL_DUNGEONS {
        for attempt in 0..5000 {
            let (x, y) = (4 + r.int_n(l.w - 8), 4 + r.int_n(l.h - 8));
            if !main[(y * l.w + x) as usize] || !is_ground(l.at(x, y)) || l.def(x, y).biome != *biome {
                continue;
            }
            let mut inside = 0;
            for dy in -6..=6 {
                for dx in -6..=6 {
                    if l.def(x + dx, y + dy).biome == *biome {
                        inside += 1;
                    }
                }
            }
            if inside < 13 * 13 * 3 / 4 && attempt < 4000 {
                continue;
            }
            if es.iter().any(|e| (e.pos.x - x).abs() < 16 && (e.pos.y - y).abs() < 12) {
                continue;
            }
            let name = pick_unique(r, dungeon_names(theme), &mut used);
            es.push(Entrance { pos: Pos::new(x, y), theme: theme.to_string(), name, max_depth: 4 });
            break;
        }
    }
    for e in &es {
        let p = e.pos;
        for dy in -1..=1 {
            for dx in -2..=2 {
                if l.inside(p.x + dx, p.y + dy) && !l.walkable(p.x + dx, p.y + dy) {
                    l.set(p.x + dx, p.y + dy, t("rubble"));
                }
            }
        }
        l.set(p.x, p.y, t("dungeon"));
    }
    es
}
