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
    AugmentedFifth,
    MinorSixth,
    MajorSixth,
    DiminishedSeventh,
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
        Interval::AugmentedFifth,
        Interval::MinorSixth,
        Interval::MajorSixth,
        Interval::DiminishedSeventh,
        Interval::MinorSeventh,
        Interval::MajorSeventh,
    ];

    /// Which degree, 1..=7; decides the letter.
    pub fn number(self) -> u8 {
        match self {
            Interval::Unison => 1,
            Interval::MinorSecond | Interval::MajorSecond => 2,
            Interval::MinorThird | Interval::MajorThird => 3,
            Interval::PerfectFourth | Interval::AugmentedFourth => 4,
            Interval::DiminishedFifth | Interval::PerfectFifth | Interval::AugmentedFifth => 5,
            Interval::MinorSixth | Interval::MajorSixth => 6,
            Interval::DiminishedSeventh | Interval::MinorSeventh | Interval::MajorSeventh => 7,
        }
    }

    /// How this degree differs from the major scale's; decides the glyph.
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
            Interval::AugmentedFourth | Interval::AugmentedFifth => Accidental::Sharp,
            Interval::DiminishedSeventh => Accidental::DoubleFlat,
        }
    }

    /// Derived from the degree's letter, so a perfect fourth is 5 and an augmented fourth 6.
    pub fn semitones(self) -> u8 {
        let natural = i16::from(Letter::C.step(self.number() - 1).natural_semitone());
        let altered = natural + i16::from(self.alteration().offset());

        u8::try_from(altered).expect("no degree's alteration reaches below the root")
    }

    /// The interval from one written note up to another, or `None` for a distance
    /// outside the fifteen, such as `F♭` to `B♯`.
    ///
    /// `not(test)` because this module's own tests call it, which would leave a bare `expect` unfulfilled.
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

/// The degree-formula spelling — `1`, `b3`, `#4`, `b5` — in ASCII for canvas markers.
/// A degree rather than a quality, so the two tritones read `#4` and `b5`.
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
        // a variant added to the enum but not to ALL would be skipped by the tests below
        assert_eq!(Interval::ALL.len(), 15);
    }

    #[test]
    fn the_chord_degrees_derive_from_number_and_alteration() {
        assert_eq!(Interval::AugmentedFifth.semitones(), 8);
        assert_eq!(Interval::DiminishedSeventh.semitones(), 9);
        assert_eq!(Interval::AugmentedFifth.to_string(), "#5");
        assert_eq!(Interval::DiminishedSeventh.to_string(), "bb7");
    }

    #[test]
    fn semitones_matches_the_written_out_distances() {
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
            (Interval::AugmentedFifth, 8),
            (Interval::MinorSixth, 8),
            (Interval::MajorSixth, 9),
            (Interval::DiminishedSeventh, 9),
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
            (Interval::AugmentedFifth, "#5"),
            (Interval::MinorSixth, "b6"),
            (Interval::MajorSixth, "6"),
            (Interval::DiminishedSeventh, "bb7"),
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
        // an unaltered degree renders bare, so a marker never reads `n3`
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
    fn between_is_none_beyond_the_fifteen() {
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
