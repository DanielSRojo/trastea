//! A small deterministic pseudo-random number generator.
//!
//! Trastea only ever needs to pick one item out of a list of a dozen or two, so
//! this is a hand-written splitmix64 rather than a dependency on `rand`. The
//! important property is not statistical quality but that the state lives
//! somewhere and is *carried*: drawing a number takes `&mut self`, so two draws
//! in a row cannot come out correlated the way two clock reads do.
//!
//! Seed it once and it is reproducible, which is what lets the tests here and in
//! `ui` assert things about the sequence.

use std::time::{SystemTime, UNIX_EPOCH};

/// 2^64 divided by the golden ratio, rounded to an odd number.
///
/// Any odd addend walks the whole of `u64` before repeating; this is the specific
/// constant splitmix64 is defined with.
const GOLDEN_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

/// A splitmix64 generator.
///
/// The state advances by adding [`GOLDEN_GAMMA`], and the output is that state
/// run through two rounds of xor-shift-multiply. Because the state moves by
/// addition rather than by shifting, no seed is degenerate — a plain xorshift is
/// stuck at zero forever if it is ever seeded with zero, and this is not.
pub struct Rng {
    state: u64,
}

impl Rng {
    /// A generator that will always produce the same sequence for the same seed.
    pub fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Seeds from the wall clock, for a sequence that differs between runs.
    ///
    /// Call this once, at startup — reading the clock per draw is what made the
    /// old `random_note`/`random_scale_kind` pair correlated.
    pub fn from_clock() -> Self {
        let since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        // The nanoseconds occupy only the low 30 bits, so the seconds are shifted
        // clear of them instead of xored on top.
        let seed = (since_epoch.as_secs() << 30) ^ u64::from(since_epoch.subsec_nanos());

        Self::from_seed(seed)
    }

    /// Advances the state and returns the scrambled output.
    ///
    /// `wrapping_add`/`wrapping_mul` are the point of the algorithm, not a way to
    /// dodge an overflow check: splitmix64 is defined in arithmetic modulo 2^64.
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_GAMMA);

        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A uniform index into a slice of `len` items.
    ///
    /// Plain modulo, no rejection sampling: the bias is on the order of
    /// `len / 2^64`, which for the sizes here is around one part in 10^18.
    ///
    /// # Panics
    ///
    /// Panics if `len` is zero.
    pub fn below(&mut self, len: usize) -> usize {
        assert!(len > 0, "there is no index below zero");

        (self.next_u64() % len as u64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn matches_the_reference_vectors() {
        // The published splitmix64 outputs for a zero seed. If a constant or a
        // shift is mistyped, this is what catches it.
        let mut rng = Rng::from_seed(0);

        assert_eq!(rng.next_u64(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(rng.next_u64(), 0x6e78_9e6a_a1b9_65f4);
        assert_eq!(rng.next_u64(), 0x06c4_5d18_8009_454f);
    }

    #[test]
    fn a_zero_seed_is_not_degenerate() {
        // The trap a plain xorshift would fall into: seeded with zero it emits
        // zeroes forever.
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
        // Seeds one apart are what two clock reads a few hundred nanoseconds apart
        // look like; the scrambler has to pull them far apart.
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
}
