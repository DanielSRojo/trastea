//! Getting sound into the app, and turning it into a frequency.
//!
//! The line this module holds is the same one `music/` holds against iced, one step
//! further out: **`audio/` speaks hertz and knows nothing about notes.** Naming a
//! frequency is theory's job and `music::pitch` already does it. Keeping the vocabulary
//! out of here is also what lets the detector be tested against sine waves with no music
//! in scope at all — a test that asserts "82.41 Hz within a cent" needs no `Pitch`.
//!
//! `detect` is a pure function of its inputs and holds all of the interesting logic.

pub mod detect;
pub mod input;
