//! The guitar in the room: each string's inharmonicity open and at the twelfth fret.
//! Instrument knowledge like the tuning beside it, but measured rather than tabulated.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use crate::audio::input::config_dir;

use super::neck::NECK_STRINGS;

/// What the model predicts across twelve frets, in log₂ B: B ∝ 1/L², and the twelfth fret
/// halves L, so B quadruples.
const MODEL_RISE: f32 = 2.0;

/// How far a string's measured rise may sit from [`MODEL_RISE`] and still be trusted. The
/// spike's Stratocaster B string rises about 2.6 on every take, so 0.5 flagged a sound string.
const RISE_TOLERANCE: f32 = 0.8;

/// How many confirmed notes drift is judged over.
const DRIFT_RUN: usize = 5;

/// The smallest miss, in log₂ B, that counts toward drift; a scatter under it is the pluck.
const DRIFT_MIN: f32 = 0.3;

/// How alike the misses must be, in log₂ B, to read as one factor rather than a scatter.
const DRIFT_SPREAD: f32 = 0.3;

/// Each string's coefficient open and at the twelfth fret: the second reading measures how
/// fast it rises up that string, which is the model, rather than checking the first.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Calibration {
    open: [f32; NECK_STRINGS],
    twelfth: [f32; NECK_STRINGS],
}

impl Calibration {
    /// Builds one only from all twelve readings, each a positive coefficient; a harmonic
    /// reading has no logarithm to rise from.
    pub(super) fn from_readings(
        open: [Option<f32>; NECK_STRINGS],
        twelfth: [Option<f32>; NECK_STRINGS],
    ) -> Option<Calibration> {
        let usable = |readings: [Option<f32>; NECK_STRINGS]| -> Option<[f32; NECK_STRINGS]> {
            readings
                .into_iter()
                .map(|reading| reading.filter(|b| b.is_finite() && *b > 0.0))
                .collect::<Option<Vec<f32>>>()?
                .try_into()
                .ok()
        };

        Some(Calibration {
            open: usable(open)?,
            twelfth: usable(twelfth)?,
        })
    }

    /// How far log₂ B rises across the string's twelve frets.
    fn rise(&self, string: usize) -> Option<f32> {
        Some((self.twelfth.get(string)? / self.open.get(string)?).log2())
    }

    /// `log₂ B(s, f) = log₂ B₀[s] + f · rise[s] / 12`, or `None` off the neck's strings.
    pub(super) fn predict_log2(&self, string: usize, fret: usize) -> Option<f32> {
        Some(self.open.get(string)?.log2() + fret as f32 * self.rise(string)? / 12.0)
    }

    /// Whether the string's rise is too far from the model's for its predictions to be trusted.
    pub(super) fn is_flagged(&self, string: usize) -> bool {
        self.rise(string)
            .is_some_and(|rise| (rise - MODEL_RISE).abs() > RISE_TOLERANCE)
    }

    pub(super) fn flagged(&self) -> impl Iterator<Item = usize> + '_ {
        (0..NECK_STRINGS).filter(|&string| self.is_flagged(string))
    }

    /// One line per string, open then twelfth. `f32`'s `Display` is the shortest text that
    /// reads back to the same value, so a record survives the round trip exactly.
    fn to_record(&self) -> String {
        self.open
            .iter()
            .zip(&self.twelfth)
            .map(|(open, twelfth)| format!("{open} {twelfth}\n"))
            .collect()
    }

    /// The inverse of [`Calibration::to_record`]; anything but twelve usable numbers is no record.
    fn from_record(record: &str) -> Option<Calibration> {
        let numbers: Vec<f32> = record
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<_, _>>()
            .ok()?;
        if numbers.len() != 2 * NECK_STRINGS {
            return None;
        }

        let column = |offset: usize| -> [Option<f32>; NECK_STRINGS] {
            std::array::from_fn(|string| numbers.get(2 * string + offset).copied())
        };

        Calibration::from_readings(column(0), column(1))
    }
}

/// Where the calibration is kept, beside the remembered device. `None` under test, so no test
/// reads the learner's record or replaces it.
fn kept_path() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }

    Some(config_dir()?.join("calibration"))
}

/// The calibration kept last time, or `None` if there is none that can be read.
pub(super) fn kept() -> Option<Calibration> {
    read(&kept_path()?)
}

/// Keeps a calibration for next time. Best effort, as for the device.
pub(super) fn keep(calibration: &Calibration) {
    if let Some(path) = kept_path() {
        write(&path, calibration);
    }
}

/// The two above with the path passed in, so tests need not `set_var` across threads.
fn read(path: &Path) -> Option<Calibration> {
    Calibration::from_record(&std::fs::read_to_string(path).ok()?)
}

fn write(path: &Path, calibration: &Calibration) {
    if let Some(directory) = path.parent() {
        let _ = std::fs::create_dir_all(directory);
    }

    let _ = std::fs::write(path, calibration.to_record());
}

/// The last few confirmed notes' misses from their nearest prediction, in log₂ B, signed.
/// Strings age and a retuned string moves every coefficient together, so a run that misses
/// by one factor is the calibration going stale rather than the playing going wrong.
#[derive(Debug, Default)]
pub(super) struct Drift {
    misses: VecDeque<f32>,
}

impl Drift {
    pub(super) fn record(&mut self, miss: f32) {
        self.misses.push_back(miss);
        if self.misses.len() > DRIFT_RUN {
            self.misses.pop_front();
        }
    }

    pub(super) fn clear(&mut self) {
        self.misses.clear();
    }

    pub(super) fn is_stale(&self) -> bool {
        if self.misses.len() < DRIFT_RUN {
            return false;
        }

        let lowest = self.misses.iter().copied().fold(f32::INFINITY, f32::min);
        let highest = self
            .misses
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);

        // one direction, far enough to matter, and close enough together to be one factor
        let one_way = lowest >= DRIFT_MIN || highest <= -DRIFT_MIN;
        one_way && highest - lowest <= DRIFT_SPREAD
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// A calibration whose strings each rise by exactly the model's two octaves of log₂ B,
    /// opening at the spike's measured coefficients.
    pub(in crate::ui) fn spike_guitar() -> Calibration {
        let open = [-12.2f32, -13.0, -13.2, -13.09, -14.57, -16.0].map(f32::exp2);
        Calibration::from_readings(open.map(Some), open.map(|b| Some(4.0 * b)))
            .expect("twelve positive readings")
    }

    /// The spike guitar with its D string opening low enough that D@7 and G@2 share a group.
    pub(in crate::ui) fn grouping_guitar() -> Calibration {
        let open = [-12.2f32, -13.0, -13.6, -13.09, -14.57, -16.0].map(f32::exp2);
        Calibration::from_readings(open.map(Some), open.map(|b| Some(4.0 * b))).expect("complete")
    }

    fn uniform(open: f32, twelfth: f32) -> Option<Calibration> {
        Calibration::from_readings([Some(open); 6], [Some(twelfth); 6])
    }

    #[test]
    fn an_incomplete_set_builds_no_calibration() {
        let mut twelfth = [Some(4e-4f32); 6];
        twelfth[3] = None;

        assert_eq!(Calibration::from_readings([Some(1e-4); 6], twelfth), None);
        assert!(uniform(1e-4, 4e-4).is_some());
    }

    #[test]
    fn a_harmonic_reading_builds_no_calibration() {
        assert_eq!(uniform(0.0, 4e-4), None);
        assert_eq!(uniform(1e-4, f32::NAN), None);
    }

    #[test]
    fn a_string_that_quadruples_is_accepted_and_predicts_half_way_up() {
        let b0 = 1e-4f32;
        let calibration = uniform(b0, 4.0 * b0).expect("complete");

        assert!(!calibration.is_flagged(2));
        let predicted = calibration.predict_log2(2, 6).expect("on the neck");
        assert!(
            (predicted - (2.0 * b0).log2()).abs() < 1e-5,
            "the 6th fret predicted {} rather than log₂ 2·B₀",
            predicted.exp2()
        );
    }

    #[test]
    fn a_steep_plain_string_is_trusted_and_a_steeper_one_is_flagged() {
        let b0 = 1e-4f32;

        assert!(
            !uniform(b0, b0 * 2.6f32.exp2())
                .expect("complete")
                .is_flagged(4)
        );
        assert!(
            uniform(b0, b0 * 2.9f32.exp2())
                .expect("complete")
                .is_flagged(4)
        );
    }

    #[test]
    fn a_string_that_does_not_rise_is_flagged() {
        let calibration = uniform(1e-4, 1e-4).expect("complete");

        assert!(calibration.is_flagged(0));
        assert_eq!(calibration.flagged().count(), NECK_STRINGS);
        assert_eq!(spike_guitar().flagged().count(), 0);
    }

    #[test]
    fn a_prediction_off_the_strings_is_none() {
        assert_eq!(spike_guitar().predict_log2(NECK_STRINGS, 0), None);
    }

    fn temporary(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "trastea-calibration-{name}-{}-{:?}.txt",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    #[test]
    fn a_kept_calibration_reads_back_the_same() {
        let path = temporary("round-trip");
        let calibration = spike_guitar();

        write(&path, &calibration);
        assert_eq!(read(&path), Some(calibration));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn an_unreadable_record_is_no_calibration() {
        let path = temporary("unreadable");
        let _ = std::fs::remove_file(&path);
        assert_eq!(read(&path), None, "a missing file read as a calibration");

        let whole = spike_guitar().to_record();
        let truncated: String = whole.lines().take(4).collect::<Vec<_>>().join("\n");
        for broken in [truncated.as_str(), "not a calibration", "", "1e-4 4e-4 x"] {
            std::fs::write(&path, broken).expect("the temporary directory is writable");
            assert_eq!(read(&path), None, "{broken:?} read as a calibration");
        }

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn no_test_touches_the_learners_record() {
        assert_eq!(kept_path(), None);
    }

    #[test]
    fn misses_by_one_factor_are_drift() {
        let mut drift = Drift::default();
        for miss in [0.62, 0.55, 0.7, 0.6, 0.58] {
            assert!(!drift.is_stale(), "stale before the run was complete");
            drift.record(miss);
        }

        assert!(drift.is_stale());
    }

    #[test]
    fn scattered_misses_are_not_drift() {
        let mut drift = Drift::default();
        for miss in [0.6, -0.5, 0.1, 0.9, -0.3, 0.4, -0.7] {
            drift.record(miss);
            assert!(!drift.is_stale(), "a scatter read as drift");
        }

        // one way, but too unlike one another to be one factor
        let mut spread = Drift::default();
        for miss in [0.35, 1.4, 0.5, 2.0, 0.9] {
            spread.record(miss);
        }
        assert!(!spread.is_stale());
    }

    #[test]
    fn a_small_consistent_miss_is_the_pluck_not_drift() {
        let mut drift = Drift::default();
        for _ in 0..DRIFT_RUN {
            drift.record(0.1);
        }
        assert!(!drift.is_stale());
    }
}
