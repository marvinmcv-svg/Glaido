//! The outside world, as traits: gateway, paste/notifications/UI, history.
//! The desktop app implements these; tests use fakes.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Settings sent with every dictation (the gateway's `start` message).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StartSettings {
    pub languages: Vec<String>,
    pub app: vocalis_core::formatting::AppContext,
    pub dictionary: Vec<vocalis_core::dictionary::Entry>,
    pub snippets: Vec<vocalis_core::snippets::Snippet>,
    pub rules: Option<vocalis_core::formatting::RuleSet>,
    pub tags: vocalis_core::tags::TagContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanKind {
    Paste,
    Command,
    OpenCommand,
    Empty,
}

/// The gateway's `final` message.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalResult {
    pub plan: PlanKind,
    pub text: String,
    pub words: u32,
    pub wpm: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GatewayError {
    #[error("could not connect")]
    Connect,
    #[error("connection lost")]
    Disconnected,
    #[error("timed out")]
    Timeout,
    #[error("server error {code}")]
    Server { code: String, retryable: bool },
}

impl GatewayError {
    /// The error code stored on a failed record and shown in its notification.
    pub fn record_code(&self) -> &str {
        match self {
            Self::Server { code, .. } if code == "QUOTA_EXCEEDED" => "QUOTA_EXCEEDED",
            Self::Server { code, .. } if code == "RECORDING_TOO_LONG" => "RECORDING_TOO_LONG",
            Self::Connect | Self::Disconnected | Self::Timeout => "NETWORK",
            _ => "TRANSCRIPTION_FAILED",
        }
    }
}

#[async_trait]
pub trait Gateway: Send + Sync {
    async fn open(&self, settings: &StartSettings) -> Result<Box<dyn GatewaySession>, GatewayError>;
}

#[async_trait]
pub trait GatewaySession: Send {
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<(), GatewayError>;
    async fn finish(self: Box<Self>) -> Result<FinalResult, GatewayError>;
    async fn cancel(self: Box<Self>);
}

/// What the bar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BarState {
    Hidden,
    Recording,
    Processing,
    SlowConnection,
    NearLimit,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    Start,
    Stop,
    Error,
}

/// User-facing notices (Appendix C codes). Copy is written in the UI layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    TranscriptionFailed {
        record_id: String,
        code: String,
    },
    NowhereToPaste {
        record_id: String,
    },
    PasteFailed {
        record_id: String,
    },
    RecordingLimitNear,
    MicSilent,
    RetrySucceeded {
        record_id: String,
    },
    /// The microphone went away mid-recording. `record_id` is set when there
    /// was speech worth keeping (a failed record with Retry).
    MicDisconnected {
        record_id: Option<String>,
    },
    /// The microphone could not be opened at all (missing, busy, no permission).
    MicUnavailable {
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteOutcome {
    Pasted,
    /// No focused text field (macOS); the text is in History.
    NowhereToPaste,
    Failed,
}

pub trait Ui: Send + Sync {
    fn bar(&self, state: BarState);
    fn level(&self, level: f32);
    fn sound(&self, sound: Sound);
    fn notify(&self, notice: Notice);
    fn paste(&self, text: &str) -> PasteOutcome;
    /// A command (wake word) was recognized; P3 opens the command window.
    fn command(&self, request: Option<&str>, record_id: &str);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEntry {
    pub id: String,
    pub is_command: bool,
    pub text: String,
    pub app_name: Option<String>,
    pub app_id: Option<String>,
    pub url: Option<String>,
    pub words: u32,
    pub duration_ms: u64,
    pub wpm: u32,
    pub audio_path: Option<String>,
    pub error_code: Option<String>,
    pub created_at: i64,
}

pub trait History: Send + Sync {
    fn insert(&self, entry: NewEntry);
    fn complete(&self, id: &str, text: &str, words: u32, wpm: u32);
    fn fail(&self, id: &str, code: &str);
}
