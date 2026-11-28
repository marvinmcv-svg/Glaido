//! Deepgram pre-recorded transcription (adapter; live streaming comes with ★ live text).

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use super::{http_client, Audio, ProviderError, SpeechToText};

pub const DEFAULT_BASE_URL: &str = "https://api.deepgram.com/v1";
pub const DEFAULT_MODEL: &str = "nova-3";

pub struct Deepgram {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl Deepgram {
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self {
        Self {
            http: http_client(Duration::from_secs(30)),
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
    results: Results,
}
#[derive(Deserialize)]
struct Results {
    channels: Vec<Channel>,
}
#[derive(Deserialize)]
struct Channel {
    alternatives: Vec<Alternative>,
}
#[derive(Deserialize)]
struct Alternative {
    transcript: String,
}

#[async_trait]
impl SpeechToText for Deepgram {
    fn name(&self) -> &'static str {
        "deepgram"
    }

    async fn transcribe(&self, audio: &Audio<'_>) -> Result<String, ProviderError> {
        let mut query: Vec<(&str, String)> = vec![
            ("model", self.model.clone()),
            ("smart_format", "true".into()),
            ("punctuate", "true".into()),
        ];
        match audio.languages {
            [lang] => query.push(("language", lang.clone())),
            _ => query.push(("detect_language", "true".into())),
        }
        for h in audio.hints.iter().take(50) {
            query.push(("keyterm", h.clone()));
        }
        let wav = vocalis_audio::wav::encode(audio.pcm, audio.sample_rate);
        let res = self
            .http
            .post(format!("{}/listen", self.base_url))
            .header("authorization", format!("Token {}", self.api_key))
            .header("content-type", "audio/wav")
            .query(&query)
            .body(wav)
            .send()
            .await
            .map_err(ProviderError::from_reqwest)?;
        if !res.status().is_success() {
            return Err(ProviderError::from_status(res.status().as_u16()));
        }
        let body: Response = res.json().await.map_err(|_| ProviderError::Malformed)?;
        let text = body
            .results
            .channels
            .into_iter()
            .next()
            .and_then(|c| c.alternatives.into_iter().next())
            .map(|a| a.transcript)
            .ok_or(ProviderError::Malformed)?;
        Ok(text.trim().to_owned())
    }
}
