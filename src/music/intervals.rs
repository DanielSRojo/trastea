use std::fmt;

use super::notes::{Accidental, Letter, Note};

#[derive(PartialEq, Debug, Clone, Copy, Eq, Hash)]
pub enum Interval {
    Unison,
    MinorSecond,
    MajorSecond,
    MinorThird,
    MajorThird,
    PerfectFourth,
    AugmentedFourth,
    DiminishedFifth,
    PerfectFifth,
    MinorSixth,
    MajorSixth,
    MinorSeventh,
    MajorSeventh,
}

impl Interval {
    pub const ALL: &'static [Interval] = &[
        Interval::Unison,
        Interval::MinorSecond,
        Interval::MajorSecond,
        Interval::MinorThird,
        Interval::MajorThird,
        Interval::PerfectFourth,
        Interval::AugmentedFourth,
        Interval::DiminishedFifth,
        Interval::PerfectFifth,
        Interval::MinorSixth,
        Interval::MajorSixth,
        Interval::MinorSeventh,
        Interval::MajorSeventh,
    ];

    /// Which degree, 1..=7. This is what decides the letter.
    pub fn number(self) -> u8 {
        match self {
            Interval::Unison => 1,
            Interval::MinorSecond | Interval::MajorSecond => 2,
            Interval::MinorThird | Interval::MajorThird => 3,
            Interval::PerfectFourth | Interval::AugmentedFourth => 4,
            Interval::DiminishedFifth | Interval::PerfectFifth => 5,
            Interval::MinorSixth | Interval::MajorSixth => 6,
            Interval::MinorSeventh | Interval::MajorSeventh => 7,
        }
    }

    /// How this degree differs from the major scale's. This decides the glyph.
    pub fn alteration(self) -> Accidental {
        match self {
            Interval::Unison
            | Interval::MajorSecond
            | Interval::MajorThird
            | Interval::PerfectFourth
            | Interval::PerfectFifth
            | Interval::MajorSixth
            | Interval::MajorSeventh => Accidental::Natural,
            Interval::MinorSecond
            | Interval::MinorThird
            | Interval::DiminishedFifth
            | Interval::MinorSixth
            | Interval::MinorSeventh => Accidental::Flat,
            Interval::AugmentedFourth => Accidental::Sharp,
        }
    }

    /// Derived from the degree's letter rather than a second hand-written table:
    /// degree 4 is the letter F, F natural is 5 semitones up, so a perfect
    /// fourth is 5 and an augmented fourth is 6.
    pub fn semitones(self) -> u8 {
        let natural = i16::from(Letter::C.step(self.number() - 1).natural_semitone());
        let altered = natural + i16::from(self.alteration().offset());

        u8::try_from(altered).expect("no degree's alteration reaches below the root")
    }

    /// The interval from one written note up to another.
    ///
    /// `Option` because two spelled notes can sit a doubly augmented distance
    /// apart that is not among the thirteen — `F♭` to `B♯` — and saying
    /// "genuinely partial" beats picking a wrong answer. The letters are what
    /// resolve `F`→`B` as an augmented fourth and `F`→`B♭` as a perfect fourth.
    ///
    /// Nothing in the app calls this. The interval trainer used to be the expected
    /// caller, and now that the screen exists it turns out not to be: it lights its
    /// tonal center rather than naming a key, so nothing on it is spelled and it
    /// judges by semitone distance instead. A drill that *establishes* a key is what
    /// would call this — there, `F`→`B` and `F`→`B♭` are different questions and the
    /// degree number is the answer.
    ///
    /// Kept because it is the one function whose meaning gets sharper under spelling,
    /// and `expect` rather than `allow` so that the day a caller appears, the stale
    /// attribute is a warning rather than a silent leftover — this project has no
    /// `[lints]` table and nothing promotes `unfulfilled_lint_expectations` above
    /// warn, but the zero-warning build is enforced all the same, so a real caller
    /// landing here still gets caught.
    ///
    /// `cfg_attr(not(test), ...)`, not a bare `#[expect]`, because this module's
    /// own tests call `between` directly: under a test build the function is no
    /// longer dead, and a bare `#[expect(dead_code)]` would itself warn as
    /// unfulfilled. Gating on `not(test)` keeps the tripwire pointed at the one
    /// build — the real binary — where "no caller yet" is actually true.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "degree-aware theory kept ahead of its first caller"
        )
    )]
    pub fn between(from: Note, to: Note) -> Option<Interval> {
        let steps = (to.letter as u8 + 7 - from.letter as u8) % 7;
        let number = steps + 1;
        let semitones = (to.pitch_class().semitone() + 12 - from.pitch_class().semitone()) % 12;

        Interval::ALL
            .iter()
            .copied()
            .find(|interval| interval.number() == number && interval.semitones() == semitones)
    }
}

/// The degree-formula spelling: the degree number with its alteration in front —
/// `1`, `b3`, `#4`, `b5`. The same convention the scale trainer's formula row
/// prints with SMuFL glyphs, in the ASCII a canvas marker is limited to.
///
/// A degree, deliberately, and not a quality: `b3` rather than `m3`, `5` rather
/// than `P5`. The two tritones are why the distinction earns a comment — they are
/// one distance written two ways, and only the degree number tells `#4` from `b5`.
/// Should quality names ever be wanted, they belong in a method named for them
/// rather than in a second reading of `Display`.
impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.alteration().ascii(), self.number())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(letter: Letter, accidental: Accidental) -> Note {
        Note { letter, accidental }
    }

    #[test]
    fn all_lists_every_variant() {
        // The same tripwire ScaleKind::ALL carries: the tests below iterate ALL,
        // so a variant added to the enum and not to the list is silently skipped.
        assert_eq!(Interval::ALL.len(), 13);
    }

    #[test]
    fn semitones_matches_the_written_out_distances() {
        // semitones() derives these from Letter; this table is the independent
        // statement of what they must come to.
        let expected: &[(Interval, u8)] = &[
            (Interval::Unison, 0),
            (Interval::MinorSecond, 1),
            (Interval::MajorSecond, 2),
            (Interval::MinorThird, 3),
            (Interval::MajorThird, 4),
            (Interval::PerfectFourth, 5),
            (Interval::AugmentedFourth, 6),
            (Interval::DiminishedFifth, 6),
            (Interval::PerfectFifth, 7),
            (Interval::MinorSixth, 8),
            (Interval::MajorSixth, 9),
            (Interval::MinorSeventh, 10),
            (Interval::MajorSeventh, 11),
        ];

        assert_eq!(expected.len(), Interval::ALL.len(), "a variant is unlisted");

        for &(interval, semitones) in expected {
            assert_eq!(interval.semitones(), semitones, "{interval:?}");
        }
    }

    #[test]
    fn no_two_variants_share_a_number_and_an_alteration() {
        // What makes between's lookup well-defined.
        for (i, &interval) in Interval::ALL.iter().enumerate() {
            for &other in &Interval::ALL[i + 1..] {
                assert_ne!(
                    (interval.number(), interval.alteration()),
                    (other.number(), other.alteration()),
                    "{interval:?} and {other:?} are indistinguishable"
                );
            }
        }
    }

    #[test]
    fn the_two_tritones_differ_by_degree() {
        // Replaces scales.rs's one_interval_variant_spells_two_different_degrees,
        // which documented the conflation. This asserts the split.
        assert_eq!(
            Interval::AugmentedFourth.semitones(),
            Interval::DiminishedFifth.semitones()
        );

        assert_eq!(Interval::AugmentedFourth.number(), 4);
        assert_eq!(Interval::DiminishedFifth.number(), 5);
        assert_eq!(Interval::AugmentedFourth.alteration(), Accidental::Sharp);
        assert_eq!(Interval::DiminishedFifth.alteration(), Accidental::Flat);
    }

    #[test]
    fn display_spells_every_degree() {
        // The independent statement of what Display comes to, the way
        // semitones_matches_the_written_out_distances is for semitones().
        let expected: &[(Interval, &str)] = &[
            (Interval::Unison, "1"),
            (Interval::MinorSecond, "b2"),
            (Interval::MajorSecond, "2"),
            (Interval::MinorThird, "b3"),
            (Interval::MajorThird, "3"),
            (Interval::PerfectFourth, "4"),
            (Interval::AugmentedFourth, "#4"),
            (Interval::DiminishedFifth, "b5"),
            (Interval::PerfectFifth, "5"),
            (Interval::MinorSixth, "b6"),
            (Interval::MajorSixth, "6"),
            (Interval::MinorSeventh, "b7"),
            (Interval::MajorSeventh, "7"),
        ];

        assert_eq!(expected.len(), Interval::ALL.len(), "a variant is unlisted");

        for &(interval, text) in expected {
            assert_eq!(interval.to_string(), text, "{interval:?}");
        }
    }

    #[test]
    fn the_unaltered_degrees_carry_no_prefix() {
        // Not just "the major scale's degrees read 1..7": that an unaltered degree
        // renders bare is what keeps a marker from reading `n3`.
        for &interval in Interval::ALL {
            let bare = interval.to_string() == interval.number().to_string();
            assert_eq!(
                bare,
                interval.alteration() == Accidental::Natural,
                "{interval:?} renders as {interval}"
            );
        }
    }

    #[test]
    fn the_two_tritones_are_displayed_apart() {
        // The pair that makes Display a degree rather than a distance: equal
        // semitones, different text.
        assert_eq!(
            Interval::AugmentedFourth.semitones(),
            Interval::DiminishedFifth.semitones()
        );
        assert_ne!(
            Interval::AugmentedFourth.to_string(),
            Interval::DiminishedFifth.to_string()
        );
    }

    #[test]
    fn between_reads_letters_not_just_distances() {
        let f = note(Letter::F, Accidental::Natural);
        let b = note(Letter::B, Accidental::Natural);
        let b_flat = note(Letter::B, Accidental::Flat);

        assert_eq!(Interval::between(f, b), Some(Interval::AugmentedFourth));
        assert_eq!(Interval::between(f, b_flat), Some(Interval::PerfectFourth));
    }

    #[test]
    fn between_is_none_beyond_the_thirteen() {
        // A doubly augmented fourth is a real distance and not one of ours.
        let f_flat = note(Letter::F, Accidental::Flat);
        let b_sharp = note(Letter::B, Accidental::Sharp);

        assert_eq!(Interval::between(f_flat, b_sharp), None);
    }

    #[test]
    fn between_test() {
        let c = note(Letter::C, Accidental::Natural);
        let d = note(Letter::D, Accidental::Natural);

        assert_eq!(Interval::between(c, c), Some(Interval::Unison));
        assert_eq!(Interval::between(c, d), Some(Interval::MajorSecond));
        assert_eq!(Interval::between(d, c), Some(Interval::MinorSeventh));
    }
}
