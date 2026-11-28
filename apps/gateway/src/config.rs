//! Gateway configuration from the environment. Secrets are read once and never logged.

use std::fmt;

#[derive(Clone)]
pub struct Config {
    pub port: u16,
    /// Supabase project URL; enables JWKS verification of asymmetric tokens (ADR-0007).
    pub supabase_url: Option<String>,
    /// Legacy Supabase JWT secret for HS256 tokens. Optional.
    pub jwt_secret: Option<String>,
    pub min_client_version: String,
    pub latest_client_version: String,
    pub groq_configured: bool,
    pub deepgram_configured: bool,
    pub anthropic_configured: bool,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("port", &self.port)
            .field("supabase_url", &self.supabase_url)
            .field("jwt_secret", &self.jwt_secret.as_ref().map(|_| "<redacted>"))
            .field("min_client_version", &self.min_client_version)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("set SUPABASE_URL (JWKS) and/or SUPABASE_JWT_SECRET (legacy HS256)")]
    MissingAuthConfig,
    #[error("PORT is not a valid port number")]
    BadPort,
}

fn present(key: &str) -> bool {
    std::env::var(key).is_ok_and(|v| !v.trim().is_empty())
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let nonempty = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let supabase_url = nonempty("SUPABASE_URL");
        let jwt_secret = nonempty("SUPABASE_JWT_SECRET");
        if supabase_url.is_none() && jwt_secret.is_none() {
            return Err(ConfigError::MissingAuthConfig);
        }
        let port = match std::env::var("PORT") {
            Ok(p) => p.parse().map_err(|_| ConfigError::BadPort)?,
            Err(_) => 8787,
        };
        Ok(Self {
            port,
            supabase_url,
            jwt_secret,
            min_client_version: std::env::var("MIN_CLIENT_VERSION").unwrap_or_else(|_| "0.1.0".into()),
            latest_client_version: std::env::var("LATEST_CLIENT_VERSION")
                .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").into()),
            groq_configured: present("GROQ_API_KEY"),
            deepgram_configured: present("DEEPGRAM_API_KEY"),
            anthropic_configured: present("ANTHROPIC_API_KEY"),
        })
    }

    /// A config for tests and local tooling.
    pub fn for_tests(jwt_secret: &str) -> Self {
        Self {
            port: 0,
            supabase_url: None,
            jwt_secret: Some(jwt_secret.into()),
            min_client_version: "0.1.0".into(),
            latest_client_version: "0.1.0".into(),
            groq_configured: false,
            deepgram_configured: false,
            anthropic_configured: false,
        }
    }
}
