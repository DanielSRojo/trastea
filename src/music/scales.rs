// TODO: implement spell(semitone: u8, scale: &Scale) -> &str
// Returns the enharmonic spelling of a pitch in the context of a given scale.
// e.g. semitone 1 → "C#" in G major, "Db" in F minor.

use std::fmt;

use super::intervals::Interval;
use super::notes::Note;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleFormula {
    // Diatonic Modes
    Ionian, // Major scale
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Aeolian, // Natural minor
    Locrian,
    HarmonicMinor,
    MelodicMinor,
    MinorPentatonic,
    MajorPentatonic,
    Blues,
    Voodoo,
    SpanishGypsy, // Phrygian dominant
    WholeTone,
    Diminished, // Symmetric diminished
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scale {
    pub root: Note,
    pub formula: ScaleFormula,
}

impl fmt::Display for ScaleFormula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScaleFormula::Ionian => write!(f, "Ionian"),
            ScaleFormula::Dorian => write!(f, "Dorian"),
            ScaleFormula::Phrygian => write!(f, "Phrygian"),
            ScaleFormula::Lydian => write!(f, "Lydian"),
            ScaleFormula::Mixolydian => write!(f, "Mixolydian"),
            ScaleFormula::Aeolian => write!(f, "Aeolian"),
            ScaleFormula::Locrian => write!(f, "Locrian"),
            ScaleFormula::HarmonicMinor => write!(f, "Harmonic Minor"),
            ScaleFormula::MelodicMinor => write!(f, "Melodic Minor"),
            ScaleFormula::MinorPentatonic => write!(f, "Minor Pentatonic"),
            ScaleFormula::MajorPentatonic => write!(f, "Major Pentatonic"),
            ScaleFormula::Blues => write!(f, "Blues"),
            ScaleFormula::Voodoo => write!(f, "Voodoo"),
            ScaleFormula::SpanishGypsy => write!(f, "Spanish Gypsy"),
            ScaleFormula::WholeTone => write!(f, "Whole Tone"),
            ScaleFormula::Diminished => write!(f, "Diminished"),
        }
    }
}

impl Scale {
    pub fn notes(self) -> Vec<Note> {
        self.formula
            .intervals()
            .iter()
            .map(|interval| self.root.transpose(interval.to_semitone()))
            .collect()
    }
}
impl ScaleFormula {
    pub const ALL: &'static [ScaleFormula] = &[
        ScaleFormula::Ionian,
        ScaleFormula::Dorian,
        ScaleFormula::Phrygian,
        ScaleFormula::Lydian,
        ScaleFormula::Mixolydian,
        ScaleFormula::Aeolian,
        ScaleFormula::Locrian,
        ScaleFormula::HarmonicMinor,
        ScaleFormula::MelodicMinor,
        ScaleFormula::MinorPentatonic,
        ScaleFormula::MajorPentatonic,
        ScaleFormula::Blues,
        ScaleFormula::Voodoo,
        ScaleFormula::SpanishGypsy,
        ScaleFormula::WholeTone,
        ScaleFormula::Diminished,
    ];
    pub fn intervals(self) -> &'static [Interval] {
        match self {
            ScaleFormula::Ionian => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MajorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MajorSixth,
                Interval::MajorSeventh,
            ],
            ScaleFormula::Dorian => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MajorSixth,
                Interval::MinorSeventh,
            ],
            ScaleFormula::Phrygian => &[
                Interval::Unison,
                Interval::MinorSecond,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MinorSixth,
                Interval::MinorSeventh,
            ],
            ScaleFormula::Lydian => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MajorThird,
                Interval::AugmentedFourth,
                Interval::PerfectFifth,
                Interval::MajorSixth,
                Interval::MajorSeventh,
            ],
            ScaleFormula::Mixolydian => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MajorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MajorSixth,
                Interval::MinorSeventh,
            ],
            ScaleFormula::Aeolian => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MinorSixth,
                Interval::MinorSeventh,
            ],
            ScaleFormula::Locrian => &[
                Interval::Unison,
                Interval::MinorSecond,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::AugmentedFourth,
                Interval::MinorSixth,
                Interval::MinorSeventh,
            ],
            ScaleFormula::HarmonicMinor => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MinorSixth,
                Interval::MajorSeventh,
            ],
            ScaleFormula::MelodicMinor => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MajorSixth,
                Interval::MajorSeventh,
            ],
            ScaleFormula::MinorPentatonic => &[
                Interval::Unison,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MinorSeventh,
            ],
            ScaleFormula::MajorPentatonic => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MajorThird,
                Interval::PerfectFifth,
                Interval::MajorSixth,
            ],
            ScaleFormula::Blues => &[
                Interval::Unison,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::AugmentedFourth,
                Interval::PerfectFifth,
                Interval::MinorSeventh,
            ],
            ScaleFormula::Voodoo => &[
                Interval::Unison,
                Interval::MinorSecond,
                Interval::MinorThird,
                Interval::MajorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MajorSixth,
                Interval::MinorSeventh,
            ],
            ScaleFormula::SpanishGypsy => &[
                Interval::Unison,
                Interval::MinorSecond,
                Interval::MajorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MinorSixth,
                Interval::MinorSeventh,
            ],
            ScaleFormula::WholeTone => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MajorThird,
                Interval::AugmentedFourth,
                Interval::MinorSixth,
                Interval::MinorSeventh,
            ],
            ScaleFormula::Diminished => &[
                Interval::Unison,
                Interval::MinorSecond,
                Interval::MinorThird,
                Interval::MajorThird,
                Interval::AugmentedFourth,
                Interval::PerfectFifth,
                Interval::MajorSixth,
                Interval::MinorSeventh,
            ],
        }
    }
}
