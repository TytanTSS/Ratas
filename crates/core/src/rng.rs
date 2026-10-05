//! A small deterministic random number generator (PCG-XSH-RR 64/32).
//!
//! World generation must give the same world for the same seed on every
//! platform, so the game does not depend on an external RNG crate whose
//! algorithm might change.

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
    inc: u64,
}

const MUL: u64 = 6364136223846793005;

/// FNV-1a 64-bit hash of a string (labels of generators, tile variants).
pub fn fnv64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

impl Rng {
    pub fn new(seed: u64, stream: u64) -> Rng {
        let mut r = Rng { state: 0, inc: (stream << 1) | 1 };
        r.next_u32();
        r.state = r.state.wrapping_add(seed);
        r.next_u32();
        r
    }

    /// A generator for (seed, label): every part of the world has its own stream.
    pub fn labeled(seed: i64, label: &str) -> Rng {
        Rng::new(seed as u64, fnv64(label))
    }

    /// A generator seeded from the clock (gameplay randomness).
    pub fn from_time() -> Rng {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(42);
        let addr = &t as *const u64 as u64;
        Rng::new(t, addr ^ 0x9e3779b97f4a7c15)
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(MUL).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    pub fn next_u64(&mut self) -> u64 {
        ((self.next_u32() as u64) << 32) | self.next_u32() as u64
    }

    /// A uniform integer in [0, n); 0 when n <= 0.
    pub fn int_n(&mut self, n: i32) -> i32 {
        if n <= 0 {
            return 0;
        }
        let n = n as u32;
        // rejection sampling avoids modulo bias
        let threshold = n.wrapping_neg() % n;
        loop {
            let r = self.next_u32();
            if r >= threshold {
                return (r % n) as i32;
            }
        }
    }

    pub fn usize_n(&mut self, n: usize) -> usize {
        self.int_n(n as i32) as usize
    }

    /// A uniform float in [0, 1).
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn f32(&mut self) -> f32 {
        self.f64() as f32
    }

    /// A random int in [lo, hi].
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + self.int_n(hi - lo + 1)
    }

    /// A float in [lo, hi).
    pub fn roll(&mut self, lo: f64, hi: f64) -> f64 {
        if hi <= lo {
            return lo;
        }
        lo + self.f64() * (hi - lo)
    }

    /// True with the given percent chance.
    pub fn chance(&mut self, pct: f64) -> bool {
        self.f64() * 100.0 < pct
    }

    /// A random permutation of 0..n.
    pub fn perm(&mut self, n: usize) -> Vec<usize> {
        let mut v: Vec<usize> = (0..n).collect();
        self.shuffle(&mut v);
        v
    }

    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.usize_n(i + 1);
            v.swap(i, j);
        }
    }

    pub fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.usize_n(v.len())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let mut a = Rng::labeled(7, "x");
        let mut b = Rng::labeled(7, "x");
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
        let mut c = Rng::labeled(7, "y");
        assert_ne!(Rng::labeled(7, "x").next_u32(), c.next_u32());
    }

    #[test]
    fn ranges() {
        let mut r = Rng::new(1, 2);
        for _ in 0..1000 {
            let v = r.int_n(7);
            assert!((0..7).contains(&v));
            let f = r.f64();
            assert!((0.0..1.0).contains(&f));
            let x = r.range(3, 5);
            assert!((3..=5).contains(&x));
        }
    }
}
