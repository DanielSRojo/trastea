//! The palette, fonts, glyphs, and the control and container styles every screen builds from.

use iced::widget::canvas;
use iced::{Background, Border, Color, Element, Padding, Pixels, Shadow, Vector, font};

use crate::music::notes::Spelling;

use super::Message;

pub(super) const INK: Color = Color::WHITE;
pub(super) const BODY: Color = Color::from_rgb8(0xb5, 0xb5, 0xb5);
pub(super) const MUTE: Color = Color::from_rgb8(0x77, 0x77, 0x77);
const HAIRLINE: Color = Color::from_rgb8(0x1f, 0x1f, 0x1f);
/// Brighter than `HAIRLINE`, for rules between groups rather than edges.
pub(super) const HAIRLINE_INK: Color = Color::from_rgb8(0x33, 0x33, 0x33);
pub(super) const CANVAS: Color = Color::BLACK;
pub(super) const CANVAS_SOFT: Color = Color::from_rgb8(0x0a, 0x0a, 0x0a);
pub(super) const CANVAS_SOFT_2: Color = Color::from_rgb8(0x11, 0x11, 0x11);
pub(super) const LINK: Color = Color::from_rgb8(0x50, 0xa7, 0xff);
/// Kept in step with `theme()` in `main.rs`, for code with no `&Theme` in hand.
pub(super) const DANGER: Color = Color::from_rgb8(0xff, 0x4d, 0x4d);
pub(super) const SUCCESS: Color = Color::from_rgb8(0x50, 0xe3, 0xc2);
/// The Interval Trainer's tonal center, in a colour no other mark on that neck uses.
pub(super) const ROOT_MARKER: Color = Color::from_rgb8(0xff, 0xb3, 0x4d);
pub(super) const SMUFL_FLAT: char = '\u{E260}';
pub(super) const SMUFL_SHARP: char = '\u{E262}';
pub(super) const SMUFL_DOUBLE_SHARP: char = '\u{E263}';
pub(super) const SMUFL_DOUBLE_FLAT: char = '\u{E264}';
/// SMuFL's chord-symbol range, present in the embedded Leland subset.
pub(super) const SMUFL_CSYM_DIMINISHED: char = '\u{E870}';
pub(super) const SMUFL_CSYM_HALF_DIMINISHED: char = '\u{E871}';
pub(super) const SMUFL_CSYM_AUGMENTED: char = '\u{E872}';
pub(super) const SMUFL_CSYM_MAJOR_SEVENTH: char = '\u{E873}';
/// Every arrow the chrome draws, all from `KEYCAP_FONT` since most text faces lack the tab pair.
pub(super) const KEYCAP_TAB: &str = "⇥";
pub(super) const KEYCAP_BACKTAB: &str = "⇤";
pub(super) const KEYCAP_ARROWS: &str = "↑ ↓ ← →";
pub(super) const KEYCAP_BACK: &str = "←";
pub(super) const FEEL_FONT: iced::Font = iced::Font {
    family: font::Family::Name("Dancing Script"),
    weight: font::Weight::Bold,
    ..iced::Font::DEFAULT
};

pub(super) const MUSIC_FONT: iced::Font = iced::Font::with_name("Leland Text");
/// Subset to the arrows above by `just fonts`; the family name survives the subset.
pub(super) const KEYCAP_FONT: iced::Font = iced::Font::with_name("Noto Sans Math");

pub(super) fn focus_ring<'a>(
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

/// Pill control metrics. The label decides the pill's shape, so `control_label` pins its line
/// box and `control_glyph` puts a floor under its width.
pub(super) const CONTROL_SIZE: f32 = 15.0;
const CONTROL_LINE_HEIGHT: f32 = 19.0;
const CONTROL_GLYPH_WIDTH: f32 = 26.0;
const CONTROL_ACCIDENTAL_SIZE: f32 = 28.0;
/// How far a lone accidental drops, as a fraction of its size. Measured off the rendered pills.
const SHARP_DROP: f32 = 0.11;
const FLAT_DROP: f32 = 0.18;

pub(super) fn control_button<'a>(
    label: impl Into<Element<'a, Message>>,
    message: Message,
    is_focused: bool,
) -> Element<'a, Message> {
    use iced::widget::button;

    focus_ring(
        button(label)
            .padding([8, 14])
            .style(ghost_button)
            .on_press(message),
        is_focused,
    )
}

/// A control's label, laid in the row's line box. SMuFL glyphs need a larger size than words.
pub(super) fn control_label(
    label: iced::widget::Text<'static>,
    size: f32,
) -> iced::widget::Text<'static> {
    label
        .size(size)
        .line_height(iced::widget::text::LineHeight::Absolute(Pixels(
            CONTROL_LINE_HEIGHT,
        )))
}

/// A label with no word to it — a glyph, or a glyph and a degree — widened to the floor.
pub(super) fn control_glyph<'a>(label: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    use iced::Length;
    use iced::widget::container;

    container(label)
        .center_x(Length::Fixed(CONTROL_GLYPH_WIDTH))
        .into()
}

/// The accidental a spelling control wears: the one in force, not the one pressing it brings.
pub(super) fn spelling_glyph(spelling: Spelling) -> char {
    match spelling {
        Spelling::Sharps => SMUFL_SHARP,
        Spelling::Flats => SMUFL_FLAT,
    }
}

/// A lone accidental, dropped onto the row's optical centre since SMuFL ink sits high.
pub(super) fn control_accidental(glyph: char) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{container, text};

    let drop = if glyph == SMUFL_FLAT {
        FLAT_DROP
    } else {
        SHARP_DROP
    };

    container(control_label(
        text(glyph.to_string()).font(MUSIC_FONT),
        CONTROL_ACCIDENTAL_SIZE,
    ))
    .center_x(Length::Fixed(CONTROL_GLYPH_WIDTH))
    .height(Length::Fixed(CONTROL_LINE_HEIGHT))
    .padding(Padding::default().top(CONTROL_ACCIDENTAL_SIZE * drop))
    .into()
}

/// The reroll mark. Drawn because Unicode's only shuffle, U+1F500, is an emoji.
struct Shuffle;

/// Wider than tall, since the horizontal tails are what separate a shuffle from a ✗.
const SHUFFLE_WIDTH: f32 = 20.0;
const SHUFFLE_HEIGHT: f32 = 13.0;
const SHUFFLE_STROKE: f32 = 1.6;

impl canvas::Program<Message> for Shuffle {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use iced::widget::canvas::{Fill, Frame, LineCap, LineJoin, Path, Stroke};

        let mut frame = Frame::new(renderer, bounds.size());

        // fractions of the box, since rounding to whole pixels at this size makes it lopsided
        let point = |tx: f32, ty: f32| iced::Point::new(tx * bounds.width, ty * bounds.height);

        // flat on arrival so the head points along the line rather than across it
        let arrow = |from: f32, to: f32| {
            Path::new(|path| {
                path.move_to(point(0.00, from));
                path.line_to(point(0.28, from));
                path.line_to(point(0.58, to));
                path.line_to(point(0.72, to));
            })
        };

        // longer than wide; a taller head reads as a spike crossing the line
        let head = |at: f32| {
            Path::new(|path| {
                path.move_to(point(1.00, at));
                path.line_to(point(0.68, at - 0.22));
                path.line_to(point(0.68, at + 0.22));
                path.close();
            })
        };

        let stroke = Stroke::default()
            .with_color(INK)
            .with_width(SHUFFLE_STROKE)
            .with_line_cap(LineCap::Round)
            .with_line_join(LineJoin::Round);

        for (from, to) in [(0.18, 0.82), (0.82, 0.18)] {
            frame.stroke(&arrow(from, to), stroke);
            frame.fill(&head(to), Fill::from(INK));
        }

        vec![frame.into_geometry()]
    }
}

/// The reroll mark in the pills' line box, which a canvas does not have on its own.
pub(super) fn control_shuffle() -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::canvas::Canvas;
    use iced::widget::container;

    container(
        Canvas::new(Shuffle)
            .width(Length::Fixed(SHUFFLE_WIDTH))
            .height(Length::Fixed(SHUFFLE_HEIGHT)),
    )
    .center_x(Length::Fixed(CONTROL_GLYPH_WIDTH))
    .center_y(Length::Fixed(CONTROL_LINE_HEIGHT))
    .into()
}

/// The current run and the best of the session.
pub(super) fn streak_readout(streak: u32, best: u32) -> Element<'static, Message> {
    use iced::widget::{column, row, text};

    let stat = |label: &'static str, value: u32, color: Color| {
        column![
            text(value.to_string()).size(30).color(color),
            text(label).size(12).color(MUTE),
        ]
        .spacing(2)
    };

    let live = if streak > 0 { SUCCESS } else { MUTE };

    row![stat("streak", streak, live), stat("best", best, INK)]
        .spacing(28)
        .into()
}

pub(super) fn page_container(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        text_color: Some(INK),
        background: Some(Background::Color(CANVAS)),
        border: Border::default(),
        shadow: Shadow::default(),
        snap: true,
    }
}

/// A one-pixel rule, filled because a `Border` draws on all four sides.
pub(super) fn hairline_rule(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        text_color: None,
        background: Some(Background::Color(HAIRLINE_INK)),
        border: Border::default(),
        shadow: Shadow::default(),
        snap: true,
    }
}

pub(super) fn card_container(_theme: &iced::Theme) -> iced::widget::container::Style {
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

pub(super) fn card_button(
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

/// A Home category whose entries are showing, marked even while the ring is among them.
pub(super) fn open_card_button(
    theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    iced::widget::button::Style {
        background: Some(Background::Color(CANVAS_SOFT_2)),
        border: Border::default().rounded(12).width(1).color(BODY),
        ..card_button(theme, status)
    }
}

pub(super) fn ghost_button(
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

/// A row in the chord list, or an unpicked diagram in the strip.
pub(super) fn row_button(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let background = match status {
        iced::widget::button::Status::Hovered => CANVAS_SOFT_2,
        _ => Color::TRANSPARENT,
    };

    iced::widget::button::Style {
        background: Some(Background::Color(background)),
        text_color: BODY,
        border: Border::default().rounded(8),
        shadow: Shadow::default(),
        snap: true,
    }
}

/// The picked row, and the picked diagram.
pub(super) fn selected_row_button(
    _theme: &iced::Theme,
    _status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    iced::widget::button::Style {
        background: Some(Background::Color(CANVAS_SOFT_2)),
        text_color: INK,
        border: Border::default().rounded(8).width(1).color(LINK),
        shadow: Shadow::default(),
        snap: true,
    }
}

pub(super) fn selected_root_button(
    _theme: &iced::Theme,
    _status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    iced::widget::button::Style {
        background: Some(Background::Color(SUCCESS)),
        text_color: CANVAS,
        border: Border::default().rounded(64).width(1).color(SUCCESS),
        shadow: Shadow::default(),
        snap: true,
    }
}

/// An answer button the learner has already tried and got wrong.
pub(super) fn wrong_answer_button(
    _theme: &iced::Theme,
    _status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    iced::widget::button::Style {
        background: Some(Background::Color(DANGER)),
        text_color: CANVAS,
        border: Border::default().rounded(64).width(1).color(DANGER),
        shadow: Shadow::default(),
        snap: true,
    }
}

/// The answer that scored, while it is lit. Same pixels as `selected_root_button`.
pub(super) fn correct_answer_button(
    _theme: &iced::Theme,
    _status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    iced::widget::button::Style {
        background: Some(Background::Color(SUCCESS)),
        text_color: CANVAS,
        border: Border::default().rounded(64).width(1).color(SUCCESS),
        shadow: Shadow::default(),
        snap: true,
    }
}
