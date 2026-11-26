//! `WS /v1/dictate`: the dictation session (master prompt §5.3, P1).
//!
//! ```text
//! client → {"type":"start", languages, app, dictionary, snippets, rules, tags, sampleRate}
//! server → {"type":"ready"}
//! client → binary frames: 16-bit little-endian mono PCM at 16 kHz
//! client → {"type":"stop"}            (or {"type":"cancel"})
//! server → {"type":"final", ...}       (or {"type":"error", code, retryable, message})
//! ```
//! Audio and text live only in memory for the length of the session. Logs carry
//! sizes, durations, provider names and the plan kind, never content.
//! PCM rather than Opus for the MVP: 32 KB/s is fine on broadband; Opus is a
//! later bandwidth optimization behind the same message types.

use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use vocalis_core::dictionary::{self, Entry};
use vocalis_core::formatting::{self, AppContext, Plan, RuleSet};
use vocalis_core::snippets::Snippet;
use vocalis_core::tags::TagContext;
use vocalis_core::text::{count_words, words_per_minute};

use crate::auth::Caller;
use crate::providers::Audio;
use crate::AppState;

pub const SAMPLE_RATE: u32 = 16_000;
/// 10-minute cap plus 30 s of grace for the client's own auto-stop.
pub const MAX_SAMPLES: usize = SAMPLE_RATE as usize * 630;
const START_TIMEOUT: Duration = Duration::from_secs(10);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_DICTIONARY: usize = 5_000;
const MAX_SNIPPETS: usize = 2_000;
const MAX_LANGUAGES: usize = 20;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMsg {
    Start(Box<Start>),
    Stop,
    Cancel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Start {
    #[serde(default)]
    languages: Vec<String>,
    #[serde(default)]
    app: AppContext,
    #[serde(default)]
    dictionary: Vec<Entry>,
    #[serde(default)]
    snippets: Vec<Snippet>,
    #[serde(default)]
    rules: Option<RuleSet>,
    #[serde(default)]
    tags: TagContext,
    sample_rate: u32,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanKind {
    /// Paste `text` at the cursor.
    Paste,
    /// Run `text` as a Command (wake word).
    Command,
    /// Open the command window empty (wake word alone).
    OpenCommand,
    /// Nothing was said.
    Empty,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMsg {
    Ready,
    #[serde(rename_all = "camelCase")]
    Final {
        plan: PlanKind,
        text: String,
        words: u32,
        wpm: u32,
        duration_ms: u64,
        stt_ms: u64,
        cleanup_ms: u64,
        /// False when cleanup was skipped or fell back to deterministic formatting.
        cleaned: bool,
        stt_provider: &'static str,
    },
    Error {
        code: &'static str,
        retryable: bool,
        message: &'static str,
    },
}

impl ServerMsg {
    fn error(code: &'static str, retryable: bool, message: &'static str) -> Self {
        Self::Error {
            code,
            retryable,
            message,
        }
    }
}

pub async fn handler(ws: WebSocketUpgrade, State(state): State<AppState>, caller: Caller) -> Response {
    ws.max_message_size(4 * 1024 * 1024)
        .on_upgrade(move |socket| session(socket, state, caller))
}

async fn send(socket: &mut WebSocket, msg: &ServerMsg) -> bool {
    let text = serde_json::to_string(msg).expect("server messages serialize");
    socket.send(Message::Text(text.into())).await.is_ok()
}

async fn recv(socket: &mut WebSocket, timeout: Duration) -> Option<Message> {
    match tokio::time::timeout(timeout, socket.recv()).await {
        Ok(Some(Ok(m))) => Some(m),
        _ => None,
    }
}

fn validate(start: &Start) -> Result<(), &'static str> {
    if start.sample_rate != SAMPLE_RATE {
        return Err("Audio must be 16 kHz mono PCM.");
    }
    if start.languages.len() > MAX_LANGUAGES
        || start.dictionary.len() > MAX_DICTIONARY
        || start.snippets.len() > MAX_SNIPPETS
    {
        return Err("Too many languages, dictionary entries or snippets.");
    }
    if start.rules.as_ref().is_some_and(|r| r.validate().is_err()) {
        return Err("The formatting rules are invalid.");
    }
    Ok(())
}

async fn session(mut socket: WebSocket, state: AppState, _caller: Caller) {
    // 1. Start message.
    let start = match recv(&mut socket, START_TIMEOUT).await {
        Some(Message::Text(t)) => match serde_json::from_str::<ClientMsg>(&t) {
            Ok(ClientMsg::Start(s)) => s,
            _ => {
                send(
                    &mut socket,
                    &ServerMsg::error("BAD_REQUEST", false, "Start the session with a start message."),
                )
                .await;
                return;
            }
        },
        _ => return,
    };
    if let Err(message) = validate(&start) {
        send(&mut socket, &ServerMsg::error("BAD_REQUEST", false, message)).await;
        return;
    }
    if !send(&mut socket, &ServerMsg::Ready).await {
        return;
    }

    // 2. Audio until stop.
    let mut pcm: Vec<i16> = Vec::with_capacity(SAMPLE_RATE as usize * 30);
    loop {
        match recv(&mut socket, IDLE_TIMEOUT).await {
            Some(Message::Binary(b)) => {
                if b.len() % 2 != 0 {
                    send(
                        &mut socket,
                        &ServerMsg::error("BAD_REQUEST", false, "Audio frames must contain whole 16-bit samples."),
                    )
                    .await;
                    return;
                }
                if pcm.len() + b.len() / 2 > MAX_SAMPLES {
                    send(
                        &mut socket,
                        &ServerMsg::error("RECORDING_TOO_LONG", false, "Recordings are limited to 10 minutes."),
                    )
                    .await;
                    return;
                }
                pcm.extend(b.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])));
            }
            Some(Message::Text(t)) => match serde_json::from_str::<ClientMsg>(&t) {
                Ok(ClientMsg::Stop) => break,
                Ok(ClientMsg::Cancel) => return,
                _ => {
                    send(
                        &mut socket,
                        &ServerMsg::error("BAD_REQUEST", false, "Unexpected message."),
                    )
                    .await;
                    return;
                }
            },
            Some(Message::Ping(_) | Message::Pong(_)) => {}
            // Closed, errored or idle: the client keeps its local copy for Retry.
            _ => return,
        }
    }

    // 3. Transcribe, format, reply.
    let msg = process(&state, &start, &pcm).await;
    send(&mut socket, &msg).await;
    let _ = socket.send(Message::Close(None)).await;
}

async fn process(state: &AppState, start: &Start, pcm: &[i16]) -> ServerMsg {
    let duration_ms = pcm.len() as u64 * 1000 / SAMPLE_RATE as u64;
    let rules = start.rules.clone().unwrap_or_default();
    let hints = dictionary::vocab_hints(&start.dictionary);

    let t0 = Instant::now();
    let (transcript, provider) = if pcm.is_empty() {
        (String::new(), "none")
    } else if state.stt.is_empty() {
        return ServerMsg::error(
            "PROVIDER_UNAVAILABLE",
            true,
            "Transcription is not available right now. Your audio is saved; try again shortly.",
        );
    } else {
        let audio = Audio {
            pcm,
            sample_rate: SAMPLE_RATE,
            languages: &start.languages,
            hints: &hints,
        };
        match state.stt.transcribe(&audio).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(target: "dictate", error = %e, duration_ms, "transcription failed");
                return ServerMsg::error(
                    "TRANSCRIPTION_FAILED",
                    true,
                    "Transcription failed. Your audio is saved; try again.",
                );
            }
        }
    };
    let stt_ms = t0.elapsed().as_millis() as u64;

    let input = formatting::Input {
        transcript: &transcript,
        app: &start.app,
        rules: &rules,
        dictionary: &start.dictionary,
        snippets: &start.snippets,
        wake_word: vocalis_core::product::WAKE_WORD,
        tags: &start.tags,
    };
    let t1 = Instant::now();
    let (plan, text, cleaned) = match formatting::plan(input) {
        Plan::Empty => (PlanKind::Empty, String::new(), false),
        Plan::Command(req) => (PlanKind::Command, req, false),
        Plan::OpenCommandWindow => (PlanKind::OpenCommand, String::new(), false),
        Plan::Final(text) => (PlanKind::Paste, text, false),
        Plan::Llm(req) => {
            let cleaned = match &state.cleanup {
                Some(c) => match tokio::time::timeout(CLEANUP_TIMEOUT, c.clean(&req.system, &req.user)).await {
                    Ok(Ok(out)) => Some(formatting::finish(&out, input)),
                    Ok(Err(e)) => {
                        tracing::warn!(target: "dictate", error = %e, "cleanup failed; using deterministic formatting");
                        None
                    }
                    Err(_) => {
                        tracing::warn!(target: "dictate", "cleanup timed out; using deterministic formatting");
                        None
                    }
                },
                None => None,
            };
            match cleaned {
                Some(t) => (PlanKind::Paste, t, true),
                None => (PlanKind::Paste, formatting::fallback(input), false),
            }
        }
    };
    let cleanup_ms = t1.elapsed().as_millis() as u64;
    let words = count_words(&text);
    tracing::info!(
        target: "dictate",
        duration_ms, stt_ms, cleanup_ms, words, cleaned, stt_provider = provider, plan = ?plan,
        "dictation processed"
    );
    ServerMsg::Final {
        plan,
        words,
        wpm: words_per_minute(words, duration_ms),
        text,
        duration_ms,
        stt_ms,
        cleanup_ms,
        cleaned,
        stt_provider: provider,
    }
}
