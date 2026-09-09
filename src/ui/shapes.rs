//! The movable shapes, the voicings they place, and the arithmetic between them.
//!
//! A shape is instrument knowledge rather than theory, which is why it lives here and not
//! in `music/` — the same reason `STANDARD_TUNING` and `pitch_class_at` sit in `ui`. It is
//! not a screen's either: a shape belongs to the instrument, not to whichever screen asks
//! about it, and a screen importing another screen is what keeping each screen's state and
//! views in one module exists to prevent.
//!
//! The curation stops at [`SHAPES`]. Which strings sound and which degree each carries is
//! a thing guitarists know and no arithmetic produces; everything downstream of that table
//! is arithmetic. Placing a shape adds one number to every offset, and changing a chord's
//! quality moves only the strings whose degrees that quality alters. There is no per-chord
//! entry, no per-root entry, and no per-position entry anywhere below.

use crate::music::chords::{Chord, ChordQuality};
use crate::music::notes::PitchClass;

use super::chord_diagram::window_for;
use super::{NECK_FRETS, STANDARD_TUNING};

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

/// Which of the five open major shapes a shape derives from — the letters CAGED is named for.
///
/// An enum rather than the letter inside a name string, because the reduced entries are also
/// CAGED shapes: `"E shape"` and `"E shape, four strings"` are the same letter to a guitarist
/// and different values to `==`. `match`ing it is exhaustive on purpose — a sixth shape would
/// make the compiler name every place that has to decide about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CagedShape {
    C,
    A,
    G,
    E,
    D,
}

impl CagedShape {
    /// How a shape of this letter is written, reduced or not. The one place the printed name
    /// is built, so `Shape` and `Voicing` cannot disagree about it.
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
///
/// `base` rather than a separate list of degree numbers. The two would be one thing to
/// keep in step and the list is derivable, so the shape names the quality it was drawn
/// from and the degrees follow.
#[derive(Debug, Clone, Copy)]
struct Shape {
    caged: CagedShape,
    /// How this entry is reduced from the full shape, or `None` for the full one. Display only:
    /// the identity is `caged`, and `None` is what makes a shape one of the five.
    reduction: Option<&'static str>,
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
    // the triad shapes above — see the placement rule in `Shape::place`.
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
    // Sevenths. The largest family — seven of the fifteen qualities share `1 3 5 7`.
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
    // Reduced shapes, and the reason they are here: the six-string ones above only stay
    // fingerable near the nut. Altering a quality drags strings apart, and a placement that
    // needs a fifth finger is refused — which left ten of the twelve major sixths with no
    // shape at all. Dropping a string or two costs a doubled note nobody misses and takes
    // the A-shape sixth from two playable placements to twenty-two.
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

/// One way to play a chord: a fret per string, or nothing sounding there.
///
/// `x 3 2 0 1 0` is what guitarists already write, and this is that. Absolute frets rather
/// than offsets from a window, so the window can be derived from the voicing and the two
/// can never disagree — a diagram drawn against a stale window puts its dots at the wrong
/// frets while still looking perfectly legible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Voicing {
    strings: [Option<u8>; 6],
    /// The fret the shape sits at. Zero is the open position.
    index_fret: u8,
    caged: CagedShape,
    reduction: Option<&'static str>,
    /// The shape's own fingering, carried into this placement. What [`Voicing::fingers`] uses
    /// where it stands, and what it checks before deciding that.
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
    pub(super) fn fingers(self) -> [Option<u8>; 6] {
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
    pub(super) fn barre_fret(self) -> Option<u8> {
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
pub(super) fn voicings(chord: Chord) -> Vec<Voicing> {
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

    fn pc(semitone: u8) -> PitchClass {
        PitchClass::new(semitone)
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

    /// The names the entries carried when they were stored strings rather than a letter and a
    /// reduction. Listed rather than derived, so this is a check on the derivation and not a
    /// restatement of it.
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
                .find(|shape| {
                    shape.caged.name(shape.reduction) == name && shape.base == ChordQuality::Major
                })
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
                // Unread by `shape_fingering`; the frets and the fingering are the whole input.
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
}
