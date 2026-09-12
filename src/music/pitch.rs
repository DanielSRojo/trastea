//! Absolute pitch: which sound, as opposed to which of twelve.
//!
//! [`PitchClass`] answers "which of twelve" and deliberately throws the octave away — a
//! scale has no octave, and neither does a chord quality. That is the right model for
//! every screen the app draws. It stops being enough the moment a frequency is involved:
//! 82.41 Hz is a low E and 329.63 Hz is a high one, and both are pitch class 4.

// Nothing outside the tests reads a pitch until the tuner lands. `expect` rather than
// `allow`, so the attribute warns of its own accord once that happens — and
// `cfg_attr(not(test))` because this module's own tests do call all of it, so a bare
// expectation would itself go unfulfilled under a test build.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "absolute pitch precedes the tuner that reads it")
)]

use super::notes::PitchClass;

/// The reference the whole temperament hangs off. Named rather than inlined because it is
/// the one figure here a player has cause to move — 442 for an orchestra, 432 for the
/// people who are into that — and a literal 440 buried in a formula would be the thing
/// they had to find.
const A4_HZ: f32 = 440.0;

/// A4's place in the numbering below. The pair of constants is what keeps the formula
/// readable as the temperament rather than as two magic numbers.
const A4_MIDI: u8 = 69;

/// One sounding pitch, in twelve-tone equal temperament.
///
/// The field is a MIDI note number, private for the reason [`PitchClass`]'s is: MIDI is a
/// wire protocol and the domain word is pitch. That the representation happens to be
/// MIDI-compatible is a convenience if this app ever speaks MIDI, not a promise it makes.
///
/// Both halves of "which sound" fall out of the number by division, so they cannot
/// disagree — there is nothing to disagree. 0 is C-1, 60 is C4 (middle C), 69 is A4.
///
/// There is deliberately no `Display`, for the reason `PitchClass` has none: naming this
/// still means choosing between A♯4 and B♭4, and a pitch has no more context to choose
/// with than a pitch class does. The caller supplies a `Spelling`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pitch(u8);

impl Pitch {
    /// The highest MIDI number, and so the top of the representable range: G9.
    const MAX_MIDI: u8 = 127;

    /// `None` past G9. The guitar occupies 40..=79, so the bound is unreachable from the
    /// neck — but the type should not promise what only the caller knows.
    pub const fn from_midi(midi: u8) -> Option<Pitch> {
        if midi <= Pitch::MAX_MIDI {
            Some(Pitch(midi))
        } else {
            None
        }
    }

    /// Which of twelve this pitch is. Total: a sound always belongs to a pitch class,
    /// where the reverse needs an octave and so has no constructor here.
    pub fn pitch_class(self) -> PitchClass {
        // Sound arithmetic only because MIDI 0 is a C, so the numbering and
        // `PitchClass`'s both start their cycle in the same place.
        PitchClass::new(self.0 % 12)
    }

    /// The octave in scientific pitch notation, where middle C is C4 and the number
    /// advances at C rather than at A.
    ///
    /// `i8` because MIDI 0 is C-1: the lowest representable pitch has a negative octave.
    pub fn octave(self) -> i8 {
        // MIDI 12 is C0, so the octave is the number of complete twelves less one.
        (self.0 / 12) as i8 - 1
    }

    /// This pitch moved by `semitones`, or `None` off either end of the range.
    ///
    /// Note the contrast with [`PitchClass::transpose`], which takes a `u8` and wraps and
    /// cannot fail. That difference is the reason the two types exist rather than one:
    /// wrapping is the *point* of a pitch class, and wrapping here would turn a high E
    /// into a low one with nothing failing to compile.
    pub fn transpose(self, semitones: i8) -> Option<Pitch> {
        let midi = i16::from(self.0) + i16::from(semitones);
        u8::try_from(midi).ok().and_then(Pitch::from_midi)
    }

    /// What this pitch sounds at, in hertz.
    ///
    /// Arithmetic rather than a table of 128 entries, for the reason scale spelling is
    /// arithmetic rather than a table of key signatures: a table has entries that can be
    /// individually wrong, and `440 * 2^((m - 69) / 12)` cannot be.
    pub fn frequency(self) -> f32 {
        A4_HZ * 2.0f32.powf((f32::from(self.0) - f32::from(A4_MIDI)) / 12.0)
    }

    /// The pitch nearest `hz`, and how far `hz` sits from it.
    ///
    /// `None` for a frequency at or below zero, or one outside the representable range,
    /// rather than a clamped or wrapped pitch — a reading off the end of the piano is a
    /// reading to discard, not one to round into something plausible.
    ///
    /// The offset lands in −50..+50 cents, so exactly one pitch is ever named. Rounding
    /// the log to nearest puts the tie at exactly 50 cents and resolves it upward, which
    /// is what `f32::round` already does; the rule is the obvious implementation rather
    /// than something bolted onto it.
    pub fn nearest(hz: f32) -> Option<(Pitch, Cents)> {
        // `is_finite` rather than a negated comparison: a NaN reading compares false to
        // everything, so `!(hz > 0.0)` would reject it by accident rather than on purpose,
        // and an infinity would reach the logarithm. Both are discarded here, said plainly.
        if !hz.is_finite() || hz <= 0.0 {
            return None;
        }

        let exact = f32::from(A4_MIDI) + 12.0 * (hz / A4_HZ).log2();
        let nearest = exact.round();

        // The cast is guarded by the range check rather than by saturation: `as` on a
        // float clamps, which would turn an absurd reading into a legitimate-looking G9.
        if !(0.0..=f32::from(Pitch::MAX_MIDI)).contains(&nearest) {
            return None;
        }

        let pitch = Pitch::from_midi(nearest as u8)?;
        Some((pitch, Cents((exact - nearest) * 100.0)))
    }
}

/// A distance in cents, where 1200 is an octave and 100 a semitone.
///
/// A newtype rather than a bare `f32` because cents travels — out of [`Pitch::nearest`],
/// into a comparison against a tolerance, into a label — and a hertz value is an `f32`
/// too. Passing one where the other belongs would compile and be silent.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Cents(f32);

impl Cents {
    pub fn value(self) -> f32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pitches this app cares about, named once so the tests read as music.
    const E2: u8 = 40;
    const A2: u8 = 45;
    const D3: u8 = 50;
    const G3: u8 = 55;
    const B3: u8 = 59;
    const E4: u8 = 64;

    fn p(midi: u8) -> Pitch {
        Pitch::from_midi(midi).expect("the tests name only representable pitches")
    }

    #[test]
    fn the_numbering_is_scientific_pitch_notation() {
        // Middle C and the reference pitch, which between them pin the numbering.
        assert_eq!(p(60).pitch_class(), PitchClass::new(0));
        assert_eq!(p(60).octave(), 4);
        assert_eq!(p(A4_MIDI).pitch_class(), PitchClass::new(9));
        assert_eq!(p(A4_MIDI).octave(), 4);
    }

    #[test]
    fn the_lowest_pitch_has_a_negative_octave() {
        assert_eq!(p(0).pitch_class(), PitchClass::new(0));
        assert_eq!(p(0).octave(), -1);
    }

    #[test]
    fn the_range_ends_where_it_says_it_does() {
        assert_eq!(
            Pitch::from_midi(Pitch::MAX_MIDI).map(Pitch::octave),
            Some(9)
        );
        assert_eq!(Pitch::from_midi(128), None);
    }

    #[test]
    fn one_pitch_class_holds_more_than_one_pitch() {
        assert_ne!(p(E2), p(E4));
        assert_eq!(p(E2).pitch_class(), p(E4).pitch_class());
        assert_eq!(p(E2).octave(), 2);
        assert_eq!(p(E4).octave(), 4);
    }

    #[test]
    fn the_octave_advances_at_c() {
        // B3 up a semitone is C4 — the octave turns over here and not at A.
        assert_eq!(p(B3).transpose(1), Pitch::from_midi(60));
        assert_eq!(p(B3).transpose(1).map(Pitch::octave), Some(4));
        // And C4 up eleven is B4, so the turn happened once rather than twice.
        assert_eq!(p(60).transpose(11), Pitch::from_midi(71));
        assert_eq!(p(60).transpose(11).map(Pitch::octave), Some(4));
    }

    #[test]
    fn transposing_off_either_end_yields_nothing() {
        // Neither wrapped nor saturated: a high E transposed up must not come back low.
        assert_eq!(p(Pitch::MAX_MIDI).transpose(1), None);
        assert_eq!(p(0).transpose(-1), None);
        assert_eq!(p(0).transpose(i8::MIN), None);
        assert_eq!(p(Pitch::MAX_MIDI).transpose(i8::MAX), None);
    }

    #[test]
    fn the_reference_pitch_is_exact() {
        assert_eq!(p(A4_MIDI).frequency(), A4_HZ);
    }

    /// Pins the octave convention, not just the arithmetic. Everything being an octave
    /// out would still round-trip and still tune correctly against itself, so only a
    /// frequency catches it — a guitarist's written low E is E3 and sounds E2.
    #[test]
    fn the_open_strings_sound_their_standard_frequencies() {
        for (midi, expected) in [
            (E2, 82.41),
            (A2, 110.00),
            (D3, 146.83),
            (G3, 196.00),
            (B3, 246.94),
            (E4, 329.63),
        ] {
            let hz = p(midi).frequency();
            assert!(
                (hz - expected).abs() < 0.01,
                "MIDI {midi} sounds {hz} Hz, expected {expected}"
            );
        }
    }

    #[test]
    fn an_octave_doubles_the_frequency() {
        for midi in [E2, E2 + 12, E2 + 24] {
            let below = p(midi).frequency();
            let above = p(midi + 12).frequency();
            assert!(
                (above - below * 2.0).abs() < below * 1e-5,
                "{above} is not twice {below}"
            );
        }
    }

    #[test]
    fn an_exact_pitch_resolves_to_itself() {
        let (pitch, cents) = Pitch::nearest(A4_HZ).expect("440 Hz is representable");
        assert_eq!(pitch, p(A4_MIDI));
        assert!(cents.value().abs() < 0.01, "{} cents", cents.value());
    }

    #[test]
    fn a_flat_string_is_named_by_the_pitch_it_reaches_for() {
        let (pitch, cents) = Pitch::nearest(82.00).expect("82 Hz is representable");
        assert_eq!(pitch, p(E2));
        assert!(
            (cents.value() - -8.6).abs() < 0.1,
            "{} cents, expected about -8.6",
            cents.value()
        );
    }

    #[test]
    fn a_sharp_string_reads_positive() {
        let (pitch, cents) = Pitch::nearest(330.00).expect("330 Hz is representable");
        assert_eq!(pitch, p(E4));
        assert!(
            (cents.value() - 2.0).abs() < 0.1,
            "{} cents, expected about +2.0",
            cents.value()
        );
    }

    #[test]
    fn a_frequency_midway_between_two_pitches_takes_the_upper() {
        let midway = A4_HZ * 2.0f32.powf(50.0 / 1200.0);
        let (pitch, cents) = Pitch::nearest(midway).expect("midway is representable");

        assert_eq!(pitch, p(A4_MIDI + 1));
        assert_eq!(pitch.pitch_class(), PitchClass::new(10));
        assert_eq!(pitch.octave(), 4);
        assert!(
            (cents.value() - -50.0).abs() < 0.5,
            "{} cents, expected about -50",
            cents.value()
        );
    }

    /// Exhaustive rather than illustrative: the domain is 128 wide, so an off-by-one in
    /// the rounding has nowhere to hide.
    #[test]
    fn resolving_round_trips_with_the_frequency_of_a_pitch() {
        for midi in 0..=Pitch::MAX_MIDI {
            let pitch = p(midi);
            let (resolved, cents) = Pitch::nearest(pitch.frequency())
                .unwrap_or_else(|| panic!("MIDI {midi} resolves to nothing"));

            assert_eq!(resolved, pitch, "MIDI {midi} resolved to another pitch");
            assert!(
                cents.value().abs() < 0.05,
                "MIDI {midi} resolved {} cents off itself",
                cents.value()
            );
        }
    }

    #[test]
    fn an_unusable_frequency_names_no_pitch() {
        assert_eq!(Pitch::nearest(0.0), None);
        assert_eq!(Pitch::nearest(-440.0), None);
        assert_eq!(Pitch::nearest(f32::NAN), None);
        assert_eq!(Pitch::nearest(f32::INFINITY), None);
        assert_eq!(Pitch::nearest(f32::NEG_INFINITY), None);
        // Above G9 and below C-1: off the representable range at either end.
        assert_eq!(Pitch::nearest(30000.0), None);
        assert_eq!(Pitch::nearest(1.0), None);
    }
}
