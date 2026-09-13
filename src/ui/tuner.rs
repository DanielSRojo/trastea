//! The Tuner: what the app hears, named and measured.
//!
//! The screen is chromatic rather than six-string — it names the nearest of all twelve
//! pitch classes in whatever octave it hears, and never snaps to an open string. A capo
//! and an alternate tuning then need nothing said about them. The cost is stated rather
//! than hidden: a string tuned a full semitone flat reads as the note it has become.
//!
//! The needle is the readout and the name is a label on it. That is not a presentation
//! preference — a pitch moving smoothly across a semitone boundary changes its name
//! abruptly while the sound does not, and a bend measured here crossed three names while
//! travelling continuously.

use iced::widget::{Space, button, column, container, row, text};
use iced::{Element, Length, Padding};

use crate::audio::detect::{Heard, Tracker, detect};
use crate::audio::input::{
    DeviceChoice, Input, InputError, devices, remember_device, remembered_device,
};
use crate::music::notes::Spelling;
use crate::music::pitch::{Cents, Pitch};

use super::{
    BODY, FocusTarget, INK, KEYCAP_FONT, LINK, MARK_TUNE_DOWN, MARK_TUNE_UP, MUTE, Message,
    card_button, focus_ring, ghost_button, note_label, row_button, selected_row_button,
};

/// How far from a pitch still counts as in tune.
///
/// Three cents rather than one: a guitar's own intonation moves a fretted note further
/// than that, and a needle that refuses to settle is a needle nobody trusts.
const IN_TUNE_CENTS: f32 = 3.0;

/// The widest deviation the needle shows. Past this the reading is the next note along,
/// since `Pitch::nearest` never returns an offset beyond half a semitone.
const NEEDLE_RANGE_CENTS: f32 = 50.0;

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

    /// What the screen says it is doing, as one of four states rather than a needle that
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
        .padding(Padding::from([24, 24]))
        .into()
}

/// The reading itself: name, cents, needle, and what the screen is doing.
fn ui_reading(tuner: &Tuner, focused: FocusTarget) -> Element<'static, Message> {
    let state = tuner.state();

    let in_tune = tuner
        .reading()
        .is_some_and(|(_, cents)| cents.value().abs() <= IN_TUNE_CENTS);
    let colour = if in_tune { LINK } else { INK };

    // `note_label` rather than `format!`: it draws the accidental as the music face's own
    // glyph, which is what every other note in the app is drawn with.
    //
    // The marks flank it rather than sitting under it, because which way to turn the peg is
    // the thing a learner wants without reading a number, and a mark beside the note is
    // read at the same glance as the note. Both sides are always present — one of them
    // transparent — so the name does not shift sideways as the reading crosses the centre.
    let heading: Element<'static, Message> = match tuner.reading() {
        Some((pitch, _)) => row![
            direction_mark(MARK_TUNE_DOWN, tuner.correction() == Some(Correction::Down)),
            note_label(tuner.spelling.spell(pitch.pitch_class()), 96, colour),
            text(pitch.octave().to_string()).size(52).color(colour),
            direction_mark(MARK_TUNE_UP, tuner.correction() == Some(Correction::Up)),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(10)
        .into(),
        None => text("—").size(96).color(MUTE).into(),
    };

    let cents_label = match tuner.reading() {
        Some((_, cents)) => format!("{:+.1} cents", cents.value()),
        None => String::new(),
    };

    let cents_line = text(cents_label)
        .size(20)
        .color(if in_tune { LINK } else { BODY });

    let status = text(match state {
        State::NotStarted => "not listening",
        State::NoInput => "no input",
        State::Silent => "listening — play a note",
        State::Unnameable => "hearing something it will not name — one string at a time",
        State::Reading if in_tune => "in tune",
        State::Reading => "",
    })
    .size(14)
    .color(if in_tune { LINK } else { MUTE });

    column![
        ui_device_line(tuner, focused),
        Space::new().height(Length::Fixed(28.0)),
        heading,
        cents_line,
        Space::new().height(Length::Fixed(20.0)),
        ui_needle(tuner),
        Space::new().height(Length::Fixed(14.0)),
        status,
    ]
    .align_x(iced::Alignment::Center)
    .spacing(4)
    .into()
}

/// One of the two triangles beside the note, shown or held invisible.
///
/// Drawn rather than omitted when it does not apply: a row that gains and loses a widget
/// re-centres itself, and a note name that jumps sideways every time the reading crosses
/// the centre is harder to read than one that does not move. Transparent keeps the space.
///
/// The glyph comes from the embedded subset for the reason the chrome's arrows do — a
/// solid triangle is in most faces, but each machine's fallback draws it at its own weight,
/// and two marks from one place are one pair of shapes where two fallbacks are not.
fn direction_mark(glyph: &'static str, lit: bool) -> Element<'static, Message> {
    text(glyph)
        .font(KEYCAP_FONT)
        .size(34)
        .color(if lit { LINK } else { iced::Color::TRANSPARENT })
        .into()
}

/// The needle: where the sounding pitch sits between half a semitone flat and half sharp.
///
/// Built from two proportional spacers either side of a mark rather than drawn on a canvas.
/// A canvas here would inherit the repo's rule that one `Layout` drives drawing and
/// hit-testing — and nothing hit-tests a needle, so the round-trip test that rule exists
/// for would have nothing to assert.
fn ui_needle(tuner: &Tuner) -> Element<'static, Message> {
    const TRACK: f32 = 520.0;
    const MARK: f32 = 4.0;

    let Some((_, cents)) = tuner.reading() else {
        // No reading: the track with no mark on it. Deliberately not a mark at centre,
        // which reads as a note that is perfectly in tune.
        return column![
            ui_track_labels(),
            container(Space::new().height(Length::Fixed(3.0)))
                .width(Length::Fixed(TRACK))
                .style(|_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(MUTE)),
                    ..Default::default()
                }),
        ]
        .align_x(iced::Alignment::Center)
        .spacing(6)
        .into();
    };

    let offset = cents.value().clamp(-NEEDLE_RANGE_CENTS, NEEDLE_RANGE_CENTS);
    // 0.0 at fully flat, 1.0 at fully sharp.
    let position = (offset + NEEDLE_RANGE_CENTS) / (NEEDLE_RANGE_CENTS * 2.0);
    let before = (TRACK - MARK) * position;

    let in_tune = offset.abs() <= IN_TUNE_CENTS;
    let colour = if in_tune { LINK } else { INK };

    column![
        ui_track_labels(),
        row![
            Space::new().width(Length::Fixed(before)),
            container(Space::new().height(Length::Fixed(22.0)))
                .width(Length::Fixed(MARK))
                .style(move |_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(colour)),
                    ..Default::default()
                }),
        ]
        .width(Length::Fixed(TRACK)),
    ]
    .align_x(iced::Alignment::Center)
    .spacing(6)
    .into()
}

fn ui_track_labels() -> Element<'static, Message> {
    row![
        text("♭ 50").size(12).color(MUTE),
        Space::new().width(Length::Fill),
        text("0").size(12).color(MUTE),
        Space::new().width(Length::Fill),
        text("50 ♯").size(12).color(MUTE),
    ]
    .width(Length::Fixed(520.0))
    .into()
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
/// needle nobody is looking at is a needle drawn for nothing.
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
