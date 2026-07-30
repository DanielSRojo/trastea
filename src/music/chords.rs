//! Chord construction from scale degrees.
//!
//! Nothing here yet — this module is a placeholder for the chord layer that sits
//! on top of [`super::scales`]. Sketch of what belongs in it:
//!
//! TODO: `ChordQuality` — major, minor, diminished, augmented, dominant 7th,
//!       major 7th, minor 7th, half-diminished, ...
//! TODO: triads and sevenths built by stacking the 1-3-5(-7) degrees of a
//!       `Scale`, rather than hardcoding an interval set per quality.
//! TODO: the diatonic chords of a `ScaleKind` — the seven chords of Ionian, the
//!       major V chord of Harmonic Minor that makes it useful, and so on.
//!
//! Naming a chord correctly needs exactly what `Interval` now carries: a degree
//! number, not just a pitch-class distance. `AugmentedFourth` and
//! `DiminishedFifth` are the same six semitones but different degrees — 4 versus
//! 5 — and a chord's quality reads straight off degree numbers like that one, the
//! same way scale degrees already do in `scales`.
