use super::*;
use crate::world::{Level, Pos, DIRS4};

/// A generated dungeon level plus suggested spawn points.
pub struct DungeonFloor {
    pub level: Level,
    pub monsters: Vec<Pos>,
    pub items: Vec<Pos>,
    pub boss: Option<Pos>,
}

/// The set of tiles a dungeon theme is built from.
struct Palette {
    cave: bool,
    wall: &'static str,
    floor: &'static str,
    pool: &'static str,
    pool3: &'static str,
    deco: &'static [&'static str],
    pillar: &'static str,
    chest: &'static str,
    crystal: &'static str,
    carpet: &'static str,
    pool_threshold: f64,
}

fn palette(theme: &str) -> Palette {
    let base = Palette {
        cave: false,
        wall: "stone_wall",
        floor: "stone_floor",
        pool: "",
        pool3: "",
        deco: &["bones", "rubble"],
        pillar: "pillar",
        chest: "chest",
        crystal: "",
        carpet: "",
        pool_threshold: 0.0,
    };
    match theme {
        "cave" => Palette {
            cave: true,
            wall: "cave_wall",
            floor: "cave_floor",
            pool: "water",
            pool3: "lava",
            deco: &["rubble", "bones", "web"],
            pool_threshold: 0.42,
            ..base
        },
        "ice" => Palette {
            cave: true,
            wall: "ice_wall",
            floor: "ice_floor",
            pool: "ice",
            pool3: "ice",
            deco: &["snowdrift", "bones"],
            crystal: "crystal",
            pool_threshold: 0.36,
            ..base
        },
        "volcano" => Palette {
            cave: true,
            wall: "basalt_wall",
            floor: "basalt_floor",
            pool: "lava",
            pool3: "lava",
            deco: &["magma_crack", "rubble", "bones"],
            pool_threshold: 0.4,
            ..base
        },
        "temple" => Palette {
            wall: "sandstone_wall",
            floor: "sandstone_floor",
            chest: "sarcophagus",
            ..base
        },
        "fortress" => Palette {
            wall: "dark_wall",
            floor: "dark_floor",
            pillar: "brazier",
            carpet: "carpet",
            ..base
        },
        _ => base,
    }
}

/// Every dungeon theme.
pub fn themes() -> &'static [&'static str] {
    &["cave", "crypt", "ice", "volcano", "temple", "fortress"]
}

/// Builds one floor. Room themes (crypt, temple, fortress) use BSP rooms and
/// corridors, cave themes (cave, ice, volcano) use cellular automata. The last
/// floor (depth == max_depth) has a boss lair instead of stairs down.
pub fn generate_dungeon(
    seed: i64,
    id: &str,
    name: &str,
    theme: &str,
    dungeon_idx: i32,
    depth: i32,
    max_depth: i32,
) -> DungeonFloor {
    let mut r = Rng::labeled(seed, id);
    let (w, h) = (64 + depth * 4, 50 + depth * 3);
    let pal = palette(theme);
    let mut f = if pal.cave {
        gen_cave(&mut r, w, h, depth, max_depth, &pal)
    } else {
        gen_crypt(&mut r, w, h, depth, max_depth, &pal)
    };
    f.level.id = id.into();
    f.level.name = format!("{name} — ур. {depth}");
    f.level.theme = theme.into();
    f.level.depth = depth;
    f.level.dungeon = dungeon_idx;
    f
}

// ---------- BSP crypt ----------

struct Bsp {
    r: Rect,
    kids: Option<Box<(Bsp, Bsp)>>,
    room: Option<Rect>,
}

impl Bsp {
    fn new(r: Rect) -> Bsp {
        Bsp {
            r,
            kids: None,
            room: None,
        }
    }

    fn split(&mut self, rng: &mut Rng, min_w: i32, min_h: i32) {
        let mut horiz = rng.int_n(2) == 0;
        if self.r.w > self.r.h * 2 {
            horiz = false;
        } else if self.r.h * 2 > self.r.w * 3 && self.r.h > min_h * 2 {
            horiz = true;
        }
        let r = self.r;
        let (a, b) = if horiz {
            if r.h < min_h * 2 {
                return;
            }
            let cut = rng.range(min_h, r.h - min_h);
            (
                Rect::new(r.x, r.y, r.w, cut),
                Rect::new(r.x, r.y + cut, r.w, r.h - cut),
            )
        } else {
            if r.w < min_w * 2 {
                return;
            }
            let cut = rng.range(min_w, r.w - min_w);
            (
                Rect::new(r.x, r.y, cut, r.h),
                Rect::new(r.x + cut, r.y, r.w - cut, r.h),
            )
        };
        let mut k = Box::new((Bsp::new(a), Bsp::new(b)));
        k.0.split(rng, min_w, min_h);
        k.1.split(rng, min_w, min_h);
        self.kids = Some(k);
    }

    fn rooms(&self, out: &mut Vec<Rect>) {
        if let Some(r) = self.room {
            out.push(r);
        }
        if let Some(k) = &self.kids {
            k.0.rooms(out);
            k.1.rooms(out);
        }
    }

    fn any_room(&self, rng: &mut Rng) -> Option<Rect> {
        let mut rs = Vec::new();
        self.rooms(&mut rs);
        if rs.is_empty() {
            None
        } else {
            Some(*rng.pick(&rs))
        }
    }

    fn carve(&mut self, rng: &mut Rng, l: &mut Level, floor: u8) {
        if let Some(k) = &mut self.kids {
            k.0.carve(rng, l, floor);
            k.1.carve(rng, l, floor);
            return;
        }
        let n = self.r;
        let rw = rng.range(5, n.w - 3);
        let rh = rng.range(4, n.h - 3);
        let rx = rng.range(n.x + 1, n.x + n.w - rw - 1);
        let ry = rng.range(n.y + 1, n.y + n.h - rh - 1);
        let room = Rect::new(rx, ry, rw, rh);
        self.room = Some(room);
        for y in ry..ry + rh {
            for x in rx..rx + rw {
                l.set(x, y, floor);
            }
        }
    }

    fn connect(&self, rng: &mut Rng, l: &mut Level, wall: u8, floor: u8) {
        let Some(k) = &self.kids else { return };
        k.0.connect(rng, l, wall, floor);
        k.1.connect(rng, l, wall, floor);
        if let (Some(a), Some(b)) = (k.0.any_room(rng), k.1.any_room(rng)) {
            corridor(rng, l, a.center(), b.center(), wall, floor);
        }
    }
}

fn corridor(rng: &mut Rng, l: &mut Level, a: Pos, b: Pos, wall: u8, floor: u8) {
    let (mut x, mut y) = (a.x, a.y);
    let horiz_first = rng.int_n(2) == 0;
    let step = |l: &mut Level, x: &mut i32, y: &mut i32, tx: i32, ty: i32| {
        while *x != tx {
            *x += (tx - *x).signum();
            if l.at(*x, *y) == wall {
                l.set(*x, *y, floor);
            }
        }
        while *y != ty {
            *y += (ty - *y).signum();
            if l.at(*x, *y) == wall {
                l.set(*x, *y, floor);
            }
        }
    };
    if horiz_first {
        let yy = y;
        step(l, &mut x, &mut y, b.x, yy);
        let xx = x;
        step(l, &mut x, &mut y, xx, b.y);
    } else {
        let xx = x;
        step(l, &mut x, &mut y, xx, b.y);
        let yy = y;
        step(l, &mut x, &mut y, b.x, yy);
    }
}

fn gen_crypt(
    r: &mut Rng,
    w: i32,
    h: i32,
    depth: i32,
    max_depth: i32,
    pal: &Palette,
) -> DungeonFloor {
    let (wall, floor) = (t(pal.wall), t(pal.floor));
    let mut l = Level::new("", "", w, h, wall);
    let mut root = Bsp::new(Rect::new(1, 1, w - 2, h - 2));
    root.split(r, 12, 10);
    root.carve(r, &mut l, floor);
    root.connect(r, &mut l, wall, floor);
    let mut rooms = Vec::new();
    root.rooms(&mut rooms);

    // doors where corridors meet rooms
    let door = t("door");
    for rm in &rooms {
        for x in rm.x - 1..=rm.x + rm.w {
            for y in [rm.y - 1, rm.y + rm.h] {
                if l.at(x, y) == floor
                    && l.at(x - 1, y) == wall
                    && l.at(x + 1, y) == wall
                    && r.int_n(2) == 0
                {
                    l.set(x, y, door);
                }
            }
        }
        for y in rm.y - 1..=rm.y + rm.h {
            for x in [rm.x - 1, rm.x + rm.w] {
                if l.at(x, y) == floor
                    && l.at(x, y - 1) == wall
                    && l.at(x, y + 1) == wall
                    && r.int_n(2) == 0
                {
                    l.set(x, y, door);
                }
            }
        }
    }

    // decoration
    for rm in &rooms {
        if rm.w >= 8 && rm.h >= 6 && r.int_n(3) == 0 {
            let (x0, y0, x1, y1) = (rm.x + 1, rm.y + 1, rm.x + rm.w - 2, rm.y + rm.h - 2);
            for p in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                l.set(p.0, p.1, t(pal.pillar));
            }
        }
        if !pal.carpet.is_empty() && rm.w >= 7 && rm.h >= 5 && r.int_n(2) == 0 {
            let cy = rm.center().y;
            for x in rm.x + 2..rm.x + rm.w - 2 {
                if l.at(x, cy) == floor {
                    l.set(x, cy, t(pal.carpet));
                }
            }
        }
        for _ in 0..3 {
            let (x, y) = (
                r.range(rm.x, rm.x + rm.w - 1),
                r.range(rm.y, rm.y + rm.h - 1),
            );
            if l.at(x, y) == floor && r.int_n(2) == 0 {
                let d = *r.pick(pal.deco);
                l.set(x, y, t(d));
            }
        }
    }

    let first = rooms[0];
    let up = first.center();
    l.set(up.x, up.y, t("stairs_up"));
    l.up = up;
    let far = farthest_room(&l, up, &rooms);
    let mut boss = None;
    if depth >= max_depth {
        let bc = far.center();
        l.set(bc.x, bc.y - 1, t("altar"));
        let mut b = Pos::new(bc.x, bc.y + 1);
        if !l.walkable(b.x, b.y) {
            b = bc;
        }
        boss = Some(b);
        l.down = Pos::new(-1, -1);
    } else {
        let dn = far.center();
        l.set(dn.x, dn.y, t("stairs_down"));
        l.down = dn;
    }
    let n = 1 + r.int_n(2) + depth / 2;
    place_chests(r, &mut l, &rooms, first, n, t(pal.chest), floor);
    let monsters = scatter(r, &l, &rooms, first, (7 + depth * 3) as usize);
    let k = 3 + r.int_n(3);
    let items = scatter(r, &l, &rooms, first, k as usize);
    DungeonFloor {
        level: l,
        monsters,
        items,
        boss,
    }
}

fn farthest_room(l: &Level, from: Pos, rooms: &[Rect]) -> Rect {
    let dist = bfs(l, from);
    let mut best = *rooms.last().unwrap();
    let mut bd = -1;
    for rm in rooms {
        let c = rm.center();
        let d = dist[l.idx(c)];
        if d > bd {
            best = *rm;
            bd = d;
        }
    }
    best
}

/// Walking distances from `from` (-1 = unreachable); doors count as passable.
pub fn bfs(l: &Level, from: Pos) -> Vec<i32> {
    let mut dist = vec![-1; (l.w * l.h) as usize];
    let mut q = std::collections::VecDeque::new();
    q.push_back(from);
    dist[l.idx(from)] = 0;
    while let Some(c) = q.pop_front() {
        for d in DIRS4 {
            let n = c.add(d);
            if !l.inside(n.x, n.y) || dist[l.idx(n)] >= 0 {
                continue;
            }
            let def = l.def_at(n);
            if !def.walkable && def.interact != "door" {
                continue;
            }
            dist[l.idx(n)] = dist[l.idx(c)] + 1;
            q.push_back(n);
        }
    }
    dist
}

fn place_chests(
    r: &mut Rng,
    l: &mut Level,
    rooms: &[Rect],
    skip: Rect,
    mut n: i32,
    chest: u8,
    floor: u8,
) {
    let mut k = 0;
    while k < n * 10 && n > 0 {
        k += 1;
        let rm = *r.pick(rooms);
        if rm == skip {
            continue;
        }
        // against a wall, not in a doorway
        let x = r.range(rm.x, rm.x + rm.w - 1);
        let y = if r.int_n(2) == 0 {
            rm.y + rm.h - 1
        } else {
            rm.y
        };
        if l.at(x, y) != floor || x == rm.center().x {
            continue;
        }
        l.set(x, y, chest);
        if !all_rooms_reachable(l, rooms) {
            l.set(x, y, floor);
            continue;
        }
        n -= 1;
    }
}

fn all_rooms_reachable(l: &Level, rooms: &[Rect]) -> bool {
    let dist = bfs(l, l.up);
    rooms.iter().all(|rm| {
        let c = rm.center();
        !l.walkable(c.x, c.y) || dist[l.idx(c)] >= 0
    })
}

fn scatter(r: &mut Rng, l: &Level, rooms: &[Rect], skip: Rect, n: usize) -> Vec<Pos> {
    let mut out = Vec::new();
    for _ in 0..n * 20 {
        if out.len() >= n {
            break;
        }
        let rm = *r.pick(rooms);
        if rm == skip && rooms.len() > 1 {
            continue;
        }
        let (x, y) = (
            r.range(rm.x, rm.x + rm.w - 1),
            r.range(rm.y, rm.y + rm.h - 1),
        );
        if l.walkable(x, y) && l.def(x, y).interact.is_empty() {
            out.push(Pos::new(x, y));
        }
    }
    out
}

// ring lists the 8 neighbours in order around a cell.
const RING: [Pos; 8] = [
    Pos::new(0, -1),
    Pos::new(1, -1),
    Pos::new(1, 0),
    Pos::new(1, 1),
    Pos::new(0, 1),
    Pos::new(-1, 1),
    Pos::new(-1, 0),
    Pos::new(-1, -1),
];

/// Whether a floor cell next to walls can be filled without disconnecting
/// anything: its walkable neighbours form one unbroken arc.
fn blockable(l: &Level, p: Pos) -> bool {
    let mut open = [false; 8];
    let mut walls = 0;
    for (i, d) in RING.iter().enumerate() {
        let q = p.add(*d);
        open[i] = l.walkable(q.x, q.y);
        if !open[i] {
            walls += 1;
        }
    }
    if walls < 3 || walls == 8 {
        return false;
    }
    let changes = (0..8).filter(|&i| open[i] != open[(i + 1) % 8]).count();
    changes == 2
}

// ---------- cellular automata cave ----------

fn gen_cave(
    r: &mut Rng,
    w: i32,
    h: i32,
    depth: i32,
    max_depth: i32,
    pal: &Palette,
) -> DungeonFloor {
    let (wall, floor) = (t(pal.wall), t(pal.floor));
    let n = (w * h) as usize;
    let mut l;
    let mut attempt = 0;
    loop {
        l = Level::new("", "", w, h, wall);
        let mut cells = vec![false; n]; // true = wall
        for y in 0..h {
            for x in 0..w {
                let edge = x == 0 || y == 0 || x == w - 1 || y == h - 1;
                cells[(y * w + x) as usize] = edge || r.f64() < 0.45;
            }
        }
        for iter in 0..5 {
            let mut next = vec![false; n];
            for y in 0..h {
                for x in 0..w {
                    if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                        next[(y * w + x) as usize] = true;
                        continue;
                    }
                    let (mut n1, mut n2) = (0, 0);
                    for dy in -2..=2i32 {
                        for dx in -2..=2i32 {
                            let (xx, yy) = (x + dx, y + dy);
                            let wall_at = xx < 0
                                || yy < 0
                                || xx >= w
                                || yy >= h
                                || cells[(yy * w + xx) as usize];
                            if !wall_at {
                                continue;
                            }
                            if dx.abs() <= 1 && dy.abs() <= 1 {
                                n1 += 1;
                            }
                            n2 += 1;
                        }
                    }
                    next[(y * w + x) as usize] = n1 >= 5 || (iter < 3 && n2 <= 2);
                }
            }
            cells = next;
        }
        for (i, c) in cells.iter().enumerate() {
            if !c {
                l.tiles[i] = floor;
            }
        }
        let mask = largest_region(&l);
        let mut count = 0;
        for i in 0..n {
            if !mask[i] {
                l.tiles[i] = wall;
            } else {
                count += 1;
            }
        }
        attempt += 1;
        if count > n * 35 / 100 || attempt > 8 {
            break;
        }
    }

    // puddles and lava
    let pool = Perlin::new(r);
    for y in 0..h {
        for x in 0..w {
            if l.at(x, y) != floor {
                continue;
            }
            let v = pool.fbm(x as f64 / 7.0, y as f64 / 7.0, 2);
            if v > pal.pool_threshold {
                l.set(x, y, t(if depth >= 3 { pal.pool3 } else { pal.pool }));
            } else if r.int_n(45) == 0 {
                let d = *r.pick(pal.deco);
                l.set(x, y, t(d));
            }
        }
    }

    let floors: Vec<Pos> = (0..n as i32)
        .map(|i| Pos::new(i % w, i / w))
        .filter(|p| l.at(p.x, p.y) == floor)
        .collect();
    let up = *r.pick(&floors);
    l.set(up.x, up.y, t("stairs_up"));
    l.up = up;
    let dist = bfs(&l, up);
    let (mut far, mut fd) = (up, -1);
    for p in &floors {
        let d = dist[l.idx(*p)];
        if d > fd {
            far = *p;
            fd = d;
        }
    }
    let mut boss = None;
    if depth >= max_depth {
        boss = Some(far);
        l.down = Pos::new(-1, -1);
    } else {
        l.set(far.x, far.y, t("stairs_down"));
        l.down = far;
    }
    // crystals along the walls, only where they cannot cut a passage
    if !pal.crystal.is_empty() {
        let crystal = t(pal.crystal);
        for i in r.perm(floors.len()) {
            let p = floors[i];
            if l.at(p.x, p.y) != floor
                || p.manhattan(up) < 4
                || p == l.down
                || boss.map(|b: Pos| p.manhattan(b) < 3).unwrap_or(false)
                || r.int_n(10) > 0
            {
                continue;
            }
            if blockable(&l, p) {
                l.set(p.x, p.y, crystal);
            }
        }
    }
    // chests in dead ends
    let chest = t(pal.chest);
    let mut chests = 1 + r.int_n(2) + depth / 2;
    for i in r.perm(floors.len()) {
        if chests == 0 {
            break;
        }
        let p = floors[i];
        if l.at(p.x, p.y) != floor || p.manhattan(up) < 8 {
            continue;
        }
        let walls = DIRS4
            .iter()
            .filter(|d| l.at(p.x + d.x, p.y + d.y) == wall)
            .count();
        if walls == 3 {
            l.set(p.x, p.y, chest);
            chests -= 1;
        }
    }
    let pick = |r: &mut Rng, n: usize| {
        let mut out = Vec::new();
        for _ in 0..n * 30 {
            if out.len() >= n {
                break;
            }
            let p = *r.pick(&floors);
            if l.at(p.x, p.y) == floor && p.manhattan(up) > 10 {
                out.push(p);
            }
        }
        out
    };
    let monsters = pick(r, (8 + depth * 3) as usize);
    let k = 3 + r.int_n(3);
    let items = pick(r, k as usize);
    DungeonFloor {
        level: l,
        monsters,
        items,
        boss,
    }
}
