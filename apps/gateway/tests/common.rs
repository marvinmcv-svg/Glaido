#![allow(dead_code)]

use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::json;

pub const SECRET: &str = "test-secret-with-at-least-32-characters!!";

pub fn token(sub: &str, aud: &str, exp_offset_secs: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    encode(
        &Header::default(),
        &json!({ "sub": sub, "aud": aud, "exp": now + exp_offset_secs, "role": "authenticated" }),
        &EncodingKey::from_secret(SECRET.as_bytes()),
    )
    .unwrap()
}
