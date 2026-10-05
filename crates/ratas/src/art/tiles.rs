//! Tile art: grounds (16x16, some animated), objects drawn over a ground
//! (16x16 or tall 16x24) and walls (3/4 view blocks, 16x24).

use std::f64::consts::PI;

use ratas_core::content::TileDef;
use ratas_core::rng::Rng;

use super::*;

/// What a tile looks like.
pub struct TileArt {
    /// [variant][animation frame]
    pub ground: Vec<Vec<Pc>>,
    /// variants drawn over a ground tile
    pub object: Vec<Pc>,
    /// the object is 16x24 and depth-sorted with creatures
    pub tall: bool,
    /// a tall block that fully covers its tile
    pub wall: bool,
}

pub fn tile_colors(def: &TileDef) -> (Rgba, Rgba) {
    let fg = hex(&def.fg);
    let bg = if def.bg.is_empty() {
        mul(fg, 0.3)
    } else {
        hex(&def.bg)
    };
    (fg, bg)
}

pub const ANIMATED: &[&str] = &["water", "deep_water", "lava"];

const KNOWN_GROUND: &[&str] = &[
    "grass",
    "grass2",
    "tall_grass",
    "flowers",
    "forest_floor",
    "sand",
    "swamp",
    "reeds",
    "hill",
    "road",
    "bridge",
    "water",
    "deep_water",
    "lava",
    "house_floor",
    "stone_floor",
    "rubble",
    "bones",
    "cave_floor",
    "door_open",
    "stairs_down",
    "stairs_up",
    "dungeon",
    "void",
    "desert_sand",
    "dunes",
    "snow_ground",
    "snowdrift",
    "ice",
    "ash",
    "magma_crack",
    "blight_grass",
    "mushrooms",
    "web",
    "ice_floor",
    "basalt_floor",
    "sandstone_floor",
    "dark_floor",
    "carpet",
    "cobblestone",
    "city_gate",
    "garden",
];

pub fn is_known_ground(k: &str) -> bool {
    KNOWN_GROUND.contains(&k)
}

/// Builds the art of a tile type. None means "unknown tile": the caller
/// draws a coloured block with the tile's glyph.
pub fn tile_art(def: &TileDef) -> Option<TileArt> {
    let (fg, bg) = tile_colors(def);
    if let Some((first, tall, wall)) = object_art(&def.key, fg, bg, 0) {
        let mut object = vec![first];
        for v in 1..3 {
            object.push(object_art(&def.key, fg, bg, v).unwrap().0);
        }
        return Some(TileArt {
            ground: vec![],
            object,
            tall,
            wall,
        });
    }
    if is_known_ground(&def.key) {
        let frames = if ANIMATED.contains(&def.key.as_str()) {
            4
        } else {
            1
        };
        let ground = (0..3)
            .map(|v| {
                (0..frames)
                    .map(|f| ground_art(&def.key, fg, bg, v, f))
                    .collect()
            })
            .collect();
        return Some(TileArt {
            ground,
            object: vec![],
            tall: false,
            wall: false,
        });
    }
    None
}

/// Picks the ground drawn under an object tile.
pub fn base_ground(def: &TileDef, theme: &str) -> &'static str {
    match def.biome.as_str() {
        "forest" => return "forest_floor",
        "plains" => return "grass",
        "swamp" => return "swamp",
        "hills" | "snow" => return "hill",
        "sand" => return "sand",
        "desert" => return "desert_sand",
        "tundra" => return "snow_ground",
        "ash" => return "ash",
        "cursed" => return "blight_grass",
        _ => {}
    }
    match theme {
        "crypt" => "stone_floor",
        "cave" => "cave_floor",
        "ice" => "ice_floor",
        "volcano" => "basalt_floor",
        "temple" => "sandstone_floor",
        "fortress" => "dark_floor",
        _ => "grass",
    }
}

fn grass_base(p: &mut Pc, fg: Rgba, blades: i32) {
    p.fill(mul(fg, 0.46));
    p.noise(0.07);
    p.speckle(mul(fg, 0.56), 0.18);
    p.speckle(mul(fg, 0.7), 0.05);
    for _ in 0..blades {
        let (x, y) = (p.ri(SPX), 2 + p.ri(SPX - 3));
        let c = mul(fg, 0.85 + p.rf() * 0.35);
        p.set(x, y, c);
        p.set(x, y - 1, mul(c, 1.15));
    }
}

fn water_frame(p: &mut Pc, fg: Rgba, bg: Rgba, frame: i32, deep: bool) {
    p.fill(bg);
    p.noise(0.08);
    let n = if deep { 3 } else { 5 };
    let mut rr = Rng::new(7, 11);
    for _ in 0..n {
        let x = rr.int_n(SPX);
        let y = rr.int_n(SPX);
        let x = (x + frame * 2 + y / 4) % SPX;
        let c = alpha(mix(bg, fg, 0.85), 220);
        for k in 0..3 {
            p.set((x + k) % SPX, y, c);
        }
        p.set((x + 1) % SPX, (y + SPX - 1) % SPX, alpha(WHITE, 70));
    }
}

fn lava_frame(p: &mut Pc, fg: Rgba, bg: Rgba, frame: i32) {
    p.fill(bg);
    let mut rr = Rng::new(3, 5);
    for i in 0..7 {
        let (x, y) = (rr.int_n(SPX), rr.int_n(SPX));
        let r = 1.5 + rr.f64() * 2.0;
        let ph = (frame as f64 * PI / 2.0 + i as f64).sin();
        p.circle(x as f64, y as f64, r + ph * 0.6, mul(fg, 0.9 + 0.2 * ph));
    }
    for _ in 0..5 {
        let (x, y) = (rr.int_n(SPX), rr.int_n(SPX));
        p.set((x + frame) % SPX, y, rgb(255, 230, 120));
    }
}

fn planks(p: &mut Pc, c: Rgba, vertical: bool) {
    for y in 0..SPX {
        for x in 0..SPX {
            let k = if vertical { x } else { y };
            let board = k / 4;
            let shade = 0.9 + 0.12 * (board % 2) as f64;
            let mut col = mul(c, shade + (p.rf() - 0.5) * 0.08);
            if k % 4 == 3 {
                col = mul(c, 0.55);
            }
            p.set(x, y, col);
        }
    }
}

fn flagstones(p: &mut Pc, c: Rgba) {
    for by in 0..2 {
        for bx in 0..2 {
            let sh = 0.85 + p.rf() * 0.25;
            p.rect(bx * 8, by * 8, 8, 8, mul(c, sh));
        }
    }
    p.noise(0.14);
    let mortar = mul(c, 0.5);
    for i in 0..SPX {
        p.set(i, 7, mortar);
        p.set(7, i, mortar);
        p.set(i, 15, mul(c, 0.6));
        p.set(15, i, mul(c, 0.6));
    }
}

pub fn ground_art(key: &str, fg: Rgba, bg: Rgba, v: i32, frame: i32) -> Pc {
    let mut p = Pc::new(SPX, SPX, &format!("{key}{v}"));
    let p = &mut p;
    match key {
        "grass" | "grass2" => grass_base(p, fg, 4),
        "tall_grass" => {
            grass_base(p, fg, 2);
            for _ in 0..9 {
                let (x, y) = (p.ri(SPX), 4 + p.ri(SPX - 4));
                for k in 0..3 {
                    p.set(x, y - k, mul(fg, 0.75 + 0.15 * k as f64));
                }
            }
        }
        "flowers" => {
            grass_base(p, hex("#5fa043"), 3);
            for i in 0..3 {
                let (x, y) = (2 + p.ri(12), 2 + p.ri(12));
                let petal = if i == 1 { rgb(240, 240, 255) } else { fg };
                p.set(x - 1, y, petal);
                p.set(x + 1, y, petal);
                p.set(x, y - 1, petal);
                p.set(x, y + 1, petal);
                p.set(x, y, rgb(255, 220, 60));
            }
        }
        "forest_floor" => {
            p.fill(mix(mul(fg, 0.5), rgb(60, 45, 25), 0.35));
            p.noise(0.2);
            p.speckle(rgb(90, 70, 40), 0.08);
            p.speckle(mul(fg, 0.8), 0.07);
        }
        "sand" => {
            p.fill(mul(fg, 0.82));
            p.noise(0.1);
            p.speckle(mul(fg, 0.95), 0.12);
            p.speckle(mul(fg, 0.65), 0.05);
        }
        "swamp" | "reeds" => {
            p.fill(mix(bg, mul(fg, 0.45), 0.5));
            p.noise(0.18);
            for _ in 0..2 {
                let (x, y) = (2 + p.ri(11), 2 + p.ri(11));
                p.rect(x, y, 3, 2, rgb(28, 48, 44));
                p.set(x, y, rgb(70, 100, 90));
            }
            p.speckle(mul(fg, 0.75), 0.07);
            if key == "reeds" {
                for _ in 0..6 {
                    let (x, y) = (1 + p.ri(14), 7 + p.ri(8));
                    let h = 4 + p.ri(4);
                    p.line(x, y, x, y - h, mul(fg, 0.9));
                    p.set(x, y - h, rgb(110, 80, 45));
                }
            }
        }
        "hill" => {
            p.fill(mix(mul(fg, 0.5), rgb(60, 90, 45), 0.45));
            p.noise(0.14);
            for y in 0..SPX {
                for x in 0..SPX {
                    let (dx, dy) = ((x as f64 - 7.5) / 7.0, (y as f64 - 9.0) / 5.0);
                    let d = dx * dx + dy * dy;
                    if d < 1.0 {
                        p.set(
                            x,
                            y,
                            alpha(mul(fg, 0.95 - 0.35 * dy), (60.0 * (1.0 - d)) as u8),
                        );
                    }
                }
            }
        }
        "road" => {
            p.fill(mul(fg, 0.6));
            p.noise(0.16);
            p.speckle(mul(fg, 0.8), 0.06);
            p.speckle(mul(fg, 0.4), 0.05);
        }
        "cobblestone" | "city_gate" => {
            p.fill(mul(fg, 0.38));
            for row in 0..4 {
                let off = (row % 2) * 2;
                for col in -1..4 {
                    let cx = (col * 4 + off) as f64 + 1.5;
                    let cy = (row * 4) as f64 + 1.5;
                    let sh = 0.7 + p.rf() * 0.3;
                    p.ball(cx, cy, 1.7, mul(fg, sh), 0.15);
                }
            }
            if key == "city_gate" {
                for x in 0..SPX {
                    p.set(x, 0, mul(bg, 1.6));
                    p.set(x, SPX - 1, mul(bg, 1.6));
                }
                p.rect(0, 0, 2, SPX, mul(fg, 0.5));
                p.rect(SPX - 2, 0, 2, SPX, mul(fg, 0.5));
            }
        }
        "garden" => {
            grass_base(p, hex("#5a9a3a"), 4);
            let flowers = [
                hex("#ff7a9a"),
                hex("#ffe070"),
                hex("#b080ff"),
                hex("#ffffff"),
            ];
            for i in 0..7 {
                let (x, y) = (1 + p.ri(14), 1 + p.ri(14));
                p.set(x, y, flowers[((i + v) % 4) as usize]);
                p.set(x, y + 1, hex("#3a7a2a"));
            }
            for i in 0..SPX {
                p.set(i, 0, hex("#8a6a4a"));
                p.set(i, SPX - 1, hex("#8a6a4a"));
            }
        }
        "bridge" => {
            planks(p, fg, false);
            p.rect(0, 0, SPX, 1, mul(fg, 0.45));
            p.rect(0, SPX - 1, SPX, 1, mul(fg, 0.45));
        }
        "water" | "deep_water" => water_frame(p, fg, bg, frame, key == "deep_water"),
        "lava" => lava_frame(p, fg, bg, frame),
        "house_floor" => planks(p, fg, true),
        "stone_floor" | "rubble" | "bones" => {
            flagstones(p, mul(STONE, 0.55));
            if key == "rubble" {
                for _ in 0..5 {
                    let (x, y) = (p.ri(14), p.ri(14));
                    let k = 0.9 + p.rf() * 0.3;
                    p.rect(x, y, 2, 2, mul(fg, k));
                    p.set(x, y, mul(fg, 1.3));
                }
            }
            if key == "bones" {
                let bone = rgb(220, 220, 205);
                p.line(3, 11, 10, 8, bone);
                p.set(2, 11, bone);
                p.set(3, 12, bone);
                p.set(10, 7, bone);
                p.set(11, 8, bone);
                p.circle(11.5, 12.5, 2.2, bone);
                p.set(11, 12, BLACK);
                p.set(12, 12, BLACK);
            }
        }
        "cave_floor" => {
            p.fill(mul(fg, 0.62));
            p.noise(0.22);
            p.speckle(mul(fg, 0.42), 0.08);
            p.speckle(mul(fg, 0.85), 0.04);
        }
        "door_open" => {
            planks(p, rgb(120, 92, 60), true);
            p.rect(0, 0, 2, SPX, mul(fg, 0.55));
            p.rect(14, 0, 2, SPX, mul(fg, 0.55));
        }
        "stairs_down" | "stairs_up" => {
            p.fill(mul(STONE, 0.45));
            for i in 0..4 {
                let mut k = i as f64 / 3.0;
                if key == "stairs_down" {
                    k = 1.0 - k;
                }
                let c = mul(STONE, 0.35 + 0.6 * k);
                p.rect(2 + i, 2 + i * 3, 12 - 2 * i, 3, c);
                p.rect(2 + i, 2 + i * 3, 12 - 2 * i, 1, mul(c, 1.25));
            }
        }
        "dungeon" => {
            grass_base(p, hex("#4c8a35"), 2);
            p.circle(8.0, 9.0, 6.0, rgb(60, 52, 48));
            p.circle(8.0, 9.5, 4.6, rgb(10, 6, 6));
            for x in 4..=12 {
                p.set(x, 13, alpha(fg, 200));
            }
        }
        "void" => p.fill(BLACK),
        "desert_sand" | "dunes" => {
            p.fill(mul(fg, 0.86));
            p.noise(0.06);
            let ph = v as f64 * 2.0;
            for y in 0..SPX {
                for x in 0..SPX {
                    let (fx, fy) = (x as f64, y as f64);
                    let w = if key == "dunes" {
                        ((fx * 0.25 + fy * 0.7 + ph) * 0.8).sin() * 0.9
                    } else {
                        ((fx * 0.45 + fy * 1.1 + ph) * 0.9).sin() * 0.5
                    };
                    if w > 0.35 {
                        p.set(x, y, mul(fg, 0.97 + w * 0.12));
                    } else if w < -0.42 {
                        p.set(x, y, mul(fg, 0.74));
                    }
                }
            }
            p.speckle(mul(fg, 0.62), 0.03);
        }
        "snow_ground" | "snowdrift" => {
            p.fill(mul(fg, 0.93));
            p.noise(0.04);
            p.speckle(rgb(196, 212, 236), 0.08);
            p.speckle(WHITE, 0.03);
            if key == "snowdrift" {
                for y in 0..SPX {
                    for x in 0..SPX {
                        let (dx, dy) = ((x as f64 - 7.5) / 7.0, (y as f64 - 9.0) / 5.0);
                        if dx * dx + dy * dy < 1.0 {
                            p.set(x, y, mul(fg, 1.0 - 0.15 * dy - 0.08 * dx));
                        }
                    }
                }
                p.speckle(rgb(200, 216, 240), 0.04);
            }
        }
        "ice" | "ice_floor" => {
            let base = if key == "ice_floor" {
                mul(fg, 0.78)
            } else {
                mix(fg, bg, 0.45)
            };
            p.fill(base);
            p.noise(0.05);
            for _ in 0..3 {
                let (x0, y0) = (p.ri(SPX), p.ri(SPX));
                let (dx, dy) = (p.ri(9) - 4, p.ri(9) - 4);
                p.line(x0, y0, x0 + dx, y0 + dy, mix(base, WHITE, 0.45));
            }
            for _ in 0..4 {
                let x = p.ri(SPX - 3);
                let y = p.ri(SPX - 3);
                p.set(x, y + 2, alpha(WHITE, 140));
                p.set(x + 1, y + 1, alpha(WHITE, 170));
                p.set(x + 2, y, alpha(WHITE, 140));
            }
        }
        "ash" | "magma_crack" => {
            p.fill(mul(fg, 0.72));
            p.noise(0.18);
            p.speckle(mul(fg, 0.5), 0.1);
            p.speckle(mul(fg, 1.05), 0.06);
            if v == 1 {
                let (x, y) = (p.ri(SPX), p.ri(SPX));
                p.set(x, y, rgb(255, 120, 40));
            }
            if key == "magma_crack" {
                let (mut x, mut y) = (p.ri(4), p.ri(SPX));
                for _ in 0..14 {
                    let c = rgb(255, 110 + p.ri(80) as u8, 30);
                    p.set(x, y, c);
                    p.set(x, y + 1, mul(c, 0.6));
                    x += 1;
                    y += p.ri(3) - 1;
                }
            }
        }
        "blight_grass" | "mushrooms" => {
            grass_base(p, hex("#6a6070"), 3);
            p.speckle(rgb(40, 30, 44), 0.06);
            if key == "mushrooms" {
                for _ in 0..3 {
                    let (x, y) = (2 + p.ri(11), 4 + p.ri(10));
                    p.set(x, y, rgb(230, 225, 210));
                    p.set(x, y - 1, rgb(230, 225, 210));
                    p.set(x - 1, y - 2, fg);
                    p.set(x, y - 2, mix(fg, WHITE, 0.4));
                    p.set(x + 1, y - 2, mul(fg, 0.8));
                }
            }
        }
        "web" => {
            p.fill(rgb(58, 48, 42));
            p.noise(0.2);
            let wc = alpha(fg, 200);
            for i in 0..4 {
                let a = i as f64 * PI / 4.0 + v as f64;
                let (c, s) = ((a.cos() * 8.0) as i32, (a.sin() * 8.0) as i32);
                p.line(8 - c, 8 - s, 8 + c, 8 + s, wc);
            }
            let mut r = 2.5;
            while r < 8.0 {
                for i in 0..16 {
                    let a = i as f64 / 16.0 * 2.0 * PI;
                    p.set(
                        8 + (a.cos() * r) as i32,
                        8 + (a.sin() * r) as i32,
                        alpha(fg, 150),
                    );
                }
                r += 2.5;
            }
        }
        "basalt_floor" => {
            p.fill(mul(fg, 0.7));
            p.noise(0.16);
            for _ in 0..3 {
                let (x, y) = (p.ri(SPX), p.ri(SPX));
                p.line(x, y, x + 3, y + 1, mul(fg, 0.45));
            }
            if v == 2 {
                let (x, y) = (p.ri(SPX), p.ri(SPX));
                p.set(x, y, rgb(220, 80, 30));
            }
        }
        "sandstone_floor" | "dark_floor" => flagstones(p, mul(fg, 0.85)),
        "carpet" => {
            flagstones(p, hex("#4a3c4c"));
            p.rect(0, 2, SPX, 12, mul(fg, 0.8));
            p.noise(0.05);
            let gold = rgb(220, 180, 80);
            p.rect(0, 3, SPX, 1, gold);
            p.rect(0, 12, SPX, 1, gold);
            let mut x = 2;
            while x < SPX {
                p.set(x, 7, gold);
                p.set(x + 1, 8, gold);
                x += 5;
            }
        }
        _ => {
            p.fill(bg);
            p.noise(0.1);
        }
    }
    std::mem::replace(p, Pc::new(0, 0, ""))
}

/// Draws an object with a transparent background: (art, tall, wall).
pub fn object_art(key: &str, fg: Rgba, bg: Rgba, v: i32) -> Option<(Pc, bool, bool)> {
    let seed = format!("{key}{v}");
    let r = match key {
        "tree" => {
            let mut p = Pc::new(SPX, 24, &seed);
            p.circle(8.0, 22.0, 4.5, alpha(BLACK, 70));
            p.rect(7, 15, 2, 8, BARK);
            p.rect(8, 15, 1, 8, mul(BARK, 0.7));
            let r = 6.2 + (v % 3) as f64 * 0.5;
            p.ball(8.0, 9.0 + (v % 2) as f64, r, mul(fg, 0.95), 0.25);
            p.ball(5.5, 7.0, r * 0.55, mul(fg, 1.05), 0.2);
            for _ in 0..6 {
                let (x, y) = (3 + p.ri(10), 3 + p.ri(10));
                if p.get(x, y).a > 0 {
                    p.set(x, y, mul(fg, 1.35));
                }
            }
            (p, true, false)
        }
        "pine" => {
            let mut p = Pc::new(SPX, 24, &seed);
            p.circle(8.0, 22.0, 4.0, alpha(BLACK, 70));
            p.rect(7, 18, 2, 5, BARK);
            for t in [[1, 7, 3], [6, 13, 5], [11, 19, 7]] {
                for y in t[0]..=t[1] {
                    let span = (t[1] - t[0] + 1) as f64;
                    let w = (y - t[0] + 1) as f64 / span * t[2] as f64;
                    for x in 0..SPX {
                        let d = x as f64 + 0.5 - 8.0;
                        if d.abs() <= w {
                            let l = 1.05
                                - 0.45 * (d / w.max(1.0)) * 0.6
                                - 0.15 * (y - t[0]) as f64 / span;
                            let j = (p.rf() - 0.5) * 0.15;
                            p.set(x, y, mul(fg, l + j));
                        }
                    }
                }
            }
            p.set(8, 0, mul(fg, 1.3));
            (p, true, false)
        }
        "dead_tree" => {
            let mut p = Pc::new(SPX, 24, key);
            let c = mul(fg, 0.9);
            p.circle(8.0, 22.0, 3.5, alpha(BLACK, 60));
            p.rect(7, 8, 2, 15, c);
            p.line(8, 12, 3, 6, c);
            p.line(8, 10, 13, 4, c);
            p.line(4, 7, 3, 3, c);
            p.line(12, 5, 14, 2, c);
            (p, true, false)
        }
        "bush" => {
            let mut p = Pc::new(SPX, SPX, &seed);
            p.circle(8.0, 13.0, 5.0, alpha(BLACK, 60));
            p.ball(8.0, 9.0, 5.5, mul(fg, 0.95), 0.3);
            p.ball(5.0, 10.0, 3.5, mul(fg, 0.9), 0.3);
            p.ball(11.0, 10.0, 3.5, mul(fg, 0.85), 0.3);
            if v % 2 == 0 {
                p.set(6, 8, rgb(200, 40, 60));
                p.set(10, 11, rgb(200, 40, 60));
            }
            (p, false, false)
        }
        "mountain" | "snow" | "sandstone" | "ice_rock" | "basalt" => {
            let mut p = Pc::new(SPX, 24, &seed);
            let peak = 2 + v % 3;
            let rock = match key {
                "snow" => mul(STONE, 0.95),
                "sandstone" | "ice_rock" | "basalt" => fg,
                _ => mul(STONE, 0.8),
            };
            for y in peak..24 {
                let half = (y - peak) as f64 / (24 - peak) as f64 * 8.5;
                for x in 0..SPX {
                    let d = x as f64 + 0.5 - 8.0;
                    if d.abs() > half {
                        continue;
                    }
                    let l = if d > 0.0 { 0.62 } else { 1.0 };
                    let mut c = mul(rock, l + (p.rf() - 0.5) * 0.12);
                    let snow_line = match key {
                        "snow" => peak + 12,
                        "ice_rock" => peak + 3,
                        "sandstone" | "basalt" => {
                            if key == "sandstone" && (y - peak) % 5 == 0 {
                                c = mul(c, 0.8);
                            }
                            -1
                        }
                        _ => peak + 5,
                    };
                    if y < snow_line {
                        c = mul(WHITE, l * 0.98);
                    }
                    p.set(x, y, c);
                }
            }
            p.line(8, peak + 4, 6, peak + 11, mul(rock, 0.55));
            if key == "basalt" && v == 1 {
                p.line(9, peak + 8, 11, peak + 15, rgb(255, 100, 30));
            }
            (p, true, false)
        }
        "fountain" => {
            let mut p = Pc::new(SPX, SPX, &seed);
            p.circle(8.0, 9.0, 7.5, mul(hex("#b0a898"), 0.85));
            p.circle(8.0, 9.0, 6.0, hex("#c8c0b0"));
            p.circle(8.0, 9.0, 5.0, hex("#2a5a8a"));
            p.circle(7.0, 8.0, 3.5, hex("#3a7ab0"));
            p.rect(7, 3, 2, 7, hex("#d8d0c0"));
            p.circle(8.0, 3.0, 1.6, alpha(hex("#c0e8ff"), 220));
            for i in 0..4 {
                p.set(4 + i * 2 + v % 2, 6 + i % 2, alpha(WHITE, 200));
            }
            (p, false, false)
        }
        "market_stall" => {
            let mut p = Pc::new(SPX, 24, &seed);
            let awn = [hex("#c03a2a"), hex("#2a6ac0"), hex("#3a9a3a")][(v % 3) as usize];
            p.circle(8.0, 22.0, 6.0, alpha(BLACK, 70));
            p.rect(2, 6, 1, 16, BARK);
            p.rect(13, 6, 1, 16, BARK);
            for x in 0..SPX {
                let c = if (x / 2) % 2 == 1 {
                    hex("#f0e8d8")
                } else {
                    awn
                };
                p.rect(x, 2, 1, 5, c);
            }
            p.rect(1, 15, 14, 5, mul(BARK, 1.2));
            p.rect(1, 15, 14, 1, mul(BARK, 1.5));
            let goods = [
                hex("#e0402a"),
                hex("#ffd040"),
                hex("#80c040"),
                hex("#c08040"),
            ];
            for i in 0..5 {
                let x = 3 + i * 2 + p.ri(2);
                p.ball(x as f64, 13.5, 1.2, goods[((i + v) % 4) as usize], 0.1);
            }
            (p, true, false)
        }
        "lamp_post" => {
            let mut p = Pc::new(SPX, 24, key);
            p.circle(8.0, 22.0, 3.0, alpha(BLACK, 80));
            p.rect(7, 6, 2, 17, hex("#2a2a30"));
            p.rect(6, 21, 4, 2, hex("#3a3a40"));
            p.rect(5, 2, 6, 5, hex("#3a3a40"));
            p.rect(6, 3, 4, 3, hex("#ffe0a0"));
            p.set(7, 4, WHITE);
            (p, true, false)
        }
        "statue" => {
            let mut p = Pc::new(SPX, 24, &seed);
            let st = hex("#d0d0c8");
            p.circle(8.0, 22.0, 6.0, alpha(BLACK, 80));
            p.rect(3, 17, 10, 6, mul(st, 0.7));
            p.rect(3, 17, 10, 1, mul(st, 0.9));
            p.rect(6, 7, 4, 10, st);
            p.circle(8.0, 5.0, 2.5, st);
            p.rect(4, 8, 2, 6, mul(st, 0.85));
            p.line(11, 3, 11, 15, mul(st, 0.8));
            p.rect(10, 8, 2, 2, mul(st, 0.85));
            (p, true, false)
        }
        "well" => {
            let mut p = Pc::new(SPX, SPX, key);
            p.circle(8.0, 9.0, 6.5, mul(STONE, 0.75));
            p.circle(8.0, 9.0, 4.5, rgb(20, 40, 70));
            p.circle(7.0, 8.0, 1.2, alpha(fg, 160));
            p.rect(1, 2, 14, 2, BARK);
            p.rect(2, 2, 1, 7, BARK);
            p.rect(13, 2, 1, 7, BARK);
            (p, false, false)
        }
        "fence" => {
            let mut p = Pc::new(SPX, SPX, key);
            p.rect(0, 7, SPX, 2, fg);
            p.rect(0, 11, SPX, 1, mul(fg, 0.8));
            p.rect(2, 4, 2, 10, mul(fg, 1.1));
            p.rect(12, 4, 2, 10, mul(fg, 1.1));
            (p, false, false)
        }
        "chest" | "chest_open" => {
            let mut p = Pc::new(SPX, SPX, key);
            let wood = rgb(120, 80, 40);
            p.circle(8.0, 14.0, 6.0, alpha(BLACK, 70));
            if key == "chest" {
                p.rect(2, 5, 12, 9, wood);
                p.rect(2, 5, 12, 3, mul(wood, 1.25));
                p.rect(2, 8, 12, 1, mul(wood, 0.5));
                p.rect(4, 5, 1, 9, fg);
                p.rect(11, 5, 1, 9, fg);
                p.rect(7, 8, 2, 3, fg);
            } else {
                p.rect(2, 2, 12, 4, mul(wood, 0.8));
                p.rect(2, 7, 12, 7, wood);
                p.rect(3, 7, 10, 3, rgb(20, 12, 8));
                p.rect(4, 7, 1, 7, fg);
                p.rect(11, 7, 1, 7, fg);
            }
            (p, false, false)
        }
        "altar" => {
            let mut p = Pc::new(SPX, SPX, key);
            p.rect(1, 6, 14, 8, mul(fg, 0.45));
            p.rect(1, 6, 14, 2, mul(fg, 0.7));
            p.set(3, 4, rgb(255, 220, 120));
            p.set(12, 4, rgb(255, 220, 120));
            p.rect(3, 5, 1, 1, WHITE);
            p.rect(12, 5, 1, 1, WHITE);
            p.circle(8.0, 10.0, 2.0, fg);
            (p, false, false)
        }
        "pillar" => {
            let mut p = Pc::new(SPX, 24, key);
            p.circle(8.0, 22.0, 5.0, alpha(BLACK, 70));
            for y in 4..22 {
                for x in 4..12 {
                    let l = 1.1 - 0.09 * (x - 4) as f64;
                    p.set(x, y, mul(fg, l * 0.85));
                }
            }
            p.rect(3, 2, 10, 3, fg);
            p.rect(3, 20, 10, 3, mul(fg, 0.75));
            (p, true, false)
        }
        "stone_wall" | "cave_wall" | "house_wall" | "door" | "ice_wall" | "basalt_wall"
        | "sandstone_wall" | "dark_wall" | "ruin_wall" | "city_wall" => {
            (wall_art(key, fg, bg, v), true, true)
        }
        "snow_pine" => {
            let (mut p, _, _) = object_art("pine", fg, bg, v)?;
            for y in 0..22 {
                for x in 0..SPX {
                    let c = p.get(x, y);
                    if c.a == 0 || c == BARK || y > 18 {
                        continue;
                    }
                    if p.get(x, y - 1).a == 0 || (y % 5 == 1 && (x + y) % 3 == 0) {
                        p.set(x, y, rgb(236, 242, 255));
                    }
                }
            }
            (p, true, false)
        }
        "charred_tree" => {
            let (mut p, _, _) = object_art("dead_tree", fg, bg, v)?;
            p.set(7, 14, rgb(255, 120, 40));
            p.set(8, 18, rgb(255, 90, 30));
            (p, true, false)
        }
        "twisted_tree" => {
            let mut p = Pc::new(SPX, 24, &seed);
            let trunk = rgb(60, 44, 56);
            p.circle(8.0, 22.0, 4.5, alpha(BLACK, 70));
            p.thick(8.0, 23.0, 7.0, 16.0, 2.0, trunk);
            p.thick(7.0, 16.0, 9.0, 11.0, 2.0, mul(trunk, 0.9));
            p.thick(9.0, 12.0, 4.0, 8.0, 1.0, trunk);
            p.thick(8.0, 12.0, 13.0, 9.0, 1.0, mul(trunk, 0.8));
            p.ball(5.0, 6.0, 3.6, fg, 0.3);
            p.ball(11.0, 7.0, 3.4, mul(fg, 0.85), 0.3);
            p.ball(8.0, 4.0, 3.0, mul(fg, 1.1), 0.3);
            p.set(7, 15, rgb(200, 255, 120));
            p.set(9, 15, rgb(200, 255, 120));
            (p, true, false)
        }
        "palm" => {
            let mut p = Pc::new(SPX, 24, &seed);
            p.circle(8.0, 22.0, 4.0, alpha(BLACK, 70));
            for y in 7..23 {
                let x = 7 + ((y as f64 * 0.25 + v as f64).sin() * 1.5) as i32;
                let mut c = rgb(150, 110, 60);
                if y % 3 == 0 {
                    c = mul(c, 0.75);
                }
                p.set(x, y, c);
                p.set(x + 1, y, mul(c, 0.8));
            }
            let top = 7 + ((7.0 * 0.25 + v as f64).sin() * 1.5) as i32;
            for i in 0..6 {
                let a = i as f64 / 6.0 * 2.0 * PI + 0.4;
                let mut t = 1.0;
                while t < 6.5 {
                    let x = top as f64 + 1.0 + a.cos() * t;
                    let y = 7.0 + a.sin() * t * 0.6 + t * t * 0.06;
                    p.set(x as i32, y as i32, mul(fg, 1.05 - t * 0.05));
                    t += 0.5;
                }
            }
            p.set(top + 1, 8, rgb(110, 70, 30));
            p.set(top, 9, rgb(110, 70, 30));
            (p, true, false)
        }
        "cactus" => {
            let mut p = Pc::new(SPX, 24, &seed);
            p.circle(8.0, 22.0, 3.5, alpha(BLACK, 70));
            let col = fg;
            p.rect(7, 8, 3, 15, col);
            p.rect(7, 8, 1, 15, mul(col, 1.2));
            p.rect(9, 8, 1, 15, mul(col, 0.7));
            p.rect(3, 12, 2, 4, col);
            p.rect(3, 15, 4, 2, col);
            p.rect(12, 10, 2, 5, mul(col, 0.85));
            p.rect(10, 14, 3, 2, mul(col, 0.85));
            p.set(8, 7, mul(col, 1.1));
            let mut y = 9;
            while y < 22 {
                p.set(8, y, alpha(WHITE, 120));
                y += 3;
            }
            if v == 1 {
                p.set(8, 7, rgb(240, 100, 160));
            }
            (p, true, false)
        }
        "gravestone" => {
            let mut p = Pc::new(SPX, SPX, &seed);
            p.circle(8.0, 14.0, 5.0, alpha(BLACK, 60));
            let st = fg;
            p.rect(4, 4, 8, 10, st);
            p.rect(5, 3, 6, 1, st);
            p.rect(4, 4, 1, 10, mul(st, 1.15));
            p.rect(11, 4, 1, 10, mul(st, 0.7));
            p.rect(7, 6, 2, 6, mul(st, 0.55));
            p.rect(5, 8, 6, 1, mul(st, 0.55));
            p.rect(3, 13, 10, 2, rgb(70, 56, 40));
            if v == 2 {
                p.set(10, 5, rgb(90, 120, 60));
                p.set(11, 6, rgb(90, 120, 60));
            }
            (p, false, false)
        }
        "menhir" => {
            let mut p = Pc::new(SPX, 24, &seed);
            p.circle(8.0, 22.0, 4.5, alpha(BLACK, 70));
            for y in 2 + v..23 {
                let half = 3.2 - (6 - y).max(0) as f64 * 0.4;
                for x in (8.0 - half) as i32..=(8.0 + half) as i32 {
                    let l = 1.1 - 0.12 * (x as f64 - 8.0 + half);
                    let j = (p.rf() - 0.5) * 0.08;
                    p.set(x, y, mul(fg, l + j));
                }
            }
            let mut y = 8;
            while y < 18 {
                p.set(7, y, rgb(140, 200, 255));
                p.set(
                    8,
                    y + 1,
                    Rgba {
                        r: 140,
                        g: 200,
                        b: 255,
                        a: 200,
                    },
                );
                y += 3;
            }
            (p, true, false)
        }
        "shrine" | "shrine_used" => {
            let mut p = Pc::new(SPX, SPX, key);
            p.circle(8.0, 14.0, 6.0, alpha(BLACK, 60));
            let st = rgb(150, 146, 140);
            p.rect(3, 9, 10, 5, st);
            p.rect(3, 9, 10, 1, mul(st, 1.2));
            p.rect(12, 9, 1, 5, mul(st, 0.7));
            p.rect(5, 6, 6, 3, mul(st, 0.9));
            if key == "shrine" {
                p.ball(8.0, 4.0, 2.6, fg, 0.1);
                p.set(7, 3, WHITE);
            } else {
                p.circle(8.0, 5.0, 1.8, mul(fg, 0.6));
            }
            (p, false, false)
        }
        "tent" => {
            let mut p = Pc::new(SPX, 24, &seed);
            p.circle(8.0, 22.0, 6.0, alpha(BLACK, 70));
            for y in 8..23 {
                let half = (y - 8) as f64 * 0.5;
                for x in (8.0 - half) as i32..=(8.0 + half) as i32 {
                    let l = if x > 8 { 0.75 } else { 1.1 };
                    p.set(x, y, mul(fg, l));
                }
            }
            for y in 15..23 {
                let half = (y - 15) as f64 * 0.32;
                for x in (8.0 - half) as i32..=(8.0 + half) as i32 {
                    p.set(x, y, rgb(30, 20, 14));
                }
            }
            p.line(8, 6, 8, 9, BARK);
            (p, true, false)
        }
        "campfire" | "brazier" => {
            let mut p = Pc::new(SPX, SPX, key);
            if key == "campfire" {
                p.circle(8.0, 12.0, 5.0, rgb(60, 56, 50));
                p.thick(3.0, 13.0, 13.0, 10.0, 2.0, BARK);
                p.thick(3.0, 10.0, 13.0, 13.0, 2.0, mul(BARK, 0.8));
            } else {
                p.rect(5, 13, 1, 3, rgb(60, 56, 60));
                p.rect(10, 13, 1, 3, rgb(60, 56, 60));
                p.rect(3, 10, 10, 3, rgb(90, 84, 90));
                p.rect(3, 10, 10, 1, rgb(130, 124, 130));
            }
            p.ball(8.0, 8.0, 3.2, rgb(255, 120, 30), 0.2);
            p.ball(8.0, 7.0, 2.0, rgb(255, 210, 80), 0.1);
            p.set(8, 3, rgb(255, 160, 40));
            p.set(6, 5, rgb(255, 140, 40));
            (p, false, false)
        }
        "crystal" => {
            let mut p = Pc::new(SPX, 24, &seed);
            p.circle(8.0, 22.0, 4.0, alpha(BLACK, 50));
            let shard = |p: &mut Pc, cx: f64, top: f64, bot: f64, half: f64, c: Rgba| {
                for y in top as i32..=bot as i32 {
                    let k = (y as f64 - top) / (bot - top);
                    let w = half * (k * 2.2).min(1.0);
                    for x in (cx - w) as i32..=(cx + w) as i32 {
                        let l = 1.25 - 0.45 * (x as f64 - (cx - w)) / (2.0 * w).max(1.0);
                        p.set(x, y, alpha(mul(c, l), 230));
                    }
                }
            };
            shard(&mut p, 5.0, 11.0, 22.0, 2.0, mul(fg, 0.8));
            shard(&mut p, 11.0, 9.0, 22.0, 2.0, mul(fg, 0.75));
            shard(&mut p, 8.0, 4.0 + v as f64, 22.0, 2.8, fg);
            p.set(7, 8, WHITE);
            (p, true, false)
        }
        "sarcophagus" | "sarcophagus_open" => {
            let mut p = Pc::new(SPX, SPX, key);
            p.circle(8.0, 13.0, 6.0, alpha(BLACK, 70));
            let st = rgb(170, 150, 110);
            p.rect(3, 3, 10, 11, mul(st, 0.8));
            if key == "sarcophagus" {
                p.rect(3, 2, 10, 10, st);
                p.rect(3, 2, 10, 1, mul(st, 1.2));
                p.rect(7, 3, 2, 8, fg);
                p.rect(5, 5, 6, 1, fg);
                p.set(8, 4, rgb(60, 120, 200));
            } else {
                p.rect(4, 4, 8, 8, rgb(24, 18, 14));
                p.rect(9, 1, 6, 9, mul(st, 0.9));
                p.set(6, 9, rgb(220, 220, 200));
            }
            (p, false, false)
        }
        _ => return None,
    };
    Some(r)
}

/// A 3/4 view block: a 16px top face and an 8px front face.
pub fn wall_art(key: &str, fg: Rgba, bg: Rgba, v: i32) -> Pc {
    let mut p = Pc::new(SPX, 24, &format!("{key}{v}"));
    let (top, front) = match key {
        "cave_wall" | "basalt_wall" => (mix(fg, bg, 0.3), mul(bg, 1.1)),
        "ice_wall" => (mul(fg, 0.85), mix(fg, bg, 0.5)),
        "house_wall" | "door" => (mul(fg, 0.8), mul(fg, 0.55)),
        _ => (mul(fg, 0.62), mul(fg, 0.42)),
    };
    let brick = matches!(
        key,
        "stone_wall" | "sandstone_wall" | "dark_wall" | "ruin_wall" | "city_wall"
    );
    p.rect(0, 0, SPX, 16, top);
    p.noise(0.12);
    if brick {
        for i in 0..SPX {
            p.set(i, 0, mul(top, 1.35));
            p.set(i, 8, mul(top, 0.8));
        }
        p.rect(v % 8 + 3, 1, 1, 7, mul(top, 0.8));
        p.rect((v * 5) % 8 + 7, 9, 1, 7, mul(top, 0.8));
        if key == "ruin_wall" {
            p.speckle(rgb(80, 110, 60), 0.08);
        }
    } else if key == "ice_wall" {
        p.speckle(mul(top, 1.15), 0.12);
        p.line(2, 3, 9, 12, alpha(WHITE, 120));
    } else if key == "cave_wall" || key == "basalt_wall" {
        p.speckle(mul(top, 0.7), 0.15);
        p.speckle(mul(top, 1.25), 0.06);
    } else if key == "house_wall" || key == "door" {
        let mut y = 0;
        while y < 16 {
            for x in 0..SPX {
                p.set(x, y, mul(top, 0.75));
            }
            y += 4;
        }
        p.rect(0, 0, SPX, 1, mul(top, 1.3));
    }
    for y in 16..24 {
        for x in 0..SPX {
            let mut c = mul(front, 1.0 - 0.04 * (y - 16) as f64 + (p.rf() - 0.5) * 0.1);
            if brick {
                let off = if (y - 16) / 4 % 2 == 1 { 4 } else { 0 };
                if (y - 16) % 4 == 3 || (x + off) % 8 == 7 {
                    c = mul(front, 0.6);
                }
            } else if key == "ice_wall" {
                if (x + y) % 5 == 0 {
                    c = mix(c, WHITE, 0.25);
                }
            } else if key == "house_wall" {
                if (y - 16) % 3 == 2 {
                    c = mul(front, 0.65);
                }
            } else if key == "door" {
                if !(3..=12).contains(&x) {
                    c = mul(front, 0.7);
                } else {
                    c = mul(rgb(130, 90, 50), 1.0 - 0.04 * (y - 16) as f64);
                    if x == 8 {
                        c = mul(c, 0.6);
                    }
                    if x == 10 && y == 20 {
                        c = rgb(230, 200, 90);
                    }
                }
            }
            p.set(x, y, c);
        }
    }
    for x in 0..SPX {
        p.set(x, 16, mul(front, 1.3));
        p.set(x, 23, mul(front, 0.5));
    }
    if key == "city_wall" {
        let mut x = 0;
        while x < SPX {
            p.rect(x, 0, 2, 3, mul(top, 1.25));
            p.rect(x, 3, 2, 1, mul(top, 0.7));
            x += 4;
        }
    }
    if key == "ruin_wall" {
        for y in 0..6 + v {
            for x in SPX - 1 - (6 - y / 2)..SPX {
                p.clear(x, y);
            }
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_tile_has_art() {
        let db = ratas_core::content::db();
        for def in db.b.tiles.iter() {
            let art = tile_art(def);
            if let Some(a) = &art {
                assert!(!a.ground.is_empty() || !a.object.is_empty(), "{}", def.key);
            } else {
                // unknown tiles fall back to a glyph; builtin ones should be drawn
                panic!("no art for builtin tile {}", def.key);
            }
        }
    }
}
