//! Speech-to-text and cleanup providers (master prompt §5.4).
//!
//! Zero retention: request and response bodies never reach logs. Errors carry
//! only a status code and a fixed category, never provider response text, so
//! a transcript echoed in an error body cannot leak.

pub mod anthropic;
pub mod deepgram;
pub mod groq;

use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderError {
    #[error("provider is not configured")]
    NotConfigured,
    #[error("provider timed out")]
    Timeout,
    #[error("provider rate limited the request")]
    RateLimited,
    #[error("provider returned HTTP {0}")]
    Status(u16),
    #[error("provider response was malformed")]
    Malformed,
    #[error("provider declined the request")]
    Refused,
    #[error("network error")]
    Network,
}

impl ProviderError {
    /// Whether trying another provider (or again later) can help.
    pub fn retryable(&self) -> bool {
        match self {
            Self::Timeout | Self::RateLimited | Self::Network | Self::NotConfigured => true,
            Self::Status(s) => *s >= 500 || *s == 408 || *s == 409,
            Self::Malformed | Self::Refused => false,
        }
    }

    pub(crate) fn from_reqwest(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            Self::Timeout
        } else if let Some(s) = e.status() {
            Self::from_status(s.as_u16())
        } else if e.is_decode() {
            Self::Malformed
        } else {
            Self::Network
        }
    }

    pub(crate) fn from_status(s: u16) -> Self {
        if s == 429 {
            Self::RateLimited
        } else {
            Self::Status(s)
        }
    }
}

/// One audio clip to transcribe: 16-bit mono PCM.
pub struct Audio<'a> {
    pub pcm: &'a [i16],
    pub sample_rate: u32,
    /// ISO 639-1 codes. One code = fixed language; several or none = auto-detect.
    pub languages: &'a [String],
    /// Dictionary words to bias recognition toward.
    pub hints: &'a [String],
}

#[async_trait]
pub trait SpeechToText: Send + Sync {
    fn name(&self) -> &'static str;
    async fn transcribe(&self, audio: &Audio<'_>) -> Result<String, ProviderError>;
}

/// Tries providers in order, moving on when an error is retryable (§5.3 failover).
pub struct SttChain {
    providers: Vec<Box<dyn SpeechToText>>,
}

impl SttChain {
    pub fn new(providers: Vec<Box<dyn SpeechToText>>) -> Self {
        Self { providers }
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Returns the transcript and the name of the provider that produced it.
    pub async fn transcribe(&self, audio: &Audio<'_>) -> Result<(String, &'static str), ProviderError> {
        let mut last = ProviderError::NotConfigured;
        for p in &self.providers {
            match p.transcribe(audio).await {
                Ok(text) => return Ok((text, p.name())),
                Err(e) => {
                    tracing::warn!(target: "providers", provider = p.name(), error = %e, "stt attempt failed");
                    let stop = !e.retryable();
                    last = e;
                    if stop {
                        break;
                    }
                }
            }
        }
        Err(last)
    }
}

#[async_trait]
pub trait Cleanup: Send + Sync {
    fn name(&self) -> &'static str;
    async fn clean(&self, system: &str, user: &str) -> Result<String, ProviderError>;
}

fn env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

/// Build providers from the environment. STT order comes from `STT_PROVIDERS`
/// (default `groq,deepgram`); providers without a key are skipped.
pub fn from_env() -> (SttChain, Option<std::sync::Arc<dyn Cleanup>>) {
    let order = env("STT_PROVIDERS").unwrap_or_else(|| "groq,deepgram".into());
    let mut stt: Vec<Box<dyn SpeechToText>> = Vec::new();
    for name in order.split(',').map(str::trim) {
        match name {
            "groq" => {
                if let Some(k) = env("GROQ_API_KEY") {
                    stt.push(Box::new(groq::Groq::new(
                        k,
                        env("GROQ_BASE_URL"),
                        env("GROQ_STT_MODEL"),
                    )));
                }
            }
            "deepgram" => {
                if let Some(k) = env("DEEPGRAM_API_KEY") {
                    stt.push(Box::new(deepgram::Deepgram::new(
                        k,
                        env("DEEPGRAM_BASE_URL"),
                        env("DEEPGRAM_STT_MODEL"),
                    )));
                }
            }
            other => tracing::warn!(target: "providers", provider = other, "unknown STT provider ignored"),
        }
    }
    let cleanup = env("ANTHROPIC_API_KEY").map(|k| {
        std::sync::Arc::new(anthropic::Anthropic::new(
            k,
            env("ANTHROPIC_BASE_URL"),
            env("ANTHROPIC_CLEANUP_MODEL"),
        )) as std::sync::Arc<dyn Cleanup>
    });
    (SttChain::new(stt), cleanup)
}

pub(crate) fn http_client(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(3))
        .pool_idle_timeout(Duration::from_secs(90))
        .build()
        .expect("http client")
}

/// Whisper-style prompts are limited (~224 tokens); keep hints short.
pub(crate) fn hint_prompt(hints: &[String], max_chars: usize) -> Option<String> {
    let mut out = String::new();
    for h in hints {
        let sep = if out.is_empty() { "" } else { ", " };
        if out.len() + sep.len() + h.len() > max_chars {
            break;
        }
        out.push_str(sep);
        out.push_str(h);
    }
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_prompt_truncates_on_word_boundaries() {
        let h: Vec<String> = ["Vocalis", "PostgreSQL", "Supabase"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(hint_prompt(&h, 100).as_deref(), Some("Vocalis, PostgreSQL, Supabase"));
        assert_eq!(hint_prompt(&h, 20).as_deref(), Some("Vocalis, PostgreSQL"));
        assert_eq!(hint_prompt(&[], 20), None);
    }

    #[test]
    fn retryability() {
        assert!(ProviderError::Status(503).retryable());
        assert!(!ProviderError::Status(400).retryable());
        assert!(ProviderError::RateLimited.retryable());
        assert!(!ProviderError::Refused.retryable());
    }
}
