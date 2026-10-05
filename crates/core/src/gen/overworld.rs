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
        self.x - gap < o.x + o.w
            && o.x - gap < self.x + self.w
            && self.y - gap < o.y + o.h
            && o.y - gap < self.y + self.h
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
    pub region_map: Vec<u16>,
    pub landmarks: Vec<Landmark>,
}

pub fn t(key: &str) -> u8 {
    db().tile_id(key)
}

/// Builds the surface map.
///
/// 1. Elevation = fractal Perlin noise minus a radial falloff (island shape);
///    on maps larger than the classic 300×200 a slow "continental" layer that
///    grows with the map shapes seas, bays and peninsulas. Moisture and
///    temperature (north is cold, south is hot, highlands are cooler) are
///    independent noise fields whose features grow with the square root of
///    the map's scale, classified by quantiles so the share of water, deserts
///    or tundra is stable for any seed and size.
/// 2. Biomes come from (elevation, moisture, temperature); local detail noise
///    adds tree density, dunes, snowdrifts and reeds.
/// 3. Rivers flow downhill from highlands to the sea (frozen in the north).
/// 4. Volcanic wastes and cursed lands scar the land far from the heart of
///    the world (the land nearest the middle of the map, where the journey
///    starts).
/// 5. Cities and villages are spread over the largest walkable landmass, as
///    many per area of land as in the classic world; the first village is the
///    one nearest the heart.
/// 6. Settlements are joined by roads along a minimum spanning tree, each
///    found with weighted A* (bridges over water).
/// 7. Connected areas of one biome become named regions; landmarks are
///    stamped on free land; dungeon entrances are scattered, the deeper ones
///    further from the start.
pub fn generate_overworld(seed: i64, w: i32, h: i32) -> Overworld {
    let sc = MapScale::of(w, h);
    let mut r = Rng::labeled(seed, "overworld");
    let name = world_name(&mut r);
    let mut l = Level::new("overworld", &name, w, h, t("grass"));
    l.lit = true;
    l.theme = "overworld".into();

    let noise = [
        Perlin::new(&mut r),
        Perlin::new(&mut r),
        Perlin::new(&mut r),
        Perlin::new(&mut r),
        Perlin::new(&mut r),
    ];
    let NoiseFields {
        e: e_,
        m: m_,
        d: d_,
        t: mut tm,
    } = noise_fields(w, h, sc, &noise);
    let n = (w * h) as usize;
    let eq = quantiles(&e_);
    let mq = quantiles(&m_);
    let tq = quantiles(&tm);
    let e75 = eq(0.75);
    for i in 0..n {
        tm[i] -= (e_[i] - e75).max(0.0) * 1.5;
    }

    let k = |key: &str| t(key);
    let (deep_water, water, ice, snow_ground) =
        (k("deep_water"), k("water"), k("ice"), k("snow_ground"));
    let (desert_sand, sand, snow, ice_rock) =
        (k("desert_sand"), k("sand"), k("snow"), k("ice_rock"));
    let (sandstone, mountain, snowdrift, dunes) =
        (k("sandstone"), k("mountain"), k("snowdrift"), k("dunes"));
    let (hill, snow_pine, cactus, dead_tree) =
        (k("hill"), k("snow_pine"), k("cactus"), k("dead_tree"));
    let (reeds, swamp, pine, tree) = (k("reeds"), k("swamp"), k("pine"), k("tree"));
    let (bush, forest_floor, flowers, tall_grass) =
        (k("bush"), k("forest_floor"), k("flowers"), k("tall_grass"));
    let (grass2, grass) = (k("grass2"), k("grass"));
    let (e20, e28, e31, e985, e945, e87, e70, e60) = (
        eq(0.20),
        eq(0.28),
        eq(0.31),
        eq(0.985),
        eq(0.945),
        eq(0.87),
        eq(0.7),
        eq(0.6),
    );
    let (m40, m52, m62, m86) = (mq(0.4), mq(0.52), mq(0.62), mq(0.86));
    let (t15, t80) = (tq(0.15), tq(0.8));

    let mut cold = vec![false; n];
    for i in 0..n {
        let (e, m, det) = (e_[i], m_[i], d_[i] as f64);
        let is_cold = tm[i] < t15;
        let is_hot = tm[i] > t80 && m < m62;
        cold[i] = is_cold;
        let tile = if e < e20 {
            deep_water
        } else if e < e28 {
            if is_cold {
                ice
            } else {
                water
            }
        } else if e < e31 {
            if is_cold {
                snow_ground
            } else if is_hot {
                desert_sand
            } else {
                sand
            }
        } else if e > e985 {
            snow
        } else if e > e945 || (e > e87 && det > 0.25) {
            if is_cold {
                if r.int_n(3) == 0 {
                    snow
                } else {
                    ice_rock
                }
            } else if is_hot {
                sandstone
            } else {
                mountain
            }
        } else if e > e87 {
            if is_cold {
                snowdrift
            } else if is_hot {
                dunes
            } else {
                hill
            }
        } else if is_cold {
            let v = r.f64();
            if v < 0.3 + det * 0.4 && m > m40 {
                snow_pine
            } else if v < 0.012 {
                ice_rock
            } else if det > 0.2 {
                snowdrift
            } else {
                snow_ground
            }
        } else if is_hot {
            let v = r.f64();
            if v < 0.018 {
                cactus
            } else if v < 0.026 {
                sandstone
            } else if v < 0.03 {
                dead_tree
            } else if det > 0.12 {
                dunes
            } else {
                desert_sand
            }
        } else if m > m86 && e < e60 {
            let v = r.f64();
            if v < 0.08 {
                water
            } else if v < 0.12 {
                dead_tree
            } else if v < 0.32 || det > 0.2 {
                reeds
            } else {
                swamp
            }
        } else if m > m52 {
            let density = 0.38 + det * 0.5;
            let v = r.f64();
            if v < density {
                if e > e70 || r.int_n(3) == 0 {
                    pine
                } else {
                    tree
                }
            } else if v < density + 0.07 {
                bush
            } else {
                forest_floor
            }
        } else {
            let v = r.f64();
            if v < 0.012 {
                flowers
            } else if v < 0.03 {
                tree
            } else if det > 0.18 {
                tall_grass
            } else if v < 0.5 {
                grass2
            } else {
                grass
            }
        };
        l.tiles[i] = tile;
    }
    drop((m_, d_, tm));

    carve_rivers(&mut r, &mut l, &e_, &eq, sc);
    drop(e_);
    // rivers freeze in the north
    for i in 0..n {
        if l.tiles[i] == water && cold[i] {
            l.tiles[i] = ice;
        }
    }
    drop(cold);

    let mut main = largest_region(&l);
    let heart = central_land(&l, &main);
    scar_land(&mut r, &mut l, &main, heart, sc);
    main = largest_region(&l);
    let land = main.iter().filter(|&&b| b).count();
    let mut names = HashSet::new();
    let cities = place_cities(&mut r, &mut l, &main, land, &mut names);
    // villages first: the start is in the first one
    let mut villages = place_villages(&mut r, &mut l, &main, &cities, heart, land, &mut names);
    villages.extend(cities);
    let start = villages.first().map(|v| v.center).unwrap_or(heart);
    build_roads(&mut l, &villages);
    let (regions, region_map) = name_regions(&mut r, &l, &villages, sc);
    main = largest_region(&l);
    let landmarks = place_landmarks(&mut r, &mut l, &main, &villages, start, land);
    main = largest_region(&l);
    let entrances = place_entrances(&mut r, &mut l, &main, &villages, start, land, sc);

    let start = if let Some(v) = villages.first() {
        find_free(&l, Pos::new(v.center.x, v.center.y + 2))
    } else {
        let i = main.iter().position(|&b| b).unwrap_or(0) as i32;
        Pos::new(i % w, i / w)
    };
    Overworld {
        name,
        level: l,
        villages,
        entrances,
        start,
        regions,
        region_map,
        landmarks,
    }
}

struct NoiseFields {
    /// elevation (with the island falloff)
    e: Vec<f32>,
    /// moisture
    m: Vec<f32>,
    /// local detail
    d: Vec<f32>,
    /// temperature
    t: Vec<f32>,
}

/// Computes the noise fields of the map, rows split between threads (noise
/// is a pure function of the cell, so the result does not depend on the
/// number of threads).
fn noise_fields(w: i32, h: i32, sc: MapScale, n: &[Perlin; 5]) -> NoiseFields {
    let [elev, moist, detail, temp, cont] = n;
    let cells = (w * h) as usize;
    let mut f = NoiseFields {
        e: vec![0.0; cells],
        m: vec![0.0; cells],
        d: vec![0.0; cells],
        t: vec![0.0; cells],
    };
    // continents grow with the map, biomes with the square root of its scale
    let continents = sc.lin > 1.5;
    let cs = 48.0 * sc.lin;
    let bs = sc.lin.sqrt();
    let cell = |x: i32, y: i32| -> [f32; 4] {
        let (fx, fy) = (x as f64 / 48.0, y as f64 / 48.0);
        let mut e = (elev.fbm(fx, fy, 6) + 1.0) / 2.0;
        if continents {
            let c = (cont.fbm(x as f64 / cs, y as f64 / cs, 5) + 1.0) / 2.0;
            e = 0.35 * e + 0.65 * c;
        }
        let (nx, ny) = (
            x as f64 / w as f64 * 2.0 - 1.0,
            y as f64 / h as f64 * 2.0 - 1.0,
        );
        let d = 1.0 - (1.0 - nx * nx) * (1.0 - ny * ny);
        let (mx, my) = (x as f64 / (60.0 * bs), y as f64 / (60.0 * bs));
        let (tx, ty) = (x as f64 / (80.0 * bs), y as f64 / (80.0 * bs));
        [
            (e - 0.55 * d * d) as f32,
            ((moist.fbm(mx + 100.0, my + 100.0, 4) + 1.0) / 2.0) as f32,
            detail.fbm(x as f64 / 6.0, y as f64 / 6.0, 2) as f32,
            (y as f64 / h as f64 * 1.1 + temp.fbm(tx + 50.0, ty + 50.0, 3) * 0.55) as f32,
        ]
    };
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 16);
    let rows = (h as usize).div_ceil(threads).max(1);
    let span = rows * w as usize;
    std::thread::scope(|s| {
        let parts =
            f.e.chunks_mut(span)
                .zip(f.m.chunks_mut(span))
                .zip(f.d.chunks_mut(span).zip(f.t.chunks_mut(span)));
        for (k, ((e, m), (d, t))) in parts.enumerate() {
            let cell = &cell;
            s.spawn(move || {
                let y0 = (k * rows) as i32;
                for (j, out) in e.iter_mut().enumerate() {
                    let (x, y) = (j as i32 % w, y0 + j as i32 / w);
                    let v = cell(x, y);
                    *out = v[0];
                    m[j] = v[1];
                    d[j] = v[2];
                    t[j] = v[3];
                }
            });
        }
    });
    f
}

/// A function mapping q in [0,1] to the value at that quantile (estimated
/// from an even sample on big maps).
fn quantiles(v: &[f32]) -> impl Fn(f64) -> f32 {
    let step = (v.len() / 400_000).max(1);
    let mut s: Vec<f32> = v.iter().step_by(step).copied().collect();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    move |q: f64| {
        let i = (q * (s.len() - 1) as f64) as usize;
        s[i.min(s.len() - 1)]
    }
}

fn carve_rivers(r: &mut Rng, l: &mut Level, e_: &[f32], eq: &dyn Fn(f64) -> f32, sc: MapScale) {
    let (water, deep) = (t("water"), t("deep_water"));
    let (lo, hi) = (eq(0.75), eq(0.93));
    // as many rivers per area as in the classic world, longer on big maps
    let want = ((7.0 * sc.area / sc.lin).round() as i32).max(1);
    let max_steps = (900.0 * sc.lin) as usize;
    let mut rivers = 0;
    let mut attempt = 0;
    while attempt < 60 * want && rivers < want {
        attempt += 1;
        let (mut x, mut y) = (r.int_n(l.w), r.int_n(l.h));
        let e = e_[(y * l.w + x) as usize];
        if e < lo || e > hi {
            continue;
        }
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        let mut reached = false;
        for steps in 0..max_steps {
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
                let v = e_[(ny * l.w + nx) as usize] as f64 + r.f64() * 0.004;
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
    is_ground_def(db().tile(tile))
}

/// Open land of a natural biome without hazards.
pub fn is_ground_def(def: &crate::content::TileDef) -> bool {
    matches!(
        def.biome.as_str(),
        "plains" | "forest" | "sand" | "hills" | "swamp" | "desert" | "tundra" | "ash" | "cursed"
    ) && def.walkable
        && def.damage == 0.0
}

const VILLAGE_W: i32 = 26;
const VILLAGE_H: i32 = 18;

fn place_villages(
    r: &mut Rng,
    l: &mut Level,
    main: &[bool],
    cities: &[Village],
    heart: Pos,
    land: usize,
    used: &mut HashSet<String>,
) -> Vec<Village> {
    let mut vs: Vec<Village> = Vec::new();
    // four or five per classic world of land
    let want = per_land(land, 4.0 + r.f64() * 1.5, 1);
    let good_t = tile_table(|d| matches!(d.biome.as_str(), "plains" | "forest" | "sand"));
    let good = Sat::new(l, |i| good_t[l.tiles[i] as usize] as u32);
    let cities = VillageIndex::new(cities);
    let mut near = Buckets::new(64);
    let (w, h) = (l.w, l.h);
    let fits = |vs: &[Village], near: &Buckets, cx: i32, cy: i32| -> Option<Rect> {
        if cx < 20 || cy < 16 || cx >= w - 20 || cy >= h - 16 || !main[(cy * w + cx) as usize] {
            return None;
        }
        let area = Rect::new(cx - VILLAGE_W / 2, cy - VILLAGE_H / 2, VILLAGE_W, VILLAGE_H);
        // mostly dry flat land
        let ok = good.sum(area.x, area.y, area.x + area.w, area.y + area.h) * 100
            >= (area.w * area.h) as u32 * 85;
        let far = near.near(Pos::new(cx, cy), 50).into_iter().all(|i| {
            let c = vs[i].center;
            !((c.x - cx).abs() < 50 && (c.y - cy).abs() < 40)
        });
        (ok && far && !cities.overlaps(&area, 12)).then_some(area)
    };
    // the first village, where the journey starts, near the heart of the world
    let mut rad = 24;
    'start: while rad < l.w.max(l.h) {
        for _ in 0..300 {
            let (cx, cy) = (
                heart.x + r.int_n(2 * rad + 1) - rad,
                heart.y + r.int_n(2 * rad + 1) - rad,
            );
            if let Some(area) = fits(&vs, &near, cx, cy) {
                let name = village_name(r, used);
                near.insert(area.center(), vs.len());
                vs.push(build_village(r, l, area, name));
                break 'start;
            }
        }
        rad *= 2;
    }
    // the rest spread evenly: a few tries in every cell of a grid
    let step = ((l.w as f64 * l.h as f64 / (want as f64 * 2.6)).sqrt() as i32).max(40);
    let cells = scatter_cells(r, l.w, l.h, step);
    let tries = (4000 / cells.len().max(1)).max(16);
    for c in &cells {
        if vs.len() >= want {
            break;
        }
        for _ in 0..tries {
            let p = in_cell(r, c, l.w, l.h, 16);
            if let Some(area) = fits(&vs, &near, p.x, p.y) {
                let name = village_name(r, used);
                near.insert(area.center(), vs.len());
                vs.push(build_village(r, l, area, name));
                break;
            }
        }
    }
    vs
}

fn build_village(r: &mut Rng, l: &mut Level, area: Rect, name: String) -> Village {
    let mut v = Village {
        name,
        center: area.center(),
        area,
        ..Default::default()
    };
    // clear an ellipse of grass
    for y in area.y..area.y + area.h {
        for x in area.x..area.x + area.w {
            let dx = (x - v.center.x) as f64 / (area.w / 2) as f64;
            let dy = (y - v.center.y) as f64 / (area.h / 2) as f64;
            if dx * dx + dy * dy <= 1.05 {
                l.set(
                    x,
                    y,
                    if r.int_n(2) == 0 {
                        t("grass")
                    } else {
                        t("grass2")
                    },
                );
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
                l.set(
                    x,
                    y,
                    if wall {
                        t("house_wall")
                    } else {
                        t("house_floor")
                    },
                );
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
                "elder" | "merchant" | "smith" | "healer" | "priest"
                    if house_spot < houses.len() =>
                {
                    house_spot += 1;
                    houses[house_spot - 1].center()
                }
                _ => village_free(r, l, &area, c),
            };
            v.npcs.push(NpcSpawn {
                role: role.key.clone(),
                name: person_name(r),
                pos: p,
                night: Pos::default(),
            });
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
        "well"
            | "house_wall"
            | "door"
            | "house_floor"
            | "city_wall"
            | "stone_wall"
            | "stone_floor"
            | "carpet"
            | "fountain"
            | "market_stall"
            | "lamp_post"
            | "statue"
            | "altar"
            | "brazier"
            | "crystal"
            | "chest_open"
    )
}

/// Joins the settlements with roads along a minimum spanning tree (Prim's
/// algorithm on straight distances), so every road leads to a near neighbour.
fn build_roads(l: &mut Level, vs: &[Village]) {
    if vs.len() < 2 {
        return;
    }
    let n = vs.len();
    let mut in_tree = vec![false; n];
    let mut best = vec![(i64::MAX, 0usize); n];
    let d2 = |a: usize, b: usize| vs[a].center.dist_sq(vs[b].center) as i64;
    in_tree[0] = true;
    for j in 1..n {
        best[j] = (d2(0, j), 0);
    }
    let mut edges = Vec::with_capacity(n - 1);
    for _ in 1..n {
        let Some(i) = (0..n).filter(|&i| !in_tree[i]).min_by_key(|&i| best[i].0) else {
            break;
        };
        in_tree[i] = true;
        edges.push((i, best[i].1));
        for j in 0..n {
            if !in_tree[j] {
                let d = d2(i, j);
                if d < best[j].0 {
                    best[j] = (d, i);
                }
            }
        }
    }
    let (road, bridge) = (t("road"), t("bridge"));
    let (water, deep) = (t("water"), t("deep_water"));
    let cost_t = tile_table(|def| {
        if def.key == "road" || def.key == "bridge" {
            0.3
        } else if def.key == "deep_water" {
            25.0
        } else if def.key == "water" {
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
    });
    let keep_t = tile_table(|def| {
        built_up(&def.key)
            || def.key == "city_gate"
            || def.key == "cobblestone"
            || def.key == "garden"
    });
    for (i, j) in edges {
        let from = Pos::new(vs[i].center.x, vs[i].center.y + 1);
        let to = Pos::new(vs[j].center.x, vs[j].center.y + 1);
        // a generous node budget for detours around lakes and mountains
        let budget = (from.manhattan(to) as usize * 400).clamp(20_000, 600_000);
        let path = {
            let lv: &Level = l;
            let mut cost = |x: i32, y: i32| -> f64 { cost_t[lv.at(x, y) as usize] };
            find_path(lv.w, lv.h, from, to, budget, false, &mut cost)
        };
        for p in path {
            let tt = l.at(p.x, p.y);
            if tt == water || tt == deep {
                l.set(p.x, p.y, bridge);
            } else if !keep_t[tt as usize] {
                l.set(p.x, p.y, road);
            }
        }
    }
}

fn place_entrances(
    r: &mut Rng,
    l: &mut Level,
    main: &[bool],
    vs: &[Village],
    start: Pos,
    land: usize,
    sc: MapScale,
) -> Vec<Entrance> {
    let mut es: Vec<Entrance> = Vec::new();
    let mut used = HashSet::new();
    let vi = VillageIndex::new(vs);
    let mut near = Buckets::new(64);
    let ground = tile_table(is_ground_def);
    let hills = tile_table(|d| d.biome == "hills");
    let themes = ["cave", "crypt", "cave", "crypt", "crypt", "cave"];
    let want = per_land(land, themes.len() as f64, 2);
    // caves prefer hills: a while after each entrance only hills will do
    let mut since = 0;
    for _ in 0..6000 * want.div_ceil(6) {
        if es.len() >= want {
            break;
        }
        since += 1;
        let (x, y) = (4 + r.int_n(l.w - 8), 4 + r.int_n(l.h - 8));
        let tile = l.at(x, y) as usize;
        if !main[(y * l.w + x) as usize] || !ground[tile] {
            continue;
        }
        let p = Pos::new(x, y);
        if (p.x - start.x).abs() < 25 && (p.y - start.y).abs() < 20 {
            continue;
        }
        if vi.overlaps(&Rect::new(x - 3, y - 3, 7, 7), 2) {
            continue;
        }
        if near
            .near(p, 30)
            .into_iter()
            .any(|i| (es[i].pos.x - x).abs() < 30 && (es[i].pos.y - y).abs() < 25)
        {
            continue;
        }
        let theme = themes[es.len() % themes.len()];
        if theme == "cave" && !hills[tile] && since < 3000 {
            continue;
        }
        since = 0;
        let name = place_name(r, dungeon_names(theme), vi.nearest_name(p), &mut used);
        near.insert(p, es.len());
        es.push(Entrance {
            pos: p,
            theme: theme.into(),
            name,
            max_depth: 0,
        });
    }
    // deeper dungeons further from the start: the same mix of depths as in
    // the classic world, by rank of distance
    es.sort_by_key(|e| e.pos.dist_sq(start));
    let n = es.len().max(1);
    near = Buckets::new(64);
    for (i, e) in es.iter_mut().enumerate() {
        e.max_depth = [2, 3, 4, 4, 5, 5][i * 6 / n];
        near.insert(e.pos, i);
    }
    // every special land hides its own dungeons deep inside: one per area of
    // the land a classic region had
    let mut per_tile = [0usize; 256];
    for &tt in &l.tiles {
        per_tile[tt as usize] += 1;
    }
    for (theme, biome) in REGIONAL_DUNGEONS {
        let have: usize = db()
            .b
            .tiles
            .iter()
            .enumerate()
            .filter(|(_, d)| d.biome == *biome)
            .map(|(i, _)| per_tile[i])
            .sum();
        let count = ((have as f64 / (2500.0 * sc.lin)).round() as usize).max(1);
        let mut placed = 0;
        let mut attempt = 0;
        while placed < count && attempt < 5000 * count {
            attempt += 1;
            let (x, y) = (4 + r.int_n(l.w - 8), 4 + r.int_n(l.h - 8));
            if !main[(y * l.w + x) as usize]
                || !ground[l.at(x, y) as usize]
                || l.def(x, y).biome != *biome
            {
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
            if inside < 13 * 13 * 3 / 4 && attempt < 4000 * count {
                continue;
            }
            let p = Pos::new(x, y);
            if near
                .near(p, 16)
                .into_iter()
                .any(|i| (es[i].pos.x - x).abs() < 16 && (es[i].pos.y - y).abs() < 12)
            {
                continue;
            }
            // regional dungeons of one land keep apart
            let far = es
                .iter()
                .filter(|e| e.theme == *theme)
                .all(|e| e.pos.dist(p) as f64 >= 60.0 * sc.lin.sqrt());
            if !far && attempt < 4000 * count {
                continue;
            }
            let name = place_name(r, dungeon_names(theme), vi.nearest_name(p), &mut used);
            near.insert(p, es.len());
            es.push(Entrance {
                pos: p,
                theme: theme.to_string(),
                name,
                max_depth: 4,
            });
            placed += 1;
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
