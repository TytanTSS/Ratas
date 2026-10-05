use crate::rng::Rng;

/// Classic 2D gradient noise.
pub struct Perlin {
    perm: [usize; 512],
}

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + t * (b - a)
}

fn grad(h: usize, x: f64, y: f64) -> f64 {
    match h & 7 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x,
        5 => -x,
        6 => y,
        _ => -y,
    }
}

impl Perlin {
    pub fn new(r: &mut Rng) -> Perlin {
        let idx = r.perm(256);
        let mut perm = [0usize; 512];
        for (i, p) in perm.iter_mut().enumerate() {
            *p = idx[i & 255];
        }
        Perlin { perm }
    }

    /// A value roughly in [-1, 1].
    pub fn noise(&self, x: f64, y: f64) -> f64 {
        let (xi, yi) = (
            (x.floor() as i64 & 255) as usize,
            (y.floor() as i64 & 255) as usize,
        );
        let (xf, yf) = (x - x.floor(), y - y.floor());
        let (u, v) = (fade(xf), fade(yf));
        let p = &self.perm;
        let aa = p[p[xi] + yi];
        let ab = p[p[xi] + yi + 1];
        let ba = p[p[xi + 1] + yi];
        let bb = p[p[xi + 1] + yi + 1];
        let x1 = lerp(grad(aa, xf, yf), grad(ba, xf - 1.0, yf), u);
        let x2 = lerp(grad(ab, xf, yf - 1.0), grad(bb, xf - 1.0, yf - 1.0), u);
        lerp(x1, x2, v)
    }

    /// Several octaves of noise.
    pub fn fbm(&self, x: f64, y: f64, octaves: i32) -> f64 {
        let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
        for _ in 0..octaves {
            sum += amp * self.noise(x * freq, y * freq);
            norm += amp;
            amp *= 0.5;
            freq *= 2.03;
        }
        sum / norm
    }
}
