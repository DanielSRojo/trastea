//! A seedable splitmix64 generator, so tests can reproduce drill sequences.

use std::time::{SystemTime, UNIX_EPOCH};

/// 2^64 divided by the golden ratio, rounded to an odd number.
const GOLDEN_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

/// The state advances by addition, so unlike xorshift no seed (not even zero) is degenerate.
pub struct Rng {
    state: u64,
}

impl Rng {
    /// A generator that will always produce the same sequence for the same seed.
    pub fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Seeds from the wall clock. Call once at startup; clock reads per draw come out correlated.
    pub fn from_clock() -> Self {
        let since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        // nanoseconds fit in the low 30 bits, so the seconds are shifted clear of them
        let seed = (since_epoch.as_secs() << 30) ^ u64::from(since_epoch.subsec_nanos());

        Self::from_seed(seed)
    }

    /// Wrapping arithmetic is intended: splitmix64 is defined modulo 2^64.
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_GAMMA);

        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A uniform index into a slice of `len` items. Modulo bias is about `len / 2^64`.
    ///
    /// # Panics
    ///
    /// Panics if `len` is zero.
    pub fn below(&mut self, len: usize) -> usize {
        assert!(len > 0, "there is no index below zero");

        (self.next_u64() % len as u64) as usize
    }

    /// An index into `weights`, drawn in proportion to them. Equal weights replay `below`'s
    /// sequence exactly, so weighting a pool changes no seed's draws until a weight moves.
    ///
    /// # Panics
    ///
    /// Panics if the weights sum to zero.
    pub fn weighted(&mut self, weights: &[u32]) -> usize {
        let total: u64 = weights.iter().copied().map(u64::from).sum();
        assert!(
            total > 0,
            "there is nothing to draw from weights summing to zero"
        );

        let ticket = self.next_u64() % total;

        weights
            .iter()
            .scan(0, |end, &weight| {
                *end += u64::from(weight);
                Some(*end)
            })
            .position(|end| ticket < end)
            .expect("the ticket is below the total, which is the last running end")
    }

    /// Draws with `draw` until it yields something other than `current`.
    ///
    /// Never returns if `draw` can only produce `current`.
    pub fn draw_other_than<T: PartialEq>(
        &mut self,
        current: T,
        mut draw: impl FnMut(&mut Rng) -> T,
    ) -> T {
        loop {
            let candidate = draw(self);
            if candidate != current {
                return candidate;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn matches_the_reference_vectors() {
        // the published splitmix64 outputs for a zero seed
        let mut rng = Rng::from_seed(0);

        assert_eq!(rng.next_u64(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(rng.next_u64(), 0x6e78_9e6a_a1b9_65f4);
        assert_eq!(rng.next_u64(), 0x06c4_5d18_8009_454f);
    }

    #[test]
    fn a_zero_seed_is_not_degenerate() {
        let mut rng = Rng::from_seed(0);
        let drawn: HashSet<u64> = (0..64).map(|_| rng.next_u64()).collect();

        assert_eq!(drawn.len(), 64);
        assert!(!drawn.contains(&0));
    }

    #[test]
    fn the_same_seed_replays_the_same_sequence() {
        let mut a = Rng::from_seed(0xfeed_face);
        let mut b = Rng::from_seed(0xfeed_face);

        for _ in 0..32 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn adjacent_seeds_do_not_produce_adjacent_output() {
        // seeds one apart are what two close clock reads look like
        let first = Rng::from_seed(1).next_u64();
        let second = Rng::from_seed(2).next_u64();

        assert_ne!(first, second);
        assert!(first.abs_diff(second) > 1_000_000);
    }

    #[test]
    fn below_stays_in_range() {
        let mut rng = Rng::from_seed(99);

        for len in 1..=32_usize {
            for _ in 0..64 {
                assert!(rng.below(len) < len);
            }
        }
    }

    #[test]
    fn below_reaches_every_index() {
        let mut rng = Rng::from_seed(0xc0ffee);
        let drawn: HashSet<usize> = (0..2_000).map(|_| rng.below(12)).collect();

        assert_eq!(drawn.len(), 12);
    }

    #[test]
    #[should_panic(expected = "no index below zero")]
    fn below_zero_panics() {
        Rng::from_seed(1).below(0);
    }

    #[test]
    fn equal_weights_replay_below() {
        let mut weighted = Rng::from_seed(0x5eed);
        let mut uniform = Rng::from_seed(0x5eed);

        for _ in 0..500 {
            assert_eq!(weighted.weighted(&[1; 45]), uniform.below(45));
        }
    }

    #[test]
    fn a_zero_weight_is_never_drawn() {
        let mut rng = Rng::from_seed(11);

        assert!((0..2_000).all(|_| rng.weighted(&[3, 0, 1, 0]).is_multiple_of(2)));
    }

    #[test]
    fn draws_follow_the_weights() {
        let mut rng = Rng::from_seed(0xbeef);
        let mut counts = [0_u32; 2];

        for _ in 0..4_000 {
            counts[rng.weighted(&[1, 3])] += 1;
        }

        // three to one, within a few percent
        assert!((2.7..3.3).contains(&(f64::from(counts[1]) / f64::from(counts[0]))));
    }

    #[test]
    #[should_panic(expected = "weights summing to zero")]
    fn all_zero_weights_panic() {
        Rng::from_seed(1).weighted(&[0, 0]);
    }

    #[test]
    fn a_redraw_never_lands_on_what_is_already_there() {
        let mut rng = Rng::from_seed(7);
        let mut current = 0;

        // a pool of two forces every repeat to be rejected
        for _ in 0..200 {
            let next = rng.draw_other_than(current, |rng| rng.below(2));
            assert_ne!(next, current);
            current = next;
        }
    }
}
