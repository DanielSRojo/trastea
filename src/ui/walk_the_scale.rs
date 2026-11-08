//! Walk the Scale: the drill's state machine and the screen that draws it.
//!
//! It owns the scale it drills, so rerolling here never moves the Scale Trainer.

use std::time::{Duration, Instant};

use iced::{Color, Element, Length};

use super::Message;
use super::calibration;
use super::focus::{Direction, FocusTarget};
use super::fretboard::{Fretboard, MarkerStyle, NoteMarker, fretboard};
use super::listener::{Listener, State, ui_device_line, ui_device_picker};
use super::neck::{CURSOR_HOME, DRILL_NECK, NECK_STRINGS, Position};
use super::placement::{Placed, Placement, Placer};
use super::root_stage::{formula, kind_card, root_column, root_glyph, selector_column, stage};
use super::style::{
    BODY, CONTROL_SIZE, DANGER, INK, LINK, MUTE, SUCCESS, control_button, control_label,
    control_shuffle,
};
use crate::audio::detect::Heard;
use crate::audio::inharmonicity::Fit;
use crate::music::intervals::Interval;
use crate::music::notes::PitchClass;
use crate::music::pitch::Pitch;
use crate::music::scales::{Scale, ScaleKind};
use crate::rng::Rng;

/// How the learner answers the drill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Answering {
    Pressing,
    Playing { neck_hidden: bool },
}

/// What the drill does with the next confirmed reading.
/// Duplicates `play_the_interval::Gate` on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gate {
    Deaf,
    Listening,
}

/// What an answer was judged, which says where its place is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Judged {
    Found,
    Wrong,
}

/// One run of the drill, so nothing from it survives into the ended state.
struct Run {
    /// How far through the sequence. The degrees are read from the scale, never stored.
    at: usize,
    /// Sequences completed without a wrong answer.
    streak: u32,
    /// Sequences completed at all, which is what a timed run reports.
    completed: u32,
    /// Only a sequence with no wrong answer raises the streak.
    clean: bool,
    answered: Vec<PitchClass>,
    /// Every wrong pitch class given to the current element, cleared when it advances.
    wrong: Vec<PitchClass>,
    /// The pitch class that completed a sequence, held while it is lit. Flashes per
    /// sequence, not per element.
    flashing: Option<PitchClass>,
    gate: Gate,
    /// Where the sequence's found degrees were played, and the current element's wrong answers.
    found_at: Placed,
    wrong_at: Placed,
    /// Stored rather than a countdown, which would drift if a tick were missed.
    started: Instant,
    /// `None` for an untimed run.
    duration: Option<Duration>,
    cursor: Position,
}

/// What the screen is doing. Entering starts a run, so there is no display state.
enum Mode {
    Running(Run),
    Ended {
        completed: u32,
        best_streak: u32,
        /// What the run was set to, so "again" means the same run again.
        duration: Option<Duration>,
    },
}

/// No iced types, so the sequence and gate are testable without a widget tree or a device.
pub(super) struct WalkTheScale {
    scale: Scale,
    /// A preference that survives leaving the drill, unlike the `Listener`.
    answering: Answering,
    best_streak: u32,
    /// The capture device, held only while a run is listening.
    listener: Option<Listener>,
    placer: Placer,
    mode: Mode,
}

/// The durations `t` cycles through. `None` first, so the first press starts timing.
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
            found_at: Placed::default(),
            wrong_at: Placed::default(),
            started: Instant::now(),
            duration,
            cursor: CURSOR_HOME,
        }
    }

    /// The degree the drill is asking for, or `None` past the end of the sequence. `get`
    /// because a kind change can leave `at` past a shorter kind's degrees.
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
        self.found_at.clear();
        self.wrong_at.clear();
    }

    /// Narrows whichever answer this placement is the note of.
    fn place(&mut self, placement: &Placement) {
        if !self.found_at.place(placement) {
            self.wrong_at.place(placement);
        }
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
            // the device is not claimed here; `enter` opens it
            answering: Answering::Playing { neck_hidden: false },
            best_streak: 0,
            listener: None,
            placer: Placer::default(),
            mode: Mode::Running(Run::new(None, Gate::Listening)),
        }
    }

    pub(super) fn scale(&self) -> Scale {
        self.scale
    }

    /// Changing the root starts the sequence again.
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

    pub(super) fn select_kind(&mut self, kind: ScaleKind) {
        self.scale = Scale::new(self.scale.root(), kind);
        self.restart_sequence();
    }

    /// `r`: reroll the scale being drilled.
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
            self.claim();
        }

        // armed off the device so a run begun over a ringing string waits for it to stop
        let gate = match self.listener.as_ref().and_then(Listener::reading) {
            Some(_) => Gate::Deaf,
            None => Gate::Listening,
        };

        self.mode = Mode::Running(Run::new(duration, gate));
    }

    /// Leaving the screen: the run ends and the device is released, so a duration is not
    /// spent off screen.
    pub(super) fn leave(&mut self) {
        self.mode = Mode::Ended {
            completed: 0,
            best_streak: self.best_streak,
            duration: self.duration(),
        };
        self.release();
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

    /// Opening the screen: the scale is kept, the run is fresh, the best streak persists, and
    /// the kept calibration is read again.
    pub(super) fn enter(&mut self) {
        self.placer = Placer::new(calibration::kept());
        self.release();
        self.begin(None);
    }

    /// Stands in for the kept calibration, which no test may read.
    #[cfg(test)]
    pub(super) fn calibrate_with(&mut self, calibration: Option<calibration::Calibration>) {
        self.placer = Placer::new(calibration);
    }

    /// The selection changed under a running sequence, so the sequence starts again.
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
            Answering::Playing { .. } if self.is_running() => self.claim(),
            _ => self.release(),
        }
    }

    /// Hides the neck. Inert in pressing mode, where the neck is the answer surface.
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

    /// A position pressed on the neck. Inert in playing mode.
    pub(super) fn press(&mut self, pitch_class: PitchClass) {
        if self.answering == Answering::Pressing {
            self.judge(pitch_class);
        }
    }

    /// A released reading and its fit, folded in. Judging does not wait for the fit: a
    /// placement only narrows where an answer already judged is drawn.
    pub(super) fn apply(&mut self, heard: Heard, fit: Option<Fit>) {
        if !matches!(self.answering, Answering::Playing { .. }) {
            return;
        }

        self.gate(heard);

        for placement in self.placer.hear(heard, fit) {
            if let Mode::Running(run) = &mut self.mode {
                run.place(&placement);
            }
        }
    }

    /// A non-pitch reading only re-arms the gate, so silence and a fumble cost nothing.
    fn gate(&mut self, heard: Heard) {
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

        // a held string confirms the same reading every poll; this makes one note one answer
        let judged = self.judge(pitch.pitch_class());
        if let Mode::Running(run) = &mut self.mode {
            run.gate = Gate::Deaf;
            match judged {
                Some(Judged::Found) => run.found_at.expect(pitch),
                Some(Judged::Wrong) => run.wrong_at.expect(pitch),
                None => {}
            }
        }
    }

    /// Scores a pitch class against the element being asked for. The octave is ignored, so a
    /// walked scale need not ascend.
    ///
    /// Not `drill::Score`: the streak counts clean sequences and the best outlives each run.
    fn judge(&mut self, played: PitchClass) -> Option<Judged> {
        let scale = self.scale;
        let (streak, judged) = {
            let Mode::Running(run) = &mut self.mode else {
                return None;
            };
            if run.flashing.is_some() {
                return None;
            }
            let target = run.target(scale)?;

            let judged = if played == target {
                run.answered.push(played);
                run.wrong.clear();
                run.wrong_at.clear();
                run.at += 1;

                // only the completion holds the screen; elements advance as they are judged
                if run.at >= scale.kind().intervals().len() {
                    run.completed += 1;
                    if run.clean {
                        run.streak += 1;
                    }
                    run.flashing = Some(played);
                }
                Judged::Found
            } else {
                if !run.wrong.contains(&played) {
                    run.wrong.push(played);
                }
                run.clean = false;
                run.streak = 0;
                Judged::Wrong
            };

            (run.streak, judged)
        };

        self.best_streak = self.best_streak.max(streak);
        Some(judged)
    }

    /// Ends the flash and begins the next sequence. A no-op outside the flash, so a stale
    /// tick cannot retire a sequence the learner is still reading.
    pub(super) fn advance(&mut self) {
        // read off the device: the completing note is still ringing and must not answer the
        // next sequence's first element
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

    /// The neck claims the motion keys while it is focused.
    pub(super) fn move_cursor(&mut self, direction: Direction) {
        let Mode::Running(run) = &mut self.mode else {
            return;
        };
        run.cursor = run.cursor.step(direction);
    }

    /// Enter on the neck answers with the position under the cursor.
    pub(super) fn answer_at_cursor(&mut self) {
        let Some(Position { string, fret }) = self.cursor() else {
            return;
        };
        if let Some(pitch_class) = DRILL_NECK.pitch_class_at(string, fret) {
            self.press(pitch_class);
        }
    }

    /// Moves a run's start back, so a duration can be spent in a test without waiting.
    #[cfg(test)]
    fn rewind(&mut self, by: Duration) {
        if let Mode::Running(run) = &mut self.mode {
            run.started -= by;
        }
    }

    /// Cycles the duration, restarting the run so the new duration counts from its start.
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

/// The screen: the neck on the left, the root and sequence in the middle, the selector and
/// controls on the right.
pub(super) fn ui_walk_the_scale(
    drill: &WalkTheScale,
    dragging: bool,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::widget::{container, text};

    // modal while it is up: nothing is drawn behind it
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
        ..Fretboard::default()
    };

    let neck: Element<'static, Message> = if drill.neck_hidden() {
        container(
            text("the neck is hidden — find the degree by ear")
                .size(13)
                .color(MUTE),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    } else {
        fretboard(fb)
    };

    let centre = root_column(
        root_glyph(scale.root_note(), dragging),
        sequence_text(drill),
        None,
    );

    let selector = selector_column(
        kind_card(
            scale.kind(),
            focused,
            Message::SelectDrillKind,
            FocusTarget::DrillKind,
        ),
        drill_panel(drill, focused),
    );

    stage(neck, centre, selector)
}

/// The scale's formula, with the drill's place in it marked: found degrees lit, the current
/// one bright, the rest muted.
fn sequence_text(drill: &WalkTheScale) -> iced::widget::Row<'static, Message> {
    let at = drill.progress().map_or(usize::MAX, |(found, _)| found);

    formula(drill.scale().kind().intervals(), |index| {
        match index.cmp(&at) {
            std::cmp::Ordering::Less => SUCCESS,
            std::cmp::Ordering::Equal => INK,
            std::cmp::Ordering::Greater => MUTE,
        }
    })
}

/// The run's controls and its numbers, at the foot of the right column.
fn drill_panel(drill: &WalkTheScale, focused: FocusTarget) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{column, row, text};

    let mut controls = row![control_button(
        control_shuffle(),
        Message::RerollDrillScale,
        focused == FocusTarget::RerollDrillScale,
    )]
    .spacing(8);

    if drill.is_running() {
        controls = controls.push(control_button(
            // `control_glyph` pins its width to one glyph, which would put a word off-centre
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
        controls = controls.push(control_button(
            control_label(text("again"), CONTROL_SIZE),
            Message::StartWalkTheScale,
            focused == FocusTarget::RestartDrill,
        ));
    }

    let readout: Element<'static, Message> = match drill.summary() {
        Some((completed, best)) => column![
            text(format!("{completed} sequences")).size(22).color(INK),
            text(format!("best streak {best}"))
                .size(CONTROL_SIZE)
                .color(MUTE),
        ]
        .spacing(4)
        .into(),
        // the time is only pushed when there is one, so an untimed run stays centred
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

    let mut panel = column![controls, readout]
        .spacing(12)
        .align_x(iced::Alignment::Center)
        .width(Length::Fill);

    if let Some(listener) = drill.listener() {
        panel = panel.push(text(listening_line(listener)).size(13).color(MUTE));
        panel = panel.push(ui_device_line(listener, focused));
    }

    panel.into()
}

/// What the screen says it is hearing, as words so the tests can read them.
fn listening_line(listener: &Listener) -> &'static str {
    match listener.state() {
        State::NotStarted => "not listening",
        State::NoInput => "no input — press m to answer with the neck instead",
        State::Silent => "listening, play the degree...",
        State::Unnameable => "one string at a time",
        State::Reading => "heard",
    }
}

/// What one position is marked as while a drill is running, or `None` if it is not marked. An
/// answer is marked where it was played: where it was placed, where its pitch sounds if it
/// was not, and wherever its pitch class falls only if it was pressed.
///
/// Priority is flash, then wrong, then answered. Labels are degrees, never note names.
fn mark_for(run: &Run, scale: Scale, position: Position) -> Option<(Color, String)> {
    let pitch_class = position.pitch_class()?;
    let found_here = run.found_at.shows(pitch_class, position);

    if run.flashing == Some(pitch_class) && found_here {
        return Some((SUCCESS, String::new()));
    }
    if run.wrong.contains(&pitch_class) && run.wrong_at.shows(pitch_class, position) {
        return Some((DANGER, String::new()));
    }
    if !found_here {
        return None;
    }

    let index = run.answered.iter().position(|at| *at == pitch_class)?;
    let label = scale
        .kind()
        .intervals()
        .get(index)
        .map_or_else(String::new, Interval::to_string);

    Some((LINK, label))
}

/// The neck while a drill is running: answered and wrong marks only, never the scale, which
/// would give the answers away.
fn drill_markers(drill: &WalkTheScale) -> Vec<NoteMarker> {
    let scale = drill.scale();
    let Some(run) = drill.run() else {
        return Vec::new();
    };

    (0..NECK_STRINGS)
        .flat_map(|string| (0..=DRILL_NECK.frets()).map(move |fret| Position { string, fret }))
        .filter_map(|position| {
            let (color, label) = mark_for(run, scale, position)?;
            Some(NoteMarker {
                string: position.string,
                fret: position.fret,
                label,
                color,
                style: MarkerStyle::Filled,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::tests::{app_with_seed, press_into};
    use super::super::{App, Message, Screen};
    use super::*;
    use crate::audio::detect::Detection;
    use crate::ui::calibration::Calibration;
    use crate::ui::calibration::tests::spike_guitar;
    use crate::ui::placement::FITS_PER_NOTE;
    use crate::ui::placement::tests::{measured, pluck};
    use crate::ui::position::{Estimate, estimate};
    use crate::ui::root_stage::STEP_DISTANCE;
    use iced::keyboard;

    /// An app on the drill with a known scale and a run going. `navigate_to` rather than
    /// `open`, so nothing is rerolled.
    fn drill_app(root: u8, kind: ScaleKind) -> App {
        let mut app = app_with_seed(0);
        app.navigate_to(Screen::WalkTheScale);
        // into pressing before the run begins, so no capture device is claimed
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

    /// The lowest position carrying a pitch class.
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

    /// Answers with the semitone above, never a degree of the scales these tests use.
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

    // ---- The root, as the drill holds it ----

    fn root(app: &App) -> PitchClass {
        app.walk_the_scale.scale().root()
    }

    /// Steps of drag travel, as the x a `DragRoot` carries. The press is at zero.
    fn drags(app: &mut App, steps: f32) {
        let _ = app.update(Message::DragRoot(steps * STEP_DISTANCE));
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
        let _ = app.update(Message::GrabRoot(0.0));

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
        let _ = app.update(Message::GrabRoot(0.0));

        drags(&mut app, -2.0);
        assert_eq!(root(&app), PitchClass::new(3));
    }

    #[test]
    fn a_drag_back_to_the_press_point_returns_the_root() {
        let mut app = drill_app(9, ScaleKind::Ionian);
        let _ = app.update(Message::GrabRoot(0.0));

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

        let _ = app.update(Message::GrabRoot(120.0));
        // Short of a step, in both directions.
        let _ = app.update(Message::DragRoot(120.0 + STEP_DISTANCE * 0.9));
        let _ = app.update(Message::DragRoot(120.0 - STEP_DISTANCE * 0.9));
        let _ = app.update(Message::DropRoot);

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
        let _ = app.update(Message::GrabRoot(0.0));
        let _ = app.update(Message::DropRoot);

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

        let _ = app.update(Message::ScrollRoot(1.0));
        assert_eq!(root(&app), PitchClass::new(1));

        let _ = app.update(Message::ScrollRoot(-2.0));
        assert_eq!(
            root(&app),
            PitchClass::new(11),
            "the wheel did not wrap downwards"
        );
    }

    /// Trackpad deltas arrive far short of a step each, so the remainder must carry.
    #[test]
    fn two_half_steps_of_scroll_make_one_semitone() {
        let mut app = drill_app(0, ScaleKind::Ionian);

        let _ = app.update(Message::ScrollRoot(0.5));
        assert_eq!(
            root(&app),
            PitchClass::new(0),
            "half a step is not a semitone"
        );

        let _ = app.update(Message::ScrollRoot(0.5));
        assert_eq!(root(&app), PitchClass::new(1));
    }

    #[test]
    fn a_scroll_and_its_reverse_leave_the_root_alone() {
        let mut app = drill_app(0, ScaleKind::Ionian);

        let _ = app.update(Message::ScrollRoot(0.6));
        let _ = app.update(Message::ScrollRoot(-0.6));

        assert_eq!(root(&app), PitchClass::new(0));
    }

    #[test]
    fn the_wheel_is_ignored_mid_drag() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        let _ = app.update(Message::GrabRoot(0.0));

        let _ = app.update(Message::ScrollRoot(3.0));
        assert_eq!(
            root(&app),
            PitchClass::new(0),
            "the wheel stepped during a drag"
        );

        drags(&mut app, 1.0);
        assert_eq!(root(&app), PitchClass::new(1), "the travel alone decides");
    }

    #[test]
    fn passing_through_roots_does_not_score() {
        let mut app = drill_app(0, ScaleKind::MinorPentatonic);
        let _ = app.update(Message::CycleScaleDuration);
        completes(&mut app);
        let _ = app.update(Message::AdvanceScaleSequence);

        let _ = app.update(Message::GrabRoot(0.0));
        drags(&mut app, 5.0);
        let _ = app.update(Message::DropRoot);

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
        // eight degrees then five, so a carried-over index would point past the end
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

        // bound first: `target(&app)` inline would borrow `app` while `presses` holds it mutably
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
        // a sequence need not ascend
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
        // the detector releases a non-pitch reading mid-slide, so no slide handling is needed
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

    // ---- Where a played answer is drawn ----

    /// A drill answered by playing, placing what it hears with `calibration`.
    fn playing_app(kind: ScaleKind, calibration: Option<Calibration>) -> App {
        let mut app = drill_app(0, kind);
        app.walk_the_scale.calibrate_with(calibration);
        let _ = app.update(Message::ToggleScaleAnswering);
        app
    }

    /// A position carrying `pitch_class` that this calibration places alone.
    fn named_position(guitar: &Calibration, pitch_class: PitchClass) -> Position {
        super::super::neck::neck()
            .into_iter()
            .filter(|&(_, at)| at == pitch_class)
            .map(|(position, _)| position)
            .find(|&position| {
                let (pitch, log2) = pluck(guitar, position);
                estimate(pitch, log2.exp2(), Some(guitar), &DRILL_NECK) == Estimate::Named(position)
            })
            .expect("the spike guitar names some position of every pitch class")
    }

    /// A clean pluck of `position`, held long enough to be placed, and let go.
    fn plucks(app: &mut App, guitar: &Calibration, position: Position) {
        let (pitch, log2) = pluck(guitar, position);
        for _ in 0..FITS_PER_NOTE {
            let _ = app.update(Message::Fitted(
                Heard::Pitch(Detection::synthesized(pitch.frequency(), 0.99)),
                measured(log2),
            ));
        }
        stops(app);
    }

    fn marked_in(app: &App, color: Color) -> Vec<(Position, String)> {
        drill_markers(&app.walk_the_scale)
            .into_iter()
            .filter(|marker| marker.color == color)
            .map(|marker| {
                (
                    Position {
                        string: marker.string,
                        fret: marker.fret,
                    },
                    marker.label,
                )
            })
            .collect()
    }

    #[test]
    fn the_playing_device_fits_what_it_hears() {
        let app = playing_app(ScaleKind::Ionian, None);
        assert!(
            app.walk_the_scale
                .listener()
                .is_some_and(Listener::is_fitting)
        );
    }

    #[test]
    fn a_walked_scale_builds_up_where_it_was_played() {
        let guitar = spike_guitar();
        let mut app = playing_app(ScaleKind::Ionian, Some(guitar.clone()));

        let mut played = Vec::new();
        for degree in &ScaleKind::Ionian.intervals()[..4] {
            let position = named_position(&guitar, target(&app));
            plucks(&mut app, &guitar, position);
            played.push((position, degree.to_string()));
        }

        let mut marked = marked_in(&app, LINK);
        marked.sort_by_key(|(position, _)| (position.string, position.fret));
        played.sort_by_key(|(position, _)| (position.string, position.fret));
        assert_eq!(marked, played);
    }

    #[test]
    fn an_unplaced_degree_is_marked_where_its_pitch_sounds() {
        let mut app = playing_app(ScaleKind::Ionian, None);
        let note = sounding(target(&app));

        plays(&mut app, note);

        let marked: Vec<Position> = marked_in(&app, LINK)
            .into_iter()
            .map(|(position, _)| position)
            .collect();
        let sounds: Vec<Position> = DRILL_NECK.positions_of(note).collect();
        assert_eq!(marked, sounds);
    }

    #[test]
    fn a_placed_wrong_note_is_marked_where_it_was_played() {
        let guitar = spike_guitar();
        let mut app = playing_app(ScaleKind::Ionian, Some(guitar.clone()));
        let position = named_position(&guitar, target(&app).transpose(1));

        plucks(&mut app, &guitar, position);

        assert_eq!(app.walk_the_scale.progress(), Some((0, 7)));
        assert_eq!(marked_in(&app, DANGER), vec![(position, String::new())]);
    }

    #[test]
    fn pressing_still_marks_everywhere_with_a_calibration_kept() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        app.walk_the_scale.calibrate_with(Some(spike_guitar()));
        let root = target(&app);

        answers(&mut app);

        let everywhere = super::super::neck::neck()
            .into_iter()
            .filter(|&(_, at)| at == root)
            .count();
        assert_eq!(marked_in(&app, LINK).len(), everywhere);
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

        // swapping back needs no code: the flag is dropped with the variant
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

        // a countdown would still read 3:00 here; `started.elapsed()` cannot
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
        // no `m`: the screen already opens playing, which is what makes `n` live
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

        // `p` is the one key the summary still claims: another run
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

    /// An accelerator fires a widget without moving the focus ring.
    #[test]
    fn the_root_keys_leave_the_focus_ring_alone() {
        let mut app = drill_app(0, ScaleKind::Ionian);
        app.focused = FocusTarget::DrillKind(4);

        press_into(&mut app, "]", keyboard::Modifiers::empty());

        assert_eq!(root(&app), PitchClass::new(1));
        assert_eq!(app.focused, FocusTarget::DrillKind(4));
    }

    /// The ring's whole path: Back, the sixteen kinds in row order, then the run's controls.
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
