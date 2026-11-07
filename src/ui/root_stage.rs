//! The big root and the stage built around it: the neck on the left, the root glyph — stepped
//! by a drag or the wheel where the learner picks it — and a card level with it.

use iced::mouse::ScrollDelta;
use iced::widget::canvas::{self, Canvas};
use iced::{Color, Element, Length, Padding, Point, Rectangle, Renderer, Size, Theme};

use super::focus::{FocusTarget, kind_row_spans};
use super::style::{INK, MUSIC_FONT, card_container};
use super::{Message, note_label, scale_kind_row};
use crate::music::intervals::Interval;
use crate::music::notes::Note;
use crate::music::scales::ScaleKind;

/// How far the pointer travels, or the wheel scrolls, for one semitone.
pub(super) const STEP_DISTANCE: f32 = 40.0;

const ROOT_DISPLAY_SIZE: f32 = 120.0;

/// Fixed so the canvas under the glyph does not move with the window.
const ROOT_ROW_HEIGHT: f32 = ROOT_DISPLAY_SIZE * 1.3;

/// Tall enough for the kind card's seven rows, the tallest card any screen sets on the line.
pub(super) const ROOT_BAND_HEIGHT: f32 = 420.0;

pub(super) const FORMULA_SIZE: f32 = 34.0;

/// Fixed because the canvas caps its own width off its height; this is what the neck draws at
/// this window's height plus its padding.
const NECK_COLUMN_WIDTH: f32 = 340.0;

/// Fits the widest `kind_row_spans` row plus the card's padding.
pub(super) const SELECTOR_COLUMN_WIDTH: f32 = 390.0;

pub(super) const CARD_PADDING: f32 = 24.0;

/// A root drag and the wheel's remainder. Holds no root: a gesture yields steps, and `App`
/// hands them to whichever screen is on show.
#[derive(Debug, Default)]
pub(super) struct RootDial {
    /// Where the drag was last measured from, in window coordinates.
    drag: Option<f32>,
    /// Wheel travel that has not yet made a semitone, kept so a nudge and a nudge back cancel.
    scroll: f32,
}

impl RootDial {
    pub(super) fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    pub(super) fn grab(&mut self, x: f32) {
        self.drag = Some(x);
    }

    /// The whole steps a drag that has reached `x` makes. They are consumed off the anchor,
    /// so the pointer must return to where it was to undo one.
    pub(super) fn drag(&mut self, x: f32) -> i32 {
        let Some(anchor) = self.drag else {
            return 0;
        };

        let steps = ((x - anchor) / STEP_DISTANCE).trunc();
        self.drag = Some(anchor + steps * STEP_DISTANCE);
        steps as i32
    }

    pub(super) fn release(&mut self) {
        self.drag = None;
    }

    /// The whole steps a scroll makes. None while a drag is going: one gesture owns the root.
    pub(super) fn scroll(&mut self, steps: f32) -> i32 {
        if self.drag.is_some() {
            return 0;
        }

        self.scroll += steps;
        let whole = self.scroll.trunc();
        self.scroll -= whole;
        whole as i32
    }
}

/// The three columns every root screen shares: the neck, the root, and the selector.
pub(super) fn stage(
    neck: impl Into<Element<'static, Message>>,
    centre: impl Into<Element<'static, Message>>,
    selector: impl Into<Element<'static, Message>>,
) -> Element<'static, Message> {
    use iced::widget::{container, row};

    let neck = container(neck)
        .width(Length::Fixed(NECK_COLUMN_WIDTH))
        .height(Length::Fill);

    container(row![neck, centre.into(), selector.into()].spacing(32))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: 24.0,
            right: 64.0,
            bottom: 48.0,
            left: 64.0,
        })
        .into()
}

/// A root glyph centred on the root line with `below` hanging under it, and `foot` at the
/// column's foot where its height cannot move the line.
pub(super) fn root_column(
    glyph: Element<'static, Message>,
    below: impl Into<Element<'static, Message>>,
    foot: Option<Element<'static, Message>>,
) -> Element<'static, Message> {
    use iced::widget::{Space, column, container, stack};

    // the fill above and the container below split what is left evenly, so the glyph is centred
    let line = on_root_line(
        column![
            Space::new().height(Length::Fill),
            glyph,
            container(below)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill),
        ]
        .height(Length::Fill),
    );

    stack![
        line,
        column![Space::new().height(Length::Fill)]
            .push(foot.map(|foot| container(foot).width(Length::Fill).center_x(Length::Fill)))
            .width(Length::Fill)
            .height(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// A card centred level with the root, and `foot` at the column's foot.
pub(super) fn selector_column(
    card: impl Into<Element<'static, Message>>,
    foot: impl Into<Element<'static, Message>>,
) -> Element<'static, Message> {
    use iced::widget::{Space, column, stack};

    stack![
        on_root_line(card),
        column![Space::new().height(Length::Fill), foot.into()],
    ]
    .width(Length::Fixed(SELECTOR_COLUMN_WIDTH))
    .height(Length::Fill)
    .into()
}

/// Every scale kind as pills, in `kind_row_spans` rows; the message and focus target are the
/// screen's, since the trainer and the drill each own a scale.
pub(super) fn kind_card(
    selected: ScaleKind,
    focused: FocusTarget,
    on_press: fn(ScaleKind) -> Message,
    target: fn(usize) -> FocusTarget,
) -> Element<'static, Message> {
    use iced::widget::{column, container};

    container(
        kind_row_spans().fold(column![].spacing(10), |rows, (start, len)| {
            rows.push(scale_kind_row(
                &ScaleKind::ALL[start..start + len],
                selected,
                start,
                focused,
                on_press,
                target,
            ))
        }),
    )
    .width(Length::Fill)
    .padding(CARD_PADDING)
    .style(card_container)
    .into()
}

/// A formula at the size it is read under the root, each token coloured by its place in it.
pub(super) fn formula(
    intervals: &[Interval],
    color_at: impl Fn(usize) -> Color,
) -> iced::widget::Row<'static, Message> {
    use iced::widget::{row, text};

    intervals
        .iter()
        .enumerate()
        .fold(row![].spacing(14), |tokens, (index, interval)| {
            let color = color_at(index);
            let (glyph, digit) = super::interval_token(*interval);
            let mut token = row![].spacing(0);

            if let Some(glyph) = glyph {
                token = token.push(
                    text(glyph.to_string())
                        .size(FORMULA_SIZE)
                        .font(MUSIC_FONT)
                        .color(color),
                );
            }

            tokens.push(token.push(text(digit.to_string()).size(FORMULA_SIZE).color(color)))
        })
}

/// Places `content` centred in a band at the same height in every column that uses it, so
/// the root glyph and the selector card share one horizontal line.
fn on_root_line(
    content: impl Into<Element<'static, Message>>,
) -> iced::widget::Column<'static, Message> {
    use iced::widget::{Space, column, container};

    // one above to two below puts the band a little above the column's middle
    column![
        Space::new().height(Length::FillPortion(1)),
        container(content)
            .width(Length::Fill)
            .height(Length::Fixed(ROOT_BAND_HEIGHT))
            .align_y(iced::Alignment::Center),
        Space::new().height(Length::FillPortion(2)),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
}

/// A root the screen's learner sets. A `Frame` cannot measure text, so the glyph is a widget
/// stacked over the canvas that takes the drag and the wheel.
pub(super) fn root_glyph(root: Note, dragging: bool) -> Element<'static, Message> {
    use iced::widget::{container, stack};

    container(stack![
        Canvas::new(RootGrip { dragging })
            .width(Length::Fill)
            .height(Length::Fill),
        container(note_label(root, ROOT_DISPLAY_SIZE, INK))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    ])
    .width(Length::Fill)
    .height(Length::Fixed(ROOT_ROW_HEIGHT))
    .into()
}

/// A root the drill chose, which nothing on screen can change.
pub(super) fn still_root_glyph(root: Note) -> Element<'static, Message> {
    use iced::widget::container;

    container(note_label(root, ROOT_DISPLAY_SIZE, INK))
        .width(Length::Fill)
        .height(Length::Fixed(ROOT_ROW_HEIGHT))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

/// Where the root's glyph sits, and what counts as a press on it.
struct RootDisplay {
    center: Point,
    half_width: f32,
    half_height: f32,
}

impl RootDisplay {
    /// Half the width of the widest glyph. Not measured per note, so the box does not change
    /// size under the hand mid-drag.
    const GLYPH_HALF_WIDTH: f32 = 80.0;

    fn new(size: Size) -> RootDisplay {
        RootDisplay {
            center: Point::new(size.width / 2.0, size.height / 2.0),
            // never wider than the canvas, or it would claim presses beside it
            half_width: (size.width / 2.0).min(Self::GLYPH_HALF_WIDTH),
            half_height: size.height.min(ROOT_DISPLAY_SIZE) / 2.0,
        }
    }

    fn grab_box(&self) -> Rectangle {
        Rectangle {
            x: self.center.x - self.half_width,
            y: self.center.y - self.half_height,
            width: 2.0 * self.half_width,
            height: 2.0 * self.half_height,
        }
    }

    fn grabs(&self, point: Point) -> bool {
        self.grab_box().contains(point)
    }
}

/// A scroll, in steps. A notch is one step and pixels are worth what drag pixels are worth.
/// Only the vertical axis is read, so a diagonal flick does not step twice.
fn steps_from(delta: ScrollDelta) -> f32 {
    match delta {
        ScrollDelta::Lines { y, .. } => y,
        ScrollDelta::Pixels { y, .. } => y / STEP_DISTANCE,
    }
}

/// The root glyph's gesture surface. A canvas rather than a `mouse_area` because `update`
/// sees every event, so a drag continues past the glyph's edge.
struct RootGrip {
    /// Read off `App`'s `RootDial`, so `App::update` owns the gesture.
    dragging: bool,
}

impl canvas::Program<Message> for RootGrip {
    type State = ();

    /// Nothing; the glyph is the widget stacked over this canvas.
    fn draw(
        &self,
        _state: &(),
        _renderer: &Renderer,
        _theme: &Theme,
        _bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        Vec::new()
    }

    fn update(
        &self,
        _state: &mut (),
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let display = RootDisplay::new(bounds.size());
        let over = cursor
            .position_in(bounds)
            .is_some_and(|point| display.grabs(point));

        match event {
            // carries the window's x: `position_in` is `None` once the pointer leaves
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left))
                if over =>
            {
                let x = cursor.position()?.x;
                Some(canvas::Action::publish(Message::GrabRoot(x)).and_capture())
            }
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) if self.dragging => {
                Some(canvas::Action::publish(Message::DragRoot(position.x)).and_capture())
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
                if self.dragging =>
            {
                Some(canvas::Action::publish(Message::DropRoot).and_capture())
            }
            // captured whether or not it stepped, so a wheel over the glyph never falls through
            iced::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) if over => {
                Some(canvas::Action::publish(Message::ScrollRoot(steps_from(*delta))).and_capture())
            }
            _ => None,
        }
    }

    fn mouse_interaction(
        &self,
        _state: &(),
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> iced::mouse::Interaction {
        if self.dragging {
            iced::mouse::Interaction::Grabbing
        } else if cursor
            .position_in(bounds)
            .is_some_and(|point| RootDisplay::new(bounds.size()).grabs(point))
        {
            iced::mouse::Interaction::Grab
        } else {
            iced::mouse::Interaction::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::KIND_BUTTON_SIZE;
    use iced::widget::canvas::Program as _;

    // ---- The dial ----

    #[test]
    fn a_drag_yields_a_step_for_each_step_of_travel_and_gives_it_back() {
        let mut dial = RootDial::default();
        dial.grab(0.0);

        assert_eq!(dial.drag(STEP_DISTANCE * 0.9), 0, "short of a step");
        assert_eq!(dial.drag(STEP_DISTANCE * 3.0), 3);
        assert_eq!(dial.drag(0.0), -3, "the travel was not given back");
    }

    #[test]
    fn nothing_steps_with_no_drag_in_progress() {
        let mut dial = RootDial::default();
        assert_eq!(dial.drag(STEP_DISTANCE * 4.0), 0);

        dial.grab(0.0);
        dial.release();
        assert_eq!(
            dial.drag(STEP_DISTANCE * 4.0),
            0,
            "a move after the drop stepped"
        );
    }

    #[test]
    fn the_wheel_carries_its_remainder_and_is_ignored_mid_drag() {
        let mut dial = RootDial::default();
        assert_eq!(dial.scroll(0.5), 0);
        assert_eq!(dial.scroll(0.5), 1, "two half steps are a semitone");
        assert_eq!(dial.scroll(0.6) + dial.scroll(-0.6), 0);

        dial.grab(0.0);
        assert_eq!(dial.scroll(3.0), 0, "the wheel stepped during a drag");
    }

    // ---- The grip ----

    /// Widths the root display is plausibly drawn at; the height is fixed.
    const ROOT_SIZES: [Size; 4] = [
        Size::new(220.0, ROOT_ROW_HEIGHT),
        Size::new(278.0, ROOT_ROW_HEIGHT),
        Size::new(360.0, ROOT_ROW_HEIGHT),
        Size::new(520.0, ROOT_ROW_HEIGHT),
    ];

    #[test]
    fn the_grab_box_is_centred_and_stays_inside_the_canvas() {
        for size in ROOT_SIZES {
            let display = RootDisplay::new(size);
            let grab_box = display.grab_box();

            assert!(
                display.grabs(display.center),
                "{size:?} does not grab its own centre"
            );

            // A point just outside each edge belongs to nothing.
            for outside in [
                Point::new(grab_box.x - 1.0, display.center.y),
                Point::new(grab_box.x + grab_box.width + 1.0, display.center.y),
                Point::new(display.center.x, grab_box.y - 1.0),
                Point::new(display.center.x, grab_box.y + grab_box.height + 1.0),
            ] {
                assert!(!display.grabs(outside), "{size:?} grabs {outside:?}");
            }

            assert!(
                grab_box.x >= 0.0 && grab_box.x + grab_box.width <= size.width,
                "{size:?} puts the box past the canvas edge"
            );
        }
    }

    #[test]
    fn a_notch_and_a_step_of_pixels_are_each_one_step() {
        assert_eq!(steps_from(ScrollDelta::Lines { x: 0.0, y: 1.0 }), 1.0);
        assert_eq!(
            steps_from(ScrollDelta::Pixels {
                x: 0.0,
                y: STEP_DISTANCE
            }),
            1.0,
            "a trackpad steps at a different rate from a drag"
        );
        assert_eq!(
            steps_from(ScrollDelta::Lines { x: 4.0, y: 0.0 }),
            0.0,
            "sideways scrolling stepped the root"
        );
    }

    const GRIP_BOUNDS: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 278.0,
        height: ROOT_ROW_HEIGHT,
    };

    /// Whether the canvas acts on an event; what it publishes is sealed inside a `canvas::Action`.
    fn acts_on(dragging: bool, event: &canvas::Event, at: Point) -> bool {
        RootGrip { dragging }
            .update(
                &mut (),
                event,
                GRIP_BOUNDS,
                iced::mouse::Cursor::Available(at),
            )
            .is_some()
    }

    #[test]
    fn the_wheel_is_only_the_roots_over_the_glyph() {
        let wheel = iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
            delta: ScrollDelta::Lines { x: 0.0, y: 1.0 },
        });

        let display = RootDisplay::new(GRIP_BOUNDS.size());
        let corner = Point::new(2.0, 2.0);
        assert!(!display.grabs(corner), "the corner is inside the box");

        assert!(
            !acts_on(false, &wheel, corner),
            "a scroll off the glyph reached the root"
        );
        assert!(
            acts_on(false, &wheel, display.center),
            "a scroll on the glyph did not"
        );
    }

    /// A press has to land on the glyph; the rest of the drag is claimed wherever it goes.
    #[test]
    fn the_press_is_placed_and_the_rest_of_the_drag_is_not() {
        let display = RootDisplay::new(GRIP_BOUNDS.size());
        let corner = Point::new(2.0, 2.0);

        let press =
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left));
        let moved = iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: corner });
        let release = iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        ));

        assert!(acts_on(false, &press, display.center));
        assert!(
            !acts_on(false, &press, corner),
            "a press off the glyph began a drag"
        );

        assert!(
            !acts_on(false, &moved, display.center),
            "a cursor move with no drag going was claimed"
        );
        assert!(
            acts_on(true, &moved, corner),
            "a drag stopped at the canvas edge"
        );
        assert!(
            acts_on(true, &release, corner),
            "the drag could not be released outside the glyph"
        );
        assert!(
            !acts_on(false, &release, display.center),
            "a release with no drag going was claimed"
        );
    }

    // ---- The kind card ----

    /// The glyph width is a generous estimate, since there is no font to measure here.
    #[test]
    fn the_widest_kind_row_fits_the_selector_column() {
        // a pill is its text, the button's `.padding([8, 12])` and the 3px `focus_ring`;
        // pills sit `.spacing(8)` apart
        const EM_PER_GLYPH: f32 = 0.58;
        const BUTTON_PADDING_X: f32 = 12.0;
        const FOCUS_RING_PADDING: f32 = 3.0;
        const ROW_SPACING: f32 = 8.0;

        let widest = kind_row_spans()
            .map(|(start, len)| {
                let kinds = &ScaleKind::ALL[start..start + len];
                let text: f32 = kinds
                    .iter()
                    .map(|kind| {
                        kind.name().chars().count() as f32 * EM_PER_GLYPH * KIND_BUTTON_SIZE
                    })
                    .sum();
                let chrome = len as f32 * 2.0 * (BUTTON_PADDING_X + FOCUS_RING_PADDING);
                text + chrome + (len - 1) as f32 * ROW_SPACING
            })
            .fold(0.0_f32, f32::max);

        let available = SELECTOR_COLUMN_WIDTH - 2.0 * CARD_PADDING;

        assert!(
            available >= widest,
            "the selector column leaves {available} for its widest kind row, which needs \
             {widest}; the last pill would draw past the card's edge"
        );
    }

    #[test]
    fn the_kind_card_fits_the_root_band() {
        // iced's default line height, the card's `.spacing(10)`, and a pill's `.padding([8, 12])`
        // inside its 3px `focus_ring`
        const TEXT_LINE_HEIGHT: f32 = 1.3;
        const ROW_SPACING: f32 = 10.0;
        const BUTTON_PADDING_Y: f32 = 8.0;
        const FOCUS_RING_PADDING: f32 = 3.0;

        let rows = kind_row_spans().count() as f32;
        let pill =
            KIND_BUTTON_SIZE * TEXT_LINE_HEIGHT + 2.0 * BUTTON_PADDING_Y + 2.0 * FOCUS_RING_PADDING;
        let needed = rows * pill + (rows - 1.0) * ROW_SPACING + 2.0 * CARD_PADDING;

        assert!(
            ROOT_BAND_HEIGHT >= needed,
            "the root band is {ROOT_BAND_HEIGHT} tall but the kind card needs {needed}; an \
             overflowing card would no longer be centred on the root"
        );
    }
}
