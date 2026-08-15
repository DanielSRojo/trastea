use std::cmp::Ordering;

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

/// All three fields are private, and the third is why the first two are.
///
/// `spelling` is derived from `root` and `kind` rather than chosen — see `new`. It
/// is stored rather than recomputed on read because deriving it means spelling the
/// whole scale, and spelling the scale starts from the root note; resolving once in
/// `new` is what breaks that cycle. But a private `spelling` beside a public `root`
/// would be theatre: `scale.root = other` would compile and leave the spelling
/// derived from a root the scale no longer has. Sealing all three is what actually
/// makes a misspelled `Scale` unrepresentable outside this module.
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

    /// Names a scale the way it would be taught. Nobody teaches `A♯ Ionian` — it
    /// spells `A♯ B♯ C𝄪 D♯ E♯ F𝄪 G𝄪`, and a learner reading `G𝄪` off a fret has to
    /// translate it back to A before they can play it. `B♭ Ionian` is the same
    /// shapes with names that can be read at a glance.
    ///
    /// Fewer double accidentals wins; then fewer accidentals overall. That is one
    /// tuple comparison rather than two hand-written ones, because `Ord` on tuples
    /// is already lexicographic.
    ///
    /// The rule has no thumb on the scale for either spelling — it reaches `G♯
    /// Aeolian` and `D♭ Ionian` by the same arithmetic. Where it reaches neither,
    /// `Spelling::conventional_for` decides; see its comment for why a table is the
    /// honest answer there and nowhere else.
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

    /// How much ink this spelling costs: `(double accidentals, total accidental
    /// distance)`.
    ///
    /// The doubles come first deliberately, and the pair that makes it load-bearing
    /// is harmonic minor on pitch class 8: `G♯ A♯ B C♯ D♯ E F𝄪` and `A♭ B♭ C♭ D♭ E♭
    /// F♭ G` both spend six accidentals, but only the first needs a double sharp.
    /// Summed into one number they tie and fall through to convention, which would
    /// pick `A♭` for pitch class 8 by luck. Compared in this order, `A♭` wins for a
    /// reason.
    ///
    /// Measured by spelling the scale and looking, rather than from a table of which
    /// roots are troublesome — so it stays correct if a kind's intervals ever change,
    /// as Whole Tone's already have.
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

    /// This scale's *degree* for a pitch class — the interval-shaped counterpart of
    /// `spell`, and `None` on the same pitch classes, since both search the one
    /// formula. Together they are the two things there are to say about a fretboard
    /// marker: what it is called, and what job it does.
    ///
    /// Not `Interval::between(self.root_note(), note)`, which would reach the same
    /// answer by spelling the pitch and reading the letters back off it — a
    /// round trip through a fallibility of its own (a distance outside the
    /// thirteen), leaving a caller with two failure modes that mean different
    /// things. The formula states the degree directly, so this asks it directly.
    ///
    /// Independent of `spelling`, unlike `spell`: a degree is a position in the
    /// formula, and no choice of sharps or flats moves it.
    pub fn degree(self, pitch_class: PitchClass) -> Option<Interval> {
        self.kind
            .intervals()
            .iter()
            .copied()
            .find(|interval| self.root.transpose(interval.semitones()) == pitch_class)
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
        //
        // Deliberately builds literals rather than going through `Scale::new`, which
        // looks like an oversight and is the opposite. `new` steers away from the
        // spellings that cost the most accidentals, so iterating it would stop
        // covering exactly the combinations most likely to strain the ±2 bound. Those
        // combinations stay representable in this module — `spelling_cost` builds them
        // on every call — so the licence the expect needs is over what can be
        // represented, not over what `new` returns.
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

                    // The two spelling paths — the 12-arm Spelling::spell table
                    // for a bare pitch class, and this letter-walking algorithm
                    // for a scale degree — must name the same note for the root.
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

    #[test]
    fn degree_names_the_position_and_rejects_the_rest() {
        let f_ionian = Scale {
            root: pc(5),
            spelling: Spelling::Sharps,
            kind: ScaleKind::Ionian,
        };

        // The same three pitch classes spell_names… asks about, read as jobs
        // rather than names: pitch class 10 is B♭ *because* it is the fourth.
        assert_eq!(f_ionian.degree(pc(10)), Some(Interval::PerfectFourth));
        assert_eq!(f_ionian.degree(pc(5)), Some(Interval::Unison));
        assert_eq!(f_ionian.degree(pc(6)), None);
    }

    #[test]
    fn degree_is_the_same_under_both_spellings() {
        // The claim the fretboard leans on when the ♯♭ toggle is pressed in
        // interval notation: nothing moves, and no label changes.
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
        // 12 roots × 2 spellings × 16 kinds × 12 pitch classes. Both read the one
        // formula, and this is what holds them to it — a marker that gets a name
        // gets a degree, and one that gets neither is genuinely not in the scale.
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
        // Asked of a scale's own notes, degree() must give the formula back exactly
        // — the interval-side counterpart of every_scale_spells_without_failing,
        // and what licenses the expect below being unreachable in the app.
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
        // What makes degree()'s `find` well-defined rather than order-dependent: a
        // kind holding both a ♯4 and a ♭5 would put two degrees on one pitch class,
        // and the first listed would shadow the second. No kind does — and if one is
        // ever added, this fails before the fretboard starts lying.
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

    /// A scale's notes as text, for the naming tables below.
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
        // The rule's whole job, stated as the names rather than as the rule — so a
        // change to `spelling_cost` that still satisfies its own arithmetic but starts
        // naming scales wrongly fails here.
        //
        // The four cases are chosen to pin it from four directions:
        //   - pitch class 1 Ionian: D♭ (5 flats) over the theoretical C♯ (7 sharps).
        //   - pitch class 10 Ionian: B♭, avoiding A♯ Ionian's three double sharps.
        //   - pitch class 1 Harmonic Minor: C♯, avoiding D♭'s B𝄫 — the same rule
        //     running the other way, which is what shows it is not "prefer flats".
        //   - pitch class 8 Aeolian and Mixolydian: G♯ for one and A♭ for the other,
        //     the same pitch class landing on different spellings because the kind
        //     changed. No fixed table of root names could do that.
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
        // Why `spelling_cost` returns a pair rather than one number. Harmonic minor on
        // pitch class 8 spends six accidentals either way, so a single total ties and
        // falls through to convention — which for pitch class 8 is flats, and would
        // reach A♭ by luck. Comparing doubles first reaches it for a reason: only the
        // G♯ candidate needs a double sharp.
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
        // The eleven scales where the arithmetic has nothing left to say. Both of
        // these spend six accidentals with no doubles either way, so the only thing
        // that separates F♯ from G♭ and D♯ from E♭ is what the note is called.
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
        // Written as an allowlist rather than a loose skip, so that a kind added later
        // that cannot be spelled cleanly has to be admitted here deliberately instead
        // of slipping past. Six notes and eight notes cannot be spread over seven
        // letters without a double; every other scale can be, and is.
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
        // The names are the only thing the rule chooses. Whatever it picks, the scale
        // is still the same pitch classes in the same order — which is what the neck
        // draws and what `degree` reads. Replaces the UI's
        // `toggling_spelling_moves_no_marker_but_relabels_at_least_one`, over all 192
        // combinations rather than one root spelled two ways.
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
