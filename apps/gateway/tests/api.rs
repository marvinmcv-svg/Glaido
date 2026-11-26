mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use vocalis_gateway::{app, config::Config, AppState};

fn router() -> axum::Router {
    app(AppState::new(Config::for_tests(common::SECRET)))
}

async fn send(req: Request<Body>) -> (StatusCode, Value) {
    let res = router().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn meta_is_public() {
    let (status, body) = send(Request::get("/v1/meta").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["service"], "vocalis-gateway");
    assert_eq!(body["minClientVersion"], "0.1.0");
    assert_eq!(body["freeWordsPerWeek"], 2000);
    assert_eq!(body["providers"]["stt"]["groq"], "unconfigured");
}

#[tokio::test]
async fn me_requires_a_valid_token() {
    let (status, body) = send(Request::get("/v1/me").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "UNAUTHORIZED");

    let bad = [
        common::token("u1", "authenticated", -3600), // expired
        common::token("u1", "anon", 3600),           // wrong audience
        "not-a-jwt".to_owned(),
    ];
    for t in bad {
        let req = Request::get("/v1/me")
            .header("authorization", format!("Bearer {t}"))
            .body(Body::empty())
            .unwrap();
        assert_eq!(send(req).await.0, StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn me_returns_the_caller() {
    let req = Request::get("/v1/me")
        .header(
            "authorization",
            format!("Bearer {}", common::token("user-123", "authenticated", 3600)),
        )
        .header("x-device-id", "dev-abc")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["userId"], "user-123");
    assert_eq!(body["deviceId"], "dev-abc");
}

#[tokio::test]
async fn retry_stub_is_authenticated_and_not_implemented() {
    let req = Request::post("/v1/dictate/retry")
        .header(
            "authorization",
            format!("Bearer {}", common::token("u", "authenticated", 60)),
        )
        .body(Body::from("x"))
        .unwrap();
    let (status, body) = send(req).await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(body["error"]["retryable"], false);
}
