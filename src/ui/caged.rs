//! The CAGED window: one root's five shapes tiled along one neck, and the screen that draws
//! them.
//!
//! State and views in one module so the state's fields stay private — the arrangement the
//! three other stateful screens use, and for the same reason.
//!
//! The screen holds no shape data. What it shows is [`super::shapes::voicings`] filtered and
//! ordered, which is what keeps the cycle a consequence of the placement arithmetic rather
//! than a second thing to keep in step with it.

use crate::music::chords::{Chord, ChordQuality};
use crate::music::notes::PitchClass;

use iced::widget::text;
use iced::{Color, Element};

use super::fretboard::{Fretboard, MarkerStyle, NoteMarker, fretboard};
use super::shapes::{Voicing, voicings};
use super::{
    BODY, CAGED_NECK, CANVAS, FocusTarget, INK, LINK, MUSIC_FONT, MUTE, Message, Notation,
    ROOT_MARKER, ROOT_SELECTOR_CARD_WIDTH, SELECTOR_CARD_HEIGHT, SMUFL_FLAT, SUMMARY_CARD_HEIGHT,
    SUMMARY_NAME_SIZE, card_container, control_button, control_glyph, control_label,
    control_shuffle, focus_ring, ghost_button, intervalic_text, marker_label, note_label,
    root_row_spans, root_square, selected_root_button,
};

/// The quality the window shows. Major because it is the only one whose shapes are a cycle:
/// minor yields three, and the diminished and augmented triads one or two. The screen is
/// written against a `Chord` so admitting the others is a selector rather than a rewrite.
const QUALITY: ChordQuality = ChordQuality::Major;

/// How wide the two cards are. Twice the root selector's own width, which is what the row of
/// shape pills needs and what keeps the twelve roots from floating in the middle of a card
/// stretched to the window.
const DETAILS_WIDTH: f32 = ROOT_SELECTOR_CARD_WIDTH * 2.0 + 16.0;

/// Which root the window is showing, and which of its shapes is picked.
///
/// The cycle itself is not here. It is derived from `root` wherever it is wanted, the way
/// `ChordLibrary::rows` derives its list every draw — a cached `Vec<Voicing>` would be a
/// field free to disagree with the root beside it.
pub(super) struct Caged {
    root: PitchClass,
    /// An index into the derived order, not a `CagedShape`: the order's length depends on the
    /// quality, so an index into what was found cannot name a shape that is not there.
    selected: usize,
}

/// The shapes for `root`, lowest first — one placement per letter, being the lowest fret each
/// sits at.
///
/// The lowest rather than every placement, because a shape recurs an octave up and CAGED means
/// five windows rather than eleven. Ordering by fret is the whole derivation: nothing here says
/// C-A-G-E-D, and the letters come out as a rotation of it anyway.
fn cycle(root: PitchClass) -> Vec<Voicing> {
    let mut found: Vec<Voicing> = Vec::new();

    // A `for` rather than a fold: the membership test reads as what it is, and the accumulator
    // a fold would thread through would say nothing the loop does not.
    for voicing in voicings(Chord::new(root, QUALITY), &CAGED_NECK) {
        if voicing.is_caged() && !found.iter().any(|seen| seen.caged() == voicing.caged()) {
            found.push(voicing);
        }
    }

    found
}

impl Caged {
    pub(super) fn new() -> Self {
        Self {
            root: PitchClass::new(0),
            selected: 0,
        }
    }

    pub(super) fn root(&self) -> PitchClass {
        self.root
    }

    pub(super) fn selected(&self) -> usize {
        self.selected
    }

    pub(super) fn shapes(&self) -> Vec<Voicing> {
        cycle(self.root)
    }

    pub(super) fn selected_shape(&self) -> Option<Voicing> {
        self.shapes().get(self.selected).copied()
    }

    /// Picks a root, keeping the selection's place in the new cycle where there is one.
    pub(super) fn select_root(&mut self, root: PitchClass) {
        self.root = root;
        self.clamp();
    }

    pub(super) fn select_shape(&mut self, index: usize) {
        if index < self.shapes().len() {
            self.selected = index;
        }
    }

    /// Walks the cycle, stopping at the ends rather than wrapping — the rule the focus grid
    /// and the library's list both follow.
    pub(super) fn move_selection(&mut self, delta: isize) {
        let count = self.shapes().len();
        if count == 0 {
            return;
        }

        let next = (self.selected as isize + delta).clamp(0, count as isize - 1);
        self.selected = next as usize;
    }

    /// Called when the screen opens. The root and the selection survive leaving it, so this
    /// only re-establishes the invariant `clamp` keeps.
    pub(super) fn enter(&mut self) {
        self.clamp();
    }

    /// Keeps `selected` pointing at a shape that exists. `saturating_sub` covers the empty
    /// cycle, which no major triad produces but which a later quality could.
    fn clamp(&mut self) {
        self.selected = self.selected.min(self.shapes().len().saturating_sub(1));
    }
}

/// What claims a position on the neck.
///
/// Three states rather than two booleans threaded to the call site: a position the selected
/// shape shares with a neighbour is a different thing to draw from one either owns alone, and
/// that difference is the overlap the screen exists to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Claim {
    Selected,
    Shared,
    Unselected,
}

impl Claim {
    /// Colour carries the claim; `MarkerStyle` carries the root. Those are the two dimensions
    /// `NoteMarker` already has, which is what keeps `fretboard.rs` — and its round-trip test —
    /// out of this change.
    ///
    /// `LINK` rather than `INK` for the selected shape: `INK` is pure white and the fretboard
    /// draws every label white, so a filled `INK` dot would swallow its own text.
    fn color(self) -> Color {
        match self {
            Claim::Selected => LINK,
            Claim::Shared => ROOT_MARKER,
            Claim::Unselected => MUTE,
        }
    }
}

/// One position, and which shapes reached it.
struct Claimed {
    string: usize,
    fret: usize,
    by_selected: bool,
    by_other: bool,
}

/// The positions a placement sounds, as `(string, fret)`.
///
/// Takes the `Voicing` by value and returns an iterator borrowing nothing — `Voicing` is `Copy`
/// and `strings()` hands back an array, so there is no lifetime to thread here.
fn sounded(shape: Voicing) -> impl Iterator<Item = (usize, usize)> {
    shape
        .strings()
        .into_iter()
        .enumerate()
        .filter_map(|(string, fret)| fret.map(|fret| (string, usize::from(fret))))
}

/// Every position any shape in the cycle sounds, once each, with what claims it.
///
/// Once each is the point: adjacent shapes genuinely land on the same fret of the same string,
/// and drawing two dots there would either stack them or let one win silently.
fn claims(shapes: &[Voicing], selected: usize) -> Vec<Claimed> {
    let mut found: Vec<Claimed> = Vec::new();

    for (index, &shape) in shapes.iter().enumerate() {
        let mine = index == selected;

        for (string, fret) in sounded(shape) {
            match found
                .iter_mut()
                .find(|at| at.string == string && at.fret == fret)
            {
                Some(at) => {
                    at.by_selected |= mine;
                    at.by_other |= !mine;
                }
                None => found.push(Claimed {
                    string,
                    fret,
                    by_selected: mine,
                    by_other: !mine,
                }),
            }
        }
    }

    found
}

/// The dots the neck draws for what is on show.
fn caged_markers(caged: &Caged, notation: Notation) -> Vec<NoteMarker> {
    let chord = Chord::new(caged.root(), QUALITY);
    let shapes = caged.shapes();

    claims(&shapes, caged.selected())
        .into_iter()
        .filter_map(|at| {
            // Every sounded string of a placement carries one of the chord's notes — the
            // library's `a_voicing_sounds_the_chord_and_nothing_else` walks all of them. `?`
            // rather than `expect` so a lapse loses a dot that a test counts, instead of
            // taking the process down mid-draw.
            let pitch_class = CAGED_NECK.pitch_class_at(at.string, at.fret)?;
            let note = chord.spell(pitch_class)?;
            let degree = chord.degree(pitch_class)?;

            let claim = match (at.by_selected, at.by_other) {
                (true, true) => Claim::Shared,
                (true, false) => Claim::Selected,
                (false, _) => Claim::Unselected,
            };

            Some(NoteMarker {
                string: at.string,
                fret: at.fret,
                label: marker_label(notation, note, degree),
                color: claim.color(),
                // The root filled, the third and the fifth ringed. Finding the root inside a
                // shape is the skill CAGED teaches, so it does not wait on reading a label.
                style: if degree.number() == 1 {
                    MarkerStyle::Filled
                } else {
                    MarkerStyle::Outlined
                },
            })
        })
        .collect()
}

/// Which string carries the shape's root, counted the way guitarists count — the low E is the
/// sixth. Derived from the degrees on the neck rather than from the shape's own record of it,
/// so the caption describes the dots that are drawn.
fn root_string(chord: Chord, shape: Voicing) -> Option<usize> {
    sounded(shape)
        .find(|&(string, fret)| {
            CAGED_NECK
                .pitch_class_at(string, fret)
                .and_then(|pitch_class| chord.degree(pitch_class))
                .is_some_and(|degree| degree.number() == 1)
        })
        .map(|(string, _)| super::NECK_STRINGS - string)
}

/// How the selected shape reads: which one it is, where it sits, and which string roots it.
fn shape_caption(caged: &Caged) -> String {
    let chord = Chord::new(caged.root(), QUALITY);

    match caged.selected_shape() {
        Some(shape) => {
            let position = if shape.index_fret() == 0 {
                "at the nut".to_string()
            } else {
                format!("at fret {}", shape.index_fret())
            };

            match root_string(chord, shape) {
                Some(string) => format!(
                    "{}  ·  {position}  ·  root on the {string}th string",
                    shape.shape_name()
                ),
                None => format!("{}  ·  {position}", shape.shape_name()),
            }
        }
        None => "no shape sits on this neck".to_string(),
    }
}

/// The count, stated rather than assumed. Five is what a major triad happens to yield; a
/// quality whose shapes cannot all be placed yields fewer, and the screen has to be able to
/// say so without contradicting the neck beside it.
fn count_caption(shapes: &[Voicing]) -> String {
    match shapes.len() {
        1 => "1 shape".to_string(),
        count => format!("{count} shapes"),
    }
}

pub(super) fn ui_caged(
    caged: &Caged,
    notation: Notation,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::widget::{Space, button, column, container, row};
    use iced::{Length, Padding};

    let chord = Chord::new(caged.root(), QUALITY);
    let shapes = caged.shapes();

    // Display only: no press handler and no cursor. The shapes are walked with the pills and
    // the arrows, so the neck stays a picture — which is also why nothing here needs the
    // fretboard's hit-testing.
    let fb = Fretboard {
        num_frets: CAGED_NECK.frets(),
        highlighted: caged_markers(caged, notation),
        ..Fretboard::default()
    };

    let pills = shapes
        .iter()
        .enumerate()
        .fold(row![].spacing(6), |pills, (index, shape)| {
            let chosen = index == caged.selected();

            pills.push(focus_ring(
                button(text(shape.shape_name()).size(14))
                    .padding([6, 14])
                    .style(if chosen {
                        selected_root_button
                    } else {
                        ghost_button
                    })
                    .on_press(Message::SelectCagedShape(index)),
                focused == FocusTarget::CagedShape(index),
            ))
        });

    let summary_card = container(
        column![
            row![
                note_label(chord.root_note(), SUMMARY_NAME_SIZE, INK),
                Space::new().width(Length::Fill),
                text(count_caption(&shapes)).size(16).color(MUTE),
                control_button(
                    control_glyph(
                        row![
                            control_label(text(SMUFL_FLAT.to_string()).font(MUSIC_FONT), 20.0),
                            control_label(text("3"), 20.0),
                        ]
                        .spacing(0)
                    ),
                    Message::ToggleNotation,
                    focused == FocusTarget::NotationToggle,
                ),
                control_button(
                    control_shuffle(),
                    Message::RerollCagedRoot,
                    focused == FocusTarget::RerollCagedRoot,
                ),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            intervalic_text(chord.degrees()),
            text(shape_caption(caged)).size(16).color(BODY),
            pills,
        ]
        .spacing(14),
    )
    .width(Length::Fill)
    .height(Length::Fixed(SUMMARY_CARD_HEIGHT))
    .padding(32)
    .style(card_container);

    let root_selector_content = container(root_row_spans().fold(
        column![].spacing(16),
        |rows, (start, len)| {
            rows.push(
                container(caged_root_row(
                    &PitchClass::ALL[start..start + len],
                    caged.root(),
                    start,
                    focused,
                ))
                .width(Length::Fill)
                .center_x(Length::Fill),
            )
        },
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill);

    let root_selector_card = container(root_selector_content)
        .width(Length::Fill)
        .height(Length::Fixed(SELECTOR_CARD_HEIGHT))
        .padding(32)
        .style(card_container);

    // Both cards to one width, and that width capped: the pills row is the widest thing
    // either of them holds, and a card stretched to the whole window would leave the roots
    // an island in the middle of it. The row is then centred rather than left-aligned, so
    // what is left over sits either side of the screen instead of all down one edge.
    let details = column![summary_card, root_selector_card]
        .width(Length::Fixed(DETAILS_WIDTH))
        .spacing(16);

    container(row![fretboard(fb), details].spacing(32))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: 24.0,
            right: 64.0,
            bottom: 48.0,
            left: 64.0,
        })
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

/// One row of the root selector. Named by the chord a press would build, the way the Scale
/// Trainer's rows are named by the scale — so pitch class 1 reads `D♭` here, since a major
/// triad on it costs two flats where `C♯` would cost three sharps.
fn caged_root_row(
    pitch_classes: &[PitchClass],
    root: PitchClass,
    start_index: usize,
    focused: FocusTarget,
) -> iced::widget::Row<'static, Message> {
    use iced::widget::row;

    pitch_classes
        .iter()
        .enumerate()
        .fold(row![].spacing(28), |row, (i, pitch_class)| {
            let is_selected = *pitch_class == root;
            let color = if is_selected { CANVAS } else { INK };

            row.push(root_square(
                note_label(Chord::new(*pitch_class, QUALITY).root_note(), 24, color),
                is_selected,
                focused == FocusTarget::CagedRoot(start_index + i),
                Message::SelectCagedRoot(*pitch_class),
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ui::shapes::CagedShape;

    fn letters(root: PitchClass) -> Vec<CagedShape> {
        cycle(root).into_iter().map(Voicing::caged).collect()
    }

    /// The property the whole screen rests on: nothing in `cycle` names an order, and the order
    /// that comes out is the one the system is named for.
    #[test]
    fn every_root_spells_a_rotation_of_caged() {
        const RING: [CagedShape; 5] = [
            CagedShape::C,
            CagedShape::A,
            CagedShape::G,
            CagedShape::E,
            CagedShape::D,
        ];

        let rotations: Vec<Vec<CagedShape>> = (0..RING.len())
            .map(|at| {
                RING.iter()
                    .cycle()
                    .skip(at)
                    .take(RING.len())
                    .copied()
                    .collect()
            })
            .collect();

        for root in PitchClass::ALL {
            let found = letters(root);

            assert_eq!(found.len(), 5, "{root:?} yields {found:?}");
            assert!(rotations.contains(&found), "{root:?} yields {found:?}");
        }
    }

    #[test]
    fn the_rotation_starts_where_the_root_sits_lowest() {
        assert_eq!(
            letters(PitchClass::new(0)),
            [
                CagedShape::C,
                CagedShape::A,
                CagedShape::G,
                CagedShape::E,
                CagedShape::D
            ]
        );
        assert_eq!(letters(PitchClass::new(9))[0], CagedShape::A);
        assert_eq!(letters(PitchClass::new(4))[0], CagedShape::E);
    }

    #[test]
    fn the_cycle_is_ordered_up_the_neck_and_holds_one_placement_each() {
        for root in PitchClass::ALL {
            let shapes = cycle(root);
            let frets: Vec<u8> = shapes.iter().map(|v| v.index_fret()).collect();

            assert!(
                frets.windows(2).all(|pair| pair[0] < pair[1]),
                "{root:?} is out of order: {frets:?}"
            );

            // One window per letter. `voicings` offers an octave repeat of some shapes, and
            // showing both would make a five-shape system a six- or seven-shape one.
            let mut seen = shapes.iter().map(|v| v.caged()).collect::<Vec<_>>();
            let before = seen.len();
            seen.dedup();
            assert_eq!(seen.len(), before, "{root:?} repeats a letter");
        }
    }

    fn markers_at(caged: &Caged, notation: Notation) -> Vec<(usize, usize, Color, MarkerStyle)> {
        caged_markers(caged, notation)
            .into_iter()
            .map(|marker| (marker.string, marker.fret, marker.color, marker.style))
            .collect()
    }

    fn c_major_on(shape: CagedShape) -> Caged {
        let mut caged = Caged::new();
        let at = letters(caged.root())
            .into_iter()
            .position(|found| found == shape)
            .expect("C major offers all five shapes");
        caged.select_shape(at);
        caged
    }

    #[test]
    fn a_position_two_shapes_claim_is_drawn_once_and_marked_as_shared() {
        let caged = c_major_on(CagedShape::G);
        let markers = markers_at(&caged, Notation::Notes);

        let shared: Vec<(usize, usize)> = markers
            .iter()
            .filter(|&&(.., color, _)| color == Claim::Shared.color())
            .map(|&(string, fret, ..)| (string, fret))
            .collect();

        assert!(
            !shared.is_empty(),
            "the G shape shares nothing: {markers:?}"
        );

        for at in &shared {
            let drawn = markers
                .iter()
                .filter(|&&(string, fret, ..)| (string, fret) == *at)
                .count();
            assert_eq!(drawn, 1, "{at:?} is drawn {drawn} times");
        }

        // A position only the selected shape reaches is not shared. The G shape's own root on
        // the low E at the eighth fret is inside no other window.
        let alone: Vec<(usize, usize)> = markers
            .iter()
            .filter(|&&(.., color, _)| color == Claim::Selected.color())
            .map(|&(string, fret, ..)| (string, fret))
            .collect();
        assert!(!alone.is_empty(), "{markers:?}");
        assert!(alone.iter().all(|at| !shared.contains(at)));
    }

    /// The concrete overlap: C major's G shape sits at frets 5-8 and its A shape at 3-5, and
    /// the two meet on the fifth fret of the D, G and B strings.
    #[test]
    fn the_g_shape_meets_the_a_shape_on_the_fifth_fret() {
        let caged = c_major_on(CagedShape::G);
        let markers = markers_at(&caged, Notation::Notes);

        for string in [2, 3, 4] {
            let found: Vec<&(usize, usize, Color, MarkerStyle)> = markers
                .iter()
                .filter(|&&(at_string, fret, ..)| (at_string, fret) == (string, 5))
                .collect();

            assert_eq!(found.len(), 1, "string {string} fret 5: {found:?}");
            assert_eq!(
                found[0].2,
                Claim::Shared.color(),
                "string {string} fret 5 is not marked shared"
            );
        }
    }

    #[test]
    fn a_root_is_filled_and_every_other_degree_is_ringed() {
        for notation in [Notation::Notes, Notation::Intervals] {
            for root in PitchClass::ALL {
                let mut caged = Caged::new();
                caged.select_root(root);
                let chord = Chord::new(root, QUALITY);

                for (string, fret, _, style) in markers_at(&caged, notation) {
                    let degree = CAGED_NECK
                        .pitch_class_at(string, fret)
                        .and_then(|pitch_class| chord.degree(pitch_class))
                        .expect("a placed shape sounds only the chord's notes");

                    let expected = if degree.number() == 1 {
                        MarkerStyle::Filled
                    } else {
                        MarkerStyle::Outlined
                    };

                    assert_eq!(style, expected, "{root:?} {degree} at {string}:{fret}");
                }
            }
        }
    }

    #[test]
    fn every_shape_in_the_cycle_reaches_the_neck() {
        // No dot is dropped by the `?` chain in `caged_markers`: what the shapes sound and
        // what the neck draws are the same set of positions.
        for root in PitchClass::ALL {
            let mut caged = Caged::new();
            caged.select_root(root);

            let mut wanted: Vec<(usize, usize)> =
                caged.shapes().into_iter().flat_map(sounded).collect();
            wanted.sort_unstable();
            wanted.dedup();

            let mut drawn: Vec<(usize, usize)> = caged_markers(&caged, Notation::Notes)
                .into_iter()
                .map(|marker| (marker.string, marker.fret))
                .collect();
            drawn.sort_unstable();

            assert_eq!(drawn, wanted, "{root:?}");
        }
    }

    #[test]
    fn the_caption_counts_what_the_neck_draws() {
        for root in PitchClass::ALL {
            let mut caged = Caged::new();
            caged.select_root(root);
            let shapes = caged.shapes();

            assert_eq!(count_caption(&shapes), format!("{} shapes", shapes.len()));
            assert!(
                shape_caption(&caged).starts_with(
                    &caged
                        .selected_shape()
                        .expect("the cycle is not empty")
                        .shape_name()
                )
            );
        }
    }

    #[test]
    fn the_selection_walks_and_stops_at_both_ends() {
        let mut caged = Caged::new();
        assert_eq!(caged.selected(), 0);

        caged.move_selection(-1);
        assert_eq!(caged.selected(), 0, "walked off the low end");

        for expected in 1..5 {
            caged.move_selection(1);
            assert_eq!(caged.selected(), expected);
        }

        caged.move_selection(1);
        assert_eq!(caged.selected(), 4, "walked off the high end");
    }

    #[test]
    fn changing_the_root_keeps_the_selection_where_the_cycle_is_long_enough() {
        let mut caged = Caged::new();
        caged.move_selection(3);

        caged.select_root(PitchClass::new(9));

        assert_eq!(caged.selected(), 3);
        assert_eq!(caged.root(), PitchClass::new(9));
    }

    /// Every root and every reachable selection: the index always names a shape that is there.
    #[test]
    fn the_selection_always_indexes_a_shape() {
        for root in PitchClass::ALL {
            let mut caged = Caged::new();
            caged.select_root(root);

            for _ in 0..8 {
                assert!(
                    caged.selected_shape().is_some(),
                    "{root:?} at {}",
                    caged.selected()
                );
                caged.move_selection(1);
            }

            // And after every root change from the far end of some other cycle.
            for next in PitchClass::ALL {
                caged.select_root(next);
                assert!(caged.selected_shape().is_some(), "{next:?} after {root:?}");
            }
        }
    }
}
