//! The Chord Library: movable shapes, the voicings they produce, and the screen that
//! draws them.
//!
//! A shape is instrument knowledge rather than theory, which is why it lives here and not
//! in `music/` — the same reason `STANDARD_TUNING` and `pitch_class_at` sit in `ui`.
//!
//! The curation stops at [`SHAPES`]. Which strings sound and which degree each carries is
//! a thing guitarists know and no arithmetic produces; everything downstream of that table
//! is arithmetic. Placing a shape adds one number to every offset, and changing a chord's
//! quality moves only the strings whose degrees that quality alters. There is no per-chord
//! entry, no per-root entry, and no per-position entry anywhere below.

use iced::keyboard;

use crate::music::chords::{Chord, ChordQuality, Query};
use crate::music::notes::{PitchClass, Spelling};

use super::chord_diagram::{ChordDiagram, FEATURE, STRIP, StringMark, chord_diagram, window_for};
use super::{
    BODY, FocusTarget, HAIRLINE_INK, INK, MUSIC_FONT, MUTE, Message, NECK_FRETS, Notation,
    SMUFL_CSYM_AUGMENTED, SMUFL_CSYM_DIMINISHED, SMUFL_CSYM_HALF_DIMINISHED,
    SMUFL_CSYM_MAJOR_SEVENTH, SMUFL_SHARP, STANDARD_TUNING, SUCCESS, card_container, focus_ring,
    ghost_button, hairline_rule, intervalic_text, note_label, pitch_class_at,
};

/// How many frets apart a shape's stopped notes may sit.
///
/// Three, not four: a hand covers four *frets*, which is a distance of three between the
/// lowest and the highest. The five CAGED shapes all fit inside it, and the C and G shapes
/// use every bit of it.
const REACH: u8 = 3;

/// How many fingers a hand brings to the neck. A barre counts as one however many strings
/// it covers.
const FINGERS: usize = 4;

/// What one string does in a shape.
///
/// An enum rather than an `Option<u8>` with a comment, because a muted string and a string
/// stopped at the shape's own index fret are different in kind, and the degree only means
/// anything for the second. `Option<u8>` would leave the degree somewhere else, free to
/// disagree.
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

/// A movable shape: which strings sound, what each carries, and which quality's degrees
/// the offsets were measured against.
///
/// `base` rather than a separate list of degree numbers. The two would be one thing to
/// keep in step and the list is derivable, so the shape names the quality it was drawn
/// from and the degrees follow.
#[derive(Debug, Clone, Copy)]
struct Shape {
    name: &'static str,
    strings: [StringRole; 6],
    base: ChordQuality,
    root_string: usize,
}

use StringRole::{Muted, Sounded};

/// One shape per string a root can sit on, per degree-set family, plus the rest of CAGED
/// for the triads.
///
/// Every entry is a chord a guitarist would recognise at the nut — E and A major, Asus2,
/// Esus4, E6 and A6, E7 and A7 — which is the check on the arithmetic below: placed at the
/// nut, a shape has to come out as the open chord it was drawn from.
const SHAPES: &[Shape] = &[
    // Triads, all five CAGED shapes.
    Shape {
        name: "E shape",
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
        name: "A shape",
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
        name: "D shape",
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
        name: "C shape",
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
        name: "G shape",
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
    // the triad shapes above — see the placement rule in `Shape::place`.
    Shape {
        name: "E shape",
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
        name: "A shape",
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
        name: "E shape",
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
        name: "A shape",
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
        name: "E shape",
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
        name: "A shape",
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
    // Sevenths. The largest family — seven of the fifteen qualities share `1 3 5 7`.
    Shape {
        name: "E shape",
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
        name: "A shape",
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
    // Reduced shapes, and the reason they are here: the six-string ones above only stay
    // fingerable near the nut. Altering a quality drags strings apart, and a placement that
    // needs a fifth finger is refused — which left ten of the twelve major sixths with no
    // shape at all. Dropping a string or two costs a doubled note nobody misses and takes
    // the A-shape sixth from two playable placements to twenty-two.
    Shape {
        name: "E shape, four strings",
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
        name: "A shape, three strings",
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
        name: "E shape, four strings",
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
        name: "A shape, four strings",
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
        name: "E shape, four strings",
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
        name: "A shape, four strings",
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

/// One way to play a chord: a fret per string, or nothing sounding there.
///
/// `x 3 2 0 1 0` is what guitarists already write, and this is that. Absolute frets rather
/// than offsets from a window, so the window can be derived from the voicing and the two
/// can never disagree — a diagram drawn against a stale window puts its dots at the wrong
/// frets while still looking perfectly legible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Voicing {
    strings: [Option<u8>; 6],
    /// The fret the shape sits at. Zero is the open position.
    index_fret: u8,
    shape_name: &'static str,
    /// The shape's own fingering, carried into this placement. What [`Voicing::fingers`] uses
    /// where it stands, and what it checks before deciding that.
    fingering: [Option<u8>; 6],
}

impl Voicing {
    pub fn strings(self) -> [Option<u8>; 6] {
        self.strings
    }

    pub fn index_fret(self) -> u8 {
        self.index_fret
    }

    pub fn shape_name(self) -> &'static str {
        self.shape_name
    }

    /// The frets that a finger has to stop. Open strings are sounded by nobody.
    fn stopped(self) -> impl Iterator<Item = u8> {
        self.strings.into_iter().flatten().filter(|&fret| fret > 0)
    }

    /// Which finger stops each string, or `None` where nothing does.
    ///
    /// The shape's own fingering where it stands for this placement, and an ascending-fret
    /// assignment where it does not. The shape has to be asked first because a released finger
    /// is invisible to the frets: Em is `0 2 2 0 0 0`, and nothing in those numbers says the
    /// index was lifted rather than never placed. The frets are the better authority the rest of
    /// the time, which is most of the time. A real solver is still its own project.
    pub fn fingers(self) -> [Option<u8>; 6] {
        self.shape_fingering()
            .unwrap_or_else(|| self.ordered_fingering())
    }

    /// The fingering carried from the shape, or `None` where it does not describe this placement.
    ///
    /// It stops describing it whenever an alteration moves a string without asking the shape:
    /// a fifth finger, a barred string held by something other than the first, one finger on two
    /// unbarred strings, or a crossing — a higher finger below a lower one. Am is the everyday
    /// case. Flattening the third drops the B string under its neighbours, so carrying the shape's
    /// fingering would cross, and the sort takes over.
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

        // Ordered by `(fret, string)`, which is the whole rule: fingers go on in order of
        // fret, and where two strings share a fret the lower one takes the earlier finger.
        //
        // Keying on the fret alone was the first attempt, and it is wrong on the two most
        // common open chords in the instrument. G is `3 2 0 0 0 3` — two strings at the
        // third fret with three strings between them, which one finger cannot reach, and
        // D is `x x 0 2 3 2` for the same reason. Same fret means same finger only when
        // the strings are barred, and that case is already handled above.
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
            // Fingers after the barre, or from the first finger when there is none. Past
            // the fourth there is no finger to name, so the dot goes unlabelled rather
            // than carrying one a hand does not have.
            u8::try_from(rank)
                .ok()?
                .checked_add(if barre.is_some() { 2 } else { 1 })
                .filter(|&finger| usize::from(finger) <= FINGERS)
        })
    }

    /// Whether a hand has enough fingers for this.
    ///
    /// Four, with a barre counting as one however many strings it covers. A voicing needing
    /// a fifth is refused with the placements that run off the neck or past a reach — it is
    /// the same kind of unplayable, and a diagram that drew a dot it could not name a finger
    /// for was the visible symptom of it not being checked.
    fn is_fingerable(self) -> bool {
        self.fingers()
            .iter()
            .zip(self.strings)
            .all(|(finger, fret)| finger.is_some() || !matches!(fret, Some(f) if f > 0))
    }

    /// The fret a barre sits on: the lowest stopped one, when more than one string rests
    /// there *and* the voicing needs more fingers than a hand has.
    ///
    /// The *lowest stopped* fret, not the shape's index fret, which is where this was wrong.
    /// Altering a quality can drop a string below the fret its shape sits at — C♯°7 on the A
    /// shape is `x 4 5 3 5 3`, sitting at the fourth with two strings at the third — and
    /// keying on the index fret found no barre there, leaving five stopped strings for four
    /// fingers. A finger bars the lowest thing it is holding; nothing about the shape's own
    /// index fret enters into it.
    ///
    /// The strings resting on it need not be adjacent. A bar covers the ones between as
    /// well, and a string stopped higher up simply sounds its own note instead — which is
    /// how every barre chord with a shape on top of it works.
    pub fn barre_fret(self) -> Option<u8> {
        let lowest = self.stopped().min()?;
        let resting = self.stopped().filter(|&fret| fret == lowest).count();

        // A hand has four fingers, so a barre is what happens when a voicing needs five.
        // Below that there is no reason to lay one down: open D is `x x 0 2 3 2`, two
        // strings on its lowest fret and barre-able in principle, and nobody plays it that
        // way because three separate fingers are free and easier.
        (resting > 1 && self.stopped().count() > FINGERS).then_some(lowest)
    }
}

impl Shape {
    /// This shape with every string stopped: the fingering of its barre chord.
    ///
    /// The first finger holds the index fret; the offsets above it take the rest in order, the
    /// lower string first where two share one. Derived rather than recorded for the reason `base`
    /// is a quality rather than a list of degrees — a table beside the offsets would be a second
    /// thing to keep in step with them.
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
    /// A refusal is ordinary — most shapes cannot be played at most positions — so it is
    /// an absent voicing rather than an error anyone reports.
    fn place(&self, root: PitchClass, kind: ChordQuality, index_fret: u8) -> Option<Voicing> {
        if !self.carries(kind) {
            return None;
        }

        // The shape only sits here if its root string lands on the chord's root.
        let root_at = self.string_pitch(self.root_string, index_fret, kind)?;
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

            if fret > NECK_FRETS as u8 {
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
            shape_name: self.name,
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
    ///
    /// The rule the whole arithmetic rests on. Altering `3` into `♭3` moves one finger;
    /// a quality with no third at all leaves the string carrying it nowhere to go, and
    /// muting that string would be an editorial decision made by arithmetic rather than
    /// by a guitarist.
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
    fn string_pitch(&self, index: usize, index_fret: u8, kind: ChordQuality) -> Option<PitchClass> {
        let Sounded { offset, degree } = self.strings[index] else {
            return None;
        };

        let fret = i16::from(index_fret) + i16::from(offset) + self.shift(degree, kind)?;
        let fret = u8::try_from(fret).ok()?;

        STANDARD_TUNING
            .get(index)
            .map(|open| open.transpose(fret % 12))
    }
}

/// Every way the library can play `chord`, lowest position first.
///
/// Each shape is tried at every fret it could sit at rather than only at the first: a
/// shape repeats up the neck an octave later, and both placements are worth showing.
pub fn voicings(chord: Chord) -> Vec<Voicing> {
    let mut found: Vec<Voicing> = SHAPES
        .iter()
        .flat_map(|shape| {
            (0..=NECK_FRETS as u8)
                .filter_map(move |index_fret| shape.place(chord.root(), chord.kind(), index_fret))
        })
        .collect();

    found.sort_by_key(|voicing| (voicing.index_fret(), voicing.shape_name()));
    found
}

/// How a voicing's position reads: the nut, or the fret its diagram begins at.
///
/// Derived from the same `window_for` the diagram is drawn against rather than from the
/// shape's own index fret, because the two can disagree — a diminished seventh built on the
/// A shape sits at the third fret and reaches down to the second, so its window opens at
/// the nut. Labelled by the shape it would read `3fr` above a picture of the nut, which is
/// the caption and the picture telling the reader different things. One source, no drift.
pub fn position_label(voicing: Voicing) -> String {
    let window = window_for(&voicing.strings());

    if window.shows_nut() {
        "open".to_string()
    } else {
        format!("{}fr", window.first_fret)
    }
}

/// The library's state: what has been typed, which root it settled on, and what is picked.
///
/// Its own struct rather than five more fields on `App`, so they can stay private — they
/// are read all over the views below and nowhere else. The arrangement both trainers use.
/// The three notations the library can draw, in the order `i` walks them.
///
/// Its own list rather than one shared with the Scale Trainer: a scale has no fingering, so
/// the two screens have different vocabularies and no reason to move together.
pub(super) const NOTATIONS: [Notation; 3] =
    [Notation::Notes, Notation::Intervals, Notation::Fingers];

pub(super) struct ChordLibrary {
    query: String,
    /// Where the caret sits, as a character index into `query`.
    ///
    /// The box is drawn and edited here rather than by iced's `text_input`, because that
    /// widget would receive keys *as well as* the global subscription — both would fire,
    /// and `l` would insert a character and move the focus ring. One owner, one caret.
    caret: usize,
    selected_row: usize,
    selected_voicing: usize,
    search_focused: bool,
    /// Whether a `g` is waiting for its second half.
    ///
    /// `gg` is the one two-key gesture in the app. Anything other than a second `g` clears
    /// it, so a half-typed motion never lies in wait to change what the next key does.
    pending_g: bool,
    /// What the diagrams' marks say, kept here rather than on `App`.
    ///
    /// Held by the screen because only this screen can draw all three, and because sharing
    /// one field with the Scale Trainer meant picking `fingers` here quietly changed what
    /// that screen labelled its markers with.
    notation: Notation,
}

impl ChordLibrary {
    pub(super) fn new() -> Self {
        Self {
            query: String::new(),
            caret: 0,
            selected_row: 0,
            selected_voicing: 0,
            search_focused: false,
            pending_g: false,
            // Fingers by default: the question this screen is opened to answer is "how do
            // I play this", and a fingering answers it in a way a note name does not.
            notation: Notation::Fingers,
        }
    }

    pub(super) fn notation(&self) -> Notation {
        self.notation
    }

    pub(super) fn set_notation(&mut self, index: usize) {
        if let Some(&notation) = NOTATIONS.get(index) {
            self.notation = notation;
        }
    }

    /// Opening the screen: an empty box with the caret in it, and the notation left as the
    /// learner last set it. The query goes because the first keystroke should start a new
    /// search rather than extend the last one.
    pub(super) fn enter(&mut self) {
        self.set_query(String::new());
        self.focus_search();
    }

    pub(super) fn query(&self) -> &str {
        &self.query
    }

    pub(super) fn search_focused(&self) -> bool {
        self.search_focused
    }

    /// The qualities surviving the query, in the order they are listed.
    ///
    /// An empty query is every quality on the current root — browsing is the zero-input
    /// case of searching rather than a mode of its own. A query that parses picks exactly;
    /// one that does not falls back to approximate matching, which is the only place a
    /// score is involved and never competes with an exact hit.
    pub(super) fn rows(&self) -> Vec<Chord> {
        match Query::parse(&self.query) {
            None if self.query.trim().is_empty() => every_chord().collect(),
            None => self.approximate_rows(),
            Some(Query::Root(root)) => every_chord().filter(|chord| chord.root() == root).collect(),
            Some(Query::Quality(kind)) => {
                every_chord().filter(|chord| chord.kind() == kind).collect()
            }
            Some(Query::Chord { root, kind }) => vec![Chord::new(root, kind)],
        }
    }

    /// Rows for a query the grammar could not read: the chords whose symbol contains the
    /// query's characters in order, nearest match first.
    ///
    /// A subsequence rather than a substring, so `cmj7` still reaches `Cmaj7`. The hazard
    /// this would otherwise carry — `cm7` scoring against `Cmaj7` — cannot arise, because
    /// `cm7` parses and never reaches here.
    fn approximate_rows(&self) -> Vec<Chord> {
        let needle = self.query.trim().to_lowercase();
        // The library's own order is the last tiebreak, so two equally good matches come
        // out in the order the list would have shown them anyway.
        let mut scored: Vec<(usize, usize, usize, Chord)> = every_chord()
            .enumerate()
            .filter_map(|(order, chord)| {
                let (start, span) = subsequence(&needle, &chord.to_string().to_lowercase())?;

                Some((start, span, order, chord))
            })
            .collect();

        scored.sort_unstable_by_key(|&(start, span, order, _)| (start, span, order));
        scored.into_iter().map(|(.., chord)| chord).collect()
    }

    pub(super) fn selected_row(&self) -> usize {
        self.selected_row
    }

    /// Scrolls the list so the selected row is on screen.
    ///
    /// Proportional: the offset is the selection's share of the rows, so the highlight
    /// starts at the top of the viewport for the first row and reaches the bottom for the
    /// last, staying inside it all the way between. Exact at both ends, and near enough in
    /// the middle that the group headers' extra height does not push it out — which is what
    /// spares this from having to know the viewport's size or a row's height in pixels.
    pub(super) fn follow_selection(&self) -> iced::Task<Message> {
        let rows = self.rows().len();
        let y = if rows > 1 {
            self.selected_row as f32 / (rows - 1) as f32
        } else {
            0.0
        };

        iced::widget::operation::snap_to(
            list_id(),
            iced::widget::operation::RelativeOffset { x: 0.0, y },
        )
    }

    pub(super) fn selected_chord(&self) -> Option<Chord> {
        self.rows().get(self.selected_row).copied()
    }

    pub(super) fn selected_voicing(&self) -> usize {
        self.selected_voicing
    }

    /// Replaces the query. Nothing is remembered from it: the list is the whole library
    /// filtered by whatever is typed, so an empty box means the whole library rather than
    /// "the last root you named", which was state the screen never showed anybody.
    fn set_query(&mut self, query: String) {
        self.query = query;
        self.selected_row = 0;
        self.selected_voicing = 0;
    }

    pub(super) fn select_row(&mut self, index: usize) {
        if index < self.rows().len() {
            self.selected_row = index;
            self.selected_voicing = 0;
        }
    }

    pub(super) fn select_voicing(&mut self, index: usize) {
        let count = self
            .selected_chord()
            .map_or(0, |chord| voicings(chord).len());

        if index < count {
            self.selected_voicing = index;
        }
    }

    /// Walks the list, stopping at the ends rather than wrapping — the rule the focus grid
    /// already follows for a row of unequal width.
    pub(super) fn move_row(&mut self, delta: isize) {
        let count = self.rows().len();
        if count == 0 {
            return;
        }

        let next = (self.selected_row as isize + delta).clamp(0, count as isize - 1);
        self.select_row(next as usize);
    }

    pub(super) fn move_voicing(&mut self, delta: isize) {
        let count = self
            .selected_chord()
            .map_or(0, |chord| voicings(chord).len());
        if count == 0 {
            return;
        }

        let next = (self.selected_voicing as isize + delta).clamp(0, count as isize - 1);
        self.selected_voicing = next as usize;
    }

    /// The vim motions this screen claims for itself: `gg` to the first chord, `G` to the
    /// last. Reports whether it took the key.
    ///
    /// Only reached with the search box unfocused, since a focused box takes every character
    /// as text — which it must, because `g` is a note name.
    pub(super) fn motion(&mut self, c: char) -> bool {
        let pending = std::mem::replace(&mut self.pending_g, false);

        match c {
            'g' if pending => self.select_row(0),
            'g' => {
                self.pending_g = true;

                return true;
            }
            'G' => {
                let last = self.rows().len().saturating_sub(1);
                self.select_row(last);
            }
            _ => return false,
        }

        true
    }

    pub(super) fn focus_search(&mut self) {
        self.search_focused = true;
        self.caret = self.query.chars().count();
        self.pending_g = false;
    }

    /// Handles a key while the box has focus, reporting what it did.
    ///
    /// Almost everything is text here: a character key types rather than acting, which is
    /// why `j` does not move and `?` does not open the help. The exceptions are the picker
    /// set — the arrows, `Enter` and `Esc` — and they are what make typing and choosing one
    /// gesture rather than two.
    pub(super) fn handle_key(
        &mut self,
        key: &keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> KeyOutcome {
        use keyboard::key::Named;

        match key.as_ref() {
            // Leaves the box without clearing it: the query is how the learner got here,
            // and the gesture is "I found it, now let me look at the shapes".
            keyboard::Key::Named(Named::Escape) => {
                self.search_focused = false;

                return KeyOutcome::Handled;
            }
            // Accepting: the row is already selected, so this is the learner saying they
            // have found it. The ring moves on to the shapes.
            keyboard::Key::Named(Named::Enter) => {
                self.search_focused = false;

                return KeyOutcome::Accepted;
            }
            // The one pair that still navigates while typing, so a learner never has to
            // leave the box to pick from what they have narrowed to.
            keyboard::Key::Named(Named::ArrowUp) => self.move_row(-1),
            keyboard::Key::Named(Named::ArrowDown) => self.move_row(1),
            keyboard::Key::Named(Named::ArrowLeft) => self.caret = self.caret.saturating_sub(1),
            keyboard::Key::Named(Named::ArrowRight) => {
                self.caret = (self.caret + 1).min(self.query.chars().count());
            }
            keyboard::Key::Named(Named::Backspace) => {
                if let Some(at) = self.caret.checked_sub(1) {
                    let mut next: Vec<char> = self.query.chars().collect();
                    next.remove(at);
                    self.set_query(next.into_iter().collect());
                    self.caret = at;
                }
            }
            keyboard::Key::Character(text) if !modifiers.intersects(super::COMMAND_MODIFIERS) => {
                let mut next: Vec<char> = self.query.chars().collect();
                let at = self.caret.min(next.len());

                for (offset, c) in text.chars().enumerate() {
                    next.insert(at + offset, c);
                }

                let typed = text.chars().count();
                self.set_query(next.into_iter().collect());
                self.caret = at + typed;
            }
            // Anything else is not this box's business — a command-modified key, say, so
            // `Ctrl+K` from inside the box still reaches the app.
            _ => return KeyOutcome::Ignored,
        }

        KeyOutcome::Handled
    }
}

/// What a keystroke in the search box did, which is more than "was it mine".
///
/// `Accepted` is its own case because `Enter` and `Esc` mean different things: leaving the
/// box with `Esc` is backing out, and leaves the ring where it was, while `Enter` is
/// finishing — the learner has found their chord and wants to look through its shapes, so
/// the ring goes to the list, whose arrows walk exactly that.
///
/// Returned rather than reaching for `App`'s focus from in here: this struct knows what its
/// own key did, and where the ring sits is the parent's business.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyOutcome {
    /// Not this box's key — the app should go on and translate it.
    Ignored,
    /// Claimed, and the ring stays where it is.
    Handled,
    /// Claimed, and the box is finished: move the ring to the shapes.
    Accepted,
}

/// The chord list's scrollable, named so `follow_selection` can reach it.
fn list_id() -> iced::widget::Id {
    iced::widget::Id::new("chord-library-list")
}

/// Every chord the library holds, by root and then by quality.
///
/// The whole cross product, because the list is a filter over it rather than a window onto
/// one root. Building all 180 costs about 33µs — the rows are cheap; it is the widgets that
/// are not, which is why the view groups them rather than showing a flat wall.
fn every_chord() -> impl Iterator<Item = Chord> {
    PitchClass::ALL.into_iter().flat_map(|root| {
        ChordQuality::ALL
            .iter()
            .map(move |&kind| Chord::new(root, kind))
    })
}

/// Where `needle`'s characters appear in `haystack`, in order, as `(start, span)`.
///
/// `None` when they do not all appear. The pair is what ranks a match: an earlier start
/// beats a later one, and a tighter span beats a scattered one, so `maj7` sits above
/// `mMaj7` for the query `maj7`.
fn subsequence(needle: &str, haystack: &str) -> Option<(usize, usize)> {
    if needle.is_empty() {
        return Some((0, 0));
    }

    let hay: Vec<char> = haystack.chars().collect();
    let mut start = None;
    let mut at = 0;

    for wanted in needle.chars() {
        let found = hay[at..].iter().position(|&c| c == wanted)? + at;

        start.get_or_insert(found);
        at = found + 1;
    }

    let start = start?;

    Some((start, at - start))
}

/// One piece of a written chord symbol: body-font text, or a SMuFL glyph.
///
/// Two fonts, so one `text` widget cannot carry both — the split `note_label` and
/// `intervalic_text` already make for accidentals, applied one level up to the quality.
enum SymbolPart {
    Text(&'static str),
    Glyph(char),
    /// A glyph set as a superscript rather than on the baseline.
    ///
    /// Only the diminished and half-diminished circles: engraving convention raises those
    /// two and leaves every other quality mark sitting on the baseline beside the root.
    /// Raising them all — which is what this did at first — makes `Cm` and `Csus4` look
    /// like exponents.
    Raised(char),
}

/// How a quality is written after its root.
///
/// The four with a glyph of their own use it; the rest are their abbreviation. This is the
/// typographic counterpart of `ChordQuality::suffix`, which is ASCII because `music/`
/// cannot reach these constants.
fn symbol_parts(kind: ChordQuality) -> &'static [SymbolPart] {
    use SymbolPart::{Glyph, Raised, Text};

    match kind {
        // A plain major triad is written as its root alone.
        ChordQuality::Major => &[],
        ChordQuality::Minor => &[Text("m")],
        ChordQuality::Diminished => &[Raised(SMUFL_CSYM_DIMINISHED)],
        ChordQuality::Augmented => &[Glyph(SMUFL_CSYM_AUGMENTED)],
        ChordQuality::Sus2 => &[Text("sus2")],
        ChordQuality::Sus4 => &[Text("sus4")],
        ChordQuality::Major6 => &[Text("6")],
        ChordQuality::Minor6 => &[Text("m6")],
        ChordQuality::Dominant7 => &[Text("7")],
        ChordQuality::Major7 => &[Glyph(SMUFL_CSYM_MAJOR_SEVENTH)],
        ChordQuality::Minor7 => &[Text("m7")],
        ChordQuality::MinorMajor7 => &[Text("m"), Glyph(SMUFL_CSYM_MAJOR_SEVENTH)],
        ChordQuality::HalfDiminished7 => &[Raised(SMUFL_CSYM_HALF_DIMINISHED)],
        // The circle is raised; the seventh after it is not.
        ChordQuality::Diminished7 => &[Raised(SMUFL_CSYM_DIMINISHED), Text("7")],
        // `7♯5`, not `+7`. Both are used, but `+7` is read by some as "with an augmented
        // seventh" — a major seventh — which is a different chord. `7♯5` says the same
        // thing as the degree row beneath it and cannot be read two ways.
        ChordQuality::Augmented7 => &[Text("7"), Glyph(SMUFL_SHARP), Text("5")],
    }
}

/// A chord written the way a musician writes it: the root with a real accidental, the
/// quality with its own glyph where it has one.
fn chord_symbol(
    chord: Chord,
    size: impl Into<iced::Pixels>,
    color: iced::Color,
) -> iced::widget::Row<'static, Message> {
    use iced::widget::{container, text};

    let size = size.into();
    // The quality is set smaller than the root, the way chord symbols are engraved. At the
    // root's own size the diminished circle comes out at cap height and `C°` reads as `CO`.
    let quality = iced::Pixels(size.0 * 0.66);
    // A superscript is smaller again than the baseline marks, not merely lifted off them.
    // At the quality's own size the circle comes out as big as the `m` in `Cm`, which is
    // the size of a letter rather than of a mark about one.
    let raised = iced::Pixels(size.0 * 0.42);

    // The row bottom-aligns its children, which lines up the *boxes* rather than the
    // baselines — and a smaller box carries proportionally less descender under its text,
    // so a mark would hang below the root's baseline like a subscript. Lifting each by its
    // own shortfall in descender space is what puts them all on one line. iced's default
    // line height is 1.3, of which roughly 0.3 sits under the baseline.
    let sits_on_baseline = |mark: iced::Pixels| (size.0 - mark.0) * 0.3;
    // The superscript then rises a percentage of its own height above that shared
    // baseline, which brings its top up near the root's cap — where a chart puts it. Half
    // that left it sitting around the root's middle, reading as a mark that had slipped.
    let lift = sits_on_baseline(raised) + raised.0 * 0.85;

    symbol_parts(chord.kind())
        .iter()
        .fold(note_label(chord.root_note(), size, color), |row, part| {
            let (content, music, mark_size, bottom) = match part {
                SymbolPart::Text(written) => (
                    (*written).to_string(),
                    false,
                    quality,
                    sits_on_baseline(quality),
                ),
                SymbolPart::Glyph(glyph) => {
                    (glyph.to_string(), true, quality, sits_on_baseline(quality))
                }
                SymbolPart::Raised(glyph) => (glyph.to_string(), true, raised, lift),
            };

            let mark = text(content).size(mark_size).color(color);
            let mark = if music { mark.font(MUSIC_FONT) } else { mark };

            row.push(container(mark).padding(iced::Padding {
                bottom,
                ..iced::Padding::ZERO
            }))
        })
        // Bottom-aligned so a smaller mark sits beside the root rather than over it; the
        // paddings above are what turn "same bottom edge" into "same baseline".
        .align_y(iced::Alignment::End)
}

/// What one string's mark says, under the notation in force.
fn mark_label(
    chord: Chord,
    voicing: Voicing,
    notation: Notation,
    string: usize,
    fret: u8,
) -> String {
    let Some(sounding) = pitch_class_at(string, usize::from(fret)) else {
        return String::new();
    };

    // `spell` and `degree` rather than a search through `notes()` here: what a pitch class
    // is called in a chord, and what job it does, are both questions for `music/`. This
    // module decides which of the two answers to draw and nothing else.
    match notation {
        Notation::Notes => chord
            .spell(sounding)
            .map(|note| note.to_string())
            .unwrap_or_default(),
        Notation::Intervals => chord
            .degree(sounding)
            .map(|interval| interval.to_string())
            .unwrap_or_default(),
        // An open string is stopped by nobody, so it carries no finger.
        Notation::Fingers => voicing.fingers()[string]
            .map(|finger| finger.to_string())
            .unwrap_or_default(),
    }
}

fn diagram_for(chord: Chord, voicing: Voicing, notation: Notation) -> ChordDiagram<Message> {
    let frets = voicing.strings();

    ChordDiagram {
        strings: std::array::from_fn(|string| {
            let fret = frets[string];
            let sounding = fret.and_then(|fret| pitch_class_at(string, usize::from(fret)));

            StringMark {
                fret,
                label: fret.map_or_else(String::new, |fret| {
                    mark_label(chord, voicing, notation, string, fret)
                }),
                is_root: sounding == Some(chord.root()),
            }
        }),
        barre: voicing.barre_fret(),
        // Display-only in this change. The widget hit-tests regardless, so the editing
        // pass has a surface to land on — see the module comment on `chord_diagram`.
        on_press: None,
    }
}

/// The rule between one root's chords and the next.
///
/// Both names on the five black keys, because the rows below genuinely use both: pitch
/// class 1 spells `D♭` under a major triad and `C♯` under a minor one, since `D♭ F A♭`
/// costs two flats where `C♯ E♯ G♯` costs three sharps. A header reading `C♯` alone would
/// be wrong about a third of its own group — and showing the pair says the thing the
/// spelling rule exists to teach, which is that the name follows the chord.
fn root_header(root: PitchClass, after_a_group: bool) -> iced::Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, column, container, row, text};

    let sharp = Spelling::Sharps.spell(root);
    let flat = Spelling::Flats.spell(root);

    let name = if sharp == flat {
        row![note_label(sharp, 13, MUTE)]
    } else {
        row![
            note_label(sharp, 13, MUTE),
            text("/").size(13).color(HAIRLINE_INK),
            note_label(flat, 13, MUTE),
        ]
        .spacing(5)
    };

    container(
        column![
            name,
            // A hairline rather than a bordered container: a `Border` would draw on all
            // four sides, and only the underline is wanted.
            container(Space::new().height(Length::Fixed(1.0)))
                .width(Length::Fill)
                .style(hairline_rule),
        ]
        .spacing(4),
    )
    .padding(iced::Padding {
        top: if after_a_group { 14.0 } else { 4.0 },
        bottom: 2.0,
        left: 12.0,
        right: 12.0,
    })
    .into()
}

pub(super) fn ui_chord_library(
    library: &ChordLibrary,
    focused: FocusTarget,
) -> iced::Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, button, column, container, row, scrollable, text};

    let typed = library.query();
    let box_text = if typed.is_empty() {
        text("Search a chord…").size(16).color(MUTE)
    } else {
        text(typed.to_string()).size(16).color(INK)
    };

    // The caret is a glyph rather than a blinking cursor: nothing here animates, and a
    // learner needs to see that the box has the keyboard, not where a redraw is due.
    let caret = if library.search_focused() {
        text("▏").size(16).color(SUCCESS)
    } else {
        text("/").size(16).color(MUTE)
    };

    let search_box = focus_ring(
        button(
            row![
                text("⌕").size(16).color(MUTE),
                box_text,
                Space::new().width(Length::Fill),
                caret
            ]
            .spacing(8),
        )
        .padding([10, 14])
        .width(Length::Fill)
        .style(ghost_button)
        .on_press(Message::OpenSearch),
        focused == FocusTarget::SearchBox,
    );

    let rows = library.rows();
    let list: iced::Element<'static, Message> = if rows.is_empty() {
        container(
            text(format!("No chord matches “{typed}”"))
                .size(15)
                .color(MUTE),
        )
        .padding(12)
        .into()
    } else {
        scrollable(
            rows.iter()
                .enumerate()
                .fold(column![].spacing(2), |list, (index, &chord)| {
                    // A header wherever the root changes. Derived here rather than stored
                    // in the rows, so the selection stays a plain index into the chords and
                    // the arrows never have to step over anything unselectable.
                    let opens_a_group = index == 0 || rows[index - 1].root() != chord.root();
                    let list = if opens_a_group {
                        list.push(root_header(chord.root(), index > 0))
                    } else {
                        list
                    };
                    let picked = index == library.selected_row();

                    list.push(
                        button(chord_symbol(chord, 17, if picked { INK } else { BODY }))
                            .padding([7, 12])
                            .width(Length::Fill)
                            .style(if picked {
                                super::selected_row_button
                            } else {
                                super::row_button
                            })
                            .on_press(Message::SelectChordRow(index)),
                    )
                }),
        )
        .id(list_id())
        .height(Length::Fill)
        .into()
    };

    let list_card = focus_ring(
        container(list)
            .width(Length::Fixed(232.0))
            .height(Length::Fill)
            .padding(10)
            .style(card_container),
        focused == FocusTarget::ChordList,
    );

    let detail = match library.selected_chord() {
        None => container(Space::new()).into(),
        Some(chord) => detail_pane(
            chord,
            library.selected_voicing(),
            library.notation(),
            focused,
        ),
    };

    container(
        row![
            column![search_box, list_card]
                .spacing(12)
                .width(Length::Fixed(240.0)),
            detail,
        ]
        .spacing(20),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(iced::Padding {
        top: 8.0,
        right: 48.0,
        bottom: 40.0,
        left: 48.0,
    })
    .into()
}

fn detail_pane(
    chord: Chord,
    selected: usize,
    notation: Notation,
    focused: FocusTarget,
) -> iced::Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, button, column, container, row, scrollable, text};

    // Spelled with the same glyphs the symbol above them uses, the way the scale trainer's
    // summary card spells its root and its formula. `note.to_string()` is the ASCII the
    // canvas is limited to, and this is not a canvas.
    let notes = chord
        .notes()
        .into_iter()
        .fold(row![].spacing(14), |line, note| {
            line.push(note_label(note, 18, BODY))
        });
    let degrees = intervalic_text(chord.degrees());

    // Three buttons rather than one that cycles. A control whose label changes on every
    // press does not read as a control with options — it reads as a status line, and
    // nobody presses a status line to see what else it could say. Laid out as the scale
    // trainer lays out its roots: pills, with the chosen one filled.
    let notation_buttons =
        NOTATIONS
            .iter()
            .enumerate()
            .fold(row![].spacing(6), |buttons, (index, &choice)| {
                let chosen = choice == notation;

                buttons.push(focus_ring(
                    button(
                        text(match choice {
                            Notation::Notes => "notes",
                            Notation::Intervals => "degrees",
                            Notation::Fingers => "fingers",
                        })
                        .size(14),
                    )
                    .padding([6, 14])
                    .style(if chosen {
                        super::selected_root_button
                    } else {
                        ghost_button
                    })
                    .on_press(Message::SetNotation(index)),
                    focused == FocusTarget::NotationChoice(index),
                ))
            });

    let header = container(
        column![
            row![
                chord_symbol(chord, 44, INK),
                Space::new().width(Length::Fill),
                notation_buttons,
            ]
            .align_y(iced::Alignment::Center),
            row![notes, Space::new().width(Length::Fixed(32.0)), degrees]
                .align_y(iced::Alignment::Center),
        ]
        .spacing(10),
    )
    .width(Length::Fill)
    .padding(24)
    .style(card_container);

    let shapes = voicings(chord);
    let strip: iced::Element<'static, Message> = if shapes.is_empty() {
        // Specified rather than hidden: the chord keeps its notes and degrees, and the
        // screen says why there is nothing to look at.
        text("No shape available for this chord")
            .size(16)
            .color(MUTE)
            .into()
    } else {
        // Wrapped rather than scrolled sideways: a shape that does not fit drops to the
        // next line, so the whole set is one block to read instead of a strip to drag.
        scrollable(
            shapes
                .iter()
                .enumerate()
                .fold(row![].spacing(12), |strip, (index, &voicing)| {
                    strip.push(voicing_card(
                        chord,
                        voicing,
                        index,
                        index == selected,
                        notation,
                    ))
                })
                .wrap()
                .vertical_spacing(12),
        )
        .into()
    };

    container(column![header, container(strip).padding(8)].spacing(16))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn voicing_card(
    chord: Chord,
    voicing: Voicing,
    index: usize,
    picked: bool,
    notation: Notation,
) -> iced::Element<'static, Message> {
    use iced::widget::{button, column, container, text};

    let caption = format!("{}  ·  {}", position_label(voicing), voicing.shape_name());
    // The picked shape is drawn half again as large. Its caption grows with it, so the two
    // stay one object rather than a big picture with a small note under it.
    let (size, caption_size) = if picked { (FEATURE, 16) } else { (STRIP, 13) };

    container(
        button(
            column![
                chord_diagram(diagram_for(chord, voicing, notation), size),
                text(caption)
                    .size(caption_size)
                    .color(if picked { INK } else { MUTE }),
            ]
            .spacing(8)
            .align_x(iced::Alignment::Center),
        )
        .padding(8)
        .style(if picked {
            super::selected_row_button
        } else {
            super::row_button
        })
        // A press anywhere inside picks the voicing: selection is about the diagram as a
        // whole, never about which position was pressed.
        .on_press(Message::SelectVoicing(index)),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    use keyboard::key::Named;

    fn pc(semitone: u8) -> PitchClass {
        PitchClass::new(semitone)
    }

    fn arrow(named: Named) -> keyboard::Key {
        keyboard::Key::Named(named)
    }

    fn frets(voicing: Voicing) -> Vec<Option<u8>> {
        voicing.strings().to_vec()
    }

    fn only(chord: Chord, name: &str, index_fret: u8) -> Voicing {
        voicings(chord)
            .into_iter()
            .find(|v| v.shape_name() == name && v.index_fret() == index_fret)
            .unwrap_or_else(|| panic!("{name} at {index_fret} is not offered for {chord}"))
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
                "{} on {:?} covers the wrong degrees",
                shape.name, shape.base
            );
        }
    }

    #[test]
    fn every_shape_puts_its_root_on_its_root_string() {
        for shape in SHAPES {
            assert!(
                matches!(shape.strings[shape.root_string], Sounded { degree: 1, .. }),
                "{} on {:?} has no root on its root string",
                shape.name,
                shape.base
            );
        }
    }

    #[test]
    fn every_shape_has_a_string_at_its_index_fret() {
        // What makes the offsets mean "from the index fret": the lowest stopped string of
        // a shape sits at zero, so placing it at fret n puts a barre there.
        for shape in SHAPES {
            let lowest = shape
                .strings
                .iter()
                .filter_map(|role| match role {
                    Sounded { offset, .. } => Some(*offset),
                    Muted => None,
                })
                .min();

            assert_eq!(lowest, Some(0), "{} on {:?}", shape.name, shape.base);
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
                "{} on {:?} spans {span}",
                shape.name,
                shape.base
            );
        }
    }

    #[test]
    fn every_family_has_a_shape() {
        // A quality with no shape family shows no diagram, which is specified but should
        // never be true of the fifteen that ship.
        for &kind in ChordQuality::ALL {
            assert!(
                SHAPES.iter().any(|shape| shape.carries(kind)),
                "{kind:?} has no shape family"
            );
        }
    }

    #[test]
    fn the_open_chords_come_out_of_the_arithmetic() {
        // The check on the whole table: placed at the nut, a shape has to reproduce the
        // open chord it was drawn from. These are the shapes as a guitarist writes them.
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
        // The property that catches an arithmetic slip anywhere in placement or
        // alteration: every sounded string has to be a chord tone, for every chord.
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                let chord = Chord::new(root, kind);
                let tones: Vec<PitchClass> = chord
                    .degrees()
                    .iter()
                    .map(|interval| root.transpose(interval.semitones()))
                    .collect();

                for voicing in voicings(chord) {
                    for (string, fret) in voicing.strings().iter().enumerate() {
                        let Some(fret) = *fret else { continue };
                        let sounded = STANDARD_TUNING[string].transpose(fret % 12);

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
                for voicing in voicings(Chord::new(root, kind)) {
                    let stopped: Vec<u8> = voicing.stopped().collect();

                    for fret in voicing.strings().iter().flatten() {
                        assert!(
                            *fret <= NECK_FRETS as u8,
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
                let found = voicings(Chord::new(root, kind));

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
        // The label reads the window, not the shape's index fret, and the two do come
        // apart: C diminished seventh sits on the A shape at the third fret but reaches
        // down to the second, so the diagram opens at the nut. Captioned by the shape it
        // would read `3fr` over a picture of the nut.
        let c_dim7 = Chord::new(pc(0), ChordQuality::Diminished7);

        for voicing in voicings(c_dim7) {
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
                for voicing in voicings(Chord::new(root, kind)) {
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
        // The C shape holds its third on an open string, so flattening it would need a
        // fret below the nut. C minor is therefore not a C shape at the nut — but it is
        // reachable elsewhere.
        let c_minor = Chord::new(pc(0), ChordQuality::Minor);

        assert!(
            !voicings(c_minor)
                .iter()
                .any(|v| v.shape_name() == "C shape" && v.index_fret() == 0)
        );
        assert!(!voicings(c_minor).is_empty());
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
        // The bug this exists for: `C♯°7` on the A shape is `x 4 5 3 5 3`, a shape sitting
        // at the fourth fret with two strings dropped to the third by the alteration.
        // `barre_fret` keyed on the shape's index fret, found no barre at 4, and left five
        // stopped strings for four fingers — the fifth drew a dot with no number in it.
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                for voicing in voicings(Chord::new(root, kind)) {
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
        // Not on the shape's index fret, which is what it used to read.
        let c_sharp_dim7 = Chord::new(pc(1), ChordQuality::Diminished7);
        let voicing = voicings(c_sharp_dim7)
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
        // A barre is what happens when a chord needs a fifth finger. Open D is `x x 0 2 3 2`
        // — two strings on its lowest fret, barre-able in principle, and fingered with three
        // separate fingers by everyone who plays it.
        let open_d = only(Chord::new(pc(2), ChordQuality::Major), "D shape", 0);

        assert_eq!(open_d.barre_fret(), None);
    }

    #[test]
    fn a_barre_takes_the_first_finger_and_nothing_exceeds_the_fourth() {
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                for voicing in voicings(Chord::new(root, kind)) {
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
        // The check on the rule, and the reason it orders by `(fret, string)` rather than
        // by fret: G and D each put two strings on one fret with others between them, and
        // keying on the fret alone gave both of those strings the same finger.
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
            // 0 2 2 0 0 0 — the third falls open here, and the fingers that survive keep the
            // numbers they had in E major rather than sliding down to the first and second.
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
        // The five CAGED triads as their barre chords are played. Every placement of a shape is
        // this, less the strings that sound open, so these are the numbers the open chords above
        // are derived from rather than a second record of them.
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
                .find(|shape| shape.name == name && shape.base == ChordQuality::Major)
                .unwrap_or_else(|| panic!("{name} is not in the table"));

            assert_eq!(shape.movable_fingering(), expected, "{name}");
        }
    }

    #[test]
    fn no_shape_skips_a_finger_or_doubles_one_above_its_index_fret() {
        // The check on the derivation across every entry, not just the five pinned above. The
        // first finger holds the index fret however many strings rest there; each finger above it
        // holds one string, and they run without a gap.
        for shape in SHAPES {
            let fingering = shape.movable_fingering();

            let mut named: Vec<u8> = fingering.iter().flatten().copied().collect();
            named.sort_unstable();
            named.dedup();

            let run: Vec<u8> = (1..=u8::try_from(named.len()).unwrap_or(u8::MAX)).collect();
            assert_eq!(named, run, "{} skips a finger", shape.name);

            for finger in named.into_iter().filter(|&finger| finger > 1) {
                let held = fingering
                    .iter()
                    .filter(|&&held| held == Some(finger))
                    .count();

                assert_eq!(
                    held, 1,
                    "{} puts finger {finger} on two strings",
                    shape.name
                );
            }
        }
    }

    #[test]
    fn a_barred_shape_is_fingered_the_way_the_shape_records_it() {
        // F major is the E shape with the nut's open strings under the bar. Nothing is released,
        // so nothing is lowered, and the fingering is the shape's own.
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
        // The E shape carries a fifth on its index fret, so sharpening it stops a string the
        // first finger was holding. The index has work, nothing comes down, and no dot is
        // numbered zero — which is what lowering on `index_fret == 0` instead would have done.
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
                shape_name: "by hand",
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
                for voicing in voicings(Chord::new(root, kind)) {
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
        // Where a placement has no open string, release and lowering do nothing, so the carried
        // fingering either agrees with the sort or contradicts the frets and is thrown away for
        // it. A released finger is the one thing the frets cannot see, and this is the bound on
        // where the two can differ.
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                for voicing in voicings(Chord::new(root, kind)) {
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
        // A placement is refused for needing a fifth finger when more than four strings need
        // stopping off the barre, which is a property of the placement rather than of the
        // numbering. 592 before this module learned to carry a shape's fingering, and after.
        let total: usize = PitchClass::ALL
            .iter()
            .flat_map(|&root| ChordQuality::ALL.iter().map(move |&kind| (root, kind)))
            .map(|(root, kind)| voicings(Chord::new(root, kind)).len())
            .sum();

        assert_eq!(total, 592);
    }

    #[test]
    fn no_two_strings_share_a_finger_unless_they_are_barred() {
        // The property the ordering guarantees, across every chord in the roster.
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                for voicing in voicings(Chord::new(root, kind)) {
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

    fn typed(text: &str) -> ChordLibrary {
        let mut library = ChordLibrary::new();
        library.set_query(text.to_string());
        library
    }

    #[test]
    fn an_empty_query_lists_the_whole_library() {
        let library = ChordLibrary::new();

        assert_eq!(library.rows().len(), 12 * 15);
    }

    #[test]
    fn the_rows_run_by_root_then_by_the_curated_quality_order() {
        // Not alphabetical: within a root, triads first and extensions last.
        let rows = ChordLibrary::new().rows();

        assert_eq!(rows.first().map(|c| c.to_string()), Some("C".into()));
        assert_eq!(rows[1].to_string(), "Cm");
        assert_eq!(rows[15].root(), pc(1), "the second group is the next root");
        assert_eq!(
            rows.last().map(|c| c.kind()),
            Some(ChordQuality::Augmented7)
        );
    }

    #[test]
    fn a_root_alone_narrows_to_that_root() {
        let library = typed("f#");
        let rows = library.rows();

        assert_eq!(rows.len(), 15);
        assert!(rows.iter().all(|chord| chord.root() == pc(6)));
    }

    #[test]
    fn a_quality_alone_narrows_to_that_quality_on_every_root() {
        // The list is a filter over the whole library now, so a bare quality is twelve
        // chords rather than one — which is the more useful answer and the more obvious
        // one, since nothing on screen was ever naming a "current root".
        let library = typed("m7b5");
        let rows = library.rows();

        assert_eq!(rows.len(), 12);
        assert!(
            rows.iter()
                .all(|chord| chord.kind() == ChordQuality::HalfDiminished7)
        );
    }

    #[test]
    fn a_root_and_a_quality_narrow_to_one_chord() {
        let library = typed("bbmaj7");

        assert_eq!(library.rows().len(), 1);
        assert_eq!(
            library.selected_chord().map(|c| c.to_string()),
            Some("Bbmaj7".into())
        );
    }

    #[test]
    fn clearing_the_query_restores_the_whole_library() {
        let mut library = typed("f#maj7");
        library.set_query(String::new());

        assert_eq!(library.rows().len(), 12 * 15);
    }

    #[test]
    fn an_exact_parse_is_not_ranked_against_an_approximate_one() {
        // `cm7` is a subsequence of `Cmaj7`, so a matcher scoring by adjacency would offer
        // the major seventh here. Parsing settles it outright and nothing else is listed.
        let library = typed("cm7");

        assert_eq!(library.rows().len(), 1);
        assert_eq!(
            library.selected_chord().map(|c| c.to_string()),
            Some("Cm7".into())
        );
    }

    #[test]
    fn a_query_that_does_not_parse_falls_back_to_approximate_matching() {
        // A typo: `cmj7` reads as no chord, and the subsequence finds the one meant.
        let library = typed("cmj7");

        assert_eq!(
            library.rows().first().map(|c| c.to_string()),
            Some("Cmaj7".into())
        );
    }

    #[test]
    fn a_query_matching_nothing_leaves_the_list_empty() {
        let library = typed("zzz");

        assert!(library.rows().is_empty());
        assert_eq!(library.selected_chord(), None);
    }

    #[test]
    fn a_row_is_named_under_the_quality_it_carries() {
        // The scale trainer's rule, carried across: a pitch class is not spelled one fixed
        // way down a group, it is spelled as each chord is written. This is also why a
        // group header has to show both names — see `root_header`.
        let names: Vec<String> = typed("a#")
            .rows()
            .iter()
            .map(|chord| chord.to_string())
            .collect();

        assert!(names.iter().any(|name| name.starts_with("Bb")));
        assert!(names.iter().any(|name| name.starts_with("A#")));
    }

    #[test]
    fn a_black_key_group_needs_both_of_its_names() {
        // What `root_header` renders the pair for. On the five black keys the qualities
        // genuinely disagree, so one name would be wrong about part of its own group.
        for &root in &PitchClass::ALL {
            let spellings: Vec<String> = ChordQuality::ALL
                .iter()
                .map(|&kind| Chord::new(root, kind).root_note().to_string())
                .collect();
            let mut distinct = spellings.clone();
            distinct.sort();
            distinct.dedup();

            let has_two_names = Spelling::Sharps.spell(root) != Spelling::Flats.spell(root);

            assert_eq!(
                distinct.len() > 1,
                has_two_names,
                "{root:?} spells {distinct:?}"
            );
        }
    }

    #[test]
    fn while_the_box_has_focus_a_motion_key_types() {
        let mut library = ChordLibrary::new();
        library.focus_search();

        for c in ["j", "k", "h", "l", "?"] {
            library.handle_key(
                &keyboard::Key::Character(c.into()),
                keyboard::Modifiers::empty(),
            );
        }

        assert_eq!(library.query(), "jkhl?");
        assert!(library.search_focused());
    }

    #[test]
    fn the_arrows_pick_a_row_without_leaving_the_box() {
        let mut library = ChordLibrary::new();
        library.focus_search();

        library.handle_key(&arrow(Named::ArrowDown), keyboard::Modifiers::empty());
        library.handle_key(&arrow(Named::ArrowDown), keyboard::Modifiers::empty());

        assert_eq!(library.selected_row(), 2);
        assert!(library.search_focused(), "the box lost focus");
        assert_eq!(library.query(), "", "an arrow typed");
    }

    #[test]
    fn the_selection_stops_at_the_ends() {
        let mut library = ChordLibrary::new();

        library.move_row(-1);
        assert_eq!(library.selected_row(), 0);

        library.move_row(10_000);
        assert_eq!(library.selected_row(), library.rows().len() - 1);
    }

    #[test]
    fn escape_unfocuses_the_box_and_keeps_the_query() {
        let mut library = typed("cmaj7");
        library.focus_search();
        library.move_row(0);

        let outcome = library.handle_key(&arrow(Named::Escape), keyboard::Modifiers::empty());

        assert_eq!(
            outcome,
            KeyOutcome::Handled,
            "escape fell through to the app"
        );
        assert!(!library.search_focused());
        assert_eq!(library.query(), "cmaj7", "the query was cleared");
        assert_eq!(library.rows().len(), 1);
    }

    #[test]
    fn enter_finishes_the_search_and_hands_on_the_keyboard() {
        // The difference from escape: escape backs out, enter is done. Only enter says the
        // ring should move, and `App` is what moves it.
        let mut library = typed("cmaj7");
        library.focus_search();

        let outcome = library.handle_key(&arrow(Named::Enter), keyboard::Modifiers::empty());

        assert_eq!(outcome, KeyOutcome::Accepted);
        assert!(!library.search_focused());
        assert_eq!(library.query(), "cmaj7", "enter cleared the query");
    }

    #[test]
    fn backspace_deletes_at_the_caret() {
        let mut library = ChordLibrary::new();
        library.focus_search();

        for c in ["c", "m", "a", "j", "7"] {
            library.handle_key(
                &keyboard::Key::Character(c.into()),
                keyboard::Modifiers::empty(),
            );
        }
        library.handle_key(&arrow(Named::Backspace), keyboard::Modifiers::empty());

        assert_eq!(library.query(), "cmaj");
    }

    #[test]
    fn a_command_modified_key_falls_through_to_the_app() {
        // `Ctrl+K` has to keep working from inside the box, so the box must decline it.
        let mut library = ChordLibrary::new();
        library.focus_search();

        let outcome = library.handle_key(
            &keyboard::Key::Character("k".into()),
            keyboard::Modifiers::CTRL,
        );

        assert_eq!(outcome, KeyOutcome::Ignored);
        assert_eq!(library.query(), "");
    }

    #[test]
    fn the_voicing_selection_walks_and_stops() {
        let mut library = typed("e");
        library.select_row(0);

        let count = voicings(library.selected_chord().expect("E major is listed")).len();
        assert!(count > 1, "E major should offer more than one shape");

        library.move_voicing(1);
        assert_eq!(library.selected_voicing(), 1);

        library.move_voicing(-100);
        assert_eq!(library.selected_voicing(), 0);

        library.move_voicing(100);
        assert_eq!(library.selected_voicing(), count - 1);
    }

    #[test]
    fn picking_a_new_chord_returns_to_its_first_shape() {
        let mut library = typed("e");
        library.move_voicing(1);
        library.select_row(1);

        assert_eq!(library.selected_voicing(), 0);
    }

    #[test]
    fn every_chord_in_the_roster_can_be_played() {
        // Not required by the spec — a chord with no shape is specified and handled — but
        // true of the fifteen that ship, and worth knowing if it stops being true.
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                let chord = Chord::new(root, kind);

                assert!(!voicings(chord).is_empty(), "{chord} cannot be played");
            }
        }
    }
}
