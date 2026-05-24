// TODO: implement spell(semitone: u8, scale: &Scale) -> &str
// Returns the enharmonic spelling of a pitch in the context of a given scale.
// e.g. semitone 1 → "C#" in G major, "Db" in F minor.

use super::intervals::Interval;
use super::notes::Note;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleKind {
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
    pub kind: ScaleKind,
}

impl Scale {
    pub fn notes(self) -> Vec<Note> {
        self.kind
            .intervals()
            .iter()
            .map(|interval| self.root.transpose(interval.to_semitone()))
            .collect()
    }
}
impl ScaleKind {
    pub const ALL: &'static [ScaleKind] = &[
        ScaleKind::Ionian,
        ScaleKind::Dorian,
        ScaleKind::Phrygian,
        ScaleKind::Lydian,
        ScaleKind::Mixolydian,
        ScaleKind::Aeolian,
        ScaleKind::Locrian,
        ScaleKind::HarmonicMinor,
        ScaleKind::MelodicMinor,
        ScaleKind::MinorPentatonic,
        ScaleKind::MajorPentatonic,
        ScaleKind::Blues,
        ScaleKind::Voodoo,
        ScaleKind::SpanishGypsy,
        ScaleKind::WholeTone,
        ScaleKind::Diminished,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ScaleKind::Ionian => "Ionian",
            ScaleKind::Dorian => "Dorian",
            ScaleKind::Phrygian => "Phrygian",
            ScaleKind::Lydian => "Lydian",
            ScaleKind::Mixolydian => "Mixolydian",
            ScaleKind::Aeolian => "Aeolian",
            ScaleKind::Locrian => "Locrian",
            ScaleKind::HarmonicMinor => "Harmonic Minor",
            ScaleKind::MelodicMinor => "Melodic Minor",
            ScaleKind::MinorPentatonic => "Minor Pentatonic",
            ScaleKind::MajorPentatonic => "Major Pentatonic",
            ScaleKind::Blues => "Blues",
            ScaleKind::Voodoo => "Voodoo",
            ScaleKind::SpanishGypsy => "Spanish Gypsy",
            ScaleKind::WholeTone => "Whole Tone",
            ScaleKind::Diminished => "Diminished",
        }
    }

    pub fn intervalic(self) -> &'static str {
        match self {
            ScaleKind::Ionian => "1 2 3 4 5 6 7",
            ScaleKind::Dorian => "1 2 ♭3 4 5 6 ♭7",
            ScaleKind::Phrygian => "1 ♭2 ♭3 4 5 ♭6 ♭7",
            ScaleKind::Lydian => "1 2 3 ♯4 5 6 7",
            ScaleKind::Mixolydian => "1 2 3 4 5 6 ♭7",
            ScaleKind::Aeolian => "1 2 ♭3 4 5 ♭6 ♭7",
            ScaleKind::Locrian => "1 ♭2 ♭3 4 ♭5 ♭6 ♭7",
            ScaleKind::HarmonicMinor => "1 2 ♭3 4 5 ♭6 7",
            ScaleKind::MelodicMinor => "1 2 ♭3 4 5 6 7",
            ScaleKind::MinorPentatonic => "1 ♭3 4 5 ♭7",
            ScaleKind::MajorPentatonic => "1 2 3 5 6",
            ScaleKind::Blues => "1 ♭3 4 ♭5 5 ♭7",
            ScaleKind::Voodoo => "1 ♭2 ♭3 3 4 5 6 ♭7",
            ScaleKind::SpanishGypsy => "1 ♭2 3 4 5 ♭6 ♭7",
            ScaleKind::WholeTone => "1 2 3 ♯4 ♭6 ♭7",
            ScaleKind::Diminished => "1 ♭2 ♭3 3 ♭5 5 6 ♭7",
        }
    }

    pub fn intervals(self) -> &'static [Interval] {
        match self {
            ScaleKind::Ionian => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MajorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MajorSixth,
                Interval::MajorSeventh,
            ],
            ScaleKind::Dorian => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MajorSixth,
                Interval::MinorSeventh,
            ],
            ScaleKind::Phrygian => &[
                Interval::Unison,
                Interval::MinorSecond,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MinorSixth,
                Interval::MinorSeventh,
            ],
            ScaleKind::Lydian => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MajorThird,
                Interval::AugmentedFourth,
                Interval::PerfectFifth,
                Interval::MajorSixth,
                Interval::MajorSeventh,
            ],
            ScaleKind::Mixolydian => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MajorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MajorSixth,
                Interval::MinorSeventh,
            ],
            ScaleKind::Aeolian => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MinorSixth,
                Interval::MinorSeventh,
            ],
            ScaleKind::Locrian => &[
                Interval::Unison,
                Interval::MinorSecond,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::AugmentedFourth,
                Interval::MinorSixth,
                Interval::MinorSeventh,
            ],
            ScaleKind::HarmonicMinor => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MinorSixth,
                Interval::MajorSeventh,
            ],
            ScaleKind::MelodicMinor => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MajorSixth,
                Interval::MajorSeventh,
            ],
            ScaleKind::MinorPentatonic => &[
                Interval::Unison,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MinorSeventh,
            ],
            ScaleKind::MajorPentatonic => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MajorThird,
                Interval::PerfectFifth,
                Interval::MajorSixth,
            ],
            ScaleKind::Blues => &[
                Interval::Unison,
                Interval::MinorThird,
                Interval::PerfectFourth,
                Interval::AugmentedFourth,
                Interval::PerfectFifth,
                Interval::MinorSeventh,
            ],
            ScaleKind::Voodoo => &[
                Interval::Unison,
                Interval::MinorSecond,
                Interval::MinorThird,
                Interval::MajorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MajorSixth,
                Interval::MinorSeventh,
            ],
            ScaleKind::SpanishGypsy => &[
                Interval::Unison,
                Interval::MinorSecond,
                Interval::MajorThird,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
                Interval::MinorSixth,
                Interval::MinorSeventh,
            ],
            ScaleKind::WholeTone => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::MajorThird,
                Interval::AugmentedFourth,
                Interval::MinorSixth,
                Interval::MinorSeventh,
            ],
            ScaleKind::Diminished => &[
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
