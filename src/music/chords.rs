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
//! Naming a chord correctly runs into the same limitation as the `spell` TODO in
//! `scales`: `Interval` is a pitch-class distance, so it cannot tell an
//! augmented fourth from a diminished fifth — and a chord's quality depends on
//! exactly that distinction.
