//! Choosing among the positions that carry a pitch, by how far each one's predicted
//! inharmonicity sits from the measured one.

use std::collections::BTreeSet;

use crate::music::pitch::Pitch;

use super::calibration::Calibration;
use super::neck::{NECK_STRINGS, Neck, Position};

/// How near a candidate's prediction a measurement must sit to match it at all, in log₂ B.
/// Four times run 3's widest take-to-take spread (0.099), the spike's line for "build it";
/// tuned on a real guitar.
pub(super) const FIT_GATE: f32 = 0.4;

/// How far apart two candidates' predictions must sit to be told apart, in log₂ B. The same
/// starting point as [`FIT_GATE`], and tuned the same way.
pub(super) const MARGIN: f32 = 0.4;

/// What the estimate can say: three cases for the reason `Heard` has its own. A place, the
/// places it cannot split, and no place are different facts to a screen, and none is an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Estimate {
    Named(Position),
    /// Two or more, fewer than every candidate, in no order of preference.
    Narrowed(Vec<Position>),
    Unnamed,
}

/// A group the estimate will not split, known from the calibration before anything is played.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Group {
    pub(super) pitch: Pitch,
    pub(super) positions: Vec<Position>,
}

#[derive(Debug, Clone, Copy)]
struct Candidate {
    position: Position,
    predicted: f32,
}

/// Every position carrying `pitch`, with its predicted log₂ B, lowest prediction first.
fn candidates(pitch: Pitch, calibration: &Calibration, neck: &Neck) -> Vec<Candidate> {
    let mut found: Vec<Candidate> = neck
        .positions_of(pitch)
        .filter_map(|position| {
            Some(Candidate {
                position,
                predicted: calibration.predict_log2(position.string, position.fret)?,
            })
        })
        .collect();

    found.sort_by(|a, b| a.predicted.total_cmp(&b.predicted));
    found
}

/// The index of the candidate predicted nearest `measured`.
fn nearest(candidates: &[Candidate], measured: f32) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (a.predicted - measured)
                .abs()
                .total_cmp(&(b.predicted - measured).abs())
        })
        .map(|(index, _)| index)
}

/// The positions the estimate will not split from `candidates[best]`, ordered by string.
///
/// Linked through neighbours within [`MARGIN`] rather than measured from `best` alone, so the
/// groups partition the candidates and [`groups`] can list them before anything is played.
/// A flagged string's positions always join, since its prediction cannot rule them out.
fn group(candidates: &[Candidate], best: usize, calibration: &Calibration) -> Vec<Position> {
    let flagged = |candidate: &Candidate| calibration.is_flagged(candidate.position.string);

    let mut members: Vec<Position> = if candidates.get(best).is_some_and(flagged) {
        candidates.iter().map(|c| c.position).collect()
    } else {
        // `candidates` is sorted by prediction, so a group is a run broken by gaps over MARGIN
        let linked = |pair: &[Candidate]| pair[1].predicted - pair[0].predicted <= MARGIN;
        let below = candidates[..=best]
            .windows(2)
            .rev()
            .take_while(|pair| linked(pair))
            .count();
        let above = candidates[best..]
            .windows(2)
            .take_while(|pair| linked(pair))
            .count();

        candidates[best - below..=best + above]
            .iter()
            .chain(candidates.iter().filter(|c| flagged(c)))
            .map(|c| c.position)
            .collect()
    };

    members.sort_by_key(|position| (position.string, position.fret));
    members.dedup();
    members
}

/// Turns a group into what the screen is told, given how many candidates there were.
fn outcome(group: Vec<Position>, of: usize, best_flagged: bool) -> Estimate {
    match group.len() {
        // a flagged string is never named alone, even as the only candidate
        n if n == of && (n > 1 || best_flagged) => Estimate::Unnamed,
        1 => Estimate::Named(group[0]),
        _ => Estimate::Narrowed(group),
    }
}

/// Where on `neck` a note of `pitch` with this coefficient was played.
pub(super) fn estimate(
    pitch: Pitch,
    coefficient: f32,
    calibration: Option<&Calibration>,
    neck: &Neck,
) -> Estimate {
    let Some(calibration) = calibration else {
        return Estimate::Unnamed;
    };

    // a harmonic reading's log is −∞: distortion or a synthesizer, and no string
    let measured = coefficient.log2();
    if !measured.is_finite() {
        return Estimate::Unnamed;
    }

    let candidates = candidates(pitch, calibration, neck);
    let Some(best) = nearest(&candidates, measured) else {
        return Estimate::Unnamed;
    };

    if (candidates[best].predicted - measured).abs() > FIT_GATE {
        return Estimate::Unnamed;
    }

    let best_flagged = calibration.is_flagged(candidates[best].position.string);
    outcome(
        group(&candidates, best, calibration),
        candidates.len(),
        best_flagged,
    )
}

/// How far, signed and in log₂ B, the measurement sits from the nearest candidate's prediction,
/// whether or not it matched. What drift is judged on.
pub(super) fn miss(
    pitch: Pitch,
    coefficient: f32,
    calibration: &Calibration,
    neck: &Neck,
) -> Option<f32> {
    let measured = coefficient.log2();
    if !measured.is_finite() {
        return None;
    }

    let candidates = candidates(pitch, calibration, neck);
    let best = nearest(&candidates, measured)?;
    Some(measured - candidates[best].predicted)
}

/// Every group of two or more positions the estimate will answer together, per pitch, by the
/// same predictions and the same [`MARGIN`] as [`estimate`] rather than a second rule.
pub(super) fn groups(calibration: &Calibration, neck: &Neck) -> Vec<Group> {
    let pitches: BTreeSet<Pitch> = (0..NECK_STRINGS)
        .flat_map(|string| (0..=neck.frets()).filter_map(move |fret| neck.pitch_at(string, fret)))
        .collect();

    pitches
        .into_iter()
        .flat_map(|pitch| {
            let candidates = candidates(pitch, calibration, neck);
            let mut seen: Vec<Vec<Position>> = Vec::new();

            for best in 0..candidates.len() {
                let members = group(&candidates, best, calibration);
                if members.len() > 1 && !seen.contains(&members) {
                    seen.push(members);
                }
            }

            seen.into_iter()
                .map(move |positions| Group { pitch, positions })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::calibration::tests::spike_guitar;
    use crate::ui::neck::{DRILL_NECK, STANDARD_TUNING};

    fn at(string: usize, fret: usize) -> Position {
        Position { string, fret }
    }

    /// A calibration whose strings open at these log₂ B and rise as the model says.
    fn opening_at(open_log2: [f32; NECK_STRINGS]) -> Calibration {
        let open = open_log2.map(f32::exp2);
        Calibration::from_readings(open.map(Some), open.map(|b| Some(4.0 * b)))
            .expect("twelve positive readings")
    }

    /// The spike guitar's log₂ B₀, to adjust one string at a time.
    const SPIKE: [f32; NECK_STRINGS] = [-12.2, -13.0, -13.2, -13.09, -14.57, -16.0];

    fn e4() -> Pitch {
        STANDARD_TUNING[5]
    }

    fn a3() -> Pitch {
        STANDARD_TUNING[1].transpose(12).expect("A3")
    }

    fn predicted(calibration: &Calibration, position: Position) -> f32 {
        calibration
            .predict_log2(position.string, position.fret)
            .expect("on the neck")
    }

    fn estimate_at(calibration: &Calibration, pitch: Pitch, log2: f32) -> Estimate {
        estimate(pitch, log2.exp2(), Some(calibration), &DRILL_NECK)
    }

    #[test]
    fn one_candidate_well_clear_of_the_rest_is_named() {
        let guitar = spike_guitar();
        let measured = predicted(&guitar, at(4, 5)) + 0.1;

        assert_eq!(
            estimate_at(&guitar, e4(), measured),
            Estimate::Named(at(4, 5))
        );
    }

    #[test]
    fn two_close_candidates_and_a_distant_third_are_narrowed_to_the_two() {
        // the high E opens 0.2 above where B at the 5th predicts
        let mut open = SPIKE;
        open[5] = SPIKE[4] + 5.0 / 6.0 + 0.2;
        let guitar = opening_at(open);

        assert_eq!(
            estimate_at(&guitar, e4(), predicted(&guitar, at(4, 5))),
            Estimate::Narrowed(vec![at(4, 5), at(5, 0)])
        );
    }

    #[test]
    fn three_candidates_within_the_margin_name_nothing() {
        let b_at_5 = SPIKE[4] + 5.0 / 6.0;
        let mut open = SPIKE;
        open[5] = b_at_5 + 0.2;
        open[3] = b_at_5 + 0.1 - 9.0 / 6.0;
        let guitar = opening_at(open);

        assert_eq!(estimate_at(&guitar, e4(), b_at_5), Estimate::Unnamed);
    }

    #[test]
    fn a_measurement_far_from_every_candidate_names_nothing() {
        let guitar = spike_guitar();
        assert_eq!(estimate_at(&guitar, e4(), -20.0), Estimate::Unnamed);
        assert_eq!(
            estimate_at(&guitar, e4(), predicted(&guitar, at(4, 5)) + 3.0 * FIT_GATE),
            Estimate::Unnamed,
            "nearer B than the rest, but several gates from it"
        );
    }

    #[test]
    fn a_single_candidate_still_has_to_match() {
        let guitar = spike_guitar();
        let e2 = STANDARD_TUNING[0];
        let open = predicted(&guitar, at(0, 0));

        assert_eq!(
            estimate_at(&guitar, e2, open + FIT_GATE * 1.2),
            Estimate::Unnamed
        );
        assert_eq!(
            estimate_at(&guitar, e2, open + FIT_GATE * 0.8),
            Estimate::Named(at(0, 0))
        );
    }

    #[test]
    fn a_harmonic_coefficient_names_nothing() {
        assert_eq!(
            estimate(e4(), 0.0, Some(&spike_guitar()), &DRILL_NECK),
            Estimate::Unnamed
        );
    }

    #[test]
    fn a_runner_up_just_inside_the_margin_is_grouped_and_just_outside_is_not() {
        let b_at_5 = SPIKE[4] + 5.0 / 6.0;

        for (gap, expected) in [
            (MARGIN - 0.01, Estimate::Narrowed(vec![at(4, 5), at(5, 0)])),
            (MARGIN + 0.01, Estimate::Named(at(4, 5))),
        ] {
            let mut open = SPIKE;
            open[5] = b_at_5 + gap;
            let guitar = opening_at(open);

            assert_eq!(estimate_at(&guitar, e4(), b_at_5), expected, "gap {gap}");
        }
    }

    #[test]
    fn the_same_inputs_give_the_same_answer() {
        let guitar = spike_guitar();
        let measured = predicted(&guitar, at(3, 9));
        assert_eq!(
            estimate_at(&guitar, e4(), measured),
            estimate_at(&guitar, e4(), measured)
        );
    }

    #[test]
    fn without_a_calibration_nothing_is_named() {
        assert_eq!(estimate(e4(), 1e-4, None, &DRILL_NECK), Estimate::Unnamed);
    }

    #[test]
    fn a_flagged_strings_position_is_never_named_alone() {
        // B reads the same at the 12th fret as open
        let open = SPIKE.map(f32::exp2);
        let mut twelfth = open.map(|b| 4.0 * b);
        twelfth[4] = open[4];
        let guitar =
            Calibration::from_readings(open.map(Some), twelfth.map(Some)).expect("complete");
        assert!(guitar.is_flagged(4));

        let measured = predicted(&guitar, at(4, 5));
        assert!(
            !matches!(estimate_at(&guitar, e4(), measured), Estimate::Named(_)),
            "a position on the flagged B string was named alone"
        );

        // and a clean string's match keeps the flagged one in its group
        let on_g = predicted(&guitar, at(3, 9));
        assert_eq!(
            estimate_at(&guitar, e4(), on_g),
            Estimate::Narrowed(vec![at(3, 9), at(4, 5)])
        );
    }

    #[test]
    fn a_pair_the_calibration_cannot_split_is_grouped_before_anything_is_played() {
        // D at the 7th lands 0.33 below G at the 2nd
        let mut open = SPIKE;
        open[2] = -13.6;
        let guitar = opening_at(open);

        assert!(
            groups(&guitar, &DRILL_NECK).contains(&Group {
                pitch: a3(),
                positions: vec![at(2, 7), at(3, 2)],
            }),
            "D@7 and G@2 were not grouped at A3"
        );
    }

    #[test]
    fn the_same_pair_further_apart_is_not_grouped() {
        // the spike guitar's model puts D at the 7th 0.73 below G at the 2nd
        let grouped_at_a3 = groups(&spike_guitar(), &DRILL_NECK)
            .into_iter()
            .any(|group| group.pitch == a3());
        assert!(!grouped_at_a3);
    }

    #[test]
    fn every_predicted_group_is_what_the_estimate_answers_at_its_midpoint() {
        let mut open = SPIKE;
        open[2] = -13.6;
        let grouping = opening_at(open);

        // and one with a flagged string, whose positions join groups by a different rule
        let readings = open.map(f32::exp2);
        let mut twelfth = readings.map(|b| 4.0 * b);
        twelfth[4] = readings[4];
        let flagged =
            Calibration::from_readings(readings.map(Some), twelfth.map(Some)).expect("complete");

        for guitar in [spike_guitar(), grouping, flagged] {
            for Group { pitch, positions } in groups(&guitar, &DRILL_NECK) {
                let predictions: Vec<f32> = positions
                    .iter()
                    .filter(|p| !guitar.is_flagged(p.string))
                    .map(|&p| predicted(&guitar, p))
                    .collect();
                let (low, high) = predictions
                    .iter()
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &p| {
                        (lo.min(p), hi.max(p))
                    });
                let midpoint = if predictions.is_empty() {
                    predicted(&guitar, positions[0])
                } else {
                    f32::midpoint(low, high)
                };

                let carried = DRILL_NECK.positions_of(pitch).count();
                let expected = if positions.len() == carried {
                    Estimate::Unnamed
                } else {
                    Estimate::Narrowed(positions.clone())
                };

                assert_eq!(
                    estimate_at(&guitar, pitch, midpoint),
                    expected,
                    "{pitch:?}: the listed group {positions:?} is not what the estimate answers"
                );
            }
        }
    }

    #[test]
    fn a_miss_is_signed_from_the_nearest_prediction() {
        let guitar = spike_guitar();
        let b_at_5 = predicted(&guitar, at(4, 5));

        let above = miss(e4(), (b_at_5 + 0.3).exp2(), &guitar, &DRILL_NECK).expect("a miss");
        let below = miss(e4(), (b_at_5 - 0.3).exp2(), &guitar, &DRILL_NECK).expect("a miss");

        assert!((above - 0.3).abs() < 1e-4, "{above}");
        assert!((below + 0.3).abs() < 1e-4, "{below}");
        assert_eq!(miss(e4(), 0.0, &guitar, &DRILL_NECK), None);
    }
}
