mod fretboard;
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
const SUMMARY_CARD_HEIGHT: f32 = 180.0;
const ROOT_SELECTOR_CARD_WIDTH: f32 = 320.0;
const SELECTOR_CARD_HEIGHT: f32 = 324.0;
const EXPLANATION_CARD_HEIGHT: f32 = 188.0;
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
}

impl Default for App {
    fn default() -> Self {
        Self {
            screen: Screen::default(),
            history: Vec::new(),
            selected_scale_kind: ScaleKind::Ionian,
            selected_root: Note::C,
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
                self.history.push(self.screen.clone());
                self.screen = Screen::ScaleTrainer;
                self.selected_scale_kind = random_scale_kind();
                self.selected_root = random_note();
            }
            Message::Navigate(screen) => {
                self.history.push(self.screen.clone());
                self.screen = screen;
            }
            Message::GoBack => {
                if let Some(prev) = self.history.pop() {
                    self.screen = prev;
                }
            }
            Message::SelectRoot(root) => {
                self.selected_root = root;
            }
            Message::SelectScaleKind(kind) => {
                self.selected_scale_kind = kind;
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        match self.screen {
            Screen::Home => ui_home(),
            Screen::ScaleTrainer => with_top_bar(
                "Scale Trainer",
                ui_scale_trainer(self.selected_root, self.selected_scale_kind),
                true,
            ),
            Screen::NoteTrainer => {
                with_top_bar("Note Trainer", ui_placeholder("Note Trainer"), true)
            }
            Screen::IntervalTrainer => {
                with_top_bar("Interval Trainer", ui_placeholder("Interval Trainer"), true)
            }
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        keyboard::listen().filter_map(|event| {
            let keyboard::Event::KeyPressed { key, .. } = event else {
                return None;
            };
            if matches!(key.as_ref(), keyboard::Key::Named(Named::Escape)) {
                Some(Message::GoBack)
            } else {
                None
            }
        })
    }
}

fn with_top_bar(
    label: &'static str,
    content: Element<'static, Message>,
    has_back: bool,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{button, column, container, row, text};

    let back_button = button(text("←").size(18))
        .style(ghost_button)
        .padding([6, 12])
        .on_press(Message::GoBack);

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

fn ui_home() -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{column, container, row, text};

    let menu = column![
        trainer_button("Scale Trainer", "Explore a random key and scale formula")
            .on_press(Message::Navigate(Screen::ScaleTrainer)),
        trainer_button("Note Trainer", "Build fretboard recall one pitch at a time")
            .on_press(Message::Navigate(Screen::NoteTrainer)),
        trainer_button(
            "Interval Trainer",
            "Recognize distances from a tonal center"
        )
        .on_press(Message::Navigate(Screen::IntervalTrainer)),
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

    with_top_bar("Trastea", content.into(), false)
}

fn ui_scale_trainer(root: Note, kind: ScaleKind) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{column, container, row, text};

    let fb = Fretboard {
        num_frets: 12,
        highlighted: scale_markers(root, kind),
    };

    let current_scale_card = container(
        column![
            note_label(root, 56, INK),
            text(kind.name()).size(34).color(INK),
            intervalic_text(kind.intervalic()),
        ]
        .spacing(8),
    )
    .width(Length::Fill)
    .height(Length::Fixed(SUMMARY_CARD_HEIGHT))
    .padding(32)
    .style(card_container);

    let root_selector_content = container(
        column![
            container(root_note_row(&Note::ALL[..3], root))
                .width(Length::Fill)
                .center_x(Length::Fill),
            container(root_note_row(&Note::ALL[3..6], root))
                .width(Length::Fill)
                .center_x(Length::Fill),
            container(root_note_row(&Note::ALL[6..9], root))
                .width(Length::Fill)
                .center_x(Length::Fill),
            container(root_note_row(&Note::ALL[9..], root))
                .width(Length::Fill)
                .center_x(Length::Fill),
        ]
        .spacing(16),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill);

    let root_selector_card = container(root_selector_content)
    .width(Length::Fixed(ROOT_SELECTOR_CARD_WIDTH))
    .height(Length::Fixed(SELECTOR_CARD_HEIGHT))
    .padding(32)
    .style(card_container);

    let scale_selector_content = container(
        column![
            scale_kind_row(&ScaleKind::ALL[..4], kind),
            scale_kind_row(&ScaleKind::ALL[4..7], kind),
            scale_kind_row(&ScaleKind::ALL[7..9], kind),
            scale_kind_row(&ScaleKind::ALL[9..12], kind),
            scale_kind_row(&ScaleKind::ALL[12..], kind),
        ]
        .spacing(12),
    )
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
    .height(Length::Fixed(EXPLANATION_CARD_HEIGHT))
    .padding(32)
    .style(card_container);

    let selector_cards = row![root_selector_card, scale_selector_card]
        .width(Length::Fill)
        .spacing(16);

    let details = column![current_scale_card, selector_cards, explanation_card]
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

fn root_note_row(notes: &[Note], selected: Note) -> iced::widget::Row<'static, Message> {
    use iced::widget::{button, row};

    notes.iter().fold(row![].spacing(16), |row, note| {
        let color = if *note == selected { CANVAS } else { INK };

        row.push(
            button(note_label(*note, 28, color))
                .padding([8, 12])
                .style(if *note == selected {
                    selected_root_button
                } else {
                    ghost_button
                })
                .on_press(Message::SelectRoot(*note)),
        )
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

fn scale_kind_row(kinds: &[ScaleKind], selected: ScaleKind) -> iced::widget::Row<'static, Message> {
    use iced::widget::{button, row, text};

    kinds.iter().fold(row![].spacing(8), |row, kind| {
        row.push(
            button(text(kind.name()).size(15))
                .padding([8, 12])
                .style(if *kind == selected {
                    selected_root_button
                } else {
                    ghost_button
                })
                .on_press(Message::SelectScaleKind(*kind)),
        )
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
