use super::intervals::Interval;
use super::notes::{Accidental, Letter, Note, PitchClass, Spelling};

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
    pub spelling: Spelling,
    pub kind: ScaleKind,
}

impl Scale {
    pub fn root_note(self) -> Note {
        self.spelling.spell(self.root)
    }

    /// Every degree spelled: the degree number forces the letter, and the
    /// accidental is whatever the arithmetic then demands. No lookup tables, no
    /// sharp-or-flat heuristic, no key signatures.
    ///
    /// Total rather than fallible, which takes more than "only 24 roots exist" to
    /// justify — the root's accidental is only one of three ±1 terms that sum
    /// into a degree's offset. The other two: the degree's own alteration (±1),
    /// and how far that letter's natural distance sits from the major scale's
    /// distance for that degree (±1 again) — naively ±3 in total, except the
    /// extremes cannot align. That third term is +1 exactly where a natural
    /// root's major scale sharpens the degree and −1 exactly where it flattens
    /// one, and the only flat any natural root's major scale needs is F major's
    /// B♭ on degree 4 — which is also the only degree these formulas ever
    /// sharpen (`AugmentedFourth`) and one they never flatten. So a +1 alteration
    /// can only ever meet a third term of 0 or −1, and a −1 alteration can only
    /// meet 0 or +1 — either way the sum stays within ±2. See
    /// `every_scale_spells_without_failing`, which pins this by exhaustion across
    /// all 384 reachable scales rather than leaving the argument in prose alone,
    /// and `docs/superpowers/specs/2026-07-30-note-spelling-design.md`'s "Why
    /// double accidentals are enough" for the full derivation.
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

    /// This scale's name for a pitch class, or `None` when the pitch class is not
    /// in the scale. The `Option` is the membership test.
    pub fn spell(self, pitch_class: PitchClass) -> Option<Note> {
        self.notes()
            .into_iter()
            .find(|note| note.pitch_class() == pitch_class)
    }
}

/// How far `target` sits from `letter`'s natural pitch, as the nearest signed
/// distance in −5..=6.
///
/// The fold is the fiddly part: `rem_euclid(12)` gives 0..=11, then anything
/// above 6 has 12 subtracted. Without it a difference reads as +11 where −1 is
/// meant, and no accidental covers +11.
fn nearest_offset(target: PitchClass, letter: Letter) -> i8 {
    let raw = (i16::from(target.semitone()) - i16::from(letter.natural_semitone())).rem_euclid(12);
    let folded = if raw > 6 { raw - 12 } else { raw };

    i8::try_from(folded).expect("the fold yields -5..=6")
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
            // Looks like an oversight — the textbook formula is 1 2 3 ♯4 ♯5 ♯6,
            // which this is not — but it is deliberate. `AugmentedFifth` and
            // `AugmentedSixth` do not exist among `Interval`'s thirteen variants,
            // and per `Interval::ALL`'s doc comment, `AugmentedSixth` would push
            // an `A♯` root past the ±2 bound `notes()`'s `expect` relies on. So
            // `MinorSixth`/`MinorSeventh` is also the choice that keeps spelling
            // total. It also matches this branch's deleted `intervalic()` string
            // for Whole Tone, `"1 2 3 ♯4 ♭6 ♭7"` — before this branch the card
            // read ♭6 while the fretboard rendered G♯, so this is a consistency
            // fix as well as a load-bearing one.
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
    use Accidental::{DoubleFlat, DoubleSharp, Flat, Natural, Sharp};
    use Letter::{A, B, C, D, E, F, G};

    /// A written note, for the expectation tables below.
    fn spelled(letter: Letter, accidental: Accidental) -> Note {
        Note { letter, accidental }
    }

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
    /// start in different places. Spelling does not affect these.
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
        // Adding a variant already fails to compile in name, feel, common_usage
        // and intervals, which all match exhaustively. ALL is the one place the
        // compiler cannot help, so the count is pinned here instead.
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
        // ♯4 and ♭5 are the same distance, so putting one where the other belongs
        // is invisible to every semitone-based test here. It shows up only once a
        // scale is spelled from a root — which reference_scales_have_the_textbook_notes
        // now catches for Blues and Diminished, but not for Lydian or WholeTone.
        // This is the assertion that keeps all five apart.
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

    #[test]
    fn every_scale_spells_without_failing() {
        // 12 pitch classes × 2 spellings × 16 kinds = 384. The domain is closed
        // and tiny, so this is a real proof that ±2 accidentals are enough — and
        // it is what licenses the expect inside notes().
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
        // True by construction — degrees 1..7 are seven distinct letter steps —
        // but it is exactly the property F Ionian violated, so it gets pinned.
        // The eight-note kinds correctly reuse a letter and are excluded.
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
        // Ordered from the root and wrapping past B, not sorted by pitch.
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
                // The ♭5 is the blue note, and it is spelled E♭, not D♯.
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
                // Eight notes, so E and G each appear twice — as they are written.
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
                // Five notes, so letters B and F go unused — the pentatonic gap.
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
        // The two furthest scales in the domain. If the fold in notes() is wrong,
        // these break first.
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

        // Pitch class 10 is in F major, and in it the name is B♭ — not A♯.
        assert_eq!(f_ionian.spell(pc(10)), Some(spelled(B, Flat)));
        assert_eq!(f_ionian.spell(pc(5)), Some(spelled(F, Natural)));
        // Pitch class 6 (F♯/G♭) is not in F major at all.
        assert_eq!(f_ionian.spell(pc(6)), None);
    }
}
