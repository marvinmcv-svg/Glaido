//! Zero retention through a full dictation session: transcript, cleanup output,
//! dictionary and app context never appear in logs (ADR-0008).

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use vocalis_gateway::auth::Verifier;
use vocalis_gateway::providers::{Audio, Cleanup, ProviderError, SpeechToText, SttChain};
use vocalis_gateway::{app, config::Config, AppState};

const SENTINEL: &str = "SENTINELtranscript42";

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct Stt(Result<String, ProviderError>);
#[async_trait]
impl SpeechToText for Stt {
    fn name(&self) -> &'static str {
        "fake"
    }
    async fn transcribe(&self, _: &Audio<'_>) -> Result<String, ProviderError> {
        self.0.clone()
    }
}
struct Clean;
#[async_trait]
impl Cleanup for Clean {
    fn name(&self) -> &'static str {
        "fake"
    }
    async fn clean(&self, _: &str, _: &str) -> Result<String, ProviderError> {
        Err(ProviderError::Refused) // exercise the warn path too
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dictation_content_never_logged() {
    let capture = Capture::default();
    let w = capture.clone();
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || w.clone())
            .finish(),
    )
    .unwrap();

    for stt in [
        Ok(format!("please write {SENTINEL} now")),
        Err(ProviderError::Status(500)),
    ] {
        let state = AppState::with_parts(
            Config::for_tests(common::SECRET),
            Verifier::new(None, Some(common::SECRET)),
            SttChain::new(vec![Box::new(Stt(stt))]),
            Some(Arc::new(Clean)),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app(state)).await.unwrap() });

        let mut req = format!("ws://{addr}/v1/dictate").into_client_request().unwrap();
        req.headers_mut().insert(
            "authorization",
            format!("Bearer {}", common::token("u", "authenticated", 600))
                .parse()
                .unwrap(),
        );
        let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
        let start = json!({
            "type": "start", "sampleRate": 16000,
            "app": { "name": SENTINEL, "app_id": SENTINEL, "url": format!("https://{SENTINEL}.example") },
            "dictionary": [ { "word": SENTINEL, "misspelling": null } ],
            "tags": { "clipboard": SENTINEL, "datetime": "", "timezone": "" },
        });
        ws.send(Message::Text(start.to_string().into())).await.unwrap();
        ws.next().await.unwrap().unwrap();
        ws.send(Message::Binary(vec![0u8; 3200].into())).await.unwrap();
        ws.send(Message::Text(json!({"type": "stop"}).to_string().into()))
            .await
            .unwrap();
        while let Some(Ok(m)) = ws.next().await {
            if matches!(m, Message::Close(_)) {
                break;
            }
        }
    }
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let logs = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    assert!(logs.contains("dictation processed"), "metrics line present:\n{logs}");
    assert!(logs.contains("transcription failed"), "failure path logged:\n{logs}");
    assert!(logs.contains("cleanup failed"), "cleanup fallback logged:\n{logs}");
    assert!(!logs.contains(SENTINEL), "content leaked into logs:\n{logs}");
}
