//! Following one sounding note until it can be placed on the neck, and placing it once.
//! The stateful half of placement; `position` is the pure half it calls.

use crate::audio::detect::Heard;
use crate::audio::inharmonicity::Fit;
use crate::music::notes::PitchClass;
use crate::music::pitch::Pitch;

use super::calibration::Calibration;
use super::neck::{DRILL_NECK, Position};
use super::position::{Estimate, estimate};

/// How many accepted fits a note's coefficient is the median of. One window can still hold
/// the attack, and lighting a place and then moving it is what the screens promise not to do.
pub(super) const FITS_PER_NOTE: usize = 3;

/// Why a note was not placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Declined {
    Uncalibrated,
    /// Bent or slid into: the pitch of one fret with the inharmonicity of another.
    Slid,
    /// Every fit of it was refused.
    Unreadable,
}

/// What one note was settled into, once.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Placement {
    /// `coefficient` is the median it was estimated from, which is what drift is judged on.
    Placed {
        pitch: Pitch,
        estimate: Estimate,
        coefficient: f32,
    },
    Declined {
        pitch: Pitch,
        why: Declined,
    },
}

impl Placement {
    pub(super) fn pitch(&self) -> Pitch {
        match self {
            Placement::Placed { pitch, .. } | Placement::Declined { pitch, .. } => *pitch,
        }
    }

    /// The positions it names or narrows to; empty when it gives none.
    pub(super) fn positions(&self) -> &[Position] {
        match self {
            Placement::Placed {
                estimate: Estimate::Named(position),
                ..
            } => std::slice::from_ref(position),
            Placement::Placed {
                estimate: Estimate::Narrowed(positions),
                ..
            } => positions,
            Placement::Placed {
                estimate: Estimate::Unnamed,
                ..
            }
            | Placement::Declined { .. } => &[],
        }
    }
}

/// The note sounding now, gathering fits until it can be placed.
#[derive(Debug, Clone)]
struct Sounding {
    pitch: Pitch,
    slid: bool,
    fits: Vec<f32>,
    refused: bool,
    settled: bool,
}

impl Sounding {
    fn new(pitch: Pitch) -> Sounding {
        Sounding {
            pitch,
            slid: false,
            fits: Vec::new(),
            refused: false,
            settled: false,
        }
    }

    /// Places the note from whatever it has gathered, unless it is placed already or has
    /// nothing to be placed from.
    fn settle(&mut self, calibration: Option<&Calibration>) -> Option<Placement> {
        let decided = self.slid || calibration.is_none() || self.refused || !self.fits.is_empty();
        if self.settled || !decided {
            return None;
        }
        self.settled = true;

        let pitch = self.pitch;
        Some(match (calibration, median(&self.fits)) {
            _ if self.slid => Placement::Declined {
                pitch,
                why: Declined::Slid,
            },
            (None, _) => Placement::Declined {
                pitch,
                why: Declined::Uncalibrated,
            },
            (Some(_), None) => Placement::Declined {
                pitch,
                why: Declined::Unreadable,
            },
            (Some(kept), Some(coefficient)) => Placement::Placed {
                pitch,
                estimate: estimate(pitch, coefficient, Some(kept), &DRILL_NECK),
                coefficient,
            },
        })
    }
}

/// Places each note it hears once, by one rule for every screen that places notes.
#[derive(Debug, Clone, Default)]
pub(super) struct Placer {
    calibration: Option<Calibration>,
    sounding: Option<Sounding>,
}

impl Placer {
    pub(super) fn new(calibration: Option<Calibration>) -> Placer {
        Placer {
            calibration,
            sounding: None,
        }
    }

    pub(super) fn calibration(&self) -> Option<&Calibration> {
        self.calibration.as_ref()
    }

    /// Whether the note sounding now has been placed, rather than one heard before it.
    pub(super) fn has_placed_sounding(&self) -> bool {
        self.sounding
            .as_ref()
            .is_some_and(|sounding| sounding.settled)
    }

    /// Folds one released reading and its fit, and yields what it placed: the note that just
    /// ended or changed, and the note sounding now if that one could be placed on arrival.
    ///
    /// `use<>` because nothing yielded borrows the placer, so the caller may read it mid-loop.
    pub(super) fn hear(
        &mut self,
        heard: Heard,
        fit: Option<Fit>,
    ) -> impl Iterator<Item = Placement> + use<> {
        let calibration = self.calibration.as_ref();

        let Heard::Pitch(detection) = heard else {
            let ended = self
                .sounding
                .take()
                .and_then(|mut sounding| sounding.settle(calibration));
            return ended.into_iter().chain(None);
        };

        let Some((pitch, _)) = Pitch::nearest(detection.hz()) else {
            return None.into_iter().chain(None);
        };

        let ended = if self
            .sounding
            .as_ref()
            .is_none_or(|sounding| sounding.pitch != pitch)
        {
            self.sounding
                .replace(Sounding::new(pitch))
                .and_then(|mut sounding| sounding.settle(calibration))
        } else {
            None
        };

        let arrived = self.sounding.as_mut().and_then(|sounding| {
            sounding.slid |= detection.arrived_by_sliding();
            match fit {
                Some(Ok(measured)) => sounding.fits.push(measured.coefficient()),
                Some(Err(_)) => sounding.refused = true,
                None => {}
            }

            // a slide and a missing calibration decide the placement without any fit
            let ready =
                sounding.slid || calibration.is_none() || sounding.fits.len() >= FITS_PER_NOTE;
            ready.then(|| sounding.settle(calibration)).flatten()
        });

        ended.into_iter().chain(arrived)
    }
}

/// The middle reading, or `None` with nothing to take it from.
pub(super) fn median(values: &[f32]) -> Option<f32> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable_by(f32::total_cmp);
    sorted.get(sorted.len() / 2).copied()
}

/// Where each answer to one prompt was played, for a screen that draws answers there rather
/// than wherever their pitch class falls.
#[derive(Debug, Clone, Default)]
pub(super) struct Placed {
    kept: Vec<(PitchClass, Vec<Position>)>,
    /// The answer judged last, drawn wherever its pitch sounds until its placement narrows it.
    pending: Option<Pitch>,
}

impl Placed {
    /// An answer of `pitch` was judged and waits for its note to be placed. One still waiting
    /// is kept where its pitch sounds, since its note ended without a placement.
    pub(super) fn expect(&mut self, pitch: Pitch) {
        if let Some(unplaced) = self.pending.replace(pitch) {
            self.record_pitch(unplaced);
        }
    }

    /// Narrows the waiting answer to where this placement puts it, if the placement is its,
    /// and says whether it was.
    pub(super) fn place(&mut self, placement: &Placement) -> bool {
        let pitch = placement.pitch();
        if self.pending != Some(pitch) {
            return false;
        }
        self.pending = None;

        match placement.positions() {
            [] => self.record_pitch(pitch),
            positions => self.record(pitch.pitch_class(), positions),
        }
        true
    }

    /// Keeps where an answer of `pitch_class` was played, beside wherever it was played before.
    pub(super) fn record(&mut self, pitch_class: PitchClass, positions: &[Position]) {
        match self.kept.iter_mut().find(|(kept, _)| *kept == pitch_class) {
            Some((_, kept)) => {
                for position in positions {
                    if !kept.contains(position) {
                        kept.push(*position);
                    }
                }
            }
            None => self.kept.push((pitch_class, positions.to_vec())),
        }
    }

    /// Keeps a pitch that could not be placed at every position sounding it.
    fn record_pitch(&mut self, pitch: Pitch) {
        let positions: Vec<Position> = DRILL_NECK.positions_of(pitch).collect();
        self.record(pitch.pitch_class(), &positions);
    }

    pub(super) fn clear(&mut self) {
        self.kept.clear();
        self.pending = None;
    }

    /// Whether an answer of `pitch_class` is drawn at `position`: where it was played, and
    /// anywhere its pitch class falls only if no pitch of it was ever heard, as when pressed.
    pub(super) fn shows(&self, pitch_class: PitchClass, position: Position) -> bool {
        let pending = self
            .pending
            .filter(|pitch| pitch.pitch_class() == pitch_class);
        let kept = self.kept.iter().find(|(kept, _)| *kept == pitch_class);

        if kept.is_none() && pending.is_none() {
            return true;
        }

        kept.is_some_and(|(_, positions)| positions.contains(&position))
            || pending.is_some_and(|pitch| {
                DRILL_NECK.pitch_at(position.string, position.fret) == Some(pitch)
            })
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::audio::detect::Detection;
    use crate::audio::inharmonicity::{Inharmonicity, Refusal};
    use crate::ui::calibration::tests::{grouping_guitar, spike_guitar};
    use crate::ui::neck::STANDARD_TUNING;

    pub(in crate::ui) fn at(string: usize, fret: usize) -> Position {
        Position { string, fret }
    }

    pub(in crate::ui) fn heard(pitch: Pitch) -> Heard {
        Heard::Pitch(Detection::synthesized(pitch.frequency(), 0.999))
    }

    pub(in crate::ui) fn measured(log2: f32) -> Option<Fit> {
        Some(Ok(Inharmonicity::synthesized(log2.exp2())))
    }

    pub(in crate::ui) fn predicted(calibration: &Calibration, position: Position) -> f32 {
        calibration
            .predict_log2(position.string, position.fret)
            .expect("on the neck")
    }

    /// The pitch a position sounds, with the fit a clean pluck of it would give.
    pub(in crate::ui) fn pluck(calibration: &Calibration, position: Position) -> (Pitch, f32) {
        (
            DRILL_NECK
                .pitch_at(position.string, position.fret)
                .expect("on the neck"),
            predicted(calibration, position),
        )
    }

    fn e4() -> Pitch {
        STANDARD_TUNING[5]
    }

    /// Feeds readings in, and gathers everything placed along the way.
    fn feed(placer: &mut Placer, readings: &[(Heard, Option<Fit>)]) -> Vec<Placement> {
        readings
            .iter()
            .flat_map(|&(heard, fit)| placer.hear(heard, fit).collect::<Vec<_>>())
            .collect()
    }

    fn held(pitch: Pitch, log2: f32, fits: usize) -> Vec<(Heard, Option<Fit>)> {
        vec![(heard(pitch), measured(log2)); fits]
    }

    #[test]
    fn a_held_note_is_placed_once() {
        let guitar = spike_guitar();
        let (pitch, log2) = pluck(&guitar, at(5, 0));
        let mut placer = Placer::new(Some(guitar));

        let placed = feed(&mut placer, &held(pitch, log2, 3 * FITS_PER_NOTE));
        assert_eq!(placed.len(), 1, "{placed:?}");
        assert_eq!(placed[0].positions(), [at(5, 0)]);
        assert!(placer.has_placed_sounding());

        // and its end places nothing more
        assert!(feed(&mut placer, &[(Heard::Silence, None)]).is_empty());
    }

    #[test]
    fn one_stray_fit_does_not_decide_the_place() {
        let guitar = spike_guitar();
        let (pitch, log2) = pluck(&guitar, at(5, 0));
        let mut placer = Placer::new(Some(guitar));

        let mut readings = held(pitch, log2, FITS_PER_NOTE - 1);
        readings.insert(0, (heard(pitch), measured(-20.0)));

        let placed = feed(&mut placer, &readings);
        assert_eq!(placed.len(), 1);
        assert_eq!(placed[0].positions(), [at(5, 0)]);
    }

    #[test]
    fn a_note_slid_into_is_declined_whatever_its_fits_say() {
        let guitar = spike_guitar();
        let (pitch, log2) = pluck(&guitar, at(3, 8));
        let from = DRILL_NECK.pitch_at(3, 7).expect("on the neck").frequency();
        let mut placer = Placer::new(Some(guitar));

        let bent = Heard::Pitch(Detection::synthesized(pitch.frequency(), 0.999).slid_from(from));
        let placed = feed(&mut placer, &[(bent, measured(log2))]);

        assert_eq!(
            placed,
            vec![Placement::Declined {
                pitch,
                why: Declined::Slid
            }]
        );
    }

    #[test]
    fn without_a_calibration_a_note_is_declined_on_arrival() {
        let mut placer = Placer::new(None);

        let placed = feed(&mut placer, &[(heard(e4()), None)]);

        assert_eq!(
            placed,
            vec![Placement::Declined {
                pitch: e4(),
                why: Declined::Uncalibrated
            }]
        );
    }

    #[test]
    fn a_short_note_with_one_clean_fit_is_placed_from_it() {
        let guitar = spike_guitar();
        let (pitch, log2) = pluck(&guitar, at(5, 0));
        let mut placer = Placer::new(Some(guitar));

        assert!(feed(&mut placer, &held(pitch, log2, 1)).is_empty());
        let placed = feed(&mut placer, &[(Heard::Silence, None)]);

        assert_eq!(placed.len(), 1);
        assert_eq!(placed[0].positions(), [at(5, 0)]);
    }

    #[test]
    fn a_short_note_with_only_refusals_is_unreadable() {
        let mut placer = Placer::new(Some(spike_guitar()));

        let refused = Some(Err(Refusal::TooFewPartials));
        let placed = feed(
            &mut placer,
            &[(heard(e4()), refused), (Heard::Unpitched, None)],
        );

        assert_eq!(
            placed,
            vec![Placement::Declined {
                pitch: e4(),
                why: Declined::Unreadable
            }]
        );
    }

    #[test]
    fn a_note_too_short_to_fit_is_not_placed() {
        let mut placer = Placer::new(Some(spike_guitar()));

        let placed = feed(&mut placer, &[(heard(e4()), None), (Heard::Silence, None)]);

        assert!(placed.is_empty(), "{placed:?}");
    }

    #[test]
    fn a_note_ending_and_the_next_arriving_are_placed_in_one_reading() {
        let guitar = spike_guitar();
        let (e, open_e) = pluck(&guitar, at(5, 0));
        let mut placer = Placer::new(Some(guitar));
        assert!(feed(&mut placer, &held(e, open_e, 1)).is_empty());

        // the next note arrives bent, so it is declined at once, as the first is placed
        let d_sharp = DRILL_NECK.pitch_at(3, 8).expect("on the neck");
        let bent = Heard::Pitch(
            Detection::synthesized(d_sharp.frequency(), 0.999).slid_from(e.frequency()),
        );
        let placed: Vec<Placement> = placer.hear(bent, None).collect();

        assert_eq!(placed.len(), 2, "{placed:?}");
        assert_eq!(placed[0].pitch(), e);
        assert_eq!(placed[0].positions(), [at(5, 0)]);
        assert_eq!(
            placed[1],
            Placement::Declined {
                pitch: d_sharp,
                why: Declined::Slid
            }
        );
    }

    #[test]
    fn a_narrowed_placement_gives_every_position_it_cannot_split() {
        let guitar = grouping_guitar();
        let (pitch, log2) = pluck(&guitar, at(3, 2));
        let mut placer = Placer::new(Some(guitar));

        let placed = feed(&mut placer, &held(pitch, log2, FITS_PER_NOTE));

        assert_eq!(placed[0].positions(), [at(2, 7), at(3, 2)]);
    }

    #[test]
    fn a_pressed_answer_is_shown_everywhere_its_pitch_class_falls() {
        let a = PitchClass::new(9);
        let placed = Placed::default();
        assert!(placed.shows(a, at(1, 0)) && placed.shows(a, at(0, 5)));
    }

    #[test]
    fn a_heard_answer_is_shown_where_its_pitch_sounds_until_it_is_placed() {
        let guitar = spike_guitar();
        let (e4, open_e) = pluck(&guitar, at(5, 0));
        let sounding_e4: Vec<Position> = DRILL_NECK.positions_of(e4).collect();
        let shown = |placed: &Placed| -> Vec<Position> {
            crate::ui::neck::neck()
                .into_iter()
                .filter(|&(position, pc)| pc == e4.pitch_class() && placed.shows(pc, position))
                .map(|(position, _)| position)
                .collect()
        };

        let mut placed = Placed::default();
        placed.expect(e4);
        assert_eq!(shown(&placed), sounding_e4, "the octave was not kept");

        let mut placer = Placer::new(Some(guitar));
        let placements = feed(&mut placer, &held(e4, open_e, FITS_PER_NOTE));
        assert!(placed.place(&placements[0]));
        assert_eq!(shown(&placed), vec![at(5, 0)]);

        // and with nothing waiting, a placement is no answer's
        assert!(!placed.place(&placements[0]));
    }

    #[test]
    fn an_answer_that_cannot_be_placed_stays_where_its_pitch_sounds() {
        let mut placed = Placed::default();
        placed.expect(e4());
        assert!(placed.place(&Placement::Declined {
            pitch: e4(),
            why: Declined::Uncalibrated,
        }));

        assert!(placed.shows(PitchClass::new(4), at(5, 0)));
        assert!(placed.shows(PitchClass::new(4), at(4, 5)));
        assert!(
            !placed.shows(PitchClass::new(4), at(0, 0)),
            "E2 was drawn for an E4"
        );
    }

    #[test]
    fn an_answer_whose_note_was_never_placed_is_kept_when_the_next_is_heard() {
        let mut placed = Placed::default();
        placed.expect(e4());
        placed.expect(e4().transpose(1).expect("in range"));

        assert!(placed.shows(PitchClass::new(4), at(5, 0)));
        assert!(!placed.shows(PitchClass::new(4), at(0, 0)));
    }

    #[test]
    fn a_second_sounding_elsewhere_adds_its_place() {
        let a = PitchClass::new(9);
        let mut placed = Placed::default();
        placed.record(a, &[at(1, 0)]);
        assert!(placed.shows(a, at(1, 0)) && !placed.shows(a, at(0, 5)));

        placed.record(a, &[at(0, 5)]);
        assert!(placed.shows(a, at(0, 5)) && placed.shows(a, at(1, 0)));

        placed.clear();
        assert!(placed.shows(a, at(4, 10)));
    }
}
