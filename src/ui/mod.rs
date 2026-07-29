mod fretboard;
use std::ops::Range;
use std::time::{SystemTime, UNIX_EPOCH};

use fretboard::{Fretboard, NoteMarker, fretboard};

use iced::{
    Background, Border, Color, Element, Padding, Shadow, Subscription, Task, Vector, font, keyboard,
};
use keyboard::key::Named;

use crate::music::{notes::Note, scales::Scale, scales::ScaleKind};

const INK: Color = Color::WHITE;
const BODY: Color = Color::from_rgb8(0xb5, 0xb5, 0xb5);
const MUTE: Color = Color::from_rgb8(0x77, 0x77, 0x77);
const HAIRLINE: Color = Color::from_rgb8(0x1f, 0x1f, 0x1f);
const CANVAS: Color = Color::BLACK;
const CANVAS_SOFT: Color = Color::from_rgb8(0x0a, 0x0a, 0x0a);
const CANVAS_SOFT_2: Color = Color::from_rgb8(0x11, 0x11, 0x11);
const LINK: Color = Color::from_rgb8(0x50, 0xa7, 0xff);
const SUMMARY_CARD_HEIGHT: f32 = 212.0;
const ROOT_SELECTOR_CARD_WIDTH: f32 = 320.0;
const SELECTOR_CARD_HEIGHT: f32 = 324.0;
const ROOT_BUTTON_SIZE: f32 = 50.0;
const SMUFL_FLAT: char = '\u{E260}';
const SMUFL_SHARP: char = '\u{E262}';
const FEEL_FONT: iced::Font = iced::Font {
    family: font::Family::Name("Dancing Script"),
    weight: font::Weight::Bold,
    ..iced::Font::DEFAULT
};
const MUSIC_FONT: iced::Font = iced::Font::with_name("Leland Text");

const STANDARD_TUNING: [Note; 6] = [Note::E, Note::A, Note::D, Note::G, Note::B, Note::E];

pub struct App {
    screen: Screen,
    history: Vec<Screen>,
    selected_scale_kind: ScaleKind,
    selected_root: Note,
    focused: FocusTarget,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub enum Screen {
    #[default]
    Home,
    ScaleTrainer,
    NoteTrainer,
    IntervalTrainer,
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Screen),
    GoBack,
    SelectRoot(Note),
    SelectScaleKind(ScaleKind),
    RerollScale,
    FocusNext,
    FocusPrevious,
    FocusUp,
    FocusDown,
    FocusLeft,
    FocusRight,
    ActivateFocused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    HomeMenuItem(usize),
    Back,
    RerollScale,
    Root(usize),
    ScaleKind(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// One row of a screen's focus grid. `None` is a cell with no widget in it, which
/// keeps the columns of neighbouring cards aligned.
type FocusRow = Vec<Option<FocusTarget>>;

const HOME_MENU_ITEMS: usize = 3;

/// Row shapes of the two selector grids on the scale trainer. Both the views and
/// the focus grid are built from these, so the two cannot drift out of sync.
const ROOT_ROW_WIDTH: usize = 3;
const KIND_ROW_WIDTHS: [usize; 5] = [4, 3, 2, 3, 4];

impl Default for App {
    fn default() -> Self {
        Self {
            screen: Screen::default(),
            history: Vec::new(),
            selected_scale_kind: ScaleKind::Ionian,
            selected_root: Note::C,
            focused: FocusTarget::HomeMenuItem(0),
        }
    }
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        (Self::default(), Task::none())
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(Screen::ScaleTrainer) => {
                self.navigate_to(Screen::ScaleTrainer);
                self.selected_scale_kind = random_scale_kind();
                self.selected_root = random_note();
            }
            Message::Navigate(screen) => self.navigate_to(screen),
            Message::GoBack => self.go_back(),
            Message::SelectRoot(root) => {
                self.selected_root = root;
            }
            Message::SelectScaleKind(kind) => {
                self.selected_scale_kind = kind;
            }
            Message::RerollScale => {
                self.selected_scale_kind = random_scale_kind();
                self.selected_root = random_note();
            }
            Message::FocusNext => self.cycle_focus(1),
            Message::FocusPrevious => self.cycle_focus(-1),
            Message::FocusUp => self.move_focus(Direction::Up),
            Message::FocusDown => self.move_focus(Direction::Down),
            Message::FocusLeft => self.move_focus(Direction::Left),
            Message::FocusRight => self.move_focus(Direction::Right),
            Message::ActivateFocused => self.activate_focused(),
        }
        Task::none()
    }

    fn navigate_to(&mut self, screen: Screen) {
        self.history.push(self.screen.clone());
        self.screen = screen;
        self.reset_focus();
    }

    fn go_back(&mut self) {
        if let Some(prev) = self.history.pop() {
            self.screen = prev;
            self.reset_focus();
        }
    }

    fn reset_focus(&mut self) {
        self.focused = focusables(&self.screen)
            .first()
            .copied()
            .unwrap_or(FocusTarget::Back);
    }

    /// Tab order: walks every focusable in reading order, wrapping at the ends.
    fn cycle_focus(&mut self, delta: isize) {
        let list = focusables(&self.screen);
        self.focused = step_focus(&list, self.focused, delta);
    }

    /// Arrow keys: moves one cell within the screen's focus grid, stopping at edges.
    fn move_focus(&mut self, direction: Direction) {
        let grid = focus_grid(&self.screen);
        self.focused = step_focus_2d(&grid, self.focused, direction);
    }

    fn activate_focused(&mut self) {
        match self.focused {
            FocusTarget::HomeMenuItem(0) => {
                self.navigate_to(Screen::ScaleTrainer);
                self.selected_scale_kind = random_scale_kind();
                self.selected_root = random_note();
            }
            FocusTarget::HomeMenuItem(1) => self.navigate_to(Screen::NoteTrainer),
            FocusTarget::HomeMenuItem(2) => self.navigate_to(Screen::IntervalTrainer),
            FocusTarget::HomeMenuItem(_) => {}
            FocusTarget::Back => self.go_back(),
            FocusTarget::RerollScale => {
                self.selected_scale_kind = random_scale_kind();
                self.selected_root = random_note();
            }
            FocusTarget::Root(index) => {
                if let Some(&note) = Note::ALL.get(index) {
                    self.selected_root = note;
                }
            }
            FocusTarget::ScaleKind(index) => {
                if let Some(&kind) = ScaleKind::ALL.get(index) {
                    self.selected_scale_kind = kind;
                }
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        match self.screen {
            Screen::Home => ui_home(self.focused),
            Screen::ScaleTrainer => with_top_bar(
                "Scale Trainer",
                ui_scale_trainer(self.selected_root, self.selected_scale_kind, self.focused),
                true,
                self.focused,
            ),
            Screen::NoteTrainer => with_top_bar(
                "Note Trainer",
                ui_placeholder("Note Trainer"),
                true,
                self.focused,
            ),
            Screen::IntervalTrainer => with_top_bar(
                "Interval Trainer",
                ui_placeholder("Interval Trainer"),
                true,
                self.focused,
            ),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        keyboard::listen().filter_map(|event| {
            let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
                return None;
            };
            translate_key(key, modifiers)
        })
    }
}

/// Yields the `(start, len)` of each row of the root selector, derived from
/// `Note::ALL` so adding a note reshapes the grid and the view together.
fn root_row_spans() -> impl Iterator<Item = (usize, usize)> {
    let total = Note::ALL.len();
    (0..total)
        .step_by(ROOT_ROW_WIDTH)
        .map(move |start| (start, ROOT_ROW_WIDTH.min(total - start)))
}

/// Yields the `(start, len)` of each row of the scale-kind selector. Unlike the
/// root grid these rows are ragged, so the widths are spelled out.
fn kind_row_spans() -> impl Iterator<Item = (usize, usize)> {
    KIND_ROW_WIDTHS.into_iter().scan(0, |start, len| {
        let span = (*start, len);
        *start += len;
        Some(span)
    })
}

/// The focusable widgets of a screen laid out as they appear on it.
///
/// Columns are shared across cards that sit side by side: on the scale trainer the
/// root card occupies columns `0..ROOT_ROW_WIDTH` and the scale-kind card the
/// columns after it, so pressing Right at the edge of the root grid steps into the
/// kinds grid. Cells the layout leaves empty are `None`, which is what stops a
/// vertical move at the bottom of the root card instead of dropping it into the
/// taller kinds card alongside.
fn focus_grid(screen: &Screen) -> Vec<FocusRow> {
    match screen {
        Screen::Home => (0..HOME_MENU_ITEMS)
            .map(|i| vec![Some(FocusTarget::HomeMenuItem(i))])
            .collect(),
        Screen::ScaleTrainer => {
            // The reroll button sits at the right edge of the summary card, which is
            // as wide as the root card below it, so it lines up with the last column.
            let mut top = vec![None; ROOT_ROW_WIDTH];
            top[0] = Some(FocusTarget::Back);
            top[ROOT_ROW_WIDTH - 1] = Some(FocusTarget::RerollScale);

            let root_rows: Vec<_> = root_row_spans().collect();
            let kind_rows: Vec<_> = kind_row_spans().collect();

            let mut grid = vec![top];
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
        Screen::NoteTrainer | Screen::IntervalTrainer => vec![vec![Some(FocusTarget::Back)]],
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
fn card_bands(screen: &Screen, width: usize) -> Vec<Range<usize>> {
    match screen {
        Screen::ScaleTrainer => vec![0..ROOT_ROW_WIDTH, ROOT_ROW_WIDTH..width],
        // These screens have a single card, so one band spans the whole width.
        Screen::Home | Screen::NoteTrainer | Screen::IntervalTrainer => vec![0..width],
    }
}

/// Every focusable on a screen, card by card — the Tab order.
///
/// Derived from the same grid the arrow keys use, so a widget can never be
/// reachable by one and not the other.
fn focusables(screen: &Screen) -> Vec<FocusTarget> {
    let grid = focus_grid(screen);
    let width = grid.iter().map(Vec::len).max().unwrap_or(0);

    let mut targets = Vec::new();
    for band in card_bands(screen, width) {
        for row in &grid {
            let cells = row.iter().skip(band.start).take(band.end - band.start);
            targets.extend(cells.flatten().copied());
        }
    }
    targets
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
fn step_focus_2d(grid: &[FocusRow], current: FocusTarget, direction: Direction) -> FocusTarget {
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

fn step_focus(list: &[FocusTarget], current: FocusTarget, delta: isize) -> FocusTarget {
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

fn translate_key(key: keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Message> {
    match key.as_ref() {
        keyboard::Key::Named(Named::Escape) => Some(Message::GoBack),
        keyboard::Key::Named(Named::Tab) if modifiers.shift() => Some(Message::FocusPrevious),
        keyboard::Key::Named(Named::Tab) => Some(Message::FocusNext),
        keyboard::Key::Named(Named::Enter) => Some(Message::ActivateFocused),
        keyboard::Key::Named(Named::Space) => Some(Message::ActivateFocused),
        keyboard::Key::Named(Named::ArrowUp) => Some(Message::FocusUp),
        keyboard::Key::Named(Named::ArrowDown) => Some(Message::FocusDown),
        keyboard::Key::Named(Named::ArrowLeft) => Some(Message::FocusLeft),
        keyboard::Key::Named(Named::ArrowRight) => Some(Message::FocusRight),
        keyboard::Key::Character(c) if modifiers.is_empty() => vim_motion(c),
        _ => None,
    }
}

/// Maps the vim motion keys onto the same focus moves the arrow keys make.
fn vim_motion(c: &str) -> Option<Message> {
    match c {
        "h" => Some(Message::FocusLeft),
        "j" => Some(Message::FocusDown),
        "k" => Some(Message::FocusUp),
        "l" => Some(Message::FocusRight),
        _ => None,
    }
}

fn with_top_bar(
    label: &'static str,
    content: Element<'static, Message>,
    has_back: bool,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{button, column, container, row, text};

    let back_button = focus_ring(
        button(text("←").size(18))
            .style(ghost_button)
            .padding([6, 12])
            .on_press(Message::GoBack),
        focused == FocusTarget::Back,
    );

    let page = if has_back {
        let header = row![back_button, text(label).size(24).color(INK)]
            .spacing(16)
            .padding([18, 32]);

        column![header, content].spacing(12)
    } else {
        column![content]
    };

    container(page)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(page_container)
        .into()
}

fn focus_ring<'a>(
    element: impl Into<Element<'a, Message>>,
    is_focused: bool,
) -> Element<'a, Message> {
    use iced::widget::container;

    container(element)
        .padding(3)
        .style(move |_theme: &iced::Theme| focus_ring_style(is_focused))
        .into()
}

fn focus_ring_style(is_focused: bool) -> iced::widget::container::Style {
    let color = if is_focused { LINK } else { Color::TRANSPARENT };

    iced::widget::container::Style {
        text_color: None,
        background: None,
        border: Border::default().rounded(10).width(2).color(color),
        shadow: Shadow::default(),
        snap: true,
    }
}

fn ui_home(focused: FocusTarget) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{column, container, row, text};

    let menu = column![
        focus_ring(
            trainer_button("Scale Trainer", "Explore and learn guitar scales")
                .on_press(Message::Navigate(Screen::ScaleTrainer)),
            focused == FocusTarget::HomeMenuItem(0),
        ),
        focus_ring(
            trainer_button("Note Trainer", "Build fretboard recall one pitch at a time")
                .on_press(Message::Navigate(Screen::NoteTrainer)),
            focused == FocusTarget::HomeMenuItem(1),
        ),
        focus_ring(
            trainer_button(
                "Interval Trainer",
                "Recognize distances from a tonal center"
            )
            .on_press(Message::Navigate(Screen::IntervalTrainer)),
            focused == FocusTarget::HomeMenuItem(2),
        ),
    ]
    .spacing(12);

    let hero = column![
        text("Trastea").size(56).color(INK),
        text("A focused guitar trainer for scales, intervals, and fretboard fluency.")
            .size(21)
            .color(BODY),
        row![
            // text("α").size(13).color(CANVAS),
            // text("desktop practice lab").size(13).color(INK)
        ]
        .spacing(8)
        .padding([6, 12])
    ]
    .spacing(16);

    let content = container(row![hero, menu].spacing(64))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([48, 64])
        .center_y(Length::Fill);

    with_top_bar("Trastea", content.into(), false, focused)
}

fn ui_scale_trainer(
    root: Note,
    kind: ScaleKind,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, button, column, container, row, text};

    let fb = Fretboard {
        num_frets: 12,
        highlighted: scale_markers(root, kind),
    };

    let current_scale_card = container(
        column![
            row![
                note_label(root, 56, INK),
                Space::new().width(Length::Fill),
                focus_ring(
                    button(text("R").size(20))
                        .padding([8, 12])
                        .style(ghost_button)
                        .on_press(Message::RerollScale),
                    focused == FocusTarget::RerollScale,
                ),
            ],
            text(kind.name()).size(34).color(INK),
            intervalic_text(kind.intervalic()),
        ]
        .spacing(10),
    )
    .width(Length::Fixed(ROOT_SELECTOR_CARD_WIDTH))
    .height(Length::Fixed(SUMMARY_CARD_HEIGHT))
    .padding(32)
    .style(card_container);

    let root_selector_content = container(root_row_spans().fold(
        column![].spacing(16),
        |rows, (start, len)| {
            rows.push(
                container(root_note_row(
                    &Note::ALL[start..start + len],
                    root,
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
        .width(Length::Fixed(ROOT_SELECTOR_CARD_WIDTH))
        .height(Length::Fixed(SELECTOR_CARD_HEIGHT))
        .padding(32)
        .style(card_container);

    let scale_selector_content = container(kind_row_spans().fold(
        column![].spacing(12),
        |rows, (start, len)| {
            rows.push(scale_kind_row(
                &ScaleKind::ALL[start..start + len],
                kind,
                start,
                focused,
            ))
        },
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .center_y(Length::Fill);

    let scale_selector_card = container(scale_selector_content)
        .width(Length::Fill)
        .height(Length::Fixed(SELECTOR_CARD_HEIGHT))
        .padding(32)
        .style(card_container);

    let explanation_font = iced::Font {
        family: font::Family::Cursive,
        style: font::Style::Italic,
        ..iced::Font::DEFAULT
    };

    let explanation_card = container(
        column![
            text(kind.feel())
                .size(22)
                .font(FEEL_FONT)
                .color(BODY)
                .width(Length::Fill),
            text(kind.common_usage())
                .size(18)
                .font(explanation_font)
                .color(MUTE)
                .width(Length::Fill),
        ]
        .spacing(12),
    )
    .width(Length::Fill)
    .height(Length::Fixed(SUMMARY_CARD_HEIGHT))
    .padding(32)
    .style(card_container);

    let summary_cards = row![current_scale_card, explanation_card]
        .width(Length::Fill)
        .spacing(16);

    let selector_cards = row![root_selector_card, scale_selector_card]
        .width(Length::Fill)
        .spacing(16);

    let details = column![summary_cards, selector_cards]
        .width(Length::Fill)
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
        .center_y(Length::Fill)
        .into()
}

fn root_note_row(
    notes: &[Note],
    selected: Note,
    start_index: usize,
    focused: FocusTarget,
) -> iced::widget::Row<'static, Message> {
    use iced::Length;
    use iced::widget::{button, container, row};

    notes
        .iter()
        .enumerate()
        .fold(row![].spacing(28), |row, (i, note)| {
            let color = if *note == selected { CANVAS } else { INK };

            let root_button = button(
                container(note_label(*note, 24, color))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            )
            .width(Length::Fixed(ROOT_BUTTON_SIZE))
            .height(Length::Fixed(ROOT_BUTTON_SIZE))
            .padding(0)
            .style(if *note == selected {
                selected_root_button
            } else {
                ghost_button
            })
            .on_press(Message::SelectRoot(*note));

            row.push(focus_ring(
                container(root_button)
                    .width(Length::Fixed(ROOT_BUTTON_SIZE))
                    .height(Length::Fixed(ROOT_BUTTON_SIZE))
                    .center_x(Length::Fixed(ROOT_BUTTON_SIZE))
                    .center_y(Length::Fixed(ROOT_BUTTON_SIZE)),
                focused == FocusTarget::Root(start_index + i),
            ))
        })
}

fn note_label(note: Note, size: u32, color: Color) -> iced::widget::Row<'static, Message> {
    use iced::widget::{row, text};

    let label = note.to_string();
    let mut chars = label.chars();
    let letter = chars.next().unwrap_or_default().to_string();
    let label = row![text(letter).size(size).color(color)].spacing(0);

    if chars.next().is_some() {
        label.push(
            text(SMUFL_SHARP.to_string())
                .size(size)
                .font(MUSIC_FONT)
                .color(color),
        )
    } else {
        label
    }
}

fn intervalic_text(formula: &'static str) -> iced::widget::Row<'static, Message> {
    use iced::widget::{row, text};

    formula.chars().fold(row![].spacing(0), |row, ch| {
        let text = text(ch.to_string()).size(24).color(BODY);

        if ch == SMUFL_FLAT || ch == SMUFL_SHARP {
            row.push(text.font(MUSIC_FONT))
        } else {
            row.push(text)
        }
    })
}

fn scale_kind_row(
    kinds: &[ScaleKind],
    selected: ScaleKind,
    start_index: usize,
    focused: FocusTarget,
) -> iced::widget::Row<'static, Message> {
    use iced::widget::{button, row, text};

    kinds
        .iter()
        .enumerate()
        .fold(row![].spacing(8), |row, (i, kind)| {
            row.push(focus_ring(
                button(text(kind.name()).size(16))
                    .padding([8, 12])
                    .style(if *kind == selected {
                        selected_root_button
                    } else {
                        ghost_button
                    })
                    .on_press(Message::SelectScaleKind(*kind)),
                focused == FocusTarget::ScaleKind(start_index + i),
            ))
        })
}

fn scale_markers(root: Note, kind: ScaleKind) -> Vec<NoteMarker> {
    let scale = Scale { root, kind };
    let scale_notes = scale.notes();

    let mut markers = Vec::new();

    for (string, open_note) in STANDARD_TUNING.iter().enumerate() {
        for fret in 0_u8..=12 {
            let note = open_note.transpose(fret);

            if scale_notes.contains(&note) {
                markers.push(NoteMarker {
                    string,
                    fret: fret as usize,
                    note,
                    color: if note == root {
                        Color::from_rgb8(0xff, 0x4d, 0x4d)
                    } else {
                        LINK
                    },
                });
            }
        }
    }
    markers
}

fn random_scale_kind() -> ScaleKind {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let seed = duration.as_secs() as usize ^ duration.subsec_nanos() as usize;

    ScaleKind::ALL[seed % ScaleKind::ALL.len()]
}

fn random_note() -> Note {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let seed = duration.as_secs() as usize ^ duration.subsec_nanos() as usize;

    Note::ALL[seed % Note::ALL.len()]
}

fn ui_placeholder(label: &str) -> Element<'_, Message> {
    use iced::Length;
    use iced::widget::{column, container, text};

    container(
        column![
            text(label).size(36).color(INK),
            text("Coming soon").size(17).color(MUTE),
        ]
        .spacing(8),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(page_container)
    .into()
}

fn trainer_button<'a>(
    title: &'static str,
    caption: &'static str,
) -> iced::widget::Button<'a, Message> {
    use iced::widget::{button, column, text};

    button(
        column![
            text(title).size(19).color(INK),
            text(caption).size(16).color(BODY)
        ]
        .spacing(4),
    )
    .width(360)
    .padding([16, 20])
    .style(card_button)
}

fn page_container(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        text_color: Some(INK),
        background: Some(Background::Color(CANVAS)),
        border: Border::default(),
        shadow: Shadow::default(),
        snap: true,
    }
}

fn card_container(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        text_color: Some(INK),
        background: Some(Background::Color(CANVAS_SOFT)),
        border: Border::default().rounded(12).width(1).color(HAIRLINE),
        shadow: Shadow {
            color: Color {
                a: 0.45,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 12.0),
            blur_radius: 32.0,
        },
        snap: true,
    }
}

fn card_button(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let border_color = match status {
        iced::widget::button::Status::Hovered => LINK,
        _ => HAIRLINE,
    };

    iced::widget::button::Style {
        background: Some(Background::Color(CANVAS_SOFT)),
        text_color: INK,
        border: Border::default().rounded(12).width(1).color(border_color),
        shadow: Shadow {
            color: Color {
                a: 0.40,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
        snap: true,
    }
}

fn ghost_button(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let background = match status {
        iced::widget::button::Status::Hovered => CANVAS_SOFT_2,
        _ => CANVAS,
    };

    iced::widget::button::Style {
        background: Some(Background::Color(background)),
        text_color: INK,
        border: Border::default().rounded(64).width(1).color(HAIRLINE),
        shadow: Shadow::default(),
        snap: true,
    }
}

fn selected_root_button(
    _theme: &iced::Theme,
    _status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let accent = Color::from_rgb8(0x50, 0xe3, 0xc2);

    iced::widget::button::Style {
        background: Some(Background::Color(accent)),
        text_color: CANVAS,
        border: Border::default().rounded(64).width(1).color(accent),
        shadow: Shadow::default(),
        snap: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_has_three_focusables() {
        assert_eq!(focusables(&Screen::Home).len(), 3);
    }

    #[test]
    fn scale_trainer_reaches_every_widget_exactly_once() {
        let targets = focusables(&Screen::ScaleTrainer);
        assert_eq!(targets.len(), 2 + Note::ALL.len() + ScaleKind::ALL.len());

        for i in 0..Note::ALL.len() {
            assert!(targets.contains(&FocusTarget::Root(i)), "missing root {i}");
        }
        for i in 0..ScaleKind::ALL.len() {
            assert!(
                targets.contains(&FocusTarget::ScaleKind(i)),
                "missing kind {i}"
            );
        }
    }

    #[test]
    fn tab_walks_one_card_at_a_time() {
        let mut expected = vec![FocusTarget::Back, FocusTarget::RerollScale];
        expected.extend((0..Note::ALL.len()).map(FocusTarget::Root));
        expected.extend((0..ScaleKind::ALL.len()).map(FocusTarget::ScaleKind));

        // Every root before any kind — not C, C#, D, Ionian, Dorian, ...
        assert_eq!(focusables(&Screen::ScaleTrainer), expected);
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
            let mut from_tab = focusables(&screen);
            let mut in_grid: Vec<_> = focus_grid(&screen)
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
    fn placeholder_screens_only_focus_back() {
        assert_eq!(focusables(&Screen::NoteTrainer), vec![FocusTarget::Back]);
        assert_eq!(
            focusables(&Screen::IntervalTrainer),
            vec![FocusTarget::Back]
        );
    }

    #[test]
    fn selector_row_spans_cover_their_arrays() {
        let roots: Vec<_> = root_row_spans().collect();
        assert_eq!(roots, vec![(0, 3), (3, 3), (6, 3), (9, 3)]);

        let kinds: Vec<_> = kind_row_spans().collect();
        assert_eq!(kinds, vec![(0, 4), (4, 3), (7, 2), (9, 3), (12, 4)]);

        let covered: usize = kinds.iter().map(|&(_, len)| len).sum();
        assert_eq!(covered, ScaleKind::ALL.len());
    }

    #[test]
    fn step_focus_wraps_forward_past_end() {
        let list = focusables(&Screen::Home);
        let last = *list.last().unwrap();
        assert_eq!(step_focus(&list, last, 1), list[0]);
    }

    #[test]
    fn step_focus_wraps_backward_past_start() {
        let list = focusables(&Screen::Home);
        let last = *list.last().unwrap();
        assert_eq!(step_focus(&list, list[0], -1), last);
    }

    #[test]
    fn step_focus_snaps_to_first_when_target_absent() {
        let list = focusables(&Screen::Home);
        // Back is not focusable on Home, so a stale focus snaps to the first item.
        assert_eq!(step_focus(&list, FocusTarget::Back, 1), list[0]);
    }

    /// Presses one arrow key on the scale trainer.
    fn arrow(from: FocusTarget, direction: Direction) -> FocusTarget {
        step_focus_2d(&focus_grid(&Screen::ScaleTrainer), from, direction)
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

        // ScaleKind(3) ends the widest kind row, so nothing is to its right.
        assert_eq!(
            arrow(FocusTarget::ScaleKind(3), Direction::Right),
            FocusTarget::ScaleKind(3)
        );
    }

    #[test]
    fn down_past_the_root_grid_does_not_jump_into_the_taller_kinds_card() {
        // The kinds card has a fifth row, the root card does not. Leaving the last
        // root row must stay put rather than teleporting across to ScaleKind(12).
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
        // Row 1 of the kinds card holds 3 items (indices 4..7), so leaving the 4-wide
        // row above it from its last column lands on that row's last item.
        assert_eq!(
            arrow(FocusTarget::ScaleKind(3), Direction::Down),
            FocusTarget::ScaleKind(6)
        );
        // Row 2 is narrower still (indices 7..9).
        assert_eq!(
            arrow(FocusTarget::ScaleKind(6), Direction::Down),
            FocusTarget::ScaleKind(8)
        );
    }

    #[test]
    fn top_row_lines_up_with_the_columns_below_it() {
        assert_eq!(
            arrow(FocusTarget::Back, Direction::Down),
            FocusTarget::Root(0)
        );
        assert_eq!(
            arrow(FocusTarget::Root(0), Direction::Up),
            FocusTarget::Back
        );

        // Reroll sits at the right edge of the summary card, above the third root.
        assert_eq!(
            arrow(FocusTarget::RerollScale, Direction::Down),
            FocusTarget::Root(2)
        );
        assert_eq!(
            arrow(FocusTarget::Root(2), Direction::Up),
            FocusTarget::RerollScale
        );

        // Right from Back skips the empty middle cell.
        assert_eq!(
            arrow(FocusTarget::Back, Direction::Right),
            FocusTarget::RerollScale
        );
    }

    #[test]
    fn home_menu_is_vertical_only() {
        let grid = focus_grid(&Screen::Home);
        let first = FocusTarget::HomeMenuItem(0);

        assert_eq!(
            step_focus_2d(&grid, first, Direction::Down),
            FocusTarget::HomeMenuItem(1)
        );
        assert_eq!(step_focus_2d(&grid, first, Direction::Up), first);
        assert_eq!(step_focus_2d(&grid, first, Direction::Right), first);
        assert_eq!(step_focus_2d(&grid, first, Direction::Left), first);
    }

    /// Presses a character key with the given modifiers held.
    fn press(c: &str, modifiers: keyboard::Modifiers) -> Option<Message> {
        translate_key(keyboard::Key::Character(c.into()), modifiers)
    }

    #[test]
    fn vim_motions_move_the_focus_ring() {
        let none = keyboard::Modifiers::empty();

        assert!(matches!(press("h", none), Some(Message::FocusLeft)));
        assert!(matches!(press("j", none), Some(Message::FocusDown)));
        assert!(matches!(press("k", none), Some(Message::FocusUp)));
        assert!(matches!(press("l", none), Some(Message::FocusRight)));
    }

    #[test]
    fn modified_vim_letters_are_not_motions() {
        for modifiers in [
            keyboard::Modifiers::LOGO,
            keyboard::Modifiers::CTRL,
            keyboard::Modifiers::ALT,
        ] {
            assert!(press("h", modifiers).is_none(), "{modifiers:?}+h");
        }
    }

    #[test]
    fn capital_vim_letters_are_unbound() {
        assert!(press("h", keyboard::Modifiers::SHIFT).is_none());
    }

    #[test]
    fn unbound_letters_are_ignored() {
        assert!(press("x", keyboard::Modifiers::empty()).is_none());
    }

    #[test]
    fn adding_vim_motions_did_not_shadow_the_named_keys() {
        let escape = keyboard::Key::Named(Named::Escape);
        let modifiers = keyboard::Modifiers::empty();

        assert!(matches!(
            translate_key(escape, modifiers),
            Some(Message::GoBack)
        ));
    }

    #[test]
    fn arrows_snap_stale_focus_back_onto_the_grid() {
        let grid = focus_grid(&Screen::Home);
        assert_eq!(
            step_focus_2d(&grid, FocusTarget::Back, Direction::Down),
            FocusTarget::HomeMenuItem(0)
        );
    }
}
