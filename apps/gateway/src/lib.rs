//! Vocalis gateway: the zero-retention voice and AI edge (master prompt §5.3).
//!
//! P0 surface: `GET /healthz`, `GET /v1/meta`, `GET /v1/me`, and stubs for the
//! dictation endpoints that later phases fill in.

pub mod auth;
pub mod config;
pub mod error;
pub mod logging;

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::routing::{get, post};
use axum::{middleware, Json, Router};
use serde::Serialize;
use serde_json::{json, Value};

use auth::Caller;
use config::Config;
use error::ApiError;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub verifier: Arc<auth::Verifier>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let verifier = auth::Verifier::new(config.supabase_url.as_deref(), config.jwt_secret.as_deref());
        Self::with_verifier(config, verifier)
    }

    pub fn with_verifier(config: Config, verifier: auth::Verifier) -> Self {
        Self {
            config: Arc::new(config),
            verifier: Arc::new(verifier),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub service: &'static str,
    pub version: &'static str,
    pub min_client_version: String,
    pub latest_client_version: String,
    pub free_words_per_week: u32,
    pub features: Value,
    pub providers: Value,
}

fn health(ok: bool) -> &'static str {
    if ok {
        "configured"
    } else {
        "unconfigured"
    }
}

async fn meta(State(s): State<AppState>) -> Json<Meta> {
    let c = &s.config;
    Json(Meta {
        service: "vocalis-gateway",
        version: env!("CARGO_PKG_VERSION"),
        min_client_version: c.min_client_version.clone(),
        latest_client_version: c.latest_client_version.clone(),
        free_words_per_week: vocalis_core::product::FREE_WORDS_PER_WEEK,
        features: json!({ "liveText": false, "commands": false, "localStt": false }),
        providers: json!({
            "stt": { "groq": health(c.groq_configured), "deepgram": health(c.deepgram_configured) },
            "llm": { "anthropic": health(c.anthropic_configured) }
        }),
    })
}

async fn me(caller: Caller) -> Json<Value> {
    Json(json!({ "userId": caller.user_id, "deviceId": caller.device_id }))
}

/// Placeholder for `POST /v1/dictate/retry` (P1). It consumes the body so the
/// no-body-logging test exercises a handler that actually reads content.
async fn dictate_retry(_caller: Caller, body: Bytes) -> Result<Json<Value>, ApiError> {
    drop(body);
    Err(ApiError::NotImplemented)
}

pub fn app(state: AppState) -> Router {
    let authed = Router::new()
        .route("/v1/me", get(me))
        .route("/v1/dictate/retry", post(dictate_retry))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::require_auth));

    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/v1/meta", get(meta))
        .merge(authed)
        .layer(middleware::from_fn(logging::log_request))
        .with_state(state)
}
