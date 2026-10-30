//! Small deterministic random number generator.
//!
//! The simulation must produce identical results for the same seed on every
//! machine, so it carries its own generator instead of relying on a crate
//! whose algorithm could change between versions.

/// SplitMix64-seeded xoshiro256** generator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    s: [u64; 4],
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut sm = seed;
        Self {
            s: [
                splitmix64(&mut sm),
                splitmix64(&mut sm),
                splitmix64(&mut sm),
                splitmix64(&mut sm),
            ],
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform float in `[0, 1)`.
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform float in `[lo, hi)`.
    pub fn range_f64(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f64()
    }

    /// Uniform integer in `[lo, hi)`. Panics if the range is empty.
    pub fn range_usize(&mut self, lo: usize, hi: usize) -> usize {
        assert!(hi > lo, "empty range {lo}..{hi}");
        lo + (self.next_u64() % (hi - lo) as u64) as usize
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.f64() < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            Some(&items[self.range_usize(0, items.len())])
        }
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.range_usize(0, i + 1);
            items.swap(i, j);
        }
    }
}

/// Stateless hash used for stable per-viewer noise (intel estimates).
pub fn hash64(parts: &[u64]) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    for &p in parts {
        let mut x = p;
        h ^= splitmix64(&mut x);
        h = h.wrapping_mul(0x0100_0000_01B3);
    }
    let mut x = h;
    splitmix64(&mut x)
}

/// Maps a hash to a float in `[-1, 1)`.
pub fn hash_signed_unit(parts: &[u64]) -> f64 {
    (hash64(parts) >> 11) as f64 * (2.0 / (1u64 << 53) as f64) - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn floats_in_range() {
        let mut r = Rng::new(1);
        for _ in 0..10_000 {
            let x = r.f64();
            assert!((0.0..1.0).contains(&x));
            let i = r.range_usize(3, 9);
            assert!((3..9).contains(&i));
        }
    }

    #[test]
    fn signed_unit_hash_in_range() {
        for i in 0..1000u64 {
            let v = hash_signed_unit(&[i, 3, 9]);
            assert!((-1.0..1.0).contains(&v));
        }
    }
}
