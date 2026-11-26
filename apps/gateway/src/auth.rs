//! Supabase JWT verification (ADR-0007, amended).
//!
//! Hosted and current local Supabase sign access tokens with an asymmetric key
//! (ES256 or RS256, identified by `kid`) published at
//! `<SUPABASE_URL>/auth/v1/.well-known/jwks.json`. Older projects sign with a
//! shared HS256 secret. The [`Verifier`] supports both: JWKS keys are cached and
//! re-fetched when a token carries an unknown `kid` (at most every 30 s), and the
//! HS256 path is used only when a legacy secret is configured.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::{Mutex, RwLock};

use crate::error::ApiError;
use crate::AppState;

const REFRESH_COOLDOWN: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
}

/// The authenticated caller, inserted into request extensions by [`require_auth`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    pub user_id: String,
    pub device_id: Option<String>,
}

type KeyMap = HashMap<String, (DecodingKey, Algorithm)>;

pub struct Verifier {
    hs_secret: Option<DecodingKey>,
    jwks_url: Option<String>,
    keys: RwLock<KeyMap>,
    last_refresh: Mutex<Option<Instant>>,
    http: reqwest::Client,
}

impl Verifier {
    /// `supabase_url` enables JWKS; `hs_secret` enables the legacy HS256 path.
    pub fn new(supabase_url: Option<&str>, hs_secret: Option<&str>) -> Self {
        Self {
            hs_secret: hs_secret
                .filter(|s| !s.is_empty())
                .map(|s| DecodingKey::from_secret(s.as_bytes())),
            jwks_url: supabase_url
                .filter(|u| !u.is_empty())
                .map(|u| format!("{}/auth/v1/.well-known/jwks.json", u.trim_end_matches('/'))),
            keys: RwLock::new(HashMap::new()),
            last_refresh: Mutex::new(None),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("http client"),
        }
    }

    /// A verifier with a fixed key set and no network access (tests, offline tools).
    pub fn with_jwks(set: &JwkSet, hs_secret: Option<&str>) -> Self {
        let v = Self::new(None, hs_secret);
        *v.keys.try_write().expect("fresh lock") = keys_from_set(set);
        v
    }

    pub async fn verify(&self, token: &str) -> Result<String, ApiError> {
        let header = decode_header(token).map_err(|_| ApiError::Unauthorized)?;
        let (key, alg) = if header.alg == Algorithm::HS256 {
            (self.hs_secret.clone().ok_or(ApiError::Unauthorized)?, Algorithm::HS256)
        } else {
            let kid = header.kid.ok_or(ApiError::Unauthorized)?;
            self.key_for(&kid).await.ok_or(ApiError::Unauthorized)?
        };
        // Never trust the token's own `alg` beyond selecting the key: the key decides.
        if alg != header.alg {
            return Err(ApiError::Unauthorized);
        }
        let mut v = Validation::new(alg);
        v.set_audience(&["authenticated"]);
        v.set_required_spec_claims(&["exp", "sub", "aud"]);
        v.leeway = 30;
        decode::<Claims>(token, &key, &v)
            .map(|d| d.claims.sub)
            .map_err(|_| ApiError::Unauthorized)
    }

    async fn key_for(&self, kid: &str) -> Option<(DecodingKey, Algorithm)> {
        if let Some(k) = self.keys.read().await.get(kid) {
            return Some(k.clone());
        }
        self.refresh().await;
        self.keys.read().await.get(kid).cloned()
    }

    /// Re-fetch the JWKS, rate-limited so bogus `kid`s cannot hammer the auth server.
    async fn refresh(&self) {
        let Some(url) = &self.jwks_url else { return };
        let mut last = self.last_refresh.lock().await;
        if last.is_some_and(|t| t.elapsed() < REFRESH_COOLDOWN) {
            return;
        }
        *last = Some(Instant::now());
        match self.http.get(url).send().await.and_then(|r| r.error_for_status()) {
            Ok(res) => match res.json::<JwkSet>().await {
                Ok(set) => *self.keys.write().await = keys_from_set(&set),
                Err(_) => tracing::warn!(target: "auth", "jwks response was not a valid key set"),
            },
            Err(e) => tracing::warn!(target: "auth", status = e.status().map(|s| s.as_u16()), "jwks fetch failed"),
        }
    }
}

fn keys_from_set(set: &JwkSet) -> KeyMap {
    set.keys
        .iter()
        .filter_map(|jwk| {
            let kid = jwk.common.key_id.clone()?;
            let alg = match &jwk.algorithm {
                AlgorithmParameters::EllipticCurve(_) => Algorithm::ES256,
                AlgorithmParameters::RSA(_) => Algorithm::RS256,
                _ => return None, // symmetric keys are never published
            };
            let alg = jwk
                .common
                .key_algorithm
                .and_then(|a| a.to_string().parse().ok())
                .unwrap_or(alg);
            DecodingKey::from_jwk(jwk).ok().map(|k| (kid, (k, alg)))
        })
        .collect()
}

/// Middleware: reject requests without a valid bearer token.
pub async fn require_auth(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .map(str::to_owned);
    let Some(token) = token else {
        return ApiError::Unauthorized.into_response();
    };
    let user_id = match state.verifier.verify(&token).await {
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
