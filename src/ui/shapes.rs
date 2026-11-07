//! The movable shapes, the voicings they place, and the arithmetic between them.
//! `SHAPES` is the only curated table; everything downstream of it is arithmetic.

use crate::music::chords::{Chord, ChordQuality};
use crate::music::notes::PitchClass;

use super::chord_diagram::window_for;
use super::neck::Neck;

/// How many frets apart a shape's stopped notes may sit: four frets span a distance of three.
const REACH: u8 = 3;

/// How many fingers a hand brings to the neck. A barre counts as one.
const FINGERS: usize = 4;

/// What one string does in a shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StringRole {
    Muted,
    /// `offset` is measured from the shape's index fret, so the lowest stopped string of
    /// every shape sits at zero and placing the shape is one addition.
    Sounded {
        offset: u8,
        degree: u8,
    },
}

/// Which of the five open major shapes a shape derives from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CagedShape {
    C,
    A,
    G,
    E,
    D,
}

impl CagedShape {
    /// How a shape of this letter is written, reduced or not.
    fn name(self, reduction: Option<&str>) -> String {
        match reduction {
            Some(reduction) => format!("{} shape, {reduction}", self.letter()),
            None => format!("{} shape", self.letter()),
        }
    }

    fn letter(self) -> &'static str {
        match self {
            CagedShape::C => "C",
            CagedShape::A => "A",
            CagedShape::G => "G",
            CagedShape::E => "E",
            CagedShape::D => "D",
        }
    }
}

/// A movable shape: which strings sound, what each carries, and which quality's degrees
/// the offsets were measured against.
#[derive(Debug, Clone, Copy)]
struct Shape {
    caged: CagedShape,
    /// How this entry is reduced from the full shape, or `None` for one of the five. Display only.
    reduction: Option<&'static str>,
    strings: [StringRole; 6],
    base: ChordQuality,
    root_string: usize,
}

use StringRole::{Muted, Sounded};

/// The shape families. Placed at the nut, each entry has to come out as the open chord
/// it was drawn from.
const SHAPES: &[Shape] = &[
    // Triads, all five CAGED shapes.
    Shape {
        caged: CagedShape::E,
        reduction: None,
        strings: [
            Sounded {
                offset: 0,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 5,
            },
            Sounded {
                offset: 2,
                degree: 1,
            },
            Sounded {
                offset: 1,
                degree: 3,
            },
            Sounded {
                offset: 0,
                degree: 5,
            },
            Sounded {
                offset: 0,
                degree: 1,
            },
        ],
        base: ChordQuality::Major,
        root_string: 0,
    },
    Shape {
        caged: CagedShape::A,
        reduction: None,
        strings: [
            Muted,
            Sounded {
                offset: 0,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 5,
            },
            Sounded {
                offset: 2,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 3,
            },
            Sounded {
                offset: 0,
                degree: 5,
            },
        ],
        base: ChordQuality::Major,
        root_string: 1,
    },
    Shape {
        caged: CagedShape::D,
        reduction: None,
        strings: [
            Muted,
            Muted,
            Sounded {
                offset: 0,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 5,
            },
            Sounded {
                offset: 3,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 3,
            },
        ],
        base: ChordQuality::Major,
        root_string: 2,
    },
    Shape {
        caged: CagedShape::C,
        reduction: None,
        strings: [
            Muted,
            Sounded {
                offset: 3,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 3,
            },
            Sounded {
                offset: 0,
                degree: 5,
            },
            Sounded {
                offset: 1,
                degree: 1,
            },
            Sounded {
                offset: 0,
                degree: 3,
            },
        ],
        base: ChordQuality::Major,
        root_string: 1,
    },
    Shape {
        caged: CagedShape::G,
        reduction: None,
        strings: [
            Sounded {
                offset: 3,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 3,
            },
            Sounded {
                offset: 0,
                degree: 5,
            },
            Sounded {
                offset: 0,
                degree: 1,
            },
            Sounded {
                offset: 0,
                degree: 3,
            },
            Sounded {
                offset: 3,
                degree: 1,
            },
        ],
        base: ChordQuality::Major,
        root_string: 0,
    },
    // Suspensions. `1 2 5` and `1 4 5` are different degree sets, so neither can borrow
    // the triad shapes above.
    Shape {
        caged: CagedShape::E,
        reduction: None,
        strings: [
            Sounded {
                offset: 0,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 5,
            },
            Sounded {
                offset: 2,
                degree: 1,
            },
            Muted,
            Sounded {
                offset: 0,
                degree: 5,
            },
            Sounded {
                offset: 2,
                degree: 2,
            },
        ],
        base: ChordQuality::Sus2,
        root_string: 0,
    },
    Shape {
        caged: CagedShape::A,
        reduction: None,
        strings: [
            Muted,
            Sounded {
                offset: 0,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 5,
            },
            Sounded {
                offset: 2,
                degree: 1,
            },
            Sounded {
                offset: 0,
                degree: 2,
            },
            Sounded {
                offset: 0,
                degree: 5,
            },
        ],
        base: ChordQuality::Sus2,
        root_string: 1,
    },
    Shape {
        caged: CagedShape::E,
        reduction: None,
        strings: [
            Sounded {
                offset: 0,
                degree: 1,
            },
            Sounded {
                offset: 0,
                degree: 4,
            },
            Sounded {
                offset: 2,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 4,
            },
            Sounded {
                offset: 0,
                degree: 5,
            },
            Sounded {
                offset: 0,
                degree: 1,
            },
        ],
        base: ChordQuality::Sus4,
        root_string: 0,
    },
    Shape {
        caged: CagedShape::A,
        reduction: None,
        strings: [
            Muted,
            Sounded {
                offset: 0,
                degree: 1,
            },
            Sounded {
                offset: 0,
                degree: 4,
            },
            Sounded {
                offset: 2,
                degree: 1,
            },
            Sounded {
                offset: 3,
                degree: 4,
            },
            Sounded {
                offset: 0,
                degree: 5,
            },
        ],
        base: ChordQuality::Sus4,
        root_string: 1,
    },
    // Sixths.
    Shape {
        caged: CagedShape::E,
        reduction: None,
        strings: [
            Sounded {
                offset: 0,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 5,
            },
            Sounded {
                offset: 2,
                degree: 1,
            },
            Sounded {
                offset: 1,
                degree: 3,
            },
            Sounded {
                offset: 2,
                degree: 6,
            },
            Sounded {
                offset: 0,
                degree: 1,
            },
        ],
        base: ChordQuality::Major6,
        root_string: 0,
    },
    Shape {
        caged: CagedShape::A,
        reduction: None,
        strings: [
            Muted,
            Sounded {
                offset: 0,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 5,
            },
            Sounded {
                offset: 2,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 3,
            },
            Sounded {
                offset: 2,
                degree: 6,
            },
        ],
        base: ChordQuality::Major6,
        root_string: 1,
    },
    // Sevenths. Seven of the fifteen qualities share `1 3 5 7`.
    Shape {
        caged: CagedShape::E,
        reduction: None,
        strings: [
            Sounded {
                offset: 0,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 5,
            },
            Sounded {
                offset: 0,
                degree: 7,
            },
            Sounded {
                offset: 1,
                degree: 3,
            },
            Sounded {
                offset: 0,
                degree: 5,
            },
            Sounded {
                offset: 0,
                degree: 1,
            },
        ],
        base: ChordQuality::Dominant7,
        root_string: 0,
    },
    Shape {
        caged: CagedShape::A,
        reduction: None,
        strings: [
            Muted,
            Sounded {
                offset: 0,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 5,
            },
            Sounded {
                offset: 0,
                degree: 7,
            },
            Sounded {
                offset: 2,
                degree: 3,
            },
            Sounded {
                offset: 0,
                degree: 5,
            },
        ],
        base: ChordQuality::Dominant7,
        root_string: 1,
    },
    // Reduced shapes: the six-string ones above only stay fingerable near the nut, and
    // dropping a string or two makes most alterations placeable again.
    Shape {
        caged: CagedShape::E,
        reduction: Some("four strings"),
        strings: [
            Sounded {
                offset: 0,
                degree: 1,
            },
            Muted,
            Muted,
            Sounded {
                offset: 1,
                degree: 3,
            },
            Sounded {
                offset: 0,
                degree: 5,
            },
            Sounded {
                offset: 0,
                degree: 1,
            },
        ],
        base: ChordQuality::Major,
        root_string: 0,
    },
    Shape {
        caged: CagedShape::A,
        reduction: Some("three strings"),
        strings: [
            Muted,
            Sounded {
                offset: 0,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 5,
            },
            Muted,
            Sounded {
                offset: 2,
                degree: 3,
            },
            Muted,
        ],
        base: ChordQuality::Major,
        root_string: 1,
    },
    Shape {
        caged: CagedShape::E,
        reduction: Some("four strings"),
        strings: [
            Sounded {
                offset: 0,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 5,
            },
            Muted,
            Sounded {
                offset: 1,
                degree: 3,
            },
            Sounded {
                offset: 2,
                degree: 6,
            },
            Sounded {
                offset: 0,
                degree: 1,
            },
        ],
        base: ChordQuality::Major6,
        root_string: 0,
    },
    Shape {
        caged: CagedShape::A,
        reduction: Some("four strings"),
        strings: [
            Muted,
            Sounded {
                offset: 0,
                degree: 1,
            },
            Sounded {
                offset: 2,
                degree: 5,
            },
            Muted,
            Sounded {
                offset: 2,
                degree: 3,
            },
            Sounded {
                offset: 2,
                degree: 6,
            },
        ],
        base: ChordQuality::Major6,
        root_string: 1,
    },
    Shape {
        caged: CagedShape::E,
        reduction: Some("four strings"),
        strings: [
            Sounded {
                offset: 0,
                degree: 1,
            },
            Muted,
            Sounded {
                offset: 0,
                degree: 7,
            },
            Sounded {
                offset: 1,
                degree: 3,
            },
            Sounded {
                offset: 0,
                degree: 5,
            },
            Muted,
        ],
        base: ChordQuality::Dominant7,
        root_string: 0,
    },
    Shape {
        caged: CagedShape::A,
        reduction: Some("four strings"),
        strings: [
            Muted,
            Sounded {
                offset: 0,
                degree: 1,
            },
            Muted,
            Sounded {
                offset: 0,
                degree: 7,
            },
            Sounded {
                offset: 2,
                degree: 3,
            },
            Sounded {
                offset: 0,
                degree: 5,
            },
        ],
        base: ChordQuality::Dominant7,
        root_string: 1,
    },
];

/// One way to play a chord: an absolute fret per string, or nothing sounding there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Voicing {
    strings: [Option<u8>; 6],
    /// The fret the shape sits at. Zero is the open position.
    index_fret: u8,
    caged: CagedShape,
    reduction: Option<&'static str>,
    /// The shape's own fingering, carried into this placement.
    fingering: [Option<u8>; 6],
}

impl Voicing {
    pub(super) fn strings(self) -> [Option<u8>; 6] {
        self.strings
    }

    pub(super) fn index_fret(self) -> u8 {
        self.index_fret
    }

    pub(super) fn shape_name(self) -> String {
        self.caged.name(self.reduction)
    }

    pub(super) fn caged(self) -> CagedShape {
        self.caged
    }

    /// Whether this is one of the five, rather than a reduction of one.
    pub(super) fn is_caged(self) -> bool {
        self.reduction.is_none()
    }

    /// The frets that a finger has to stop.
    fn stopped(self) -> impl Iterator<Item = u8> {
        self.strings.into_iter().flatten().filter(|&fret| fret > 0)
    }

    /// Which finger stops each string, or `None` where nothing does.
    ///
    /// The shape's own fingering where it stands, since a released finger is invisible to the
    /// frets (Em is `0 2 2 0 0 0`); an ascending-fret assignment where it does not.
    pub(super) fn fingers(self) -> [Option<u8>; 6] {
        self.shape_fingering()
            .unwrap_or_else(|| self.ordered_fingering())
    }

    /// The fingering carried from the shape, or `None` where it does not describe this placement.
    ///
    /// It stops describing it when an alteration needs a fifth finger, a barred string held by
    /// something other than the first, one finger on two unbarred strings, or a crossing.
    /// Am is the everyday case: the flattened third would cross.
    fn shape_fingering(self) -> Option<[Option<u8>; 6]> {
        let barre = self.barre_fret();
        let stopped = self.strings.map(|fret| fret.filter(|&fret| fret > 0));

        let mut assigned: Vec<(u8, u8)> = Vec::new();
        for pair in self.fingering.into_iter().zip(stopped) {
            match pair {
                (Some(finger), Some(fret)) => {
                    if usize::from(finger) > FINGERS || (Some(fret) == barre && finger != 1) {
                        return None;
                    }

                    assigned.push((finger, fret));
                }
                (None, None) => {}
                // A stopped string with nobody on it, or a finger on a string nothing stops.
                _ => return None,
            }
        }

        // Sorted by finger, so the frets beside them may not descend. Two strings may share a
        // finger only where the bar covers them both.
        assigned.sort_unstable();
        assigned
            .windows(2)
            .all(|pair| {
                let &[(finger, fret), (next_finger, next_fret)] = pair else {
                    return true;
                };

                if finger == next_finger {
                    Some(fret) == barre && Some(next_fret) == barre
                } else {
                    fret <= next_fret
                }
            })
            .then_some(self.fingering)
    }

    /// Fingers by ascending fret, the lower string first where two share one.
    fn ordered_fingering(self) -> [Option<u8>; 6] {
        let barre = self.barre_fret();

        // Ordered by `(fret, string)`, not the fret alone: G is `3 2 0 0 0 3`, and one finger
        // cannot reach two third-fret strings with three between them.
        let mut order: Vec<(u8, usize)> = (0..6)
            .filter_map(|string| {
                let fret = self.strings[string].filter(|&fret| fret > 0)?;

                (Some(fret) != barre).then_some((fret, string))
            })
            .collect();
        order.sort_unstable();

        std::array::from_fn(|string| {
            let fret = self.strings[string].filter(|&fret| fret > 0)?;

            if Some(fret) == barre {
                return Some(1);
            }

            let rank = order.iter().position(|&(_, at)| at == string)?;
            // Past the fourth finger the dot goes unlabelled.
            u8::try_from(rank)
                .ok()?
                .checked_add(if barre.is_some() { 2 } else { 1 })
                .filter(|&finger| usize::from(finger) <= FINGERS)
        })
    }

    /// Whether a hand has enough fingers for this, a barre counting as one.
    fn is_fingerable(self) -> bool {
        self.fingers()
            .iter()
            .zip(self.strings)
            .all(|(finger, fret)| finger.is_some() || !matches!(fret, Some(f) if f > 0))
    }

    /// The fret a barre sits on: the lowest stopped one, when more than one string rests
    /// there *and* the voicing needs more fingers than a hand has.
    ///
    /// The lowest stopped fret, not the index fret: C♯°7 on the A shape is `x 4 5 3 5 3`.
    /// The strings resting on it need not be adjacent.
    pub(super) fn barre_fret(self) -> Option<u8> {
        let lowest = self.stopped().min()?;
        let resting = self.stopped().filter(|&fret| fret == lowest).count();

        // No barre below five stopped strings: open D could bar `x x 0 2 3 2` but nobody does.
        (resting > 1 && self.stopped().count() > FINGERS).then_some(lowest)
    }
}

impl Shape {
    /// This shape with every string stopped: the fingering of its barre chord.
    ///
    /// The first finger holds the index fret; the offsets above it take the rest in order, the
    /// lower string first where two share one.
    fn movable_fingering(&self) -> [Option<u8>; 6] {
        let mut above: Vec<(u8, usize)> = self
            .strings
            .iter()
            .enumerate()
            .filter_map(|(string, role)| match *role {
                Sounded { offset, .. } if offset > 0 => Some((offset, string)),
                _ => None,
            })
            .collect();
        above.sort_unstable();

        std::array::from_fn(|string| match self.strings[string] {
            Muted => None,
            Sounded { offset: 0, .. } => Some(1),
            Sounded { .. } => above
                .iter()
                .position(|&(_, at)| at == string)
                .and_then(|rank| u8::try_from(rank + 2).ok()),
        })
    }

    /// Places this shape so its root lands on `root`, built as `kind`.
    ///
    /// `None` when the shape cannot carry the quality or the placement is unplayable:
    /// a stopped fret below the nut, a stretch beyond `REACH`, or a fret past the neck.
    fn place(
        &self,
        root: PitchClass,
        kind: ChordQuality,
        index_fret: u8,
        neck: &Neck,
    ) -> Option<Voicing> {
        if !self.carries(kind) {
            return None;
        }

        // The shape only sits here if its root string lands on the chord's root.
        let root_at = self.string_pitch(self.root_string, index_fret, kind, neck)?;
        if root_at != root {
            return None;
        }

        let mut strings = [None; 6];
        for (index, role) in self.strings.iter().enumerate() {
            let Sounded { offset, degree } = *role else {
                continue;
            };

            let fret = i16::from(index_fret) + i16::from(offset) + self.shift(degree, kind)?;
            let fret = u8::try_from(fret).ok()?;

            if usize::from(fret) > neck.frets() {
                return None;
            }

            strings[index] = Some(fret);
        }

        let carried = self.movable_fingering();

        // The first finger is off the neck when every string it holds sounds open, and the rest
        // come down one to meet it. Em is E major with its third released, not renumbered.
        let released = !(0..6).any(|string| {
            carried[string] == Some(1) && strings[string].is_some_and(|fret| fret > 0)
        });

        let fingering = std::array::from_fn(|string| {
            strings[string]
                .filter(|&fret| fret > 0)
                .and(carried[string])
                .and_then(|finger| finger.checked_sub(u8::from(released)))
        });

        let voicing = Voicing {
            strings,
            index_fret,
            caged: self.caged,
            reduction: self.reduction,
            fingering,
        };

        let stopped: Vec<u8> = voicing.stopped().collect();
        let span = stopped.iter().max()?.checked_sub(*stopped.iter().min()?)?;

        if span > REACH || !voicing.is_fingerable() {
            return None;
        }

        Some(voicing)
    }

    /// Whether `kind` names the same degree numbers this shape was drawn against.
    /// A quality missing a degree the shape holds is refused rather than muted.
    fn carries(&self, kind: ChordQuality) -> bool {
        let mut theirs: Vec<u8> = kind.degrees().collect();
        let mut ours: Vec<u8> = self.base.degrees().collect();
        theirs.sort_unstable();
        ours.sort_unstable();

        theirs == ours
    }

    /// How far a degree moves when the shape's base quality becomes `kind`.
    fn shift(&self, degree: u8, kind: ChordQuality) -> Option<i16> {
        let semitones = |quality: ChordQuality| {
            quality
                .intervals()
                .iter()
                .find(|interval| interval.number() == degree)
                .map(|interval| i16::from(interval.semitones()))
        };

        Some(semitones(kind)? - semitones(self.base)?)
    }

    /// What a string sounds once the shape is placed and altered.
    fn string_pitch(
        &self,
        index: usize,
        index_fret: u8,
        kind: ChordQuality,
        neck: &Neck,
    ) -> Option<PitchClass> {
        let Sounded { offset, degree } = self.strings[index] else {
            return None;
        };

        let fret = i16::from(index_fret) + i16::from(offset) + self.shift(degree, kind)?;
        let fret = u8::try_from(fret).ok()?;

        neck.pitch_class_at(index, usize::from(fret))
    }
}

/// Every way the library can play `chord`, lowest position first, octave repeats included.
pub(super) fn voicings(chord: Chord, neck: &Neck) -> Vec<Voicing> {
    let mut found: Vec<Voicing> = SHAPES
        .iter()
        .flat_map(|shape| {
            (0..=neck.frets() as u8).filter_map(move |index_fret| {
                shape.place(chord.root(), chord.kind(), index_fret, neck)
            })
        })
        .collect();

    found.sort_by_key(|voicing| (voicing.index_fret(), voicing.shape_name()));
    found
}

/// How a voicing's position reads: the nut, or the fret its diagram begins at.
/// Read from `window_for` rather than the index fret, which can disagree with the diagram.
pub(super) fn position_label(voicing: Voicing) -> String {
    let window = window_for(&voicing.strings());

    if window.shows_nut() {
        "open".to_string()
    } else {
        format!("{}fr", window.first_fret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ui::neck::{CAGED_NECK, DRILL_NECK, STANDARD_TUNING};

    fn pc(semitone: u8) -> PitchClass {
        PitchClass::new(semitone)
    }

    fn frets(voicing: Voicing) -> Vec<Option<u8>> {
        voicing.strings().to_vec()
    }

    fn only(chord: Chord, name: &str, index_fret: u8) -> Voicing {
        voicings(chord, &DRILL_NECK)
            .into_iter()
            .find(|v| v.shape_name() == name && v.index_fret() == index_fret)
            .unwrap_or_else(|| panic!("{name} at {index_fret} is not offered for {chord}"))
    }

    /// Why `CAGED_NECK` is fifteen frets rather than twelve: on twelve, most roots cannot
    /// complete the cycle.
    #[test]
    fn the_cycle_completes_on_the_caged_neck_and_not_on_the_drills() {
        let letters = |root: PitchClass, neck: &Neck| {
            let mut found: Vec<CagedShape> = voicings(Chord::new(root, ChordQuality::Major), neck)
                .into_iter()
                .filter(|voicing| voicing.is_caged())
                .map(Voicing::caged)
                .collect();
            found.sort_by_key(|shape| shape.letter());
            found.dedup();
            found
        };

        let truncated: Vec<PitchClass> = PitchClass::ALL
            .into_iter()
            .filter(|&root| letters(root, &DRILL_NECK).len() < 5)
            .collect();

        for root in PitchClass::ALL {
            assert_eq!(
                letters(root, &CAGED_NECK).len(),
                5,
                "{root:?} is short a shape on a fifteen-fret neck"
            );
        }

        // The four that survive twelve frets are the roots of the open shapes themselves.
        assert_eq!(truncated.len(), 8, "{truncated:?}");
        let survives: Vec<u8> = PitchClass::ALL
            .iter()
            .filter(|root| !truncated.contains(root))
            .map(|root| root.semitone())
            .collect();
        assert_eq!(survives, [2, 4, 7, 9]);
    }

    #[test]
    fn every_shape_is_named_what_it_was_named_before() {
        let expected = [
            "E shape",
            "A shape",
            "D shape",
            "C shape",
            "G shape",
            "E shape",
            "A shape",
            "E shape",
            "A shape",
            "E shape",
            "A shape",
            "E shape",
            "A shape",
            "E shape, four strings",
            "A shape, three strings",
            "E shape, four strings",
            "A shape, four strings",
            "E shape, four strings",
            "A shape, four strings",
        ];

        let names: Vec<String> = SHAPES
            .iter()
            .map(|shape| shape.caged.name(shape.reduction))
            .collect();

        assert_eq!(names, expected);
    }

    #[test]
    fn a_major_triad_offers_the_five_shapes_among_the_reductions() {
        let c_major = Chord::new(pc(0), ChordQuality::Major);
        let offered = voicings(c_major, &DRILL_NECK);

        let shapes: Vec<CagedShape> = SHAPES
            .iter()
            .filter(|shape| shape.base == ChordQuality::Major)
            .map(|shape| shape.caged)
            .collect();
        assert_eq!(shapes.len(), 7, "{shapes:?}");

        let five: Vec<CagedShape> = SHAPES
            .iter()
            .filter(|shape| shape.base == ChordQuality::Major && shape.reduction.is_none())
            .map(|shape| shape.caged)
            .collect();
        assert_eq!(
            five,
            [
                CagedShape::E,
                CagedShape::A,
                CagedShape::D,
                CagedShape::C,
                CagedShape::G
            ]
        );

        // The predicate has to survive placement, not just the table.
        assert!(offered.iter().any(|voicing| !voicing.is_caged()));
        assert!(
            offered
                .iter()
                .filter(|voicing| voicing.is_caged())
                .all(|voicing| matches!(
                    voicing.caged(),
                    CagedShape::C | CagedShape::A | CagedShape::G | CagedShape::E | CagedShape::D
                ))
        );
    }

    #[test]
    fn every_shape_carries_the_degrees_its_base_quality_names() {
        for shape in SHAPES {
            let mut sounded: Vec<u8> = shape
                .strings
                .iter()
                .filter_map(|role| match role {
                    Sounded { degree, .. } => Some(*degree),
                    Muted => None,
                })
                .collect();
            sounded.sort_unstable();
            sounded.dedup();

            let mut declared: Vec<u8> = shape.base.degrees().collect();
            declared.sort_unstable();

            assert_eq!(
                sounded, declared,
                "{:?} on {:?} covers the wrong degrees",
                shape.caged, shape.base
            );
        }
    }

    #[test]
    fn every_shape_puts_its_root_on_its_root_string() {
        for shape in SHAPES {
            assert!(
                matches!(shape.strings[shape.root_string], Sounded { degree: 1, .. }),
                "{:?} on {:?} has no root on its root string",
                shape.caged,
                shape.base
            );
        }
    }

    #[test]
    fn every_shape_has_a_string_at_its_index_fret() {
        // The lowest stopped string sits at zero, so placing a shape at fret n puts a barre there.
        for shape in SHAPES {
            let lowest = shape
                .strings
                .iter()
                .filter_map(|role| match role {
                    Sounded { offset, .. } => Some(*offset),
                    Muted => None,
                })
                .min();

            assert_eq!(lowest, Some(0), "{:?} on {:?}", shape.caged, shape.base);
        }
    }

    #[test]
    fn every_shape_fits_a_hand() {
        for shape in SHAPES {
            let offsets: Vec<u8> = shape
                .strings
                .iter()
                .filter_map(|role| match role {
                    Sounded { offset, .. } => Some(*offset),
                    Muted => None,
                })
                .collect();
            let span = offsets.iter().max().unwrap() - offsets.iter().min().unwrap();

            assert!(
                span <= REACH,
                "{:?} on {:?} spans {span}",
                shape.caged,
                shape.base
            );
        }
    }

    #[test]
    fn every_family_has_a_shape() {
        // Specified, but should never be true of the fifteen that ship.
        for &kind in ChordQuality::ALL {
            assert!(
                SHAPES.iter().any(|shape| shape.carries(kind)),
                "{kind:?} has no shape family"
            );
        }
    }

    #[test]
    fn the_open_chords_come_out_of_the_arithmetic() {
        // Placed at the nut, a shape has to reproduce the open chord it was drawn from.
        let cases: &[(u8, ChordQuality, &str, &[Option<u8>])] = &[
            (
                4,
                ChordQuality::Major,
                "E shape",
                &[Some(0), Some(2), Some(2), Some(1), Some(0), Some(0)],
            ),
            (
                9,
                ChordQuality::Major,
                "A shape",
                &[None, Some(0), Some(2), Some(2), Some(2), Some(0)],
            ),
            (
                2,
                ChordQuality::Major,
                "D shape",
                &[None, None, Some(0), Some(2), Some(3), Some(2)],
            ),
            (
                0,
                ChordQuality::Major,
                "C shape",
                &[None, Some(3), Some(2), Some(0), Some(1), Some(0)],
            ),
            (
                7,
                ChordQuality::Major,
                "G shape",
                &[Some(3), Some(2), Some(0), Some(0), Some(0), Some(3)],
            ),
            (
                9,
                ChordQuality::Sus2,
                "A shape",
                &[None, Some(0), Some(2), Some(2), Some(0), Some(0)],
            ),
            (
                4,
                ChordQuality::Sus4,
                "E shape",
                &[Some(0), Some(0), Some(2), Some(2), Some(0), Some(0)],
            ),
            (
                4,
                ChordQuality::Major6,
                "E shape",
                &[Some(0), Some(2), Some(2), Some(1), Some(2), Some(0)],
            ),
            (
                4,
                ChordQuality::Dominant7,
                "E shape",
                &[Some(0), Some(2), Some(0), Some(1), Some(0), Some(0)],
            ),
            (
                9,
                ChordQuality::Dominant7,
                "A shape",
                &[None, Some(0), Some(2), Some(0), Some(2), Some(0)],
            ),
        ];

        for &(root, kind, name, expected) in cases {
            let voicing = only(Chord::new(pc(root), kind), name, 0);

            assert_eq!(frets(voicing), expected, "{kind:?} on {root} as {name}");
        }
    }

    #[test]
    fn altering_a_degree_moves_only_the_strings_carrying_it() {
        // E major becomes E minor by lowering the one string holding the third.
        let major = frets(only(Chord::new(pc(4), ChordQuality::Major), "E shape", 0));
        let minor = frets(only(Chord::new(pc(4), ChordQuality::Minor), "E shape", 0));

        assert_eq!(
            minor,
            vec![Some(0), Some(2), Some(2), Some(0), Some(0), Some(0)]
        );

        let moved: Vec<usize> = (0..6).filter(|&i| major[i] != minor[i]).collect();
        assert_eq!(moved, vec![3]);
    }

    #[test]
    fn a_shape_at_two_positions_differs_by_that_distance() {
        // G major and A major are the same E shape three frets apart.
        let g = only(Chord::new(pc(7), ChordQuality::Major), "E shape", 3);
        let a = only(Chord::new(pc(9), ChordQuality::Major), "E shape", 5);

        for (left, right) in g.strings().iter().zip(a.strings().iter()) {
            match (left, right) {
                (Some(l), Some(r)) => assert_eq!(r - l, 2),
                (None, None) => {}
                _ => panic!("the same shape muted different strings"),
            }
        }
    }

    #[test]
    fn a_voicing_sounds_the_chord_and_nothing_else() {
        // Every sounded string has to be a chord tone, for every chord.
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                let chord = Chord::new(root, kind);
                let tones: Vec<PitchClass> = chord
                    .degrees()
                    .iter()
                    .map(|interval| root.transpose(interval.semitones()))
                    .collect();

                for voicing in voicings(chord, &DRILL_NECK) {
                    for (string, fret) in voicing.strings().iter().enumerate() {
                        let Some(fret) = *fret else { continue };
                        let sounded = STANDARD_TUNING[string].pitch_class().transpose(fret % 12);

                        assert!(
                            tones.contains(&sounded),
                            "{chord} as {} at {} sounds a foreign note on string {string}",
                            voicing.shape_name(),
                            voicing.index_fret(),
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_voicing_stays_on_the_neck_and_within_a_hand() {
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                for voicing in voicings(Chord::new(root, kind), &DRILL_NECK) {
                    let stopped: Vec<u8> = voicing.stopped().collect();

                    for fret in voicing.strings().iter().flatten() {
                        assert!(
                            *fret <= DRILL_NECK.frets() as u8,
                            "{root:?} {kind:?} runs off the neck"
                        );
                    }

                    if let (Some(low), Some(high)) = (stopped.iter().min(), stopped.iter().max()) {
                        assert!(
                            high - low <= REACH,
                            "{root:?} {kind:?} spans {}",
                            high - low
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn voicings_are_ordered_up_the_neck() {
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                let found = voicings(Chord::new(root, kind), &DRILL_NECK);

                for pair in found.windows(2) {
                    assert!(pair[0].index_fret() <= pair[1].index_fret());
                }
            }
        }
    }

    #[test]
    fn a_position_reads_as_the_nut_or_a_fret() {
        let open = only(Chord::new(pc(4), ChordQuality::Major), "E shape", 0);
        let up = only(Chord::new(pc(7), ChordQuality::Major), "E shape", 3);

        assert_eq!(position_label(open), "open");
        assert_eq!(position_label(up), "3fr");
    }

    #[test]
    fn a_caption_says_what_its_diagram_shows() {
        // C°7 sits on the A shape at the third fret but reaches the second, so it opens at the nut.
        let c_dim7 = Chord::new(pc(0), ChordQuality::Diminished7);

        for voicing in voicings(c_dim7, &DRILL_NECK) {
            let window = window_for(&voicing.strings());
            let caption = position_label(voicing);

            if window.shows_nut() {
                assert_eq!(caption, "open", "{voicing:?} captioned away from its nut");
            } else {
                assert_eq!(caption, format!("{}fr", window.first_fret));
            }
        }
    }

    #[test]
    fn every_caption_agrees_with_its_window() {
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                for voicing in voicings(Chord::new(root, kind), &DRILL_NECK) {
                    let window = window_for(&voicing.strings());
                    let caption = position_label(voicing);

                    assert_eq!(
                        caption == "open",
                        window.shows_nut(),
                        "{root:?} {kind:?} captions {caption} against {window:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_placement_below_the_nut_is_refused() {
        // The C shape holds its third on an open string, so C minor is not a C shape at the nut.
        let c_minor = Chord::new(pc(0), ChordQuality::Minor);

        assert!(
            !voicings(c_minor, &DRILL_NECK)
                .iter()
                .any(|v| v.shape_name() == "C shape" && v.index_fret() == 0)
        );
        assert!(!voicings(c_minor, &DRILL_NECK).is_empty());
    }

    #[test]
    fn a_quality_cannot_borrow_a_shape_from_another_family() {
        // Sus4 is `1 4 5`, so no triad shape can carry it however the offsets fall.
        for shape in SHAPES.iter().filter(|s| s.base == ChordQuality::Major) {
            assert!(!shape.carries(ChordQuality::Sus4));
            assert!(!shape.carries(ChordQuality::Dominant7));
            assert!(shape.carries(ChordQuality::Minor));
        }
    }

    #[test]
    fn every_stopped_string_gets_a_finger() {
        // `C♯°7` on the A shape is `x 4 5 3 5 3`: two strings drop below the index fret.
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                for voicing in voicings(Chord::new(root, kind), &DRILL_NECK) {
                    let fingers = voicing.fingers();

                    for (string, fret) in voicing.strings().iter().enumerate() {
                        if matches!(fret, Some(f) if *f > 0) {
                            assert!(
                                fingers[string].is_some(),
                                "{root:?} {kind:?} as {} leaves string {string} unfingered",
                                voicing.shape_name(),
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_barre_sits_on_the_lowest_stopped_fret() {
        let c_sharp_dim7 = Chord::new(pc(1), ChordQuality::Diminished7);
        let voicing = voicings(c_sharp_dim7, &DRILL_NECK)
            .into_iter()
            .find(|v| v.shape_name() == "A shape")
            .expect("the A shape carries a diminished seventh");

        assert_eq!(
            voicing.strings()[1],
            Some(4),
            "the shape sits at the fourth"
        );
        assert_eq!(voicing.barre_fret(), Some(3), "the barre is below it");
    }

    #[test]
    fn a_voicing_within_a_hand_is_not_barred() {
        // Open D is `x x 0 2 3 2`, barre-able in principle but fingered with three fingers.
        let open_d = only(Chord::new(pc(2), ChordQuality::Major), "D shape", 0);

        assert_eq!(open_d.barre_fret(), None);
    }

    #[test]
    fn a_barre_takes_the_first_finger_and_nothing_exceeds_the_fourth() {
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                for voicing in voicings(Chord::new(root, kind), &DRILL_NECK) {
                    let fingers = voicing.fingers();

                    for (string, finger) in fingers.iter().enumerate() {
                        let Some(finger) = *finger else { continue };
                        assert!((1..=4).contains(&finger), "finger {finger} on {string}");

                        if Some(voicing.strings()[string].unwrap_or(0)) == voicing.barre_fret() {
                            assert_eq!(finger, 1, "the barre is not the first finger");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_open_chords_are_fingered_the_way_they_are_taught() {
        // G and D each put two strings on one fret with others between them.
        let cases: &[(u8, ChordQuality, &str, [Option<u8>; 6])] = &[
            // x 3 2 0 1 0 — ring, middle, index walking down the neck.
            (
                0,
                ChordQuality::Major,
                "C shape",
                [None, Some(3), Some(2), None, Some(1), None],
            ),
            // 3 2 0 0 0 3 — middle and ring at the third, index at the second between them.
            (
                7,
                ChordQuality::Major,
                "G shape",
                [Some(2), Some(1), None, None, None, Some(3)],
            ),
            // x x 0 2 3 2 — the two second-fret strings take different fingers.
            (
                2,
                ChordQuality::Major,
                "D shape",
                [None, None, None, Some(1), Some(3), Some(2)],
            ),
            // 0 2 2 1 0 0 — index on the third string, middle and ring below it.
            (
                4,
                ChordQuality::Major,
                "E shape",
                [None, Some(2), Some(3), Some(1), None, None],
            ),
            // x 0 2 2 2 0 — three fingers in a row at the second fret.
            (
                9,
                ChordQuality::Major,
                "A shape",
                [None, None, Some(1), Some(2), Some(3), None],
            ),
            // x 0 2 2 1 0 — the minor's flattened third moves one finger, not the rest.
            (
                9,
                ChordQuality::Minor,
                "A shape",
                [None, None, Some(2), Some(3), Some(1), None],
            ),
            // 0 2 2 0 0 0 — the third falls open; the other fingers keep their E major numbers.
            (
                4,
                ChordQuality::Minor,
                "E shape",
                [None, Some(2), Some(3), None, None, None],
            ),
            // x x 0 2 3 1 — the third stays fretted, so the frets order the fingers.
            (
                2,
                ChordQuality::Minor,
                "D shape",
                [None, None, None, Some(2), Some(3), Some(1)],
            ),
        ];

        for &(root, kind, shape, expected) in cases {
            let voicing = only(Chord::new(pc(root), kind), shape, 0);

            assert_eq!(
                voicing.fingers(),
                expected,
                "{kind:?} on {root} as {shape}: {:?}",
                voicing.strings()
            );
        }
    }

    #[test]
    fn a_shape_fully_stopped_is_fingered_like_its_barre_chord() {
        // The five CAGED triads as their barre chords are played.
        let cases: &[(&str, [Option<u8>; 6])] = &[
            (
                "E shape",
                [Some(1), Some(3), Some(4), Some(2), Some(1), Some(1)],
            ),
            (
                "A shape",
                [None, Some(1), Some(2), Some(3), Some(4), Some(1)],
            ),
            ("D shape", [None, None, Some(1), Some(2), Some(4), Some(3)]),
            (
                "C shape",
                [None, Some(4), Some(3), Some(1), Some(2), Some(1)],
            ),
            (
                "G shape",
                [Some(3), Some(2), Some(1), Some(1), Some(1), Some(4)],
            ),
        ];

        for &(name, expected) in cases {
            let shape = SHAPES
                .iter()
                .find(|shape| {
                    shape.caged.name(shape.reduction) == name && shape.base == ChordQuality::Major
                })
                .unwrap_or_else(|| panic!("{name} is not in the table"));

            assert_eq!(shape.movable_fingering(), expected, "{name}");
        }
    }

    #[test]
    fn no_shape_skips_a_finger_or_doubles_one_above_its_index_fret() {
        // The first finger holds the index fret; each finger above holds one string, with no gap.
        for shape in SHAPES {
            let fingering = shape.movable_fingering();

            let mut named: Vec<u8> = fingering.iter().flatten().copied().collect();
            named.sort_unstable();
            named.dedup();

            let run: Vec<u8> = (1..=u8::try_from(named.len()).unwrap_or(u8::MAX)).collect();
            assert_eq!(named, run, "{:?} skips a finger", shape.caged);

            for finger in named.into_iter().filter(|&finger| finger > 1) {
                let held = fingering
                    .iter()
                    .filter(|&&held| held == Some(finger))
                    .count();

                assert_eq!(
                    held, 1,
                    "{:?} puts finger {finger} on two strings",
                    shape.caged
                );
            }
        }
    }

    #[test]
    fn a_barred_shape_is_fingered_the_way_the_shape_records_it() {
        // F major is the E shape with nothing released, so the fingering is the shape's own.
        let voicing = only(Chord::new(pc(5), ChordQuality::Major), "E shape", 1);

        assert_eq!(
            frets(voicing),
            vec![Some(1), Some(3), Some(3), Some(2), Some(1), Some(1)]
        );
        assert_eq!(
            voicing.fingers(),
            [Some(1), Some(3), Some(4), Some(2), Some(1), Some(1)]
        );
    }

    #[test]
    fn a_sharpened_degree_on_the_index_fret_keeps_the_first_finger_busy() {
        // The sharpened fifth stops a string the first finger held, so nothing is lowered.
        let voicing = only(Chord::new(pc(4), ChordQuality::Augmented), "E shape", 0);

        assert_eq!(
            frets(voicing),
            vec![Some(0), Some(3), Some(2), Some(1), Some(1), Some(0)]
        );
        assert!(
            voicing
                .fingers()
                .iter()
                .flatten()
                .all(|&finger| (1..=4).contains(&finger)),
            "{:?}",
            voicing.fingers()
        );
    }

    #[test]
    fn a_carried_fingering_stands_only_where_it_describes_the_placement() {
        let open_d = [None, None, Some(0), Some(2), Some(3), Some(2)];
        let f_major = [Some(1), Some(3), Some(3), Some(2), Some(1), Some(1)];

        let stands = |strings, fingering| {
            Voicing {
                strings,
                index_fret: 0,
                // Unread by `shape_fingering`.
                caged: CagedShape::E,
                reduction: None,
                fingering,
            }
            .shape_fingering()
            .is_some()
        };

        assert!(stands(
            open_d,
            [None, None, None, Some(1), Some(3), Some(2)]
        ));
        // A fifth finger.
        assert!(!stands(
            open_d,
            [None, None, None, Some(1), Some(5), Some(2)]
        ));
        // A stopped string with nobody on it.
        assert!(!stands(open_d, [None, None, None, None, Some(3), Some(2)]));
        // One finger on two strings no bar covers.
        assert!(!stands(
            open_d,
            [None, None, None, Some(1), Some(1), Some(2)]
        ));
        // A crossing: the third finger below the second.
        assert!(!stands(
            open_d,
            [None, None, None, Some(1), Some(2), Some(3)]
        ));

        assert!(stands(
            f_major,
            [Some(1), Some(3), Some(4), Some(2), Some(1), Some(1)]
        ));
        // A barred string held by something other than the first finger.
        assert!(!stands(
            f_major,
            [Some(1), Some(3), Some(4), Some(2), Some(2), Some(1)]
        ));
    }

    #[test]
    fn no_voicing_in_the_library_crosses_its_fingers() {
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                for voicing in voicings(Chord::new(root, kind), &DRILL_NECK) {
                    let fingers = voicing.fingers();
                    let mut assigned: Vec<(u8, u8)> = (0..6)
                        .filter_map(|string| Some((fingers[string]?, voicing.strings()[string]?)))
                        .collect();
                    assigned.sort_unstable();

                    for pair in assigned.windows(2) {
                        let &[(finger, fret), (next_finger, next_fret)] = pair else {
                            continue;
                        };

                        assert!(
                            fret <= next_fret,
                            "{root:?} {kind:?} puts finger {next_finger} at {next_fret}, \
                             below finger {finger} at {fret}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn only_an_open_string_changes_what_the_frets_alone_would_say() {
        // Without open strings, the carried fingering either agrees with the sort or is discarded.
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                for voicing in voicings(Chord::new(root, kind), &DRILL_NECK) {
                    if voicing.fingers() == voicing.ordered_fingering() {
                        continue;
                    }

                    assert!(
                        voicing.strings().contains(&Some(0)),
                        "{root:?} {kind:?} at {} differs from the sort with nothing open",
                        voicing.index_fret()
                    );
                }
            }
        }
    }

    #[test]
    fn the_library_offers_the_same_voicings_however_the_fingers_are_numbered() {
        // A property of the placement rather than of the numbering.
        let total: usize = PitchClass::ALL
            .iter()
            .flat_map(|&root| ChordQuality::ALL.iter().map(move |&kind| (root, kind)))
            .map(|(root, kind)| voicings(Chord::new(root, kind), &DRILL_NECK).len())
            .sum();

        assert_eq!(total, 592);
    }

    #[test]
    fn no_two_strings_share_a_finger_unless_they_are_barred() {
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                for voicing in voicings(Chord::new(root, kind), &DRILL_NECK) {
                    let fingers = voicing.fingers();
                    let barre = voicing.barre_fret();

                    for left in 0..6 {
                        for right in (left + 1)..6 {
                            let (Some(a), Some(b)) = (fingers[left], fingers[right]) else {
                                continue;
                            };
                            if a != b {
                                continue;
                            }

                            assert_eq!(
                                (voicing.strings()[left], voicing.strings()[right]),
                                (barre, barre),
                                "{root:?} {kind:?} puts finger {a} on two unbarred strings"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn an_open_string_carries_no_finger() {
        let e_major = only(Chord::new(pc(4), ChordQuality::Major), "E shape", 0);
        let fingers = e_major.fingers();

        // 0 2 2 1 0 0 — the three open strings are stopped by nobody.
        assert_eq!(fingers[0], None);
        assert_eq!(fingers[4], None);
        assert_eq!(fingers[5], None);
        assert!(fingers[1].is_some());
    }
}
