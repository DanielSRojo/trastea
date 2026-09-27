//! Where each screen's focusable widgets are, and how the ring moves between them: a 2-D grid
//! per screen so the arrows go where the layout looks like they should, and the flat order Tab
//! walks derived from it.

use std::ops::Range;

use crate::music::notes::PitchClass;

use super::drill::{ANSWER_COUNT, Drill};
use super::walk_the_scale;
use super::{App, HOME_MENU_ITEMS, Screen};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    HomeMenuItem(usize),
    Back,
    NotationToggle,
    RerollScale,
    Root(usize),
    ScaleKind(usize),
    /// The Note Trainer's neck, as the *Find it* answer surface.
    ///
    /// The one focusable that claims the motion keys: while it is focused the arrows move
    /// the cursor within it instead of moving the ring off it. See `App::move_focus`.
    Fretboard,
    /// One of the twelve *Name it* answer buttons, indexed into `PitchClass::ALL`.
    NoteAnswer(usize),
    DrillDirectionToggle,
    PoolToggle,
    /// The Note Trainer's own. The scale trainer derives its spelling rather than offering
    /// a choice — see `Scale::new` — so this and the Tuner's are the only two.
    NoteSpellingToggle,
    TunerSpellingToggle,
    SkipPrompt,
    /// The Interval Trainer's neck, as its *Find it* answer surface. Claims the motion keys
    /// while focused, exactly as `Fretboard` does on the Note Trainer.
    IntervalFretboard,
    /// One of the twelve interval answer buttons, indexed into the drill's vocabulary.
    IntervalAnswer(usize),
    IntervalDirectionToggle,
    SkipIntervalPrompt,
    /// A listening screen's device control, which opens the input list.
    InputDevice,
    /// The played drill's skip control.
    SkipPlayPrompt,
    /// The Chord Library's search box.
    SearchBox,
    /// One of the notation buttons, indexed into the screen's `Notation::cycle_for`.
    ///
    /// Three buttons rather than one that cycles: a control that changes what it says each
    /// time it is pressed does not look like a control with options, so nobody presses it
    /// twice to find out. The `i` key still cycles — see `App::accelerators`.
    NotationChoice(usize),
    /// The Chord Library's chord list.
    ///
    /// Claims all four motion keys while focused, as the two necks do: up and down walk
    /// the chords, left and right walk the selected chord's voicings. The strip is not a
    /// focusable of its own — the search spec puts the voicings on the horizontal axis
    /// whenever the box is unfocused, and a third stop in the ring would instead make them
    /// reachable only after tabbing past the list.
    ChordList,
    /// The CAGED window's root grid, indexed like the Scale Trainer's.
    CagedRoot(usize),
    /// The CAGED window's shuffle button: a new root, not a new scale.
    RerollCagedRoot,
    /// The scale drill's kind grid, indexed like the Scale Trainer's.
    DrillKind(usize),
    /// A different scale to drill.
    RerollDrillScale,
    /// Starts another run, offered only once one has ended on its duration.
    RestartDrill,
    /// The three controls that exist only while a run is going.
    ScaleAnswering,
    ScaleDuration,
    ScaleNeck,
    /// The Scale Trainer's neck, as the drill's answer surface while pressing. Claims the
    /// motion keys while focused, exactly as the two trainers' necks do.
    ScaleFretboard,
    /// One shape pill in the CAGED window.
    ///
    /// Claims the horizontal motions while focused, as the Chord Library's list does for its
    /// voicings: the arrows change the selection rather than moving the ring along the row,
    /// so walking the cycle is one gesture.
    CagedShape(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// One row of a screen's focus grid. `None` is a cell with no widget in it, which
/// keeps the columns of neighbouring cards aligned.
pub(super) type FocusRow = Vec<Option<FocusTarget>>;

/// Row shapes of the two selector grids on the scale trainer. Both the views and
/// the focus grid are built from these, so the two cannot drift out of sync.
const ROOT_ROW_WIDTH: usize = 3;

/// Six rows, not the five a sixteen-item grid would suggest. `Voodoo | Spanish Gypsy |
/// Whole Tone | Diminished` came to 456px against the 440px the card gets at the default
/// 1280-wide window, so `Diminished` was cut off its right edge. Of every contiguous
/// five-row shape only two fit at all, with 10px and 0px to spare; this is the one
/// six-row shape that both fits — 73px spare on its widest row — and stays a palindrome,
/// which the old 4-3-2-3-4 was.
pub(super) const KIND_ROW_WIDTHS: [usize; 6] = [3, 3, 2, 2, 3, 3];

/// Columns in the Note Trainer's answer grid. Four divides the twelve pitch classes evenly
/// and matches the four header controls above them, so the columns line up.
pub(super) const ANSWER_ROW_WIDTH: usize = 4;

/// Yields the `(start, len)` of each row of the root selector, derived from
/// `PitchClass::ALL` so adding a note reshapes the grid and the view together.
pub(super) fn root_row_spans() -> impl Iterator<Item = (usize, usize)> {
    let total = PitchClass::ALL.len();
    (0..total)
        .step_by(ROOT_ROW_WIDTH)
        .map(move |start| (start, ROOT_ROW_WIDTH.min(total - start)))
}

/// Yields the `(start, len)` of each row of the scale-kind selector. Unlike the
/// root grid these rows are ragged, so the widths are spelled out.
pub(super) fn kind_row_spans() -> impl Iterator<Item = (usize, usize)> {
    KIND_ROW_WIDTHS.into_iter().scan(0, |start, len| {
        let span = (*start, len);
        *start += len;
        Some(span)
    })
}

/// The three functions that describe where a screen's focusable widgets are.
///
/// Methods rather than free functions of `&Screen`, because a screen's shape is no longer
/// determined by which screen it is: the Note Trainer's focusables depend on which drill
/// direction is in play, which is application state. Threading that as an extra parameter
/// would work today and need another parameter the next time — so they read `self`.
impl App {
    /// The focusable widgets of a screen laid out as they appear on it.
    ///
    /// Columns are shared across cards that sit side by side: on the scale trainer the
    /// root card occupies columns `0..ROOT_ROW_WIDTH` and the scale-kind card the
    /// columns after it, so pressing Right at the edge of the root grid steps into the
    /// kinds grid. Cells the layout leaves empty are `None`, which is what stops a
    /// vertical move at the bottom of the root card instead of dropping it into the
    /// taller kinds card alongside.
    pub(super) fn focus_grid(&self) -> Vec<FocusRow> {
        match &self.screen {
            // Back, the control that opens the device list, and the spelling under the dial.
            // The list itself is not in the ring — while it is up it claims the motion keys
            // outright, the way the CAGED shape row does, so walking it is one gesture.
            Screen::Tuner => vec![
                vec![Some(FocusTarget::Back)],
                vec![Some(FocusTarget::InputDevice)],
                vec![Some(FocusTarget::TunerSpellingToggle)],
            ],
            // Back, the skip control, and the device control. The neck is not a focus
            // target here: it takes no press, so there would be nothing to activate on it.
            Screen::PlayTheInterval => vec![
                vec![Some(FocusTarget::Back)],
                vec![Some(FocusTarget::SkipPlayPrompt)],
                vec![Some(FocusTarget::InputDevice)],
            ],
            Screen::Home => (0..HOME_MENU_ITEMS)
                .map(|i| vec![Some(FocusTarget::HomeMenuItem(i))])
                .collect(),
            Screen::Caged => {
                // Back on a row of its own, as on the two trainers and for the same reason:
                // the summary card's own controls already fill the row below it.
                let mut back_row: FocusRow = vec![None; ROOT_ROW_WIDTH];
                back_row[0] = Some(FocusTarget::Back);

                let mut controls: FocusRow = vec![None; ROOT_ROW_WIDTH];
                for (cell, target) in controls
                    .iter_mut()
                    .zip([FocusTarget::NotationToggle, FocusTarget::RerollCagedRoot])
                {
                    *cell = Some(target);
                }

                let mut grid = vec![back_row, controls];

                // As wide as the cycle turned out to be, which is the same number the caption
                // states — so a quality yielding three shapes gets three stops, not five.
                let shapes = self.caged.shapes().len();
                if shapes > 0 {
                    grid.push(
                        (0..shapes)
                            .map(|i| Some(FocusTarget::CagedShape(i)))
                            .collect(),
                    );
                }

                for (start, len) in root_row_spans() {
                    grid.push(
                        (0..len)
                            .map(|i| Some(FocusTarget::CagedRoot(start + i)))
                            .collect(),
                    );
                }

                grid
            }
            Screen::ScaleTrainer => {
                // Two header rows. The summary card's buttons — notation, reroll — sit
                // along its right edge, and the card is as wide as the root card below
                // it, so they take the leftmost columns above the first three
                // roots. That leaves no cell for Back, and widening the row is not an
                // option: `card_bands` splits at ROOT_ROW_WIDTH, so a fourth column would
                // land in the kinds card and Tab would reach the reroll button only after
                // all sixteen scale kinds.
                //
                // So Back takes a row of its own — which is where it really is, in the top
                // bar above the card rather than inside it. The cost is that Right from
                // Back no longer walks into the card; Tab still visits it first.
                let mut back_row: FocusRow = vec![None; ROOT_ROW_WIDTH];
                back_row[0] = Some(FocusTarget::Back);

                // Zipped rather than indexed so the row cannot be built wider than the band
                // it belongs to: a narrower ROOT_ROW_WIDTH drops buttons off the end instead
                // of panicking, a wider one leaves the extra cells empty.
                let mut card_buttons: FocusRow = vec![None; ROOT_ROW_WIDTH];
                for (cell, target) in card_buttons
                    .iter_mut()
                    .zip([FocusTarget::NotationToggle, FocusTarget::RerollScale])
                {
                    *cell = Some(target);
                }

                let root_rows: Vec<_> = root_row_spans().collect();
                let kind_rows: Vec<_> = kind_row_spans().collect();

                let mut grid = vec![back_row, card_buttons];
                for r in 0..root_rows.len().max(kind_rows.len()) {
                    let mut row: FocusRow = vec![None; ROOT_ROW_WIDTH];
                    if let Some(&(start, len)) = root_rows.get(r) {
                        for (col, cell) in row.iter_mut().enumerate().take(len) {
                            *cell = Some(FocusTarget::Root(start + col));
                        }
                    }
                    if let Some(&(start, len)) = kind_rows.get(r) {
                        row.extend((0..len).map(|i| Some(FocusTarget::ScaleKind(start + i))));
                    }
                    grid.push(row);
                }
                grid
            }
            // Back on a row of its own, then the kind grid alone: the drill's root is set on
            // the glyph in the middle column, so there is no root row to walk and no per-root
            // stop to land on. What differs from every other screen is the last row, which
            // grows with the run — a control that is not on screen is not a focus target,
            // which is what keeps Tab from stopping on nothing.
            Screen::WalkTheScale => {
                let mut grid: Vec<FocusRow> = vec![vec![Some(FocusTarget::Back)]];

                for (start, len) in kind_row_spans() {
                    grid.push(
                        (0..len)
                            .map(|i| Some(FocusTarget::DrillKind(start + i)))
                            .collect(),
                    );
                }

                let mut drill_row: FocusRow = Vec::new();
                let running = self.walk_the_scale.is_running();
                let playing = matches!(
                    self.walk_the_scale.answering(),
                    walk_the_scale::Answering::Playing { .. }
                );

                // The neck is only an answer surface while a press means something on it.
                if running && !playing {
                    drill_row.push(Some(FocusTarget::ScaleFretboard));
                }
                drill_row.push(Some(FocusTarget::RerollDrillScale));
                if running {
                    drill_row.push(Some(FocusTarget::ScaleAnswering));
                    drill_row.push(Some(FocusTarget::ScaleDuration));
                    if playing {
                        drill_row.push(Some(FocusTarget::ScaleNeck));
                        drill_row.push(Some(FocusTarget::InputDevice));
                    }
                } else {
                    drill_row.push(Some(FocusTarget::RestartDrill));
                }
                grid.push(drill_row);

                grid
            }
            Screen::NoteTrainer => {
                // Back on a row of its own, as on the scale trainer and for the same
                // reason: the four header controls already fill the card's own row.
                let mut back_row: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                back_row[0] = Some(FocusTarget::Back);

                let mut controls: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                for (cell, target) in controls.iter_mut().zip([
                    FocusTarget::DrillDirectionToggle,
                    FocusTarget::PoolToggle,
                    FocusTarget::NoteSpellingToggle,
                    FocusTarget::SkipPrompt,
                ]) {
                    *cell = Some(target);
                }

                let mut grid = vec![back_row, controls];

                // This is why these are methods on `App`: the answer surface — and so what
                // is focusable at all — depends on which way the drill is running, and the
                // prompt is the only thing that knows.
                match self.note_trainer.drill() {
                    Drill::NameIt => {
                        let total = PitchClass::ALL.len();

                        for start in (0..total).step_by(ANSWER_ROW_WIDTH) {
                            let len = ANSWER_ROW_WIDTH.min(total - start);
                            let mut row: FocusRow = vec![None; ANSWER_ROW_WIDTH];

                            for (col, cell) in row.iter_mut().enumerate().take(len) {
                                *cell = Some(FocusTarget::NoteAnswer(start + col));
                            }
                            grid.push(row);
                        }
                    }
                    // The neck is the answer surface, and it is one cell no matter how many
                    // positions it holds — the cursor walks those, not the focus ring.
                    Drill::FindIt => {
                        let mut row: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                        row[0] = Some(FocusTarget::Fretboard);
                        grid.push(row);
                    }
                }

                grid
            }
            Screen::IntervalTrainer => {
                // Back on a row of its own, as on the two screens above and for the same
                // reason: the card's own row belongs to its controls.
                let mut back_row: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                back_row[0] = Some(FocusTarget::Back);

                let mut controls: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                for (cell, target) in controls.iter_mut().zip([
                    FocusTarget::IntervalDirectionToggle,
                    FocusTarget::SkipIntervalPrompt,
                ]) {
                    *cell = Some(target);
                }

                let mut grid = vec![back_row, controls];

                match self.interval_trainer.drill() {
                    Drill::NameIt => {
                        let total = ANSWER_COUNT;

                        for start in (0..total).step_by(ANSWER_ROW_WIDTH) {
                            let len = ANSWER_ROW_WIDTH.min(total - start);
                            let mut row: FocusRow = vec![None; ANSWER_ROW_WIDTH];

                            for (col, cell) in row.iter_mut().enumerate().take(len) {
                                *cell = Some(FocusTarget::IntervalAnswer(start + col));
                            }
                            grid.push(row);
                        }
                    }
                    // The neck is the answer surface, and it is one cell no matter how many
                    // positions it holds — the cursor walks those, not the focus ring.
                    Drill::FindIt => {
                        let mut row: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                        row[0] = Some(FocusTarget::IntervalFretboard);
                        grid.push(row);
                    }
                }

                grid
            }
            // Two focusables, not three. The search spec puts the voicings on the
            // horizontal axis whenever the box is unfocused, so the strip is browsed from
            // the list rather than tabbed into — a third stop would make a voicing
            // reachable only after walking past every chord.
            Screen::ChordLibrary => vec![
                vec![Some(FocusTarget::Back), None, None, None],
                // The notation buttons sit beside the box rather than below the list, so
                // Right walks into them: the list claims the motion keys, and anything
                // under it in the grid would be reachable by Tab alone.
                vec![
                    Some(FocusTarget::SearchBox),
                    Some(FocusTarget::NotationChoice(0)),
                    Some(FocusTarget::NotationChoice(1)),
                    Some(FocusTarget::NotationChoice(2)),
                ],
                vec![Some(FocusTarget::ChordList), None, None, None],
            ],
        }
    }

    /// The column bands the cards of a screen occupy, in the order Tab visits them.
    ///
    /// Splitting the scale trainer at the root card's edge is what makes Tab finish one
    /// card before starting the next; reading the grid straight across would instead
    /// hop between the two side-by-side cards every few widgets. The back and reroll
    /// buttons fall in the first band, so they lead the order.
    #[expect(
        clippy::single_range_in_vec_init,
        reason = "a one-element Vec holding the full-width band, not a collected range"
    )]
    fn card_bands(&self, width: usize) -> Vec<Range<usize>> {
        match &self.screen {
            Screen::ScaleTrainer => vec![0..ROOT_ROW_WIDTH, ROOT_ROW_WIDTH..width],
            // These screens have a single card, so one band spans the whole width. Walk the
            // Scale joined them when its root card went: two bands split the kind rows down
            // the middle, and Tab walked the first three columns of every row before coming
            // back for the rest.
            Screen::Home
            | Screen::NoteTrainer
            | Screen::IntervalTrainer
            | Screen::PlayTheInterval
            | Screen::ChordLibrary
            | Screen::Caged
            | Screen::WalkTheScale
            | Screen::Tuner => vec![0..width],
        }
    }

    /// Every focusable on a screen, card by card — the Tab order.
    ///
    /// Derived from the same grid the arrow keys use, so a widget can never be
    /// reachable by one and not the other.
    pub(super) fn focusables(&self) -> Vec<FocusTarget> {
        let grid = self.focus_grid();
        let width = grid.iter().map(Vec::len).max().unwrap_or(0);

        let mut targets = Vec::new();
        for band in self.card_bands(width) {
            for row in &grid {
                let cells = row.iter().skip(band.start).take(band.end - band.start);
                targets.extend(cells.flatten().copied());
            }
        }
        targets
    }
}

fn grid_position(grid: &[FocusRow], target: FocusTarget) -> Option<(usize, usize)> {
    grid.iter().enumerate().find_map(|(row, cells)| {
        cells
            .iter()
            .position(|&cell| cell == Some(target))
            .map(|col| (row, col))
    })
}

/// Moves one cell in `direction`, staying put when there is nothing that way.
pub(super) fn step_focus_2d(
    grid: &[FocusRow],
    current: FocusTarget,
    direction: Direction,
) -> FocusTarget {
    let Some((row, col)) = grid_position(grid, current) else {
        // Focus is stale (the screen changed under it) — snap back onto the grid.
        return grid
            .iter()
            .flatten()
            .find_map(|&cell| cell)
            .unwrap_or(current);
    };

    let next = match direction {
        Direction::Left => scan_row(&grid[row], col, -1),
        Direction::Right => scan_row(&grid[row], col, 1),
        Direction::Up => scan_column(grid, row, col, -1),
        Direction::Down => scan_column(grid, row, col, 1),
    };

    next.unwrap_or(current)
}

/// Walks sideways from `col`, skipping empty cells, until a widget or the row's end.
fn scan_row(row: &FocusRow, col: usize, delta: isize) -> Option<FocusTarget> {
    let mut i = col as isize + delta;
    while let Some(cell) = usize::try_from(i).ok().and_then(|i| row.get(i)) {
        if cell.is_some() {
            return *cell;
        }
        i += delta;
    }
    None
}

/// Steps to the adjacent row and takes the widget nearest to `col` without looking
/// past it. Searching leftwards only is what keeps a vertical move inside the card
/// it started in: a shorter row clamps to its own last widget, and a row that is
/// only occupied further right than `col` yields nothing at all.
fn scan_column(grid: &[FocusRow], row: usize, col: usize, delta: isize) -> Option<FocusTarget> {
    let next = usize::try_from(row as isize + delta).ok()?;
    grid.get(next)?
        .iter()
        .take(col + 1)
        .rev()
        .find_map(|&cell| cell)
}

pub(super) fn step_focus(list: &[FocusTarget], current: FocusTarget, delta: isize) -> FocusTarget {
    if list.is_empty() {
        return current;
    }

    match list.iter().position(|&target| target == current) {
        Some(i) => {
            let len = list.len() as isize;
            let next = (i as isize + delta).rem_euclid(len) as usize;
            list[next]
        }
        None => list[0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::scales::ScaleKind;
    use crate::ui::tests::app_on;

    /// One focusable per menu entry, counted from the menu rather than written out — the
    /// same source the buttons and the digit keys come from, so adding a trainer cannot
    /// leave this asserting the old number.
    #[test]
    fn home_has_a_focusable_per_menu_entry() {
        assert_eq!(
            app_on(Screen::Home).focusables().len(),
            HOME_MENU_ITEMS,
            "the menu and its focus ring disagree"
        );
    }

    #[test]
    fn scale_trainer_reaches_every_widget_exactly_once() {
        let targets = app_on(Screen::ScaleTrainer).focusables();
        // Three pieces of chrome: Back, and the summary card's two buttons.
        assert_eq!(
            targets.len(),
            3 + PitchClass::ALL.len() + ScaleKind::ALL.len()
        );

        for i in 0..PitchClass::ALL.len() {
            assert!(targets.contains(&FocusTarget::Root(i)), "missing root {i}");
        }
        for i in 0..ScaleKind::ALL.len() {
            assert!(
                targets.contains(&FocusTarget::ScaleKind(i)),
                "missing kind {i}"
            );
        }
        assert!(targets.contains(&FocusTarget::NotationToggle));
    }

    #[test]
    fn tab_walks_one_card_at_a_time() {
        // Reading order: Back, the card's two buttons left to right, then every root
        // before any kind. Back and the buttons are on separate grid rows now, and the
        // point of this test is that the split changes nothing about the Tab order —
        // both rows are in the root card's band, so they still lead it.
        let mut expected = vec![
            FocusTarget::Back,
            FocusTarget::NotationToggle,
            FocusTarget::RerollScale,
        ];
        expected.extend((0..PitchClass::ALL.len()).map(FocusTarget::Root));
        expected.extend((0..ScaleKind::ALL.len()).map(FocusTarget::ScaleKind));

        assert_eq!(app_on(Screen::ScaleTrainer).focusables(), expected);
    }

    #[test]
    fn tab_and_arrows_agree_on_what_is_focusable() {
        // The two orders are built from the same grid; this catches a widget that
        // becomes reachable by one and not the other.
        for screen in [
            Screen::Home,
            Screen::ScaleTrainer,
            Screen::NoteTrainer,
            Screen::IntervalTrainer,
        ] {
            let mut from_tab = app_on(screen.clone()).focusables();
            let mut in_grid: Vec<_> = app_on(screen.clone())
                .focus_grid()
                .into_iter()
                .flatten()
                .flatten()
                .collect();

            assert_eq!(from_tab.len(), in_grid.len(), "{screen:?} count differs");

            from_tab.sort_by_key(|t| format!("{t:?}"));
            in_grid.sort_by_key(|t| format!("{t:?}"));
            assert_eq!(from_tab, in_grid, "{screen:?} membership differs");
        }
    }

    #[test]
    fn selector_row_spans_cover_their_arrays() {
        let roots: Vec<_> = root_row_spans().collect();
        assert_eq!(roots, vec![(0, 3), (3, 3), (6, 3), (9, 3)]);

        let kinds: Vec<_> = kind_row_spans().collect();
        assert_eq!(
            kinds,
            vec![(0, 3), (3, 3), (6, 2), (8, 2), (10, 3), (13, 3)]
        );

        let covered: usize = kinds.iter().map(|&(_, len)| len).sum();
        assert_eq!(covered, ScaleKind::ALL.len());
    }

    #[test]
    fn step_focus_wraps_forward_past_end() {
        let list = app_on(Screen::Home).focusables();
        let last = *list.last().unwrap();
        assert_eq!(step_focus(&list, last, 1), list[0]);
    }

    #[test]
    fn step_focus_wraps_backward_past_start() {
        let list = app_on(Screen::Home).focusables();
        let last = *list.last().unwrap();
        assert_eq!(step_focus(&list, list[0], -1), last);
    }

    #[test]
    fn step_focus_snaps_to_first_when_target_absent() {
        let list = app_on(Screen::Home).focusables();
        // Back is not focusable on Home, so a stale focus snaps to the first item.
        assert_eq!(step_focus(&list, FocusTarget::Back, 1), list[0]);
    }

    /// Presses one arrow key on the scale trainer.
    fn arrow(from: FocusTarget, direction: Direction) -> FocusTarget {
        step_focus_2d(&app_on(Screen::ScaleTrainer).focus_grid(), from, direction)
    }

    #[test]
    fn arrows_walk_within_the_root_grid() {
        // C C# D / D# E F / F# G G# / A A# B
        assert_eq!(
            arrow(FocusTarget::Root(0), Direction::Right),
            FocusTarget::Root(1)
        );
        assert_eq!(
            arrow(FocusTarget::Root(1), Direction::Left),
            FocusTarget::Root(0)
        );
        assert_eq!(
            arrow(FocusTarget::Root(1), Direction::Down),
            FocusTarget::Root(4)
        );
        assert_eq!(
            arrow(FocusTarget::Root(4), Direction::Up),
            FocusTarget::Root(1)
        );
    }

    #[test]
    fn right_edge_of_root_grid_crosses_into_the_kinds_card() {
        // D is the last root in its row; Ionian is the first kind in the same row.
        assert_eq!(
            arrow(FocusTarget::Root(2), Direction::Right),
            FocusTarget::ScaleKind(0)
        );
        assert_eq!(
            arrow(FocusTarget::ScaleKind(0), Direction::Left),
            FocusTarget::Root(2)
        );
    }

    #[test]
    fn arrows_stop_at_the_outer_edges() {
        assert_eq!(
            arrow(FocusTarget::Root(0), Direction::Left),
            FocusTarget::Root(0)
        );
        assert_eq!(arrow(FocusTarget::Back, Direction::Up), FocusTarget::Back);

        // ScaleKind(2) ends the widest kind row, so nothing is to its right.
        assert_eq!(
            arrow(FocusTarget::ScaleKind(2), Direction::Right),
            FocusTarget::ScaleKind(2)
        );
    }

    #[test]
    fn down_past_the_root_grid_does_not_jump_into_the_taller_kinds_card() {
        // The kinds card has a fifth and sixth row, the root card does not. Leaving the
        // last root row must stay put rather than teleporting across to ScaleKind(10).
        for last_row_root in [
            FocusTarget::Root(9),
            FocusTarget::Root(10),
            FocusTarget::Root(11),
        ] {
            assert_eq!(arrow(last_row_root, Direction::Down), last_row_root);
        }
    }

    #[test]
    fn vertical_moves_clamp_into_shorter_rows() {
        // Row 2 of the kinds card holds 2 items (indices 6..8), so leaving the 3-wide
        // row above it from its last column lands on that row's last item.
        assert_eq!(
            arrow(FocusTarget::ScaleKind(5), Direction::Down),
            FocusTarget::ScaleKind(7)
        );
        // Row 3 is the same width as row 2, so the column is carried straight down
        // rather than clamped.
        assert_eq!(
            arrow(FocusTarget::ScaleKind(7), Direction::Down),
            FocusTarget::ScaleKind(9)
        );
    }

    #[test]
    fn the_card_buttons_line_up_with_the_columns_below_them() {
        // The header is two rows: Back alone, then the card's two buttons, which take
        // one column each above the first two roots. The third column of that row is
        // empty since the spelling toggle went — which is why `Root(2)` has no button
        // above it and is not in the loop below.
        assert_eq!(
            arrow(FocusTarget::Back, Direction::Down),
            FocusTarget::NotationToggle
        );
        assert_eq!(
            arrow(FocusTarget::NotationToggle, Direction::Up),
            FocusTarget::Back
        );

        for (button, root) in [
            (FocusTarget::NotationToggle, FocusTarget::Root(0)),
            (FocusTarget::RerollScale, FocusTarget::Root(1)),
        ] {
            assert_eq!(arrow(button, Direction::Down), root, "down from {button:?}");
            assert_eq!(arrow(root, Direction::Up), button, "up from {root:?}");
        }
    }

    #[test]
    fn the_card_button_row_walks_notation_reroll() {
        assert_eq!(
            arrow(FocusTarget::NotationToggle, Direction::Right),
            FocusTarget::RerollScale
        );
        assert_eq!(
            arrow(FocusTarget::RerollScale, Direction::Left),
            FocusTarget::NotationToggle
        );
        // The row's third cell is empty, so Right from the reroll button stays put
        // rather than stepping into a hole. Asserted because an empty trailing cell is
        // exactly the kind of thing a grid walk can fall into.
        assert_eq!(
            arrow(FocusTarget::RerollScale, Direction::Right),
            FocusTarget::RerollScale
        );

        // Back is alone on its row, so the horizontal keys have nowhere to take it.
        // It used to step sideways into the card; reaching the card is Tab's job now,
        // or Down's. Asserted rather than left implicit because it is the one thing
        // the two-row header gave up.
        assert_eq!(
            arrow(FocusTarget::Back, Direction::Right),
            FocusTarget::Back
        );
        assert_eq!(arrow(FocusTarget::Back, Direction::Left), FocusTarget::Back);
    }

    #[test]
    fn home_menu_is_vertical_only() {
        let grid = app_on(Screen::Home).focus_grid();
        let first = FocusTarget::HomeMenuItem(0);

        assert_eq!(
            step_focus_2d(&grid, first, Direction::Down),
            FocusTarget::HomeMenuItem(1)
        );
        assert_eq!(step_focus_2d(&grid, first, Direction::Up), first);
        assert_eq!(step_focus_2d(&grid, first, Direction::Right), first);
        assert_eq!(step_focus_2d(&grid, first, Direction::Left), first);
    }

    #[test]
    fn arrows_snap_stale_focus_back_onto_the_grid() {
        let grid = app_on(Screen::Home).focus_grid();
        assert_eq!(
            step_focus_2d(&grid, FocusTarget::Back, Direction::Down),
            FocusTarget::HomeMenuItem(0)
        );
    }
}
