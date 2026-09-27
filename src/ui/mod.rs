mod caged;
mod chord_diagram;
mod chord_library;
mod drill;
mod fretboard;
mod interval_trainer;
mod listener;
mod neck;
mod note_trainer;
mod play_the_interval;
mod scale_trainer;
mod shapes;
mod style;
mod tuner;
mod walk_the_scale;
use std::borrow::Cow;
use std::ops::Range;
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

use iced::{Background, Border, Color, Element, Pixels, Shadow, Subscription, Task, keyboard};
use keyboard::key::Named;

#[cfg(test)]
use crate::audio::detect::Heard;
use crate::music::{
    intervals::Interval,
    notes::{Accidental, Note, PitchClass},
    scales::Scale,
    scales::ScaleKind,
};
use crate::rng::Rng;

use drill::{ANSWER_COUNT, Drill, answer_at};
use neck::{DRILL_NECK, NECK_STRINGS};
use style::{
    BODY, CANVAS, CANVAS_SOFT_2, INK, KEYCAP_ARROWS, KEYCAP_BACK, KEYCAP_BACKTAB, KEYCAP_FONT,
    KEYCAP_TAB, MUSIC_FONT, MUTE, SMUFL_DOUBLE_FLAT, SMUFL_DOUBLE_SHARP, SMUFL_FLAT, SMUFL_SHARP,
    card_button, card_container, focus_ring, ghost_button, page_container, row_button,
    selected_root_button, selected_row_button,
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    HomeMenuItem(usize),
    Back,
    NotationToggle,
    RerollScale,
    Root(usize),
    ScaleKind(usize),
    /// The Note Trainer's neck, as the *Find it* answer surface.
    ///
    /// The one focusable that claims the motion keys: while it is focused the arrows move
    /// the cursor within it instead of moving the ring off it. See `App::move_focus`.
    Fretboard,
    /// One of the twelve *Name it* answer buttons, indexed into `PitchClass::ALL`.
    NoteAnswer(usize),
    DrillDirectionToggle,
    PoolToggle,
    /// The Note Trainer's own. The scale trainer derives its spelling rather than offering
    /// a choice — see `Scale::new` — so this and the Tuner's are the only two.
    NoteSpellingToggle,
    TunerSpellingToggle,
    SkipPrompt,
    /// The Interval Trainer's neck, as its *Find it* answer surface. Claims the motion keys
    /// while focused, exactly as `Fretboard` does on the Note Trainer.
    IntervalFretboard,
    /// One of the twelve interval answer buttons, indexed into the drill's vocabulary.
    IntervalAnswer(usize),
    IntervalDirectionToggle,
    SkipIntervalPrompt,
    /// A listening screen's device control, which opens the input list.
    InputDevice,
    /// The played drill's skip control.
    SkipPlayPrompt,
    /// The Chord Library's search box.
    SearchBox,
    /// One of the notation buttons, indexed into the screen's `Notation::cycle_for`.
    ///
    /// Three buttons rather than one that cycles: a control that changes what it says each
    /// time it is pressed does not look like a control with options, so nobody presses it
    /// twice to find out. The `i` key still cycles — see `App::accelerators`.
    NotationChoice(usize),
    /// The Chord Library's chord list.
    ///
    /// Claims all four motion keys while focused, as the two necks do: up and down walk
    /// the chords, left and right walk the selected chord's voicings. The strip is not a
    /// focusable of its own — the search spec puts the voicings on the horizontal axis
    /// whenever the box is unfocused, and a third stop in the ring would instead make them
    /// reachable only after tabbing past the list.
    ChordList,
    /// The CAGED window's root grid, indexed like the Scale Trainer's.
    CagedRoot(usize),
    /// The CAGED window's shuffle button: a new root, not a new scale.
    RerollCagedRoot,
    /// The scale drill's kind grid, indexed like the Scale Trainer's.
    DrillKind(usize),
    /// The two keys that step the drill's root, down and up.
    ///
    /// The first targets in the app that are actions rather than focus stops: the drill's root
    /// is dragged on the glyph that names it, so there is no widget for a key to fire. Nothing
    /// requires a target to be in the grid — `activate` is a match and the key lookup never
    /// consults it — but the name is less true than it was. The honest fix is an `Accelerator`
    /// whose second element is a message rather than a target, which is worth doing when a
    /// second screen wants a key with no widget behind it and not before.
    DrillRootDown,
    DrillRootUp,
    /// A different scale to drill.
    RerollDrillScale,
    /// Starts another run, offered only once one has ended on its duration.
    RestartDrill,
    /// The three controls that exist only while a run is going.
    ScaleAnswering,
    ScaleDuration,
    ScaleNeck,
    /// The Scale Trainer's neck, as the drill's answer surface while pressing. Claims the
    /// motion keys while focused, exactly as the two trainers' necks do.
    ScaleFretboard,
    /// One shape pill in the CAGED window.
    ///
    /// Claims the horizontal motions while focused, as the Chord Library's list does for its
    /// voicings: the arrows change the selection rather than moving the ring along the row,
    /// so walking the cycle is one gesture.
    CagedShape(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// One row of a screen's focus grid. `None` is a cell with no widget in it, which
/// keeps the columns of neighbouring cards aligned.
type FocusRow = Vec<Option<FocusTarget>>;

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

/// Row shapes of the two selector grids on the scale trainer. Both the views and
/// the focus grid are built from these, so the two cannot drift out of sync.
const ROOT_ROW_WIDTH: usize = 3;
/// Six rows, not the five a sixteen-item grid would suggest. `Voodoo | Spanish Gypsy |
/// Whole Tone | Diminished` came to 456px against the 440px the card gets at the default
/// 1280-wide window, so `Diminished` was cut off its right edge. Of every contiguous
/// five-row shape only two fit at all, with 10px and 0px to spare; this is the one
/// six-row shape that both fits — 73px spare on its widest row — and stays a palindrome,
/// which the old 4-3-2-3-4 was.
const KIND_ROW_WIDTHS: [usize; 6] = [3, 3, 2, 2, 3, 3];

/// Columns in the Note Trainer's answer grid. Four divides the twelve pitch classes evenly
/// and matches the four header controls above them, so the columns line up.
const ANSWER_ROW_WIDTH: usize = 4;

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
            Message::AnswerNote(pitch_class) => {
                self.note_trainer.answer_name(pitch_class, &mut self.rng)
            }
            Message::ChooseNotePosition(string, fret) => {
                self.note_trainer
                    .answer_position(string, fret, &mut self.rng)
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
            Message::Accelerate(c) => self.accelerate(c),
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
    fn accelerate(&mut self, c: char) {
        // Offered to the screen first, because `gg` is a *pair* of keys and the accelerator
        // table maps one key to one widget — it has nowhere to keep "a `g` is pending". A
        // screen that wants a motion of its own takes it here and the table never sees it.
        if self.screen == Screen::ChordLibrary && self.chord_library.motion(c) {
            return;
        }

        let target = self
            .accelerators()
            .into_iter()
            .find_map(|(key, target, _)| (key == c).then_some(target));

        if let Some(target) = target {
            self.activate(target);
        }
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
            FocusTarget::DrillRootDown => self.walk_the_scale.step_root(-1),
            FocusTarget::DrillRootUp => self.walk_the_scale.step_root(1),
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
            FocusTarget::Fretboard => self.note_trainer.answer_at_cursor(&mut self.rng),
            FocusTarget::NoteAnswer(index) => {
                if let Some(&pitch_class) = PitchClass::ALL.get(index) {
                    self.note_trainer.answer_name(pitch_class, &mut self.rng);
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

/// Yields the `(start, len)` of each row of the root selector, derived from
/// `PitchClass::ALL` so adding a note reshapes the grid and the view together.
fn root_row_spans() -> impl Iterator<Item = (usize, usize)> {
    let total = PitchClass::ALL.len();
    (0..total)
        .step_by(ROOT_ROW_WIDTH)
        .map(move |start| (start, ROOT_ROW_WIDTH.min(total - start)))
}

/// Yields the `(start, len)` of each row of the scale-kind selector. Unlike the
/// root grid these rows are ragged, so the widths are spelled out.
fn kind_row_spans() -> impl Iterator<Item = (usize, usize)> {
    KIND_ROW_WIDTHS.into_iter().scan(0, |start, len| {
        let span = (*start, len);
        *start += len;
        Some(span)
    })
}

/// The three functions that describe where a screen's focusable widgets are.
///
/// Methods rather than free functions of `&Screen`, because a screen's shape is no longer
/// determined by which screen it is: the Note Trainer's focusables depend on which drill
/// direction is in play, which is application state. Threading that as an extra parameter
/// would work today and need another parameter the next time — so they read `self`.
impl App {
    /// The focusable widgets of a screen laid out as they appear on it.
    ///
    /// Columns are shared across cards that sit side by side: on the scale trainer the
    /// root card occupies columns `0..ROOT_ROW_WIDTH` and the scale-kind card the
    /// columns after it, so pressing Right at the edge of the root grid steps into the
    /// kinds grid. Cells the layout leaves empty are `None`, which is what stops a
    /// vertical move at the bottom of the root card instead of dropping it into the
    /// taller kinds card alongside.
    fn focus_grid(&self) -> Vec<FocusRow> {
        match &self.screen {
            // Back, the control that opens the device list, and the spelling under the dial.
            // The list itself is not in the ring — while it is up it claims the motion keys
            // outright, the way the CAGED shape row does, so walking it is one gesture.
            Screen::Tuner => vec![
                vec![Some(FocusTarget::Back)],
                vec![Some(FocusTarget::InputDevice)],
                vec![Some(FocusTarget::TunerSpellingToggle)],
            ],
            // Back, the skip control, and the device control. The neck is not a focus
            // target here: it takes no press, so there would be nothing to activate on it.
            Screen::PlayTheInterval => vec![
                vec![Some(FocusTarget::Back)],
                vec![Some(FocusTarget::SkipPlayPrompt)],
                vec![Some(FocusTarget::InputDevice)],
            ],
            Screen::Home => (0..HOME_MENU_ITEMS)
                .map(|i| vec![Some(FocusTarget::HomeMenuItem(i))])
                .collect(),
            Screen::Caged => {
                // Back on a row of its own, as on the two trainers and for the same reason:
                // the summary card's own controls already fill the row below it.
                let mut back_row: FocusRow = vec![None; ROOT_ROW_WIDTH];
                back_row[0] = Some(FocusTarget::Back);

                let mut controls: FocusRow = vec![None; ROOT_ROW_WIDTH];
                for (cell, target) in controls
                    .iter_mut()
                    .zip([FocusTarget::NotationToggle, FocusTarget::RerollCagedRoot])
                {
                    *cell = Some(target);
                }

                let mut grid = vec![back_row, controls];

                // As wide as the cycle turned out to be, which is the same number the caption
                // states — so a quality yielding three shapes gets three stops, not five.
                let shapes = self.caged.shapes().len();
                if shapes > 0 {
                    grid.push(
                        (0..shapes)
                            .map(|i| Some(FocusTarget::CagedShape(i)))
                            .collect(),
                    );
                }

                for (start, len) in root_row_spans() {
                    grid.push(
                        (0..len)
                            .map(|i| Some(FocusTarget::CagedRoot(start + i)))
                            .collect(),
                    );
                }

                grid
            }
            Screen::ScaleTrainer => {
                // Two header rows. The summary card's buttons — notation, reroll — sit
                // along its right edge, and the card is as wide as the root card below
                // it, so they take the leftmost columns above the first three
                // roots. That leaves no cell for Back, and widening the row is not an
                // option: `card_bands` splits at ROOT_ROW_WIDTH, so a fourth column would
                // land in the kinds card and Tab would reach the reroll button only after
                // all sixteen scale kinds.
                //
                // So Back takes a row of its own — which is where it really is, in the top
                // bar above the card rather than inside it. The cost is that Right from
                // Back no longer walks into the card; Tab still visits it first.
                let mut back_row: FocusRow = vec![None; ROOT_ROW_WIDTH];
                back_row[0] = Some(FocusTarget::Back);

                // Zipped rather than indexed so the row cannot be built wider than the band
                // it belongs to: a narrower ROOT_ROW_WIDTH drops buttons off the end instead
                // of panicking, a wider one leaves the extra cells empty.
                let mut card_buttons: FocusRow = vec![None; ROOT_ROW_WIDTH];
                for (cell, target) in card_buttons
                    .iter_mut()
                    .zip([FocusTarget::NotationToggle, FocusTarget::RerollScale])
                {
                    *cell = Some(target);
                }

                let root_rows: Vec<_> = root_row_spans().collect();
                let kind_rows: Vec<_> = kind_row_spans().collect();

                let mut grid = vec![back_row, card_buttons];
                for r in 0..root_rows.len().max(kind_rows.len()) {
                    let mut row: FocusRow = vec![None; ROOT_ROW_WIDTH];
                    if let Some(&(start, len)) = root_rows.get(r) {
                        for (col, cell) in row.iter_mut().enumerate().take(len) {
                            *cell = Some(FocusTarget::Root(start + col));
                        }
                    }
                    if let Some(&(start, len)) = kind_rows.get(r) {
                        row.extend((0..len).map(|i| Some(FocusTarget::ScaleKind(start + i))));
                    }
                    grid.push(row);
                }
                grid
            }
            // Back on a row of its own, then the kind grid alone: the drill's root is set on
            // the glyph in the middle column, so there is no root row to walk and no per-root
            // stop to land on. What differs from every other screen is the last row, which
            // grows with the run — a control that is not on screen is not a focus target,
            // which is what keeps Tab from stopping on nothing.
            Screen::WalkTheScale => {
                let mut grid: Vec<FocusRow> = vec![vec![Some(FocusTarget::Back)]];

                for (start, len) in kind_row_spans() {
                    grid.push(
                        (0..len)
                            .map(|i| Some(FocusTarget::DrillKind(start + i)))
                            .collect(),
                    );
                }

                let mut drill_row: FocusRow = Vec::new();
                let running = self.walk_the_scale.is_running();
                let playing = matches!(
                    self.walk_the_scale.answering(),
                    walk_the_scale::Answering::Playing { .. }
                );

                // The neck is only an answer surface while a press means something on it.
                if running && !playing {
                    drill_row.push(Some(FocusTarget::ScaleFretboard));
                }
                drill_row.push(Some(FocusTarget::RerollDrillScale));
                if running {
                    drill_row.push(Some(FocusTarget::ScaleAnswering));
                    drill_row.push(Some(FocusTarget::ScaleDuration));
                    if playing {
                        drill_row.push(Some(FocusTarget::ScaleNeck));
                        drill_row.push(Some(FocusTarget::InputDevice));
                    }
                } else {
                    drill_row.push(Some(FocusTarget::RestartDrill));
                }
                grid.push(drill_row);

                grid
            }
            Screen::NoteTrainer => {
                // Back on a row of its own, as on the scale trainer and for the same
                // reason: the four header controls already fill the card's own row.
                let mut back_row: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                back_row[0] = Some(FocusTarget::Back);

                let mut controls: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                for (cell, target) in controls.iter_mut().zip([
                    FocusTarget::DrillDirectionToggle,
                    FocusTarget::PoolToggle,
                    FocusTarget::NoteSpellingToggle,
                    FocusTarget::SkipPrompt,
                ]) {
                    *cell = Some(target);
                }

                let mut grid = vec![back_row, controls];

                // This is why these are methods on `App`: the answer surface — and so what
                // is focusable at all — depends on which way the drill is running, and the
                // prompt is the only thing that knows.
                match self.note_trainer.drill() {
                    Drill::NameIt => {
                        let total = PitchClass::ALL.len();

                        for start in (0..total).step_by(ANSWER_ROW_WIDTH) {
                            let len = ANSWER_ROW_WIDTH.min(total - start);
                            let mut row: FocusRow = vec![None; ANSWER_ROW_WIDTH];

                            for (col, cell) in row.iter_mut().enumerate().take(len) {
                                *cell = Some(FocusTarget::NoteAnswer(start + col));
                            }
                            grid.push(row);
                        }
                    }
                    // The neck is the answer surface, and it is one cell no matter how many
                    // positions it holds — the cursor walks those, not the focus ring.
                    Drill::FindIt => {
                        let mut row: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                        row[0] = Some(FocusTarget::Fretboard);
                        grid.push(row);
                    }
                }

                grid
            }
            Screen::IntervalTrainer => {
                // Back on a row of its own, as on the two screens above and for the same
                // reason: the card's own row belongs to its controls.
                let mut back_row: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                back_row[0] = Some(FocusTarget::Back);

                let mut controls: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                for (cell, target) in controls.iter_mut().zip([
                    FocusTarget::IntervalDirectionToggle,
                    FocusTarget::SkipIntervalPrompt,
                ]) {
                    *cell = Some(target);
                }

                let mut grid = vec![back_row, controls];

                match self.interval_trainer.drill() {
                    Drill::NameIt => {
                        let total = ANSWER_COUNT;

                        for start in (0..total).step_by(ANSWER_ROW_WIDTH) {
                            let len = ANSWER_ROW_WIDTH.min(total - start);
                            let mut row: FocusRow = vec![None; ANSWER_ROW_WIDTH];

                            for (col, cell) in row.iter_mut().enumerate().take(len) {
                                *cell = Some(FocusTarget::IntervalAnswer(start + col));
                            }
                            grid.push(row);
                        }
                    }
                    // The neck is the answer surface, and it is one cell no matter how many
                    // positions it holds — the cursor walks those, not the focus ring.
                    Drill::FindIt => {
                        let mut row: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                        row[0] = Some(FocusTarget::IntervalFretboard);
                        grid.push(row);
                    }
                }

                grid
            }
            // Two focusables, not three. The search spec puts the voicings on the
            // horizontal axis whenever the box is unfocused, so the strip is browsed from
            // the list rather than tabbed into — a third stop would make a voicing
            // reachable only after walking past every chord.
            Screen::ChordLibrary => vec![
                vec![Some(FocusTarget::Back), None, None, None],
                // The notation buttons sit beside the box rather than below the list, so
                // Right walks into them: the list claims the motion keys, and anything
                // under it in the grid would be reachable by Tab alone.
                vec![
                    Some(FocusTarget::SearchBox),
                    Some(FocusTarget::NotationChoice(0)),
                    Some(FocusTarget::NotationChoice(1)),
                    Some(FocusTarget::NotationChoice(2)),
                ],
                vec![Some(FocusTarget::ChordList), None, None, None],
            ],
        }
    }

    /// The column bands the cards of a screen occupy, in the order Tab visits them.
    ///
    /// Splitting the scale trainer at the root card's edge is what makes Tab finish one
    /// card before starting the next; reading the grid straight across would instead
    /// hop between the two side-by-side cards every few widgets. The back and reroll
    /// buttons fall in the first band, so they lead the order.
    #[expect(
        clippy::single_range_in_vec_init,
        reason = "a one-element Vec holding the full-width band, not a collected range"
    )]
    fn card_bands(&self, width: usize) -> Vec<Range<usize>> {
        match &self.screen {
            Screen::ScaleTrainer => vec![0..ROOT_ROW_WIDTH, ROOT_ROW_WIDTH..width],
            // These screens have a single card, so one band spans the whole width. Walk the
            // Scale joined them when its root card went: two bands split the kind rows down
            // the middle, and Tab walked the first three columns of every row before coming
            // back for the rest.
            Screen::Home
            | Screen::NoteTrainer
            | Screen::IntervalTrainer
            | Screen::PlayTheInterval
            | Screen::ChordLibrary
            | Screen::Caged
            | Screen::WalkTheScale
            | Screen::Tuner => vec![0..width],
        }
    }

    /// Every focusable on a screen, card by card — the Tab order.
    ///
    /// Derived from the same grid the arrow keys use, so a widget can never be
    /// reachable by one and not the other.
    fn focusables(&self) -> Vec<FocusTarget> {
        let grid = self.focus_grid();
        let width = grid.iter().map(Vec::len).max().unwrap_or(0);

        let mut targets = Vec::new();
        for band in self.card_bands(width) {
            for row in &grid {
                let cells = row.iter().skip(band.start).take(band.end - band.start);
                targets.extend(cells.flatten().copied());
            }
        }
        targets
    }
}

fn grid_position(grid: &[FocusRow], target: FocusTarget) -> Option<(usize, usize)> {
    grid.iter().enumerate().find_map(|(row, cells)| {
        cells
            .iter()
            .position(|&cell| cell == Some(target))
            .map(|col| (row, col))
    })
}

/// Moves one cell in `direction`, staying put when there is nothing that way.
fn step_focus_2d(grid: &[FocusRow], current: FocusTarget, direction: Direction) -> FocusTarget {
    let Some((row, col)) = grid_position(grid, current) else {
        // Focus is stale (the screen changed under it) — snap back onto the grid.
        return grid
            .iter()
            .flatten()
            .find_map(|&cell| cell)
            .unwrap_or(current);
    };

    let next = match direction {
        Direction::Left => scan_row(&grid[row], col, -1),
        Direction::Right => scan_row(&grid[row], col, 1),
        Direction::Up => scan_column(grid, row, col, -1),
        Direction::Down => scan_column(grid, row, col, 1),
    };

    next.unwrap_or(current)
}

/// Walks sideways from `col`, skipping empty cells, until a widget or the row's end.
fn scan_row(row: &FocusRow, col: usize, delta: isize) -> Option<FocusTarget> {
    let mut i = col as isize + delta;
    while let Some(cell) = usize::try_from(i).ok().and_then(|i| row.get(i)) {
        if cell.is_some() {
            return *cell;
        }
        i += delta;
    }
    None
}

/// Steps to the adjacent row and takes the widget nearest to `col` without looking
/// past it. Searching leftwards only is what keeps a vertical move inside the card
/// it started in: a shorter row clamps to its own last widget, and a row that is
/// only occupied further right than `col` yields nothing at all.
fn scan_column(grid: &[FocusRow], row: usize, col: usize, delta: isize) -> Option<FocusTarget> {
    let next = usize::try_from(row as isize + delta).ok()?;
    grid.get(next)?
        .iter()
        .take(col + 1)
        .rev()
        .find_map(|&cell| cell)
}

fn step_focus(list: &[FocusTarget], current: FocusTarget, delta: isize) -> FocusTarget {
    if list.is_empty() {
        return current;
    }

    match list.iter().position(|&target| target == current) {
        Some(i) => {
            let len = list.len() as isize;
            let next = (i as isize + delta).rem_euclid(len) as usize;
            list[next]
        }
        None => list[0],
    }
}

/// The modifiers that suppress a character binding.
///
/// Shift is deliberately absent. `Key::Character` already reports the shifted character,
/// so Shift has done its work by the time the key arrives and testing for it again would
/// make `?` — Shift+`/` on most layouts — unreachable. Capital letters stay unbound because
/// nothing claims `H`, not because a guard stopped the lookup.
const COMMAND_MODIFIERS: keyboard::Modifiers = keyboard::Modifiers::LOGO
    .union(keyboard::Modifiers::CTRL)
    .union(keyboard::Modifiers::ALT);

fn translate_key(key: keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Message> {
    match key.as_ref() {
        keyboard::Key::Named(Named::Escape | Named::Backspace) => Some(Message::GoBack),
        keyboard::Key::Named(Named::Tab) if modifiers.shift() => Some(Message::FocusPrevious),
        keyboard::Key::Named(Named::Tab) => Some(Message::FocusNext),
        keyboard::Key::Named(Named::Enter) => Some(Message::ActivateFocused),
        keyboard::Key::Named(Named::Space) => Some(Message::ActivateFocused),
        keyboard::Key::Named(Named::ArrowUp) => Some(Message::FocusUp),
        keyboard::Key::Named(Named::ArrowDown) => Some(Message::FocusDown),
        keyboard::Key::Named(Named::ArrowLeft) => Some(Message::FocusLeft),
        keyboard::Key::Named(Named::ArrowRight) => Some(Message::FocusRight),
        // Ahead of the general character arm below, which would otherwise bind `?` first
        // and swallow it — the guard means the compiler cannot warn about the shadowing.
        // `?` is listed here rather than in a screen's accelerators because it has to work
        // everywhere, including on screens that declare none of their own.
        keyboard::Key::Character("?") if !modifiers.intersects(COMMAND_MODIFIERS) => {
            Some(Message::ToggleHelp)
        }
        // The mirror of the arm above: that one is a character admitted despite coming
        // before the general arm, this one is a character admitted despite a command
        // modifier being held. `COMMAND_MODIFIERS` is not relaxed — exactly one key is
        // carved out of it, which is why `command_modifiers_suppress_character_keys` still
        // covers every other letter under every modifier.
        keyboard::Key::Character("k") if modifiers.contains(keyboard::Modifiers::CTRL) => {
            Some(Message::OpenSearch)
        }
        keyboard::Key::Character(c) if !modifiers.intersects(COMMAND_MODIFIERS) => character_key(c),
        _ => None,
    }
}

/// Maps an unmodified character key onto a message.
///
/// Motions win over accelerators, so `h` can never be claimed as one. Everything else that
/// is a single character is forwarded for the active screen to resolve — this function
/// cannot tell whether a character is bound, because binding is per-screen and it has no
/// screen. Multi-character payloads (dead keys, IME output) are not accelerator material
/// and stop here.
fn character_key(c: &str) -> Option<Message> {
    if let Some(motion) = vim_motion(c) {
        return Some(motion);
    }

    let mut chars = c.chars();
    let first = chars.next()?;

    chars.next().is_none().then_some(Message::Accelerate(first))
}

/// Maps the vim motion keys onto the same focus moves the arrow keys make.
fn vim_motion(c: &str) -> Option<Message> {
    match c {
        "h" => Some(Message::FocusLeft),
        "j" => Some(Message::FocusDown),
        "k" => Some(Message::FocusUp),
        "l" => Some(Message::FocusRight),
        _ => None,
    }
}

/// A key that fires a widget's action directly, skipping the walk to it.
///
/// The label is what the help overlay shows, so a new accelerator documents itself.
/// A key, the control it fires, and how the overlay names it.
///
/// The label is a `Cow` rather than a `&'static str` because three of the drill's read a
/// setting — `timer: 5:00` is no literal. It is not a `String` either: that would cost every
/// one of the fixed labels an allocation and a `.to_owned()` to serve the three that report.
/// And it cannot be a `&'a str` borrowed from `App`: the reported labels are built by
/// `format!`, so a borrow would point at a temporary owned by the function returning it —
/// `error[E0515]: cannot return value referencing temporary value`. `Cow::Borrowed` for the
/// literals, `Cow::Owned` for the three, and one type covering both.
type Accelerator = (char, FocusTarget, Cow<'static, str>);

impl App {
    /// The accelerators the current screen claims. A key absent here is inert on that
    /// screen, which is what keeps `r` from rerolling an invisible scale from the Home menu.
    ///
    /// A method rather than a free function of `&Screen`, for the reason the focus
    /// functions above are: a screen's shape is no longer decided by which screen it is.
    /// `i` on the library has to name *the next* notation button, and which one that is
    /// depends on the notation now in force.
    fn accelerators(&self) -> Vec<Accelerator> {
        accelerators_for(
            &self.screen,
            self.chord_library.notation(),
            &self.walk_the_scale,
        )
    }
}

fn accelerators_for(screen: &Screen, notation: Notation, drill: &WalkTheScale) -> Vec<Accelerator> {
    match screen {
        // Numbered by position, so the menu order is the key order. `from_digit` runs out
        // after nine, which is the point at which a menu needs more than digits anyway.
        Screen::Home => HOME_MENU
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let key = char::from_digit(index as u32 + 1, 10)?;
                Some((key, FocusTarget::HomeMenuItem(index), item.label.into()))
            })
            .collect(),
        Screen::ScaleTrainer => vec![
            ('r', FocusTarget::RerollScale, "new scale".into()),
            ('i', FocusTarget::NotationToggle, "interval notation".into()),
        ],
        // `r` means here what it means on the Scale Trainer. The rest belong to the
        // drill, and three of them report the setting they hold rather than only naming what
        // the key does, which is why the label is a `Cow` and this arm is built rather than
        // listed.
        Screen::WalkTheScale => {
            // `[` and `]` rather than the arrows: the ring still has sixteen kinds to walk on
            // this screen, and every other widget that claims the motion keys is either a focus
            // stop a Tab can leave or a list `Esc` closes. The root display is neither.
            let mut keys: Vec<Accelerator> = vec![
                ('r', FocusTarget::RerollDrillScale, "new scale".into()),
                ('[', FocusTarget::DrillRootDown, "root down".into()),
                (']', FocusTarget::DrillRootUp, "root up".into()),
            ];

            if !drill.is_running() {
                keys.push(('p', FocusTarget::RestartDrill, "another run".into()));
            }

            if drill.is_running() {
                keys.push((
                    'm',
                    FocusTarget::ScaleAnswering,
                    match drill.answering() {
                        walk_the_scale::Answering::Pressing => "answer: press".into(),
                        walk_the_scale::Answering::Playing { .. } => "answer: play".into(),
                    },
                ));
                keys.push((
                    't',
                    FocusTarget::ScaleDuration,
                    Cow::Owned(format!("timer: {}", duration_label(drill.duration()))),
                ));

                // Offered only while playing: in pressing mode the neck is the surface the
                // answer is given on, so a key that took it away would leave the drill with
                // nothing to answer with.
                if matches!(drill.answering(), walk_the_scale::Answering::Playing { .. }) {
                    keys.push((
                        'n',
                        FocusTarget::ScaleNeck,
                        if drill.neck_hidden() {
                            "neck: hidden".into()
                        } else {
                            "neck: shown".into()
                        },
                    ));
                    keys.push(('d', FocusTarget::InputDevice, "input device".into()));
                }
            }

            keys
        }
        // `r` means the same here as on the scale trainer — replace what is on screen.
        // `d` and `a` are free: `h j k l` are claimed as motions before accelerators are
        // consulted, and neither `d` nor `a` is one.
        Screen::NoteTrainer => vec![
            ('r', FocusTarget::SkipPrompt, "skip this note".into()),
            (
                'd',
                FocusTarget::DrillDirectionToggle,
                "swap direction".into(),
            ),
            ('a', FocusTarget::PoolToggle, "include accidentals".into()),
        ],
        // `r` and `d` mean here what they mean on the Note Trainer — replace what is on
        // screen, and swap which way the drill runs. There is no `a`: this screen has no
        // pool to widen.
        Screen::IntervalTrainer => vec![
            (
                'r',
                FocusTarget::SkipIntervalPrompt,
                "skip this interval".into(),
            ),
            (
                'd',
                FocusTarget::IntervalDirectionToggle,
                "swap direction".into(),
            ),
        ],
        // `r` means what it means everywhere — replace what is on screen. `d` means what
        // it means on the Tuner, which is the other screen that listens; there is no
        // direction to swap here, since an interval cannot be named by playing it.
        Screen::PlayTheInterval => vec![
            (
                'r',
                FocusTarget::SkipPlayPrompt,
                "skip this interval".into(),
            ),
            ('d', FocusTarget::InputDevice, "input device".into()),
        ],
        // `/` is the same physical key as `?`, distinguished by shift: search, and help.
        //
        // One key per mode as well as the cycling one. With three modes visible, a learner
        // who can see `fingers` should be able to reach it directly rather than pressing
        // `i` until it comes round; `i` stays because that is what it means on the scale
        // trainer, and it is the key a hand already knows.
        Screen::ChordLibrary => {
            let next = chord_library::NOTATIONS
                .iter()
                .position(|&at| at == notation)
                .map_or(0, |at| (at + 1) % chord_library::NOTATIONS.len());

            vec![
                ('/', FocusTarget::SearchBox, "search".into()),
                ('n', FocusTarget::NotationChoice(0), "note names".into()),
                ('d', FocusTarget::NotationChoice(1), "degrees".into()),
                ('f', FocusTarget::NotationChoice(2), "fingers".into()),
                (
                    'i',
                    FocusTarget::NotationChoice(next),
                    "cycle the three".into(),
                ),
                // Here for the overlay's sake. `accelerate` takes `g` and `G` before it
                // consults this table, because `gg` is two keys and a table entry is one —
                // so the target is the widget they move rather than one they activate, and
                // `gg` is named in the label because it has no row of its own.
                (
                    'G',
                    FocusTarget::ChordList,
                    "last chord, gg for the first".into(),
                ),
            ]
        }
        // `r` and `i` mean here what they mean on the Scale Trainer — replace what is on
        // screen, and switch what the dots say. There is no `d`: this screen has no direction
        // to swap, and the library's `d` is a notation it does not offer.
        Screen::Caged => vec![
            ('r', FocusTarget::RerollCagedRoot, "new root".into()),
            ('i', FocusTarget::NotationToggle, "interval notation".into()),
        ],
        // `d` is the device list. It means something different again here — the library's
        // `d` picks degrees and a trainer's swaps direction — which is the arrangement a
        // per-screen table exists to allow. `i` keeps its meaning: switch how a note is
        // written, which here is the sharp or flat name of the pitch being heard.
        Screen::Tuner => vec![
            ('d', FocusTarget::InputDevice, "input device".into()),
            ('i', FocusTarget::TunerSpellingToggle, "flat names".into()),
        ],
    }
}

/// A duration as the overlay says it: `off`, or minutes and seconds.
fn duration_label(duration: Option<Duration>) -> String {
    match duration {
        None => "off".to_owned(),
        Some(total) => {
            let seconds = total.as_secs();
            format!("{}:{:02}", seconds / 60, seconds % 60)
        }
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

/// One piece of a key hint: a word in the body font, or a keycap in `KEYCAP_FONT`.
///
/// Two fonts, so one `text` widget cannot carry both — the same split `chord_symbol` and
/// `interval_label` already make for accidentals, applied here to the modifier keys.
enum KeyPart {
    Word(&'static str),
    /// A run of keycaps rather than one, because `↑ ↓ ← →` is written as a single mark in
    /// the row and splitting it into four parts would put the row's spacing between them.
    Cap(&'static str),
    /// The space between the two ways of doing one thing, on one row.
    Gap,
}

/// The keys that mean the same thing on every screen.
///
/// Every arrow is a keycap; the rest stay words because a keycap glyph for them is either
/// unavailable — there is no Esc key in Unicode outside the symbol fonts — or no clearer
/// than the word, and every glyph here has to be paid for in the embedded subset.
///
/// Written out rather than derived from `translate_key`. These are structural and will not
/// move, so the cost of them drifting is near zero — unlike the per-screen accelerators
/// beneath them in the overlay, which grow with every feature and so are read from the same
/// declaration that dispatches them.
const NAVIGATION_KEYS: [(&[KeyPart], &str); 5] = {
    use KeyPart::{Cap, Gap, Word};

    [
        (
            &[Cap(KEYCAP_TAB), Gap, Cap(KEYCAP_BACKTAB)],
            "next / previous",
        ),
        (
            &[Cap(KEYCAP_ARROWS), Gap, Word("k j h l")],
            "move the focus ring",
        ),
        (&[Word("Enter"), Gap, Word("Space")], "activate"),
        (&[Word("Esc"), Gap, Word("Backspace")], "back"),
        (&[Word("?")], "this list"),
    ]
};

/// The help overlay's key column. A keycap is set larger than the words beside it, for the
/// reason `control_label` documents: the arrows are drawn against a maths font's optical
/// size rather than a text face's cap height.
const HELP_KEY_SIZE: f32 = 15.0;
const HELP_KEYCAP_SIZE: f32 = 17.0;
const HELP_KEY_GAP: f32 = 12.0;

/// A key hint as a row: keycaps in `KEYCAP_FONT`, everything else in the body font.
fn key_hint(parts: &'static [KeyPart]) -> Element<'static, Message> {
    use iced::widget::{Space, row, text};

    row(parts.iter().map(|part| match part {
        KeyPart::Word(word) => text(*word).size(HELP_KEY_SIZE).color(INK).into(),
        KeyPart::Cap(run) => text(*run)
            .font(KEYCAP_FONT)
            .size(HELP_KEYCAP_SIZE)
            .color(INK)
            .into(),
        KeyPart::Gap => Space::new().width(HELP_KEY_GAP).into(),
    }))
    .spacing(3)
    .align_y(iced::Alignment::Center)
    .into()
}

/// The `?` overlay: a scrim over the live screen and a card of the keys that work on it.
///
/// Takes the screen rather than reading the history, so what it lists is always what is
/// actually behind it. Nothing in here is focusable — it contributes no entry to
/// `focus_grid`, which is why the focus system needs no notion of a modal layer.
fn ui_help_overlay(claimed: Vec<Accelerator>) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{column, container, text};

    let navigation = NAVIGATION_KEYS
        .into_iter()
        .map(|(parts, description)| (key_hint(parts), Cow::Borrowed(description)));

    let has_accelerators = !claimed.is_empty();
    // Accelerators are single characters typed as themselves, so no row of them ever needs
    // a keycap — `key_hint` would be a `Word` of one letter.
    let screen_keys = claimed.into_iter().map(|(key, _, label)| {
        (
            text(key.to_string()).size(HELP_KEY_SIZE).color(INK).into(),
            label,
        )
    });

    let mut card = column![
        text("Keys").size(26).color(INK),
        help_section("Anywhere", navigation),
    ]
    .spacing(22);

    card = card.push(if has_accelerators {
        help_section("On this screen", screen_keys)
    } else {
        text("No shortcuts on this screen.")
            .size(15)
            .color(MUTE)
            .into()
    });

    let panel = container(card)
        .padding([28, 32])
        .width(440)
        .style(help_panel_container);

    container(panel)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(scrim_container)
        .into()
}

/// One labelled group of key rows in the help overlay.
fn help_section(
    title: &'static str,
    rows: impl Iterator<Item = (Element<'static, Message>, Cow<'static, str>)>,
) -> Element<'static, Message> {
    use iced::widget::{column, container, row, text};

    let entries = column(rows.map(|(keys, description)| {
        row![
            container(keys).width(150),
            text(description.into_owned())
                .size(HELP_KEY_SIZE)
                .color(BODY),
        ]
        .spacing(14)
        .into()
    }))
    .spacing(9);

    column![text(title).size(13).color(MUTE), entries]
        .spacing(11)
        .into()
}

/// The help card itself: `card_container`'s shape and chrome on the next surface up.
///
/// Cards on the screen behind already sit on `CANVAS_SOFT`, so reusing that here would put
/// the panel at the same tone as what it floats over and leave the scrim doing all the
/// separating. `CANVAS_SOFT_2` is the tone `DESIGN.md` assigns to menus and other surfaces
/// that float above cards, which is what this is.
///
/// The shadow stays a single drop because `container::Style` holds one `Shadow`; the
/// document's Level 5 modal treatment stacks three, which iced cannot express — a limit
/// every card in the app already lives with.
fn help_panel_container(theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(CANVAS_SOFT_2)),
        ..card_container(theme)
    }
}

/// The dimming layer behind the help card. Opaque enough to push the screen back without
/// hiding it, so the overlay reads as covering the page rather than replacing it.
fn scrim_container(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        text_color: Some(INK),
        background: Some(Background::Color(Color { a: 0.86, ..CANVAS })),
        border: Border::default(),
        shadow: Shadow::default(),
        snap: true,
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
    use super::*;
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
    fn app_on(screen: Screen) -> App {
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

    /// One focusable per menu entry, counted from the menu rather than written out — the
    /// same source the buttons and the digit keys come from, so adding a trainer cannot
    /// leave this asserting the old number.
    #[test]
    fn home_has_a_focusable_per_menu_entry() {
        assert_eq!(
            app_on(Screen::Home).focusables().len(),
            HOME_MENU_ITEMS,
            "the menu and its focus ring disagree"
        );
    }

    #[test]
    fn scale_trainer_reaches_every_widget_exactly_once() {
        let targets = app_on(Screen::ScaleTrainer).focusables();
        // Three pieces of chrome: Back, and the summary card's two buttons.
        assert_eq!(
            targets.len(),
            3 + PitchClass::ALL.len() + ScaleKind::ALL.len()
        );

        for i in 0..PitchClass::ALL.len() {
            assert!(targets.contains(&FocusTarget::Root(i)), "missing root {i}");
        }
        for i in 0..ScaleKind::ALL.len() {
            assert!(
                targets.contains(&FocusTarget::ScaleKind(i)),
                "missing kind {i}"
            );
        }
        assert!(targets.contains(&FocusTarget::NotationToggle));
    }

    #[test]
    fn tab_walks_one_card_at_a_time() {
        // Reading order: Back, the card's two buttons left to right, then every root
        // before any kind. Back and the buttons are on separate grid rows now, and the
        // point of this test is that the split changes nothing about the Tab order —
        // both rows are in the root card's band, so they still lead it.
        let mut expected = vec![
            FocusTarget::Back,
            FocusTarget::NotationToggle,
            FocusTarget::RerollScale,
        ];
        expected.extend((0..PitchClass::ALL.len()).map(FocusTarget::Root));
        expected.extend((0..ScaleKind::ALL.len()).map(FocusTarget::ScaleKind));

        assert_eq!(app_on(Screen::ScaleTrainer).focusables(), expected);
    }

    #[test]
    fn tab_and_arrows_agree_on_what_is_focusable() {
        // The two orders are built from the same grid; this catches a widget that
        // becomes reachable by one and not the other.
        for screen in [
            Screen::Home,
            Screen::ScaleTrainer,
            Screen::NoteTrainer,
            Screen::IntervalTrainer,
        ] {
            let mut from_tab = app_on(screen.clone()).focusables();
            let mut in_grid: Vec<_> = app_on(screen.clone())
                .focus_grid()
                .into_iter()
                .flatten()
                .flatten()
                .collect();

            assert_eq!(from_tab.len(), in_grid.len(), "{screen:?} count differs");

            from_tab.sort_by_key(|t| format!("{t:?}"));
            in_grid.sort_by_key(|t| format!("{t:?}"));
            assert_eq!(from_tab, in_grid, "{screen:?} membership differs");
        }
    }

    #[test]
    fn selector_row_spans_cover_their_arrays() {
        let roots: Vec<_> = root_row_spans().collect();
        assert_eq!(roots, vec![(0, 3), (3, 3), (6, 3), (9, 3)]);

        let kinds: Vec<_> = kind_row_spans().collect();
        assert_eq!(
            kinds,
            vec![(0, 3), (3, 3), (6, 2), (8, 2), (10, 3), (13, 3)]
        );

        let covered: usize = kinds.iter().map(|&(_, len)| len).sum();
        assert_eq!(covered, ScaleKind::ALL.len());
    }

    #[test]
    fn step_focus_wraps_forward_past_end() {
        let list = app_on(Screen::Home).focusables();
        let last = *list.last().unwrap();
        assert_eq!(step_focus(&list, last, 1), list[0]);
    }

    #[test]
    fn step_focus_wraps_backward_past_start() {
        let list = app_on(Screen::Home).focusables();
        let last = *list.last().unwrap();
        assert_eq!(step_focus(&list, list[0], -1), last);
    }

    #[test]
    fn step_focus_snaps_to_first_when_target_absent() {
        let list = app_on(Screen::Home).focusables();
        // Back is not focusable on Home, so a stale focus snaps to the first item.
        assert_eq!(step_focus(&list, FocusTarget::Back, 1), list[0]);
    }

    /// Presses one arrow key on the scale trainer.
    fn arrow(from: FocusTarget, direction: Direction) -> FocusTarget {
        step_focus_2d(&app_on(Screen::ScaleTrainer).focus_grid(), from, direction)
    }

    #[test]
    fn arrows_walk_within_the_root_grid() {
        // C C# D / D# E F / F# G G# / A A# B
        assert_eq!(
            arrow(FocusTarget::Root(0), Direction::Right),
            FocusTarget::Root(1)
        );
        assert_eq!(
            arrow(FocusTarget::Root(1), Direction::Left),
            FocusTarget::Root(0)
        );
        assert_eq!(
            arrow(FocusTarget::Root(1), Direction::Down),
            FocusTarget::Root(4)
        );
        assert_eq!(
            arrow(FocusTarget::Root(4), Direction::Up),
            FocusTarget::Root(1)
        );
    }

    #[test]
    fn right_edge_of_root_grid_crosses_into_the_kinds_card() {
        // D is the last root in its row; Ionian is the first kind in the same row.
        assert_eq!(
            arrow(FocusTarget::Root(2), Direction::Right),
            FocusTarget::ScaleKind(0)
        );
        assert_eq!(
            arrow(FocusTarget::ScaleKind(0), Direction::Left),
            FocusTarget::Root(2)
        );
    }

    #[test]
    fn arrows_stop_at_the_outer_edges() {
        assert_eq!(
            arrow(FocusTarget::Root(0), Direction::Left),
            FocusTarget::Root(0)
        );
        assert_eq!(arrow(FocusTarget::Back, Direction::Up), FocusTarget::Back);

        // ScaleKind(2) ends the widest kind row, so nothing is to its right.
        assert_eq!(
            arrow(FocusTarget::ScaleKind(2), Direction::Right),
            FocusTarget::ScaleKind(2)
        );
    }

    #[test]
    fn down_past_the_root_grid_does_not_jump_into_the_taller_kinds_card() {
        // The kinds card has a fifth and sixth row, the root card does not. Leaving the
        // last root row must stay put rather than teleporting across to ScaleKind(10).
        for last_row_root in [
            FocusTarget::Root(9),
            FocusTarget::Root(10),
            FocusTarget::Root(11),
        ] {
            assert_eq!(arrow(last_row_root, Direction::Down), last_row_root);
        }
    }

    #[test]
    fn vertical_moves_clamp_into_shorter_rows() {
        // Row 2 of the kinds card holds 2 items (indices 6..8), so leaving the 3-wide
        // row above it from its last column lands on that row's last item.
        assert_eq!(
            arrow(FocusTarget::ScaleKind(5), Direction::Down),
            FocusTarget::ScaleKind(7)
        );
        // Row 3 is the same width as row 2, so the column is carried straight down
        // rather than clamped.
        assert_eq!(
            arrow(FocusTarget::ScaleKind(7), Direction::Down),
            FocusTarget::ScaleKind(9)
        );
    }

    #[test]
    fn the_card_buttons_line_up_with_the_columns_below_them() {
        // The header is two rows: Back alone, then the card's two buttons, which take
        // one column each above the first two roots. The third column of that row is
        // empty since the spelling toggle went — which is why `Root(2)` has no button
        // above it and is not in the loop below.
        assert_eq!(
            arrow(FocusTarget::Back, Direction::Down),
            FocusTarget::NotationToggle
        );
        assert_eq!(
            arrow(FocusTarget::NotationToggle, Direction::Up),
            FocusTarget::Back
        );

        for (button, root) in [
            (FocusTarget::NotationToggle, FocusTarget::Root(0)),
            (FocusTarget::RerollScale, FocusTarget::Root(1)),
        ] {
            assert_eq!(arrow(button, Direction::Down), root, "down from {button:?}");
            assert_eq!(arrow(root, Direction::Up), button, "up from {root:?}");
        }
    }

    #[test]
    fn the_card_button_row_walks_notation_reroll() {
        assert_eq!(
            arrow(FocusTarget::NotationToggle, Direction::Right),
            FocusTarget::RerollScale
        );
        assert_eq!(
            arrow(FocusTarget::RerollScale, Direction::Left),
            FocusTarget::NotationToggle
        );
        // The row's third cell is empty, so Right from the reroll button stays put
        // rather than stepping into a hole. Asserted because an empty trailing cell is
        // exactly the kind of thing a grid walk can fall into.
        assert_eq!(
            arrow(FocusTarget::RerollScale, Direction::Right),
            FocusTarget::RerollScale
        );

        // Back is alone on its row, so the horizontal keys have nowhere to take it.
        // It used to step sideways into the card; reaching the card is Tab's job now,
        // or Down's. Asserted rather than left implicit because it is the one thing
        // the two-row header gave up.
        assert_eq!(
            arrow(FocusTarget::Back, Direction::Right),
            FocusTarget::Back
        );
        assert_eq!(arrow(FocusTarget::Back, Direction::Left), FocusTarget::Back);
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

    #[test]
    fn home_menu_is_vertical_only() {
        let grid = app_on(Screen::Home).focus_grid();
        let first = FocusTarget::HomeMenuItem(0);

        assert_eq!(
            step_focus_2d(&grid, first, Direction::Down),
            FocusTarget::HomeMenuItem(1)
        );
        assert_eq!(step_focus_2d(&grid, first, Direction::Up), first);
        assert_eq!(step_focus_2d(&grid, first, Direction::Right), first);
        assert_eq!(step_focus_2d(&grid, first, Direction::Left), first);
    }

    /// Presses a character key with the given modifiers held.
    ///
    /// `c` is the *modified* key — the character the keyboard actually produces, which is
    /// what the subscription feeds `translate_key`. So Shift+/ is `press("?", SHIFT)` and
    /// Shift+r is `press("R", SHIFT)`; writing those as `"/"` or `"r"` would describe an
    /// event no keyboard sends, which is exactly how `?` shipped broken behind a passing
    /// test.
    fn press(c: &str, modifiers: keyboard::Modifiers) -> Option<Message> {
        translate_key(keyboard::Key::Character(c.into()), modifiers)
    }

    #[test]
    fn vim_motions_move_the_focus_ring() {
        let none = keyboard::Modifiers::empty();

        assert!(matches!(press("h", none), Some(Message::FocusLeft)));
        assert!(matches!(press("j", none), Some(Message::FocusDown)));
        assert!(matches!(press("k", none), Some(Message::FocusUp)));
        assert!(matches!(press("l", none), Some(Message::FocusRight)));
    }

    #[test]
    fn modified_vim_letters_are_not_motions() {
        for modifiers in [
            keyboard::Modifiers::LOGO,
            keyboard::Modifiers::CTRL,
            keyboard::Modifiers::ALT,
        ] {
            assert!(press("h", modifiers).is_none(), "{modifiers:?}+h");
        }
    }

    /// Every screen, so a new one cannot quietly escape the checks that sweep them all.
    pub(super) fn every_screen() -> [Screen; 8] {
        [
            Screen::Home,
            Screen::ScaleTrainer,
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

    #[test]
    fn command_modifiers_suppress_character_keys() {
        for modifiers in [
            keyboard::Modifiers::LOGO,
            keyboard::Modifiers::CTRL,
            keyboard::Modifiers::ALT,
        ] {
            assert!(press("r", modifiers).is_none(), "{modifiers:?}+r");
            // `k` too, everywhere except the one combination carved out below. The letter
            // is named here so the exception cannot silently widen to the whole key.
            if modifiers != keyboard::Modifiers::CTRL {
                assert!(press("k", modifiers).is_none(), "{modifiers:?}+k");
            }
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

    /// Guards the promise that scoping is structural: an accelerator can only name a widget
    /// the screen actually has, so reshaping a focus grid cannot strand one.
    #[test]
    fn every_accelerator_targets_a_widget_on_its_screen() {
        for screen in every_screen() {
            let reachable = app_on(screen.clone()).focusables();

            // Every notation, not just the default: the library's `i` names whichever
            // button comes next, so its target moves as the setting does and each of
            // those has to be a widget the screen actually has.
            for (index, &notation) in chord_library::NOTATIONS.iter().enumerate() {
                let mut app = app_on(screen.clone());
                app.chord_library.set_notation(index);

                for (key, target, label) in app.accelerators() {
                    assert!(
                        reachable.contains(&target),
                        "{screen:?} under {notation:?} binds {key:?} ({label}) to \
                         {target:?}, which is not on it"
                    );
                }
            }
        }
    }

    /// A trainer added to `HOME_MENU` must arrive with its digit already attached. Without
    /// this, a fourth entry would silently be the only one you cannot reach by number.
    #[test]
    fn every_home_menu_item_has_a_digit_accelerator() {
        let bound = accelerators_for(&Screen::Home, Notation::Notes, &WalkTheScale::new());

        assert_eq!(bound.len(), HOME_MENU.len());

        for (index, item) in HOME_MENU.iter().enumerate() {
            let expected = char::from_digit(index as u32 + 1, 10).expect("menu fits in digits");

            assert_eq!(
                bound[index],
                (
                    expected,
                    FocusTarget::HomeMenuItem(index),
                    item.label.into()
                ),
                "menu item {index} ({}) is misnumbered or mislabelled",
                item.label
            );
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

    /// A keycap outside the two ranges `just fonts` passes to `--unicodes` falls through to
    /// whatever the system happens to have — the failure the subset exists to prevent, and
    /// one that shows up as a glyph at the wrong weight rather than as anything that fails.
    /// Nothing at runtime can read the font, so the ranges are written out here and the
    /// table is checked against them.
    #[test]
    fn every_mark_is_one_the_embedded_subset_carries() {
        let carried = |glyph: char| {
            matches!(
                glyph,
                '\u{2190}'..='\u{2193}' | '\u{21E4}'..='\u{21E5}'
            )
        };

        // The back arrow is drawn in the same font and so answers to the same range, even
        // though it is not a keycap and does not appear in `NAVIGATION_KEYS`.
        for glyph in KEYCAP_BACK.chars() {
            assert!(carried(glyph), "{glyph:?} is not in the subset");
        }

        let caps = NAVIGATION_KEYS
            .into_iter()
            .flat_map(|(parts, description)| parts.iter().map(move |part| (part, description)));

        for (part, description) in caps {
            let KeyPart::Cap(run) = part else { continue };

            for glyph in run.chars().filter(|glyph| !glyph.is_whitespace()) {
                assert!(
                    carried(glyph),
                    "{glyph:?} on the {description:?} row is not in the subset"
                );
            }
        }

        for glyph in KEYCAP_BACK.chars() {
            assert!(carried(glyph), "the back arrow is not in the subset");
        }
    }

    #[test]
    fn no_screen_binds_the_same_accelerator_twice() {
        for screen in every_screen() {
            let mut keys: Vec<char> =
                accelerators_for(&screen, Notation::Notes, &WalkTheScale::new())
                    .iter()
                    .map(|(k, _, _)| *k)
                    .collect();
            let count = keys.len();
            keys.sort_unstable();
            keys.dedup();

            assert_eq!(keys.len(), count, "{screen:?} binds a key twice");
        }
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

    /// Named keys that carry no character and claim no binding must translate to nothing,
    /// rather than falling through to the accelerator path.
    #[test]
    fn unbound_named_keys_produce_no_message() {
        for named in [Named::F1, Named::Home, Named::PageDown, Named::Insert] {
            assert!(
                translate_key(keyboard::Key::Named(named), keyboard::Modifiers::empty()).is_none(),
                "{named:?} is bound"
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
    fn adding_vim_motions_did_not_shadow_the_named_keys() {
        let escape = keyboard::Key::Named(Named::Escape);
        let modifiers = keyboard::Modifiers::empty();

        assert!(matches!(
            translate_key(escape, modifiers),
            Some(Message::GoBack)
        ));
    }

    #[test]
    fn arrows_snap_stale_focus_back_onto_the_grid() {
        let grid = app_on(Screen::Home).focus_grid();
        assert_eq!(
            step_focus_2d(&grid, FocusTarget::Back, Direction::Down),
            FocusTarget::HomeMenuItem(0)
        );
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
mod tuner_tests {
    use super::*;
    use crate::audio::detect::{Detection, Heard};
    use crate::music::notes::Letter;
    use crate::ui::tests::press_into;

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
                .any(|(key, target, _)| *key == '8' && *target == FocusTarget::HomeMenuItem(entry)),
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
mod tuner_device_tests {
    use super::tests::every_screen;
    use super::*;

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

    /// The invariant `App::listener` rests on: moving between the two listening screens
    /// closes one stream before the other opens, so there is never a moment with both.
    ///
    /// `leaving_for` is what enforces it, and it is asked of the screen being left before
    /// `self.screen` moves — which is the whole reason it is a separate method rather than
    /// a line inside `navigate_to`.
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

    /// Both ways out of the played drill release the device, as both ways out of the Tuner
    /// do — `Esc` and a Home navigation go through the one method that says so.
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

    /// The poll ticks on either listening screen and on none of the others, read from the
    /// same routing the messages take — so the subscription and the routing cannot
    /// disagree about which screen is listening.
    #[test]
    fn the_poll_is_live_on_the_listening_screens_and_no_others() {
        for screen in every_screen() {
            let (mut app, _) = App::new();
            app.open(screen.clone());

            let listens = matches!(screen, Screen::Tuner | Screen::PlayTheInterval);
            assert_eq!(
                app.listener_ref().is_some(),
                listens,
                "{screen:?} disagrees about whether it listens"
            );

            // With no device on the machine the stream never opens, and the poll correctly
            // does not tick — the screen says "no input" instead of appearing to listen.
            if !listens {
                assert!(!app.is_listening(), "{screen:?} is polling for audio");
            }
        }
    }

    #[test]
    fn navigating_elsewhere_puts_it_down_too() {
        // The other way out: a Home digit rather than Esc. Both paths go through
        // `leaving_for`, and this is what holds them together.
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

#[cfg(test)]
mod tuner_direction_tests {
    use super::*;
    use crate::audio::detect::{Detection, Heard};
    use crate::ui::tuner::Correction;

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
