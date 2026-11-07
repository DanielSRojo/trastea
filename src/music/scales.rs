use std::cmp::Ordering;

use super::intervals::Interval;
use super::notes::{Accidental, Note, PitchClass, Spelling, nearest_offset};

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

/// All fields are private: `spelling` is derived from `root` and `kind`, so none may change alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scale {
    root: PitchClass,
    kind: ScaleKind,
    spelling: Spelling,
}

impl Scale {
    pub fn root(self) -> PitchClass {
        self.root
    }

    pub fn kind(self) -> ScaleKind {
        self.kind
    }

    /// Picks the spelling a scale would be taught in: fewest double accidentals, then
    /// fewest accidentals overall, with `Spelling::conventional_for` breaking ties.
    pub fn new(root: PitchClass, kind: ScaleKind) -> Self {
        let candidate = |spelling| Scale {
            root,
            kind,
            spelling,
        };
        let (sharps, flats) = (candidate(Spelling::Sharps), candidate(Spelling::Flats));

        match sharps.spelling_cost().cmp(&flats.spelling_cost()) {
            Ordering::Less => sharps,
            Ordering::Greater => flats,
            Ordering::Equal => candidate(Spelling::conventional_for(root)),
        }
    }

    /// How much ink this spelling costs: `(double accidentals, total accidental distance)`.
    ///
    /// Doubles come first so harmonic minor on pitch class 8, six accidentals either
    /// way, picks `A♭` because `G♯` needs an `F𝄪` rather than by falling to convention.
    fn spelling_cost(self) -> (usize, u32) {
        self.notes()
            .iter()
            .map(|note| u32::from(note.accidental.offset().unsigned_abs()))
            .fold((0, 0), |(doubles, total), offset| {
                (doubles + usize::from(offset == 2), total + offset)
            })
    }

    pub fn root_note(self) -> Note {
        self.spelling.spell(self.root)
    }

    /// Every degree spelled: the degree number forces the letter, and the accidental
    /// is whatever the arithmetic then demands.
    ///
    /// Total because a degree's offset never exceeds ±2, which
    /// `every_scale_spells_without_failing` pins by exhaustion over all 384 scales.
    pub fn notes(self) -> Vec<Note> {
        let root = self.root_note();

        self.kind
            .intervals()
            .iter()
            .map(|interval| {
                let letter = root.letter.step(interval.number() - 1);
                let target = self.root.transpose(interval.semitones());
                let accidental = Accidental::from_offset(nearest_offset(target, letter))
                    .expect("every_scale_spells_without_failing proves ±2 is enough");

                Note { letter, accidental }
            })
            .collect()
    }

    /// This scale's name for a pitch class, or `None` when it is not in the scale.
    pub fn spell(self, pitch_class: PitchClass) -> Option<Note> {
        self.notes()
            .into_iter()
            .find(|note| note.pitch_class() == pitch_class)
    }

    /// This scale's degree for a pitch class, or `None` when it is not in the scale.
    /// Independent of `spelling`: no choice of sharps or flats moves a degree.
    pub fn degree(self, pitch_class: PitchClass) -> Option<Interval> {
        self.kind
            .intervals()
            .iter()
            .copied()
            .find(|interval| self.root.transpose(interval.semitones()) == pitch_class)
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
            // ♭6 ♭7 rather than the textbook ♯5 ♯6, which would push an A♯ root past ±2
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
    use crate::music::notes::Letter;
    use Accidental::{DoubleFlat, DoubleSharp, Flat, Natural, Sharp};
    use Letter::{A, B, C, D, E, F, G};

    fn spelled(letter: Letter, accidental: Accidental) -> Note {
        Note { letter, accidental }
    }

    fn semitones(kind: ScaleKind) -> Vec<u8> {
        kind.intervals()
            .iter()
            .map(|interval| interval.semitones())
            .collect()
    }

    fn pc(semitone: u8) -> PitchClass {
        PitchClass::new(semitone)
    }

    /// A scale's pitch classes, sorted, for comparing scales that start in different places.
    fn pitch_classes(root: PitchClass, kind: ScaleKind) -> Vec<u8> {
        let mut classes: Vec<u8> = Scale {
            root,
            spelling: Spelling::Sharps,
            kind,
        }
        .notes()
        .iter()
        .map(|note| note.pitch_class().semitone())
        .collect();
        classes.sort_unstable();
        classes
    }

    #[test]
    fn all_lists_every_kind() {
        // the one list the compiler cannot check for a new variant
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
    fn the_tritone_kinds_use_the_degree_they_are_written_with() {
        // ♯4 and ♭5 are the same distance, so only the degree number keeps them apart
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
    fn the_seven_diatonic_modes_share_one_pitch_class_set() {
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
        assert_eq!(
            pitch_classes(pc(0), ScaleKind::WholeTone),
            pitch_classes(pc(2), ScaleKind::WholeTone)
        );
        assert_eq!(
            pitch_classes(pc(0), ScaleKind::Diminished),
            pitch_classes(pc(3), ScaleKind::Diminished)
        );
    }

    #[test]
    fn every_scale_spells_without_failing() {
        // 12 pitch classes × 2 spellings × 16 kinds = 384; this licenses the expect in notes().
        // Builds literals rather than using `Scale::new`, which avoids the costliest spellings.
        let mut checked = 0;

        for spelling in [Spelling::Sharps, Spelling::Flats] {
            for root in PitchClass::ALL {
                for &kind in ScaleKind::ALL {
                    let scale = Scale {
                        root,
                        spelling,
                        kind,
                    };
                    let notes = scale.notes();

                    assert_eq!(
                        notes.len(),
                        kind.intervals().len(),
                        "{} {} lost a note",
                        scale.root_note(),
                        kind.name()
                    );

                    // Spelling::spell and the letter walk must agree on the root
                    assert_eq!(
                        notes[0],
                        scale.root_note(),
                        "{} {} disagrees with root_note() on its own root",
                        scale.root_note(),
                        kind.name()
                    );

                    let mut pitch_classes: Vec<u8> = notes
                        .iter()
                        .map(|note| note.pitch_class().semitone())
                        .collect();
                    pitch_classes.sort_unstable();
                    let total = pitch_classes.len();
                    pitch_classes.dedup();

                    assert_eq!(
                        pitch_classes.len(),
                        total,
                        "{} {} repeats a pitch class",
                        scale.root_note(),
                        kind.name()
                    );

                    checked += 1;
                }
            }
        }

        assert_eq!(checked, 384, "the sweep did not cover every scale");
    }

    #[test]
    fn seven_note_scales_use_each_letter_once() {
        // the eight-note kinds correctly reuse a letter and are excluded
        let mut checked = 0;

        for spelling in [Spelling::Sharps, Spelling::Flats] {
            for root in PitchClass::ALL {
                for &kind in ScaleKind::ALL {
                    if kind.intervals().len() != 7 {
                        continue;
                    }

                    let scale = Scale {
                        root,
                        spelling,
                        kind,
                    };
                    let letters: Vec<Letter> =
                        scale.notes().iter().map(|note| note.letter).collect();

                    for (i, letter) in letters.iter().enumerate() {
                        assert!(
                            !letters[..i].contains(letter),
                            "{} {} repeats the letter {letter}",
                            scale.root_note(),
                            kind.name()
                        );
                    }

                    checked += 1;
                }
            }
        }

        assert_eq!(
            checked,
            12 * 2 * 10,
            "the seven-note kinds were not all swept"
        );
    }

    #[test]
    fn reference_scales_have_the_textbook_notes() {
        // ordered from the root and wrapping past B, not sorted by pitch
        let cases: &[(u8, Spelling, ScaleKind, &[Note])] = &[
            (
                5,
                Spelling::Sharps,
                ScaleKind::Ionian,
                &[
                    spelled(F, Natural),
                    spelled(G, Natural),
                    spelled(A, Natural),
                    spelled(B, Flat),
                    spelled(C, Natural),
                    spelled(D, Natural),
                    spelled(E, Natural),
                ],
            ),
            (
                0,
                Spelling::Sharps,
                ScaleKind::HarmonicMinor,
                &[
                    spelled(C, Natural),
                    spelled(D, Natural),
                    spelled(E, Flat),
                    spelled(F, Natural),
                    spelled(G, Natural),
                    spelled(A, Flat),
                    spelled(B, Natural),
                ],
            ),
            (
                3,
                Spelling::Flats,
                ScaleKind::Ionian,
                &[
                    spelled(E, Flat),
                    spelled(F, Natural),
                    spelled(G, Natural),
                    spelled(A, Flat),
                    spelled(B, Flat),
                    spelled(C, Natural),
                    spelled(D, Natural),
                ],
            ),
            (
                1,
                Spelling::Sharps,
                ScaleKind::Ionian,
                &[
                    spelled(C, Sharp),
                    spelled(D, Sharp),
                    spelled(E, Sharp),
                    spelled(F, Sharp),
                    spelled(G, Sharp),
                    spelled(A, Sharp),
                    spelled(B, Sharp),
                ],
            ),
            (
                9,
                Spelling::Sharps,
                ScaleKind::Blues,
                &[
                    spelled(A, Natural),
                    spelled(C, Natural),
                    spelled(D, Natural),
                    spelled(E, Flat),
                    spelled(E, Natural),
                    spelled(G, Natural),
                ],
            ),
            (
                0,
                Spelling::Flats,
                ScaleKind::Diminished,
                &[
                    spelled(C, Natural),
                    spelled(D, Flat),
                    spelled(E, Flat),
                    spelled(E, Natural),
                    spelled(G, Flat),
                    spelled(G, Natural),
                    spelled(A, Natural),
                    spelled(B, Flat),
                ],
            ),
            (
                9,
                Spelling::Sharps,
                ScaleKind::MinorPentatonic,
                &[
                    spelled(A, Natural),
                    spelled(C, Natural),
                    spelled(D, Natural),
                    spelled(E, Natural),
                    spelled(G, Natural),
                ],
            ),
        ];

        for &(root, spelling, kind, expected) in cases {
            let scale = Scale {
                root: pc(root),
                spelling,
                kind,
            };
            assert_eq!(
                scale.notes(),
                expected,
                "{} {}",
                scale.root_note(),
                kind.name()
            );
        }
    }

    #[test]
    fn the_double_accidental_extremes_are_reached_and_not_exceeded() {
        // the two furthest scales in the domain
        let g_sharp_harmonic_minor = Scale {
            root: pc(8),
            spelling: Spelling::Sharps,
            kind: ScaleKind::HarmonicMinor,
        };
        assert_eq!(
            g_sharp_harmonic_minor.notes()[6],
            spelled(F, DoubleSharp),
            "G♯ harmonic minor's seventh"
        );

        let d_flat_aeolian = Scale {
            root: pc(1),
            spelling: Spelling::Flats,
            kind: ScaleKind::Aeolian,
        };
        assert_eq!(
            d_flat_aeolian.notes()[5],
            spelled(B, DoubleFlat),
            "D♭ aeolian's sixth"
        );
    }

    #[test]
    fn spell_names_the_scales_own_pitch_classes_and_rejects_the_rest() {
        let f_ionian = Scale {
            root: pc(5),
            spelling: Spelling::Sharps,
            kind: ScaleKind::Ionian,
        };

        assert_eq!(f_ionian.spell(pc(10)), Some(spelled(B, Flat)));
        assert_eq!(f_ionian.spell(pc(5)), Some(spelled(F, Natural)));
        assert_eq!(f_ionian.spell(pc(6)), None);
    }

    #[test]
    fn degree_names_the_position_and_rejects_the_rest() {
        let f_ionian = Scale {
            root: pc(5),
            spelling: Spelling::Sharps,
            kind: ScaleKind::Ionian,
        };

        assert_eq!(f_ionian.degree(pc(10)), Some(Interval::PerfectFourth));
        assert_eq!(f_ionian.degree(pc(5)), Some(Interval::Unison));
        assert_eq!(f_ionian.degree(pc(6)), None);
    }

    #[test]
    fn degree_is_the_same_under_both_spellings() {
        for root in PitchClass::ALL {
            for &kind in ScaleKind::ALL {
                let sharps = Scale {
                    root,
                    spelling: Spelling::Sharps,
                    kind,
                };
                let flats = Scale {
                    spelling: Spelling::Flats,
                    ..sharps
                };

                for pitch_class in PitchClass::ALL {
                    assert_eq!(
                        sharps.degree(pitch_class),
                        flats.degree(pitch_class),
                        "{} {} disagreed about pitch class {}",
                        sharps.root_note(),
                        kind.name(),
                        pitch_class.semitone()
                    );
                }
            }
        }
    }

    #[test]
    fn degree_and_spell_agree_on_membership() {
        for spelling in [Spelling::Sharps, Spelling::Flats] {
            for root in PitchClass::ALL {
                for &kind in ScaleKind::ALL {
                    let scale = Scale {
                        root,
                        spelling,
                        kind,
                    };

                    for pitch_class in PitchClass::ALL {
                        assert_eq!(
                            scale.degree(pitch_class).is_some(),
                            scale.spell(pitch_class).is_some(),
                            "{} {} disagreed about pitch class {}",
                            scale.root_note(),
                            kind.name(),
                            pitch_class.semitone()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn degree_recovers_the_formula_in_order() {
        for spelling in [Spelling::Sharps, Spelling::Flats] {
            for root in PitchClass::ALL {
                for &kind in ScaleKind::ALL {
                    let scale = Scale {
                        root,
                        spelling,
                        kind,
                    };

                    let recovered: Vec<Interval> = scale
                        .notes()
                        .iter()
                        .map(|note| {
                            scale
                                .degree(note.pitch_class())
                                .expect("a scale contains its own notes")
                        })
                        .collect();

                    assert_eq!(
                        recovered,
                        kind.intervals(),
                        "{} {}",
                        scale.root_note(),
                        kind.name()
                    );
                }
            }
        }
    }

    #[test]
    fn no_kind_uses_one_pitch_class_twice() {
        // a kind holding both ♯4 and ♭5 would let the first shadow the second in degree()
        for &kind in ScaleKind::ALL {
            let mut seen: Vec<u8> = Vec::new();

            for interval in kind.intervals() {
                let semitones = interval.semitones();
                assert!(
                    !seen.contains(&semitones),
                    "{} uses {semitones} semitones twice",
                    kind.name()
                );
                seen.push(semitones);
            }
        }
    }

    fn spelled_scale(semitone: u8, kind: ScaleKind) -> String {
        Scale::new(pc(semitone), kind)
            .notes()
            .iter()
            .map(Note::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn a_scale_is_named_the_way_it_is_taught() {
        // pitch class 1 Harmonic Minor goes to C♯, which shows the rule is not "prefer flats";
        // pitch class 8 lands on G♯ or A♭ depending on the kind
        let cases: &[(u8, ScaleKind, &str)] = &[
            (1, ScaleKind::Ionian, "Db Eb F Gb Ab Bb C"),
            (10, ScaleKind::Ionian, "Bb C D Eb F G A"),
            (1, ScaleKind::HarmonicMinor, "C# D# E F# G# A B#"),
            (8, ScaleKind::Aeolian, "G# A# B C# D# E F#"),
            (8, ScaleKind::Mixolydian, "Ab Bb C Db Eb F Gb"),
        ];

        for &(semitone, kind, expected) in cases {
            assert_eq!(
                spelled_scale(semitone, kind),
                expected,
                "pitch class {semitone} {}",
                kind.name()
            );
        }
    }

    #[test]
    fn doubles_are_weighed_before_the_total() {
        let sharps = Scale {
            root: pc(8),
            kind: ScaleKind::HarmonicMinor,
            spelling: Spelling::Sharps,
        };
        let flats = Scale {
            root: pc(8),
            kind: ScaleKind::HarmonicMinor,
            spelling: Spelling::Flats,
        };

        assert_eq!(
            sharps.spelling_cost().1,
            flats.spelling_cost().1,
            "the case only bites when the totals tie"
        );
        assert!(sharps.spelling_cost().0 > flats.spelling_cost().0);

        assert_eq!(
            spelled_scale(8, ScaleKind::HarmonicMinor),
            "Ab Bb Cb Db Eb Fb G"
        );
    }

    #[test]
    fn an_exact_tie_takes_the_conventional_name() {
        // no doubles and six accidentals either way, so only convention separates these
        assert_eq!(
            spelled_scale(6, ScaleKind::Ionian),
            "F# G# A# B C# D# E#",
            "F♯ major, not G♭"
        );
        assert_eq!(
            spelled_scale(3, ScaleKind::Aeolian),
            "Eb F Gb Ab Bb Cb Db",
            "E♭ minor, not D♯"
        );
    }

    #[test]
    fn only_the_symmetric_two_keep_a_double_accidental() {
        // an allowlist, so a new kind that cannot spell cleanly must be admitted deliberately
        let irreducible = [(1, ScaleKind::WholeTone), (3, ScaleKind::Diminished)];

        for root in PitchClass::ALL {
            for &kind in ScaleKind::ALL {
                let scale = Scale::new(root, kind);
                let has_double = scale
                    .notes()
                    .iter()
                    .any(|note| note.accidental.offset().abs() == 2);

                let expected_double = irreducible
                    .iter()
                    .any(|&(semitone, k)| root == pc(semitone) && k == kind);

                assert_eq!(
                    has_double,
                    expected_double,
                    "{} {}",
                    scale.root_note(),
                    kind.name()
                );
            }
        }
    }

    #[test]
    fn naming_never_moves_the_scale() {
        for root in PitchClass::ALL {
            for &kind in ScaleKind::ALL {
                let scale = Scale::new(root, kind);

                let spelled: Vec<PitchClass> = scale
                    .notes()
                    .iter()
                    .map(|note| note.pitch_class())
                    .collect();
                let from_formula: Vec<PitchClass> = kind
                    .intervals()
                    .iter()
                    .map(|interval| root.transpose(interval.semitones()))
                    .collect();

                assert_eq!(
                    spelled,
                    from_formula,
                    "{} {} was moved by its naming",
                    scale.root_note(),
                    kind.name()
                );
            }
        }
    }
}
