use rand::prelude::IndexedRandom;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// All game randomness flows through one seeded stream: same seed, same run.
pub struct RunRng(ChaCha8Rng);

impl RunRng {
    pub fn new(seed: u64) -> Self {
        Self(ChaCha8Rng::seed_from_u64(seed))
    }

    /// Uniform integer in `lo..=hi`.
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        self.0.random_range(lo..=hi)
    }

    /// Uniform 0..=99, for percentage rolls.
    pub fn percent(&mut self) -> u32 {
        self.0.random_range(0..100)
    }

    pub fn shuffle<T>(&mut self, xs: &mut [T]) {
        xs.shuffle(&mut self.0);
    }

    pub fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        *xs.choose(&mut self.0).expect("pick from empty slice")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = RunRng::new(42);
        let mut b = RunRng::new(42);
        let sa: Vec<u32> = (0..100).map(|_| a.range(0, 1000)).collect();
        let sb: Vec<u32> = (0..100).map(|_| b.range(0, 1000)).collect();
        assert_eq!(sa, sb);
    }

    #[test]
    fn different_seed_different_sequence() {
        let mut a = RunRng::new(1);
        let mut b = RunRng::new(2);
        let sa: Vec<u32> = (0..100).map(|_| a.range(0, 1000)).collect();
        let sb: Vec<u32> = (0..100).map(|_| b.range(0, 1000)).collect();
        assert_ne!(sa, sb);
    }

    #[test]
    fn range_is_inclusive_and_bounded() {
        let mut r = RunRng::new(7);
        let mut seen_lo = false;
        let mut seen_hi = false;
        for _ in 0..2000 {
            let v = r.range(3, 7);
            assert!((3..=7).contains(&v));
            seen_lo |= v == 3;
            seen_hi |= v == 7;
        }
        assert!(seen_lo && seen_hi);
    }

    #[test]
    fn percent_is_0_to_99() {
        let mut r = RunRng::new(7);
        for _ in 0..2000 {
            assert!(r.percent() <= 99);
        }
    }

    #[test]
    fn shuffle_is_deterministic_permutation() {
        let mut a = RunRng::new(9);
        let mut b = RunRng::new(9);
        let mut xs: Vec<u32> = (0..20).collect();
        let mut ys = xs.clone();
        a.shuffle(&mut xs);
        b.shuffle(&mut ys);
        assert_eq!(xs, ys);
        let mut sorted = xs.clone();
        sorted.sort();
        assert_eq!(sorted, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn pick_returns_an_element() {
        let mut r = RunRng::new(3);
        for _ in 0..100 {
            let v = r.pick(&[10u32, 20, 30]);
            assert!([10, 20, 30].contains(&v));
        }
    }
}
