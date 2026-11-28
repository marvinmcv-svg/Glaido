//! Desktop sign-in through the web app (ADR-0012) and the session that feeds
//! the gateway its access tokens.
//!
//! 1. `begin`: make a PKCE verifier and state, return the web URL to open.
//! 2. The browser comes back as `vocalis://auth/callback?code=…&state=…`;
//!    `complete` checks the state and redeems the code with the verifier.
//! 3. The refresh token lives in the OS keychain. A background loop refreshes
//!    the access token before it expires; [`Auth::access_token`] is what the
//!    gateway reads for each dictation.

use std::path::PathBuf;
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Keychain entry holding [`Stored`] as JSON.
const SECRET: &str = "session";
/// A sign-in started more than this long ago is no longer accepted.
const PENDING_TTL: Duration = Duration::from_secs(15 * 60);
/// Refresh when the access token has less than this left.
const REFRESH_MARGIN_S: i64 = 120;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub user_id: String,
    pub email: String,
}

/// What survives a restart (in the keychain).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    refresh_token: String,
    supabase_url: String,
    anon_key: String,
    user_id: String,
    email: String,
}

#[derive(Debug, Clone)]
struct Live {
    stored: Stored,
    access_token: Option<String>,
    /// Unix seconds.
    expires_at: i64,
}

struct Pending {
    verifier: String,
    state: String,
    started: Instant,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("This sign-in link isn't from Vocalis.")]
    BadLink,
    #[error("This sign-in has expired or was started elsewhere. Choose Sign in again.")]
    UnknownState,
    #[error("The sign-in code was already used or has expired. Choose Sign in again.")]
    Rejected,
    #[error("Couldn't reach Vocalis. Check your connection and try again.")]
    Network,
    #[error("Your session ended. Sign in again.")]
    SessionEnded,
}

/// PKCE (RFC 7636, S256) verifier and challenge.
pub fn pkce_pair() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let challenge = challenge_for(&verifier);
    (verifier, challenge)
}

pub fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn random_state() -> String {
    let mut bytes = [0u8; 18];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// `Some((code, state))` for a well-formed `vocalis://auth/callback` link.
pub fn parse_callback(link: &str) -> Option<(String, String)> {
    let url = tauri::Url::parse(link).ok()?;
    if url.scheme() != "vocalis" || url.host_str() != Some("auth") || url.path() != "/callback" {
        return None;
    }
    let get = |k: &str| url.query_pairs().find(|(n, _)| n == k).map(|(_, v)| v.into_owned());
    let (code, state) = (get("code")?, get("state")?);
    let ok = |s: &str, max: usize| {
        !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    };
    (ok(&code, 64) && ok(&state, 128)).then_some((code, state))
}

pub fn is_callback(link: &str) -> bool {
    link.starts_with("vocalis://auth/")
}

fn now_s() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExchangeResponse {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    user: ExchangeUser,
    supabase_url: String,
    anon_key: String,
}

#[derive(Deserialize)]
struct ExchangeUser {
    id: String,
    email: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_at: Option<i64>,
    expires_in: Option<i64>,
}

pub struct Auth {
    web_url: String,
    key_dir: PathBuf,
    http: reqwest::Client,
    pending: Mutex<Option<Pending>>,
    live: RwLock<Option<Live>>,
    /// Development and E2E: a fixed token that bypasses sign-in.
    dev_token: Option<String>,
    /// Keychain entry name (tests use their own).
    secret: String,
}

impl Auth {
    pub fn new(web_url: &str, key_dir: PathBuf, dev_token: Option<String>) -> Self {
        Self::with_secret(web_url, key_dir, dev_token, SECRET.to_owned())
    }

    fn with_secret(web_url: &str, key_dir: PathBuf, dev_token: Option<String>, secret: String) -> Self {
        let stored =
            crate::secrets::get_secret(&secret, &key_dir).and_then(|s| serde_json::from_str::<Stored>(&s).ok());
        Self {
            web_url: web_url.trim_end_matches('/').to_owned(),
            key_dir,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .expect("http client"),
            pending: Mutex::new(None),
            live: RwLock::new(stored.map(|stored| Live {
                stored,
                access_token: None,
                expires_at: 0,
            })),
            dev_token,
            secret,
        }
    }

    pub fn account(&self) -> Option<Account> {
        self.live.read().expect("auth lock").as_ref().map(|l| Account {
            user_id: l.stored.user_id.clone(),
            email: l.stored.email.clone(),
        })
    }

    /// The token for the next dictation, if one is valid now.
    pub fn access_token(&self) -> Option<String> {
        if let Some(t) = &self.dev_token {
            return Some(t.clone());
        }
        let live = self.live.read().expect("auth lock");
        let l = live.as_ref()?;
        (l.expires_at > now_s() + 5).then(|| l.access_token.clone()).flatten()
    }

    /// Start a sign-in; returns the URL to open in the browser.
    pub fn begin(&self) -> String {
        let (verifier, challenge) = pkce_pair();
        let state = random_state();
        let url = format!("{}/auth/desktop?challenge={challenge}&state={state}", self.web_url);
        *self.pending.lock().expect("auth lock") = Some(Pending {
            verifier,
            state,
            started: Instant::now(),
        });
        url
    }

    /// Finish a sign-in from the deep link.
    pub async fn complete(&self, link: &str) -> Result<Account, AuthError> {
        let (code, state) = parse_callback(link).ok_or(AuthError::BadLink)?;
        let verifier = {
            let mut pending = self.pending.lock().expect("auth lock");
            match pending.as_ref() {
                Some(p) if p.state == state && p.started.elapsed() < PENDING_TTL => {
                    pending.take().expect("checked").verifier
                }
                _ => return Err(AuthError::UnknownState),
            }
        };
        let res = self
            .http
            .post(format!("{}/api/desktop/exchange", self.web_url))
            .json(&serde_json::json!({ "code": code, "verifier": verifier }))
            .send()
            .await
            .map_err(|_| AuthError::Network)?;
        if res.status().is_client_error() {
            return Err(AuthError::Rejected);
        }
        if !res.status().is_success() {
            return Err(AuthError::Network);
        }
        let r: ExchangeResponse = res.json().await.map_err(|_| AuthError::Network)?;
        let stored = Stored {
            refresh_token: r.refresh_token,
            supabase_url: r.supabase_url.trim_end_matches('/').to_owned(),
            anon_key: r.anon_key,
            user_id: r.user.id,
            email: r.user.email,
        };
        self.save(&stored);
        *self.live.write().expect("auth lock") = Some(Live {
            stored,
            access_token: Some(r.access_token),
            expires_at: r.expires_at,
        });
        Ok(self.account().expect("just signed in"))
    }

    fn save(&self, stored: &Stored) {
        if let Ok(json) = serde_json::to_string(stored) {
            crate::secrets::set_secret(&self.secret, &json, &self.key_dir);
        }
    }

    /// Whether the access token should be refreshed now.
    fn needs_refresh(&self) -> Option<Stored> {
        let live = self.live.read().expect("auth lock");
        let l = live.as_ref()?;
        (l.access_token.is_none() || l.expires_at - now_s() < REFRESH_MARGIN_S).then(|| l.stored.clone())
    }

    /// Refresh if due. `Err(SessionEnded)` means the user must sign in again.
    pub async fn refresh_if_due(&self) -> Result<(), AuthError> {
        let Some(stored) = self.needs_refresh() else {
            return Ok(());
        };
        let res = self
            .http
            .post(format!(
                "{}/auth/v1/token?grant_type=refresh_token",
                stored.supabase_url
            ))
            .header("apikey", &stored.anon_key)
            .json(&serde_json::json!({ "refresh_token": stored.refresh_token }))
            .send()
            .await
            .map_err(|_| AuthError::Network)?;
        let status = res.status();
        if status == reqwest::StatusCode::BAD_REQUEST || status == reqwest::StatusCode::UNAUTHORIZED {
            // Revoked, reused or expired refresh token.
            self.forget();
            return Err(AuthError::SessionEnded);
        }
        if !status.is_success() {
            return Err(AuthError::Network);
        }
        let t: TokenResponse = res.json().await.map_err(|_| AuthError::Network)?;
        let stored = Stored {
            refresh_token: t.refresh_token,
            ..stored
        };
        self.save(&stored);
        *self.live.write().expect("auth lock") = Some(Live {
            stored,
            access_token: Some(t.access_token),
            expires_at: t.expires_at.unwrap_or_else(|| now_s() + t.expires_in.unwrap_or(3600)),
        });
        Ok(())
    }

    /// Sign out this device: revoke the session (best effort) and forget it.
    pub async fn sign_out(&self) {
        let live = self.live.read().expect("auth lock").clone();
        if let Some(l) = live {
            if let Some(token) = l.access_token {
                let _ = self
                    .http
                    .post(format!("{}/auth/v1/logout?scope=local", l.stored.supabase_url))
                    .header("apikey", &l.stored.anon_key)
                    .bearer_auth(token)
                    .send()
                    .await;
            }
        }
        self.forget();
    }

    fn forget(&self) {
        crate::secrets::delete_secret(&self.secret, &self.key_dir);
        *self.live.write().expect("auth lock") = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn pkce_pair_is_s256() {
        let (v, c) = pkce_pair();
        assert_eq!(v.len(), 43);
        assert_eq!(c, challenge_for(&v));
        assert_ne!(pkce_pair().0, v);
        // Independently computed with Python hashlib (same vector as the web app test).
        assert_eq!(
            challenge_for("vocalis-test-verifier-0123456789-abcdefghijkl"),
            "6c-bTGbAeoNzckoIrB4Np1vr5_T2iIdZ8welx-D9f7E"
        );
    }

    #[test]
    fn parses_only_our_callback() {
        let code = "a".repeat(43);
        let ok = format!("vocalis://auth/callback?code={code}&state=st_ate-1");
        assert_eq!(parse_callback(&ok), Some((code.clone(), "st_ate-1".into())));
        for bad in [
            format!("https://auth/callback?code={code}&state=s"),
            format!("vocalis://auth/other?code={code}&state=s"),
            format!("vocalis://evil/callback?code={code}&state=s"),
            "vocalis://auth/callback?state=s".to_owned(),
            format!("vocalis://auth/callback?code={code}&state=has%20space"),
            format!("vocalis://auth/callback?code={}&state=s", "a".repeat(65)),
        ] {
            assert_eq!(parse_callback(&bad), None, "{bad}");
        }
    }

    /// One-route HTTP server answering every request with `(status, body)` and
    /// recording request bodies.
    async fn mock(responses: Vec<(u16, String)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        tokio::spawn(async move {
            for (status, body) in responses {
                let (mut sock, _) = listener.accept().await.unwrap();
                let mut buf = vec![0u8; 16_384];
                let mut n = 0;
                loop {
                    n += sock.read(&mut buf[n..]).await.unwrap();
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    if let Some(i) = text.find("\r\n\r\n") {
                        let len = text
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if n >= i + 4 + len {
                            log.lock().unwrap().push(text);
                            break;
                        }
                    }
                }
                let reply = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                sock.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        (format!("http://{addr}"), seen)
    }

    /// A per-test keychain entry, so tests never touch a real signed-in session.
    fn auth(web: &str, dir: &std::path::Path) -> Auth {
        let name = format!("test-{}", dir.file_name().unwrap().to_string_lossy());
        Auth::with_secret(web, dir.to_owned(), None, name)
    }

    #[tokio::test]
    async fn sign_in_refresh_and_session_end() {
        let dir = tempfile::tempdir().unwrap();
        let (supabase, _) = mock(vec![
            (
                200,
                r#"{"access_token":"at2","refresh_token":"rt2","expires_in":3600}"#.into(),
            ),
            (400, r#"{"error":"invalid_grant"}"#.into()),
        ])
        .await;
        let exchange = format!(
            r#"{{"accessToken":"at1","refreshToken":"rt1","expiresAt":{},"user":{{"id":"u1","email":"u@example.com"}},"supabaseUrl":"{supabase}","anonKey":"anon"}}"#,
            now_s() + 60
        );
        let (web, seen) = mock(vec![(200, exchange)]).await;
        let a = auth(&web, dir.path());
        assert_eq!(a.access_token(), None);

        let url = a.begin();
        let state = url.split("state=").nth(1).unwrap().to_owned();
        let challenge = url
            .split("challenge=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap()
            .to_owned();
        let code = "c".repeat(43);
        let account = a
            .complete(&format!("vocalis://auth/callback?code={code}&state={state}"))
            .await
            .unwrap();
        assert_eq!(account.email, "u@example.com");
        assert_eq!(a.access_token().as_deref(), Some("at1"));
        // The verifier sent matches the challenge in the URL.
        let req = seen.lock().unwrap()[0].clone();
        let body: serde_json::Value = serde_json::from_str(req.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(challenge_for(body["verifier"].as_str().unwrap()), challenge);
        assert_eq!(body["code"], code.as_str());

        // Survives a restart via the stored refresh token; 60 s left means refresh.
        let b = auth(&web, dir.path());
        assert_eq!(b.account(), Some(account));
        assert_eq!(b.access_token(), None, "no access token until refreshed");
        b.refresh_if_due().await.unwrap();
        assert_eq!(b.access_token().as_deref(), Some("at2"));
        // Not due again: no request (the mock has only one more answer, a 400).
        b.refresh_if_due().await.unwrap();

        // A revoked refresh token signs the device out.
        if let Some(l) = b.live.write().unwrap().as_mut() {
            l.expires_at = 0;
        }
        assert_eq!(b.refresh_if_due().await, Err(AuthError::SessionEnded));
        assert_eq!(b.account(), None);
        assert_eq!(auth(&web, dir.path()).account(), None, "forgotten in storage too");
    }

    #[tokio::test]
    async fn callbacks_need_the_pending_state_once() {
        let dir = tempfile::tempdir().unwrap();
        let (web, _) = mock(vec![(400, r#"{"error":"invalid_grant"}"#.into())]).await;
        let a = auth(&web, dir.path());
        let code = "c".repeat(43);
        let link = |s: &str| format!("vocalis://auth/callback?code={code}&state={s}");
        assert_eq!(a.complete(&link("nobodyasked")).await, Err(AuthError::UnknownState));
        let url = a.begin();
        let state = url.split("state=").nth(1).unwrap().to_owned();
        assert_eq!(a.complete(&link("wrongstate")).await, Err(AuthError::UnknownState));
        assert_eq!(a.complete(&link(&state)).await, Err(AuthError::Rejected));
        assert_eq!(
            a.complete(&link(&state)).await,
            Err(AuthError::UnknownState),
            "single use"
        );
        assert_eq!(a.complete("vocalis://auth/callback").await, Err(AuthError::BadLink));
    }

    #[test]
    fn dev_token_wins() {
        let dir = tempfile::tempdir().unwrap();
        let a = Auth::new("http://x", dir.path().to_owned(), Some("dev".into()));
        assert_eq!(a.access_token().as_deref(), Some("dev"));
    }
}
