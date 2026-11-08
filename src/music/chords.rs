//! Chords built by stacking degrees over a root, spelled by the same arithmetic as `scales`.

use std::cmp::Ordering;
use std::fmt;

use super::intervals::Interval;
use super::notes::{Accidental, Letter, Note, PitchClass, Spelling, nearest_offset};

/// What is stacked over the root.
///
/// A shape on the neck carries one degree per string, so a quality can only borrow a
/// shape whose degree numbers match its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordQuality {
    Major,
    Minor,
    Diminished,
    Augmented,
    Sus2,
    Sus4,
    Major6,
    Minor6,
    Dominant7,
    Major7,
    Minor7,
    MinorMajor7,
    HalfDiminished7,
    Diminished7,
    Augmented7,
}

impl ChordQuality {
    /// Triads, then suspensions, sixths and sevenths: ordered by distance from a plain triad.
    pub const ALL: &'static [ChordQuality] = &[
        ChordQuality::Major,
        ChordQuality::Minor,
        ChordQuality::Diminished,
        ChordQuality::Augmented,
        ChordQuality::Sus2,
        ChordQuality::Sus4,
        ChordQuality::Major6,
        ChordQuality::Minor6,
        ChordQuality::Dominant7,
        ChordQuality::Major7,
        ChordQuality::Minor7,
        ChordQuality::MinorMajor7,
        ChordQuality::HalfDiminished7,
        ChordQuality::Diminished7,
        ChordQuality::Augmented7,
    ];

    pub fn intervals(self) -> &'static [Interval] {
        match self {
            ChordQuality::Major => &[
                Interval::Unison,
                Interval::MajorThird,
                Interval::PerfectFifth,
            ],
            ChordQuality::Minor => &[
                Interval::Unison,
                Interval::MinorThird,
                Interval::PerfectFifth,
            ],
            ChordQuality::Diminished => &[
                Interval::Unison,
                Interval::MinorThird,
                Interval::DiminishedFifth,
            ],
            ChordQuality::Augmented => &[
                Interval::Unison,
                Interval::MajorThird,
                Interval::AugmentedFifth,
            ],
            ChordQuality::Sus2 => &[
                Interval::Unison,
                Interval::MajorSecond,
                Interval::PerfectFifth,
            ],
            ChordQuality::Sus4 => &[
                Interval::Unison,
                Interval::PerfectFourth,
                Interval::PerfectFifth,
            ],
            ChordQuality::Major6 => &[
                Interval::Unison,
                Interval::MajorThird,
                Interval::PerfectFifth,
                Interval::MajorSixth,
            ],
            ChordQuality::Minor6 => &[
                Interval::Unison,
                Interval::MinorThird,
                Interval::PerfectFifth,
                Interval::MajorSixth,
            ],
            ChordQuality::Dominant7 => &[
                Interval::Unison,
                Interval::MajorThird,
                Interval::PerfectFifth,
                Interval::MinorSeventh,
            ],
            ChordQuality::Major7 => &[
                Interval::Unison,
                Interval::MajorThird,
                Interval::PerfectFifth,
                Interval::MajorSeventh,
            ],
            ChordQuality::Minor7 => &[
                Interval::Unison,
                Interval::MinorThird,
                Interval::PerfectFifth,
                Interval::MinorSeventh,
            ],
            ChordQuality::MinorMajor7 => &[
                Interval::Unison,
                Interval::MinorThird,
                Interval::PerfectFifth,
                Interval::MajorSeventh,
            ],
            ChordQuality::HalfDiminished7 => &[
                Interval::Unison,
                Interval::MinorThird,
                Interval::DiminishedFifth,
                Interval::MinorSeventh,
            ],
            ChordQuality::Diminished7 => &[
                Interval::Unison,
                Interval::MinorThird,
                Interval::DiminishedFifth,
                Interval::DiminishedSeventh,
            ],
            ChordQuality::Augmented7 => &[
                Interval::Unison,
                Interval::MajorThird,
                Interval::AugmentedFifth,
                Interval::MinorSeventh,
            ],
        }
    }

    /// The degree numbers this quality covers, which decide whether a shape can carry it.
    pub fn degrees(self) -> impl Iterator<Item = u8> {
        self.intervals().iter().map(|interval| interval.number())
    }

    /// What `Display` writes after the root. Empty for a plain major triad.
    pub fn suffix(self) -> &'static str {
        match self {
            ChordQuality::Major => "",
            ChordQuality::Minor => "m",
            ChordQuality::Diminished => "dim",
            ChordQuality::Augmented => "aug",
            ChordQuality::Sus2 => "sus2",
            ChordQuality::Sus4 => "sus4",
            ChordQuality::Major6 => "6",
            ChordQuality::Minor6 => "m6",
            ChordQuality::Dominant7 => "7",
            ChordQuality::Major7 => "maj7",
            ChordQuality::Minor7 => "m7",
            ChordQuality::MinorMajor7 => "mMaj7",
            ChordQuality::HalfDiminished7 => "m7b5",
            ChordQuality::Diminished7 => "dim7",
            // matches the `7♯5` the screen shows
            ChordQuality::Augmented7 => "7#5",
        }
    }

    /// The quality said aloud, the prose counterpart of `suffix`.
    pub fn spoken(self) -> &'static str {
        match self {
            ChordQuality::Major => "major",
            ChordQuality::Minor => "minor",
            ChordQuality::Diminished => "diminished",
            ChordQuality::Augmented => "augmented",
            ChordQuality::Sus2 => "suspended second",
            ChordQuality::Sus4 => "suspended fourth",
            ChordQuality::Major6 => "major sixth",
            ChordQuality::Minor6 => "minor sixth",
            ChordQuality::Dominant7 => "dominant seventh",
            ChordQuality::Major7 => "major seventh",
            ChordQuality::Minor7 => "minor seventh",
            ChordQuality::MinorMajor7 => "minor-major seventh",
            ChordQuality::HalfDiminished7 => "half-diminished seventh",
            ChordQuality::Diminished7 => "diminished seventh",
            // "augmented seventh" is heard by some as a different chord
            ChordQuality::Augmented7 => "seventh sharp five",
        }
    }

    /// Every written form that names this quality, longest first within the entry.
    /// Matched case-sensitively, because `M7` and `m7` are a major and a minor seventh.
    pub fn forms(self) -> &'static [&'static str] {
        match self {
            ChordQuality::Major => &["major", "maj", "M"],
            ChordQuality::Minor => &["min", "m", "-"],
            ChordQuality::Diminished => &["dim", "°", "o"],
            ChordQuality::Augmented => &["aug", "+"],
            ChordQuality::Sus2 => &["sus2"],
            ChordQuality::Sus4 => &["sus4", "sus"],
            ChordQuality::Major6 => &["maj6", "M6", "6"],
            ChordQuality::Minor6 => &["min6", "m6", "-6"],
            ChordQuality::Dominant7 => &["dom7", "7"],
            ChordQuality::Major7 => &["major7", "maj7", "M7", "Δ7", "△7", "Δ", "△"],
            ChordQuality::Minor7 => &["min7", "m7", "-7"],
            ChordQuality::MinorMajor7 => {
                &["minMaj7", "mMaj7", "mM7", "-Maj7", "mΔ7", "m△7", "mΔ", "m△"]
            }
            ChordQuality::HalfDiminished7 => &["min7b5", "m7b5", "-7b5", "ø7", "ø"],
            ChordQuality::Diminished7 => &["dim7", "°7", "o7"],
            ChordQuality::Augmented7 => &["aug7", "7#5", "+7"],
        }
    }

    /// The qualities that extend this one: written as a longer form of one of its own, and
    /// containing every degree it contains. Both halves are needed.
    pub fn extensions(self) -> impl Iterator<Item = ChordQuality> {
        let written_longer = move |longer: ChordQuality| {
            longer.forms().iter().any(|form| {
                self.forms()
                    .iter()
                    .any(|own| form.len() > own.len() && form.starts_with(own))
            })
        };
        let contains_every_degree = move |longer: ChordQuality| {
            self.intervals()
                .iter()
                .all(|degree| longer.intervals().contains(degree))
        };

        ChordQuality::ALL.iter().copied().filter(move |&longer| {
            longer != self && written_longer(longer) && contains_every_degree(longer)
        })
    }

    /// Whether naming this quality should also offer `other` — itself, or one of its `extensions`.
    pub fn covers(self, other: ChordQuality) -> bool {
        self == other || self.extensions().any(|kind| kind == other)
    }
}

/// A root and a quality. `spelling` is derived from them in `new`, so it is not part of identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    root: PitchClass,
    kind: ChordQuality,
    spelling: Spelling,
}

impl Chord {
    pub fn root(self) -> PitchClass {
        self.root
    }

    pub fn kind(self) -> ChordQuality {
        self.kind
    }

    /// Names a chord the way it would be written, by `Scale::new`'s rule.
    ///
    /// Also rejects an unspellable candidate: a diminished seventh on `C♭` or `F♭` lands
    /// three flats below the letter, so it sorts behind every spellable one.
    pub fn new(root: PitchClass, kind: ChordQuality) -> Self {
        let candidate = |spelling| Chord {
            root,
            kind,
            spelling,
        };
        let (sharps, flats) = (candidate(Spelling::Sharps), candidate(Spelling::Flats));

        // `Option`'s `Ord` puts the unspellable `None` after every `Some`
        match (sharps.spelling_cost(), flats.spelling_cost()) {
            (None, None) => unreachable!("every_chord_spells_without_failing proves one works"),
            (a, b) => match a.cmp(&b) {
                Ordering::Less => sharps,
                Ordering::Greater => flats,
                Ordering::Equal => candidate(Spelling::conventional_for(root)),
            },
        }
    }

    pub fn root_note(self) -> Note {
        self.spelling.spell(self.root)
    }

    /// The chord said aloud — `C♯m7` reads "C sharp minor seventh", and `C` reads "C major".
    pub fn spoken(self) -> String {
        format!("{} {}", self.root_note().spoken(), self.kind.spoken())
    }

    /// Every degree spelled: the degree number forces the letter, the accidental corrects
    /// the pitch. Total, because `new` has already discarded any unspellable candidate.
    pub fn notes(self) -> Vec<Note> {
        self.try_notes()
            .expect("new rejects unspellable spellings; every_chord_spells_without_failing proves one remains")
    }

    /// The degrees this chord is built from, in ascending order.
    pub fn degrees(self) -> &'static [Interval] {
        self.kind.intervals()
    }

    /// This chord's name for a pitch class, or `None` when it is not in the chord.
    pub fn spell(self, pitch_class: PitchClass) -> Option<Note> {
        self.notes()
            .into_iter()
            .find(|note| note.pitch_class() == pitch_class)
    }

    /// This chord's degree for a pitch class, or `None` when it is not in the chord.
    pub fn degree(self, pitch_class: PitchClass) -> Option<Interval> {
        self.kind
            .intervals()
            .iter()
            .copied()
            .find(|interval| self.root.transpose(interval.semitones()) == pitch_class)
    }

    /// `None` where some degree lands beyond a double accidental.
    fn try_notes(self) -> Option<Vec<Note>> {
        let root = self.root_note();

        self.kind
            .intervals()
            .iter()
            .map(|interval| {
                let letter = root.letter.step(interval.number() - 1);
                let target = self.root.transpose(interval.semitones());
                let accidental = Accidental::from_offset(nearest_offset(target, letter))?;

                Some(Note { letter, accidental })
            })
            .collect()
    }

    /// `(double accidentals, total accidental distance)`, or `None` when it cannot be written.
    fn spelling_cost(self) -> Option<(usize, u32)> {
        let notes = self.try_notes()?;

        Some(
            notes
                .iter()
                .map(|note| u32::from(note.accidental.offset().unsigned_abs()))
                .fold((0, 0), |(doubles, total), offset| {
                    (doubles + usize::from(offset == 2), total + offset)
                }),
        )
    }
}

/// ASCII, because this module cannot reach the SMuFL glyphs; it is also what the parser reads.
impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.root_note(), self.kind.suffix())
    }
}

/// What a typed query names: a root, a quality, or both, never neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Query {
    Root(PitchClass),
    Quality(ChordQuality),
    Chord {
        root: PitchClass,
        kind: ChordQuality,
    },
}

impl Query {
    /// Reads a chord symbol, or `None` for input this grammar cannot account for —
    /// the signal to fall back to approximate matching.
    ///
    /// Whole-input forms are tried before a root, because `dim`, `aug` and `sus` begin
    /// with a note name.
    pub fn parse(input: &str) -> Option<Query> {
        let input = input.trim();

        if input.is_empty() {
            return None;
        }

        if let Some(kind) = quality_from_form(input) {
            return Some(Query::Quality(kind));
        }

        let (root, rest) = parse_root(input)?;

        if rest.is_empty() {
            return Some(Query::Root(root));
        }

        quality_from_form(rest).map(|kind| Query::Chord { root, kind })
    }
}

/// A root and whatever follows it: a letter in either case, then accidentals taken
/// greedily, so `Cb` is always C flat.
pub fn parse_root(input: &str) -> Option<(PitchClass, &str)> {
    let mut chars = input.chars();
    let letter = letter_from_char(chars.next()?)?;
    let mut rest = chars.as_str();
    let mut offset: i8 = 0;

    // `probe` only becomes `rest` once its character is claimed, so the terminator is not consumed
    loop {
        let mut probe = rest.chars();

        match probe.next().and_then(accidental_step) {
            Some(step) => {
                offset = offset.checked_add(step)?;
                rest = probe.as_str();
            }
            None => break,
        }
    }

    let accidental = Accidental::from_offset(offset)?;

    Some((Note { letter, accidental }.pitch_class(), rest))
}

fn letter_from_char(c: char) -> Option<Letter> {
    match c.to_ascii_uppercase() {
        'C' => Some(Letter::C),
        'D' => Some(Letter::D),
        'E' => Some(Letter::E),
        'F' => Some(Letter::F),
        'G' => Some(Letter::G),
        'A' => Some(Letter::A),
        'B' => Some(Letter::B),
        _ => None,
    }
}

/// Both spellings of each accidental, so `c#` and `C♯` are one root.
fn accidental_step(c: char) -> Option<i8> {
    match c {
        '#' | '♯' => Some(1),
        'b' | '♭' => Some(-1),
        _ => None,
    }
}

/// Case-sensitive: `M7` and `m7` are a major and a minor seventh.
fn quality_from_form(form: &str) -> Option<ChordQuality> {
    ChordQuality::ALL
        .iter()
        .copied()
        .find(|kind| kind.forms().contains(&form))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(letter: Letter, accidental: Accidental) -> Note {
        Note { letter, accidental }
    }

    #[test]
    fn all_lists_every_quality() {
        // a quality added to the enum but not to ALL would be skipped by the tests below
        assert_eq!(ChordQuality::ALL.len(), 15);
    }

    #[test]
    fn every_quality_starts_on_its_root() {
        for &kind in ChordQuality::ALL {
            assert_eq!(
                kind.intervals().first(),
                Some(&Interval::Unison),
                "{kind:?} does not start on the root"
            );
        }
    }

    #[test]
    fn no_quality_names_a_degree_twice() {
        for &kind in ChordQuality::ALL {
            let mut degrees: Vec<u8> = kind.degrees().collect();
            let count = degrees.len();
            degrees.sort_unstable();
            degrees.dedup();

            assert_eq!(degrees.len(), count, "{kind:?} repeats a degree");
        }
    }

    #[test]
    fn the_qualities_fall_into_five_degree_sets() {
        // the five degree sets the shape table rests on
        let mut sets: Vec<Vec<u8>> = ChordQuality::ALL
            .iter()
            .map(|kind| kind.degrees().collect())
            .collect();
        sets.sort_unstable();
        sets.dedup();

        assert_eq!(
            sets,
            vec![
                vec![1, 2, 5],
                vec![1, 3, 5],
                vec![1, 3, 5, 6],
                vec![1, 3, 5, 7],
                vec![1, 4, 5],
            ]
        );
    }

    #[test]
    fn no_two_qualities_share_a_written_form() {
        for (i, &kind) in ChordQuality::ALL.iter().enumerate() {
            for &other in &ChordQuality::ALL[i + 1..] {
                for form in kind.forms() {
                    assert!(
                        !other.forms().contains(form),
                        "{kind:?} and {other:?} both answer to {form}"
                    );
                }
            }
        }
    }

    #[test]
    fn no_two_qualities_are_said_the_same_way() {
        for (i, &kind) in ChordQuality::ALL.iter().enumerate() {
            for &other in &ChordQuality::ALL[i + 1..] {
                assert_ne!(
                    kind.spoken(),
                    other.spoken(),
                    "{kind:?} and {other:?} are both called {}",
                    kind.spoken()
                );
            }
        }
    }

    #[test]
    fn a_spoken_name_is_prose_and_not_notation() {
        for &kind in ChordQuality::ALL {
            assert!(
                kind.spoken()
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == ' ' || c == '-'),
                "{kind:?} is said as {}",
                kind.spoken()
            );
        }
    }

    #[test]
    fn case_separates_the_major_and_minor_sevenths() {
        assert!(ChordQuality::Major7.forms().contains(&"M7"));
        assert!(ChordQuality::Minor7.forms().contains(&"m7"));
    }

    #[test]
    fn a_seventh_chord_spells_four_letters_a_third_apart() {
        for &kind in ChordQuality::ALL {
            if kind.degrees().count() != 4 || !kind.degrees().any(|degree| degree == 7) {
                continue;
            }

            for &root in &PitchClass::ALL {
                let letters: Vec<Letter> = Chord::new(root, kind)
                    .notes()
                    .iter()
                    .map(|note| note.letter)
                    .collect();

                assert_eq!(letters.len(), 4);
                for pair in letters.windows(2) {
                    assert_eq!(
                        pair[0].step(2),
                        pair[1],
                        "{kind:?} on {root:?} skips a letter"
                    );
                }
            }
        }
    }

    #[test]
    fn the_augmented_triad_sharpens_the_fifth_rather_than_flattening_the_sixth() {
        // without `AugmentedFifth` this would spell C E A♭
        assert_eq!(
            Chord::new(PitchClass::new(0), ChordQuality::Augmented).notes(),
            vec![
                note(Letter::C, Accidental::Natural),
                note(Letter::E, Accidental::Natural),
                note(Letter::G, Accidental::Sharp),
            ]
        );
    }

    #[test]
    fn the_diminished_seventh_flattens_the_seventh_twice() {
        assert_eq!(
            Chord::new(PitchClass::new(0), ChordQuality::Diminished7).notes(),
            vec![
                note(Letter::C, Accidental::Natural),
                note(Letter::E, Accidental::Flat),
                note(Letter::G, Accidental::Flat),
                note(Letter::B, Accidental::DoubleFlat),
            ]
        );
    }

    #[test]
    fn a_root_is_named_the_way_it_is_written() {
        let chord = Chord::new(PitchClass::new(10), ChordQuality::Major);

        assert_eq!(chord.root_note(), note(Letter::B, Accidental::Flat));
    }

    #[test]
    fn every_chord_spells_without_failing() {
        // licenses `notes`'s `expect`; pitch classes 4 and 11 under a diminished seventh
        // survive only with sharps
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                let chord = Chord::new(root, kind);
                let notes = chord.notes();

                assert_eq!(notes.len(), kind.intervals().len());
                for note in notes {
                    assert!(
                        note.accidental.offset().abs() <= 2,
                        "{kind:?} on {root:?} needs {note}"
                    );
                }
            }
        }
    }

    #[test]
    fn no_chord_spells_two_notes_on_one_letter() {
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                // as `u8` because `Letter` derives no `Ord`
                let mut letters: Vec<u8> = Chord::new(root, kind)
                    .notes()
                    .iter()
                    .map(|note| note.letter as u8)
                    .collect();
                let count = letters.len();
                letters.sort_unstable();
                letters.dedup();

                assert_eq!(
                    letters.len(),
                    count,
                    "{kind:?} on {root:?} repeats a letter"
                );
            }
        }
    }

    #[test]
    fn the_notes_sound_the_degrees_they_are_built_from() {
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                let chord = Chord::new(root, kind);

                for (note, interval) in chord.notes().iter().zip(chord.degrees()) {
                    assert_eq!(
                        note.pitch_class(),
                        root.transpose(interval.semitones()),
                        "{kind:?} on {root:?} spells {interval} as {note}"
                    );
                }
            }
        }
    }

    fn pc(semitone: u8) -> PitchClass {
        PitchClass::new(semitone)
    }

    #[test]
    fn display_is_the_root_and_the_suffix() {
        assert_eq!(Chord::new(pc(0), ChordQuality::Major).to_string(), "C");
        assert_eq!(Chord::new(pc(0), ChordQuality::Major7).to_string(), "Cmaj7");
        assert_eq!(
            Chord::new(pc(11), ChordQuality::HalfDiminished7).to_string(),
            "Bm7b5"
        );
        assert_eq!(Chord::new(pc(10), ChordQuality::Minor7).to_string(), "Bbm7");
    }

    #[test]
    fn a_chord_is_said_as_its_root_then_its_quality() {
        assert_eq!(Chord::new(pc(0), ChordQuality::Major).spoken(), "C major");
        assert_eq!(
            Chord::new(pc(1), ChordQuality::Minor7).spoken(),
            "C sharp minor seventh"
        );
        assert_eq!(
            Chord::new(pc(1), ChordQuality::Major).spoken(),
            "D flat major"
        );
    }

    #[test]
    fn a_root_and_a_quality_parse_to_a_chord() {
        let cmaj7 = Some(Query::Chord {
            root: pc(0),
            kind: ChordQuality::Major7,
        });

        assert_eq!(Query::parse("cmaj7"), cmaj7);
        assert_eq!(Query::parse("Cmaj7"), cmaj7);
        assert_eq!(Query::parse("CM7"), cmaj7);
        assert_eq!(Query::parse("  Cmaj7  "), cmaj7);

        assert_eq!(
            Query::parse("c#m7b5"),
            Some(Query::Chord {
                root: pc(1),
                kind: ChordQuality::HalfDiminished7,
            })
        );
        assert_eq!(Query::parse("C♯m7"), Query::parse("c#m7"));
        assert_eq!(
            Query::parse("gsus"),
            Some(Query::Chord {
                root: pc(7),
                kind: ChordQuality::Sus4,
            })
        );
    }

    #[test]
    fn a_root_alone_parses_to_a_root() {
        assert_eq!(Query::parse("f#"), Some(Query::Root(pc(6))));
        assert_eq!(Query::parse("C"), Some(Query::Root(pc(0))));
        // greedy accidentals: the second `b` is a flat, not the start of a quality
        assert_eq!(Query::parse("bb"), Some(Query::Root(pc(10))));
        assert_eq!(Query::parse("b"), Some(Query::Root(pc(11))));
    }

    #[test]
    fn a_quality_alone_parses_to_a_quality() {
        assert_eq!(
            Query::parse("maj7"),
            Some(Query::Quality(ChordQuality::Major7))
        );
        assert_eq!(
            Query::parse("m7"),
            Some(Query::Quality(ChordQuality::Minor7))
        );
        assert_eq!(
            Query::parse("dim"),
            Some(Query::Quality(ChordQuality::Diminished))
        );
        assert_eq!(
            Query::parse("aug"),
            Some(Query::Quality(ChordQuality::Augmented))
        );
        assert_eq!(
            Query::parse("sus4"),
            Some(Query::Quality(ChordQuality::Sus4))
        );
    }

    #[test]
    fn input_the_grammar_cannot_account_for_parses_to_nothing() {
        assert_eq!(Query::parse(""), None);
        assert_eq!(Query::parse("   "), None);
        assert_eq!(Query::parse("xyz"), None);
        assert_eq!(Query::parse("CMAJ7"), None);
        assert_eq!(Query::parse("cmaj7z"), None);
        assert_eq!(Query::parse("c###"), None);
    }

    #[test]
    fn the_triangle_names_a_major_seventh() {
        // U+0394 GREEK CAPITAL DELTA and U+25B3 WHITE UP-POINTING TRIANGLE
        let cmaj7 = Query::parse("Cmaj7");

        assert_eq!(Query::parse("CΔ"), cmaj7);
        assert_eq!(Query::parse("C△"), cmaj7);
        assert_eq!(Query::parse("C△7"), cmaj7);
        assert_eq!(Query::parse("Cm△"), Query::parse("CmMaj7"));
    }

    #[test]
    fn a_quality_extends_into_the_longer_names_that_contain_it() {
        let extensions = |kind: ChordQuality| kind.extensions().collect::<Vec<_>>();

        assert_eq!(
            extensions(ChordQuality::Major),
            [ChordQuality::Major6, ChordQuality::Major7]
        );
        assert_eq!(
            extensions(ChordQuality::Minor),
            [
                ChordQuality::Minor6,
                ChordQuality::Minor7,
                ChordQuality::MinorMajor7
            ]
        );
        assert_eq!(
            extensions(ChordQuality::Diminished),
            [ChordQuality::Diminished7]
        );
        assert!(extensions(ChordQuality::Major7).is_empty());
    }

    #[test]
    fn an_extension_needs_both_the_name_and_the_degrees() {
        assert!(
            !ChordQuality::Minor.covers(ChordQuality::Major7),
            "`major7` starts with `m`, which the name test alone would accept"
        );
        assert!(
            !ChordQuality::Major.covers(ChordQuality::Dominant7),
            "a dominant seventh contains a major triad, which the degree test alone would accept"
        );
        assert!(ChordQuality::Minor.covers(ChordQuality::Minor));
    }

    #[test]
    fn cm7_is_a_minor_seventh_and_not_a_major_one() {
        // a subsequence matcher would rank `Cmaj7` against the `Cm7` typed here
        assert_eq!(
            Query::parse("cm7"),
            Some(Query::Chord {
                root: pc(0),
                kind: ChordQuality::Minor7,
            })
        );
        assert_ne!(Query::parse("cm7"), Query::parse("cmaj7"));
    }

    #[test]
    fn every_declared_form_reaches_its_chord() {
        for &kind in ChordQuality::ALL {
            for form in kind.forms() {
                for &root in &PitchClass::ALL {
                    for spelling in [Spelling::Sharps, Spelling::Flats] {
                        let written = format!("{}{form}", spelling.spell(root));

                        assert_eq!(
                            Query::parse(&written),
                            Some(Query::Chord { root, kind }),
                            "{written} does not reach {kind:?} on {root:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_written_suffix_is_always_typeable() {
        // a suffix outside the form list would render a chord that cannot be typed back
        for &kind in ChordQuality::ALL {
            if kind.suffix().is_empty() {
                continue;
            }

            assert_eq!(
                quality_from_form(kind.suffix()),
                Some(kind),
                "{kind:?} writes {} and the parser does not accept it",
                kind.suffix()
            );
        }
    }

    #[test]
    fn the_major_triad_is_written_as_its_root() {
        for &root in &PitchClass::ALL {
            let chord = Chord::new(root, ChordQuality::Major);

            assert_eq!(chord.to_string(), chord.root_note().to_string());
            assert_eq!(Query::parse(&chord.to_string()), Some(Query::Root(root)));
        }
    }

    #[test]
    fn every_other_chord_parses_back_from_what_it_writes() {
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                if kind.suffix().is_empty() {
                    continue;
                }

                let chord = Chord::new(root, kind);

                assert_eq!(
                    Query::parse(&chord.to_string()),
                    Some(Query::Chord { root, kind }),
                    "{chord} does not parse back"
                );
            }
        }
    }

    #[test]
    fn spell_and_degree_agree_on_membership() {
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                let chord = Chord::new(root, kind);

                for &pitch_class in &PitchClass::ALL {
                    assert_eq!(
                        chord.spell(pitch_class).is_some(),
                        chord.degree(pitch_class).is_some(),
                        "{chord} disagrees about {pitch_class:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_degree_is_the_same_under_both_spellings() {
        let chord = Chord::new(PitchClass::new(0), ChordQuality::Diminished7);

        assert_eq!(
            chord.degree(PitchClass::new(9)),
            Some(Interval::DiminishedSeventh)
        );
        assert_eq!(
            chord.spell(PitchClass::new(9)).map(|n| n.to_string()),
            Some("Bbb".into())
        );
    }

    #[test]
    fn the_same_chord_built_twice_agrees_with_itself() {
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                let chord = Chord::new(root, kind);

                assert_eq!(chord, Chord::new(root, kind));
                assert_eq!(chord.notes(), Chord::new(root, kind).notes());
                assert_eq!(chord.root(), root);
                assert_eq!(chord.kind(), kind);
            }
        }
    }
}
