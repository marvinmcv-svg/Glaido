//! Vocalis audio pipeline (master prompt §5.2 "Audio").
//!
//! capture (cpal, feature `capture`) → [`dsp::downmix`] → [`dsp::Resampler`] to
//! 16 kHz mono → [`level::LevelMeter`] for the bar and settings meter →
//! [`vad::Vad`] for silence detection → [`journal`] (AES-256-GCM, crash-safe)
//! and the network stream. Everything except capture is pure and tested.

pub mod dsp;
pub mod journal;
pub mod level;
pub mod vad;
pub mod wav;

#[cfg(feature = "capture")]
pub mod capture;

/// Sample rate sent to speech-to-text providers.
pub const TARGET_RATE: u32 = 16_000;
/// Frame length used by the VAD and level meter: 20 ms at 16 kHz.
pub const FRAME_SAMPLES: usize = 320;
