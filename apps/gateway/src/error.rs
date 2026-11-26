//! API errors. Messages are fixed strings: user input is never echoed back.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiError {
    Unauthorized,
    NotImplemented,
    QuotaExceeded,
}

impl ApiError {
    fn parts(self) -> (StatusCode, &'static str, bool, &'static str) {
        match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                false,
                "Sign in again to continue.",
            ),
            Self::NotImplemented => (
                StatusCode::NOT_IMPLEMENTED,
                "NOT_IMPLEMENTED",
                false,
                "This endpoint is not available yet.",
            ),
            Self::QuotaExceeded => (
                StatusCode::PAYMENT_REQUIRED,
                "QUOTA_EXCEEDED",
                false,
                "You've used this week's free words. Upgrade to keep going.",
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, retryable, message) = self.parts();
        (
            status,
            Json(json!({ "error": { "code": code, "retryable": retryable, "message": message } })),
        )
            .into_response()
    }
}
