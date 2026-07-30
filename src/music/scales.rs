// TODO: implement spell(semitone: u8, scale: &Scale) -> &str
// Returns the enharmonic spelling of a pitch in the context of a given scale.
// e.g. semitone 1 → "C#" in G major, "Db" in F minor.

use super::intervals::Interval;
use super::notes::PitchClass;

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
    pub root: PitchClass,
    pub kind: ScaleKind,
}

impl Scale {
    pub fn notes(self) -> Vec<PitchClass> {
        self.kind
            .intervals()
            .iter()
            .map(|interval| self.root.transpose(interval.semitones()))
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
            ScaleKind::Dorian => "1 2 \u{E260}3 4 5 6 \u{E260}7",
            ScaleKind::Phrygian => "1 \u{E260}2 \u{E260}3 4 5 \u{E260}6 \u{E260}7",
            ScaleKind::Lydian => "1 2 3 \u{E262}4 5 6 7",
            ScaleKind::Mixolydian => "1 2 3 4 5 6 \u{E260}7",
            ScaleKind::Aeolian => "1 2 \u{E260}3 4 5 \u{E260}6 \u{E260}7",
            ScaleKind::Locrian => "1 \u{E260}2 \u{E260}3 4 \u{E260}5 \u{E260}6 \u{E260}7",
            ScaleKind::HarmonicMinor => "1 2 \u{E260}3 4 5 \u{E260}6 7",
            ScaleKind::MelodicMinor => "1 2 \u{E260}3 4 5 6 7",
            ScaleKind::MinorPentatonic => "1 \u{E260}3 4 5 \u{E260}7",
            ScaleKind::MajorPentatonic => "1 2 3 5 6",
            ScaleKind::Blues => "1 \u{E260}3 4 \u{E260}5 5 \u{E260}7",
            ScaleKind::Voodoo => "1 \u{E260}2 \u{E260}3 3 4 5 6 \u{E260}7",
            ScaleKind::SpanishGypsy => "1 \u{E260}2 3 4 5 \u{E260}6 \u{E260}7",
            ScaleKind::WholeTone => "1 2 3 \u{E262}4 \u{E260}6 \u{E260}7",
            ScaleKind::Diminished => "1 \u{E260}2 \u{E260}3 3 \u{E260}5 5 6 \u{E260}7",
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
                Interval::DiminishedFifth,
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
                Interval::DiminishedFifth,
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
                Interval::DiminishedFifth,
                Interval::PerfectFifth,
                Interval::MajorSixth,
                Interval::MinorSeventh,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMUFL_FLAT: char = '\u{E260}';
    const SMUFL_SHARP: char = '\u{E262}';

    /// The semitones a scale spans above its root.
    fn semitones(kind: ScaleKind) -> Vec<u8> {
        kind.intervals()
            .iter()
            .map(|interval| interval.semitones())
            .collect()
    }

    /// A pitch class from its semitone, for the tables below.
    fn pc(semitone: u8) -> PitchClass {
        PitchClass::new(semitone)
    }

    /// A scale's pitch classes, sorted, for comparing scales that share notes but
    /// start in different places.
    fn pitch_classes(root: PitchClass, kind: ScaleKind) -> Vec<u8> {
        let mut classes: Vec<u8> = Scale { root, kind }
            .notes()
            .iter()
            .map(|pitch_class| pitch_class.semitone())
            .collect();
        classes.sort_unstable();
        classes
    }

    /// Semitones above the root of the unaltered degrees, i.e. the major scale.
    fn degree_semitone(degree: u32) -> i16 {
        match degree {
            1 => 0,
            2 => 2,
            3 => 4,
            4 => 5,
            5 => 7,
            6 => 9,
            7 => 11,
            other => panic!("{other} is not a scale degree"),
        }
    }

    /// Reads a formula like `1 ♭3 4 ♭5 5 ♭7` into semitones above the root.
    ///
    /// Going through semitones rather than comparing text is deliberate: `♭5` and
    /// `♯4` name the same distance, and the two are not interchangeable as strings.
    fn parse_intervalic(formula: &str) -> Vec<u8> {
        formula
            .split_whitespace()
            .map(|token| {
                let mut chars = token.chars();
                let first = chars.next().expect("split_whitespace yields no empties");

                let (accidental, digit) = match first {
                    SMUFL_FLAT => (-1, chars.next().expect("a flat precedes a degree")),
                    SMUFL_SHARP => (1, chars.next().expect("a sharp precedes a degree")),
                    digit => (0, digit),
                };

                let degree = digit.to_digit(10).expect("a degree is a digit");
                let semitone = degree_semitone(degree) + accidental;

                u8::try_from(semitone).expect("a degree does not fall below the root")
            })
            .collect()
    }

    #[test]
    fn all_lists_every_kind() {
        // Adding a variant already fails to compile in name, intervalic, feel,
        // common_usage and intervals, which all match exhaustively. ALL is the one
        // place the compiler cannot help, so the count is pinned here instead.
        assert_eq!(ScaleKind::ALL.len(), 16);
    }

    #[test]
    fn interval_lists_ascend_from_the_root_without_repeats() {
        for &kind in ScaleKind::ALL {
            let semitones = semitones(kind);

            assert_eq!(
                semitones.first(),
                Some(&0),
                "{} does not start on its root",
                kind.name()
            );

            for pair in semitones.windows(2) {
                assert!(
                    pair[0] < pair[1],
                    "{} is out of order or repeats at {pair:?}",
                    kind.name()
                );
            }
        }
    }

    #[test]
    fn no_two_kinds_share_a_name_or_an_interval_list() {
        for (i, &kind) in ScaleKind::ALL.iter().enumerate() {
            for &other in &ScaleKind::ALL[i + 1..] {
                assert_ne!(kind.name(), other.name(), "duplicated name");
                assert_ne!(
                    kind.intervals(),
                    other.intervals(),
                    "{} and {} have identical intervals",
                    kind.name(),
                    other.name()
                );
            }
        }
    }

    #[test]
    fn the_intervalic_formula_agrees_with_the_interval_list() {
        // The two are written out separately, so they can drift apart. This is what
        // catches it.
        for &kind in ScaleKind::ALL {
            assert_eq!(
                parse_intervalic(kind.intervalic()),
                semitones(kind),
                "{}: formula {:?} and interval list disagree",
                kind.name(),
                kind.intervalic()
            );
        }
    }

    #[test]
    fn the_tritone_kinds_use_the_degree_they_are_written_with() {
        // ♯4 and ♭5 are the same distance, so putting one where the other belongs
        // is invisible to every other test here: the formula check compares
        // semitones, and the spelling only diverges once a scale is spelled from a
        // root. Lydian's ♯4 really is a sharpened fourth; Locrian's ♭5 really is a
        // flattened fifth. This is the assertion that keeps them apart.
        for kind in [ScaleKind::Lydian, ScaleKind::WholeTone] {
            let intervals = kind.intervals();
            assert!(
                intervals.contains(&Interval::AugmentedFourth),
                "{} is written with a ♯4",
                kind.name()
            );
            assert!(
                !intervals.contains(&Interval::DiminishedFifth),
                "{} has no ♭5",
                kind.name()
            );
        }

        for kind in [ScaleKind::Locrian, ScaleKind::Blues, ScaleKind::Diminished] {
            let intervals = kind.intervals();
            assert!(
                intervals.contains(&Interval::DiminishedFifth),
                "{} is written with a ♭5",
                kind.name()
            );
            assert!(
                !intervals.contains(&Interval::AugmentedFourth),
                "{} has no ♯4",
                kind.name()
            );
        }
    }

    #[test]
    fn notes_are_distinct_and_match_the_interval_count() {
        for root in PitchClass::ALL {
            for &kind in ScaleKind::ALL {
                let notes = Scale { root, kind }.notes();

                assert_eq!(
                    notes.len(),
                    kind.intervals().len(),
                    "{root:?} {} lost a note",
                    kind.name()
                );

                for (i, note) in notes.iter().enumerate() {
                    assert!(
                        !notes[..i].contains(note),
                        "{root:?} {} repeats {note:?}",
                        kind.name()
                    );
                }
            }
        }
    }

    #[test]
    fn reference_scales_have_the_textbook_pitch_classes() {
        // Ordered from the root and wrapping past 11, not sorted: A Aeolian ends
        // on 7 rather than starting at 0.
        let cases: &[(u8, ScaleKind, &[u8])] = &[
            (0, ScaleKind::Ionian, &[0, 2, 4, 5, 7, 9, 11]),
            (7, ScaleKind::Ionian, &[7, 9, 11, 0, 2, 4, 6]),
            (9, ScaleKind::Aeolian, &[9, 11, 0, 2, 4, 5, 7]),
            (4, ScaleKind::Phrygian, &[4, 5, 7, 9, 11, 0, 2]),
            (0, ScaleKind::HarmonicMinor, &[0, 2, 3, 5, 7, 8, 11]),
            (9, ScaleKind::MinorPentatonic, &[9, 0, 2, 4, 7]),
            // The ♭5 between D and E is the blue note.
            (9, ScaleKind::Blues, &[9, 0, 2, 3, 4, 7]),
            (0, ScaleKind::WholeTone, &[0, 2, 4, 6, 8, 10]),
        ];

        for &(root, kind, expected) in cases {
            let actual: Vec<u8> = Scale {
                root: pc(root),
                kind,
            }
            .notes()
            .iter()
            .map(|pitch_class| pitch_class.semitone())
            .collect();

            assert_eq!(actual, expected, "{root} {}", kind.name());
        }
    }

    #[test]
    fn the_seven_diatonic_modes_share_one_pitch_class_set() {
        // Each mode rooted on its own degree of C major must yield exactly C major's
        // notes. One wrong interval anywhere in the seven breaks this.
        let c_major = pitch_classes(pc(0), ScaleKind::Ionian);

        for (root, kind) in [
            (0, ScaleKind::Ionian),
            (2, ScaleKind::Dorian),
            (4, ScaleKind::Phrygian),
            (5, ScaleKind::Lydian),
            (7, ScaleKind::Mixolydian),
            (9, ScaleKind::Aeolian),
            (11, ScaleKind::Locrian),
        ] {
            assert_eq!(
                pitch_classes(pc(root), kind),
                c_major,
                "{root} {} is not a mode of C major",
                kind.name()
            );
        }
    }

    #[test]
    fn a_relative_minor_shares_its_majors_notes() {
        assert_eq!(
            pitch_classes(pc(9), ScaleKind::Aeolian),
            pitch_classes(pc(0), ScaleKind::Ionian)
        );
        assert_eq!(
            pitch_classes(pc(4), ScaleKind::Aeolian),
            pitch_classes(pc(7), ScaleKind::Ionian)
        );
    }

    #[test]
    fn the_symmetric_scales_repeat_under_transposition() {
        // Whole tone maps onto itself a whole step up, and the diminished scale a
        // minor third up. Getting one of their steps wrong breaks the symmetry.
        assert_eq!(
            pitch_classes(pc(0), ScaleKind::WholeTone),
            pitch_classes(pc(2), ScaleKind::WholeTone)
        );
        assert_eq!(
            pitch_classes(pc(0), ScaleKind::Diminished),
            pitch_classes(pc(3), ScaleKind::Diminished)
        );
    }
}
