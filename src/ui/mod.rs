mod caged;
mod calibration;
mod chord_diagram;
mod chord_library;
mod drill;
mod focus;
mod fretboard;
mod home;
mod interval_trainer;
mod keys;
mod listener;
mod neck;
mod note_trainer;
mod placement;
mod play_the_interval;
mod position;
mod position_finder;
mod scale_trainer;
mod shapes;
mod style;
mod tuner;
mod walk_the_scale;
use std::time::Duration;

use caged::{Caged, ui_caged};
use home::{Category, ui_home};
use interval_trainer::{IntervalTrainer, ui_interval_trainer};
use listener::Listener;
use note_trainer::{NoteTrainer, ui_note_trainer};
use play_the_interval::{PlayTheInterval, ui_play_the_interval};
use position_finder::{PositionFinder, ui_position_finder};
use scale_trainer::ui_scale_trainer;
use tuner::{Tuner, ui_tuner};
use walk_the_scale::{WalkTheScale, ui_walk_the_scale};

use chord_library::{ChordLibrary, KeyOutcome, ui_chord_library};

use iced::{Color, Element, Pixels, Subscription, Task, keyboard};

#[cfg(test)]
use crate::audio::detect::Heard;
#[cfg(test)]
use crate::audio::inharmonicity::Fit;
use crate::music::{
    intervals::Interval,
    notes::{Accidental, Note, PitchClass},
    scales::Scale,
    scales::ScaleKind,
};
use crate::rng::Rng;

use drill::answer_at;
use focus::{Direction, FocusTarget, step_focus, step_focus_2d};
use keys::{Fires, translate_key, ui_help_overlay};
use neck::DRILL_NECK;
use style::{
    BODY, CANVAS, INK, KEYCAP_BACK, KEYCAP_FONT, MUSIC_FONT, SMUFL_DOUBLE_FLAT, SMUFL_DOUBLE_SHARP,
    SMUFL_FLAT, SMUFL_SHARP, card_button, focus_ring, ghost_button, page_container,
    selected_root_button,
};

/// How long a correct answer stays lit before the next prompt replaces it.
const ANSWER_FLASH: Duration = Duration::from_secs(1);
/// Tall enough for a two-line kind name plus the formula. iced gives a `Shrink` child that
/// runs out of room a height of zero rather than overflowing.
const SUMMARY_CARD_HEIGHT: f32 = 280.0;
/// Named so the fit test budgets against the same sizes the card draws with.
const SUMMARY_ROOT_SIZE: f32 = 56.0;
const SUMMARY_NAME_SIZE: f32 = 34.0;
const SUMMARY_FORMULA_SIZE: f32 = 24.0;
const ROOT_SELECTOR_CARD_WIDTH: f32 = 320.0;
/// Tall enough for `KIND_ROW_WIDTHS`'s six rows; the root selector shares it.
const SELECTOR_CARD_HEIGHT: f32 = 400.0;
const ROOT_BUTTON_SIZE: f32 = 50.0;
/// Named so `selector_card_fits_every_kind_row` budgets against it.
const KIND_BUTTON_SIZE: f32 = 16.0;

pub struct App {
    screen: Screen,
    history: Vec<Screen>,
    scale: Scale,
    focused: FocusTarget,
    /// Owned rather than global, which is also why `App` has no `Default`: it would fix the seed.
    rng: Rng,
    /// A flag rather than a `Screen`, so the screen underneath stays the active one.
    help_open: bool,
    /// What the fretboard's markers say. Not on `Scale`, so it survives a reroll.
    notation: Notation,
    note_trainer: NoteTrainer,
    interval_trainer: IntervalTrainer,
    play_the_interval: PlayTheInterval,
    chord_library: ChordLibrary,
    caged: Caged,
    walk_the_scale: WalkTheScale,
    /// Holds the open audio stream, which a subscription would drop whenever its hash changed.
    tuner: Tuner,
    position_finder: PositionFinder,
}

/// What the fretboard's markers are labelled with: the notes' names, or their scale degrees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Notation {
    Notes,
    Intervals,
    /// Which finger stops a string. Only a chord diagram has one, hence `cycle_for`.
    Fingers,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub enum Screen {
    #[default]
    Home,
    ScaleTrainer,
    NoteTrainer,
    IntervalTrainer,
    PlayTheInterval,
    ChordLibrary,
    Caged,
    Tuner,
    /// The drill that walks a scale by degree.
    WalkTheScale,
    /// The answering screen that draws where on the neck a played note was.
    PositionFinder,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Navigate(Screen),
    /// A press on a Home category, which takes the ring and so opens its entries.
    OpenCategory(Category),
    GoBack,
    SelectRoot(PitchClass),
    SelectScaleKind(ScaleKind),
    ToggleNotation,
    RerollScale,
    /// Answers a *Name it* prompt with a note name.
    AnswerNote(PitchClass),
    /// Answers a *Find it* prompt with a position; also what a press on the neck sends.
    /// Loose `usize`s so the constructor fits `Fretboard::on_press`'s `fn(usize, usize)`.
    ChooseNotePosition(usize, usize),
    SkipPrompt,
    /// Retires the flashed prompt and draws the next. Sent only by the timer.
    AdvancePrompt,
    ToggleDrillDirection,
    /// Swaps how the Note Trainer is answered — pressed on screen, or played on the guitar.
    ToggleNoteAnswering,
    TogglePool,
    /// The Note Trainer's spelling, not the scale's.
    ToggleNoteSpelling,
    /// The Tuner's, which names what it hears and moves no other screen.
    ToggleTunerSpelling,
    /// Answers a *Name it* prompt on the Interval Trainer with an interval.
    AnswerInterval(Interval),
    /// Answers the Interval Trainer's *Find it* prompt with a position; also what a press on
    /// that screen's neck sends.
    ChooseIntervalPosition(usize, usize),
    SkipIntervalPrompt,
    ToggleIntervalDirection,
    /// The end of the Interval Trainer's flash. Per screen, so a tick cannot cross screens.
    AdvanceIntervalPrompt,
    /// Selects one chord in the library's list, by its index among the rows on show.
    SelectChordRow(usize),
    /// Selects one of the selected chord's voicings; a press anywhere inside a diagram sends it.
    SelectVoicing(usize),
    /// Picks a notation outright, by its index in the screen's `Notation::cycle_for`.
    SetNotation(usize),
    /// Picks the root the CAGED window shows.
    SelectCagedRoot(PitchClass),
    /// Picks one shape of the CAGED cycle, by its place in the derived order.
    SelectCagedShape(usize),
    /// A different root for the CAGED window, without touching the Scale Trainer's scale.
    RerollCagedRoot,
    /// One tick of the audio poll, routed by `App::listener` to the screen holding the device.
    AudioPoll,
    /// A released reading, applied directly. The test seam in place of `AudioPoll`.
    #[cfg(test)]
    Heard(Heard),
    /// A released reading and the fit that came with it, as the Position Finder's poll
    /// delivers them together.
    #[cfg(test)]
    Fitted(Heard, Option<Fit>),
    /// Starts the Position Finder's calibration walk from its first step.
    Calibrate,
    /// Shows or hides the groups the Position Finder's calibration will not split.
    TogglePositionGroups,
    /// Opens the input device list.
    OpenDevices,
    /// Chooses a device by its place in the offered list.
    ChooseDevice(usize),
    /// Skips the played drill's prompt.
    SkipPlayPrompt,
    /// The played drill's flash timer.
    AdvancePlayPrompt,
    /// Starts another run after one has ended on its duration.
    StartWalkTheScale,
    /// Picks the kind the drill walks.
    SelectDrillKind(ScaleKind),
    /// The root display's drag. x is in window coordinates, since the gesture outlives the
    /// pointer's stay over the canvas and only differences are read.
    GrabDrillRoot(f32),
    DragDrillRoot(f32),
    DropDrillRoot,
    /// The wheel over the root display, already converted from `ScrollDelta` to steps.
    ScrollDrillRoot(f32),
    /// `[` and `]`: the root a semitone down or up, from wherever the ring is standing.
    StepDrillRoot(i32),
    /// A different scale to drill.
    RerollDrillScale,
    /// Swaps how the drill is answered — a press on the neck, or a note played.
    ToggleScaleAnswering,
    /// Cycles the run's duration: off, three, five, ten minutes.
    CycleScaleDuration,
    /// Hides the neck, so a degree is found from memory. Inert while answering by pressing.
    ToggleScaleNeck,
    /// Answers the drill's current element with a position from the Scale Trainer's neck.
    ChooseScalePosition(usize, usize),
    /// The end of the drill's completed-sequence flash.
    AdvanceScaleSequence,
    /// One second of a timed run, which is what spends its duration.
    WalkTheScaleTick,
    /// A raw key press, translated in `update` rather than the subscription: iced keys a
    /// subscription by its recipe hash, so a closure capturing state would go stale.
    Key(keyboard::Key, keyboard::Modifiers),
    /// A character key that may be an accelerator on the current screen; `update` resolves it.
    Accelerate(char),
    /// Puts the cursor in a screen's search box. Sent by `/` and by `Ctrl+K`.
    OpenSearch,
    ToggleHelp,
    FocusNext,
    FocusPrevious,
    FocusUp,
    FocusDown,
    FocusLeft,
    FocusRight,
    ActivateFocused,
}

/// How often the Tuner asks for audio while it is showing.
const AUDIO_POLL: std::time::Duration = std::time::Duration::from_millis(10);

impl App {
    pub fn new() -> (Self, Task<Message>) {
        // built first because the trainer's opening prompt is drawn from it
        let mut rng = Rng::from_clock();
        let note_trainer = NoteTrainer::new(&mut rng);
        let interval_trainer = IntervalTrainer::new(&mut rng);
        let play_the_interval = PlayTheInterval::new(&mut rng);

        let app = Self {
            screen: Screen::default(),
            history: Vec::new(),
            scale: Scale::new(PitchClass::new(0), ScaleKind::Ionian),
            focused: FocusTarget::HomeCategory(Category::Scales),
            rng,
            help_open: false,
            notation: Notation::Notes,
            note_trainer,
            interval_trainer,
            play_the_interval,
            chord_library: ChordLibrary::new(),
            caged: Caged::new(),
            walk_the_scale: WalkTheScale::new(),
            tuner: Tuner::new(),
            position_finder: PositionFinder::new(),
        };

        (app, Task::none())
    }

    /// Dispatches the message, then keeps the chord list scrolled to its selection.
    ///
    /// Comparing before and after here saves every arm that moves the selection a `Task`.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        let was = self.chord_library.selected_row();
        let task = self.dispatch(message);

        if self.screen == Screen::ChordLibrary && self.chord_library.selected_row() != was {
            return Task::batch([task, self.chord_library.follow_selection()]);
        }

        task
    }

    fn dispatch(&mut self, message: Message) -> Task<Message> {
        // `translate_key` never yields another `Key`, so this recurses exactly once
        if let Message::Key(key, modifiers) = message {
            // while the search box has focus almost every key is text, so it must not reach
            // `translate_key`; what the box declines falls through
            if self.screen == Screen::ChordLibrary && self.chord_library.search_focused() {
                match self.chord_library.handle_key(&key, modifiers) {
                    KeyOutcome::Ignored => {}
                    KeyOutcome::Handled => return Task::none(),
                    // Enter hands the keyboard to the shapes, so left and right walk the voicings
                    KeyOutcome::Accepted => {
                        self.focused = FocusTarget::ChordList;

                        return Task::none();
                    }
                }
            }

            return match translate_key(key, modifiers) {
                // `dispatch`, not `update`: the wrapper has already taken its before reading
                Some(message) => self.dispatch(message),
                None => Task::none(),
            };
        }

        // ahead of the overlay, so a tick does not dismiss the help panel
        if matches!(message, Message::AdvancePrompt) {
            self.note_trainer.advance(&mut self.rng);
            return Task::none();
        }

        if matches!(message, Message::AdvancePlayPrompt) {
            self.play_the_interval.advance(&mut self.rng);
            return Task::none();
        }

        if matches!(message, Message::AdvanceIntervalPrompt) {
            self.interval_trainer.advance(&mut self.rng);
            return Task::none();
        }

        if matches!(message, Message::AdvanceScaleSequence) {
            self.walk_the_scale.advance();
            return Task::none();
        }

        // ahead of the overlay, so a tick spends the duration rather than dismissing the panel
        if matches!(message, Message::WalkTheScaleTick) {
            self.walk_the_scale.tick();
            return Task::none();
        }

        // ahead of the overlay too: it ticks every 10 ms, so the panel would last one frame
        if matches!(message, Message::AudioPoll) {
            self.poll_audio();
            return Task::none();
        }

        // the overlay is modal: anything dismisses it and nothing else happens
        if self.help_open {
            self.help_open = false;
            return Task::none();
        }

        match message {
            Message::Navigate(screen) => self.open(screen),
            Message::OpenCategory(category) => self.focused = FocusTarget::HomeCategory(category),
            // on Home, Esc steps out of a category's entries as Left does
            Message::GoBack if matches!(self.focused, FocusTarget::HomeEntry(..)) => {
                self.focused = home::step(self.focused, Direction::Left);
            }
            // Esc closes the device list before it leaves the screen
            Message::GoBack if self.is_picking() => {
                if let Some(listener) = self.listener() {
                    listener.close_picker();
                }
            }
            // and leaves the calibration walk, keeping what was kept, before it leaves the screen
            Message::GoBack
                if self.screen == Screen::PositionFinder
                    && self.position_finder.is_calibrating() =>
            {
                self.position_finder.leave_calibration();
            }
            // and leaves the drill before it leaves the screen
            Message::GoBack
                if self.screen == Screen::WalkTheScale && self.walk_the_scale.is_running() =>
            {
                self.walk_the_scale.leave();
            }
            Message::GoBack => self.go_back(),
            Message::SelectRoot(root) => self.set_scale(root, self.scale.kind()),
            Message::SelectScaleKind(kind) => self.set_scale(self.scale.root(), kind),
            Message::ToggleNotation => self.toggle_notation(),
            Message::RerollScale => self.reroll_scale(),
            Message::StartWalkTheScale => self.walk_the_scale.restart(),
            Message::SelectDrillKind(kind) => self.walk_the_scale.select_kind(kind),
            Message::GrabDrillRoot(x) => self.walk_the_scale.grab_root(x),
            Message::DragDrillRoot(x) => self.walk_the_scale.drag_root(x),
            Message::DropDrillRoot => self.walk_the_scale.drop_root(),
            Message::ScrollDrillRoot(steps) => self.walk_the_scale.scroll_root(steps),
            Message::StepDrillRoot(steps) => self.walk_the_scale.step_root(steps),
            Message::RerollDrillScale => self.walk_the_scale.reroll(&mut self.rng),
            Message::ToggleScaleAnswering => self.walk_the_scale.toggle_answering(),
            Message::CycleScaleDuration => self.walk_the_scale.cycle_duration(),
            Message::ToggleScaleNeck => self.walk_the_scale.toggle_neck(),
            Message::ChooseScalePosition(string, fret) => {
                if let Some(pitch_class) = DRILL_NECK.pitch_class_at(string, fret) {
                    self.walk_the_scale.press(pitch_class);
                }
            }
            // Both handled above, before the help overlay gets a say.
            Message::AdvanceScaleSequence | Message::WalkTheScaleTick => {}
            // these borrow `self.note_trainer` and `self.rng` at once, as disjoint fields
            Message::AnswerNote(pitch_class) => self.note_trainer.answer_name(pitch_class),
            Message::ChooseNotePosition(string, fret) => {
                self.note_trainer.answer_position(string, fret)
            }
            Message::SkipPrompt => self.note_trainer.skip(&mut self.rng),
            // Handled above, before the help overlay gets a say.
            Message::AdvancePrompt => {}
            Message::ToggleDrillDirection => self.note_trainer.toggle_direction(&mut self.rng),
            Message::ToggleNoteAnswering => self.note_trainer.toggle_answering(&mut self.rng),
            Message::TogglePool => self.note_trainer.toggle_pool(&mut self.rng),
            Message::ToggleNoteSpelling => self.note_trainer.toggle_spelling(),
            Message::ToggleTunerSpelling => self.tuner.toggle_spelling(),
            Message::AnswerInterval(interval) => self.interval_trainer.answer_interval(interval),
            Message::ChooseIntervalPosition(string, fret) => {
                self.interval_trainer.answer_position(string, fret)
            }
            Message::SkipIntervalPrompt => self.interval_trainer.skip(&mut self.rng),
            Message::ToggleIntervalDirection => {
                self.interval_trainer.toggle_direction(&mut self.rng)
            }
            // Handled above, before the help overlay gets a say.
            Message::AdvanceIntervalPrompt => {}
            Message::SkipPlayPrompt => self.play_the_interval.skip(&mut self.rng),
            Message::AdvancePlayPrompt => {}
            // Handled above, ahead of the help overlay — see the early return there.
            Message::AudioPoll => {}
            #[cfg(test)]
            Message::Heard(heard) => self.hear(heard),
            #[cfg(test)]
            Message::Fitted(heard, fit) => self.hear_fitted(heard, fit),
            Message::Calibrate => self.position_finder.start_calibration(),
            Message::TogglePositionGroups => self.position_finder.toggle_groups(),
            Message::OpenDevices => {
                if let Some(listener) = self.listener() {
                    listener.open_picker();
                }
            }
            Message::ChooseDevice(index) => {
                if let Some(listener) = self.listener() {
                    listener.choose(index);
                }
            }
            Message::Key(..) => {}
            Message::SelectChordRow(index) => self.chord_library.select_row(index),
            Message::SelectVoicing(index) => self.chord_library.select_voicing(index),
            Message::SetNotation(index) => self.activate(FocusTarget::NotationChoice(index)),
            Message::SelectCagedRoot(root) => self.caged.select_root(root),
            Message::SelectCagedShape(index) => self.caged.select_shape(index),
            Message::RerollCagedRoot => self.reroll_caged_root(),
            Message::Accelerate(c) => return self.accelerate(c),
            // only the library has a box, and `Ctrl+K` arrives from any screen
            Message::OpenSearch => {
                if self.screen == Screen::ChordLibrary {
                    self.chord_library.focus_search();
                }
            }
            Message::ToggleHelp => self.help_open = true,
            Message::FocusNext => self.cycle_focus(1),
            Message::FocusPrevious => self.cycle_focus(-1),
            Message::FocusUp => self.move_focus(Direction::Up),
            Message::FocusDown => self.move_focus(Direction::Down),
            Message::FocusLeft => self.move_focus(Direction::Left),
            Message::FocusRight => self.move_focus(Direction::Right),
            Message::ActivateFocused => {
                // while the list is up, Enter picks a device rather than firing the ring
                match self.listener().and_then(|listener| listener.picking()) {
                    Some(index) => {
                        if let Some(listener) = self.listener() {
                            listener.choose(index);
                        }
                    }
                    None => self.activate_focused(),
                }
            }
        }
        Task::none()
    }

    /// Opens a screen from anywhere, so every entry point shares the same entry rules.
    fn open(&mut self, screen: Screen) {
        let wants_fresh_scale = screen == Screen::ScaleTrainer;
        let wants_fresh_prompt = screen == Screen::NoteTrainer;
        let wants_fresh_interval = screen == Screen::IntervalTrainer;
        let wants_the_search_box = screen == Screen::ChordLibrary;
        let wants_the_caged_window = screen == Screen::Caged;
        let wants_fresh_play = screen == Screen::PlayTheInterval;
        let wants_a_fresh_walk = screen == Screen::WalkTheScale;
        let wants_the_calibration = screen == Screen::PositionFinder;
        let wants_to_listen = matches!(
            screen,
            Screen::Tuner | Screen::PlayTheInterval | Screen::PositionFinder
        );

        self.navigate_to(screen);

        // opens on entering, so the tuner never looks broken; failing is a state the screen draws
        if wants_to_listen && let Some(listener) = self.listener() {
            listener.listen();
        }

        if wants_fresh_scale {
            self.reroll_scale();
        }

        // the scale is kept; the run is fresh and the best streak persists
        if wants_a_fresh_walk {
            self.walk_the_scale.enter();
        }

        // a fresh scale each visit; settings and the best streak persist
        if wants_fresh_prompt {
            self.note_trainer.enter(&mut self.rng);
        }

        if wants_fresh_interval {
            self.interval_trainer.enter(&mut self.rng);
        }

        if wants_fresh_play {
            self.play_the_interval.enter(&mut self.rng);
        }

        // opens with the search box focused; `enter` clears the query but keeps the root
        if wants_the_search_box {
            self.chord_library.enter();
            self.focused = FocusTarget::SearchBox;
        }

        // a reference, so it reopens on what it last showed
        if wants_the_caged_window {
            self.caged.enter();
        }

        // reads the kept calibration afresh, so one captured since is in use
        if wants_the_calibration {
            self.position_finder.enter();
        }
    }

    fn navigate_to(&mut self, screen: Screen) {
        self.history.push(self.screen.clone());
        self.leaving_for(&screen);
        self.screen = screen;
        self.reset_focus();
    }

    fn go_back(&mut self) {
        if let Some(prev) = self.history.pop() {
            self.leaving_for(&prev);
            let left = std::mem::replace(&mut self.screen, prev);
            self.reset_focus();

            // with the category it sits in open, so the way back retraces the way in
            if self.screen == Screen::Home
                && let Some(entry) = home::entry_for(&left)
            {
                self.focused = entry;
            }
        }
    }

    /// What a screen has to put down on the way out, such as an open capture device.
    fn leaving_for(&mut self, next: &Screen) {
        if self.screen == *next {
            return;
        }

        // a run's clock must not keep spending behind a screen nobody is looking at
        if self.screen == Screen::WalkTheScale {
            self.walk_the_scale.leave();
        }

        if self.screen == Screen::NoteTrainer {
            self.note_trainer.leave();
        }

        // asked before `self.screen` moves, so one stream closes before the other opens
        if let Some(listener) = self.listener() {
            listener.stop();
        }
    }

    /// The device of whichever screen is listening.
    ///
    /// Routing by the screen on show is sound only because `leaving_for` closes the device.
    fn listener(&mut self) -> Option<&mut Listener> {
        match self.screen {
            Screen::Tuner => Some(self.tuner.listener_mut()),
            Screen::PlayTheInterval => Some(self.play_the_interval.listener_mut()),
            Screen::WalkTheScale => self.walk_the_scale.listener_mut(),
            Screen::NoteTrainer => self.note_trainer.listener_mut(),
            Screen::PositionFinder => Some(self.position_finder.listener_mut()),
            _ => None,
        }
    }

    fn listener_ref(&self) -> Option<&Listener> {
        match self.screen {
            Screen::Tuner => Some(self.tuner.listener()),
            Screen::PlayTheInterval => Some(self.play_the_interval.listener()),
            Screen::WalkTheScale => self.walk_the_scale.listener(),
            Screen::NoteTrainer => self.note_trainer.listener(),
            Screen::PositionFinder => Some(self.position_finder.listener()),
            _ => None,
        }
    }

    fn is_picking(&self) -> bool {
        self.listener_ref().is_some_and(Listener::is_picking)
    }

    fn is_listening(&self) -> bool {
        self.listener_ref().is_some_and(Listener::is_listening)
    }

    /// Whether the run's one-second clock should tick. Split out because a `Subscription` cannot
    /// be inspected.
    fn wants_run_clock(&self) -> bool {
        self.screen == Screen::WalkTheScale && self.walk_the_scale.is_running()
    }

    /// One tick of audio, folded into the screen that asked for it.
    fn poll_audio(&mut self) {
        match self.screen {
            Screen::Tuner => {
                self.tuner.listener_mut().poll();
            }
            // fold and judge in one call, so there is only one reader of the reading
            Screen::PlayTheInterval => {
                if let Some((heard, fit)) = self.play_the_interval.listener_mut().poll_fitted() {
                    self.play_the_interval.apply(heard, fit);
                }
            }
            Screen::WalkTheScale => {
                let heard = self
                    .walk_the_scale
                    .listener_mut()
                    .and_then(Listener::poll_fitted);
                if let Some((heard, fit)) = heard {
                    self.walk_the_scale.apply(heard, fit);
                }
            }
            Screen::NoteTrainer => {
                let heard = self
                    .note_trainer
                    .listener_mut()
                    .and_then(Listener::poll_fitted);
                if let Some((heard, fit)) = heard {
                    self.note_trainer.apply(heard, fit);
                }
            }
            Screen::PositionFinder => {
                if let Some((heard, fit)) = self.position_finder.listener_mut().poll_fitted() {
                    self.position_finder.apply(heard, fit);
                }
            }
            _ => {}
        }
    }

    /// A released reading, handed to the screen listening. The test seam in place of a device.
    #[cfg(test)]
    fn hear(&mut self, heard: Heard) {
        match self.screen {
            Screen::Tuner => self.tuner.listener_mut().apply(heard),
            Screen::WalkTheScale
            | Screen::NoteTrainer
            | Screen::PlayTheInterval
            | Screen::PositionFinder => self.hear_fitted(heard, None),
            _ => {}
        }
    }

    /// A released reading with its fit, handed to the screen that places notes as its poll
    /// would.
    #[cfg(test)]
    fn hear_fitted(&mut self, heard: Heard, fit: Option<Fit>) {
        match self.screen {
            Screen::WalkTheScale => {
                if let Some(listener) = self.walk_the_scale.listener_mut() {
                    listener.apply(heard);
                }
                self.walk_the_scale.apply(heard, fit);
            }
            Screen::NoteTrainer => {
                if let Some(listener) = self.note_trainer.listener_mut() {
                    listener.apply(heard);
                }
                self.note_trainer.apply(heard, fit);
            }
            Screen::PlayTheInterval => {
                self.play_the_interval.listener_mut().apply(heard);
                self.play_the_interval.apply(heard, fit);
            }
            Screen::PositionFinder => {
                self.position_finder.listener_mut().apply(heard);
                self.position_finder.apply(heard, fit);
            }
            _ => {}
        }
    }

    /// Picks a new scale, never the one already on screen.
    fn reroll_scale(&mut self) {
        let current = (self.scale.root(), self.scale.kind());
        let (root, kind) = self.rng.draw_other_than(current, |rng| {
            (
                PitchClass::ALL[rng.below(PitchClass::ALL.len())],
                ScaleKind::ALL[rng.below(ScaleKind::ALL.len())],
            )
        });
        self.set_scale(root, kind);
    }

    /// A different root than the one on show.
    fn reroll_caged_root(&mut self) {
        let root = self.rng.draw_other_than(self.caged.root(), |rng| {
            PitchClass::ALL[rng.below(PitchClass::ALL.len())]
        });
        self.caged.select_root(root);
    }

    /// The one place a `Scale` is built, since its spelling derives from root and kind together.
    fn set_scale(&mut self, root: PitchClass, kind: ScaleKind) {
        self.scale = Scale::new(root, kind);
    }

    /// Flips what the markers say. Nothing about the scale moves.
    fn toggle_notation(&mut self) {
        self.notation = match self.notation {
            Notation::Notes => Notation::Intervals,
            // a scale has no fingering, so this never holds `Fingers`
            Notation::Intervals | Notation::Fingers => Notation::Notes,
        };
    }

    fn reset_focus(&mut self) {
        self.focused = self
            .focusables()
            .first()
            .copied()
            .unwrap_or(FocusTarget::Back);
    }

    /// Tab order: walks every focusable in reading order, wrapping at the ends.
    fn cycle_focus(&mut self, delta: isize) {
        let list = self.focusables();
        self.focused = step_focus(&list, self.focused, delta);
    }

    /// Arrow keys: moves one cell within the screen's focus grid, stopping at edges.
    ///
    /// The focused neck is a deliberate special case that claims them for its own cursor.
    fn move_focus(&mut self, direction: Direction) {
        // the open device list is modal and claims up and down before anything else
        if self.is_picking() {
            let delta = match direction {
                Direction::Up => -1,
                Direction::Down => 1,
                Direction::Left | Direction::Right => 0,
            };
            if let Some(listener) = self.listener() {
                listener.move_pick(delta);
            }
            return;
        }

        if matches!(
            self.focused,
            FocusTarget::HomeCategory(_) | FocusTarget::HomeEntry(..)
        ) {
            self.focused = home::step(self.focused, direction);
            return;
        }

        if self.focused == FocusTarget::Fretboard {
            self.note_trainer.move_cursor(direction);
            return;
        }

        if self.focused == FocusTarget::IntervalFretboard {
            self.interval_trainer.move_cursor(direction);
            return;
        }

        if self.focused == FocusTarget::ScaleFretboard {
            self.walk_the_scale.move_cursor(direction);
            return;
        }

        // up and down walk the chords, left and right walk the voicings
        if self.focused == FocusTarget::ChordList {
            match direction {
                Direction::Up => self.chord_library.move_row(-1),
                Direction::Down => self.chord_library.move_row(1),
                Direction::Left => self.chord_library.move_voicing(-1),
                Direction::Right => self.chord_library.move_voicing(1),
            }
            return;
        }

        // the pills claim left and right only; up and down still leave the row
        if let FocusTarget::CagedShape(_) = self.focused {
            let delta = match direction {
                Direction::Left => Some(-1),
                Direction::Right => Some(1),
                Direction::Up | Direction::Down => None,
            };

            if let Some(delta) = delta {
                self.caged.move_selection(delta);
                self.focused = FocusTarget::CagedShape(self.caged.selected());
                return;
            }
        }

        let grid = self.focus_grid();
        self.focused = step_focus_2d(&grid, self.focused, direction);
    }

    fn activate_focused(&mut self) {
        self.activate(self.focused);
    }

    /// Fires the accelerator `c` names on the current screen, if it names one.
    fn accelerate(&mut self, c: char) -> Task<Message> {
        // offered to the screen first, since the table has nowhere to keep the pending `g` of `gg`
        if self.screen == Screen::ChordLibrary && self.chord_library.motion(c) {
            return Task::none();
        }

        let fires = self
            .accelerators()
            .into_iter()
            .find_map(|(key, fires, _)| (key == c).then_some(fires));

        match fires {
            Some(Fires::Widget(target)) => self.activate(target),
            // `dispatch`, as for a translated key, with its task handed back
            Some(Fires::Message(message)) => return self.dispatch(message),
            None => {}
        }

        Task::none()
    }

    /// Performs `target`'s action without walking the focus ring onto it.
    fn activate(&mut self, target: FocusTarget) {
        match target {
            FocusTarget::HomeCategory(category) => {
                self.focused = FocusTarget::HomeEntry(category, 0);
            }
            FocusTarget::HomeEntry(category, index) => {
                if let Some(item) = category.entries().get(index) {
                    self.open(item.screen.clone());
                }
            }
            FocusTarget::Back => self.go_back(),
            FocusTarget::SkipPlayPrompt => self.play_the_interval.skip(&mut self.rng),
            FocusTarget::InputDevice => {
                if let Some(listener) = self.listener() {
                    listener.open_picker();
                }
            }
            FocusTarget::CagedRoot(index) => {
                if let Some(&pitch_class) = PitchClass::ALL.get(index) {
                    self.caged.select_root(pitch_class);
                }
            }
            FocusTarget::CagedShape(index) => self.caged.select_shape(index),
            FocusTarget::RerollCagedRoot => self.reroll_caged_root(),
            FocusTarget::DrillKind(index) => {
                if let Some(&kind) = ScaleKind::ALL.get(index) {
                    self.walk_the_scale.select_kind(kind);
                }
            }
            FocusTarget::RerollDrillScale => self.walk_the_scale.reroll(&mut self.rng),
            FocusTarget::RestartDrill => self.walk_the_scale.restart(),
            FocusTarget::ScaleAnswering => self.walk_the_scale.toggle_answering(),
            FocusTarget::ScaleDuration => self.walk_the_scale.cycle_duration(),
            FocusTarget::ScaleNeck => self.walk_the_scale.toggle_neck(),
            FocusTarget::ScaleFretboard => self.walk_the_scale.answer_at_cursor(),
            FocusTarget::NotationToggle => self.toggle_notation(),
            FocusTarget::RerollScale => self.reroll_scale(),
            FocusTarget::Root(index) => {
                if let Some(&pitch_class) = PitchClass::ALL.get(index) {
                    self.set_scale(pitch_class, self.scale.kind());
                }
            }
            FocusTarget::ScaleKind(index) => {
                if let Some(&kind) = ScaleKind::ALL.get(index) {
                    self.set_scale(self.scale.root(), kind);
                }
            }
            // the same as a click on the cursor's position
            FocusTarget::Fretboard => self.note_trainer.answer_at_cursor(),
            FocusTarget::NoteAnswer(index) => {
                if let Some(&pitch_class) = PitchClass::ALL.get(index) {
                    self.note_trainer.answer_name(pitch_class);
                }
            }
            FocusTarget::DrillDirectionToggle => self.note_trainer.toggle_direction(&mut self.rng),
            FocusTarget::NoteAnswering => self.note_trainer.toggle_answering(&mut self.rng),
            FocusTarget::PoolToggle => self.note_trainer.toggle_pool(&mut self.rng),
            FocusTarget::NoteSpellingToggle => self.note_trainer.toggle_spelling(),
            FocusTarget::TunerSpellingToggle => self.tuner.toggle_spelling(),
            FocusTarget::Calibrate => self.position_finder.start_calibration(),
            FocusTarget::PositionGroups => self.position_finder.toggle_groups(),
            FocusTarget::SkipPrompt => self.note_trainer.skip(&mut self.rng),
            FocusTarget::SearchBox => self.chord_library.focus_search(),
            FocusTarget::NotationChoice(index) => self.chord_library.set_notation(index),
            // nothing to fire: a chord is selected as soon as the ring reaches it
            FocusTarget::ChordList => {}
            FocusTarget::IntervalFretboard => self.interval_trainer.answer_at_cursor(),
            FocusTarget::IntervalAnswer(index) => {
                if let Some(interval) = answer_at(index) {
                    self.interval_trainer.answer_interval(interval);
                }
            }
            FocusTarget::IntervalDirectionToggle => {
                self.interval_trainer.toggle_direction(&mut self.rng)
            }
            FocusTarget::SkipIntervalPrompt => self.interval_trainer.skip(&mut self.rng),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let screen = match self.screen {
            Screen::Home => ui_home(self.focused),
            Screen::ScaleTrainer => with_top_bar(
                "Scale Trainer",
                ui_scale_trainer(self.scale, self.notation, self.focused),
                true,
                self.focused,
            ),
            Screen::NoteTrainer => with_top_bar(
                "Note Trainer",
                ui_note_trainer(&self.note_trainer, self.focused),
                true,
                self.focused,
            ),
            Screen::IntervalTrainer => with_top_bar(
                "Interval Trainer",
                ui_interval_trainer(&self.interval_trainer, self.focused),
                true,
                self.focused,
            ),
            Screen::ChordLibrary => with_top_bar(
                "Chord Library",
                ui_chord_library(&self.chord_library, self.focused),
                true,
                self.focused,
            ),
            Screen::Caged => with_top_bar(
                "CAGED",
                ui_caged(&self.caged, self.notation, self.focused),
                true,
                self.focused,
            ),
            Screen::PlayTheInterval => with_top_bar(
                "Play the Interval",
                ui_play_the_interval(&self.play_the_interval, self.focused),
                true,
                self.focused,
            ),
            Screen::WalkTheScale => with_top_bar(
                "Walk the Scale",
                ui_walk_the_scale(&self.walk_the_scale, self.focused),
                true,
                self.focused,
            ),
            Screen::Tuner => with_top_bar(
                "Tuner",
                ui_tuner(&self.tuner, self.focused),
                true,
                self.focused,
            ),
            Screen::PositionFinder => with_top_bar(
                "Position Finder",
                ui_position_finder(&self.position_finder, self.focused),
                true,
                self.focused,
            ),
        };

        if self.help_open {
            iced::widget::stack![screen, ui_help_overlay(self.accelerators())].into()
        } else {
            screen
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let keys = keyboard::listen().filter_map(|event| {
            // `modified_key`, since `key` arrives without modifiers and Shift+/ would read as `/`
            let keyboard::Event::KeyPressed {
                modified_key,
                modifiers,
                ..
            } = event
            else {
                return None;
            };
            // no translation here; see `Message::Key`
            Some(Message::Key(modified_key, modifiers))
        });

        // only while a flash is on screen, so its first tick lands a full second after the
        // answer. Each trainer has its own, so a tick cannot retire another screen's prompt.
        let mut streams = vec![keys];

        if self.note_trainer.is_flashing() {
            streams.push(iced::time::every(ANSWER_FLASH).map(|_| Message::AdvancePrompt));
        }

        if self.interval_trainer.is_flashing() {
            streams.push(iced::time::every(ANSWER_FLASH).map(|_| Message::AdvanceIntervalPrompt));
        }

        if self.play_the_interval.is_flashing() {
            streams.push(iced::time::every(ANSWER_FLASH).map(|_| Message::AdvancePlayPrompt));
        }

        if self.walk_the_scale.is_flashing() {
            streams.push(iced::time::every(ANSWER_FLASH).map(|_| Message::AdvanceScaleSequence));
        }

        // one second is the resolution the readout needs
        if self.wants_run_clock() {
            streams
                .push(iced::time::every(Duration::from_secs(1)).map(|_| Message::WalkTheScaleTick));
        }

        // 10 ms is a little under the detector's hop, so a window is picked up the tick after
        // it completes
        if self.is_listening() {
            streams.push(iced::time::every(AUDIO_POLL).map(|_| Message::AudioPoll));
        }

        Subscription::batch(streams)
    }
}

/// The back arrow's size. The maths face needs more than the body's 18 to read as an arrow.
const BACK_ARROW_SIZE: f32 = 22.0;

fn with_top_bar(
    label: &'static str,
    content: Element<'static, Message>,
    has_back: bool,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{button, column, container, row, text};

    let back_button = focus_ring(
        button(text(KEYCAP_BACK).font(KEYCAP_FONT).size(BACK_ARROW_SIZE))
            .style(ghost_button)
            .padding([6, 12])
            .on_press(Message::GoBack),
        focused == FocusTarget::Back,
    );

    let page = if has_back {
        let header = row![back_button, text(label).size(24).color(INK)]
            .spacing(16)
            .padding([18, 32]);

        column![header, content].spacing(12)
    } else {
        column![content]
    };

    container(page)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(page_container)
        .into()
}

/// Each button is named by the scale pressing it would build, so labels move with the kind.
/// The message and focus target are parameters because the trainer and the drill each own a scale.
fn root_note_row(
    pitch_classes: &[PitchClass],
    scale: Scale,
    start_index: usize,
    focused: FocusTarget,
    on_press: fn(PitchClass) -> Message,
    target: fn(usize) -> FocusTarget,
) -> iced::widget::Row<'static, Message> {
    use iced::widget::row;

    pitch_classes
        .iter()
        .enumerate()
        .fold(row![].spacing(28), |row, (i, pitch_class)| {
            let is_selected = *pitch_class == scale.root();
            let color = if is_selected { CANVAS } else { INK };

            row.push(root_square(
                note_label(
                    Scale::new(*pitch_class, scale.kind()).root_note(),
                    24,
                    color,
                ),
                is_selected,
                focused == target(start_index + i),
                on_press(*pitch_class),
            ))
        })
}

/// The kind pills, parameterised like `root_note_row`.
fn scale_kind_row(
    kinds: &[ScaleKind],
    selected: ScaleKind,
    start_index: usize,
    focused: FocusTarget,
    on_press: fn(ScaleKind) -> Message,
    target: fn(usize) -> FocusTarget,
) -> iced::widget::Row<'static, Message> {
    use iced::widget::{button, row, text};

    kinds
        .iter()
        .enumerate()
        .fold(row![].spacing(8), |row, (i, kind)| {
            row.push(focus_ring(
                button(text(kind.name()).size(KIND_BUTTON_SIZE))
                    .padding([8, 12])
                    .style(if *kind == selected {
                        selected_root_button
                    } else {
                        ghost_button
                    })
                    .on_press(on_press(*kind)),
                focused == target(start_index + i),
            ))
        })
}

/// One square root button, wrapped in its focus ring. Shared by the Scale Trainer and CAGED.
fn root_square(
    label: iced::widget::Row<'static, Message>,
    is_selected: bool,
    is_focused: bool,
    message: Message,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{button, container};

    let square = button(
        container(label)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(ROOT_BUTTON_SIZE))
    .height(Length::Fixed(ROOT_BUTTON_SIZE))
    .padding(0)
    .style(if is_selected {
        selected_root_button
    } else {
        ghost_button
    })
    .on_press(message);

    focus_ring(
        container(square)
            .width(Length::Fixed(ROOT_BUTTON_SIZE))
            .height(Length::Fixed(ROOT_BUTTON_SIZE))
            .center_x(Length::Fixed(ROOT_BUTTON_SIZE))
            .center_y(Length::Fixed(ROOT_BUTTON_SIZE)),
        is_focused,
    )
}

fn note_label(
    note: Note,
    size: impl Into<Pixels>,
    color: Color,
) -> iced::widget::Row<'static, Message> {
    use iced::widget::{row, text};

    // resolved once because `impl Into<Pixels>` would be moved by the first `size`
    let size = size.into();
    let label = row![text(note.letter.to_string()).size(size).color(color)].spacing(0);

    match accidental_glyph(note.accidental) {
        Some(glyph) => label.push(
            text(glyph.to_string())
                .size(size)
                .font(MUSIC_FONT)
                .color(color),
        ),
        None => label,
    }
}

/// The SMuFL glyph for an accidental. `None` for a natural, which a label never shows.
fn accidental_glyph(accidental: Accidental) -> Option<char> {
    match accidental {
        Accidental::DoubleFlat => Some(SMUFL_DOUBLE_FLAT),
        Accidental::Flat => Some(SMUFL_FLAT),
        Accidental::Natural => None,
        Accidental::Sharp => Some(SMUFL_SHARP),
        Accidental::DoubleSharp => Some(SMUFL_DOUBLE_SHARP),
    }
}

/// The glyph (if any) and degree digit labelling one formula interval, e.g. Blues's ♭5 is
/// `(Some(SMUFL_FLAT), 5)`. Split because the two parts render in different fonts.
fn interval_token(interval: Interval) -> (Option<char>, u8) {
    (accidental_glyph(interval.alteration()), interval.number())
}

fn intervalic_text(intervals: &'static [Interval]) -> iced::widget::Row<'static, Message> {
    use iced::widget::{row, text};

    intervals
        .iter()
        .fold(row![].spacing(8), |tokens, interval| {
            let (glyph, digit) = interval_token(*interval);
            let mut token = row![].spacing(0);

            if let Some(glyph) = glyph {
                token = token.push(
                    text(glyph.to_string())
                        .size(SUMMARY_FORMULA_SIZE)
                        .font(MUSIC_FONT)
                        .color(BODY),
                );
            }

            tokens.push(
                token.push(
                    text(digit.to_string())
                        .size(SUMMARY_FORMULA_SIZE)
                        .color(BODY),
                ),
            )
        })
}

/// The text inside one marker: a note's name, or the job it does in the scale.
fn marker_label(notation: Notation, note: Note, degree: Interval) -> String {
    match notation {
        Notation::Notes => note.to_string(),
        // unreachable here, since the library keeps its own fingering setting
        Notation::Intervals | Notation::Fingers => degree.to_string(),
    }
}

fn trainer_button<'a>(
    title: &'static str,
    caption: &'static str,
) -> iced::widget::Button<'a, Message> {
    use iced::widget::{button, column, text};

    button(
        column![
            text(title).size(19).color(INK),
            text(caption).size(16).color(BODY)
        ]
        .spacing(4),
    )
    .width(360)
    .padding([16, 20])
    .style(card_button)
}

#[cfg(test)]
mod tests {
    use super::focus::KIND_ROW_WIDTHS;
    use super::keys::accelerators_for;
    use super::*;
    use iced::keyboard::key::Named;
    use std::collections::HashSet;

    /// An app whose scale rolls are reproducible.
    pub(super) fn app_with_seed(seed: u64) -> App {
        let (mut app, _) = App::new();
        app.rng = Rng::from_seed(seed);
        app
    }

    /// An app parked on `screen`, for the focus-layout methods.
    pub(super) fn app_on(screen: Screen) -> App {
        let mut app = app_with_seed(0);
        app.screen = screen;
        app
    }

    /// The current scale as a pair of indices into `PitchClass::ALL` and
    /// `ScaleKind::ALL`.
    fn scale_indices(app: &App) -> (usize, usize) {
        let root = PitchClass::ALL
            .iter()
            .position(|pitch_class| *pitch_class == app.scale.root())
            .expect("root is one of PitchClass::ALL");
        let kind = ScaleKind::ALL
            .iter()
            .position(|k| *k == app.scale.kind())
            .expect("kind is one of ScaleKind::ALL");

        (root, kind)
    }

    #[test]
    fn every_root_and_kind_combination_is_reachable() {
        let mut app = app_with_seed(0x5ca1e);
        let mut seen = HashSet::new();

        for _ in 0..20_000 {
            app.reroll_scale();
            seen.insert(scale_indices(&app));
        }

        // a clock-seeded draw per roll once left only lcm(12, 16) = 48 of the 192 pairs reachable
        let total = PitchClass::ALL.len() * ScaleKind::ALL.len();
        assert_eq!(
            seen.len(),
            total,
            "only {} of {total} pairs seen",
            seen.len()
        );
    }

    #[test]
    fn rerolling_always_changes_the_scale() {
        let mut app = app_with_seed(7);

        for _ in 0..2_000 {
            let before = (app.scale.root(), app.scale.kind());
            app.reroll_scale();
            assert_ne!((app.scale.root(), app.scale.kind()), before);
        }
    }

    #[test]
    fn rolls_are_reproducible_from_a_seed() {
        let (mut a, mut b) = (app_with_seed(42), app_with_seed(42));

        for _ in 0..64 {
            a.reroll_scale();
            b.reroll_scale();
            assert_eq!(scale_indices(&a), scale_indices(&b));
        }
    }

    #[test]
    fn selecting_a_root_names_it_the_way_it_is_taught() {
        // pitch class 10 under Ionian is B♭ major, not A♯ major
        let mut app = app_with_seed(7);
        let _ = app.update(Message::SelectScaleKind(ScaleKind::Ionian));
        let _ = app.update(Message::SelectRoot(PitchClass::new(10)));

        assert_eq!(app.scale.root_note().to_string(), "Bb");
        assert!(
            app.scale
                .notes()
                .iter()
                .all(|note| note.accidental.offset().abs() < 2),
            "B♭ Ionian should need no double accidental: {:?}",
            app.scale.notes()
        );
    }

    #[test]
    fn a_root_button_is_named_by_the_scale_pressing_it_builds() {
        // both kinds, since the label moves with the kind
        for kind in [ScaleKind::Ionian, ScaleKind::MinorPentatonic] {
            let mut app = app_with_seed(5);
            let _ = app.update(Message::SelectScaleKind(kind));

            for &pitch_class in &PitchClass::ALL {
                let label = Scale::new(pitch_class, kind).root_note();

                let _ = app.update(Message::SelectRoot(pitch_class));

                assert_eq!(
                    app.scale.root_note(),
                    label,
                    "{kind:?} button for {pitch_class:?} promised {label}"
                );
            }
        }
    }

    #[test]
    fn a_natural_renders_no_glyph_and_the_rest_map_to_smufl() {
        // the glyph constants are hand-written, so a swapped pair would otherwise compile
        assert_eq!(accidental_glyph(Accidental::Natural), None);
        assert_eq!(accidental_glyph(Accidental::Flat), Some(SMUFL_FLAT));
        assert_eq!(accidental_glyph(Accidental::Sharp), Some(SMUFL_SHARP));
        assert_eq!(
            accidental_glyph(Accidental::DoubleFlat),
            Some(SMUFL_DOUBLE_FLAT)
        );
        assert_eq!(
            accidental_glyph(Accidental::DoubleSharp),
            Some(SMUFL_DOUBLE_SHARP)
        );
    }

    /// Presses a character key with the given modifiers held.
    ///
    /// `c` is the modified key, so Shift+/ is `press("?", SHIFT)`, not `press("/", SHIFT)`.
    pub(super) fn press(c: &str, modifiers: keyboard::Modifiers) -> Option<Message> {
        translate_key(keyboard::Key::Character(c.into()), modifiers)
    }

    /// Every screen, so a new one cannot quietly escape the checks that sweep them all.
    pub(super) fn every_screen() -> [Screen; 10] {
        // no `_` arm, so a new `Screen` stops this compiling until it is listed
        match Screen::default() {
            Screen::Home
            | Screen::ScaleTrainer
            | Screen::WalkTheScale
            | Screen::NoteTrainer
            | Screen::IntervalTrainer
            | Screen::PlayTheInterval
            | Screen::ChordLibrary
            | Screen::Caged
            | Screen::Tuner
            | Screen::PositionFinder => {}
        }

        [
            Screen::Home,
            Screen::ScaleTrainer,
            Screen::WalkTheScale,
            Screen::NoteTrainer,
            Screen::IntervalTrainer,
            Screen::PlayTheInterval,
            Screen::ChordLibrary,
            Screen::Caged,
            Screen::Tuner,
            Screen::PositionFinder,
        ]
    }

    /// The state a keypress could disturb; an unbound key shows only as no change here.
    fn snapshot(app: &App) -> (Screen, FocusTarget, PitchClass, ScaleKind, Notation, usize) {
        (
            app.screen.clone(),
            app.focused,
            app.scale.root(),
            app.scale.kind(),
            // without `notation`, `i` doing nothing would pass
            app.notation,
            app.history.len(),
        )
    }

    /// Drives a key press the way the subscription does: as a raw `Message::Key` into `update`.
    pub(super) fn press_into(app: &mut App, c: &str, modifiers: keyboard::Modifiers) {
        let _ = app.update(Message::Key(keyboard::Key::Character(c.into()), modifiers));
    }

    #[test]
    fn three_then_two_opens_the_caged_window() {
        let mut app = app_with_seed(0xca6);
        press_into(&mut app, "3", keyboard::Modifiers::empty());
        press_into(&mut app, "2", keyboard::Modifiers::empty());

        assert_eq!(app.screen, Screen::Caged);
        assert!(!app.caged.shapes().is_empty());
    }

    #[test]
    fn the_caged_window_reopens_on_the_root_it_was_left_on() {
        let mut app = app_with_seed(0xca6);
        app.open(Screen::Caged);

        app.caged.select_root(PitchClass::new(9));
        app.caged.move_selection(2);
        let (root, selected) = (app.caged.root(), app.caged.selected());

        app.go_back();
        app.open(Screen::Caged);

        // A reference rather than a run: unlike the trainers, it comes back to what was on it.
        assert_eq!(app.caged.root(), root);
        assert_eq!(app.caged.selected(), selected);
    }

    #[test]
    fn the_arrows_walk_the_caged_cycle_without_a_second_press() {
        let mut app = app_with_seed(0xca6);
        app.open(Screen::Caged);
        app.focused = FocusTarget::CagedShape(0);

        app.move_focus(Direction::Right);

        assert_eq!(app.caged.selected(), 1, "the selection did not follow");
        assert_eq!(
            app.focused,
            FocusTarget::CagedShape(1),
            "the ring did not follow the selection"
        );

        // Stops at the ends rather than wrapping, as every other row does.
        app.move_focus(Direction::Left);
        app.move_focus(Direction::Left);
        assert_eq!(app.caged.selected(), 0);
        assert_eq!(app.focused, FocusTarget::CagedShape(0));
    }

    #[test]
    fn tab_still_escapes_the_caged_pills() {
        let mut app = app_with_seed(0xca6);
        app.open(Screen::Caged);
        app.focused = FocusTarget::CagedShape(0);

        app.cycle_focus(1);

        assert_ne!(app.focused, FocusTarget::CagedShape(0));
        assert!(matches!(
            app.focused,
            FocusTarget::CagedShape(_) | FocusTarget::CagedRoot(_)
        ));
    }

    #[test]
    fn the_caged_accelerators_act_without_moving_the_ring() {
        let mut app = app_with_seed(0xca6);
        app.open(Screen::Caged);
        app.focused = FocusTarget::Back;

        let root = app.caged.root();
        press_into(&mut app, "r", keyboard::Modifiers::empty());
        assert_ne!(app.caged.root(), root, "r left the root alone");
        assert_eq!(app.focused, FocusTarget::Back);

        assert_eq!(app.notation, Notation::Notes);
        press_into(&mut app, "i", keyboard::Modifiers::empty());
        assert_eq!(app.notation, Notation::Intervals);
        assert_eq!(app.focused, FocusTarget::Back);
    }

    #[test]
    fn the_caged_window_claims_neither_a_direction_nor_a_pool() {
        let mut app = app_with_seed(0xca6);
        app.open(Screen::Caged);

        let keys: Vec<char> = accelerators_for(
            &Screen::Caged,
            app.chord_library.notation(),
            &WalkTheScale::new(),
        )
        .into_iter()
        .map(|(key, ..)| key)
        .collect();

        assert_eq!(keys, ['r', 'i']);
        for inert in ["d", "a"] {
            let before = (app.caged.root(), app.caged.selected(), app.notation);
            press_into(&mut app, inert, keyboard::Modifiers::empty());
            assert_eq!(
                (app.caged.root(), app.caged.selected(), app.notation),
                before,
                "{inert} did something here"
            );
        }
    }

    /// Shift+h delivers the capital, not the lowercase letter with a flag set.
    #[test]
    fn capital_letters_are_unbound() {
        for screen in every_screen() {
            let mut app = app_with_seed(0xca9);
            app.navigate_to(screen);

            let before = snapshot(&app);
            press_into(&mut app, "H", keyboard::Modifiers::SHIFT);
            press_into(&mut app, "R", keyboard::Modifiers::SHIFT);
            press_into(&mut app, "I", keyboard::Modifiers::SHIFT);

            assert_eq!(snapshot(&app), before, "{:?}", app.screen);
        }
    }

    #[test]
    fn unbound_letters_change_nothing() {
        for screen in every_screen() {
            let mut app = app_with_seed(0x0b0);
            app.navigate_to(screen);

            let before = snapshot(&app);
            press_into(&mut app, "x", keyboard::Modifiers::empty());

            assert_eq!(snapshot(&app), before, "{:?}", app.screen);
        }
    }

    /// The one character key admitted while a command modifier is held.
    #[test]
    fn ctrl_k_is_the_only_hole_in_the_command_modifiers() {
        assert!(matches!(
            press("k", keyboard::Modifiers::CTRL),
            Some(Message::OpenSearch)
        ));

        // Neither neighbouring letter, and not `k` under the other two modifiers.
        assert!(press("j", keyboard::Modifiers::CTRL).is_none());
        assert!(press("l", keyboard::Modifiers::CTRL).is_none());
        assert!(press("k", keyboard::Modifiers::ALT).is_none());
        assert!(press("k", keyboard::Modifiers::LOGO).is_none());
    }

    #[test]
    fn r_rerolls_the_scale_from_the_scale_trainer() {
        let mut app = app_with_seed(0x5eed);
        app.navigate_to(Screen::ScaleTrainer);

        for _ in 0..50 {
            let before = (app.scale.root(), app.scale.kind());
            press_into(&mut app, "r", keyboard::Modifiers::empty());
            assert_ne!((app.scale.root(), app.scale.kind()), before);
        }
    }

    #[test]
    fn r_is_inert_on_screens_that_do_not_claim_it() {
        for screen in every_screen() {
            if screen == Screen::ScaleTrainer {
                continue;
            }

            let mut app = app_with_seed(0xdead);
            app.navigate_to(screen);

            let before = snapshot(&app);
            press_into(&mut app, "r", keyboard::Modifiers::empty());

            assert_eq!(snapshot(&app), before, "{:?} claimed r", app.screen);
        }
    }

    #[test]
    fn i_toggles_the_notation_and_leaves_the_ring_alone() {
        let mut app = app_with_seed(0x1a7e);
        app.navigate_to(Screen::ScaleTrainer);
        app.focused = FocusTarget::ScaleKind(2);

        press_into(&mut app, "i", keyboard::Modifiers::empty());
        assert_eq!(app.notation, Notation::Intervals);
        assert_eq!(
            app.focused,
            FocusTarget::ScaleKind(2),
            "the accelerator walked the ring onto the button"
        );

        press_into(&mut app, "i", keyboard::Modifiers::empty());
        assert_eq!(app.notation, Notation::Notes, "not a one-way switch");
    }

    #[test]
    fn i_is_inert_on_screens_that_do_not_claim_it() {
        for screen in every_screen() {
            // read from the table, so a screen that takes up `i` exempts itself
            if accelerators_for(&screen, Notation::Notes, &WalkTheScale::new())
                .iter()
                .any(|&(key, ..)| key == 'i')
            {
                continue;
            }

            let mut app = app_with_seed(0xfeed);
            app.navigate_to(screen);

            let before = snapshot(&app);
            press_into(&mut app, "i", keyboard::Modifiers::empty());

            assert_eq!(snapshot(&app), before, "{:?} claimed i", app.screen);
        }
    }

    /// The scale drill's brackets, read from the table like `i` above.
    #[test]
    fn the_brackets_are_inert_on_screens_that_do_not_claim_them() {
        for screen in every_screen() {
            for key in ["[", "]"] {
                if accelerators_for(&screen, Notation::Notes, &WalkTheScale::new())
                    .iter()
                    .any(|(bound, ..)| bound.to_string() == key)
                {
                    continue;
                }

                let mut app = app_with_seed(0xbeef);
                app.navigate_to(screen.clone());

                let before = snapshot(&app);
                press_into(&mut app, key, keyboard::Modifiers::empty());

                assert_eq!(snapshot(&app), before, "{:?} claimed {key}", app.screen);
            }
        }
    }

    /// The mode is a preference, so nothing that replaces the scale may reset it.
    #[test]
    fn the_notation_survives_everything_that_replaces_the_scale() {
        let mut app = app_with_seed(0x5a1e);
        app.open(Screen::ScaleTrainer);

        let _ = app.update(Message::ToggleNotation);
        assert_eq!(app.notation, Notation::Intervals);

        let _ = app.update(Message::RerollScale);
        assert_eq!(app.notation, Notation::Intervals, "a reroll reset it");

        let _ = app.update(Message::SelectRoot(PitchClass::new(3)));
        let _ = app.update(Message::SelectScaleKind(ScaleKind::Aeolian));
        assert_eq!(app.notation, Notation::Intervals, "a selection reset it");

        let _ = app.update(Message::GoBack);
        let _ = app.update(Message::Navigate(Screen::ScaleTrainer));
        assert_eq!(
            app.notation,
            Notation::Intervals,
            "leaving and coming back reset it"
        );
    }

    /// Equal scales against an identically seeded app mean `r` never touched the generator.
    #[test]
    fn r_outside_the_scale_trainer_does_not_even_advance_the_rng() {
        let mut pressed = app_with_seed(0x1de17);
        let mut untouched = app_with_seed(0x1de17);

        for _ in 0..10 {
            press_into(&mut pressed, "r", keyboard::Modifiers::empty());
        }

        pressed.open(Screen::ScaleTrainer);
        untouched.open(Screen::ScaleTrainer);

        assert_eq!(
            (pressed.scale.root(), pressed.scale.kind()),
            (untouched.scale.root(), untouched.scale.kind()),
            "r on Home disturbed the scale stream"
        );
    }

    /// An accelerator that stays on the screen must leave focus where the user left it.
    #[test]
    fn an_in_screen_accelerator_leaves_focus_alone() {
        let mut app = app_with_seed(0xf0c05);
        app.navigate_to(Screen::ScaleTrainer);
        app.focused = FocusTarget::ScaleKind(2);

        press_into(&mut app, "r", keyboard::Modifiers::empty());

        assert_eq!(app.focused, FocusTarget::ScaleKind(2));
    }

    #[test]
    fn the_chord_library_is_reachable_from_the_menu() {
        let mut app = app_with_seed(0xc40d);
        // the first entry of the third category
        press_into(&mut app, "3", keyboard::Modifiers::empty());
        press_into(&mut app, "1", keyboard::Modifiers::empty());

        assert_eq!(app.screen, Screen::ChordLibrary);
    }

    #[test]
    fn the_chord_library_reaches_every_widget_exactly_once() {
        let targets = app_on(Screen::ChordLibrary).focusables();

        assert_eq!(
            targets,
            vec![
                FocusTarget::Back,
                FocusTarget::SearchBox,
                FocusTarget::NotationChoice(0),
                FocusTarget::NotationChoice(1),
                FocusTarget::NotationChoice(2),
                FocusTarget::ChordList,
            ]
        );
    }

    #[test]
    fn slash_and_ctrl_k_both_open_the_search() {
        for open in ["/", "k"] {
            let mut app = app_on(Screen::ChordLibrary);
            let modifiers = if open == "k" {
                keyboard::Modifiers::CTRL
            } else {
                keyboard::Modifiers::empty()
            };

            press_into(&mut app, open, modifiers);

            assert!(
                app.chord_library.search_focused(),
                "{open:?} did not open the box"
            );
            assert_eq!(app.chord_library.query(), "", "{open:?} typed itself");
        }
    }

    #[test]
    fn the_search_keys_are_inert_on_the_other_screens() {
        for screen in every_screen() {
            if screen == Screen::ChordLibrary {
                continue;
            }

            let mut app = app_with_seed(0x5ea4);
            app.navigate_to(screen);

            let before = snapshot(&app);
            press_into(&mut app, "/", keyboard::Modifiers::empty());
            press_into(&mut app, "k", keyboard::Modifiers::CTRL);

            assert_eq!(
                snapshot(&app),
                before,
                "{:?} claimed a search key",
                app.screen
            );
            assert!(!app.chord_library.search_focused());
        }
    }

    #[test]
    fn a_focused_box_swallows_the_motions_and_the_help_key() {
        let mut app = app_on(Screen::ChordLibrary);
        press_into(&mut app, "/", keyboard::Modifiers::empty());

        let ring = app.focused;
        for c in ["c", "m", "j", "?"] {
            press_into(&mut app, c, keyboard::Modifiers::empty());
        }

        assert_eq!(app.chord_library.query(), "cmj?");
        assert_eq!(app.focused, ring, "a motion moved the ring");
        assert!(!app.help_open, "the help key opened the overlay");
    }

    #[test]
    fn a_focused_box_swallows_the_screen_accelerators() {
        let mut app = app_on(Screen::ChordLibrary);
        press_into(&mut app, "/", keyboard::Modifiers::empty());

        let notation = app.notation;
        press_into(&mut app, "i", keyboard::Modifiers::empty());

        assert_eq!(app.chord_library.query(), "i");
        assert_eq!(app.notation, notation, "the accelerator fired while typing");
    }

    #[test]
    fn escape_leaves_the_box_before_it_leaves_the_screen() {
        // `navigate_to` rather than `app_on`, so `GoBack` has a history to pop
        let mut app = app_with_seed(0xe5c);
        app.navigate_to(Screen::ChordLibrary);
        press_into(&mut app, "/", keyboard::Modifiers::empty());
        press_into(&mut app, "c", keyboard::Modifiers::empty());

        press_named(&mut app, Named::Escape);
        assert!(!app.chord_library.search_focused());
        assert_eq!(app.chord_library.query(), "c", "escape cleared the query");
        assert_eq!(app.screen, Screen::ChordLibrary, "escape went back too");

        // Once the box is out of the way, escape means what it means everywhere.
        press_named(&mut app, Named::Escape);
        assert_eq!(app.screen, Screen::Home);
    }

    #[test]
    fn the_keys_get_their_meanings_back_after_the_box() {
        let mut app = app_on(Screen::ChordLibrary);
        press_into(&mut app, "/", keyboard::Modifiers::empty());
        press_named(&mut app, Named::Escape);

        press_into(&mut app, "i", keyboard::Modifiers::empty());
        assert_ne!(
            app.chord_library.notation(),
            Notation::Fingers,
            "i did not fire again"
        );

        press_into(&mut app, "?", keyboard::Modifiers::empty());
        assert!(app.help_open, "? did not open the overlay again");
    }

    #[test]
    fn enter_moves_the_ring_from_the_box_to_the_shapes() {
        // the arrows walk the voicings the moment the box lets go
        let mut app = app_with_seed(0x3117);
        app.open(Screen::ChordLibrary);

        press_into(&mut app, "e", keyboard::Modifiers::empty());
        press_named(&mut app, Named::Enter);

        assert!(!app.chord_library.search_focused());
        assert_eq!(app.focused, FocusTarget::ChordList);

        press_into(&mut app, "l", keyboard::Modifiers::empty());
        assert_eq!(
            app.chord_library.selected_voicing(),
            1,
            "the arrows did not walk"
        );
    }

    #[test]
    fn escape_leaves_the_ring_where_it_was() {
        // Escape is backing out rather than finishing, so it does not hand the keyboard on.
        let mut app = app_with_seed(0xe5c2);
        app.open(Screen::ChordLibrary);

        press_named(&mut app, Named::Escape);

        assert!(!app.chord_library.search_focused());
        assert_eq!(app.focused, FocusTarget::SearchBox);
    }

    #[test]
    fn g_and_shift_g_jump_to_the_ends_of_the_list() {
        let mut app = app_with_seed(0x99);
        app.open(Screen::ChordLibrary);
        press_named(&mut app, Named::Escape);

        let last = app.chord_library.rows().len() - 1;

        press_into(&mut app, "G", keyboard::Modifiers::SHIFT);
        assert_eq!(app.chord_library.selected_row(), last);

        press_into(&mut app, "g", keyboard::Modifiers::empty());
        press_into(&mut app, "g", keyboard::Modifiers::empty());
        assert_eq!(app.chord_library.selected_row(), 0);
    }

    #[test]
    fn one_g_is_not_a_jump_and_does_not_lie_in_wait() {
        let mut app = app_with_seed(0x9a);
        app.open(Screen::ChordLibrary);
        press_named(&mut app, Named::Escape);

        let last = app.chord_library.rows().len() - 1;
        press_into(&mut app, "G", keyboard::Modifiers::SHIFT);

        // One `g` on its own moves nothing.
        press_into(&mut app, "g", keyboard::Modifiers::empty());
        assert_eq!(app.chord_library.selected_row(), last);

        // and anything between the two halves cancels it
        press_into(&mut app, "i", keyboard::Modifiers::empty());
        press_into(&mut app, "g", keyboard::Modifiers::empty());
        assert_eq!(app.chord_library.selected_row(), last, "a stale g fired");
    }

    #[test]
    fn the_jump_keys_type_while_the_box_has_focus() {
        // `g` is a note name, so a focused box has to take it as text.
        let mut app = app_with_seed(0x9b);
        app.open(Screen::ChordLibrary);

        press_into(&mut app, "g", keyboard::Modifiers::empty());
        press_into(&mut app, "g", keyboard::Modifiers::empty());

        assert_eq!(app.chord_library.query(), "gg");
        assert_eq!(app.chord_library.selected_row(), 0);
    }

    #[test]
    fn the_list_claims_the_motions_and_walks_two_axes() {
        let mut app = app_on(Screen::ChordLibrary);
        app.focused = FocusTarget::ChordList;

        press_into(&mut app, "j", keyboard::Modifiers::empty());
        assert_eq!(app.chord_library.selected_row(), 1);
        assert_eq!(app.focused, FocusTarget::ChordList, "the ring moved off");

        press_into(&mut app, "l", keyboard::Modifiers::empty());
        assert_eq!(app.chord_library.selected_voicing(), 1);

        press_into(&mut app, "h", keyboard::Modifiers::empty());
        assert_eq!(app.chord_library.selected_voicing(), 0);
    }

    #[test]
    fn a_notation_button_picks_that_notation_rather_than_the_next() {
        // pressing the second means the second, wherever the setting is
        let mut app = app_on(Screen::ChordLibrary);

        for (index, expected) in [
            (2, Notation::Fingers),
            (0, Notation::Notes),
            (1, Notation::Intervals),
            (1, Notation::Intervals),
        ] {
            let _ = app.update(Message::SetNotation(index));

            assert_eq!(app.chord_library.notation(), expected, "button {index}");
        }
    }

    #[test]
    fn the_library_opens_on_fingers_and_leaves_the_scale_trainer_alone() {
        // Fingers here; the Scale Trainer keeps its own setting
        let mut app = app_with_seed(0xf14);
        app.open(Screen::ChordLibrary);

        assert_eq!(app.chord_library.notation(), Notation::Fingers);
        assert_eq!(app.notation, Notation::Notes, "the scale trainer moved");

        // out of the box first: `d` is a note name, so a focused box would type it
        press_named(&mut app, Named::Escape);
        press_into(&mut app, "d", keyboard::Modifiers::empty());

        assert_eq!(app.chord_library.notation(), Notation::Intervals);
        assert_eq!(app.notation, Notation::Notes, "the scale trainer moved");
    }

    #[test]
    fn the_mode_keys_type_while_the_box_has_focus() {
        // `n`, `d` and `f` are note names, so a focused search box takes them as text
        let mut app = app_with_seed(0x4b0);
        app.open(Screen::ChordLibrary);

        press_into(&mut app, "d", keyboard::Modifiers::empty());

        assert_eq!(app.chord_library.query(), "d");
        assert_eq!(
            app.chord_library.notation(),
            Notation::Fingers,
            "d switched"
        );
    }

    #[test]
    fn n_d_and_f_each_pick_their_own_mode() {
        let mut app = app_on(Screen::ChordLibrary);

        for (key, expected) in [
            ("n", Notation::Notes),
            ("d", Notation::Intervals),
            ("f", Notation::Fingers),
            ("n", Notation::Notes),
        ] {
            press_into(&mut app, key, keyboard::Modifiers::empty());

            assert_eq!(app.chord_library.notation(), expected, "{key}");
        }
    }

    #[test]
    fn i_still_cycles_now_that_the_buttons_are_separate() {
        // `i` names whichever button comes next, so its target moves with the setting
        let mut app = app_on(Screen::ChordLibrary);
        app.chord_library.set_notation(0);

        for expected in [Notation::Intervals, Notation::Fingers, Notation::Notes] {
            press_into(&mut app, "i", keyboard::Modifiers::empty());

            assert_eq!(app.chord_library.notation(), expected);
        }
    }

    #[test]
    fn the_library_cycles_three_notations_and_the_scale_trainer_two() {
        let mut app = app_on(Screen::ChordLibrary);
        app.chord_library.set_notation(0);

        for expected in [
            Notation::Intervals,
            Notation::Fingers,
            Notation::Notes,
            Notation::Intervals,
        ] {
            press_into(&mut app, "i", keyboard::Modifiers::empty());
            assert_eq!(app.chord_library.notation(), expected);
        }

        let mut app = app_on(Screen::ScaleTrainer);
        for expected in [Notation::Intervals, Notation::Notes, Notation::Intervals] {
            press_into(&mut app, "i", keyboard::Modifiers::empty());
            assert_eq!(app.notation, expected, "the scale trainer reached a third");
        }
    }

    /// When the action navigates, focus moves because navigating moves it.
    #[test]
    fn a_navigating_accelerator_resets_focus_on_the_new_screen() {
        let entries = Category::ALL.into_iter().flat_map(|category| {
            let last = category.entries().len() - 1;
            (category.entries().iter().enumerate())
                .map(move |(index, item)| (category, last, index, item))
        });

        for (category, last, index, item) in entries {
            let key = char::from_digit(index as u32 + 1, 10).expect("a category fits in digits");

            let mut app = app_with_seed(0x6070);
            app.focused = FocusTarget::HomeEntry(category, last);

            press_into(&mut app, &key.to_string(), keyboard::Modifiers::empty());

            assert_eq!(app.screen, item.screen, "{key} opened the wrong screen");

            // the first focusable, except the library, which opens in its search box
            let landed = match item.screen {
                Screen::ChordLibrary => FocusTarget::SearchBox,
                _ => app_on(item.screen.clone()).focusables()[0],
            };

            assert_eq!(app.focused, landed, "{key} did not reset focus");
        }
    }

    /// Shift is how `?` is typed, so it must not be filtered like Cmd, Ctrl and Alt.
    #[test]
    fn the_help_key_survives_its_own_shift() {
        assert!(matches!(
            press("?", keyboard::Modifiers::SHIFT),
            Some(Message::ToggleHelp)
        ));
    }

    #[test]
    fn help_opens_on_every_screen() {
        for screen in every_screen() {
            let mut app = app_with_seed(0x8e19);
            app.navigate_to(screen);

            press_into(&mut app, "?", keyboard::Modifiers::SHIFT);

            assert!(app.help_open, "{:?} would not open help", app.screen);
        }
    }

    #[test]
    fn any_key_dismisses_help_and_does_nothing_else() {
        for key in ["?", "x", "j", "r", "1"] {
            let mut app = app_with_seed(0xd1551);
            app.navigate_to(Screen::ScaleTrainer);
            app.focused = FocusTarget::ScaleKind(1);
            app.help_open = true;

            let before = snapshot(&app);
            press_into(&mut app, key, keyboard::Modifiers::SHIFT);

            assert!(!app.help_open, "{key} left help open");
            assert_eq!(snapshot(&app), before, "{key} did more than dismiss");
        }
    }

    #[test]
    fn back_navigation_is_consumed_by_an_open_overlay() {
        for named in [Named::Escape, Named::Backspace] {
            let mut app = app_with_seed(0xbac4);
            app.navigate_to(Screen::ScaleTrainer);
            app.help_open = true;

            let message = translate_key(keyboard::Key::Named(named), keyboard::Modifiers::empty())
                .expect("named key is bound");
            let _ = app.update(message);

            assert!(!app.help_open, "{named:?} left help open");
            assert_eq!(app.screen, Screen::ScaleTrainer, "{named:?} navigated away");
        }
    }

    /// The overlay contributes nothing focusable, so it must leave the focus ring where it was.
    #[test]
    fn help_preserves_focus_and_adds_no_targets() {
        let mut app = app_with_seed(0x0fc5);
        app.navigate_to(Screen::ScaleTrainer);
        app.focused = FocusTarget::Root(4);

        let reachable = app_on(Screen::ScaleTrainer).focusables();

        press_into(&mut app, "?", keyboard::Modifiers::SHIFT);
        assert_eq!(app.focusables(), reachable, "help added focus targets");

        press_into(&mut app, "?", keyboard::Modifiers::SHIFT);

        assert!(!app.help_open);
        assert_eq!(app.focused, FocusTarget::Root(4));
    }

    /// Presses a named key, for the keys `press` cannot build because they produce no character.
    pub(super) fn press_named(app: &mut App, named: Named) {
        let _ = app.update(Message::Key(
            keyboard::Key::Named(named),
            keyboard::Modifiers::empty(),
        ));
    }

    #[test]
    fn escape_and_backspace_both_go_back() {
        for named in [Named::Escape, Named::Backspace] {
            let mut app = app_with_seed(0xbac1);
            app.navigate_to(Screen::ScaleTrainer);

            press_named(&mut app, named);

            assert_eq!(app.screen, Screen::Home, "{named:?} did not go back");
        }
    }

    #[test]
    fn going_back_from_the_root_screen_does_nothing() {
        for named in [Named::Escape, Named::Backspace] {
            let mut app = app_with_seed(0x1007);
            let before = snapshot(&app);

            press_named(&mut app, named);

            assert_eq!(
                snapshot(&app),
                before,
                "{named:?} disturbed the root screen"
            );
        }
    }

    #[test]
    fn enter_and_space_activate_the_focused_widget() {
        for named in [Named::Enter, Named::Space] {
            let mut app = app_with_seed(0xac71);
            app.focused = FocusTarget::HomeEntry(Category::Scales, 1);

            press_named(&mut app, named);

            assert_eq!(
                app.screen,
                Screen::WalkTheScale,
                "{named:?} did not activate the focused menu item"
            );
        }
    }

    /// Key translation reads no application state, so a press means the same on every screen.
    #[test]
    fn translation_does_not_vary_by_state() {
        let pressed = press("r", keyboard::Modifiers::empty());

        assert!(matches!(pressed, Some(Message::Accelerate('r'))));

        for screen in every_screen() {
            let mut app = app_with_seed(0x57a7e);
            app.navigate_to(screen);

            assert_eq!(
                format!("{:?}", press("r", keyboard::Modifiers::empty())),
                format!("{pressed:?}"),
                "translation changed on {:?}",
                app.screen
            );
        }
    }

    #[test]
    fn interval_token_renders_blues_with_the_smufl_flat_and_naturals_bare() {
        let tokens: Vec<(Option<char>, u8)> = ScaleKind::Blues
            .intervals()
            .iter()
            .map(|&interval| interval_token(interval))
            .collect();

        // 1 ♭3 4 ♭5 5 ♭7
        assert_eq!(
            tokens,
            vec![
                (None, 1),
                (Some(SMUFL_FLAT), 3),
                (None, 4),
                (Some(SMUFL_FLAT), 5),
                (None, 5),
                (Some(SMUFL_FLAT), 7),
            ]
        );
    }

    #[test]
    fn selector_card_fits_every_kind_row() {
        // iced's default line height, and the card's `.spacing(12)` and `.padding(32)`
        const TEXT_LINE_HEIGHT: f32 = 1.3;
        const COLUMN_SPACING: f32 = 12.0;
        const CARD_PADDING: f32 = 32.0;
        // a pill is its text, the button's `.padding([8, 12])` and the 3px `focus_ring`
        const BUTTON_PADDING_Y: f32 = 8.0;
        const FOCUS_RING_PADDING: f32 = 3.0;

        let rows = KIND_ROW_WIDTHS.len() as f32;
        let pill =
            KIND_BUTTON_SIZE * TEXT_LINE_HEIGHT + 2.0 * BUTTON_PADDING_Y + 2.0 * FOCUS_RING_PADDING;
        let needed = rows * pill + (rows - 1.0) * COLUMN_SPACING + 2.0 * CARD_PADDING;

        assert!(
            SELECTOR_CARD_HEIGHT >= needed,
            "the selector card is {SELECTOR_CARD_HEIGHT} tall but needs {needed} for \
             {rows} rows of scale kinds; the last row would lay out at zero height and \
             stop drawing, taking its kinds off the screen with it."
        );
    }

    #[test]
    fn summary_card_fits_a_two_line_name() {
        // iced's `LineHeight::default()`, `Relative(1.3)`, which iced resolves internally
        const TEXT_LINE_HEIGHT: f32 = 1.3;
        // The card's own `.spacing(10)` and `.padding(32)`, both in `ui_scale_trainer`.
        const COLUMN_SPACING: f32 = 10.0;
        const CARD_PADDING: f32 = 32.0;
        // several names are wider than the card at `SUMMARY_NAME_SIZE`, so budget two lines
        const NAME_LINES: f32 = 2.0;

        let line = |size: f32| size * TEXT_LINE_HEIGHT;
        // the header row is as tall as the root label
        let needed = line(SUMMARY_ROOT_SIZE)
            + COLUMN_SPACING
            + NAME_LINES * line(SUMMARY_NAME_SIZE)
            + COLUMN_SPACING
            + line(SUMMARY_FORMULA_SIZE)
            + 2.0 * CARD_PADDING;

        assert!(
            SUMMARY_CARD_HEIGHT >= needed,
            "the summary card is {SUMMARY_CARD_HEIGHT} tall but needs {needed} for a \
             two-line scale name above its formula. iced resolves a `Shrink` child against \
             the space that is left, so the formula row would lay out at zero height and \
             stop drawing entirely — silently, with no clipping and no panic."
        );
    }
}

#[cfg(test)]
mod help_overlay_tests {
    use super::*;

    #[test]
    fn a_timer_tick_does_not_dismiss_the_help_overlay() {
        // the Tuner's poll fires every 10 ms, which would close the overlay a frame later
        let (mut app, _) = App::new();
        let _ = app.update(Message::Navigate(Screen::Tuner));
        let _ = app.update(Message::ToggleHelp);
        assert!(app.help_open);

        for _ in 0..5 {
            let _ = app.update(Message::AudioPoll);
            assert!(app.help_open, "a poll tick closed the help overlay");
        }

        // the trainers' flash timers, kept here so all three are tested together
        for tick in [Message::AdvancePrompt, Message::AdvanceIntervalPrompt] {
            let _ = app.update(tick);
            assert!(app.help_open, "a flash tick closed the help overlay");
        }

        // A key still closes it, which is the behaviour the guard exists to keep.
        let _ = app.update(Message::Key(
            keyboard::Key::Character("x".into()),
            keyboard::Modifiers::default(),
        ));
        assert!(
            !app.help_open,
            "a key press no longer dismisses the overlay"
        );
    }
}
