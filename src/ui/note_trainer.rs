//! The Note Trainer: the drill's state machine and the screen that draws it.
//!
//! Both halves live here so the state can keep its fields to itself. They are read all over
//! the views below and nowhere else — `App` drives the drill through the handful of methods
//! marked `pub(super)` and never touches what is behind them. Splitting the views out to the
//! parent would mean opening every field up to it, which is the encapsulation this module
//! exists to hold.
//!
//! What the neck *is* — its strings, its frets, the note under a finger — stays in the
//! parent: that is the instrument, and the next trainer will want it on the same terms.

use iced::{Color, Element, Padding};

use super::fretboard::{Fretboard, MarkerStyle, NoteMarker, fretboard};
use super::{
    ANSWER_ROW_WIDTH, BODY, CANVAS, CURSOR_HOME, DANGER, Direction, FocusTarget, INK, LINK,
    MUSIC_FONT, MUTE, Message, NECK_FRETS, NECK_STRINGS, ROOT_BUTTON_SIZE, SELECTOR_CARD_HEIGHT,
    SMUFL_FLAT, SMUFL_SHARP, SUCCESS, SUMMARY_CARD_HEIGHT, card_container, correct_answer_button,
    focus_ring, ghost_button, note_label, pitch_class_at, wrong_answer_button,
};
use crate::music::notes::{PitchClass, Spelling};
use crate::rng::Rng;

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
pub(super) enum Drill {
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

/// The Note Trainer's whole state.
///
/// No iced types here on purpose: the drill is a state machine, and keeping it one means
/// the streak rules and the judging can be tested without a widget tree or a renderer.
pub(super) struct NoteTrainer {
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
    /// The correct answer just given, held while it is lit on screen.
    ///
    /// `Some` is the paused state: the prompt stays put with its answer marked, and further
    /// answers are ignored until `advance` retires it a second later. One `Option` rather
    /// than a flag beside the answer, because "paused" and "which answer to mark" are the
    /// same fact — a flag could be set with nothing to mark, and this cannot be.
    correct: Option<Answer>,
    cursor: (usize, usize),
}

impl NoteTrainer {
    /// Note the `&mut Rng` on this and on everything that redraws a prompt.
    ///
    /// The generator lives on `App` beside this struct, and a method here cannot reach it:
    /// `&mut self.note_trainer` and `&mut self.rng` are two mutable borrows of one `App`.
    /// Passing it in is the idiomatic answer, and it is also what keeps the drill seedable
    /// from the tests. A second generator of its own would mean a second seed.
    pub(super) fn new(rng: &mut Rng) -> Self {
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
            correct: None,
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

        // Both kinds of feedback belong to the prompt they were given against, so a new
        // prompt arrives with a clean surface — and a skip or a toggle during the flash
        // ends it, since every one of them comes through here.
        self.wrong.clear();
        self.correct = None;
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

    /// Takes an answer, unless one is already being marked.
    ///
    /// The generator is still taken during the pause even though nothing is drawn then: it
    /// is the same call either way, and a signature that changed with the state would push
    /// the pause out to every caller.
    fn answer(&mut self, answer: Answer, _rng: &mut Rng) {
        // A correct answer is on screen, so the drill is not asking anything. Answers
        // arriving now are the tail of the press that scored — a held key, a double
        // click — and counting them would either inflate the streak or, worse, mark the
        // learner wrong for a question they have already got right.
        if self.correct.is_some() {
            return;
        }

        if self.judge(answer) {
            self.streak += 1;
            self.best_streak = self.best_streak.max(self.streak);

            // The prompt is *not* replaced here. It stands, with this answer lit, until
            // `advance` retires it — see `correct`.
            self.correct = Some(answer);
        } else {
            // Deduplicated so hammering one wrong button cannot grow this without bound.
            if !self.wrong.contains(&answer) {
                self.wrong.push(answer);
            }
            self.streak = 0;
        }
    }

    /// Answers with a note name — the *Name it* surface.
    ///
    /// `Answer` is built here rather than by the caller, which is what keeps it, `Prompt`,
    /// and `Pool` private to this module: `App` names a pitch class or a position, and the
    /// shape they travel in is the drill's own business.
    pub(super) fn answer_name(&mut self, pitch_class: PitchClass, rng: &mut Rng) {
        self.answer(Answer::Name(pitch_class), rng);
    }

    /// Answers with a position, and takes the cursor there.
    ///
    /// The cursor follows, so the mouse and the keyboard never disagree about where it is —
    /// and because a press *is* a move-then-answer, the two arrive as one call rather than
    /// as an assignment the caller has to remember to make first.
    pub(super) fn answer_position(&mut self, string: usize, fret: usize, rng: &mut Rng) {
        self.cursor = (string, fret);
        self.answer(Answer::Position { string, fret }, rng);
    }

    /// Answers with wherever the cursor is sitting — what Enter on the neck does.
    ///
    /// The same thing a press on that position would send, reached the other way round: the
    /// cursor is already there, so only the answer half of `answer_position` is left to do.
    pub(super) fn answer_at_cursor(&mut self, rng: &mut Rng) {
        let (string, fret) = self.cursor;
        self.answer(Answer::Position { string, fret }, rng);
    }

    /// Whether a correct answer is lit, which is the window the flash timer runs in.
    ///
    /// A question rather than the `Option` itself: the subscription only needs to know that
    /// the pause is on, not what is being marked.
    pub(super) fn is_flashing(&self) -> bool {
        self.correct.is_some()
    }

    /// Which way the drill is running. The focus grid asks, because the answer surface — and
    /// so what is focusable at all — differs between the two directions.
    pub(super) fn drill(&self) -> Drill {
        self.prompt.drill()
    }

    /// Ends the flash a correct answer left on screen and draws the next prompt.
    ///
    /// A no-op when nothing is being marked, so a tick that outlived its flash — one that
    /// crossed a skip, or landed after the screen was left — cannot retire a prompt the
    /// learner is still reading.
    pub(super) fn advance(&mut self, rng: &mut Rng) {
        if self.correct.is_some() {
            let drill = self.prompt.drill();
            self.draw_prompt(drill, rng);
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

    pub(super) fn skip(&mut self, rng: &mut Rng) {
        let drill = self.prompt.drill();
        self.break_streak();
        self.draw_prompt(drill, rng);
    }

    pub(super) fn toggle_direction(&mut self, rng: &mut Rng) {
        let flipped = self.prompt.drill().flipped();
        self.break_streak();
        self.cursor = CURSOR_HOME;
        self.draw_prompt(flipped, rng);
    }

    pub(super) fn toggle_pool(&mut self, rng: &mut Rng) {
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
    pub(super) fn toggle_spelling(&mut self) {
        self.spelling = match self.spelling {
            Spelling::Sharps => Spelling::Flats,
            Spelling::Flats => Spelling::Sharps,
        };
    }

    /// Opening the screen: the settings and the best streak persist, the run does not.
    pub(super) fn enter(&mut self, rng: &mut Rng) {
        let drill = self.prompt.drill();
        self.break_streak();
        self.cursor = CURSOR_HOME;
        self.draw_prompt(drill, rng);
    }

    /// Walks the cursor one position, stopping at the neck's edges rather than wrapping —
    /// the same way the focus ring already behaves at the edge of a grid.
    ///
    /// Up is towards the nut, because the neck is drawn with the nut at the top.
    pub(super) fn move_cursor(&mut self, direction: Direction) {
        let (string, fret) = self.cursor;

        self.cursor = match direction {
            Direction::Left => (string.saturating_sub(1), fret),
            Direction::Right => ((string + 1).min(NECK_STRINGS - 1), fret),
            Direction::Up => (string, fret.saturating_sub(1)),
            Direction::Down => (string, (fret + 1).min(NECK_FRETS)),
        };
    }
}

/// The Note Trainer.
///
/// Both directions share this one function, branching on the prompt's variant, because the
/// header, the streak, and the neck are common to both — only the answer surface differs.
pub(super) fn ui_note_trainer(
    trainer: &NoteTrainer,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, column, container, row, text};

    let neck = match trainer.prompt {
        // The prompt itself: one ring — see `prompt_marker`.
        Prompt::NameIt { .. } => Fretboard {
            num_frets: NECK_FRETS,
            highlighted: prompt_marker(trainer),
            ..Fretboard::default()
        },
        // Here the neck is the answer surface, so it takes a press handler and shows the
        // cursor. Guesses stay marked on it until the prompt advances.
        Prompt::FindIt(_) => Fretboard {
            num_frets: NECK_FRETS,
            highlighted: position_markers(trainer),
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

/// The position the *Name it* prompt is asking about, as a marker on the neck — or nothing
/// at all in *Find it*, where the prompt is a note name in the card and the neck is the
/// answer surface.
///
/// An unfilled ring, and deliberately so. A fill is what this screen uses to say "you put
/// this here": the guesses in `position_markers` are filled, and so are the scale trainer's
/// dots. The prompt is the opposite — a question about a spot with nothing on it yet — and a
/// solid dot read as an answer already placed.
///
/// Unlabelled for a separate reason: a label would print the answer inside the question.
///
/// A `Vec` rather than an `Option<NoteMarker>` so it drops straight into `highlighted`, and
/// so a prompt that ever wants two marks needs no new signature.
fn prompt_marker(trainer: &NoteTrainer) -> Vec<NoteMarker> {
    match trainer.prompt {
        Prompt::NameIt { string, fret } => vec![NoteMarker {
            string,
            fret,
            label: String::new(),
            color: LINK,
            style: MarkerStyle::Outlined,
        }],
        Prompt::FindIt(_) => Vec::new(),
    }
}

/// The positions guessed against the current prompt, as markers on the neck: the wrong ones
/// in the danger colour, and the one that scored in the success colour.
///
/// Filled, both of them — that is what keeps them readable as answers rather than as the
/// question. The ring belongs to the prompt alone; see `prompt_marker`.
///
/// The right answer comes last so it is drawn over the wrong ones, which matters only if a
/// learner presses the same fret twice — and there the green is the newer news.
///
/// Only `Answer::Position` guesses can appear on a neck; a `Name` guess belongs to the other
/// direction and is filtered out rather than being an error, since both lists are cleared
/// whenever the prompt advances and the two can never mix in practice.
fn position_markers(trainer: &NoteTrainer) -> Vec<NoteMarker> {
    let marker = |answer: &Answer, color: Color| match *answer {
        Answer::Position { string, fret } => Some(NoteMarker {
            string,
            fret,
            label: String::new(),
            color,
            style: MarkerStyle::Filled,
        }),
        Answer::Name(_) => None,
    };

    trainer
        .wrong
        .iter()
        .filter_map(|answer| marker(answer, DANGER))
        .chain(
            trainer
                .correct
                .iter()
                .filter_map(|answer| marker(answer, SUCCESS)),
        )
        .collect()
}

/// The current run and the best of the session.
///
/// The live streak is drawn in the theme's success colour: the per-answer flash says a
/// single answer was right and then goes, and a standing streak keeps saying it.
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
            let answer = Answer::Name(pitch_class);
            let was_wrong = trainer.wrong.contains(&answer);
            let was_right = trainer.correct == Some(answer);
            // Both marked states fill the button, so both need ink that reads on a fill.
            let color = if was_wrong || was_right { CANVAS } else { INK };

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
            // Right beats wrong: they cannot both be this button under one prompt, but if
            // that ever changes, the answer that scored is the one worth showing.
            .style(if was_right {
                correct_answer_button
            } else if was_wrong {
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

#[cfg(test)]
mod tests {
    use super::super::tests::{app_with_seed, every_screen, press_into, press_named};
    use super::super::{App, Screen, accelerators, translate_key};
    use super::*;
    use iced::keyboard;
    use iced::keyboard::key::Named;

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

    /// Answers correctly and lets the flash run its course, which is what the timer does a
    /// second later. Tests about the streak want the drill moving; the ones about the pause
    /// itself call `answer` and stop there.
    fn answer_and_advance(trainer: &mut NoteTrainer, rng: &mut Rng) {
        let answer = correct_answer(trainer);
        trainer.answer(answer, rng);
        trainer.advance(rng);
    }

    #[test]
    fn consecutive_correct_answers_raise_the_streak() {
        let (mut trainer, mut rng) = trainer_with_seed(3);

        for expected in 1..=3 {
            answer_and_advance(&mut trainer, &mut rng);
            assert_eq!(trainer.streak, expected);
        }
    }

    /// The pause: a correct answer marks itself and holds the prompt, and the drill takes
    /// nothing else until the flash ends.
    #[test]
    fn a_correct_answer_is_marked_and_holds_the_prompt() {
        let (mut trainer, mut rng) = trainer_with_seed(0xf1a5);

        let standing = trainer.prompt;
        let answer = correct_answer(&trainer);
        trainer.answer(answer, &mut rng);

        assert_eq!(trainer.correct, Some(answer), "the answer went unmarked");
        assert_eq!(trainer.prompt, standing, "the prompt left before its flash");
        assert_eq!(trainer.streak, 1);

        // Nothing lands while it is up — not another correct press, and not a wrong one.
        let wrong = wrong_answer(&trainer);
        trainer.answer(wrong, &mut rng);
        trainer.answer(answer, &mut rng);

        assert_eq!((trainer.streak, trainer.prompt), (1, standing));
        assert!(trainer.wrong.is_empty(), "a press during the flash counted");

        trainer.advance(&mut rng);

        assert_eq!(trainer.correct, None, "the flash outlived its prompt");
        assert_ne!(trainer.prompt, standing, "the flash never ended");
    }

    /// The flash belongs to the prompt it was given against, so anything that retires that
    /// prompt ends it — including the learner moving on before the second is up.
    #[test]
    fn moving_on_ends_the_flash_early() {
        let (mut trainer, mut rng) = trainer_with_seed(0xea51);

        for interrupt in [
            NoteTrainer::skip as fn(&mut NoteTrainer, &mut Rng),
            NoteTrainer::toggle_pool,
            NoteTrainer::toggle_direction,
            NoteTrainer::enter,
        ] {
            let answer = correct_answer(&trainer);
            trainer.answer(answer, &mut rng);
            assert!(trainer.correct.is_some());

            interrupt(&mut trainer, &mut rng);
            assert_eq!(trainer.correct, None, "a flash survived being interrupted");
        }
    }

    /// A tick can outlive its flash — one that crossed a skip, or arrived after the screen
    /// was left. It must not retire the prompt now on screen.
    #[test]
    fn advancing_without_a_flash_does_nothing() {
        let (mut trainer, mut rng) = trainer_with_seed(0x71c4);

        let standing = trainer.prompt;
        trainer.advance(&mut rng);

        assert_eq!(trainer.prompt, standing);
        assert_eq!(trainer.streak, 0);
    }

    #[test]
    fn a_wrong_answer_zeroes_the_streak_and_keeps_the_prompt() {
        let (mut trainer, mut rng) = trainer_with_seed(11);

        answer_and_advance(&mut trainer, &mut rng);
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

        // The right answer joins them rather than clearing them: for as long as the flash
        // is up, the learner can see what they tried and what it turned out to be.
        trainer.answer(Answer::Name(actual), &mut rng);
        assert_eq!(trainer.wrong.len(), before);

        trainer.advance(&mut rng);
        assert!(trainer.wrong.is_empty(), "feedback outlived its prompt");
    }

    #[test]
    fn the_best_streak_survives_what_the_current_one_does_not() {
        let (mut trainer, mut rng) = trainer_with_seed(21);

        for _ in 0..5 {
            answer_and_advance(&mut trainer, &mut rng);
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

        answer_and_advance(&mut trainer, &mut rng);

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
    ///
    /// The flash's timer message follows, standing in for the second the subscription
    /// spends waiting, so the drill ends up where a learner would find it.
    fn answer_correctly(app: &mut App) {
        let message = match correct_answer(&app.note_trainer) {
            Answer::Name(pitch_class) => Message::AnswerNote(pitch_class),
            Answer::Position { string, fret } => Message::ChooseNotePosition(string, fret),
        };

        let _ = app.update(message);
        let _ = app.update(Message::AdvancePrompt);
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

        let markers = position_markers(&app.note_trainer);
        assert_eq!(markers.len(), 1);
        // Filled, so a guess can never be mistaken for the prompt's ring.
        assert_eq!(markers[0].style, MarkerStyle::Filled);
    }

    /// The prompt is a ring, not a dot. On a screen where a fill means "you put this here",
    /// the question has to look unlike the answers — so this checks the shape as closely as
    /// it checks the position.
    #[test]
    fn the_name_it_prompt_is_an_unfilled_ring() {
        let (mut trainer, mut rng) = trainer_with_seed(0x21e6);

        let Prompt::NameIt { string, fret } = trainer.prompt else {
            unreachable!("the Note Trainer opens in Name it")
        };

        let markers = prompt_marker(&trainer);
        assert_eq!(markers.len(), 1, "the prompt marked more than its position");
        assert_eq!((markers[0].string, markers[0].fret), (string, fret));
        assert_eq!(
            markers[0].style,
            MarkerStyle::Outlined,
            "the prompt is a filled dot again"
        );
        assert_eq!(markers[0].color, LINK);
        assert!(
            markers[0].label.is_empty(),
            "the prompt printed its own answer"
        );

        // ...and the other direction puts nothing of the prompt on the neck: there the note
        // is named in the card and the neck is the answer surface.
        trainer.toggle_direction(&mut rng);
        assert!(
            prompt_marker(&trainer).is_empty(),
            "Find it marked the neck with its prompt"
        );
    }

    /// The other half of the neck's feedback: the position that scored is marked too, in
    /// the success colour, for as long as the flash lasts.
    #[test]
    fn a_right_cursor_position_marks_the_neck_green() {
        let mut app = find_it_app(0x9e);
        app.focused = FocusTarget::Fretboard;

        let Answer::Position { string, fret } = correct_answer(&app.note_trainer) else {
            unreachable!("find_it_app guarantees the direction")
        };
        app.note_trainer.cursor = (string, fret);

        press_named(&mut app, Named::Space);

        let markers = position_markers(&app.note_trainer);
        assert_eq!(markers.len(), 1);
        assert_eq!((markers[0].string, markers[0].fret), (string, fret));
        assert_eq!(markers[0].color, SUCCESS, "the right note is not green");
        assert_eq!(
            markers[0].style,
            MarkerStyle::Filled,
            "an answer went hollow"
        );

        // ...and the neck is clean again once the flash ends.
        let _ = app.update(Message::AdvancePrompt);
        assert!(position_markers(&app.note_trainer).is_empty());
    }

    /// The *Name it* half of the same thing: the button that scored is filled in the success
    /// colour, which is the state `note_answer_row` styles.
    #[test]
    fn a_right_answer_marks_its_button() {
        let mut app = note_trainer_app(0xb17);

        let Answer::Name(pitch_class) = correct_answer(&app.note_trainer) else {
            unreachable!("the Note Trainer opens in Name it")
        };

        let _ = app.update(Message::AnswerNote(pitch_class));
        assert_eq!(app.note_trainer.correct, Some(Answer::Name(pitch_class)));
        let _ = app.view();

        let _ = app.update(Message::AdvancePrompt);
        assert_eq!(app.note_trainer.correct, None);
    }

    /// The flash's tick is the clock talking, not the user, so it must not be spent
    /// dismissing the help overlay the way a keypress would be.
    #[test]
    fn the_flash_ends_behind_the_help_overlay() {
        let mut app = note_trainer_app(0x4e19);

        let standing = app.note_trainer.prompt;
        let Answer::Name(pitch_class) = correct_answer(&app.note_trainer) else {
            unreachable!()
        };
        let _ = app.update(Message::AnswerNote(pitch_class));

        app.help_open = true;
        let _ = app.update(Message::AdvancePrompt);

        assert!(app.help_open, "the tick dismissed the overlay");
        assert_ne!(app.note_trainer.prompt, standing, "the drill stalled");
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

            // ...and then the right one, so the flash is drawn on both surfaces too.
            let message = match correct_answer(&app.note_trainer) {
                Answer::Name(pitch_class) => Message::AnswerNote(pitch_class),
                Answer::Position { string, fret } => Message::ChooseNotePosition(string, fret),
            };
            let _ = app.update(message);
            assert!(app.note_trainer.correct.is_some());
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
