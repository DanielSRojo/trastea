//! The capture device, what it has heard, and the list to change it with.
//!
//! This is not a screen. It moved out of `tuner.rs` when a second screen wanted to listen:
//! a screen reaching into another screen is the arrangement that keeping each screen's
//! state and views in one module exists to prevent, so the machinery left rather than the
//! new screen reaching across — the move `shapes.rs` made for the same reason.
//!
//! It sits in `ui/` rather than in `audio/` because it holds a highlighted row and draws a
//! list. `audio/` reports hertz and knows nothing about screens, which is the line that
//! makes its detector testable; a picker index is on this side of it.
//!
//! What it deliberately does not hold is any opinion about what a reading *means*. In tune
//! is the Tuner's question and correct is the drill's, and both are asked of the same
//! `Heard`.

use iced::widget::{Space, button, column, container, row, text};
use iced::{Element, Length, Padding};

use crate::audio::detect::{Heard, Tracker, detect};
use crate::audio::input::{
    DeviceChoice, Input, InputError, devices, remember_device, remembered_device,
};
use crate::music::pitch::{Cents, Pitch};

use super::{
    BODY, FocusTarget, MUTE, Message, card_button, focus_ring, ghost_button, row_button,
    selected_row_button,
};

/// What the screen is doing, which is not the same question as what it is hearing.
///
/// No `Debug`: it holds a `cpal::Stream`, which has none, and deriving one here would mean
/// hand-writing it for the sake of a trait nothing asks for.
enum Listening {
    /// No device has been opened yet — no listening screen has been entered.
    Idle,
    /// A device is open.
    Open(Box<Input>),
    /// Opening failed, and this is why.
    Failed(InputError),
}

pub(super) struct Listener {
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
}

impl Listener {
    pub(super) fn new() -> Listener {
        Listener {
            listening: Listening::Idle,
            tracker: Tracker::new(),
            heard: Heard::Silence,
            devices: Vec::new(),
            picking: None,
            fell_back: false,
        }
    }

    /// Opens the remembered device, or the default. Called when a listening screen is
    /// entered.
    ///
    /// Idempotent: re-entering with a device already open leaves it alone, rather than
    /// closing and reopening one that is working.
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
    /// buys nothing: the poll timer is already gated on a listening screen being on show,
    /// so a held stream is a device claimed for no reading. And an audio interface is a
    /// device other programs want; a trainer that keeps one while its user is looking at
    /// chord diagrams is the reason that program cannot open it.
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

    /// Takes whatever audio has arrived and folds it into the reading, returning what was
    /// released so a caller with its own use for it does not have to ask again.
    ///
    /// Analysis runs here, on the UI thread, rather than in the audio callback: the
    /// callback's budget is a deadline, and a detector that occasionally runs long there
    /// is a glitch in the system's audio rather than a late reading. Here the worst case
    /// is a late frame.
    pub(super) fn poll(&mut self) -> Option<Heard> {
        let Listening::Open(input) = &self.listening else {
            return None;
        };

        if let Some(lost) = input.lost() {
            self.listening = Listening::Failed(lost);
            self.heard = Heard::Silence;
            return None;
        }

        let sample_rate = input.sample_rate();
        let window = input.take_window()?;

        let heard = self.tracker.push(detect(&window, sample_rate));
        self.apply(heard);
        Some(heard)
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
    ///
    /// The choice is remembered for the app rather than for the screen that made it: a
    /// learner who has said which interface they play through has said it about the
    /// instrument, and two records of it would be two records free to disagree.
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

    /// What is sounding, in hertz. Separate from [`Listener::reading`] because it is the
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

/// The four things a listening screen can be saying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    NotStarted,
    NoInput,
    Silent,
    Unnameable,
    Reading,
}

/// The line naming the device, and the way to change it.
pub(super) fn ui_device_line(
    listener: &Listener,
    focused: FocusTarget,
) -> Element<'static, Message> {
    let label = match (listener.device_name(), listener.error()) {
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
                    .on_press(Message::OpenDevices),
                focused == FocusTarget::InputDevice,
            ),
        ]
        .align_y(iced::Alignment::Center),
    );

    if listener.fell_back() {
        line = line.push(
            text("the remembered device was not found — using the system default")
                .size(11)
                .color(MUTE),
        );
    }

    line.into()
}

/// The device list. Modal while it is up: what the screen was showing is not drawn behind
/// it, because a dial nobody is looking at is a dial drawn for nothing.
pub(super) fn ui_device_picker(listener: &Listener) -> Element<'static, Message> {
    let highlighted = listener.picking().unwrap_or(0);

    let rows = listener.devices().iter().enumerate().fold(
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
                    .on_press(Message::ChooseDevice(index)),
            )
        },
    );

    let body = if listener.devices().is_empty() {
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
