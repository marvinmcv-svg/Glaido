//! Groq Whisper (OpenAI-compatible transcription endpoint), batch on stop.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;

use super::{hint_prompt, http_client, Audio, ProviderError, SpeechToText};

pub const DEFAULT_BASE_URL: &str = "https://api.groq.com/openai/v1";
pub const DEFAULT_MODEL: &str = "whisper-large-v3-turbo";

pub struct Groq {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl Groq {
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
    text: String,
}

#[async_trait]
impl SpeechToText for Groq {
    fn name(&self) -> &'static str {
        "groq"
    }

    async fn transcribe(&self, audio: &Audio<'_>) -> Result<String, ProviderError> {
        let wav = vocalis_audio::wav::encode(audio.pcm, audio.sample_rate);
        let file = Part::bytes(wav)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|_| ProviderError::Malformed)?;
        let mut form = Form::new()
            .part("file", file)
            .text("model", self.model.clone())
            .text("response_format", "json")
            .text("temperature", "0");
        if let [lang] = audio.languages {
            form = form.text("language", lang.clone());
        }
        if let Some(prompt) = hint_prompt(audio.hints, 600) {
            form = form.text("prompt", prompt);
        }
        let res = self
            .http
            .post(format!("{}/audio/transcriptions", self.base_url))
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(ProviderError::from_reqwest)?;
        if !res.status().is_success() {
            return Err(ProviderError::from_status(res.status().as_u16()));
        }
        let body: Response = res.json().await.map_err(|_| ProviderError::Malformed)?;
        Ok(body.text.trim().to_owned())
    }
}
