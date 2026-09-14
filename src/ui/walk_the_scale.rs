//! Walk the Scale: the drill's state machine and the screen that draws it.
//!
//! Both halves live here so the state can keep its fields to itself — they are read all over
//! the views below and nowhere else, and `App` drives the drill through the handful of methods
//! marked `pub(super)`.
//!
//! A screen rather than a mode of the Scale Trainer, which is what this was first built as.
//! The two want different things from one keyboard: the trainer is a reference you read, and
//! this asks you to stop reading. `play_the_interval.rs` made the same split from the Interval
//! Trainer, for the same reason and one release earlier.
//!
//! It owns the scale it drills rather than reading `App`'s. The Scale Trainer's scale is a
//! different thing being looked at for a different reason, and rerolling here would otherwise
//! move a screen the learner is not on — the CAGED window keeps its own root on exactly that
//! argument.

use std::time::{Duration, Instant};

use iced::{Color, Element, Length, Padding};

use super::fretboard::{Fretboard, MarkerStyle, NoteMarker, fretboard};
use super::listener::{Listener, State, ui_device_line, ui_device_picker};
use super::{
    BODY, CONTROL_SIZE, CURSOR_HOME, DANGER, DRILL_NECK, Direction, FocusTarget, INK, LINK,
    MUSIC_FONT, MUTE, Message, NECK_STRINGS, SUCCESS, card_container, control_button,
    control_label, control_shuffle, kind_row_spans, note_label, root_note_row, root_row_spans,
    scale_kind_row,
};
use crate::audio::detect::Heard;
use crate::music::intervals::Interval;
use crate::music::notes::PitchClass;
use crate::music::pitch::Pitch;
use crate::music::scales::{Scale, ScaleKind};
use crate::rng::Rng;

/// How the learner answers the drill.
///
/// `neck_hidden` sits *inside* `Playing` rather than beside it because the two facts are not
/// independent: the neck is the surface a press is given on, so "pressing, with the neck
/// hidden" is a state the drill has no answer for. Putting the flag in the variant makes
/// that state unwriteable rather than merely unwritten — swapping to `Pressing` drops the
/// flag with the variant that held it, which is also the whole implementation of "swapping
/// back shows the neck again".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Answering {
    Pressing,
    Playing { neck_hidden: bool },
}

/// What the drill will do with the next confirmed reading.
///
/// A copy of `play_the_interval::Gate`, deliberately. The two drills agree today, and the
/// Changes Trainer — which walks a progression, where the chord under an element moves as
/// well as the degree — is the screen that will say whether there is one abstraction here or
/// two. `shapes.rs` and `listener.rs` both moved out when a second screen actually wanted
/// them rather than when a first author guessed; this copy waits for the same signal, and
/// names its twin so the eventual extraction has both sites to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gate {
    Deaf,
    Listening,
}

/// One run of the drill.
///
/// Everything here belongs to a single run, which is why it is a struct behind one `Mode`
/// variant rather than a handful of fields on `WalkTheScale` that would survive into display
/// mode carrying the last run's values.
struct Run {
    /// How far through the sequence. The *whole* of what is stored about it: the degrees
    /// themselves are `scale.kind().intervals()`, asked for at read time. A stored sequence
    /// would be a field free to disagree with the scale beside it, which is the argument the
    /// CAGED window's derived cycle already makes.
    at: usize,
    /// Sequences completed without a wrong answer.
    streak: u32,
    /// Sequences completed at all, which is what a timed run reports.
    completed: u32,
    /// Whether this sequence has been answered wrongly yet. A sequence is clean until it is
    /// not, and only a clean one raises the streak.
    clean: bool,
    /// The degrees found so far in this sequence, marked as the walk goes up.
    answered: Vec<PitchClass>,
    /// Every wrong pitch class given to the *current* element, cleared when it advances — a
    /// `Vec` for the reason the other drills' is one: replacing the last wrong answer would
    /// un-mark the earlier ones.
    wrong: Vec<PitchClass>,
    /// The pitch class that completed a sequence, held while it is lit. `Some` is the paused
    /// state, and it marks the end of a *sequence* rather than of an element — a flash
    /// between every degree would cost seven seconds to walk a seven-note scale.
    flashing: Option<PitchClass>,
    gate: Gate,
    /// When the run began. Stored rather than a countdown: a `remaining` decremented on a
    /// tick drifts, and lies outright if a tick is missed while the window is not drawn.
    started: Instant,
    /// `None` for an untimed run, so "no duration chosen" and "runs until you leave" are one
    /// fact rather than two.
    duration: Option<Duration>,
    /// Where the keyboard is pointing on the neck, for answering without a mouse.
    cursor: (usize, usize),
}

/// What the screen is doing.
///
/// Two states rather than three: the drill is the whole screen now, so there is no display to
/// fall back to — entering it starts a run. What a finished run completed is a state of its
/// own rather than the absence of one, which is why this is not an `Option<Run>`.
enum Mode {
    Running(Run),
    Ended { completed: u32, best_streak: u32 },
}

/// The Scale Trainer's drill.
///
/// No iced types, so the sequence, the gate and the streak rules are testable without a
/// widget tree — and, because `Listener::apply` is the seam, without a capture device.
pub(super) struct WalkTheScale {
    /// The scale being drilled, owned here rather than read off `App`.
    ///
    /// The Scale Trainer's scale is a different thing being looked at for a different reason,
    /// and a drill that moved it would mean rerolling here changed a screen the learner was
    /// not on. The CAGED window keeps its own root for the same reason.
    scale: Scale,
    /// A preference rather than part of a run: how a learner answers survives leaving the
    /// drill, which is why the `Listener` below is *not* inside this.
    answering: Answering,
    best_streak: u32,
    /// The capture device, held only while a run is listening. Opened and released
    /// explicitly, the way `leaving_for` already manages the two screens that listen; a
    /// device living inside `Answering::Playing` would still be open in display mode.
    listener: Option<Listener>,
    mode: Mode,
}

/// The durations `t` cycles through. `None` first, so the key's first press from the default
/// is a step *into* timing rather than out of it.
const DURATIONS: [Option<Duration>; 4] = [
    None,
    Some(Duration::from_secs(3 * 60)),
    Some(Duration::from_secs(5 * 60)),
    Some(Duration::from_secs(10 * 60)),
];

impl Run {
    fn new(duration: Option<Duration>, gate: Gate) -> Run {
        Run {
            at: 0,
            streak: 0,
            completed: 0,
            clean: true,
            answered: Vec::new(),
            wrong: Vec::new(),
            flashing: None,
            gate,
            started: Instant::now(),
            duration,
            cursor: CURSOR_HOME,
        }
    }

    /// The degree the drill is asking for, or `None` past the end of the sequence.
    ///
    /// `get` rather than an index: changing the scale kind mid-sequence can leave `at`
    /// pointing past a shorter kind's degrees, and the honest answer to "which degree is
    /// that" is that there is not one.
    fn degree(&self, scale: Scale) -> Option<Interval> {
        scale.kind().intervals().get(self.at).copied()
    }

    /// The pitch class that degree reaches above the root.
    fn target(&self, scale: Scale) -> Option<PitchClass> {
        self.degree(scale)
            .map(|degree| scale.root().transpose(degree.semitones()))
    }

    fn restart(&mut self) {
        self.at = 0;
        self.clean = true;
        self.answered.clear();
        self.wrong.clear();
        self.flashing = None;
    }

    fn remaining(&self) -> Option<Duration> {
        self.duration
            .map(|total| total.saturating_sub(self.started.elapsed()))
    }

    fn is_over(&self) -> bool {
        self.remaining().is_some_and(|left| left.is_zero())
    }
}

impl WalkTheScale {
    pub(super) fn new() -> WalkTheScale {
        WalkTheScale {
            scale: Scale::new(PitchClass::new(9), ScaleKind::MinorPentatonic),
            answering: Answering::Pressing,
            best_streak: 0,
            listener: None,
            mode: Mode::Running(Run::new(None, Gate::Listening)),
        }
    }

    pub(super) fn scale(&self) -> Scale {
        self.scale
    }

    /// Changing what is being drilled starts the sequence again: a position part-way through
    /// one kind's degrees means nothing in another's, and `at` would be left pointing past
    /// the end of a shorter kind — which `Run::degree` answers with `None` rather than a
    /// panic, but which no learner should ever see.
    pub(super) fn select_root(&mut self, root: PitchClass) {
        self.scale = Scale::new(root, self.scale.kind());
        self.restart_sequence();
    }

    pub(super) fn select_kind(&mut self, kind: ScaleKind) {
        self.scale = Scale::new(self.scale.root(), kind);
        self.restart_sequence();
    }

    /// `r`: replace what is on screen, which here is the scale being drilled.
    pub(super) fn reroll(&mut self, rng: &mut Rng) {
        let current = (self.scale.root(), self.scale.kind());

        loop {
            let root = PitchClass::ALL[rng.below(PitchClass::ALL.len())];
            let kind = ScaleKind::ALL[rng.below(ScaleKind::ALL.len())];

            if (root, kind) != current {
                self.scale = Scale::new(root, kind);
                self.restart_sequence();
                return;
            }
        }
    }

    pub(super) fn answering(&self) -> Answering {
        self.answering
    }

    pub(super) fn best_streak(&self) -> u32 {
        self.best_streak
    }

    pub(super) fn is_running(&self) -> bool {
        matches!(self.mode, Mode::Running(_))
    }

    /// Starts another run after one has ended on its duration.
    pub(super) fn restart(&mut self) {
        let duration = self.run().and_then(|run| run.duration);
        self.begin(duration);
    }

    pub(super) fn is_flashing(&self) -> bool {
        matches!(&self.mode, Mode::Running(run) if run.flashing.is_some())
    }

    pub(super) fn listener(&self) -> Option<&Listener> {
        self.listener.as_ref()
    }

    pub(super) fn listener_mut(&mut self) -> Option<&mut Listener> {
        self.listener.as_mut()
    }

    fn run(&self) -> Option<&Run> {
        match &self.mode {
            Mode::Running(run) => Some(run),
            _ => None,
        }
    }

    /// How far through the sequence the run is, as `(found, of)`.
    pub(super) fn progress(&self) -> Option<(usize, usize)> {
        let len = self.scale.kind().intervals().len();
        self.run().map(|run| (run.at, len))
    }

    pub(super) fn streak(&self) -> u32 {
        self.run().map_or(0, |run| run.streak)
    }

    pub(super) fn remaining(&self) -> Option<Duration> {
        self.run().and_then(Run::remaining)
    }

    pub(super) fn summary(&self) -> Option<(u32, u32)> {
        match &self.mode {
            Mode::Ended {
                completed,
                best_streak,
            } => Some((*completed, *best_streak)),
            _ => None,
        }
    }

    fn begin(&mut self, duration: Option<Duration>) {
        if matches!(self.answering, Answering::Playing { .. }) {
            let listener = self.listener.get_or_insert_with(Listener::new);
            listener.listen();
        }

        // Armed off the device rather than set flat, so a run begun in silence judges the
        // next note and one begun over a ringing string waits for it to stop.
        let gate = match self.listener.as_ref().and_then(Listener::reading) {
            Some(_) => Gate::Deaf,
            None => Gate::Listening,
        };

        self.mode = Mode::Running(Run::new(duration, gate));
    }

    /// Leaving the screen: the run ends and the device goes down with it, so a duration
    /// cannot go on being spent behind a screen nobody is looking at.
    pub(super) fn leave(&mut self) {
        self.mode = Mode::Ended {
            completed: 0,
            best_streak: self.best_streak,
        };
        self.release();
    }

    fn release(&mut self) {
        if let Some(listener) = self.listener.as_mut() {
            listener.stop();
        }
        self.listener = None;
    }

    /// Opening the screen. The scale is what it was left on — a learner comes back to the
    /// scale they are working on, which is the CAGED window's rule rather than the Scale
    /// Trainer's — while the run itself is fresh and the best streak persists.
    pub(super) fn enter(&mut self) {
        self.release();
        self.begin(None);
    }

    /// The selection changed under a running sequence, so the sequence starts again — a
    /// position part-way through one kind's degrees means nothing in another's.
    pub(super) fn restart_sequence(&mut self) {
        if let Mode::Running(run) = &mut self.mode {
            run.restart();
        }
    }

    pub(super) fn toggle_answering(&mut self) {
        self.answering = match self.answering {
            Answering::Pressing => Answering::Playing { neck_hidden: false },
            Answering::Playing { .. } => Answering::Pressing,
        };

        match self.answering {
            Answering::Playing { .. } if self.is_running() => {
                let listener = self.listener.get_or_insert_with(Listener::new);
                listener.listen();
            }
            _ => self.release(),
        }
    }

    /// Hiding the neck is a playing-mode option: in pressing mode the neck is the surface
    /// the answer is given on, so this is inert there rather than leaving a drill with
    /// nothing to answer with.
    pub(super) fn toggle_neck(&mut self) {
        if let Answering::Playing { neck_hidden } = &mut self.answering {
            *neck_hidden = !*neck_hidden;
        }
    }

    pub(super) fn neck_hidden(&self) -> bool {
        matches!(self.answering, Answering::Playing { neck_hidden: true })
    }

    pub(super) fn duration(&self) -> Option<Duration> {
        self.run().and_then(|run| run.duration)
    }

    /// A position pressed on the neck. Inert in playing mode, where the answer is given on
    /// the instrument and a press would be a second way to answer the same prompt.
    pub(super) fn press(&mut self, pitch_class: PitchClass) {
        if self.answering == Answering::Pressing {
            self.judge(pitch_class);
        }
    }

    /// A released reading, folded in and judged.
    ///
    /// The asymmetry is `play_the_interval::apply`'s: a reading that is *not* a pitch
    /// re-arms and is otherwise ignored, so silence and a strum cost nothing — which is what
    /// makes a fumble on the way to a degree free, and what lets two consecutive degrees be
    /// judged without the drill being told where one ends.
    pub(super) fn apply(&mut self, heard: Heard) {
        if !matches!(self.answering, Answering::Playing { .. }) {
            return;
        }

        let Heard::Pitch(detection) = heard else {
            if let Mode::Running(run) = &mut self.mode {
                run.gate = Gate::Listening;
            }
            return;
        };

        let Mode::Running(run) = &self.mode else {
            return;
        };
        if run.flashing.is_some() || run.gate == Gate::Deaf {
            return;
        }

        let Some((pitch, _)) = Pitch::nearest(detection.hz()) else {
            return;
        };

        // Judged once, whatever happens next: a held string confirms the same reading on
        // every poll, and this is what makes one unbroken note one answer.
        if let Mode::Running(run) = &mut self.mode {
            run.gate = Gate::Deaf;
        }
        self.judge(pitch.pitch_class());
    }

    /// Scores a pitch class against the element being asked for.
    ///
    /// The octave is thrown away, which is the rule every other drill in the app applies —
    /// so a degree is correct wherever on the neck it lies. The cost is that a walked scale
    /// need not literally ascend; requiring that would be drilling where the octave is, on
    /// top of the degree it means to teach.
    fn judge(&mut self, played: PitchClass) {
        let scale = self.scale;
        let streak = {
            let Mode::Running(run) = &mut self.mode else {
                return;
            };
            if run.flashing.is_some() {
                return;
            }
            let Some(target) = run.target(scale) else {
                return;
            };

            if played == target {
                run.answered.push(played);
                run.wrong.clear();
                run.at += 1;

                // The sequence is over, so this is what scores. Elements advance on the tick
                // they are judged; only the completion holds the screen.
                if run.at >= scale.kind().intervals().len() {
                    run.completed += 1;
                    if run.clean {
                        run.streak += 1;
                    }
                    run.flashing = Some(played);
                }
            } else {
                if !run.wrong.contains(&played) {
                    run.wrong.push(played);
                }
                run.clean = false;
                run.streak = 0;
            }

            run.streak
        };

        self.best_streak = self.best_streak.max(streak);
    }

    /// Ends the flash and begins the next sequence.
    ///
    /// A no-op when nothing is being marked, so a tick that outlived its flash — one that
    /// crossed a selection change, or landed after the screen was left — cannot retire a
    /// sequence the learner is still reading.
    pub(super) fn advance(&mut self) {
        // Read before the run is borrowed, and off the device rather than set flat: the note
        // that completed the sequence is still ringing as the next one is drawn, and a
        // sequence that took what was already sounding would have its first element answered
        // by the last one's answer.
        let gate = match self.listener.as_ref().and_then(Listener::reading) {
            Some(_) => Gate::Deaf,
            None => Gate::Listening,
        };

        if let Mode::Running(run) = &mut self.mode
            && run.flashing.is_some()
        {
            run.restart();
            run.gate = gate;
        }
    }

    /// One second of a timed run. Ends it when the duration is spent.
    pub(super) fn tick(&mut self) {
        let ended = match &self.mode {
            Mode::Running(run) if run.is_over() => Some((run.completed, self.best_streak)),
            _ => None,
        };

        if let Some((completed, best_streak)) = ended {
            self.mode = Mode::Ended {
                completed,
                best_streak,
            };
            self.release();
        }
    }

    pub(super) fn cursor(&self) -> Option<(usize, usize)> {
        self.run().map(|run| run.cursor)
    }

    /// The neck claims the motion keys while it is focused, as the two trainers' do.
    pub(super) fn move_cursor(&mut self, direction: Direction) {
        let Mode::Running(run) = &mut self.mode else {
            return;
        };
        let (string, fret) = run.cursor;

        run.cursor = match direction {
            Direction::Left => (string.saturating_sub(1), fret),
            Direction::Right => ((string + 1).min(NECK_STRINGS - 1), fret),
            Direction::Up => (string, fret.saturating_sub(1)),
            Direction::Down => (string, (fret + 1).min(DRILL_NECK.frets())),
        };
    }

    /// Enter on the neck answers with wherever the cursor is sitting — the same thing a
    /// click on that position would send.
    pub(super) fn answer_at_cursor(&mut self) {
        let Some((string, fret)) = self.cursor() else {
            return;
        };
        if let Some(pitch_class) = DRILL_NECK.pitch_class_at(string, fret) {
            self.press(pitch_class);
        }
    }

    /// Moves a run's start back, so a duration can be spent in a test without spending it.
    #[cfg(test)]
    fn rewind(&mut self, by: Duration) {
        if let Mode::Running(run) = &mut self.mode {
            run.started -= by;
        }
    }

    /// Cycles the duration. Restarting the run is deliberate: a duration applied part-way
    /// through would be counted from a start that belonged to the old one.
    pub(super) fn cycle_duration(&mut self) {
        let current = self.duration();
        let next = DURATIONS
            .iter()
            .position(|d| *d == current)
            .map_or(0, |at| (at + 1) % DURATIONS.len());

        if self.is_running() {
            self.begin(DURATIONS[next]);
        }
    }
}

/// The screen: the neck on the left, what is being asked and what it is set to beneath it,
/// and the two selector grids on the right.
pub(super) fn ui_walk_the_scale(
    drill: &WalkTheScale,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::widget::{Space, column, container, row, text};

    // Modal while it is up, as it is on the two other screens that listen: what the screen
    // was showing is not drawn behind it.
    if let Some(listener) = drill.listener()
        && listener.is_picking()
    {
        return container(ui_device_picker(listener))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
    }

    let scale = drill.scale();

    let fb = Fretboard {
        num_frets: DRILL_NECK.frets(),
        highlighted: drill_markers(drill),
        cursor: (drill.answering() == Answering::Pressing)
            .then(|| drill.cursor())
            .flatten(),
        on_press: Some(Message::ChooseScalePosition),
    };

    // The neck has a column to itself. It is seventy-eight positions in six rows and it is
    // what the learner is reading, so anything sharing the column takes width the strings
    // need — which is the whole reason this screen is laid out in three.
    let neck: Element<'static, Message> = if drill.neck_hidden() {
        container(
            text("the neck is hidden — find the degree by ear")
                .size(13)
                .color(MUTE),
        )
        .width(Length::Fixed(NECK_COLUMN_WIDTH))
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    } else {
        // Centred in its column. The canvas caps its own width off the height it is given —
        // a twelve-fret neck has an aspect ratio and `Layout` keeps it — so a column wider
        // than the neck leaves a gap, and the gap belongs on both sides rather than one.
        container(fretboard(fb))
            .width(Length::Fixed(NECK_COLUMN_WIDTH))
            .height(Length::Fill)
            .into()
    };

    // The middle column, on the ground rather than in a card: the root the scale is built on,
    // set large, and the sequence along the bottom. No card, because a card is a boundary and
    // there is nothing here to bound — this is the one thing the screen is saying.
    // The root and the sequence are one mark, not two: the degrees sit at the root's feet
    // rather than at the bottom of the column, and the pair is centred together.
    let centre = column![
        Space::new().height(Length::Fill),
        container(note_label(scale.root_note(), ROOT_DISPLAY_SIZE, INK))
            .width(Length::Fill)
            .center_x(Length::Fill),
        container(sequence_text(drill))
            .width(Length::Fill)
            .center_x(Length::Fill),
        Space::new().height(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    let root_card = container(
        container(
            root_row_spans().fold(column![].spacing(12), |rows, (start, len)| {
                rows.push(
                    container(root_note_row(
                        &PitchClass::ALL[start..start + len],
                        scale,
                        start,
                        focused,
                        Message::SelectDrillRoot,
                        FocusTarget::DrillRoot,
                    ))
                    .width(Length::Fill)
                    .center_x(Length::Fill),
                )
            }),
        )
        .width(Length::Fill)
        .center_x(Length::Fill),
    )
    .width(Length::Fill)
    .padding(24)
    .style(card_container);

    let kind_card = container(kind_row_spans().fold(
        column![].spacing(10),
        |rows, (start, len)| {
            rows.push(scale_kind_row(
                &ScaleKind::ALL[start..start + len],
                scale.kind(),
                start,
                focused,
                Message::SelectDrillKind,
                FocusTarget::DrillKind,
            ))
        },
    ))
    .width(Length::Fill)
    .padding(24)
    .style(card_container);

    // The selector and the run's controls share the right column: the selector says what is
    // being drilled and the controls say how, and neither belongs beside the neck.
    let selector = column![
        root_card,
        kind_card,
        Space::new().height(Length::Fill),
        drill_panel(drill, focused),
    ]
    .width(Length::Fixed(SELECTOR_COLUMN_WIDTH))
    .height(Length::Fill)
    .spacing(16);

    container(row![neck, centre, selector].spacing(32))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: 24.0,
            right: 64.0,
            bottom: 48.0,
            left: 64.0,
        })
        .into()
}

/// The root, set as large as the column allows. The one thing on the screen that is not a
/// question: which scale this is.
const ROOT_DISPLAY_SIZE: f32 = 120.0;

/// The sequence's own size, along the bottom of the middle column.
const SEQUENCE_SIZE: f32 = 34.0;

/// How wide the neck's column is.
///
/// Fixed rather than a share of the window, because the canvas caps its own width off the
/// height it is given — `Layout` holds a twelve-fret neck's aspect ratio — so a column with a
/// share of the width would leave the slack beside the neck instead of where it is wanted. The
/// number is what the neck actually draws at this window's height plus its own padding, so
/// there is no gap to give away.
const NECK_COLUMN_WIDTH: f32 = 340.0;

/// How wide the right column is.
///
/// Fixed rather than `Fill` because the kind pills carry names as long as `Harmonic Minor`, and
/// a column that shrank with the window would clip them before the neck ran out of room. The
/// number is what the widest of `kind_row_spans`'s rows asks for — `Spanish Gypsy`, `Whole Tone`
/// and `Diminished` together — plus the card's padding. A narrower column drew that row past the
/// card's edge, which is what `the_widest_kind_row_fits_the_selector_column` now catches.
const SELECTOR_COLUMN_WIDTH: f32 = 470.0;

/// The scale's formula, with the drill's place in it marked.
///
/// This is the sequence and the progress readout at once: the degrees already found are lit,
/// the one being asked for is bright, and the rest are muted. A `2 of 5` beside it would be a
/// second description of the same fact.
fn sequence_text(drill: &WalkTheScale) -> iced::widget::Row<'static, Message> {
    use iced::widget::{row, text};

    let scale = drill.scale();
    let at = drill.progress().map_or(usize::MAX, |(found, _)| found);

    scale.kind().intervals().iter().enumerate().fold(
        row![].spacing(14),
        |tokens, (index, interval)| {
            let color = match index.cmp(&at) {
                std::cmp::Ordering::Less => SUCCESS,
                std::cmp::Ordering::Equal => INK,
                std::cmp::Ordering::Greater => MUTE,
            };

            let (glyph, digit) = super::interval_token(*interval);
            let mut token = row![].spacing(0);

            if let Some(glyph) = glyph {
                token = token.push(
                    text(glyph.to_string())
                        .size(SEQUENCE_SIZE)
                        .font(MUSIC_FONT)
                        .color(color),
                );
            }

            tokens.push(token.push(text(digit.to_string()).size(SEQUENCE_SIZE).color(color)))
        },
    )
}

/// The run's controls and its numbers, at the foot of the right column.
///
/// Not beside the neck: the neck has a column to itself so the strings stay legible, and not
/// in the middle, which says one thing and says it large.
fn drill_panel(drill: &WalkTheScale, focused: FocusTarget) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, column, row, text};

    // `r` is the first control here, as the shuffle is on the Scale Trainer and the CAGED
    // window: replace what is on screen, which here is the scale being drilled.
    let mut controls = row![control_button(
        control_shuffle(),
        Message::RerollDrillScale,
        focused == FocusTarget::RerollDrillScale,
    )]
    .spacing(8);

    if drill.is_running() {
        controls = controls.push(control_button(
            // A word, not a glyph: `control_glyph` pins its content to one glyph's width so
            // that single marks line up, and anything longer is centred on a box narrower
            // than itself — which is how `press` came to sit off-centre in its button.
            control_label(
                text(match drill.answering() {
                    Answering::Pressing => "press",
                    Answering::Playing { .. } => "play",
                }),
                CONTROL_SIZE,
            ),
            Message::ToggleScaleAnswering,
            focused == FocusTarget::ScaleAnswering,
        ));
        controls = controls.push(control_button(
            control_label(text(super::duration_label(drill.duration())), CONTROL_SIZE),
            Message::CycleScaleDuration,
            focused == FocusTarget::ScaleDuration,
        ));

        if matches!(drill.answering(), Answering::Playing { .. }) {
            controls = controls.push(control_button(
                control_label(
                    text(if drill.neck_hidden() { "show" } else { "hide" }),
                    CONTROL_SIZE,
                ),
                Message::ToggleScaleNeck,
                focused == FocusTarget::ScaleNeck,
            ));
        }
    } else {
        // A run that ended on its duration: the one control is starting another.
        controls = controls.push(control_button(
            control_label(text("again"), CONTROL_SIZE),
            Message::StartWalkTheScale,
            focused == FocusTarget::RestartDrill,
        ));
    }

    // The run's numbers, under the controls. The degree being asked for is not here — it is
    // the middle column's job, and two places saying it would be two places to disagree.
    let readout: Element<'static, Message> = match drill.summary() {
        Some((completed, best)) => column![
            text(format!("{completed} sequences")).size(22).color(INK),
            text(format!("best streak {best}"))
                .size(CONTROL_SIZE)
                .color(MUTE),
        ]
        .spacing(4)
        .into(),
        None => row![
            text(format!(
                "streak {} · best {}",
                drill.streak(),
                drill.best_streak()
            ))
            .size(CONTROL_SIZE)
            .color(BODY),
            Space::new().width(Length::Fill),
            text(match drill.remaining() {
                Some(left) => super::duration_label(Some(left)),
                None => String::new(),
            })
            .size(CONTROL_SIZE)
            .color(MUTE),
        ]
        .into(),
    };

    let mut panel = column![controls, readout].spacing(12);

    // Only while playing: a screen answered with a press has nothing to say about a device
    // it is not holding.
    if let Some(listener) = drill.listener() {
        panel = panel.push(text(listening_line(listener)).size(13).color(MUTE));
        panel = panel.push(ui_device_line(listener, focused));
    }

    panel.into()
}

/// What the screen says it is hearing.
///
/// Split out as the words rather than the widget for the reason `play_the_interval`'s is:
/// the claim worth testing is that the four states read differently, and that none of them
/// names a note — neither of which a widget tree can be asked about.
fn listening_line(listener: &Listener) -> &'static str {
    match listener.state() {
        State::NotStarted => "not listening",
        State::NoInput => "no input — press m to answer with the neck instead",
        State::Silent => "listening, play the degree...",
        State::Unnameable => "one string at a time",
        State::Reading => "heard",
    }
}

/// What one pitch class is marked as while a drill is running, or `None` if it is not marked.
///
/// Priority is flash, then wrong, then answered: the note that completed a sequence is also
/// the last one answered, and the completion is what the screen is saying at that moment.
///
/// Only the answered marks carry a label, and it is the *degree* rather than the note — a
/// note name would let the next element be answered by hunting for a name, which is the
/// Note Trainer's drill rather than this one.
fn mark_for(run: &Run, scale: Scale, pitch_class: PitchClass) -> Option<(Color, String)> {
    if run.flashing == Some(pitch_class) {
        return Some((SUCCESS, String::new()));
    }
    if run.wrong.contains(&pitch_class) {
        return Some((DANGER, String::new()));
    }

    let index = run.answered.iter().position(|at| *at == pitch_class)?;
    let label = scale
        .kind()
        .intervals()
        .get(index)
        .map_or_else(String::new, Interval::to_string);

    Some((LINK, label))
}

/// The neck while a drill is running: what has been answered, what was answered wrongly, and
/// nothing else.
///
/// Deliberately *not* the scale. A neck with every degree marked and labelled carries the
/// answer to every element of the sequence, so the display's picture is withheld for as long
/// as the drill is asking for it.
fn drill_markers(drill: &WalkTheScale) -> Vec<NoteMarker> {
    let scale = drill.scale();
    let Some(run) = drill.run() else {
        return Vec::new();
    };

    let mut markers = Vec::new();

    for string in 0..NECK_STRINGS {
        for fret in 0..=DRILL_NECK.frets() {
            let Some(pitch_class) = DRILL_NECK.pitch_class_at(string, fret) else {
                continue;
            };
            let Some((color, label)) = mark_for(run, scale, pitch_class) else {
                continue;
            };

            markers.push(NoteMarker {
                string,
                fret,
                label,
                color,
                style: MarkerStyle::Filled,
            });
        }
    }

    markers
}

#[cfg(test)]
mod tests {
    use super::super::tests::{app_with_seed, press_into};
    use super::super::{App, Message, Screen};
    use super::*;
    use crate::audio::detect::Detection;
    use crate::ui::KIND_BUTTON_SIZE;
    use iced::keyboard;

    /// An app parked on the Scale Trainer with a known scale and a run going.
    ///
    /// `navigate_to` rather than `open`, so nothing is rerolled and no device is claimed:
    /// every assertion below is about what the drill does with an answer, and a run started
    /// in pressing mode listens to nothing.
    fn drill_app(root: u8, kind: ScaleKind) -> App {
        let mut app = app_with_seed(0);
        app.navigate_to(Screen::WalkTheScale);
        app.walk_the_scale.enter();
        app.walk_the_scale.select_root(PitchClass::new(root));
        app.walk_the_scale.select_kind(kind);
        app
    }

    /// The degree being asked for, read the way the view reads it.
    fn degree(app: &App) -> Interval {
        let (at, _) = app
            .walk_the_scale
            .progress()
            .expect("a run is asking for a degree");
        app.walk_the_scale.scale().kind().intervals()[at]
    }

    fn target(app: &App) -> PitchClass {
        app.walk_the_scale
            .scale()
            .root()
            .transpose(degree(app).semitones())
    }

    /// The lowest position carrying a pitch class. Which one is arbitrary, and that the
    /// drill is indifferent to the choice is asserted by `a_degree_is_accepted_anywhere`.
    fn position_of(pitch_class: PitchClass) -> (usize, usize) {
        (0..NECK_STRINGS)
            .flat_map(|string| (0..=DRILL_NECK.frets()).map(move |fret| (string, fret)))
            .find(|&(string, fret)| DRILL_NECK.pitch_class_at(string, fret) == Some(pitch_class))
            .expect("the neck carries every pitch class")
    }

    fn presses(app: &mut App, pitch_class: PitchClass) {
        let (string, fret) = position_of(pitch_class);
        let _ = app.update(Message::ChooseScalePosition(string, fret));
    }

    /// Answers the element on screen correctly.
    fn answers(app: &mut App) {
        presses(app, target(app));
    }

    /// Answers it with the semitone above — near enough to be a plausible slip, and never
    /// a degree of the scale for the kinds these tests use.
    fn answers_wrong(app: &mut App) {
        presses(app, target(app).transpose(1));
    }

    /// Walks a whole sequence correctly, leaving the completion flashing.
    fn completes(app: &mut App) {
        for _ in 0..app.walk_the_scale.scale().kind().intervals().len() {
            answers(app);
        }
    }

    fn plays(app: &mut App, pitch: Pitch) {
        let _ = app.update(Message::Heard(Heard::Pitch(Detection::synthesized(
            pitch.frequency(),
            0.99,
        ))));
    }

    fn stops(app: &mut App) {
        let _ = app.update(Message::Heard(Heard::Silence));
    }

    /// Some pitch sounding `pitch_class`, low on the neck.
    fn sounding(pitch_class: PitchClass) -> Pitch {
        (40u8..=79)
            .filter_map(Pitch::from_midi)
            .find(|pitch| pitch.pitch_class() == pitch_class)
            .expect("the neck sounds every pitch class")
    }

    /// The kind pills are laid out in the rows `kind_row_spans` gives, and the widest of them
    /// — `Spanish Gypsy`, `Whole Tone` and `Diminished` — has to fit inside the card or it is
    /// drawn past its edge. The Scale Trainer never hits this because its kind card is half
    /// the window wide; a fixed column does.
    ///
    /// The glyph width is an estimate, deliberately generous: iced measures text at layout
    /// time and there is no font here to ask. What the test is for is the *ordering* — a kind
    /// with a longer name, or a narrower column, should fail this before it reaches a window.
    #[test]
    fn the_widest_kind_row_fits_the_selector_column() {
        // A conservative average advance for the body font at `KIND_BUTTON_SIZE`.
        const GLYPH_WIDTH: f32 = 0.58;
        const BUTTON_PADDING_X: f32 = 12.0;
        const FOCUS_RING_PADDING: f32 = 3.0;
        const ROW_SPACING: f32 = 8.0;
        const CARD_PADDING: f32 = 24.0;

        let widest = kind_row_spans()
            .map(|(start, len)| {
                let kinds = &ScaleKind::ALL[start..start + len];
                let text: f32 = kinds
                    .iter()
                    .map(|kind| kind.name().chars().count() as f32 * KIND_BUTTON_SIZE * GLYPH_WIDTH)
                    .sum();
                let pills = len as f32 * 2.0 * (BUTTON_PADDING_X + FOCUS_RING_PADDING);
                text + pills + (len as f32 - 1.0) * ROW_SPACING
            })
            .fold(0.0_f32, f32::max);

        let available = SELECTOR_COLUMN_WIDTH - 2.0 * CARD_PADDING;

        assert!(
            available >= widest,
            "the selector column leaves {available} for a kind row that needs {widest}; the \
             longest row would be drawn past the card's edge"
        );
    }

    // ---- The sequence ----

    #[test]
    fn the_sequence_is_the_kinds_formula_in_order() {
        for &kind in ScaleKind::ALL {
            let mut app = drill_app(0, kind);
            let mut asked = Vec::new();

            for _ in 0..kind.intervals().len() {
                asked.push(degree(&app));
                answers(&mut app);
            }

            assert_eq!(asked, kind.intervals(), "{kind:?}");
        }
    }

    #[test]
    fn the_root_is_asked_for_first() {
        for &kind in ScaleKind::ALL {
            let app = drill_app(7, kind);
            assert_eq!(degree(&app), Interval::Unison, "{kind:?}");
        }
    }

    #[test]
    fn every_degree_of_every_kind_falls_somewhere_on_the_neck() {
        let reachable: Vec<_> = (0..NECK_STRINGS)
            .flat_map(|string| (0..=DRILL_NECK.frets()).map(move |fret| (string, fret)))
            .filter_map(|(string, fret)| DRILL_NECK.pitch_class_at(string, fret))
            .collect();

        for root in PitchClass::ALL {
            for &kind in ScaleKind::ALL {
                for degree in kind.intervals() {
                    let wanted = root.transpose(degree.semitones());
                    assert!(
                        reachable.contains(&wanted),
                        "{root:?} {kind:?} {degree} is nowhere on the neck"
                    );
                }
            }
        }
    }

    #[test]
    fn changing_the_kind_restarts_the_sequence() {
        // Diminished has eight degrees and Minor Pentatonic five, so an index carried over
        // would be pointing past the end of the second — the case `Run::degree`'s `get`
        // answers with `None` rather than a panic, and which this asserts never arises.
        let mut app = drill_app(0, ScaleKind::Diminished);
        for _ in 0..6 {
            answers(&mut app);
        }

        let _ = app.update(Message::SelectDrillKind(ScaleKind::MinorPentatonic));

        assert_eq!(app.walk_the_scale.progress(), Some((0, 5)));
        assert_eq!(degree(&app), Interval::Unison);
    }

    #[test]
    fn changing_the_root_restarts_the_sequence() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        answers(&mut app);
        assert_eq!(app.walk_the_scale.progress(), Some((1, 7)));

        let _ = app.update(Message::SelectDrillRoot(PitchClass::new(5)));

        assert_eq!(app.walk_the_scale.progress(), Some((0, 7)));
    }

    // ---- Entering and leaving ----

    #[test]
    fn the_screen_reopens_on_the_scale_it_was_left_on() {
        // The CAGED window's rule rather than the Scale Trainer's: a learner comes back to
        // what they were working on, and a drill that rerolled on entry would never let them.
        let mut app = drill_app(3, ScaleKind::Aeolian);
        answers(&mut app);

        app.go_back();
        app.open(Screen::WalkTheScale);

        assert_eq!(app.walk_the_scale.scale().root(), PitchClass::new(3));
        assert_eq!(app.walk_the_scale.scale().kind(), ScaleKind::Aeolian);
        // The run is fresh, though.
        assert_eq!(app.walk_the_scale.progress(), Some((0, 7)));
    }

    // ---- Advancing ----

    #[test]
    fn a_wrong_answer_does_not_advance() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let asked = degree(&app);

        answers_wrong(&mut app);

        assert_eq!(degree(&app), asked);
        assert_eq!(app.walk_the_scale.progress(), Some((0, 7)));
    }

    #[test]
    fn earlier_wrong_answers_stay_marked_and_a_new_element_starts_clean() {
        let mut app = drill_app(0, ScaleKind::Ionian);

        // Bound before the call rather than written inline: `presses(&mut app, target(&app))`
        // takes the mutable borrow for the whole call, so the second argument's `&app` is a
        // second borrow of something already borrowed mutably — E0502.
        let first = target(&app).transpose(1);
        presses(&mut app, first);
        let second = target(&app).transpose(6);
        presses(&mut app, second);
        let wrong: Vec<_> = drill_markers(&app.walk_the_scale)
            .into_iter()
            .filter(|marker| marker.color == DANGER)
            .map(|marker| DRILL_NECK.pitch_class_at(marker.string, marker.fret))
            .collect();
        assert_eq!(
            wrong.iter().collect::<std::collections::HashSet<_>>().len(),
            2
        );

        answers(&mut app);

        assert!(
            drill_markers(&app.walk_the_scale)
                .iter()
                .all(|marker| marker.color != DANGER)
        );
    }

    #[test]
    fn an_element_advances_without_a_flash() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        answers(&mut app);

        assert!(!app.walk_the_scale.is_flashing());
        assert_eq!(app.walk_the_scale.progress(), Some((1, 7)));
    }

    #[test]
    fn completing_the_sequence_flashes_and_the_tick_begins_the_next() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        completes(&mut app);

        assert!(app.walk_the_scale.is_flashing());
        assert_eq!(app.walk_the_scale.progress(), Some((7, 7)));

        let _ = app.update(Message::AdvanceScaleSequence);

        assert!(!app.walk_the_scale.is_flashing());
        assert_eq!(app.walk_the_scale.progress(), Some((0, 7)));
    }

    #[test]
    fn answers_during_the_flash_are_ignored() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        completes(&mut app);
        let streak = app.walk_the_scale.streak();

        let anything = app.walk_the_scale.scale().root().transpose(1);
        presses(&mut app, anything);

        assert_eq!(app.walk_the_scale.streak(), streak);
        assert!(app.walk_the_scale.is_flashing());
    }

    // ---- The streak ----

    #[test]
    fn the_streak_counts_clean_sequences() {
        let mut app = drill_app(0, ScaleKind::MinorPentatonic);

        for expected in 1..=3 {
            completes(&mut app);
            assert_eq!(app.walk_the_scale.streak(), expected);
            let _ = app.update(Message::AdvanceScaleSequence);
        }
    }

    #[test]
    fn a_wrong_answer_resets_the_streak_and_the_best_survives_it() {
        let mut app = drill_app(0, ScaleKind::MinorPentatonic);
        completes(&mut app);
        let _ = app.update(Message::AdvanceScaleSequence);
        completes(&mut app);
        let _ = app.update(Message::AdvanceScaleSequence);
        assert_eq!(app.walk_the_scale.best_streak(), 2);

        answers_wrong(&mut app);

        assert_eq!(app.walk_the_scale.streak(), 0);
        assert_eq!(app.walk_the_scale.best_streak(), 2);
    }

    #[test]
    fn a_sequence_with_a_wrong_answer_in_it_does_not_raise_the_streak() {
        let mut app = drill_app(0, ScaleKind::MinorPentatonic);
        answers_wrong(&mut app);
        completes(&mut app);

        assert_eq!(app.walk_the_scale.streak(), 0);
    }

    // ---- What the neck says ----

    #[test]
    fn the_drill_does_not_light_the_scale() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        assert!(drill_markers(&app.walk_the_scale).is_empty());

        answers(&mut app);
        let marked = drill_markers(&app.walk_the_scale);

        // Only the one degree that has been answered, and it is marked at every position
        // carrying it — which is the honest drawing of a rule that judges pitch classes.
        assert!(!marked.is_empty());
        assert!(marked.iter().all(
            |marker| DRILL_NECK.pitch_class_at(marker.string, marker.fret)
                == Some(app.walk_the_scale.scale().root())
        ));
    }

    #[test]
    fn nothing_the_drill_marks_names_a_note() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        answers(&mut app);
        answers_wrong(&mut app);

        for marker in drill_markers(&app.walk_the_scale) {
            assert!(
                marker.label.is_empty()
                    || marker
                        .label
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_digit() || c == 'b' || c == '#'),
                "a marker named a note: {}",
                marker.label
            );
        }
    }

    // ---- Pressing ----

    #[test]
    fn a_degree_is_accepted_anywhere_it_falls() {
        // Every position carrying the asked-for pitch class answers, including ones lower on
        // the neck than the element answered before — a sequence need not ascend.
        for string in 0..NECK_STRINGS {
            for fret in 0..=DRILL_NECK.frets() {
                let mut app = drill_app(0, ScaleKind::Ionian);
                answers(&mut app);
                let wanted = target(&app);

                if DRILL_NECK.pitch_class_at(string, fret) != Some(wanted) {
                    continue;
                }

                let _ = app.update(Message::ChooseScalePosition(string, fret));
                assert_eq!(
                    app.walk_the_scale.progress(),
                    Some((2, 7)),
                    "string {string} fret {fret}"
                );
            }
        }
    }

    #[test]
    fn a_press_in_playing_mode_judges_nothing() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::ToggleScaleAnswering);

        answers(&mut app);

        assert_eq!(app.walk_the_scale.progress(), Some((0, 7)));
    }

    // ---- Playing, and the gate ----

    #[test]
    fn playing_answers_in_playing_mode() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::ToggleScaleAnswering);

        let note = sounding(target(&app));
        plays(&mut app, note);

        assert_eq!(app.walk_the_scale.progress(), Some((1, 7)));
    }

    #[test]
    fn two_consecutive_degrees_are_both_judged() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::ToggleScaleAnswering);

        let note = sounding(target(&app));
        plays(&mut app, note);
        stops(&mut app);
        let note = sounding(target(&app));
        plays(&mut app, note);

        assert_eq!(app.walk_the_scale.progress(), Some((2, 7)));
    }

    #[test]
    fn a_slide_between_degrees_is_judged() {
        // The detector breaks its run on a reading further than its tolerance from the one
        // before, so a slide releases something other than a pitch on the way — which is why
        // the drill needs no notion of a slide of its own.
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::ToggleScaleAnswering);

        let note = sounding(target(&app));
        plays(&mut app, note);
        let _ = app.update(Message::Heard(Heard::Unpitched));
        let note = sounding(target(&app));
        plays(&mut app, note);

        assert_eq!(app.walk_the_scale.progress(), Some((2, 7)));
    }

    #[test]
    fn the_completing_note_does_not_answer_the_next_sequence() {
        let mut app = drill_app(0, ScaleKind::MinorPentatonic);
        let _ = app.update(Message::ToggleScaleAnswering);

        for _ in 0..5 {
            let note = sounding(target(&app));
            plays(&mut app, note);
            stops(&mut app);
        }
        assert!(app.walk_the_scale.is_flashing());

        // The note that completed the sequence is still ringing as the flash retires it.
        let ringing = sounding(
            app.walk_the_scale
                .scale()
                .root()
                .transpose(app.walk_the_scale.scale().kind().intervals()[4].semitones()),
        );
        plays(&mut app, ringing);
        let _ = app.update(Message::AdvanceScaleSequence);
        plays(&mut app, ringing);

        assert_eq!(app.walk_the_scale.progress(), Some((0, 5)));
    }

    #[test]
    fn a_held_wrong_note_costs_the_streak_once() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::ToggleScaleAnswering);
        let wrong = sounding(target(&app).transpose(1));

        for _ in 0..20 {
            plays(&mut app, wrong);
        }

        let marked = drill_markers(&app.walk_the_scale)
            .into_iter()
            .filter(|marker| marker.color == DANGER)
            .filter_map(|marker| DRILL_NECK.pitch_class_at(marker.string, marker.fret))
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(marked.len(), 1);
    }

    #[test]
    fn a_strum_and_a_fumble_are_not_answers() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::ToggleScaleAnswering);

        let _ = app.update(Message::Heard(Heard::Unpitched));
        let _ = app.update(Message::Heard(Heard::Silence));

        assert_eq!(app.walk_the_scale.progress(), Some((0, 7)));
        assert!(drill_markers(&app.walk_the_scale).is_empty());
    }

    // ---- The device ----

    #[test]
    fn pressing_mode_holds_no_device_and_playing_puts_it_down_on_the_way_out() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        assert!(app.walk_the_scale.listener().is_none());

        let _ = app.update(Message::ToggleScaleAnswering);
        assert!(app.walk_the_scale.listener().is_some());

        let _ = app.update(Message::ToggleScaleAnswering);
        assert!(app.walk_the_scale.listener().is_none());
    }

    #[test]
    fn leaving_the_screen_releases_the_device() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::ToggleScaleAnswering);
        assert!(app.walk_the_scale.listener().is_some());

        app.go_back();

        assert!(app.walk_the_scale.listener().is_none());
    }

    #[test]
    fn the_drill_is_still_answerable_without_a_device() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::ToggleScaleAnswering);
        let _ = app.update(Message::ToggleScaleAnswering);

        answers(&mut app);

        assert_eq!(app.walk_the_scale.progress(), Some((1, 7)));
    }

    #[test]
    fn the_four_states_read_differently_and_name_no_note() {
        use std::collections::HashSet;

        // Every state the line can report, gathered from the words rather than the widget.
        let lines: HashSet<&str> = [
            State::NotStarted,
            State::NoInput,
            State::Silent,
            State::Unnameable,
            State::Reading,
        ]
        .iter()
        .map(|state| match state {
            State::NotStarted => "not listening",
            State::NoInput => "no input — press m to answer with the neck instead",
            State::Silent => "listening, play the degree...",
            State::Unnameable => "one string at a time",
            State::Reading => "heard",
        })
        .collect();

        assert_eq!(lines.len(), 5, "two states read the same");

        for line in lines {
            assert!(
                !line
                    .split_whitespace()
                    .any(|word| { matches!(word, "A" | "B" | "C" | "D" | "E" | "F" | "G") }),
                "the status line named a note: {line}"
            );
        }
    }

    #[test]
    fn a_missing_device_is_stated_rather_than_waited_on() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::ToggleScaleAnswering);

        let listener = app
            .walk_the_scale
            .listener()
            .expect("playing mode holds a listener");

        // Whether this machine has an input decides which of the two it is; neither of them
        // is the drill silently appearing to listen to something that is not there.
        assert!(matches!(
            listening_line(listener),
            "no input — press m to answer with the neck instead"
                | "listening, play the degree..."
                | "not listening"
        ));
    }

    // ---- The workout options ----

    #[test]
    fn the_neck_can_be_hidden_only_while_playing() {
        let mut app = drill_app(0, ScaleKind::Ionian);

        let _ = app.update(Message::ToggleScaleNeck);
        assert!(
            !app.walk_the_scale.neck_hidden(),
            "pressing mode hid the neck"
        );

        let _ = app.update(Message::ToggleScaleAnswering);
        let _ = app.update(Message::ToggleScaleNeck);
        assert!(app.walk_the_scale.neck_hidden());

        // Swapping back needs no code of its own: the flag is dropped with the variant.
        let _ = app.update(Message::ToggleScaleAnswering);
        assert!(!app.walk_the_scale.neck_hidden());
    }

    #[test]
    fn the_remaining_time_is_derived_rather_than_counted_down() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::CycleScaleDuration);
        assert_eq!(
            app.walk_the_scale.duration(),
            Some(Duration::from_secs(180))
        );

        // One jump rather than a hundred and twenty ticks. A countdown would still read
        // 3:00 here; `started.elapsed()` cannot.
        app.walk_the_scale.rewind(Duration::from_secs(120));

        let left = app
            .walk_the_scale
            .remaining()
            .expect("a duration is in force");
        assert!(left <= Duration::from_secs(60) && left > Duration::from_secs(59));
    }

    #[test]
    fn an_elapsed_duration_ends_the_run_and_states_what_it_completed() {
        let mut app = drill_app(0, ScaleKind::MinorPentatonic);
        let _ = app.update(Message::CycleScaleDuration);
        completes(&mut app);
        let _ = app.update(Message::AdvanceScaleSequence);

        app.walk_the_scale.rewind(Duration::from_secs(180));
        let _ = app.update(Message::WalkTheScaleTick);

        assert!(!app.walk_the_scale.is_running());
        assert_eq!(app.walk_the_scale.summary(), Some((1, 1)));
    }

    #[test]
    fn another_run_can_be_started_without_leaving() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::CycleScaleDuration);
        app.walk_the_scale.rewind(Duration::from_secs(180));
        let _ = app.update(Message::WalkTheScaleTick);
        assert!(!app.walk_the_scale.is_running());

        let _ = app.update(Message::StartWalkTheScale);

        assert!(app.walk_the_scale.is_running());
        assert_eq!(app.walk_the_scale.progress(), Some((0, 7)));
    }

    #[test]
    fn leaving_the_screen_ends_a_running_duration() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::CycleScaleDuration);

        app.go_back();

        assert!(app.walk_the_scale.remaining().is_none());
    }

    // ---- The keyboard ----

    #[test]
    fn the_drill_keys_act_without_moving_focus() {
        let mut app = app_with_seed(0);
        app.open(Screen::WalkTheScale);
        let focused = app.focused;
        assert!(app.walk_the_scale.is_running(), "the screen opens asking");

        press_into(&mut app, "m", keyboard::Modifiers::empty());
        assert!(matches!(
            app.walk_the_scale.answering(),
            Answering::Playing { .. }
        ));

        press_into(&mut app, "n", keyboard::Modifiers::empty());
        assert!(app.walk_the_scale.neck_hidden());

        press_into(&mut app, "t", keyboard::Modifiers::empty());
        assert_eq!(
            app.walk_the_scale.duration(),
            Some(Duration::from_secs(180))
        );

        let before = app.walk_the_scale.scale();
        press_into(&mut app, "r", keyboard::Modifiers::empty());
        assert_ne!(app.walk_the_scale.scale(), before, "r replaced nothing");

        assert_eq!(app.focused, focused, "an accelerator moved the focus ring");
    }

    #[test]
    fn the_overlay_reports_what_the_drill_is_set_to() {
        let mut app = app_with_seed(0);
        app.navigate_to(Screen::WalkTheScale);
        press_into(&mut app, "p", keyboard::Modifiers::empty());
        press_into(&mut app, "m", keyboard::Modifiers::empty());
        press_into(&mut app, "t", keyboard::Modifiers::empty());
        press_into(&mut app, "n", keyboard::Modifiers::empty());

        let labels: Vec<String> = app
            .accelerators()
            .into_iter()
            .map(|(key, _, label)| format!("{key} {label}"))
            .collect();

        assert!(
            labels.iter().any(|line| line == "t timer: 3:00"),
            "{labels:?}"
        );
        assert!(
            labels.iter().any(|line| line == "n neck: hidden"),
            "{labels:?}"
        );
        assert!(
            labels.iter().any(|line| line == "m answer: play"),
            "{labels:?}"
        );
    }

    #[test]
    fn the_drills_keys_are_inert_once_a_run_has_ended() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::CycleScaleDuration);
        app.walk_the_scale.rewind(Duration::from_secs(180));
        let _ = app.update(Message::WalkTheScaleTick);
        assert!(!app.walk_the_scale.is_running());

        press_into(&mut app, "m", keyboard::Modifiers::empty());
        press_into(&mut app, "n", keyboard::Modifiers::empty());

        // The summary is not a drill, so the settings it would report have nothing to
        // report about. `p` is the one key the screen still claims: another run.
        assert_eq!(app.walk_the_scale.answering(), Answering::Pressing);
        assert!(!app.walk_the_scale.neck_hidden());

        press_into(&mut app, "p", keyboard::Modifiers::empty());
        assert!(app.walk_the_scale.is_running());
    }

    #[test]
    fn esc_leaves_the_drill_before_it_leaves_the_screen() {
        let mut app = app_with_seed(0);
        app.open(Screen::WalkTheScale);
        let _ = app.update(Message::StartWalkTheScale);

        let _ = app.update(Message::GoBack);
        assert!(!app.walk_the_scale.is_running());
        assert_eq!(app.screen, Screen::WalkTheScale);

        let _ = app.update(Message::GoBack);
        assert_eq!(app.screen, Screen::Home);
    }

    #[test]
    fn the_drill_controls_are_reached_in_order() {
        let mut app = app_with_seed(0);
        app.navigate_to(Screen::WalkTheScale);
        let _ = app.update(Message::StartWalkTheScale);
        let _ = app.update(Message::ToggleScaleAnswering);

        let targets: Vec<_> = app
            .focusables()
            .into_iter()
            .filter(|target| {
                matches!(
                    target,
                    FocusTarget::RerollDrillScale
                        | FocusTarget::ScaleAnswering
                        | FocusTarget::ScaleDuration
                        | FocusTarget::ScaleNeck
                )
            })
            .collect();

        assert_eq!(
            targets,
            vec![
                FocusTarget::RerollDrillScale,
                FocusTarget::ScaleAnswering,
                FocusTarget::ScaleDuration,
                FocusTarget::ScaleNeck,
            ]
        );
    }
}
