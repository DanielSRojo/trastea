use std::fmt;

/// One of twelve pitch classes. All arithmetic lives here.
///
/// The field is private because it carries an invariant — always 0..=11 — and
/// `new` is the only way in. There is deliberately no `Display`: naming a pitch
/// class means choosing a letter, and that choice needs context this type does
/// not have. See [`Spelling::spell`] and `Scale::spell`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PitchClass(u8);

impl PitchClass {
    /// A fixed-size array, unlike the slices elsewhere: a miscount is then a
    /// compile error. It does not catch a thirteenth pitch class going unlisted,
    /// but there will never be one.
    pub const ALL: [PitchClass; 12] = [
        PitchClass::new(0),
        PitchClass::new(1),
        PitchClass::new(2),
        PitchClass::new(3),
        PitchClass::new(4),
        PitchClass::new(5),
        PitchClass::new(6),
        PitchClass::new(7),
        PitchClass::new(8),
        PitchClass::new(9),
        PitchClass::new(10),
        PitchClass::new(11),
    ];

    /// The seven pitch classes a letter names without an accidental — the white keys.
    ///
    /// Pitch classes rather than [`Letter`]s, because the callers that want this want to
    /// draw one at random and compare it to what is under a fret, and a letter would have
    /// to be converted at every such use. `Letter::ALL` stays private for the reason its
    /// own comment gives.
    pub const NATURALS: [PitchClass; 7] = [
        PitchClass::new(0),  // C
        PitchClass::new(2),  // D
        PitchClass::new(4),  // E
        PitchClass::new(5),  // F
        PitchClass::new(7),  // G
        PitchClass::new(9),  // A
        PitchClass::new(11), // B
    ];

    pub const fn new(semitone: u8) -> PitchClass {
        PitchClass(semitone % 12)
    }

    pub fn semitone(self) -> u8 {
        self.0
    }

    pub fn transpose(self, semitones: u8) -> PitchClass {
        // `semitones % 12` first, so the sum cannot exceed 22 and overflow a u8.
        PitchClass::new(self.0 + semitones % 12)
    }
}

/// The seven letter names, ordered from C rather than A, so `as usize` indexes
/// [`Letter::ALL`] and `step` is one modular add.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Letter {
    C,
    D,
    E,
    F,
    G,
    A,
    B,
}

impl Letter {
    /// Private: `step` is the only reason this list exists, and nothing outside
    /// this module needs to enumerate letters.
    const ALL: [Letter; 7] = [
        Letter::C,
        Letter::D,
        Letter::E,
        Letter::F,
        Letter::G,
        Letter::A,
        Letter::B,
    ];

    pub fn natural_semitone(self) -> u8 {
        match self {
            Letter::C => 0,
            Letter::D => 2,
            Letter::E => 4,
            Letter::F => 5,
            Letter::G => 7,
            Letter::A => 9,
            Letter::B => 11,
        }
    }

    /// Walks up the letters, wrapping past B: `D.step(2) == F`, `step(7) == self`.
    pub fn step(self, degrees: u8) -> Letter {
        Letter::ALL[(self as usize + degrees as usize) % Letter::ALL.len()]
    }
}

impl fmt::Display for Letter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let letter = match self {
            Letter::C => "C",
            Letter::D => "D",
            Letter::E => "E",
            Letter::F => "F",
            Letter::G => "G",
            Letter::A => "A",
            Letter::B => "B",
        };
        write!(f, "{letter}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Accidental {
    DoubleFlat,
    Flat,
    Natural,
    Sharp,
    DoubleSharp,
}

impl Accidental {
    pub fn offset(self) -> i8 {
        match self {
            Accidental::DoubleFlat => -2,
            Accidental::Flat => -1,
            Accidental::Natural => 0,
            Accidental::Sharp => 1,
            Accidental::DoubleSharp => 2,
        }
    }

    /// `None` beyond ±2. Triple accidentals are unreachable in this domain, so
    /// they stay unrepresentable rather than being rounded into range.
    pub fn from_offset(offset: i8) -> Option<Accidental> {
        match offset {
            -2 => Some(Accidental::DoubleFlat),
            -1 => Some(Accidental::Flat),
            0 => Some(Accidental::Natural),
            1 => Some(Accidental::Sharp),
            2 => Some(Accidental::DoubleSharp),
            _ => None,
        }
    }

    /// The typewriter spelling, for text the fretboard canvas draws: that canvas
    /// draws with one font, so the SMuFL glyphs the cards use cannot appear inside
    /// a marker. The empty string for a natural is the point of the table as much
    /// as the glyphs are — an explicit ♮ would be wrong in a note name and wrong
    /// in a scale degree alike.
    ///
    /// Shared rather than written out at each use: `Note` and `Interval` both
    /// render an accidental followed by something, and two copies of this table
    /// would be two things to keep in step.
    pub fn ascii(self) -> &'static str {
        match self {
            Accidental::DoubleFlat => "bb",
            Accidental::Flat => "b",
            Accidental::Natural => "",
            Accidental::Sharp => "#",
            Accidental::DoubleSharp => "##",
        }
    }
}

/// A note as written: a letter plus an accidental.
///
/// The fields are public because there is no invariant to hold — every pair is a
/// legitimate note, B♯ and F♭♭ included — so private fields would buy nothing
/// but ceremony.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Note {
    pub letter: Letter,
    pub accidental: Accidental,
}

impl Note {
    /// Total and infallible — the direction that needs no context.
    pub fn pitch_class(self) -> PitchClass {
        let semitone =
            i16::from(self.letter.natural_semitone()) + i16::from(self.accidental.offset());
        PitchClass::new(
            u8::try_from(semitone.rem_euclid(12)).expect("rem_euclid(12) yields 0..=11"),
        )
    }
}

/// ASCII, for the fretboard markers — see `Accidental::ascii`. The cards spell the
/// same notes with SMuFL glyphs instead.
impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.letter, self.accidental.ascii())
    }
}

/// The minimum context needed to name a pitch class on its own — the ♯/♭ toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Spelling {
    Sharps,
    Flats,
}

impl Spelling {
    /// The seven naturals are the same under both; the five black keys are the
    /// whole of the difference.
    pub fn spell(self, pitch_class: PitchClass) -> Note {
        let (letter, accidental) = match (pitch_class.semitone(), self) {
            (0, _) => (Letter::C, Accidental::Natural),
            (1, Spelling::Sharps) => (Letter::C, Accidental::Sharp),
            (1, Spelling::Flats) => (Letter::D, Accidental::Flat),
            (2, _) => (Letter::D, Accidental::Natural),
            (3, Spelling::Sharps) => (Letter::D, Accidental::Sharp),
            (3, Spelling::Flats) => (Letter::E, Accidental::Flat),
            (4, _) => (Letter::E, Accidental::Natural),
            (5, _) => (Letter::F, Accidental::Natural),
            (6, Spelling::Sharps) => (Letter::F, Accidental::Sharp),
            (6, Spelling::Flats) => (Letter::G, Accidental::Flat),
            (7, _) => (Letter::G, Accidental::Natural),
            (8, Spelling::Sharps) => (Letter::G, Accidental::Sharp),
            (8, Spelling::Flats) => (Letter::A, Accidental::Flat),
            (9, _) => (Letter::A, Accidental::Natural),
            (10, Spelling::Sharps) => (Letter::A, Accidental::Sharp),
            (10, Spelling::Flats) => (Letter::B, Accidental::Flat),
            (11, _) => (Letter::B, Accidental::Natural),
            (semitone, _) => unreachable!("PitchClass holds 0..=11, got {semitone}"),
        };

        Note { letter, accidental }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for pitch_class in PitchClass::ALL {
            assert_eq!(pitch_class, PitchClass::new(pitch_class.semitone()));
        }
    }

    /// Built from the letters rather than trusting the literal semitones, so a typo in
    /// `NATURALS` fails here instead of quietly drilling the wrong notes.
    #[test]
    fn the_naturals_are_exactly_what_the_letters_spell() {
        let from_letters: Vec<PitchClass> = [
            Letter::C,
            Letter::D,
            Letter::E,
            Letter::F,
            Letter::G,
            Letter::A,
            Letter::B,
        ]
        .into_iter()
        .map(|letter| PitchClass::new(letter.natural_semitone()))
        .collect();

        assert_eq!(PitchClass::NATURALS.to_vec(), from_letters);
    }

    // No test that the naturals spell without an accidental under both spellings, nor
    // that five accidentals are left over: both follow from the assertion above pinning
    // `NATURALS` to the letters, combined with `the_naturals_agree_under_both_spellings`.

    #[test]
    fn transpose_test() {
        assert_eq!(PitchClass::new(4).transpose(7), PitchClass::new(11));
    }

    #[test]
    fn wrapping() {
        assert_eq!(PitchClass::new(11).transpose(1), PitchClass::new(0));
    }

    #[test]
    fn letter_step_wraps_past_b() {
        assert_eq!(Letter::D.step(2), Letter::F);
        assert_eq!(Letter::B.step(1), Letter::C);

        for letter in Letter::ALL {
            assert_eq!(letter.step(7), letter, "{letter} moved a full octave");
        }
    }

    #[test]
    fn accidental_offsets_round_trip() {
        for accidental in [
            Accidental::DoubleFlat,
            Accidental::Flat,
            Accidental::Natural,
            Accidental::Sharp,
            Accidental::DoubleSharp,
        ] {
            assert_eq!(
                Accidental::from_offset(accidental.offset()),
                Some(accidental)
            );
        }

        // Triple accidentals have no representation, by design.
        assert_eq!(Accidental::from_offset(3), None);
        assert_eq!(Accidental::from_offset(-3), None);
    }

    #[test]
    fn pitch_class_on_the_enharmonic_edges() {
        // Where an off-by-one in the fold surfaces first.
        let cases = [
            (Letter::B, Accidental::Sharp, 0),
            (Letter::C, Accidental::Flat, 11),
            (Letter::F, Accidental::DoubleFlat, 3),
        ];

        for (letter, accidental, expected) in cases {
            let note = Note { letter, accidental };
            assert_eq!(note.pitch_class().semitone(), expected, "{note}");
        }
    }

    #[test]
    fn every_pitch_class_round_trips_through_both_spellings() {
        for spelling in [Spelling::Sharps, Spelling::Flats] {
            for pitch_class in PitchClass::ALL {
                assert_eq!(
                    spelling.spell(pitch_class).pitch_class(),
                    pitch_class,
                    "{spelling:?} lost {pitch_class:?}"
                );
            }
        }
    }

    #[test]
    fn sharps_never_yield_a_flat_and_flats_never_yield_a_sharp() {
        for pitch_class in PitchClass::ALL {
            assert_ne!(
                Spelling::Sharps.spell(pitch_class).accidental,
                Accidental::Flat
            );
            assert_ne!(
                Spelling::Flats.spell(pitch_class).accidental,
                Accidental::Sharp
            );
        }
    }

    #[test]
    fn the_naturals_agree_under_both_spellings() {
        for pitch_class in PitchClass::ALL {
            let sharp = Spelling::Sharps.spell(pitch_class);
            let flat = Spelling::Flats.spell(pitch_class);

            if sharp.accidental == Accidental::Natural {
                assert_eq!(sharp, flat, "{pitch_class:?} disagrees");
            }
        }
    }

    #[test]
    fn display_is_ascii_for_the_fretboard_markers() {
        // What a canvas marker draws — one font, so no SMuFL glyphs here.
        assert_eq!(
            Note {
                letter: Letter::B,
                accidental: Accidental::Flat
            }
            .to_string(),
            "Bb"
        );
        assert_eq!(
            Note {
                letter: Letter::F,
                accidental: Accidental::DoubleSharp
            }
            .to_string(),
            "F##"
        );
        assert_eq!(
            Note {
                letter: Letter::C,
                accidental: Accidental::Natural
            }
            .to_string(),
            "C"
        );
    }
}
