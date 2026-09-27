//! Opening a capture device and getting its samples to the detector.
//!
//! This is the only file in the change that cannot be tested, which is the whole reason
//! the seam sits where it does: everything with a decision in it lives in `detect`, and
//! what is left here is opening a device, normalising what it gives us, and handing the
//! samples across a thread boundary. What can be tested here — de-interleaving, format
//! conversion, the buffer's behaviour under contention — is, below.
//!
//! The hard constraint is the callback. It runs on a thread the audio system owns, to a
//! deadline rather than an average, and whatever it does is done inside the time budget of
//! one buffer. It may not allocate, may not wait on a lock another thread holds, and may
//! not panic — an unwind through a C callback is undefined. So it copies and returns, and
//! every decision happens elsewhere.

// The screen is the caller and lands in the same change.
//
// A bare `#[expect]` rather than `detect.rs`'s `cfg_attr(not(test), ...)`, and the
// difference is the point: everything in `detect` is exercised by its own tests, so under
// a test build nothing there is dead and a bare expectation would go unfulfilled. Here the
// opposite holds — opening a device needs hardware, so `Input` and everything reaching it
// stay dead under both builds, and gating the attribute on `not(test)` would leave the
// test build warning about exactly the items that cannot be tested.
#![expect(
    dead_code,
    reason = "input precedes the tuner screen that opens it; the device path needs hardware"
)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{ErrorKind, SampleFormat};

use super::detect::{HOP, WINDOW};

/// Why there is nothing to listen to.
///
/// Three cases rather than one, because a learner hearing nothing needs to tell a machine
/// with no interface from an interface another program is holding — the first is a
/// shopping problem and the second is a "quit your DAW" problem. A tuner that renders all
/// of them as an empty dial is indistinguishable from a tuner that is broken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputError {
    /// The host reported no capture device at all.
    NoDevice,
    /// A device exists but is held by something else. Its own case because the fix is
    /// "close the other program", which no other failure here shares.
    Busy,
    /// The OS refused access — a privacy setting, or missing group membership on Linux.
    NotPermitted,
    /// A device exists, but a stream could not be built on it for any other reason.
    Unavailable(String),
    /// A stream that was running stopped on its own. Carries what the host said.
    Lost(String),
}

impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputError::NoDevice => write!(f, "no input device"),
            InputError::Busy => write!(f, "the input device is in use by another program"),
            InputError::NotPermitted => write!(f, "not allowed to use the input device"),
            InputError::Unavailable(why) => write!(f, "could not open the input device: {why}"),
            InputError::Lost(why) => write!(f, "lost the input device: {why}"),
        }
    }
}

impl From<cpal::Error> for InputError {
    /// cpal already classifies its failures; throwing that away and keeping only the
    /// message would make the screen parse prose to say anything useful.
    fn from(error: cpal::Error) -> InputError {
        match error.kind() {
            ErrorKind::DeviceNotAvailable | ErrorKind::HostUnavailable => InputError::NoDevice,
            ErrorKind::DeviceBusy => InputError::Busy,
            ErrorKind::PermissionDenied => InputError::NotPermitted,
            _ => InputError::Unavailable(
                error
                    .message()
                    .map_or_else(|| error.kind().to_string(), str::to_owned),
            ),
        }
    }
}

/// One input device the learner may choose, as the screen needs to show it.
///
/// The id rather than an index, because an index is a position in an enumeration that
/// changes when anything is plugged in, and this is the value that gets remembered across
/// restarts. cpal builds it from the host's own identifier — the ALSA PCM id here — and
/// documents `Display`/`FromStr` on it for exactly this purpose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceChoice {
    id: String,
    name: String,
}

impl DeviceChoice {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Every input device worth offering, in the order the host lists them.
///
/// Three rules, in order, and the second and third are why this is not just a `map` over
/// `input_devices`. On the development machine they turn 32 entries into 4.
pub fn devices() -> Vec<DeviceChoice> {
    let host = cpal::default_host();
    let Ok(found) = host.input_devices() else {
        return Vec::new();
    };

    let mut offered: Vec<DeviceChoice> = Vec::new();

    for device in found {
        let (Ok(id), Ok(description)) = (device.id(), device.description()) else {
            continue;
        };
        let id = id.to_string();

        if !is_hardware(&id) {
            continue;
        }

        // Offering a device whose configuration cannot even be read is offering a failure.
        // This is also what drops ALSA's `usbstream:` entries, which enumerate but never open.
        if device.default_input_config().is_err() {
            continue;
        }

        // One entry per device. A host may expose the same hardware several times — the
        // interface here appears four times, as `sysdefault:`, `front:`, `hw:` and
        // `plughw:` — and first wins because ALSA lists `sysdefault` first, which is the
        // one that converts format and rate rather than demanding the card's raw layout.
        let name = description.name().to_owned();
        if offered.iter().any(|offer| offer.name == name) {
            continue;
        }

        offered.push(DeviceChoice { id, name });
    }

    offered
}

/// Whether an id names real hardware rather than one of the host's software plugins.
///
/// ALSA enumerates its whole plugin chain as devices — `null`, `pulse`, `pipewire`,
/// `speexrate`, `upmix` and a dozen more — and a learner looking for their interface is not
/// looking for a rate converter. The test is the id rather than the display name, because
/// the name is prose ("Rate Converter Plugin Using Speex Resampler") and translatable,
/// while `CARD=` is an ALSA convention. Hardware PCMs name a card and plugins do not.
///
/// Hosts without a plugin chain lose nothing: no other backend's ids contain `CARD=`, so
/// the ALSA-shaped test only applies where ALSA-shaped ids appear.
fn is_hardware(id: &str) -> bool {
    !id.starts_with("alsa:") || id.contains("CARD=")
}

/// The most recent window of audio, written by the audio thread and read by whoever asks.
///
/// A `Mutex` in an audio path deserves its raised eyebrow, so: the callback only ever
/// `try_lock`s, and drops its chunk when the lock is held. It never waits, which is the
/// property that matters — priority inversion needs a blocking acquire. What that costs is
/// one window of freshness, about 43 ms, in the rare moment the reader holds the lock.
///
/// A lock-free ring would avoid even that, and in a DAW it would be the right call. Here
/// the consumer is a redraw a few times a second and the cost of a dropped window is a
/// dial that updates a beat later, so the simpler thing is chosen deliberately rather
/// than by not thinking about it.
#[derive(Debug)]
struct Shared {
    samples: Mutex<Vec<f32>>,
    /// Set by the error callback. `Ordering::Relaxed` throughout: this is one flag with no
    /// other memory ordered against it, and a reader seeing it a few microseconds late is
    /// indistinguishable from the device having failed a few microseconds later.
    lost: AtomicBool,
    /// What the host said when it failed, for the screen to show.
    reason: Mutex<Option<String>>,
}

/// An open capture device.
///
/// **Holding this is what keeps the audio running** — dropping it closes the stream. It is
/// deliberately not `Clone` and deliberately owned by the app rather than by anything the
/// UI framework may rebuild: iced hashes a subscription recipe rather than the values its
/// closure captured, so a stream owned inside a subscription would be dropped and rebuilt
/// whenever that hash changed. `ui/mod.rs` documents the same hazard for key translation,
/// where the cost was a stale boolean; here it is an audible gap and a device that may
/// refuse to reopen while it is still closing.
pub struct Input {
    /// Never read. Present because dropping it stops the audio.
    _stream: cpal::Stream,
    shared: Arc<Shared>,
    device_name: String,
    sample_rate: f32,
    /// Set when a device was asked for by id and could not be found, so the default was
    /// opened instead. The screen has to say so: a fallback that looks like a choice is how
    /// a learner ends up tuning against a headset without knowing it.
    fell_back: bool,
}

impl Input {
    /// Opens the system default capture device.
    pub fn open() -> Result<Input, InputError> {
        Input::open_id(None)
    }

    /// Opens the device with this id, or the system default when `wanted` is `None` or names
    /// a device that is not present.
    ///
    /// Falling back rather than failing is deliberate — a remembered interface that is
    /// merely unplugged should leave a working tuner rather than an error — but the fallback
    /// is recorded so the screen can say it happened.
    pub fn open_id(wanted: Option<&str>) -> Result<Input, InputError> {
        let host = cpal::default_host();

        let chosen = wanted.and_then(|id| {
            host.input_devices()
                .ok()?
                .find(|device| device.id().is_ok_and(|found| found.to_string() == id))
        });

        let fell_back = wanted.is_some() && chosen.is_none();
        let device = match chosen {
            Some(device) => device,
            None => host.default_input_device().ok_or(InputError::NoDevice)?,
        };

        let device_name = device.description().map_or_else(
            |_| "unnamed input device".to_owned(),
            |d| d.name().to_owned(),
        );

        let config = device.default_input_config()?;

        // `SampleRate` is a plain `u32` in this version rather than a newtype.
        let sample_rate = config.sample_rate() as f32;
        let channels = config.channels() as usize;

        let shared = Arc::new(Shared {
            samples: Mutex::new(Vec::with_capacity(WINDOW)),
            lost: AtomicBool::new(false),
            reason: Mutex::new(None),
        });

        let stream = build_stream(&device, &config, channels, Arc::clone(&shared))?;
        stream.play()?;

        Ok(Input {
            _stream: stream,
            shared,
            device_name,
            sample_rate,
            fell_back,
        })
    }

    /// Whether the device asked for was absent and the default was opened instead.
    pub fn fell_back(&self) -> bool {
        self.fell_back
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// The most recent full window, or `None` if a hop has not arrived since the last call.
    pub fn take_window(&self) -> Option<Vec<f32>> {
        let mut held = self.shared.samples.lock().ok()?;
        take(&mut held)
    }

    /// Whether the device has failed since it was opened, and what the host said.
    pub fn lost(&self) -> Option<InputError> {
        self.shared.lost.load(Ordering::Relaxed).then(|| {
            let reason = self
                .shared
                .reason
                .lock()
                .ok()
                .and_then(|held| held.clone())
                .unwrap_or_else(|| "the device stopped".to_owned());
            InputError::Lost(reason)
        })
    }
}

/// Builds the stream for whichever sample format the device offers.
///
/// The match is the whole of the format handling: `cpal` hands each format as its own
/// native type, and [`Sample::to_f32`] puts all of them on the same scale.
fn build_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    channels: usize,
    shared: Arc<Shared>,
) -> Result<cpal::Stream, InputError> {
    let errors = Arc::clone(&shared);
    let on_error = move |err: cpal::Error| {
        // Runs on the audio thread's error path: record and return, nothing more.
        if let Ok(mut reason) = errors.reason.try_lock() {
            *reason = Some(err.to_string());
        }
        errors.lost.store(true, Ordering::Relaxed);
    };

    let stream_config = config.config();

    macro_rules! build {
        ($sample:ty) => {
            device.build_input_stream(
                stream_config.clone(),
                move |data: &[$sample], _: &cpal::InputCallbackInfo| {
                    collect(data, channels, &shared);
                },
                on_error,
                None,
            )
        };
    }

    match config.sample_format() {
        SampleFormat::F32 => build!(f32),
        SampleFormat::I16 => build!(i16),
        SampleFormat::I32 => build!(i32),
        SampleFormat::U16 => build!(u16),
        other => {
            return Err(InputError::Unavailable(format!(
                "unsupported sample format {other}"
            )));
        }
    }
    .map_err(InputError::from)
}

/// The body of the audio callback.
///
/// Allocates nothing — the buffer was reserved at `open` and only ever grows to the cap
/// below — never waits, and cannot panic: every fallible step is a `try_lock` or a
/// saturating truncation.
fn collect<S: Sample>(data: &[S], channels: usize, shared: &Shared) {
    let Ok(mut held) = shared.samples.try_lock() else {
        // The reader has it. Dropping this chunk costs one window of freshness and is the
        // whole reason this is a `try_lock` rather than a `lock`.
        return;
    };

    // Keep the buffer from growing without bound if nothing ever reads it — two windows is
    // enough to always have a full one ready, and the oldest is the one worth losing.
    if held.len() > WINDOW * 2 {
        // The length is read into a local first. `held` is a `MutexGuard`, so `held.drain(..)`
        // goes through `DerefMut` — and the two-phase borrow that makes `v.push(v.len())`
        // legal on a plain `Vec` does not reach through an overloaded deref, so the
        // mutable borrow is already live when the argument wants to read the length.
        let excess = held.len() - WINDOW;
        held.drain(..excess);
    }

    held.extend(deinterleave(data, channels));
}

/// The newest `WINDOW` of `held`, keeping only the overlap the next window will share.
///
/// Keeping `WINDOW - HOP` rather than nothing is what makes the next window arrive one
/// [`HOP`] later instead of one `WINDOW` later — the spacing the tracker is tuned for. A
/// redraw faster than that still gets `None`, so the same window is never analysed twice.
fn take(held: &mut Vec<f32>) -> Option<Vec<f32>> {
    if held.len() < WINDOW {
        return None;
    }

    // The newest of what is there. Handing over everything that had accumulated would
    // make the analysed length — and so the latency — depend on how long it had been
    // since the last redraw.
    let window = held[held.len() - WINDOW..].to_vec();
    let consumed = held.len() - (WINDOW - HOP);
    held.drain(..consumed);
    Some(window)
}

/// The first channel of an interleaved buffer, converted to `f32`.
///
/// Not optional. A 2-in interface hands `[in1, in2, in1, in2, ...]`, and passing that
/// through as if it were mono halves the apparent sample rate and reports every pitch an
/// octave low — with perfect confidence, because the interleaved signal is still periodic.
fn deinterleave<S: Sample>(data: &[S], channels: usize) -> impl Iterator<Item = f32> + '_ {
    data.iter().step_by(channels.max(1)).map(|s| s.to_f32())
}

/// Scaling whatever the device gives us onto the same range.
///
/// A local trait rather than cpal's own, because cpal's conversion traits are generic over
/// far more than this needs and naming the four cases here is shorter than the bounds
/// would be — and because the scaling is the thing worth being able to read.
trait Sample: Copy {
    fn to_f32(self) -> f32;
}

impl Sample for f32 {
    fn to_f32(self) -> f32 {
        self
    }
}

impl Sample for i16 {
    fn to_f32(self) -> f32 {
        f32::from(self) / -f32::from(i16::MIN)
    }
}

impl Sample for i32 {
    fn to_f32(self) -> f32 {
        self as f32 / -(i32::MIN as f32)
    }
}

impl Sample for u16 {
    // Unsigned formats sit at half scale rather than at zero.
    fn to_f32(self) -> f32 {
        (f32::from(self) - f32::from(i16::MAX)) / -f32::from(i16::MIN)
    }
}

/// Where the chosen device id is kept between runs.
///
/// One line of text in the user's config directory, not a settings system: there is exactly
/// one thing to remember, and a format with structure would be a format to migrate later.
/// `$XDG_CONFIG_HOME` or `~/.config`, which is where a Linux desktop expects it; anything
/// else returns `None` and the tuner opens the default every time, which is a worse tuner
/// rather than a broken one.
fn choice_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::Path::new(&home).join(".config"))
        })?;

    Some(base.join("trastea").join("input-device"))
}

/// The device id chosen last time, if there was one and it can still be read.
///
/// Every failure is `None`: no file, an unreadable one, a home directory that is not where
/// it should be. A remembered device is a convenience, and none of those is worth refusing
/// to start a tuner over.
pub fn remembered_device() -> Option<String> {
    read_choice(&choice_path()?)
}

/// Remembers this device id for next time. Best effort, for the reason above.
pub fn remember_device(id: &str) {
    let Some(path) = choice_path() else { return };
    write_choice(&path, id);
}

/// The two above, with the location passed in rather than read from the environment.
///
/// Split out so the tests can drive them against a temporary file. The alternative — a
/// test that points `XDG_CONFIG_HOME` somewhere safe — is what this replaced: `set_var`
/// mutates process-global state, the test harness runs tests on parallel threads, and
/// every test that opens the Tuner reads that same variable through `choice_path`. Edition
/// 2024 made `set_var` `unsafe` for exactly that reason, and it is undefined behaviour
/// rather than merely flaky. Passing the path in removes the shared state instead of
/// timing around it.
fn read_choice(path: &std::path::Path) -> Option<String> {
    let id = std::fs::read_to_string(path).ok()?;
    let id = id.trim();

    (!id.is_empty()).then(|| id.to_owned())
}

fn write_choice(path: &std::path::Path, id: &str) {
    if let Some(directory) = path.parent() {
        let _ = std::fs::create_dir_all(directory);
    }

    let _ = std::fs::write(path, id);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opening a device needs hardware, so what is tested here is everything on the near
    /// side of that: the conversions, the classification of failures, and the buffer's
    /// behaviour when two threads reach it at once.

    #[test]
    fn the_first_channel_is_taken_from_an_interleaved_buffer() {
        // What a 2-in interface delivers: [in1, in2, in1, in2, ...].
        let frames: Vec<f32> = vec![1.0, -1.0, 2.0, -2.0, 3.0, -3.0];

        let mono: Vec<f32> = deinterleave(&frames, 2).collect();

        assert_eq!(mono, vec![1.0, 2.0, 3.0]);
        // Emphatically not the interleaved buffer: passing that through reads every pitch
        // an octave low, confidently, because the interleaving is itself periodic.
        assert_ne!(mono, frames);
    }

    #[test]
    fn a_single_channel_source_passes_through() {
        let frames: Vec<f32> = vec![0.25, 0.5, -0.25];
        assert_eq!(deinterleave(&frames, 1).collect::<Vec<_>>(), frames);
    }

    #[test]
    fn a_channel_count_of_zero_does_not_divide_by_it() {
        // Nothing should report zero channels, but a stride of zero is an infinite loop
        // rather than a wrong answer, so it is clamped rather than trusted.
        let frames: Vec<f32> = vec![0.1, 0.2];
        assert_eq!(deinterleave(&frames, 0).count(), 2);
    }

    #[test]
    fn every_format_scales_onto_the_same_range() {
        // Full scale is 1.0 whatever the device's native type is, so one level gate and
        // one clarity threshold work for all of them.
        assert!((f32::to_f32(1.0) - 1.0).abs() < 1e-6);
        assert!((i16::MAX.to_f32() - 1.0).abs() < 1e-3);
        assert!((i32::MAX.to_f32() - 1.0).abs() < 1e-3);
        assert!((u16::MAX.to_f32() - 1.0).abs() < 1e-3);

        // And silence is zero in all of them, including the unsigned one, where silence
        // sits at half scale rather than at the bottom.
        assert!(f32::to_f32(0.0).abs() < 1e-6);
        assert!(0i16.to_f32().abs() < 1e-3);
        assert!(0i32.to_f32().abs() < 1e-3);
        assert!((i16::MAX as u16).to_f32().abs() < 1e-3);
    }

    #[test]
    fn integer_and_float_devices_agree_on_a_signal() {
        // The same half-scale ramp in three formats lands in the same place.
        let floats = [0.5f32, -0.5, 0.0];
        let shorts = [i16::MAX / 2, i16::MIN / 2, 0];

        for (&f, &s) in floats.iter().zip(shorts.iter()) {
            assert!(
                (f - s.to_f32()).abs() < 1e-3,
                "{f} and {} disagree",
                s.to_f32()
            );
        }
    }

    fn shared() -> Arc<Shared> {
        Arc::new(Shared {
            samples: Mutex::new(Vec::with_capacity(WINDOW)),
            lost: AtomicBool::new(false),
            reason: Mutex::new(None),
        })
    }

    #[test]
    fn the_buffer_keeps_the_newest_samples_and_drops_the_oldest() {
        let shared = shared();

        // Far more than the cap, in identifiable order.
        for chunk in 0..8 {
            let block: Vec<f32> = (0..WINDOW).map(|i| (chunk * WINDOW + i) as f32).collect();
            collect(&block, 1, &shared);
        }

        let held = shared.samples.lock().expect("uncontended");
        assert!(
            held.len() <= WINDOW * 2 + WINDOW,
            "the buffer grew to {} without a reader",
            held.len()
        );

        // What survived is the tail, in order — the oldest is what a starved buffer loses.
        let newest = held[held.len() - 1];
        assert_eq!(newest, (8 * WINDOW - 1) as f32);
        assert!(
            held.windows(2).all(|pair| pair[1] > pair[0]),
            "the samples that survived are out of order"
        );
    }

    #[test]
    fn the_callback_drops_its_chunk_rather_than_waiting_for_the_reader() {
        let shared = shared();
        let held = shared.samples.lock().expect("the test takes it first");

        // With the lock held, a callback must return rather than block. If it blocked,
        // this test would deadlock rather than fail — which is itself the assertion.
        collect(&[0.5f32; 64], 1, &shared);

        assert!(
            held.is_empty(),
            "the callback wrote while the lock was held"
        );
    }

    #[test]
    fn a_full_window_is_handed_over_exactly_once() {
        let mut held = vec![0.25f32; WINDOW * 2];

        let first = take(&mut held).expect("a full window had arrived");
        assert_eq!(
            first.len(),
            WINDOW,
            "the window was not the specified length"
        );
        assert!(
            take(&mut held).is_none(),
            "the same samples were handed over twice"
        );
    }

    #[test]
    fn the_next_window_arrives_one_hop_later_and_overlaps_the_last() {
        let mut held: Vec<f32> = (0..WINDOW).map(|i| i as f32).collect();
        let first = take(&mut held).expect("a full window had arrived");

        held.extend((WINDOW..WINDOW + HOP - 1).map(|i| i as f32));
        assert!(take(&mut held).is_none(), "a window arrived short of a hop");

        held.push((WINDOW + HOP - 1) as f32);
        let second = take(&mut held).expect("a hop had arrived");
        assert_eq!(
            second[..WINDOW - HOP],
            first[HOP..],
            "consecutive windows did not share their overlap"
        );
    }

    #[test]
    fn cpal_failures_keep_their_classification() {
        use cpal::Error;

        assert_eq!(
            InputError::from(Error::new(ErrorKind::DeviceNotAvailable)),
            InputError::NoDevice
        );
        assert_eq!(
            InputError::from(Error::new(ErrorKind::HostUnavailable)),
            InputError::NoDevice
        );
        assert_eq!(
            InputError::from(Error::new(ErrorKind::DeviceBusy)),
            InputError::Busy
        );
        assert_eq!(
            InputError::from(Error::new(ErrorKind::PermissionDenied)),
            InputError::NotPermitted
        );

        // Anything else keeps whatever the host said rather than being flattened.
        let other = Error::with_message(ErrorKind::UnsupportedConfig, "48 kHz only");
        assert_eq!(
            InputError::from(other),
            InputError::Unavailable("48 kHz only".to_owned())
        );
    }

    #[test]
    fn every_failure_says_something_a_learner_can_act_on() {
        for error in [
            InputError::NoDevice,
            InputError::Busy,
            InputError::NotPermitted,
            InputError::Unavailable("the reason".to_owned()),
            InputError::Lost("the reason".to_owned()),
        ] {
            let said = error.to_string();
            assert!(!said.is_empty(), "{error:?} says nothing");
            assert!(
                said.chars().next().is_some_and(char::is_lowercase),
                "{error:?} starts with a capital, unlike the rest of the app's prose"
            );
        }
    }

    #[test]
    fn a_lost_device_is_reported_once_it_is_flagged() {
        let shared = shared();
        assert!(!shared.lost.load(Ordering::Relaxed));

        *shared.reason.lock().expect("uncontended") = Some("unplugged".to_owned());
        shared.lost.store(true, Ordering::Relaxed);

        assert!(shared.lost.load(Ordering::Relaxed));
        assert_eq!(
            shared.reason.lock().expect("uncontended").clone(),
            Some("unplugged".to_owned())
        );
    }
}

#[cfg(test)]
mod device_tests {
    use super::*;

    #[test]
    fn plugin_ids_are_not_offered_and_hardware_ids_are() {
        // The ids ALSA actually produced on the development machine.
        for plugin in [
            "alsa:null",
            "alsa:pulse",
            "alsa:pipewire",
            "alsa:jack",
            "alsa:speexrate",
            "alsa:samplerate",
            "alsa:lavrate",
            "alsa:upmix",
            "alsa:vdownmix",
            "alsa:default",
            "alsa:sysdefault",
        ] {
            assert!(!is_hardware(plugin), "{plugin} was offered");
        }

        for hardware in [
            "alsa:sysdefault:CARD=USB",
            "alsa:front:CARD=USB,DEV=0",
            "alsa:hw:CARD=4,DEV=0",
            "alsa:plughw:CARD=4,DEV=0",
            "alsa:sysdefault:CARD=A7",
        ] {
            assert!(is_hardware(hardware), "{hardware} was hidden");
        }
    }

    #[test]
    fn a_host_without_a_plugin_chain_loses_nothing() {
        // No other backend's ids look like ALSA's, so the ALSA-shaped rule must not reach
        // them — a CoreAudio UID or a WASAPI id has no `CARD=` and must still be offered.
        for elsewhere in [
            "coreaudio:AppleUSBAudioEngine:Focusrite:Scarlett 2i2:1",
            "wasapi:{0.0.1.00000000}.{9c7b1b2a}",
            "asio:Focusrite USB ASIO",
        ] {
            assert!(is_hardware(elsewhere), "{elsewhere} was hidden");
        }
    }

    #[test]
    fn the_filter_rests_on_the_id_rather_than_the_name() {
        // The display names are prose and translatable; the rule must not consult them.
        // "Rate Converter Plugin Using Speex Resampler" is `alsa:speexrate`, and a machine
        // in another locale reports the same id with a different name.
        assert!(!is_hardware("alsa:speexrate"));
        assert!(is_hardware("alsa:sysdefault:CARD=Generic_1"));
    }

    #[test]
    fn devices_are_offered_once_each() {
        // Runs against whatever this machine has, so it asserts the invariant rather than a
        // particular list: no duplicate names, and nothing that failed the rules above.
        let offered = devices();

        let mut names: Vec<&str> = offered.iter().map(DeviceChoice::name).collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(before, names.len(), "a device was offered twice");

        for device in &offered {
            assert!(is_hardware(device.id()), "{} is not hardware", device.id());
            assert!(!device.name().is_empty(), "{} has no name", device.id());
        }
    }

    #[test]
    fn a_remembered_choice_is_recovered() {
        // A file of this test's own. Nothing here touches the process environment, so it
        // cannot race the tests running beside it — several of which open the Tuner and
        // read the real `choice_path`.
        let path = std::env::temp_dir().join(format!(
            "trastea-input-choice-{}-{:?}.txt",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);

        assert_eq!(read_choice(&path), None, "nothing was written yet");

        write_choice(&path, "alsa:sysdefault:CARD=USB");
        assert_eq!(
            read_choice(&path),
            Some("alsa:sysdefault:CARD=USB".to_owned())
        );

        // A blank or whitespace-only file is nothing remembered rather than an empty id,
        // which would otherwise be asked for and never found.
        write_choice(&path, "   \n");
        assert_eq!(read_choice(&path), None);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_remembered_choice_lives_where_a_linux_desktop_expects_it() {
        // The real path is still exercised, just not mutated: it has to end up under a
        // config directory rather than beside the binary or in the working directory.
        if let Some(path) = choice_path() {
            assert!(
                path.ends_with("trastea/input-device"),
                "the choice would be written to {path:?}"
            );
        }
    }
}

#[cfg(test)]
mod fallback_tests {
    use super::*;

    #[test]
    fn a_remembered_device_that_is_gone_falls_back_and_says_so() {
        // Needs a default device to fall back *to*. Where there is none, the interesting
        // assertion is the other one: that asking for a missing device is still an error
        // about there being no device rather than a panic.
        let asking_for_nothing = Input::open_id(Some("no-such-device-anywhere"));

        match asking_for_nothing {
            Ok(input) => {
                assert!(
                    input.fell_back(),
                    "a device that does not exist was opened without reporting a fallback"
                );
                assert!(!input.device_name().is_empty());
            }
            Err(error) => assert_eq!(
                error,
                InputError::NoDevice,
                "a missing device should read as there being none, not as {error:?}"
            ),
        }
    }

    #[test]
    fn the_default_is_not_a_fallback() {
        // Asking for nothing in particular and getting the default is a choice, not a
        // failure — the screen must not claim a remembered device went missing.
        if let Ok(input) = Input::open() {
            assert!(
                !input.fell_back(),
                "the default reported itself as a fallback"
            );
        }
    }
}
