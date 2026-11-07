//! Where each screen's focusable widgets are: a 2-D grid per screen for the arrows, and the
//! Tab order derived from it.

use std::ops::Range;

use crate::music::notes::PitchClass;

use super::drill::{ANSWER_COUNT, Drill};
use super::home::{self, Category};
use super::{App, Screen};
use super::{note_trainer, walk_the_scale};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    /// A block of the Home menu; the focused one is the one whose entries show.
    HomeCategory(Category),
    /// One screen in a Home category, indexed into its `entries`.
    HomeEntry(Category, usize),
    Back,
    NotationToggle,
    RerollScale,
    Root(usize),
    ScaleKind(usize),
    /// The Note Trainer's neck, as the *Find it* answer surface. Claims the motion keys.
    Fretboard,
    /// One of the twelve *Name it* answer buttons, indexed into `PitchClass::ALL`.
    NoteAnswer(usize),
    DrillDirectionToggle,
    /// Swaps the Note Trainer between pressing and playing.
    NoteAnswering,
    PoolToggle,
    NoteSpellingToggle,
    TunerSpellingToggle,
    /// Starts the Position Finder's calibration walk.
    Calibrate,
    /// Shows the groups the Position Finder's calibration will not split.
    PositionGroups,
    SkipPrompt,
    /// The Interval Trainer's neck, as its *Find it* answer surface. Claims the motion keys.
    IntervalFretboard,
    /// One of the twelve interval answer buttons, indexed into the drill's vocabulary.
    IntervalAnswer(usize),
    IntervalDirectionToggle,
    SkipIntervalPrompt,
    /// A listening screen's device control, which opens the input list.
    InputDevice,
    SkipPlayPrompt,
    SearchBox,
    /// One of the notation buttons, indexed into the screen's `Notation::cycle_for`.
    NotationChoice(usize),
    /// The Chord Library's chord list. Claims all four motion keys: up and down walk the
    /// chords, left and right the selected chord's voicings.
    ChordList,
    /// The CAGED window's root grid, indexed like the Scale Trainer's.
    CagedRoot(usize),
    /// A new root for the CAGED window, not a new scale.
    RerollCagedRoot,
    /// The scale drill's kind grid, indexed like the Scale Trainer's.
    DrillKind(usize),
    RerollDrillScale,
    /// Offered only once a run has ended on its duration.
    RestartDrill,
    /// The three controls that exist only while a run is going.
    ScaleAnswering,
    ScaleDuration,
    ScaleNeck,
    /// The scale drill's neck, as its answer surface while pressing. Claims the motion keys.
    ScaleFretboard,
    /// One shape pill in the CAGED window. Claims the horizontal motion keys.
    CagedShape(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// One row of a screen's focus grid. `None` cells keep neighbouring cards' columns aligned.
pub(super) type FocusRow = Vec<Option<FocusTarget>>;

/// Row shapes of the scale trainer's selector grids, shared by the views and the focus grid.
const ROOT_ROW_WIDTH: usize = 3;

/// Six rows because no five-row shape fits the 440px card at the default window width.
pub(super) const KIND_ROW_WIDTHS: [usize; 6] = [3, 3, 2, 2, 3, 3];

/// Columns in the trainers' answer grids: four divides the twelve answers.
pub(super) const ANSWER_ROW_WIDTH: usize = 4;

/// Yields the `(start, len)` of each row of the root selector.
pub(super) fn root_row_spans() -> impl Iterator<Item = (usize, usize)> {
    let total = PitchClass::ALL.len();
    (0..total)
        .step_by(ROOT_ROW_WIDTH)
        .map(move |start| (start, ROOT_ROW_WIDTH.min(total - start)))
}

/// Yields the `(start, len)` of each row of the scale-kind selector.
pub(super) fn kind_row_spans() -> impl Iterator<Item = (usize, usize)> {
    KIND_ROW_WIDTHS.into_iter().scan(0, |start, len| {
        let span = (*start, len);
        *start += len;
        Some(span)
    })
}

/// Where a screen's focusable widgets are; methods because some depend on screen state.
impl App {
    /// The focusable widgets of a screen laid out as they appear on it.
    ///
    /// Side-by-side cards share one row, so Right at a card's edge steps into the next;
    /// empty cells are `None` so a vertical move stays inside its card.
    pub(super) fn focus_grid(&self) -> Vec<FocusRow> {
        match &self.screen {
            // the device list is not in the ring; while up it claims the motion keys
            Screen::Tuner => vec![
                vec![Some(FocusTarget::Back)],
                vec![Some(FocusTarget::InputDevice)],
                vec![Some(FocusTarget::TunerSpellingToggle)],
            ],
            // the neck answers rather than asks, so it is not a focus target either
            Screen::PositionFinder => vec![
                vec![Some(FocusTarget::Back), None],
                vec![
                    Some(FocusTarget::Calibrate),
                    Some(FocusTarget::PositionGroups),
                ],
                vec![Some(FocusTarget::InputDevice), None],
            ],
            // the neck takes no press here, so it is not a focus target
            Screen::PlayTheInterval => vec![
                vec![Some(FocusTarget::Back)],
                vec![Some(FocusTarget::SkipPlayPrompt)],
                vec![Some(FocusTarget::InputDevice)],
            ],
            // drawn and hit-tested by this grid, but the arrows go through `home::step`
            Screen::Home => home::focus_grid(self.focused),
            Screen::Caged => {
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

                // as wide as the cycle, which may be fewer than five shapes
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
                // Back gets its own row: a fourth column in the button row would fall in the
                // kinds card's band and Tab would reach it only after all the kinds
                let mut back_row: FocusRow = vec![None; ROOT_ROW_WIDTH];
                back_row[0] = Some(FocusTarget::Back);

                // zipped so a narrower ROOT_ROW_WIDTH drops buttons instead of panicking
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
            // the last row holds only the controls the run currently shows
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
                let mut back_row: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                back_row[0] = Some(FocusTarget::Back);

                let playing = self.note_trainer.answering() == note_trainer::Answering::Playing;

                // as many as the header shows: direction is not drawn while playing
                let controls: FocusRow = [
                    Some(FocusTarget::NoteAnswering),
                    (!playing).then_some(FocusTarget::DrillDirectionToggle),
                    Some(FocusTarget::PoolToggle),
                    Some(FocusTarget::NoteSpellingToggle),
                    Some(FocusTarget::SkipPrompt),
                ]
                .into_iter()
                .flatten()
                .map(Some)
                .collect();

                let mut grid = vec![back_row, controls];

                if playing {
                    let mut row: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                    row[0] = Some(FocusTarget::InputDevice);
                    grid.push(row);
                    return grid;
                }

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
                    // one cell; the cursor, not the ring, walks the neck's positions
                    Drill::FindIt => {
                        let mut row: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                        row[0] = Some(FocusTarget::Fretboard);
                        grid.push(row);
                    }
                }

                grid
            }
            Screen::IntervalTrainer => {
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
                    // one cell; the cursor, not the ring, walks the neck's positions
                    Drill::FindIt => {
                        let mut row: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                        row[0] = Some(FocusTarget::IntervalFretboard);
                        grid.push(row);
                    }
                }

                grid
            }
            // the voicing strip is browsed from the list, not a stop of its own
            Screen::ChordLibrary => vec![
                vec![Some(FocusTarget::Back), None, None, None],
                // beside the box, since anything under the list would be reachable by Tab alone
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

    /// The column bands the cards of a screen occupy, in the order Tab visits them, so Tab
    /// finishes one card before starting the next.
    #[expect(
        clippy::single_range_in_vec_init,
        reason = "a one-element Vec holding the full-width band, not a collected range"
    )]
    fn card_bands(&self, width: usize) -> Vec<Range<usize>> {
        match &self.screen {
            Screen::ScaleTrainer => vec![0..ROOT_ROW_WIDTH, ROOT_ROW_WIDTH..width],
            // single-card screens: one band spans the whole width
            Screen::Home
            | Screen::NoteTrainer
            | Screen::IntervalTrainer
            | Screen::PlayTheInterval
            | Screen::ChordLibrary
            | Screen::Caged
            | Screen::WalkTheScale
            | Screen::Tuner
            | Screen::PositionFinder => vec![0..width],
        }
    }

    /// Every focusable on a screen, card by card — the Tab order, from the arrows' grid.
    pub(super) fn focusables(&self) -> Vec<FocusTarget> {
        if self.screen == Screen::Home {
            return home::tab_order(self.focused);
        }

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
/// past it, which keeps a vertical move inside the card it started in.
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

    #[test]
    fn home_rings_every_category_and_only_the_open_one_s_entries() {
        assert_eq!(
            app_on(Screen::Home).focusables().len(),
            Category::ALL.len() + Category::Scales.entries().len(),
            "the menu and its focus ring disagree"
        );
    }

    #[test]
    fn scale_trainer_reaches_every_widget_exactly_once() {
        let targets = app_on(Screen::ScaleTrainer).focusables();
        // Back, and the summary card's two buttons
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
        // row 2 of the kinds card holds 6..8
        assert_eq!(
            arrow(FocusTarget::ScaleKind(5), Direction::Down),
            FocusTarget::ScaleKind(7)
        );
        // row 3 is as wide as row 2, so no clamp
        assert_eq!(
            arrow(FocusTarget::ScaleKind(7), Direction::Down),
            FocusTarget::ScaleKind(9)
        );
    }

    #[test]
    fn the_card_buttons_line_up_with_the_columns_below_them() {
        // the button row's third cell is empty, so `Root(2)` has nothing above it
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
        // the row's third cell is empty
        assert_eq!(
            arrow(FocusTarget::RerollScale, Direction::Right),
            FocusTarget::RerollScale
        );

        // Back is alone on its row
        assert_eq!(
            arrow(FocusTarget::Back, Direction::Right),
            FocusTarget::Back
        );
        assert_eq!(arrow(FocusTarget::Back, Direction::Left), FocusTarget::Back);
    }

    #[test]
    fn arrows_snap_stale_focus_back_onto_the_grid() {
        let grid = app_on(Screen::Home).focus_grid();
        assert_eq!(
            step_focus_2d(&grid, FocusTarget::Back, Direction::Down),
            FocusTarget::HomeCategory(Category::Scales)
        );
    }
}
