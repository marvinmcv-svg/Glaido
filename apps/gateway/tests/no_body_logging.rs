//! ADR-0008 / master prompt §8: prove request content never reaches the logs.

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use tower::ServiceExt;
use vocalis_gateway::{app, config::Config, AppState};

const SENTINEL: &str = "SENTINEL-transcript-7f3a9c";

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn sentinel_never_appears_in_logs() {
    let capture = Capture::default();
    let writer = capture.clone();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let router = app(AppState::new(Config::for_tests(common::SECRET)));
    let token = common::token("user-1", "authenticated", 3600);
    let requests = vec![
        // Authenticated handler that reads the body.
        Request::post(format!("/v1/dictate/retry?text={SENTINEL}"))
            .header("authorization", format!("Bearer {token}"))
            .header("x-transcript", SENTINEL)
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"text":"{SENTINEL}"}}"#)))
            .unwrap(),
        // Rejected by auth.
        Request::post("/v1/dictate/retry")
            .header("authorization", format!("Bearer {SENTINEL}"))
            .body(Body::from(SENTINEL))
            .unwrap(),
        // Unknown route containing the sentinel in its path.
        Request::get(format!("/v1/{SENTINEL}")).body(Body::empty()).unwrap(),
    ];
    for req in requests {
        let res = router.clone().oneshot(req).await.unwrap();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        assert!(
            !String::from_utf8_lossy(&body).contains(SENTINEL),
            "responses must not echo input"
        );
    }

    let logs = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    assert_eq!(
        logs.matches("\"request\"").count(),
        3,
        "one access log line per request:\n{logs}"
    );
    assert!(logs.contains("\"route\":\"/v1/dictate/retry\""));
    assert!(logs.contains("\"route\":\"unmatched\""));
    assert!(!logs.contains(SENTINEL), "sentinel leaked into logs:\n{logs}");
    assert!(!logs.contains(&token), "token leaked into logs");
}
