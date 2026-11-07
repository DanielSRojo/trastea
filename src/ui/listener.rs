//! The capture device, what it has heard, and the picker to change it.
//! Holds no opinion on what a reading means; that is each screen's question.

use iced::widget::{Space, button, column, container, row, text};
use iced::{Element, Length, Padding};

use crate::audio::detect::{Heard, Tracker, detect};
use crate::audio::inharmonicity::{Fit, fit};
use crate::audio::input::{
    DeviceChoice, Input, InputError, devices, remember_device, remembered_device,
};
use crate::music::pitch::{Cents, Pitch};

use super::Message;

use super::focus::FocusTarget;

use super::style::{
    BODY, MUTE, card_button, focus_ring, ghost_button, row_button, selected_row_button,
};

/// How many hops apart a sustained note is fitted again: 85 ms at 48 kHz, half a fit window,
/// so consecutive fits overlap rather than reading the same audio twice.
const FIT_EVERY: usize = 8;

/// What the screen is doing, as opposed to what it is hearing.
/// No `Debug`: `cpal::Stream` has none.
enum Listening {
    /// No listening screen has been entered yet.
    Idle,
    Open(Box<Input>),
    Failed(InputError),
}

pub(super) struct Listener {
    listening: Listening,
    /// Per-session: a run of agreeing windows means nothing after the screen is re-entered.
    tracker: Tracker,
    /// `Heard::Silence` at rest.
    heard: Heard,
    /// Read when the picker opens rather than cached, so it matches the machine.
    devices: Vec<DeviceChoice>,
    /// `Some` while the picker is open, carrying the highlighted row.
    picking: Option<usize>,
    /// Set when the device asked for was absent and the default was opened instead.
    fell_back: bool,
    /// Whether confirmed notes are also fitted for inharmonicity; only the Position Finder asks.
    fits: bool,
    /// Hops since the sounding note was confirmed, which is what [`fit_due`] schedules by.
    confirmed_hops: usize,
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
            fits: false,
            confirmed_hops: 0,
        }
    }

    /// The same listener, also fitting the notes it confirms.
    pub(super) fn fitting(self) -> Listener {
        Listener { fits: true, ..self }
    }

    #[cfg(test)]
    pub(super) fn is_fitting(&self) -> bool {
        self.fits
    }

    /// Opens the remembered device, or the default. Idempotent: a working device is left
    /// open.
    pub(super) fn listen(&mut self) {
        if matches!(self.listening, Listening::Open(_)) {
            return;
        }

        let wanted = remembered_device();
        self.settle(Input::open_id(wanted.as_deref()));
    }

    /// The one place `fell_back` is set.
    fn settle(&mut self, opened: Result<Input, InputError>) {
        self.listening = match opened {
            Ok(input) => {
                self.fell_back = input.fell_back();
                Listening::Open(Box::new(input))
            }
            Err(error) => Listening::Failed(error),
        };
        self.tracker = Tracker::new();
        self.heard = Heard::Silence;
        self.confirmed_hops = 0;
    }

    /// Closes the device, if one is open. Dropping the `Input` is what stops the stream.
    pub(super) fn stop(&mut self) {
        self.listening = Listening::Idle;
        self.tracker = Tracker::new();
        self.heard = Heard::Silence;
        self.confirmed_hops = 0;
        self.picking = None;
        self.fell_back = false;
    }

    /// Whether there is a stream to poll.
    pub(super) fn is_listening(&self) -> bool {
        matches!(self.listening, Listening::Open(_))
    }

    /// Folds whatever audio has arrived into the reading and returns what was released.
    /// Analysis runs on the UI thread so a slow detector cannot glitch the audio callback.
    pub(super) fn poll(&mut self) -> Option<Heard> {
        self.poll_fitted().map(|(heard, _)| heard)
    }

    /// [`Listener::poll`], with the note's fit when one was due, so a reading and its fit
    /// arrive together. The fit is `None` on every hop it was not due, and always unless
    /// this listener is [`Listener::fitting`].
    pub(super) fn poll_fitted(&mut self) -> Option<(Heard, Option<Fit>)> {
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

        let fitted = match heard {
            Heard::Pitch(detection) if self.fits && fit_due(heard, self.confirmed_hops) => input
                .take_fit_window()
                .map(|longer| fit(&longer, sample_rate, detection.hz())),
            _ => None,
        };

        self.confirmed_hops = match heard {
            Heard::Pitch(_) => self.confirmed_hops + 1,
            _ => 0,
        };

        self.apply(heard);
        Some((heard, fitted))
    }

    /// Records a released reading. The seam the tests drive, so they need no device.
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

    /// Moves the highlight within the picker, clamping rather than wrapping.
    pub(super) fn move_pick(&mut self, delta: isize) {
        let Some(index) = self.picking else { return };
        if self.devices.is_empty() {
            return;
        }

        let last = self.devices.len() - 1;
        let moved = index.saturating_add_signed(delta).min(last);
        self.picking = Some(moved);
    }

    /// Opens the highlighted device and remembers it for the app, not the screen.
    pub(super) fn choose(&mut self, index: usize) {
        let Some(device) = self.devices.get(index) else {
            return;
        };

        let id = device.id().to_owned();
        let opened = Input::open_id(Some(&id));

        // A device unplugged since the list was read opens the default; don't remember that.
        if opened.as_ref().is_ok_and(|input| !input.fell_back()) {
            remember_device(&id);
        }

        self.settle(opened);
        self.picking = None;
    }

    pub(super) fn picking(&self) -> Option<usize> {
        self.picking
    }

    pub(super) fn devices(&self) -> &[DeviceChoice] {
        &self.devices
    }

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

    /// The raw frequency; moves continuously while [`Listener::reading`]'s name stays put.
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

/// Whether a note `confirmed_hops` into its confirmation is due a fit: on confirmation, then
/// every [`FIT_EVERY`] hops while it sustains, and never while nothing nameable sounds.
fn fit_due(heard: Heard, confirmed_hops: usize) -> bool {
    matches!(heard, Heard::Pitch(_)) && confirmed_hops.is_multiple_of(FIT_EVERY)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    NotStarted,
    NoInput,
    Silent,
    Unnameable,
    Reading,
}

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
            text("the chosen device was not found — using the system default")
                .size(11)
                .color(MUTE),
        );
    }

    line.into()
}

/// The device list. Modal: the screen behind it is not drawn.
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

#[cfg(test)]
mod schedule_tests {
    use super::*;
    use crate::audio::detect::Detection;

    fn pitch() -> Heard {
        Heard::Pitch(Detection::synthesized(196.0, 0.999))
    }

    /// Which hops of a stream of releases a listener would fit, counting as `poll_fitted` does.
    fn fitted_hops(stream: &[Heard]) -> Vec<usize> {
        stream
            .iter()
            .scan(0usize, |confirmed, &heard| {
                let due = fit_due(heard, *confirmed);
                *confirmed = if matches!(heard, Heard::Pitch(_)) {
                    *confirmed + 1
                } else {
                    0
                };
                Some(due)
            })
            .enumerate()
            .filter_map(|(hop, due)| due.then_some(hop))
            .collect()
    }

    #[test]
    fn a_note_is_fitted_on_confirmation_and_then_at_an_interval() {
        let mut stream = vec![Heard::Silence, Heard::Unpitched, Heard::Unpitched];
        stream.extend(std::iter::repeat_n(pitch(), FIT_EVERY * 2 + 1));

        assert_eq!(
            fitted_hops(&stream),
            vec![3, 3 + FIT_EVERY, 3 + 2 * FIT_EVERY]
        );
    }

    #[test]
    fn a_note_is_never_fitted_on_every_hop() {
        let stream = vec![pitch(); 64];
        assert_eq!(fitted_hops(&stream).len(), 64 / FIT_EVERY);
    }

    #[test]
    fn nothing_is_fitted_while_silent_or_unnameable() {
        let mut stream = vec![Heard::Silence; 40];
        stream.extend(vec![Heard::Unpitched; 40]);
        assert!(fitted_hops(&stream).is_empty());
    }

    #[test]
    fn a_new_note_is_fitted_on_its_own_confirmation() {
        // the tracker releases nothing between two notes while it re-confirms
        let mut stream = vec![pitch(); 3];
        stream.extend([Heard::Unpitched, Heard::Unpitched]);
        stream.extend(vec![pitch(); 2]);

        assert_eq!(fitted_hops(&stream), vec![0, 5]);
    }

    #[test]
    fn fitting_is_off_unless_asked_for() {
        assert!(!Listener::new().is_fitting());
        assert!(Listener::new().fitting().is_fitting());
    }
}

#[cfg(test)]
mod device_tests {
    use crate::ui::tests::every_screen;
    use crate::ui::*;

    #[test]
    fn leaving_the_screen_puts_the_device_down() {
        let (mut app, _) = App::new();

        let _ = app.update(Message::Navigate(Screen::Tuner));
        let _ = app.update(Message::OpenDevices);
        assert!(app.tuner.listener().is_picking());

        // Esc closes the picker, and the second one leaves.
        let _ = app.update(Message::GoBack);
        let _ = app.update(Message::GoBack);

        assert_eq!(app.screen, Screen::Home);
        assert!(
            !app.tuner.listener().is_listening(),
            "the tuner kept the capture device after leaving the screen"
        );
        assert!(
            !app.tuner.listener().is_picking(),
            "the device list survived the exit"
        );
    }

    /// Moving between the two listening screens closes one stream before the other opens.
    #[test]
    fn only_one_screen_holds_a_stream_at_a_time() {
        let (mut app, _) = App::new();

        let _ = app.update(Message::Navigate(Screen::Tuner));
        let opened = app.tuner.listener().is_listening();

        let _ = app.update(Message::Navigate(Screen::PlayTheInterval));

        assert!(
            !app.tuner.listener().is_listening(),
            "the Tuner kept its device when the drill took over"
        );
        assert_eq!(
            app.play_the_interval.listener().is_listening(),
            opened,
            "the drill did not open the device the Tuner put down"
        );

        // ...and back the other way.
        let _ = app.update(Message::Navigate(Screen::Tuner));

        assert!(
            !app.play_the_interval.listener().is_listening(),
            "the drill kept its device when the Tuner took over"
        );
        assert_eq!(app.tuner.listener().is_listening(), opened);
    }

    #[test]
    fn leaving_the_played_drill_puts_the_device_down() {
        for leave in [Message::GoBack, Message::Navigate(Screen::Caged)] {
            let (mut app, _) = App::new();

            let _ = app.update(Message::Navigate(Screen::PlayTheInterval));
            let _ = app.update(leave);

            assert_ne!(app.screen, Screen::PlayTheInterval);
            assert!(
                !app.play_the_interval.listener().is_listening(),
                "the drill kept the capture device after leaving the screen"
            );
        }
    }

    /// The poll ticks on the listening screens and on none of the others.
    #[test]
    fn the_poll_is_live_on_the_listening_screens_and_no_others() {
        for screen in every_screen() {
            let (mut app, _) = App::new();
            app.open(screen.clone());

            let listens = matches!(
                screen,
                Screen::Tuner
                    | Screen::PlayTheInterval
                    | Screen::WalkTheScale
                    | Screen::PositionFinder
            );
            assert_eq!(
                app.listener_ref().is_some(),
                listens,
                "{screen:?} disagrees about whether it listens"
            );

            // only the Position Finder pays for the slow path
            assert_eq!(
                app.listener_ref().is_some_and(Listener::is_fitting),
                screen == Screen::PositionFinder,
                "{screen:?} disagrees about whether it fits"
            );

            // With no device on the machine the stream never opens and the poll does not tick.
            if !listens {
                assert!(!app.is_listening(), "{screen:?} is polling for audio");
            }
        }
    }

    #[test]
    fn navigating_elsewhere_puts_it_down_too() {
        // The other way out: a Home digit rather than Esc.
        let (mut app, _) = App::new();

        let _ = app.update(Message::Navigate(Screen::Tuner));
        let _ = app.update(Message::Navigate(Screen::Caged));

        assert_eq!(app.screen, Screen::Caged);
        assert!(!app.tuner.listener().is_listening());
    }

    #[test]
    fn re_entering_listens_again() {
        let (mut app, _) = App::new();

        let _ = app.update(Message::Navigate(Screen::Tuner));
        let first = app.tuner.listener().is_listening();

        let _ = app.update(Message::GoBack);
        let _ = app.update(Message::Navigate(Screen::Tuner));

        assert_eq!(
            app.tuner.listener().is_listening(),
            first,
            "re-entering did not restore whatever the machine could do the first time"
        );
    }
}
