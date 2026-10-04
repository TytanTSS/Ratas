package world

import "container/heap"

// octant transforms for recursive shadowcasting
var octants = [8][4]int{
	{1, 0, 0, 1}, {0, 1, 1, 0}, {0, -1, 1, 0}, {-1, 0, 0, 1},
	{-1, 0, 0, -1}, {0, -1, -1, 0}, {0, 1, -1, 0}, {1, 0, 0, -1},
}

// FOV computes the field of view from (ox, oy) using recursive shadowcasting
// and calls mark for every visible cell.
func FOV(l *Level, ox, oy, radius int, mark func(x, y int)) {
	if !l.In(ox, oy) {
		return
	}
	mark(ox, oy)
	for _, o := range octants {
		castLight(l, ox, oy, radius, 1, 1.0, 0.0, o[0], o[1], o[2], o[3], mark)
	}
}

func castLight(l *Level, cx, cy, radius, row int, start, end float64, xx, xy, yx, yy int, mark func(x, y int)) {
	if start < end {
		return
	}
	r2 := radius*radius + radius
	newStart := 0.0
	for j := row; j <= radius; j++ {
		dx, dy := -j-1, -j
		blocked := false
		for dx <= 0 {
			dx++
			x := cx + dx*xx + dy*xy
			y := cy + dx*yx + dy*yy
			lSlope := (float64(dx) - 0.5) / (float64(dy) + 0.5)
			rSlope := (float64(dx) + 0.5) / (float64(dy) - 0.5)
			if start < rSlope {
				continue
			}
			if end > lSlope {
				break
			}
			if dx*dx+dy*dy <= r2 && l.In(x, y) {
				mark(x, y)
			}
			opaque := !l.Transparent(x, y)
			if blocked {
				if opaque {
					newStart = rSlope
					continue
				}
				blocked = false
				start = newStart
			} else if opaque && j < radius {
				blocked = true
				castLight(l, cx, cy, radius, j+1, start, lSlope, xx, xy, yx, yy, mark)
				newStart = rSlope
			}
		}
		if blocked {
			break
		}
	}
}

// LOS reports whether b is visible from a (Bresenham line through transparent cells).
func LOS(l *Level, a, b Pos) bool {
	ok := true
	Line(a, b, func(p Pos) bool {
		if p == a || p == b {
			return true
		}
		if !l.Transparent(p.X, p.Y) {
			ok = false
			return false
		}
		return true
	})
	return ok
}

// Line walks a Bresenham line from a to b, stopping when fn returns false.
func Line(a, b Pos, fn func(Pos) bool) {
	dx, dy := abs(b.X-a.X), -abs(b.Y-a.Y)
	sx, sy := 1, 1
	if a.X > b.X {
		sx = -1
	}
	if a.Y > b.Y {
		sy = -1
	}
	err := dx + dy
	x, y := a.X, a.Y
	for {
		if !fn(Pos{x, y}) {
			return
		}
		if x == b.X && y == b.Y {
			return
		}
		e2 := 2 * err
		if e2 >= dy {
			err += dy
			x += sx
		}
		if e2 <= dx {
			err += dx
			y += sy
		}
	}
}

// LinePoints returns the cells of a ray from a through b extended to length n (excluding a).
func LinePoints(a, b Pos, n int) []Pos {
	if a == b {
		return nil
	}
	// extend the target far away so the ray continues past b
	dx, dy := b.X-a.X, b.Y-a.Y
	k := 1
	for max(abs(dx*k), abs(dy*k)) < n {
		k++
	}
	far := Pos{a.X + dx*k, a.Y + dy*k}
	var pts []Pos
	Line(a, far, func(p Pos) bool {
		if p != a {
			pts = append(pts, p)
		}
		return len(pts) < n
	})
	return pts
}

// --- A* path finding over a 4-connected grid ---

type node struct {
	idx   int
	f     float64
	index int
}

type nodeHeap []*node

func (h nodeHeap) Len() int           { return len(h) }
func (h nodeHeap) Less(i, j int) bool { return h[i].f < h[j].f }
func (h nodeHeap) Swap(i, j int)      { h[i], h[j] = h[j], h[i]; h[i].index = i; h[j].index = j }
func (h *nodeHeap) Push(x any)        { n := x.(*node); n.index = len(*h); *h = append(*h, n) }
func (h *nodeHeap) Pop() any          { old := *h; n := old[len(old)-1]; *h = old[:len(old)-1]; return n }

// FindPath returns a path from 'from' to 'to' (excluding 'from'). cost returns
// the cost to enter a cell or a negative value if it is impassable; the goal
// cell is always enterable. If the node budget runs out, the path to the
// explored cell closest to the goal is returned.
func FindPath(w, h int, from, to Pos, maxNodes int, cost func(x, y int) float64) []Pos {
	if from == to {
		return nil
	}
	start := from.Y*w + from.X
	goal := to.Y*w + to.X
	g := map[int]float64{start: 0}
	came := map[int]int{}
	closed := map[int]bool{}
	hfn := func(i int) float64 {
		x, y := i%w, i/w
		return float64(abs(x-to.X) + abs(y-to.Y))
	}
	open := &nodeHeap{}
	heap.Push(open, &node{idx: start, f: hfn(start)})
	best, bestH := start, hfn(start)
	expanded := 0
	for open.Len() > 0 {
		cur := heap.Pop(open).(*node)
		if closed[cur.idx] {
			continue
		}
		if cur.idx == goal {
			best = goal
			break
		}
		closed[cur.idx] = true
		expanded++
		if expanded > maxNodes {
			break
		}
		cx, cy := cur.idx%w, cur.idx/w
		for _, d := range AllDirs {
			dd := d.Delta()
			nx, ny := cx+dd.X, cy+dd.Y
			if nx < 0 || ny < 0 || nx >= w || ny >= h {
				continue
			}
			ni := ny*w + nx
			if closed[ni] {
				continue
			}
			c := 1.0
			if ni != goal {
				c = cost(nx, ny)
				if c < 0 {
					continue
				}
			}
			ng := g[cur.idx] + c
			if old, ok := g[ni]; ok && old <= ng {
				continue
			}
			g[ni] = ng
			came[ni] = cur.idx
			hv := hfn(ni)
			if hv < bestH {
				best, bestH = ni, hv
			}
			heap.Push(open, &node{idx: ni, f: ng + hv})
		}
	}
	if best == start {
		return nil
	}
	var path []Pos
	for i := best; i != start; i = came[i] {
		path = append(path, Pos{i % w, i / w})
	}
	for a, b := 0, len(path)-1; a < b; a, b = a+1, b-1 {
		path[a], path[b] = path[b], path[a]
	}
	return path
}
