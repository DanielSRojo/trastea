//! The Note Trainer: the drill's state machine and the screen that draws it.

use iced::{Color, Element, Padding};

use super::calibration;
use super::drill::{Drill, Score};
use super::focus::{ANSWER_ROW_WIDTH, Direction, FocusTarget};
use super::fretboard::{Fretboard, LitString, MarkerStyle, NoteMarker, fretboard};
use super::listener::{Listener, State, ui_device_line, ui_device_picker};
use super::neck::{CURSOR_HOME, DRILL_NECK, NECK_STRINGS, Position, neck, string_name};
use super::placement::{Declined, Placed, Placement, Placer};
use super::position::{Group, groups};
use super::root_stage::{root_column, selector_column, stage, still_root_glyph};
use super::style::{
    BODY, CANVAS, CONTROL_SIZE, DANGER, INK, LINK, MUTE, SUCCESS, card_container,
    control_accidental, control_button, control_label, control_shuffle, correct_answer_button,
    focus_ring, ghost_button, spelling_glyph, streak_readout, wrong_answer_button,
};
use super::{Message, ROOT_BUTTON_SIZE, SELECTOR_CARD_HEIGHT, SUMMARY_CARD_HEIGHT, note_label};
use crate::audio::detect::Heard;
use crate::audio::inharmonicity::Fit;
use crate::music::notes::{PitchClass, Spelling};
use crate::music::pitch::Pitch;
use crate::rng::Rng;

/// What the Note Trainer is asking right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prompt {
    /// A lit position; the answer names the note on it.
    NameIt { string: usize, fret: usize },
    /// A named note; the answer is any position carrying it.
    FindIt(PitchClass),
    /// A named note on a named string; the answer is played there.
    PlayIt {
        pitch_class: PitchClass,
        string: usize,
    },
    /// Playing, with no note and string the calibration can judge.
    Unaskable,
}

/// What the user offered in reply to a `Prompt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    Name(PitchClass),
    Position {
        string: usize,
        fret: usize,
    },
    /// Where it was played is kept beside the score, which stores answers by value.
    Played(PitchClass),
}

/// How the learner answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Answering {
    Pressing,
    Playing,
}

/// Which pitch classes prompts are drawn from.
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

/// What the drill does with the next confirmed reading.
/// Duplicates `play_the_interval::Gate` and `walk_the_scale::Gate` on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gate {
    Deaf,
    Listening,
}

/// No iced types here, so the drill is testable without a widget tree.
pub(super) struct NoteTrainer {
    prompt: Prompt,
    /// Which way pressing asks, kept while playing so swapping back restores it.
    direction: Drill,
    /// A preference that survives leaving the drill, unlike the `Listener`.
    answering: Answering,
    pool: Pool,
    /// Separate from the scale trainer's spelling.
    spelling: Spelling,
    score: Score<Answer>,
    cursor: Position,
    /// The capture device, held only while playing.
    listener: Option<Listener>,
    placer: Placer,
    gate: Gate,
    right_at: Placed,
    wrong_at: Placed,
    /// The asked note last played, until its placement says which string it was on.
    awaiting: Option<Pitch>,
    /// The asked note last played, when it could not be placed and so was not judged.
    unplaced: Option<Placement>,
}

impl NoteTrainer {
    pub(super) fn new(rng: &mut Rng) -> Self {
        let mut trainer = Self {
            // Replaced by `draw_prompt` before anyone sees it.
            prompt: Prompt::NameIt { string: 0, fret: 0 },
            direction: Drill::NameIt,
            answering: Answering::Pressing,
            pool: Pool::Naturals,
            spelling: Spelling::Sharps,
            score: Score::default(),
            cursor: CURSOR_HOME,
            listener: None,
            placer: Placer::default(),
            gate: Gate::Listening,
            right_at: Placed::default(),
            wrong_at: Placed::default(),
            awaiting: None,
            unplaced: None,
        };

        trainer.draw_prompt(rng);
        trainer
    }

    /// Every position on the neck whose note is in the current pool.
    fn positions(&self) -> Vec<(usize, usize)> {
        let mut positions = Vec::new();

        for string in 0..NECK_STRINGS {
            for fret in 0..=DRILL_NECK.frets() {
                if DRILL_NECK
                    .pitch_class_at(string, fret)
                    .is_some_and(|pc| self.pool.contains(pc))
                {
                    positions.push((string, fret));
                }
            }
        }

        positions
    }

    /// Every note and string playing can ask for: the pool's notes on each string the
    /// calibration trusts, unless one of its positions there is in a group the estimate will
    /// not split — playing it on the string the group shares would be judged right too.
    fn playable(&self) -> Vec<(PitchClass, usize)> {
        let Some(kept) = self.placer.calibration() else {
            return Vec::new();
        };

        let grouped: Vec<Position> = groups(kept, &DRILL_NECK)
            .into_iter()
            .flat_map(|Group { positions, .. }| positions)
            .collect();

        (0..NECK_STRINGS)
            .filter(|&string| !kept.is_flagged(string))
            .flat_map(|string| {
                self.pool
                    .pitch_classes()
                    .iter()
                    .map(move |&pitch_class| (pitch_class, string))
            })
            .filter(|&(pitch_class, string)| {
                (0..=DRILL_NECK.frets())
                    .map(|fret| Position { string, fret })
                    .filter(|position| position.pitch_class() == Some(pitch_class))
                    .all(|position| !grouped.contains(&position))
            })
            .collect()
    }

    /// Draws a fresh prompt for the mode and direction in force, never the one already
    /// showing. Terminates only because every pressing pool holds at least two prompts, and a
    /// playing pool of fewer than two is not drawn from.
    fn draw_prompt(&mut self, rng: &mut Rng) {
        self.prompt = match (self.answering, self.direction) {
            (Answering::Pressing, Drill::NameIt) => {
                let positions = self.positions();
                debug_assert!(
                    positions.len() >= 2,
                    "a pool must hold at least two positions or this loop cannot end"
                );

                rng.draw_other_than(self.prompt, |rng| {
                    let (string, fret) = positions[rng.below(positions.len())];
                    Prompt::NameIt { string, fret }
                })
            }
            (Answering::Pressing, Drill::FindIt) => {
                let choices = self.pool.pitch_classes();
                debug_assert!(
                    choices.len() >= 2,
                    "a pool must hold at least two pitch classes or this loop cannot end"
                );

                rng.draw_other_than(self.prompt, |rng| {
                    Prompt::FindIt(choices[rng.below(choices.len())])
                })
            }
            (Answering::Playing, _) => {
                let ask = |(pitch_class, string)| Prompt::PlayIt {
                    pitch_class,
                    string,
                };

                match self.playable().as_slice() {
                    [] => Prompt::Unaskable,
                    [only] => ask(*only),
                    playable => rng.draw_other_than(self.prompt, |rng| {
                        ask(playable[rng.below(playable.len())])
                    }),
                }
            }
        };

        // Feedback belongs to the prompt it was given against; a skip or toggle also ends the flash.
        self.score.next_prompt();
        self.right_at.clear();
        self.wrong_at.clear();
        self.awaiting = None;
        self.unplaced = None;
        // read off the device rather than set flat, so a prompt drawn over a ringing answer waits
        self.gate = match self.listener.as_ref().and_then(Listener::reading) {
            Some(_) => Gate::Deaf,
            None => Gate::Listening,
        };
    }

    /// Judges by pitch class, so the two names of a black key are one answer.
    fn judge(&self, answer: Answer) -> bool {
        match (self.prompt, answer) {
            (Prompt::NameIt { string, fret }, Answer::Name(named)) => {
                DRILL_NECK.pitch_class_at(string, fret) == Some(named)
            }
            // Any position carrying the note counts.
            (Prompt::FindIt(target), Answer::Position { string, fret }) => {
                DRILL_NECK.pitch_class_at(string, fret) == Some(target)
            }
            // A mismatched pair is a wiring bug; `false` rather than a panic.
            _ => false,
        }
    }

    /// Takes a pressed answer, unless one is already being marked. Inert while playing,
    /// where a press would be a second way to answer.
    fn answer(&mut self, answer: Answer) {
        if self.answering == Answering::Pressing {
            self.score.mark(answer, self.judge(answer));
        }
    }

    /// Answers with a note name — the *Name it* surface.
    pub(super) fn answer_name(&mut self, pitch_class: PitchClass) {
        self.answer(Answer::Name(pitch_class));
    }

    /// Answers with a position, and takes the cursor there.
    pub(super) fn answer_position(&mut self, string: usize, fret: usize) {
        self.cursor = Position { string, fret };
        self.answer(Answer::Position { string, fret });
    }

    /// Answers with wherever the cursor is sitting — what Enter on the neck does.
    pub(super) fn answer_at_cursor(&mut self) {
        let Position { string, fret } = self.cursor;
        self.answer(Answer::Position { string, fret });
    }

    /// Folds a released reading and its fit, while playing. A note of another pitch class is
    /// wrong at once; the asked one waits for its placement, since the string is the question.
    pub(super) fn apply(&mut self, heard: Heard, fit: Option<Fit>) {
        if self.answering != Answering::Playing {
            return;
        }

        self.gate(heard);

        for placement in self.placer.hear(heard, fit) {
            self.place(placement);
        }
    }

    /// A non-pitch reading only re-arms the gate, so silence and a fumble cost nothing.
    fn gate(&mut self, heard: Heard) {
        let Heard::Pitch(detection) = heard else {
            self.gate = Gate::Listening;
            return;
        };

        // what is sounding during the flash is the tail of the note that scored
        if self.score.is_flashing() || self.gate == Gate::Deaf {
            return;
        }

        let Prompt::PlayIt {
            pitch_class: asked, ..
        } = self.prompt
        else {
            return;
        };

        let Some((pitch, _)) = Pitch::nearest(detection.hz()) else {
            return;
        };

        // a held string confirms the same reading every poll; this makes one note one answer
        self.gate = Gate::Deaf;
        self.unplaced = None;

        if pitch.pitch_class() == asked {
            self.awaiting = Some(pitch);
        } else {
            self.score.mark(Answer::Played(pitch.pitch_class()), false);
            self.wrong_at.expect(pitch);
        }
    }

    /// Narrows a wrong answer to where it was played, or judges the asked note by its string.
    fn place(&mut self, placement: Placement) {
        if self.wrong_at.place(&placement) {
            return;
        }

        let Prompt::PlayIt { string, .. } = self.prompt else {
            return;
        };
        if self.awaiting != Some(placement.pitch()) {
            return;
        }
        self.awaiting = None;

        let played = placement.pitch().pitch_class();
        let positions = placement.positions();

        // the benefit of the doubt: a note the screen cannot place is played again
        if positions.is_empty() {
            self.unplaced = Some(placement);
            return;
        }

        let right = positions.iter().any(|position| position.string == string);
        self.score.mark(Answer::Played(played), right);

        let at = if right {
            &mut self.right_at
        } else {
            &mut self.wrong_at
        };
        at.record(played, positions);
    }

    /// Whether a correct answer is lit, which is the window the flash timer runs in.
    pub(super) fn is_flashing(&self) -> bool {
        self.score.is_flashing()
    }

    pub(super) fn drill(&self) -> Drill {
        self.direction
    }

    pub(super) fn answering(&self) -> Answering {
        self.answering
    }

    pub(super) fn listener(&self) -> Option<&Listener> {
        self.listener.as_ref()
    }

    pub(super) fn listener_mut(&mut self) -> Option<&mut Listener> {
        self.listener.as_mut()
    }

    /// Opens the device, fitting what it hears so a played answer can be placed.
    fn claim(&mut self) {
        self.listener
            .get_or_insert_with(|| Listener::new().fitting())
            .listen();
    }

    fn release(&mut self) {
        if let Some(listener) = self.listener.as_mut() {
            listener.stop();
        }
        self.listener = None;
    }

    /// Ends the flash and draws the next prompt. A no-op when nothing is being marked, so
    /// a stale tick cannot retire the prompt on screen.
    pub(super) fn advance(&mut self, rng: &mut Rng) {
        if self.score.is_flashing() {
            self.draw_prompt(rng);
        }
    }

    pub(super) fn skip(&mut self, rng: &mut Rng) {
        self.score.break_streak();
        self.draw_prompt(rng);
    }

    /// Inert while playing, where a note and a string have no direction.
    pub(super) fn toggle_direction(&mut self, rng: &mut Rng) {
        if self.answering == Answering::Playing {
            return;
        }

        self.direction = self.direction.flipped();
        self.score.break_streak();
        self.cursor = CURSOR_HOME;
        self.draw_prompt(rng);
    }

    /// Swaps pressing and playing. The streak survives, since both ask where a note is; the
    /// prompt is redrawn, since they ask it differently.
    pub(super) fn toggle_answering(&mut self, rng: &mut Rng) {
        self.answering = match self.answering {
            Answering::Pressing => Answering::Playing,
            Answering::Playing => Answering::Pressing,
        };

        match self.answering {
            Answering::Playing => self.claim(),
            Answering::Pressing => self.release(),
        }

        self.cursor = CURSOR_HOME;
        self.draw_prompt(rng);
    }

    pub(super) fn toggle_pool(&mut self, rng: &mut Rng) {
        self.pool = match self.pool {
            Pool::Naturals => Pool::All,
            Pool::All => Pool::Naturals,
        };

        self.score.break_streak();
        self.draw_prompt(rng);
    }

    /// Cosmetic: judging compares pitch classes, so it cannot change whether an answer is right.
    pub(super) fn toggle_spelling(&mut self) {
        self.spelling = match self.spelling {
            Spelling::Sharps => Spelling::Flats,
            Spelling::Flats => Spelling::Sharps,
        };
    }

    /// Opening the screen: the settings and the best streak persist, the run does not, and
    /// the kept calibration is read again.
    pub(super) fn enter(&mut self, rng: &mut Rng) {
        self.placer = Placer::new(calibration::kept());
        if self.answering == Answering::Playing {
            self.claim();
        }

        self.score.break_streak();
        self.cursor = CURSOR_HOME;
        self.draw_prompt(rng);
    }

    /// Leaving the screen puts the device down.
    pub(super) fn leave(&mut self) {
        self.release();
    }

    /// Stands in for the kept calibration, which no test may read.
    #[cfg(test)]
    fn calibrate_with(&mut self, calibration: Option<calibration::Calibration>) {
        self.placer = Placer::new(calibration);
    }

    /// Walks the cursor one position, stopping at the neck's edges rather than wrapping.
    pub(super) fn move_cursor(&mut self, direction: Direction) {
        self.cursor = self.cursor.step(direction);
    }
}

pub(super) fn ui_note_trainer(
    trainer: &NoteTrainer,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, column, container, row, text};

    // modal while it is up: nothing is drawn behind it
    if let Some(listener) = trainer.listener()
        && listener.is_picking()
    {
        return container(ui_device_picker(listener))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .padding(Padding::from([24, 24]))
            .into();
    }

    let neck = match trainer.prompt {
        // The prompt itself: one ring — see `prompt_marker`.
        Prompt::NameIt { .. } => Fretboard {
            num_frets: DRILL_NECK.frets(),
            highlighted: prompt_marker(trainer),
            ..Fretboard::default()
        },
        // Here the neck is the answer surface, so it takes a press handler and shows the
        // cursor.
        Prompt::FindIt(_) => Fretboard {
            num_frets: DRILL_NECK.frets(),
            highlighted: position_markers(trainer),
            cursor: Some(trainer.cursor),
            on_press: Some(Message::ChooseNotePosition),
            ..Fretboard::default()
        },
        // No fret lit until it is answered, since the lit place would be the answer.
        Prompt::PlayIt { .. } | Prompt::Unaskable => Fretboard {
            num_frets: DRILL_NECK.frets(),
            highlighted: played_markers(trainer),
            lit_string: asked_string(trainer),
            ..Fretboard::default()
        },
    };

    match trainer.prompt {
        Prompt::NameIt { .. } => ui_pressing(
            trainer,
            neck,
            column![
                text("What note is this?").size(32).color(INK),
                text("Name the lit fret").size(16).color(MUTE),
            ]
            .spacing(6),
            Some(note_answer_card(trainer, focused)),
            focused,
        ),
        Prompt::FindIt(pitch_class) => ui_pressing(
            trainer,
            neck,
            column![
                row![
                    text("Find").size(26).color(BODY),
                    note_label(trainer.spelling.spell(pitch_class), 40, INK),
                ]
                .spacing(12),
                text("Press any fret that plays it").size(16).color(MUTE),
            ]
            .spacing(6),
            None,
            focused,
        ),
        Prompt::PlayIt { pitch_class, .. } => ui_playing(
            trainer,
            neck,
            still_root_glyph(trainer.spelling.spell(pitch_class)),
            column![
                text(string_line(trainer)).size(26).color(BODY),
                text("anywhere along it").size(16).color(MUTE),
            ],
            focused,
        ),
        Prompt::Unaskable => ui_playing(
            trainer,
            neck,
            Space::new().into(),
            column![text("nothing to ask").size(26).color(BODY)],
            focused,
        ),
    }
}

/// Pressing: the question and the controls in a card beside the neck, and the answers under it
/// in *Name it*.
fn ui_pressing(
    trainer: &NoteTrainer,
    neck: Fretboard<Message>,
    question: impl Into<Element<'static, Message>>,
    answers: Option<Element<'static, Message>>,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, column, container, row};

    let prompt_body = column![
        row![
            question.into(),
            Space::new().width(Length::Fill),
            streak_readout(trainer.score.streak(), trainer.score.best()),
        ],
        note_trainer_controls(trainer, focused),
    ]
    .spacing(20);

    let prompt_card = container(prompt_body)
        .width(Length::Fill)
        .height(Length::Fixed(SUMMARY_CARD_HEIGHT))
        .padding(32)
        .style(card_container);

    let details = column![prompt_card]
        .push(answers)
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

/// Playing: the asked note stands as the big root with its string under it, as Play the
/// Interval's root does, and the controls and the device sit at the right column's foot.
fn ui_playing(
    trainer: &NoteTrainer,
    neck: Fretboard<Message>,
    glyph: Element<'static, Message>,
    question: iced::widget::Column<'static, Message>,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, column, container, text};

    let below = question
        .push(text(notice_line(trainer)).size(14).color(BODY))
        .spacing(6)
        .align_x(iced::Alignment::Center);

    let centre = root_column(glyph, below, None);

    let mut foot = column![
        note_trainer_controls(trainer, focused),
        streak_readout(trainer.score.streak(), trainer.score.best()),
    ]
    .spacing(12)
    .align_x(iced::Alignment::Center);

    if let Some(listener) = trainer.listener() {
        foot = foot
            .push(text(listening_line(listener)).size(13).color(MUTE))
            .push(ui_device_line(listener, focused));
    }

    // the guitar is the answer surface, so nothing sits on the root's line in this column
    let selector = selector_column(
        Space::new(),
        container(foot).width(Length::Fill).center_x(Length::Fill),
    );

    stage(fretboard(neck), centre, selector)
}

/// The string a playing prompt names, as a guitarist says it: "on the low E string".
fn string_line(trainer: &NoteTrainer) -> String {
    match trainer.prompt {
        Prompt::PlayIt { string, .. } => {
            format!("on the {} string", string_name(string).unwrap_or_default())
        }
        _ => String::new(),
    }
}

/// What playing has to say beyond the prompt: why nothing is asked, or why the last note
/// played was not judged.
fn notice_line(trainer: &NoteTrainer) -> String {
    if trainer.prompt == Prompt::Unaskable {
        return match trainer.placer.calibration() {
            None => "playing asks by string, which needs this guitar calibrated — \
                     calibrate it on the Position Finder first"
                .into(),
            Some(_) => "this calibration cannot tell any string apart — \
                        calibrate again on the Position Finder"
                .into(),
        };
    }

    match &trainer.unplaced {
        Some(Placement::Declined {
            why: Declined::Slid,
            ..
        }) => "bent or slid into — not judged, play it again".into(),
        Some(Placement::Declined { .. }) => {
            "the strings did not read cleanly — not judged, play it again".into()
        }
        Some(Placement::Placed { .. }) => {
            "could not tell which string — not judged, play it again".into()
        }
        None => String::new(),
    }
}

/// What the screen says it is hearing, as words so the tests can read them.
fn listening_line(listener: &Listener) -> &'static str {
    match listener.state() {
        State::NotStarted => "not listening",
        State::NoInput => "no input — press m to answer on the neck instead",
        State::Silent => "listening, play the note...",
        State::Unnameable => "one string at a time",
        State::Reading => "heard",
    }
}

/// The *Name it* prompt's position as an unfilled, unlabelled ring, since a fill means an
/// answer placed and a label would print the answer.
fn prompt_marker(trainer: &NoteTrainer) -> Vec<NoteMarker> {
    match trainer.prompt {
        Prompt::NameIt { string, fret } => vec![NoteMarker {
            string,
            fret,
            label: String::new(),
            color: LINK,
            style: MarkerStyle::Outlined,
        }],
        Prompt::FindIt(_) | Prompt::PlayIt { .. } | Prompt::Unaskable => Vec::new(),
    }
}

/// The string a playing prompt names, in the prompt's colour.
fn asked_string(trainer: &NoteTrainer) -> Option<LitString> {
    match trainer.prompt {
        Prompt::PlayIt { string, .. } => Some(LitString {
            string,
            color: LINK,
        }),
        Prompt::NameIt { .. } | Prompt::FindIt(_) | Prompt::Unaskable => None,
    }
}

/// The positions guessed against the current prompt, filled. The right answer comes last so
/// it is drawn over a wrong one on the same fret.
fn position_markers(trainer: &NoteTrainer) -> Vec<NoteMarker> {
    let marker = |answer: &Answer, color: Color| match *answer {
        Answer::Position { string, fret } => Some(NoteMarker {
            string,
            fret,
            label: String::new(),
            color,
            style: MarkerStyle::Filled,
        }),
        Answer::Name(_) | Answer::Played(_) => None,
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

/// Where the played answers to the current prompt were placed — every position of the pitch
/// class for one that was not — the right one drawn over a wrong one on the same fret.
fn played_markers(trainer: &NoteTrainer) -> Vec<NoteMarker> {
    let mark = |position: Position, color: Color| NoteMarker {
        string: position.string,
        fret: position.fret,
        label: String::new(),
        color,
        style: MarkerStyle::Filled,
    };

    let wrong = neck().into_iter().filter(|&(position, pitch_class)| {
        trainer.score.wrong().contains(&Answer::Played(pitch_class))
            && trainer.wrong_at.shows(pitch_class, position)
    });
    let right = neck().into_iter().filter(|&(position, pitch_class)| {
        trainer.score.correct() == Some(Answer::Played(pitch_class))
            && trainer.right_at.shows(pitch_class, position)
    });

    wrong
        .map(|(position, _)| mark(position, DANGER))
        .chain(right.map(|(position, _)| mark(position, SUCCESS)))
        .collect()
}

/// The header row: answering, direction, pool, spelling, skip. Labelled with the current mode,
/// so it doubles as a status line. Direction is absent while playing, where it means nothing.
fn note_trainer_controls(trainer: &NoteTrainer, focused: FocusTarget) -> Element<'static, Message> {
    use iced::widget::{row, text};

    let answering = match trainer.answering {
        Answering::Pressing => "press",
        Answering::Playing => "play",
    };

    let direction = match trainer.direction {
        Drill::NameIt => "name it",
        Drill::FindIt => "find it",
    };

    let pool = match trainer.pool {
        Pool::Naturals => "naturals",
        Pool::All => "all 12",
    };

    let mut controls = row![control_button(
        control_label(text(answering), CONTROL_SIZE),
        Message::ToggleNoteAnswering,
        focused == FocusTarget::NoteAnswering,
    )]
    .spacing(8);

    if trainer.answering == Answering::Pressing {
        controls = controls.push(control_button(
            control_label(text(direction), CONTROL_SIZE),
            Message::ToggleDrillDirection,
            focused == FocusTarget::DrillDirectionToggle,
        ));
    }

    controls
        .push(control_button(
            control_label(text(pool), CONTROL_SIZE),
            Message::TogglePool,
            focused == FocusTarget::PoolToggle,
        ))
        .push(control_button(
            control_accidental(spelling_glyph(trainer.spelling)),
            Message::ToggleNoteSpelling,
            focused == FocusTarget::NoteSpellingToggle,
        ))
        .push(control_button(
            control_shuffle(),
            Message::SkipPrompt,
            focused == FocusTarget::SkipPrompt,
        ))
        .into()
}

/// All twelve answer buttons under either pool, so a wrong answer stays reachable.
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
            let was_wrong = trainer.score.wrong().contains(&answer);
            let was_right = trainer.score.correct() == Some(answer);
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
            // Right beats wrong, should both ever apply.
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
    use super::super::keys::{accelerators_for, translate_key};
    use super::super::style::{SMUFL_FLAT, SMUFL_SHARP};
    use super::super::tests::{app_with_seed, every_screen, press_into, press_named};
    use super::super::{App, Notation, Screen};
    use super::*;
    use crate::audio::detect::Detection;
    use crate::audio::inharmonicity::Refusal;
    use crate::ui::WalkTheScale;
    use crate::ui::calibration::Calibration;
    use crate::ui::calibration::tests::{grouping_guitar, spike_guitar};
    use crate::ui::keys::Fires;
    use crate::ui::placement::FITS_PER_NOTE;
    use crate::ui::placement::tests::{heard, measured, pluck};
    use iced::keyboard;
    use iced::keyboard::key::Named;

    fn at(string: usize, fret: usize) -> Position {
        Position { string, fret }
    }

    // ---- Note Trainer drill logic ----

    fn trainer_with_seed(seed: u64) -> (NoteTrainer, Rng) {
        let mut rng = Rng::from_seed(seed);
        let trainer = NoteTrainer::new(&mut rng);
        (trainer, rng)
    }

    /// Every position on the neck, in the order the drill enumerates them.
    fn all_positions() -> impl Iterator<Item = (usize, usize)> {
        (0..NECK_STRINGS).flat_map(|s| (0..=DRILL_NECK.frets()).map(move |f| (s, f)))
    }

    /// The pitch class the current prompt is about, whichever direction it runs.
    fn prompt_pitch_class(trainer: &NoteTrainer) -> PitchClass {
        match trainer.prompt {
            Prompt::NameIt { string, fret } => DRILL_NECK.pitch_class_at(string, fret).unwrap(),
            Prompt::FindIt(target) => target,
            Prompt::PlayIt { pitch_class, .. } => pitch_class,
            Prompt::Unaskable => panic!("nothing is asked"),
        }
    }

    /// An answer that satisfies the current prompt.
    fn correct_answer(trainer: &NoteTrainer) -> Answer {
        match trainer.prompt {
            Prompt::NameIt { string, fret } => {
                Answer::Name(DRILL_NECK.pitch_class_at(string, fret).unwrap())
            }
            Prompt::FindIt(target) => {
                let (string, fret) = all_positions()
                    .find(|&(s, f)| DRILL_NECK.pitch_class_at(s, f) == Some(target))
                    .expect("every pitch class appears within twelve frets");
                Answer::Position { string, fret }
            }
            Prompt::PlayIt { .. } | Prompt::Unaskable => panic!("a playing prompt is not pressed"),
        }
    }

    /// An answer that does not.
    fn wrong_answer(trainer: &NoteTrainer) -> Answer {
        match trainer.prompt {
            Prompt::NameIt { string, fret } => {
                let actual = DRILL_NECK.pitch_class_at(string, fret).unwrap();
                let other = PitchClass::ALL
                    .into_iter()
                    .find(|&pc| pc != actual)
                    .unwrap();
                Answer::Name(other)
            }
            Prompt::FindIt(target) => {
                let (string, fret) = all_positions()
                    .find(|&(s, f)| DRILL_NECK.pitch_class_at(s, f) != Some(target))
                    .unwrap();
                Answer::Position { string, fret }
            }
            Prompt::PlayIt { .. } | Prompt::Unaskable => panic!("a playing prompt is not pressed"),
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

    /// Answers correctly and lets the flash run its course, as the timer would.
    fn answer_and_advance(trainer: &mut NoteTrainer, rng: &mut Rng) {
        let answer = correct_answer(trainer);
        trainer.answer(answer);
        trainer.advance(rng);
    }

    #[test]
    fn consecutive_correct_answers_raise_the_streak() {
        let (mut trainer, mut rng) = trainer_with_seed(3);

        for expected in 1..=3 {
            answer_and_advance(&mut trainer, &mut rng);
            assert_eq!(trainer.score.streak(), expected);
        }
    }

    #[test]
    fn a_correct_answer_is_marked_and_holds_the_prompt() {
        let (mut trainer, mut rng) = trainer_with_seed(0xf1a5);

        let standing = trainer.prompt;
        let answer = correct_answer(&trainer);
        trainer.answer(answer);

        assert_eq!(
            trainer.score.correct(),
            Some(answer),
            "the answer went unmarked"
        );
        assert_eq!(trainer.prompt, standing, "the prompt left before its flash");
        assert_eq!(trainer.score.streak(), 1);

        // Nothing lands while it is up — not another correct press, and not a wrong one.
        let wrong = wrong_answer(&trainer);
        trainer.answer(wrong);
        trainer.answer(answer);

        assert_eq!((trainer.score.streak(), trainer.prompt), (1, standing));
        assert!(
            trainer.score.wrong().is_empty(),
            "a press during the flash counted"
        );

        trainer.advance(&mut rng);

        assert_eq!(
            trainer.score.correct(),
            None,
            "the flash outlived its prompt"
        );
        assert_ne!(trainer.prompt, standing, "the flash never ended");
    }

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
            trainer.answer(answer);
            assert!(trainer.score.correct().is_some());

            interrupt(&mut trainer, &mut rng);
            assert_eq!(
                trainer.score.correct(),
                None,
                "a flash survived being interrupted"
            );
        }
    }

    /// A tick that crossed a skip or a screen change must not retire the prompt on screen.
    #[test]
    fn advancing_without_a_flash_does_nothing() {
        let (mut trainer, mut rng) = trainer_with_seed(0x71c4);

        let standing = trainer.prompt;
        trainer.advance(&mut rng);

        assert_eq!(trainer.prompt, standing);
        assert_eq!(trainer.score.streak(), 0);
    }

    #[test]
    fn a_wrong_answer_zeroes_the_streak_and_keeps_the_prompt() {
        let (mut trainer, mut rng) = trainer_with_seed(11);

        answer_and_advance(&mut trainer, &mut rng);
        assert_eq!(trainer.score.streak(), 1);

        let standing = trainer.prompt;
        let wrong = wrong_answer(&trainer);
        trainer.answer(wrong);

        assert_eq!(trainer.score.streak(), 0);
        assert_eq!(
            trainer.prompt, standing,
            "a wrong answer retired the prompt"
        );
        assert!(
            trainer.score.wrong().contains(&wrong),
            "the wrong answer went unmarked"
        );
    }

    #[test]
    fn wrong_answers_accumulate_until_the_prompt_advances() {
        let (mut trainer, mut rng) = trainer_with_seed(0x5eed);
        trainer.prompt = Prompt::NameIt { string: 0, fret: 0 };

        let actual = DRILL_NECK.pitch_class_at(0, 0).unwrap();
        let wrongs: Vec<Answer> = PitchClass::ALL
            .into_iter()
            .filter(|&pc| pc != actual)
            .take(3)
            .map(Answer::Name)
            .collect();

        for &w in &wrongs {
            trainer.answer(w);
        }

        for &w in &wrongs {
            assert!(trainer.score.wrong().contains(&w));
        }

        // Repeating one does not grow the list.
        let before = trainer.score.wrong().len();
        trainer.answer(wrongs[0]);
        assert_eq!(trainer.score.wrong().len(), before);

        // The right answer joins them rather than clearing them.
        trainer.answer(Answer::Name(actual));
        assert_eq!(trainer.score.wrong().len(), before);

        trainer.advance(&mut rng);
        assert!(
            trainer.score.wrong().is_empty(),
            "feedback outlived its prompt"
        );
    }

    #[test]
    fn the_best_streak_survives_what_the_current_one_does_not() {
        let (mut trainer, mut rng) = trainer_with_seed(21);

        for _ in 0..5 {
            answer_and_advance(&mut trainer, &mut rng);
        }
        assert_eq!(trainer.score.best(), 5);

        let wrong = wrong_answer(&trainer);
        trainer.answer(wrong);
        assert_eq!((trainer.score.streak(), trainer.score.best()), (0, 5));

        trainer.skip(&mut rng);
        assert_eq!((trainer.score.streak(), trainer.score.best()), (0, 5));

        trainer.toggle_pool(&mut rng);
        assert_eq!((trainer.score.streak(), trainer.score.best()), (0, 5));

        trainer.toggle_direction(&mut rng);
        assert_eq!((trainer.score.streak(), trainer.score.best()), (0, 5));

        trainer.enter(&mut rng);
        assert_eq!((trainer.score.streak(), trainer.score.best()), (0, 5));
    }

    #[test]
    fn the_two_names_of_a_black_key_are_one_answer() {
        let (mut trainer, _) = trainer_with_seed(0);
        // Open low E plus two semitones — F♯, or G♭.
        trainer.prompt = Prompt::NameIt { string: 0, fret: 2 };

        let pitch_class = DRILL_NECK.pitch_class_at(0, 2).unwrap();
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

    #[test]
    fn find_it_accepts_every_position_carrying_the_note() {
        let (mut trainer, _) = trainer_with_seed(0);
        let target = PitchClass::new(7); // G
        trainer.prompt = Prompt::FindIt(target);

        let mut accepted = 0;
        for (string, fret) in all_positions() {
            let answer = Answer::Position { string, fret };
            let carries_it = DRILL_NECK.pitch_class_at(string, fret) == Some(target);

            assert_eq!(trainer.judge(answer), carries_it, "({string}, {fret})");
            accepted += usize::from(carries_it);
        }

        assert!(accepted > 1, "G should appear more than once on the neck");
    }

    /// A mismatched pair is always wrong, not a panic.
    #[test]
    fn an_answer_of_the_wrong_shape_is_simply_wrong() {
        let (mut trainer, _) = trainer_with_seed(0);

        trainer.prompt = Prompt::NameIt { string: 0, fret: 0 };
        assert!(!trainer.judge(Answer::Position { string: 0, fret: 0 }));

        trainer.prompt = Prompt::FindIt(DRILL_NECK.pitch_class_at(0, 0).unwrap());
        assert!(!trainer.judge(Answer::Name(DRILL_NECK.pitch_class_at(0, 0).unwrap())));
    }

    #[test]
    fn toggling_the_direction_flips_which_way_the_drill_runs() {
        let (mut trainer, mut rng) = trainer_with_seed(5);
        assert_eq!(trainer.drill(), Drill::NameIt, "Name it is not the default");

        trainer.toggle_direction(&mut rng);
        assert_eq!(trainer.drill(), Drill::FindIt);

        trainer.toggle_direction(&mut rng);
        assert_eq!(trainer.drill(), Drill::NameIt);
    }

    #[test]
    fn toggling_the_spelling_changes_nothing_but_the_names() {
        let (mut trainer, mut rng) = trainer_with_seed(8);

        answer_and_advance(&mut trainer, &mut rng);

        let before = (trainer.prompt, trainer.score.streak(), trainer.score.best());
        trainer.toggle_spelling();

        assert_eq!(trainer.spelling, Spelling::Flats);
        assert_eq!(
            (trainer.prompt, trainer.score.streak(), trainer.score.best()),
            before,
            "the spelling toggle disturbed the drill"
        );
    }

    #[test]
    fn the_spelling_control_wears_the_spelling_in_force() {
        let (mut trainer, _) = trainer_with_seed(8);

        assert_eq!(spelling_glyph(trainer.spelling), SMUFL_SHARP);

        trainer.toggle_spelling();

        assert_eq!(spelling_glyph(trainer.spelling), SMUFL_FLAT);
    }

    #[test]
    fn every_position_the_drill_can_prompt_is_on_the_neck() {
        for pool in [Pool::Naturals, Pool::All] {
            let (mut trainer, _) = trainer_with_seed(0);
            trainer.pool = pool;

            for (string, fret) in trainer.positions() {
                assert!(string < NECK_STRINGS && fret <= DRILL_NECK.frets());
                assert!(pool.contains(DRILL_NECK.pitch_class_at(string, fret).unwrap()));
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
        assert_eq!(app.note_trainer.drill(), Drill::FindIt);
        app
    }

    /// Answers the current prompt correctly through whichever message the view would send,
    /// then sends the flash's timer message.
    fn answer_correctly(app: &mut App) {
        let message = match correct_answer(&app.note_trainer) {
            Answer::Name(pitch_class) => Message::AnswerNote(pitch_class),
            Answer::Position { string, fret } => Message::ChooseNotePosition(string, fret),
            Answer::Played(_) => unreachable!("pressing answers are never played"),
        };

        let _ = app.update(message);
        let _ = app.update(Message::AdvancePrompt);
    }

    #[test]
    fn opening_the_note_trainer_lands_on_a_prompt() {
        let app = note_trainer_app(1);

        assert_eq!(app.screen, Screen::NoteTrainer);
        assert_eq!(app.note_trainer.score.streak(), 0);
        assert!(app.note_trainer.score.wrong().is_empty());
    }

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
            app.note_trainer.drill(),
            app.note_trainer.pool,
            app.note_trainer.spelling,
            app.note_trainer.score.best(),
        );

        let _ = app.update(Message::GoBack);
        let _ = app.update(Message::Navigate(Screen::NoteTrainer));

        assert_eq!(
            (
                app.note_trainer.drill(),
                app.note_trainer.pool,
                app.note_trainer.spelling,
                app.note_trainer.score.best(),
            ),
            kept,
        );
        assert_eq!(
            app.note_trainer.score.streak(),
            0,
            "the run outlived the visit"
        );
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
        app.note_trainer.cursor = at(2, 4);

        for (named, expected) in [
            (Named::ArrowRight, at(3, 4)),
            (Named::ArrowDown, at(3, 5)),
            (Named::ArrowLeft, at(2, 5)),
            (Named::ArrowUp, at(2, 4)),
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
        app.note_trainer.cursor = at(2, 4);

        for (key, expected) in [
            ("l", at(3, 4)),
            ("j", at(3, 5)),
            ("h", at(2, 5)),
            ("k", at(2, 4)),
        ] {
            press_into(&mut app, key, keyboard::Modifiers::empty());

            assert_eq!(app.note_trainer.cursor, expected, "{key}");
            assert_eq!(app.focused, FocusTarget::Fretboard, "{key} moved the ring");
        }
    }

    /// Arrows never leave the neck, so Tab has to — from every corner of it.
    #[test]
    fn tab_always_escapes_the_neck() {
        for cursor in [
            at(0, 0),
            at(0, DRILL_NECK.frets()),
            at(NECK_STRINGS - 1, 0),
            at(NECK_STRINGS - 1, DRILL_NECK.frets()),
            at(3, 6),
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
            .find(|&(s, f)| DRILL_NECK.pitch_class_at(s, f) == Some(target))
            .unwrap();
        app.note_trainer.cursor = Position { string, fret };

        press_named(&mut app, Named::Enter);

        assert_eq!(app.note_trainer.score.streak(), 1, "Enter did not answer");
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
            .find(|&(s, f)| DRILL_NECK.pitch_class_at(s, f) != Some(target))
            .unwrap();
        app.note_trainer.cursor = Position { string, fret };

        press_named(&mut app, Named::Space);

        assert_eq!(app.note_trainer.score.streak(), 0);
        assert_eq!(app.note_trainer.prompt, standing);
        assert!(
            app.note_trainer
                .score
                .wrong()
                .contains(&Answer::Position { string, fret })
        );

        let markers = position_markers(&app.note_trainer);
        assert_eq!(markers.len(), 1);
        // Filled, so a guess can never be mistaken for the prompt's ring.
        assert_eq!(markers[0].style, MarkerStyle::Filled);
    }

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

        // ...and the other direction puts nothing of the prompt on the neck.
        trainer.toggle_direction(&mut rng);
        assert!(
            prompt_marker(&trainer).is_empty(),
            "Find it marked the neck with its prompt"
        );
    }

    #[test]
    fn a_right_cursor_position_marks_the_neck_green() {
        let mut app = find_it_app(0x9e);
        app.focused = FocusTarget::Fretboard;

        let Answer::Position { string, fret } = correct_answer(&app.note_trainer) else {
            unreachable!("find_it_app guarantees the direction")
        };
        app.note_trainer.cursor = Position { string, fret };

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

    #[test]
    fn a_right_answer_marks_its_button() {
        let mut app = note_trainer_app(0xb17);

        let Answer::Name(pitch_class) = correct_answer(&app.note_trainer) else {
            unreachable!("the Note Trainer opens in Name it")
        };

        let _ = app.update(Message::AnswerNote(pitch_class));
        assert_eq!(
            app.note_trainer.score.correct(),
            Some(Answer::Name(pitch_class))
        );
        let _ = app.view();

        let _ = app.update(Message::AdvancePrompt);
        assert_eq!(app.note_trainer.score.correct(), None);
    }

    /// The flash's tick is not a keypress, so it must not dismiss the help overlay.
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

    /// A click sends the canvas's press message and drags the cursor along.
    #[test]
    fn a_press_on_the_neck_answers_and_moves_the_cursor() {
        let mut app = find_it_app(0x5a);

        let Prompt::FindIt(target) = app.note_trainer.prompt else {
            unreachable!()
        };
        let (string, fret) = all_positions()
            .find(|&(s, f)| DRILL_NECK.pitch_class_at(s, f) == Some(target))
            .unwrap();

        let _ = app.update(Message::ChooseNotePosition(string, fret));

        assert_eq!(
            app.note_trainer.cursor,
            Position { string, fret },
            "the cursor lagged"
        );
        assert_eq!(app.note_trainer.score.streak(), 1);
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
        assert_eq!(app.note_trainer.drill(), Drill::FindIt, "d did not swap");

        // Focus stays on a widget gone in the new direction; `step_focus` snaps it back.
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

    /// `i` toggles interval notation on the scale trainer and must stay inert here.
    #[test]
    fn the_interval_notation_key_is_inert_on_the_note_trainer() {
        let mut app = note_trainer_app(0x11);

        let before = (note_trainer_state(&app), app.notation);
        press_into(&mut app, "i", keyboard::Modifiers::empty());

        assert_eq!((note_trainer_state(&app), app.notation), before);
    }

    /// Everything a keypress on the Note Trainer could disturb.
    fn note_trainer_state(app: &App) -> (Prompt, Pool, Spelling, u32, u32, Position) {
        let t = &app.note_trainer;
        (
            t.prompt,
            t.pool,
            t.spelling,
            t.score.streak(),
            t.score.best(),
            t.cursor,
        )
    }

    #[test]
    fn the_help_overlay_lists_the_note_trainers_keys() {
        let bound = accelerators_for(&Screen::NoteTrainer, Notation::Notes, &WalkTheScale::new());
        let keys: Vec<char> = bound.iter().map(|&(key, _, _)| key).collect();

        assert_eq!(keys, vec!['r', 'd', 'a', 'm']);

        for (key, _, label) in bound {
            assert!(!label.is_empty(), "{key} has no label for the overlay");
        }
    }

    /// Builds the widget tree for every screen and drill direction, since layout is checked
    /// at construction rather than by the type system.
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

        // The scale drill: pressing, playing, with the neck hidden, and with a picker up.
        app.open(Screen::WalkTheScale);
        let _ = app.view();
        let _ = app.update(Message::CycleScaleDuration);
        let _ = app.view();
        let _ = app.update(Message::ToggleScaleAnswering);
        let _ = app.view();
        let _ = app.update(Message::ToggleScaleNeck);
        let _ = app.view();
        let _ = app.update(Message::OpenDevices);
        let _ = app.view();
        let _ = app.update(Message::GoBack);
        let _ = app.view();

        // The Position Finder: a note heard, its groups shown, mid-walk, and with a picker up.
        app.open(Screen::PositionFinder);
        let _ = app.update(Message::Fitted(
            crate::audio::detect::Heard::Pitch(crate::audio::detect::Detection::synthesized(
                329.63, 0.999,
            )),
            None,
        ));
        let _ = app.view();
        let _ = app.update(Message::TogglePositionGroups);
        let _ = app.view();
        let _ = app.update(Message::Calibrate);
        let _ = app.view();
        let _ = app.update(Message::OpenDevices);
        let _ = app.view();
        let _ = app.update(Message::GoBack);
        let _ = app.update(Message::GoBack);
        let _ = app.view();

        // Playing: with nothing to ask, asking by string, answered, and with a picker up.
        app.open(Screen::NoteTrainer);
        let _ = app.update(Message::ToggleNoteAnswering);
        let _ = app.view();
        let guitar = spike_guitar();
        app.note_trainer.calibrate_with(Some(guitar.clone()));
        let _ = app.update(Message::SkipPrompt);
        let _ = app.view();
        asking_for_the_high_e(&mut app);
        plucks(&mut app, &guitar, at(3, 9));
        let _ = app.view();
        let _ = app.update(Message::OpenDevices);
        let _ = app.view();
        let _ = app.update(Message::GoBack);
        let _ = app.update(Message::ToggleNoteAnswering);

        app.open(Screen::NoteTrainer);

        for _ in 0..2 {
            // A wrong answer in whichever direction is current, so the feedback path is built.
            let wrong = wrong_answer(&app.note_trainer);
            let message = match wrong {
                Answer::Name(pitch_class) => Message::AnswerNote(pitch_class),
                Answer::Position { string, fret } => Message::ChooseNotePosition(string, fret),
                Answer::Played(_) => unreachable!("pressing answers are never played"),
            };
            let _ = app.update(message);
            assert!(!app.note_trainer.score.wrong().is_empty());
            let _ = app.view();

            // ...and then the right one, so the flash is drawn on both surfaces too.
            let message = match correct_answer(&app.note_trainer) {
                Answer::Name(pitch_class) => Message::AnswerNote(pitch_class),
                Answer::Position { string, fret } => Message::ChooseNotePosition(string, fret),
                Answer::Played(_) => unreachable!("pressing answers are never played"),
            };
            let _ = app.update(message);
            assert!(app.note_trainer.score.correct().is_some());
            let _ = app.view();

            let _ = app.update(Message::ToggleDrillDirection);
            let _ = app.view();
        }
    }

    // ---- Playing ----

    /// An app on the Note Trainer, swapped into playing against `calibration`.
    fn playing_app(seed: u64, calibration: Option<Calibration>) -> App {
        let mut app = note_trainer_app(seed);
        app.note_trainer.calibrate_with(calibration);
        let _ = app.update(Message::ToggleNoteAnswering);
        app
    }

    /// Asks for E on the high E string, which the spike guitar places at its open string.
    fn asking_for_the_high_e(app: &mut App) {
        app.note_trainer.prompt = Prompt::PlayIt {
            pitch_class: PitchClass::new(4),
            string: 5,
        };
    }

    /// A clean pluck of `position`, held long enough to be placed.
    fn plucks(app: &mut App, guitar: &Calibration, position: Position) {
        let (pitch, log2) = pluck(guitar, position);
        for _ in 0..FITS_PER_NOTE {
            let _ = app.update(Message::Fitted(heard(pitch), measured(log2)));
        }
    }

    fn stops(app: &mut App) {
        let _ = app.update(Message::Fitted(crate::audio::detect::Heard::Silence, None));
    }

    fn marked(app: &App, color: Color) -> Vec<Position> {
        played_markers(&app.note_trainer)
            .into_iter()
            .filter(|marker| marker.color == color)
            .map(|marker| at(marker.string, marker.fret))
            .collect()
    }

    /// Every (pitch class, string) a playing prompt names over many draws.
    fn drawn(app: &mut App, draws: usize) -> Vec<(PitchClass, usize)> {
        (0..draws)
            .map(|_| {
                let _ = app.update(Message::SkipPrompt);
                match app.note_trainer.prompt {
                    Prompt::PlayIt {
                        pitch_class,
                        string,
                    } => (pitch_class, string),
                    other => panic!("{other:?} is not a playing prompt"),
                }
            })
            .collect()
    }

    #[test]
    fn swapping_into_playing_keeps_the_streak_and_asks_by_string() {
        let mut app = note_trainer_app(0x91a);
        app.note_trainer.calibrate_with(Some(spike_guitar()));
        answer_correctly(&mut app);
        answer_correctly(&mut app);

        press_into(&mut app, "m", keyboard::Modifiers::empty());

        assert_eq!(app.note_trainer.answering(), Answering::Playing);
        assert!(matches!(app.note_trainer.prompt, Prompt::PlayIt { .. }));
        assert_eq!(app.note_trainer.score.streak(), 2);
    }

    #[test]
    fn playing_lights_the_asked_string_and_pressing_lights_none() {
        let mut app = playing_app(0x91d, Some(spike_guitar()));
        for _ in 0..20 {
            let _ = app.update(Message::SkipPrompt);
            let Prompt::PlayIt { string, .. } = app.note_trainer.prompt else {
                panic!("{:?} is not a playing prompt", app.note_trainer.prompt);
            };

            let lit = asked_string(&app.note_trainer).expect("a playing prompt lights its string");
            assert_eq!((lit.string, lit.color), (string, LINK));
        }

        let _ = app.update(Message::ToggleNoteAnswering);
        assert!(asked_string(&app.note_trainer).is_none());
    }

    #[test]
    fn swapping_back_asks_in_the_direction_it_was_set_to() {
        let mut app = find_it_app(0x91b);
        app.note_trainer.calibrate_with(Some(spike_guitar()));

        press_into(&mut app, "m", keyboard::Modifiers::empty());
        press_into(&mut app, "m", keyboard::Modifiers::empty());

        assert_eq!(app.note_trainer.answering(), Answering::Pressing);
        assert_eq!(app.note_trainer.drill(), Drill::FindIt);
        assert!(matches!(app.note_trainer.prompt, Prompt::FindIt(_)));
    }

    #[test]
    fn a_device_is_held_only_while_playing() {
        let mut app = note_trainer_app(0x91c);
        assert!(
            app.note_trainer.listener().is_none(),
            "pressing holds a device"
        );

        let _ = app.update(Message::ToggleNoteAnswering);
        assert!(
            app.note_trainer
                .listener()
                .is_some_and(Listener::is_fitting)
        );

        let _ = app.update(Message::ToggleNoteAnswering);
        assert!(app.note_trainer.listener().is_none());
    }

    #[test]
    fn leaving_while_playing_releases_the_device_and_coming_back_claims_it() {
        let mut app = playing_app(0x91d, None);

        let _ = app.update(Message::GoBack);
        assert_eq!(app.screen, Screen::Home);
        assert!(app.note_trainer.listener().is_none());

        let _ = app.update(Message::Navigate(Screen::NoteTrainer));
        assert_eq!(app.note_trainer.answering(), Answering::Playing);
        assert!(app.note_trainer.listener().is_some());
    }

    #[test]
    fn without_a_calibration_nothing_is_asked_and_the_position_finder_is_named() {
        let app = playing_app(0x91e, None);

        assert_eq!(app.note_trainer.prompt, Prompt::Unaskable);
        assert!(played_markers(&app.note_trainer).is_empty());
        assert!(notice_line(&app.note_trainer).contains("Position Finder"));
    }

    #[test]
    fn no_grouped_position_is_ever_asked_for() {
        let guitar = grouping_guitar();
        let grouped: Vec<Position> = groups(&guitar, &DRILL_NECK)
            .into_iter()
            .flat_map(|group| group.positions)
            .collect();
        assert!(!grouped.is_empty(), "the guitar groups nothing to avoid");

        let mut app = playing_app(0x92a, Some(guitar));
        let _ = app.update(Message::TogglePool);

        for (pitch_class, string) in drawn(&mut app, 500) {
            for fret in 0..=DRILL_NECK.frets() {
                if DRILL_NECK.pitch_class_at(string, fret) == Some(pitch_class) {
                    assert!(
                        !grouped.contains(&at(string, fret)),
                        "{pitch_class:?} on string {string} sits in a group"
                    );
                }
            }
        }
    }

    #[test]
    fn a_flagged_string_is_never_asked_for() {
        let open = [-12.2f32, -13.0, -13.2, -13.09, -14.57, -16.0].map(f32::exp2);
        let mut twelfth = open.map(|b| 4.0 * b);
        twelfth[2] = open[2];
        let guitar = Calibration::from_readings(open.map(Some), twelfth.map(Some));
        assert!(guitar.as_ref().is_some_and(|kept| kept.is_flagged(2)));

        let mut app = playing_app(0x92b, guitar);

        assert!(drawn(&mut app, 500).iter().all(|&(_, string)| string != 2));
    }

    #[test]
    fn playing_draws_from_the_pool_and_never_repeats_a_prompt() {
        let mut app = playing_app(0x92c, Some(spike_guitar()));

        let first = match app.note_trainer.prompt {
            Prompt::PlayIt {
                pitch_class,
                string,
            } => (pitch_class, string),
            other => panic!("{other:?} is not a playing prompt"),
        };
        let mut prompts = vec![first];
        prompts.extend(drawn(&mut app, 300));

        assert!(
            prompts
                .iter()
                .all(|(pc, _)| PitchClass::NATURALS.contains(pc))
        );
        assert!(prompts.windows(2).all(|pair| pair[0] != pair[1]));

        press_into(&mut app, "a", keyboard::Modifiers::empty());
        assert!(
            drawn(&mut app, 300)
                .iter()
                .any(|(pitch_class, _)| !PitchClass::NATURALS.contains(pitch_class)),
            "all twelve never reached an accidental"
        );
    }

    #[test]
    fn the_right_note_on_the_asked_string_is_right() {
        let guitar = spike_guitar();
        let mut app = playing_app(0x93a, Some(guitar.clone()));
        asking_for_the_high_e(&mut app);

        plucks(&mut app, &guitar, at(5, 0));

        assert_eq!(app.note_trainer.score.streak(), 1);
        assert_eq!(marked(&app, SUCCESS), vec![at(5, 0)]);
        assert!(marked(&app, DANGER).is_empty());
    }

    #[test]
    fn the_right_note_on_another_string_is_wrong_and_shown_there() {
        let guitar = spike_guitar();
        let mut app = playing_app(0x93b, Some(guitar.clone()));
        answer_correctly_by_playing(&mut app, &guitar);
        asking_for_the_high_e(&mut app);
        let standing = app.note_trainer.prompt;

        // E4 on the G string's 9th fret
        plucks(&mut app, &guitar, at(3, 9));

        assert_eq!(app.note_trainer.score.streak(), 0);
        assert_eq!(app.note_trainer.prompt, standing);
        assert_eq!(marked(&app, DANGER), vec![at(3, 9)]);
    }

    #[test]
    fn a_narrowed_placement_on_the_asked_string_is_right() {
        let guitar = grouping_guitar();
        let mut app = playing_app(0x93c, Some(guitar.clone()));
        app.note_trainer.prompt = Prompt::PlayIt {
            pitch_class: PitchClass::new(9),
            string: 3,
        };

        plucks(&mut app, &guitar, at(3, 2));

        assert_eq!(app.note_trainer.score.streak(), 1);
        assert_eq!(marked(&app, SUCCESS), vec![at(2, 7), at(3, 2)]);
    }

    #[test]
    fn the_wrong_note_is_wrong_without_waiting_for_a_placement() {
        let guitar = spike_guitar();
        let mut app = playing_app(0x93d, Some(guitar.clone()));
        answer_correctly_by_playing(&mut app, &guitar);
        asking_for_the_high_e(&mut app);

        let f4 = DRILL_NECK.pitch_at(5, 1).expect("on the neck");
        let _ = app.update(Message::Fitted(heard(f4), None));

        assert_eq!(app.note_trainer.score.streak(), 0);
        assert_eq!(
            app.note_trainer.score.wrong(),
            [Answer::Played(f4.pitch_class())]
        );
    }

    #[test]
    fn a_note_slid_into_is_not_judged_and_says_why() {
        let guitar = spike_guitar();
        let mut app = playing_app(0x93e, Some(guitar.clone()));
        answer_correctly_by_playing(&mut app, &guitar);
        asking_for_the_high_e(&mut app);
        let standing = app.note_trainer.prompt;

        let (e4, open_e) = pluck(&guitar, at(5, 0));
        let from = DRILL_NECK.pitch_at(5, 1).expect("on the neck").frequency();
        let bent = crate::audio::detect::Heard::Pitch(
            Detection::synthesized(e4.frequency(), 0.999).slid_from(from),
        );
        let _ = app.update(Message::Fitted(bent, measured(open_e)));

        assert_eq!(app.note_trainer.score.streak(), 1);
        assert!(app.note_trainer.score.wrong().is_empty());
        assert_eq!(app.note_trainer.prompt, standing);
        assert!(notice_line(&app.note_trainer).contains("slid"));
    }

    #[test]
    fn a_note_that_cannot_be_read_is_not_judged_and_the_prompt_stays() {
        let guitar = spike_guitar();
        let mut app = playing_app(0x93f, Some(guitar.clone()));
        answer_correctly_by_playing(&mut app, &guitar);
        asking_for_the_high_e(&mut app);
        let standing = app.note_trainer.prompt;

        let (e4, _) = pluck(&guitar, at(5, 0));
        let _ = app.update(Message::Fitted(
            heard(e4),
            Some(Err(Refusal::TooFewPartials)),
        ));
        stops(&mut app);

        assert_eq!(app.note_trainer.score.streak(), 1);
        assert_eq!(app.note_trainer.prompt, standing);
        assert!(notice_line(&app.note_trainer).contains("did not read cleanly"));

        // and played again cleanly, it is judged
        plucks(&mut app, &guitar, at(5, 0));
        assert_eq!(app.note_trainer.score.streak(), 2);
        assert!(notice_line(&app.note_trainer).is_empty());
    }

    #[test]
    fn a_held_wrong_note_is_judged_once() {
        let guitar = spike_guitar();
        let mut app = playing_app(0x940, Some(guitar.clone()));
        asking_for_the_high_e(&mut app);

        for _ in 0..20 {
            plucks(&mut app, &guitar, at(3, 9));
        }

        assert_eq!(app.note_trainer.score.wrong().len(), 1);
        assert_eq!(marked(&app, DANGER), vec![at(3, 9)]);
    }

    #[test]
    fn the_previous_answer_still_ringing_answers_nothing() {
        let guitar = spike_guitar();
        let mut app = playing_app(0x941, Some(guitar.clone()));
        asking_for_the_high_e(&mut app);
        plucks(&mut app, &guitar, at(5, 0));
        assert_eq!(app.note_trainer.score.streak(), 1);

        let _ = app.update(Message::AdvancePrompt);
        asking_for_the_high_e(&mut app);
        plucks(&mut app, &guitar, at(5, 0));
        assert_eq!(app.note_trainer.score.streak(), 1, "the tail answered");
        assert!(app.note_trainer.score.wrong().is_empty());

        stops(&mut app);
        plucks(&mut app, &guitar, at(5, 0));
        assert_eq!(app.note_trainer.score.streak(), 2);
    }

    /// Answers whatever is asked on the string it names, then lets the flash end.
    fn answer_correctly_by_playing(app: &mut App, guitar: &Calibration) {
        asking_for_the_high_e(app);
        plucks(app, guitar, at(5, 0));
        let _ = app.update(Message::AdvancePrompt);
        stops(app);
        assert_eq!(app.note_trainer.score.streak(), 1);
    }

    #[test]
    fn a_playing_prompt_names_the_note_and_string_and_lights_nothing() {
        let mut app = playing_app(0x950, Some(spike_guitar()));
        asking_for_the_high_e(&mut app);

        assert_eq!(string_line(&app.note_trainer), "on the high E string");
        assert!(played_markers(&app.note_trainer).is_empty());
        assert!(prompt_marker(&app.note_trainer).is_empty());

        app.note_trainer.prompt = Prompt::PlayIt {
            pitch_class: PitchClass::new(0),
            string: 3,
        };
        assert_eq!(string_line(&app.note_trainer), "on the G string");
    }

    #[test]
    fn while_playing_d_opens_the_input_list_and_the_overlay_says_so() {
        let mut app = playing_app(0x951, Some(spike_guitar()));
        let before = app.note_trainer.drill();

        let labels: Vec<String> = app
            .accelerators()
            .into_iter()
            .map(|(key, _, label)| format!("{key} {label}"))
            .collect();
        assert!(labels.contains(&"m answer: play".to_owned()), "{labels:?}");
        assert!(labels.contains(&"d input device".to_owned()), "{labels:?}");

        press_into(&mut app, "d", keyboard::Modifiers::empty());
        assert!(
            app.note_trainer
                .listener()
                .is_some_and(Listener::is_picking)
        );
        assert_eq!(app.note_trainer.drill(), before, "d swapped the direction");

        let _ = app.update(Message::GoBack);
        assert_eq!(app.screen, Screen::NoteTrainer, "Esc left with the list up");
    }

    #[test]
    fn every_key_claimed_while_playing_fires_a_target_on_the_grid() {
        let app = playing_app(0x952, Some(spike_guitar()));
        let reachable = app.focusables();

        for (key, fires, _) in app.accelerators() {
            let Fires::Widget(target) = fires else {
                panic!("`{key}` fires a message no widget stands behind");
            };
            assert!(reachable.contains(&target), "`{key}` targets {target:?}");
        }
        assert!(!reachable.contains(&FocusTarget::DrillDirectionToggle));
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
