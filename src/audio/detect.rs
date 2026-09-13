//! Pitch detection: a window of samples in, a frequency out.
//!
//! McLeod's normalized square difference function, peaks picked by the threshold rule,
//! parabolic interpolation on the winner. Time domain rather than an FFT, for two reasons
//! that are both about the low E. An FFT at 48 kHz over 4096 samples resolves 11.7 Hz per
//! bin, where E2 and F2 are 4.9 Hz apart — it cannot separate adjacent semitones down
//! there, let alone the cent a tuner wants, which at E2 is 0.048 Hz. And peak-picking a
//! spectrum returns whichever partial is loudest, which on a guitar pickup is routinely
//! the second harmonic rather than the fundamental.
//!
//! It is also affordable: the difference function over this window is one to two million
//! multiply-adds, roughly ninety times a second, which is low single-digit percent of one
//! core. No FFT is needed, so no FFT crate is either.
//!
//! Every constant below was measured against a guitar through an audio interface rather
//! than taken from a paper; where a figure has a reason, the reason is beside it.

// The screen is the caller and lands in the same change; until it does, this module is
// reached only from its own tests. `expect` rather than `allow` so the attribute fails the
// build of its own accord once that caller exists.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "detection precedes the tuner screen that reads it"
    )
)]

/// The analysis window.
///
/// This is a floor rather than a preference. The difference function needs two periods of
/// the lowest pitch it must measure, and the open low E at 82.41 Hz is 582 samples at
/// 48 kHz — so 1165 samples is the hard minimum and 1024 would lose that string entirely.
/// 2048 gives three and a half periods of it.
///
/// It is therefore also the detector's latency: 42.7 ms at 48 kHz, before anything else.
/// That number is physics, not a tuning parameter to be lowered until the low E breaks.
pub const WINDOW: usize = 2048;

/// The lowest pitch searched for. The open low E is 82.41 Hz; the margin below it is for a
/// string being tuned up from slack, which is exactly when a tuner is wanted.
const MIN_HZ: f32 = 70.0;

/// The highest. The fifteenth fret of the high E — the top of the longest neck drawn — is
/// G5 at 783.99 Hz, and the margin above is for a note bent past it.
const MAX_HZ: f32 = 900.0;

/// How close to the strongest peak a peak must come to be taken instead of it.
///
/// The direction here is counter-intuitive and worth stating, because it is the constant
/// most likely to be "fixed" by someone reasoning from the wrong intuition. Peak selection
/// takes the *first* peak clearing this fraction of the strongest one. Lowering it accepts
/// an earlier peak — a shorter period, a *higher* frequency — so a lower threshold biases
/// toward harmonics rather than away from them.
///
/// Measured against a recorded guitar: at 0.90 a decaying open D jumped an octave for
/// 0.39 s; at 0.75 it did so for 0.71 s and the low E stopped being detected at all. At
/// 0.97 the jump lasted 0.15 s and the boundary artefacts disappeared.
const PEAK_THRESHOLD: f32 = 0.97;

/// Below this RMS, a window is silence rather than a quiet note.
const LEVEL_GATE: f32 = 0.005;

/// How sure the detector must be before a reading is worth releasing.
///
/// Measured: held notes score 0.996 to 1.000, and the best frame of a strummed chord
/// reached 0.987 across 237 strum frames. This separates them — but by 0.003, and quiet
/// legitimate notes reach down to about 0.991. That margin is too thin to rest on, which
/// is why [`Tracker`] gates on agreement as well.
const CLARITY_GATE: f32 = 0.99;

/// How many consecutive windows must agree before a reading is released.
///
/// Three at a 512-sample hop is 32 ms of agreement, which the attack of a plucked note —
/// inharmonic for its first ~40 ms — cannot supply.
const STABLE_RUN: usize = 3;

/// How far apart consecutive readings may sit and still count as agreeing.
///
/// Deliberately generous. Its job is not to reject strums, which the clarity gate already
/// does; it is to reject the attack transient and the octave jump, which miss by hundreds
/// or by 1200 cents. Generous is what lets a fast bend through: a whole tone in 0.3 s
/// moves about 7 cents between windows, and a held note jitters by under 2.
const STABLE_CENTS: f32 = 20.0;

/// What one window of audio sounded like.
///
/// Three cases rather than an `Option`, because "heard nothing" and "heard something I
/// will not name" are different facts to the screen: one means the room is quiet and the
/// other means it is not listening hard enough, or that a chord was strummed at it.
///
/// [`Tracker::push`] returns this same vocabulary one stage later — what a window sounded
/// like, versus what is worth showing — because the three cases are the same three.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Heard {
    /// Below the level gate.
    Silence,
    /// Loud enough, but with no periodicity worth reporting.
    Unpitched,
    /// A frequency, and how sure the detector is of it.
    Pitch(Detection),
}

/// A frequency and the confidence behind it.
///
/// Deliberately not a note. Naming this needs `music::pitch`, which this module does not
/// import and does not want to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detection {
    hz: f32,
    clarity: f32,
}

impl Detection {
    /// Builds a reading directly, for tests elsewhere in the tree that need to drive a
    /// screen without a device. Not `pub` beyond the crate and not used by anything that
    /// runs: a `Detection` in the app itself always comes from analysing audio.
    #[cfg(test)]
    pub fn synthesized(hz: f32, clarity: f32) -> Detection {
        Detection { hz, clarity }
    }

    pub fn hz(self) -> f32 {
        self.hz
    }

    /// How close the normalized difference function came to a perfect match at the period
    /// it chose: 1.0 is a pure repeating waveform, and a strummed chord sits near 0.5.
    pub fn clarity(self) -> f32 {
        self.clarity
    }
}

/// Analyses one window with the guitar's range.
///
/// Pure: the same samples always give the same answer, which is what lets every test here
/// run on synthesized audio with no device present.
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
/// the two windows being compared. The normalization is what makes the result independent
/// of how loud the note is, so one threshold works for a struck string and a decaying one.
///
/// The accumulators are `f64` while everything around them is `f32`: this is the one place
/// where thousands of products are summed, and the window's worth of rounding is free to
/// avoid here even though the result is rounded back down immediately.
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
/// The first peak clearing `PEAK_THRESHOLD × the strongest peak` rather than the strongest
/// peak outright. That rule is the whole of the octave robustness: a harmonic produces a
/// taller peak at a shorter period whenever the fundamental is weak, and taking the first
/// one over the threshold prefers the longer period — the fundamental — while the
/// threshold stops it reaching for noise.
fn pick_peak(nsdf: &[f32], min_lag: usize, max_lag: usize) -> Option<(f32, f32)> {
    // Skip the descent from `nsdf[0]`, which is always 1.0 — it is the window matching
    // itself, not a period. Peak picking starts after the function first goes negative,
    // or there is no peak to find.
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

    // A peak sitting exactly on either end of the search is an artefact of where the
    // search was stopped rather than a measurement — the real peak is outside, and this is
    // the shoulder of it. Measured: at a low threshold the picker parked on the shortest
    // period searched and reported a steady 1432.84 Hz whose giveaway was a frame-to-frame
    // spread of exactly zero, which no real note has.
    if lag <= min_lag || lag >= max_lag {
        return None;
    }

    Some((lag as f32 + refine(nsdf, lag), clarity))
}

/// The sub-sample correction to a peak, by fitting a parabola through it and its
/// neighbours. This is what turns a period quantized to whole samples into cent-level
/// precision: at E2 one sample of lag is 2.9 cents, and a tuner wants better than that.
///
/// Clamped to half a sample, which is not a formality. A flat-topped peak drives the
/// denominator toward zero and the correction explodes — unclamped, this returned a shift
/// of −30 samples on a real recording and reported 14930 Hz from a lag that could not be
/// shorter than 34.
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

/// Holds readings over time and releases one only when it has been confirmed.
///
/// The two gates are both necessary and neither is sufficient. Clarity alone separates a
/// held note from a strummed chord by 0.003, which is too little. Agreement alone would
/// pass a sustained harmonic, which is perfectly stable and perfectly wrong.
///
/// What it deliberately does *not* do is average or smooth. A tuner's needle has to follow
/// a bend, and anything that steadies a held reading by mixing in older ones makes a moving
/// one lag behind the sound. Agreement gates; it does not filter.
#[derive(Debug, Default)]
pub struct Tracker {
    /// The tail of consecutive confident readings, most recent last. Never longer than
    /// `STABLE_RUN`, and emptied by anything that breaks the run.
    run: Vec<Detection>,
}

impl Tracker {
    pub fn new() -> Tracker {
        Tracker::default()
    }

    /// Folds one window's analysis into the run, and reports what is worth showing.
    ///
    /// `Heard::Pitch` here means confirmed, where the same variant coming out of [`detect`]
    /// means only measured.
    pub fn push(&mut self, heard: Heard) -> Heard {
        let Heard::Pitch(detection) = heard else {
            // Silence and noise both break the run: a reading has to be re-confirmed
            // after either, which is what stops a stale one outliving its sound.
            self.run.clear();
            return heard;
        };

        if detection.clarity < CLARITY_GATE {
            self.run.clear();
            return Heard::Unpitched;
        }

        // A jump beyond the tolerance is a different note, not a continuation of this one:
        // the run restarts from it rather than being extended or averaged with it.
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

/// The distance between two frequencies, in cents. Here rather than in `music::pitch`
/// because it compares two readings rather than naming either of them, and importing the
/// theory for one logarithm is what the line between these modules exists to prevent.
fn cents_between(from: f32, to: f32) -> f32 {
    1200.0 * (to / from).log2()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: f32 = 48_000.0;

    /// The frequencies this module exists to measure, so the tests read as an instrument.
    const E2: f32 = 82.41;
    const A2: f32 = 110.00;
    const D3: f32 = 146.83;
    const G5: f32 = 783.99;

    /// A tone built from a fundamental and whatever harmonics are given, `harmonics[0]`
    /// being the fundamental's own amplitude. A real string is never a sine, and a
    /// detector tested only on sines would be tested on the one signal it never sees.
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

    /// A plain sine.
    fn sine(hz: f32, len: usize) -> Vec<f32> {
        tone(hz, len, &[0.5])
    }

    /// A guitar-ish string: a fundamental with a decaying harmonic series over it.
    fn plucked(hz: f32, len: usize) -> Vec<f32> {
        tone(hz, len, &[0.5, 0.3, 0.2, 0.12, 0.08, 0.05])
    }

    /// Several notes at once, which is what a strum is and what must never be named.
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
        // Counts positive-going zero crossings over a whole number of seconds' worth of
        // samples, so the generator is pinned independently of anything it is used to test.
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
        // The two ways it classically goes wrong: the second harmonic, and the third.
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
        // The second harmonic carries three times the fundamental's amplitude, which is
        // the ordinary case for a pickup on a low string and gets worse as it decays.
        let samples = tone(D3, WINDOW, &[0.15, 0.45, 0.25, 0.15]);
        let measured = hz_of(detect(&samples, SR));

        assert!(
            cents_off(measured, D3) < 2.0,
            "measured {measured} Hz where the fundamental is {D3} Hz"
        );
    }

    #[test]
    fn an_interpolated_peak_cannot_leave_the_sample_it_corrects() {
        // A deliberately flat top: the denominator of the parabola goes to zero and an
        // unclamped correction runs away. Three equal points have no curvature to fit.
        let flat = [0.0, -0.1, 0.9, 0.9, 0.9, -0.1];
        for lag in 2..=4 {
            assert!(
                refine(&flat, lag).abs() <= 0.5,
                "a flat peak at {lag} shifted by {}",
                refine(&flat, lag)
            );
        }

        // And a genuine peak is corrected in the right direction, by less than a sample.
        let leaning = [0.0, -0.1, 0.6, 0.9, 0.8, -0.1];
        let shift = refine(&leaning, 3);
        assert!(shift > 0.0 && shift <= 0.5, "shift was {shift}");
    }

    #[test]
    fn a_peak_at_the_edge_of_the_search_is_refused() {
        let samples = sine(A2, WINDOW);

        // A2's period is exactly the shortest the search will look at, so the peak it
        // would pick sits on the boundary. It is the shoulder of a peak outside the range.
        assert_eq!(analyse(&samples, SR, MIN_HZ, A2), Heard::Unpitched);

        // And the longest, from the other side.
        assert_eq!(analyse(&samples, SR, A2, MAX_HZ), Heard::Unpitched);

        // The same signal inside a range that contains it is measured normally, so the
        // refusals above are about the boundary rather than about the signal.
        assert!(cents_off(hz_of(analyse(&samples, SR, MIN_HZ, MAX_HZ)), A2) < 1.0);
    }

    #[test]
    fn a_clean_tone_is_confident_and_a_chord_is_not() {
        let Heard::Pitch(single) = detect(&plucked(A2, WINDOW), SR) else {
            panic!("a single note should be measured");
        };
        assert!(single.clarity() > CLARITY_GATE, "{}", single.clarity());

        // An open A chord: A2 E3 A3 C#4 E4.
        let strum = chord(&[110.0, 164.81, 220.0, 277.18, 329.63], WINDOW);
        let clarity = match detect(&strum, SR) {
            Heard::Pitch(detection) => detection.clarity(),
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

        // Just under the gate is still silence, and reports as such rather than as a
        // frequency of zero — a needle resting at zero reads as a note in tune.
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

    /// Drives a tracker across a signal the way the app will: a sliding window, one
    /// analysis per hop. Returns what the tracker released at each step.
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
        let readings = track(&plucked(A2, WINDOW * 6), 512);
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
        let readings = track(&strum, 512);

        assert!(
            released(&readings).is_empty(),
            "a strum released {} readings",
            released(&readings).len()
        );
    }

    #[test]
    fn the_attack_of_a_pluck_is_not_released() {
        // The first windows of a struck string are inharmonic noise rising into a note.
        // Modelled here as noise-like content that settles: what matters is that nothing
        // is released until several windows agree.
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
        // Two semitones over two seconds, the shape of a real bend.
        let from = D3;
        let to = D3 * 2.0f32.powf(2.0 / 12.0);
        let readings = track(&sweep(from, to, (SR * 2.0) as usize), 512);
        let hz = released(&readings);

        assert!(hz.len() > 20, "a bend released only {} readings", hz.len());

        // Monotonic, within the jitter of a single window — the point being that it
        // travels rather than resting on the semitones it passes through.
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

        let hz = released(&track(&samples, 512));

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
        // And the run has to be rebuilt rather than resuming where it left off.
        assert_eq!(tracker.push(detect(&samples, SR)), Heard::Unpitched);
    }
}
