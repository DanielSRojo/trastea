//! The instrument: string tuning, neck length, and a place on one.

use crate::music::notes::PitchClass;
use crate::music::pitch::Pitch;

use super::focus::Direction;

/// Concert pitch, not written: the low E written E3 sounds the E2 named here.
pub(super) const STANDARD_TUNING: [Pitch; 6] = [
    midi(40), // E2
    midi(45), // A2
    midi(50), // D3
    midi(55), // G3
    midi(59), // B3
    midi(64), // E4
];

/// `Pitch::from_midi` in const position, so a typo in the tuning panics at compile time.
const fn midi(number: u8) -> Pitch {
    match Pitch::from_midi(number) {
        Some(pitch) => pitch,
        None => panic!("the standard tuning names only representable pitches"),
    }
}

/// Where the keyboard cursor starts: the open low E, the neck's top-left corner.
pub(super) const CURSOR_HOME: Position = Position { string: 0, fret: 0 };
pub(super) const NECK_STRINGS: usize = STANDARD_TUNING.len();

/// A neck: what its strings are tuned to, and how many frets it has.
pub(super) struct Neck {
    tuning: [Pitch; NECK_STRINGS],
    frets: usize,
}

/// Twelve frets covers every pitch class on every string.
pub(super) const DRILL_NECK: Neck = Neck {
    tuning: STANDARD_TUNING,
    frets: 12,
};

/// Fourteen is the shortest that holds every CAGED shape; fifteen ends on an inlay.
pub(super) const CAGED_NECK: Neck = Neck {
    tuning: STANDARD_TUNING,
    frets: 15,
};

impl Neck {
    pub(super) const fn frets(&self) -> usize {
        self.frets
    }

    /// The pitch sounding at a position on this neck, or `None` off it.
    fn pitch_at(&self, string: usize, fret: usize) -> Option<Pitch> {
        (fret <= self.frets)
            .then(|| self.tuning.get(string))
            .flatten()
            .and_then(|open| open.transpose(i8::try_from(fret).ok()?))
    }

    /// Derived from [`Neck::pitch_at`] so the two readings cannot disagree.
    pub(super) fn pitch_class_at(&self, string: usize, fret: usize) -> Option<PitchClass> {
        self.pitch_at(string, fret).map(Pitch::pitch_class)
    }
}

/// Named fields so a root and target cannot be silently swapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Position {
    pub(super) string: usize,
    pub(super) fret: usize,
}

impl Position {
    /// Bound to `DRILL_NECK`, the only neck the drills draw.
    pub(super) fn pitch_class(self) -> Option<PitchClass> {
        DRILL_NECK.pitch_class_at(self.string, self.fret)
    }

    /// One arrow's move across the drill neck, stopping at its edges rather than wrapping.
    pub(super) fn step(self, direction: Direction) -> Position {
        let Position { string, fret } = self;
        match direction {
            Direction::Left => Position {
                string: string.saturating_sub(1),
                ..self
            },
            Direction::Right => Position {
                string: (string + 1).min(NECK_STRINGS - 1),
                ..self
            },
            Direction::Up => Position {
                fret: fret.saturating_sub(1),
                ..self
            },
            Direction::Down => Position {
                fret: (fret + 1).min(DRILL_NECK.frets()),
                ..self
            },
        }
    }
}

/// Every position on the neck, paired with the pitch class sounding there.
pub(super) fn neck() -> Vec<(Position, PitchClass)> {
    (0..NECK_STRINGS)
        .flat_map(|string| (0..=DRILL_NECK.frets()).map(move |fret| Position { string, fret }))
        .filter_map(|position| position.pitch_class().map(|pitch| (position, pitch)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the octave convention, which only a frequency can catch.
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
        // The fifth fret of the low E is the open A.
        assert_eq!(
            DRILL_NECK.pitch_at(0, 5),
            Some(STANDARD_TUNING[1]),
            "the fifth fret of the low E is not the open A"
        );
    }

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
        // The same string and fret, answered differently by the two necks.
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

    fn at(string: usize, fret: usize) -> Position {
        Position { string, fret }
    }

    #[test]
    fn a_step_stops_at_the_necks_edges() {
        let corner = at(0, 0);
        assert_eq!(corner.step(Direction::Left), corner, "walked off the low E");
        assert_eq!(corner.step(Direction::Up), corner, "walked off the nut");

        let far = at(NECK_STRINGS - 1, DRILL_NECK.frets());
        assert_eq!(far.step(Direction::Right), far, "walked off the high e");
        assert_eq!(far.step(Direction::Down), far, "walked off the last fret");
    }

    #[test]
    fn a_step_moves_one_position_at_a_time() {
        let start = at(2, 5);

        assert_eq!(start.step(Direction::Right), at(3, 5));
        assert_eq!(start.step(Direction::Down), at(2, 6));
        assert_eq!(start.step(Direction::Left), at(1, 5));
        // Up is towards the nut, because the neck is drawn with the nut at the top.
        assert_eq!(start.step(Direction::Up), at(2, 4));
    }
}
