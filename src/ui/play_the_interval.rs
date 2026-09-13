//! Play the Interval: the drill's state machine and the screen that draws it.
//!
//! Both halves live here for the reason the other drills give — the state keeps its fields
//! to itself, and the views below are the only things that read them.
//!
//! What sets this drill apart is where the answer comes from. The neck takes no press and
//! there is no answer grid: a prompt lights a root, names an interval, and waits to *hear*
//! the note. Judging compares pitch classes, so the interval is correct in any octave and
//! at any position carrying it — the rule the Interval Trainer already applies to a
//! position offered for a named interval.
//!
//! And nothing here is named. Not the root, not the target, not what was played. A screen
//! that printed the name of the note it just heard would let a learner answer by hunting
//! until the expected name appeared, which is note-name recall rather than interval recall.
//! The cents offset is the one number on screen, and it names nothing: it says *your
//! intonation is off by this much* without saying what it is off from.

use iced::{Color, Element, Length, Padding};

use super::fretboard::{Fretboard, MarkerStyle, NoteMarker, fretboard};
use super::listener::{Listener, State, ui_device_line, ui_device_picker};
use super::{
    ANSWERS, BODY, DANGER, DRILL_NECK, FocusTarget, INK, MUTE, Message, Position, ROOT_MARKER,
    SUCCESS, SUMMARY_CARD_HEIGHT, card_container, choice_for, control_button, control_shuffle,
    interval_label, neck, streak_readout,
};
use crate::audio::detect::Heard;
use crate::music::intervals::Interval;
use crate::music::notes::PitchClass;
use crate::rng::Rng;

/// What the drill is asking: a position to measure from, and how far.
///
/// One struct rather than an enum, because there is one direction. *Name it* has no
/// counterpart here — a learner cannot name an interval by playing one, since the thing
/// played is the answer rather than a description of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Prompt {
    root: Position,
    interval: Interval,
}

/// What the drill will do with the next confirmed reading.
///
/// The whole of "a fresh prompt hears nothing until what is sounding stops". A correct
/// answer is still ringing when the flash retires the prompt a second later, and a prompt
/// that accepted what was already sounding would be answered by the previous answer.
///
/// Detecting the break costs nothing, which is why this is two states and not a timer.
/// `Tracker` clears its run on silence, on an unclear window, *and* on a reading more than
/// its tolerance from the one before it — so a note stopping, a re-pluck through its
/// inharmonic attack, and a slide from one note to another all release something other
/// than `Heard::Pitch` first. The detector's definition of "a different note" is this
/// drill's definition of "a different answer", which is one definition rather than two.
///
/// A debounce timer was the alternative, and its `n` is a number nobody can pick: shorter
/// than the ring of an open low E and the previous answer answers the next prompt, longer
/// and a learner on a run is made to wait. There is no number here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gate {
    /// Something is sounding that has already been answered with, or was already ringing
    /// when this prompt was drawn.
    Deaf,
    /// Nothing is sounding. The next confirmed reading is an answer.
    Listening,
}

/// The drill's whole state.
///
/// No iced types, so the gate, the judging and the streak rules can be tested without a
/// widget tree — and, because `Listener::apply` is the seam, without a capture device.
pub(super) struct PlayTheInterval {
    prompt: Prompt,
    streak: u32,
    best_streak: u32,
    /// Every wrong pitch class played against the current prompt, cleared when it advances
    /// — a `Vec` for the reason the other drills' is one: replacing the last wrong answer
    /// would un-mark the earlier ones.
    wrong: Vec<PitchClass>,
    /// The pitch class that scored, held while it is lit. `Some` is the paused state.
    correct: Option<PitchClass>,
    gate: Gate,
    listener: Listener,
}

impl PlayTheInterval {
    pub(super) fn new(rng: &mut Rng) -> PlayTheInterval {
        let mut drill = PlayTheInterval {
            // Replaced by `draw_prompt` before anything reads it; a struct literal cannot
            // call a method on the value it is still building.
            prompt: Prompt {
                root: Position { string: 0, fret: 0 },
                interval: Interval::Unison,
            },
            streak: 0,
            best_streak: 0,
            wrong: Vec::new(),
            correct: None,
            gate: Gate::Deaf,
            listener: Listener::new(),
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

    /// Draws a fresh prompt, never the one already showing.
    ///
    /// Generated forward — a root and an interval — rather than by lighting two positions
    /// and asking what lies between them, for the reason the Interval Trainer gives: it is
    /// what guarantees the distance is one the vocabulary can name.
    fn draw_prompt(&mut self, rng: &mut Rng) {
        let current = self.prompt;
        let neck = neck();

        debug_assert!(
            neck.len() * ANSWERS.len() >= 2,
            "the drill must have at least two prompts to choose between or this loop cannot end"
        );

        loop {
            let (root, _) = neck[rng.below(neck.len())];
            let interval = ANSWERS[rng.below(ANSWERS.len())].interval;
            let candidate = Prompt { root, interval };

            if candidate != current {
                self.prompt = candidate;
                break;
            }
        }

        self.wrong.clear();
        self.correct = None;
        // Deaf to whatever is sounding *now*, whatever the reason the prompt was drawn —
        // advanced after a flash, skipped, or the screen entered. Read off the device
        // rather than set flat, so the state says what is true: entering the screen in
        // silence arms immediately, while a prompt drawn over a ringing answer waits for
        // it to stop. A flat `Deaf` would have been the same thing in the running app,
        // where the next poll is 10 ms away — and a lie for those 10 ms.
        self.gate = match self.listener.reading() {
            Some(_) => Gate::Deaf,
            None => Gate::Listening,
        };
    }

    /// The pitch class the prompt is asking for.
    ///
    /// `None` only if the root is off the neck, which `draw_prompt` cannot produce — the
    /// positions it chooses from are the ones that yielded a pitch class in the first place.
    fn target(&self) -> Option<PitchClass> {
        Some(
            self.prompt
                .root
                .pitch_class()?
                .transpose(self.prompt.interval.semitones()),
        )
    }

    /// Folds a released reading into the drill.
    ///
    /// The gate's three rows, and the only place the streak moves. Note the asymmetry: a
    /// reading that is *not* a pitch re-arms and is otherwise ignored, so silence and a
    /// strum cost nothing — which is what makes a fumble on the way to the answer free.
    pub(super) fn apply(&mut self, heard: Heard) {
        let Heard::Pitch(detection) = heard else {
            self.gate = Gate::Listening;
            return;
        };

        // A correct answer is on screen, so the drill is not asking anything. What is
        // sounding now is the tail of the note that scored.
        if self.correct.is_some() || self.gate == Gate::Deaf {
            return;
        }

        let Some((pitch, _)) = crate::music::pitch::Pitch::nearest(detection.hz()) else {
            return;
        };

        // Judged once, whatever happens next: a held string confirms the same reading on
        // every 10 ms tick, and this is what makes one unbroken note one answer.
        self.gate = Gate::Deaf;
        self.judge(pitch.pitch_class());
    }

    /// Scores a played pitch class against the prompt.
    ///
    /// The octave is thrown away deliberately. A root at the third fret of the low E asked
    /// for a minor seventh has no answer above it inside one octave that a hand can reach
    /// from where it is sitting, and an octave-exact rule would be drilling *where the
    /// octave is* — the Note Trainer's neck knowledge — on top of the interval it means to
    /// teach. The cost is that an octave error is never caught, which is the same trade
    /// the Interval Trainer's *Find it* already makes.
    fn judge(&mut self, played: PitchClass) {
        if self.target() == Some(played) {
            self.streak += 1;
            self.best_streak = self.best_streak.max(self.streak);
            // The prompt is *not* replaced here. It stands, with this answer lit, until
            // `advance` retires it.
            self.correct = Some(played);
        } else {
            if !self.wrong.contains(&played) {
                self.wrong.push(played);
            }
            self.streak = 0;
        }
    }

    /// Whether a correct answer is lit, which is the window the flash timer runs in.
    pub(super) fn is_flashing(&self) -> bool {
        self.correct.is_some()
    }

    /// Ends the flash and draws the next prompt.
    ///
    /// A no-op when nothing is being marked, so a tick that outlived its flash — one that
    /// crossed a skip, or landed after the screen was left — cannot retire a prompt the
    /// learner is still reading.
    pub(super) fn advance(&mut self, rng: &mut Rng) {
        if self.correct.is_some() {
            self.draw_prompt(rng);
        }
    }

    pub(super) fn skip(&mut self, rng: &mut Rng) {
        self.streak = 0;
        self.draw_prompt(rng);
    }

    /// Opening the screen: the best streak persists, the run does not.
    pub(super) fn enter(&mut self, rng: &mut Rng) {
        self.streak = 0;
        self.draw_prompt(rng);
    }
}

/// The marks on the neck.
///
/// The root is an unfilled ring, for the reason `note_trainer::prompt_marker` gives and
/// which `NoteMarker`'s missing `Default` exists to enforce — a fill is what this app uses
/// to say "you put this here", and the prompt is the opposite.
///
/// A heard pitch class is marked at *every* position carrying it, which is the honest
/// drawing of a rule that judges pitch classes: those are all the places the learner could
/// have played it, and the drill would have accepted any of them.
///
/// **The root's own position wins any collision.** A learner who plays the root's pitch
/// class back, or answers a unison prompt, would otherwise have the one position the prompt
/// depends on repainted as an answer. The other positions carrying that pitch class still
/// take the answer's colour, so the answer is visible without the prompt losing its anchor.
///
/// Every mark is unlabelled, and that is load-bearing rather than cosmetic: a label would
/// name a note, and the whole point of this drill is that it can be answered without one.
fn markers(drill: &PlayTheInterval) -> Vec<NoteMarker> {
    let root = drill.prompt.root;

    // The live reading, unless it has already been judged — judging is immediate, so what
    // survives this filter is a note heard while the gate was deaf: the previous answer
    // ringing into a fresh prompt, drawn as heard rather than as scored.
    let sounding = drill
        .listener
        .reading()
        .map(|(pitch, _)| pitch.pitch_class())
        .filter(|played| drill.correct != Some(*played) && !drill.wrong.contains(played));

    let colour_for = |pitch: PitchClass| -> Option<(Color, MarkerStyle)> {
        if drill.correct == Some(pitch) {
            Some((SUCCESS, MarkerStyle::Filled))
        } else if drill.wrong.contains(&pitch) {
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

            let (color, style) = colour_for(pitch)?;

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
        // No press handler: the answer arrives from the instrument, so there is no press
        // to ignore. The same arrangement the CAGED window's neck uses.
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
                streak_readout(drill.streak, drill.best_streak),
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

/// What the app is hearing, said as one of the four states — and, while a note is being
/// read, how far it sits from the pitch it was named as.
///
/// The offset is the one number on this screen, and it names nothing. The drill judges the
/// pitch the instrument actually sounds, so an instrument far enough out of tune is judged
/// on the note it has become; without this a learner whose guitar is half a semitone flat
/// is told only that they are wrong, over and over, with nothing to say why.
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

/// The two strings the line above draws, apart from the widgets that draw them.
///
/// Split out so the tests can assert on the words — that the four states read differently,
/// and that neither string ever names a note, which is the one claim this whole screen
/// rests on and the one a widget tree cannot be asked about.
fn heard_line(drill: &PlayTheInterval) -> (&'static str, String) {
    let status = match drill.listener.state() {
        State::NotStarted => "not listening",
        State::NoInput => "no input — this drill needs one",
        State::Silent => "listening, play the interval...",
        State::Unnameable => "one string at a time",
        State::Reading => "heard",
    };

    // `+ 0.0` because rounding keeps the sign it rounded away: a hair flat would otherwise
    // print "-0 cents", which reads as a fault in the reading.
    let offset = match drill.listener.reading() {
        Some((_, cents)) => format!("{:+.0} cents", cents.value().round() + 0.0),
        None => String::new(),
    };

    (status, offset)
}

/// The header row. One control: skip. There is no direction to swap — a learner cannot name
/// an interval by playing one — and no pool to widen.
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
    use iced::keyboard;

    /// An app sitting on the played drill with a reproducible prompt stream.
    ///
    /// `navigate_to` rather than `open`, so no capture device is claimed: every assertion
    /// below is about what the drill does with a reading, and `Message::Heard` delivers one
    /// without a device. The tests that are about the device say so by using `open`.
    fn play_app(seed: u64) -> App {
        let mut app = app_with_seed(seed);
        app.navigate_to(Screen::PlayTheInterval);
        app.play_the_interval.enter(&mut app.rng);
        app
    }

    /// A pitch, sent through as the confirmed reading the detector would have released.
    ///
    /// A pitch rather than a frequency, because what a test here means is "the learner
    /// played E2" and the hertz is an implementation detail of saying so. The clarity is
    /// above the gate because this is the *released* seam — whether a strum ever gets this
    /// far is `audio::detect`'s question, tested there against real waveforms.
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

    /// Some pitch sounding the prompt's answer. The lowest one on the neck, which is an
    /// arbitrary choice the drill is indifferent to — that indifference is itself asserted
    /// by `the_interval_is_correct_in_every_octave`.
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

    /// A pitch a semitone above the answer — near enough to be a plausible slip, and never
    /// the answer itself.
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

        // A held string confirms the same reading on every 10 ms tick. Fifty of them is
        // half a second of ringing, and it is one wrong answer.
        for _ in 0..50 {
            plays(&mut app, wrong);
        }

        assert_eq!(app.play_the_interval.wrong.len(), 1);
        assert_eq!(app.play_the_interval.streak, 0);
    }

    #[test]
    fn a_correct_note_held_does_not_score_twice() {
        let mut app = play_app(0x9a2);
        let right = correct_pitch(&app);

        for _ in 0..50 {
            plays(&mut app, right);
        }

        assert_eq!(app.play_the_interval.streak, 1);
    }

    /// The whole reason `Gate` exists, and the one behaviour that would break silently.
    #[test]
    fn a_ringing_answer_does_not_answer_the_next_prompt() {
        let mut app = play_app(0x9a3);
        let right = correct_pitch(&app);

        plays(&mut app, right);
        assert_eq!(app.play_the_interval.streak, 1);

        // The flash retires the prompt a second later — while the string is still ringing.
        let _ = app.update(Message::AdvancePlayPrompt);
        plays(&mut app, right);

        assert_eq!(
            app.play_the_interval.streak, 1,
            "the previous answer answered the next prompt"
        );
        assert!(
            app.play_the_interval.wrong.is_empty(),
            "and was not judged wrong either"
        );
        assert!(app.play_the_interval.correct.is_none());
    }

    #[test]
    fn a_skipped_prompts_note_does_not_carry_over() {
        let mut app = play_app(0x9a4);
        let sounding = wrong_pitch(&app);

        plays(&mut app, sounding);
        let _ = app.update(Message::SkipPlayPrompt);

        plays(&mut app, sounding);
        assert!(
            app.play_the_interval.wrong.is_empty(),
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

        // Deduplicated in `wrong`, so what proves it was judged twice is that the second
        // sounding was judged at all — the gate re-armed on the silence between them.
        assert_eq!(app.play_the_interval.wrong.len(), 1);
        assert_eq!(app.play_the_interval.gate, Gate::Deaf);
    }

    /// The re-arming path a slide gives for free: `Tracker` releases `Unpitched` the moment
    /// the pitch moves past its tolerance, so sliding from a wrong note to the right one
    /// needs no silence between them.
    #[test]
    fn a_slide_from_a_wrong_note_to_the_right_one_is_judged() {
        let mut app = play_app(0x9a6);
        let (wrong, right) = (wrong_pitch(&app), correct_pitch(&app));

        plays(&mut app, wrong);
        let _ = app.update(Message::Heard(Heard::Unpitched));
        plays(&mut app, right);

        assert_eq!(app.play_the_interval.streak, 1);
    }

    #[test]
    fn a_fumble_that_never_settles_costs_nothing() {
        let mut app = play_app(0x9a7);

        for _ in 0..20 {
            let _ = app.update(Message::Heard(Heard::Unpitched));
        }

        assert!(app.play_the_interval.wrong.is_empty());
        assert_eq!(app.play_the_interval.streak, 0);
        assert!(app.play_the_interval.correct.is_none());
    }

    #[test]
    fn notes_during_the_flash_are_ignored() {
        let mut app = play_app(0x9a8);
        let right = correct_pitch(&app);
        let wrong = wrong_pitch(&app);

        plays(&mut app, right);
        stops(&mut app);
        plays(&mut app, wrong);

        assert_eq!(app.play_the_interval.streak, 1, "the flash was interrupted");
        assert!(app.play_the_interval.wrong.is_empty());
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

            // A fresh app per octave, so each one is judged against the same prompt rather
            // than against whatever the previous answer advanced to.
            for pitch in carriers {
                let mut app = play_app(seed);
                plays(&mut app, pitch);

                assert_eq!(
                    app.play_the_interval.streak, 1,
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
        assert_eq!(app.play_the_interval.streak, 1);
    }

    #[test]
    fn a_neighbouring_pitch_class_is_wrong() {
        let mut app = play_app(0xb5);
        let wrong = wrong_pitch(&app);

        plays(&mut app, wrong);

        assert_eq!(app.play_the_interval.streak, 0);
        assert_eq!(app.play_the_interval.wrong, vec![wrong.pitch_class()]);
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

            assert!(prompt.root.string < super::super::NECK_STRINGS);
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
            assert_eq!(app.play_the_interval.streak, expected);

            let _ = app.update(Message::AdvancePlayPrompt);
            stops(&mut app);
        }

        assert_eq!(app.play_the_interval.best_streak, 4);
    }

    #[test]
    fn a_skip_resets_the_streak_and_the_best_survives_it() {
        let mut app = play_app(0xd2);

        let right = correct_pitch(&app);
        plays(&mut app, right);
        let _ = app.update(Message::AdvancePlayPrompt);
        stops(&mut app);

        let _ = app.update(Message::SkipPlayPrompt);

        assert_eq!(app.play_the_interval.streak, 0);
        assert_eq!(app.play_the_interval.best_streak, 1);
    }

    #[test]
    fn re_entering_starts_a_fresh_run_and_keeps_the_best() {
        let mut app = play_app(0xd3);

        let right = correct_pitch(&app);
        plays(&mut app, right);
        let _ = app.update(Message::AdvancePlayPrompt);
        stops(&mut app);
        assert_eq!(app.play_the_interval.best_streak, 1);

        app.play_the_interval.enter(&mut app.rng);

        assert_eq!(app.play_the_interval.streak, 0);
        assert_eq!(app.play_the_interval.best_streak, 1);
        assert!(app.play_the_interval.wrong.is_empty());
        assert!(app.play_the_interval.correct.is_none());
    }

    // ---- The marks ----

    #[test]
    fn a_heard_pitch_class_is_marked_everywhere_it_falls() {
        let mut app = play_app(0xe1);
        let wrong = wrong_pitch(&app);
        plays(&mut app, wrong);

        let root = app.play_the_interval.prompt.root;
        let marks = markers(&app.play_the_interval);

        for (position, pitch) in neck() {
            let marked = marks
                .iter()
                .find(|m| m.string == position.string && m.fret == position.fret);

            if position == root {
                assert_eq!(
                    marked.expect("the root is always marked").color,
                    ROOT_MARKER
                );
            } else if pitch == wrong.pitch_class() {
                assert_eq!(
                    marked.expect("a heard position is marked").color,
                    DANGER,
                    "{position:?} carries what was played and is not marked"
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

    /// The collision rule: the root's own position keeps the root's mark whatever else
    /// claims it, so a prompt can never lose the anchor the interval is measured from.
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

        // The other positions carrying it still take the answer's colour, so the answer is
        // visible without the prompt losing its anchor.
        assert!(
            marks.iter().any(|m| {
                (m.string, m.fret) != (root.string, root.fret) && m.color != ROOT_MARKER
            }),
            "the played pitch class was marked nowhere"
        );
    }

    /// The screen's headline claim, asserted where a widget tree cannot be asked: no mark
    /// carries a label, in any state the drill can reach.
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

    // ---- What the screen says it is hearing ----

    #[test]
    fn the_four_states_read_differently() {
        let mut app = app_with_seed(0xf1);
        app.open(Screen::PlayTheInterval);

        // `NotStarted` needs a screen that has not been entered, so it is read before the
        // device is opened rather than from here.
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
            // No device on this machine: all three collapse to the one state that matters,
            // and the drill says so rather than appearing to listen.
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

    /// A learner whose guitar is far out of tune is judged on the note it has become, so
    /// the offset is the one thing on screen that can explain why correct playing keeps
    /// reading as wrong.
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

        // `a` widens the Note Trainer's pool and `i` switches a notation; neither is
        // claimed here.
        for key in ["a", "i", "n", "f"] {
            press_into(&mut app, key, keyboard::Modifiers::empty());
        }

        assert_eq!(app.play_the_interval.prompt, before);
        assert_eq!(app.play_the_interval.streak, 0);
        assert!(!app.play_the_interval.listener().is_picking());
    }
}
