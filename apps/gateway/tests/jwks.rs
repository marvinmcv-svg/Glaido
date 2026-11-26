//! Asymmetric (ES256) token verification via a JWKS, as issued by current Supabase.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::pkcs8::{EncodePrivateKey, LineEnding};
use serde_json::json;
use tower::ServiceExt;
use vocalis_gateway::auth::Verifier;
use vocalis_gateway::{app, config::Config, AppState};

struct TestKey {
    kid: String,
    pem: String,
    jwks: JwkSet,
}

/// A fresh P-256 key per test run, so no private key lives in the repo.
fn test_key(kid: &str) -> TestKey {
    let sk = p256::SecretKey::random(&mut rand_core::OsRng);
    let pem = sk.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
    let point = sk.public_key().to_encoded_point(false);
    let jwks: JwkSet = serde_json::from_value(json!({ "keys": [{
        "kty": "EC", "crv": "P-256", "alg": "ES256", "use": "sig", "kid": kid,
        "x": URL_SAFE_NO_PAD.encode(point.x().unwrap()),
        "y": URL_SAFE_NO_PAD.encode(point.y().unwrap()),
    }]}))
    .unwrap();
    TestKey {
        kid: kid.into(),
        pem,
        jwks,
    }
}

fn es256_token(key: &TestKey, kid: &str, sub: &str, aud: &str, exp_offset: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(kid.into());
    encode(
        &header,
        &json!({ "sub": sub, "aud": aud, "exp": now + exp_offset, "role": "authenticated" }),
        &EncodingKey::from_ec_pem(key.pem.as_bytes()).unwrap(),
    )
    .unwrap()
}

async fn status(verifier: Verifier, token: &str) -> StatusCode {
    let router = app(AppState::with_verifier(Config::for_tests(common::SECRET), verifier));
    let req = Request::get("/v1/me")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    router.oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn accepts_es256_from_jwks() {
    let k = test_key("kid-1");
    let v = Verifier::with_jwks(&k.jwks, None);
    assert_eq!(
        v.verify(&es256_token(&k, &k.kid, "user-9", "authenticated", 600)).await,
        Ok("user-9".into())
    );
}

#[tokio::test]
async fn rejects_bad_es256_tokens() {
    let k = test_key("kid-1");
    let other = test_key("kid-1"); // same kid, different key: signature must fail
    let cases = [
        es256_token(&k, "unknown-kid", "u", "authenticated", 600),
        es256_token(&k, &k.kid, "u", "anon", 600),
        es256_token(&k, &k.kid, "u", "authenticated", -3600),
        es256_token(&other, &k.kid, "u", "authenticated", 600),
    ];
    for t in cases {
        assert_eq!(
            status(Verifier::with_jwks(&k.jwks, None), &t).await,
            StatusCode::UNAUTHORIZED
        );
    }
}

#[tokio::test]
async fn hs256_is_rejected_without_a_legacy_secret() {
    let k = test_key("kid-1");
    let hs = common::token("u", "authenticated", 600);
    assert_eq!(
        status(Verifier::with_jwks(&k.jwks, None), &hs).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(Verifier::with_jwks(&k.jwks, Some(common::SECRET)), &hs).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn end_to_end_through_the_router() {
    let k = test_key("kid-2");
    let t = es256_token(&k, &k.kid, "user-1", "authenticated", 600);
    assert_eq!(status(Verifier::with_jwks(&k.jwks, None), &t).await, StatusCode::OK);
}
