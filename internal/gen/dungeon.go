package gen

import (
	"fmt"
	"math/rand/v2"

	"ratas/internal/content"
	"ratas/internal/world"
)

// DungeonFloor is a generated dungeon level plus suggested spawn points.
type DungeonFloor struct {
	Level    *world.Level
	Monsters []world.Pos
	Items    []world.Pos
	Boss     *world.Pos
}

// palette is the set of tiles a dungeon theme is built from.
type palette struct {
	cave         bool   // cellular automata instead of rooms
	wall, floor  string // base tiles
	pool, pool3  string // noise pools (caves), pool3 from depth 3
	deco         []string
	pillar       string // room corners (rooms)
	chest        string
	crystal      string // non-walkable cave decoration in alcoves
	carpet       string // carpets along big rooms
	poolTreshold float64
}

var palettes = map[string]palette{
	"cave":     {cave: true, wall: "cave_wall", floor: "cave_floor", pool: "water", pool3: "lava", deco: []string{"rubble", "bones", "web"}, chest: "chest", poolTreshold: 0.42},
	"ice":      {cave: true, wall: "ice_wall", floor: "ice_floor", pool: "ice", pool3: "ice", deco: []string{"snowdrift", "bones"}, chest: "chest", crystal: "crystal", poolTreshold: 0.36},
	"volcano":  {cave: true, wall: "basalt_wall", floor: "basalt_floor", pool: "lava", pool3: "lava", deco: []string{"magma_crack", "rubble", "bones"}, chest: "chest", poolTreshold: 0.4},
	"crypt":    {wall: "stone_wall", floor: "stone_floor", deco: []string{"bones", "rubble"}, pillar: "pillar", chest: "chest"},
	"temple":   {wall: "sandstone_wall", floor: "sandstone_floor", deco: []string{"bones", "rubble"}, pillar: "pillar", chest: "sarcophagus"},
	"fortress": {wall: "dark_wall", floor: "dark_floor", deco: []string{"bones", "rubble"}, pillar: "brazier", chest: "chest", carpet: "carpet"},
}

// Themes lists every dungeon theme.
func Themes() []string { return []string{"cave", "crypt", "ice", "volcano", "temple", "fortress"} }

// GenerateDungeon builds one floor. Room themes (crypt, temple, fortress) use
// BSP rooms and corridors, cave themes (cave, ice, volcano) use cellular
// automata. The last floor (depth == maxDepth) has a boss lair instead of
// stairs down.
func GenerateDungeon(seed int64, id, name, theme string, dungeonIdx, depth, maxDepth int) *DungeonFloor {
	r := RNG(seed, id)
	w, h := 84+depth*4, 40+depth*2
	pal, ok := palettes[theme]
	if !ok {
		pal = palettes["crypt"]
	}
	var f *DungeonFloor
	if pal.cave {
		f = genCave(r, w, h, depth, maxDepth, pal)
	} else {
		f = genCrypt(r, w, h, depth, maxDepth, pal)
	}
	f.Level.ID = id
	f.Level.Name = fmt.Sprintf("%s — ур. %d", name, depth)
	f.Level.Theme = theme
	f.Level.Depth = depth
	f.Level.Dungeon = dungeonIdx
	return f
}

// ---------- BSP crypt ----------

type bspNode struct {
	r           Rect
	left, right *bspNode
	room        *Rect
}

func (n *bspNode) split(r *rand.Rand, minW, minH int) {
	horiz := r.IntN(2) == 0
	if n.r.W > n.r.H*2 {
		horiz = false
	} else if n.r.H*2 > n.r.W*1 && n.r.H > minH*2 {
		horiz = true
	}
	if horiz {
		if n.r.H < minH*2 {
			return
		}
		cut := Range(r, minH, n.r.H-minH)
		n.left = &bspNode{r: Rect{n.r.X, n.r.Y, n.r.W, cut}}
		n.right = &bspNode{r: Rect{n.r.X, n.r.Y + cut, n.r.W, n.r.H - cut}}
	} else {
		if n.r.W < minW*2 {
			return
		}
		cut := Range(r, minW, n.r.W-minW)
		n.left = &bspNode{r: Rect{n.r.X, n.r.Y, cut, n.r.H}}
		n.right = &bspNode{r: Rect{n.r.X + cut, n.r.Y, n.r.W - cut, n.r.H}}
	}
	n.left.split(r, minW, minH)
	n.right.split(r, minW, minH)
}

func (n *bspNode) rooms(out *[]*Rect) {
	if n.room != nil {
		*out = append(*out, n.room)
	}
	if n.left != nil {
		n.left.rooms(out)
		n.right.rooms(out)
	}
}

func (n *bspNode) anyRoom(r *rand.Rand) *Rect {
	var rs []*Rect
	n.rooms(&rs)
	if len(rs) == 0 {
		return nil
	}
	return rs[r.IntN(len(rs))]
}

func genCrypt(r *rand.Rand, w, h, depth, maxDepth int, pal palette) *DungeonFloor {
	T := content.TileID
	wall, floor := T(pal.wall), T(pal.floor)
	l := world.NewLevel("", "", w, h, wall)
	root := &bspNode{r: Rect{1, 1, w - 2, h - 2}}
	root.split(r, 14, 8)

	var carveLeaves func(n *bspNode)
	carveLeaves = func(n *bspNode) {
		if n.left != nil {
			carveLeaves(n.left)
			carveLeaves(n.right)
			return
		}
		rw := Range(r, 5, n.r.W-3)
		rh := Range(r, 4, n.r.H-2)
		rx := Range(r, n.r.X+1, n.r.X+n.r.W-rw-1)
		ry := Range(r, n.r.Y+1, n.r.Y+n.r.H-rh-1)
		room := Rect{rx, ry, rw, rh}
		n.room = &room
		for y := ry; y < ry+rh; y++ {
			for x := rx; x < rx+rw; x++ {
				l.Set(x, y, floor)
			}
		}
	}
	carveLeaves(root)

	corridor := func(a, b world.Pos) {
		x, y := a.X, a.Y
		horizFirst := r.IntN(2) == 0
		step := func(tx, ty int) {
			for x != tx {
				if x < tx {
					x++
				} else {
					x--
				}
				if l.At(x, y) == wall {
					l.Set(x, y, floor)
				}
			}
			for y != ty {
				if y < ty {
					y++
				} else {
					y--
				}
				if l.At(x, y) == wall {
					l.Set(x, y, floor)
				}
			}
		}
		if horizFirst {
			step(b.X, y)
			step(x, b.Y)
		} else {
			step(x, b.Y)
			step(b.X, y)
		}
	}
	var connect func(n *bspNode)
	connect = func(n *bspNode) {
		if n.left == nil {
			return
		}
		connect(n.left)
		connect(n.right)
		a, b := n.left.anyRoom(r), n.right.anyRoom(r)
		if a != nil && b != nil {
			corridor(a.Center(), b.Center())
		}
	}
	connect(root)

	var rooms []*Rect
	root.rooms(&rooms)

	// doors where corridors meet rooms
	door := T("door")
	for _, rm := range rooms {
		for x := rm.X - 1; x <= rm.X+rm.W; x++ {
			for _, y := range []int{rm.Y - 1, rm.Y + rm.H} {
				if l.At(x, y) == floor && l.At(x-1, y) == wall && l.At(x+1, y) == wall && r.IntN(2) == 0 {
					l.Set(x, y, door)
				}
			}
		}
		for y := rm.Y - 1; y <= rm.Y+rm.H; y++ {
			for _, x := range []int{rm.X - 1, rm.X + rm.W} {
				if l.At(x, y) == floor && l.At(x, y-1) == wall && l.At(x, y+1) == wall && r.IntN(2) == 0 {
					l.Set(x, y, door)
				}
			}
		}
	}

	// decoration
	for _, rm := range rooms {
		if rm.W >= 8 && rm.H >= 6 && r.IntN(3) == 0 {
			x0, y0, x1, y1 := rm.X+1, rm.Y+1, rm.X+rm.W-2, rm.Y+rm.H-2
			for _, p := range [][2]int{{x0, y0}, {x1, y0}, {x0, y1}, {x1, y1}} {
				l.Set(p[0], p[1], T(pal.pillar))
			}
		}
		if pal.carpet != "" && rm.W >= 7 && rm.H >= 5 && r.IntN(2) == 0 {
			cy := rm.Center().Y
			for x := rm.X + 2; x < rm.X+rm.W-2; x++ {
				if l.At(x, cy) == floor {
					l.Set(x, cy, T(pal.carpet))
				}
			}
		}
		for k := 0; k < 3; k++ {
			x, y := Range(r, rm.X, rm.X+rm.W-1), Range(r, rm.Y, rm.Y+rm.H-1)
			if l.At(x, y) == floor && r.IntN(2) == 0 {
				l.Set(x, y, T(pal.deco[r.IntN(len(pal.deco))]))
			}
		}
	}

	f := &DungeonFloor{Level: l}
	first := rooms[0]
	up := first.Center()
	l.Set(up.X, up.Y, T("stairs_up"))
	l.Up = up
	far := farthestRoom(l, up, rooms)
	if depth >= maxDepth {
		bc := far.Center()
		l.Set(bc.X, bc.Y-1, T("altar"))
		b := world.Pos{X: bc.X, Y: bc.Y + 1}
		if !l.Walkable(b.X, b.Y) {
			b = bc
		}
		f.Boss = &b
		l.Down = world.Pos{X: -1, Y: -1}
	} else {
		dn := far.Center()
		l.Set(dn.X, dn.Y, T("stairs_down"))
		l.Down = dn
	}
	placeChests(r, l, rooms, first, 1+r.IntN(2)+depth/2, T(pal.chest), floor)
	f.Monsters = scatter(r, l, rooms, first, 7+depth*3)
	f.Items = scatter(r, l, rooms, first, 3+r.IntN(3))
	return f
}

func farthestRoom(l *world.Level, from world.Pos, rooms []*Rect) *Rect {
	dist := bfs(l, from)
	best, bd := rooms[len(rooms)-1], -1
	for _, rm := range rooms {
		c := rm.Center()
		if d := dist[c.Y*l.W+c.X]; d > bd {
			best, bd = rm, d
		}
	}
	return best
}

// bfs returns walking distances from 'from' (-1 = unreachable); doors count as passable.
func bfs(l *world.Level, from world.Pos) []int {
	dist := make([]int, l.W*l.H)
	for i := range dist {
		dist[i] = -1
	}
	q := []world.Pos{from}
	dist[from.Y*l.W+from.X] = 0
	for len(q) > 0 {
		c := q[0]
		q = q[1:]
		for _, d := range world.AllDirs {
			n := c.Add(d.Delta())
			if !l.In(n.X, n.Y) || dist[n.Y*l.W+n.X] >= 0 {
				continue
			}
			def := l.Def(n.X, n.Y)
			if !def.Walkable && def.Interact != "door" {
				continue
			}
			dist[n.Y*l.W+n.X] = dist[c.Y*l.W+c.X] + 1
			q = append(q, n)
		}
	}
	return dist
}

func placeChests(r *rand.Rand, l *world.Level, rooms []*Rect, skip *Rect, n int, chest, floor uint8) {
	for k := 0; k < n*10 && n > 0; k++ {
		rm := rooms[r.IntN(len(rooms))]
		if rm == skip {
			continue
		}
		// against a wall, not in a doorway
		x := Range(r, rm.X, rm.X+rm.W-1)
		y := rm.Y
		if r.IntN(2) == 0 {
			y = rm.Y + rm.H - 1
		}
		if l.At(x, y) != floor || x == rm.Center().X {
			continue
		}
		l.Set(x, y, chest)
		if !allRoomsReachable(l, rooms) {
			l.Set(x, y, floor)
			continue
		}
		n--
	}
}

func allRoomsReachable(l *world.Level, rooms []*Rect) bool {
	dist := bfs(l, l.Up)
	for _, rm := range rooms {
		c := rm.Center()
		if l.Walkable(c.X, c.Y) && dist[c.Y*l.W+c.X] < 0 {
			return false
		}
	}
	return true
}

func scatter(r *rand.Rand, l *world.Level, rooms []*Rect, skip *Rect, n int) []world.Pos {
	var out []world.Pos
	for k := 0; k < n*20 && len(out) < n; k++ {
		rm := rooms[r.IntN(len(rooms))]
		if rm == skip && len(rooms) > 1 {
			continue
		}
		x, y := Range(r, rm.X, rm.X+rm.W-1), Range(r, rm.Y, rm.Y+rm.H-1)
		if l.Walkable(x, y) && l.Def(x, y).Interact == "" {
			out = append(out, world.Pos{X: x, Y: y})
		}
	}
	return out
}

// ring lists the 8 neighbours in order around a cell.
var ring = [8]world.Pos{{X: 0, Y: -1}, {X: 1, Y: -1}, {X: 1, Y: 0}, {X: 1, Y: 1}, {X: 0, Y: 1}, {X: -1, Y: 1}, {X: -1, Y: 0}, {X: -1, Y: -1}}

// blockable reports whether a floor cell next to walls can be filled without
// disconnecting anything: its walkable neighbours form one unbroken arc
// (consecutive ring cells are orthogonally adjacent, so the arc stays
// connected around the blocked cell).
func blockable(l *world.Level, p world.Pos) bool {
	open := [8]bool{}
	walls := 0
	for i, d := range ring {
		q := p.Add(d)
		open[i] = l.Walkable(q.X, q.Y)
		if !open[i] {
			walls++
		}
	}
	if walls < 3 || walls == 8 {
		return false
	}
	changes := 0
	for i := range open {
		if open[i] != open[(i+1)%8] {
			changes++
		}
	}
	return changes == 2
}

// ---------- cellular automata cave ----------

func genCave(r *rand.Rand, w, h, depth, maxDepth int, pal palette) *DungeonFloor {
	T := content.TileID
	wall, floor := T(pal.wall), T(pal.floor)
	var l *world.Level
	for attempt := 0; ; attempt++ {
		l = world.NewLevel("", "", w, h, wall)
		cells := make([]bool, w*h) // true = wall
		for y := 0; y < h; y++ {
			for x := 0; x < w; x++ {
				edge := x == 0 || y == 0 || x == w-1 || y == h-1
				cells[y*w+x] = edge || r.Float64() < 0.45
			}
		}
		for iter := 0; iter < 5; iter++ {
			next := make([]bool, w*h)
			for y := 0; y < h; y++ {
				for x := 0; x < w; x++ {
					if x == 0 || y == 0 || x == w-1 || y == h-1 {
						next[y*w+x] = true
						continue
					}
					n1, n2 := 0, 0
					for dy := -2; dy <= 2; dy++ {
						for dx := -2; dx <= 2; dx++ {
							xx, yy := x+dx, y+dy
							wallAt := xx < 0 || yy < 0 || xx >= w || yy >= h || cells[yy*w+xx]
							if !wallAt {
								continue
							}
							if abs(dx) <= 1 && abs(dy) <= 1 {
								n1++
							}
							n2++
						}
					}
					next[y*w+x] = n1 >= 5 || (iter < 3 && n2 <= 2)
				}
			}
			cells = next
		}
		for i, c := range cells {
			if !c {
				l.Tiles[i] = floor
			}
		}
		mask := largestRegion(l)
		count := 0
		for i := range l.Tiles {
			if !mask[i] {
				l.Tiles[i] = wall
			} else {
				count++
			}
		}
		if count > w*h*35/100 || attempt > 8 {
			break
		}
	}

	// puddles and lava
	pool := NewPerlin(r)
	for y := 0; y < h; y++ {
		for x := 0; x < w; x++ {
			if l.At(x, y) != floor {
				continue
			}
			v := pool.FBM(float64(x)/7, float64(y)*2/7, 2)
			if v > pal.poolTreshold {
				if depth >= 3 {
					l.Set(x, y, T(pal.pool3))
				} else {
					l.Set(x, y, T(pal.pool))
				}
			} else if r.IntN(45) == 0 {
				l.Set(x, y, T(pal.deco[r.IntN(len(pal.deco))]))
			}
		}
	}

	var floors []world.Pos
	for y := 0; y < h; y++ {
		for x := 0; x < w; x++ {
			if l.At(x, y) == floor {
				floors = append(floors, world.Pos{X: x, Y: y})
			}
		}
	}
	f := &DungeonFloor{Level: l}
	up := floors[r.IntN(len(floors))]
	l.Set(up.X, up.Y, T("stairs_up"))
	l.Up = up
	dist := bfs(l, up)
	far, fd := up, -1
	for _, p := range floors {
		if d := dist[p.Y*w+p.X]; d > fd {
			far, fd = p, d
		}
	}
	if depth >= maxDepth {
		b := far
		f.Boss = &b
		l.Down = world.Pos{X: -1, Y: -1}
	} else {
		l.Set(far.X, far.Y, T("stairs_down"))
		l.Down = far
	}
	// crystals along the walls, only where they cannot cut a passage
	if pal.crystal != "" {
		crystal := T(pal.crystal)
		for _, i := range r.Perm(len(floors)) {
			p := floors[i]
			if l.At(p.X, p.Y) != floor || p.Manhattan(up) < 4 || p == l.Down || (f.Boss != nil && p.Manhattan(*f.Boss) < 3) || r.IntN(10) > 0 {
				continue
			}
			if blockable(l, p) {
				l.Set(p.X, p.Y, crystal)
			}
		}
	}
	// chests in dead ends
	chest := T(pal.chest)
	chests := 1 + r.IntN(2) + depth/2
	for _, i := range r.Perm(len(floors)) {
		if chests == 0 {
			break
		}
		p := floors[i]
		if l.At(p.X, p.Y) != floor || p.Manhattan(up) < 8 {
			continue
		}
		walls := 0
		for _, d := range world.AllDirs {
			n := p.Add(d.Delta())
			if l.At(n.X, n.Y) == wall {
				walls++
			}
		}
		if walls == 3 {
			l.Set(p.X, p.Y, chest)
			chests--
		}
	}
	pick := func(n int) []world.Pos {
		var out []world.Pos
		for k := 0; k < n*30 && len(out) < n; k++ {
			p := floors[r.IntN(len(floors))]
			if l.At(p.X, p.Y) == floor && p.Manhattan(up) > 10 {
				out = append(out, p)
			}
		}
		return out
	}
	f.Monsters = pick(8 + depth*3)
	f.Items = pick(3 + r.IntN(3))
	return f
}
