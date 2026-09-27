//! The Tuner: what the app hears, named and measured.
//!
//! The screen is chromatic rather than six-string — it names the nearest of all twelve
//! pitch classes in whatever octave it hears, and never snaps to an open string. A capo
//! and an alternate tuning then need nothing said about them. The cost is stated rather
//! than hidden: a string tuned a full semitone flat reads as the note it has become.
//!
//! The dial is the readout and the name is a label on it. That is not a presentation
//! preference — a pitch moving smoothly across a semitone boundary changes its name
//! abruptly while the sound does not, and a bend measured here crossed three names while
//! travelling continuously.

use std::f32::consts::{FRAC_PI_2, PI};

use iced::widget::canvas::{self, Canvas, Path, Stroke, Text};
use iced::widget::{Space, column, container, row, stack, text};
use iced::{
    Color, Element, Length, Padding, Pixels, Point, Radians, Rectangle, Renderer, Size, Theme,
};

use crate::music::notes::Spelling;

use super::focus::FocusTarget;
use super::listener::{Listener, State, ui_device_line, ui_device_picker};
use super::style::{
    BODY, INK, MUSIC_FONT, MUTE, SMUFL_FLAT, SMUFL_SHARP, SUCCESS, control_accidental,
    control_button, spelling_glyph,
};
use super::{Message, note_label};

/// How far from a pitch still counts as in tune.
///
/// Three cents rather than one: a guitar's own intonation moves a fretted note further
/// than that, and a reading that refuses to settle is a reading nobody trusts.
const IN_TUNE_CENTS: f32 = 3.0;

/// The widest deviation the dial shows. Past this the reading is the next note along,
/// since `Pitch::nearest` never returns an offset beyond half a semitone.
const DIAL_RANGE_CENTS: f32 = 50.0;

/// The dial's box. Fixed rather than filled: an arc is read as a shape, and a shape that
/// changes proportion with the window is a different shape every time it is looked at.
const DIAL_SIZE: Size = Size::new(460.0, 235.0);

/// The dial's track — between the app's hairline and its muted text, because the arc is a
/// scale to read the dot against rather than a thing to read on its own.
const TRACK: Color = Color::from_rgb8(0x55, 0x55, 0x55);

pub(super) struct Tuner {
    /// The device and what it has heard. Owned rather than shared: two listening screens
    /// each hold one, and only one of them ever has a stream open — `leaving_for` closes
    /// it on the way out, which is what keeps that true.
    listener: Listener,
    /// How the pitch is named. Its own, like the Note Trainer's — a spelling chosen while
    /// tuning has no business renaming anything else.
    spelling: Spelling,
}

impl Tuner {
    pub(super) fn new() -> Tuner {
        Tuner {
            listener: Listener::new(),
            spelling: Spelling::Sharps,
        }
    }

    /// The device, for `App` to drive and for the tests to read. The views below reach the
    /// field directly, being in this module.
    pub(super) fn listener(&self) -> &Listener {
        &self.listener
    }

    pub(super) fn listener_mut(&mut self) -> &mut Listener {
        &mut self.listener
    }

    /// Read by the screen's tests. The views are in this module and reach the field.
    #[cfg(test)]
    pub(super) fn spelling(&self) -> Spelling {
        self.spelling
    }

    pub(super) fn toggle_spelling(&mut self) {
        self.spelling = match self.spelling {
            Spelling::Sharps => Spelling::Flats,
            Spelling::Flats => Spelling::Sharps,
        };
    }

    /// Which way the string has to move, or `None` when it is in tune or nothing is
    /// sounding. The view draws from this rather than comparing cents itself, so the mark
    /// and the "in tune" colouring cannot disagree about where the boundary is.
    pub(super) fn correction(&self) -> Option<Correction> {
        let (_, cents) = self.listener.reading()?;

        if cents.value() > IN_TUNE_CENTS {
            Some(Correction::Down)
        } else if cents.value() < -IN_TUNE_CENTS {
            Some(Correction::Up)
        } else {
            None
        }
    }
}

/// Which way a string has to be turned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Correction {
    /// Sharp: slacken it.
    Down,
    /// Flat: tighten it.
    Up,
}

pub(super) fn ui_tuner(tuner: &Tuner, focused: FocusTarget) -> Element<'static, Message> {
    let body = if tuner.listener.is_picking() {
        ui_device_picker(&tuner.listener)
    } else {
        ui_reading(tuner, focused)
    };

    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .padding(Padding::from([24, 24]))
        .into()
}

/// The reading itself: the dial, the name sitting inside it, and what the screen is doing.
fn ui_reading(tuner: &Tuner, focused: FocusTarget) -> Element<'static, Message> {
    let state = tuner.listener.state();

    let in_tune = tuner
        .listener
        .reading()
        .is_some_and(|(_, cents)| cents.value().abs() <= IN_TUNE_CENTS);
    let colour = if in_tune { SUCCESS } else { INK };

    // `note_label` rather than `format!`: it draws the accidental as the music face's own
    // glyph, which is what every other note in the app is drawn with.
    let name: Element<'static, Message> = match tuner.listener.reading() {
        Some((pitch, _)) => row![
            note_label(tuner.spelling.spell(pitch.pitch_class()), NAME_SIZE, colour),
            text(pitch.octave().to_string())
                .size(OCTAVE_SIZE)
                .color(colour),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(4)
        .into(),
        // At the name's own size an em dash sets as a rule across the dial. Small enough
        // to read as a dash, and still holding the slot so nothing shifts when a note lands.
        None => text("—").size(NAME_SIZE / 2.4).color(MUTE).into(),
    };

    // Two readings of the same thing, side by side: the hertz is what was measured and the
    // cents is that measurement against the note it was given, so a learner who does not
    // yet trust the dial has both halves of its reasoning.
    let (hz_label, cents_label) = match (tuner.listener.hz(), tuner.listener.reading()) {
        (Some(hz), Some((_, cents))) => (
            format!("HZ {hz:.1}"),
            // `+ 0.0` because rounding keeps the sign it rounded away: a hair flat would
            // otherwise print "-0 cents", which reads as a fault in the tuner.
            format!("{:+.0} cents", cents.value().round() + 0.0),
        ),
        _ => (String::new(), String::new()),
    };

    let status = text(match state {
        State::NotStarted => "not listening",
        State::NoInput => "no input",
        State::Silent => "listening, play a note...",
        State::Unnameable => "one string at a time",
        // The dial says which way by where the dot sits; this says it in words, so the
        // learner turning the peg does not have to hold "left means flat" as well.
        State::Reading => match tuner.correction() {
            None => "in tune",
            Some(Correction::Up) => "flat — tighten it",
            Some(Correction::Down) => "sharp — slacken it",
        },
    })
    .size(14)
    .color(if in_tune { SUCCESS } else { MUTE });

    column![
        ui_device_line(&tuner.listener, focused),
        Space::new().height(Length::Fixed(16.0)),
        ui_dial(tuner, name),
        row![
            text(hz_label).size(15).color(MUTE),
            Space::new().width(Length::Fixed(64.0)),
            text(cents_label)
                .size(15)
                .color(if in_tune { SUCCESS } else { BODY }),
        ],
        Space::new().height(Length::Fixed(14.0)),
        status,
        Space::new().height(Length::Fixed(14.0)),
        control_button(
            control_accidental(spelling_glyph(tuner.spelling)),
            Message::ToggleTunerSpelling,
            focused == FocusTarget::TunerSpellingToggle,
        ),
    ]
    .align_x(iced::Alignment::Center)
    .spacing(4)
    .into()
}

/// How big the note name and its octave are drawn, inside the arc.
const NAME_SIZE: f32 = 128.0;
const OCTAVE_SIZE: f32 = 44.0;

/// The dial, with the note name stacked over it.
///
/// A `stack` rather than canvas text because the name is two glyphs in two faces — the
/// letter in the body font, the accidental in the music one — and `note_label` already
/// composes that pair for every other screen. Placing it needs the canvas's geometry, so
/// [`Dial`] hands out the padding rather than the view guessing at it.
fn ui_dial<'a>(tuner: &Tuner, name: Element<'a, Message>) -> Element<'a, Message> {
    let dial = Dial::new(DIAL_SIZE);

    let gauge = Gauge {
        position: tuner
            .listener
            .reading()
            .map(|(_, cents)| position_for(cents.value())),
        in_tune: tuner
            .listener
            .reading()
            .is_some_and(|(_, cents)| cents.value().abs() <= IN_TUNE_CENTS),
    };

    stack![
        Canvas::new(gauge)
            .width(Length::Fixed(DIAL_SIZE.width))
            .height(Length::Fixed(DIAL_SIZE.height)),
        container(name)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .padding(dial.name_padding()),
    ]
    .width(Length::Fixed(DIAL_SIZE.width))
    .height(Length::Fixed(DIAL_SIZE.height))
    .into()
}

/// Where a reading sits on the track: −1 half a semitone flat, +1 half a semitone sharp.
///
/// Clamped even though `Pitch::nearest` never returns an offset outside the range — the
/// clamp is what makes that a fact about this function rather than a fact about its caller.
fn position_for(cents: f32) -> f32 {
    (cents / DIAL_RANGE_CENTS).clamp(-1.0, 1.0)
}

/// Where every part of the dial sits, derived once from its box.
///
/// One struct for the same reason [`super::fretboard::Layout`] is one: two things read this
/// geometry — the canvas that draws the arc and the overlay that centres the note name
/// inside it — and a second copy of the arithmetic drifts the moment either changes. There
/// is no hit-testing here and so no round-trip test; what stands in for it is a test that
/// pins the track's three landmarks and one that checks the name lands where the dial says.
struct Dial {
    size: Size,
    center: Point,
    radius: f32,
}

impl Dial {
    /// How far either side of the top the track runs. Eighty degrees rather than ninety, so
    /// the ends sit above the arc's widest point and lean towards the reader.
    const HALF_SWEEP: f32 = 80.0 * PI / 180.0;
    /// Room above the top of the track for the target mark.
    const PAD_TOP: f32 = 40.0;
    /// Room outside the ends of the track for the ♭ and the ♯.
    const EDGE: f32 = 56.0;
    const TRACK_WIDTH: f32 = 3.0;
    const DOT_RADIUS: f32 = 9.0;
    /// The target mark: a triangle this wide and this tall, its point this far above the
    /// track's top.
    const MARK_HALF_WIDTH: f32 = 8.0;
    const MARK_HEIGHT: f32 = 10.0;
    const MARK_LIFT: f32 = 24.0;
    /// How far beyond the track's radius the accidentals sit, and how big they are drawn.
    ///
    /// The size is an em rather than a height: a SMuFL glyph is drawn against a four-space
    /// stave, so a flat at 44 sets about as tall as a letter at 26.
    const LABEL_GAP: f32 = 32.0;
    const LABEL_SIZE: f32 = 44.0;

    fn new(size: Size) -> Dial {
        // Whichever bound bites first: the width at the track's widest point, or the height
        // from the mark above the top down to the ends.
        let by_width = (size.width / 2.0 - Dial::EDGE) / Dial::HALF_SWEEP.sin();
        let by_height = (size.height - Dial::PAD_TOP - Dial::EDGE) / (1.0 - Dial::HALF_SWEEP.cos());
        let radius = by_width.min(by_height).max(1.0);

        Dial {
            size,
            center: Point::new(size.width / 2.0, Dial::PAD_TOP + radius),
            radius,
        }
    }

    /// The angle a position on the track sits at, measured the way `canvas` measures: zero
    /// at the positive x-axis, growing clockwise, so straight up is −π/2.
    fn angle_at(&self, position: f32) -> f32 {
        -FRAC_PI_2 + position * Dial::HALF_SWEEP
    }

    /// A point `out` from the centre along the track's angle at `position`.
    fn radial(&self, position: f32, out: f32) -> Point {
        let angle = self.angle_at(position);
        Point::new(
            self.center.x + (self.radius + out) * angle.cos(),
            self.center.y + (self.radius + out) * angle.sin(),
        )
    }

    fn point_at(&self, position: f32) -> Point {
        self.radial(position, 0.0)
    }

    /// Where the note name's centre belongs: a shade below the middle of the bowl, which is
    /// where a run of cap-height glyphs looks centred rather than measures centred.
    fn name_center(&self) -> Point {
        let top = self.point_at(0.0).y;
        let ends = self.point_at(1.0).y;

        Point::new(self.center.x, top + (ends - top) * 0.58)
    }

    /// The padding that drops a vertically centred overlay onto [`Dial::name_center`].
    ///
    /// Centring in a box of height `h` with padding `p` on one side puts the content's
    /// middle at `(h ∓ p) / 2`, so the shift wanted decides which side the padding goes on.
    fn name_padding(&self) -> Padding {
        let shift = 2.0 * self.name_center().y - self.size.height;

        if shift >= 0.0 {
            Padding::ZERO.top(shift)
        } else {
            Padding::ZERO.bottom(-shift)
        }
    }
}

/// The dial as the canvas draws it: a track, the target, and the reading's place on it.
struct Gauge {
    /// Where the reading sits on the track, or `None` when nothing nameable is sounding —
    /// which draws the track with no dot on it. Deliberately not a dot at the top, which
    /// reads as a note that is perfectly in tune.
    position: Option<f32>,
    in_tune: bool,
}

impl<Message> canvas::Program<Message> for Gauge {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let dial = Dial::new(bounds.size());
        let lit = if self.in_tune { SUCCESS } else { INK };

        let track = Path::new(|path| {
            path.arc(canvas::path::Arc {
                center: dial.center,
                radius: dial.radius,
                start_angle: Radians(dial.angle_at(-1.0)),
                end_angle: Radians(dial.angle_at(1.0)),
            });
        });
        frame.stroke(
            &track,
            Stroke {
                line_cap: canvas::LineCap::Round,
                ..Stroke::default()
                    .with_color(TRACK)
                    .with_width(Dial::TRACK_WIDTH)
            },
        );

        // The target: a triangle pointing down at the top of the track. Fixed, so the dot
        // has something to arrive at rather than only a curve to sit somewhere on.
        let tip = dial.radial(0.0, Dial::MARK_LIFT);
        let mark = Path::new(|path| {
            path.move_to(Point::new(
                tip.x - Dial::MARK_HALF_WIDTH,
                tip.y - Dial::MARK_HEIGHT,
            ));
            path.line_to(Point::new(
                tip.x + Dial::MARK_HALF_WIDTH,
                tip.y - Dial::MARK_HEIGHT,
            ));
            path.line_to(tip);
            path.close();
        });
        frame.fill(&mark, if self.in_tune { SUCCESS } else { TRACK });

        for (glyph, position) in [(SMUFL_FLAT, -1.0), (SMUFL_SHARP, 1.0)] {
            frame.fill_text(Text {
                content: glyph.to_string(),
                position: dial.radial(position, Dial::LABEL_GAP),
                color: MUTE,
                size: Pixels(Dial::LABEL_SIZE),
                font: MUSIC_FONT,
                align_x: iced::widget::text::Alignment::Center,
                align_y: iced::alignment::Vertical::Center,
                // The accidentals are private-use codepoints, which basic shaping maps by
                // luck rather than by rule.
                shaping: iced::widget::text::Shaping::Advanced,
                ..Text::default()
            });
        }

        if let Some(position) = self.position {
            frame.fill(
                &Path::circle(dial.point_at(position), Dial::DOT_RADIUS),
                lit,
            );
        }

        vec![frame.into_geometry()]
    }
}

/// The dial's geometry, which nothing else can assert on: the canvas draws into a frame the
/// tests cannot read back, so what is pinned here is the arithmetic the drawing reads.
#[cfg(test)]
mod dial_tests {
    use super::*;

    /// A sweep across the whole track, at a resolution finer than a pixel of movement.
    fn positions() -> impl Iterator<Item = f32> {
        (0u8..=200).map(|step| f32::from(step) / 100.0 - 1.0)
    }

    #[test]
    fn the_track_runs_from_flat_up_over_the_top_and_down_to_sharp() {
        let dial = Dial::new(DIAL_SIZE);
        let (flat, top, sharp) = (dial.point_at(-1.0), dial.point_at(0.0), dial.point_at(1.0));

        assert!(
            flat.x < top.x && top.x < sharp.x,
            "flat is not left of sharp"
        );
        assert!(
            (top.x - DIAL_SIZE.width / 2.0).abs() < 0.5,
            "the top of the track is off centre"
        );
        // Canvas y grows downward, so the top of the arc is the smallest y.
        assert!(top.y < flat.y && top.y < sharp.y, "the arc is upside down");
        assert!(
            (flat.y - sharp.y).abs() < 0.5,
            "the two ends sit at different heights"
        );
    }

    #[test]
    fn cents_land_on_the_track_where_they_are_read() {
        assert_eq!(position_for(0.0), 0.0);
        assert_eq!(position_for(-DIAL_RANGE_CENTS), -1.0);
        assert_eq!(position_for(DIAL_RANGE_CENTS), 1.0);
        // Past the range the dot stops at the end rather than leaving the arc — a reading
        // that far out is the next note along and the name has already changed.
        assert_eq!(position_for(DIAL_RANGE_CENTS * 2.0), 1.0);
    }

    #[test]
    fn nothing_the_dial_draws_leaves_its_box() {
        let dial = Dial::new(DIAL_SIZE);
        let inside = |point: Point, slack: f32| {
            point.x - slack >= 0.0
                && point.x + slack <= DIAL_SIZE.width
                && point.y - slack >= 0.0
                && point.y + slack <= DIAL_SIZE.height
        };

        assert!(
            positions().all(|position| inside(dial.point_at(position), Dial::DOT_RADIUS)),
            "the dot overhangs the dial somewhere along the track"
        );
        assert!(
            inside(dial.radial(0.0, Dial::MARK_LIFT), Dial::MARK_HEIGHT),
            "the target mark is clipped off the top"
        );
        for end in [-1.0, 1.0] {
            assert!(
                inside(dial.radial(end, Dial::LABEL_GAP), Dial::LABEL_SIZE / 2.0),
                "the accidental at {end} is clipped off the edge"
            );
        }
    }

    #[test]
    fn the_note_name_lands_where_the_dial_puts_it() {
        let dial = Dial::new(DIAL_SIZE);
        let name = dial.name_center();

        assert!(
            name.y > dial.point_at(0.0).y && name.y < dial.point_at(0.0).y + dial.radius,
            "the name is not inside the bowl of the arc"
        );

        // The padding is what actually places it, and the canvas never sees the padding —
        // this is the seam where the two readers of the geometry could drift apart.
        let padding = dial.name_padding();
        let placed = (DIAL_SIZE.height + padding.top - padding.bottom) / 2.0;
        assert!(
            (placed - name.y).abs() < 0.5,
            "centring with the dial's padding lands at {placed}, not {}",
            name.y
        );
    }
}

#[cfg(test)]
mod screen_tests {
    use crate::audio::detect::{Detection, Heard};
    use crate::music::notes::Letter;
    use crate::ui::keys::accelerators_for;
    use crate::ui::tests::press_into;
    use crate::ui::*;

    /// A released reading at a given frequency, as the detector would produce one.
    fn heard(hz: f32) -> Heard {
        Heard::Pitch(Detection::synthesized(hz, 0.999))
    }

    /// Drives the app to the Tuner and feeds it a reading, with no audio anywhere.
    fn tuned_to(hz: f32) -> App {
        let (mut app, _) = App::new();
        let _ = app.update(Message::Navigate(Screen::Tuner));
        let _ = app.update(Message::Heard(heard(hz)));
        app
    }

    #[test]
    fn the_tuner_is_reachable_and_leavable_like_every_other_screen() {
        let (mut app, _) = App::new();

        // Present in the menu, and as the eighth entry it has a digit.
        let entry = HOME_MENU
            .iter()
            .position(|item| item.screen == Screen::Tuner)
            .expect("the Tuner is a Home menu entry");
        assert!(
            accelerators_for(&Screen::Home, Notation::Notes, &WalkTheScale::new())
                .iter()
                .any(|(key, target, _)| *key == '8'
                    && *target == Fires::Widget(FocusTarget::HomeMenuItem(entry))),
            "the Tuner has no digit on the Home menu"
        );

        let _ = app.update(Message::Navigate(Screen::Tuner));
        assert_eq!(app.screen, Screen::Tuner);

        let _ = app.update(Message::GoBack);
        assert_eq!(app.screen, Screen::Home, "Esc did not leave the Tuner");
    }

    #[test]
    fn an_open_string_is_named_with_its_octave() {
        // 82.41 Hz is the open low E. Naming it "E" would be the pitch-class answer, and
        // the whole reason this screen needed absolute pitch.
        let app = tuned_to(82.41);
        let (pitch, cents) = app
            .tuner
            .listener()
            .reading()
            .expect("a reading was applied");

        assert_eq!(pitch.pitch_class(), PitchClass::new(4));
        assert_eq!(
            pitch.octave(),
            2,
            "named E{} rather than E2",
            pitch.octave()
        );
        assert!(cents.value().abs() < 1.0);
    }

    #[test]
    fn a_badly_mistuned_string_reads_as_what_it_is() {
        // A low E tuned a semitone flat sounds D#2, and the screen says so rather than
        // calling it a very flat E. The trade chromatic buys, stated in the spec.
        let app = tuned_to(77.78);
        let (pitch, _) = app.tuner.listener().reading().expect("a reading");

        assert_eq!(pitch.pitch_class(), PitchClass::new(3));
        assert_eq!(pitch.octave(), 2);
    }

    #[test]
    fn an_alternate_tuning_needs_no_setting() {
        // Drop D: the lowest string at D2, with nothing having been configured.
        let app = tuned_to(73.42);
        let (pitch, cents) = app.tuner.listener().reading().expect("a reading");

        assert_eq!(pitch.pitch_class(), PitchClass::new(2));
        assert_eq!(pitch.octave(), 2);
        assert!(cents.value().abs() < 2.0);
    }

    #[test]
    fn sharp_reads_positive_and_flat_reads_negative() {
        let sharp = tuned_to(440.0 * 2.0f32.powf(10.0 / 1200.0));
        assert!(
            sharp
                .tuner
                .listener()
                .reading()
                .expect("a reading")
                .1
                .value()
                > 0.0
        );

        let flat = tuned_to(440.0 * 2.0f32.powf(-10.0 / 1200.0));
        assert!(
            flat.tuner
                .listener()
                .reading()
                .expect("a reading")
                .1
                .value()
                < 0.0
        );
    }

    #[test]
    fn the_four_states_are_distinguishable() {
        let (mut app, _) = App::new();

        // Before the screen is entered, nothing is being listened to.
        assert_eq!(app.tuner.listener().state(), listener::State::NotStarted);

        let _ = app.update(Message::Navigate(Screen::Tuner));
        // Whether a device opened depends on the machine, so both are legitimate here —
        // what matters is that silence and a reading are distinct from each other and
        // from a failure, which the arms below assert.
        let opened = app.tuner.listener().error().is_none();

        let _ = app.update(Message::Heard(Heard::Silence));
        assert_eq!(
            app.tuner.listener().state(),
            if opened {
                listener::State::Silent
            } else {
                listener::State::NoInput
            }
        );

        if opened {
            let _ = app.update(Message::Heard(Heard::Unpitched));
            assert_eq!(app.tuner.listener().state(), listener::State::Unnameable);

            let _ = app.update(Message::Heard(heard(440.0)));
            assert_eq!(app.tuner.listener().state(), listener::State::Reading);
        }
    }

    #[test]
    fn a_reading_does_not_outlive_the_sound() {
        let mut app = tuned_to(440.0);
        assert!(app.tuner.listener().reading().is_some());

        let _ = app.update(Message::Heard(Heard::Silence));
        assert!(
            app.tuner.listener().reading().is_none(),
            "a stale reading survived the silence after it"
        );
    }

    #[test]
    fn a_strum_names_nothing() {
        let mut app = tuned_to(440.0);

        // What the detector releases for a chord: nothing nameable, at any level.
        let _ = app.update(Message::Heard(Heard::Unpitched));
        assert!(app.tuner.listener().reading().is_none());
    }

    #[test]
    fn the_device_list_opens_walks_and_closes() {
        let (mut app, _) = App::new();
        let _ = app.update(Message::Navigate(Screen::Tuner));
        assert!(!app.tuner.listener().is_picking());

        let _ = app.update(Message::OpenDevices);
        assert!(app.tuner.listener().is_picking(), "the list did not open");

        // The list claims the vertical pair while it is up, and clamps rather than wraps.
        let offered = app.tuner.listener().devices().len();
        let _ = app.update(Message::FocusDown);
        let _ = app.update(Message::FocusDown);
        let walked = app.tuner.listener().picking().expect("still picking");
        assert!(walked < offered.max(1), "the highlight ran off the end");

        let _ = app.update(Message::FocusUp);
        let _ = app.update(Message::FocusUp);
        let _ = app.update(Message::FocusUp);
        assert_eq!(
            app.tuner.listener().picking(),
            Some(0),
            "the highlight wrapped"
        );

        // Esc closes the list rather than leaving the screen.
        let _ = app.update(Message::GoBack);
        assert!(!app.tuner.listener().is_picking());
        assert_eq!(app.screen, Screen::Tuner, "Esc left the screen as well");

        // And then Esc does leave it.
        let _ = app.update(Message::GoBack);
        assert_eq!(app.screen, Screen::Home);
    }

    #[test]
    fn the_device_control_is_in_the_focus_ring_and_opens_the_list() {
        let (mut app, _) = App::new();
        let _ = app.update(Message::Navigate(Screen::Tuner));

        assert!(
            app.focusables().contains(&FocusTarget::InputDevice),
            "the device control is unreachable by keyboard"
        );

        app.focused = FocusTarget::InputDevice;
        let _ = app.update(Message::ActivateFocused);
        assert!(app.tuner.listener().is_picking());
    }

    #[test]
    fn i_renames_the_pitch_without_touching_any_other_screen() {
        let mut app = tuned_to(233.08); // A#3 / Bb3 — one of the five with two names.
        let before = app.notation;

        press_into(&mut app, "i", keyboard::Modifiers::empty());

        let (pitch, _) = app.tuner.listener().reading().expect("a reading");
        assert_eq!(
            app.tuner.spelling().spell(pitch.pitch_class()).letter,
            Letter::B,
            "`i` did not switch the name to its flat spelling"
        );
        assert_eq!(
            app.notation, before,
            "`i` on the Tuner changed the fretboard notation as well"
        );
    }

    #[test]
    fn the_help_overlay_lists_the_keys_the_screen_claims() {
        let keys: Vec<char> =
            accelerators_for(&Screen::Tuner, Notation::Notes, &WalkTheScale::new())
                .iter()
                .map(|(key, _, _)| *key)
                .collect();

        assert_eq!(keys, vec!['d', 'i']);
    }

    #[test]
    fn a_key_bound_elsewhere_is_inert_here() {
        let (mut app, _) = App::new();
        let _ = app.update(Message::Navigate(Screen::Tuner));

        // `a` widens the Note Trainer's pool, `r` rerolls on three other screens, and the
        // library's `d` picks degrees — none of which this screen claims. `d` *is* claimed
        // here, and means the device list, which is the point of a per-screen table.
        let spelling = app.tuner.spelling();
        for key in ["a", "r"] {
            let _ = app.update(Message::Key(
                keyboard::Key::Character(key.into()),
                keyboard::Modifiers::default(),
            ));

            assert_eq!(app.screen, Screen::Tuner, "`{key}` navigated");
            assert!(
                !app.tuner.listener().is_picking(),
                "`{key}` opened the device list"
            );
            assert_eq!(app.tuner.spelling(), spelling, "`{key}` renamed the pitch");
        }
    }
}

#[cfg(test)]
mod direction_tests {
    use crate::audio::detect::{Detection, Heard};
    use crate::ui::tuner::Correction;
    use crate::ui::*;

    fn hearing(hz: f32) -> App {
        let (mut app, _) = App::new();
        let _ = app.update(Message::Navigate(Screen::Tuner));
        let _ = app.update(Message::Heard(Heard::Pitch(Detection::synthesized(
            hz, 0.999,
        ))));
        app
    }

    /// A frequency a given number of cents from A4.
    fn off_by(cents: f32) -> f32 {
        440.0 * 2.0f32.powf(cents / 1200.0)
    }

    #[test]
    fn a_sharp_string_is_told_to_come_down_and_a_flat_one_to_go_up() {
        assert_eq!(
            hearing(off_by(20.0)).tuner.correction(),
            Some(Correction::Down)
        );
        assert_eq!(
            hearing(off_by(-20.0)).tuner.correction(),
            Some(Correction::Up)
        );
    }

    #[test]
    fn an_in_tune_string_is_told_nothing() {
        for cents in [-2.0, -1.0, 0.0, 1.0, 2.0] {
            assert_eq!(
                hearing(off_by(cents)).tuner.correction(),
                None,
                "{cents} cents asked for a correction"
            );
        }
    }

    #[test]
    fn the_correction_and_the_in_tune_colouring_agree_on_where_the_boundary_is() {
        // Just inside and just outside, on both sides. The two must flip together — a dial
        // that reads "in tune" beside a line saying "keep going" is the one failure this
        // pairing can have.
        let inside = 2.9;
        let outside = 3.1;

        assert_eq!(hearing(off_by(inside)).tuner.correction(), None);
        assert_eq!(hearing(off_by(-inside)).tuner.correction(), None);
        assert!(hearing(off_by(outside)).tuner.correction().is_some());
        assert!(hearing(off_by(-outside)).tuner.correction().is_some());
    }

    #[test]
    fn nothing_sounding_asks_for_no_correction() {
        let (mut app, _) = App::new();
        let _ = app.update(Message::Navigate(Screen::Tuner));

        let _ = app.update(Message::Heard(Heard::Silence));
        assert_eq!(app.tuner.correction(), None);

        let _ = app.update(Message::Heard(Heard::Unpitched));
        assert_eq!(app.tuner.correction(), None);
    }
}
