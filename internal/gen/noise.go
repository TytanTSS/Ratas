// Package gen implements procedural generation: Perlin-noise overworld with
// biomes, rivers, villages and roads; BSP crypts and cellular-automata caves.
// All generation is deterministic for a given seed.
package gen

import (
	"hash/fnv"
	"math"
	"math/rand/v2"
)

// RNG returns a deterministic generator for (seed, label).
func RNG(seed int64, label string) *rand.Rand {
	h := fnv.New64a()
	h.Write([]byte(label))
	return rand.New(rand.NewPCG(uint64(seed), h.Sum64()))
}

// Range returns a random int in [lo, hi].
func Range(r *rand.Rand, lo, hi int) int {
	if hi <= lo {
		return lo
	}
	return lo + r.IntN(hi-lo+1)
}

// Perlin is classic 2D gradient noise.
type Perlin struct{ perm [512]int }

func NewPerlin(r *rand.Rand) *Perlin {
	p := &Perlin{}
	idx := r.Perm(256)
	for i := 0; i < 512; i++ {
		p.perm[i] = idx[i&255]
	}
	return p
}

func fade(t float64) float64       { return t * t * t * (t*(t*6-15) + 10) }
func lerp(a, b, t float64) float64 { return a + t*(b-a) }

func grad(h int, x, y float64) float64 {
	switch h & 7 {
	case 0:
		return x + y
	case 1:
		return -x + y
	case 2:
		return x - y
	case 3:
		return -x - y
	case 4:
		return x
	case 5:
		return -x
	case 6:
		return y
	default:
		return -y
	}
}

// Noise returns a value roughly in [-1, 1].
func (p *Perlin) Noise(x, y float64) float64 {
	xi, yi := int(math.Floor(x))&255, int(math.Floor(y))&255
	xf, yf := x-math.Floor(x), y-math.Floor(y)
	u, v := fade(xf), fade(yf)
	aa := p.perm[p.perm[xi]+yi]
	ab := p.perm[p.perm[xi]+yi+1]
	ba := p.perm[p.perm[xi+1]+yi]
	bb := p.perm[p.perm[xi+1]+yi+1]
	x1 := lerp(grad(aa, xf, yf), grad(ba, xf-1, yf), u)
	x2 := lerp(grad(ab, xf, yf-1), grad(bb, xf-1, yf-1), u)
	return lerp(x1, x2, v)
}

// FBM sums several octaves of noise.
func (p *Perlin) FBM(x, y float64, octaves int) float64 {
	sum, amp, freq, norm := 0.0, 1.0, 1.0, 0.0
	for i := 0; i < octaves; i++ {
		sum += amp * p.Noise(x*freq, y*freq)
		norm += amp
		amp *= 0.5
		freq *= 2.03
	}
	return sum / norm
}
