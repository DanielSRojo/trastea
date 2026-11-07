//! Inharmonicity: how far a sounding string's partials drift sharp of whole multiples of its
//! fundamental. A measurement, like `detect`'s hertz, so it names no note and knows no neck.

use super::detect::{LEVEL_GATE, rms};
use super::fft::{Complex, transform};

/// The fit's window, 170.7 ms at 48 kHz. Partials are already resolved at 4096, so the length
/// buys signal-to-noise and clean peaks — not a resolution floor, as `WINDOW`'s low E does.
pub const FIT_WINDOW: usize = 8192;

/// Zero-padding, so the parabola is fitted across a peak sampled finely enough to be unbiased.
const PAD: usize = 4;

// the transform refuses anything else, and every spectrum here is exactly this long
const _: () = assert!((FIT_WINDOW * PAD).is_power_of_two());

/// Partials above this are mostly pickup noise.
const TOP_HZ: f64 = 5000.0;

/// How far either side of the starting estimate partial one is looked for: about 50 cents.
const FUNDAMENTAL_SEARCH: f64 = 0.03;

/// A partial's search reaches this far past its prediction, plus `SEARCH_BINS` unpadded bins.
const SEARCH_CENTS: f64 = 15.0;
const SEARCH_BINS: f64 = 3.0;

/// How many times partials are located again against the coefficient the last pass fitted,
/// since the upper ones sit too sharp to be found against a harmonic guess.
const LOCATE_PASSES: usize = 4;

/// A partial is kept only within this of the strongest, in dB...
const RELATIVE_FLOOR_DB: f64 = 50.0;
/// ...and this far above the spectrum around it, in dB.
const SNR_DB: f64 = 15.0;

/// Fewer than this and one coefficient fits anything.
const MIN_PARTIALS: usize = 4;

/// Above this RMS residual, in cents, no single coefficient describes the partials. The
/// spike's clean fits sat under 1, an old wound string at 2.5, and broken fits at 7–9.
const MAX_RESIDUAL_CENTS: f64 = 4.0;

/// A partial further from the fit than this many robust deviations is rejected; never
/// tighter than `OUTLIER_FLOOR_CENTS`, or a near-perfect series rejects its own rounding.
const OUTLIER_DEVIATIONS: f64 = 6.0;
const OUTLIER_FLOOR_CENTS: f64 = 1.0;

/// The coefficient B in `f_n = n · f₀ · √(1 + B·n²)`, and how well the partials agreed on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Inharmonicity {
    /// Never negative: a series that fits flat of harmonic is reported as harmonic.
    coefficient: f32,
    residual_cents: f32,
}

impl Inharmonicity {
    /// Builds a measurement directly, for tests that drive a screen without a device.
    #[cfg(test)]
    pub fn synthesized(coefficient: f32) -> Inharmonicity {
        Inharmonicity {
            coefficient,
            residual_cents: 0.0,
        }
    }

    pub fn coefficient(self) -> f32 {
        self.coefficient
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "reported with every fit; nothing gates on it yet")
    )]
    pub fn residual_cents(self) -> f32 {
        self.residual_cents
    }
}

/// Why a window was not fitted. Distinct from a coefficient of zero, which is a harmonic series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Fewer samples than [`FIT_WINDOW`].
    TooShort,
    /// Below detection's level gate.
    Silence,
    /// Nothing near the starting estimate to take as partial one.
    NoFundamental,
    /// Too few partials above the floor; a bare sine has one, which fits every coefficient.
    TooFewPartials,
    /// The partials scatter too far for any one coefficient.
    Scattered,
}

pub type Fit = Result<Inharmonicity, Refusal>;

/// Fits the newest [`FIT_WINDOW`] samples. `estimate_hz` only starts the search for partial
/// one; detection's fundamental compromises across the sharpened partials, and fitting
/// against it would bias the coefficient.
///
/// Measured at 0.88 ms a fit in a release build (1.25 ms in dev) on a Ryzen 5 5600G, against
/// a hop of 10.7 ms, so it stays on the UI thread beside detection.
///
/// # Errors
///
/// A [`Refusal`] for any window that cannot honestly be fitted.
pub fn fit(samples: &[f32], sample_rate: f32, estimate_hz: f32) -> Fit {
    let start = samples
        .len()
        .checked_sub(FIT_WINDOW)
        .ok_or(Refusal::TooShort)?;
    let window = &samples[start..];

    if rms(window) < LEVEL_GATE {
        return Err(Refusal::Silence);
    }

    let spectrum = Spectrum::of(window, f64::from(sample_rate));

    let estimate = f64::from(estimate_hz);
    let f0 = spectrum
        .peak_near(estimate, estimate * FUNDAMENTAL_SEARCH)
        .ok_or(Refusal::NoFundamental)?
        .hz;

    let mut coefficient = 0.0;
    let mut partials = Vec::new();

    for _ in 0..LOCATE_PASSES {
        let located = spectrum.partials(f0, coefficient);
        if located.len() < MIN_PARTIALS {
            return Err(Refusal::TooFewPartials);
        }

        (coefficient, partials) = fit_rejecting_outliers(f0, located);
    }

    let residual_cents = rms_cents(f0, coefficient, &partials);
    if residual_cents > MAX_RESIDUAL_CENTS {
        return Err(Refusal::Scattered);
    }

    Ok(Inharmonicity {
        coefficient: coefficient.max(0.0) as f32,
        residual_cents: residual_cents as f32,
    })
}

/// One located partial: its number in the series, where it sits, and how loud.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Partial {
    n: usize,
    hz: f64,
    db: f64,
    /// How far it stands above the spectrum around it.
    snr_db: f64,
}

/// A Hann-windowed, zero-padded spectrum in dB, positive frequencies only.
struct Spectrum {
    db: Vec<f64>,
    bin_hz: f64,
}

impl Spectrum {
    fn of(window: &[f32], sample_rate: f64) -> Spectrum {
        let len = window.len() * PAD;
        let mut buffer = vec![Complex::default(); len];

        let width = window.len() as f64;
        for (index, (slot, &sample)) in buffer.iter_mut().zip(window).enumerate() {
            let hann = 0.5 - 0.5 * (std::f64::consts::TAU * index as f64 / width).cos();
            *slot = Complex::new(f64::from(sample) * hann, 0.0);
        }

        // `fit` hands over exactly FIT_WINDOW samples, and the const assertion above
        // proves FIT_WINDOW * PAD a power of two, so the transform cannot refuse this buffer
        transform(&mut buffer).expect("FIT_WINDOW * PAD is a power of two");

        Spectrum {
            // the epsilon keeps an exact zero from becoming −∞ and poisoning the parabola
            db: buffer[..len / 2]
                .iter()
                .map(|bin| 10.0 * (bin.norm_sqr() + 1e-30).log10())
                .collect(),
            bin_hz: sample_rate / len as f64,
        }
    }

    /// The strongest local maximum within `hz ± half`, parabolic-interpolated on log
    /// magnitude, or `None` when the search rises toward its edge — the real peak is outside it.
    fn peak_near(&self, hz: f64, half: f64) -> Option<Peak> {
        let low = (((hz - half) / self.bin_hz).floor().max(1.0)) as usize;
        let high = (((hz + half) / self.bin_hz).ceil() as usize).min(self.db.len().checked_sub(2)?);
        if high <= low {
            return None;
        }

        let (k, _) = self.db[low..=high]
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))?;
        let k = low + k;
        if k == low || k == high {
            return None;
        }

        let (a, b, c) = (self.db[k - 1], self.db[k], self.db[k + 1]);
        let curvature = a - 2.0 * b + c;
        let shift = if curvature.abs() > f64::EPSILON {
            (0.5 * (a - c) / curvature).clamp(-0.5, 0.5)
        } else {
            0.0
        };

        Some(Peak {
            hz: (k as f64 + shift) * self.bin_hz,
            db: b - 0.25 * (a - c) * shift,
        })
    }

    /// The median level within `hz ± half`, which a partial has to stand clear of.
    fn noise_near(&self, hz: f64, half: f64) -> f64 {
        let low = (((hz - half) / self.bin_hz).max(1.0)) as usize;
        let high = (((hz + half) / self.bin_hz) as usize).min(self.db.len());
        let Some(around) = self.db.get(low..high).filter(|slice| !slice.is_empty()) else {
            return f64::INFINITY;
        };

        let mut sorted = around.to_vec();
        sorted.sort_unstable_by(f64::total_cmp);
        sorted[sorted.len() / 2]
    }

    /// Every partial of the series `f0` and `coefficient` predict, found and kept above the
    /// floors. A partial that is missing is left out rather than located anyway.
    ///
    /// Walks up the series refitting as it goes, because a high partial can sit further sharp
    /// of the starting guess than its search reaches, and would be missed or mistaken.
    fn partials(&self, f0: f64, coefficient: f64) -> Vec<Partial> {
        let nyquist = self.bin_hz * self.db.len() as f64;
        let unpadded_bin = self.bin_hz * PAD as f64;

        let mut running = coefficient;
        let mut found: Vec<Partial> = Vec::new();

        for n in 1.. {
            // a noisy fit can go negative, which would put the upper partials flat
            let hz = predicted(f0, running.max(0.0), n);
            if hz > TOP_HZ || hz >= nyquist * 0.95 {
                break;
            }

            let half = (0.3 * f0)
                .min(hz * ((SEARCH_CENTS / 1200.0).exp2() - 1.0) + SEARCH_BINS * unpadded_bin);
            let Some(peak) = self.peak_near(hz, half) else {
                continue;
            };

            let partial = Partial {
                n,
                hz: peak.hz,
                db: peak.db,
                snr_db: peak.db - self.noise_near(hz, 0.5 * f0),
            };
            if partial.snr_db <= SNR_DB {
                continue;
            }

            found.push(partial);
            if found.len() >= MIN_PARTIALS {
                running = closed_form(f0, &found);
            }
        }

        let strongest = found.iter().map(|p| p.db).fold(f64::MIN, f64::max);
        found.retain(|p| p.db > strongest - RELATIVE_FLOOR_DB);
        found
    }
}

struct Peak {
    hz: f64,
    db: f64,
}

/// Where partial `n` of a stiff string sits.
fn predicted(f0: f64, coefficient: f64, n: usize) -> f64 {
    let n = n as f64;
    n * f0 * (1.0 + coefficient * n * n).max(0.0).sqrt()
}

/// How far a partial sits from where the fit puts it, in cents.
fn cents_off(f0: f64, coefficient: f64, partial: &Partial) -> f64 {
    1200.0 * (partial.hz / predicted(f0, coefficient, partial.n)).log2()
}

/// Least squares through the origin: `y = B·x` with `x = n²` and `y = (f_n / n·f₀)² − 1`,
/// so `B = Σxy / Σx²` — no solver, the same reason scales are spelled by arithmetic.
fn closed_form(f0: f64, partials: &[Partial]) -> f64 {
    let (xy, xx) = partials.iter().fold((0.0, 0.0), |(xy, xx), partial| {
        let n = partial.n as f64;
        let x = n * n;
        let y = (partial.hz / (n * f0)).powi(2) - 1.0;
        (xy + x * y, xx + x * x)
    });

    if xx > 0.0 { xy / xx } else { 0.0 }
}

/// Fits, drops partials far from what the rest agree on, and fits again until nothing moves.
fn fit_rejecting_outliers(f0: f64, partials: Vec<Partial>) -> (f64, Vec<Partial>) {
    let mut kept = partials.clone();
    let mut coefficient = closed_form(f0, &kept);

    for _ in 0..3 {
        let deviations: Vec<f64> = kept.iter().map(|p| cents_off(f0, coefficient, p)).collect();
        let centre = median(&deviations);
        let spread: Vec<f64> = deviations.iter().map(|d| (d - centre).abs()).collect();
        // 1.4826 turns a median absolute deviation into a normal standard deviation
        let limit = (OUTLIER_DEVIATIONS * 1.4826 * median(&spread)).max(OUTLIER_FLOOR_CENTS);

        let survivors: Vec<Partial> = partials
            .iter()
            .copied()
            .filter(|p| cents_off(f0, coefficient, p).abs() < limit)
            .collect();

        if survivors.len() < MIN_PARTIALS || survivors == kept {
            break;
        }

        kept = survivors;
        coefficient = closed_form(f0, &kept);
    }

    (coefficient, kept)
}

fn rms_cents(f0: f64, coefficient: f64, partials: &[Partial]) -> f64 {
    if partials.is_empty() {
        return f64::INFINITY;
    }

    let sum: f64 = partials
        .iter()
        .map(|p| cents_off(f0, coefficient, p).powi(2))
        .sum();
    (sum / partials.len() as f64).sqrt()
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_unstable_by(f64::total_cmp);
    sorted[sorted.len() / 2]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;
    use std::f64::consts::TAU;

    const SR: f32 = 48_000.0;

    /// A stiff string's series, `n · f₀ · √(1 + B·n²)`, with amplitudes falling as 1/n.
    struct Series {
        f0: f64,
        coefficient: f64,
        removed: Vec<usize>,
        /// One partial moved off the series, by this many hertz.
        displaced: Option<(usize, f64)>,
    }

    impl Series {
        fn new(f0: f64, coefficient: f64) -> Series {
            Series {
                f0,
                coefficient,
                removed: Vec::new(),
                displaced: None,
            }
        }

        fn without(mut self, removed: &[usize]) -> Series {
            self.removed = removed.to_vec();
            self
        }

        fn displacing(mut self, n: usize, hz: f64) -> Series {
            self.displaced = Some((n, hz));
            self
        }

        fn partial_hz(&self, n: usize) -> f64 {
            let shift = match self.displaced {
                Some((moved, hz)) if moved == n => hz,
                _ => 0.0,
            };
            predicted(self.f0, self.coefficient, n) + shift
        }

        /// The partials that sound: under `TOP_HZ` and not removed.
        fn sounding(&self) -> impl Iterator<Item = usize> + '_ {
            (1..)
                .take_while(|&n| predicted(self.f0, self.coefficient, n) < TOP_HZ)
                .filter(|n| !self.removed.contains(n))
        }

        fn samples(&self, len: usize) -> Vec<f32> {
            let sr = f64::from(SR);
            (0..len)
                .map(|i| {
                    let t = i as f64 / sr;
                    let sum: f64 = self
                        .sounding()
                        // a phase per partial, so their peaks do not all line up at t = 0
                        .map(|n| 0.3 / n as f64 * (TAU * self.partial_hz(n) * t + n as f64).sin())
                        .sum();
                    sum as f32
                })
                .collect()
        }
    }

    fn log2_error(measured: f32, expected: f64) -> f64 {
        (f64::from(measured).log2() - expected.log2()).abs()
    }

    fn assert_recovers(series: &Series, estimate_hz: f64) {
        let fitted = fit(&series.samples(FIT_WINDOW), SR, estimate_hz as f32)
            .unwrap_or_else(|refusal| panic!("refused with {refusal:?}"));
        let error = log2_error(fitted.coefficient(), series.coefficient);

        assert!(
            error < 0.1,
            "B = {} read as {}, {error:.3} out in log₂ B",
            series.coefficient,
            fitted.coefficient()
        );
    }

    #[test]
    fn the_generator_sharpens_the_tenth_partial_as_a_stiff_string_does() {
        let b = 1e-4;
        let series = Series::new(110.0, b);
        let cents = 1200.0 * (series.partial_hz(10) / (10.0 * 110.0)).log2();
        let expected = 866.0 * b * 100.0;

        assert!(
            (cents - expected).abs() < expected * 0.01,
            "the 10th partial is {cents} cents sharp, expected about {expected}"
        );
    }

    #[test]
    fn every_partial_is_located_within_a_tenth_of_a_hertz() {
        for (f0, b) in [(329.63, 1e-4), (110.0, 2e-5), (82.41, 3e-4)] {
            let series = Series::new(f0, b);
            let spectrum = Spectrum::of(&series.samples(FIT_WINDOW), f64::from(SR));
            let located = spectrum.partials(f0, b);

            assert_eq!(
                located.iter().map(|p| p.n).collect::<Vec<_>>(),
                series.sounding().collect::<Vec<_>>(),
                "{f0} Hz: a partial was missed or invented"
            );
            for partial in &located {
                let expected = series.partial_hz(partial.n);
                assert!(
                    (partial.hz - expected).abs() < 0.1,
                    "{f0} Hz partial {} at {} Hz, expected {expected}",
                    partial.n,
                    partial.hz
                );
            }
        }
    }

    #[test]
    fn a_starting_estimate_a_few_cents_off_still_recovers_the_coefficient() {
        let series = Series::new(329.63, 1e-4);
        for cents in [-5.0, 5.0] {
            assert_recovers(&series, 329.63 * (cents / 1200.0f64).exp2());
        }
    }

    #[test]
    fn a_plain_strings_coefficient_is_recovered() {
        assert_recovers(&Series::new(329.63, 1e-4), 329.63);
    }

    #[test]
    fn a_wound_strings_coefficient_is_recovered() {
        assert_recovers(&Series::new(110.0, 2e-5), 110.0);
    }

    #[test]
    fn the_range_a_guitar_produces_is_recovered() {
        // log₂ B from −16 to −10 in the spike, across the neck's fundamentals
        for f0 in [82.41, 146.83, 246.94, 329.63, 659.26] {
            for b in [1.5e-5, 1e-4, 9e-4] {
                assert_recovers(&Series::new(f0, b), f0);
            }
        }
    }

    #[test]
    fn holes_in_the_series_are_left_out_rather_than_fitted() {
        assert_recovers(&Series::new(329.63, 1e-4).without(&[3, 6]), 329.63);
    }

    #[test]
    fn one_partial_out_of_place_is_rejected() {
        let f0 = 329.63;
        assert_recovers(&Series::new(f0, 1e-4).displacing(8, f0 / 4.0), f0);
        // and one displaced by less than the search reaches, which only rejection removes
        assert_recovers(&Series::new(f0, 1e-4).displacing(9, 9.0), f0);
    }

    #[test]
    fn a_harmonic_series_reads_as_harmonic() {
        let series = Series::new(196.0, 0.0);
        let fitted = fit(&series.samples(FIT_WINDOW), SR, 196.0).expect("a clean series fits");

        assert!(
            fitted.coefficient() < 1e-6,
            "a harmonic series read as B = {}",
            fitted.coefficient()
        );
    }

    #[test]
    fn silence_is_refused() {
        assert_eq!(fit(&[0.0; FIT_WINDOW], SR, 110.0), Err(Refusal::Silence));
    }

    #[test]
    fn noise_is_refused() {
        let mut rng = Rng::from_seed(0x0ddba11);
        let noise: Vec<f32> = (0..FIT_WINDOW)
            .map(|_| rng.below(2001) as f32 / 4000.0 - 0.25)
            .collect();

        let fitted = fit(&noise, SR, 110.0);
        assert!(fitted.is_err(), "white noise fitted as {fitted:?}");
    }

    #[test]
    fn a_bare_fundamental_is_refused() {
        let sine: Vec<f32> = (0..FIT_WINDOW)
            .map(|i| 0.3 * (TAU * 220.0 * i as f64 / f64::from(SR)).sin() as f32)
            .collect();

        assert_eq!(fit(&sine, SR, 220.0), Err(Refusal::TooFewPartials));
    }

    #[test]
    fn a_window_the_length_of_detections_is_refused() {
        let series = Series::new(329.63, 1e-4);
        assert_eq!(
            fit(&series.samples(crate::audio::detect::WINDOW), SR, 329.63),
            Err(Refusal::TooShort)
        );
    }

    #[test]
    fn a_longer_buffer_is_fitted_on_its_newest_window() {
        let series = Series::new(329.63, 1e-4);
        let mut samples = vec![0.0; FIT_WINDOW];
        samples.extend(series.samples(FIT_WINDOW));

        assert_recovers_from(&samples, 1e-4, 329.63);
    }

    fn assert_recovers_from(samples: &[f32], b: f64, estimate: f32) {
        let fitted = fit(samples, SR, estimate).expect("the newest window holds a clean series");
        assert!(log2_error(fitted.coefficient(), b) < 0.1);
    }

    #[test]
    fn the_same_window_always_gives_the_same_answer() {
        let samples = Series::new(146.83, 4e-5).samples(FIT_WINDOW);
        assert_eq!(fit(&samples, SR, 146.83), fit(&samples, SR, 146.83));
    }

    #[test]
    fn a_clean_fit_reports_a_small_residual() {
        let samples = Series::new(329.63, 1e-4).samples(FIT_WINDOW);
        let fitted = fit(&samples, SR, 329.63).expect("a clean series fits");

        assert!(fitted.residual_cents() < 0.5, "{}", fitted.residual_cents());
    }
}
