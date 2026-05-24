mod fretboard;
use std::alloc::System;
use std::time::{SystemTime, UNIX_EPOCH};

use fretboard::{Fretboard, NoteMarker, fretboard};

use iced::{Color, Element, Subscription, Task, keyboard};
use keyboard::key::Named;

use crate::music::{notes::Note, scales::Scale, scales::ScaleFormula};

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

    let back_button = button(text("<"))
        .style(|_theme, _status| button::Style {
            background: None,
            text_color: Color::WHITE,
            border: Default::default(),
            shadow: Default::default(),
            snap: Default::default(),
        })
        .padding(0)
        .on_press(Message::GoBack);

    let header = if has_back {
        row![back_button, text(label).size(24)]
    } else {
        row![text(label).size(24)]
    }
    .spacing(16)
    .padding(16);

    container(column![header, content])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn ui_home() -> Element<'static, Message> {
    use iced::widget::{button, column};

    let menu = column![
        button("Scale Trainer").on_press(Message::Navigate(Screen::ScaleTrainer)),
        button("Note Trainer").on_press(Message::Navigate(Screen::NoteTrainer)),
        button("Interval Trainer").on_press(Message::Navigate(Screen::IntervalTrainer)),
    ]
    .spacing(12)
    .padding([0, 16]);

    with_top_bar("Trastea", menu.into(), false)
}

fn ui_scale_trainer(formula: ScaleFormula, root: Note) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{container, row, text};

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

    container(
        row![
            fretboard(fb),
            text(format!("{root} - {formula:?}")).size(24),
        ]
        .spacing(32),
    )
    .into()

    // container(row![fretboard(fb), text("Scale Trainer").size(24),])
    //     .width(Length::Fill)
    //     .height(Length::Fill)
    //     .center_y(Length::Fill)
    //     .into()
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

    container(column![text(label).size(30),])
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
