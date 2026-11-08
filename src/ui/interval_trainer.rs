//! The Interval Trainer: the drill's state machine and the screen that draws it.
//! Only the root is named on screen; the target never is, and judging compares semitone
//! distance.

use iced::{Color, Element};

use super::drill::{ANSWERS, Drill, Score, choice_for, interval_label};
use super::focus::{Direction, FocusTarget, INTERVAL_ROW_WIDTH};
use super::fretboard::{Fretboard, MarkerStyle, NoteMarker, fretboard};
use super::neck::{CURSOR_HOME, DRILL_NECK, Position, neck};
use super::root_stage::{CARD_PADDING, root_column, selector_column, stage, still_root_glyph};
use super::style::{
    BODY, CANVAS, CONTROL_SIZE, DANGER, INK, LINK, MUTE, ROOT_MARKER, SUCCESS, card_container,
    control_button, control_label, control_shuffle, correct_answer_button, focus_ring,
    ghost_button, streak_readout, wrong_answer_button,
};
use super::{Message, ROOT_BUTTON_SIZE};
use crate::music::intervals::Interval;
use crate::music::notes::Note;
use crate::rng::Rng;

/// What the Interval Trainer is asking right now. `NameIt` stores no interval, so judging
/// recomputes it from what is on the neck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prompt {
    /// Two lit positions; the answer is the interval from the first to the second.
    NameIt { root: Position, target: Position },
    /// A lit root and a named interval; the answer is a position carrying it.
    FindIt { root: Position, interval: Interval },
}

/// What the user offered in reply to a `Prompt`.
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

/// The semitone distance upward from `root` to `target` modulo the octave, or `None` if
/// either is off the neck.
fn distance(root: Position, target: Position) -> Option<u8> {
    Some(root.pitch_class()?.semitones_up_to(target.pitch_class()?))
}

/// No iced types, so the drill is testable without a widget tree.
pub(super) struct IntervalTrainer {
    prompt: Prompt,
    score: Score<Answer>,
    cursor: Position,
}

impl IntervalTrainer {
    pub(super) fn new(rng: &mut Rng) -> Self {
        let mut trainer = Self {
            // Replaced by `draw_prompt` before anyone sees it.
            prompt: Prompt::NameIt {
                root: CURSOR_HOME,
                target: CURSOR_HOME,
            },
            score: Score::default(),
            cursor: CURSOR_HOME,
        };

        trainer.draw_prompt(Drill::NameIt, rng);
        trainer
    }

    /// Draws a fresh prompt of `drill`, never the one already showing. Generated root and
    /// interval first, so the distance is always one the answer grid can express.
    fn draw_prompt(&mut self, drill: Drill, rng: &mut Rng) {
        let neck = neck();

        // The rejection loop below ends only if there are at least two prompts to choose
        // between.
        debug_assert!(
            neck.len() * ANSWERS.len() >= 2,
            "the drill must have at least two prompts to choose between or this loop cannot end"
        );

        self.prompt = rng.draw_other_than(self.prompt, |rng| {
            let (root, root_pitch) = neck[rng.below(neck.len())];
            let interval = ANSWERS[rng.below(ANSWERS.len())].interval;

            match drill {
                Drill::NameIt => {
                    // The root's own position is excluded, so a unison lights an octave
                    // elsewhere rather than stacking both rings on one dot.
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
            }
        });

        // Both kinds of feedback belong to the prompt they were given against.
        self.score.next_prompt();
    }

    /// Judges by semitone distance, so the two tritone spellings are one answer.
    fn judge(&self, answer: Answer) -> bool {
        match (self.prompt, answer) {
            (Prompt::NameIt { root, target }, Answer::Interval(named)) => {
                distance(root, target) == Some(named.semitones())
            }
            // Any position carrying the target pitch class counts.
            (Prompt::FindIt { root, interval }, Answer::Position(position)) => {
                match (root.pitch_class(), position.pitch_class()) {
                    (Some(root_pitch), Some(answered)) => {
                        root_pitch.transpose(interval.semitones()) == answered
                    }
                    // Off the neck; not a `None == None` match.
                    _ => false,
                }
            }
            // A mismatched pair is a wiring bug; `false` rather than a panic.
            _ => false,
        }
    }

    /// Takes an answer, unless one is already being marked.
    fn answer(&mut self, answer: Answer) {
        self.score.mark(answer, self.judge(answer));
    }

    /// Answers with an interval — the *Name it* surface.
    pub(super) fn answer_interval(&mut self, interval: Interval) {
        self.answer(Answer::Interval(interval));
    }

    /// Answers with a position, and takes the cursor there.
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
        self.score.is_flashing()
    }

    pub(super) fn drill(&self) -> Drill {
        self.prompt.drill()
    }

    /// What the big glyph says: the root's name, and never the target's.
    fn root_name(&self) -> Option<Note> {
        Some(self.prompt.root().pitch_class()?.conventional_name())
    }

    /// Ends the flash and draws the next prompt. A no-op when nothing is being marked, so
    /// a stale tick cannot retire the prompt on screen.
    pub(super) fn advance(&mut self, rng: &mut Rng) {
        if self.score.is_flashing() {
            let drill = self.prompt.drill();
            self.draw_prompt(drill, rng);
        }
    }

    pub(super) fn skip(&mut self, rng: &mut Rng) {
        let drill = self.prompt.drill();
        self.score.break_streak();
        self.draw_prompt(drill, rng);
    }

    pub(super) fn toggle_direction(&mut self, rng: &mut Rng) {
        let flipped = self.prompt.drill().flipped();
        self.score.break_streak();
        self.cursor = CURSOR_HOME;
        self.draw_prompt(flipped, rng);
    }

    /// Opening the screen: the direction and the best streak persist, the run does not.
    pub(super) fn enter(&mut self, rng: &mut Rng) {
        let drill = self.prompt.drill();
        self.score.break_streak();
        self.cursor = CURSOR_HOME;
        self.draw_prompt(drill, rng);
    }

    /// Walks the cursor one position, stopping at the neck's edges rather than wrapping.
    pub(super) fn move_cursor(&mut self, direction: Direction) {
        self.cursor = self.cursor.step(direction);
    }
}

/// Wider than `ROOT_BUTTON_SIZE` to fit the twinned tritone labels.
const ANSWER_BUTTON_WIDTH: f32 = 76.0;
const ANSWER_COLUMN_SPACING: f32 = 20.0;
const ANSWER_ROW_SPACING: f32 = 16.0;

pub(super) fn ui_interval_trainer(
    trainer: &IntervalTrainer,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, column, container, row, text};

    let neck = match trainer.prompt {
        // Two rings and nothing else — see `prompt_markers`.
        Prompt::NameIt { .. } => Fretboard {
            num_frets: DRILL_NECK.frets(),
            highlighted: prompt_markers(trainer),
            ..Fretboard::default()
        },
        // Here the neck is the answer surface, so it takes a press handler and shows the
        // cursor. The root ring stays as the anchor.
        Prompt::FindIt { .. } => Fretboard {
            num_frets: DRILL_NECK.frets(),
            highlighted: prompt_markers(trainer)
                .into_iter()
                .chain(position_markers(trainer))
                .collect(),
            cursor: Some(trainer.cursor),
            on_press: Some(Message::ChooseIntervalPosition),
            ..Fretboard::default()
        },
    };

    let question: Element<'static, Message> = match trainer.prompt {
        Prompt::NameIt { .. } => column![
            text("how far is this?").size(26).color(BODY),
            text("measure up from the amber root").size(16).color(MUTE),
        ]
        .spacing(6)
        .align_x(iced::Alignment::Center)
        .into(),
        Prompt::FindIt { interval, .. } => column![
            row![
                text("find").size(26).color(BODY),
                interval_label(&choice_for(interval), 34, INK),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
            text("press any fret that far above it")
                .size(16)
                .color(MUTE),
        ]
        .spacing(6)
        .align_x(iced::Alignment::Center)
        .into(),
    };

    let glyph = match trainer.root_name() {
        Some(root) => still_root_glyph(root),
        None => Space::new().into(),
    };

    let centre = root_column(glyph, question, None);

    // only *Name it* has a card; in *Find it* the neck is the answer surface
    let card: Element<'static, Message> = match trainer.prompt {
        Prompt::NameIt { .. } => interval_answer_card(trainer, focused),
        Prompt::FindIt { .. } => Space::new().into(),
    };

    let foot = column![
        interval_trainer_controls(trainer, focused),
        streak_readout(trainer.score.streak(), trainer.score.best()),
    ]
    .spacing(12)
    .align_x(iced::Alignment::Center);

    let selector = selector_column(
        card,
        container(foot).width(Length::Fill).center_x(Length::Fill),
    );

    stage(fretboard(neck), centre, selector)
}

/// The prompt's marks: the root always, and in *Name it* the target too. Unfilled rings,
/// told apart by colour, and unlabelled so no note is named.
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

/// The positions guessed against the current *Find it* prompt, filled. The right answer
/// comes last so it is drawn over a wrong one.
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
        .score
        .wrong()
        .iter()
        .filter_map(|answer| marker(answer, DANGER))
        .chain(
            trainer
                .score
                .correct()
                .iter()
                .filter_map(|answer| marker(answer, SUCCESS)),
        )
        .collect()
}

/// The header row: direction and skip. Labelled with the current mode, so it doubles as a
/// status line.
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

/// The twelve answer buttons, in rows of `INTERVAL_ROW_WIDTH`.
fn interval_answer_card(
    trainer: &IntervalTrainer,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{column, container};

    let rows = (0..ANSWERS.len()).step_by(INTERVAL_ROW_WIDTH).fold(
        column![].spacing(ANSWER_ROW_SPACING),
        |rows, start| {
            let len = INTERVAL_ROW_WIDTH.min(ANSWERS.len() - start);

            rows.push(
                container(interval_answer_row(start, len, trainer, focused))
                    .width(Length::Fill)
                    .center_x(Length::Fill),
            )
        },
    );

    container(rows)
        .width(Length::Fill)
        .padding(CARD_PADDING)
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
        .fold(row![].spacing(ANSWER_COLUMN_SPACING), |acc, (i, choice)| {
            let answer = Answer::Interval(choice.interval);
            let was_wrong = trainer.score.wrong().contains(&answer);
            let was_right = trainer.score.correct() == Some(answer);
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
            // Right beats wrong, should both ever apply.
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
#[cfg(test)]
mod tests {
    use super::super::keys::accelerators_for;
    use super::super::neck::NECK_STRINGS;
    use super::super::tests::{app_with_seed, press_into};
    use super::super::{App, Notation, Screen};
    use super::*;
    use crate::ui::WalkTheScale;
    use iced::keyboard;

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

        assert_eq!(neck.len(), NECK_STRINGS * (DRILL_NECK.frets() + 1));
        for (position, pitch) in &neck {
            assert!(position.string < NECK_STRINGS);
            assert!(position.fret <= DRILL_NECK.frets());
            assert_eq!(position.pitch_class(), Some(*pitch));
        }
    }

    /// Every root paired with every interval has somewhere other than the root to land,
    /// which `draw_prompt`'s `debug_assert!` relies on.
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
        // Four frets up is a major third, and the same pitch class on another string reads
        // the same.
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
                    fret: DRILL_NECK.frets() + 1
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

    /// Both marks must be on the neck and distinct, so there is a distance to measure.
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

        // A tritone prompt: the low E and the B♭ above it.
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

        // And a distance that is not six is still wrong.
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
            fret: DRILL_NECK.frets() + 1
        })));
    }

    #[test]
    fn consecutive_correct_answers_raise_the_streak() {
        let (mut trainer, mut rng) = trainer_with_seed(0x57ea);

        for expected in 1..=10 {
            answer_and_advance(&mut trainer, &mut rng);
            assert_eq!(trainer.score.streak(), expected);
            assert_eq!(trainer.score.best(), expected);
        }
    }

    #[test]
    fn a_correct_answer_is_marked_and_holds_the_prompt() {
        let (mut trainer, mut rng) = trainer_with_seed(0x11);

        let standing = trainer.prompt;
        let answer = correct_answer(&trainer);
        trainer.answer(answer);

        assert_eq!(trainer.prompt, standing, "the prompt moved under the flash");
        assert_eq!(trainer.score.correct(), Some(answer));
        assert!(trainer.is_flashing());

        // A second answer during the flash changes nothing — a held key or a double click.
        trainer.answer(wrong_answer(&trainer));
        assert_eq!(trainer.score.streak(), 1);
        assert!(trainer.score.wrong().is_empty());

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
        assert_eq!(trainer.score.streak(), 1);

        let standing = trainer.prompt;
        let wrong = wrong_answer(&trainer);
        trainer.answer(wrong);

        assert_eq!(trainer.score.streak(), 0);
        assert_eq!(trainer.score.best(), 1);
        assert_eq!(trainer.prompt, standing);
        assert_eq!(trainer.score.wrong(), vec![wrong]);
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

        assert_eq!(
            trainer.score.wrong(),
            wrong,
            "earlier wrong answers were lost"
        );

        trainer.answer(correct_answer(&trainer));
        trainer.advance(&mut rng);
        assert!(
            trainer.score.wrong().is_empty(),
            "a new prompt started dirty"
        );
    }

    #[test]
    fn the_best_streak_survives_what_the_current_one_does_not() {
        let (mut trainer, mut rng) = trainer_with_seed(0xbe57);

        for _ in 0..4 {
            answer_and_advance(&mut trainer, &mut rng);
        }
        assert_eq!(trainer.score.best(), 4);

        trainer.skip(&mut rng);
        assert_eq!(
            trainer.score.streak(),
            0,
            "skipping past what you do not know counted"
        );
        assert_eq!(trainer.score.best(), 4);

        answer_and_advance(&mut trainer, &mut rng);
        trainer.toggle_direction(&mut rng);
        assert_eq!(trainer.score.streak(), 0);
        assert_eq!(trainer.score.best(), 4);

        trainer.enter(&mut rng);
        assert_eq!(trainer.score.streak(), 0);
        assert_eq!(
            trainer.score.best(),
            4,
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

    /// Answers correctly through whichever message the view would send, then sends the
    /// flash's timer message.
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
        assert_eq!(app.interval_trainer.score.streak(), 0);
        assert!(app.interval_trainer.score.wrong().is_empty());
    }

    #[test]
    fn reopening_draws_a_fresh_prompt_and_keeps_the_settings() {
        let mut app = find_it_app(0xfa17);

        answer_correctly(&mut app);
        answer_correctly(&mut app);
        let best = app.interval_trainer.score.best();
        assert_eq!(best, 2);

        for _ in 0..20 {
            let before = app.interval_trainer.prompt;
            let _ = app.update(Message::GoBack);
            let _ = app.update(Message::Navigate(Screen::IntervalTrainer));

            assert_ne!(app.interval_trainer.prompt, before, "reopened on its last");
            assert_eq!(app.interval_trainer.drill(), Drill::FindIt);
            assert_eq!(app.interval_trainer.score.best(), best);
            assert_eq!(
                app.interval_trainer.score.streak(),
                0,
                "the run carried over"
            );
        }
    }

    #[test]
    fn the_answer_buttons_are_reachable_in_name_it() {
        let app = interval_app(0xb7);
        assert_eq!(app.interval_trainer.drill(), Drill::NameIt);

        let focusables = app.focusables();
        for index in 0..ANSWERS.len() {
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
        assert_eq!(app.interval_trainer.score.streak(), 1);
    }

    /// A click sends the canvas's press message and drags the cursor along.
    #[test]
    fn a_press_on_the_neck_answers_and_moves_the_cursor() {
        let mut app = find_it_app(0x9e55);

        let Answer::Position(target) = correct_answer(&app.interval_trainer) else {
            unreachable!()
        };

        let _ = app.update(Message::ChooseIntervalPosition(target.string, target.fret));

        assert_eq!(app.interval_trainer.cursor, target);
        assert_eq!(app.interval_trainer.score.streak(), 1);
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

    /// `a` and `i` are claimed by other screens and are inert here.
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
        // The mirror of the test above: this screen's accelerators are inert elsewhere.
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
        let claimed: Vec<char> = accelerators_for(
            &Screen::IntervalTrainer,
            Notation::Notes,
            &WalkTheScale::new(),
        )
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

        // The Note Trainer's tick, which must not reach this drill.
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

    /// Builds both answer surfaces, which `every_screen_builds_its_view` does not reach.
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

    #[test]
    fn the_answer_card_fits_the_selector_column_and_the_root_band() {
        // each button sits in a 3px `focus_ring`
        const FOCUS_RING_PADDING: f32 = 3.0;

        let columns = INTERVAL_ROW_WIDTH as f32;
        let rows = ANSWERS.len().div_ceil(INTERVAL_ROW_WIDTH) as f32;
        let width = columns * (ANSWER_BUTTON_WIDTH + 2.0 * FOCUS_RING_PADDING)
            + (columns - 1.0) * ANSWER_COLUMN_SPACING
            + 2.0 * CARD_PADDING;
        let height = rows * (ROOT_BUTTON_SIZE + 2.0 * FOCUS_RING_PADDING)
            + (rows - 1.0) * ANSWER_ROW_SPACING
            + 2.0 * CARD_PADDING;

        assert!(
            width <= crate::ui::root_stage::SELECTOR_COLUMN_WIDTH,
            "the answer card needs {width}; its last column would draw past the card's edge"
        );
        assert!(
            height <= crate::ui::root_stage::ROOT_BAND_HEIGHT,
            "the answer card needs {height}; it would no longer be centred on the root"
        );
    }

    #[test]
    fn the_glyph_names_the_root_and_nothing_names_the_target() {
        let mut app = app_with_seed(0x1a7);
        app.open(Screen::IntervalTrainer);

        for _ in 0..32 {
            let trainer = &app.interval_trainer;
            let root = trainer
                .prompt
                .root()
                .pitch_class()
                .expect("the root is on the neck");
            assert_eq!(trainer.root_name(), Some(root.conventional_name()));

            // the rings stay unlabelled, so the name above is the only one on screen
            assert!(prompt_markers(trainer).iter().all(|m| m.label.is_empty()));

            let _ = app.update(Message::SkipIntervalPrompt);
        }
    }
}
