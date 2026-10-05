use super::*;
use crate::content::db;
use crate::world::{Level, Pos, DIRS4};
use std::collections::HashSet;

/// A named connected area of one biome on the overworld.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Region {
    pub name: String,
    /// plains, forest, swamp, hills, desert, tundra, ash, cursed
    pub kind: String,
    /// 0..3, raises monster levels
    pub danger: i32,
}

/// A notable place on the overworld.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Landmark {
    pub name: String,
    /// shrine, circle, ruins, graveyard, camp, oasis
    pub kind: String,
    pub pos: Pos,
}

/// How dangerous each kind of land is.
pub fn region_danger(kind: &str) -> i32 {
    match kind {
        "swamp" | "hills" | "desert" => 1,
        "tundra" | "cursed" => 2,
        "ash" => 3,
        _ => 0,
    }
}

pub(super) const REGIONAL_DUNGEONS: &[(&str, &str)] = &[("temple", "desert"), ("ice", "tundra"), ("volcano", "ash"), ("fortress", "cursed")];

pub fn near_village(vs: &[Village], x: i32, y: i32, margin: i32) -> bool {
    vs.iter().any(|v| {
        let a = v.area;
        x >= a.x - margin && y >= a.y - margin && x < a.x + a.w + margin && y < a.y + a.h + margin
    })
}

// ---- volcanic and cursed lands ----

/// Turns two areas far from people into a volcanic waste and a cursed land.
pub(super) fn scar_land(r: &mut Rng, l: &mut Level, main: &[bool], vs: &[Village], start: Pos) {
    let rx = (l.w / 12).max(10);
    let ry = rx;
    let mut centers: Vec<Pos> = Vec::new();
    for kind in ["ash", "cursed"] {
        let Some(c) = scar_center(r, l, main, vs, start, &centers, rx, ry) else { continue };
        centers.push(c);
        stamp_scar(r, l, c, rx, ry, vs, kind);
    }
}

#[allow(clippy::too_many_arguments)]
fn scar_center(r: &mut Rng, l: &Level, main: &[bool], vs: &[Village], start: Pos, others: &[Pos], rx: i32, ry: i32) -> Option<Pos> {
    let min_start = (l.w + l.h) / 4;
    for _ in 0..3000 {
        let x = rx + r.int_n((l.w - 2 * rx).max(1));
        let y = ry + r.int_n((l.h - 2 * ry).max(1));
        if !main[(y * l.w + x) as usize] || (x - start.x).abs() + (y - start.y).abs() < min_start {
            continue;
        }
        let mut ok = true;
        for v in vs {
            let (dx, dy) = (v.center.x - x, v.center.y - y);
            if dx * dx + dy * dy < (rx + 18) * (rx + 18) {
                ok = false;
            }
        }
        for o in others {
            let (dx, dy) = (o.x - x, o.y - y);
            if dx * dx + dy * dy < (3 * rx) * (3 * rx) {
                ok = false;
            }
        }
        if !ok {
            continue;
        }
        let (mut land, mut total) = (0, 0);
        for dy in -ry..=ry {
            for dx in -rx..=rx {
                total += 1;
                if is_ground(l.at(x + dx, y + dy)) {
                    land += 1;
                }
            }
        }
        if land * 100 / total >= 55 {
            return Some(Pos::new(x, y));
        }
    }
    None
}

fn settlement_tile(key: &str) -> bool {
    matches!(key, "house_wall" | "house_floor" | "door" | "door_open" | "well" | "fence" | "road" | "bridge" | "dungeon")
}

fn stamp_scar(r: &mut Rng, l: &mut Level, c: Pos, rx: i32, ry: i32, vs: &[Village], kind: &str) {
    let edge_n = Perlin::new(r);
    let det_n = Perlin::new(r);
    for y in c.y - ry * 3 / 2..=c.y + ry * 3 / 2 {
        for x in c.x - rx * 3 / 2..=c.x + rx * 3 / 2 {
            if !l.inside(x, y) || near_village(vs, x, y, 3) {
                continue;
            }
            let def = l.def(x, y);
            if settlement_tile(&def.key) || def.key == "deep_water" {
                continue;
            }
            let (dx, dy) = ((x - c.x) as f64 / rx as f64, (y - c.y) as f64 / ry as f64);
            let edge = 1.0 + edge_n.fbm(x as f64 / 9.0, y as f64 / 9.0, 2) * 0.45;
            let k = dx.hypot(dy) / edge;
            if k > 1.0 {
                continue;
            }
            // ragged border: the old land shows through
            if k > 0.82 && r.f64() < (k - 0.82) / 0.18 {
                continue;
            }
            let det = det_n.fbm(x as f64 / 5.0, y as f64 / 5.0, 2);
            let v = r.f64();
            let key = def.key.as_str();
            let tile: Option<&str> = if kind == "ash" {
                if key == "water" || key == "ice" {
                    if k < 0.6 {
                        Some("lava")
                    } else {
                        None
                    }
                } else if k < 0.09 {
                    Some("lava")
                } else if k < 0.2 {
                    Some("basalt") // the volcano cone
                } else if !def.walkable && !matches!(def.biome.as_str(), "plains" | "forest" | "tundra" | "desert") {
                    Some("basalt")
                } else if det > 0.4 {
                    Some("lava")
                } else if det > 0.3 {
                    Some("basalt")
                } else if v < 0.03 {
                    Some("charred_tree")
                } else if v < 0.07 {
                    Some("magma_crack")
                } else if matches!(key, "tree" | "pine" | "snow_pine" | "palm") {
                    Some("charred_tree")
                } else {
                    Some("ash")
                }
            } else {
                match key {
                    "tree" | "pine" | "snow_pine" | "palm" | "bush" => Some(if v < 0.3 { "dead_tree" } else { "twisted_tree" }),
                    "swamp" | "reeds" | "water" | "ice" | "mountain" | "snow" | "ice_rock" | "sandstone" | "cactus" | "dead_tree" => None,
                    _ => Some(if v < 0.025 {
                        "mushrooms"
                    } else if v < 0.04 {
                        "dead_tree"
                    } else if v < 0.045 {
                        "gravestone"
                    } else if det > 0.35 && v < 0.5 {
                        "twisted_tree"
                    } else {
                        "blight_grass"
                    }),
                }
            };
            if let Some(tk) = tile {
                l.set(x, y, t(tk));
            }
        }
    }
}

// ---- named regions ----

fn region_names(kind: &str) -> &'static [&'static str] {
    match kind {
        "plains" => &["Солнечные Луга", "Вольные Поля", "Ковыльная Степь", "Медовые Луга", "Долина Ветров", "Зелёный Дол", "Пастушьи Холмы", "Широкое Поле"],
        "forest" => &["Шепчущий Лес", "Чернолесье", "Дубрава Старых Богов", "Зелёная Пуща", "Совиный Бор", "Еловый Край", "Туманный Лес", "Волчья Чаща", "Медвежий Бор", "Ясеневая Роща"],
        "swamp" => &["Гнилые Топи", "Туманные Болота", "Ведьмина Трясина", "Камышовые Плавни", "Чёрная Гать", "Лягушачьи Мхи"],
        "hills" => &["Каменные Холмы", "Гремящий Кряж", "Седые Предгорья", "Орлиные Утёсы", "Хребет Великана", "Ветреные Склоны"],
        "desert" => &["Пески Забвения", "Пустыня Аш-Шарр", "Золотые Барханы", "Море Песка", "Выжженная Равнина"],
        "tundra" => &["Ледяной Предел", "Белая Пустошь", "Земли Вечной Зимы", "Северная Тундра", "Стылые Равнины"],
        "ash" => &["Пепельные Пустоши", "Огненный Разлом", "Земли Пламени", "Пепелище"],
        _ => &["Проклятые Земли", "Сумрачный Край", "Долина Мёртвых", "Край Скорби"],
    }
}

fn region_kind(tile: u8) -> &'static str {
    match db().tile(tile).biome.as_str() {
        "plains" | "sand" => "plains",
        "hills" | "snow" => "hills",
        "forest" => "forest",
        "swamp" => "swamp",
        "desert" => "desert",
        "tundra" => "tundra",
        "ash" => "ash",
        "cursed" => "cursed",
        _ => "",
    }
}

/// Labels large connected areas of one kind of land and spreads the labels
/// over roads, rivers and small patches.
pub(super) fn name_regions(r: &mut Rng, l: &Level) -> (Vec<Region>, Vec<u8>) {
    let n = (l.w * l.h) as usize;
    let mut labels = vec![0i32; n]; // 0 = unvisited, -1 = too small, >0 region
    let min_size = (n / 300).max(120);
    let mut regions: Vec<Region> = Vec::new();
    let mut used = HashSet::new();
    let mut stack = Vec::with_capacity(1024);
    let mut cells = Vec::new();
    let kinds: Vec<&str> = l.tiles.iter().map(|&t| region_kind(t)).collect();
    for i in 0..n {
        let kind = kinds[i];
        if labels[i] != 0 || kind.is_empty() {
            continue;
        }
        cells.clear();
        stack.clear();
        stack.push(i);
        labels[i] = -1;
        while let Some(c) = stack.pop() {
            cells.push(c);
            let (cx, cy) = (c as i32 % l.w, c as i32 / l.w);
            for d in DIRS4 {
                let (nx, ny) = (cx + d.x, cy + d.y);
                if !l.inside(nx, ny) {
                    continue;
                }
                let ni = (ny * l.w + nx) as usize;
                if labels[ni] == 0 && kinds[ni] == kind {
                    labels[ni] = -1;
                    stack.push(ni);
                }
            }
        }
        if cells.len() < min_size || regions.len() >= 250 {
            continue;
        }
        let (mut sx, mut sy) = (0i64, 0i64);
        for &c in &cells {
            labels[c] = regions.len() as i32 + 1;
            sx += (c as i32 % l.w) as i64;
            sy += (c as i32 / l.w) as i64;
        }
        let center = Pos::new((sx / cells.len() as i64) as i32, (sy / cells.len() as i64) as i32);
        let name = region_name(r, kind, center, l, &mut used);
        regions.push(Region { name, kind: kind.into(), danger: region_danger(kind) });
    }
    // spread labels to everything else (breadth first from labelled cells)
    let mut queue: Vec<usize> = (0..n).filter(|&i| labels[i] > 0).collect();
    let mut q = 0;
    while q < queue.len() {
        let c = queue[q];
        q += 1;
        let (cx, cy) = (c as i32 % l.w, c as i32 / l.w);
        for d in DIRS4 {
            let (nx, ny) = (cx + d.x, cy + d.y);
            if !l.inside(nx, ny) {
                continue;
            }
            let ni = (ny * l.w + nx) as usize;
            if labels[ni] <= 0 {
                labels[ni] = labels[c];
                queue.push(ni);
            }
        }
    }
    (regions, labels.iter().map(|&lb| if lb > 0 { lb as u8 } else { 0 }).collect())
}

fn region_name(r: &mut Rng, kind: &str, c: Pos, l: &Level, used: &mut HashSet<String>) -> String {
    let list = region_names(kind);
    let free: Vec<&str> = list.iter().copied().filter(|s| !used.contains(*s)).collect();
    if !free.is_empty() {
        let s = *r.pick(&free);
        used.insert(s.to_string());
        return s.to_string();
    }
    let (dx, dy) = (c.x as f64 / l.w as f64 - 0.5, c.y as f64 / l.h as f64 - 0.5);
    let dir = if dx.abs() < 0.12 && dy.abs() < 0.12 {
        "центр"
    } else if dx.abs() > dy.abs() && dx > 0.0 {
        "восток"
    } else if dx.abs() > dy.abs() {
        "запад"
    } else if dy > 0.0 {
        "юг"
    } else {
        "север"
    };
    format!("{} ({})", r.pick(list), dir)
}

// ---- landmarks ----

fn landmark_names(kind: &str) -> &'static [&'static str] {
    match kind {
        "shrine" => &["Святилище Перуна", "Святилище Велеса", "Святилище Лады", "Святилище Даждьбога", "Святилище Мокоши", "Святилище Стрибога", "Капище Сварога"],
        "circle" => &["Круг Камней", "Кольцо Древних", "Менгиры Предков", "Каменный Хоровод"],
        "ruins" => &["Руины Старой Башни", "Руины Заставы", "Развалины Храма", "Руины Сторожевого Поста", "Обломки Древнего Форта"],
        "graveyard" => &["Старое Кладбище", "Забытый Погост", "Кладбище у Ручья", "Погост Безымянных"],
        "camp" => &["Лагерь Разбойников", "Логово Шайки", "Стоянка Головорезов"],
        _ => &["Оазис Миражей", "Пальмовый Оазис", "Зелёный Оазис"],
    }
}

struct LandmarkSpec {
    kind: &'static str,
    count: i32,
    w: i32,
    h: i32,
    biomes: &'static [&'static str],
}

const LANDMARK_SPECS: &[LandmarkSpec] = &[
    LandmarkSpec { kind: "camp", count: 2, w: 9, h: 9, biomes: &["plains", "hills", "forest", "sand"] },
    LandmarkSpec { kind: "ruins", count: 3, w: 9, h: 9, biomes: &["plains", "forest", "hills", "desert", "tundra", "cursed", "sand"] },
    LandmarkSpec { kind: "graveyard", count: 2, w: 11, h: 9, biomes: &["plains", "forest", "cursed"] },
    LandmarkSpec { kind: "circle", count: 2, w: 9, h: 9, biomes: &["plains", "hills", "tundra", "forest"] },
    LandmarkSpec { kind: "oasis", count: 2, w: 11, h: 9, biomes: &["desert"] },
    LandmarkSpec { kind: "shrine", count: 5, w: 3, h: 3, biomes: &["plains", "forest", "hills", "desert", "tundra", "cursed", "swamp", "sand"] },
];

fn ground_for(biome: &str) -> &'static str {
    match biome {
        "sand" => "sand",
        "forest" => "forest_floor",
        "desert" => "desert_sand",
        "tundra" => "snow_ground",
        "cursed" => "blight_grass",
        "swamp" => "swamp",
        "ash" => "ash",
        _ => "grass",
    }
}

pub(super) fn place_landmarks(r: &mut Rng, l: &mut Level, main: &[bool], vs: &[Village], start: Pos) -> Vec<Landmark> {
    let mut out: Vec<Landmark> = Vec::new();
    let mut used = HashSet::new();
    for sp in LANDMARK_SPECS {
        let mut placed = 0;
        for _ in 0..3000 {
            if placed >= sp.count {
                break;
            }
            let x = sp.w + r.int_n((l.w - 2 * sp.w).max(1));
            let y = sp.h + r.int_n((l.h - 2 * sp.h).max(1));
            if !main[(y * l.w + x) as usize] {
                continue;
            }
            let biome = l.def(x, y).biome.clone();
            if !sp.biomes.contains(&biome.as_str()) || near_village(vs, x, y, sp.w) || (x - start.x).abs() + (y - start.y).abs() < 20 {
                continue;
            }
            if out.iter().any(|o| (o.pos.x - x).abs() < 18 && (o.pos.y - y).abs() < 14) {
                continue;
            }
            if !free_area(l, x - sp.w / 2, y - sp.h / 2, sp.w, sp.h) {
                continue;
            }
            stamp_landmark(r, l, sp.kind, Pos::new(x, y), sp.w, sp.h, ground_for(&biome));
            out.push(Landmark { name: pick_unique(r, landmark_names(sp.kind), &mut used), kind: sp.kind.into(), pos: Pos::new(x, y) });
            placed += 1;
        }
    }
    out
}

/// Plain land (trees allowed) without water, rocks or buildings.
fn free_area(l: &Level, x0: i32, y0: i32, w: i32, h: i32) -> bool {
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            if !l.inside(x, y) {
                return false;
            }
            let def = l.def(x, y);
            if settlement_tile(&def.key) || def.damage > 0.0 || def.biome == "water" || def.key == "ice" {
                return false;
            }
            if !def.walkable && !matches!(def.key.as_str(), "tree" | "pine" | "snow_pine" | "bush" | "cactus" | "dead_tree" | "twisted_tree") {
                return false;
            }
        }
    }
    true
}

fn stamp_landmark(r: &mut Rng, l: &mut Level, kind: &str, c: Pos, w: i32, h: i32, ground: &str) {
    let (x0, y0) = (c.x - w / 2, c.y - h / 2);
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            if !l.def(x, y).walkable || kind != "shrine" {
                l.set(x, y, t(ground));
            }
        }
    }
    let set = |l: &mut Level, x: i32, y: i32, k: &str| l.set(x, y, t(k));
    match kind {
        "shrine" => {
            set(l, c.x, c.y, "shrine");
            for d in [(-1, 1), (1, 1)] {
                if r.int_n(2) == 0 && !matches!(ground, "snow_ground" | "desert_sand" | "blight_grass") {
                    set(l, c.x + d.0, c.y + d.1, "flowers");
                }
            }
        }
        "circle" => {
            for i in 0..8 {
                let a = i as f64 * std::f64::consts::PI / 4.0;
                set(l, c.x + (a.cos() * 3.4).round() as i32, c.y + (a.sin() * 3.4).round() as i32, "menhir");
            }
            set(l, c.x, c.y, "shrine");
        }
        "ruins" => {
            let (rx0, ry0, rw, rh) = (c.x - 3, c.y - 3, 7, 7);
            let gate = r.int_n(4);
            for y in ry0..ry0 + rh {
                for x in rx0..rx0 + rw {
                    let border = x == rx0 || y == ry0 || x == rx0 + rw - 1 || y == ry0 + rh - 1;
                    if border {
                        let is_gate = (gate == 0 && y == ry0 && x == c.x)
                            || (gate == 1 && y == ry0 + rh - 1 && x == c.x)
                            || (gate == 2 && x == rx0 && y == c.y)
                            || (gate == 3 && x == rx0 + rw - 1 && y == c.y);
                        if is_gate || r.f64() >= 0.7 {
                            set(l, x, y, "rubble");
                        } else {
                            set(l, x, y, "ruin_wall");
                        }
                    } else if r.f64() < 0.25 {
                        set(l, x, y, "rubble");
                    }
                }
            }
            set(l, rx0 + 1, ry0 + 1, "pillar");
            set(l, c.x, c.y, "chest");
            for _ in 0..4 {
                let (x, y) = (x0 + r.int_n(w), y0 + r.int_n(h));
                set(l, x, y, "rubble");
            }
        }
        "graveyard" => {
            let mut y = y0 + 1;
            while y < y0 + h - 1 {
                let mut x = x0 + 1;
                while x < x0 + w - 1 {
                    if r.f64() < 0.75 {
                        set(l, x, y, "gravestone");
                    }
                    x += 2;
                }
                y += 2;
            }
            set(l, x0, y0, "dead_tree");
            set(l, x0 + w - 1, y0 + h - 1, "dead_tree");
        }
        "camp" => {
            set(l, c.x, c.y, "campfire");
            for d in [(-3, -3), (3, -3), (-3, 3), (3, 3)] {
                if r.int_n(4) > 0 {
                    set(l, c.x + d.0, c.y + d.1, "tent");
                }
            }
            set(l, c.x, c.y - 2, "chest");
        }
        "oasis" => {
            for y in y0..y0 + h {
                for x in x0..x0 + w {
                    let (dx, dy) = ((x - c.x) as f64 / 3.6, (y - c.y) as f64 / 2.8);
                    let d = dx * dx + dy * dy;
                    if d <= 1.0 {
                        set(l, x, y, "water");
                    } else if d <= 2.6 && r.f64() < 0.3 {
                        set(l, x, y, "palm");
                    } else if d <= 2.6 {
                        set(l, x, y, "grass");
                        if r.f64() < 0.3 {
                            set(l, x, y, "tall_grass");
                        }
                    }
                }
            }
        }
        _ => {}
    }
}
