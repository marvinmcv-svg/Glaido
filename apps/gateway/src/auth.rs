//! Supabase JWT verification (ADR-0007).

use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;

use crate::error::ApiError;
use crate::AppState;

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    #[allow(dead_code)]
    exp: u64,
}

/// The authenticated caller, inserted into request extensions by [`require_auth`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    pub user_id: String,
    pub device_id: Option<String>,
}

pub fn verify(token: &str, secret: &str) -> Result<String, ApiError> {
    let mut v = Validation::new(Algorithm::HS256);
    v.set_audience(&["authenticated"]);
    v.leeway = 30;
    decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &v)
        .map(|d| d.claims.sub)
        .map_err(|_| ApiError::Unauthorized)
}

/// Middleware: reject requests without a valid bearer token.
pub async fn require_auth(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let Some(token) = token else {
        return ApiError::Unauthorized.into_response();
    };
    let user_id = match verify(token.trim(), &state.config.jwt_secret) {
        Ok(sub) => sub,
        Err(e) => return e.into_response(),
    };
    let device_id = req
        .headers()
        .get("x-device-id")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty() && v.len() <= 128)
        .map(str::to_owned);
    req.extensions_mut().insert(Caller { user_id, device_id });
    next.run(req).await
}

impl<S: Send + Sync> FromRequestParts<S> for Caller {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<Caller>().cloned().ok_or(ApiError::Unauthorized)
    }
}
