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

use iced::mouse::ScrollDelta;
use iced::widget::canvas::{self, Canvas};
use iced::{Color, Element, Length, Padding, Point, Rectangle, Renderer, Size, Theme};

use super::focus::{Direction, FocusTarget, kind_row_spans};
use super::fretboard::{Fretboard, MarkerStyle, NoteMarker, fretboard};
use super::listener::{Listener, State, ui_device_line, ui_device_picker};
use super::neck::{CURSOR_HOME, DRILL_NECK, NECK_STRINGS, Position};
use super::style::{
    BODY, CONTROL_SIZE, DANGER, INK, LINK, MUSIC_FONT, MUTE, SUCCESS, card_container,
    control_button, control_label, control_shuffle,
};
use super::{Message, note_label, scale_kind_row};
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
    cursor: Position,
}

/// What the screen is doing.
///
/// Two states rather than three: the drill is the whole screen now, so there is no display to
/// fall back to — entering it starts a run. What a finished run completed is a state of its
/// own rather than the absence of one, which is why this is not an `Option<Run>`.
enum Mode {
    Running(Run),
    Ended {
        completed: u32,
        best_streak: u32,
        /// What the run was set to, so "again" means the same run again.
        duration: Option<Duration>,
    },
}

/// The scale drill.
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
    /// Where a drag of the root display was last measured from, in window coordinates.
    ///
    /// `Option<f32>` rather than a `bool` beside an `f32`: the anchor exists exactly when a
    /// drag does, so "dragging, from nowhere" is a state that cannot be written down.
    drag: Option<f32>,
    /// What the wheel has scrolled that has not yet bought a semitone.
    ///
    /// Kept between gestures rather than reset: a nudge one way and a nudge back should leave
    /// the root where it was, and resetting would make that depend on where one gesture ended
    /// and the next began.
    scroll: f32,
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
            // Playing rather than pressing: the drill is about finding a degree on the
            // instrument, and the neck is the surface you fall back to. The device is not
            // claimed here — `enter` is what opens it, so an app that never reaches this
            // screen never listens.
            answering: Answering::Playing { neck_hidden: false },
            best_streak: 0,
            drag: None,
            scroll: 0.0,
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

    /// Steps the root `steps` semitones, wrapping in both directions.
    pub(super) fn step_root(&mut self, steps: i32) {
        if steps == 0 {
            return;
        }

        self.select_root(self.scale.root().shift(steps));
    }

    pub(super) fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    pub(super) fn grab_root(&mut self, x: f32) {
        self.drag = Some(x);
    }

    /// A drag that has reached `x`. Whole steps of travel are consumed off the anchor rather
    /// than measured from the press point, so the drill never has to remember the root it
    /// started on — and the pointer has to return to where it was to undo a step.
    pub(super) fn drag_root(&mut self, x: f32) {
        let Some(anchor) = self.drag else {
            return;
        };

        let steps = ((x - anchor) / STEP_DISTANCE).trunc();

        if steps != 0.0 {
            self.drag = Some(anchor + steps * STEP_DISTANCE);
            self.step_root(steps as i32);
        }
    }

    pub(super) fn drop_root(&mut self) {
        self.drag = None;
    }

    /// The wheel, in steps. Ignored while a drag is going: one gesture owns the root at a
    /// time, which is this `if` rather than a rule about how two accumulators compose.
    pub(super) fn scroll_root(&mut self, steps: f32) {
        if self.drag.is_some() {
            return;
        }

        self.scroll += steps;
        let whole = self.scroll.trunc();

        if whole != 0.0 {
            self.scroll -= whole;
            self.step_root(whole as i32);
        }
    }

    pub(super) fn select_kind(&mut self, kind: ScaleKind) {
        self.scale = Scale::new(self.scale.root(), kind);
        self.restart_sequence();
    }

    /// `r`: replace what is on screen, which here is the scale being drilled.
    pub(super) fn reroll(&mut self, rng: &mut Rng) {
        let current = (self.scale.root(), self.scale.kind());
        let (root, kind) = rng.draw_other_than(current, |rng| {
            (
                PitchClass::ALL[rng.below(PitchClass::ALL.len())],
                ScaleKind::ALL[rng.below(ScaleKind::ALL.len())],
            )
        });
        self.scale = Scale::new(root, kind);
        self.restart_sequence();
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
        self.begin(self.duration());
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
                ..
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
            duration: self.duration(),
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
        match &self.mode {
            Mode::Running(run) => run.duration,
            Mode::Ended { duration, .. } => *duration,
        }
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
    ///
    /// Not `drill::Score`: a streak here counts clean sequences rather than answers, and the
    /// best outlives each run.
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
            Mode::Running(run) if run.is_over() => Some((run.completed, run.duration)),
            _ => None,
        };

        if let Some((completed, duration)) = ended {
            self.mode = Mode::Ended {
                completed,
                best_streak: self.best_streak,
                duration,
            };
            self.release();
        }
    }

    pub(super) fn cursor(&self) -> Option<Position> {
        self.run().map(|run| run.cursor)
    }

    /// The neck claims the motion keys while it is focused, as the two trainers' do.
    pub(super) fn move_cursor(&mut self, direction: Direction) {
        let Mode::Running(run) = &mut self.mode else {
            return;
        };
        run.cursor = run.cursor.step(direction);
    }

    /// Enter on the neck answers with wherever the cursor is sitting — the same thing a
    /// click on that position would send.
    pub(super) fn answer_at_cursor(&mut self) {
        let Some(Position { string, fret }) = self.cursor() else {
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
    use iced::widget::{Space, column, container, row, stack, text};

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
    // The root is a canvas with the glyph stacked over it. The canvas is the surface the drag
    // and the wheel land on, and draws the arrows that say so; the glyph stays a widget
    // because a `Frame` cannot measure text, and an accidental has to be placed beside a
    // letter it has measured. Both halves centre themselves, so there is no padding to keep
    // in step — which is the whole reason the row is a fixed height.
    let root = stack![
        Canvas::new(RootGrip {
            dragging: drill.is_dragging(),
        })
        .width(Length::Fill)
        .height(Length::Fill),
        container(note_label(scale.root_note(), ROOT_DISPLAY_SIZE, INK))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    ];

    let centre = column![
        // Two above to three below rather than evenly: the pair sits a tenth of the column's
        // free height above its middle, where the eye is already reading the neck. The
        // sequence is the row under the root rather than the bottom of the column, so it
        // moves with it.
        Space::new().height(Length::FillPortion(2)),
        container(root)
            .width(Length::Fill)
            .height(Length::Fixed(ROOT_ROW_HEIGHT)),
        container(sequence_text(drill))
            .width(Length::Fill)
            .center_x(Length::Fill),
        Space::new().height(Length::FillPortion(3)),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

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

    // The kind selector and the run's controls share the right column: the selector says what
    // is being drilled and the controls say how, and neither belongs beside the neck. The root
    // is not here — it is the glyph in the middle column, which is the one thing on the screen
    // that both states the scale and sets it.
    let selector = column![
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

/// How far the pointer travels, or the wheel scrolls, for one semitone.
///
/// Wide enough that a hand that slips on the glyph does not walk the scale, narrow enough that
/// all twelve are one gesture — 480px from a root back to itself. Nothing in the geometry below
/// reads it, so it is one number to change if it turns out to be wrong.
const STEP_DISTANCE: f32 = 40.0;

/// How tall the row holding the root is. Fixed, because the canvas under the glyph has to be
/// the same height whatever the window is: a canvas that filled would put the arrows wherever
/// the column's slack happened to leave them.
const ROOT_ROW_HEIGHT: f32 = ROOT_DISPLAY_SIZE * 1.3;

/// Where the root's glyph sits, and what counts as a press on it.
///
/// The one `Layout` this canvas has, and the one thing it knows: three gestures read the same
/// box — the press that begins a drag, the wheel, and the pointer shape — so the box is a
/// struct rather than a condition written out three times.
struct RootDisplay {
    center: Point,
    half_width: f32,
    half_height: f32,
}

impl RootDisplay {
    /// Half the width of the widest glyph drawn here: a letter and an accidental at
    /// `ROOT_DISPLAY_SIZE`. Deliberately not measured per note — a box that changed size with
    /// the note in it would move under the hand mid-drag.
    const GLYPH_HALF_WIDTH: f32 = 80.0;

    fn new(size: Size) -> RootDisplay {
        RootDisplay {
            center: Point::new(size.width / 2.0, size.height / 2.0),
            // Never wider than the canvas: a box reaching past the edge would claim presses
            // on whatever is drawn beside it.
            half_width: (size.width / 2.0).min(Self::GLYPH_HALF_WIDTH),
            half_height: size.height.min(ROOT_DISPLAY_SIZE) / 2.0,
        }
    }

    fn grab_box(&self) -> Rectangle {
        Rectangle {
            x: self.center.x - self.half_width,
            y: self.center.y - self.half_height,
            width: 2.0 * self.half_width,
            height: 2.0 * self.half_height,
        }
    }

    /// Whether a point on the canvas is on the glyph — the one question all three gestures ask.
    fn grabs(&self, point: Point) -> bool {
        self.grab_box().contains(point)
    }
}

/// A scroll, in steps.
///
/// `Lines` comes from a notched wheel and `Pixels` from a trackpad or a high-resolution one,
/// and which arrives is the machine's choice rather than ours. A notch is one semitone,
/// because that is what a notch means; a pixel is worth what a pixel of drag is worth, which
/// is why the wheel needs no constant of its own and why a trackpad does not race through the
/// twelve. Only the vertical axis is read — a notch is up or down, and counting a diagonal
/// flick's sideways component too would step twice for one gesture.
fn steps_from(delta: ScrollDelta) -> f32 {
    match delta {
        ScrollDelta::Lines { y, .. } => y,
        ScrollDelta::Pixels { y, .. } => y / STEP_DISTANCE,
    }
}

/// The root display as a canvas: the surface all three gestures land on, and nothing else.
///
/// It draws nothing. The glyph over it is a widget, because a `Frame` cannot measure text and
/// the letter and its accidental are set in two different fonts — the same reason the Tuner's
/// note name sits over its dial rather than in it. A canvas rather than a `mouse_area` because
/// `update` is handed every event wherever the pointer is, which is what lets a drag go on
/// past the edge of the glyph it began on.
struct RootGrip {
    /// Whether a drag is already going, which is what decides that a cursor move anywhere in
    /// the window is this widget's business. Read off the drill rather than kept here: the
    /// view is rebuilt from state on every update, so it cannot go stale, and a `State` on the
    /// widget would put the gesture somewhere `App::update` cannot see.
    dragging: bool,
}

impl canvas::Program<Message> for RootGrip {
    type State = ();

    /// Nothing. What the learner sees here is the glyph stacked over this canvas, and the
    /// pointer turning to a grab is what says the glyph can be taken hold of.
    fn draw(
        &self,
        _state: &(),
        _renderer: &Renderer,
        _theme: &Theme,
        _bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        Vec::new()
    }

    fn update(
        &self,
        _state: &mut (),
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let display = RootDisplay::new(bounds.size());
        let over = cursor
            .position_in(bounds)
            .is_some_and(|point| display.grabs(point));

        match event {
            // The press is the one moment the pointer has to be on the glyph. What it carries
            // is the window's x rather than the canvas's: `position_in` is `None` the moment
            // the pointer leaves, which is exactly when the drag still has work to do.
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left))
                if over =>
            {
                let x = cursor.position()?.x;
                Some(canvas::Action::publish(Message::GrabDrillRoot(x)).and_capture())
            }
            // `position` here is already the window's, which is the frame the anchor is in.
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) if self.dragging => {
                Some(canvas::Action::publish(Message::DragDrillRoot(position.x)).and_capture())
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
                if self.dragging =>
            {
                Some(canvas::Action::publish(Message::DropDrillRoot).and_capture())
            }
            // Captured whether or not it stepped, so a wheel over the glyph never falls
            // through. Nothing on this screen scrolls today, and this is what keeps that true.
            iced::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) if over => Some(
                canvas::Action::publish(Message::ScrollDrillRoot(steps_from(*delta))).and_capture(),
            ),
            _ => None,
        }
    }

    fn mouse_interaction(
        &self,
        _state: &(),
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> iced::mouse::Interaction {
        if self.dragging {
            // Held for the length of the gesture, wherever the pointer has got to — the same
            // reason `update` stops asking where it is once a drag has begun.
            iced::mouse::Interaction::Grabbing
        } else if cursor
            .position_in(bounds)
            .is_some_and(|point| RootDisplay::new(bounds.size()).grabs(point))
        {
            iced::mouse::Interaction::Grab
        } else {
            iced::mouse::Interaction::default()
        }
    }
}

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
    use iced::widget::{column, row, text};

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
            control_label(
                text(super::keys::duration_label(drill.duration())),
                CONTROL_SIZE,
            ),
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
        // Shrink-wide rather than pushing the time to the far edge, so the column can centre
        // it; the time is only pushed when there is one, so an untimed run is not off-centre
        // by a spacing's width.
        None => row![
            text(format!(
                "streak {} · best {}",
                drill.streak(),
                drill.best_streak()
            ))
            .size(CONTROL_SIZE)
            .color(BODY),
        ]
        .push(drill.remaining().map(|left| {
            text(super::keys::duration_label(Some(left)))
                .size(CONTROL_SIZE)
                .color(MUTE)
        }))
        .spacing(16)
        .into(),
    };

    // Centred under the kind card, which is the column's one other occupant.
    let mut panel = column![controls, readout]
        .spacing(12)
        .align_x(iced::Alignment::Center)
        .width(Length::Fill);

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
    // `as _` brings the trait in for method resolution without binding the name: `update` is
    // `Program`'s, and a trait's methods are only callable where the trait is in scope.
    use iced::keyboard;
    use iced::widget::canvas::Program as _;

    /// An app parked on the Scale Trainer with a known scale and a run going.
    ///
    /// `navigate_to` rather than `open`, so nothing is rerolled and no device is claimed:
    /// every assertion below is about what the drill does with an answer, and a run started
    /// in pressing mode listens to nothing.
    fn drill_app(root: u8, kind: ScaleKind) -> App {
        let mut app = app_with_seed(0);
        app.navigate_to(Screen::WalkTheScale);
        // Into pressing before the run begins, so no capture device is claimed: the screen
        // opens in playing mode, and most of what is asserted below is about a press. The
        // tests that want playing toggle back, as they always did.
        app.walk_the_scale.toggle_answering();
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

    /// Widths the root display is plausibly drawn at. The middle column takes what the neck
    /// and the selector leave, so the width is what varies with the window; the height is
    /// `ROOT_ROW_HEIGHT` and does not.
    const ROOT_SIZES: [Size; 4] = [
        Size::new(220.0, ROOT_ROW_HEIGHT),
        Size::new(278.0, ROOT_ROW_HEIGHT),
        Size::new(360.0, ROOT_ROW_HEIGHT),
        Size::new(520.0, ROOT_ROW_HEIGHT),
    ];

    /// The box every gesture is tested against: centred on the canvas, the glyph's size, and
    /// never reaching past the edge into what is drawn beside it.
    ///
    /// The canvas draws nothing, so there is no drawing for this to be held against — what it
    /// guards instead is that the one region three gestures share stays where the glyph is.
    #[test]
    fn the_grab_box_is_centred_and_stays_inside_the_canvas() {
        for size in ROOT_SIZES {
            let display = RootDisplay::new(size);
            let grab_box = display.grab_box();

            assert!(
                display.grabs(display.center),
                "{size:?} does not grab its own centre"
            );

            // A point just outside each edge belongs to nothing.
            for outside in [
                Point::new(grab_box.x - 1.0, display.center.y),
                Point::new(grab_box.x + grab_box.width + 1.0, display.center.y),
                Point::new(display.center.x, grab_box.y - 1.0),
                Point::new(display.center.x, grab_box.y + grab_box.height + 1.0),
            ] {
                assert!(!display.grabs(outside), "{size:?} grabs {outside:?}");
            }

            assert!(
                grab_box.x >= 0.0 && grab_box.x + grab_box.width <= size.width,
                "{size:?} puts the box past the canvas edge"
            );
        }
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

    // ---- The root display ----

    /// The two units one movement arrives in, reduced to the one the drill counts.
    #[test]
    fn a_notch_and_a_step_of_pixels_are_each_one_step() {
        assert_eq!(steps_from(ScrollDelta::Lines { x: 0.0, y: 1.0 }), 1.0);
        assert_eq!(
            steps_from(ScrollDelta::Pixels {
                x: 0.0,
                y: STEP_DISTANCE
            }),
            1.0,
            "a trackpad steps at a different rate from a drag"
        );
        assert_eq!(
            steps_from(ScrollDelta::Lines { x: 4.0, y: 0.0 }),
            0.0,
            "sideways scrolling stepped the root"
        );
    }

    const GRIP_BOUNDS: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 278.0,
        height: ROOT_ROW_HEIGHT,
    };

    /// Whether the canvas acts on an event — which is all a test can read, since what it
    /// publishes is sealed inside a `canvas::Action`. What the message then does to the drill
    /// is asserted on the state above, where the messages are synthesized by hand.
    fn acts_on(dragging: bool, event: &canvas::Event, at: Point) -> bool {
        RootGrip { dragging }
            .update(
                &mut (),
                event,
                GRIP_BOUNDS,
                iced::mouse::Cursor::Available(at),
            )
            .is_some()
    }

    /// The wheel is the one gesture that asks where the pointer is on every event, so the
    /// hit-test is what keeps a scroll aimed elsewhere on the screen out of the drill.
    #[test]
    fn the_wheel_is_only_the_drills_over_the_glyph() {
        let wheel = iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
            delta: ScrollDelta::Lines { x: 0.0, y: 1.0 },
        });

        let display = RootDisplay::new(GRIP_BOUNDS.size());
        let corner = Point::new(2.0, 2.0);
        assert!(!display.grabs(corner), "the corner is inside the box");

        assert!(
            !acts_on(false, &wheel, corner),
            "a scroll off the glyph reached the drill"
        );
        assert!(
            acts_on(false, &wheel, display.center),
            "a scroll on the glyph did not"
        );
    }

    /// Which events the canvas claims, and when. A press has to land on the glyph; everything
    /// after it is claimed on the strength of the drag being under way, wherever the pointer
    /// has got to — which is the asymmetry the whole gesture rests on.
    #[test]
    fn the_press_is_placed_and_the_rest_of_the_drag_is_not() {
        let display = RootDisplay::new(GRIP_BOUNDS.size());
        let corner = Point::new(2.0, 2.0);

        let press =
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left));
        let moved = iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: corner });
        let release = iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        ));

        assert!(acts_on(false, &press, display.center));
        assert!(
            !acts_on(false, &press, corner),
            "a press off the glyph began a drag"
        );

        assert!(
            !acts_on(false, &moved, display.center),
            "a cursor move with no drag going was claimed"
        );
        assert!(
            acts_on(true, &moved, corner),
            "a drag stopped at the canvas edge"
        );
        assert!(
            acts_on(true, &release, corner),
            "the drag could not be released outside the glyph"
        );
        assert!(
            !acts_on(false, &release, display.center),
            "a release with no drag going was claimed"
        );
    }

    // ---- The root, as the drill holds it ----

    fn root(app: &App) -> PitchClass {
        app.walk_the_scale.scale().root()
    }

    /// Steps of drag travel, as the x a `DragDrillRoot` carries. The press is at zero, so the
    /// arithmetic under test is the only thing deciding where a step falls.
    fn drags(app: &mut App, steps: f32) {
        let _ = app.update(Message::DragDrillRoot(steps * STEP_DISTANCE));
    }

    #[test]
    fn stepping_the_root_wraps_both_ways() {
        let mut app = drill_app(11, ScaleKind::Ionian);
        app.walk_the_scale.step_root(1);
        assert_eq!(root(&app), PitchClass::new(0), "B did not wrap to C");

        app.walk_the_scale.step_root(-1);
        assert_eq!(root(&app), PitchClass::new(11), "C did not wrap to B");
    }

    #[test]
    fn a_drag_steps_a_semitone_for_each_step_of_travel() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::GrabDrillRoot(0.0));

        drags(&mut app, 1.0);
        assert_eq!(root(&app), PitchClass::new(1));

        drags(&mut app, 5.0);
        assert_eq!(
            root(&app),
            PitchClass::new(5),
            "the drag is measured from the press"
        );
    }

    #[test]
    fn a_drag_left_lowers_the_root() {
        let mut app = drill_app(5, ScaleKind::Ionian);
        let _ = app.update(Message::GrabDrillRoot(0.0));

        drags(&mut app, -2.0);
        assert_eq!(root(&app), PitchClass::new(3));
    }

    #[test]
    fn a_drag_back_to_the_press_point_returns_the_root() {
        let mut app = drill_app(9, ScaleKind::Ionian);
        let _ = app.update(Message::GrabDrillRoot(0.0));

        drags(&mut app, 3.0);
        assert_eq!(root(&app), PitchClass::new(0));

        drags(&mut app, 0.0);
        assert_eq!(
            root(&app),
            PitchClass::new(9),
            "the travel was not given back"
        );
    }

    #[test]
    fn a_press_that_goes_nowhere_changes_nothing() {
        let mut app = drill_app(9, ScaleKind::Ionian);

        let _ = app.update(Message::GrabDrillRoot(120.0));
        // Short of a step, in both directions.
        let _ = app.update(Message::DragDrillRoot(120.0 + STEP_DISTANCE * 0.9));
        let _ = app.update(Message::DragDrillRoot(120.0 - STEP_DISTANCE * 0.9));
        let _ = app.update(Message::DropDrillRoot);

        assert_eq!(root(&app), PitchClass::new(9));
    }

    #[test]
    fn a_move_with_no_drag_in_progress_changes_nothing() {
        let mut app = drill_app(9, ScaleKind::Ionian);
        drags(&mut app, 4.0);
        assert_eq!(root(&app), PitchClass::new(9));
    }

    #[test]
    fn a_drop_ends_the_gesture() {
        let mut app = drill_app(9, ScaleKind::Ionian);
        let _ = app.update(Message::GrabDrillRoot(0.0));
        let _ = app.update(Message::DropDrillRoot);

        drags(&mut app, 4.0);
        assert_eq!(
            root(&app),
            PitchClass::new(9),
            "a move after the drop still stepped"
        );
    }

    #[test]
    fn the_wheel_steps_the_root() {
        let mut app = drill_app(0, ScaleKind::Ionian);

        let _ = app.update(Message::ScrollDrillRoot(1.0));
        assert_eq!(root(&app), PitchClass::new(1));

        let _ = app.update(Message::ScrollDrillRoot(-2.0));
        assert_eq!(
            root(&app),
            PitchClass::new(11),
            "the wheel did not wrap downwards"
        );
    }

    /// The remainder is what makes a trackpad usable: its deltas arrive in pixels, far short
    /// of a step each, and a drill that discarded them would never move at all.
    #[test]
    fn two_half_steps_of_scroll_make_one_semitone() {
        let mut app = drill_app(0, ScaleKind::Ionian);

        let _ = app.update(Message::ScrollDrillRoot(0.5));
        assert_eq!(
            root(&app),
            PitchClass::new(0),
            "half a step is not a semitone"
        );

        let _ = app.update(Message::ScrollDrillRoot(0.5));
        assert_eq!(root(&app), PitchClass::new(1));
    }

    #[test]
    fn a_scroll_and_its_reverse_leave_the_root_alone() {
        let mut app = drill_app(0, ScaleKind::Ionian);

        let _ = app.update(Message::ScrollDrillRoot(0.6));
        let _ = app.update(Message::ScrollDrillRoot(-0.6));

        assert_eq!(root(&app), PitchClass::new(0));
    }

    #[test]
    fn the_wheel_is_ignored_mid_drag() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::GrabDrillRoot(0.0));

        let _ = app.update(Message::ScrollDrillRoot(3.0));
        assert_eq!(
            root(&app),
            PitchClass::new(0),
            "the wheel stepped during a drag"
        );

        drags(&mut app, 1.0);
        assert_eq!(root(&app), PitchClass::new(1), "the travel alone decides");
    }

    /// A drag passes through roots on its way. None of them is a scale that was walked, so
    /// none of them may show up as one.
    #[test]
    fn passing_through_roots_does_not_score() {
        let mut app = drill_app(0, ScaleKind::MinorPentatonic);
        let _ = app.update(Message::CycleScaleDuration);
        completes(&mut app);
        let _ = app.update(Message::AdvanceScaleSequence);

        let _ = app.update(Message::GrabDrillRoot(0.0));
        drags(&mut app, 5.0);
        let _ = app.update(Message::DropDrillRoot);

        assert_eq!(root(&app), PitchClass::new(5));
        assert_eq!(app.walk_the_scale.progress(), Some((0, 5)));

        app.walk_the_scale.rewind(Duration::from_secs(180));
        let _ = app.update(Message::WalkTheScaleTick);
        assert_eq!(app.walk_the_scale.summary(), Some((1, 1)));
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

        app.walk_the_scale.step_root(5);

        assert_eq!(app.walk_the_scale.scale().root(), PitchClass::new(5));
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
        assert_eq!(
            app.walk_the_scale.duration(),
            Some(Duration::from_secs(180)),
            "another run forgot the duration of the last"
        );
    }

    #[test]
    fn the_run_clock_ticks_only_while_a_run_is_on_screen() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::CycleScaleDuration);
        assert!(app.wants_run_clock(), "a timed run has no clock to end it");

        app.go_back();

        assert!(!app.wants_run_clock(), "the clock outlived the screen");
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

        assert!(
            matches!(app.walk_the_scale.answering(), Answering::Playing { .. }),
            "the screen opens on the instrument"
        );

        press_into(&mut app, "m", keyboard::Modifiers::empty());
        assert_eq!(app.walk_the_scale.answering(), Answering::Pressing);

        press_into(&mut app, "m", keyboard::Modifiers::empty());
        assert!(
            matches!(app.walk_the_scale.answering(), Answering::Playing { .. }),
            "not a one-way switch"
        );

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
        // No `m`: the screen already opens playing, which is what makes `n` live.
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
    fn the_bracket_keys_step_the_root() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let none = keyboard::Modifiers::empty();

        press_into(&mut app, "]", none);
        assert_eq!(root(&app), PitchClass::new(1));

        press_into(&mut app, "[", none);
        press_into(&mut app, "[", none);
        assert_eq!(
            root(&app),
            PitchClass::new(11),
            "the step down did not wrap"
        );
    }

    /// An accelerator fires a widget without walking the ring onto it, and these two have no
    /// widget to walk onto at all — so the ring has to be exactly where it was.
    #[test]
    fn the_root_keys_leave_the_focus_ring_alone() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        app.focused = FocusTarget::DrillKind(4);

        press_into(&mut app, "]", keyboard::Modifiers::empty());

        assert_eq!(root(&app), PitchClass::new(1));
        assert_eq!(app.focused, FocusTarget::DrillKind(4));
    }

    /// The ring's whole path on this screen, now that the root grid has gone: Back, the
    /// sixteen kinds in the order the rows lay them out, then the run's controls.
    ///
    /// Pinned as deliberately as the path it replaces. Removing the root card also took the
    /// screen down to one card, and a two-band split left over from the old layout would walk
    /// the first three columns of every kind row before coming back for the rest — a wrong
    /// order that nothing else here would notice.
    #[test]
    fn the_ring_walks_back_then_the_kinds_then_the_controls() {
        let mut app = app_with_seed(0);
        app.navigate_to(Screen::WalkTheScale);
        let _ = app.update(Message::StartWalkTheScale);

        let targets = app.focusables();
        let kinds = ScaleKind::ALL.len();

        assert_eq!(targets.first(), Some(&FocusTarget::Back));
        assert_eq!(
            targets[1..=kinds],
            (0..kinds).map(FocusTarget::DrillKind).collect::<Vec<_>>()[..],
            "the kinds are not walked in the order they are drawn"
        );
        assert!(
            targets[kinds + 1..]
                .iter()
                .all(|target| !matches!(target, FocusTarget::DrillKind(_) | FocusTarget::Back)),
            "the run's controls are interleaved with the kinds"
        );
    }

    /// The drill sets its root on the glyph, so nothing in its ring names one — while the two
    /// screens that still choose a root from a grid keep every one of their twelve stops.
    #[test]
    fn no_root_is_a_focus_stop_here_and_every_one_still_is_elsewhere() {
        let mut app = app_with_seed(0);
        app.navigate_to(Screen::WalkTheScale);
        let _ = app.update(Message::StartWalkTheScale);

        assert!(
            app.focusables()
                .iter()
                .all(|target| !matches!(target, FocusTarget::Root(_) | FocusTarget::CagedRoot(_))),
            "the drill stops on a root"
        );

        // The first element is annotated so both closures coerce to the same fn pointer —
        // two closures are two distinct types, and an array needs one.
        for (screen, is_a_root) in [
            (
                Screen::ScaleTrainer,
                (|target: &FocusTarget| matches!(target, FocusTarget::Root(_)))
                    as fn(&FocusTarget) -> bool,
            ),
            (Screen::Caged, |target| {
                matches!(target, FocusTarget::CagedRoot(_))
            }),
        ] {
            app.navigate_to(screen.clone());
            let stops = app.focusables().iter().filter(|t| is_a_root(t)).count();

            assert_eq!(
                stops,
                PitchClass::ALL.len(),
                "{screen:?} lost a root button"
            );
        }
    }

    #[test]
    fn the_drill_controls_are_reached_in_order() {
        let mut app = app_with_seed(0);
        app.navigate_to(Screen::WalkTheScale);
        let _ = app.update(Message::StartWalkTheScale);

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
