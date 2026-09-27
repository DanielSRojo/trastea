//! How the app looks: the palette, the faces, the glyphs drawn out of them, and the control
//! and container styles every screen builds from.

use iced::widget::canvas;
use iced::{Background, Border, Color, Element, Padding, Pixels, Shadow, Vector, font};

use crate::music::notes::Spelling;

use super::Message;

pub(super) const INK: Color = Color::WHITE;
pub(super) const BODY: Color = Color::from_rgb8(0xb5, 0xb5, 0xb5);
pub(super) const MUTE: Color = Color::from_rgb8(0x77, 0x77, 0x77);
const HAIRLINE: Color = Color::from_rgb8(0x1f, 0x1f, 0x1f);
/// A hairline bright enough to read as a rule between groups rather than as an edge, which
/// is what `HAIRLINE` is for. Also the colour of the slash between a black key's two names.
pub(super) const HAIRLINE_INK: Color = Color::from_rgb8(0x33, 0x33, 0x33);
pub(super) const CANVAS: Color = Color::BLACK;
pub(super) const CANVAS_SOFT: Color = Color::from_rgb8(0x0a, 0x0a, 0x0a);
pub(super) const CANVAS_SOFT_2: Color = Color::from_rgb8(0x11, 0x11, 0x11);
pub(super) const LINK: Color = Color::from_rgb8(0x50, 0xa7, 0xff);
/// The theme's two semantic colours, named here because canvas markers and button styles
/// are built where no `&Theme` is in hand. Kept in step with `theme()` in `main.rs`; these
/// are the same literals `scale_markers` and `selected_root_button` already used inline.
pub(super) const DANGER: Color = Color::from_rgb8(0xff, 0x4d, 0x4d);
pub(super) const SUCCESS: Color = Color::from_rgb8(0x50, 0xe3, 0xc2);
/// The Interval Trainer's tonal center, which needs a colour no other mark on that neck
/// uses: `LINK` is the prompt, `DANGER` and `SUCCESS` are the answers, and white is the
/// keyboard cursor's ring. Amber is what is left, and it reads as warm against all four.
pub(super) const ROOT_MARKER: Color = Color::from_rgb8(0xff, 0xb3, 0x4d);
pub(super) const SMUFL_FLAT: char = '\u{E260}';
pub(super) const SMUFL_SHARP: char = '\u{E262}';
pub(super) const SMUFL_DOUBLE_SHARP: char = '\u{E263}';
pub(super) const SMUFL_DOUBLE_FLAT: char = '\u{E264}';
/// SMuFL's chord-symbol range, already present in the embedded Leland subset and unused
/// until now. A chord symbol set with these reads as one a musician wrote; the ASCII the
/// parser accepts is a different job, done by `Display for Chord`.
pub(super) const SMUFL_CSYM_DIMINISHED: char = '\u{E870}';
pub(super) const SMUFL_CSYM_HALF_DIMINISHED: char = '\u{E871}';
pub(super) const SMUFL_CSYM_AUGMENTED: char = '\u{E872}';
pub(super) const SMUFL_CSYM_MAJOR_SEVENTH: char = '\u{E873}';
/// Every arrow the chrome draws, U+2190–2193 and U+21E4–21E5, out of one embedded face.
///
/// The tab pair is in almost no text face, so what the fallback found for them differed per
/// machine and drew at that stranger's metrics. The four plain arrows are in nearly every
/// face and so always drew *something*, but a body font sets them as hairlines against its
/// own optical size — beside a keycap they read as a lighter mark rather than the same one.
/// Taking all six from the same place is what makes the overlay one row of keys.
///
/// Back-tab is the one glyph rather than ⇧ and ⇥ together: ⇧ is UPWARDS WHITE ARROW, drawn
/// hollow wherever it is drawn at all, and beside a line arrow the pair reads as two marks
/// from different families rather than as one shortcut.
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
/// Noto Sans Math, subset to the arrows above by `just fonts`. The family name survives the
/// subset, which is what `with_name` matches on.
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

/// The pill controls: the row above a drill, and the pair beside the scale trainer's root.
///
/// `ghost_button` rounds to a full pill, so the label is what decides the shape the button
/// comes out — one glyph and nothing else draws a circle beside its neighbours' lozenges,
/// and a label in `MUSIC_FONT` stands shorter than one in the body font, since the button is
/// as tall as the label's line box and SMuFL's metrics are nothing like a text face's.
/// `control_label` pins the line box; `control_glyph` puts a floor under the width.
pub(super) const CONTROL_SIZE: f32 = 15.0;
const CONTROL_LINE_HEIGHT: f32 = 19.0;
const CONTROL_GLYPH_WIDTH: f32 = 26.0;
const CONTROL_ACCIDENTAL_SIZE: f32 = 28.0;
/// How far a lone accidental drops, as a fraction of its own size. See `control_accidental`.
/// Measured off the rendered pills: each lands the glyph's ink on the centre the letters in
/// the row beside it sit on.
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

/// A control's label, laid in the row's line box. The size stays the caller's: a SMuFL glyph
/// is drawn against a staff space rather than a cap height, so it has to be set larger than
/// the words beside it to carry the same weight.
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

/// A lone accidental: set large enough to read with nothing beside it, and dropped onto the
/// row's optical centre.
///
/// SMuFL draws an accidental against a staff rather than a cap height, so its ink sits high
/// in the ascent-and-descent box the line box centres — and a flat, which hangs its bowl
/// under the staff line it belongs to, starts higher than a sharp and has further to fall.
/// Next to a letter that is exactly what an accidental should do; alone in a pill it reads
/// as floating. Fixing the container's height is what makes the top padding move the glyph
/// rather than the pill.
/// The accidental a spelling control wears: the one in force, not the one pressing it
/// would bring.
pub(super) fn spelling_glyph(spelling: Spelling) -> char {
    match spelling {
        Spelling::Sharps => SMUFL_SHARP,
        Spelling::Flats => SMUFL_FLAT,
    }
}

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

/// The reroll control's mark: the crossing arrows a shuffle is drawn with.
///
/// Drawn rather than set. Unicode's only shuffle is U+1F500, an emoji-block codepoint that
/// exists on most systems in a colour font and on the rest in none — neither of which a
/// `text` widget can turn into a stroke that follows the pill's label colour.
///
/// Drawn in `INK` rather than taking a colour, the way `SearchIcon` is: `ghost_button` sets
/// `text_color` to `INK` in every status, so the mark matches the lettered pills beside it
/// hovered or not.
struct Shuffle;

/// The mark's box and the weight it is stroked at. Wider than it is tall, because the
/// horizontal tails are what separate a shuffle from a ✗ and a square box leaves no room
/// for them — in one the arrows cross almost as soon as they start. Shorter than
/// `CONTROL_LINE_HEIGHT` so it reads as a mark set in the row rather than as art filling
/// the pill.
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

        // Laid out in fractions of the box so the mark is resolution-independent: at
        // `SHUFFLE_SIZE` there are fifteen pixels to place a whole icon in, and rounding
        // any of these to whole pixels first is what makes it lopsided.
        let point = |tx: f32, ty: f32| iced::Point::new(tx * bounds.width, ty * bounds.height);

        // Each arrow leaves flat, crosses, and arrives flat — the horizontal tails are what
        // separate a shuffle from a plain ✗, and the flat arrival is what lets the head
        // point along the line rather than across it.
        let arrow = |from: f32, to: f32| {
            Path::new(|path| {
                path.move_to(point(0.00, from));
                path.line_to(point(0.28, from));
                path.line_to(point(0.58, to));
                path.line_to(point(0.72, to));
            })
        };

        // Longer than it is wide. The first pass had it the other way round, and at this
        // size a head taller than its own length reads as a spike crossing the line.
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

/// The reroll control's label, in the line box the pills' words sit in.
///
/// A canvas has no line box of its own, so without the fixed height here the reroll pill
/// would come out shorter than the lettered ones beside it — the same mismatch
/// `control_label` exists to prevent between `MUSIC_FONT` and the body font.
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
///
/// The live streak is drawn in the theme's success colour: the per-answer flash says a
/// single answer was right and then goes, and a standing streak keeps saying it.
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

/// A one-pixel rule. A filled container rather than a border, since a `Border` draws on all
/// four sides and only the underline is wanted.
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
///
/// Square rather than the pill `ghost_button` draws, and borderless: a column of pills
/// reads as a column of buttons, and the list is meant to read as a list.
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

/// The picked row, and the picked diagram. Lifted off the card rather than coloured in:
/// the symbol inside is already the loudest thing in the row, and a filled row would
/// compete with it.
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
///
/// Filled rather than outlined, so that at a glance the buttons still to try are the empty
/// ones — the same read as the selected root, in the opposite colour.
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

/// The answer that scored, while it is lit.
///
/// The same filled shape as `wrong_answer_button` in the opposite colour, because the two
/// say the same kind of thing about the same button and only differ in the verdict. Note it
/// is `selected_root_button` to the pixel — kept separate anyway, since a scale trainer
/// selection and a drill's verdict have no reason to move together.
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
