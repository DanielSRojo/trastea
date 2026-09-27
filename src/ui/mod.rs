mod caged;
mod chord_diagram;
mod chord_library;
mod drill;
mod focus;
mod fretboard;
mod interval_trainer;
mod keys;
mod listener;
mod neck;
mod note_trainer;
mod play_the_interval;
mod scale_trainer;
mod shapes;
mod style;
mod tuner;
mod walk_the_scale;
use std::time::Duration;

use caged::{Caged, ui_caged};
use interval_trainer::{IntervalTrainer, ui_interval_trainer};
use listener::Listener;
use note_trainer::{NoteTrainer, ui_note_trainer};
use play_the_interval::{PlayTheInterval, ui_play_the_interval};
use scale_trainer::ui_scale_trainer;
use tuner::{Tuner, ui_tuner};
use walk_the_scale::{WalkTheScale, ui_walk_the_scale};

use chord_library::{ChordLibrary, KeyOutcome, ui_chord_library};

use iced::{Color, Element, Pixels, Subscription, Task, keyboard};

#[cfg(test)]
use crate::audio::detect::Heard;
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
///
/// Long enough to register as an answer being *marked* rather than as the screen
/// flickering, and short enough that a learner on a run never waits on it.
const ANSWER_FLASH: Duration = Duration::from_secs(1);
/// Tall enough for a scale kind whose name wraps to two lines — `Minor Pentatonic` and
/// friends — *plus* the interval formula under it. iced lays a `Shrink` child out against
/// the space that is left, so a column that runs out gives its last child a height of zero
/// and the formula silently stops drawing rather than overflowing. The fit test below
/// derives the budget this has to clear.
const SUMMARY_CARD_HEIGHT: f32 = 280.0;
/// The three text sizes the summary card has to stack inside `SUMMARY_CARD_HEIGHT`. Named
/// rather than inlined so `summary_card_fits_a_two_line_name` budgets against the same
/// numbers the card draws with — a copy of them would drift silently, which is the whole
/// failure that test exists to catch.
const SUMMARY_ROOT_SIZE: f32 = 56.0;
const SUMMARY_NAME_SIZE: f32 = 34.0;
const SUMMARY_FORMULA_SIZE: f32 = 24.0;
const ROOT_SELECTOR_CARD_WIDTH: f32 = 320.0;
/// Tall enough for `KIND_ROW_WIDTHS`'s six rows; the root selector shares the constant and
/// has room to spare. Sized by the same rule as `SUMMARY_CARD_HEIGHT`, and for the same
/// reason — a fixed height a column outgrows costs the last row entirely, not visibly.
const SELECTOR_CARD_HEIGHT: f32 = 400.0;
const ROOT_BUTTON_SIZE: f32 = 50.0;
/// The scale-kind pills' text size. Named for the reason the summary card's sizes are:
/// `selector_card_fits_every_kind_row` budgets the card's height against it.
const KIND_BUTTON_SIZE: f32 = 16.0;

pub struct App {
    screen: Screen,
    history: Vec<Screen>,
    scale: Scale,
    focused: FocusTarget,
    /// Owned rather than reached for globally, so every draw is a state change the
    /// borrow checker can see. Note this makes `App` un-`Default`-able on purpose:
    /// a `Default` seed would have to be a constant, and an app that replays the
    /// same scales every launch is worse than no `Default` at all.
    rng: Rng,
    /// Whether the help overlay is up.
    ///
    /// A flag rather than a `Screen`, so the screen underneath stays the active one. The
    /// overlay lists *its* keys and reads them from `self.screen` directly, instead of
    /// recovering them from the top of the navigation history.
    help_open: bool,
    /// What the fretboard's markers say. Lives here rather than on `Scale` — see
    /// `Notation`.
    notation: Notation,
    /// The Note Trainer's drill. Its own struct rather than seven more fields here, so its
    /// rules stay testable without an `App`.
    note_trainer: NoteTrainer,
    /// The Interval Trainer's drill, on the same terms and for the same reasons.
    interval_trainer: IntervalTrainer,
    /// The played drill: its prompt, its streak, and the device it listens to.
    play_the_interval: PlayTheInterval,
    /// The Chord Library's query and selections. Its own struct for the reason the two
    /// drills have one: the fields are read all over its views and nowhere else.
    chord_library: ChordLibrary,
    /// The CAGED window's root and selection, on the same terms.
    caged: Caged,
    /// The scale drill: its own scale, which element of the sequence it is asking for, how
    /// it is answered, and the device it listens to when that is by playing.
    walk_the_scale: WalkTheScale,
    /// The Tuner's device, detector state and reading. Holds the open audio stream, which
    /// is why it lives on `App` rather than inside a subscription — iced hashes a
    /// subscription recipe rather than its captured values, so a stream owned there would
    /// be dropped and rebuilt whenever that hash changed. See `Message::Key`, which
    /// documents the same hazard in its milder form.
    tuner: Tuner,
}

/// What the fretboard's markers are labelled with: the notes' names, or the degrees
/// they occupy in the scale.
///
/// A UI type, unlike `Spelling`, which sits on `Scale`. Spelling earns its place
/// there because it changes what the notes *are called* — `Scale::notes` and
/// `Scale::spell` both read it, and a name is music rather than chrome. This
/// changes nothing about the scale; it picks which of two true things about an
/// already-determined scale gets drawn, so `music/` has no business knowing about
/// it.
///
/// Keeping it off `Scale` also keeps it from being replaced along with one: the mode
/// has to survive a reroll, and it would not if it travelled with the scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Notation {
    Notes,
    Intervals,
    /// Which finger stops a string. Only a chord diagram has one to show, which is why
    /// `cycle_for` exists rather than a plain three-way rotation.
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
    /// The drill that walks a scale by degree. Its own screen rather than a mode of the
    /// Scale Trainer, for the reason `PlayTheInterval` is not a mode of the Interval
    /// Trainer: a reference and a drill are two things to look at, and one screen holding
    /// both had two jobs and one keyboard.
    WalkTheScale,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Navigate(Screen),
    GoBack,
    SelectRoot(PitchClass),
    SelectScaleKind(ScaleKind),
    ToggleNotation,
    RerollScale,
    /// Answers a *Name it* prompt with a note name.
    AnswerNote(PitchClass),
    /// Answers a *Find it* prompt with a position — and what a press on the neck sends.
    ///
    /// The variant's constructor is passed to `Fretboard::on_press` as a plain
    /// `fn(usize, usize) -> Message`, which is why it takes two loose `usize`s rather than
    /// a tuple or an `Answer`.
    ChooseNotePosition(usize, usize),
    SkipPrompt,
    /// The end of the flash a correct answer leaves on screen: retires the answered prompt
    /// and draws the next one. Sent by the timer in `App::subscription`, never by a widget.
    AdvancePrompt,
    ToggleDrillDirection,
    TogglePool,
    /// The Note Trainer's spelling, not the scale's — see `NoteTrainer::spelling`.
    ToggleNoteSpelling,
    /// The Tuner's, which names what it hears and moves no other screen.
    ToggleTunerSpelling,
    /// Answers a *Name it* prompt on the Interval Trainer with an interval.
    AnswerInterval(Interval),
    /// Answers the Interval Trainer's *Find it* prompt with a position — and what a press
    /// on that screen's neck sends.
    ///
    /// Two loose `usize`s for the reason `ChooseNotePosition` takes them: the bare variant
    /// constructor is handed to `Fretboard::on_press` as a `fn(usize, usize) -> Message`.
    ChooseIntervalPosition(usize, usize),
    SkipIntervalPrompt,
    ToggleIntervalDirection,
    /// The end of the Interval Trainer's answer flash. Its own variant rather than a shared
    /// `AdvancePrompt` routed by screen, so a tick cannot retire the other trainer's prompt.
    AdvanceIntervalPrompt,
    /// Selects one chord in the library's list, by its index among the rows on show.
    SelectChordRow(usize),
    /// Selects one of the selected chord's voicings. A press anywhere inside a diagram
    /// sends this, so selection does not depend on which position was pressed.
    SelectVoicing(usize),
    /// Picks a notation outright, by its index in the screen's `Notation::cycle_for`.
    ///
    /// Distinct from `ToggleNotation`, which advances: the library shows all three options
    /// at once, and a press on one of them means that one rather than the next.
    SetNotation(usize),
    /// Picks the root the CAGED window shows.
    SelectCagedRoot(PitchClass),
    /// Picks one shape of the CAGED cycle, by its place in the derived order.
    SelectCagedShape(usize),
    /// A different root for the CAGED window. Its own variant rather than `RerollScale`,
    /// which would replace the Scale Trainer's scale from another screen.
    RerollCagedRoot,
    /// One tick of the audio poll: take whatever has arrived, analyse it, and fold the
    /// result into the screen listening. Sent by the timer in `App::subscription` while a
    /// listening screen is showing and a device is open, and never by a widget.
    ///
    /// Named for what it carries rather than for the screen that wanted it, because two
    /// screens listen now. `App::listener` is what routes it, and the reason routing by
    /// the screen on show is sound is that only one of them can hold the device.
    AudioPoll,
    /// A released reading, applied directly.
    ///
    /// `cfg(test)`: the running app reaches the same state through `AudioPoll`, which
    /// analyses real audio. This is the seam the screens' tests drive instead, so a screen
    /// assertion needs no device — the same arrangement the trainers use for a neck press.
    #[cfg(test)]
    Heard(Heard),
    /// Opens the input device list.
    OpenDevices,
    /// Chooses a device by its place in the offered list.
    ChooseDevice(usize),
    /// Skips the played drill's prompt. Its own variant rather than `SkipIntervalPrompt`,
    /// which would replace the Interval Trainer's prompt from another screen.
    SkipPlayPrompt,
    /// The played drill's flash timer. Its own variant for the reason the other two have
    /// theirs: a tick left behind on one screen must not retire another screen's prompt.
    AdvancePlayPrompt,
    /// Starts another run after one has ended on its duration.
    StartWalkTheScale,
    /// Picks the kind the drill walks. Its own variant rather than `SelectScaleKind`, which
    /// would move the Scale Trainer's scale from here.
    SelectDrillKind(ScaleKind),
    /// The root display's drag, as the three moments it has. Each carries the cursor's x in
    /// window coordinates rather than in the canvas's, since the gesture outlives the
    /// pointer's stay over the canvas and only differences are read.
    GrabDrillRoot(f32),
    DragDrillRoot(f32),
    DropDrillRoot,
    /// The wheel over the root display, in steps rather than in the `ScrollDelta` it came
    /// from: a line and a pixel are two units for one movement, and the canvas is where they
    /// stop being two.
    ScrollDrillRoot(f32),
    /// `[` and `]`: the root a semitone down or up, from wherever the ring is standing.
    StepDrillRoot(i32),
    /// A different scale to drill. Its own variant rather than `RerollScale`, for the reason
    /// `RerollCagedRoot` is one.
    RerollDrillScale,
    /// Swaps how the drill is answered — a press on the neck, or a note played.
    ToggleScaleAnswering,
    /// Cycles the run's duration: off, three, five, ten minutes.
    CycleScaleDuration,
    /// Hides the neck, so a degree is found from memory. Inert while pressing, where the
    /// neck is the surface the answer is given on.
    ToggleScaleNeck,
    /// Answers the drill's current element with a position — what a press on the Scale
    /// Trainer's neck sends while a run is going.
    ///
    /// Two loose `usize`s for the reason `ChooseNotePosition` takes them: the bare variant
    /// constructor is handed to `Fretboard::on_press` as a `fn(usize, usize) -> Message`.
    ChooseScalePosition(usize, usize),
    /// The end of the drill's completed-sequence flash. Its own variant for the reason the
    /// other three have theirs: a tick left behind on one screen must not retire another's.
    AdvanceScaleSequence,
    /// One second of a timed run, which is what spends its duration.
    WalkTheScaleTick,
    /// A key press, straight from the subscription and not yet meaning anything.
    ///
    /// The subscription used to translate keys inside its own closure, which worked only
    /// while translation read no state. A focused text box makes it stateful, and a closure
    /// capturing that state would be built once and kept: iced hashes a subscription recipe
    /// rather than the values its closure closed over, so the stream would go on running
    /// with the flag it started with. Carrying the raw key to `update` — where `&mut self`
    /// is in hand — removes the question rather than answering it.
    Key(keyboard::Key, keyboard::Modifiers),
    /// A character key that may be an accelerator on the current screen.
    ///
    /// Carries the character rather than an action because `translate_key` has no screen
    /// to look it up against; `update` resolves it, and ignores the ones nothing claims.
    Accelerate(char),
    /// Puts the cursor in a screen's search box. Sent by `/` and by `Ctrl+K`.
    ///
    /// Not an `Accelerate`, because the two keys that send it are not characters a screen
    /// claims: `Ctrl+K` is carved out of `COMMAND_MODIFIERS` by name, and both have to
    /// reach one target rather than whichever focusable a screen happens to list.
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

/// One entry of the Home screen's menu.
struct MenuItem {
    label: &'static str,
    caption: &'static str,
    screen: Screen,
}

/// The Home menu, in the order it is drawn.
///
/// The buttons, the focus grid's item count, and the digit accelerators are all built from
/// this, so a trainer added here gains all three at once and cannot end up with a button
/// but no key, or a key labelled with the wrong name.
/// How often the Tuner asks for audio while it is showing.
const AUDIO_POLL: std::time::Duration = std::time::Duration::from_millis(10);

const HOME_MENU: [MenuItem; 8] = [
    MenuItem {
        label: "Scale Trainer",
        caption: "Explore and learn guitar scales",
        screen: Screen::ScaleTrainer,
    },
    MenuItem {
        label: "Walk the Scale",
        caption: "Find a scale's degrees in order, by ear or by eye",
        screen: Screen::WalkTheScale,
    },
    MenuItem {
        label: "Note Trainer",
        caption: "Build fretboard recall one pitch at a time",
        screen: Screen::NoteTrainer,
    },
    MenuItem {
        label: "Interval Trainer",
        caption: "Recognize distances from a tonal center",
        screen: Screen::IntervalTrainer,
    },
    MenuItem {
        label: "Play the Interval",
        caption: "Play the distance a lit position asks for",
        screen: Screen::PlayTheInterval,
    },
    MenuItem {
        label: "Chord Library",
        caption: "Look up a chord and see where it sits",
        screen: Screen::ChordLibrary,
    },
    MenuItem {
        label: "CAGED",
        caption: "See one chord in all five places on the neck",
        screen: Screen::Caged,
    },
    MenuItem {
        label: "Tuner",
        caption: "Tune by ear against what the app hears",
        screen: Screen::Tuner,
    },
];

const HOME_MENU_ITEMS: usize = HOME_MENU.len();

impl App {
    pub fn new() -> (Self, Task<Message>) {
        // The generator is built before the struct, because the trainer's opening prompt is
        // drawn from it and a struct literal cannot borrow a field it is still building.
        let mut rng = Rng::from_clock();
        let note_trainer = NoteTrainer::new(&mut rng);
        let interval_trainer = IntervalTrainer::new(&mut rng);
        let play_the_interval = PlayTheInterval::new(&mut rng);

        let app = Self {
            screen: Screen::default(),
            history: Vec::new(),
            scale: Scale::new(PitchClass::new(0), ScaleKind::Ionian),
            focused: FocusTarget::HomeMenuItem(0),
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
        };

        (app, Task::none())
    }

    /// Dispatches the message, then keeps the chord list scrolled to its selection.
    ///
    /// Split from `dispatch` rather than folded into every arm that moves the selection:
    /// the selection moves from the arrows, from a click, and from the query changing under
    /// it, and threading a `Task` back out of each would spread one concern across all of
    /// them. Comparing before and after asks the question once, where the answer is known.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        let was = self.chord_library.selected_row();
        let task = self.dispatch(message);

        if self.screen == Screen::ChordLibrary && self.chord_library.selected_row() != was {
            return Task::batch([task, self.chord_library.follow_selection()]);
        }

        task
    }

    fn dispatch(&mut self, message: Message) -> Task<Message> {
        // A raw key is resolved before anything else, because until it is translated it is
        // not yet one of the messages the arms below are written against. `translate_key`
        // never yields another `Key`, so this recurses exactly once.
        if let Message::Key(key, modifiers) = message {
            // The one branch that asks whether the search box has the keyboard, and the
            // reason translation moved here from the subscription. While the box holds
            // focus almost everything is text — `j` types rather than moving, `?` types
            // rather than opening the help — so the key must not reach `translate_key` at
            // all. What the box declines (a command-modified key, say) falls through.
            if self.screen == Screen::ChordLibrary && self.chord_library.search_focused() {
                match self.chord_library.handle_key(&key, modifiers) {
                    KeyOutcome::Ignored => {}
                    KeyOutcome::Handled => return Task::none(),
                    // Enter hands the keyboard to the shapes, which is where a learner who
                    // has just found their chord wants to be: the list claims the arrows,
                    // so left and right walk the voicings straight away.
                    KeyOutcome::Accepted => {
                        self.focused = FocusTarget::ChordList;

                        return Task::none();
                    }
                }
            }

            return match translate_key(key, modifiers) {
                // `dispatch`, not `update`: the wrapper has already taken its "before"
                // reading, and recursing through it would take a second one that could
                // only ever agree with the first.
                Some(message) => self.dispatch(message),
                None => Task::none(),
            };
        }

        // The flash timer is the clock talking, not the user, so it is handled ahead of the
        // overlay: a tick that arrived while the help panel was up would otherwise be
        // spent dismissing a panel the learner is still reading.
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

        // The run's clock, like the flash timers: a tick that arrived while the help panel
        // was up must spend the duration rather than dismiss the panel.
        if matches!(message, Message::WalkTheScaleTick) {
            self.walk_the_scale.tick();
            return Task::none();
        }

        // The audio poll, for the same reason and much more urgently: it ticks every
        // 10 ms, so an overlay that a tick could dismiss was on screen for one frame. The
        // reading also keeps up to date behind the panel rather than going stale and
        // having to re-confirm when it closes.
        if matches!(message, Message::AudioPoll) {
            self.poll_audio();
            return Task::none();
        }

        // The overlay is modal: while it is up, anything at all dismisses it and nothing
        // else happens. Handling it here rather than per-message is what spares `GoBack`
        // from having to mean two things, and stops an accelerator from firing blind
        // behind a panel the user is still reading.
        if self.help_open {
            self.help_open = false;
            return Task::none();
        }

        match message {
            Message::Navigate(screen) => self.open(screen),
            // Esc closes the device list before it leaves the screen, the way it leaves
            // the library's search box without leaving the library.
            Message::GoBack if self.is_picking() => {
                if let Some(listener) = self.listener() {
                    listener.close_picker();
                }
            }
            // And leaves the drill before it leaves the screen, which is what `Esc` does to
            // the library's search box: the run is the modal thing here, and a learner
            // pressing Esc mid-sequence means "stop asking", not "take me Home".
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
            // Every arm below borrows `self.note_trainer` and `self.rng` at once. That is
            // allowed because they are disjoint fields — the same reason the drill's methods
            // take the generator as a parameter instead of reaching for it.
            Message::AnswerNote(pitch_class) => self.note_trainer.answer_name(pitch_class),
            Message::ChooseNotePosition(string, fret) => {
                self.note_trainer.answer_position(string, fret)
            }
            Message::SkipPrompt => self.note_trainer.skip(&mut self.rng),
            // Handled above, before the help overlay gets a say.
            Message::AdvancePrompt => {}
            Message::ToggleDrillDirection => self.note_trainer.toggle_direction(&mut self.rng),
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
            // Handled above too, and earliest of all: by here it has already become one of
            // the other arms, or nothing.
            // Handled above, ahead of the help overlay — see the early return there.
            Message::AudioPoll => {}
            #[cfg(test)]
            Message::Heard(heard) => self.hear(heard),
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
            // Scoped to the one screen with a box, the way an accelerator a screen does
            // not claim is inert there. `Ctrl+K` reaches here from anywhere, so the guard
            // is what keeps it from focusing a box nobody can see.
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
                // While the list is up, Enter means "this one" rather than "fire whatever
                // the ring is on" — the ring is behind a modal and pressing it would
                // reopen the list that is already open.
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

    /// Opens a screen from anywhere — a click, a Tab-and-Enter, or a digit accelerator.
    ///
    /// Entering the scale trainer draws a fresh scale, so it never reopens on the one it
    /// last showed. That rule lives here rather than at each entry point, which is what
    /// keeps the menu button and the accelerator from drifting apart.
    fn open(&mut self, screen: Screen) {
        let wants_fresh_scale = screen == Screen::ScaleTrainer;
        let wants_fresh_prompt = screen == Screen::NoteTrainer;
        let wants_fresh_interval = screen == Screen::IntervalTrainer;
        let wants_the_search_box = screen == Screen::ChordLibrary;
        let wants_the_caged_window = screen == Screen::Caged;
        let wants_fresh_play = screen == Screen::PlayTheInterval;
        let wants_a_fresh_walk = screen == Screen::WalkTheScale;
        let wants_to_listen = matches!(screen, Screen::Tuner | Screen::PlayTheInterval);

        self.navigate_to(screen);

        // The device opens on entering rather than on a press: a tuner that has to be
        // started is a tuner that looks broken for as long as it takes to notice. Failing
        // is a state the screen draws, so a machine with no input still gets a screen.
        if wants_to_listen && let Some(listener) = self.listener() {
            listener.listen();
        }

        if wants_fresh_scale {
            self.reroll_scale();
        }

        // The scale is what it was left on — a learner comes back to what they are working
        // on, which is the CAGED window's rule rather than the Scale Trainer's — while the
        // run itself is fresh and the best streak persists.
        if wants_a_fresh_walk {
            self.walk_the_scale.enter();
        }

        // Same rule as the scale trainer's: the screen never reopens on what it last
        // showed. Settings and the best streak persist; the run does not.
        if wants_fresh_prompt {
            self.note_trainer.enter(&mut self.rng);
        }

        if wants_fresh_interval {
            self.interval_trainer.enter(&mut self.rng);
        }

        if wants_fresh_play {
            self.play_the_interval.enter(&mut self.rng);
        }

        // The library opens ready to be typed into: it is a thing you come to in order to
        // look something up, and a focus ring parked on Back would make the first keystroke
        // do nothing. `enter` clears the query for the reason the trainers draw a fresh
        // prompt — the screen never reopens on what it last showed — and the root it
        // established survives, so an empty box still means "that root, every quality".
        if wants_the_search_box {
            self.chord_library.enter();
            self.focused = FocusTarget::SearchBox;
        }

        // Unlike the trainers, this screen reopens on what it last showed: it is a reference
        // rather than a run, so the root a learner was working on is what they came back for.
        // `enter` only re-establishes the selection's invariant.
        if wants_the_caged_window {
            self.caged.enter();
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
            self.screen = prev;
            self.reset_focus();
        }
    }

    /// What a screen has to put down on the way out.
    ///
    /// The screens that listen hold an open capture device, and an audio interface is a
    /// device other programs want. Both navigation paths call this, which
    /// is what stops the two from disagreeing about it.
    fn leaving_for(&mut self, next: &Screen) {
        if self.screen == *next {
            return;
        }

        // A run does not outlive the screen it is on: its clock would go on spending a
        // duration behind a screen nobody is looking at, and `leave` is also what puts the
        // device down when the run was listening.
        if self.screen == Screen::WalkTheScale {
            self.walk_the_scale.leave();
        }

        // Asked of the screen being left rather than of the one being entered, and before
        // `self.screen` moves — which is what makes moving between the two listening
        // screens close one stream before the other opens, rather than holding both.
        if let Some(listener) = self.listener() {
            listener.stop();
        }
    }

    /// The device of whichever screen is listening, or `None` on the screens that do not.
    ///
    /// Routing by the screen on show is sound here, and would not be for the trainers'
    /// flash timers: both trainers can genuinely be mid-flash at once, while only one
    /// screen can hold the device — `leaving_for` closes it on the way out, so a reading
    /// arriving for a screen that is not showing has no screen it could have come from.
    /// That invariant is what this helper rests on, and what the test asserting one stream
    /// at a time exists to protect.
    fn listener(&mut self) -> Option<&mut Listener> {
        match self.screen {
            Screen::Tuner => Some(self.tuner.listener_mut()),
            Screen::PlayTheInterval => Some(self.play_the_interval.listener_mut()),
            Screen::WalkTheScale => self.walk_the_scale.listener_mut(),
            _ => None,
        }
    }

    fn listener_ref(&self) -> Option<&Listener> {
        match self.screen {
            Screen::Tuner => Some(self.tuner.listener()),
            Screen::PlayTheInterval => Some(self.play_the_interval.listener()),
            Screen::WalkTheScale => self.walk_the_scale.listener(),
            _ => None,
        }
    }

    fn is_picking(&self) -> bool {
        self.listener_ref().is_some_and(Listener::is_picking)
    }

    fn is_listening(&self) -> bool {
        self.listener_ref().is_some_and(Listener::is_listening)
    }

    /// Whether the run's one-second clock should tick. Named rather than inlined in
    /// `subscription` because a `Subscription` cannot be inspected, and this is the part a
    /// test can reach.
    fn wants_run_clock(&self) -> bool {
        self.screen == Screen::WalkTheScale && self.walk_the_scale.is_running()
    }

    /// One tick of audio, folded into the screen that asked for it.
    ///
    /// Not `listener().poll()` alone: a drill has to see the reading to judge it, so the
    /// screen is given what was released rather than left to read it back off the device
    /// afterwards. The Tuner wants nothing beyond the fold, and says so by doing nothing.
    fn poll_audio(&mut self) {
        match self.screen {
            Screen::Tuner => {
                self.tuner.listener_mut().poll();
            }
            // The drill has to see the reading to judge it, so the fold and the judging
            // happen in one call rather than the screen reading it back off the device
            // afterwards — two readers of one reading are two readers free to disagree.
            Screen::PlayTheInterval => {
                if let Some(heard) = self.play_the_interval.listener_mut().poll() {
                    self.play_the_interval.apply(heard);
                }
            }
            Screen::WalkTheScale => {
                let heard = self.walk_the_scale.listener_mut().and_then(Listener::poll);
                if let Some(heard) = heard {
                    self.walk_the_scale.apply(heard);
                }
            }
            _ => {}
        }
    }

    /// A released reading, handed to the screen listening. The seam the screens' tests
    /// drive in place of a device.
    #[cfg(test)]
    fn hear(&mut self, heard: Heard) {
        match self.screen {
            Screen::Tuner => self.tuner.listener_mut().apply(heard),
            Screen::WalkTheScale => {
                if let Some(listener) = self.walk_the_scale.listener_mut() {
                    listener.apply(heard);
                }
                self.walk_the_scale.apply(heard);
            }
            Screen::PlayTheInterval => {
                self.play_the_interval.listener_mut().apply(heard);
                self.play_the_interval.apply(heard);
            }
            _ => {}
        }
    }

    /// Picks a new scale, never the one already on screen.
    ///
    /// Root and kind are drawn from the same advancing stream, so the second draw
    /// cannot be a function of the first. Rerolling onto the current scale would
    /// make the button look broken, so that one outcome is rejected and redrawn —
    /// which is also why the very first scale of a session is never C Ionian.
    fn reroll_scale(&mut self) {
        let current = (self.scale.root(), self.scale.kind());

        loop {
            let root = PitchClass::ALL[self.rng.below(PitchClass::ALL.len())];
            let kind = ScaleKind::ALL[self.rng.below(ScaleKind::ALL.len())];

            if (root, kind) != current {
                self.set_scale(root, kind);
                return;
            }
        }
    }

    /// A different root than the one on show, so a press always changes the neck — the rule
    /// `reroll_scale` follows, with one field to draw instead of two.
    fn reroll_caged_root(&mut self) {
        let current = self.caged.root();

        loop {
            let root = PitchClass::ALL[self.rng.below(PitchClass::ALL.len())];

            if root != current {
                self.caged.select_root(root);
                return;
            }
        }
    }

    /// The one place a `Scale` is built, because its spelling is derived from the
    /// root and the kind together: changing either has to re-derive it, and five
    /// call sites each calling `Scale::new` would be five chances to pass a stale
    /// field. See `Scale::new` for what is being derived.
    fn set_scale(&mut self, root: PitchClass, kind: ScaleKind) {
        self.scale = Scale::new(root, kind);
    }

    /// Flips what the markers say. Nothing about the scale moves — see `Notation`.
    fn toggle_notation(&mut self) {
        self.notation = match self.notation {
            Notation::Notes => Notation::Intervals,
            // A scale has no fingering, so this field never holds `Fingers` — the library
            // keeps its own setting, and the two screens no longer move each other.
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
    /// Except on the neck, which claims the motion keys while focused and moves its own
    /// cursor instead. That is a deliberate special case rather than a mechanism: the neck
    /// is a grid of seventy-eight positions, and reaching them by anything other than the
    /// arrows would cost more than Tab becoming the only way out. If a second such widget
    /// ever appears, that is the time to generalise — not before.
    fn move_focus(&mut self, direction: Direction) {
        // The device list claims the vertical pair outright while it is open, the way the
        // CAGED pills claim the horizontal one: choosing is one gesture rather than
        // focus-then-activate. It is modal, so this comes before every other claim.
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

        // The list claims all four: up and down walk the chords, left and right walk the
        // selected chord's voicings. Tab is still the way out, as it is on the two necks.
        if self.focused == FocusTarget::ChordList {
            match direction {
                Direction::Up => self.chord_library.move_row(-1),
                Direction::Down => self.chord_library.move_row(1),
                Direction::Left => self.chord_library.move_voicing(-1),
                Direction::Right => self.chord_library.move_voicing(1),
            }
            return;
        }

        // The pills claim the horizontal pair only, as the library's list claims all four:
        // walking the cycle is one gesture rather than focus-then-activate. Up and down still
        // leave the row, since there is nowhere further along it to go.
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
    ///
    /// A miss is silent: `translate_key` forwards every character it does not recognise as
    /// a motion, so most of what arrives here is nothing at all.
    fn accelerate(&mut self, c: char) -> Task<Message> {
        // Offered to the screen first, because `gg` is a *pair* of keys and the accelerator
        // table maps one key to one action — it has nowhere to keep "a `g` is pending". A
        // screen that wants a motion of its own takes it here and the table never sees it.
        if self.screen == Screen::ChordLibrary && self.chord_library.motion(c) {
            return Task::none();
        }

        let fires = self
            .accelerators()
            .into_iter()
            .find_map(|(key, fires, _)| (key == c).then_some(fires));

        match fires {
            Some(Fires::Widget(target)) => self.activate(target),
            // `dispatch`, as for a translated key, and its task handed back rather than
            // dropped, so a message that has an effect to run keeps it.
            Some(Fires::Message(message)) => return self.dispatch(message),
            None => {}
        }

        Task::none()
    }

    /// Performs `target`'s action. Focus is left alone: an accelerator fires a widget
    /// without walking the ring onto it, so the only focus changes here are the ones
    /// the action itself causes — navigating resets focus exactly as a click would.
    fn activate(&mut self, target: FocusTarget) {
        match target {
            FocusTarget::HomeMenuItem(index) => {
                if let Some(item) = HOME_MENU.get(index) {
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
            // Enter on the neck answers with wherever the cursor is sitting, which is the
            // same thing a click on that position would send.
            FocusTarget::Fretboard => self.note_trainer.answer_at_cursor(),
            FocusTarget::NoteAnswer(index) => {
                if let Some(&pitch_class) = PitchClass::ALL.get(index) {
                    self.note_trainer.answer_name(pitch_class);
                }
            }
            FocusTarget::DrillDirectionToggle => self.note_trainer.toggle_direction(&mut self.rng),
            FocusTarget::PoolToggle => self.note_trainer.toggle_pool(&mut self.rng),
            FocusTarget::NoteSpellingToggle => self.note_trainer.toggle_spelling(),
            FocusTarget::TunerSpellingToggle => self.tuner.toggle_spelling(),
            FocusTarget::SkipPrompt => self.note_trainer.skip(&mut self.rng),
            FocusTarget::SearchBox => self.chord_library.focus_search(),
            FocusTarget::NotationChoice(index) => self.chord_library.set_notation(index),
            // Nothing to fire: the list is browsed with the arrows rather than activated,
            // and a chord is already selected the moment the ring reaches it.
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
        };

        if self.help_open {
            iced::widget::stack![screen, ui_help_overlay(self.accelerators())].into()
        } else {
            screen
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let keys = keyboard::listen().filter_map(|event| {
            // `modified_key`, not `key`: iced reports `key` *without* modifiers applied, so
            // Shift+/ arrives there as `/` and `?` could never match. `modified_key` is the
            // character the user actually typed, which is also what makes Shift+R read as
            // `R` — unbound — rather than as a plain `r` that would reroll.
            let keyboard::Event::KeyPressed {
                modified_key,
                modifiers,
                ..
            } = event
            else {
                return None;
            };
            // Deliberately no translation here — see `Message::Key`. The closure captures
            // nothing, which is what keeps this stream correct when the state that decides
            // what a key means starts changing.
            Some(Message::Key(modified_key, modifiers))
        });

        // The flash's timer, alive only while one is on screen. Starting the stream with
        // the flash is what puts its first tick a second later; an always-on timer would
        // land anywhere between nothing and a full second after the answer, which is the
        // one thing a fixed pause must not do. It also keeps the app idle the rest of the
        // time, since nothing else here ticks.
        // Each trainer's flash gets its own timer and its own message. Only one screen is
        // ever on show, but a flash left behind on the other one keeps its timer alive until
        // it is retired — harmless, because `advance` is a no-op once the prompt has moved
        // on, and because a tick can only reach the trainer its variant names.
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

        // One second is the resolution the readout needs — `4:59` — and anything finer would
        // redraw for nothing. Gated on a run rather than on the screen, so an untimed run
        // ticks no more than a timed one needs.
        if self.wants_run_clock() {
            streams
                .push(iced::time::every(Duration::from_secs(1)).map(|_| Message::WalkTheScaleTick));
        }

        // Only while the Tuner is showing and a device is open, so nothing ticks on any
        // other screen. Ten milliseconds is a little under the hop the detector wants, so
        // a window is picked up in the tick after it completes rather than one later.
        //
        // Note what does *not* travel in this closure: the stream lives on `App`, because
        // iced hashes a subscription recipe rather than its captured values and would
        // otherwise drop and rebuild the device whenever that hash changed.
        if self.is_listening() {
            streams.push(iced::time::every(AUDIO_POLL).map(|_| Message::AudioPoll));
        }

        Subscription::batch(streams)
    }
}

/// The back arrow's size. Larger than the 18 the body font wore, because the maths face
/// draws an arrow against its own optical size rather than a cap height — at 18 the mark
/// came out as a rule with a nick in it.
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

fn ui_home(focused: FocusTarget) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{column, container, row, text};

    let menu = column(HOME_MENU.iter().enumerate().map(|(index, item)| {
        focus_ring(
            trainer_button(item.label, item.caption)
                .on_press(Message::Navigate(item.screen.clone())),
            focused == FocusTarget::HomeMenuItem(index),
        )
    }))
    .spacing(12);

    let hero = column![
        text("Trastea").size(56).color(INK),
        text("A focused guitar trainer for scales, intervals, and fretboard fluency.")
            .size(21)
            .color(BODY),
        row![
            // text("α").size(13).color(CANVAS),
            // text("desktop practice lab").size(13).color(INK)
        ]
        .spacing(8)
        .padding([6, 12])
    ]
    .spacing(16);

    let content = container(row![hero, menu].spacing(64))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([48, 64])
        .center_y(Length::Fill);

    with_top_bar("Trastea", content.into(), false, focused)
}

/// Each button is named by the scale pressing it would build, so the label is a
/// promise the press keeps. That means a label moves when the *kind* selection
/// moves — pitch class 10 reads `A♯` under Minor Pentatonic and `B♭` under Ionian —
/// which is the fact the whole naming rule is about: the same fret is called
/// different things depending on what you are playing over it.
///
/// The message a press sends and the focus target a button answers to are both parameters,
/// because the Scale Trainer and the scale drill each own their own scale — a shared grid
/// that hard-coded `Message::SelectRoot` would move the trainer's scale from the drill.
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

/// The kind pills. Parameterised like `root_note_row` and for the same reason.
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

/// One square root button, wrapped in its focus ring.
///
/// Shared by the Scale Trainer and the CAGED window. The two differ only in what names a root
/// and what a press on one means, so both are parameters and the geometry is not.
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

    // `Pixels` is `Copy`, so resolving the conversion once lets both the letter and the
    // accidental share it — `impl Into<Pixels>` itself would be moved by the first `size`.
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

/// The SMuFL glyph for an accidental. `None` for a natural: a natural sign would
/// be wrong in a note label, and the major scale's degrees carry no glyph.
fn accidental_glyph(accidental: Accidental) -> Option<char> {
    match accidental {
        Accidental::DoubleFlat => Some(SMUFL_DOUBLE_FLAT),
        Accidental::Flat => Some(SMUFL_FLAT),
        Accidental::Natural => None,
        Accidental::Sharp => Some(SMUFL_SHARP),
        Accidental::DoubleSharp => Some(SMUFL_DOUBLE_SHARP),
    }
}

/// The glyph (if any) and the degree digit that together label one interval in
/// the formula card — e.g. Blues's ♭5 is `(Some(SMUFL_FLAT), 5)`.
///
/// Split out as a pure decision rather than a rendered `String` because the two
/// parts render in different fonts — the glyph in `MUSIC_FONT`, the digit in the
/// body font — so `intervalic_text` still needs two separate `text` widgets per
/// token; this is what lets that per-part font split be tested without building
/// an iced widget tree.
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

/// The text inside one marker — a note's name, or the job it does in the scale.
///
/// A pure decision split out of `scale_markers` for the reason `interval_token` is
/// split out of `intervalic_text`: it is the part worth testing, and testing it
/// here needs no widget tree and no canvas.
fn marker_label(notation: Notation, note: Note, degree: Interval) -> String {
    match notation {
        Notation::Notes => note.to_string(),
        // A scale has no fingering. The library keeps its own setting now, so this field
        // never actually holds `Fingers`; the arm is here to keep the match total rather
        // than to describe anything the screen can reach.
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
    ///
    /// `focus_grid` and `focusables` read `self` rather than a bare `&Screen`, because a
    /// screen's shape can depend on state — so asking what is focusable means having an
    /// app to ask.
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

        // Reseeding from the clock on every draw made root and kind both functions
        // of the same instant, so only lcm(12, 16) = 48 of the 192 pairs could ever
        // come up — C Dorian, for one, was unreachable.
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

    // `toggling_spelling_renames_the_scale_without_moving_it` and
    // `rerolling_leaves_the_spelling_alone` stood here. Both tested a control that no
    // longer exists, and neither survives as a repair: spelling is not a user setting
    // to be preserved across a reroll, it is derived from the root and kind. What the
    // first of them really protected — that naming a scale never moves it — is now
    // `naming_never_moves_the_scale` in `music/scales.rs`, asserted across all 192
    // combinations rather than one. The rest is carried by `Scale`'s private fields,
    // which is the better place for it: a test could only check the paths it thought
    // to walk, where the type rules the bad state out everywhere at once.

    #[test]
    fn selecting_a_root_names_it_the_way_it_is_taught() {
        // The headline case, asserted where it reaches the screen: pitch class 10 under
        // Ionian is B♭ major, not the A♯ major that would put three double sharps on
        // the neck.
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
        // The label is a promise; this is the promise being kept. Both kinds are
        // checked because the label moves with the kind — pitch class 10 reads A♯
        // under Minor Pentatonic and B♭ under Ionian — and a rule that only held for
        // one of them would be worse than no rule.
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
        // A natural sign would be wrong in a note label and in a formula alike,
        // which is why this returns Option rather than a char. The four glyph
        // constants are hand-written, so a swapped pair would otherwise compile
        // and pass every other test in this file.
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
    /// `c` is the *modified* key — the character the keyboard actually produces, which is
    /// what the subscription feeds `translate_key`. So Shift+/ is `press("?", SHIFT)` and
    /// Shift+r is `press("R", SHIFT)`; writing those as `"/"` or `"r"` would describe an
    /// event no keyboard sends, which is exactly how `?` shipped broken behind a passing
    /// test.
    pub(super) fn press(c: &str, modifiers: keyboard::Modifiers) -> Option<Message> {
        translate_key(keyboard::Key::Character(c.into()), modifiers)
    }

    /// Every screen, so a new one cannot quietly escape the checks that sweep them all.
    pub(super) fn every_screen() -> [Screen; 9] {
        // No `_` arm, so a new `Screen` stops this compiling until it is listed — the array
        // below is the next line to change.
        match Screen::default() {
            Screen::Home
            | Screen::ScaleTrainer
            | Screen::WalkTheScale
            | Screen::NoteTrainer
            | Screen::IntervalTrainer
            | Screen::PlayTheInterval
            | Screen::ChordLibrary
            | Screen::Caged
            | Screen::Tuner => {}
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
        ]
    }

    /// The state a keypress could disturb.
    ///
    /// `translate_key` no longer answers "is this key bound?" — it forwards every
    /// unrecognised character for the screen to resolve — so the absence of a binding is
    /// only observable here, as the absence of a change.
    fn snapshot(app: &App) -> (Screen, FocusTarget, PitchClass, ScaleKind, Notation, usize) {
        (
            app.screen.clone(),
            app.focused,
            app.scale.root(),
            app.scale.kind(),
            // The scale's spelling was watched here too. It is derived from the two
            // fields above now, so it cannot move on its own and watching it would only
            // restate them.
            //
            // Without `notation` the tests below would watch `i` do nothing and call
            // that a pass.
            app.notation,
            app.history.len(),
        )
    }

    /// Presses a character key and lets the app act on whatever it translates to.
    /// Drives a key press the way the subscription does: as a raw `Message::Key` into
    /// `update`, which is where translation now happens. `press` above still probes
    /// `translate_key` directly, because it is still a pure function and a test that can
    /// say "this produced no message" is stronger than one that can only say "nothing
    /// changed".
    pub(super) fn press_into(app: &mut App, c: &str, modifiers: keyboard::Modifiers) {
        let _ = app.update(Message::Key(keyboard::Key::Character(c.into()), modifiers));
    }

    #[test]
    fn the_seventh_digit_opens_the_caged_window() {
        let mut app = app_with_seed(0xca6);
        press_into(&mut app, "7", keyboard::Modifiers::empty());

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

    /// Shift+h delivers the capital, not the lowercase letter with a flag set. An earlier    /// Shift+h delivers the capital, not the lowercase letter with a flag set. An earlier
    /// version of this test pressed `"h"` with SHIFT — which no keyboard produces — and
    /// passed only because the guard rejected every modifier, Shift included.
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

    /// The one character key admitted while a command modifier is held. `COMMAND_MODIFIERS`
    /// still suppresses everything else, which the test above is what proves.
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
            // Read from the table rather than listed here, so a screen that takes up `i`
            // exempts itself instead of turning this into a failure to edit by hand.
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

    /// The scale drill's two, on the same terms as `i` above: read from the table, so a
    /// screen that takes a bracket up exempts itself rather than making this a failure to
    /// edit by hand.
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

    /// The mode is a preference, not a property of what is on screen — so nothing that
    /// replaces the scale may quietly put the neck back into note names.
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

    /// Sharper than comparing snapshots: if `r` on Home reached the reroll at all it would
    /// consume draws, so an identically seeded app that never pressed it would diverge.
    /// Equal scales here mean the key did not touch the generator, let alone the scale.
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

    /// An accelerator fires its target without walking the ring onto it, so an action that
    /// stays on the screen must leave focus exactly where the user left it.
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
        // The library is the sixth entry, so it takes the sixth digit — both derived
        // from `HOME_MENU`, which is why a screen inserted above it needed nothing else.
        press_into(&mut app, "6", keyboard::Modifiers::empty());

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
        // `navigate_to` rather than `app_on`, because the second half of this needs a
        // history for `GoBack` to pop.
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
        // Type to find it, enter to look at it: the arrows have to walk the voicings the
        // moment the box lets go, without a Tab in between.
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

        // And anything between the two halves cancels it, so a stale `g` cannot turn the
        // next keystroke into a jump.
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
        // The difference between three buttons and one that cycles: pressing the second
        // means the second, from wherever the setting happens to be.
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
        // Fingers is what a learner opening this screen wants: it answers "how do I play
        // this". The Scale Trainer has no fingering and keeps its own setting, so picking
        // one here cannot change what that screen labels its markers with.
        let mut app = app_with_seed(0xf14);
        app.open(Screen::ChordLibrary);

        assert_eq!(app.chord_library.notation(), Notation::Fingers);
        assert_eq!(app.notation, Notation::Notes, "the scale trainer moved");

        // Out of the box first: `d` is a note name, so while the box has the keyboard it
        // has to type rather than switch modes, or `d` could never begin a search for a D
        // chord. See `the_mode_keys_type_while_the_box_has_focus`.
        press_named(&mut app, Named::Escape);
        press_into(&mut app, "d", keyboard::Modifiers::empty());

        assert_eq!(app.chord_library.notation(), Notation::Intervals);
        assert_eq!(app.notation, Notation::Notes, "the scale trainer moved");
    }

    #[test]
    fn the_mode_keys_type_while_the_box_has_focus() {
        // The collision worth stating: `n`, `d` and `f` are all note names, so a focused
        // search box must take them as text. The screen opens with the box focused, so the
        // mode keys are a thing you reach for after `Esc` — exactly as `/` and `i` are.
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
        // The keyboard keeps its one-key habit: `i` names whichever button comes next, so
        // its accelerator target moves with the setting rather than being fixed.
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

    /// The counterpart to `an_in_screen_accelerator_leaves_focus_alone`: when the action
    /// navigates, focus does move — because navigating moves it, not because the
    /// accelerator did. This distinction only became visible once Home had keys.
    #[test]
    fn a_navigating_accelerator_resets_focus_on_the_new_screen() {
        for (index, item) in HOME_MENU.iter().enumerate() {
            let key = char::from_digit(index as u32 + 1, 10).expect("menu fits in digits");

            let mut app = app_with_seed(0x6070);
            app.focused = FocusTarget::HomeMenuItem(HOME_MENU.len() - 1);

            press_into(&mut app, &key.to_string(), keyboard::Modifiers::empty());

            assert_eq!(app.screen, item.screen, "{key} opened the wrong screen");

            // The ring lands on the screen's first focusable, except where the screen
            // asks for somewhere else: the library opens with the caret in its search
            // box, since looking something up starts with typing.
            let landed = match item.screen {
                Screen::ChordLibrary => FocusTarget::SearchBox,
                _ => app_on(item.screen.clone()).focusables()[0],
            };

            assert_eq!(app.focused, landed, "{key} did not reset focus");
        }
    }

    /// Shift is how `?` is typed. Were it filtered out the way Cmd, Ctrl, and Alt are, the
    /// overlay would be unreachable on most keyboards.
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

    /// The overlay is a layer, not a screen: it contributes nothing focusable, so opening
    /// and dismissing it must leave the focus ring exactly where it was.
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

    /// Pressing a named key and letting the app act on it, for the keys `press` cannot
    /// build because they produce no character.
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
            app.focused = FocusTarget::HomeMenuItem(1);

            press_named(&mut app, named);

            assert_eq!(
                app.screen,
                Screen::WalkTheScale,
                "{named:?} did not activate the focused menu item"
            );
        }
    }

    /// Key translation reads no application state, so the same press must mean the same
    /// thing everywhere — screen scoping happens after this point, not inside it.
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
        // As in `summary_card_fits_a_two_line_name`: iced's default relative line height,
        // and the card's own `.spacing(12)` and `.padding(32)` from `ui_scale_trainer`.
        const TEXT_LINE_HEIGHT: f32 = 1.3;
        const COLUMN_SPACING: f32 = 12.0;
        const CARD_PADDING: f32 = 32.0;
        // A pill is its text, the button's own `.padding([8, 12])`, and the 3px `focus_ring`
        // wrapped round it. Only the vertical half of each matters here.
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
        // iced's `LineHeight::default()` — `Relative(1.3)` in `iced_core::text`. Restated
        // here because iced resolves it internally, and the budget below is in laid-out
        // pixels rather than font sizes.
        const TEXT_LINE_HEIGHT: f32 = 1.3;
        // The card's own `.spacing(10)` and `.padding(32)`, both in `ui_scale_trainer`.
        const COLUMN_SPACING: f32 = 10.0;
        const CARD_PADDING: f32 = 32.0;
        // `Harmonic Minor` and the two pentatonics are all wider than the card at
        // `SUMMARY_NAME_SIZE`, so a name has to be budgeted at two lines, not one.
        const NAME_LINES: f32 = 2.0;

        let line = |size: f32| size * TEXT_LINE_HEIGHT;
        // The header row is as tall as the root label, which dominates the two ghost
        // buttons sharing the line with it.
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
        // The Tuner's poll fires every 10 ms. Before this, `?` there opened the overlay
        // and the next tick closed it — one frame, and unreadable.
        let (mut app, _) = App::new();
        let _ = app.update(Message::Navigate(Screen::Tuner));
        let _ = app.update(Message::ToggleHelp);
        assert!(app.help_open);

        for _ in 0..5 {
            let _ = app.update(Message::AudioPoll);
            assert!(app.help_open, "a poll tick closed the help overlay");
        }

        // The trainers' flash timers, which this file already handled the same way. They
        // are here so the three stay together: a fourth timer added above the overlay
        // guard and not below it would pass this test only by accident.
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
