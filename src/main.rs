use iced::{application, Theme};

mod ui;
use ui::App;

fn main() -> iced::Result {
    application(App::new, App::update, App::view)
        .title("Trastea")
        .theme(|_: &App| Theme::CatppuccinMocha)
        .run()
}
