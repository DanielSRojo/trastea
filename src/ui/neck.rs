//! The instrument: what its strings are tuned to, how long a neck is, and a place on one.
//! Instrument knowledge rather than theory, which is why it is here and not in `music/`, and
//! shared by every screen that draws a neck, which is why it is not in any one of them.

use crate::music::notes::PitchClass;
use crate::music::pitch::Pitch;

/// Concert pitch — what the strings sound, not what a guitarist reads. Guitar is a
/// transposing instrument: its music is written an octave above where it sounds, so the
/// low E is written E3 and sounds the E2 named here. Getting this wrong is silent, because
/// everything an octave out still tunes correctly against itself; only a frequency catches
/// it, which is what `the_open_strings_sound_the_frequencies_a_tuner_expects` is for.
pub(super) const STANDARD_TUNING: [Pitch; 6] = [
    midi(40), // E2
    midi(45), // A2
    midi(50), // D3
    midi(55), // G3
    midi(59), // B3
    midi(64), // E4
];

/// `Pitch::from_midi` in const position, for the tuning above.
///
/// The panic is the point: it fires at compile time, so a typo in the tuning is a build
/// failure rather than a `None` the neck would have to answer for at runtime.
const fn midi(number: u8) -> Pitch {
    match Pitch::from_midi(number) {
        Some(pitch) => pitch,
        None => panic!("the standard tuning names only representable pitches"),
    }
}

/// Where the keyboard cursor starts: the open low E, the neck's top-left corner.
pub(super) const CURSOR_HOME: (usize, usize) = (0, 0);
pub(super) const NECK_STRINGS: usize = STANDARD_TUNING.len();

/// A neck: what its strings are tuned to, and how many frets it has.
///
/// A named value rather than a `frets: usize` parameter threaded through the placement
/// arithmetic, because the screens no longer agree on the answer. `place(.., 12)` and
/// `place(.., 15)` are both well-typed and swapping them silently lengthens or shortens a
/// neck; `DRILL_NECK` and `CAGED_NECK` are named once where they are chosen.
///
/// The tuning travels with the fret count because [`Neck::pitch_class_at`] needs both, and
/// taking the bound from `self` while reaching for a global for the notes is the worse of the
/// two. There is one tuning, and both consts name it.
pub(super) struct Neck {
    tuning: [Pitch; NECK_STRINGS],
    frets: usize,
}

/// The neck the three drills and the Chord Library draw. Twelve frets covers every pitch class
/// on every string, which is all a drill needs.
pub(super) const DRILL_NECK: Neck = Neck {
    tuning: STANDARD_TUNING,
    frets: 12,
};

/// The neck the CAGED window draws. Fifteen because twelve truncates the cycle — eight of the
/// twelve roots lose a shape to the neck's end, and C major loses its D shape, which reaches the
/// thirteenth fret. Fourteen is the shortest that holds every shape; fifteen is taken because
/// the inlays fall on 3-5-7-9-12-15, so the neck ends on a marked fret.
pub(super) const CAGED_NECK: Neck = Neck {
    tuning: STANDARD_TUNING,
    frets: 15,
};

impl Neck {
    pub(super) const fn frets(&self) -> usize {
        self.frets
    }

    /// The pitch sounding at a position on this neck, or `None` off it.
    ///
    /// `Option` rather than an index that panics: the callers are a click and a cursor, and a
    /// bounds bug should misjudge one answer rather than take the process down. The fret is
    /// bounded by `self.frets`, so the `i8` conversion cannot fail for any neck this app
    /// draws — but it is fallible all the same, on the same reasoning.
    fn pitch_at(&self, string: usize, fret: usize) -> Option<Pitch> {
        (fret <= self.frets)
            .then(|| self.tuning.get(string))
            .flatten()
            .and_then(|open| open.transpose(i8::try_from(fret).ok()?))
    }

    /// The note at a position on this neck, or `None` off it.
    ///
    /// Derived from [`Neck::pitch_at`] rather than computed beside it. Two parallel
    /// computations would only be unlikely to disagree; this one cannot, which is what the
    /// spec asks for. `the_two_readings_of_a_position_agree_everywhere` guards against
    /// someone un-deriving it.
    pub(super) fn pitch_class_at(&self, string: usize, fret: usize) -> Option<PitchClass> {
        self.pitch_at(string, fret).map(Pitch::pitch_class)
    }
}

/// One place on the neck.
///
/// Here rather than in a trainer, because a position is a fact about the instrument and
/// not about any one drill — the same reason `pitch_class_at` and `STANDARD_TUNING` live
/// in this module.
///
/// Named fields rather than the bare `(usize, usize)` the Note Trainer uses. That pair is
/// unambiguous there, where a prompt holds exactly one position; the Interval Trainer's
/// *Name it* prompt holds two of them, and a root swapped with a target is a silent bug
/// that inverts every interval on screen — a minor third read back as a major sixth, with
/// nothing failing to compile. `Position { string, fret }` makes that swap unwriteable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Position {
    pub(super) string: usize,
    pub(super) fret: usize,
}

impl Position {
    /// The pitch class sounding here, or `None` off the neck — `Neck::pitch_class_at`'s
    /// `Option` and its reasoning, reached through the named fields.
    ///
    /// Bound to `DRILL_NECK` because only the Interval Trainer holds a `Position`. A neck
    /// parameter here would be threaded through a dozen call sites to vary nothing.
    pub(super) fn pitch_class(self) -> Option<PitchClass> {
        DRILL_NECK.pitch_class_at(self.string, self.fret)
    }
}

/// Every position on the neck, paired with the pitch class sounding there.
///
/// The pitch class is carried along rather than asked for again later, which is what keeps
/// an `expect` out of the drawing code: a position that yielded `None` never enters the
/// list, so everything downstream has one in hand.
pub(super) fn neck() -> Vec<(Position, PitchClass)> {
    (0..NECK_STRINGS)
        .flat_map(|string| (0..=DRILL_NECK.frets()).map(move |fret| Position { string, fret }))
        .filter_map(|position| position.pitch_class().map(|pitch| (position, pitch)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the octave convention, which a pitch-class test cannot: a tuning an
    /// octave out spells every scale identically and tunes correctly against itself. Only
    /// a frequency catches it. A guitarist's written low E is E3 and sounds the E2 here.
    #[test]
    fn the_open_strings_sound_the_frequencies_a_tuner_expects() {
        for (string, expected) in [
            (0, 82.41),
            (1, 110.00),
            (2, 146.83),
            (3, 196.00),
            (4, 246.94),
            (5, 329.63),
        ] {
            let hz = STANDARD_TUNING[string].frequency();
            assert!(
                (hz - expected).abs() < 0.01,
                "string {string} sounds {hz} Hz, expected {expected}"
            );
        }

        assert_eq!(STANDARD_TUNING[0].octave(), 2, "the low E is E2, not E3");
    }

    #[test]
    fn the_tuning_spans_two_octaves_and_a_fourth() {
        let low = STANDARD_TUNING[0];
        let high = STANDARD_TUNING[NECK_STRINGS - 1];

        let span = (0..=24).find(|&semitones| low.transpose(semitones) == Some(high));

        assert_eq!(span, Some(24));
    }

    #[test]
    fn a_fretted_position_sounds_what_the_open_string_above_it_does() {
        // The fifth fret of the low E is the open A — the thing every guitarist tunes by.
        assert_eq!(
            DRILL_NECK.pitch_at(0, 5),
            Some(STANDARD_TUNING[1]),
            "the fifth fret of the low E is not the open A"
        );
    }

    /// The claim `pitch_class_at` rests on now that it is a derivation of `pitch_at`. It
    /// would pass trivially today; it exists to fail the day someone computes the two
    /// separately again.
    #[test]
    fn the_two_readings_of_a_position_agree_everywhere() {
        for neck in [&DRILL_NECK, &CAGED_NECK] {
            for string in 0..NECK_STRINGS {
                for fret in 0..=neck.frets() {
                    assert_eq!(
                        neck.pitch_at(string, fret).map(Pitch::pitch_class),
                        neck.pitch_class_at(string, fret),
                        "string {string} fret {fret} reads two ways"
                    );
                }
            }
        }
    }

    #[test]
    fn the_drawn_necks_span_the_range_they_claim() {
        let pitches = |neck: &Neck| -> Vec<Pitch> {
            (0..NECK_STRINGS)
                .flat_map(|string| {
                    (0..=neck.frets()).filter_map(move |fret| neck.pitch_at(string, fret))
                })
                .collect()
        };

        let drill = pitches(&DRILL_NECK);
        assert_eq!(
            drill.iter().min().copied(),
            STANDARD_TUNING.first().copied(),
            "the drills' neck should bottom out at the open low E, E2"
        );
        assert_eq!(
            drill.iter().max().copied(),
            STANDARD_TUNING[5].transpose(12),
            "the drills' neck should top out at E5"
        );

        let caged = pitches(&CAGED_NECK);
        assert_eq!(
            caged.iter().max().copied(),
            STANDARD_TUNING[5].transpose(15),
            "the CAGED neck should top out at G5"
        );
    }

    #[test]
    fn pitch_at_stops_where_the_neck_does() {
        // Off the end and off the side, on both necks, matching `pitch_class_at`.
        for neck in [&DRILL_NECK, &CAGED_NECK] {
            assert_eq!(neck.pitch_at(0, neck.frets() + 1), None);
            assert_eq!(neck.pitch_class_at(0, neck.frets() + 1), None);
            assert_eq!(neck.pitch_at(NECK_STRINGS, 0), None);
            assert_eq!(neck.pitch_class_at(NECK_STRINGS, 0), None);
        }

        assert_eq!(DRILL_NECK.pitch_at(0, 13), None);
        assert!(CAGED_NECK.pitch_at(0, 13).is_some());
    }

    #[test]
    fn each_neck_answers_only_for_its_own_frets() {
        // The same string and the same fret, answered differently by the two necks — which is
        // the whole reason the fret count travels in a value rather than a constant.
        for fret in 13..=CAGED_NECK.frets() {
            assert_eq!(
                CAGED_NECK.pitch_class_at(0, fret),
                Some(PitchClass::new(4).transpose(fret as u8)),
                "the CAGED neck is short at fret {fret}"
            );
            assert_eq!(
                DRILL_NECK.pitch_class_at(0, fret),
                None,
                "the drills' neck reaches fret {fret}"
            );
        }

        assert_eq!(CAGED_NECK.pitch_class_at(0, CAGED_NECK.frets() + 1), None);
        assert_eq!(
            DRILL_NECK.pitch_class_at(0, DRILL_NECK.frets()),
            Some(PitchClass::new(4))
        );
        // Off the fretboard sideways, on either neck.
        assert_eq!(CAGED_NECK.pitch_class_at(NECK_STRINGS, 0), None);
    }
}
