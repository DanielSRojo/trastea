use iced::{Element, Task};

#[derive(Default)]
pub struct App {
    pub screen: Screen,
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
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        (Self::default(), Task::none())
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(screen) => self.screen = screen,
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        match self.screen {
            Screen::Home => ui_home(),
            Screen::ScaleTrainer => ui_placeholder("Scale Trainer"),
            Screen::NoteTrainer => ui_placeholder("Note Trainer"),
            Screen::IntervalTrainer => ui_placeholder("Interval Trainer"),
        }
    }
}

fn ui_home() -> Element<'static, Message> {
    use iced::widget::{button, column, container, text};
    use iced::Length;

    container(
        column![
            text("Guitar Trainer").size(40),
            button("Scale Trainer").on_press(Message::Navigate(Screen::ScaleTrainer)),
            button("Note Trainer").on_press(Message::Navigate(Screen::NoteTrainer)),
            button("Interval Trainer").on_press(Message::Navigate(Screen::IntervalTrainer)),
        ]
        .spacing(20),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}


fn ui_placeholder(label: &str) -> Element<Message> {
    use iced::widget::{column, container, text};
    use iced::Length;

    container(
            column![text(label).size(30),]
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}


