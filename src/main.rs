use iced::theme::palette::Palette;
use iced::{Color, Theme, application};

mod ui;
use ui::App;

mod music;

fn theme(_: &App) -> Theme {
    Theme::custom(
        "Black".to_owned(),
        Palette {
            background: Color::BLACK,
            text: Color::WHITE,
            primary: Color::from_rgb(0.5, 0.5, 1.0),
            success: Color::from_rgb(0.0, 0.8, 0.4),
            warning: Color::from_rgb(1.0, 0.7, 0.0),
            danger: Color::from_rgb(1.0, 0.3, 0.3),
        },
    )
}

fn main() -> iced::Result {
    application(App::new, App::update, App::view)
        .title("Trastea")
        .theme(theme)
        .subscription(App::subscription)
        .run()
}
