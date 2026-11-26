//! Provider clients against mock HTTP servers: exact request shape and response handling.

use serde_json::json;
use vocalis_gateway::providers::anthropic::Anthropic;
use vocalis_gateway::providers::deepgram::Deepgram;
use vocalis_gateway::providers::groq::Groq;
use vocalis_gateway::providers::{Audio, Cleanup, ProviderError, SpeechToText};
use wiremock::matchers::{body_partial_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn audio<'a>(pcm: &'a [i16], langs: &'a [String], hints: &'a [String]) -> Audio<'a> {
    Audio {
        pcm,
        sample_rate: 16_000,
        languages: langs,
        hints,
    }
}

#[tokio::test]
async fn groq_sends_multipart_whisper_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/audio/transcriptions"))
        .and(header("authorization", "Bearer gsk-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "text": "  hello world " })))
        .expect(1)
        .mount(&server)
        .await;
    let g = Groq::new("gsk-test".into(), Some(server.uri()), None);
    let pcm = vec![0i16; 1600];
    let langs = vec!["de".to_owned()];
    let hints = vec!["Vocalis".to_owned()];
    assert_eq!(g.transcribe(&audio(&pcm, &langs, &hints)).await.unwrap(), "hello world");

    let req: Request = server.received_requests().await.unwrap().remove(0);
    let body = String::from_utf8_lossy(&req.body);
    for needle in [
        "name=\"model\"\r\n\r\nwhisper-large-v3-turbo",
        "name=\"language\"\r\n\r\nde",
        "name=\"prompt\"\r\n\r\nVocalis",
        "filename=\"audio.wav\"",
        "RIFF",
    ] {
        assert!(body.contains(needle), "missing {needle:?}");
    }
}

#[tokio::test]
async fn groq_omits_language_for_auto_detect_and_maps_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;
    let g = Groq::new("k".into(), Some(server.uri()), None);
    let langs = vec!["en".to_owned(), "de".to_owned()];
    assert_eq!(
        g.transcribe(&audio(&[0; 10], &langs, &[])).await,
        Err(ProviderError::RateLimited)
    );
    let body = String::from_utf8_lossy(&server.received_requests().await.unwrap()[0].body).to_string();
    assert!(!body.contains("name=\"language\""));
}

#[tokio::test]
async fn deepgram_sends_wav_with_keyterms() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/listen"))
        .and(header("authorization", "Token dg-test"))
        .and(header("content-type", "audio/wav"))
        .and(query_param("model", "nova-3"))
        .and(query_param("detect_language", "true"))
        .and(query_param("keyterm", "Supabase"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": { "channels": [ { "alternatives": [ { "transcript": "ship it" } ] } ] }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let d = Deepgram::new("dg-test".into(), Some(server.uri()), None);
    let hints = vec!["Supabase".to_owned()];
    assert_eq!(d.transcribe(&audio(&[0; 10], &[], &hints)).await.unwrap(), "ship it");
}

#[tokio::test]
async fn deepgram_malformed_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "results": { "channels": [] } })))
        .mount(&server)
        .await;
    let d = Deepgram::new("k".into(), Some(server.uri()), None);
    assert_eq!(
        d.transcribe(&audio(&[0; 10], &[], &[])).await,
        Err(ProviderError::Malformed)
    );
}

#[tokio::test]
async fn anthropic_request_shape_and_text_extraction() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "sk-ant-test"))
        .and(header("anthropic-version", "2026-10-21"))
        .and(body_partial_json(json!({
            "model": "claude-haiku-5-5",
            "system": "SYS",
            "output_config": { "effort": "low" },
            "messages": [ { "role": "user", "content": "USER" } ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": [ { "type": "thinking", "thinking": "", "signature": "s" }, { "type": "text", "text": " Clean text. " } ],
            "stop_reason": "end_turn"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let a = Anthropic::new("sk-ant-test".into(), Some(server.uri()), None);
    assert_eq!(a.clean("SYS", "USER").await.unwrap(), "Clean text.");
    let sent: serde_json::Value = serde_json::from_slice(&server.received_requests().await.unwrap()[0].body).unwrap();
    assert!(
        sent.get("temperature").is_none(),
        "Haiku 5.5 rejects non-default sampling parameters"
    );
}

#[tokio::test]
async fn anthropic_refusal_and_truncation_are_errors() {
    for (stop, expected) in [
        ("refusal", ProviderError::Refused),
        ("max_tokens", ProviderError::Malformed),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    json!({ "content": [ { "type": "text", "text": "partial" } ], "stop_reason": stop }),
                ),
            )
            .mount(&server)
            .await;
        let a = Anthropic::new("k".into(), Some(server.uri()), None);
        assert_eq!(a.clean("s", "u").await, Err(expected));
    }
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(529))
        .mount(&server)
        .await;
    let a = Anthropic::new("k".into(), Some(server.uri()), None);
    let e = a.clean("s", "u").await.unwrap_err();
    assert_eq!(e, ProviderError::Status(529));
    assert!(e.retryable());
}
