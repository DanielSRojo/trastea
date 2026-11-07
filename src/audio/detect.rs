//! Pitch detection: a window of samples in, a frequency out.
//!
//! McLeod's normalized square difference function with parabolic peak interpolation. Time
//! domain because an FFT bin at this window size is wider than a semitone at the low E.

/// The analysis window, and so the detector's latency (42.7 ms at 48 kHz).
///
/// A floor: it must hold two periods of the low E (582 samples each at 48 kHz).
pub const WINDOW: usize = 2048;

/// How far the window slides between analyses; the tracker's constants are tuned to it.
pub const HOP: usize = 512;

/// The lowest pitch searched for, below the low E so a slack string can be tuned up.
const MIN_HZ: f32 = 70.0;

/// The highest, above G5 (fifteenth fret, high E) to allow a bend past it.
const MAX_HZ: f32 = 900.0;

/// How close to the strongest peak a peak must come to be taken instead of it.
///
/// Lowering it biases toward harmonics, not away: the first peak clearing it is a shorter
/// period. Measured: 0.90 and 0.75 made a decaying open D jump an octave for far longer.
const PEAK_THRESHOLD: f32 = 0.97;

/// Below this RMS, a window is silence rather than a quiet note.
const LEVEL_GATE: f32 = 0.005;

/// How sure the detector must be before a reading is worth releasing.
///
/// Measured: held notes score 0.991–1.000 and strums at most 0.987, too thin a margin alone.
const CLARITY_GATE: f32 = 0.99;

/// How many consecutive windows must agree before a reading is released.
///
/// 32 ms of agreement at this [`HOP`], which a pluck's ~40 ms inharmonic attack cannot supply.
const STABLE_RUN: usize = 3;

/// How far apart consecutive readings may sit and still count as agreeing.
///
/// Generous so a fast bend (about 7 cents per window) gets through; octave jumps miss by 1200.
const STABLE_CENTS: f32 = 20.0;

/// What one window of audio sounded like.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Heard {
    /// Below the level gate.
    Silence,
    /// Loud enough, but with no periodicity worth reporting.
    Unpitched,
    Pitch(Detection),
}

/// A frequency and the confidence behind it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detection {
    hz: f32,
    /// 1.0 is a pure repeating waveform; a strummed chord sits near 0.5.
    clarity: f32,
}

impl Detection {
    /// Builds a reading directly, for tests that drive a screen without a device.
    #[cfg(test)]
    pub fn synthesized(hz: f32, clarity: f32) -> Detection {
        Detection { hz, clarity }
    }

    pub fn hz(self) -> f32 {
        self.hz
    }
}

/// Analyses one window with the guitar's range.
pub fn detect(samples: &[f32], sample_rate: f32) -> Heard {
    analyse(samples, sample_rate, MIN_HZ, MAX_HZ)
}

/// [`detect`] with the search range spelled out, so a test can put a known period exactly
/// on a boundary and check that it is refused.
fn analyse(samples: &[f32], sample_rate: f32, min_hz: f32, max_hz: f32) -> Heard {
    if rms(samples) < LEVEL_GATE {
        return Heard::Silence;
    }

    let min_lag = (sample_rate / max_hz) as usize;
    let max_lag = (sample_rate / min_hz) as usize;

    // The difference function needs two periods inside the window to see the longest one.
    if min_lag < 1 || max_lag <= min_lag || max_lag * 2 > samples.len() {
        return Heard::Unpitched;
    }

    let nsdf = nsdf(samples, max_lag);

    let Some((lag, clarity)) = pick_peak(&nsdf, min_lag, max_lag) else {
        return Heard::Unpitched;
    };

    Heard::Pitch(Detection {
        hz: sample_rate / lag,
        clarity,
    })
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    (sum / samples.len() as f64).sqrt() as f32
}

/// The normalized square difference function, McLeod & Wyvill 2005.
///
/// `2·r(τ) / m(τ)`, where `r` is the autocorrelation at lag τ and `m` the summed energy of
/// the two windows compared; normalizing makes it independent of loudness.
/// `f64` accumulators because thousands of products are summed here.
fn nsdf(samples: &[f32], max_lag: usize) -> Vec<f32> {
    (0..=max_lag)
        .map(|lag| {
            let (correlation, energy) = samples.iter().zip(samples.iter().skip(lag)).fold(
                (0.0f64, 0.0f64),
                |(correlation, energy), (&a, &b)| {
                    let (a, b) = (f64::from(a), f64::from(b));
                    (correlation + a * b, energy + a * a + b * b)
                },
            );

            if energy > 0.0 {
                (2.0 * correlation / energy) as f32
            } else {
                0.0
            }
        })
        .collect()
}

/// The period the window repeats at, and how sure of it we are.
///
/// Takes the first peak clearing `PEAK_THRESHOLD × the strongest`, not the strongest, so a
/// weak fundamental still beats a taller harmonic.
fn pick_peak(nsdf: &[f32], min_lag: usize, max_lag: usize) -> Option<(f32, f32)> {
    // skip the descent from `nsdf[0]`, which is the window matching itself
    let start = nsdf.iter().position(|&v| v < 0.0)?;

    let mut peaks: Vec<(usize, f32)> = Vec::new();
    let mut current: Option<(usize, f32)> = None;

    for (lag, &value) in nsdf.iter().enumerate().skip(start) {
        match current {
            // Inside a positive run: keep the tallest point of it.
            Some((_, best)) if value > 0.0 => {
                if value > best {
                    current = Some((lag, value));
                }
            }
            // The run ended; that run's tallest point is a peak.
            Some(peak) => {
                peaks.push(peak);
                current = None;
            }
            None if value > 0.0 => current = Some((lag, value)),
            None => {}
        }
    }
    peaks.extend(current);

    let strongest = peaks.iter().map(|&(_, v)| v).fold(f32::MIN, f32::max);
    if strongest <= 0.0 {
        return None;
    }

    let &(lag, clarity) = peaks
        .iter()
        .filter(|&&(lag, _)| (min_lag..=max_lag).contains(&lag))
        .find(|&&(_, value)| value >= PEAK_THRESHOLD * strongest)?;

    // a peak on either end of the search is the shoulder of one outside it
    if lag <= min_lag || lag >= max_lag {
        return None;
    }

    Some((lag as f32 + refine(nsdf, lag), clarity))
}

/// The sub-sample correction to a peak, by fitting a parabola through it and its neighbours.
///
/// Clamped to half a sample because a flat-topped peak drives the denominator toward zero.
fn refine(nsdf: &[f32], lag: usize) -> f32 {
    let (Some(&before), Some(&after)) = (nsdf.get(lag.wrapping_sub(1)), nsdf.get(lag + 1)) else {
        return 0.0;
    };

    let denominator = 2.0 * (2.0 * nsdf[lag] - before - after);
    if denominator.abs() < f32::EPSILON {
        return 0.0;
    }

    ((after - before) / denominator).clamp(-0.5, 0.5)
}

/// Holds readings over time and releases one only when clarity and agreement both confirm it.
///
/// Never averages, so a bend is followed without lag.
#[derive(Debug, Default)]
pub struct Tracker {
    /// Consecutive confident readings, most recent last, at most `STABLE_RUN` long.
    run: Vec<Detection>,
}

impl Tracker {
    pub fn new() -> Tracker {
        Tracker::default()
    }

    /// Folds one window's analysis into the run; `Heard::Pitch` here means confirmed.
    pub fn push(&mut self, heard: Heard) -> Heard {
        let Heard::Pitch(detection) = heard else {
            // so a stale reading cannot outlive its sound
            self.run.clear();
            return heard;
        };

        if detection.clarity < CLARITY_GATE {
            self.run.clear();
            return Heard::Unpitched;
        }

        // a jump beyond the tolerance is a new note
        if self
            .run
            .last()
            .is_some_and(|last| cents_between(last.hz, detection.hz).abs() > STABLE_CENTS)
        {
            self.run.clear();
        }

        self.run.push(detection);
        if self.run.len() > STABLE_RUN {
            self.run.remove(0);
        }

        if self.run.len() < STABLE_RUN {
            return Heard::Unpitched;
        }

        Heard::Pitch(detection)
    }
}

/// The distance between two frequencies, in cents.
fn cents_between(from: f32, to: f32) -> f32 {
    1200.0 * (to / from).log2()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: f32 = 48_000.0;

    const E2: f32 = 82.41;
    const A2: f32 = 110.00;
    const D3: f32 = 146.83;
    const G5: f32 = 783.99;

    /// A tone from a fundamental and harmonics, `harmonics[0]` being the fundamental's amplitude.
    fn tone(hz: f32, len: usize, harmonics: &[f32]) -> Vec<f32> {
        (0..len)
            .map(|i| {
                let t = i as f32 / SR;
                harmonics
                    .iter()
                    .enumerate()
                    .map(|(index, &amplitude)| {
                        amplitude * (TAU * hz * (index + 1) as f32 * t).sin()
                    })
                    .sum()
            })
            .collect()
    }

    fn sine(hz: f32, len: usize) -> Vec<f32> {
        tone(hz, len, &[0.5])
    }

    /// A guitar-ish string: a fundamental with a decaying harmonic series over it.
    fn plucked(hz: f32, len: usize) -> Vec<f32> {
        tone(hz, len, &[0.5, 0.3, 0.2, 0.12, 0.08, 0.05])
    }

    /// Several notes at once, as in a strum.
    fn chord(roots: &[f32], len: usize) -> Vec<f32> {
        roots
            .iter()
            .map(|&hz| plucked(hz, len))
            .fold(vec![0.0; len], |mut sum, voice| {
                for (total, sample) in sum.iter_mut().zip(voice) {
                    *total += sample;
                }
                sum
            })
    }

    /// Scales a signal's amplitude over its length, for attack and decay.
    fn shaped(samples: &[f32], envelope: impl Fn(f32) -> f32) -> Vec<f32> {
        samples
            .iter()
            .enumerate()
            .map(|(i, &s)| s * envelope(i as f32 / samples.len() as f32))
            .collect()
    }

    /// A tone whose frequency slides from `from` to `to` across its length.
    fn sweep(from: f32, to: f32, len: usize) -> Vec<f32> {
        let mut phase = 0.0f32;
        (0..len)
            .map(|i| {
                let hz = from + (to - from) * (i as f32 / len as f32);
                phase += TAU * hz / SR;
                0.5 * phase.sin()
            })
            .collect()
    }

    fn hz_of(heard: Heard) -> f32 {
        match heard {
            Heard::Pitch(detection) => detection.hz(),
            other => panic!("expected a pitch, got {other:?}"),
        }
    }

    fn cents_off(measured: f32, expected: f32) -> f32 {
        cents_between(expected, measured).abs()
    }

    #[test]
    fn the_generator_produces_the_period_it_claims() {
        // pins the generator independently of the detector
        let seconds = 0.5;
        let len = (SR * seconds) as usize;
        let samples = sine(A2, len);

        let crossings = samples
            .windows(2)
            .filter(|pair| pair[0] <= 0.0 && pair[1] > 0.0)
            .count();

        assert_eq!(crossings, (A2 * seconds) as usize);
    }

    #[test]
    fn a_synthesized_tone_is_measured_within_a_cent() {
        for hz in [80.0, 110.0, 196.0, 330.0, 440.0, 659.0, 800.0] {
            let measured = hz_of(detect(&sine(hz, WINDOW), SR));
            assert!(
                cents_off(measured, hz) < 1.0,
                "{hz} Hz measured as {measured} Hz, {} cents out",
                cents_off(measured, hz)
            );
        }
    }

    #[test]
    fn the_lowest_string_is_measurable_and_not_an_octave_out() {
        let measured = hz_of(detect(&plucked(E2, WINDOW), SR));

        assert!(
            cents_off(measured, E2) < 1.0,
            "the low E measured as {measured} Hz"
        );
        assert!(cents_off(measured, E2 * 2.0) > 100.0, "read an octave high");
        assert!(cents_off(measured, E2 * 3.0) > 100.0, "read a twelfth high");
    }

    #[test]
    fn the_top_of_the_neck_is_measurable() {
        let measured = hz_of(detect(&plucked(G5, WINDOW), SR));
        assert!(
            cents_off(measured, G5) < 1.0,
            "G5 measured as {measured} Hz"
        );
    }

    #[test]
    fn a_weak_fundamental_is_still_the_answer() {
        // the second harmonic at three times the fundamental, as on a low-string pickup
        let samples = tone(D3, WINDOW, &[0.15, 0.45, 0.25, 0.15]);
        let measured = hz_of(detect(&samples, SR));

        assert!(
            cents_off(measured, D3) < 2.0,
            "measured {measured} Hz where the fundamental is {D3} Hz"
        );
    }

    #[test]
    fn an_interpolated_peak_cannot_leave_the_sample_it_corrects() {
        // a flat top drives the parabola's denominator to zero
        let flat = [0.0, -0.1, 0.9, 0.9, 0.9, -0.1];
        for lag in 2..=4 {
            assert!(
                refine(&flat, lag).abs() <= 0.5,
                "a flat peak at {lag} shifted by {}",
                refine(&flat, lag)
            );
        }

        let leaning = [0.0, -0.1, 0.6, 0.9, 0.8, -0.1];
        let shift = refine(&leaning, 3);
        assert!(shift > 0.0 && shift <= 0.5, "shift was {shift}");
    }

    #[test]
    fn a_peak_at_the_edge_of_the_search_is_refused() {
        let samples = sine(A2, WINDOW);

        // A2's period sits exactly on each boundary in turn
        assert_eq!(analyse(&samples, SR, MIN_HZ, A2), Heard::Unpitched);

        assert_eq!(analyse(&samples, SR, A2, MAX_HZ), Heard::Unpitched);

        assert!(cents_off(hz_of(analyse(&samples, SR, MIN_HZ, MAX_HZ)), A2) < 1.0);
    }

    #[test]
    fn a_clean_tone_is_confident_and_a_chord_is_not() {
        let Heard::Pitch(single) = detect(&plucked(A2, WINDOW), SR) else {
            panic!("a single note should be measured");
        };
        assert!(single.clarity > CLARITY_GATE, "{}", single.clarity);

        // An open A chord: A2 E3 A3 C#4 E4.
        let strum = chord(&[110.0, 164.81, 220.0, 277.18, 329.63], WINDOW);
        let clarity = match detect(&strum, SR) {
            Heard::Pitch(detection) => detection.clarity,
            _ => 0.0,
        };
        assert!(
            clarity < CLARITY_GATE,
            "a strum scored {clarity}, at or above the gate"
        );
    }

    #[test]
    fn silence_is_reported_as_silence() {
        assert_eq!(detect(&vec![0.0; WINDOW], SR), Heard::Silence);

        let whisper = sine(A2, WINDOW)
            .iter()
            .map(|s| s * LEVEL_GATE * 0.5)
            .collect::<Vec<_>>();
        assert_eq!(detect(&whisper, SR), Heard::Silence);
    }

    #[test]
    fn the_same_window_always_gives_the_same_answer() {
        let samples = plucked(D3, WINDOW);
        assert_eq!(detect(&samples, SR), detect(&samples, SR));
    }

    /// Drives a tracker across a signal, one analysis per hop, as the app does.
    fn track(samples: &[f32], hop: usize) -> Vec<Heard> {
        let mut tracker = Tracker::new();
        (0..)
            .map(|step| step * hop)
            .take_while(|&start| start + WINDOW <= samples.len())
            .map(|start| tracker.push(detect(&samples[start..start + WINDOW], SR)))
            .collect()
    }

    fn released(readings: &[Heard]) -> Vec<f32> {
        readings
            .iter()
            .filter_map(|heard| match heard {
                Heard::Pitch(detection) => Some(detection.hz()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_held_note_is_released_and_stays_put() {
        let readings = track(&plucked(A2, WINDOW * 6), HOP);
        let hz = released(&readings);

        assert!(!hz.is_empty(), "a sustained note released nothing");
        for window in hz.windows(2) {
            assert!(
                cents_between(window[0], window[1]).abs() < 5.0,
                "a held note wandered from {} to {}",
                window[0],
                window[1]
            );
        }
    }

    #[test]
    fn a_strummed_chord_is_never_released() {
        let strum = chord(&[110.0, 164.81, 220.0, 277.18, 329.63], WINDOW * 8);
        let readings = track(&strum, HOP);

        assert!(
            released(&readings).is_empty(),
            "a strum released {} readings",
            released(&readings).len()
        );
    }

    #[test]
    fn the_attack_of_a_pluck_is_not_released() {
        // inharmonic content standing in for the attack of a struck string
        let attack = shaped(&chord(&[A2, 131.0, 301.0, 517.0], WINDOW * 2), |t| {
            1.0 - t * 0.5
        });
        let readings = track(&attack, 256);

        assert!(
            released(&readings).is_empty(),
            "an inharmonic attack released a reading"
        );
    }

    #[test]
    fn a_bend_is_followed_rather_than_stepped_through() {
        let from = D3;
        let to = D3 * 2.0f32.powf(2.0 / 12.0);
        let readings = track(&sweep(from, to, (SR * 2.0) as usize), HOP);
        let hz = released(&readings);

        assert!(hz.len() > 20, "a bend released only {} readings", hz.len());

        // monotonic within a window's jitter
        for window in hz.windows(2) {
            assert!(
                window[1] > window[0] - 1.0,
                "the bend went backwards: {} then {}",
                window[0],
                window[1]
            );
        }

        let travelled = cents_between(hz[0], hz[hz.len() - 1]);
        assert!(
            travelled > 150.0,
            "the bend only travelled {travelled} cents of its two semitones"
        );
    }

    #[test]
    fn a_new_note_replaces_the_previous_one() {
        let mut samples = plucked(A2, WINDOW * 4);
        samples.extend(plucked(D3, WINDOW * 4));

        let hz = released(&track(&samples, HOP));

        assert!(cents_off(hz[0], A2) < 5.0, "started on {}", hz[0]);
        assert!(
            cents_off(hz[hz.len() - 1], D3) < 5.0,
            "ended on {} rather than D3",
            hz[hz.len() - 1]
        );
    }

    #[test]
    fn silence_clears_a_reading_rather_than_holding_it() {
        let mut tracker = Tracker::new();
        let samples = plucked(A2, WINDOW);

        for _ in 0..STABLE_RUN {
            tracker.push(detect(&samples, SR));
        }
        assert!(matches!(
            tracker.push(detect(&samples, SR)),
            Heard::Pitch(_)
        ));

        assert_eq!(tracker.push(Heard::Silence), Heard::Silence);
        assert_eq!(tracker.push(detect(&samples, SR)), Heard::Unpitched);
    }
}
