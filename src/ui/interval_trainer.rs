//! The Interval Trainer: the drill's state machine and the screen that draws it.
//!
//! Both halves live here for the reason `note_trainer` gives — the state keeps its fields
//! to itself, and the views below are the only things that read them. `App` drives the
//! drill through the methods marked `pub(super)` and never touches what is behind them.
//!
//! What sets this drill apart from the Note Trainer is what it refuses to say. The tonal
//! center is a *lit position* rather than a named key, and no note is named anywhere on the
//! screen, so a prompt is answerable by measuring the distance between two marks and by
//! nothing else. That is also why nothing here is spelled: with no key established there is
//! nothing for a spelling to be relative to, and the same fret is honestly both an
//! augmented fourth and a diminished fifth. Judging therefore compares semitone distance,
//! and the two tritone spellings are one answer — the same rule the Note Trainer already
//! applies to the two names of a black key, one level down.

use iced::{Color, Element, Padding};

use super::fretboard::{Fretboard, MarkerStyle, NoteMarker, fretboard};
use super::{
    ANSWER_ROW_WIDTH, BODY, CANVAS, CONTROL_SIZE, CURSOR_HOME, DANGER, Direction, Drill,
    FocusTarget, INK, LINK, MUSIC_FONT, MUTE, Message, NECK_FRETS, NECK_STRINGS, Position,
    ROOT_BUTTON_SIZE, ROOT_MARKER, SELECTOR_CARD_HEIGHT, SUCCESS, SUMMARY_CARD_HEIGHT,
    card_container, control_button, control_label, control_shuffle, correct_answer_button,
    focus_ring, ghost_button, interval_token, streak_readout, wrong_answer_button,
};
use crate::music::intervals::Interval;
use crate::music::notes::PitchClass;
use crate::rng::Rng;

/// Where the cursor starts, as a `Position`. The Note Trainer's `CURSOR_HOME` tuple stays
/// the one statement of *which* corner; this only reshapes it.
const CURSOR_START: Position = Position {
    string: CURSOR_HOME.0,
    fret: CURSOR_HOME.1,
};

/// One entry of the answer vocabulary.
///
/// `twin` is the other spelling of the same distance, where the neck cannot tell the two
/// apart — the tritone, and the two chord degrees the library added to `Interval`. Carrying
/// the twin as an `Interval` rather than as a written
/// label is what stops the button's text from drifting away from the interval it stands
/// for — the label is rendered from these two, never stored beside them.
struct AnswerChoice {
    interval: Interval,
    twin: Option<Interval>,
}

/// The twelve answers, in the order the grid draws them.
///
/// One table, and the grid, the focus rows, and the pool prompts are drawn from all read
/// it — the way `HOME_MENU` is the single source of the menu's buttons, focus cells, and
/// digit keys. `Interval::ALL` is fifteen; this is that list with each distance written two
/// ways collapsed into one entry, which is what makes the count divide evenly into
/// `ANSWER_ROW_WIDTH` and leaves no ragged final row.
const ANSWERS: [AnswerChoice; 12] = [
    AnswerChoice {
        interval: Interval::Unison,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::MinorSecond,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::MajorSecond,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::MinorThird,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::MajorThird,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::PerfectFourth,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::AugmentedFourth,
        twin: Some(Interval::DiminishedFifth),
    },
    AnswerChoice {
        interval: Interval::PerfectFifth,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::MinorSixth,
        twin: Some(Interval::AugmentedFifth),
    },
    AnswerChoice {
        interval: Interval::MajorSixth,
        twin: Some(Interval::DiminishedSeventh),
    },
    AnswerChoice {
        interval: Interval::MinorSeventh,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::MajorSeventh,
        twin: None,
    },
];

/// How many answers the grid offers. The focus grid reads this so the cells and the
/// buttons are built from one count and cannot drift apart.
pub(super) const ANSWER_COUNT: usize = ANSWERS.len();

/// The interval the grid's `index`-th button stands for, or `None` past the end.
///
/// `App` reaches the vocabulary through this rather than through the table itself, which
/// is what keeps `AnswerChoice` and its twin field private to this module — the parent
/// needs an interval to hand back, not the shape it was stored in.
pub(super) fn answer_at(index: usize) -> Option<Interval> {
    ANSWERS.get(index).map(|choice| choice.interval)
}

/// What the Interval Trainer is asking right now.
///
/// The variant is the drill direction, for the reason the Note Trainer's `Prompt` spells
/// out: two encodings of one fact could disagree.
///
/// `NameIt` deliberately does *not* also carry the interval it was generated from. The two
/// positions already imply the distance, and a stored `MajorThird` beside a pair of
/// positions four semitones apart is a state the view would render as a lie. Judging
/// recomputes the distance from what is actually on the neck, so it cannot disagree with
/// what the learner is looking at. `FindIt` must store its interval, because with no target
/// drawn there is nothing else carrying it — and its target pitch class is derived from the
/// root and that interval for the same one-encoding reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prompt {
    /// Two lit positions; the answer is the interval from the first to the second.
    NameIt { root: Position, target: Position },
    /// A lit root and a named interval; the answer is a position carrying it.
    FindIt { root: Position, interval: Interval },
}

/// What the user offered in reply to a `Prompt`, shaped like `Prompt` because the two
/// answer surfaces mirror the two directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    Interval(Interval),
    Position(Position),
}

impl Prompt {
    fn drill(self) -> Drill {
        match self {
            Prompt::NameIt { .. } => Drill::NameIt,
            Prompt::FindIt { .. } => Drill::FindIt,
        }
    }

    fn root(self) -> Position {
        match self {
            Prompt::NameIt { root, .. } | Prompt::FindIt { root, .. } => root,
        }
    }
}

/// Every position on the neck, paired with the pitch class sounding there.
///
/// The pitch class is carried along rather than asked for again later, which is what keeps
/// an `expect` out of the drawing code: a position that yielded `None` never enters the
/// list, so everything downstream has one in hand.
fn neck() -> Vec<(Position, PitchClass)> {
    (0..NECK_STRINGS)
        .flat_map(|string| (0..=NECK_FRETS).map(move |fret| Position { string, fret }))
        .filter_map(|position| position.pitch_class().map(|pitch| (position, pitch)))
        .collect()
}

/// The semitone distance upward from `root` to `target`, or `None` if either is off the
/// neck.
///
/// Upward and modulo the octave, so a target below the root on the neck still reads as the
/// interval it is above the root's pitch class. That is what the drill asks about: a
/// distance between two pitch classes, not a count of frets.
fn distance(root: Position, target: Position) -> Option<u8> {
    let root = root.pitch_class()?;
    let target = target.pitch_class()?;

    Some((target.semitone() + 12 - root.semitone()) % 12)
}

/// The Interval Trainer's whole state.
///
/// No iced types, so the streak rules and the judging can be tested without a widget tree.
pub(super) struct IntervalTrainer {
    prompt: Prompt,
    streak: u32,
    best_streak: u32,
    /// Every wrong answer given to the current prompt, cleared when it advances — a `Vec`
    /// for the reason the Note Trainer's is one: replacing the last wrong answer would
    /// un-mark the earlier ones.
    wrong: Vec<Answer>,
    /// The correct answer just given, held while it is lit. `Some` is the paused state.
    correct: Option<Answer>,
    cursor: Position,
}

impl IntervalTrainer {
    /// Takes the generator rather than owning one, for the borrow reason `NoteTrainer::new`
    /// documents: `&mut self.interval_trainer` and `&mut self.rng` are two mutable borrows
    /// of one `App`, and a second generator here would mean a second seed.
    pub(super) fn new(rng: &mut Rng) -> Self {
        let mut trainer = Self {
            // Replaced by `draw_prompt` before anyone sees it; it exists only to give the
            // rejection loop something to differ from.
            prompt: Prompt::NameIt {
                root: CURSOR_START,
                target: CURSOR_START,
            },
            streak: 0,
            best_streak: 0,
            wrong: Vec::new(),
            correct: None,
            cursor: CURSOR_START,
        };

        trainer.draw_prompt(Drill::NameIt, rng);
        trainer
    }

    /// Draws a fresh prompt of `drill`, never the one already showing.
    ///
    /// Generated forward — a root and an interval first, then the target derived from them
    /// — rather than by lighting two positions and asking afterwards what lies between
    /// them. Forward is what guarantees the distance drawn is one the answer grid can
    /// express; backward could land on a pair the vocabulary has no button for.
    fn draw_prompt(&mut self, drill: Drill, rng: &mut Rng) {
        let current = self.prompt;
        let neck = neck();

        // The rejection loop below ends only if there are at least two prompts to choose
        // between. There are 78 positions and 12 intervals, so this is generous — but a
        // future setting that pinned the root to one position would turn the loop into a
        // hang, and a hang inside a rejection loop is a miserable bug to find.
        debug_assert!(
            neck.len() * ANSWERS.len() >= 2,
            "the drill must have at least two prompts to choose between or this loop cannot end"
        );

        loop {
            let (root, root_pitch) = neck[rng.below(neck.len())];
            let interval = ANSWERS[rng.below(ANSWERS.len())].interval;

            let candidate = match drill {
                Drill::NameIt => {
                    // The root's own position is excluded, so a unison prompt lights an
                    // octave elsewhere on the neck rather than stacking both marks on one
                    // dot — which would show the learner a single ring and no distance to
                    // measure. Every pitch class occurs at least six times within twelve
                    // frets, so dropping one still leaves something to choose.
                    let carriers: Vec<Position> = neck
                        .iter()
                        .filter(|(position, pitch)| {
                            *pitch == root_pitch.transpose(interval.semitones())
                                && *position != root
                        })
                        .map(|(position, _)| *position)
                        .collect();

                    debug_assert!(
                        !carriers.is_empty(),
                        "every pitch class occurs several times on the neck, so an interval \
                         above any root always has somewhere to land"
                    );

                    Prompt::NameIt {
                        root,
                        target: carriers[rng.below(carriers.len())],
                    }
                }
                Drill::FindIt => Prompt::FindIt { root, interval },
            };

            if candidate != current {
                self.prompt = candidate;
                break;
            }
        }

        // Both kinds of feedback belong to the prompt they were given against, so a new
        // prompt arrives with a clean surface.
        self.wrong.clear();
        self.correct = None;
    }

    /// Judges by semitone distance, which is what makes the two tritone spellings one
    /// answer: with no key on screen, nothing distinguishes them and the drill must not
    /// pretend otherwise.
    fn judge(&self, answer: Answer) -> bool {
        match (self.prompt, answer) {
            (Prompt::NameIt { root, target }, Answer::Interval(named)) => {
                distance(root, target) == Some(named.semitones())
            }
            // Any position carrying the target pitch class counts, the same rule *Find it*
            // already follows in the Note Trainer: the note really is in several places,
            // and none of them is more correct than another.
            (Prompt::FindIt { root, interval }, Answer::Position(position)) => {
                match (root.pitch_class(), position.pitch_class()) {
                    (Some(root_pitch), Some(answered)) => {
                        root_pitch.transpose(interval.semitones()) == answered
                    }
                    // Off the neck. `false` rather than a `None == None` that would read as
                    // a match.
                    _ => false,
                }
            }
            // A mismatched pair is a wiring bug, not something a user can produce — the
            // view only draws the surface the current prompt accepts. `false` rather than a
            // panic, so the symptom is "every answer is wrong" instead of a crash.
            _ => false,
        }
    }

    /// Takes an answer, unless one is already being marked.
    fn answer(&mut self, answer: Answer) {
        // A correct answer is on screen, so the drill is not asking anything. Answers
        // arriving now are the tail of the press that scored — a held key, a double click.
        if self.correct.is_some() {
            return;
        }

        if self.judge(answer) {
            self.streak += 1;
            self.best_streak = self.best_streak.max(self.streak);

            // The prompt is *not* replaced here. It stands, with this answer lit, until
            // `advance` retires it.
            self.correct = Some(answer);
        } else {
            // Deduplicated so hammering one wrong button cannot grow this without bound.
            if !self.wrong.contains(&answer) {
                self.wrong.push(answer);
            }
            self.streak = 0;
        }
    }

    /// Answers with an interval — the *Name it* surface.
    ///
    /// `Answer` is built here rather than by the caller, which is what keeps it and
    /// `Prompt` private to this module.
    pub(super) fn answer_interval(&mut self, interval: Interval) {
        self.answer(Answer::Interval(interval));
    }

    /// Answers with a position, and takes the cursor there, so the mouse and the keyboard
    /// never disagree about where it is.
    pub(super) fn answer_position(&mut self, string: usize, fret: usize) {
        let position = Position { string, fret };
        self.cursor = position;
        self.answer(Answer::Position(position));
    }

    /// Answers with wherever the cursor is sitting — what Enter on the neck does.
    pub(super) fn answer_at_cursor(&mut self) {
        self.answer(Answer::Position(self.cursor));
    }

    /// Whether a correct answer is lit, which is the window the flash timer runs in.
    pub(super) fn is_flashing(&self) -> bool {
        self.correct.is_some()
    }

    /// Which way the drill is running. The focus grid asks, because the answer surface
    /// differs between the two directions.
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

    /// Zeroes the run without touching the best of the session. A wrong answer, a skip, and
    /// a direction change all break it, so the number measures recall rather than time.
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
        self.cursor = CURSOR_START;
        self.draw_prompt(flipped, rng);
    }

    /// Opening the screen: the direction and the best streak persist, the run does not.
    pub(super) fn enter(&mut self, rng: &mut Rng) {
        let drill = self.prompt.drill();
        self.break_streak();
        self.cursor = CURSOR_START;
        self.draw_prompt(drill, rng);
    }

    /// Walks the cursor one position, stopping at the neck's edges rather than wrapping.
    /// Up is towards the nut, because the neck is drawn with the nut at the top.
    pub(super) fn move_cursor(&mut self, direction: Direction) {
        let Position { string, fret } = self.cursor;

        self.cursor = match direction {
            Direction::Left => Position {
                string: string.saturating_sub(1),
                fret,
            },
            Direction::Right => Position {
                string: (string + 1).min(NECK_STRINGS - 1),
                fret,
            },
            Direction::Up => Position {
                string,
                fret: fret.saturating_sub(1),
            },
            Direction::Down => Position {
                string,
                fret: (fret + 1).min(NECK_FRETS),
            },
        };
    }
}

/// How wide one answer button is.
///
/// Wider than the Note Trainer's square `ROOT_BUTTON_SIZE` buttons because the twinned
/// labels are twice the length of the others — they name both spellings — and a grid whose
/// columns changed width with their contents would stop reading as a grid.
const ANSWER_BUTTON_WIDTH: f32 = 76.0;

/// The Interval Trainer.
///
/// Both directions share this one function, branching on the prompt's variant, because the
/// header, the streak, and the neck are common to both — only the answer surface differs.
pub(super) fn ui_interval_trainer(
    trainer: &IntervalTrainer,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, column, container, row, text};

    let neck = match trainer.prompt {
        // Two rings and nothing else — see `prompt_markers`.
        Prompt::NameIt { .. } => Fretboard {
            num_frets: NECK_FRETS,
            highlighted: prompt_markers(trainer),
            ..Fretboard::default()
        },
        // Here the neck is the answer surface, so it takes a press handler and shows the
        // cursor. The root ring stays on it: it is the thing the interval is measured from,
        // and hiding it would leave the prompt with no anchor.
        Prompt::FindIt { .. } => Fretboard {
            num_frets: NECK_FRETS,
            highlighted: prompt_markers(trainer)
                .into_iter()
                .chain(position_markers(trainer))
                .collect(),
            cursor: Some((trainer.cursor.string, trainer.cursor.fret)),
            on_press: Some(Message::ChooseIntervalPosition),
        },
    };

    let question: Element<'static, Message> = match trainer.prompt {
        Prompt::NameIt { .. } => column![
            text("How far is this?").size(32).color(INK),
            text("Measure up from the amber root").size(16).color(MUTE),
        ]
        .spacing(6)
        .into(),
        Prompt::FindIt { interval, .. } => column![
            row![
                text("Find").size(26).color(BODY),
                interval_label(&choice_for(interval), 34, INK),
            ]
            .spacing(12),
            text("Press any fret that far above the root")
                .size(16)
                .color(MUTE),
        ]
        .spacing(6)
        .into(),
    };

    let prompt_card = container(
        column![
            row![
                question,
                Space::new().width(Length::Fill),
                streak_readout(trainer.streak, trainer.best_streak),
            ],
            interval_trainer_controls(trainer, focused),
        ]
        .spacing(20),
    )
    .width(Length::Fill)
    .height(Length::Fixed(SUMMARY_CARD_HEIGHT))
    .padding(32)
    .style(card_container);

    // Only *Name it* has a second card; in *Find it* the neck already is the answer
    // surface, so nothing goes here and the prompt card gets the room.
    let details = match trainer.prompt {
        Prompt::NameIt { .. } => column![prompt_card, interval_answer_card(trainer, focused)],
        Prompt::FindIt { .. } => column![prompt_card],
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

/// The vocabulary entry for an interval — the one the grid would draw for it.
///
/// Two spellings of one distance share an entry, so `DiminishedFifth` resolves to the
/// `AugmentedFourth` entry that names them both. Falls back to a twinless entry for an
/// interval outside the vocabulary, which the drill cannot produce but which spares this
/// from being fallible for a case no caller could act on.
fn choice_for(interval: Interval) -> AnswerChoice {
    ANSWERS
        .iter()
        .find(|choice| {
            choice.interval == interval || choice.twin.is_some_and(|twin| twin == interval)
        })
        .map_or(
            AnswerChoice {
                interval,
                twin: None,
            },
            |choice| AnswerChoice {
                interval: choice.interval,
                twin: choice.twin,
            },
        )
}

/// The prompt's marks on the neck: the root always, and in *Name it* the target too.
///
/// Both are unfilled rings, for the reason `note_trainer::prompt_marker` gives — a fill is
/// what this app uses to say "you put this here", and the prompt is the opposite. The two
/// are told apart by colour rather than by shape, the way the wrong-red and correct-green
/// discs already are.
///
/// Unlabelled, and that is load-bearing rather than cosmetic: a label would name a note,
/// and the whole point of this drill is that it can be answered without one.
fn prompt_markers(trainer: &IntervalTrainer) -> Vec<NoteMarker> {
    let ring = |position: Position, color: Color| NoteMarker {
        string: position.string,
        fret: position.fret,
        label: String::new(),
        color,
        style: MarkerStyle::Outlined,
    };

    let root = ring(trainer.prompt.root(), ROOT_MARKER);

    match trainer.prompt {
        Prompt::NameIt { target, .. } => vec![root, ring(target, LINK)],
        Prompt::FindIt { .. } => vec![root],
    }
}

/// The positions guessed against the current *Find it* prompt: the wrong ones in the danger
/// colour, the one that scored in the success colour.
///
/// Filled, both of them — that is what keeps them readable as answers rather than as the
/// question. The rings belong to the prompt alone; see `prompt_markers`.
///
/// The right answer comes last so it is drawn over the wrong ones. Only `Answer::Position`
/// guesses can appear on a neck; an `Interval` guess belongs to the other direction and is
/// filtered out rather than being an error, since both lists are cleared whenever the
/// prompt advances and the two can never mix in practice.
fn position_markers(trainer: &IntervalTrainer) -> Vec<NoteMarker> {
    let marker = |answer: &Answer, color: Color| match *answer {
        Answer::Position(position) => Some(NoteMarker {
            string: position.string,
            fret: position.fret,
            label: String::new(),
            color,
            style: MarkerStyle::Filled,
        }),
        Answer::Interval(_) => None,
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

/// The header row: direction and skip.
///
/// The direction is labelled with the mode it is *currently* in rather than with what
/// pressing it would do, so the row doubles as a status line — there is nowhere else on
/// this screen that says which way the drill is running.
fn interval_trainer_controls(
    trainer: &IntervalTrainer,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::widget::{row, text};

    let direction = match trainer.prompt.drill() {
        Drill::NameIt => "name it",
        Drill::FindIt => "find it",
    };

    row![
        control_button(
            control_label(text(direction), CONTROL_SIZE),
            Message::ToggleIntervalDirection,
            focused == FocusTarget::IntervalDirectionToggle,
        ),
        control_button(
            control_shuffle(),
            Message::SkipIntervalPrompt,
            focused == FocusTarget::SkipIntervalPrompt,
        ),
    ]
    .spacing(8)
    .into()
}

/// The twelve answer buttons, in three rows of `ANSWER_ROW_WIDTH`.
fn interval_answer_card(
    trainer: &IntervalTrainer,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{column, container};

    let rows =
        (0..ANSWERS.len())
            .step_by(ANSWER_ROW_WIDTH)
            .fold(column![].spacing(16), |rows, start| {
                let len = ANSWER_ROW_WIDTH.min(ANSWERS.len() - start);

                rows.push(
                    container(interval_answer_row(start, len, trainer, focused))
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

fn interval_answer_row(
    start_index: usize,
    len: usize,
    trainer: &IntervalTrainer,
    focused: FocusTarget,
) -> iced::widget::Row<'static, Message> {
    use iced::Length;
    use iced::widget::{button, container, row};

    ANSWERS[start_index..start_index + len]
        .iter()
        .enumerate()
        .fold(row![].spacing(20), |acc, (i, choice)| {
            let answer = Answer::Interval(choice.interval);
            let was_wrong = trainer.wrong.contains(&answer);
            let was_right = trainer.correct == Some(answer);
            // Both marked states fill the button, so both need ink that reads on a fill.
            let color = if was_wrong || was_right { CANVAS } else { INK };

            let answer_button = button(
                container(interval_label(choice, 22, color))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            )
            .width(Length::Fixed(ANSWER_BUTTON_WIDTH))
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
            .on_press(Message::AnswerInterval(choice.interval));

            acc.push(focus_ring(
                answer_button,
                focused == FocusTarget::IntervalAnswer(start_index + i),
            ))
        })
}

/// One answer's label: its degree formula, and the twin spelling after a slash where there
/// is one.
///
/// Built the way `intervalic_text` builds the scale trainer's formula row, and for the same
/// reason: the accidental glyph renders in `MUSIC_FONT` and the degree digit in the body
/// font, and one `text` widget cannot carry both. The twinned entries run this twice.
fn interval_label(
    choice: &AnswerChoice,
    size: u32,
    color: Color,
) -> iced::widget::Row<'static, Message> {
    use iced::widget::{row, text};

    let token = |tokens: iced::widget::Row<'static, Message>, interval: Interval| {
        let (glyph, digit) = interval_token(interval);
        let mut tokens = tokens;

        if let Some(glyph) = glyph {
            tokens = tokens.push(
                text(glyph.to_string())
                    .size(size)
                    .font(MUSIC_FONT)
                    .color(color),
            );
        }

        tokens.push(text(digit.to_string()).size(size).color(color))
    };

    let label = token(row![].spacing(0), choice.interval);

    match choice.twin {
        Some(twin) => token(label.push(text("/").size(size).color(color)), twin),
        None => label,
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{app_with_seed, press_into};
    use super::super::{App, Notation, Screen, accelerators_for};
    use super::*;
    use iced::keyboard;

    // ---- The answer vocabulary ----

    #[test]
    fn the_vocabulary_holds_one_button_per_distance() {
        assert_eq!(ANSWERS.len(), 12);
        assert_eq!(ANSWER_COUNT, ANSWERS.len());

        // Twelve entries and twelve distinct distances is the whole twin decision stated as
        // an invariant: judging compares semitones, so two buttons standing for one distance
        // would be two spellings of the same answer with no way to tell which the drill
        // meant.
        let mut distances: Vec<u8> = ANSWERS
            .iter()
            .map(|choice| choice.interval.semitones())
            .collect();
        distances.sort_unstable();
        distances.dedup();

        assert_eq!(distances.len(), ANSWERS.len(), "two buttons, one distance");
    }

    #[test]
    fn the_vocabulary_covers_every_distance_in_an_octave() {
        let mut distances: Vec<u8> = ANSWERS
            .iter()
            .map(|choice| choice.interval.semitones())
            .collect();
        distances.sort_unstable();

        // 0..=11 exactly: every distance a prompt can produce has a button, so judging by
        // semitones can never leave a prompt unanswerable.
        assert_eq!(distances, (0..12).collect::<Vec<u8>>());
    }

    #[test]
    fn the_vocabulary_is_interval_all_with_the_tritones_merged() {
        let named: Vec<Interval> = ANSWERS
            .iter()
            .flat_map(|choice| std::iter::once(choice.interval).chain(choice.twin))
            .collect();

        // Every interval still reachable, one of them through a twin — this is the tripwire
        // that a variant added to `Interval` is added here too.
        assert_eq!(named.len(), Interval::ALL.len());
        for interval in Interval::ALL {
            assert!(named.contains(interval), "{interval} has no button");
        }
    }

    #[test]
    fn a_twin_is_the_same_distance_written_a_second_way() {
        let twinned: Vec<&AnswerChoice> = ANSWERS.iter().filter(|c| c.twin.is_some()).collect();

        assert_eq!(twinned.len(), 3);

        for choice in twinned {
            let twin = choice.twin.expect("filtered to the twinned entries");

            // What earns an entry a twin, and the only thing that does: the neck cannot
            // separate two spellings of one distance, so the drill offers them as one
            // answer. A pair differing in distance would be two answers wrongly merged.
            assert_eq!(
                choice.interval.semitones(),
                twin.semitones(),
                "{} and {twin} are different distances",
                choice.interval
            );
            assert_ne!(
                choice.interval.number(),
                twin.number(),
                "{} and {twin} are the same degree",
                choice.interval
            );
        }
    }

    #[test]
    fn a_button_index_resolves_to_its_interval() {
        for (index, choice) in ANSWERS.iter().enumerate() {
            assert_eq!(answer_at(index), Some(choice.interval));
        }
        assert_eq!(answer_at(ANSWERS.len()), None);
    }

    #[test]
    fn either_tritone_spelling_finds_the_one_entry() {
        // Both resolve to the entry naming both, so a *Find it* prompt for either draws the
        // same label.
        for spelling in [Interval::AugmentedFourth, Interval::DiminishedFifth] {
            let choice = choice_for(spelling);
            assert_eq!(choice.interval, Interval::AugmentedFourth);
            assert_eq!(choice.twin, Some(Interval::DiminishedFifth));
        }
    }

    // ---- The neck the drill draws from ----

    #[test]
    fn the_neck_is_every_position_and_nothing_off_it() {
        let neck = neck();

        assert_eq!(neck.len(), NECK_STRINGS * (NECK_FRETS + 1));
        for (position, pitch) in &neck {
            assert!(position.string < NECK_STRINGS);
            assert!(position.fret <= NECK_FRETS);
            assert_eq!(position.pitch_class(), Some(*pitch));
        }
    }

    /// The spec's "a prompted interval is always reachable": every root paired with every
    /// interval has somewhere to land, and somewhere other than the root itself, which is
    /// what `draw_prompt`'s `debug_assert!` relies on.
    #[test]
    fn every_interval_above_every_root_lands_somewhere_else() {
        let neck = neck();

        for (root, root_pitch) in &neck {
            for choice in &ANSWERS {
                let target = root_pitch.transpose(choice.interval.semitones());
                let carriers = neck
                    .iter()
                    .filter(|(position, pitch)| *pitch == target && position != root)
                    .count();

                assert!(
                    carriers > 0,
                    "{:?} has nowhere to put {}",
                    root,
                    choice.interval
                );
            }
        }
    }

    #[test]
    fn distance_is_measured_upward_around_the_octave() {
        // The open low E and the E an octave up: a unison by pitch class, twelve frets apart.
        let root = Position { string: 0, fret: 0 };
        assert_eq!(
            distance(
                root,
                Position {
                    string: 0,
                    fret: 12
                }
            ),
            Some(0)
        );
        // Four frets up is a major third; the same pitch class reached on another string
        // reads the same, because the drill asks about pitch classes and not fret counts.
        assert_eq!(distance(root, Position { string: 0, fret: 4 }), Some(4));
        assert_eq!(
            distance(
                root,
                Position {
                    string: 1,
                    fret: 11
                }
            ),
            Some(4)
        );
        // And a target *below* the root still reads as the interval it is above it.
        let high = Position { string: 0, fret: 5 };
        assert_eq!(distance(high, Position { string: 0, fret: 0 }), Some(7));
    }

    #[test]
    fn distance_is_none_off_the_neck() {
        let root = Position { string: 0, fret: 0 };
        assert_eq!(
            distance(
                root,
                Position {
                    string: 0,
                    fret: NECK_FRETS + 1
                }
            ),
            None
        );
        assert_eq!(
            distance(
                Position {
                    string: NECK_STRINGS,
                    fret: 0
                },
                root
            ),
            None
        );
    }

    // ---- The drill ----

    fn trainer_with_seed(seed: u64) -> (IntervalTrainer, Rng) {
        let mut rng = Rng::from_seed(seed);
        let trainer = IntervalTrainer::new(&mut rng);
        (trainer, rng)
    }

    /// The interval a *Name it* prompt is standing on.
    fn prompted_distance(trainer: &IntervalTrainer) -> u8 {
        match trainer.prompt {
            Prompt::NameIt { root, target } => {
                distance(root, target).expect("a prompt's positions are on the neck")
            }
            Prompt::FindIt { interval, .. } => interval.semitones(),
        }
    }

    /// An answer the current prompt accepts.
    fn correct_answer(trainer: &IntervalTrainer) -> Answer {
        match trainer.prompt {
            Prompt::NameIt { .. } => {
                let wanted = prompted_distance(trainer);
                let choice = ANSWERS
                    .iter()
                    .find(|choice| choice.interval.semitones() == wanted)
                    .expect("the vocabulary covers every distance");

                Answer::Interval(choice.interval)
            }
            Prompt::FindIt { root, interval } => {
                let target = root
                    .pitch_class()
                    .expect("a prompt's root is on the neck")
                    .transpose(interval.semitones());

                Answer::Position(
                    neck()
                        .into_iter()
                        .find(|(_, pitch)| *pitch == target)
                        .map(|(position, _)| position)
                        .expect("every pitch class is somewhere on the neck"),
                )
            }
        }
    }

    /// An answer of the right shape that the current prompt rejects.
    fn wrong_answer(trainer: &IntervalTrainer) -> Answer {
        match trainer.prompt {
            Prompt::NameIt { .. } => {
                let wanted = prompted_distance(trainer);
                let choice = ANSWERS
                    .iter()
                    .find(|choice| choice.interval.semitones() != wanted)
                    .expect("twelve distances, so one of them differs");

                Answer::Interval(choice.interval)
            }
            Prompt::FindIt { root, interval } => {
                let target = root
                    .pitch_class()
                    .expect("a prompt's root is on the neck")
                    .transpose(interval.semitones());

                Answer::Position(
                    neck()
                        .into_iter()
                        .find(|(_, pitch)| *pitch != target)
                        .map(|(position, _)| position)
                        .expect("the neck holds more than one pitch class"),
                )
            }
        }
    }

    fn answer_and_advance(trainer: &mut IntervalTrainer, rng: &mut Rng) {
        trainer.answer(correct_answer(trainer));
        trainer.advance(rng);
    }

    #[test]
    fn a_fresh_prompt_is_never_the_one_it_replaces() {
        let (mut trainer, mut rng) = trainer_with_seed(0x17);

        for drill in [Drill::NameIt, Drill::FindIt] {
            for _ in 0..500 {
                let before = trainer.prompt;
                trainer.draw_prompt(drill, &mut rng);
                assert_ne!(trainer.prompt, before);
            }
        }
    }

    #[test]
    fn prompts_are_reproducible_from_a_seed() {
        let sequence = |seed: u64| {
            let (mut trainer, mut rng) = trainer_with_seed(seed);
            (0..30)
                .map(|_| {
                    trainer.draw_prompt(Drill::NameIt, &mut rng);
                    trainer.prompt
                })
                .collect::<Vec<_>>()
        };

        assert_eq!(sequence(7), sequence(7));
        assert_ne!(sequence(7), sequence(8));
    }

    /// Both marks have to be somewhere the learner can see, and they have to be two marks:
    /// a unison prompt stacked on one dot would show a ring and no distance to measure.
    #[test]
    fn every_prompt_is_on_the_neck_and_shows_two_marks() {
        let (mut trainer, mut rng) = trainer_with_seed(0xd07);

        for _ in 0..2_000 {
            trainer.draw_prompt(Drill::NameIt, &mut rng);

            let Prompt::NameIt { root, target } = trainer.prompt else {
                unreachable!()
            };

            assert!(root.pitch_class().is_some());
            assert!(target.pitch_class().is_some());
            assert_ne!(root, target, "both rings landed on one position");

            trainer.draw_prompt(Drill::FindIt, &mut rng);

            let Prompt::FindIt { root, .. } = trainer.prompt else {
                unreachable!()
            };
            assert!(root.pitch_class().is_some());
        }
    }

    #[test]
    fn the_two_tritone_spellings_are_one_answer() {
        let (mut trainer, _) = trainer_with_seed(0x77);

        // A tritone prompt, built rather than waited for: the low E and the B flat above it.
        trainer.prompt = Prompt::NameIt {
            root: Position { string: 0, fret: 0 },
            target: Position { string: 0, fret: 6 },
        };

        for spelling in [Interval::AugmentedFourth, Interval::DiminishedFifth] {
            assert!(
                trainer.judge(Answer::Interval(spelling)),
                "{spelling} was rejected"
            );
        }

        // And a distance that is not six is still wrong, so the merge did not soften judging.
        assert!(!trainer.judge(Answer::Interval(Interval::PerfectFifth)));
    }

    #[test]
    fn find_it_accepts_every_position_carrying_the_interval() {
        let (mut trainer, mut rng) = trainer_with_seed(0xf1d);
        trainer.draw_prompt(Drill::FindIt, &mut rng);

        let Prompt::FindIt { root, interval } = trainer.prompt else {
            unreachable!()
        };
        let target = root
            .pitch_class()
            .expect("the root is on the neck")
            .transpose(interval.semitones());

        let carriers: Vec<Position> = neck()
            .into_iter()
            .filter(|(_, pitch)| *pitch == target)
            .map(|(position, _)| position)
            .collect();

        // A pitch class really is in several places within twelve frets, and none of them is
        // more correct than another.
        assert!(carriers.len() >= 6, "only {} carriers", carriers.len());
        for position in carriers {
            assert!(trainer.judge(Answer::Position(position)), "{position:?}");
        }
    }

    #[test]
    fn an_answer_of_the_wrong_shape_is_simply_wrong() {
        let (mut trainer, mut rng) = trainer_with_seed(0x5ae);

        trainer.draw_prompt(Drill::NameIt, &mut rng);
        assert!(!trainer.judge(Answer::Position(Position { string: 0, fret: 0 })));

        trainer.draw_prompt(Drill::FindIt, &mut rng);
        assert!(!trainer.judge(Answer::Interval(Interval::MajorThird)));
    }

    #[test]
    fn an_answer_off_the_neck_is_wrong() {
        let (mut trainer, mut rng) = trainer_with_seed(0x0ff);
        trainer.draw_prompt(Drill::FindIt, &mut rng);

        assert!(!trainer.judge(Answer::Position(Position {
            string: 0,
            fret: NECK_FRETS + 1
        })));
    }

    #[test]
    fn consecutive_correct_answers_raise_the_streak() {
        let (mut trainer, mut rng) = trainer_with_seed(0x57ea);

        for expected in 1..=10 {
            answer_and_advance(&mut trainer, &mut rng);
            assert_eq!(trainer.streak, expected);
            assert_eq!(trainer.best_streak, expected);
        }
    }

    #[test]
    fn a_correct_answer_is_marked_and_holds_the_prompt() {
        let (mut trainer, mut rng) = trainer_with_seed(0x11);

        let standing = trainer.prompt;
        let answer = correct_answer(&trainer);
        trainer.answer(answer);

        assert_eq!(trainer.prompt, standing, "the prompt moved under the flash");
        assert_eq!(trainer.correct, Some(answer));
        assert!(trainer.is_flashing());

        // A second answer during the flash changes nothing — a held key or a double click.
        trainer.answer(wrong_answer(&trainer));
        assert_eq!(trainer.streak, 1);
        assert!(trainer.wrong.is_empty());

        trainer.advance(&mut rng);
        assert_ne!(trainer.prompt, standing);
        assert!(!trainer.is_flashing());
    }

    #[test]
    fn advancing_without_a_flash_does_nothing() {
        let (mut trainer, mut rng) = trainer_with_seed(0xad);

        let standing = trainer.prompt;
        trainer.advance(&mut rng);

        assert_eq!(trainer.prompt, standing);
    }

    #[test]
    fn a_wrong_answer_zeroes_the_streak_and_keeps_the_prompt() {
        let (mut trainer, mut rng) = trainer_with_seed(0x9);

        answer_and_advance(&mut trainer, &mut rng);
        assert_eq!(trainer.streak, 1);

        let standing = trainer.prompt;
        let wrong = wrong_answer(&trainer);
        trainer.answer(wrong);

        assert_eq!(trainer.streak, 0);
        assert_eq!(trainer.best_streak, 1);
        assert_eq!(trainer.prompt, standing);
        assert_eq!(trainer.wrong, vec![wrong]);
    }

    #[test]
    fn wrong_answers_accumulate_until_the_prompt_advances() {
        let (mut trainer, mut rng) = trainer_with_seed(0xacc);
        trainer.draw_prompt(Drill::NameIt, &mut rng);

        let wanted = prompted_distance(&trainer);
        let wrong: Vec<Answer> = ANSWERS
            .iter()
            .filter(|choice| choice.interval.semitones() != wanted)
            .take(3)
            .map(|choice| Answer::Interval(choice.interval))
            .collect();

        for answer in &wrong {
            trainer.answer(*answer);
            // Twice, so the deduplication is what is under test rather than the count.
            trainer.answer(*answer);
        }

        assert_eq!(trainer.wrong, wrong, "earlier wrong answers were lost");

        trainer.answer(correct_answer(&trainer));
        trainer.advance(&mut rng);
        assert!(trainer.wrong.is_empty(), "a new prompt started dirty");
    }

    #[test]
    fn the_best_streak_survives_what_the_current_one_does_not() {
        let (mut trainer, mut rng) = trainer_with_seed(0xbe57);

        for _ in 0..4 {
            answer_and_advance(&mut trainer, &mut rng);
        }
        assert_eq!(trainer.best_streak, 4);

        trainer.skip(&mut rng);
        assert_eq!(
            trainer.streak, 0,
            "skipping past what you do not know counted"
        );
        assert_eq!(trainer.best_streak, 4);

        answer_and_advance(&mut trainer, &mut rng);
        trainer.toggle_direction(&mut rng);
        assert_eq!(trainer.streak, 0);
        assert_eq!(trainer.best_streak, 4);

        trainer.enter(&mut rng);
        assert_eq!(trainer.streak, 0);
        assert_eq!(
            trainer.best_streak, 4,
            "reopening forgot the best of the run"
        );
    }

    #[test]
    fn toggling_the_direction_flips_which_way_the_drill_runs() {
        let (mut trainer, mut rng) = trainer_with_seed(0xd17);

        let first = trainer.drill();
        trainer.toggle_direction(&mut rng);
        assert_eq!(trainer.drill(), first.flipped());
        trainer.toggle_direction(&mut rng);
        assert_eq!(trainer.drill(), first);
    }

    #[test]
    fn a_skip_keeps_the_direction() {
        let (mut trainer, mut rng) = trainer_with_seed(0x51);

        trainer.draw_prompt(Drill::FindIt, &mut rng);
        trainer.skip(&mut rng);
        assert_eq!(trainer.drill(), Drill::FindIt);
    }

    #[test]
    fn the_cursor_stops_at_the_necks_edges() {
        let (mut trainer, _) = trainer_with_seed(0xc);

        for _ in 0..NECK_STRINGS + 4 {
            trainer.move_cursor(Direction::Left);
            trainer.move_cursor(Direction::Up);
        }
        assert_eq!(trainer.cursor, Position { string: 0, fret: 0 });

        for _ in 0..NECK_STRINGS + NECK_FRETS + 4 {
            trainer.move_cursor(Direction::Right);
            trainer.move_cursor(Direction::Down);
        }
        assert_eq!(
            trainer.cursor,
            Position {
                string: NECK_STRINGS - 1,
                fret: NECK_FRETS
            }
        );
    }

    #[test]
    fn the_cursor_walks_one_position_at_a_time() {
        let (mut trainer, _) = trainer_with_seed(0xa);

        trainer.move_cursor(Direction::Down);
        assert_eq!(trainer.cursor, Position { string: 0, fret: 1 });
        trainer.move_cursor(Direction::Right);
        assert_eq!(trainer.cursor, Position { string: 1, fret: 1 });
        trainer.move_cursor(Direction::Up);
        assert_eq!(trainer.cursor, Position { string: 1, fret: 0 });
        trainer.move_cursor(Direction::Left);
        assert_eq!(trainer.cursor, Position { string: 0, fret: 0 });
    }

    // ---- What the neck is asked to draw ----

    #[test]
    fn a_name_it_prompt_is_two_unlabelled_rings() {
        let (mut trainer, mut rng) = trainer_with_seed(0x2195);
        trainer.draw_prompt(Drill::NameIt, &mut rng);

        let Prompt::NameIt { root, target } = trainer.prompt else {
            unreachable!()
        };

        let markers = prompt_markers(&trainer);
        assert_eq!(markers.len(), 2);

        // The root first, so its colour is the one the learner reads as the anchor.
        assert_eq!(
            (markers[0].string, markers[0].fret),
            (root.string, root.fret)
        );
        assert_eq!(markers[0].color, ROOT_MARKER);
        assert_eq!(
            (markers[1].string, markers[1].fret),
            (target.string, target.fret)
        );
        assert_eq!(markers[1].color, LINK);

        for marker in &markers {
            // Outlined, so neither reads as an answer already placed.
            assert_eq!(marker.style, MarkerStyle::Outlined);
            // Unlabelled, which is what keeps the drill answerable without note names.
            assert!(marker.label.is_empty());
        }
    }

    #[test]
    fn a_find_it_prompt_marks_only_the_root() {
        let (mut trainer, mut rng) = trainer_with_seed(0x4f1);
        trainer.draw_prompt(Drill::FindIt, &mut rng);

        let markers = prompt_markers(&trainer);
        assert_eq!(markers.len(), 1, "the target was given away");
        assert_eq!(markers[0].color, ROOT_MARKER);
        assert_eq!(markers[0].style, MarkerStyle::Outlined);
        assert!(markers[0].label.is_empty());
    }

    /// The spec's "the two kinds of mark are never confused", carried onto this screen.
    #[test]
    fn answer_marks_are_filled_and_the_prompt_is_not() {
        let (mut trainer, mut rng) = trainer_with_seed(0x11ed);
        trainer.draw_prompt(Drill::FindIt, &mut rng);

        let wrong = wrong_answer(&trainer);
        trainer.answer(wrong);
        trainer.answer(correct_answer(&trainer));

        let answers = position_markers(&trainer);
        assert_eq!(answers.len(), 2);
        assert_eq!(answers[0].color, DANGER);
        assert_eq!(answers[1].color, SUCCESS, "the right answer is drawn last");

        for marker in &answers {
            assert_eq!(marker.style, MarkerStyle::Filled);
        }

        for marker in prompt_markers(&trainer) {
            assert_eq!(marker.style, MarkerStyle::Outlined);
        }
    }

    #[test]
    fn an_interval_answer_never_reaches_the_neck() {
        let (mut trainer, mut rng) = trainer_with_seed(0x17);
        trainer.draw_prompt(Drill::NameIt, &mut rng);

        trainer.answer(wrong_answer(&trainer));
        assert!(
            position_markers(&trainer).is_empty(),
            "a named interval was drawn as a dot"
        );
    }

    // ---- The screen, driven through App ----

    /// An app sitting on the Interval Trainer with a reproducible prompt stream.
    fn interval_app(seed: u64) -> App {
        let mut app = app_with_seed(seed);
        app.open(Screen::IntervalTrainer);
        app
    }

    /// Walks the app into *Find it*, where the neck is the answer surface.
    fn find_it_app(seed: u64) -> App {
        let mut app = interval_app(seed);
        app.interval_trainer.toggle_direction(&mut app.rng);
        app.reset_focus();
        assert_eq!(app.interval_trainer.drill(), Drill::FindIt);
        app
    }

    /// Answers correctly through whichever message the view would send for the direction in
    /// play, then lets the flash's timer message land.
    fn answer_correctly(app: &mut App) {
        let message = match correct_answer(&app.interval_trainer) {
            Answer::Interval(interval) => Message::AnswerInterval(interval),
            Answer::Position(position) => {
                Message::ChooseIntervalPosition(position.string, position.fret)
            }
        };

        let _ = app.update(message);
        let _ = app.update(Message::AdvanceIntervalPrompt);
    }

    #[test]
    fn opening_the_interval_trainer_lands_on_a_prompt() {
        let app = interval_app(3);

        assert_eq!(app.screen, Screen::IntervalTrainer);
        assert_eq!(app.interval_trainer.streak, 0);
        assert!(app.interval_trainer.wrong.is_empty());
    }

    /// The screen never reopens on what it last showed, and what persists is the direction
    /// and the best of the session.
    #[test]
    fn reopening_draws_a_fresh_prompt_and_keeps_the_settings() {
        let mut app = find_it_app(0xfa17);

        answer_correctly(&mut app);
        answer_correctly(&mut app);
        let best = app.interval_trainer.best_streak;
        assert_eq!(best, 2);

        for _ in 0..20 {
            let before = app.interval_trainer.prompt;
            let _ = app.update(Message::GoBack);
            let _ = app.update(Message::Navigate(Screen::IntervalTrainer));

            assert_ne!(app.interval_trainer.prompt, before, "reopened on its last");
            assert_eq!(app.interval_trainer.drill(), Drill::FindIt);
            assert_eq!(app.interval_trainer.best_streak, best);
            assert_eq!(app.interval_trainer.streak, 0, "the run carried over");
        }
    }

    #[test]
    fn the_answer_buttons_are_reachable_in_name_it() {
        let app = interval_app(0xb7);
        assert_eq!(app.interval_trainer.drill(), Drill::NameIt);

        let focusables = app.focusables();
        for index in 0..ANSWER_COUNT {
            assert!(
                focusables.contains(&FocusTarget::IntervalAnswer(index)),
                "button {index} is unreachable"
            );
        }
        assert!(!focusables.contains(&FocusTarget::IntervalFretboard));
    }

    #[test]
    fn tab_reaches_the_neck_in_find_it() {
        let app = find_it_app(0x7ab);

        let focusables = app.focusables();
        assert!(focusables.contains(&FocusTarget::IntervalFretboard));
        assert!(
            !focusables
                .iter()
                .any(|target| matches!(target, FocusTarget::IntervalAnswer(_))),
            "the answer grid is not on screen in find it"
        );
    }

    #[test]
    fn the_arrows_drive_the_cursor_not_the_ring() {
        let mut app = find_it_app(0xa77);
        app.focused = FocusTarget::IntervalFretboard;

        let before = app.interval_trainer.cursor;
        let _ = app.update(Message::FocusDown);

        assert_eq!(app.focused, FocusTarget::IntervalFretboard, "focus moved");
        assert_ne!(app.interval_trainer.cursor, before, "the cursor stayed put");
    }

    #[test]
    fn enter_on_the_neck_answers_with_the_cursor() {
        let mut app = find_it_app(0xe17);
        app.focused = FocusTarget::IntervalFretboard;

        let Answer::Position(target) = correct_answer(&app.interval_trainer) else {
            unreachable!()
        };
        app.interval_trainer.cursor = target;

        let _ = app.update(Message::ActivateFocused);
        assert_eq!(app.interval_trainer.streak, 1);
    }

    /// A click reports through the fretboard's press handler — the same message the canvas
    /// publishes — and it drags the cursor along.
    #[test]
    fn a_press_on_the_neck_answers_and_moves_the_cursor() {
        let mut app = find_it_app(0x9e55);

        let Answer::Position(target) = correct_answer(&app.interval_trainer) else {
            unreachable!()
        };

        let _ = app.update(Message::ChooseIntervalPosition(target.string, target.fret));

        assert_eq!(app.interval_trainer.cursor, target);
        assert_eq!(app.interval_trainer.streak, 1);
    }

    #[test]
    fn the_interval_trainer_accelerators_act_without_moving_focus() {
        let mut app = interval_app(0xacce1);

        let focused = app.focused;
        let standing = app.interval_trainer.prompt;
        press_into(&mut app, "r", keyboard::Modifiers::empty());
        assert_ne!(app.interval_trainer.prompt, standing, "r did not skip");
        assert_eq!(app.focused, focused, "the ring moved");

        let direction = app.interval_trainer.drill();
        press_into(&mut app, "d", keyboard::Modifiers::empty());
        assert_eq!(app.interval_trainer.drill(), direction.flipped());
    }

    /// `a` widens the Note Trainer's pool and `i` flips the scale trainer's notation.
    /// Neither means anything here, and the spec says a key another screen claims is inert.
    #[test]
    fn the_other_screens_keys_are_inert_here() {
        let mut app = interval_app(0x1e7);

        let before = (
            app.interval_trainer.prompt,
            app.interval_trainer.drill(),
            app.focused,
        );

        press_into(&mut app, "a", keyboard::Modifiers::empty());
        press_into(&mut app, "i", keyboard::Modifiers::empty());

        assert_eq!(
            (
                app.interval_trainer.prompt,
                app.interval_trainer.drill(),
                app.focused
            ),
            before
        );
    }

    #[test]
    fn the_interval_trainers_keys_are_inert_on_other_screens() {
        // The mirror of the test above: this screen's accelerators are declared for it
        // alone, so pressing them elsewhere cannot reach its drill.
        for screen in [Screen::Home, Screen::ScaleTrainer, Screen::NoteTrainer] {
            let mut app = app_with_seed(0x0e);
            app.open(screen.clone());

            let before = app.interval_trainer.prompt;
            press_into(&mut app, "r", keyboard::Modifiers::empty());
            press_into(&mut app, "d", keyboard::Modifiers::empty());

            assert_eq!(
                app.interval_trainer.prompt, before,
                "{screen:?} reached the interval drill"
            );
        }
    }

    #[test]
    fn the_help_overlay_lists_the_interval_trainers_keys() {
        let claimed: Vec<char> = accelerators_for(&Screen::IntervalTrainer, Notation::Notes)
            .into_iter()
            .map(|(key, _, _)| key)
            .collect();

        assert_eq!(claimed, vec!['r', 'd']);
        // No `a`: this screen has no pool to widen, so the key stays unclaimed here.
        assert!(!claimed.contains(&'a'));
    }

    #[test]
    fn the_flash_holds_the_prompt_and_the_other_trainers_tick_cannot_retire_it() {
        let mut app = interval_app(0xf1a5);

        let standing = app.interval_trainer.prompt;
        let Answer::Interval(interval) = correct_answer(&app.interval_trainer) else {
            unreachable!()
        };
        let _ = app.update(Message::AnswerInterval(interval));

        assert!(app.interval_trainer.is_flashing());
        assert_eq!(app.interval_trainer.prompt, standing);

        // The Note Trainer's tick, which must not reach this drill — the reason the two
        // trainers have separate advance messages.
        let _ = app.update(Message::AdvancePrompt);
        assert_eq!(
            app.interval_trainer.prompt, standing,
            "the other trainer's tick retired this prompt"
        );

        let _ = app.update(Message::AdvanceIntervalPrompt);
        assert_ne!(app.interval_trainer.prompt, standing, "the drill stalled");
    }

    #[test]
    fn the_flash_ends_behind_the_help_overlay() {
        let mut app = interval_app(0x0e);

        let standing = app.interval_trainer.prompt;
        let Answer::Interval(interval) = correct_answer(&app.interval_trainer) else {
            unreachable!()
        };
        let _ = app.update(Message::AnswerInterval(interval));

        app.help_open = true;
        let _ = app.update(Message::AdvanceIntervalPrompt);

        assert!(app.help_open, "the tick dismissed the overlay");
        assert_ne!(app.interval_trainer.prompt, standing, "the drill stalled");
    }

    /// `every_screen_builds_its_view` sweeps the screens but only ever sees whichever
    /// direction a fresh trainer opens in. Both answer surfaces are built here, since the
    /// *Find it* branch draws widgets the *Name it* branch never reaches.
    #[test]
    fn both_directions_build_their_view() {
        let mut app = interval_app(0x21e0);

        for _ in 0..2 {
            let _ = app.view();

            app.help_open = true;
            let _ = app.view();
            app.help_open = false;

            // With feedback on screen too, so the marked buttons and discs are built.
            let wrong = wrong_answer(&app.interval_trainer);
            app.interval_trainer.answer(wrong);
            app.interval_trainer
                .answer(correct_answer(&app.interval_trainer));
            let _ = app.view();

            app.interval_trainer.toggle_direction(&mut app.rng);
            app.reset_focus();
        }
    }

    #[test]
    fn escape_leaves_the_interval_trainer() {
        let mut app = interval_app(0xe5c);

        let _ = app.update(Message::GoBack);
        assert_eq!(app.screen, Screen::Home);
    }
}
