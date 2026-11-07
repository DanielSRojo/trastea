//! Play the Interval: the drill's state machine and the screen that draws it.
//!
//! The answer is heard rather than pressed, and nothing on screen names a note, so the drill
//! cannot be answered by hunting for a name.

use iced::{Color, Element, Length, Padding};

use super::calibration;
use super::drill::{ANSWERS, Score, choice_for, interval_label};
use super::focus::FocusTarget;
use super::fretboard::{Fretboard, MarkerStyle, NoteMarker, fretboard};
use super::listener::{Listener, State, ui_device_line, ui_device_picker};
use super::neck::{DRILL_NECK, Position, neck};
use super::placement::{Placed, Placer};
use super::style::{
    BODY, DANGER, INK, MUTE, ROOT_MARKER, SUCCESS, card_container, control_button, control_shuffle,
    streak_readout,
};
use super::{Message, SUMMARY_CARD_HEIGHT};
use crate::audio::detect::Heard;
use crate::audio::inharmonicity::Fit;
use crate::music::intervals::Interval;
use crate::music::notes::PitchClass;
use crate::rng::Rng;

/// What the drill is asking: a position to measure from, and how far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Prompt {
    root: Position,
    interval: Interval,
}

/// What the drill will do with the next confirmed reading.
///
/// A fresh prompt ignores whatever is already sounding until `Tracker` breaks its run, so a
/// ringing answer cannot answer the next prompt. No debounce timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gate {
    /// Something already answered with, or already ringing when the prompt was drawn.
    Deaf,
    /// Nothing is sounding. The next confirmed reading is an answer.
    Listening,
}

/// No iced types, so the gate and judging are testable without a widget tree or a device.
pub(super) struct PlayTheInterval {
    prompt: Prompt,
    score: Score<PitchClass>,
    gate: Gate,
    listener: Listener,
    placer: Placer,
    /// Where this prompt's answers were played; kept beside `score`, which stores them by value.
    placed: Placed,
}

impl PlayTheInterval {
    pub(super) fn new(rng: &mut Rng) -> PlayTheInterval {
        let mut drill = PlayTheInterval {
            // replaced by `draw_prompt`; a struct literal cannot call a method on itself
            prompt: Prompt {
                root: Position { string: 0, fret: 0 },
                interval: Interval::Unison,
            },
            score: Score::default(),
            gate: Gate::Deaf,
            listener: Listener::new().fitting(),
            placer: Placer::default(),
            placed: Placed::default(),
        };

        drill.draw_prompt(rng);
        drill
    }

    pub(super) fn listener(&self) -> &Listener {
        &self.listener
    }

    pub(super) fn listener_mut(&mut self) -> &mut Listener {
        &mut self.listener
    }

    /// Draws a fresh prompt, never the one already showing. Generated forward so the
    /// distance is always one the vocabulary can name.
    fn draw_prompt(&mut self, rng: &mut Rng) {
        let neck = neck();

        debug_assert!(
            neck.len() * ANSWERS.len() >= 2,
            "the drill must have at least two prompts to choose between or this loop cannot end"
        );

        self.prompt = rng.draw_other_than(self.prompt, |rng| {
            let (root, _) = neck[rng.below(neck.len())];
            let interval = ANSWERS[rng.below(ANSWERS.len())].interval;
            Prompt { root, interval }
        });

        self.score.next_prompt();
        self.placed.clear();
        // read off the device rather than set flat, so entering in silence arms immediately
        self.gate = match self.listener.reading() {
            Some(_) => Gate::Deaf,
            None => Gate::Listening,
        };
    }

    /// The pitch class the prompt is asking for. `None` only if the root is off the neck.
    fn target(&self) -> Option<PitchClass> {
        Some(
            self.prompt
                .root
                .pitch_class()?
                .transpose(self.prompt.interval.semitones()),
        )
    }

    /// Folds a released reading and its fit into the drill. Judging does not wait for the
    /// fit: a placement only narrows where an answer already judged is drawn.
    pub(super) fn apply(&mut self, heard: Heard, fit: Option<Fit>) {
        self.gate(heard);

        for placement in self.placer.hear(heard, fit) {
            self.placed.place(&placement);
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

        let Some((pitch, _)) = crate::music::pitch::Pitch::nearest(detection.hz()) else {
            return;
        };

        // a held string confirms the same reading every tick; this makes one note one answer
        self.gate = Gate::Deaf;
        self.placed.expect(pitch);
        self.judge(pitch.pitch_class());
    }

    /// Scores a played pitch class against the prompt. The octave is ignored on purpose, so
    /// an octave error is never caught.
    fn judge(&mut self, played: PitchClass) {
        self.score.mark(played, self.target() == Some(played));
    }

    /// Whether a correct answer is lit, which is the window the flash timer runs in.
    pub(super) fn is_flashing(&self) -> bool {
        self.score.is_flashing()
    }

    /// Ends the flash and draws the next prompt. A no-op outside the flash, so a stale tick
    /// cannot retire a prompt the learner is still reading.
    pub(super) fn advance(&mut self, rng: &mut Rng) {
        if self.score.is_flashing() {
            self.draw_prompt(rng);
        }
    }

    pub(super) fn skip(&mut self, rng: &mut Rng) {
        self.score.break_streak();
        self.draw_prompt(rng);
    }

    /// Opening the screen: the best streak persists, the run does not, and the kept
    /// calibration is read again.
    pub(super) fn enter(&mut self, rng: &mut Rng) {
        self.placer = Placer::new(calibration::kept());
        self.score.break_streak();
        self.draw_prompt(rng);
    }

    /// Stands in for the kept calibration, which no test may read.
    #[cfg(test)]
    pub(super) fn calibrate_with(&mut self, calibration: Option<calibration::Calibration>) {
        self.placer = Placer::new(calibration);
    }
}

/// The marks on the neck. The root is an unfilled ring and wins any collision; an answer is
/// marked where it was placed, or where its pitch sounds if it was not. No mark is labelled.
fn markers(drill: &PlayTheInterval) -> Vec<NoteMarker> {
    let root = drill.prompt.root;

    // only a note heard while the gate was deaf survives: drawn as heard, not as scored
    let sounding = drill
        .listener
        .reading()
        .map(|(pitch, _)| pitch.pitch_class())
        .filter(|played| {
            drill.score.correct() != Some(*played) && !drill.score.wrong().contains(played)
        });

    let colour_for = |position: Position, pitch: PitchClass| -> Option<(Color, MarkerStyle)> {
        let answered_here = drill.placed.shows(pitch, position);

        if drill.score.correct() == Some(pitch) && answered_here {
            Some((SUCCESS, MarkerStyle::Filled))
        } else if drill.score.wrong().contains(&pitch) && answered_here {
            Some((DANGER, MarkerStyle::Outlined))
        } else if sounding == Some(pitch) {
            Some((INK, MarkerStyle::Filled))
        } else {
            None
        }
    };

    neck()
        .into_iter()
        .filter_map(|(position, pitch)| {
            if position == root {
                return Some(NoteMarker {
                    string: position.string,
                    fret: position.fret,
                    label: String::new(),
                    color: ROOT_MARKER,
                    style: MarkerStyle::Outlined,
                });
            }

            let (color, style) = colour_for(position, pitch)?;

            Some(NoteMarker {
                string: position.string,
                fret: position.fret,
                label: String::new(),
                color,
                style,
            })
        })
        .collect()
}

pub(super) fn ui_play_the_interval(
    drill: &PlayTheInterval,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::widget::{Space, column, container, row, text};

    if drill.listener.is_picking() {
        return container(ui_device_picker(&drill.listener))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .padding(Padding::from([24, 24]))
            .into();
    }

    let neck = Fretboard {
        num_frets: DRILL_NECK.frets(),
        highlighted: markers(drill),
        ..Fretboard::default()
    };

    let question = column![
        row![
            text("Play").size(26).color(BODY),
            interval_label(&choice_for(drill.prompt.interval), 34, INK),
        ]
        .spacing(12),
        text("above the amber root — anywhere on the neck")
            .size(16)
            .color(MUTE),
    ]
    .spacing(6);

    let prompt_card = container(
        column![
            row![
                question,
                Space::new().width(Length::Fill),
                streak_readout(drill.score.streak(), drill.score.best()),
            ],
            play_controls(focused),
            Space::new().height(Length::Fill),
            ui_device_line(&drill.listener, focused),
            ui_heard_line(drill),
        ]
        .spacing(12),
    )
    .width(Length::Fill)
    .height(Length::Fixed(SUMMARY_CARD_HEIGHT))
    .padding(32)
    .style(card_container);

    container(row![fretboard(neck), column![prompt_card].width(Length::Fill)].spacing(32))
        .width(Length::Fill)
        .padding(Padding {
            top: 8.0,
            right: 64.0,
            bottom: 48.0,
            left: 64.0,
        })
        .center_y(Length::Fill)
        .into()
}

/// What the app is hearing, and while a note is read, its cents offset — so an out-of-tune
/// instrument explains why correct playing reads as wrong.
fn ui_heard_line(drill: &PlayTheInterval) -> Element<'static, Message> {
    use iced::widget::{row, text};

    let (status, offset) = heard_line(drill);

    row![
        text(status).size(13).color(MUTE),
        text(offset).size(13).color(BODY),
    ]
    .spacing(12)
    .into()
}

/// The two strings the line above draws, split out so the tests can assert no note is named.
fn heard_line(drill: &PlayTheInterval) -> (&'static str, String) {
    let status = match drill.listener.state() {
        State::NotStarted => "not listening",
        State::NoInput => "no input — this drill needs one",
        State::Silent => "listening, play the interval...",
        State::Unnameable => "one string at a time",
        State::Reading => "heard",
    };

    // `+ 0.0` so a hair flat does not print "-0 cents"
    let offset = match drill.listener.reading() {
        Some((_, cents)) => format!("{:+.0} cents", cents.value().round() + 0.0),
        None => String::new(),
    };

    (status, offset)
}

/// The header row. One control: skip.
fn play_controls(focused: FocusTarget) -> Element<'static, Message> {
    use iced::widget::row;

    row![control_button(
        control_shuffle(),
        Message::SkipPlayPrompt,
        focused == FocusTarget::SkipPlayPrompt,
    )]
    .spacing(8)
    .into()
}

#[cfg(test)]
mod tests {
    use super::super::tests::{app_with_seed, press_into};
    use super::super::{App, Screen};
    use super::*;
    use crate::audio::detect::Detection;
    use crate::music::pitch::Pitch;
    use crate::ui::calibration::tests::{grouping_guitar, spike_guitar};
    use crate::ui::placement::FITS_PER_NOTE;
    use crate::ui::placement::tests::{at, measured, pluck};
    use iced::keyboard;

    /// An app on the played drill with a reproducible prompt stream. `navigate_to` rather
    /// than `open`, so no capture device is claimed.
    fn play_app(seed: u64) -> App {
        let mut app = app_with_seed(seed);
        app.navigate_to(Screen::PlayTheInterval);
        app.play_the_interval.enter(&mut app.rng);
        app
    }

    /// A pitch, sent through as the confirmed reading the detector would have released.
    fn plays(app: &mut App, pitch: Pitch) {
        let _ = app.update(Message::Heard(Heard::Pitch(Detection::synthesized(
            pitch.frequency(),
            0.99,
        ))));
    }

    /// Breaks the run, which is what re-arms the gate.
    fn stops(app: &mut App) {
        let _ = app.update(Message::Heard(Heard::Silence));
    }

    /// Some pitch sounding the prompt's answer: the lowest one on the neck.
    fn correct_pitch(app: &App) -> Pitch {
        let target = app
            .play_the_interval
            .target()
            .expect("the prompt's root is on the neck");

        (40u8..=79)
            .filter_map(Pitch::from_midi)
            .find(|pitch| pitch.pitch_class() == target)
            .expect("the neck sounds every pitch class")
    }

    /// A pitch a semitone above the answer, never the answer itself.
    fn wrong_pitch(app: &App) -> Pitch {
        correct_pitch(app)
            .transpose(1)
            .expect("the neck is nowhere near the top of the range")
    }

    // ---- The gate ----

    #[test]
    fn one_unbroken_note_is_one_answer() {
        let mut app = play_app(0x9a1);
        let wrong = wrong_pitch(&app);

        // fifty ticks is half a second of ringing
        for _ in 0..50 {
            plays(&mut app, wrong);
        }

        assert_eq!(app.play_the_interval.score.wrong().len(), 1);
        assert_eq!(app.play_the_interval.score.streak(), 0);
    }

    #[test]
    fn a_correct_note_held_does_not_score_twice() {
        let mut app = play_app(0x9a2);
        let right = correct_pitch(&app);

        for _ in 0..50 {
            plays(&mut app, right);
        }

        assert_eq!(app.play_the_interval.score.streak(), 1);
    }

    /// The behaviour `Gate` exists for, and the one that would break silently.
    #[test]
    fn a_ringing_answer_does_not_answer_the_next_prompt() {
        let mut app = play_app(0x9a3);
        let right = correct_pitch(&app);

        plays(&mut app, right);
        assert_eq!(app.play_the_interval.score.streak(), 1);

        // The flash retires the prompt a second later — while the string is still ringing.
        let _ = app.update(Message::AdvancePlayPrompt);
        plays(&mut app, right);

        assert_eq!(
            app.play_the_interval.score.streak(),
            1,
            "the previous answer answered the next prompt"
        );
        assert!(
            app.play_the_interval.score.wrong().is_empty(),
            "and was not judged wrong either"
        );
        assert!(app.play_the_interval.score.correct().is_none());
    }

    #[test]
    fn a_skipped_prompts_note_does_not_carry_over() {
        let mut app = play_app(0x9a4);
        let sounding = wrong_pitch(&app);

        plays(&mut app, sounding);
        let _ = app.update(Message::SkipPlayPrompt);

        plays(&mut app, sounding);
        assert!(
            app.play_the_interval.score.wrong().is_empty(),
            "a note ringing across a skip was judged against the fresh prompt"
        );
    }

    #[test]
    fn the_same_note_played_again_is_a_new_answer() {
        let mut app = play_app(0x9a5);
        let wrong = wrong_pitch(&app);

        plays(&mut app, wrong);
        stops(&mut app);
        plays(&mut app, wrong);

        // `wrong` is deduplicated, so the proof is that the second sounding was judged at all
        assert_eq!(app.play_the_interval.score.wrong().len(), 1);
        assert_eq!(app.play_the_interval.gate, Gate::Deaf);
    }

    /// `Tracker` releases `Unpitched` when the pitch slides past its tolerance, which re-arms.
    #[test]
    fn a_slide_from_a_wrong_note_to_the_right_one_is_judged() {
        let mut app = play_app(0x9a6);
        let (wrong, right) = (wrong_pitch(&app), correct_pitch(&app));

        plays(&mut app, wrong);
        let _ = app.update(Message::Heard(Heard::Unpitched));
        plays(&mut app, right);

        assert_eq!(app.play_the_interval.score.streak(), 1);
    }

    #[test]
    fn a_fumble_that_never_settles_costs_nothing() {
        let mut app = play_app(0x9a7);

        for _ in 0..20 {
            let _ = app.update(Message::Heard(Heard::Unpitched));
        }

        assert!(app.play_the_interval.score.wrong().is_empty());
        assert_eq!(app.play_the_interval.score.streak(), 0);
        assert!(app.play_the_interval.score.correct().is_none());
    }

    #[test]
    fn notes_during_the_flash_are_ignored() {
        let mut app = play_app(0x9a8);
        let right = correct_pitch(&app);
        let wrong = wrong_pitch(&app);

        plays(&mut app, right);
        stops(&mut app);
        plays(&mut app, wrong);

        assert_eq!(
            app.play_the_interval.score.streak(),
            1,
            "the flash was interrupted"
        );
        assert!(app.play_the_interval.score.wrong().is_empty());
    }

    // ---- Judging ----

    #[test]
    fn the_interval_is_correct_in_every_octave() {
        for seed in [0xb1, 0xb2, 0xb3] {
            let target = play_app(seed)
                .play_the_interval
                .target()
                .expect("a root on the neck");

            let carriers: Vec<Pitch> = (40u8..=79)
                .filter_map(Pitch::from_midi)
                .filter(|pitch| pitch.pitch_class() == target)
                .collect();

            assert!(carriers.len() >= 3, "the neck spans at least three octaves");

            // a fresh app per octave, so each is judged against the same prompt
            for pitch in carriers {
                let mut app = play_app(seed);
                plays(&mut app, pitch);

                assert_eq!(
                    app.play_the_interval.score.streak(),
                    1,
                    "{pitch:?} sounds the answer and was judged wrong"
                );
            }
        }
    }

    #[test]
    fn the_interval_is_correct_played_below_the_root() {
        let mut app = play_app(0xb4);
        let target = app.play_the_interval.target().expect("a root on the neck");
        let root = app
            .play_the_interval
            .prompt
            .root
            .pitch_class()
            .expect("a root on the neck");

        // Only meaningful where the answer is not the root's own pitch class.
        if target == root {
            return;
        }

        let lowest = (40u8..=79)
            .filter_map(Pitch::from_midi)
            .find(|pitch| pitch.pitch_class() == target)
            .expect("the neck sounds every pitch class");

        plays(&mut app, lowest);
        assert_eq!(app.play_the_interval.score.streak(), 1);
    }

    #[test]
    fn a_neighbouring_pitch_class_is_wrong() {
        let mut app = play_app(0xb5);
        let wrong = wrong_pitch(&app);

        plays(&mut app, wrong);

        assert_eq!(app.play_the_interval.score.streak(), 0);
        assert_eq!(
            app.play_the_interval.score.wrong(),
            vec![wrong.pitch_class()]
        );
    }

    // ---- Prompts ----

    #[test]
    fn a_fresh_prompt_never_repeats_the_one_it_replaced() {
        let mut app = play_app(0xc1);

        for _ in 0..100 {
            let before = app.play_the_interval.prompt;
            let _ = app.update(Message::SkipPlayPrompt);

            assert_ne!(app.play_the_interval.prompt, before);
        }
    }

    #[test]
    fn every_prompt_sits_on_the_neck_and_reaches_something() {
        let mut app = play_app(0xc2);

        for _ in 0..100 {
            let prompt = app.play_the_interval.prompt;

            assert!(prompt.root.string < super::super::neck::NECK_STRINGS);
            assert!(prompt.root.fret <= DRILL_NECK.frets());
            assert!(
                app.play_the_interval.target().is_some(),
                "the prompt asks for a pitch class nothing can sound"
            );

            let _ = app.update(Message::SkipPlayPrompt);
        }
    }

    // ---- The streak ----

    #[test]
    fn the_streak_counts_consecutive_correct_answers() {
        let mut app = play_app(0xd1);

        for expected in 1..=4 {
            let right = correct_pitch(&app);
            plays(&mut app, right);
            assert_eq!(app.play_the_interval.score.streak(), expected);

            let _ = app.update(Message::AdvancePlayPrompt);
            stops(&mut app);
        }

        assert_eq!(app.play_the_interval.score.best(), 4);
    }

    #[test]
    fn a_skip_resets_the_streak_and_the_best_survives_it() {
        let mut app = play_app(0xd2);

        let right = correct_pitch(&app);
        plays(&mut app, right);
        let _ = app.update(Message::AdvancePlayPrompt);
        stops(&mut app);

        let _ = app.update(Message::SkipPlayPrompt);

        assert_eq!(app.play_the_interval.score.streak(), 0);
        assert_eq!(app.play_the_interval.score.best(), 1);
    }

    #[test]
    fn re_entering_starts_a_fresh_run_and_keeps_the_best() {
        let mut app = play_app(0xd3);

        let right = correct_pitch(&app);
        plays(&mut app, right);
        let _ = app.update(Message::AdvancePlayPrompt);
        stops(&mut app);
        assert_eq!(app.play_the_interval.score.best(), 1);

        app.play_the_interval.enter(&mut app.rng);

        assert_eq!(app.play_the_interval.score.streak(), 0);
        assert_eq!(app.play_the_interval.score.best(), 1);
        assert!(app.play_the_interval.score.wrong().is_empty());
        assert!(app.play_the_interval.score.correct().is_none());
    }

    // ---- The marks ----

    #[test]
    fn a_heard_pitch_is_marked_everywhere_it_sounds() {
        let mut app = play_app(0xe1);
        let wrong = wrong_pitch(&app);
        plays(&mut app, wrong);

        let root = app.play_the_interval.prompt.root;
        let marks = markers(&app.play_the_interval);

        for (position, _) in neck() {
            let marked = marks
                .iter()
                .find(|m| m.string == position.string && m.fret == position.fret);

            if position == root {
                assert_eq!(
                    marked.expect("the root is always marked").color,
                    ROOT_MARKER
                );
            } else if DRILL_NECK.pitch_at(position.string, position.fret) == Some(wrong) {
                assert_eq!(
                    marked.expect("a heard position is marked").color,
                    DANGER,
                    "{position:?} sounds what was played and is not marked"
                );
            } else {
                assert!(marked.is_none(), "{position:?} is marked and should not be");
            }
        }
    }

    #[test]
    fn earlier_wrong_answers_stay_marked() {
        let mut app = play_app(0xe2);
        let first = wrong_pitch(&app);
        let second = first.transpose(1).expect("in range");

        plays(&mut app, first);
        stops(&mut app);
        plays(&mut app, second);

        let marks = markers(&app.play_the_interval);
        for pitch in [first.pitch_class(), second.pitch_class()] {
            assert!(
                marks.iter().any(|m| Position {
                    string: m.string,
                    fret: m.fret
                }
                .pitch_class()
                    == Some(pitch)),
                "{pitch:?} stopped being marked"
            );
        }
    }

    #[test]
    fn the_root_position_keeps_its_mark_when_its_pitch_class_is_played() {
        let mut app = play_app(0xe3);
        let root = app.play_the_interval.prompt.root;
        let root_pitch = root.pitch_class().expect("a root on the neck");

        let sounded = (40u8..=79)
            .filter_map(Pitch::from_midi)
            .find(|pitch| pitch.pitch_class() == root_pitch)
            .expect("the neck sounds every pitch class");

        plays(&mut app, sounded);

        let marks = markers(&app.play_the_interval);
        let at_root = marks
            .iter()
            .find(|m| m.string == root.string && m.fret == root.fret)
            .expect("the root is always marked");

        assert_eq!(at_root.color, ROOT_MARKER);
        assert_eq!(at_root.style, MarkerStyle::Outlined);

        assert!(
            marks.iter().any(|m| {
                (m.string, m.fret) != (root.string, root.fret) && m.color != ROOT_MARKER
            }),
            "the played pitch class was marked nowhere"
        );
    }

    /// No mark carries a label, in any state the drill can reach.
    #[test]
    fn nothing_the_drill_draws_names_a_note() {
        let mut app = play_app(0xe4);

        let states: [Heard; 3] = [
            Heard::Silence,
            Heard::Unpitched,
            Heard::Pitch(Detection::synthesized(
                correct_pitch(&app).frequency(),
                0.99,
            )),
        ];

        let wrong = wrong_pitch(&app);
        plays(&mut app, wrong);
        stops(&mut app);

        for heard in states {
            let _ = app.update(Message::Heard(heard));

            for mark in markers(&app.play_the_interval) {
                assert!(
                    mark.label.is_empty(),
                    "a mark is labelled {:?}, which names a note",
                    mark.label
                );
            }

            let (status, offset) = heard_line(&app.play_the_interval);
            for word in [status, offset.as_str()] {
                assert!(
                    !word.chars().any(|c| matches!(c, 'A'..='G')),
                    "{word:?} names a note"
                );
            }
        }
    }

    // ---- Where an answer was played ----

    /// Asks for a fifth above `root`, on a drill reading `calibration`.
    fn asking(calibration: Option<calibration::Calibration>, root: Position) -> App {
        let mut app = play_app(0x5a1);
        app.play_the_interval.calibrate_with(calibration);
        app.play_the_interval.prompt = Prompt {
            root,
            interval: Interval::PerfectFifth,
        };
        app
    }

    /// One note, held long enough to be fitted as often as a placement needs.
    fn plays_with(app: &mut App, pitch: Pitch, log2: f32) {
        for _ in 0..FITS_PER_NOTE {
            let _ = app.update(Message::Fitted(
                Heard::Pitch(Detection::synthesized(pitch.frequency(), 0.99)),
                measured(log2),
            ));
        }
    }

    fn marked(app: &App, color: Color) -> Vec<Position> {
        markers(&app.play_the_interval)
            .into_iter()
            .filter(|marker| marker.color == color)
            .map(|marker| at(marker.string, marker.fret))
            .collect()
    }

    /// Every position sounding `pitch` but the root, in the order the marks are drawn.
    fn sounding_at(pitch: Pitch, root: Position) -> Vec<Position> {
        neck()
            .into_iter()
            .map(|(position, _)| position)
            .filter(|&position| {
                position != root
                    && DRILL_NECK.pitch_at(position.string, position.fret) == Some(pitch)
            })
            .collect()
    }

    #[test]
    fn an_answer_placed_at_one_position_is_marked_there_alone() {
        let guitar = spike_guitar();
        let (e4, open_e) = pluck(&guitar, at(5, 0));
        // A on the A string, and a fifth above it is E
        let mut app = asking(Some(guitar), at(1, 0));

        plays_with(&mut app, e4, open_e);

        assert_eq!(app.play_the_interval.score.streak(), 1);
        assert_eq!(marked(&app, SUCCESS), vec![at(5, 0)]);
    }

    #[test]
    fn an_answer_placed_at_a_group_is_marked_at_the_group_alone() {
        let guitar = grouping_guitar();
        let (a3, g_at_2) = pluck(&guitar, at(3, 2));
        // D on the D string, and a fifth above it is A
        let mut app = asking(Some(guitar), at(2, 0));

        plays_with(&mut app, a3, g_at_2);

        assert_eq!(marked(&app, SUCCESS), vec![at(2, 7), at(3, 2)]);
    }

    #[test]
    fn an_answer_it_cannot_place_is_marked_where_its_pitch_sounds() {
        let guitar = spike_guitar();
        let (e4, open_e) = pluck(&guitar, at(5, 0));

        // for want of a calibration
        let mut app = asking(None, at(1, 0));
        plays_with(&mut app, e4, open_e);
        assert_eq!(marked(&app, SUCCESS), sounding_at(e4, at(1, 0)));

        // and slid into, with one
        let mut app = asking(Some(guitar), at(1, 0));
        let from = e4.transpose(-1).expect("in range").frequency();
        let bent = Heard::Pitch(Detection::synthesized(e4.frequency(), 0.99).slid_from(from));
        let _ = app.update(Message::Fitted(bent, measured(open_e)));
        assert_eq!(marked(&app, SUCCESS), sounding_at(e4, at(1, 0)));
    }

    #[test]
    fn a_wrong_answer_is_marked_where_it_was_placed() {
        let guitar = spike_guitar();
        let (e4, open_e) = pluck(&guitar, at(5, 0));
        // asking for B, a fifth above E
        let mut app = asking(Some(guitar), at(0, 0));

        plays_with(&mut app, e4, open_e);

        assert_eq!(app.play_the_interval.score.wrong(), [e4.pitch_class()]);
        assert_eq!(marked(&app, DANGER), vec![at(5, 0)]);
    }

    #[test]
    fn judging_does_not_wait_and_the_marks_only_narrow() {
        let guitar = spike_guitar();
        let (e4, open_e) = pluck(&guitar, at(5, 0));
        let mut app = asking(Some(guitar), at(1, 0));

        // confirmed, and not yet fitted
        let _ = app.update(Message::Fitted(
            Heard::Pitch(Detection::synthesized(e4.frequency(), 0.99)),
            None,
        ));
        assert_eq!(
            app.play_the_interval.score.streak(),
            1,
            "judging waited for a fit"
        );
        let before = marked(&app, SUCCESS);
        assert_eq!(before, sounding_at(e4, at(1, 0)));

        plays_with(&mut app, e4, open_e);
        let after = marked(&app, SUCCESS);

        assert_eq!(after, vec![at(5, 0)]);
        assert!(after.iter().all(|position| before.contains(position)));
    }

    #[test]
    fn a_placed_note_names_no_note_and_no_string() {
        let guitar = spike_guitar();
        let (e4, open_e) = pluck(&guitar, at(5, 0));
        let mut app = asking(Some(guitar), at(1, 0));

        plays_with(&mut app, e4, open_e);

        assert!(
            markers(&app.play_the_interval)
                .iter()
                .all(|m| m.label.is_empty())
        );
        let (status, offset) = heard_line(&app.play_the_interval);
        for word in [status, offset.as_str()] {
            assert!(
                !word.chars().any(|c| matches!(c, 'A'..='G')),
                "{word:?} names a note or a string"
            );
        }
    }

    #[test]
    fn a_note_heard_during_the_flash_is_not_placed_as_the_answer() {
        let guitar = spike_guitar();
        let (e4, open_e) = pluck(&guitar, at(5, 0));
        let (other_e, g_at_9) = pluck(&guitar, at(3, 9));
        let mut app = asking(Some(guitar), at(1, 0));

        plays_with(&mut app, e4, open_e);
        stops(&mut app);
        plays_with(&mut app, other_e, g_at_9);

        assert_eq!(marked(&app, SUCCESS), vec![at(5, 0)]);
    }

    // ---- What the screen says it is hearing ----

    #[test]
    fn the_four_states_read_differently() {
        let mut app = app_with_seed(0xf1);
        app.open(Screen::PlayTheInterval);

        // `NotStarted` is read before the device is opened, not from here
        let opened = app.play_the_interval.listener().error().is_none();

        let _ = app.update(Message::Heard(Heard::Silence));
        let silent = heard_line(&app.play_the_interval).0;

        let _ = app.update(Message::Heard(Heard::Unpitched));
        let unnameable = heard_line(&app.play_the_interval).0;

        let right = correct_pitch(&app);
        plays(&mut app, right);
        let reading = heard_line(&app.play_the_interval).0;

        if opened {
            assert_ne!(silent, unnameable);
            assert_ne!(unnameable, reading);
            assert_ne!(silent, reading);
        } else {
            // no device on this machine: all three collapse to one state
            assert_eq!(silent, "no input — this drill needs one");
        }
    }

    #[test]
    fn a_reading_does_not_outlive_the_sound() {
        let mut app = play_app(0xf2);

        let right = correct_pitch(&app);
        plays(&mut app, right);
        assert!(app.play_the_interval.listener().reading().is_some());

        stops(&mut app);
        assert!(
            app.play_the_interval.listener().reading().is_none(),
            "a reading outlived the note that produced it"
        );
    }

    #[test]
    fn a_badly_tuned_instrument_shows_a_large_offset() {
        let mut app = play_app(0xf3);
        let right = correct_pitch(&app);

        // Forty cents flat: still named as the same pitch, and visibly not on it.
        let flat = right.frequency() * 2.0f32.powf(-40.0 / 1200.0);
        let _ = app.update(Message::Heard(Heard::Pitch(Detection::synthesized(
            flat, 0.99,
        ))));

        let (_, offset) = heard_line(&app.play_the_interval);
        assert_eq!(offset, "-40 cents");
    }

    // ---- The keyboard ----

    #[test]
    fn r_skips_the_prompt_and_d_opens_the_device_list() {
        let mut app = play_app(0x101);

        let before = app.play_the_interval.prompt;
        press_into(&mut app, "r", keyboard::Modifiers::empty());
        assert_ne!(app.play_the_interval.prompt, before, "`r` did not skip");

        press_into(&mut app, "d", keyboard::Modifiers::empty());
        assert!(
            app.play_the_interval.listener().is_picking(),
            "`d` did not open the device list"
        );

        let _ = app.update(Message::GoBack);
        assert!(!app.play_the_interval.listener().is_picking());
        assert_eq!(app.screen, Screen::PlayTheInterval, "Esc left the screen");
    }

    #[test]
    fn another_screens_keys_are_inert_here() {
        let mut app = play_app(0x102);
        let before = app.play_the_interval.prompt;

        for key in ["a", "i", "n", "f"] {
            press_into(&mut app, key, keyboard::Modifiers::empty());
        }

        assert_eq!(app.play_the_interval.prompt, before);
        assert_eq!(app.play_the_interval.score.streak(), 0);
        assert!(!app.play_the_interval.listener().is_picking());
    }
}
