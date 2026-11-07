//! Absolute pitch: which sound, as opposed to which of twelve.

use super::notes::PitchClass;

/// Named so a player can move it, e.g. to 442 for an orchestra.
const A4_HZ: f32 = 440.0;

const A4_MIDI: u8 = 69;

/// One sounding pitch in twelve-tone equal temperament, stored as a MIDI note number.
///
/// 0 is C-1, 60 is C4 (middle C), 69 is A4. No `Display`: naming it needs a `Spelling`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pitch(u8);

impl Pitch {
    const MAX_MIDI: u8 = 127;

    /// `None` past G9.
    pub const fn from_midi(midi: u8) -> Option<Pitch> {
        if midi <= Pitch::MAX_MIDI {
            Some(Pitch(midi))
        } else {
            None
        }
    }

    pub fn pitch_class(self) -> PitchClass {
        // works only because MIDI 0 is a C
        PitchClass::new(self.0 % 12)
    }

    /// The octave in scientific pitch notation, advancing at C; MIDI 0 is C-1, hence `i8`.
    pub fn octave(self) -> i8 {
        (self.0 / 12) as i8 - 1
    }

    /// This pitch moved by `semitones`, or `None` off either end of the range.
    ///
    /// Unlike [`PitchClass::transpose`] this never wraps, so a high E cannot become a low one.
    pub fn transpose(self, semitones: i8) -> Option<Pitch> {
        let midi = i16::from(self.0) + i16::from(semitones);
        u8::try_from(midi).ok().and_then(Pitch::from_midi)
    }

    /// What this pitch sounds at, in hertz. Only the tests go this direction.
    #[cfg(test)]
    pub fn frequency(self) -> f32 {
        A4_HZ * 2.0f32.powf((f32::from(self.0) - f32::from(A4_MIDI)) / 12.0)
    }

    /// The pitch nearest `hz`, and how far `hz` sits from it.
    ///
    /// `None` for a non-positive frequency or one off the representable range. The offset
    /// lands in −50..+50 cents; a tie at exactly 50 rounds up, as `f32::round` does.
    pub fn nearest(hz: f32) -> Option<(Pitch, Cents)> {
        // NaN and infinity are rejected here explicitly rather than by accident
        if !hz.is_finite() || hz <= 0.0 {
            return None;
        }

        let exact = f32::from(A4_MIDI) + 12.0 * (hz / A4_HZ).log2();
        let nearest = exact.round();

        // range-checked first because `as` on a float saturates to a plausible-looking G9
        if !(0.0..=f32::from(Pitch::MAX_MIDI)).contains(&nearest) {
            return None;
        }

        let pitch = Pitch::from_midi(nearest as u8)?;
        Some((pitch, Cents((exact - nearest) * 100.0)))
    }
}

/// A distance in cents, where 1200 is an octave and 100 a semitone.
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
        // the octave turns over at C, not at A
        assert_eq!(p(B3).transpose(1), Pitch::from_midi(60));
        assert_eq!(p(B3).transpose(1).map(Pitch::octave), Some(4));
        assert_eq!(p(60).transpose(11), Pitch::from_midi(71));
        assert_eq!(p(60).transpose(11).map(Pitch::octave), Some(4));
    }

    #[test]
    fn transposing_off_either_end_yields_nothing() {
        assert_eq!(p(Pitch::MAX_MIDI).transpose(1), None);
        assert_eq!(p(0).transpose(-1), None);
        assert_eq!(p(0).transpose(i8::MIN), None);
        assert_eq!(p(Pitch::MAX_MIDI).transpose(i8::MAX), None);
    }

    #[test]
    fn the_reference_pitch_is_exact() {
        assert_eq!(p(A4_MIDI).frequency(), A4_HZ);
    }

    /// Pins the octave convention: a guitarist's written low E is E3 and sounds E2.
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
        assert_eq!(Pitch::nearest(30000.0), None);
        assert_eq!(Pitch::nearest(1.0), None);
    }
}
