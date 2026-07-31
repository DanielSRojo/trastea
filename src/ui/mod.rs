mod fretboard;
use std::ops::Range;

use fretboard::{Fretboard, NoteMarker, fretboard};

use iced::{
    Background, Border, Color, Element, Padding, Shadow, Subscription, Task, Vector, font, keyboard,
};
use keyboard::key::Named;

use crate::music::{
    intervals::Interval,
    notes::{Accidental, Note, PitchClass, Spelling},
    scales::Scale,
    scales::ScaleKind,
};
use crate::rng::Rng;

const INK: Color = Color::WHITE;
const BODY: Color = Color::from_rgb8(0xb5, 0xb5, 0xb5);
const MUTE: Color = Color::from_rgb8(0x77, 0x77, 0x77);
const HAIRLINE: Color = Color::from_rgb8(0x1f, 0x1f, 0x1f);
const CANVAS: Color = Color::BLACK;
const CANVAS_SOFT: Color = Color::from_rgb8(0x0a, 0x0a, 0x0a);
const CANVAS_SOFT_2: Color = Color::from_rgb8(0x11, 0x11, 0x11);
const LINK: Color = Color::from_rgb8(0x50, 0xa7, 0xff);
/// The theme's two semantic colours, named here because canvas markers and button styles
/// are built where no `&Theme` is in hand. Kept in step with `theme()` in `main.rs`; these
/// are the same literals `scale_markers` and `selected_root_button` already used inline.
const DANGER: Color = Color::from_rgb8(0xff, 0x4d, 0x4d);
const SUCCESS: Color = Color::from_rgb8(0x50, 0xe3, 0xc2);
const SUMMARY_CARD_HEIGHT: f32 = 212.0;
const ROOT_SELECTOR_CARD_WIDTH: f32 = 320.0;
const SELECTOR_CARD_HEIGHT: f32 = 324.0;
const ROOT_BUTTON_SIZE: f32 = 50.0;
const SMUFL_FLAT: char = '\u{E260}';
const SMUFL_SHARP: char = '\u{E262}';
const SMUFL_DOUBLE_SHARP: char = '\u{E263}';
const SMUFL_DOUBLE_FLAT: char = '\u{E264}';
const FEEL_FONT: iced::Font = iced::Font {
    family: font::Family::Name("Dancing Script"),
    weight: font::Weight::Bold,
    ..iced::Font::DEFAULT
};
const MUSIC_FONT: iced::Font = iced::Font::with_name("Leland Text");

const STANDARD_TUNING: [PitchClass; 6] = [
    PitchClass::new(4),  // E
    PitchClass::new(9),  // A
    PitchClass::new(2),  // D
    PitchClass::new(7),  // G
    PitchClass::new(11), // B
    PitchClass::new(4),  // e
];

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
}

#[derive(Default, Debug, Clone, PartialEq)]
pub enum Screen {
    #[default]
    Home,
    ScaleTrainer,
    NoteTrainer,
    IntervalTrainer,
}

/// What the Note Trainer is asking right now.
///
/// The variant *is* the drill direction — there is deliberately no separate `direction`
/// field. Two encodings of one fact could disagree, and a `FindIt` prompt on screen while
/// a `direction` field said `NameIt` is a state the view would render as nonsense. Here
/// each payload is reachable only through the variant that owns it, so the mismatch does
/// not exist to be guarded against.
///
/// The two directions carry genuinely different payloads, which is the other half of why
/// this is an enum rather than a struct with a tag: *Name it* knows a position and wants a
/// pitch class, *Find it* knows a pitch class and wants a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prompt {
    /// A lit position; the answer names the note on it.
    NameIt { string: usize, fret: usize },
    /// A named note; the answer is any position carrying it.
    FindIt(PitchClass),
}

/// What the user offered in reply to a `Prompt`.
///
/// Shaped like `Prompt` because the two answer surfaces mirror the two directions. A
/// mismatched pair is a wiring bug rather than something a user can produce, since the
/// view only ever draws the surface the current prompt can accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    Name(PitchClass),
    Position { string: usize, fret: usize },
}

/// Which pitch classes prompts are drawn from.
///
/// Naturals first: it is a smaller map, and it has to be automatic before the accidentals
/// between its notes mean anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pool {
    Naturals,
    All,
}

impl Pool {
    fn pitch_classes(self) -> &'static [PitchClass] {
        match self {
            Pool::Naturals => &PitchClass::NATURALS,
            Pool::All => &PitchClass::ALL,
        }
    }

    fn contains(self, pitch_class: PitchClass) -> bool {
        self.pitch_classes().contains(&pitch_class)
    }
}

/// Which way the drill is running. Transient only — passed to `draw_prompt` to say which
/// kind of prompt to make, never stored. The stored direction is `Prompt`'s variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drill {
    NameIt,
    FindIt,
}

impl Prompt {
    fn drill(self) -> Drill {
        match self {
            Prompt::NameIt { .. } => Drill::NameIt,
            Prompt::FindIt(_) => Drill::FindIt,
        }
    }
}

impl Drill {
    fn flipped(self) -> Drill {
        match self {
            Drill::NameIt => Drill::FindIt,
            Drill::FindIt => Drill::NameIt,
        }
    }
}

/// Where the keyboard cursor starts: the open low E, the neck's top-left corner.
const CURSOR_HOME: (usize, usize) = (0, 0);
const NECK_STRINGS: usize = STANDARD_TUNING.len();
const NECK_FRETS: usize = 12;

/// The note at a position in standard tuning, or `None` off the neck.
///
/// `Option` rather than an index that panics: the callers are a click and a cursor, and a
/// bounds bug should misjudge one answer rather than take the process down.
fn pitch_class_at(string: usize, fret: usize) -> Option<PitchClass> {
    (fret <= NECK_FRETS)
        .then(|| STANDARD_TUNING.get(string))
        .flatten()
        .map(|open| open.transpose(fret as u8))
}

/// The Note Trainer's whole state.
///
/// No iced types here on purpose: the drill is a state machine, and keeping it one means
/// the streak rules and the judging can be tested without a widget tree or a renderer.
struct NoteTrainer {
    prompt: Prompt,
    pool: Pool,
    /// How this screen writes note names. Its own, not the scale trainer's — a spelling
    /// chosen while practising scales has no business renaming the answer buttons here.
    spelling: Spelling,
    streak: u32,
    best_streak: u32,
    /// Every wrong answer given to the current prompt, cleared when it advances.
    ///
    /// A `Vec`, not an `Option`: the learner may keep trying, and replacing the single
    /// last wrong answer would un-mark the earlier ones, which reads as a bug rather than
    /// as feedback.
    wrong: Vec<Answer>,
    cursor: (usize, usize),
}

impl NoteTrainer {
    /// Note the `&mut Rng` on this and on everything that redraws a prompt.
    ///
    /// The generator lives on `App` beside this struct, and a method here cannot reach it:
    /// `&mut self.note_trainer` and `&mut self.rng` are two mutable borrows of one `App`.
    /// Passing it in is the idiomatic answer, and it is also what keeps the drill seedable
    /// from the tests. A second generator of its own would mean a second seed.
    fn new(rng: &mut Rng) -> Self {
        let mut trainer = Self {
            // Replaced by `draw_prompt` before anyone sees it; it exists only to give the
            // rejection loop something to differ from. There is no `Default` for the same
            // reason `App` has none — a constant first prompt would make every launch open
            // on the same note.
            prompt: Prompt::NameIt { string: 0, fret: 0 },
            pool: Pool::Naturals,
            spelling: Spelling::Sharps,
            streak: 0,
            best_streak: 0,
            wrong: Vec::new(),
            cursor: CURSOR_HOME,
        };

        trainer.draw_prompt(Drill::NameIt, rng);
        trainer
    }

    /// Every position on the neck whose note is in the current pool.
    ///
    /// Allocates, which is fine at the once-per-prompt rate a human answers at.
    fn positions(&self) -> Vec<(usize, usize)> {
        let mut positions = Vec::new();

        for string in 0..NECK_STRINGS {
            for fret in 0..=NECK_FRETS {
                if pitch_class_at(string, fret).is_some_and(|pc| self.pool.contains(pc)) {
                    positions.push((string, fret));
                }
            }
        }

        positions
    }

    /// Draws a fresh prompt of `drill`, never the one already showing.
    ///
    /// Same rejection loop as `reroll_scale`. It terminates only because every pool holds
    /// at least two distinct prompts — 7 naturals at the smallest — so the assertion below
    /// is load-bearing rather than decorative: a future "drill one note" setting would
    /// turn this into a hang, and a hang inside a rejection loop is a miserable bug to
    /// find.
    fn draw_prompt(&mut self, drill: Drill, rng: &mut Rng) {
        let current = self.prompt;

        loop {
            let candidate = match drill {
                Drill::NameIt => {
                    let positions = self.positions();
                    debug_assert!(
                        positions.len() >= 2,
                        "a pool must hold at least two positions or this loop cannot end"
                    );

                    let (string, fret) = positions[rng.below(positions.len())];
                    Prompt::NameIt { string, fret }
                }
                Drill::FindIt => {
                    let choices = self.pool.pitch_classes();
                    debug_assert!(
                        choices.len() >= 2,
                        "a pool must hold at least two pitch classes or this loop cannot end"
                    );

                    Prompt::FindIt(choices[rng.below(choices.len())])
                }
            };

            if candidate != current {
                self.prompt = candidate;
                break;
            }
        }

        self.wrong.clear();
    }

    /// Judges by pitch class, so the two names of a black key are one answer.
    fn judge(&self, answer: Answer) -> bool {
        match (self.prompt, answer) {
            (Prompt::NameIt { string, fret }, Answer::Name(named)) => {
                pitch_class_at(string, fret) == Some(named)
            }
            // Any position carrying the note counts: a note really is in seven places
            // within twelve frets, and none of them is more correct than another.
            (Prompt::FindIt(target), Answer::Position { string, fret }) => {
                pitch_class_at(string, fret) == Some(target)
            }
            // A mismatched pair is a wiring bug, not something a user can produce — the
            // view only draws the surface the current prompt accepts. `false` rather than
            // a panic, so the symptom is "every answer is wrong" instead of a crash.
            _ => false,
        }
    }

    fn answer(&mut self, answer: Answer, rng: &mut Rng) {
        if self.judge(answer) {
            self.streak += 1;
            self.best_streak = self.best_streak.max(self.streak);

            let drill = self.prompt.drill();
            self.draw_prompt(drill, rng);
        } else {
            // Deduplicated so hammering one wrong button cannot grow this without bound.
            if !self.wrong.contains(&answer) {
                self.wrong.push(answer);
            }
            self.streak = 0;
        }
    }

    /// Zeroes the run without touching the best of the session.
    ///
    /// A wrong answer, a skip, and a settings change all break it for one reason: a streak
    /// that counted across them would not measure recall. Skips included, so that skipping
    /// past the notes one does not know cannot inflate the number.
    fn break_streak(&mut self) {
        self.streak = 0;
    }

    fn skip(&mut self, rng: &mut Rng) {
        let drill = self.prompt.drill();
        self.break_streak();
        self.draw_prompt(drill, rng);
    }

    fn toggle_direction(&mut self, rng: &mut Rng) {
        let flipped = self.prompt.drill().flipped();
        self.break_streak();
        self.cursor = CURSOR_HOME;
        self.draw_prompt(flipped, rng);
    }

    fn toggle_pool(&mut self, rng: &mut Rng) {
        self.pool = match self.pool {
            Pool::Naturals => Pool::All,
            Pool::All => Pool::Naturals,
        };

        let drill = self.prompt.drill();
        self.break_streak();
        self.draw_prompt(drill, rng);
    }

    /// Cosmetic, unlike every other toggle here: it renames what is on screen and changes
    /// nothing about the drill. Judging compares pitch classes, so it cannot even change
    /// whether an answer is right.
    fn toggle_spelling(&mut self) {
        self.spelling = match self.spelling {
            Spelling::Sharps => Spelling::Flats,
            Spelling::Flats => Spelling::Sharps,
        };
    }

    /// Opening the screen: the settings and the best streak persist, the run does not.
    fn enter(&mut self, rng: &mut Rng) {
        let drill = self.prompt.drill();
        self.break_streak();
        self.cursor = CURSOR_HOME;
        self.draw_prompt(drill, rng);
    }

    /// Walks the cursor one position, stopping at the neck's edges rather than wrapping —
    /// the same way the focus ring already behaves at the edge of a grid.
    ///
    /// Up is towards the nut, because the neck is drawn with the nut at the top.
    fn move_cursor(&mut self, direction: Direction) {
        let (string, fret) = self.cursor;

        self.cursor = match direction {
            Direction::Left => (string.saturating_sub(1), fret),
            Direction::Right => ((string + 1).min(NECK_STRINGS - 1), fret),
            Direction::Up => (string, fret.saturating_sub(1)),
            Direction::Down => (string, (fret + 1).min(NECK_FRETS)),
        };
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Screen),
    GoBack,
    SelectRoot(PitchClass),
    SelectScaleKind(ScaleKind),
    ToggleSpelling,
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
    ToggleDrillDirection,
    TogglePool,
    /// The Note Trainer's spelling, not the scale's — see `NoteTrainer::spelling`.
    ToggleNoteSpelling,
    /// A character key that may be an accelerator on the current screen.
    ///
    /// Carries the character rather than an action because `translate_key` has no screen
    /// to look it up against; `update` resolves it, and ignores the ones nothing claims.
    Accelerate(char),
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
    SpellingToggle,
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
    /// Distinct from `SpellingToggle`, which spells the scale. These two never share state.
    NoteSpellingToggle,
    SkipPrompt,
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
const HOME_MENU: [MenuItem; 3] = [
    MenuItem {
        label: "Scale Trainer",
        caption: "Explore and learn guitar scales",
        screen: Screen::ScaleTrainer,
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
];

const HOME_MENU_ITEMS: usize = HOME_MENU.len();

/// Row shapes of the two selector grids on the scale trainer. Both the views and
/// the focus grid are built from these, so the two cannot drift out of sync.
const ROOT_ROW_WIDTH: usize = 3;
const KIND_ROW_WIDTHS: [usize; 5] = [4, 3, 2, 3, 4];

/// Columns in the Note Trainer's answer grid. Four divides the twelve pitch classes evenly
/// and matches the four header controls above them, so the columns line up.
const ANSWER_ROW_WIDTH: usize = 4;

impl App {
    pub fn new() -> (Self, Task<Message>) {
        // The generator is built before the struct, because the trainer's opening prompt is
        // drawn from it and a struct literal cannot borrow a field it is still building.
        let mut rng = Rng::from_clock();
        let note_trainer = NoteTrainer::new(&mut rng);

        let app = Self {
            screen: Screen::default(),
            history: Vec::new(),
            scale: Scale {
                root: PitchClass::new(0),
                spelling: Spelling::Sharps,
                kind: ScaleKind::Ionian,
            },
            focused: FocusTarget::HomeMenuItem(0),
            rng,
            help_open: false,
            notation: Notation::Notes,
            note_trainer,
        };

        (app, Task::none())
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
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
            Message::GoBack => self.go_back(),
            Message::SelectRoot(root) => {
                self.scale.root = root;
            }
            Message::SelectScaleKind(kind) => {
                self.scale.kind = kind;
            }
            Message::ToggleSpelling => self.toggle_spelling(),
            Message::ToggleNotation => self.toggle_notation(),
            Message::RerollScale => self.reroll_scale(),
            // Every arm below borrows `self.note_trainer` and `self.rng` at once. That is
            // allowed because they are disjoint fields — the same reason the drill's methods
            // take the generator as a parameter instead of reaching for it.
            Message::AnswerNote(pitch_class) => self
                .note_trainer
                .answer(Answer::Name(pitch_class), &mut self.rng),
            Message::ChooseNotePosition(string, fret) => {
                // The cursor follows the click, so the mouse and the keyboard never
                // disagree about where it is.
                self.note_trainer.cursor = (string, fret);
                self.note_trainer
                    .answer(Answer::Position { string, fret }, &mut self.rng);
            }
            Message::SkipPrompt => self.note_trainer.skip(&mut self.rng),
            Message::ToggleDrillDirection => self.note_trainer.toggle_direction(&mut self.rng),
            Message::TogglePool => self.note_trainer.toggle_pool(&mut self.rng),
            Message::ToggleNoteSpelling => self.note_trainer.toggle_spelling(),
            Message::Accelerate(c) => self.accelerate(c),
            Message::ToggleHelp => self.help_open = true,
            Message::FocusNext => self.cycle_focus(1),
            Message::FocusPrevious => self.cycle_focus(-1),
            Message::FocusUp => self.move_focus(Direction::Up),
            Message::FocusDown => self.move_focus(Direction::Down),
            Message::FocusLeft => self.move_focus(Direction::Left),
            Message::FocusRight => self.move_focus(Direction::Right),
            Message::ActivateFocused => self.activate_focused(),
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

        self.navigate_to(screen);

        if wants_fresh_scale {
            self.reroll_scale();
        }

        // Same rule as the scale trainer's: the screen never reopens on what it last
        // showed. Settings and the best streak persist; the run does not.
        if wants_fresh_prompt {
            self.note_trainer.enter(&mut self.rng);
        }
    }

    fn navigate_to(&mut self, screen: Screen) {
        self.history.push(self.screen.clone());
        self.screen = screen;
        self.reset_focus();
    }

    fn go_back(&mut self) {
        if let Some(prev) = self.history.pop() {
            self.screen = prev;
            self.reset_focus();
        }
    }

    /// Picks a new scale, never the one already on screen.
    ///
    /// Root and kind are drawn from the same advancing stream, so the second draw
    /// cannot be a function of the first. Rerolling onto the current scale would
    /// make the button look broken, so that one outcome is rejected and redrawn —
    /// which is also why the very first scale of a session is never C Ionian.
    fn reroll_scale(&mut self) {
        let current = (self.scale.root, self.scale.kind);

        loop {
            let root = PitchClass::ALL[self.rng.below(PitchClass::ALL.len())];
            let kind = ScaleKind::ALL[self.rng.below(ScaleKind::ALL.len())];

            if (root, kind) != current {
                self.scale.root = root;
                self.scale.kind = kind;
                return;
            }
        }
    }

    /// The toggle affects only the root. Every other degree follows from
    /// letter-walking, so F Ionian yields B♭ either way — but A♯ Ionian with its
    /// three double sharps becomes the clean B♭ Ionian.
    /// Flips what the markers say. Nothing about the scale moves — see `Notation`.
    fn toggle_notation(&mut self) {
        self.notation = match self.notation {
            Notation::Notes => Notation::Intervals,
            Notation::Intervals => Notation::Notes,
        };
    }

    fn toggle_spelling(&mut self) {
        self.scale.spelling = match self.scale.spelling {
            Spelling::Sharps => Spelling::Flats,
            Spelling::Flats => Spelling::Sharps,
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
        if self.focused == FocusTarget::Fretboard {
            self.note_trainer.move_cursor(direction);
            return;
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
        let target = accelerators(&self.screen)
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
            FocusTarget::SpellingToggle => self.toggle_spelling(),
            FocusTarget::NotationToggle => self.toggle_notation(),
            FocusTarget::RerollScale => self.reroll_scale(),
            FocusTarget::Root(index) => {
                if let Some(&pitch_class) = PitchClass::ALL.get(index) {
                    self.scale.root = pitch_class;
                }
            }
            FocusTarget::ScaleKind(index) => {
                if let Some(&kind) = ScaleKind::ALL.get(index) {
                    self.scale.kind = kind;
                }
            }
            // Enter on the neck answers with wherever the cursor is sitting, which is the
            // same thing a click on that position would send.
            FocusTarget::Fretboard => {
                let (string, fret) = self.note_trainer.cursor;
                self.note_trainer
                    .answer(Answer::Position { string, fret }, &mut self.rng);
            }
            FocusTarget::NoteAnswer(index) => {
                if let Some(&pitch_class) = PitchClass::ALL.get(index) {
                    self.note_trainer
                        .answer(Answer::Name(pitch_class), &mut self.rng);
                }
            }
            FocusTarget::DrillDirectionToggle => self.note_trainer.toggle_direction(&mut self.rng),
            FocusTarget::PoolToggle => self.note_trainer.toggle_pool(&mut self.rng),
            FocusTarget::NoteSpellingToggle => self.note_trainer.toggle_spelling(),
            FocusTarget::SkipPrompt => self.note_trainer.skip(&mut self.rng),
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
                ui_placeholder("Interval Trainer"),
                true,
                self.focused,
            ),
        };

        if self.help_open {
            iced::widget::stack![screen, ui_help_overlay(&self.screen)].into()
        } else {
            screen
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        keyboard::listen().filter_map(|event| {
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
            translate_key(modified_key, modifiers)
        })
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
            Screen::Home => (0..HOME_MENU_ITEMS)
                .map(|i| vec![Some(FocusTarget::HomeMenuItem(i))])
                .collect(),
            Screen::ScaleTrainer => {
                // Two header rows. The summary card's buttons — spelling, notation, reroll —
                // sit along its right edge, and the card is as wide as the root card below
                // it, so the three of them take the three columns above the first three
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
                for (cell, target) in card_buttons.iter_mut().zip([
                    FocusTarget::SpellingToggle,
                    FocusTarget::NotationToggle,
                    FocusTarget::RerollScale,
                ]) {
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
                match self.note_trainer.prompt {
                    Prompt::NameIt { .. } => {
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
                    Prompt::FindIt(_) => {
                        let mut row: FocusRow = vec![None; ANSWER_ROW_WIDTH];
                        row[0] = Some(FocusTarget::Fretboard);
                        grid.push(row);
                    }
                }

                grid
            }
            Screen::IntervalTrainer => vec![vec![Some(FocusTarget::Back)]],
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
            // These screens have a single card, so one band spans the whole width.
            Screen::Home | Screen::NoteTrainer | Screen::IntervalTrainer => vec![0..width],
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
type Accelerator = (char, FocusTarget, &'static str);

/// The accelerators a screen claims. A key absent here is inert on that screen, which is
/// what keeps `r` from rerolling an invisible scale from the Home menu.
fn accelerators(screen: &Screen) -> Vec<Accelerator> {
    match screen {
        // Numbered by position, so the menu order is the key order. `from_digit` runs out
        // after nine, which is the point at which a menu needs more than digits anyway.
        Screen::Home => HOME_MENU
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let key = char::from_digit(index as u32 + 1, 10)?;
                Some((key, FocusTarget::HomeMenuItem(index), item.label))
            })
            .collect(),
        Screen::ScaleTrainer => vec![
            ('r', FocusTarget::RerollScale, "new scale"),
            ('i', FocusTarget::NotationToggle, "interval notation"),
        ],
        // `r` means the same here as on the scale trainer — replace what is on screen.
        // `d` and `a` are free: `h j k l` are claimed as motions before accelerators are
        // consulted, and neither `d` nor `a` is one.
        Screen::NoteTrainer => vec![
            ('r', FocusTarget::SkipPrompt, "skip this note"),
            ('d', FocusTarget::DrillDirectionToggle, "swap direction"),
            ('a', FocusTarget::PoolToggle, "include accidentals"),
        ],
        Screen::IntervalTrainer => Vec::new(),
    }
}

fn with_top_bar(
    label: &'static str,
    content: Element<'static, Message>,
    has_back: bool,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{button, column, container, row, text};

    let back_button = focus_ring(
        button(text("←").size(18))
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

fn focus_ring<'a>(
    element: impl Into<Element<'a, Message>>,
    is_focused: bool,
) -> Element<'a, Message> {
    use iced::widget::container;

    container(element)
        .padding(3)
        .style(move |_theme: &iced::Theme| focus_ring_style(is_focused))
        .into()
}

fn focus_ring_style(is_focused: bool) -> iced::widget::container::Style {
    let color = if is_focused { LINK } else { Color::TRANSPARENT };

    iced::widget::container::Style {
        text_color: None,
        background: None,
        border: Border::default().rounded(10).width(2).color(color),
        shadow: Shadow::default(),
        snap: true,
    }
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

fn ui_scale_trainer(
    scale: Scale,
    notation: Notation,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, button, column, container, row, text};

    // Display only: no press handler and no cursor, so the neck stays the picture it has
    // always been here. `Message` is inferred from the `Element` this becomes.
    let fb = Fretboard {
        num_frets: 12,
        highlighted: scale_markers(scale, notation),
        ..Fretboard::default()
    };

    let current_scale_card = container(
        column![
            row![
                note_label(scale.root_note(), 56, INK),
                Space::new().width(Length::Fill),
                focus_ring(
                    button(
                        text(format!("{SMUFL_SHARP}{SMUFL_FLAT}"))
                            .size(20)
                            .font(MUSIC_FONT)
                    )
                    .padding([8, 12])
                    .style(ghost_button)
                    .on_press(Message::ToggleSpelling),
                    focused == FocusTarget::SpellingToggle,
                ),
                focus_ring(
                    // One degree, not a formula fragment: `1♭3` was tried first, on the
                    // grounds that it reads less like an instruction to flatten the
                    // third, and it crowded the header row — three buttons and the root
                    // label share this line. Built like the formula row it points at —
                    // the glyph in MUSIC_FONT, the digit in the body font — because one
                    // `text` cannot carry both.
                    button(
                        row![
                            text(SMUFL_FLAT.to_string()).size(20).font(MUSIC_FONT),
                            text("3").size(20),
                        ]
                        .spacing(0)
                    )
                    .padding([8, 12])
                    .style(ghost_button)
                    .on_press(Message::ToggleNotation),
                    focused == FocusTarget::NotationToggle,
                ),
                focus_ring(
                    button(text("R").size(20))
                        .padding([8, 12])
                        .style(ghost_button)
                        .on_press(Message::RerollScale),
                    focused == FocusTarget::RerollScale,
                ),
            ]
            .spacing(8),
            text(scale.kind.name()).size(34).color(INK),
            intervalic_text(scale.kind.intervals()),
        ]
        .spacing(10),
    )
    .width(Length::Fixed(ROOT_SELECTOR_CARD_WIDTH))
    .height(Length::Fixed(SUMMARY_CARD_HEIGHT))
    .padding(32)
    .style(card_container);

    let root_selector_content = container(root_row_spans().fold(
        column![].spacing(16),
        |rows, (start, len)| {
            rows.push(
                container(root_note_row(
                    &PitchClass::ALL[start..start + len],
                    scale,
                    start,
                    focused,
                ))
                .width(Length::Fill)
                .center_x(Length::Fill),
            )
        },
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill);

    let root_selector_card = container(root_selector_content)
        .width(Length::Fixed(ROOT_SELECTOR_CARD_WIDTH))
        .height(Length::Fixed(SELECTOR_CARD_HEIGHT))
        .padding(32)
        .style(card_container);

    let scale_selector_content = container(kind_row_spans().fold(
        column![].spacing(12),
        |rows, (start, len)| {
            rows.push(scale_kind_row(
                &ScaleKind::ALL[start..start + len],
                scale.kind,
                start,
                focused,
            ))
        },
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .center_y(Length::Fill);

    let scale_selector_card = container(scale_selector_content)
        .width(Length::Fill)
        .height(Length::Fixed(SELECTOR_CARD_HEIGHT))
        .padding(32)
        .style(card_container);

    let explanation_font = iced::Font {
        family: font::Family::Cursive,
        style: font::Style::Italic,
        ..iced::Font::DEFAULT
    };

    let explanation_card = container(
        column![
            text(scale.kind.feel())
                .size(22)
                .font(FEEL_FONT)
                .color(BODY)
                .width(Length::Fill),
            text(scale.kind.common_usage())
                .size(18)
                .font(explanation_font)
                .color(MUTE)
                .width(Length::Fill),
        ]
        .spacing(12),
    )
    .width(Length::Fill)
    .height(Length::Fixed(SUMMARY_CARD_HEIGHT))
    .padding(32)
    .style(card_container);

    let summary_cards = row![current_scale_card, explanation_card]
        .width(Length::Fill)
        .spacing(16);

    let selector_cards = row![root_selector_card, scale_selector_card]
        .width(Length::Fill)
        .spacing(16);

    let details = column![summary_cards, selector_cards]
        .width(Length::Fill)
        .spacing(16);

    container(row![fretboard(fb), details].spacing(32))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: 24.0,
            right: 64.0,
            bottom: 48.0,
            left: 64.0,
        })
        .center_y(Length::Fill)
        .into()
}

/// Each button names a *candidate* root via `scale.spelling.spell`, not a member
/// of the current scale via `Scale::spell` — so under `Spelling::Sharps` with
/// `F Ionian` on screen, this row's button for pitch class 10 reads `A♯` while
/// the fretboard (which goes through `Scale::spell`) shows `Bb` for the very
/// same pitch. That mismatch is intentional, not a bug: before a root is
/// clicked there is no scale to spell it in, only the bare toggle, and there is
/// no better answer without threading per-button context through every cell.
fn root_note_row(
    pitch_classes: &[PitchClass],
    scale: Scale,
    start_index: usize,
    focused: FocusTarget,
) -> iced::widget::Row<'static, Message> {
    use iced::Length;
    use iced::widget::{button, container, row};

    pitch_classes
        .iter()
        .enumerate()
        .fold(row![].spacing(28), |row, (i, pitch_class)| {
            let is_selected = *pitch_class == scale.root;
            let color = if is_selected { CANVAS } else { INK };

            let root_button = button(
                container(note_label(scale.spelling.spell(*pitch_class), 24, color))
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
            .on_press(Message::SelectRoot(*pitch_class));

            row.push(focus_ring(
                container(root_button)
                    .width(Length::Fixed(ROOT_BUTTON_SIZE))
                    .height(Length::Fixed(ROOT_BUTTON_SIZE))
                    .center_x(Length::Fixed(ROOT_BUTTON_SIZE))
                    .center_y(Length::Fixed(ROOT_BUTTON_SIZE)),
                focused == FocusTarget::Root(start_index + i),
            ))
        })
}

fn note_label(note: Note, size: u32, color: Color) -> iced::widget::Row<'static, Message> {
    use iced::widget::{row, text};

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
                        .size(24)
                        .font(MUSIC_FONT)
                        .color(BODY),
                );
            }

            tokens.push(token.push(text(digit.to_string()).size(24).color(BODY)))
        })
}

fn scale_kind_row(
    kinds: &[ScaleKind],
    selected: ScaleKind,
    start_index: usize,
    focused: FocusTarget,
) -> iced::widget::Row<'static, Message> {
    use iced::widget::{button, row, text};

    kinds
        .iter()
        .enumerate()
        .fold(row![].spacing(8), |row, (i, kind)| {
            row.push(focus_ring(
                button(text(kind.name()).size(16))
                    .padding([8, 12])
                    .style(if *kind == selected {
                        selected_root_button
                    } else {
                        ghost_button
                    })
                    .on_press(Message::SelectScaleKind(*kind)),
                focused == FocusTarget::ScaleKind(start_index + i),
            ))
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
        Notation::Intervals => degree.to_string(),
    }
}

fn scale_markers(scale: Scale, notation: Notation) -> Vec<NoteMarker> {
    let mut markers = Vec::new();

    for (string, open) in STANDARD_TUNING.iter().enumerate() {
        for fret in 0_u8..=12 {
            let pitch_class = open.transpose(fret);

            // `spell` stays the membership test, so the set of markers is the same
            // in both modes and the mode cannot make a dot disappear. The degree is
            // then an `expect` rather than a second gate:
            // `degree_and_spell_agree_on_membership` in scales.rs proves a spelled
            // pitch class always has one.
            if let Some(note) = scale.spell(pitch_class) {
                let degree = scale
                    .degree(pitch_class)
                    .expect("degree_and_spell_agree_on_membership: a spelled pitch has a degree");

                markers.push(NoteMarker {
                    string,
                    fret: fret as usize,
                    label: marker_label(notation, note, degree),
                    color: if pitch_class == scale.root {
                        Color::from_rgb8(0xff, 0x4d, 0x4d)
                    } else {
                        LINK
                    },
                });
            }
        }
    }
    markers
}

/// The keys that mean the same thing on every screen.
///
/// Written out rather than derived from `translate_key`. These are structural and will not
/// move, so the cost of them drifting is near zero — unlike the per-screen accelerators
/// beneath them in the overlay, which grow with every feature and so are read from the same
/// declaration that dispatches them.
const NAVIGATION_KEYS: [(&str, &str); 5] = [
    ("Tab   ⇧Tab", "next / previous"),
    ("↑ ↓ ← →   k j h l", "move the focus ring"),
    ("Enter   Space", "activate"),
    ("Esc   ⌫", "back"),
    ("?", "this list"),
];

/// The `?` overlay: a scrim over the live screen and a card of the keys that work on it.
///
/// Takes the screen rather than reading the history, so what it lists is always what is
/// actually behind it. Nothing in here is focusable — it contributes no entry to
/// `focus_grid`, which is why the focus system needs no notion of a modal layer.
fn ui_help_overlay(screen: &Screen) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{column, container, text};

    let navigation = NAVIGATION_KEYS
        .into_iter()
        .map(|(keys, description)| (keys.to_string(), description));

    let claimed = accelerators(screen);
    let has_accelerators = !claimed.is_empty();
    let screen_keys = claimed
        .into_iter()
        .map(|(key, _, label)| (key.to_string(), label));

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
    rows: impl Iterator<Item = (String, &'static str)>,
) -> Element<'static, Message> {
    use iced::widget::{column, container, row, text};

    let entries = column(rows.map(|(keys, description)| {
        row![
            container(text(keys).size(15).color(INK)).width(150),
            text(description).size(15).color(BODY),
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

/// The Note Trainer.
///
/// Both directions share this one function, branching on the prompt's variant, because the
/// header, the streak, and the neck are common to both — only the answer surface differs.
fn ui_note_trainer(trainer: &NoteTrainer, focused: FocusTarget) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, column, container, row, text};

    let neck = match trainer.prompt {
        // The prompt itself: one dot, and deliberately unlabelled — a label would print the
        // answer inside the question.
        Prompt::NameIt { string, fret } => Fretboard {
            num_frets: NECK_FRETS,
            highlighted: vec![NoteMarker {
                string,
                fret,
                label: String::new(),
                color: LINK,
            }],
            ..Fretboard::default()
        },
        // Here the neck is the answer surface, so it takes a press handler and shows the
        // cursor. Wrong guesses stay marked on it until the prompt advances.
        Prompt::FindIt(_) => Fretboard {
            num_frets: NECK_FRETS,
            highlighted: wrong_position_markers(trainer),
            cursor: Some(trainer.cursor),
            on_press: Some(Message::ChooseNotePosition),
        },
    };

    let question: Element<'static, Message> = match trainer.prompt {
        Prompt::NameIt { .. } => column![
            text("What note is this?").size(32).color(INK),
            text("Name the lit fret").size(16).color(MUTE),
        ]
        .spacing(6)
        .into(),
        Prompt::FindIt(pitch_class) => column![
            row![
                text("Find").size(26).color(BODY),
                note_label(trainer.spelling.spell(pitch_class), 40, INK),
            ]
            .spacing(12),
            text("Press any fret that plays it").size(16).color(MUTE),
        ]
        .spacing(6)
        .into(),
    };

    let prompt_card = container(
        column![
            row![
                question,
                Space::new().width(Length::Fill),
                streak_readout(trainer),
            ],
            note_trainer_controls(trainer, focused),
        ]
        .spacing(20),
    )
    .width(Length::Fill)
    .height(Length::Fixed(SUMMARY_CARD_HEIGHT))
    .padding(32)
    .style(card_container);

    // Only *Name it* has a second card; in *Find it* the neck already is the answer surface,
    // so nothing goes here and the prompt card gets the room.
    let details = match trainer.prompt {
        Prompt::NameIt { .. } => column![prompt_card, note_answer_card(trainer, focused)],
        Prompt::FindIt(_) => column![prompt_card],
    }
    .width(Length::Fill)
    .spacing(16);

    container(row![fretboard(neck), details].spacing(32))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: 24.0,
            right: 64.0,
            bottom: 48.0,
            left: 64.0,
        })
        .center_y(Length::Fill)
        .into()
}

/// The wrong positions guessed against the current prompt, as markers on the neck.
///
/// Only `Answer::Position` guesses can appear on a neck; a `Name` guess belongs to the other
/// direction and is filtered out rather than being an error, since `wrong` is cleared
/// whenever the prompt advances and the two can never mix in practice.
fn wrong_position_markers(trainer: &NoteTrainer) -> Vec<NoteMarker> {
    trainer
        .wrong
        .iter()
        .filter_map(|answer| match *answer {
            Answer::Position { string, fret } => Some(NoteMarker {
                string,
                fret,
                label: String::new(),
                color: DANGER,
            }),
            Answer::Name(_) => None,
        })
        .collect()
}

/// The current run and the best of the session.
///
/// The live streak is drawn in the theme's success colour, which is as close to
/// acknowledging a correct answer as this screen can get: a correct answer replaces the
/// prompt at once, so any per-answer flash would need the timer the design deferred. A
/// standing streak says the same thing and keeps saying it.
fn streak_readout(trainer: &NoteTrainer) -> Element<'static, Message> {
    use iced::widget::{column, row, text};

    let stat = |label: &'static str, value: u32, color: Color| {
        column![
            text(value.to_string()).size(30).color(color),
            text(label).size(12).color(MUTE),
        ]
        .spacing(2)
    };

    let live = if trainer.streak > 0 { SUCCESS } else { MUTE };

    row![
        stat("streak", trainer.streak, live),
        stat("best", trainer.best_streak, INK),
    ]
    .spacing(28)
    .into()
}

/// The header row: direction, pool, spelling, skip.
///
/// The first two are labelled with the mode they are *currently* in rather than with what
/// pressing them would do, so the row doubles as a status line — there is nowhere else on
/// this screen that says which way the drill is running.
fn note_trainer_controls(trainer: &NoteTrainer, focused: FocusTarget) -> Element<'static, Message> {
    use iced::widget::{button, row, text};

    let label = |content: String| text(content).size(15);

    let ghost = |content: Element<'static, Message>, message: Message, is_focused: bool| {
        focus_ring(
            button(content)
                .padding([8, 14])
                .style(ghost_button)
                .on_press(message),
            is_focused,
        )
    };

    let direction = match trainer.prompt.drill() {
        Drill::NameIt => "name it",
        Drill::FindIt => "find it",
    };

    let pool = match trainer.pool {
        Pool::Naturals => "naturals",
        Pool::All => "all 12",
    };

    row![
        ghost(
            label(direction.to_owned()).into(),
            Message::ToggleDrillDirection,
            focused == FocusTarget::DrillDirectionToggle,
        ),
        ghost(
            label(pool.to_owned()).into(),
            Message::TogglePool,
            focused == FocusTarget::PoolToggle,
        ),
        ghost(
            text(format!("{SMUFL_SHARP}{SMUFL_FLAT}"))
                .size(20)
                .font(MUSIC_FONT)
                .into(),
            Message::ToggleNoteSpelling,
            focused == FocusTarget::NoteSpellingToggle,
        ),
        ghost(
            text("R").size(20).into(),
            Message::SkipPrompt,
            focused == FocusTarget::SkipPrompt,
        ),
    ]
    .spacing(8)
    .into()
}

/// The twelve answer buttons.
///
/// All twelve under either pool: narrowing them to the seven naturals would make a wrong
/// answer unreachable, and a drill you cannot fail teaches nothing.
fn note_answer_card(trainer: &NoteTrainer, focused: FocusTarget) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{column, container};

    let total = PitchClass::ALL.len();

    let rows = (0..total)
        .step_by(ANSWER_ROW_WIDTH)
        .fold(column![].spacing(16), |rows, start| {
            let len = ANSWER_ROW_WIDTH.min(total - start);

            rows.push(
                container(note_answer_row(
                    &PitchClass::ALL[start..start + len],
                    trainer,
                    start,
                    focused,
                ))
                .width(Length::Fill)
                .center_x(Length::Fill),
            )
        });

    container(
        container(rows)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fixed(SELECTOR_CARD_HEIGHT))
    .padding(32)
    .style(card_container)
    .into()
}

fn note_answer_row(
    pitch_classes: &[PitchClass],
    trainer: &NoteTrainer,
    start_index: usize,
    focused: FocusTarget,
) -> iced::widget::Row<'static, Message> {
    use iced::Length;
    use iced::widget::{button, container, row};

    pitch_classes
        .iter()
        .enumerate()
        .fold(row![].spacing(20), |acc, (i, &pitch_class)| {
            let was_wrong = trainer.wrong.contains(&Answer::Name(pitch_class));
            let color = if was_wrong { CANVAS } else { INK };

            let answer_button = button(
                container(note_label(trainer.spelling.spell(pitch_class), 24, color))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            )
            .width(Length::Fixed(ROOT_BUTTON_SIZE))
            .height(Length::Fixed(ROOT_BUTTON_SIZE))
            .padding(0)
            .style(if was_wrong {
                wrong_answer_button
            } else {
                ghost_button
            })
            .on_press(Message::AnswerNote(pitch_class));

            acc.push(focus_ring(
                answer_button,
                focused == FocusTarget::NoteAnswer(start_index + i),
            ))
        })
}

fn ui_placeholder(label: &str) -> Element<'_, Message> {
    use iced::Length;
    use iced::widget::{column, container, text};

    container(
        column![
            text(label).size(36).color(INK),
            text("Coming soon").size(17).color(MUTE),
        ]
        .spacing(8),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(page_container)
    .into()
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

fn page_container(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        text_color: Some(INK),
        background: Some(Background::Color(CANVAS)),
        border: Border::default(),
        shadow: Shadow::default(),
        snap: true,
    }
}

fn card_container(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        text_color: Some(INK),
        background: Some(Background::Color(CANVAS_SOFT)),
        border: Border::default().rounded(12).width(1).color(HAIRLINE),
        shadow: Shadow {
            color: Color {
                a: 0.45,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 12.0),
            blur_radius: 32.0,
        },
        snap: true,
    }
}

fn card_button(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let border_color = match status {
        iced::widget::button::Status::Hovered => LINK,
        _ => HAIRLINE,
    };

    iced::widget::button::Style {
        background: Some(Background::Color(CANVAS_SOFT)),
        text_color: INK,
        border: Border::default().rounded(12).width(1).color(border_color),
        shadow: Shadow {
            color: Color {
                a: 0.40,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
        snap: true,
    }
}

fn ghost_button(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let background = match status {
        iced::widget::button::Status::Hovered => CANVAS_SOFT_2,
        _ => CANVAS,
    };

    iced::widget::button::Style {
        background: Some(Background::Color(background)),
        text_color: INK,
        border: Border::default().rounded(64).width(1).color(HAIRLINE),
        shadow: Shadow::default(),
        snap: true,
    }
}

fn selected_root_button(
    _theme: &iced::Theme,
    _status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    iced::widget::button::Style {
        background: Some(Background::Color(SUCCESS)),
        text_color: CANVAS,
        border: Border::default().rounded(64).width(1).color(SUCCESS),
        shadow: Shadow::default(),
        snap: true,
    }
}

/// An answer button the learner has already tried and got wrong.
///
/// Filled rather than outlined, so that at a glance the buttons still to try are the empty
/// ones — the same read as the selected root, in the opposite colour.
fn wrong_answer_button(
    _theme: &iced::Theme,
    _status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    iced::widget::button::Style {
        background: Some(Background::Color(DANGER)),
        text_color: CANVAS,
        border: Border::default().rounded(64).width(1).color(DANGER),
        shadow: Shadow::default(),
        snap: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// An app whose scale rolls are reproducible.
    fn app_with_seed(seed: u64) -> App {
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
            .position(|pitch_class| *pitch_class == app.scale.root)
            .expect("root is one of PitchClass::ALL");
        let kind = ScaleKind::ALL
            .iter()
            .position(|k| *k == app.scale.kind)
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
            let before = (app.scale.root, app.scale.kind);
            app.reroll_scale();
            assert_ne!((app.scale.root, app.scale.kind), before);
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
    fn home_has_three_focusables() {
        assert_eq!(app_on(Screen::Home).focusables().len(), 3);
    }

    #[test]
    fn scale_trainer_reaches_every_widget_exactly_once() {
        let targets = app_on(Screen::ScaleTrainer).focusables();
        // Four pieces of chrome: Back, and the summary card's three buttons.
        assert_eq!(
            targets.len(),
            4 + PitchClass::ALL.len() + ScaleKind::ALL.len()
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
        assert!(targets.contains(&FocusTarget::SpellingToggle));
        assert!(targets.contains(&FocusTarget::NotationToggle));
    }

    #[test]
    fn tab_walks_one_card_at_a_time() {
        // Reading order: Back, the card's three buttons left to right, then every root
        // before any kind. Back and the buttons are on separate grid rows now, and the
        // point of this test is that the split changes nothing about the Tab order —
        // both rows are in the root card's band, so they still lead it.
        let mut expected = vec![
            FocusTarget::Back,
            FocusTarget::SpellingToggle,
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

    /// The Note Trainer used to be listed here. It has a screen now, so the interval
    /// trainer is the last placeholder left.
    #[test]
    fn placeholder_screens_only_focus_back() {
        assert_eq!(
            app_on(Screen::IntervalTrainer).focusables(),
            vec![FocusTarget::Back]
        );
    }

    #[test]
    fn selector_row_spans_cover_their_arrays() {
        let roots: Vec<_> = root_row_spans().collect();
        assert_eq!(roots, vec![(0, 3), (3, 3), (6, 3), (9, 3)]);

        let kinds: Vec<_> = kind_row_spans().collect();
        assert_eq!(kinds, vec![(0, 4), (4, 3), (7, 2), (9, 3), (12, 4)]);

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

        // ScaleKind(3) ends the widest kind row, so nothing is to its right.
        assert_eq!(
            arrow(FocusTarget::ScaleKind(3), Direction::Right),
            FocusTarget::ScaleKind(3)
        );
    }

    #[test]
    fn down_past_the_root_grid_does_not_jump_into_the_taller_kinds_card() {
        // The kinds card has a fifth row, the root card does not. Leaving the last
        // root row must stay put rather than teleporting across to ScaleKind(12).
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
        // Row 1 of the kinds card holds 3 items (indices 4..7), so leaving the 4-wide
        // row above it from its last column lands on that row's last item.
        assert_eq!(
            arrow(FocusTarget::ScaleKind(3), Direction::Down),
            FocusTarget::ScaleKind(6)
        );
        // Row 2 is narrower still (indices 7..9).
        assert_eq!(
            arrow(FocusTarget::ScaleKind(6), Direction::Down),
            FocusTarget::ScaleKind(8)
        );
    }

    #[test]
    fn the_card_buttons_line_up_with_the_columns_below_them() {
        // The header is two rows: Back alone, then the card's three buttons, which take
        // one column each above the first three roots.
        assert_eq!(
            arrow(FocusTarget::Back, Direction::Down),
            FocusTarget::SpellingToggle
        );
        assert_eq!(
            arrow(FocusTarget::SpellingToggle, Direction::Up),
            FocusTarget::Back
        );

        for (button, root) in [
            (FocusTarget::SpellingToggle, FocusTarget::Root(0)),
            (FocusTarget::NotationToggle, FocusTarget::Root(1)),
            (FocusTarget::RerollScale, FocusTarget::Root(2)),
        ] {
            assert_eq!(arrow(button, Direction::Down), root, "down from {button:?}");
            assert_eq!(arrow(root, Direction::Up), button, "up from {root:?}");
        }
    }

    #[test]
    fn the_card_button_row_walks_spelling_notation_reroll() {
        assert_eq!(
            arrow(FocusTarget::SpellingToggle, Direction::Right),
            FocusTarget::NotationToggle
        );
        assert_eq!(
            arrow(FocusTarget::NotationToggle, Direction::Right),
            FocusTarget::RerollScale
        );
        assert_eq!(
            arrow(FocusTarget::RerollScale, Direction::Left),
            FocusTarget::NotationToggle
        );
        assert_eq!(
            arrow(FocusTarget::NotationToggle, Direction::Left),
            FocusTarget::SpellingToggle
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

    #[test]
    fn toggling_spelling_renames_the_scale_without_moving_it() {
        let mut app = app_with_seed(11);
        app.scale.root = PitchClass::new(1);
        app.scale.kind = ScaleKind::Ionian;

        let before = app.scale.notes();
        // Through update, so the message wiring is covered too. The returned
        // Task is discarded — this screen issues none.
        let _ = app.update(Message::ToggleSpelling);
        let after = app.scale.notes();

        assert_ne!(before, after, "C♯ and D♭ Ionian are spelled differently");

        let pitch_classes = |notes: &[Note]| -> Vec<u8> {
            notes
                .iter()
                .map(|note| note.pitch_class().semitone())
                .collect()
        };
        assert_eq!(
            pitch_classes(&before),
            pitch_classes(&after),
            "the toggle moved the scale"
        );
    }

    #[test]
    fn rerolling_leaves_the_spelling_alone() {
        // Spelling is a user setting, not part of the draw.
        let mut app = app_with_seed(3);
        app.scale.spelling = Spelling::Flats;

        for _ in 0..200 {
            app.reroll_scale();
            assert_eq!(app.scale.spelling, Spelling::Flats);
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
    fn every_screen() -> [Screen; 4] {
        [
            Screen::Home,
            Screen::ScaleTrainer,
            Screen::NoteTrainer,
            Screen::IntervalTrainer,
        ]
    }

    /// The state a keypress could disturb.
    ///
    /// `translate_key` no longer answers "is this key bound?" — it forwards every
    /// unrecognised character for the screen to resolve — so the absence of a binding is
    /// only observable here, as the absence of a change.
    fn snapshot(
        app: &App,
    ) -> (
        Screen,
        FocusTarget,
        PitchClass,
        ScaleKind,
        Spelling,
        Notation,
        usize,
    ) {
        (
            app.screen.clone(),
            app.focused,
            app.scale.root,
            app.scale.kind,
            app.scale.spelling,
            // Without this field the tests below would watch `i` do nothing and call
            // that a pass.
            app.notation,
            app.history.len(),
        )
    }

    /// Presses a character key and lets the app act on whatever it translates to.
    fn press_into(app: &mut App, c: &str, modifiers: keyboard::Modifiers) {
        if let Some(message) = press(c, modifiers) {
            let _ = app.update(message);
        }
    }

    /// Shift+h delivers the capital, not the lowercase letter with a flag set. An earlier
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
        }
    }

    #[test]
    fn r_rerolls_the_scale_from_the_scale_trainer() {
        let mut app = app_with_seed(0x5eed);
        app.navigate_to(Screen::ScaleTrainer);

        for _ in 0..50 {
            let before = (app.scale.root, app.scale.kind);
            press_into(&mut app, "r", keyboard::Modifiers::empty());
            assert_ne!((app.scale.root, app.scale.kind), before);
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
            if screen == Screen::ScaleTrainer {
                continue;
            }

            let mut app = app_with_seed(0xfeed);
            app.navigate_to(screen);

            let before = snapshot(&app);
            press_into(&mut app, "i", keyboard::Modifiers::empty());

            assert_eq!(snapshot(&app), before, "{:?} claimed i", app.screen);
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
            (pressed.scale.root, pressed.scale.kind),
            (untouched.scale.root, untouched.scale.kind),
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

    /// Guards the promise that scoping is structural: an accelerator can only name a widget
    /// the screen actually has, so reshaping a focus grid cannot strand one.
    #[test]
    fn every_accelerator_targets_a_widget_on_its_screen() {
        for screen in every_screen() {
            let reachable = app_on(screen.clone()).focusables();

            for (key, target, label) in accelerators(&screen) {
                assert!(
                    reachable.contains(&target),
                    "{screen:?} binds {key:?} ({label}) to {target:?}, which is not on it"
                );
            }
        }
    }

    /// A trainer added to `HOME_MENU` must arrive with its digit already attached. Without
    /// this, a fourth entry would silently be the only one you cannot reach by number.
    #[test]
    fn every_home_menu_item_has_a_digit_accelerator() {
        let bound = accelerators(&Screen::Home);

        assert_eq!(bound.len(), HOME_MENU.len());

        for (index, item) in HOME_MENU.iter().enumerate() {
            let expected = char::from_digit(index as u32 + 1, 10).expect("menu fits in digits");

            assert_eq!(
                bound[index],
                (expected, FocusTarget::HomeMenuItem(index), item.label),
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
            assert_eq!(
                app.focused,
                app_on(item.screen.clone()).focusables()[0],
                "{key} did not reset focus"
            );
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

    #[test]
    fn no_screen_binds_the_same_accelerator_twice() {
        for screen in every_screen() {
            let mut keys: Vec<char> = accelerators(&screen).iter().map(|(k, _, _)| *k).collect();
            let count = keys.len();
            keys.sort_unstable();
            keys.dedup();

            assert_eq!(keys.len(), count, "{screen:?} binds a key twice");
        }
    }

    /// Pressing a named key and letting the app act on it, for the keys `press` cannot
    /// build because they produce no character.
    fn press_named(app: &mut App, named: Named) {
        if let Some(message) =
            translate_key(keyboard::Key::Named(named), keyboard::Modifiers::empty())
        {
            let _ = app.update(message);
        }
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
                Screen::NoteTrainer,
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

    /// Every marker's pitch class, derived independently of `scale.spell` by
    /// transposing the open string — the same computation `scale_markers` itself
    /// does, kept separate here so a test failure means the marker disagrees
    /// with the tuning, not with itself.
    fn marker_pitch_class(marker: &NoteMarker) -> PitchClass {
        STANDARD_TUNING[marker.string].transpose(marker.fret as u8)
    }

    /// Every marker's position, for comparing two labellings of one neck.
    fn marker_positions(markers: &[NoteMarker]) -> Vec<(usize, usize)> {
        markers.iter().map(|m| (m.string, m.fret)).collect()
    }

    #[test]
    fn interval_notation_labels_the_markers_with_the_formula() {
        let a_aeolian = Scale {
            root: PitchClass::new(9),
            spelling: Spelling::Sharps,
            kind: ScaleKind::Aeolian,
        };

        let names = scale_markers(a_aeolian, Notation::Notes);
        let degrees = scale_markers(a_aeolian, Notation::Intervals);

        // The same dots, both ways round: the mode may change the text and nothing
        // else. Membership comes from `spell`, which never sees the mode.
        assert_eq!(
            marker_positions(&names),
            marker_positions(&degrees),
            "switching notation moved or dropped a marker"
        );
        let colors = |markers: &[NoteMarker]| -> Vec<(f32, f32, f32, f32)> {
            markers
                .iter()
                .map(|m| (m.color.r, m.color.g, m.color.b, m.color.a))
                .collect()
        };
        assert_eq!(
            colors(&names),
            colors(&degrees),
            "switching notation recoloured a marker"
        );

        // A minor's formula, and nothing outside it. Set equality both ways: no label
        // may appear that is not a degree of this scale, and every degree must show up
        // somewhere on twelve frets of six strings.
        let labels: HashSet<&str> = degrees.iter().map(|m| m.label.as_str()).collect();
        let expected: HashSet<&str> = ["1", "2", "b3", "4", "5", "b6", "b7"].into_iter().collect();
        assert_eq!(labels, expected);

        for marker in &degrees {
            if marker_pitch_class(marker) == a_aeolian.root {
                assert_eq!(marker.label, "1", "the root is degree 1");
            }
        }
    }

    #[test]
    fn the_spelling_toggle_does_not_reach_interval_labels() {
        // The ♯♭ button keeps working in interval notation — it still names the root on
        // the card — but a degree is a position in the formula, and no choice of sharps
        // or flats moves one.
        let sharps = Scale {
            root: PitchClass::new(1),
            spelling: Spelling::Sharps,
            kind: ScaleKind::Ionian,
        };
        let flats = Scale {
            spelling: Spelling::Flats,
            ..sharps
        };

        let labels = |scale: Scale| -> Vec<String> {
            scale_markers(scale, Notation::Intervals)
                .iter()
                .map(|m| m.label.clone())
                .collect()
        };

        // C♯ and D♭ Ionian are spelled differently note for note — the case
        // toggling_spelling_moves_no_marker_but_relabels_at_least_one uses — so if
        // spelling leaked into a degree, it would leak here.
        assert_eq!(labels(sharps), labels(flats));
    }

    #[test]
    fn f_ionian_names_pitch_class_ten_b_flat_not_a_sharp() {
        // The branch's headline claim, asserted where it reaches the screen:
        // F Ionian's fourth degree is B♭, not the semitone-only A♯ the old code
        // produced. `Spelling::Sharps` is exactly the setting that used to expose
        // the bug, since a naive sharps-only spelling would pick A♯ here.
        let scale = Scale {
            root: PitchClass::new(5),
            spelling: Spelling::Sharps,
            kind: ScaleKind::Ionian,
        };

        let markers = scale_markers(scale, Notation::Notes);
        let mut checked = 0;

        for marker in &markers {
            if marker_pitch_class(marker).semitone() == 10 {
                // Read off the label, now that a marker carries text rather than a
                // `Note`. That is nearer the claim in any case — `Bb` is what the
                // neck actually shows — and the structure behind it is pinned in
                // scales.rs, where the spelling is decided.
                assert_eq!(
                    marker.label, "Bb",
                    "string {} fret {}",
                    marker.string, marker.fret
                );
                checked += 1;
            }
        }

        assert!(
            checked > 0,
            "F Ionian on standard tuning never reaches pitch class 10"
        );
    }

    #[test]
    fn toggling_spelling_moves_no_marker_but_relabels_at_least_one() {
        // Automates the manual checklist item: the ♯/♭ toggle renames notes, it
        // does not redraw the fretboard. Membership is a pitch-class fact and
        // does not depend on Spelling, so both marker lists cover the same
        // string/fret cells in the same order — which is what lets them be
        // zipped positionally below instead of just compared as sets.
        //
        // The root must be a non-natural pitch class: F Ionian's B♭ is spelled
        // the same either way (letter-walked from a root whose own letter, F,
        // does not change), so it would not exercise the toggle at all. A root
        // like C♯/D♭ relabels every degree, since the root's own letter differs
        // between spellings.
        let sharps = Scale {
            root: PitchClass::new(1),
            spelling: Spelling::Sharps,
            kind: ScaleKind::Ionian,
        };
        let flats = Scale {
            spelling: Spelling::Flats,
            ..sharps
        };

        let sharp_markers = scale_markers(sharps, Notation::Notes);
        let flat_markers = scale_markers(flats, Notation::Notes);

        // `marker_positions` compares them as sequences, not as sets — which is what
        // the paragraph above claims and what the positional zip below relies on.
        assert_eq!(
            marker_positions(&sharp_markers),
            marker_positions(&flat_markers),
            "toggling spelling moved a marker"
        );

        assert_eq!(sharp_markers.len(), flat_markers.len());
        let relabelled = sharp_markers
            .iter()
            .zip(&flat_markers)
            .any(|(sharp, flat)| sharp.label != flat.label);
        assert!(relabelled, "no marker's label changed under the toggle");
    }

    #[test]
    fn root_highlighting_keys_on_pitch_class_not_on_note() {
        // Pins the exact predicate `scale_markers` highlights on: pitch-class
        // equality against `scale.root`, independently recomputed here rather
        // than trusted from `marker.note`. Swapping the source line to
        // `marker.note == scale.root_note()` happens to color identically today
        // — every note in one scale's `notes()` has a distinct pitch class, and
        // `every_scale_spells_without_failing` now pins `notes()[0] ==
        // root_note()` (Fix 5 in scales.rs) — but that equivalence depends on
        // both of those holding elsewhere. Keying on `PitchClass` directly, as
        // this test requires, stays correct even if either invariant is ever
        // broken by an unrelated change; keying on `Note` would not.
        let scale = Scale {
            root: PitchClass::new(5),
            spelling: Spelling::Sharps,
            kind: ScaleKind::Ionian,
        };

        let markers = scale_markers(scale, Notation::Notes);
        let root_color = Color::from_rgb8(0xff, 0x4d, 0x4d);
        let (mut root_markers, mut other_markers) = (0, 0);

        for marker in &markers {
            if marker_pitch_class(marker) == scale.root {
                assert_eq!(marker.color, root_color, "root marker not highlighted");
                root_markers += 1;
            } else {
                assert_eq!(marker.color, LINK, "non-root marker not LINK");
                other_markers += 1;
            }
        }

        assert!(root_markers > 0, "the root never appears on this tuning");
        assert!(other_markers > 0, "every marker was treated as the root");
    }

    #[test]
    fn standard_tuning_low_open_string_is_e_natural() {
        // Pins STANDARD_TUNING itself: E A D G B E low to high, pitch classes
        // 4 9 2 7 11 4. Nothing else asserts this constant directly.
        let e_ionian = Scale {
            root: PitchClass::new(4),
            spelling: Spelling::Sharps,
            kind: ScaleKind::Ionian,
        };

        let markers = scale_markers(e_ionian, Notation::Notes);
        let open_low_e = markers
            .iter()
            .find(|m| m.string == 0 && m.fret == 0)
            .expect("the open low E is in E Ionian");

        assert_eq!(open_low_e.label, "E");
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

    // ---- Note Trainer drill logic ----

    fn trainer_with_seed(seed: u64) -> (NoteTrainer, Rng) {
        let mut rng = Rng::from_seed(seed);
        let trainer = NoteTrainer::new(&mut rng);
        (trainer, rng)
    }

    /// Every position on the neck, in the order the drill enumerates them.
    fn all_positions() -> impl Iterator<Item = (usize, usize)> {
        (0..NECK_STRINGS).flat_map(|s| (0..=NECK_FRETS).map(move |f| (s, f)))
    }

    /// The pitch class the current prompt is about, whichever direction it runs.
    fn prompt_pitch_class(trainer: &NoteTrainer) -> PitchClass {
        match trainer.prompt {
            Prompt::NameIt { string, fret } => pitch_class_at(string, fret).unwrap(),
            Prompt::FindIt(target) => target,
        }
    }

    /// An answer that satisfies the current prompt.
    fn correct_answer(trainer: &NoteTrainer) -> Answer {
        match trainer.prompt {
            Prompt::NameIt { string, fret } => Answer::Name(pitch_class_at(string, fret).unwrap()),
            Prompt::FindIt(target) => {
                let (string, fret) = all_positions()
                    .find(|&(s, f)| pitch_class_at(s, f) == Some(target))
                    .expect("every pitch class appears within twelve frets");
                Answer::Position { string, fret }
            }
        }
    }

    /// An answer that does not.
    fn wrong_answer(trainer: &NoteTrainer) -> Answer {
        match trainer.prompt {
            Prompt::NameIt { string, fret } => {
                let actual = pitch_class_at(string, fret).unwrap();
                let other = PitchClass::ALL
                    .into_iter()
                    .find(|&pc| pc != actual)
                    .unwrap();
                Answer::Name(other)
            }
            Prompt::FindIt(target) => {
                let (string, fret) = all_positions()
                    .find(|&(s, f)| pitch_class_at(s, f) != Some(target))
                    .unwrap();
                Answer::Position { string, fret }
            }
        }
    }

    #[test]
    fn a_fresh_prompt_is_never_the_one_it_replaces() {
        let (mut trainer, mut rng) = trainer_with_seed(0xb0a7);

        for _ in 0..500 {
            let before = trainer.prompt;
            trainer.skip(&mut rng);
            assert_ne!(trainer.prompt, before);
        }
    }

    #[test]
    fn prompts_are_reproducible_from_a_seed() {
        let (mut a, mut rng_a) = trainer_with_seed(99);
        let (mut b, mut rng_b) = trainer_with_seed(99);

        assert_eq!(a.prompt, b.prompt, "the opening prompt already diverged");

        for _ in 0..64 {
            a.skip(&mut rng_a);
            b.skip(&mut rng_b);
            assert_eq!(a.prompt, b.prompt);
        }
    }

    #[test]
    fn consecutive_correct_answers_raise_the_streak() {
        let (mut trainer, mut rng) = trainer_with_seed(3);

        for expected in 1..=3 {
            let answer = correct_answer(&trainer);
            trainer.answer(answer, &mut rng);
            assert_eq!(trainer.streak, expected);
        }
    }

    #[test]
    fn a_wrong_answer_zeroes_the_streak_and_keeps_the_prompt() {
        let (mut trainer, mut rng) = trainer_with_seed(11);

        let answer = correct_answer(&trainer);
        trainer.answer(answer, &mut rng);
        assert_eq!(trainer.streak, 1);

        let standing = trainer.prompt;
        let wrong = wrong_answer(&trainer);
        trainer.answer(wrong, &mut rng);

        assert_eq!(trainer.streak, 0);
        assert_eq!(
            trainer.prompt, standing,
            "a wrong answer retired the prompt"
        );
        assert!(
            trainer.wrong.contains(&wrong),
            "the wrong answer went unmarked"
        );
    }

    /// The prompt is only ever retired by a correct answer or a skip, so the learner can
    /// keep trying — and every wrong guess stays marked while they do.
    #[test]
    fn wrong_answers_accumulate_until_the_prompt_advances() {
        let (mut trainer, mut rng) = trainer_with_seed(0x5eed);
        trainer.prompt = Prompt::NameIt { string: 0, fret: 0 };

        let actual = pitch_class_at(0, 0).unwrap();
        let wrongs: Vec<Answer> = PitchClass::ALL
            .into_iter()
            .filter(|&pc| pc != actual)
            .take(3)
            .map(Answer::Name)
            .collect();

        for &w in &wrongs {
            trainer.answer(w, &mut rng);
        }

        for &w in &wrongs {
            assert!(trainer.wrong.contains(&w));
        }

        // Repeating one does not grow the list.
        let before = trainer.wrong.len();
        trainer.answer(wrongs[0], &mut rng);
        assert_eq!(trainer.wrong.len(), before);

        trainer.answer(Answer::Name(actual), &mut rng);
        assert!(trainer.wrong.is_empty(), "feedback outlived its prompt");
    }

    #[test]
    fn the_best_streak_survives_what_the_current_one_does_not() {
        let (mut trainer, mut rng) = trainer_with_seed(21);

        for _ in 0..5 {
            let answer = correct_answer(&trainer);
            trainer.answer(answer, &mut rng);
        }
        assert_eq!(trainer.best_streak, 5);

        let wrong = wrong_answer(&trainer);
        trainer.answer(wrong, &mut rng);
        assert_eq!((trainer.streak, trainer.best_streak), (0, 5));

        trainer.skip(&mut rng);
        assert_eq!((trainer.streak, trainer.best_streak), (0, 5));

        trainer.toggle_pool(&mut rng);
        assert_eq!((trainer.streak, trainer.best_streak), (0, 5));

        trainer.toggle_direction(&mut rng);
        assert_eq!((trainer.streak, trainer.best_streak), (0, 5));

        trainer.enter(&mut rng);
        assert_eq!((trainer.streak, trainer.best_streak), (0, 5));
    }

    /// The point of judging by pitch class: a learner who thinks in flats is never marked
    /// wrong for it.
    #[test]
    fn the_two_names_of_a_black_key_are_one_answer() {
        let (mut trainer, _) = trainer_with_seed(0);
        // Open low E plus two semitones — F sharp, or G flat.
        trainer.prompt = Prompt::NameIt { string: 0, fret: 2 };

        let pitch_class = pitch_class_at(0, 2).unwrap();
        assert_eq!(Spelling::Sharps.spell(pitch_class).to_string(), "F#");
        assert_eq!(Spelling::Flats.spell(pitch_class).to_string(), "Gb");

        // One pitch class, so one answer, whichever name the button carried.
        for spelling in [Spelling::Sharps, Spelling::Flats] {
            trainer.spelling = spelling;
            assert!(trainer.judge(Answer::Name(pitch_class)), "{spelling:?}");
        }
    }

    #[test]
    fn the_naturals_pool_never_prompts_an_accidental() {
        let (mut trainer, mut rng) = trainer_with_seed(0x4a7);

        assert_eq!(trainer.pool, Pool::Naturals, "naturals is not the default");

        for _ in 0..300 {
            assert!(
                PitchClass::NATURALS.contains(&prompt_pitch_class(&trainer)),
                "{:?} is not a natural",
                trainer.prompt
            );
            trainer.skip(&mut rng);
        }

        // Both directions draw from the same pool.
        trainer.toggle_direction(&mut rng);
        for _ in 0..300 {
            assert!(PitchClass::NATURALS.contains(&prompt_pitch_class(&trainer)));
            trainer.skip(&mut rng);
        }
    }

    #[test]
    fn widening_the_pool_reaches_the_accidentals() {
        let (mut trainer, mut rng) = trainer_with_seed(0xac1d);
        trainer.toggle_pool(&mut rng);
        assert_eq!(trainer.pool, Pool::All);

        let mut seen_an_accidental = false;
        for _ in 0..500 {
            if !PitchClass::NATURALS.contains(&prompt_pitch_class(&trainer)) {
                seen_an_accidental = true;
                break;
            }
            trainer.skip(&mut rng);
        }

        assert!(
            seen_an_accidental,
            "500 draws from all twelve found no accidental"
        );
    }

    /// A note is in seven places within twelve frets, and the drill singles none of them
    /// out.
    #[test]
    fn find_it_accepts_every_position_carrying_the_note() {
        let (mut trainer, _) = trainer_with_seed(0);
        let target = PitchClass::new(7); // G
        trainer.prompt = Prompt::FindIt(target);

        let mut accepted = 0;
        for (string, fret) in all_positions() {
            let answer = Answer::Position { string, fret };
            let carries_it = pitch_class_at(string, fret) == Some(target);

            assert_eq!(trainer.judge(answer), carries_it, "({string}, {fret})");
            accepted += usize::from(carries_it);
        }

        assert!(accepted > 1, "G should appear more than once on the neck");
    }

    /// Recorded rather than incidental: a mismatched pair means the view wired the wrong
    /// answer surface to the prompt, and the chosen symptom is "always wrong", not a panic.
    #[test]
    fn an_answer_of_the_wrong_shape_is_simply_wrong() {
        let (mut trainer, _) = trainer_with_seed(0);

        trainer.prompt = Prompt::NameIt { string: 0, fret: 0 };
        assert!(!trainer.judge(Answer::Position { string: 0, fret: 0 }));

        trainer.prompt = Prompt::FindIt(pitch_class_at(0, 0).unwrap());
        assert!(!trainer.judge(Answer::Name(pitch_class_at(0, 0).unwrap())));
    }

    #[test]
    fn toggling_the_direction_flips_which_way_the_drill_runs() {
        let (mut trainer, mut rng) = trainer_with_seed(5);
        assert_eq!(
            trainer.prompt.drill(),
            Drill::NameIt,
            "Name it is not the default"
        );

        trainer.toggle_direction(&mut rng);
        assert_eq!(trainer.prompt.drill(), Drill::FindIt);

        trainer.toggle_direction(&mut rng);
        assert_eq!(trainer.prompt.drill(), Drill::NameIt);
    }

    /// Spelling is the one toggle that is pure chrome.
    #[test]
    fn toggling_the_spelling_changes_nothing_but_the_names() {
        let (mut trainer, mut rng) = trainer_with_seed(8);

        let answer = correct_answer(&trainer);
        trainer.answer(answer, &mut rng);

        let before = (trainer.prompt, trainer.streak, trainer.best_streak);
        trainer.toggle_spelling();

        assert_eq!(trainer.spelling, Spelling::Flats);
        assert_eq!(
            (trainer.prompt, trainer.streak, trainer.best_streak),
            before,
            "the spelling toggle disturbed the drill"
        );
    }

    #[test]
    fn the_cursor_stops_at_the_necks_edges() {
        let (mut trainer, _) = trainer_with_seed(0);

        trainer.cursor = (0, 0);
        trainer.move_cursor(Direction::Left);
        assert_eq!(trainer.cursor, (0, 0), "walked off the low E");
        trainer.move_cursor(Direction::Up);
        assert_eq!(trainer.cursor, (0, 0), "walked off the nut");

        trainer.cursor = (NECK_STRINGS - 1, NECK_FRETS);
        trainer.move_cursor(Direction::Right);
        assert_eq!(
            trainer.cursor,
            (NECK_STRINGS - 1, NECK_FRETS),
            "walked off the high e"
        );
        trainer.move_cursor(Direction::Down);
        assert_eq!(
            trainer.cursor,
            (NECK_STRINGS - 1, NECK_FRETS),
            "walked off the last fret"
        );
    }

    #[test]
    fn the_cursor_walks_one_position_at_a_time() {
        let (mut trainer, _) = trainer_with_seed(0);
        trainer.cursor = (2, 5);

        trainer.move_cursor(Direction::Right);
        assert_eq!(trainer.cursor, (3, 5));
        trainer.move_cursor(Direction::Down);
        assert_eq!(trainer.cursor, (3, 6));
        trainer.move_cursor(Direction::Left);
        assert_eq!(trainer.cursor, (2, 6));
        // Up is towards the nut, because the neck is drawn with the nut at the top.
        trainer.move_cursor(Direction::Up);
        assert_eq!(trainer.cursor, (2, 5));
    }

    #[test]
    fn every_position_the_drill_can_prompt_is_on_the_neck() {
        for pool in [Pool::Naturals, Pool::All] {
            let (mut trainer, _) = trainer_with_seed(0);
            trainer.pool = pool;

            for (string, fret) in trainer.positions() {
                assert!(string < NECK_STRINGS && fret <= NECK_FRETS);
                assert!(pool.contains(pitch_class_at(string, fret).unwrap()));
            }
        }
    }

    // ---- Note Trainer keys and focus ----

    /// An app sitting on the Note Trainer with a reproducible prompt stream.
    fn note_trainer_app(seed: u64) -> App {
        let mut app = app_with_seed(seed);
        app.open(Screen::NoteTrainer);
        app
    }

    /// Walks the app into the *Find it* direction, where the neck is the answer surface.
    fn find_it_app(seed: u64) -> App {
        let mut app = note_trainer_app(seed);
        app.note_trainer.toggle_direction(&mut app.rng);
        app.reset_focus();
        assert_eq!(app.note_trainer.prompt.drill(), Drill::FindIt);
        app
    }

    /// Answers the current prompt correctly, through whichever message the view would send
    /// for the direction in play. Which surface answers is the prompt's business, not the
    /// caller's — the same reason `Prompt` carries the direction.
    fn answer_correctly(app: &mut App) {
        let message = match correct_answer(&app.note_trainer) {
            Answer::Name(pitch_class) => Message::AnswerNote(pitch_class),
            Answer::Position { string, fret } => Message::ChooseNotePosition(string, fret),
        };

        let _ = app.update(message);
    }

    #[test]
    fn opening_the_note_trainer_lands_on_a_prompt() {
        let app = note_trainer_app(1);

        assert_eq!(app.screen, Screen::NoteTrainer);
        assert_eq!(app.note_trainer.streak, 0);
        assert!(app.note_trainer.wrong.is_empty());
    }

    /// The screen never reopens on what it last showed, exactly as the scale trainer never
    /// reopens on its last scale.
    #[test]
    fn reopening_the_note_trainer_draws_a_fresh_prompt() {
        let mut app = note_trainer_app(0xfa11);

        for _ in 0..50 {
            let before = app.note_trainer.prompt;
            let _ = app.update(Message::GoBack);
            let _ = app.update(Message::Navigate(Screen::NoteTrainer));

            assert_eq!(app.screen, Screen::NoteTrainer);
            assert_ne!(app.note_trainer.prompt, before);
        }
    }

    #[test]
    fn the_note_trainer_settings_survive_leaving_the_screen() {
        let mut app = note_trainer_app(4);

        let _ = app.update(Message::ToggleDrillDirection);
        let _ = app.update(Message::TogglePool);
        let _ = app.update(Message::ToggleNoteSpelling);

        for _ in 0..3 {
            answer_correctly(&mut app);
        }

        let kept = (
            app.note_trainer.prompt.drill(),
            app.note_trainer.pool,
            app.note_trainer.spelling,
            app.note_trainer.best_streak,
        );

        let _ = app.update(Message::GoBack);
        let _ = app.update(Message::Navigate(Screen::NoteTrainer));

        assert_eq!(
            (
                app.note_trainer.prompt.drill(),
                app.note_trainer.pool,
                app.note_trainer.spelling,
                app.note_trainer.best_streak,
            ),
            kept,
        );
        assert_eq!(app.note_trainer.streak, 0, "the run outlived the visit");
    }

    #[test]
    fn tab_reaches_the_neck_in_find_it() {
        let app = find_it_app(7);

        assert!(
            app.focusables().contains(&FocusTarget::Fretboard),
            "the neck is not in the Tab order"
        );
        // ...and is absent from the other direction, where the buttons answer instead.
        assert!(
            !note_trainer_app(7)
                .focusables()
                .contains(&FocusTarget::Fretboard)
        );
    }

    #[test]
    fn the_answer_buttons_are_reachable_in_name_it() {
        let app = note_trainer_app(7);
        let reachable = app.focusables();

        for index in 0..PitchClass::ALL.len() {
            assert!(
                reachable.contains(&FocusTarget::NoteAnswer(index)),
                "answer button {index} is unreachable"
            );
        }
        assert!(!reachable.contains(&FocusTarget::Fretboard));
    }

    /// The neck claims the motion keys while focused: the arrows move its cursor and the
    /// focus ring stays put.
    #[test]
    fn the_arrows_drive_the_cursor_not_the_ring() {
        let mut app = find_it_app(2);
        app.focused = FocusTarget::Fretboard;
        app.note_trainer.cursor = (2, 4);

        for (named, expected) in [
            (Named::ArrowRight, (3, 4)),
            (Named::ArrowDown, (3, 5)),
            (Named::ArrowLeft, (2, 5)),
            (Named::ArrowUp, (2, 4)),
        ] {
            press_named(&mut app, named);

            assert_eq!(app.note_trainer.cursor, expected);
            assert_eq!(app.focused, FocusTarget::Fretboard, "the ring moved");
        }
    }

    #[test]
    fn the_vim_motions_drive_the_cursor_too() {
        let mut app = find_it_app(2);
        app.focused = FocusTarget::Fretboard;
        app.note_trainer.cursor = (2, 4);

        for (key, expected) in [("l", (3, 4)), ("j", (3, 5)), ("h", (2, 5)), ("k", (2, 4))] {
            press_into(&mut app, key, keyboard::Modifiers::empty());

            assert_eq!(app.note_trainer.cursor, expected, "{key}");
            assert_eq!(app.focused, FocusTarget::Fretboard, "{key} moved the ring");
        }
    }

    /// Arrows never leave the neck, so Tab has to — from every corner of it.
    #[test]
    fn tab_always_escapes_the_neck() {
        for cursor in [
            (0, 0),
            (0, NECK_FRETS),
            (NECK_STRINGS - 1, 0),
            (NECK_STRINGS - 1, NECK_FRETS),
            (3, 6),
        ] {
            let mut app = find_it_app(9);
            app.focused = FocusTarget::Fretboard;
            app.note_trainer.cursor = cursor;

            press_named(&mut app, Named::Tab);
            assert_ne!(
                app.focused,
                FocusTarget::Fretboard,
                "Tab stuck at {cursor:?}"
            );

            // And backwards, which `press_named` cannot send because it holds no modifier.
            app.focused = FocusTarget::Fretboard;
            if let Some(message) =
                translate_key(keyboard::Key::Named(Named::Tab), keyboard::Modifiers::SHIFT)
            {
                let _ = app.update(message);
            }
            assert_ne!(
                app.focused,
                FocusTarget::Fretboard,
                "Shift+Tab stuck at {cursor:?}"
            );
        }
    }

    #[test]
    fn enter_on_the_neck_answers_with_the_cursor() {
        let mut app = find_it_app(0x3e);
        app.focused = FocusTarget::Fretboard;

        let Prompt::FindIt(target) = app.note_trainer.prompt else {
            unreachable!("find_it_app guarantees the direction")
        };

        // Park the cursor on a position that plays the prompted note, then press Enter.
        let (string, fret) = all_positions()
            .find(|&(s, f)| pitch_class_at(s, f) == Some(target))
            .unwrap();
        app.note_trainer.cursor = (string, fret);

        press_named(&mut app, Named::Enter);

        assert_eq!(app.note_trainer.streak, 1, "Enter did not answer");
    }

    #[test]
    fn a_wrong_cursor_position_marks_the_neck_and_keeps_the_prompt() {
        let mut app = find_it_app(0x77);
        app.focused = FocusTarget::Fretboard;

        let standing = app.note_trainer.prompt;
        let Prompt::FindIt(target) = standing else {
            unreachable!()
        };

        let (string, fret) = all_positions()
            .find(|&(s, f)| pitch_class_at(s, f) != Some(target))
            .unwrap();
        app.note_trainer.cursor = (string, fret);

        press_named(&mut app, Named::Space);

        assert_eq!(app.note_trainer.streak, 0);
        assert_eq!(app.note_trainer.prompt, standing);
        assert!(
            app.note_trainer
                .wrong
                .contains(&Answer::Position { string, fret })
        );
        assert_eq!(wrong_position_markers(&app.note_trainer).len(), 1);
    }

    /// A click reports through the fretboard's press handler, which is the same message the
    /// canvas would publish, and it also drags the cursor along.
    #[test]
    fn a_press_on_the_neck_answers_and_moves_the_cursor() {
        let mut app = find_it_app(0x5a);

        let Prompt::FindIt(target) = app.note_trainer.prompt else {
            unreachable!()
        };
        let (string, fret) = all_positions()
            .find(|&(s, f)| pitch_class_at(s, f) == Some(target))
            .unwrap();

        let _ = app.update(Message::ChooseNotePosition(string, fret));

        assert_eq!(app.note_trainer.cursor, (string, fret), "the cursor lagged");
        assert_eq!(app.note_trainer.streak, 1);
    }

    #[test]
    fn the_note_trainer_accelerators_act_without_moving_focus() {
        let mut app = note_trainer_app(0xacc);
        app.focused = FocusTarget::NoteAnswer(5);

        let before = app.note_trainer.prompt;
        press_into(&mut app, "r", keyboard::Modifiers::empty());
        assert_ne!(app.note_trainer.prompt, before, "r did not skip");
        assert_eq!(app.focused, FocusTarget::NoteAnswer(5), "r moved focus");

        press_into(&mut app, "d", keyboard::Modifiers::empty());
        assert_eq!(
            app.note_trainer.prompt.drill(),
            Drill::FindIt,
            "d did not swap"
        );

        // Focus is left where it was even though that widget is gone in the new direction;
        // `step_focus` snaps a stale target back onto the grid on the next motion.
        press_named(&mut app, Named::Tab);
        assert!(app.focusables().contains(&app.focused));

        let mut app = note_trainer_app(0xacc);
        assert_eq!(app.note_trainer.pool, Pool::Naturals);
        press_into(&mut app, "a", keyboard::Modifiers::empty());
        assert_eq!(app.note_trainer.pool, Pool::All, "a did not widen the pool");
    }

    #[test]
    fn the_note_trainer_keys_are_inert_on_other_screens() {
        for screen in [Screen::Home, Screen::ScaleTrainer, Screen::IntervalTrainer] {
            for key in ["d", "a"] {
                let mut app = app_with_seed(0x1e37);
                app.open(screen.clone());

                let before = note_trainer_state(&app);
                press_into(&mut app, key, keyboard::Modifiers::empty());

                assert_eq!(
                    note_trainer_state(&app),
                    before,
                    "{key} on {screen:?} reached the Note Trainer"
                );
            }
        }
    }

    /// `i` toggles interval notation on the scale trainer and must stay inert on the Note
    /// Trainer — now because that screen declares no `i`, not because it is empty.
    #[test]
    fn the_interval_notation_key_is_inert_on_the_note_trainer() {
        let mut app = note_trainer_app(0x11);

        let before = (note_trainer_state(&app), app.notation);
        press_into(&mut app, "i", keyboard::Modifiers::empty());

        assert_eq!((note_trainer_state(&app), app.notation), before);
    }

    /// Everything a keypress on the Note Trainer could disturb.
    fn note_trainer_state(app: &App) -> (Prompt, Pool, Spelling, u32, u32, (usize, usize)) {
        let t = &app.note_trainer;
        (
            t.prompt,
            t.pool,
            t.spelling,
            t.streak,
            t.best_streak,
            t.cursor,
        )
    }

    #[test]
    fn the_help_overlay_lists_the_note_trainers_keys() {
        let bound = accelerators(&Screen::NoteTrainer);
        let keys: Vec<char> = bound.iter().map(|&(key, _, _)| key).collect();

        assert_eq!(keys, vec!['r', 'd', 'a']);

        for (key, _, label) in bound {
            assert!(!label.is_empty(), "{key} has no label for the overlay");
        }
    }

    /// Builds the widget tree for every screen and every drill direction.
    ///
    /// A compiling view is not a working one: sizes, alignments, and `Length` combinations
    /// are checked when the tree is constructed, not by the type system. This is the cheapest
    /// stand-in for launching the app, and it covers the states a hand-drill would reach —
    /// including one with wrong answers marked on both surfaces.
    #[test]
    fn every_screen_builds_its_view() {
        let mut app = app_with_seed(0x21e0);

        for screen in every_screen() {
            app.open(screen.clone());
            let _ = app.view();

            // ...and with the help overlay stacked on top of it.
            app.help_open = true;
            let _ = app.view();
            app.help_open = false;
        }

        app.open(Screen::NoteTrainer);

        for _ in 0..2 {
            // A wrong answer in whichever direction is current, so the feedback path is
            // built too.
            let wrong = wrong_answer(&app.note_trainer);
            let message = match wrong {
                Answer::Name(pitch_class) => Message::AnswerNote(pitch_class),
                Answer::Position { string, fret } => Message::ChooseNotePosition(string, fret),
            };
            let _ = app.update(message);
            assert!(!app.note_trainer.wrong.is_empty());
            let _ = app.view();

            let _ = app.update(Message::ToggleDrillDirection);
            let _ = app.view();
        }
    }

    #[test]
    fn escape_leaves_the_note_trainer() {
        for named in [Named::Escape, Named::Backspace] {
            let mut app = note_trainer_app(0x3c);
            press_named(&mut app, named);

            assert_eq!(app.screen, Screen::Home, "{named:?} did not leave");
        }
    }
}
