//! One voicing, drawn as a chord diagram.
//!
//! A sibling of [`super::fretboard`] rather than a mode on it. The neck there draws twelve
//! frets and has no way to say a string is *not* sounded — a mute is the absence of a
//! marker, annotated above the nut, not a marker at a position. Bolting a window offset, a
//! barre and a mute onto that module would make its round-trip invariant conditional on a
//! mode flag, and that invariant is the one thing standing between the app and a click
//! landing a fret away from the dot it visibly hit.
//!
//! So the invariant travels here instead, and the same `Layout` drives drawing and
//! hit-testing, held together by the same round-trip test. This widget's geometry is the
//! harder of the two, which is a reason to want that test more rather than less.

// Nothing outside the tests draws a diagram until the screen lands. `expect` rather than
// `allow`, so the attribute warns of its own accord once that happens.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "the widget precedes the screen that holds it")
)]

use iced::alignment;
use iced::widget::canvas::{self, Canvas, Fill, Frame, Path, Stroke, Text};
use iced::{Color, Element, Length, Pixels, Point, Rectangle, Renderer, Size, Theme};

/// The fewest frets a diagram shows, however small the shape.
///
/// Four, so that every diagram is the same height whatever it holds — a strip of them
/// reads as a row of chords rather than as a row of different-sized boxes.
const MIN_WINDOW: usize = 4;

/// How far up the neck a shape may sit before the diagram stops showing the nut.
///
/// A shape stopping at the second fret is still an open-position chord to look at, so its
/// diagram keeps the nut. Past that the nut is dead space and the window follows the hand.
const NUT_REACH: u8 = 2;

/// Which frets a diagram shows.
///
/// Derived from the voicing every time rather than stored beside it. Stored, the two could
/// disagree, and a diagram drawn against a stale window renders its dots at the wrong frets
/// while remaining perfectly legible — the worst failure a reference can have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// The lowest fret drawn. One means the window begins at the nut.
    pub first_fret: u8,
    pub fret_count: usize,
}

impl Window {
    pub fn shows_nut(self) -> bool {
        self.first_fret == 1
    }

    fn last_fret(self) -> u8 {
        self.first_fret + self.fret_count as u8 - 1
    }
}

/// The window that holds `strings`.
///
/// The nut is kept when any string rings open or nothing is stopped above `NUT_REACH`;
/// otherwise the window starts at the lowest stopped fret. The width is whichever is
/// larger, `MIN_WINDOW` or the reach from the first fret to the highest stopped one.
pub fn window_for(strings: &[Option<u8>; 6]) -> Window {
    let stopped: Vec<u8> = strings
        .iter()
        .flatten()
        .copied()
        .filter(|&f| f > 0)
        .collect();
    let has_open = strings.iter().flatten().any(|&fret| fret == 0);

    let Some(&lowest) = stopped.iter().min() else {
        // Nothing stopped at all — an all-open chord. The nut, and the default width.
        return Window {
            first_fret: 1,
            fret_count: MIN_WINDOW,
        };
    };

    let first_fret = if has_open || lowest <= NUT_REACH {
        1
    } else {
        lowest
    };
    let highest = stopped.iter().max().copied().unwrap_or(first_fret);
    let reach = usize::from(highest.saturating_sub(first_fret)) + 1;

    Window {
        first_fret,
        fret_count: reach.max(MIN_WINDOW),
    }
}

/// Where everything in the diagram sits, computed from the canvas bounds and the window.
///
/// Both drawing and hit-testing read this, for the reason `fretboard::Layout` gives: two
/// copies of the same arithmetic drift the moment either changes, and the symptom is a
/// press resolving one fret away from the dot it landed on.
struct Layout {
    string_count: usize,
    window: Window,
    left_edge: f32,
    string_spacing: f32,
    /// The nut's y, which is also the top of the first band.
    pad_top: f32,
    fret_spacing: f32,
    dot_radius: f32,
    mark_gap: f32,
}

impl Layout {
    const PAD_X: f32 = 14.0;
    const PAD_BOTTOM: f32 = 16.0;
    const MARK_GAP: f32 = 3.0;
    const STRING_COUNT: usize = 6;
    /// Room on the right for the `5fr` label, taken out of the width before the strings
    /// are spaced so the neck stays centred on what is left.
    const LABEL_GUTTER: f32 = 28.0;
    /// A dot has to hold a two-character note name — `Bb`, `F#` — at a size worth reading.
    /// These are what the strip's fixed width is chosen against; below the floor the labels
    /// stop being legible, which is the whole point of the screen.
    const MIN_DOT: f32 = 7.5;
    const MAX_DOT: f32 = 13.0;

    fn new(size: Size, window: Window) -> Self {
        let available_w = size.width - 2.0 * Self::PAD_X - Self::LABEL_GUTTER;
        let string_spacing = available_w / (Self::STRING_COUNT - 1) as f32;

        // The largest radius the diagram could use, not the one dots are drawn with:
        // `pad_top` has to reserve the mark row before `fret_spacing` is known, so it
        // cannot depend on the final radius. Folding these together moves the whole neck.
        let max_dot_radius = (string_spacing * 0.34).clamp(Self::MIN_DOT, Self::MAX_DOT);
        // Two bands above the nut, not one: the marks, and a line above them for the open
        // strings' labels. An open string has no filled disc to write on, so its name goes
        // over the ring — and with only the mark row reserved that lands off the canvas.
        let pad_top = max_dot_radius * 2.0 + Self::MARK_GAP + max_dot_radius * 1.4;

        let usable_h = size.height - pad_top - Self::PAD_BOTTOM;
        let fret_spacing = usable_h / window.fret_count as f32;
        let usable_w = string_spacing * (Self::STRING_COUNT - 1) as f32;

        Self {
            string_count: Self::STRING_COUNT,
            window,
            left_edge: (size.width - Self::LABEL_GUTTER - usable_w) / 2.0,
            string_spacing,
            pad_top,
            fret_spacing,
            dot_radius: (string_spacing * 0.34)
                .clamp(Self::MIN_DOT, Self::MAX_DOT)
                .min((fret_spacing * 0.34).clamp(Self::MIN_DOT, Self::MAX_DOT)),
            mark_gap: Self::MARK_GAP,
        }
    }

    fn string_x(&self, string: usize) -> f32 {
        self.left_edge + string as f32 * self.string_spacing
    }

    /// The y of the line `band` bands below the top. Band 0's line is the nut.
    fn band_y(&self, band: usize) -> f32 {
        self.pad_top + band as f32 * self.fret_spacing
    }

    fn top_y(&self) -> f32 {
        self.pad_top
    }

    fn bottom_y(&self) -> f32 {
        self.band_y(self.window.fret_count)
    }

    fn left_x(&self) -> f32 {
        self.left_edge
    }

    fn right_x(&self) -> f32 {
        self.string_x(self.string_count - 1)
    }

    /// Where the mark for `(string, fret)` is centred. Fret 0 is the mark row above the
    /// nut, which is where an open string and a muted one are both annotated.
    fn marker_center(&self, string: usize, fret: u8) -> Point {
        let y = if fret == 0 {
            self.pad_top - self.dot_radius - self.mark_gap
        } else {
            let band = usize::from(fret - self.window.first_fret);
            f32::midpoint(self.band_y(band), self.band_y(band + 1))
        };

        Point::new(self.string_x(string), y)
    }

    /// The inverse of `marker_center`.
    ///
    /// `None` off the diagram rather than a clamp onto the nearest edge — the reason
    /// `fretboard::Layout::position_at` gives, and it matters more here: this widget is
    /// built to be pressable by a later editing pass, and a stray press resolved onto a
    /// position would silently rewrite a chord.
    fn position_at(&self, point: Point) -> Option<(usize, u8)> {
        let slack = self.dot_radius;
        if point.x < self.left_x() - slack || point.x > self.right_x() + slack {
            return None;
        }

        let nearest = ((point.x - self.left_edge) / self.string_spacing).round() as isize;
        let string = nearest.clamp(0, self.string_count as isize - 1) as usize;

        let fret = if point.y < self.top_y() {
            // The mark row. It stands for the open string only when the nut is on show;
            // a window starting up the neck has no fret 0 to resolve to.
            if !self.window.shows_nut() {
                return None;
            }

            let mark_top = self.marker_center(string, 0).y - self.dot_radius;
            (point.y >= mark_top).then_some(0)?
        } else if point.y > self.bottom_y() {
            return None;
        } else {
            let band = ((point.y - self.top_y()) / self.fret_spacing).floor() as usize;
            let band = band.min(self.window.fret_count - 1);

            self.window.first_fret + band as u8
        };

        Some((string, fret))
    }
}

/// What one string of the diagram shows.
///
/// One struct per string rather than parallel arrays, so a fret and the label describing it
/// cannot end up on different strings. `label` is text already decided, as `NoteMarker`'s
/// is: this module draws marks and glyphs and knows nothing about music.
#[derive(Debug, Clone)]
pub struct StringMark {
    /// The fret stopped, `Some(0)` for an open string, `None` for one that is not sounded.
    pub fret: Option<u8>,
    pub label: String,
    /// Whether this string sounds the chord's root, which is drawn distinctly under every
    /// notation setting.
    pub is_root: bool,
}

/// A diagram, and optionally a way to press it.
///
/// Generic over `Message` only because `on_press` maps a position onto one — the
/// arrangement `Fretboard` uses, and a plain `fn` pointer for the same reason: every
/// call site passes a bare enum constructor, which captures nothing.
pub struct ChordDiagram<Message> {
    pub strings: [StringMark; 6],
    /// The fret a barre spans, drawn as one bar rather than as a dot per string.
    pub barre: Option<u8>,
    pub on_press: Option<fn(usize, usize) -> Message>,
}

impl<Message> ChordDiagram<Message> {
    fn window(&self) -> Window {
        let frets = std::array::from_fn(|index| self.strings[index].fret);

        window_for(&frets)
    }

    /// The span a barre covers, as the first and last string resting on it.
    fn barre_span(&self) -> Option<(usize, usize)> {
        let barre = self.barre?;
        let on_it: Vec<usize> = (0..6)
            .filter(|&index| self.strings[index].fret == Some(barre))
            .collect();

        Some((*on_it.first()?, *on_it.last()?))
    }
}

const INK: Color = Color::WHITE;
const GROUND: Color = Color::BLACK;
const FRET_LINE: Color = Color::from_rgb8(0x3a, 0x3a, 0x3a);
const STRING_LINE: Color = Color::from_rgb8(0x77, 0x77, 0x77);
/// The root's dot. Filled white reads loudest, and every other dot is the muted grey below
/// it, so the root is told apart before any label is read.
const ROOT_DOT: Color = Color::from_rgb8(0xff, 0xb3, 0x4d);
const TONE_DOT: Color = Color::from_rgb8(0xd0, 0xd0, 0xd0);

impl<Message> canvas::Program<Message> for ChordDiagram<Message> {
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
        let window = self.window();
        let layout = Layout::new(bounds.size(), window);

        frame.fill_rectangle(Point::ORIGIN, bounds.size(), GROUND);

        let (left_x, right_x) = (layout.left_x(), layout.right_x());
        let (top_y, bottom_y) = (layout.top_y(), layout.bottom_y());

        // The nut, thick, or an ordinary line where the window starts up the neck.
        let (nut_color, nut_width) = if window.shows_nut() {
            (INK, 3.0)
        } else {
            (FRET_LINE, 1.0)
        };
        frame.stroke(
            &Path::line(Point::new(left_x, top_y), Point::new(right_x, top_y)),
            Stroke::default()
                .with_color(nut_color)
                .with_width(nut_width),
        );

        for band in 1..=window.fret_count {
            let y = layout.band_y(band);
            frame.stroke(
                &Path::line(Point::new(left_x, y), Point::new(right_x, y)),
                Stroke::default().with_color(FRET_LINE).with_width(1.0),
            );
        }

        for string in 0..layout.string_count {
            let x = layout.string_x(string);
            frame.stroke(
                &Path::line(Point::new(x, top_y), Point::new(x, bottom_y)),
                Stroke::default().with_color(STRING_LINE).with_width(1.0),
            );
        }

        // The position label, where the nut is not on show.
        if !window.shows_nut() {
            frame.fill_text(Text {
                content: format!("{}fr", window.first_fret),
                position: Point::new(
                    right_x + layout.dot_radius * 1.4,
                    layout.marker_center(0, window.first_fret).y,
                ),
                color: STRING_LINE,
                size: Pixels(layout.dot_radius * 1.15),
                align_x: iced::widget::text::Alignment::Left,
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });
        }

        // The barre, drawn before the dots so a labelled dot still reads on top of it.
        if let (Some(barre), Some((first, last))) = (self.barre, self.barre_span()) {
            let y = layout.marker_center(first, barre).y;
            frame.stroke(
                &Path::line(
                    Point::new(layout.string_x(first), y),
                    Point::new(layout.string_x(last), y),
                ),
                Stroke::default()
                    .with_color(TONE_DOT)
                    .with_width(layout.dot_radius * 1.7),
            );
        }

        for (string, mark) in self.strings.iter().enumerate() {
            let center = layout.marker_center(string, mark.fret.unwrap_or(0));

            match mark.fret {
                // Not sounded: a cross above the nut, and nothing on the neck.
                None => {
                    let arm = layout.dot_radius * 0.7;
                    let stroke = Stroke::default().with_color(STRING_LINE).with_width(1.6);

                    for (dx, dy) in [(-arm, -arm), (-arm, arm)] {
                        frame.stroke(
                            &Path::line(
                                Point::new(center.x + dx, center.y + dy),
                                Point::new(center.x - dx, center.y - dy),
                            ),
                            stroke,
                        );
                    }
                    continue;
                }
                // Open: a ring above the nut, never a filled dot — a filled mark up there
                // would read as a note being stopped at the nut.
                Some(0) => {
                    frame.stroke(
                        &Path::circle(center, layout.dot_radius * 0.72),
                        Stroke::default()
                            .with_color(if mark.is_root { ROOT_DOT } else { STRING_LINE })
                            .with_width(1.6),
                    );
                }
                Some(_) => {
                    let color = if mark.is_root { ROOT_DOT } else { TONE_DOT };
                    frame.fill(&Path::circle(center, layout.dot_radius), Fill::from(color));
                }
            }

            if mark.label.is_empty() {
                continue;
            }

            // On the dot where there is one, and beside the ring where the string is open:
            // an open string's label has no filled disc to sit on.
            let (position, color) = match mark.fret {
                Some(0) => (
                    Point::new(center.x, center.y - layout.dot_radius * 1.4),
                    INK,
                ),
                _ => (center, GROUND),
            };

            // A three-character label — `Bbb` on a diminished seventh, `bb7` as its degree —
            // does not fit a disc sized for `Bb`. Shrunk rather than clipped or the disc
            // grown, because the discs have to stay one size across a strip to read as a
            // row of chords.
            let size = match mark.label.chars().count() {
                0..=2 => layout.dot_radius * 1.15,
                _ => layout.dot_radius * 0.8,
            };

            frame.fill_text(Text {
                content: mark.label.clone(),
                position,
                color,
                size: Pixels(size),
                align_x: iced::widget::text::Alignment::Center,
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });
        }

        vec![frame.into_geometry()]
    }

    /// Turns a press into the caller's message.
    ///
    /// `&self`, so nothing is cached between events and the layout is rebuilt from
    /// `bounds` — which is what guarantees hit-testing uses the geometry the last `draw`
    /// used.
    fn update(
        &self,
        _state: &mut (),
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let on_press = self.on_press?;

        let iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) =
            event
        else {
            return None;
        };

        let point = cursor.position_in(bounds)?;
        let (string, fret) = Layout::new(bounds.size(), self.window()).position_at(point)?;

        Some(canvas::Action::publish(on_press(string, usize::from(fret))).and_capture())
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
                .and_then(|point| Layout::new(bounds.size(), self.window()).position_at(point))
                .is_some();

        if over_a_position {
            iced::mouse::Interaction::Pointer
        } else {
            iced::mouse::Interaction::default()
        }
    }
}

/// The two sizes a diagram is drawn at.
///
/// The gap between them is the point: the picked shape is the one being read, and the rest
/// are a row of options to pick from. A dot's radius follows the width, so `FEATURE`'s
/// labels come out around half again as large as `STRIP`'s rather than merely further apart
/// — which is what makes the picked one legible across the room and the others scannable.
/// The heights are chosen against the widths, not picked separately: a fret band much
/// taller than a string is wide stops looking like a fretboard and starts looking like a
/// ladder. `fretboard::Layout` targets the same proportion with `TARGET_FRET_CELL_RATIO`;
/// `a_cell_is_about_as_tall_as_it_is_wide` is what holds these two to it.
pub const STRIP: Size = Size::new(180.0, 188.0);
pub const FEATURE: Size = Size::new(264.0, 292.0);

pub fn chord_diagram<Message: 'static>(
    diagram: ChordDiagram<Message>,
    size: Size,
) -> Element<'static, Message> {
    Canvas::new(diagram)
        .width(Length::Fixed(size.width))
        .height(Length::Fixed(size.height))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sizes a diagram is plausibly drawn at. The widget is fixed, but a strip of them may
    /// be laid out tighter on a narrow window.
    /// Both sizes the strip draws, plus two the layout could squeeze them to.
    const SIZES: [Size; 4] = [
        STRIP,
        FEATURE,
        Size::new(140.0, 180.0),
        Size::new(300.0, 300.0),
    ];

    fn frets(values: [Option<u8>; 6]) -> [Option<u8>; 6] {
        values
    }

    /// The property that makes hit-testing trustworthy, and the reason `Layout` exists:
    /// the centre of every mark the window contains resolves back to it. Edited apart,
    /// drawing and hit-testing fail this.
    #[test]
    fn every_marker_center_round_trips() {
        let windows = [
            Window {
                first_fret: 1,
                fret_count: 4,
            },
            Window {
                first_fret: 5,
                fret_count: 4,
            },
            Window {
                first_fret: 9,
                fret_count: 5,
            },
        ];

        for size in SIZES {
            for window in windows {
                let layout = Layout::new(size, window);
                let first = if window.shows_nut() {
                    0
                } else {
                    window.first_fret
                };

                for string in 0..layout.string_count {
                    for fret in first..=window.last_fret() {
                        if fret != 0 && fret < window.first_fret {
                            continue;
                        }

                        let center = layout.marker_center(string, fret);

                        assert_eq!(
                            layout.position_at(center),
                            Some((string, fret)),
                            "{size:?} {window:?} lost ({string}, {fret})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_press_off_the_diagram_resolves_to_nothing() {
        let window = Window {
            first_fret: 1,
            fret_count: 4,
        };
        let layout = Layout::new(SIZES[0], window);

        // Past the outer strings, and below the last fret line.
        assert_eq!(
            layout.position_at(Point::new(layout.left_x() - 40.0, layout.top_y() + 10.0)),
            None
        );
        assert_eq!(
            layout.position_at(Point::new(layout.right_x() + 40.0, layout.top_y() + 10.0)),
            None
        );
        assert_eq!(
            layout.position_at(Point::new(layout.string_x(0), layout.bottom_y() + 20.0)),
            None
        );
        // Above the mark row entirely.
        assert_eq!(
            layout.position_at(Point::new(layout.string_x(0), -20.0)),
            None
        );
    }

    #[test]
    fn a_window_up_the_neck_has_no_open_position() {
        // The mark row still draws mutes there, but nothing above the nut is a fret.
        let layout = Layout::new(
            SIZES[0],
            Window {
                first_fret: 5,
                fret_count: 4,
            },
        );

        assert_eq!(
            layout.position_at(Point::new(layout.string_x(2), layout.top_y() - 4.0)),
            None
        );
    }

    #[test]
    fn a_voicing_at_the_nut_keeps_the_nut() {
        // E major, and an all-open chord.
        assert_eq!(
            window_for(&frets([
                Some(0),
                Some(2),
                Some(2),
                Some(1),
                Some(0),
                Some(0)
            ])),
            Window {
                first_fret: 1,
                fret_count: 4
            }
        );
        assert_eq!(
            window_for(&frets([Some(0); 6])),
            Window {
                first_fret: 1,
                fret_count: 4
            }
        );
    }

    #[test]
    fn a_voicing_low_on_the_neck_keeps_the_nut() {
        // Stopping at the second fret still reads as an open-position chord.
        let window = window_for(&frets([None, Some(2), Some(2), Some(2), None, None]));

        assert!(window.shows_nut());
    }

    #[test]
    fn a_voicing_up_the_neck_starts_at_its_lowest_fret() {
        // A barre at the eighth, spanning to the tenth.
        let window = window_for(&frets([
            Some(8),
            Some(10),
            Some(10),
            Some(9),
            Some(8),
            Some(8),
        ]));

        assert_eq!(window.first_fret, 8);
        assert!(!window.shows_nut());
        // Three frets of reach, widened to the minimum so every diagram is one height.
        assert_eq!(window.fret_count, MIN_WINDOW);
    }

    #[test]
    fn a_window_contains_every_stopped_fret() {
        let cases = [
            frets([Some(0), Some(2), Some(2), Some(1), Some(0), Some(0)]),
            frets([None, Some(3), Some(2), Some(0), Some(1), Some(0)]),
            frets([Some(8), Some(10), Some(10), Some(9), Some(8), Some(8)]),
            frets([None, None, Some(11), Some(12), Some(12), None]),
        ];

        for voicing in cases {
            let window = window_for(&voicing);

            for fret in voicing.iter().flatten().filter(|&&f| f > 0) {
                assert!(
                    (window.first_fret..=window.last_fret()).contains(fret),
                    "{window:?} does not hold fret {fret} of {voicing:?}"
                );
            }
        }
    }

    #[test]
    fn the_window_follows_the_voicing() {
        // The whole reason it is derived rather than stored.
        let low = window_for(&frets([
            Some(0),
            Some(2),
            Some(2),
            Some(1),
            Some(0),
            Some(0),
        ]));
        let high = window_for(&frets([
            Some(8),
            Some(10),
            Some(10),
            Some(9),
            Some(8),
            Some(8),
        ]));

        assert_ne!(low.first_fret, high.first_fret);
    }

    #[test]
    fn a_barre_spans_the_strings_resting_on_it() {
        let diagram: ChordDiagram<()> = ChordDiagram {
            strings: std::array::from_fn(|index| StringMark {
                fret: Some([5, 7, 7, 6, 5, 5][index]),
                label: String::new(),
                is_root: index == 0,
            }),
            barre: Some(5),
            on_press: None,
        };

        // Strings 0, 4 and 5 rest on the fifth; the bar runs from the first to the last.
        assert_eq!(diagram.barre_span(), Some((0, 5)));
    }

    #[test]
    fn a_cell_is_about_as_tall_as_it_is_wide() {
        // Not a rule about pixels but about proportion: a diagram whose bands stretch far
        // past its string spacing reads as a ladder. Sizing the two constants by eye is
        // what this catches when one of them is later nudged.
        let window = Window {
            first_fret: 1,
            fret_count: 4,
        };

        for size in [STRIP, FEATURE] {
            let layout = Layout::new(size, window);
            let ratio = layout.fret_spacing / layout.string_spacing;

            assert!(
                (1.1..=1.5).contains(&ratio),
                "{size:?} draws cells at a ratio of {ratio}"
            );
        }
    }

    #[test]
    fn the_featured_size_draws_larger_dots_than_the_strip() {
        // What makes a picked shape read as picked, and the reason both sizes are named
        // constants: a label scales with the dot, so this is the legibility difference.
        let window = Window {
            first_fret: 1,
            fret_count: 4,
        };

        assert!(
            Layout::new(FEATURE, window).dot_radius > Layout::new(STRIP, window).dot_radius * 1.3,
            "the featured diagram is not meaningfully bigger"
        );
    }

    #[test]
    fn the_widget_builds() {
        // The counterpart of `every_screen_builds_its_view`: nothing here renders, but the
        // canvas has to assemble into an `Element` before the strip can hold one.
        let diagram: ChordDiagram<()> = ChordDiagram {
            strings: std::array::from_fn(|index| StringMark {
                fret: [None, Some(3), Some(2), Some(0), Some(1), Some(0)][index],
                label: "1".to_string(),
                is_root: index == 1,
            }),
            barre: None,
            on_press: None,
        };

        let _: Element<'static, ()> = chord_diagram(diagram, STRIP);
    }

    #[test]
    fn no_barre_spans_nothing() {
        let diagram: ChordDiagram<()> = ChordDiagram {
            strings: std::array::from_fn(|_| StringMark {
                fret: Some(0),
                label: String::new(),
                is_root: false,
            }),
            barre: None,
            on_press: None,
        };

        assert_eq!(diagram.barre_span(), None);
    }
}
