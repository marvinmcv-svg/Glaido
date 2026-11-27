//! WebSocket client for the gateway's `/v1/dictate` (implements `vocalis_session::Gateway`).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use vocalis_session::{FinalResult, Gateway, GatewayError, GatewaySession, StartSettings};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Supplies a fresh access token (from the signed-in session) per dictation.
pub type TokenSource = Arc<dyn Fn() -> Option<String> + Send + Sync>;

pub struct WsGateway {
    url: String,
    token: TokenSource,
}

impl WsGateway {
    /// `base` like `https://api.vocalis.app` or `http://127.0.0.1:8787`.
    pub fn new(base: &str, token: TokenSource) -> Self {
        let base = base.trim_end_matches('/');
        let url = if let Some(rest) = base.strip_prefix("https://") {
            format!("wss://{rest}/v1/dictate")
        } else if let Some(rest) = base.strip_prefix("http://") {
            format!("ws://{rest}/v1/dictate")
        } else {
            format!("{base}/v1/dictate")
        };
        Self { url, token }
    }
}

struct WsSession(Ws);

async fn next_json(ws: &mut Ws) -> Result<Value, GatewayError> {
    while let Some(msg) = ws.next().await {
        match msg.map_err(|_| GatewayError::Disconnected)? {
            Message::Text(t) => return serde_json::from_str(&t).map_err(|_| GatewayError::Disconnected),
            Message::Close(_) => return Err(GatewayError::Disconnected),
            _ => {}
        }
    }
    Err(GatewayError::Disconnected)
}

fn server_error(v: &Value) -> GatewayError {
    GatewayError::Server {
        code: v["code"].as_str().unwrap_or("SERVER_ERROR").to_owned(),
        retryable: v["retryable"].as_bool().unwrap_or(false),
    }
}

#[async_trait]
impl Gateway for WsGateway {
    async fn open(&self, settings: &StartSettings) -> Result<Box<dyn GatewaySession>, GatewayError> {
        let token = (self.token)().ok_or(GatewayError::Server {
            code: "SIGNED_OUT".into(),
            retryable: false,
        })?;
        let mut req = self
            .url
            .as_str()
            .into_client_request()
            .map_err(|_| GatewayError::Connect)?;
        req.headers_mut().insert(
            "authorization",
            format!("Bearer {token}").parse().map_err(|_| GatewayError::Connect)?,
        );
        let connect = tokio_tungstenite::connect_async(req);
        let (mut ws, _) = tokio::time::timeout(Duration::from_secs(5), connect)
            .await
            .map_err(|_| GatewayError::Timeout)?
            .map_err(|_| GatewayError::Connect)?;
        let mut start = serde_json::to_value(settings).map_err(|_| GatewayError::Connect)?;
        start["type"] = json!("start");
        start["sampleRate"] = json!(16_000);
        ws.send(Message::Text(start.to_string().into()))
            .await
            .map_err(|_| GatewayError::Disconnected)?;
        let ready = next_json(&mut ws).await?;
        match ready["type"].as_str() {
            Some("ready") => Ok(Box::new(WsSession(ws))),
            Some("error") => Err(server_error(&ready)),
            _ => Err(GatewayError::Disconnected),
        }
    }
}

#[async_trait]
impl GatewaySession for WsSession {
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<(), GatewayError> {
        if pcm.is_empty() {
            return Ok(());
        }
        let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
        self.0
            .send(Message::Binary(bytes.into()))
            .await
            .map_err(|_| GatewayError::Disconnected)
    }

    async fn finish(mut self: Box<Self>) -> Result<FinalResult, GatewayError> {
        self.0
            .send(Message::Text(json!({"type": "stop"}).to_string().into()))
            .await
            .map_err(|_| GatewayError::Disconnected)?;
        let v = next_json(&mut self.0).await?;
        match v["type"].as_str() {
            Some("final") => serde_json::from_value(v).map_err(|_| GatewayError::Disconnected),
            Some("error") => Err(server_error(&v)),
            _ => Err(GatewayError::Disconnected),
        }
    }

    async fn cancel(mut self: Box<Self>) {
        let _ = self
            .0
            .send(Message::Text(json!({"type": "cancel"}).to_string().into()))
            .await;
        let _ = self.0.close(None).await;
    }
}
