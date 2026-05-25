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

    pub fn feel(self) -> &'static str {
        match self {
            ScaleKind::Ionian => {
                "Joyful, triumphant, stable, and wholesome. It is the definitive happy scale, though it can feel vanilla if overused."
            }
            ScaleKind::Dorian => {
                "Sophisticated, soulful, and cool minor. The major 6th lifts it out of standard minor darkness, giving it a hopeful or driving quality."
            }
            ScaleKind::Phrygian => {
                "Dark, tense, exotic, and heavy. The immediate flat 2 creates danger, malice, or ancient mystery."
            }
            ScaleKind::Lydian => {
                "Dreamy, spacey, celestial, and wondrous. The sharp 4 removes the standard major-scale resolution, so it feels like floating."
            }
            ScaleKind::Mixolydian => {
                "Dominant, gritty, bluesy, and celebratory. It is major with a rebellious, unpolished edge from the flat 7."
            }
            ScaleKind::Aeolian => {
                "Melancholic, sad, epic, and dramatic. Unlike Dorian, there is no bright 6th here; it is purely emotional and somber."
            }
            ScaleKind::Locrian => {
                "Highly unstable, chaotic, and tense. The diminished fifth prevents resolution and makes it feel claustrophobic."
            }
            ScaleKind::HarmonicMinor => {
                "Neoclassical, dramatic, and gothic. The leap between flat 6 and natural 7 gives it a vampire-castle classical flair."
            }
            ScaleKind::MelodicMinor => {
                "Mysterious, sophisticated, and cinematic. The major 6 and 7 make it sound almost major until the minor 3rd twist lands."
            }
            ScaleKind::MinorPentatonic => {
                "Raw, direct, and universally resonant. It strips away half-steps so every note feels strong and hard to misuse."
            }
            ScaleKind::MajorPentatonic => {
                "Sweet, uplifting, nostalgic, and bright. It removes tension and leaves a pure, joyful melody."
            }
            ScaleKind::Blues => {
                "Gritty, smoky, and expressive. The flat 5 blue note adds tension that begs to be bent or resolved."
            }
            ScaleKind::Voodoo => {
                "Psychedelic, slippery, and unstable. Minor and major 3rds alongside a flat 2 blur major blues with dark mysticism."
            }
            ScaleKind::SpanishGypsy => {
                "Middle Eastern, exotic, fiery, and hypnotic. The major 3rd plus flat 2 creates intense, passionate heat."
            }
            ScaleKind::WholeTone => {
                "Dream-sequence, dizzying, and rootless. Equal whole steps prevent the ear from finding a home note."
            }
            ScaleKind::Diminished => {
                "Symmetrical, tense, angular, and dark. It twists mechanically back on itself with constant shifting tension."
            }
        }
    }

    pub fn common_usage(self) -> &'static str {
        match self {
            ScaleKind::Ionian => {
                "Pop, classic rock anthems, country, and children's songs. Think the Star Wars theme or Let It Be."
            }
            ScaleKind::Dorian => {
                "Classic rock jams, funk, jazz modal playing, and blues-rock lines smoother than pentatonics. Think Oye Como Va or Breathe."
            }
            ScaleKind::Phrygian => {
                "Heavy metal, thrash riffs, and flamenco guitar. It is a staple for aggressive, looming Metallica-style tension."
            }
            ScaleKind::Lydian => {
                "Instrumental guitar virtuosos and cinematic sci-fi scores. Think Flying in a Blue Dream or magical John Williams moments."
            }
            ScaleKind::Mixolydian => {
                "Classic rock, blues, southern rock, and dominant 7th chords such as E7 or A7. Think Sweet Home Alabama."
            }
            ScaleKind::Aeolian => {
                "Rock ballads, heavy metal, neoclassical music, and sweeping dramatic melodies. Think Stairway to Heaven or Iron Maiden."
            }
            ScaleKind::Locrian => {
                "Rare in mainstream music; useful in extreme metal for dissonance or over half-diminished chords in jazz."
            }
            ScaleKind::HarmonicMinor => {
                "Shred guitar, prog metal, symphonic rock, and soloing over a major V chord in a minor key."
            }
            ScaleKind::MelodicMinor => {
                "Advanced jazz fusion and modern film scoring, especially for altered sounds over complex progressions."
            }
            ScaleKind::MinorPentatonic => {
                "The backbone of rock, blues, and hard rock soloing, from Led Zeppelin to AC/DC."
            }
            ScaleKind::MajorPentatonic => {
                "Country, southern rock, and R&B. Think sweeter BB King lines, Dickey Betts, or My Girl."
            }
            ScaleKind::Blues => {
                "Blues, jazz, and classic hard rock. It gives minor pentatonic an immediate attitude adjustment."
            }
            ScaleKind::Voodoo => {
                "Psychedelic rock jams, expressive fusion, and heavy experimental blues when you want an outside flavor."
            }
            ScaleKind::SpanishGypsy => {
                "Flamenco, progressive metal soloing, and surf-rock drama such as Misirlou."
            }
            ScaleKind::WholeTone => {
                "Dream sequences, psychedelic transitions, and jazz fusion blurs before landing back on a stable chord."
            }
            ScaleKind::Diminished => {
                "Metal riffs and jazz fusion solos, especially over dominant 7th chords that need outside tension before resolving."
            }
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
