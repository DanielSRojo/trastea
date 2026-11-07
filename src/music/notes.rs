use std::fmt;

/// One of twelve pitch classes, always 0..=11. All arithmetic lives here.
///
/// No `Display`: naming one needs a spelling. See [`Spelling::spell`] and `Scale::spell`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PitchClass(u8);

impl PitchClass {
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
        // `semitones % 12` first, so the sum cannot overflow a u8
        PitchClass::new(self.0 + semitones % 12)
    }

    /// `transpose` for a step that can go down.
    pub fn shift(self, semitones: i32) -> PitchClass {
        self.transpose(semitones.rem_euclid(12) as u8)
    }

    /// How many semitones up from `self` to `other`, within the octave: always 0..=11.
    pub fn semitones_up_to(self, other: PitchClass) -> u8 {
        (other.0 + 12 - self.0) % 12
    }
}

/// The seven letter names, ordered from C so `as usize` indexes [`Letter::ALL`].
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

    /// `None` beyond ±2; triple accidentals are unrepresentable.
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

    /// The typewriter spelling for canvas text, which cannot use SMuFL glyphs. Empty for a natural.
    pub fn ascii(self) -> &'static str {
        match self {
            Accidental::DoubleFlat => "bb",
            Accidental::Flat => "b",
            Accidental::Natural => "",
            Accidental::Sharp => "#",
            Accidental::DoubleSharp => "##",
        }
    }

    /// The accidental said aloud, or `None` for a natural.
    pub fn spoken(self) -> Option<&'static str> {
        match self {
            Accidental::DoubleFlat => Some("double flat"),
            Accidental::Flat => Some("flat"),
            Accidental::Natural => None,
            Accidental::Sharp => Some("sharp"),
            Accidental::DoubleSharp => Some("double sharp"),
        }
    }
}

/// A note as written: a letter plus an accidental. Every pair is legitimate, B♯ and F♭♭ included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Note {
    pub letter: Letter,
    pub accidental: Accidental,
}

impl Note {
    /// The note said aloud — `C♯` reads "C sharp".
    pub fn spoken(self) -> String {
        match self.accidental.spoken() {
            Some(accidental) => format!("{} {accidental}", self.letter),
            None => self.letter.to_string(),
        }
    }

    pub fn pitch_class(self) -> PitchClass {
        let semitone =
            i16::from(self.letter.natural_semitone()) + i16::from(self.accidental.offset());
        PitchClass::new(
            u8::try_from(semitone.rem_euclid(12)).expect("rem_euclid(12) yields 0..=11"),
        )
    }
}

/// How far `target` sits from `letter`'s natural pitch, as the nearest signed
/// distance in −5..=6, so a difference reads −1 rather than +11.
pub fn nearest_offset(target: PitchClass, letter: Letter) -> i8 {
    let raw = (i16::from(target.semitone()) - i16::from(letter.natural_semitone())).rem_euclid(12);
    let folded = if raw > 6 { raw - 12 } else { raw };

    i8::try_from(folded).expect("the fold yields -5..=6")
}

/// ASCII, for the fretboard markers.
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

    /// What a black key is usually called — `C♯`, `E♭`, `F♯`, `A♭`, `B♭`.
    ///
    /// Only a tie-breaker for `Scale::new` when both spellings are equally economical;
    /// it never spells anything itself.
    pub fn conventional_for(pitch_class: PitchClass) -> Spelling {
        match pitch_class.semitone() {
            1 | 6 => Spelling::Sharps,
            _ => Spelling::Flats,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_distance_up_is_what_transposing_by_it_undoes() {
        for from in PitchClass::ALL {
            for to in PitchClass::ALL {
                let up = from.semitones_up_to(to);
                assert!(up < 12, "{from:?} to {to:?} is {up}");
                assert_eq!(from.transpose(up), to, "{from:?} up {up} is not {to:?}");
            }
        }
        assert_eq!(PitchClass::new(4).semitones_up_to(PitchClass::new(0)), 8);
    }

    #[test]
    fn a_shift_down_wraps_to_the_top_of_the_octave() {
        let c = PitchClass::new(0);
        assert_eq!(c.shift(-1), PitchClass::new(11));
        assert_eq!(c.shift(-13), PitchClass::new(11));
        assert_eq!(c.shift(12), c);

        for pitch_class in PitchClass::ALL {
            for steps in -25..=25 {
                assert_eq!(pitch_class.shift(steps).shift(-steps), pitch_class);
            }
        }
    }

    #[test]
    fn round_trip() {
        for pitch_class in PitchClass::ALL {
            assert_eq!(pitch_class, PitchClass::new(pitch_class.semitone()));
        }
    }

    /// Built from the letters, so a typo in `NATURALS` fails here.
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

        assert_eq!(Accidental::from_offset(3), None);
        assert_eq!(Accidental::from_offset(-3), None);
    }

    #[test]
    fn pitch_class_on_the_enharmonic_edges() {
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
    fn a_note_is_said_as_its_letter_and_its_accidental() {
        let said = |letter, accidental| Note { letter, accidental }.spoken();

        assert_eq!(said(Letter::C, Accidental::Natural), "C");
        assert_eq!(said(Letter::C, Accidental::Sharp), "C sharp");
        assert_eq!(said(Letter::B, Accidental::DoubleFlat), "B double flat");
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
    fn the_black_keys_have_a_conventional_name() {
        let cases = [
            (1, Letter::C, Accidental::Sharp),
            (3, Letter::E, Accidental::Flat),
            (6, Letter::F, Accidental::Sharp),
            (8, Letter::A, Accidental::Flat),
            (10, Letter::B, Accidental::Flat),
        ];

        for (semitone, letter, accidental) in cases {
            let pitch_class = PitchClass::new(semitone);
            let spelling = Spelling::conventional_for(pitch_class);

            assert_eq!(
                spelling.spell(pitch_class),
                Note { letter, accidental },
                "pitch class {semitone}"
            );
        }
    }

    #[test]
    fn a_naturals_convention_names_the_same_note_either_way() {
        for pitch_class in PitchClass::NATURALS {
            let conventional = Spelling::conventional_for(pitch_class).spell(pitch_class);

            assert_eq!(conventional, Spelling::Sharps.spell(pitch_class));
            assert_eq!(conventional, Spelling::Flats.spell(pitch_class));
        }
    }

    #[test]
    fn display_is_ascii_for_the_fretboard_markers() {
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
