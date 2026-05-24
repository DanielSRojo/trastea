mod fretboard;
use std::time::{SystemTime, UNIX_EPOCH};

use fretboard::{Fretboard, NoteMarker, fretboard};

use iced::{Background, Border, Color, Element, Shadow, Subscription, Task, Vector, keyboard};
use keyboard::key::Named;

use crate::music::{notes::Note, scales::ScaleFormula};

const INK: Color = Color::WHITE;
const BODY: Color = Color::from_rgb8(0xb5, 0xb5, 0xb5);
const MUTE: Color = Color::from_rgb8(0x77, 0x77, 0x77);
const HAIRLINE: Color = Color::from_rgb8(0x1f, 0x1f, 0x1f);
const CANVAS: Color = Color::BLACK;
const CANVAS_SOFT: Color = Color::from_rgb8(0x0a, 0x0a, 0x0a);
const CANVAS_SOFT_2: Color = Color::from_rgb8(0x11, 0x11, 0x11);
const LINK: Color = Color::from_rgb8(0x50, 0xa7, 0xff);

pub struct App {
    screen: Screen,
    history: Vec<Screen>,
    selected_scale_formula: ScaleFormula,
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
}

impl Default for App {
    fn default() -> Self {
        Self {
            screen: Screen::default(),
            history: Vec::new(),
            selected_scale_formula: ScaleFormula::Ionian,
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
                self.selected_scale_formula = random_scale_formula();
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
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        match self.screen {
            Screen::Home => ui_home(),
            Screen::ScaleTrainer => with_top_bar(
                "Scale Trainer",
                ui_scale_trainer(self.selected_scale_formula, self.selected_root),
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

    let back_button = button(text("←").size(16))
        .style(ghost_button)
        .padding([6, 12])
        .on_press(Message::GoBack);

    let header = if has_back {
        row![back_button, text(label).size(20).color(INK)]
    } else {
        row![text(label).size(20).color(INK)]
    }
    .spacing(16)
    .padding([18, 32]);

    container(column![header, content].spacing(12))
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
        trainer_button("Interval Trainer", "Recognize distances from a tonal center")
            .on_press(Message::Navigate(Screen::IntervalTrainer)),
    ]
    .spacing(12);

    let hero = column![
        text("Trastea").size(48).color(INK),
        text("A focused guitar trainer for scales, intervals, and fretboard fluency.")
            .size(18)
            .color(BODY),
        row![text("α").size(13).color(CANVAS), text("desktop practice lab").size(13).color(INK)]
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

fn ui_scale_trainer(formula: ScaleFormula, root: Note) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{column, container, row, text};

    let fb = Fretboard {
        num_frets: 12,
        highlighted: vec![
            NoteMarker {
                string: 0,
                fret: 0,
                note: Note::E,
                color: Color::from_rgb(0.2, 0.6, 1.0),
            },
            NoteMarker {
                string: 1,
                fret: 2,
                note: Note::Fs,
                color: Color::from_rgb(0.2, 0.6, 1.0),
            },
            NoteMarker {
                string: 2,
                fret: 2,
                note: Note::B,
                color: Color::from_rgb(0.2, 0.6, 1.0),
            },
            NoteMarker {
                string: 3,
                fret: 2,
                note: Note::E,
                color: Color::from_rgb(1.0, 0.4, 0.2),
            },
            NoteMarker {
                string: 4,
                fret: 0,
                note: Note::B,
                color: Color::from_rgb(0.2, 0.6, 1.0),
            },
            NoteMarker {
                string: 5,
                fret: 0,
                note: Note::E,
                color: Color::from_rgb(1.0, 0.4, 0.2),
            },
        ],
    };

    let details = container(
        column![
            text("today's scale").size(12).color(MUTE),
            text(root.to_string()).size(48).color(INK),
            text(format!("{formula:?}")).size(28).color(INK),
            text("Press Esc to return home. Re-enter Scale Trainer for a new random prompt.")
                .size(14)
                .color(BODY),
        ]
        .spacing(12),
    )
    .width(Length::Fill)
    .padding(32)
    .style(card_container);

    container(row![fretboard(fb), details].spacing(32))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([48, 64])
        .center_y(Length::Fill)
    .into()
}

fn random_scale_formula() -> ScaleFormula {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let seed = duration.as_secs() as usize ^ duration.subsec_nanos() as usize;

    ScaleFormula::ALL[seed % ScaleFormula::ALL.len()]
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

    container(column![text(label).size(30).color(INK), text("Coming soon").size(14).color(MUTE),].spacing(8))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(page_container)
        .into()
}

fn trainer_button<'a>(title: &'static str, caption: &'static str) -> iced::widget::Button<'a, Message> {
    use iced::widget::{button, column, text};

    button(column![text(title).size(16).color(INK), text(caption).size(13).color(BODY)].spacing(4))
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
            color: Color { a: 0.45, ..Color::BLACK },
            offset: Vector::new(0.0, 12.0),
            blur_radius: 32.0,
        },
        snap: true,
    }
}

fn card_button(_theme: &iced::Theme, status: iced::widget::button::Status) -> iced::widget::button::Style {
    let border_color = match status {
        iced::widget::button::Status::Hovered => LINK,
        _ => HAIRLINE,
    };

    iced::widget::button::Style {
        background: Some(Background::Color(CANVAS_SOFT)),
        text_color: INK,
        border: Border::default().rounded(12).width(1).color(border_color),
        shadow: Shadow {
            color: Color { a: 0.40, ..Color::BLACK },
            offset: Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
        snap: true,
    }
}

fn ghost_button(_theme: &iced::Theme, status: iced::widget::button::Status) -> iced::widget::button::Style {
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
