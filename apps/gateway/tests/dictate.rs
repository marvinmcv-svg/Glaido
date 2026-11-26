//! End-to-end WS /v1/dictate sessions against in-process fake providers.

mod common;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use vocalis_gateway::auth::Verifier;
use vocalis_gateway::providers::{Audio, Cleanup, ProviderError, SpeechToText, SttChain};
use vocalis_gateway::{app, config::Config, AppState};

struct FakeStt {
    name: &'static str,
    result: Result<String, ProviderError>,
    seen: Arc<Mutex<Vec<(usize, Vec<String>, Vec<String>)>>>,
}

#[async_trait]
impl SpeechToText for FakeStt {
    fn name(&self) -> &'static str {
        self.name
    }
    async fn transcribe(&self, audio: &Audio<'_>) -> Result<String, ProviderError> {
        self.seen
            .lock()
            .unwrap()
            .push((audio.pcm.len(), audio.languages.to_vec(), audio.hints.to_vec()));
        self.result.clone()
    }
}

struct FakeCleanup {
    result: Result<String, ProviderError>,
    seen: Arc<Mutex<Vec<(String, String)>>>,
}

#[async_trait]
impl Cleanup for FakeCleanup {
    fn name(&self) -> &'static str {
        "fake"
    }
    async fn clean(&self, system: &str, user: &str) -> Result<String, ProviderError> {
        self.seen.lock().unwrap().push((system.to_owned(), user.to_owned()));
        self.result.clone()
    }
}

#[derive(Default, Clone)]
struct Seen {
    stt: Arc<Mutex<Vec<(usize, Vec<String>, Vec<String>)>>>,
    cleanup: Arc<Mutex<Vec<(String, String)>>>,
}

async fn serve(
    stt: Vec<Result<String, ProviderError>>,
    cleanup: Option<Result<String, ProviderError>>,
) -> (String, Seen) {
    let seen = Seen::default();
    let names = ["primary", "backup"];
    let chain = SttChain::new(
        stt.into_iter()
            .enumerate()
            .map(|(i, r)| {
                Box::new(FakeStt {
                    name: names[i],
                    result: r,
                    seen: seen.stt.clone(),
                }) as Box<dyn SpeechToText>
            })
            .collect(),
    );
    let cleanup = cleanup.map(|r| {
        Arc::new(FakeCleanup {
            result: r,
            seen: seen.cleanup.clone(),
        }) as Arc<dyn Cleanup>
    });
    let state = AppState::with_parts(
        Config::for_tests(common::SECRET),
        Verifier::new(None, Some(common::SECRET)),
        chain,
        cleanup,
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app(state)).await.unwrap() });
    (format!("ws://{addr}/v1/dictate"), seen)
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(url: &str) -> Ws {
    let mut req = url.into_client_request().unwrap();
    req.headers_mut().insert(
        "authorization",
        format!("Bearer {}", common::token("user-1", "authenticated", 600))
            .parse()
            .unwrap(),
    );
    tokio_tungstenite::connect_async(req).await.unwrap().0
}

async fn next_json(ws: &mut Ws) -> Value {
    loop {
        match ws.next().await.expect("message").expect("ok") {
            Message::Text(t) => return serde_json::from_str(&t).unwrap(),
            Message::Close(_) => panic!("closed before a reply"),
            _ => {}
        }
    }
}

fn pcm(samples: usize) -> Vec<u8> {
    (0..samples)
        .flat_map(|i| ((i % 200) as i16 * 50).to_le_bytes())
        .collect()
}

async fn dictate(url: &str, start: Value, audio: Vec<u8>) -> Value {
    let mut ws = connect(url).await;
    ws.send(Message::Text(start.to_string().into())).await.unwrap();
    assert_eq!(next_json(&mut ws).await["type"], "ready");
    for chunk in audio.chunks(6400) {
        ws.send(Message::Binary(chunk.to_vec().into())).await.unwrap();
    }
    ws.send(Message::Text(json!({"type": "stop"}).to_string().into()))
        .await
        .unwrap();
    next_json(&mut ws).await
}

fn start() -> Value {
    json!({
        "type": "start",
        "sampleRate": 16000,
        "languages": ["en"],
        "app": { "name": "Notes", "app_id": "com.apple.Notes", "field_kind": "document" },
        "dictionary": [ { "word": "PostgreSQL", "misspelling": "postgres" }, { "word": "Vocalis", "misspelling": null } ],
        "snippets": [ { "trigger": "my address", "content": "1 Main St" } ],
    })
}

#[tokio::test]
async fn happy_path_with_cleanup_and_post_pass() {
    let (url, seen) = serve(
        vec![Ok("um we moved to postgres".into())],
        Some(Ok("We moved to postgres.".into())),
    )
    .await;
    let r = dictate(&url, start(), pcm(32_000)).await;
    assert_eq!(r["type"], "final");
    assert_eq!(r["plan"], "paste");
    assert_eq!(
        r["text"], "We moved to PostgreSQL.",
        "dictionary post-pass runs after cleanup"
    );
    assert_eq!(r["cleaned"], true);
    assert_eq!(r["durationMs"], 2000);
    assert_eq!(r["words"], 4);
    assert_eq!(r["sttProvider"], "primary");
    let stt = seen.stt.lock().unwrap();
    assert_eq!(stt[0].0, 32_000, "every sample arrives");
    assert_eq!(stt[0].1, vec!["en"]);
    assert_eq!(
        stt[0].2,
        vec!["PostgreSQL", "Vocalis"],
        "vocab hints from the dictionary"
    );
    let (system, user) = &seen.cleanup.lock().unwrap()[0];
    assert!(system.contains("Prefer these spellings"));
    assert!(user.contains("<dictation>\num we moved to postgres\n</dictation>"));
}

#[tokio::test]
async fn cleanup_failure_falls_back_to_deterministic_text() {
    let (url, _) = serve(
        vec![Ok("we moved to postgres. it works".into())],
        Some(Err(ProviderError::Refused)),
    )
    .await;
    let r = dictate(&url, start(), pcm(16_000)).await;
    assert_eq!(
        r["text"], "We moved to PostgreSQL. It works",
        "deterministic path capitalizes but adds no punctuation"
    );
    assert_eq!(r["cleaned"], false);
}

#[tokio::test]
async fn no_cleanup_configured_still_works() {
    let (url, _) = serve(vec![Ok("hello there".into())], None).await;
    let r = dictate(&url, start(), pcm(16_000)).await;
    assert_eq!(r["text"], "Hello there");
}

#[tokio::test]
async fn snippet_and_wake_word_skip_cleanup() {
    let (url, seen) = serve(vec![Ok("My address.".into())], Some(Ok("never".into()))).await;
    assert_eq!(dictate(&url, start(), pcm(8000)).await["text"], "1 Main St");
    let (url2, _) = serve(
        vec![Ok("Hey Vocalis, translate this to French".into())],
        Some(Ok("never".into())),
    )
    .await;
    let r = dictate(&url2, start(), pcm(8000)).await;
    assert_eq!(r["plan"], "command");
    assert_eq!(r["text"], "translate this to French");
    assert!(seen.cleanup.lock().unwrap().is_empty());
}

#[tokio::test]
async fn empty_audio_is_empty_plan() {
    let (url, seen) = serve(vec![Ok("x".into())], None).await;
    let r = dictate(&url, start(), vec![]).await;
    assert_eq!(r["plan"], "empty");
    assert!(seen.stt.lock().unwrap().is_empty(), "no provider call for no audio");
}

#[tokio::test]
async fn failover_to_backup_then_error_when_all_fail() {
    let (url, _) = serve(vec![Err(ProviderError::Status(503)), Ok("from backup".into())], None).await;
    let r = dictate(&url, start(), pcm(8000)).await;
    assert_eq!(r["sttProvider"], "backup");
    assert_eq!(r["text"], "From backup");

    let (url, seen) = serve(vec![Err(ProviderError::Status(400)), Ok("unused".into())], None).await;
    let r = dictate(&url, start(), pcm(8000)).await;
    assert_eq!(r["type"], "error");
    assert_eq!(r["code"], "TRANSCRIPTION_FAILED");
    assert_eq!(r["retryable"], true);
    assert_eq!(
        seen.stt.lock().unwrap().len(),
        1,
        "non-retryable error does not fail over"
    );
}

#[tokio::test]
async fn rejects_bad_sessions() {
    let (url, _) = serve(vec![Ok("x".into())], None).await;
    let mut ws = connect(&url).await;
    let mut bad = start();
    bad["sampleRate"] = json!(44100);
    ws.send(Message::Text(bad.to_string().into())).await.unwrap();
    let r = next_json(&mut ws).await;
    assert_eq!(
        (r["type"].as_str(), r["code"].as_str()),
        (Some("error"), Some("BAD_REQUEST"))
    );

    let mut ws = connect(&url).await;
    ws.send(Message::Text(start().to_string().into())).await.unwrap();
    next_json(&mut ws).await;
    ws.send(Message::Binary(vec![1, 2, 3].into())).await.unwrap();
    assert_eq!(next_json(&mut ws).await["code"], "BAD_REQUEST", "odd byte count");

    let mut ws = connect(&url).await;
    ws.send(Message::Text(json!({"type": "stop"}).to_string().into()))
        .await
        .unwrap();
    assert_eq!(next_json(&mut ws).await["code"], "BAD_REQUEST", "must start first");
}

#[tokio::test]
async fn upgrade_requires_auth() {
    let (url, _) = serve(vec![Ok("x".into())], None).await;
    let err = tokio_tungstenite::connect_async(url.as_str()).await.unwrap_err();
    match err {
        tokio_tungstenite::tungstenite::Error::Http(res) => assert_eq!(res.status(), 401),
        other => panic!("expected HTTP 401, got {other:?}"),
    }
}
