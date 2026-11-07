//! The Position Finder: the screen that listens to one note and draws where on the neck it
//! was played, and the calibration walk that teaches it the guitar. State and views share a
//! module, as on every screen, so the state's fields stay private to the views that read them.

use iced::widget::{Space, column, container, row, text};
use iced::{Element, Length, Padding};

use crate::audio::detect::Heard;
use crate::audio::inharmonicity::Fit;
use crate::music::notes::Spelling;
use crate::music::pitch::Pitch;

use super::calibration::{self, Calibration, Drift};
use super::focus::FocusTarget;
use super::fretboard::{Fretboard, MarkerStyle, NoteMarker, fretboard};
use super::listener::{Listener, State, ui_device_line, ui_device_picker};
use super::neck::{DRILL_NECK, NECK_STRINGS, Position, string_name};
use super::position::{Estimate, Group, estimate, groups, miss};
use super::style::{BODY, INK, MUTE, ROOT_MARKER, card_container, control_button, control_label};
use super::{Message, note_label};

/// How many accepted fits a note's coefficient is the median of. One window can still hold
/// the attack, and lighting a place and then moving it is what the screen promises not to do.
const FITS_PER_NOTE: usize = 3;

/// Every string open and then at the twelfth fret, low E first.
const CALIBRATION_STEPS: usize = 2 * NECK_STRINGS;
const CALIBRATION_FRET: usize = 12;

/// Why a note's place was not given, for the line under its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Declined {
    Uncalibrated,
    /// Bent or slid into: the pitch of one fret with the inharmonicity of another.
    Slid,
    /// Every fit of it was refused.
    Unreadable,
}

/// What the screen answered for one note, settled once.
#[derive(Debug, Clone, PartialEq)]
struct Outcome {
    pitch: Pitch,
    estimate: Estimate,
    declined: Option<Declined>,
}

/// The note sounding now, gathering fits until it can be answered.
#[derive(Debug, Clone)]
struct Sounding {
    pitch: Pitch,
    slid: bool,
    fits: Vec<f32>,
    refused: bool,
    settled: bool,
}

/// What the walk has to say about the last thing played.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WalkNotice {
    /// The note ended before enough clean fits; it is asked for again.
    Refused,
    WrongNote {
        heard: Pitch,
    },
}

/// The calibration walk: twelve readings, kept only once all twelve are in.
#[derive(Debug, Clone, Default)]
struct Walk {
    step: usize,
    open: [Option<f32>; NECK_STRINGS],
    twelfth: [Option<f32>; NECK_STRINGS],
    fits: Vec<f32>,
    refused: bool,
    /// The pitch just recorded, whose ringing tail must not read as a wrong note.
    just_recorded: Option<Pitch>,
    notice: Option<WalkNotice>,
}

impl Walk {
    fn position(&self) -> Position {
        Position {
            string: self.step / 2,
            fret: if self.step.is_multiple_of(2) {
                0
            } else {
                CALIBRATION_FRET
            },
        }
    }

    fn expected(&self) -> Option<Pitch> {
        let Position { string, fret } = self.position();
        DRILL_NECK.pitch_at(string, fret)
    }

    /// Folds one reading into the walk, and returns the calibration once the twelfth is in.
    fn hear(&mut self, heard: Heard, fit: Option<Fit>) -> Option<Calibration> {
        let Heard::Pitch(detection) = heard else {
            if !self.fits.is_empty() || self.refused {
                self.notice = Some(WalkNotice::Refused);
            }
            self.fits.clear();
            self.refused = false;
            self.just_recorded = None;
            return None;
        };

        let (pitch, _) = Pitch::nearest(detection.hz())?;
        if self.just_recorded == Some(pitch) {
            return None;
        }

        if Some(pitch) != self.expected() {
            self.notice = Some(WalkNotice::WrongNote { heard: pitch });
            self.fits.clear();
            return None;
        }

        if matches!(self.notice, Some(WalkNotice::WrongNote { .. })) {
            self.notice = None;
        }

        match fit {
            // a harmonic reading has no logarithm to calibrate from, so it is asked for again
            Some(Ok(measured)) if measured.coefficient() > 0.0 => {
                self.fits.push(measured.coefficient());
            }
            Some(_) => self.refused = true,
            None => {}
        }

        if self.fits.len() < FITS_PER_NOTE {
            return None;
        }

        let reading = median(&self.fits);
        let Position { string, fret } = self.position();
        let slot = if fret == 0 {
            &mut self.open[string]
        } else {
            &mut self.twelfth[string]
        };
        *slot = reading;

        self.step += 1;
        self.fits.clear();
        self.refused = false;
        self.notice = None;
        self.just_recorded = Some(pitch);

        (self.step == CALIBRATION_STEPS)
            .then(|| Calibration::from_readings(self.open, self.twelfth))
            .flatten()
    }
}

enum Mode {
    Finding,
    Calibrating(Walk),
}

pub(super) struct PositionFinder {
    listener: Listener,
    calibration: Option<Calibration>,
    outcome: Option<Outcome>,
    /// Whether `outcome` is the note sounding now rather than the last one heard.
    live: bool,
    sounding: Option<Sounding>,
    drift: Drift,
    showing_groups: bool,
    mode: Mode,
}

impl PositionFinder {
    pub(super) fn new() -> PositionFinder {
        PositionFinder {
            listener: Listener::new().fitting(),
            calibration: None,
            outcome: None,
            live: false,
            sounding: None,
            drift: Drift::default(),
            showing_groups: false,
            mode: Mode::Finding,
        }
    }

    pub(super) fn listener(&self) -> &Listener {
        &self.listener
    }

    pub(super) fn listener_mut(&mut self) -> &mut Listener {
        &mut self.listener
    }

    /// Opening the screen: the kept calibration is read again, and nothing heard before
    /// carries over.
    pub(super) fn enter(&mut self) {
        self.calibration = calibration::kept();
        self.outcome = None;
        self.live = false;
        self.sounding = None;
        self.drift.clear();
        self.mode = Mode::Finding;
    }

    pub(super) fn is_calibrating(&self) -> bool {
        matches!(self.mode, Mode::Calibrating(_))
    }

    /// Starts the walk from its first step, whether or not a calibration is kept.
    pub(super) fn start_calibration(&mut self) {
        self.mode = Mode::Calibrating(Walk::default());
    }

    /// Leaves the walk, keeping whatever was kept before it began.
    pub(super) fn leave_calibration(&mut self) {
        self.mode = Mode::Finding;
    }

    pub(super) fn toggle_groups(&mut self) {
        self.showing_groups = !self.showing_groups;
    }

    /// Folds a released reading, and the fit that came with it if one was due.
    pub(super) fn apply(&mut self, heard: Heard, fit: Option<Fit>) {
        if let Mode::Calibrating(walk) = &mut self.mode {
            if let Some(calibration) = walk.hear(heard, fit) {
                self.finish_calibration(calibration);
            }
            return;
        }

        self.hear(heard, fit);
    }

    fn finish_calibration(&mut self, calibration: Calibration) {
        calibration::keep(&calibration);
        self.calibration = Some(calibration);
        self.drift.clear();
        self.outcome = None;
        self.live = false;
        self.sounding = None;
        self.mode = Mode::Finding;
    }

    fn hear(&mut self, heard: Heard, fit: Option<Fit>) {
        let Heard::Pitch(detection) = heard else {
            self.settle();
            self.sounding = None;
            self.live = false;
            return;
        };

        let Some((pitch, _)) = Pitch::nearest(detection.hz()) else {
            return;
        };

        if self
            .sounding
            .as_ref()
            .is_none_or(|sounding| sounding.pitch != pitch)
        {
            self.settle();
            self.live = false;
            self.sounding = Some(Sounding {
                pitch,
                slid: false,
                fits: Vec::new(),
                refused: false,
                settled: false,
            });
        }

        let uncalibrated = self.calibration.is_none();
        let Some(sounding) = self.sounding.as_mut() else {
            return;
        };

        sounding.slid |= detection.arrived_by_sliding();
        match fit {
            Some(Ok(measured)) => sounding.fits.push(measured.coefficient()),
            Some(Err(_)) => sounding.refused = true,
            None => {}
        }

        // a slide and a missing calibration decide the answer without any fit
        let ready = sounding.slid || uncalibrated || sounding.fits.len() >= FITS_PER_NOTE;
        if ready && !sounding.settled {
            self.settle();
            self.live = true;
        }
    }

    /// Answers the sounding note, once, from whatever it has gathered. A note with nothing
    /// to answer from leaves the previous answer up, so a pitch never shows without its place.
    fn settle(&mut self) {
        let Some(sounding) = self.sounding.as_mut() else {
            return;
        };

        let decided = sounding.slid
            || self.calibration.is_none()
            || sounding.refused
            || !sounding.fits.is_empty();
        if sounding.settled || !decided {
            return;
        }
        sounding.settled = true;

        let pitch = sounding.pitch;
        let (estimate, declined) = match (&self.calibration, median(&sounding.fits)) {
            _ if sounding.slid => (Estimate::Unnamed, Some(Declined::Slid)),
            (None, _) => (Estimate::Unnamed, Some(Declined::Uncalibrated)),
            (Some(_), None) => (Estimate::Unnamed, Some(Declined::Unreadable)),
            (Some(kept), Some(coefficient)) => {
                if let Some(missed) = miss(pitch, coefficient, kept, &DRILL_NECK) {
                    self.drift.record(missed);
                }
                (estimate(pitch, coefficient, Some(kept), &DRILL_NECK), None)
            }
        };

        self.outcome = Some(Outcome {
            pitch,
            estimate,
            declined,
        });
    }
}

/// The middle reading, or `None` with nothing to take it from.
fn median(values: &[f32]) -> Option<f32> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable_by(f32::total_cmp);
    sorted.get(sorted.len() / 2).copied()
}

fn pitch_name(pitch: Pitch) -> String {
    format!(
        "{}{}",
        Spelling::Sharps.spell(pitch.pitch_class()),
        pitch.octave()
    )
}

/// Where a position is, the way a guitarist would say it.
fn place_name(position: Position) -> String {
    let string = string_name(position.string).unwrap_or_default();
    match position.fret {
        0 => format!("{string} string, open"),
        fret => format!("{string} string, fret {fret}"),
    }
}

/// The strings a group sits on, as one phrase: "the D or G string".
fn strings_phrase(positions: &[Position]) -> String {
    let names: Vec<String> = positions
        .iter()
        .filter_map(|position| string_name(position.string))
        .collect();
    format!("the {} string", names.join(" or "))
}

/// The marks on the neck. A named position is one filled dot, a group is identical dots with
/// nothing to rank them, and an unnamed note lights nothing.
fn markers(finder: &PositionFinder) -> Vec<NoteMarker> {
    let label = |pitch: Pitch| Spelling::Sharps.spell(pitch.pitch_class()).to_string();

    if let Mode::Calibrating(walk) = &finder.mode {
        let Position { string, fret } = walk.position();
        return vec![NoteMarker {
            string,
            fret,
            label: walk.expected().map(label).unwrap_or_default(),
            color: ROOT_MARKER,
            style: MarkerStyle::Outlined,
        }];
    }

    let lit: Vec<Position> = match finder.outcome.as_ref().map(|outcome| &outcome.estimate) {
        Some(Estimate::Named(position)) => vec![*position],
        Some(Estimate::Narrowed(positions)) => positions.clone(),
        Some(Estimate::Unnamed) | None => Vec::new(),
    };

    let lit_label = finder
        .outcome
        .as_ref()
        .map(|outcome| label(outcome.pitch))
        .unwrap_or_default();
    let colour = if finder.live { INK } else { MUTE };

    let shown_groups = finder
        .showing_groups
        .then_some(())
        .and(finder.calibration.as_ref());
    let grouped = shown_groups
        .map(|kept| groups(kept, &DRILL_NECK))
        .unwrap_or_default();

    grouped
        .iter()
        .flat_map(|Group { pitch, positions }| {
            positions.iter().map(move |position| (*pitch, *position))
        })
        .filter(|(_, position)| !lit.contains(position))
        .map(|(pitch, position)| NoteMarker {
            string: position.string,
            fret: position.fret,
            label: label(pitch),
            color: BODY,
            style: MarkerStyle::Outlined,
        })
        .chain(lit.iter().map(|position| NoteMarker {
            string: position.string,
            fret: position.fret,
            label: lit_label.clone(),
            color: colour,
            style: MarkerStyle::Filled,
        }))
        .collect()
}

/// The line under the pitch: where it was played, or why that was not said.
fn answer_line(finder: &PositionFinder) -> String {
    let Some(outcome) = &finder.outcome else {
        return String::new();
    };

    let answer = match (&outcome.estimate, outcome.declined) {
        (Estimate::Named(position), _) => place_name(*position),
        (Estimate::Narrowed(positions), _) => {
            format!("{} — it cannot tell them apart", strings_phrase(positions))
        }
        (Estimate::Unnamed, Some(Declined::Uncalibrated)) => "not placed — no calibration".into(),
        (Estimate::Unnamed, Some(Declined::Slid)) => "bent or slid into — not placed".into(),
        (Estimate::Unnamed, Some(Declined::Unreadable)) => {
            "not placed — the strings did not read cleanly".into()
        }
        (Estimate::Unnamed, None) => "could not tell which string".into(),
    };

    if finder.live {
        answer
    } else {
        format!("{answer} · last note")
    }
}

/// What the screen is doing, as the Tuner says it.
fn status_line(finder: &PositionFinder) -> &'static str {
    match finder.listener.state() {
        State::NotStarted => "not listening",
        State::NoInput => "no input — this screen needs one",
        State::Silent => "listening, play a note...",
        State::Unnameable => "one string at a time",
        State::Reading => "hearing",
    }
}

/// What the screen has to say about its calibration, most pressing first.
fn notices(finder: &PositionFinder) -> Vec<String> {
    let Some(kept) = &finder.calibration else {
        return vec!["positions need a calibration — press c to capture one".into()];
    };

    let mut said = Vec::new();

    if finder.drift.is_stale() {
        said.push("the calibration looks stale — press c to capture it again".into());
    }

    for string in kept.flagged() {
        let name = string_name(string).unwrap_or_default();
        said.push(format!(
            "the {name} string did not rise across its frets as a string should — notes on it are never placed alone"
        ));
    }

    said
}

/// What the groups key shows in words beside the neck.
fn group_lines(finder: &PositionFinder) -> Vec<String> {
    let Some(kept) = &finder.calibration else {
        return Vec::new();
    };

    groups(kept, &DRILL_NECK)
        .iter()
        .map(|Group { pitch, positions }| {
            format!("{} — {}", pitch_name(*pitch), strings_phrase(positions))
        })
        .collect()
}

pub(super) fn ui_position_finder(
    finder: &PositionFinder,
    focused: FocusTarget,
) -> Element<'static, Message> {
    if finder.listener.is_picking() {
        return container(ui_device_picker(&finder.listener))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .padding(Padding::from([24, 24]))
            .into();
    }

    let neck = Fretboard {
        num_frets: DRILL_NECK.frets(),
        highlighted: markers(finder),
        ..Fretboard::default()
    };

    let card = match &finder.mode {
        Mode::Finding => ui_finding(finder, focused),
        Mode::Calibrating(walk) => ui_walk(finder, walk, focused),
    };

    container(
        row![
            fretboard(neck),
            container(card)
                .width(Length::Fill)
                .padding(32)
                .style(card_container)
        ]
        .spacing(32),
    )
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

const NAME_SIZE: f32 = 72.0;
const OCTAVE_SIZE: f32 = 28.0;

fn ui_finding(finder: &PositionFinder, focused: FocusTarget) -> Element<'static, Message> {
    let colour = if finder.live { INK } else { MUTE };

    let name: Element<'static, Message> = match &finder.outcome {
        Some(outcome) => row![
            note_label(
                Spelling::Sharps.spell(outcome.pitch.pitch_class()),
                NAME_SIZE,
                colour
            ),
            text(outcome.pitch.octave().to_string())
                .size(OCTAVE_SIZE)
                .color(colour),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(4)
        .into(),
        None => text("—").size(NAME_SIZE / 2.0).color(MUTE).into(),
    };

    let mut body = column![
        name,
        text(answer_line(finder)).size(18).color(BODY),
        text(status_line(finder)).size(13).color(MUTE),
        Space::new().height(Length::Fixed(12.0)),
        controls(finder, focused),
    ]
    .spacing(8);

    for notice in notices(finder) {
        body = body.push(text(notice).size(14).color(BODY));
    }

    if finder.showing_groups {
        let lines = group_lines(finder);
        body = body.push(
            text(if lines.is_empty() {
                "every position this calibration predicts can be told apart"
            } else {
                "answered as groups:"
            })
            .size(13)
            .color(MUTE),
        );
        for line in lines {
            body = body.push(text(line).size(13).color(BODY));
        }
    }

    body.push(Space::new().height(Length::Fill))
        .push(ui_device_line(&finder.listener, focused))
        .push(
            text(
                "positions are read from the strings this guitar was calibrated with, through a \
                 clean signal — a string change, a retuned string or distortion invalidates them",
            )
            .size(11)
            .color(MUTE),
        )
        .into()
}

fn controls(finder: &PositionFinder, focused: FocusTarget) -> Element<'static, Message> {
    row![
        control_button(
            control_label(text("calibrate"), 14.0),
            Message::Calibrate,
            focused == FocusTarget::Calibrate,
        ),
        control_button(
            control_label(
                text(if finder.showing_groups {
                    "groups: shown"
                } else {
                    "groups: hidden"
                }),
                14.0
            ),
            Message::TogglePositionGroups,
            focused == FocusTarget::PositionGroups,
        ),
    ]
    .spacing(8)
    .into()
}

fn ui_walk(
    finder: &PositionFinder,
    walk: &Walk,
    focused: FocusTarget,
) -> Element<'static, Message> {
    let Position { string, fret } = walk.position();
    let string = string_name(string).unwrap_or_default();
    let asked = match fret {
        0 => format!("play the open {string} string"),
        fret => format!("play the {string} string at the {fret}th fret"),
    };
    let expected = walk.expected().map(pitch_name).unwrap_or_default();

    let notice = match walk.notice {
        Some(WalkNotice::Refused) => "that did not read cleanly — play it again".to_owned(),
        Some(WalkNotice::WrongNote { heard }) => {
            format!("heard {} — this step wants {expected}", pitch_name(heard))
        }
        None => String::new(),
    };

    let leaving = if finder.calibration.is_some() {
        "Esc leaves the walk and keeps the calibration already kept"
    } else {
        "Esc leaves the walk without a calibration"
    };

    column![
        text(format!(
            "calibrating — {} of {CALIBRATION_STEPS}",
            walk.step + 1
        ))
        .size(14)
        .color(MUTE),
        text(asked).size(26).color(INK),
        text(expected).size(18).color(BODY),
        text(notice).size(14).color(BODY),
        text(status_line(finder)).size(13).color(MUTE),
        Space::new().height(Length::Fill),
        ui_device_line(&finder.listener, focused),
        text(leaving).size(11).color(MUTE),
    ]
    .spacing(8)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::detect::Detection;
    use crate::audio::inharmonicity::{Inharmonicity, Refusal};
    use crate::ui::calibration::tests::spike_guitar;
    use crate::ui::keys::{Fires, accelerators_for};
    use crate::ui::neck::STANDARD_TUNING;
    use crate::ui::tests::{press_into, press_named};
    use crate::ui::{App, Notation, Screen, WalkTheScale};
    use iced::keyboard;
    use iced::keyboard::key::Named;

    fn at(string: usize, fret: usize) -> Position {
        Position { string, fret }
    }

    fn e4() -> Pitch {
        STANDARD_TUNING[5]
    }

    fn a3() -> Pitch {
        STANDARD_TUNING[1].transpose(12).expect("A3")
    }

    /// The spike guitar with its D string opening low enough that D@7 and G@2 share a group.
    fn grouping_guitar() -> Calibration {
        let open = [-12.2f32, -13.0, -13.6, -13.09, -14.57, -16.0].map(f32::exp2);
        Calibration::from_readings(open.map(Some), open.map(|b| Some(4.0 * b))).expect("complete")
    }

    fn finder_with(calibration: Option<Calibration>) -> App {
        let (mut app, _) = App::new();
        let _ = app.update(Message::Navigate(Screen::PositionFinder));
        app.position_finder.calibration = calibration;
        app
    }

    fn heard(pitch: Pitch) -> Heard {
        Heard::Pitch(Detection::synthesized(pitch.frequency(), 0.999))
    }

    fn measured(log2: f32) -> Option<Fit> {
        Some(Ok(Inharmonicity::synthesized(log2.exp2())))
    }

    fn predicted(calibration: &Calibration, position: Position) -> f32 {
        calibration
            .predict_log2(position.string, position.fret)
            .expect("on the neck")
    }

    /// One plucked note, sustained long enough to be fitted as often as an answer needs.
    fn play(app: &mut App, pitch: Pitch, log2: f32) {
        for _ in 0..FITS_PER_NOTE {
            let _ = app.update(Message::Fitted(heard(pitch), measured(log2)));
        }
    }

    fn outcome(app: &App) -> Option<(Pitch, Estimate)> {
        app.position_finder
            .outcome
            .as_ref()
            .map(|outcome| (outcome.pitch, outcome.estimate.clone()))
    }

    // ---- Reaching it ----

    #[test]
    fn four_then_two_on_home_opens_the_position_finder() {
        let (mut app, _) = App::new();

        press_into(&mut app, "4", keyboard::Modifiers::empty());
        press_into(&mut app, "2", keyboard::Modifiers::empty());
        assert_eq!(app.screen, Screen::PositionFinder);

        press_named(&mut app, Named::Escape);
        assert_eq!(app.screen, Screen::Home, "Esc did not leave the screen");
    }

    #[test]
    fn the_screen_listens_and_fits() {
        let app = finder_with(None);
        assert!(
            app.listener_ref().is_some(),
            "the Position Finder does not listen"
        );
        assert!(app.position_finder.listener().is_fitting());
    }

    // ---- Answering ----

    #[test]
    fn a_named_position_is_lit_alone_with_its_pitch() {
        let guitar = spike_guitar();
        let open_e = predicted(&guitar, at(5, 0));
        let mut app = finder_with(Some(guitar));

        play(&mut app, e4(), open_e);

        assert_eq!(outcome(&app), Some((e4(), Estimate::Named(at(5, 0)))));
        assert!(app.position_finder.live);

        let lit = markers(&app.position_finder);
        assert_eq!(lit.len(), 1);
        assert_eq!((lit[0].string, lit[0].fret), (5, 0));
        assert_eq!(lit[0].style, MarkerStyle::Filled);
    }

    #[test]
    fn a_group_is_drawn_as_equals_and_the_rest_unlit() {
        let guitar = grouping_guitar();
        let g_at_2 = predicted(&guitar, at(3, 2));
        let mut app = finder_with(Some(guitar));

        play(&mut app, a3(), g_at_2);

        assert_eq!(
            outcome(&app),
            Some((a3(), Estimate::Narrowed(vec![at(2, 7), at(3, 2)])))
        );

        let lit = markers(&app.position_finder);
        let places: Vec<(usize, usize)> = lit.iter().map(|m| (m.string, m.fret)).collect();
        assert_eq!(places, vec![(2, 7), (3, 2)], "A at the 12th was marked too");
        assert!(
            lit.windows(2).all(|pair| pair[0].color == pair[1].color
                && pair[0].style == pair[1].style
                && pair[0].label == pair[1].label),
            "one marker of the group was drawn as likelier"
        );
    }

    #[test]
    fn a_note_it_cannot_place_is_named_and_lights_nothing() {
        let mut app = finder_with(Some(spike_guitar()));

        play(&mut app, e4(), -20.0);

        assert_eq!(outcome(&app), Some((e4(), Estimate::Unnamed)));
        assert!(markers(&app.position_finder).is_empty());
        assert!(!answer_line(&app.position_finder).is_empty());
    }

    #[test]
    fn a_pitch_waits_for_its_position() {
        let guitar = spike_guitar();
        let open_e = predicted(&guitar, at(5, 0));
        let mut app = finder_with(Some(guitar));
        play(&mut app, e4(), open_e);

        // the detector has released A3, and nothing has fitted it yet
        let _ = app.update(Message::Fitted(heard(a3()), None));
        assert_eq!(
            outcome(&app),
            Some((e4(), Estimate::Named(at(5, 0)))),
            "the new pitch was shown before its position"
        );

        // and from a fresh screen, nothing at all rather than a bare pitch
        let mut fresh = finder_with(Some(spike_guitar()));
        let _ = fresh.update(Message::Fitted(heard(a3()), None));
        let _ = fresh.update(Message::Fitted(heard(a3()), measured(-12.0)));
        assert_eq!(outcome(&fresh), None);
    }

    #[test]
    fn a_decaying_note_stays_lit_as_the_last_one() {
        let guitar = spike_guitar();
        let open_e = predicted(&guitar, at(5, 0));
        let mut app = finder_with(Some(guitar));
        play(&mut app, e4(), open_e);

        let _ = app.update(Message::Heard(Heard::Silence));

        assert_eq!(outcome(&app), Some((e4(), Estimate::Named(at(5, 0)))));
        assert!(!app.position_finder.live, "the answer still reads as live");
        assert!(answer_line(&app.position_finder).contains("last note"));
        assert_eq!(markers(&app.position_finder)[0].color, MUTE);
    }

    #[test]
    fn the_next_note_replaces_the_last() {
        let guitar = spike_guitar();
        let open_e = predicted(&guitar, at(5, 0));
        let g_at_9 = predicted(&guitar, at(3, 9));
        let low_e = predicted(&guitar, at(0, 0));
        let mut app = finder_with(Some(guitar));

        play(&mut app, e4(), open_e);
        let _ = app.update(Message::Heard(Heard::Silence));
        play(&mut app, STANDARD_TUNING[0], low_e);

        assert_eq!(
            outcome(&app),
            Some((STANDARD_TUNING[0], Estimate::Named(at(0, 0))))
        );
        assert_eq!(markers(&app.position_finder).len(), 1);

        // and a second E4 on another string is answered afresh
        let _ = app.update(Message::Heard(Heard::Silence));
        play(&mut app, e4(), g_at_9);
        assert_eq!(outcome(&app), Some((e4(), Estimate::Named(at(3, 9)))));
    }

    #[test]
    fn a_held_bend_is_declined_and_the_same_pitch_struck_is_placed() {
        let guitar = spike_guitar();
        // the G string's 7th fret bent up to the pitch of its 8th
        let g_at_7 = predicted(&guitar, at(3, 7));
        let g_at_8 = predicted(&guitar, at(3, 8));
        let d_sharp_4 = DRILL_NECK.pitch_at(3, 8).expect("on the neck");
        let from = DRILL_NECK.pitch_at(3, 7).expect("on the neck").frequency();
        let mut app = finder_with(Some(guitar));

        let bent =
            Heard::Pitch(Detection::synthesized(d_sharp_4.frequency(), 0.999).slid_from(from));
        for _ in 0..FITS_PER_NOTE {
            let _ = app.update(Message::Fitted(bent, measured(g_at_7)));
        }
        assert_eq!(outcome(&app), Some((d_sharp_4, Estimate::Unnamed)));
        assert_eq!(
            app.position_finder
                .outcome
                .as_ref()
                .and_then(|o| o.declined),
            Some(Declined::Slid)
        );

        let _ = app.update(Message::Heard(Heard::Silence));
        play(&mut app, d_sharp_4, g_at_8);
        assert_eq!(outcome(&app), Some((d_sharp_4, Estimate::Named(at(3, 8)))));
    }

    // ---- Listening states ----

    #[test]
    fn silence_before_any_note_lights_nothing() {
        let mut app = finder_with(Some(spike_guitar()));
        let opened = app.position_finder.listener().error().is_none();

        let _ = app.update(Message::Heard(Heard::Silence));

        assert_eq!(outcome(&app), None);
        assert!(markers(&app.position_finder).is_empty());
        if opened {
            assert_eq!(
                status_line(&app.position_finder),
                "listening, play a note..."
            );
        }
    }

    #[test]
    fn a_strum_names_and_lights_nothing() {
        let mut app = finder_with(Some(spike_guitar()));
        let opened = app.position_finder.listener().error().is_none();

        let _ = app.update(Message::Fitted(Heard::Unpitched, None));

        assert_eq!(outcome(&app), None);
        assert!(markers(&app.position_finder).is_empty());
        if opened {
            assert_eq!(status_line(&app.position_finder), "one string at a time");
        }
    }

    #[test]
    fn no_device_is_said_rather_than_looking_like_listening() {
        let app = finder_with(None);
        if app.position_finder.listener().error().is_some() {
            assert!(status_line(&app.position_finder).starts_with("no input"));
        } else {
            assert_ne!(
                app.position_finder.listener().state(),
                crate::ui::listener::State::NoInput
            );
        }
    }

    #[test]
    fn without_a_calibration_pitches_are_named_and_nothing_is_lit() {
        let mut app = finder_with(None);

        let _ = app.update(Message::Fitted(heard(a3()), None));

        assert_eq!(outcome(&app), Some((a3(), Estimate::Unnamed)));
        assert!(markers(&app.position_finder).is_empty());
        let said = notices(&app.position_finder);
        assert!(
            said.iter()
                .any(|line| line.contains("calibration") && line.contains('c')),
            "the screen did not say what it lacks: {said:?}"
        );
    }

    // ---- What the calibration says up front ----

    #[test]
    fn the_groups_are_visible_before_playing() {
        let mut app = finder_with(Some(grouping_guitar()));
        assert!(markers(&app.position_finder).is_empty());

        press_into(&mut app, "g", keyboard::Modifiers::empty());

        let shown: Vec<(usize, usize)> = markers(&app.position_finder)
            .iter()
            .map(|marker| (marker.string, marker.fret))
            .collect();
        assert!(shown.contains(&(2, 7)) && shown.contains(&(3, 2)));
        assert!(
            !shown.contains(&(1, 12)),
            "A at the 12th was drawn as grouped"
        );
        assert!(
            group_lines(&app.position_finder)
                .iter()
                .any(|line| line.starts_with("A3")),
            "A3 is not listed as a group"
        );
    }

    #[test]
    fn a_flagged_string_is_named_as_one_it_will_not_place_alone() {
        let open = [-12.2f32, -13.0, -13.2, -13.09, -14.57, -16.0].map(f32::exp2);
        let mut twelfth = open.map(|b| 4.0 * b);
        twelfth[2] = open[2];
        let app = finder_with(Calibration::from_readings(
            open.map(Some),
            twelfth.map(Some),
        ));

        let said = notices(&app.position_finder);
        assert!(
            said.iter().any(|line| line.contains("the D string")),
            "{said:?}"
        );
    }

    #[test]
    fn a_stale_calibration_is_said_and_recalibrating_offered() {
        let guitar = spike_guitar();
        // each a full margin's width from any rival, so the nearest prediction is its own
        let notes: Vec<(Pitch, f32)> = [at(0, 0), at(1, 3), at(1, 0), at(4, 1), at(5, 7)]
            .iter()
            .map(|&p| {
                (
                    DRILL_NECK.pitch_at(p.string, p.fret).expect("on the neck"),
                    predicted(&guitar, p) + 0.6,
                )
            })
            .collect();
        let mut app = finder_with(Some(guitar));

        for (pitch, log2) in notes {
            play(&mut app, pitch, log2);
            let _ = app.update(Message::Heard(Heard::Silence));
        }

        let said = notices(&app.position_finder);
        assert!(
            said.iter()
                .any(|line| line.contains("stale") && line.contains("press c")),
            "{said:?}"
        );
    }

    // ---- Calibrating ----

    /// Plays every step of the walk cleanly, each string reading `open` and four times it.
    fn walk_through(app: &mut App, open: [f32; NECK_STRINGS]) {
        for step in 0..CALIBRATION_STEPS {
            let (string, fret) = (step / 2, if step % 2 == 0 { 0 } else { 12 });
            let pitch = DRILL_NECK.pitch_at(string, fret).expect("on the neck");
            let b = if fret == 0 {
                open[string]
            } else {
                4.0 * open[string]
            };

            for _ in 0..FITS_PER_NOTE {
                let _ = app.update(Message::Fitted(
                    heard(pitch),
                    Some(Ok(Inharmonicity::synthesized(b))),
                ));
            }
            let _ = app.update(Message::Heard(Heard::Silence));
        }
    }

    #[test]
    fn a_complete_walk_replaces_the_kept_calibration() {
        let mut app = finder_with(Some(spike_guitar()));
        press_into(&mut app, "c", keyboard::Modifiers::empty());
        assert!(app.position_finder.is_calibrating());

        let open = [3e-4f32, 1.5e-4, 1e-4, 1.2e-4, 4e-5, 1.6e-5];
        walk_through(&mut app, open);

        assert!(
            !app.position_finder.is_calibrating(),
            "the walk did not end"
        );
        assert_eq!(
            app.position_finder.calibration,
            Calibration::from_readings(open.map(Some), open.map(|b| Some(4.0 * b)))
        );
    }

    #[test]
    fn a_refused_reading_is_asked_for_again() {
        let mut app = finder_with(None);
        press_into(&mut app, "c", keyboard::Modifiers::empty());

        for _ in 0..FITS_PER_NOTE {
            let _ = app.update(Message::Fitted(
                heard(STANDARD_TUNING[0]),
                Some(Err(Refusal::TooFewPartials)),
            ));
        }
        let _ = app.update(Message::Heard(Heard::Silence));

        let Mode::Calibrating(walk) = &app.position_finder.mode else {
            panic!("the walk ended");
        };
        assert_eq!(walk.step, 0, "the walk advanced past a refused reading");
        assert_eq!(walk.notice, Some(WalkNotice::Refused));
    }

    #[test]
    fn the_wrong_note_is_not_recorded_and_the_expected_one_is_named() {
        let mut app = finder_with(None);
        press_into(&mut app, "c", keyboard::Modifiers::empty());

        let f2 = STANDARD_TUNING[0].transpose(1).expect("F2");
        for _ in 0..FITS_PER_NOTE {
            let _ = app.update(Message::Fitted(heard(f2), measured(-12.0)));
        }

        let Mode::Calibrating(walk) = &app.position_finder.mode else {
            panic!("the walk ended");
        };
        assert_eq!(walk.step, 0);
        assert_eq!(walk.notice, Some(WalkNotice::WrongNote { heard: f2 }));
        assert_eq!(walk.expected(), Some(STANDARD_TUNING[0]));
    }

    #[test]
    fn an_abandoned_walk_keeps_the_old_calibration_and_esc_twice_leaves() {
        let kept = spike_guitar();
        let mut app = finder_with(Some(kept.clone()));
        press_into(&mut app, "c", keyboard::Modifiers::empty());

        // part way through
        let open = [3e-4f32, 1.5e-4, 1e-4, 1.2e-4, 4e-5, 1.6e-5];
        for _ in 0..FITS_PER_NOTE {
            let _ = app.update(Message::Fitted(
                heard(STANDARD_TUNING[0]),
                Some(Ok(Inharmonicity::synthesized(open[0]))),
            ));
        }

        press_named(&mut app, Named::Escape);
        assert_eq!(
            app.screen,
            Screen::PositionFinder,
            "Esc left the screen too"
        );
        assert!(!app.position_finder.is_calibrating());
        assert_eq!(app.position_finder.calibration, Some(kept));

        press_named(&mut app, Named::Escape);
        assert_eq!(app.screen, Screen::Home);
    }

    // ---- Keys ----

    #[test]
    fn the_overlay_lists_the_keys_the_screen_claims() {
        let claimed: Vec<(char, String)> = accelerators_for(
            &Screen::PositionFinder,
            Notation::Notes,
            &WalkTheScale::new(),
        )
        .into_iter()
        .map(|(key, _, label)| (key, label.into_owned()))
        .collect();

        assert_eq!(
            claimed,
            vec![
                ('d', "input device".to_owned()),
                ('c', "calibrate".to_owned()),
                ('g', "show the groups".to_owned()),
            ]
        );
    }

    #[test]
    fn every_key_acts_without_moving_the_ring() {
        let mut app = finder_with(Some(spike_guitar()));
        let ring = app.focused;

        press_into(&mut app, "g", keyboard::Modifiers::empty());
        assert!(app.position_finder.showing_groups);
        assert_eq!(app.focused, ring);

        press_into(&mut app, "c", keyboard::Modifiers::empty());
        assert!(app.position_finder.is_calibrating());
        assert_eq!(app.focused, ring);

        press_into(&mut app, "d", keyboard::Modifiers::empty());
        assert!(app.position_finder.listener().is_picking());
        assert_eq!(app.focused, ring);
    }

    #[test]
    fn every_claimed_key_fires_a_target_on_the_focus_grid() {
        let app = finder_with(None);
        let reachable = app.focusables();

        for (key, fires, _) in accelerators_for(
            &Screen::PositionFinder,
            Notation::Notes,
            &WalkTheScale::new(),
        ) {
            let Fires::Widget(target) = fires else {
                panic!("`{key}` fires a message no widget stands behind");
            };
            assert!(
                reachable.contains(&target),
                "`{key}`'s target is not focusable"
            );
        }
    }

    #[test]
    fn keys_claimed_elsewhere_are_inert_here() {
        let mut app = finder_with(Some(spike_guitar()));

        for key in ["r", "/", "[", "]"] {
            press_into(&mut app, key, keyboard::Modifiers::empty());

            assert_eq!(app.screen, Screen::PositionFinder, "`{key}` navigated");
            assert!(!app.position_finder.is_calibrating(), "`{key}` calibrated");
            assert!(
                !app.position_finder.showing_groups,
                "`{key}` showed the groups"
            );
            assert!(
                !app.position_finder.listener().is_picking(),
                "`{key}` opened the list"
            );
        }
    }
}
