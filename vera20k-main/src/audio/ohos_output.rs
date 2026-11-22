//! OpenHarmony audio output through an OHAudio render callback.
//!
//! cpal has no OpenHarmony backend, so the shared rodio mixer feeds an
//! `OH_AudioRenderer` directly: the system asks for a block, the callback
//! pulls the block's samples from the mixer and writes f32 stereo at 48 kHz.
//! The music and SFX players above this point stay unchanged.
//!
//! If the renderer cannot be opened, the mixer is still consumed at real time
//! on a thread (the M1 behaviour) so players keep reaching their end and the
//! voice queues keep releasing.

use std::ffi::c_void;
use std::num::NonZero;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use rodio::mixer::{Mixer, MixerSource};

const CHANNELS: u16 = 2;
const SAMPLE_RATE: u32 = 48_000;
const BLOCK_MILLIS: u64 = 10;

const AUDIOSTREAM_SUCCESS: i32 = 0;
const AUDIOSTREAM_TYPE_RENDERER: i32 = 1;
const AUDIOSTREAM_SAMPLE_F32LE: i32 = 4;
const AUDIOSTREAM_ENCODING_TYPE_RAW: i32 = 0;
const AUDIOSTREAM_USAGE_GAME: i32 = 11;
const AUDIOSTREAM_LATENCY_MODE_NORMAL: i32 = 0;
const AUDIO_DATA_CALLBACK_RESULT_INVALID: i32 = -1;
const AUDIO_DATA_CALLBACK_RESULT_VALID: i32 = 0;

const AUDIOSTREAM_INTERRUPT_HINT_RESUME: i32 = 1;
const AUDIOSTREAM_STATE_RUNNING: i32 = 2;
const AUDIOSTREAM_STATE_RELEASED: i32 = 4;

#[repr(C)]
struct OH_AudioStreamBuilder {
    _private: [u8; 0],
}

#[repr(C)]
struct OH_AudioRenderer {
    _private: [u8; 0],
}

#[link(name = "ohaudio")]
unsafe extern "C" {
    fn OH_AudioStreamBuilder_Create(
        builder: *mut *mut OH_AudioStreamBuilder,
        stream_type: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_Destroy(builder: *mut OH_AudioStreamBuilder) -> i32;
    fn OH_AudioStreamBuilder_SetSamplingRate(
        builder: *mut OH_AudioStreamBuilder,
        rate: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetChannelCount(
        builder: *mut OH_AudioStreamBuilder,
        channel_count: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetSampleFormat(
        builder: *mut OH_AudioStreamBuilder,
        sample_format: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetEncodingType(
        builder: *mut OH_AudioStreamBuilder,
        encoding_type: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetLatencyMode(
        builder: *mut OH_AudioStreamBuilder,
        latency_mode: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetRendererInfo(
        builder: *mut OH_AudioStreamBuilder,
        usage: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetRendererWriteDataCallback(
        builder: *mut OH_AudioStreamBuilder,
        callback: Option<
            unsafe extern "C" fn(
                renderer: *mut OH_AudioRenderer,
                user_data: *mut c_void,
                audio_data: *mut c_void,
                audio_data_size: i32,
            ) -> i32,
        >,
        user_data: *mut c_void,
    ) -> i32;
    fn OH_AudioStreamBuilder_GenerateRenderer(
        builder: *mut OH_AudioStreamBuilder,
        renderer: *mut *mut OH_AudioRenderer,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetRendererInterruptCallback(
        builder: *mut OH_AudioStreamBuilder,
        callback: Option<unsafe extern "C" fn(*mut OH_AudioRenderer, *mut c_void, i32, i32)>,
        user_data: *mut c_void,
    ) -> i32;
    fn OH_AudioRenderer_Start(renderer: *mut OH_AudioRenderer) -> i32;
    fn OH_AudioRenderer_GetCurrentState(renderer: *mut OH_AudioRenderer, state: *mut i32) -> i32;
    fn OH_AudioRenderer_Stop(renderer: *mut OH_AudioRenderer) -> i32;
    fn OH_AudioRenderer_Release(renderer: *mut OH_AudioRenderer) -> i32;
    fn OH_AudioRenderer_SetVolume(renderer: *mut OH_AudioRenderer, volume: f32) -> i32;
}

/// Handle to the shared mixer. The renderer (or the fallback consumer thread)
/// lives as long as the sink.
pub struct OhosAudioSink {
    mixer: Mixer,
    renderer: usize,
    source: usize,
}

impl OhosAudioSink {
    pub fn open() -> Result<Self, String> {
        let (mixer, source) = rodio::mixer::mixer(
            NonZero::new(CHANNELS).expect("stereo"),
            NonZero::new(SAMPLE_RATE).expect("48 kHz"),
        );
        let slot = Box::into_raw(Box::new(Mutex::new(source)));
        match open_renderer(slot) {
            Ok(renderer) => {
                log::info!(
                    "OHAudio renderer started: {SAMPLE_RATE} Hz stereo f32, usage game, normal latency"
                );
                Ok(Self {
                    mixer,
                    renderer: renderer as usize,
                    source: slot as usize,
                })
            }
            Err(error) => {
                log::warn!(
                    "OHAudio renderer unavailable ({error}); falling back to silent real-time mixer"
                );
                // SAFETY: the renderer never took ownership of the slot.
                let source = unsafe { Box::from_raw(slot) };
                start_silent_consumer(source);
                Ok(Self {
                    mixer,
                    renderer: 0,
                    source: 0,
                })
            }
        }
    }

    pub fn mixer(&self) -> &Mixer {
        &self.mixer
    }

    /// Restart a renderer the system stopped without a later resume hint.
    /// Another app taking the media focus sends STOP and no RESUME, so the
    /// window's activation edge is where the game wins its output back.
    pub fn resume_after_interrupt(&self) {
        if self.renderer == 0 {
            return;
        }
        start_if_not_running(self.renderer as *mut OH_AudioRenderer);
    }
}

impl Drop for OhosAudioSink {
    fn drop(&mut self) {
        if self.renderer != 0 {
            let renderer = self.renderer as *mut OH_AudioRenderer;
            // SAFETY: the handle came from a successful GenerateRenderer and
            // is released exactly once, here.
            unsafe {
                OH_AudioRenderer_Stop(renderer);
                OH_AudioRenderer_Release(renderer);
            }
            self.renderer = 0;
        }
        if self.source != 0 {
            // SAFETY: Release returned, so no callback can still be running
            // with this user data.
            unsafe {
                drop(Box::from_raw(self.source as *mut Mutex<MixerSource>));
            }
            self.source = 0;
        }
    }
}

fn open_renderer(slot: *mut Mutex<MixerSource>) -> Result<*mut OH_AudioRenderer, String> {
    let mut builder: *mut OH_AudioStreamBuilder = std::ptr::null_mut();
    // SAFETY: the out-parameter starts null and is written by the call.
    let status = unsafe { OH_AudioStreamBuilder_Create(&mut builder, AUDIOSTREAM_TYPE_RENDERER) };
    if status != AUDIOSTREAM_SUCCESS || builder.is_null() {
        return Err(format!("create builder failed with status {status}"));
    }
    // SAFETY: `builder` is live until destroyed below.
    let mut configured: Option<(&'static str, i32)> = None;
    unsafe {
        let steps = [
            (
                "sampling rate",
                OH_AudioStreamBuilder_SetSamplingRate(builder, SAMPLE_RATE as i32),
            ),
            (
                "channel count",
                OH_AudioStreamBuilder_SetChannelCount(builder, i32::from(CHANNELS)),
            ),
            (
                "sample format",
                OH_AudioStreamBuilder_SetSampleFormat(builder, AUDIOSTREAM_SAMPLE_F32LE),
            ),
            (
                "encoding type",
                OH_AudioStreamBuilder_SetEncodingType(builder, AUDIOSTREAM_ENCODING_TYPE_RAW),
            ),
            (
                "usage",
                OH_AudioStreamBuilder_SetRendererInfo(builder, AUDIOSTREAM_USAGE_GAME),
            ),
            (
                "latency mode",
                OH_AudioStreamBuilder_SetLatencyMode(builder, AUDIOSTREAM_LATENCY_MODE_NORMAL),
            ),
            (
                "write callback",
                OH_AudioStreamBuilder_SetRendererWriteDataCallback(
                    builder,
                    Some(write_data),
                    slot.cast(),
                ),
            ),
            (
                "interrupt callback",
                OH_AudioStreamBuilder_SetRendererInterruptCallback(
                    builder,
                    Some(interrupt),
                    slot.cast(),
                ),
            ),
        ];
        for (step, status) in steps {
            if status != AUDIOSTREAM_SUCCESS {
                configured = Some((step, status));
                break;
            }
        }
    }
    let mut renderer: *mut OH_AudioRenderer = std::ptr::null_mut();
    let generated = if configured.is_none() {
        // SAFETY: the out-parameter is a valid local.
        unsafe { OH_AudioStreamBuilder_GenerateRenderer(builder, &mut renderer) }
    } else {
        AUDIOSTREAM_SUCCESS
    };
    // SAFETY: the builder is not used after this point.
    unsafe { OH_AudioStreamBuilder_Destroy(builder) };
    if let Some((step, status)) = configured {
        return Err(format!("set {step} failed with status {status}"));
    }
    if generated != AUDIOSTREAM_SUCCESS || renderer.is_null() {
        return Err(format!("generate renderer failed with status {generated}"));
    }
    // SAFETY: `renderer` was generated above and has not been started.
    let status = unsafe { OH_AudioRenderer_Start(renderer) };
    if status != AUDIOSTREAM_SUCCESS {
        // SAFETY: the handle is live and never reaches the sink.
        unsafe {
            OH_AudioRenderer_Release(renderer);
        }
        return Err(format!("start renderer failed with status {status}"));
    }
    // SAFETY: `renderer` is live and owned by this function until returned.
    let status = unsafe { OH_AudioRenderer_SetVolume(renderer, 1.0) };
    if status != AUDIOSTREAM_SUCCESS {
        log::warn!("OH_AudioRenderer_SetVolume(1.0) failed with status {status}");
    }
    Ok(renderer)
}

/// A forced interrupt (a call, an alarm, voice input) leaves the renderer
/// paused or stopped on the system side; the matching resume hint is where
/// the app starts it again. Duck and mute hints need no action: the system
/// already applied the volume while mixing.
unsafe extern "C" fn interrupt(
    renderer: *mut OH_AudioRenderer,
    _user_data: *mut c_void,
    force_type: i32,
    hint: i32,
) {
    log::info!("OHAudio interrupt: type {force_type} hint {hint}");
    if hint == AUDIOSTREAM_INTERRUPT_HINT_RESUME {
        start_if_not_running(renderer);
    }
}

fn start_if_not_running(renderer: *mut OH_AudioRenderer) {
    if renderer.is_null() {
        return;
    }
    let mut state = 0i32;
    // SAFETY: the renderer is live until the sink releases it.
    let status = unsafe { OH_AudioRenderer_GetCurrentState(renderer, &mut state) };
    if status != AUDIOSTREAM_SUCCESS {
        log::warn!("OHAudio state query failed with status {status}");
        return;
    }
    if state == AUDIOSTREAM_STATE_RUNNING || state == AUDIOSTREAM_STATE_RELEASED {
        return;
    }
    // SAFETY: a paused or stopped renderer can be started again.
    let status = unsafe { OH_AudioRenderer_Start(renderer) };
    if status != AUDIOSTREAM_SUCCESS {
        log::warn!("OHAudio renderer did not restart after an interrupt: {status}");
    }
}

/// Pulls one block from the mixer. The system calls this on its audio thread;
/// an idle mixer yields `None`, which plays as silence.
unsafe extern "C" fn write_data(
    _renderer: *mut OH_AudioRenderer,
    user_data: *mut c_void,
    audio_data: *mut c_void,
    audio_data_size: i32,
) -> i32 {
    static FIRST_BLOCK: AtomicBool = AtomicBool::new(true);
    if user_data.is_null() || audio_data.is_null() || audio_data_size <= 0 {
        return AUDIO_DATA_CALLBACK_RESULT_INVALID;
    }
    let bytes = audio_data_size as usize;
    let samples = bytes / 4;
    if FIRST_BLOCK.swap(false, Ordering::Relaxed) {
        log::info!("OHAudio first callback: {samples} samples per block");
    }
    // SAFETY: OHAudio hands us a writable buffer of `audio_data_size` bytes,
    // and the slot is the one handed to the builder, kept alive by the sink
    // until the renderer is released.
    let out = unsafe { core::slice::from_raw_parts_mut(audio_data.cast::<f32>(), samples) };
    let slot = unsafe { &*(user_data as *const Mutex<MixerSource>) };
    match slot.lock() {
        Ok(mut source) => {
            for sample in out.iter_mut() {
                *sample = source.next().unwrap_or(0.0);
            }
        }
        Err(_) => out.fill(0.0),
    }
    if bytes % 4 != 0 {
        // OHAudio block sizes are sample-aligned; keep the tail silent if not.
        // SAFETY: the tail bytes are inside the same buffer.
        unsafe {
            core::ptr::write_bytes(
                (audio_data as *mut u8).add(samples * 4),
                0,
                bytes % 4,
            );
        }
    }
    AUDIO_DATA_CALLBACK_RESULT_VALID
}

/// The M1 consumer, used only when OHAudio cannot start.
fn start_silent_consumer(source: Box<Mutex<MixerSource>>) {
    let spawned = thread::Builder::new()
        .name("vera20k-audio".to_owned())
        .spawn(move || {
            let block_samples =
                SAMPLE_RATE as usize / 1000 * BLOCK_MILLIS as usize * usize::from(CHANNELS);
            loop {
                let started = Instant::now();
                if let Ok(mut mixer) = source.lock() {
                    for _ in 0..block_samples {
                        let _ = mixer.next();
                    }
                }
                let pace = Duration::from_millis(BLOCK_MILLIS);
                if let Some(rest) = pace.checked_sub(started.elapsed()) {
                    thread::sleep(rest);
                }
            }
        });
    if let Err(error) = spawned {
        log::error!("Could not start the silent audio consumer: {error}");
    }
}