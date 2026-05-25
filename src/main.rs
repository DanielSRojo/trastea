use iced::theme::palette::Palette;
use iced::{Color, Theme, application};

mod ui;
use ui::App;

mod music;

fn theme(_: &App) -> Theme {
    Theme::custom(
        "Trastea Black".to_owned(),
        Palette {
            background: Color::BLACK,
            text: Color::WHITE,
            primary: Color::WHITE,
            success: Color::from_rgb8(0x50, 0xe3, 0xc2),
            warning: Color::from_rgb8(0xf5, 0xa6, 0x23),
            danger: Color::from_rgb8(0xff, 0x4d, 0x4d),
        },
    )
}

fn main() -> iced::Result {
    application(App::new, App::update, App::view)
        .title("Trastea")
        .font(include_bytes!("../assets/fonts/DancingScript-Regular.ttf").as_slice())
        .font(include_bytes!("../assets/fonts/DancingScript-Bold.ttf").as_slice())
        .theme(theme)
        .subscription(App::subscription)
        .run()
}
