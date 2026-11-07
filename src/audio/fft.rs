//! An in-place iterative radix-2 FFT, hand-rolled for the same reason `rng.rs` is: about
//! eighty lines did not justify a third dependency.
//!
//! `detect.rs` needs no FFT because a bin is too coarse for a fundamental. The inharmonicity
//! fit reads partials, which sit a whole fundamental apart, so a bin is fine enough there.

use std::f64::consts::TAU;
use std::ops::{Add, Mul, Sub};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const fn new(re: f64, im: f64) -> Complex {
        Complex { re, im }
    }

    /// The squared magnitude, which is what a spectrum is ranked by; the root is left to
    /// whoever needs it.
    pub fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
}

impl Add for Complex {
    type Output = Complex;

    fn add(self, other: Complex) -> Complex {
        Complex::new(self.re + other.re, self.im + other.im)
    }
}

impl Sub for Complex {
    type Output = Complex;

    fn sub(self, other: Complex) -> Complex {
        Complex::new(self.re - other.re, self.im - other.im)
    }
}

impl Mul for Complex {
    type Output = Complex;

    fn mul(self, other: Complex) -> Complex {
        Complex::new(
            self.re * other.re - self.im * other.im,
            self.re * other.im + self.im * other.re,
        )
    }
}

/// A buffer the transform was refused for, because radix-2 halves it down to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotAPowerOfTwo(pub usize);

/// Replaces `buffer` with its discrete Fourier transform, unnormalised.
///
/// # Errors
///
/// Returns [`NotAPowerOfTwo`] before touching `buffer` if its length is not a power of two.
pub fn transform(buffer: &mut [Complex]) -> Result<(), NotAPowerOfTwo> {
    let len = buffer.len();
    if !len.is_power_of_two() {
        return Err(NotAPowerOfTwo(len));
    }

    reorder(buffer);

    // one table for every stage, computed directly rather than by repeated multiplication,
    // which drifts by an ulp per step across thousands of steps
    let twiddles: Vec<Complex> = (0..len / 2)
        .map(|k| {
            let (sin, cos) = (-TAU * k as f64 / len as f64).sin_cos();
            Complex::new(cos, sin)
        })
        .collect();

    let mut span = 2;
    while span <= len {
        let stride = len / span;

        for block in buffer.chunks_exact_mut(span) {
            let (low, high) = block.split_at_mut(span / 2);

            for (k, (a, b)) in low.iter_mut().zip(high.iter_mut()).enumerate() {
                let turned = *b * twiddles[k * stride];
                (*a, *b) = (*a + turned, *a - turned);
            }
        }

        span *= 2;
    }

    Ok(())
}

/// The bit-reversal permutation the iterative transform expects its input in.
fn reorder(buffer: &mut [Complex]) {
    let bits = buffer.len().trailing_zeros();
    if bits == 0 {
        return;
    }

    for index in 0..buffer.len() {
        let reversed = index.reverse_bits() >> (usize::BITS - bits);
        // each pair swapped once, from its lower end
        if index < reversed {
            buffer.swap(index, reversed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    fn real(samples: &[f64]) -> Vec<Complex> {
        samples.iter().map(|&s| Complex::new(s, 0.0)).collect()
    }

    fn close(a: Complex, b: Complex, tolerance: f64) -> bool {
        (a.re - b.re).abs() < tolerance && (a.im - b.im).abs() < tolerance
    }

    /// The textbook O(n²) sum, which the fast version has to agree with.
    fn naive(input: &[Complex]) -> Vec<Complex> {
        let len = input.len() as f64;
        (0..input.len())
            .map(|k| {
                input
                    .iter()
                    .enumerate()
                    .fold(Complex::default(), |sum, (n, &x)| {
                        let (sin, cos) = (-TAU * k as f64 * n as f64 / len).sin_cos();
                        sum + x * Complex::new(cos, sin)
                    })
            })
            .collect()
    }

    #[test]
    fn an_impulse_has_a_flat_spectrum() {
        let mut buffer = real(&[0.0; 16]);
        buffer[0] = Complex::new(1.0, 0.0);

        transform(&mut buffer).expect("16 is a power of two");

        assert!(
            buffer
                .iter()
                .all(|&bin| close(bin, Complex::new(1.0, 0.0), 1e-12)),
            "{buffer:?}"
        );
    }

    #[test]
    fn a_sine_on_a_bin_lands_in_that_bin_and_its_mirror() {
        let len = 64;
        let bin = 5;
        let samples: Vec<f64> = (0..len)
            .map(|n| (TAU * bin as f64 * n as f64 / len as f64).sin())
            .collect();
        let mut buffer = real(&samples);

        transform(&mut buffer).expect("64 is a power of two");

        for (k, value) in buffer.iter().enumerate() {
            let expected = if k == bin || k == len - bin {
                len as f64 / 2.0
            } else {
                0.0
            };
            assert!(
                (value.norm_sqr().sqrt() - expected).abs() < 1e-9,
                "bin {k} holds {}",
                value.norm_sqr().sqrt()
            );
        }
    }

    #[test]
    fn a_random_input_matches_the_naive_transform() {
        let mut rng = Rng::from_seed(0x5eed);
        let input: Vec<Complex> = (0..64)
            .map(|_| {
                let draw = |rng: &mut Rng| rng.below(2001) as f64 / 1000.0 - 1.0;
                Complex::new(draw(&mut rng), draw(&mut rng))
            })
            .collect();

        let expected = naive(&input);
        let mut buffer = input;
        transform(&mut buffer).expect("64 is a power of two");

        for (k, (fast, slow)) in buffer.iter().zip(&expected).enumerate() {
            assert!(
                close(*fast, *slow, 1e-4),
                "bin {k}: {fast:?} against {slow:?}"
            );
        }
    }

    #[test]
    fn a_length_that_is_not_a_power_of_two_is_refused_untouched() {
        let original = real(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let mut buffer = original.clone();

        assert_eq!(transform(&mut buffer), Err(NotAPowerOfTwo(6)));
        assert_eq!(buffer, original, "a refused buffer was partly transformed");

        assert_eq!(transform(&mut []), Err(NotAPowerOfTwo(0)));
    }

    #[test]
    fn a_single_sample_is_its_own_transform() {
        let mut buffer = real(&[0.75]);
        transform(&mut buffer).expect("one is two to the zero");
        assert_eq!(buffer, real(&[0.75]));
    }
}
