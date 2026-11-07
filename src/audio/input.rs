//! Opening a capture device and getting its samples to the detector.
//!
//! The audio callback may not allocate, block, or panic, so it only copies; every decision
//! lives in `detect`.

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, ErrorKind, FrameCount, SampleFormat, SupportedBufferSize};

use super::detect::{HOP, WINDOW};
use super::inharmonicity::FIT_WINDOW;

/// Why there is nothing to listen to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputError {
    /// The host reported no capture device at all.
    NoDevice,
    /// A device exists but another program holds it.
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
    /// Keeps cpal's classification so the screen never has to parse the message.
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

/// One input device the learner may choose.
///
/// Keyed by cpal's host id rather than an index, which shifts whenever anything is plugged in.
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

        // also drops ALSA's `usbstream:` entries, which enumerate but never open
        if device.default_input_config().is_err() {
            continue;
        }

        // ALSA lists the same card as `sysdefault:`, `front:`, `hw:`, `plughw:`; first wins
        // because `sysdefault` comes first and converts format and rate
        let name = description.name().to_owned();
        if offered.iter().any(|offer| offer.name == name) {
            continue;
        }

        offered.push(DeviceChoice { id, name });
    }

    offered
}

/// Whether an id names a real capture device rather than an ALSA plugin or a pulse monitor.
///
/// Tests the id, not the translatable display name.
fn is_hardware(id: &str) -> bool {
    match id.split_once(':') {
        Some(("alsa", pcm)) => pcm.contains("CARD="),
        Some(("pulseaudio", source)) => !source.ends_with(".monitor"),
        _ => true,
    }
}

/// The most recent window of audio, written by the audio thread and read by whoever asks.
///
/// The callback only ever `try_lock`s and drops its chunk if the reader holds the lock.
#[derive(Debug)]
struct Shared {
    /// Whole interleaved frames; the reader picks the channel.
    samples: Mutex<Vec<f32>>,
    /// Non-zero by type, because a zero stride panics and the callback may not panic.
    channels: NonZeroUsize,
    /// Set by the error callback. `Relaxed` because no other memory is ordered against it.
    lost: AtomicBool,
    reason: Mutex<Option<String>>,
    /// The longer stretch the inharmonicity fit reads, which detection never drains.
    history: Mutex<History>,
}

/// The newest frames, up to the fit's window and the most that can arrive past it.
#[derive(Debug)]
struct History {
    frames: Vec<f32>,
    /// Frames that landed after the last window handed to detection, which the fit window
    /// stops short of so that both end on the same sample.
    since_window: usize,
}

/// An open capture device; dropping it closes the stream.
///
/// Owned by the app, not a subscription, which iced may drop and rebuild when its hash changes.
pub struct Input {
    /// Never read; dropping it stops the audio.
    _stream: cpal::Stream,
    shared: Arc<Shared>,
    device_name: String,
    sample_rate: f32,
    /// The device asked for was missing and the default was opened instead.
    fell_back: bool,
}

impl Input {
    /// Opens the device with this id, or the system default when `wanted` is `None` or names
    /// a device that is not present.
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
        let channels = NonZeroUsize::new(usize::from(config.channels())).ok_or_else(|| {
            InputError::Unavailable("the device reports no input channels".to_owned())
        })?;

        let shared = Arc::new(Shared::new(channels));

        // a device that refuses the size it reported is opened as it would have been before
        let stream = build_stream(
            &device,
            &config,
            hop_sized(config.buffer_size()),
            Arc::clone(&shared),
        )
        .or_else(|_| build_stream(&device, &config, BufferSize::Default, Arc::clone(&shared)))?;
        stream.play()?;

        Ok(Input {
            _stream: stream,
            shared,
            device_name,
            sample_rate,
            fell_back,
        })
    }

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
        self.shared.take_window()
    }

    /// The [`FIT_WINDOW`] frames ending where the last [`Input::take_window`] did, or `None`
    /// until that many have arrived. Copies without draining; this and the retention behind it
    /// are all `input.rs` gains, since when to fit is decided on the side that can be tested.
    pub fn take_fit_window(&self) -> Option<Vec<f32>> {
        self.shared.take_fit_window()
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

/// One hop of frames, within what the device says it can deliver.
///
/// Asked for rather than left to the host: pulse, and PipeWire through it, otherwise hands a
/// record stream its server-default fragment, and audio arriving in bursts reaches the
/// detector as one window per burst.
fn hop_sized(supported: &SupportedBufferSize) -> BufferSize {
    const HOP_FRAMES: FrameCount = HOP as FrameCount;

    match *supported {
        // not `clamp`, which panics on a range a driver reported backwards
        SupportedBufferSize::Range { min, max } => BufferSize::Fixed(HOP_FRAMES.max(min).min(max)),
        SupportedBufferSize::Unknown => BufferSize::Default,
    }
}

/// Builds the stream for whichever sample format the device offers.
fn build_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    buffer_size: BufferSize,
    shared: Arc<Shared>,
) -> Result<cpal::Stream, InputError> {
    let errors = Arc::clone(&shared);
    let on_error = move |err: cpal::Error| {
        // runs on the audio thread: record and return
        if let Ok(mut reason) = errors.reason.try_lock() {
            *reason = Some(err.to_string());
        }
        errors.lost.store(true, Ordering::Relaxed);
    };

    let stream_config = cpal::StreamConfig {
        buffer_size,
        ..config.config()
    };

    macro_rules! build {
        ($sample:ty) => {
            device.build_input_stream(
                stream_config.clone(),
                move |data: &[$sample], _: &cpal::InputCallbackInfo| {
                    collect(data, &shared);
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

/// The most frames the buffer ever holds, reserved up front so the callback never allocates.
const CAPACITY: usize = WINDOW * 2;

/// The same for the history: the fit's window, plus room for what arrives past detection's.
const HISTORY: usize = FIT_WINDOW + CAPACITY;

impl Shared {
    fn new(channels: NonZeroUsize) -> Shared {
        Shared {
            samples: Mutex::new(Vec::with_capacity(CAPACITY * channels.get())),
            channels,
            lost: AtomicBool::new(false),
            reason: Mutex::new(None),
            history: Mutex::new(History {
                frames: Vec::with_capacity(HISTORY * channels.get()),
                since_window: 0,
            }),
        }
    }

    fn take_window(&self) -> Option<Vec<f32>> {
        let mut held = self.samples.lock().ok()?;
        let window = take(&mut held, self.channels)?;

        // still under detection's lock, so no chunk can land between the window and this
        if let Ok(mut history) = self.history.lock() {
            history.since_window = 0;
        }

        Some(window)
    }

    fn take_fit_window(&self) -> Option<Vec<f32>> {
        let history = self.history.lock().ok()?;
        fit_window(&history.frames, history.since_window, self.channels)
    }
}

/// The body of the audio callback.
fn collect<S: Sample>(data: &[S], shared: &Shared) {
    let Ok(mut held) = shared.samples.try_lock() else {
        // the reader has it; drop the chunk rather than wait
        return;
    };
    // both or neither, so the history never holds a chunk detection lost, or the reverse
    let Ok(mut history) = shared.history.try_lock() else {
        return;
    };

    let channels = shared.channels.get();
    append(&mut held, data, channels, CAPACITY);
    let incoming = append(&mut history.frames, data, channels, HISTORY);
    history.since_window = history.since_window.saturating_add(incoming);
}

/// Appends `data`'s whole frames, making room first so `held` never outgrows `capacity`
/// frames, and returns how many arrived. Whole frames only, so the buffer never falls out of
/// step with its channels.
fn append<S: Sample>(held: &mut Vec<f32>, data: &[S], channels: usize, capacity: usize) -> usize {
    let frames = data.chunks_exact(channels);
    let incoming = frames.len();
    let keep = capacity.saturating_sub(incoming);

    let excess = held.len().saturating_sub(keep * channels);
    held.drain(..excess);

    held.extend(
        frames
            .skip(incoming.saturating_sub(capacity))
            .flatten()
            .map(|s| s.to_f32()),
    );

    incoming
}

/// The loudest channel of the newest `WINDOW` frames, keeping only the overlap the next
/// window will share, so the next one arrives one [`HOP`] later.
fn take(held: &mut Vec<f32>, channels: NonZeroUsize) -> Option<Vec<f32>> {
    let width = channels.get();
    if held.len() < WINDOW * width {
        return None;
    }

    // newest only, so latency does not depend on how long since the last redraw
    let frames = &held[held.len() - WINDOW * width..];
    let window = channel(frames, channels, loudest(frames, channels)).collect();

    let consumed = held.len() - (WINDOW - HOP) * width;
    held.drain(..consumed);
    Some(window)
}

/// The loudest channel of the [`FIT_WINDOW`] frames ending `since_window` frames before the
/// newest, or `None` rather than a window padded with silence.
fn fit_window(frames: &[f32], since_window: usize, channels: NonZeroUsize) -> Option<Vec<f32>> {
    let width = channels.get();
    let end = frames
        .len()
        .checked_sub(since_window.saturating_mul(width))?;
    let start = end.checked_sub(FIT_WINDOW * width)?;

    let frames = &frames[start..end];
    Some(channel(frames, channels, loudest(frames, channels)).collect())
}

/// One channel of an interleaved buffer.
///
/// Passing interleaved stereo through as mono reads every pitch an octave low.
fn channel(frames: &[f32], channels: NonZeroUsize, which: usize) -> impl Iterator<Item = f32> + '_ {
    frames.iter().skip(which).step_by(channels.get()).copied()
}

/// The channel carrying the most energy across `frames`.
///
/// Walked backwards because `max_by` keeps the last of equals and a tie should go to input 1.
fn loudest(frames: &[f32], channels: NonZeroUsize) -> usize {
    (0..channels.get())
        .rev()
        .map(|which| {
            let energy: f32 = channel(frames, channels, which).map(|s| s * s).sum();
            (which, energy)
        })
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .map_or(0, |(which, _)| which)
}

/// Scaling whatever the device gives us onto the same range.
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

/// Where the app keeps what it remembers between runs, under `$XDG_CONFIG_HOME` or `~/.config`.
pub fn config_dir() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::Path::new(&home).join(".config"))
        })?;

    Some(base.join("trastea"))
}

/// Where the chosen device id is kept between runs.
fn choice_path() -> Option<std::path::PathBuf> {
    Some(config_dir()?.join("input-device"))
}

/// The device id chosen last time, if there was one and it can still be read.
pub fn remembered_device() -> Option<String> {
    read_choice(&choice_path()?)
}

/// Remembers this device id for next time. Best effort.
pub fn remember_device(id: &str) {
    let Some(path) = choice_path() else { return };
    write_choice(&path, id);
}

/// The two above with the path passed in, so tests need not `set_var` across threads.
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

    #[test]
    fn a_stream_asks_for_one_hop_at_a_time_within_what_the_device_allows() {
        let range = |min, max| SupportedBufferSize::Range { min, max };

        assert_eq!(hop_sized(&range(1, 1 << 20)), BufferSize::Fixed(HOP as u32));
        assert_eq!(hop_sized(&range(1024, 4096)), BufferSize::Fixed(1024));
        assert_eq!(hop_sized(&range(64, 256)), BufferSize::Fixed(256));
        assert_eq!(
            hop_sized(&SupportedBufferSize::Unknown),
            BufferSize::Default
        );
    }

    const MONO: NonZeroUsize = NonZeroUsize::MIN;
    const STEREO: NonZeroUsize = NonZeroUsize::new(2).unwrap();

    #[test]
    fn one_channel_is_taken_from_an_interleaved_buffer() {
        // What a 2-in interface delivers: [in1, in2, in1, in2, ...].
        let frames: Vec<f32> = vec![1.0, -1.0, 2.0, -2.0, 3.0, -3.0];

        let first: Vec<f32> = channel(&frames, STEREO, 0).collect();
        let second: Vec<f32> = channel(&frames, STEREO, 1).collect();

        assert_eq!(first, vec![1.0, 2.0, 3.0]);
        assert_eq!(second, vec![-1.0, -2.0, -3.0]);
        assert_ne!(first, frames);
    }

    #[test]
    fn a_single_channel_source_passes_through() {
        let frames: Vec<f32> = vec![0.25, 0.5, -0.25];
        assert_eq!(channel(&frames, MONO, 0).collect::<Vec<_>>(), frames);
    }

    /// `frames` of a sine at `hz` in input `into` of a `channels`-input interface, with a
    /// hiss far below it in every other input.
    fn plugged_into(into: usize, channels: usize, hz: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|frame| {
                let t = frame as f32 / 48_000.0;
                (0..channels).map(move |input| {
                    if input == into {
                        0.5 * (std::f32::consts::TAU * hz * t).sin()
                    } else if frame % 2 == 0 {
                        0.001
                    } else {
                        -0.001
                    }
                })
            })
            .collect()
    }

    #[test]
    fn the_loudest_input_is_the_one_heard() {
        for into in 0..4 {
            let frames = plugged_into(into, 4, 110.0, 64);
            assert_eq!(
                loudest(&frames, NonZeroUsize::new(4).unwrap()),
                into,
                "a guitar in input {} was not the loudest",
                into + 1
            );
        }
    }

    #[test]
    fn a_tie_goes_to_the_first_input() {
        // Silence, and a stereo pair carrying one source, are both ties.
        assert_eq!(loudest(&[0.0; 8], STEREO), 0);
        assert_eq!(loudest(&[0.3, 0.3, -0.2, -0.2], STEREO), 0);
    }

    #[test]
    fn a_guitar_in_the_second_input_is_named() {
        // end to end, from the callback to a pitch
        let shared = Shared::new(STEREO);
        collect(&plugged_into(1, 2, 110.0, WINDOW), &shared);

        let window = take(&mut shared.samples.lock().expect("uncontended"), STEREO)
            .expect("a full window had arrived");

        match super::super::detect::detect(&window, 48_000.0) {
            super::super::detect::Heard::Pitch(detection) => assert!(
                (detection.hz() - 110.0).abs() < 1.0,
                "heard {} Hz rather than 110",
                detection.hz()
            ),
            other => panic!("input 2 read as {other:?}"),
        }
    }

    #[test]
    fn every_format_scales_onto_the_same_range() {
        assert!((f32::to_f32(1.0) - 1.0).abs() < 1e-6);
        assert!((i16::MAX.to_f32() - 1.0).abs() < 1e-3);
        assert!((i32::MAX.to_f32() - 1.0).abs() < 1e-3);
        assert!((u16::MAX.to_f32() - 1.0).abs() < 1e-3);

        // unsigned silence sits at half scale
        assert!(f32::to_f32(0.0).abs() < 1e-6);
        assert!(0i16.to_f32().abs() < 1e-3);
        assert!(0i32.to_f32().abs() < 1e-3);
        assert!((i16::MAX as u16).to_f32().abs() < 1e-3);
    }

    #[test]
    fn integer_and_float_devices_agree_on_a_signal() {
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

    #[test]
    fn the_buffer_keeps_the_newest_samples_and_drops_the_oldest() {
        let shared = Shared::new(MONO);

        for chunk in 0..8 {
            let block: Vec<f32> = (0..WINDOW).map(|i| (chunk * WINDOW + i) as f32).collect();
            collect(&block, &shared);
        }

        let held = shared.samples.lock().expect("uncontended");
        assert!(
            held.len() <= CAPACITY,
            "the buffer grew to {} without a reader",
            held.len()
        );

        let newest = held[held.len() - 1];
        assert_eq!(newest, (8 * WINDOW - 1) as f32);
        assert!(
            held.windows(2).all(|pair| pair[1] > pair[0]),
            "the samples that survived are out of order"
        );
    }

    #[test]
    fn the_buffer_never_outgrows_what_was_reserved() {
        for channels in [MONO, STEREO] {
            let shared = Shared::new(channels);
            let width = channels.get();
            let reserved = shared.samples.lock().expect("uncontended").capacity();

            // includes a chunk longer than the whole buffer; input 1 counts frames
            let mut next = 0.0f32;
            for frames in [480, 1024, 3000, 1, 7000, 2048] {
                let block: Vec<f32> = (0..frames)
                    .flat_map(|frame| {
                        let n = next + frame as f32;
                        (0..width).map(move |input| if input == 0 { n } else { -n })
                    })
                    .collect();
                next += frames as f32;
                collect(&block, &shared);

                let held = shared.samples.lock().expect("uncontended");
                assert!(
                    held.len() <= CAPACITY * width,
                    "held {} after a {frames}-frame chunk",
                    held.len()
                );
                assert_eq!(
                    held.capacity(),
                    reserved,
                    "a {frames}-frame chunk reallocated"
                );
                assert_eq!(
                    held[held.len() - width],
                    next - 1.0,
                    "the newest frame was lost, or a frame was split"
                );
            }
        }
    }

    #[test]
    fn the_callback_drops_its_chunk_rather_than_waiting_for_the_reader() {
        let shared = Shared::new(MONO);
        let held = shared.samples.lock().expect("the test takes it first");

        // a blocking callback would deadlock here rather than fail
        collect(&[0.5f32; 64], &shared);

        assert!(
            held.is_empty(),
            "the callback wrote while the lock was held"
        );
    }

    #[test]
    fn a_full_window_is_handed_over_exactly_once() {
        let mut held = vec![0.25f32; WINDOW * 2];

        let first = take(&mut held, MONO).expect("a full window had arrived");
        assert_eq!(
            first.len(),
            WINDOW,
            "the window was not the specified length"
        );
        assert!(
            take(&mut held, MONO).is_none(),
            "the same samples were handed over twice"
        );
    }

    #[test]
    fn the_next_window_arrives_one_hop_later_and_overlaps_the_last() {
        let mut held: Vec<f32> = (0..WINDOW).map(|i| i as f32).collect();
        let first = take(&mut held, MONO).expect("a full window had arrived");

        held.extend((WINDOW..WINDOW + HOP - 1).map(|i| i as f32));
        assert!(
            take(&mut held, MONO).is_none(),
            "a window arrived short of a hop"
        );

        held.push((WINDOW + HOP - 1) as f32);
        let second = take(&mut held, MONO).expect("a hop had arrived");
        assert_eq!(
            second[..WINDOW - HOP],
            first[HOP..],
            "consecutive windows did not share their overlap"
        );
    }

    /// Feeds `frames` frames of a ramp — each sample its own index — in device-sized chunks.
    fn feed(shared: &Shared, from: usize, frames: usize) {
        let ramp: Vec<f32> = (from..from + frames).map(|i| i as f32).collect();
        for chunk in ramp.chunks(480) {
            collect(chunk, shared);
        }
    }

    #[test]
    fn the_fit_window_ends_where_detections_does() {
        let shared = Shared::new(MONO);
        feed(&shared, 0, FIT_WINDOW + WINDOW);

        let window = shared.take_window().expect("a full window had arrived");
        let fit = shared
            .take_fit_window()
            .expect("a full fit window had arrived");

        assert_eq!(
            fit.len(),
            FIT_WINDOW,
            "the fit window was not its specified length"
        );
        assert_eq!(
            fit.last(),
            window.last(),
            "the two windows end on different samples"
        );
        assert!(
            fit.windows(2).all(|pair| pair[1] == pair[0] + 1.0),
            "the fit window is not one unbroken stretch"
        );

        // audio arriving before the fit is asked for does not move where it ends
        feed(&shared, FIT_WINDOW + WINDOW, 300);
        assert_eq!(
            shared.take_fit_window().expect("still enough").last(),
            window.last(),
            "the fit window ran on past the note detection confirmed"
        );
    }

    #[test]
    fn taking_the_fit_window_leaves_detection_its_samples() {
        let shared = Shared::new(MONO);
        feed(&shared, 0, FIT_WINDOW + WINDOW);
        let first = shared.take_window().expect("a full window had arrived");

        let _ = shared.take_fit_window();
        let _ = shared.take_fit_window();
        feed(&shared, FIT_WINDOW + WINDOW, HOP);

        let second = shared.take_window().expect("a hop had arrived");
        assert_eq!(
            second[..WINDOW - HOP],
            first[HOP..],
            "copying the fit window drained what detection reads"
        );
    }

    #[test]
    fn a_fit_window_asked_for_too_early_is_not_padded() {
        let shared = Shared::new(MONO);
        feed(&shared, 0, WINDOW);

        assert!(shared.take_window().is_some(), "detection had its window");
        assert_eq!(
            shared.take_fit_window(),
            None,
            "a fit window was handed over before enough had arrived"
        );
    }

    #[test]
    fn the_callback_drops_its_chunk_rather_than_waiting_for_the_fit_window() {
        let shared = Shared::new(MONO);

        {
            let history = shared.history.lock().expect("the test takes it first");
            // a blocking callback would deadlock here rather than fail
            collect(&[0.5f32; 64], &shared);
            assert!(history.frames.is_empty());
        }

        assert!(
            shared.samples.lock().expect("uncontended").is_empty(),
            "detection kept a chunk the history dropped"
        );
    }

    #[test]
    fn the_history_never_outgrows_what_was_reserved() {
        let shared = Shared::new(STEREO);
        let reserved = shared
            .history
            .lock()
            .expect("uncontended")
            .frames
            .capacity();

        for frames in [480, 9000, 1, 30_000, 2048] {
            collect(&vec![0.25f32; frames * 2], &shared);

            let history = shared.history.lock().expect("uncontended");
            assert!(history.frames.len() <= HISTORY * 2);
            assert_eq!(
                history.frames.capacity(),
                reserved,
                "{frames} frames reallocated"
            );
        }
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
        let shared = Shared::new(MONO);
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
    fn a_sound_server_offers_its_sources_but_not_its_monitors() {
        assert!(is_hardware(
            "pulseaudio:alsa_input.usb-Focusrite_Scarlett_2i2_USB-00.pro-input-0"
        ));
        assert!(!is_hardware(
            "pulseaudio:alsa_output.usb-Focusrite_Scarlett_2i2_USB-00.pro-output-0.monitor"
        ));
        assert!(!is_hardware(
            "pulseaudio:alsa_output.pci-0000_03_00.1.hdmi-stereo.monitor"
        ));
    }

    #[test]
    fn a_host_without_a_plugin_chain_loses_nothing() {
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
        assert!(!is_hardware("alsa:speexrate"));
        assert!(is_hardware("alsa:sysdefault:CARD=Generic_1"));
    }

    #[test]
    fn devices_are_offered_once_each() {
        // runs against this machine's devices, so it asserts the invariant only
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

        write_choice(&path, "   \n");
        assert_eq!(read_choice(&path), None);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_remembered_choice_lives_where_a_linux_desktop_expects_it() {
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
        // without a default device, the missing one must still read as `NoDevice`
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
        if let Ok(input) = Input::open_id(None) {
            assert!(
                !input.fell_back(),
                "the default reported itself as a fallback"
            );
        }
    }
}
