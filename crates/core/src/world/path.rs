use super::{Pos, DIRS4, DIRS8};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

#[derive(PartialEq)]
struct Node {
    f: f64,
    idx: usize,
}

impl Eq for Node {}
impl Ord for Node {
    fn cmp(&self, o: &Self) -> Ordering {
        // a min-heap on f
        o.f.partial_cmp(&self.f)
            .unwrap_or(Ordering::Equal)
            .then_with(|| o.idx.cmp(&self.idx))
    }
}
impl PartialOrd for Node {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Returns a path from `from` to `to` (excluding `from`). cost returns the
/// cost to enter a cell or a negative value if it is impassable; the goal is
/// always enterable. With diagonal moves a corner may not be cut: both
/// orthogonal neighbours must be passable. If the node budget runs out, the
/// path to the explored cell closest to the goal is returned.
pub fn find_path(
    w: i32,
    h: i32,
    from: Pos,
    to: Pos,
    max_nodes: usize,
    diagonal: bool,
    cost: &mut dyn FnMut(i32, i32) -> f64,
) -> Vec<Pos> {
    if from == to {
        return Vec::new();
    }
    let wi = w as usize;
    let index = |p: Pos| p.y as usize * wi + p.x as usize;
    let start = index(from);
    let goal = index(to);
    let mut g: HashMap<usize, f64> = HashMap::new();
    let mut came: HashMap<usize, usize> = HashMap::new();
    let mut closed: HashSet<usize> = HashSet::new();
    let hfn = |i: usize| -> f64 {
        let (x, y) = ((i % wi) as i32, (i / wi) as i32);
        let (dx, dy) = ((x - to.x).abs() as f64, (y - to.y).abs() as f64);
        if diagonal {
            dx.max(dy) + 0.414 * dx.min(dy)
        } else {
            dx + dy
        }
    };
    let mut open = BinaryHeap::new();
    g.insert(start, 0.0);
    open.push(Node {
        f: hfn(start),
        idx: start,
    });
    let (mut best, mut best_h) = (start, hfn(start));
    let mut expanded = 0;
    let dirs: &[Pos] = if diagonal { &DIRS8 } else { &DIRS4 };
    let mut cache: HashMap<usize, f64> = HashMap::new();
    let mut cell_cost = |x: i32, y: i32, cost: &mut dyn FnMut(i32, i32) -> f64| -> f64 {
        let i = y as usize * wi + x as usize;
        *cache.entry(i).or_insert_with(|| cost(x, y))
    };
    while let Some(cur) = open.pop() {
        if closed.contains(&cur.idx) {
            continue;
        }
        if cur.idx == goal {
            best = goal;
            break;
        }
        closed.insert(cur.idx);
        expanded += 1;
        if expanded > max_nodes {
            break;
        }
        let (cx, cy) = ((cur.idx % wi) as i32, (cur.idx / wi) as i32);
        for d in dirs {
            let (nx, ny) = (cx + d.x, cy + d.y);
            if nx < 0 || ny < 0 || nx >= w || ny >= h {
                continue;
            }
            let ni = ny as usize * wi + nx as usize;
            if closed.contains(&ni) {
                continue;
            }
            let diag = d.x != 0 && d.y != 0;
            if diag {
                // no cutting corners
                let a = (cx + d.x, cy);
                let b = (cx, cy + d.y);
                let ia = a.1 as usize * wi + a.0 as usize;
                let ib = b.1 as usize * wi + b.0 as usize;
                if (ia != goal && cell_cost(a.0, a.1, cost) < 0.0)
                    || (ib != goal && cell_cost(b.0, b.1, cost) < 0.0)
                {
                    continue;
                }
            }
            let mut c = 1.0;
            if ni != goal {
                c = cell_cost(nx, ny, cost);
                if c < 0.0 {
                    continue;
                }
            }
            if diag {
                c *= 1.414;
            }
            let ng = g[&cur.idx] + c;
            if let Some(&old) = g.get(&ni) {
                if old <= ng {
                    continue;
                }
            }
            g.insert(ni, ng);
            came.insert(ni, cur.idx);
            let hv = hfn(ni);
            if hv < best_h {
                best = ni;
                best_h = hv;
            }
            open.push(Node {
                f: ng + hv,
                idx: ni,
            });
        }
    }
    if best == start {
        return Vec::new();
    }
    let mut path = Vec::new();
    let mut i = best;
    while i != start {
        path.push(Pos::new((i % wi) as i32, (i / wi) as i32));
        i = came[&i];
    }
    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_around_wall() {
        // wall at x=5 except y=9
        let mut cost = |x: i32, y: i32| if x == 5 && y != 9 { -1.0 } else { 1.0 };
        let p = find_path(
            10,
            10,
            Pos::new(0, 0),
            Pos::new(9, 0),
            10000,
            true,
            &mut cost,
        );
        assert_eq!(*p.last().unwrap(), Pos::new(9, 0));
        assert!(p.iter().any(|c| *c == Pos::new(5, 9)));
        let p4 = find_path(
            10,
            10,
            Pos::new(0, 0),
            Pos::new(3, 3),
            1000,
            false,
            &mut |_, _| 1.0,
        );
        assert_eq!(p4.len(), 6);
        let p8 = find_path(
            10,
            10,
            Pos::new(0, 0),
            Pos::new(3, 3),
            1000,
            true,
            &mut |_, _| 1.0,
        );
        assert_eq!(p8.len(), 3);
    }
}
