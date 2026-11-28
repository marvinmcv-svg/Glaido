//! Dictation cleanup via the Claude Messages API (raw HTTP; Rust has no official SDK).
//!
//! * Model: `claude-haiku-5-5` by default: the brief asks for a fast,
//!   Haiku-class model for formatting (`ANTHROPIC_CLEANUP_MODEL` overrides).
//! * Latency: `output_config.effort = "low"` with adaptive thinking, which can
//!   skip thinking on simple input. No sampling parameters: Haiku 5.5 rejects
//!   non-default `temperature`.
//! * Content is read by block type (a response may start with `thinking`
//!   blocks). A `refusal` or `max_tokens` stop is reported as an error so the
//!   caller falls back to deterministic formatting.
//! * Zero retention is an account-level arrangement with Anthropic; this client
//!   uses no features excluded from it (no message threads, batches or files).

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use super::{http_client, Cleanup, ProviderError};

pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
pub const DEFAULT_MODEL: &str = "claude-haiku-5-5";
pub const API_VERSION: &str = "2026-10-21";

pub struct Anthropic {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl Anthropic {
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self {
        Self {
            http: http_client(Duration::from_secs(20)),
            base_url: base_url
                .unwrap_or_else(|| DEFAULT_BASE_URL.into())
                .trim_end_matches('/')
                .to_owned(),
            api_key,
            model: model.unwrap_or_else(|| DEFAULT_MODEL.into()),
        }
    }
}

#[derive(Deserialize)]
struct Response {
    content: Vec<Block>,
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

#[async_trait]
impl Cleanup for Anthropic {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    async fn clean(&self, system: &str, user: &str) -> Result<String, ProviderError> {
        let body = json!({
            "model": self.model,
            "max_tokens": 16000,
            "system": system,
            "output_config": { "effort": "low" },
            "messages": [{ "role": "user", "content": user }],
        });
        let res = self
            .http
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .json(&body)
            .send()
            .await
            .map_err(ProviderError::from_reqwest)?;
        if !res.status().is_success() {
            return Err(ProviderError::from_status(res.status().as_u16()));
        }
        let msg: Response = res.json().await.map_err(|_| ProviderError::Malformed)?;
        match msg.stop_reason.as_deref() {
            Some("refusal") => return Err(ProviderError::Refused),
            Some("max_tokens") => return Err(ProviderError::Malformed),
            _ => {}
        }
        let text: String = msg
            .content
            .iter()
            .filter(|b| b.kind == "text")
            .map(|b| b.text.as_str())
            .collect();
        let text = text.trim();
        if text.is_empty() {
            return Err(ProviderError::Malformed);
        }
        Ok(text.to_owned())
    }
}
