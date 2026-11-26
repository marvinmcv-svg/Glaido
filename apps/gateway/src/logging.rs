//! Zero-retention request logging (ADR-0008, master prompt §5.3).
//!
//! This middleware can only see metadata. It logs the method, the *matched route
//! template* (never the raw path or query string), the status, the latency and
//! byte counts. It never reads request or response bodies or headers other than
//! `content-length`. `tests/no_body_logging.rs` proves that a sentinel sent in a
//! body, query string or header never reaches the logs.

use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;

pub async fn log_request(req: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = req.method().as_str().to_owned();
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map_or("unmatched", |p| p.as_str())
        .to_owned();
    let req_bytes = content_length(req.headers());
    let res = next.run(req).await;
    tracing::info!(
        target: "http",
        method = %method,
        route = %route,
        status = res.status().as_u16(),
        latency_ms = started.elapsed().as_millis() as u64,
        req_bytes,
        res_bytes = content_length(res.headers()),
        "request"
    );
    res
}

fn content_length(h: &axum::http::HeaderMap) -> u64 {
    h.get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}
