use super::{Level, Pos};

// octant transforms for recursive shadowcasting
const OCTANTS: [[i32; 4]; 8] = [
    [1, 0, 0, 1],
    [0, 1, 1, 0],
    [0, -1, 1, 0],
    [-1, 0, 0, 1],
    [-1, 0, 0, -1],
    [0, -1, -1, 0],
    [0, 1, -1, 0],
    [1, 0, 0, -1],
];

/// Computes the field of view from (ox, oy) with recursive shadowcasting and
/// calls mark for every visible cell.
pub fn fov(l: &Level, ox: i32, oy: i32, radius: i32, mark: &mut dyn FnMut(i32, i32)) {
    if !l.inside(ox, oy) {
        return;
    }
    // the transparency of every cell is looked up once
    mark(ox, oy);
    for o in OCTANTS {
        cast_light(l, ox, oy, radius, 1, 1.0, 0.0, o, mark);
    }
}

#[allow(clippy::too_many_arguments)]
fn cast_light(l: &Level, cx: i32, cy: i32, radius: i32, row: i32, mut start: f64, end: f64, o: [i32; 4], mark: &mut dyn FnMut(i32, i32)) {
    if start < end {
        return;
    }
    let [xx, xy, yx, yy] = o;
    let r2 = radius * radius + radius;
    let mut new_start = 0.0;
    for j in row..=radius {
        let (mut dx, dy) = (-j - 1, -j);
        let mut blocked = false;
        while dx <= 0 {
            dx += 1;
            let x = cx + dx * xx + dy * xy;
            let y = cy + dx * yx + dy * yy;
            let l_slope = (dx as f64 - 0.5) / (dy as f64 + 0.5);
            let r_slope = (dx as f64 + 0.5) / (dy as f64 - 0.5);
            if start < r_slope {
                continue;
            }
            if end > l_slope {
                break;
            }
            if dx * dx + dy * dy <= r2 && l.inside(x, y) {
                mark(x, y);
            }
            let opaque = !l.transparent(x, y);
            if blocked {
                if opaque {
                    new_start = r_slope;
                    continue;
                }
                blocked = false;
                start = new_start;
            } else if opaque && j < radius {
                blocked = true;
                cast_light(l, cx, cy, radius, j + 1, start, l_slope, o, mark);
                new_start = r_slope;
            }
        }
        if blocked {
            break;
        }
    }
}

/// Reports whether b is visible from a (a line through transparent cells).
pub fn los(l: &Level, a: Pos, b: Pos) -> bool {
    let mut ok = true;
    line(a, b, &mut |p| {
        if p == a || p == b {
            return true;
        }
        if !l.transparent(p.x, p.y) {
            ok = false;
            return false;
        }
        true
    });
    ok
}

/// Walks a Bresenham line from a to b, stopping when f returns false.
pub fn line(a: Pos, b: Pos, f: &mut dyn FnMut(Pos) -> bool) {
    let (dx, dy) = ((b.x - a.x).abs(), -(b.y - a.y).abs());
    let sx = if a.x > b.x { -1 } else { 1 };
    let sy = if a.y > b.y { -1 } else { 1 };
    let mut err = dx + dy;
    let (mut x, mut y) = (a.x, a.y);
    loop {
        if !f(Pos::new(x, y)) {
            return;
        }
        if x == b.x && y == b.y {
            return;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// The cells of a ray from a through b extended to length n (excluding a).
pub fn line_points(a: Pos, b: Pos, n: i32) -> Vec<Pos> {
    if a == b {
        return Vec::new();
    }
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let mut k = 1;
    while (dx * k).abs().max((dy * k).abs()) < n {
        k += 1;
    }
    let far = Pos::new(a.x + dx * k, a.y + dy * k);
    let mut pts = Vec::new();
    line(a, far, &mut |p| {
        if p != a {
            pts.push(p);
        }
        (pts.len() as i32) < n
    });
    pts
}
