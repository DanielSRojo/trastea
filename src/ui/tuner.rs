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
use iced::widget::{Space, button, column, container, row, stack, text};
use iced::{
    Color, Element, Length, Padding, Pixels, Point, Radians, Rectangle, Renderer, Size, Theme,
};

use crate::audio::detect::{Heard, Tracker, detect};
use crate::audio::input::{
    DeviceChoice, Input, InputError, devices, remember_device, remembered_device,
};
use crate::music::notes::Spelling;
use crate::music::pitch::{Cents, Pitch};

use super::{
    BODY, FocusTarget, INK, MUSIC_FONT, MUTE, Message, SMUFL_FLAT, SMUFL_SHARP, SUCCESS,
    card_button, focus_ring, ghost_button, note_label, row_button, selected_row_button,
};

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

/// What the screen is doing, which is not the same question as what it is hearing.
///
/// No `Debug`: it holds a `cpal::Stream`, which has none, and deriving one here would mean
/// hand-writing it for the sake of a trait nothing asks for.
enum Listening {
    /// No device has been opened yet — the screen has not been entered.
    Idle,
    /// A device is open.
    Open(Box<Input>),
    /// Opening failed, and this is why.
    Failed(InputError),
}

pub(super) struct Tuner {
    listening: Listening,
    /// Confirms readings across windows. Owned here because it is per-session state: a
    /// run of agreeing windows means nothing after the screen has been left and re-entered.
    tracker: Tracker,
    /// The last thing released. `Heard::Silence` at rest, which is what makes silence a
    /// state the screen draws rather than the absence of one.
    heard: Heard,
    /// The devices offered, read when the picker opens rather than held live — a list
    /// cached at startup is a list that disagrees with the machine by the time it is shown.
    devices: Vec<DeviceChoice>,
    /// `Some` while the picker is open, carrying the highlighted row.
    picking: Option<usize>,
    /// Set when a remembered device was absent and the default was opened instead.
    fell_back: bool,
    /// How the pitch is named. Its own, like the Note Trainer's — a spelling chosen while
    /// tuning has no business renaming anything else.
    spelling: Spelling,
}

impl Tuner {
    pub(super) fn new() -> Tuner {
        Tuner {
            listening: Listening::Idle,
            tracker: Tracker::new(),
            heard: Heard::Silence,
            devices: Vec::new(),
            picking: None,
            fell_back: false,
            spelling: Spelling::Sharps,
        }
    }

    /// Opens the remembered device, or the default. Called when the screen is entered.
    ///
    /// Idempotent: re-entering the screen with a device already open leaves it alone,
    /// rather than closing and reopening one that is working.
    pub(super) fn listen(&mut self) {
        if matches!(self.listening, Listening::Open(_)) {
            return;
        }

        let wanted = remembered_device();
        self.listening = match Input::open_id(wanted.as_deref()) {
            Ok(input) => {
                self.fell_back = input.fell_back();
                Listening::Open(Box::new(input))
            }
            Err(error) => Listening::Failed(error),
        };
        self.tracker = Tracker::new();
        self.heard = Heard::Silence;
    }

    /// Closes the device, if one is open.
    ///
    /// Called on leaving the screen. The alternative — holding it for a quick re-entry —
    /// buys nothing: the poll timer is already gated on the Tuner being the screen on
    /// show, so a held stream is a device claimed for no reading. And an audio interface
    /// is a device other programs want; a trainer that keeps one while its user is
    /// looking at chord diagrams is the reason that program cannot open it.
    ///
    /// Dropping the `Input` is what actually stops the audio — the stream stops when it
    /// is dropped, which is the same fact that decides where it is owned.
    pub(super) fn stop(&mut self) {
        self.listening = Listening::Idle;
        self.tracker = Tracker::new();
        self.heard = Heard::Silence;
        self.picking = None;
        self.fell_back = false;
    }

    /// Whether there is a stream to poll — what the subscription asks before ticking.
    pub(super) fn is_listening(&self) -> bool {
        matches!(self.listening, Listening::Open(_))
    }

    /// Takes whatever audio has arrived and folds it into the reading.
    ///
    /// Analysis runs here, on the UI thread, rather than in the audio callback: the
    /// callback's budget is a deadline, and a detector that occasionally runs long there
    /// is a glitch in the system's audio rather than a late reading. Here the worst case
    /// is a late frame.
    pub(super) fn poll(&mut self) {
        let Listening::Open(input) = &self.listening else {
            return;
        };

        if let Some(lost) = input.lost() {
            self.listening = Listening::Failed(lost);
            self.heard = Heard::Silence;
            return;
        }

        let sample_rate = input.sample_rate();
        let Some(window) = input.take_window() else {
            return;
        };

        let heard = self.tracker.push(detect(&window, sample_rate));
        self.apply(heard);
    }

    /// Records a released reading. The seam the tests drive, so a screen assertion needs
    /// no device — the same arrangement the trainers use for a press on the neck.
    pub(super) fn apply(&mut self, heard: Heard) {
        self.heard = heard;
    }

    pub(super) fn open_picker(&mut self) {
        self.devices = devices();
        // Highlight the device in use, so the list opens where the learner is.
        let current = self.device_name();
        self.picking = Some(
            self.devices
                .iter()
                .position(|device| Some(device.name()) == current)
                .unwrap_or(0),
        );
    }

    pub(super) fn close_picker(&mut self) {
        self.picking = None;
    }

    pub(super) fn is_picking(&self) -> bool {
        self.picking.is_some()
    }

    /// Moves the highlight within the picker, clamping rather than wrapping — the same
    /// behaviour the focus ring has on a row that runs out.
    pub(super) fn move_pick(&mut self, delta: isize) {
        let Some(index) = self.picking else { return };
        if self.devices.is_empty() {
            return;
        }

        let last = self.devices.len() - 1;
        let moved = index.saturating_add_signed(delta).min(last);
        self.picking = Some(moved);
    }

    /// Opens the highlighted device and remembers it.
    pub(super) fn choose(&mut self, index: usize) {
        let Some(device) = self.devices.get(index) else {
            return;
        };

        let id = device.id().to_owned();
        self.listening = match Input::open_id(Some(&id)) {
            Ok(input) => {
                // A device chosen by hand is not a fallback even if opening it fell back:
                // it was asked for, and `fell_back` is what the screen says about a
                // remembered choice that was gone before anyone chose anything.
                self.fell_back = false;
                remember_device(&id);
                Listening::Open(Box::new(input))
            }
            Err(error) => Listening::Failed(error),
        };

        self.tracker = Tracker::new();
        self.heard = Heard::Silence;
        self.picking = None;
    }

    pub(super) fn picking(&self) -> Option<usize> {
        self.picking
    }

    pub(super) fn devices(&self) -> &[DeviceChoice] {
        &self.devices
    }

    /// The name of the device being listened to, or `None` when none is.
    pub(super) fn device_name(&self) -> Option<&str> {
        match &self.listening {
            Listening::Open(input) => Some(input.device_name()),
            _ => None,
        }
    }

    pub(super) fn error(&self) -> Option<&InputError> {
        match &self.listening {
            Listening::Failed(error) => Some(error),
            _ => None,
        }
    }

    pub(super) fn fell_back(&self) -> bool {
        self.fell_back
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

    /// What is sounding, in hertz. Separate from [`Tuner::reading`] because it is the
    /// measurement and the pitch is the interpretation: a string being brought up to tune
    /// moves this continuously while the name it is given stays put.
    pub(super) fn hz(&self) -> Option<f32> {
        match self.heard {
            Heard::Pitch(detection) => Some(detection.hz()),
            _ => None,
        }
    }

    /// The reading, resolved to a pitch — `None` unless something nameable is sounding.
    pub(super) fn reading(&self) -> Option<(Pitch, Cents)> {
        match self.heard {
            Heard::Pitch(detection) => Pitch::nearest(detection.hz()),
            _ => None,
        }
    }

    /// Which way the string has to move, or `None` when it is in tune or nothing is
    /// sounding. The view draws from this rather than comparing cents itself, so the mark
    /// and the "in tune" colouring cannot disagree about where the boundary is.
    pub(super) fn correction(&self) -> Option<Correction> {
        let (_, cents) = self.reading()?;

        if cents.value() > IN_TUNE_CENTS {
            Some(Correction::Down)
        } else if cents.value() < -IN_TUNE_CENTS {
            Some(Correction::Up)
        } else {
            None
        }
    }

    /// What the screen says it is doing, as one of four states rather than a dial that
    /// may or may not mean anything. A learner has to be able to tell a broken setup from
    /// an idle one without leaving the screen.
    pub(super) fn state(&self) -> State {
        match (&self.listening, self.heard) {
            (Listening::Idle, _) => State::NotStarted,
            (Listening::Failed(_), _) => State::NoInput,
            (Listening::Open(_), Heard::Silence) => State::Silent,
            (Listening::Open(_), Heard::Unpitched) => State::Unnameable,
            (Listening::Open(_), Heard::Pitch(_)) => State::Reading,
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

/// The four things the screen can be saying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    NotStarted,
    NoInput,
    Silent,
    Unnameable,
    Reading,
}

pub(super) fn ui_tuner(tuner: &Tuner, focused: FocusTarget) -> Element<'static, Message> {
    let body = if tuner.is_picking() {
        ui_device_picker(tuner, focused)
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
    let state = tuner.state();

    let in_tune = tuner
        .reading()
        .is_some_and(|(_, cents)| cents.value().abs() <= IN_TUNE_CENTS);
    let colour = if in_tune { SUCCESS } else { INK };

    // `note_label` rather than `format!`: it draws the accidental as the music face's own
    // glyph, which is what every other note in the app is drawn with.
    let name: Element<'static, Message> = match tuner.reading() {
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
    let (hz_label, cents_label) = match (tuner.hz(), tuner.reading()) {
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
        ui_device_line(tuner, focused),
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
            .reading()
            .map(|(_, cents)| position_for(cents.value())),
        in_tune: tuner
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

/// The line naming the device, and the way to change it.
fn ui_device_line(tuner: &Tuner, focused: FocusTarget) -> Element<'static, Message> {
    let label = match (tuner.device_name(), tuner.error()) {
        (Some(name), _) => name.to_owned(),
        (None, Some(error)) => error.to_string(),
        (None, None) => "not listening yet".to_owned(),
    };

    let mut line = column![].align_x(iced::Alignment::Center).spacing(4);

    line = line.push(
        row![
            text(label).size(13).color(BODY),
            Space::new().width(Length::Fixed(12.0)),
            focus_ring(
                button(text("change").size(12))
                    .padding([4, 10])
                    .style(ghost_button)
                    .on_press(Message::TunerOpenDevices),
                focused == FocusTarget::TunerDevice,
            ),
        ]
        .align_y(iced::Alignment::Center),
    );

    if tuner.fell_back() {
        line = line.push(
            text("the remembered device was not found — using the system default")
                .size(11)
                .color(MUTE),
        );
    }

    line.into()
}

/// The device list. Modal while it is up: the reading is not shown behind it, because a
/// dial nobody is looking at is a dial drawn for nothing.
fn ui_device_picker(tuner: &Tuner, _focused: FocusTarget) -> Element<'static, Message> {
    let highlighted = tuner.picking().unwrap_or(0);

    let rows = tuner.devices().iter().enumerate().fold(
        column![].spacing(2).width(Length::Fixed(520.0)),
        |rows, (index, device)| {
            let chosen = index == highlighted;

            rows.push(
                button(text(device.name().to_owned()).size(14))
                    .width(Length::Fill)
                    .padding([8, 12])
                    .style(if chosen {
                        selected_row_button
                    } else {
                        row_button
                    })
                    .on_press(Message::TunerChooseDevice(index)),
            )
        },
    );

    let body = if tuner.devices().is_empty() {
        column![
            text("no input devices").size(16).color(BODY),
            text("nothing on this machine could be opened for recording")
                .size(12)
                .color(MUTE),
        ]
        .align_x(iced::Alignment::Center)
        .spacing(6)
    } else {
        column![rows]
    };

    container(
        column![
            text("Input device").size(20),
            Space::new().height(Length::Fixed(12.0)),
            body,
            Space::new().height(Length::Fixed(12.0)),
            text("↑ ↓ to move · Enter to choose · Esc to cancel")
                .size(11)
                .color(MUTE),
        ]
        .align_x(iced::Alignment::Center)
        .spacing(4),
    )
    .padding(Padding::from([20, 24]))
    .style(card_button_container)
    .into()
}

fn card_button_container(theme: &iced::Theme) -> iced::widget::container::Style {
    let style = card_button(theme, iced::widget::button::Status::Active);

    iced::widget::container::Style {
        background: style.background,
        border: style.border,
        ..Default::default()
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
