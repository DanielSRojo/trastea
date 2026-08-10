use iced::alignment;
use iced::widget::canvas::{self, Canvas, Fill, Frame, Path, Stroke, Text};
use iced::{Color, Element, Length, Pixels, Point, Rectangle, Renderer, Size, Theme};

/// Where everything on the neck sits, computed once from the canvas bounds.
///
/// Both drawing and hit-testing read this. Two independent copies of the same
/// arithmetic would drift the moment either one changed, and the symptom would be the
/// worst kind: a click resolving one fret away from the dot it visibly landed on. So
/// the inverse (`position_at`) lives next to the forward direction (`marker_center`),
/// and a round-trip test holds the two together.
///
/// The four edges are methods rather than fields for the same reason — stored, they
/// could disagree with the spacings they are derived from.
struct Layout {
    string_count: usize,
    fret_count: usize,
    left_edge: f32,
    string_spacing: f32,
    /// The nut's y, which is also the top of fret 1's band.
    pad_top: f32,
    fret_spacing: f32,
    note_radius: f32,
    open_marker_gap: f32,
}

impl Layout {
    const PAD_X: f32 = 20.0;
    const PAD_BOTTOM: f32 = 20.0;
    const OPEN_MARKER_GAP: f32 = 4.0;
    const TARGET_FRET_CELL_RATIO: f32 = 1.35;
    const STRING_COUNT: usize = 6;

    fn new(size: Size, fret_count: usize) -> Self {
        let available_w = size.width - 2.0 * Self::PAD_X;
        let available_string_spacing = available_w / (Self::STRING_COUNT - 1) as f32;

        // Deliberately the *largest* radius the neck could end up using, and not the one
        // markers are actually drawn with: `pad_top` has to reserve room for an
        // open-string marker before `fret_spacing` — and so the final radius — is known.
        // Collapsing this into `note_radius` below would move the whole neck down.
        let max_note_radius = (available_string_spacing * 0.35).clamp(8.0, 14.0);
        let pad_top = max_note_radius * 2.0 + Self::OPEN_MARKER_GAP;

        let usable_h = size.height - pad_top - Self::PAD_BOTTOM;
        let fret_spacing = usable_h / fret_count as f32;
        let string_spacing =
            available_string_spacing.min(fret_spacing / Self::TARGET_FRET_CELL_RATIO);
        let usable_w = string_spacing * (Self::STRING_COUNT - 1) as f32;

        Self {
            string_count: Self::STRING_COUNT,
            fret_count,
            left_edge: (size.width - usable_w) / 2.0,
            string_spacing,
            pad_top,
            fret_spacing,
            // One expression where `draw` used to take this `min` in two steps, shadowing
            // the first binding with the second. Exactly equivalent: the first fed
            // nothing else.
            note_radius: (string_spacing * 0.35)
                .clamp(8.0, 14.0)
                .min((fret_spacing * 0.35).clamp(8.0, 14.0)),
            open_marker_gap: Self::OPEN_MARKER_GAP,
        }
    }

    fn string_x(&self, string: usize) -> f32 {
        self.left_edge + string as f32 * self.string_spacing
    }

    /// The y of fret line `fret`. Line 0 is the nut.
    fn fret_y(&self, fret: usize) -> f32 {
        self.pad_top + fret as f32 * self.fret_spacing
    }

    fn top_y(&self) -> f32 {
        self.pad_top
    }

    fn bottom_y(&self) -> f32 {
        self.fret_y(self.fret_count)
    }

    fn left_x(&self) -> f32 {
        self.left_edge
    }

    fn right_x(&self) -> f32 {
        self.string_x(self.string_count - 1)
    }

    /// Where a marker at `(string, fret)` is centred. Fret 0 is the open string, which
    /// sits in the band above the nut rather than between two fret lines.
    fn marker_center(&self, string: usize, fret: usize) -> Point {
        let y = if fret == 0 {
            self.pad_top - self.note_radius - self.open_marker_gap
        } else {
            (self.fret_y(fret - 1) + self.fret_y(fret)) / 2.0
        };

        Point::new(self.string_x(string), y)
    }

    /// The inverse of `marker_center`: which position, if any, a point lands on.
    ///
    /// A point off the neck yields `None` rather than being clamped onto the nearest
    /// edge. Clamping would turn a misdirected click into an answer the user never gave,
    /// which on a trainer means a wrong answer they did not earn.
    fn position_at(&self, point: Point) -> Option<(usize, usize)> {
        // The hit area is the marker's footprint, not the bare string line: a marker on
        // an outer string extends `note_radius` past it, and a click on a dot the user
        // can see has to register. Past that footprint the padding is a genuine miss.
        let slack = self.note_radius;
        if point.x < self.left_x() - slack || point.x > self.right_x() + slack {
            return None;
        }

        // `round` can still land outside the neck when the point is in the slack beyond
        // an outer string, so the index is clamped rather than trusted.
        let nearest = ((point.x - self.left_edge) / self.string_spacing).round() as isize;
        let string = nearest.clamp(0, self.string_count as isize - 1) as usize;

        let fret = if point.y < self.top_y() {
            // Above the nut: the open-string band, and nothing above it.
            let open_top = self.marker_center(string, 0).y - self.note_radius;
            (point.y >= open_top).then_some(0)?
        } else if point.y > self.bottom_y() {
            return None;
        } else {
            // Fret n owns the band between line n-1 and line n, so the band index is one
            // past the line above it — which also makes `y == pad_top` fret 1's top edge
            // rather than fret 0's bottom.
            let band = ((point.y - self.top_y()) / self.fret_spacing).floor() as usize + 1;
            band.min(self.fret_count)
        };

        Some((string, fret))
    }
}

/// One dot on the neck: where it sits, what it says, and what colour it is.
///
/// `label` is text already decided rather than a `Note`, so this module draws
/// circles and glyphs and knows nothing about music. What the text means — a note
/// name, a scale degree — is the caller's business, and adding a third way to
/// label a marker needs no change here.
pub struct NoteMarker {
    pub string: usize, // 0 = low E, 5 = high e
    pub fret: usize,
    pub label: String,
    pub color: Color,
}

/// The neck, and optionally a way to press it.
///
/// Generic over `Message` only because `on_press` maps a position onto one. The module
/// still knows nothing of what a press *means*: it reports which dot was hit and the
/// caller decides.
pub struct Fretboard<Message> {
    pub num_frets: usize,
    pub highlighted: Vec<NoteMarker>,
    /// Where the keyboard cursor sits, drawn as a ring.
    ///
    /// Owned by the caller rather than kept in `Program::State`, because the application
    /// runs its own focus system and the canvas has no way to know whether it is the
    /// focused widget. See design decision 6.
    pub cursor: Option<(usize, usize)>,
    /// What a press on `(string, fret)` means, or `None` for a display-only neck.
    ///
    /// A plain `fn` pointer, not a `Box<dyn Fn>`: every caller passes a bare enum
    /// constructor, which captures nothing. A trait object would buy capture nobody uses
    /// and cost an allocation, an indirection, and `Copy`.
    pub on_press: Option<fn(usize, usize) -> Message>,
}

// Hand-written rather than derived: `#[derive(Default)]` would demand `Message: Default`,
// which is both unnecessary — the field is an `Option` — and wrong, since a message enum
// has no sensible default.
impl<Message> Default for Fretboard<Message> {
    fn default() -> Self {
        Self {
            num_frets: 12,
            highlighted: Vec::new(),
            cursor: None,
            on_press: None,
        }
    }
}

impl<Message> canvas::Program<Message> for Fretboard<Message> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        let layout = Layout::new(bounds.size(), self.num_frets);
        let fret_count = layout.fret_count;
        let string_count = layout.string_count;

        // Background
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), Color::BLACK);

        let string_x = |s: usize| layout.string_x(s);
        let fret_y = |f: usize| layout.fret_y(f);

        let top_y = layout.top_y();
        let bottom_y = layout.bottom_y();
        let left_x = layout.left_x();
        let right_x = layout.right_x();

        let ink = Color::WHITE;
        let gray = Color::from_rgb8(0x3a, 0x3a, 0x3a);
        let light_gray = Color::from_rgb8(0x77, 0x77, 0x77);

        frame.stroke(
            &Path::line(Point::new(left_x, top_y), Point::new(right_x, top_y)),
            Stroke::default().with_color(ink).with_width(3.0),
        );

        // Frets — thin gray horizontal lines for frets 1–12
        for f in 1..=fret_count {
            let y = fret_y(f);
            frame.stroke(
                &Path::line(Point::new(left_x, y), Point::new(right_x, y)),
                Stroke::default().with_color(gray).with_width(1.0),
            );
        }

        // Strings — thin light-gray vertical lines
        for s in 0..string_count {
            let x = string_x(s);
            frame.stroke(
                &Path::line(Point::new(x, top_y), Point::new(x, bottom_y)),
                Stroke::default().with_color(light_gray).with_width(1.0),
            );
        }

        let marker_x = (string_x(2) + string_x(3)) / 2.0;
        let lower_double_marker_x = (string_x(1) + string_x(2)) / 2.0;
        let upper_double_marker_x = (string_x(3) + string_x(4)) / 2.0;
        let dot_gray = Color::from_rgb8(0x88, 0x88, 0x88);
        let fret_marker_radius = 6.5;

        for &f in &[3_usize, 5, 7, 9] {
            let y = (fret_y(f - 1) + fret_y(f)) / 2.0;
            frame.fill(
                &Path::circle(Point::new(marker_x, y), fret_marker_radius),
                Fill::from(dot_gray),
            );
        }

        // Double dot at fret 12
        {
            let y = (fret_y(11) + fret_y(12)) / 2.0;
            frame.fill(
                &Path::circle(Point::new(lower_double_marker_x, y), fret_marker_radius),
                Fill::from(dot_gray),
            );
            frame.fill(
                &Path::circle(Point::new(upper_double_marker_x, y), fret_marker_radius),
                Fill::from(dot_gray),
            );
        }

        // Highlighted note markers
        let note_radius = layout.note_radius;

        for marker in &self.highlighted {
            let center = layout.marker_center(marker.string, marker.fret);

            frame.fill(&Path::circle(center, note_radius), Fill::from(marker.color));

            frame.fill_text(Text {
                // Cloned because `Text` owns its content and `draw` only borrows the
                // marker — the same allocation per marker per frame that
                // `note.to_string()` made here before.
                content: marker.label.clone(),
                position: center,
                color: Color::WHITE,
                size: Pixels(note_radius * 1.1),
                align_x: iced::widget::text::Alignment::Center,
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });
        }

        // The keyboard cursor, drawn last so it is never buried under a marker.
        //
        // A stroked ring, deliberately not a filled dot: on a screen whose whole point is
        // "which dot is lit", a second filled circle would read as a second lit note.
        if let Some((string, fret)) = self.cursor {
            let center = layout.marker_center(string, fret);

            frame.stroke(
                &Path::circle(center, note_radius + 3.0),
                Stroke::default().with_color(ink).with_width(2.0),
            );
        }

        vec![frame.into_geometry()]
    }

    /// Turns a press on the neck into the caller's message.
    ///
    /// Note the `&self`: iced hands the program a shared reference here, so nothing can be
    /// cached between events and the layout is rebuilt from `bounds` each time. That is
    /// cheap — a dozen float operations — and it is what guarantees hit-testing uses the
    /// same geometry the last `draw` did.
    fn update(
        &self,
        _state: &mut (),
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        // A neck with no handler is a picture: it neither consumes the event nor asks for
        // a redraw, so the scale trainer behaves exactly as it did before.
        let on_press = self.on_press?;

        let iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) =
            event
        else {
            return None;
        };

        let point = cursor.position_in(bounds)?;
        let (string, fret) = Layout::new(bounds.size(), self.num_frets).position_at(point)?;

        // Captured so the press does not also fall through to whatever is behind the
        // canvas. A miss above returns `None` instead, leaving the event alone.
        Some(canvas::Action::publish(on_press(string, fret)).and_capture())
    }

    fn mouse_interaction(
        &self,
        _state: &(),
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> iced::mouse::Interaction {
        let over_a_position = self.on_press.is_some()
            && cursor
                .position_in(bounds)
                .and_then(|point| Layout::new(bounds.size(), self.num_frets).position_at(point))
                .is_some();

        if over_a_position {
            iced::mouse::Interaction::Pointer
        } else {
            iced::mouse::Interaction::default()
        }
    }
}

pub fn fretboard<Message: 'static>(fb: Fretboard<Message>) -> Element<'static, Message> {
    Canvas::new(fb)
        .width(Length::Fixed(280.0))
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sizes the neck is plausibly drawn at. The widget is a fixed 280 wide and fills
    /// vertically, so the height is what actually varies with the window.
    const SIZES: [Size; 4] = [
        Size::new(280.0, 500.0),
        Size::new(280.0, 700.0),
        Size::new(280.0, 940.0),
        Size::new(320.0, 640.0),
    ];

    const FRETS: usize = 12;

    /// The property that makes hit-testing trustworthy: the centre of every marker
    /// resolves back to the position that marker belongs to.
    ///
    /// This is the guard for the whole reason `Layout` exists. If drawing and
    /// hit-testing are ever edited apart, this fails.
    #[test]
    fn every_marker_center_round_trips() {
        for size in SIZES {
            let layout = Layout::new(size, FRETS);

            for string in 0..layout.string_count {
                for fret in 0..=FRETS {
                    let center = layout.marker_center(string, fret);

                    assert_eq!(
                        layout.position_at(center),
                        Some((string, fret)),
                        "{size:?} lost ({string}, {fret}) at {center:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_point_between_two_strings_takes_the_nearer_one() {
        let layout = Layout::new(SIZES[1], FRETS);
        let center = layout.marker_center(2, 5);

        // A nudge short of halfway towards string 3 still belongs to string 2.
        let nudged = Point::new(center.x + layout.string_spacing * 0.4, center.y);
        assert_eq!(layout.position_at(nudged), Some((2, 5)));

        // Past halfway it belongs to string 3.
        let past = Point::new(center.x + layout.string_spacing * 0.6, center.y);
        assert_eq!(layout.position_at(past), Some((3, 5)));
    }

    #[test]
    fn a_fret_band_is_owned_by_the_line_below_it() {
        let layout = Layout::new(SIZES[1], FRETS);

        // The nut itself is the top of fret 1's band, not the bottom of fret 0's.
        assert_eq!(
            layout.position_at(Point::new(layout.string_x(0), layout.top_y())),
            Some((0, 1))
        );

        // Just above fret line 5 is still fret 5; just below it is fret 6.
        let x = layout.string_x(1);
        let line_5 = layout.fret_y(5);
        assert_eq!(
            layout.position_at(Point::new(x, line_5 - 1.0)),
            Some((1, 5))
        );
        assert_eq!(
            layout.position_at(Point::new(x, line_5 + 1.0)),
            Some((1, 6))
        );
    }

    #[test]
    fn the_last_fret_band_reaches_the_bottom_edge() {
        let layout = Layout::new(SIZES[1], FRETS);
        let x = layout.string_x(3);

        assert_eq!(
            layout.position_at(Point::new(x, layout.bottom_y())),
            Some((3, FRETS))
        );
    }

    #[test]
    fn the_open_string_band_above_the_nut_is_fret_zero() {
        let layout = Layout::new(SIZES[1], FRETS);
        let center = layout.marker_center(4, 0);

        assert_eq!(layout.position_at(center), Some((4, 0)));
        // Anywhere in the band down to the nut is still the open string.
        assert_eq!(
            layout.position_at(Point::new(center.x, layout.top_y() - 1.0)),
            Some((4, 0))
        );
    }

    /// Note this probes above the *canvas*, not above `pad_top`. When the marker radius
    /// hits its 14px clamp the open band fills the reserved padding exactly, leaving no
    /// gap between the top of the band and y = 0 — so `y = 0.0` is a legitimate fret 0 at
    /// some sizes and asserting otherwise tests the clamp rather than the boundary.
    #[test]
    fn a_point_above_the_neck_resolves_to_nothing() {
        for size in SIZES {
            let layout = Layout::new(size, FRETS);
            let x = layout.string_x(2);

            assert_eq!(layout.position_at(Point::new(x, -5.0)), None, "{size:?}");
        }
    }

    #[test]
    fn the_top_of_the_open_band_is_the_boundary() {
        for size in SIZES {
            let layout = Layout::new(size, FRETS);
            let center = layout.marker_center(2, 0);
            let band_top = center.y - layout.note_radius;

            assert_eq!(
                layout.position_at(Point::new(center.x, band_top)),
                Some((2, 0)),
                "{size:?} band top"
            );
            assert_eq!(
                layout.position_at(Point::new(center.x, band_top - 1.0)),
                None,
                "{size:?} above band top"
            );
        }
    }

    #[test]
    fn a_point_below_the_last_fret_resolves_to_nothing() {
        for size in SIZES {
            let layout = Layout::new(size, FRETS);
            let x = layout.string_x(2);

            assert_eq!(
                layout.position_at(Point::new(x, layout.bottom_y() + 1.0)),
                None,
                "{size:?}"
            );
        }
    }

    #[test]
    fn a_point_in_the_side_padding_resolves_to_nothing() {
        for size in SIZES {
            let layout = Layout::new(size, FRETS);
            let y = layout.marker_center(0, 6).y;

            let left_of_neck = layout.left_x() - layout.note_radius - 1.0;
            let right_of_neck = layout.right_x() + layout.note_radius + 1.0;

            assert_eq!(
                layout.position_at(Point::new(left_of_neck, y)),
                None,
                "{size:?}"
            );
            assert_eq!(
                layout.position_at(Point::new(right_of_neck, y)),
                None,
                "{size:?}"
            );
        }
    }

    /// A click on a visible dot has to register, so the outer strings' markers are
    /// hittable across their whole width rather than only on the string line.
    #[test]
    fn an_outer_marker_is_hittable_across_its_footprint() {
        let layout = Layout::new(SIZES[1], FRETS);

        let low_e = layout.marker_center(0, 7);
        let high_e = layout.marker_center(5, 7);

        assert_eq!(
            layout.position_at(Point::new(low_e.x - layout.note_radius * 0.9, low_e.y)),
            Some((0, 7))
        );
        assert_eq!(
            layout.position_at(Point::new(high_e.x + layout.note_radius * 0.9, high_e.y)),
            Some((5, 7))
        );
    }

    /// `pad_top` is built from `max_note_radius`, which is deliberately *not* the radius
    /// markers end up drawn with — it is the upper bound reserved before `fret_spacing`
    /// is known. Collapsing the two would move the whole neck down as the window shrank.
    ///
    /// Shown by holding the width fixed and varying the height: `pad_top` depends only on
    /// the width, while `note_radius` shrinks with the height. If the two were one value,
    /// the first assertion would fail.
    #[test]
    fn the_reserved_top_padding_does_not_follow_the_marker_radius() {
        let tall = Layout::new(Size::new(280.0, 700.0), FRETS);
        let short = Layout::new(Size::new(280.0, 500.0), FRETS);

        // available_string_spacing = (280 - 40) / 5 = 48 → clamp(16.8, 8, 14) = 14
        // pad_top = 14 * 2 + 4 = 32, at both heights.
        assert_eq!(tall.top_y(), 32.0);
        assert_eq!(short.top_y(), 32.0);

        // The radius, meanwhile, is squeezed by the shorter neck's fret spacing.
        assert!(
            short.note_radius < tall.note_radius,
            "short {} should be tighter than tall {}",
            short.note_radius,
            tall.note_radius
        );
    }
}
