# Audio and pitch detection

## Why the detector works in the time domain

`audio::detect` uses McLeod's normalized square difference function with the threshold rule for
peak picking and parabolic interpolation on the winner. An FFT was rejected for two reasons, both
about the low E:

- At 48 kHz over 4096 samples an FFT resolves 11.7 Hz per bin. E2 and F2 are 4.9 Hz apart, so
  it cannot separate adjacent semitones down there, let alone the cent a tuner wants, which at
  E2 is 0.048 Hz.
- Peak-picking a spectrum returns whichever partial is loudest, and on a guitar pickup that is
  routinely the second harmonic rather than the fundamental.

The cost is affordable: the difference function over one window is one to two million
multiply-adds, roughly ninety times a second, which is low single-digit percent of one core. No
FFT is needed, so there is no FFT crate either.

Every constant in `detect.rs` was measured against a guitar through an audio interface rather
than taken from a paper.

## Window, hop and range

`detect::WINDOW` is 2048 samples and is a floor, not a preference. The difference function
needs two periods of the lowest pitch it measures; the open low E at 82.41 Hz is 582 samples at
48 kHz, so 1165 samples is the hard minimum and 1024 would lose that string entirely. 2048 gives
three and a half periods. It is also the detector's latency, 42.7 ms at 48 kHz, and that figure
is physics rather than a parameter to lower until the low E breaks.

`detect::HOP` is a quarter window, so consecutive windows overlap by three quarters. Latency
stays at one window, but a reading can change every 10.7 ms rather than every 42.7 ms. The
tracker's constants are tuned to this spacing, which is why the constant is shared with
`audio::input` rather than restated there.

`MIN_HZ` is 70 Hz, below the low E's 82.41 Hz, because a string being tuned up from slack is
exactly when a tuner is wanted. `MAX_HZ` is 900 Hz: the fifteenth fret of the high E — the top
of the longest neck drawn — is G5 at 783.99 Hz, and the margin is for a note bent past it.

## Peak picking

`detect::pick_peak` takes the first peak clearing `PEAK_THRESHOLD × the strongest peak`, not the
strongest peak outright. That rule is the whole of the octave robustness: a harmonic produces a
taller peak at a shorter period whenever the fundamental is weak, and the first peak over the
threshold is the longer period.

`PEAK_THRESHOLD` is the constant most likely to be "fixed" from the wrong intuition. Lowering it
accepts an earlier peak, which is a shorter period and a higher frequency, so a lower threshold
biases toward harmonics. Measured on a recorded guitar:

| threshold | decaying open D jumps an octave for | other effects                   |
| --------- | ----------------------------------- | ------------------------------- |
| 0.75      | 0.71 s                              | low E no longer detected at all |
| 0.90      | 0.39 s                              | boundary artefacts              |
| 0.97      | 0.15 s                              | boundary artefacts gone         |

A peak sitting exactly on either end of the lag search is refused. At a low threshold the picker
parked on the shortest period searched and reported a steady 1432.84 Hz; the giveaway was a
frame-to-frame spread of exactly zero, which no real note has.

## Interpolation and precision

`detect::refine` fits a parabola through the peak and its neighbours. This is what turns a period
quantized to whole samples into cent-level precision: at E2 one sample of lag is 2.9 cents.

The correction is clamped to half a sample, and the clamp is load-bearing. Unclamped, a
flat-topped peak on a real recording returned a shift of −30 samples and reported 14930 Hz from
a lag that could not be shorter than 34.

`detect::nsdf` accumulates in `f64` while everything around it is `f32`. It is the one place
thousands of products are summed, and avoiding a window's worth of rounding there is free even
though the result is rounded back down at once. The normalization by summed energy is what lets
one threshold serve a struck string and a decaying one.

## Confirmation

`detect::Tracker` releases a reading only when it clears the clarity gate and agrees with the
readings before it. Both gates are needed and neither is enough:

- Clarity alone is too thin. Held notes score 0.996 to 1.000, the best frame of a strummed chord
  reached 0.987 across 237 strum frames, and quiet legitimate notes reach down to about 0.991.
  `CLARITY_GATE` at 0.99 separates them by 0.003.
- Agreement alone would pass a sustained harmonic, which is perfectly stable and perfectly wrong.

`STABLE_RUN` is three windows, which at a hop of 512 samples is 32 ms of agreement. The attack of
a plucked note is inharmonic for its first ~40 ms and cannot supply that.

`STABLE_CENTS` is 20 cents and deliberately generous. Its job is to reject the attack transient
and the octave jump, which miss by hundreds of cents or by 1200; rejecting strums is the clarity
gate's job. A fast bend — a whole tone in 0.3 s — moves about 7 cents between windows, and a held
note jitters by under 2.

The tracker never averages or smooths. Anything that steadies a held reading by mixing in older
ones makes a moving one lag behind the sound, and a tuner has to follow a bend. Agreement gates;
it does not filter. Silence and unpitched windows both clear the run, so a reading must be
re-confirmed after either, and a jump beyond the tolerance restarts the run from the new reading
rather than extending it.

## The detector's vocabulary

`detect::Heard` has three cases rather than wrapping an `Option`, because "heard nothing" and
"heard something I will not name" are different facts to the screen: the room is quiet, or a
chord was strummed at it. `Tracker::push` returns the same type one stage later, where
`Heard::Pitch` means confirmed rather than measured, because the three cases are the same three.

`detect::Detection` is a frequency and a clarity, deliberately not a note — naming it needs
`music::pitch`, which `audio/` does not import. `cents_between` lives in `detect.rs` for the same
reason: it compares two readings without naming either, and importing the theory for one
logarithm is what the module line exists to prevent.

## Testing the detector

Test signals are built from a fundamental plus harmonics, because a real string is never a sine
and a detector tested only on sines is tested on the one signal it never sees. The weak
fundamental case puts the second harmonic at three times the fundamental's amplitude, the
ordinary case for a pickup on a low string, and it gets worse as the note decays. The low E test
checks against both classic failures, the second harmonic and the third. The tone generator is
pinned separately by counting zero crossings over a whole number of seconds, so it is not only
checked by the code it is used to test.

Just below the level gate a window reports `Heard::Silence` rather than a frequency of zero,
which would draw as a note in tune.

## The audio callback

The callback runs on a thread the audio system owns, to a deadline rather than an average, inside
the time budget of one buffer. It may not allocate, may not wait on a lock another thread holds,
and may not panic, since an unwind through a C callback is undefined. `input::collect` meets
that by construction: the buffer is reserved at `CAPACITY` frames and room is made before a chunk
lands, every fallible step is a `try_lock` or a saturating subtraction, and the stride is
`NonZeroUsize`.

`input::Shared` guards the samples with a `Mutex`, which the callback only ever `try_lock`s.
Priority inversion needs a blocking acquire, and the callback never waits; the cost is one chunk,
a few milliseconds, in the rare moment the reader holds the lock. A lock-free ring would avoid
even that and would be the right call in a DAW. Here the consumer is a redraw a few times a
second and a dropped window means a dial that updates a beat later, so the simpler choice is
deliberate. A test holds the lock and calls the callback; if it blocked, the test would deadlock
rather than fail.

`Shared::lost` uses `Ordering::Relaxed` because it is one flag with nothing else ordered against
it, and a reader seeing it a few microseconds late is indistinguishable from the device failing a
few microseconds later.

`CAPACITY` is two windows: enough to always have a full one ready if the reader falls behind, and
the oldest is the one worth losing. A chunk longer than the whole buffer keeps only its newest
`CAPACITY` frames, and whole frames only, so the buffer never falls out of step with its channels.

The length in `collect` is read into a local before `held.drain(..)`. `held` is a `MutexGuard`,
so `drain` goes through `DerefMut`, and the two-phase borrow that makes `v.push(v.len())` legal on
a plain `Vec` does not reach through an overloaded deref.

## Windows and channels

`input::take` hands over the newest `WINDOW` frames, not everything accumulated, so the analysed
length — and with it the latency — does not depend on how long it has been since the last redraw.
It keeps `WINDOW - HOP` frames behind, so the next window arrives one hop later rather than one
window later. A redraw faster than that gets `None`, so the same window is never analysed twice.

De-interleaving is not optional. A 2-in interface hands `[in1, in2, in1, in2, …]`, and passing
that through as mono halves the apparent sample rate and reports every pitch an octave low, with
perfect confidence, because the interleaved signal is still periodic.

`input::loudest` picks the channel per window: a guitar is one mono source, so the input it is
plugged into is the loud one and the rest are hiss, and moving the cable needs nothing said. A tie
— silence, or a stereo pair carrying one source — goes to input 1. Before this, a 2-in interface
with the guitar in input 2 and nothing in input 1 read as permanent silence.

Formats are scaled by a local `Sample` trait rather than cpal's conversion traits, which are
generic over far more than four cases need. Full scale is 1.0 whatever the native type, so one
level gate and one clarity threshold work for every device. Unsigned silence sits at half scale.

## Choosing a device

`input::InputError` keeps cpal's classification rather than its message, so the screen never
parses prose. Its cases are distinct because their fixes are: no device is a shopping problem, a
busy device is a "quit your DAW" problem, and a tuner that renders all of them as an empty dial
looks broken.

`input::devices` turned 32 ALSA entries into 4 on the development machine, by three rules in
order:

- `is_hardware` drops ALSA's software plugins — `null`, `pulse`, `pipewire`, `speexrate`,
  `upmix` and a dozen more. It tests the id for `CARD=` rather than the display name, which is
  translatable prose ("Rate Converter Plugin Using Speex Resampler"). No other backend's ids
  contain `CARD=` or start with `alsa:`, so CoreAudio, WASAPI and ASIO devices are unaffected.
- A device whose default config cannot be read is dropped; offering it is offering a failure.
  This also removes ALSA's `usbstream:` entries, which enumerate but never open.
- One entry per name. The interface on the development machine appears four times, as
  `sysdefault:`, `front:`, `hw:` and `plughw:`. First wins because ALSA lists `sysdefault` first,
  and it converts format and rate rather than demanding the card's raw layout.

`input::DeviceChoice` is keyed by cpal's device id, which cpal builds from the host's own
identifier (the ALSA PCM id) and documents `Display`/`FromStr` for. An index would shift whenever
anything is plugged in, and this is the value remembered across restarts.

`Input::open_id` falls back to the default device when the remembered one is missing, so an
unplugged interface leaves a working tuner rather than an error. `fell_back` records it so the
screen can say so: a fallback that looks like a choice is how a learner ends up tuning against a
headset without knowing it. Asking for no device and getting the default is not a fallback.

`input::Input` is owned by the app and is not `Clone`. iced hashes a subscription recipe rather
than the values its closure captured, so a stream owned inside a subscription would be dropped
and rebuilt whenever that hash changed — an audible gap and a device that may refuse to reopen
while it is still closing. This is the same hazard the README describes for key translation.

## Remembering the device

The chosen id is one line of text at `$XDG_CONFIG_HOME/trastea/input-device` or
`~/.config/trastea/input-device`. There is exactly one thing to remember, and a structured format
would be a format to migrate later. Every failure to locate, read or write it is silent: with no
path the tuner opens the default each time, which is a worse tuner rather than a broken one. A
blank or whitespace-only file counts as nothing remembered, since an empty id would be asked for
and never found.

`read_choice` and `write_choice` take the path as an argument so tests can use a temporary file.
They replaced a test that pointed `XDG_CONFIG_HOME` elsewhere with `set_var`, which mutates
process-global state while the harness runs tests on parallel threads, several of which open the
Tuner and read that variable. Edition 2024 made `set_var` `unsafe` for this reason; it is
undefined behaviour rather than merely flaky.

## The Tuner screen

`ui::tuner::IN_TUNE_CENTS` is 3 cents rather than 1: a guitar's own intonation moves a fretted
note further than that, and a reading that refuses to settle is one nobody trusts.
`Tuner::correction` is the single place that compares against it, so the direction mark and the
in-tune colouring cannot disagree about where the boundary is; a test checks 2.9 and 3.1 cents on
both sides.

A bend measured on the Tuner crossed three note names while travelling continuously, which is
why the dial is the readout and the name is a label on it. The hertz and the cents are shown side
by side so a learner who does not yet trust the dial has both halves of its reasoning: what was
measured, and that measurement against the note it was given.

The Tuner keeps its own `Spelling`, like the Note Trainer; a spelling chosen while tuning has no
business renaming anything else.

`position_for` clamps even though `Pitch::nearest` never returns more than 50 cents, so the range
is a fact about the function rather than about its caller. Past the range the dot stops at the end
of the arc; a reading that far out is already the next note.

## The dial's geometry

`DIAL_SIZE` is fixed at 460 × 235 rather than filling its space: an arc is read as a shape, and a
shape that changes proportion with the window is a different shape every time.

`tuner::Dial` is one struct for the same reason `fretboard::Layout` is: the canvas that draws the
arc and the overlay that centres the note name both read its geometry, and a second copy of the
arithmetic would drift. There is no hit-testing and so no round-trip test; tests pin the track's
three landmarks and check that the name's padding lands where `name_center` says. The padding is
the seam, because the canvas never sees it.

- `HALF_SWEEP` is 80° rather than 90°, so the ends sit above the arc's widest point and lean
  toward the reader.
- `LABEL_SIZE` is a SMuFL em, drawn against a four-space stave, so a flat at 44 sets about as tall
  as a letter at 26.
- `name_center` sits a shade below the middle of the bowl, where a run of cap-height glyphs looks
  centred rather than measures centred.
- The name is a `stack` over the canvas rather than canvas text because it is two glyphs in two
  faces, and `note_label` already composes that pair for every other screen.
- The ♭ and ♯ are private-use codepoints, which basic shaping maps by luck, so they use
  `Shaping::Advanced`.
- With no reading the name slot holds an em dash at `NAME_SIZE / 2.4`; at full size it sets as a
  rule across the dial. It holds the slot so nothing shifts when a note lands.
