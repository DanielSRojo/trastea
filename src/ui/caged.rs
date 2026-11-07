//! The CAGED window: one root's five shapes tiled along one neck, the root set on its glyph.
//! Holds no shape data; it shows `super::shapes::voicings` filtered and ordered.

use crate::music::chords::{Chord, ChordQuality};
use crate::music::notes::PitchClass;

use iced::widget::text;
use iced::{Color, Element};

use super::focus::FocusTarget;
use super::fretboard::{Fretboard, MarkerStyle, NoteMarker, fretboard};
use super::neck::CAGED_NECK;
use super::root_stage::{
    CARD_PADDING, FORMULA_SIZE, formula, root_column, root_glyph, selector_column, stage,
};
use super::shapes::{Voicing, voicings};
use super::style::{
    BODY, INK, LINK, MUSIC_FONT, MUTE, ROOT_MARKER, SMUFL_FLAT, card_container, control_button,
    control_glyph, control_label, control_shuffle, focus_ring, ghost_button, selected_root_button,
};
use super::{Message, Notation, marker_label, note_label};
use crate::music::intervals::Interval;
use crate::music::notes::Note;

/// Major, because it is the only quality whose shapes form a cycle.
const QUALITY: ChordQuality = ChordQuality::Major;

/// Named so the fit test budgets the shape card against what it draws.
const SHAPE_PILL_SIZE: f32 = 16.0;
const COUNT_SIZE: f32 = 14.0;
const SHAPE_CARD_SPACING: f32 = 10.0;

/// Which root is shown and which of its shapes is picked.
/// The cycle is derived from `root` rather than cached, so it cannot disagree with it.
pub(super) struct Caged {
    root: PitchClass,
    /// An index into the derived cycle, whose length depends on the quality.
    selected: usize,
}

/// The shapes for `root`, one per letter at its lowest fret, ordered by fret.
/// Nothing here says C-A-G-E-D; the order falls out of the placement arithmetic.
fn cycle(root: PitchClass) -> Vec<Voicing> {
    let mut found: Vec<Voicing> = Vec::new();

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

    /// Steps the root `steps` semitones, wrapping in both directions.
    pub(super) fn step_root(&mut self, steps: i32) {
        self.select_root(self.root.shift(steps));
    }

    pub(super) fn select_shape(&mut self, index: usize) {
        if index < self.shapes().len() {
            self.selected = index;
        }
    }

    /// Walks the cycle, stopping at the ends rather than wrapping. Says whether it moved, so
    /// an arrow at either end can leave the pills instead.
    pub(super) fn move_selection(&mut self, delta: isize) -> bool {
        let count = self.shapes().len();
        if count == 0 {
            return false;
        }

        let next = (self.selected as isize + delta).clamp(0, count as isize - 1) as usize;
        let moved = next != self.selected;
        self.selected = next;
        moved
    }

    /// Called when the screen opens; the root and selection survive leaving it.
    pub(super) fn enter(&mut self) {
        self.clamp();
    }

    /// Keeps `selected` pointing at a shape that exists, including in an empty cycle.
    fn clamp(&mut self) {
        self.selected = self.selected.min(self.shapes().len().saturating_sub(1));
    }
}

/// What claims a position on the neck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Claim {
    Selected,
    Shared,
    Unselected,
}

impl Claim {
    /// `LINK` rather than `INK` for the selected shape: the fretboard draws every label white,
    /// so a filled `INK` dot would swallow its own text.
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
fn sounded(shape: Voicing) -> impl Iterator<Item = (usize, usize)> {
    shape
        .strings()
        .into_iter()
        .enumerate()
        .filter_map(|(string, fret)| fret.map(|fret| (string, usize::from(fret))))
}

/// Every position any shape in the cycle sounds, once each, with what claims it.
/// Adjacent shapes share positions, and two dots there would stack.
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
            // `?` rather than `expect`: a lapse loses a dot a test counts rather than panicking.
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
                // The root filled, the third and the fifth ringed.
                style: if degree.number() == 1 {
                    MarkerStyle::Filled
                } else {
                    MarkerStyle::Outlined
                },
            })
        })
        .collect()
}

/// Which string carries the shape's root, counted so the low E is the sixth.
/// Derived from the degrees on the neck so the caption matches the dots drawn.
fn root_string(chord: Chord, shape: Voicing) -> Option<usize> {
    sounded(shape)
        .find(|&(string, fret)| {
            CAGED_NECK
                .pitch_class_at(string, fret)
                .and_then(|pitch_class| chord.degree(pitch_class))
                .is_some_and(|degree| degree.number() == 1)
        })
        .map(|(string, _)| super::neck::NECK_STRINGS - string)
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

/// Stated rather than assumed: a quality whose shapes cannot all be placed yields fewer.
fn count_caption(shapes: &[Voicing]) -> String {
    match shapes.len() {
        1 => "1 shape".to_string(),
        count => format!("{count} shapes"),
    }
}

/// What the line under the root spells the triad with: whatever the markers say.
#[derive(Debug, PartialEq)]
enum Spelled {
    Notes(Vec<Note>),
    Degrees(&'static [Interval]),
}

fn spelled(chord: Chord, notation: Notation) -> Spelled {
    match notation {
        Notation::Notes => Spelled::Notes(chord.notes()),
        // a triad has no fingering of its own here, so this never holds `Fingers`
        Notation::Intervals | Notation::Fingers => Spelled::Degrees(chord.degrees()),
    }
}

pub(super) fn ui_caged(
    caged: &Caged,
    notation: Notation,
    dragging: bool,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{button, column, container, row};

    let chord = Chord::new(caged.root(), QUALITY);
    let shapes = caged.shapes();

    // Display only: no press handler, so the fretboard's hit-testing goes unused.
    let fb = Fretboard {
        num_frets: CAGED_NECK.frets(),
        highlighted: caged_markers(caged, notation),
        ..Fretboard::default()
    };

    let triad: Element<'static, Message> = match spelled(chord, notation) {
        Spelled::Notes(notes) => notes
            .into_iter()
            .fold(row![].spacing(14), |tokens, note| {
                tokens.push(note_label(note, FORMULA_SIZE, INK))
            })
            .into(),
        Spelled::Degrees(degrees) => formula(degrees, |_| INK).into(),
    };

    let below = column![triad, text(shape_caption(caged)).size(16).color(BODY),]
        .spacing(12)
        .align_x(iced::Alignment::Center);

    let centre = root_column(root_glyph(chord.root_note(), dragging), below, None);

    // stacked in cycle order, so the pills run the way the shapes sit down the neck
    let pills = shapes.iter().enumerate().fold(
        column![text(count_caption(&shapes)).size(COUNT_SIZE).color(MUTE)]
            .spacing(SHAPE_CARD_SPACING),
        |pills, (index, shape)| {
            let chosen = index == caged.selected();

            pills.push(focus_ring(
                button(text(shape.shape_name()).size(SHAPE_PILL_SIZE))
                    .width(Length::Fill)
                    .padding([8, 14])
                    .style(if chosen {
                        selected_root_button
                    } else {
                        ghost_button
                    })
                    .on_press(Message::SelectCagedShape(index)),
                focused == FocusTarget::CagedShape(index),
            ))
        },
    );

    let shape_card = container(pills)
        .width(Length::Fill)
        .padding(CARD_PADDING)
        .style(card_container);

    let controls = row![
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
    .spacing(8);

    let selector = selector_column(
        shape_card,
        container(controls)
            .width(Length::Fill)
            .center_x(Length::Fill),
    );

    stage(fretboard(fb), centre, selector)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ui::shapes::CagedShape;

    fn letters(root: PitchClass) -> Vec<CagedShape> {
        cycle(root).into_iter().map(Voicing::caged).collect()
    }

    /// Nothing in `cycle` names an order, and the one that comes out is C-A-G-E-D.
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

            // One window per letter, though `voicings` also offers octave repeats.
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

        // The G shape's root on the low E at the eighth fret is inside no other window.
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
        // No dot is dropped by the `?` chain in `caged_markers`.
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

        assert!(!caged.move_selection(-1));
        assert_eq!(caged.selected(), 0, "walked off the low end");

        for expected in 1..5 {
            assert!(caged.move_selection(1));
            assert_eq!(caged.selected(), expected);
        }

        assert!(!caged.move_selection(1));
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

    #[test]
    fn stepping_the_root_wraps_and_keeps_the_selection() {
        let mut caged = Caged::new();
        caged.move_selection(2);

        caged.step_root(-1);
        assert_eq!(caged.root(), PitchClass::new(11), "C did not wrap to B");
        assert_eq!(caged.selected(), 2);

        caged.step_root(13);
        assert_eq!(caged.root(), PitchClass::new(0));
    }

    #[test]
    fn the_shape_card_fits_the_root_band() {
        // iced's default line height, and a pill's `.padding([8, 14])` inside its 3px `focus_ring`
        const TEXT_LINE_HEIGHT: f32 = 1.3;
        const BUTTON_PADDING_Y: f32 = 8.0;
        const FOCUS_RING_PADDING: f32 = 3.0;

        for root in PitchClass::ALL {
            let shapes = cycle(root).len() as f32;
            let pill = SHAPE_PILL_SIZE * TEXT_LINE_HEIGHT
                + 2.0 * BUTTON_PADDING_Y
                + 2.0 * FOCUS_RING_PADDING;
            let needed = COUNT_SIZE * TEXT_LINE_HEIGHT
                + shapes * (pill + SHAPE_CARD_SPACING)
                + 2.0 * CARD_PADDING;

            assert!(
                crate::ui::root_stage::ROOT_BAND_HEIGHT >= needed,
                "{root:?}'s shape card needs {needed}, more than the root band holds"
            );
        }
    }

    #[test]
    fn the_line_under_the_root_follows_the_notation() {
        let chord = Chord::new(PitchClass::new(2), QUALITY);

        let Spelled::Notes(notes) = spelled(chord, Notation::Notes) else {
            panic!("notes notation spelled degrees");
        };
        let names: Vec<String> = notes.iter().map(Note::to_string).collect();
        assert_eq!(names, ["D", "F#", "A"]);

        assert_eq!(
            spelled(chord, Notation::Intervals),
            Spelled::Degrees(chord.degrees())
        );
    }
}
