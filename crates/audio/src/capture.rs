//! Live microphone capture via cpal (feature `capture`).
//!
//! The audio callback does the minimum: convert to f32, downmix to mono and
//! hand the block to a channel. Resampling, metering, VAD and the journal run
//! on the consumer thread, away from the real-time callback.

use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use thiserror::Error;

use crate::dsp::downmix;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("no microphone is available")]
    NoDevice,
    #[error("the microphone \"{0}\" was not found")]
    DeviceNotFound(String),
    #[error("the microphone could not be opened: {0}")]
    Open(String),
    #[error("unsupported sample format {0:?}")]
    Format(SampleFormat),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDevice {
    pub name: String,
    pub is_default: bool,
}

/// Input devices on the default host.
pub fn list_inputs() -> Vec<InputDevice> {
    let host = cpal::default_host();
    let default = host.default_input_device().and_then(|d| d.name().ok());
    host.input_devices()
        .map(|it| {
            it.filter_map(|d| d.name().ok())
                .map(|name| InputDevice {
                    is_default: Some(&name) == default.as_ref(),
                    name,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A running capture. Dropping it stops the microphone.
pub struct Capture {
    _stream: Stream,
    pub sample_rate: u32,
    pub blocks: Receiver<Vec<f32>>,
    pub errors: Receiver<String>,
}

/// Open a device (by name, or the system default) and start capturing.
/// The channel holds ~2 s of blocks; if the consumer stalls, blocks are dropped
/// rather than blocking the audio thread.
pub fn start(device_name: Option<&str>) -> Result<Capture, CaptureError> {
    let host = cpal::default_host();
    let device = match device_name {
        None => host.default_input_device().ok_or(CaptureError::NoDevice)?,
        Some(name) => host
            .input_devices()
            .map_err(|e| CaptureError::Open(e.to_string()))?
            .find(|d| d.name().is_ok_and(|n| n == name))
            .ok_or_else(|| CaptureError::DeviceNotFound(name.to_owned()))?,
    };
    let config = device
        .default_input_config()
        .map_err(|e| CaptureError::Open(e.to_string()))?;
    let channels = config.channels() as usize;
    let sample_rate = config.sample_rate().0;
    let (tx, blocks) = sync_channel::<Vec<f32>>(200);
    let (etx, errors) = sync_channel::<String>(8);
    let on_err = move |e: cpal::StreamError| {
        let _ = etx.try_send(e.to_string());
    };

    fn send(tx: &SyncSender<Vec<f32>>, block: Vec<f32>) {
        if let Err(TrySendError::Full(_)) = tx.try_send(block) {
            // Consumer is behind: drop this block instead of blocking real-time audio.
        }
    }

    let stream_config = config.config();
    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _| send(&tx, downmix(data, channels)),
            on_err,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _| {
                let f: Vec<f32> = data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                send(&tx, downmix(&f, channels))
            },
            on_err,
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            &stream_config,
            move |data: &[u16], _| {
                let f: Vec<f32> = data.iter().map(|&s| (s as f32 - 32_768.0) / 32_768.0).collect();
                send(&tx, downmix(&f, channels))
            },
            on_err,
            None,
        ),
        other => return Err(CaptureError::Format(other)),
    }
    .map_err(|e| CaptureError::Open(e.to_string()))?;
    stream.play().map_err(|e| CaptureError::Open(e.to_string()))?;
    Ok(Capture {
        _stream: stream,
        sample_rate,
        blocks,
        errors,
    })
}
