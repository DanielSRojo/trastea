//! The Tuner: what the app hears, named and measured.
//!
//! Chromatic: it names the nearest pitch in any octave and never snaps to an open string.

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

/// How far from a pitch still counts as in tune; a guitar's own intonation drifts further.
const IN_TUNE_CENTS: f32 = 3.0;

/// The widest deviation the dial shows; `Pitch::nearest` never exceeds half a semitone.
const DIAL_RANGE_CENTS: f32 = 50.0;

/// The dial's box, fixed so the arc keeps its proportions as the window resizes.
const DIAL_SIZE: Size = Size::new(460.0, 235.0);

/// The dial's track, between the app's hairline and its muted text.
const TRACK: Color = Color::from_rgb8(0x55, 0x55, 0x55);

pub(super) struct Tuner {
    /// Only one listening screen has a stream open; `leaving_for` closes it on the way out.
    listener: Listener,
    /// Separate from every other screen's spelling.
    spelling: Spelling,
}

impl Tuner {
    pub(super) fn new() -> Tuner {
        Tuner {
            listener: Listener::new(),
            spelling: Spelling::Sharps,
        }
    }

    pub(super) fn listener(&self) -> &Listener {
        &self.listener
    }

    pub(super) fn listener_mut(&mut self) -> &mut Listener {
        &mut self.listener
    }

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
    /// sounding.
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
        // smaller, or at the name's size the dash sets as a rule across the dial
        None => text("—").size(NAME_SIZE / 2.4).color(MUTE).into(),
    };

    let (hz_label, cents_label) = match (tuner.listener.hz(), tuner.listener.reading()) {
        (Some(hz), Some((_, cents))) => (
            format!("HZ {hz:.1}"),
            // `+ 0.0` turns a rounded -0 into 0, so a hair flat does not print "-0 cents"
            format!("{:+.0} cents", cents.value().round() + 0.0),
        ),
        _ => (String::new(), String::new()),
    };

    let status = text(match state {
        State::NotStarted => "not listening",
        State::NoInput => "no input",
        State::Silent => "listening, play a note...",
        State::Unnameable => "one string at a time",
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
/// A `stack` rather than canvas text so the name can use `note_label`'s two-font glyphs.
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
fn position_for(cents: f32) -> f32 {
    (cents / DIAL_RANGE_CENTS).clamp(-1.0, 1.0)
}

/// Where every part of the dial sits, derived once from its box.
///
/// Shared by the canvas and the name overlay so the two cannot drift apart.
struct Dial {
    size: Size,
    center: Point,
    radius: f32,
}

impl Dial {
    /// How far either side of the top the track runs.
    const HALF_SWEEP: f32 = 80.0 * PI / 180.0;
    /// Room above the top of the track for the target mark.
    const PAD_TOP: f32 = 40.0;
    /// Room outside the ends of the track for the ♭ and the ♯.
    const EDGE: f32 = 56.0;
    const TRACK_WIDTH: f32 = 3.0;
    const DOT_RADIUS: f32 = 9.0;
    /// The target mark's triangle, and how far its point sits above the track.
    const MARK_HALF_WIDTH: f32 = 8.0;
    const MARK_HEIGHT: f32 = 10.0;
    const MARK_LIFT: f32 = 24.0;
    /// The accidentals' offset beyond the track, and their size.
    ///
    /// The size is a SMuFL em, so a flat at 44 sets about as tall as a letter at 26.
    const LABEL_GAP: f32 = 32.0;
    const LABEL_SIZE: f32 = 44.0;

    fn new(size: Size) -> Dial {
        // whichever of width or height bounds the radius first
        let by_width = (size.width / 2.0 - Dial::EDGE) / Dial::HALF_SWEEP.sin();
        let by_height = (size.height - Dial::PAD_TOP - Dial::EDGE) / (1.0 - Dial::HALF_SWEEP.cos());
        let radius = by_width.min(by_height).max(1.0);

        Dial {
            size,
            center: Point::new(size.width / 2.0, Dial::PAD_TOP + radius),
            radius,
        }
    }

    /// The angle a position on the track sits at, in canvas terms: clockwise from +x, so up
    /// is −π/2.
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

    /// Where the note name's centre belongs: a shade below the middle of the bowl, where it
    /// looks centred.
    fn name_center(&self) -> Point {
        let top = self.point_at(0.0).y;
        let ends = self.point_at(1.0).y;

        Point::new(self.center.x, top + (ends - top) * 0.58)
    }

    /// The padding that drops a vertically centred overlay onto [`Dial::name_center`].
    ///
    /// Centring in height `h` with padding `p` on one side puts the middle at `(h ∓ p) / 2`.
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
    /// `None` draws no dot; a dot at the top would read as in tune.
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

        // the target: a triangle pointing down at the top of the track
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
                // private-use codepoints need advanced shaping
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

/// The dial's geometry, since the canvas frame cannot be read back.
#[cfg(test)]
mod dial_tests {
    use super::*;

    /// A sweep across the whole track, finer than a pixel of movement.
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
        // canvas y grows downward
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

        // the padding places it, and the canvas never sees the padding
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

    fn heard(hz: f32) -> Heard {
        Heard::Pitch(Detection::synthesized(hz, 0.999))
    }

    fn tuned_to(hz: f32) -> App {
        let (mut app, _) = App::new();
        let _ = app.update(Message::Navigate(Screen::Tuner));
        let _ = app.update(Message::Heard(heard(hz)));
        app
    }

    #[test]
    fn the_tuner_is_reachable_and_leavable_like_every_other_screen() {
        let (mut app, _) = App::new();

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
        // the open low E
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
        // a low E a semitone flat reads D#2, not a very flat E
        let app = tuned_to(77.78);
        let (pitch, _) = app.tuner.listener().reading().expect("a reading");

        assert_eq!(pitch.pitch_class(), PitchClass::new(3));
        assert_eq!(pitch.octave(), 2);
    }

    #[test]
    fn an_alternate_tuning_needs_no_setting() {
        // drop D
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

        assert_eq!(app.tuner.listener().state(), listener::State::NotStarted);

        let _ = app.update(Message::Navigate(Screen::Tuner));
        // whether a device opened depends on the machine
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

        // what the detector releases for a chord
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

        // clamps rather than wraps
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

        let _ = app.update(Message::GoBack);
        assert!(!app.tuner.listener().is_picking());
        assert_eq!(app.screen, Screen::Tuner, "Esc left the screen as well");

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
        let mut app = tuned_to(233.08); // A#3 / Bb3
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

        // `a` and `r` are bound on other screens
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
        // just inside and just outside `IN_TUNE_CENTS`, on both sides
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
