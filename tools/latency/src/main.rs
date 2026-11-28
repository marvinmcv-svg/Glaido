//! `vocalis-latency`: play scripted dictations into a gateway through the same
//! WebSocket client the desktop app uses, and check stop → result latency
//! against the §5.9 budgets. See docs/runbooks/latency.md.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use vocalis_client::WsGateway;
use vocalis_latency::{evaluate, gate_passes, parse_manifest, percentile, synthetic_audio, wer, Kind, Sample, RATE};
use vocalis_session::{Gateway, StartSettings};

const USAGE: &str = "\
Usage: vocalis-latency [options]

  --gateway URL        gateway base URL (default http://127.0.0.1:8787)
  --manifest PATH      utterances, id<TAB>kind<TAB>text (default e2e/latency/utterances.tsv)
  --audio-dir DIR      real speech: DIR/<id>.wav, 16 kHz mono 16-bit PCM
  --synthetic          speech-like tones sized from the word count (mock providers only)
  --speed X            stream audio X times faster than real time (default 1)
  --token TOKEN        access token for the gateway
  --jwt-secret SECRET  mint an HS256 test token instead (local gateways only)
  --language CODE      dictation language (default en)
  --limit N            only the first N utterances
  --json PATH          write the full report as JSON
  --gate               exit 1 if a budget is missed or a dictation fails
";

struct Args {
    gateway: String,
    manifest: PathBuf,
    audio_dir: Option<PathBuf>,
    synthetic: bool,
    speed: f64,
    token: Option<String>,
    jwt_secret: Option<String>,
    language: String,
    limit: Option<usize>,
    json: Option<PathBuf>,
    gate: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        gateway: "http://127.0.0.1:8787".into(),
        manifest: "e2e/latency/utterances.tsv".into(),
        audio_dir: None,
        synthetic: false,
        speed: 1.0,
        token: None,
        jwt_secret: None,
        language: "en".into(),
        limit: None,
        json: None,
        gate: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut val = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--gateway" => a.gateway = val()?,
            "--manifest" => a.manifest = val()?.into(),
            "--audio-dir" => a.audio_dir = Some(val()?.into()),
            "--synthetic" => a.synthetic = true,
            "--speed" => {
                a.speed = val()?
                    .parse()
                    .ok()
                    .filter(|s: &f64| *s > 0.0)
                    .ok_or("--speed must be a positive number")?
            }
            "--token" => a.token = Some(val()?),
            "--jwt-secret" => a.jwt_secret = Some(val()?),
            "--language" => a.language = val()?,
            "--limit" => a.limit = Some(val()?.parse().map_err(|_| "--limit must be a number")?),
            "--json" => a.json = Some(val()?.into()),
            "--gate" => a.gate = true,
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    if a.synthetic == a.audio_dir.is_some() {
        return Err("choose exactly one of --audio-dir or --synthetic".into());
    }
    if a.token.is_none() && a.jwt_secret.is_none() {
        return Err("pass --token, or --jwt-secret for a local gateway".into());
    }
    Ok(a)
}

fn mint(secret: &str) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Claims {
        sub: &'static str,
        aud: &'static str,
        exp: u64,
    }
    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() + 3600)
        .unwrap_or(0);
    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &Claims {
            sub: "latency-harness",
            aud: "authenticated",
            exp,
        },
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())
}

fn load_wav(dir: &std::path::Path, id: &str) -> Result<Vec<i16>, String> {
    let path = dir.join(format!("{id}.wav"));
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (samples, rate) = vocalis_audio::wav::decode(&bytes)
        .map_err(|e| format!("{}: {e} (need 16 kHz mono 16-bit PCM)", path.display()))?;
    if rate != RATE {
        return Err(format!("{}: {rate} Hz; need 16 kHz mono 16-bit PCM", path.display()));
    }
    Ok(samples)
}

/// One dictation: open, stream at the chosen pace, stop, wait for the result.
async fn measure(
    gateway: &WsGateway,
    settings: &StartSettings,
    audio: &[i16],
    speed: f64,
) -> Result<(f64, f64, String), String> {
    let t0 = Instant::now();
    let mut session = gateway.open(settings).await.map_err(|e| e.to_string())?;
    let open_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let chunk = RATE as usize / 10; // 100 ms, like the capture loop
    let pace = Duration::from_secs_f64(0.1 / speed);
    let start = Instant::now();
    for (i, c) in audio.chunks(chunk).enumerate() {
        session.send_audio(c).await.map_err(|e| e.to_string())?;
        // Keep to the wall clock so slow sends don't stretch the recording.
        let due = start + pace * (i as u32 + 1);
        tokio::time::sleep_until(due.into()).await;
    }
    let t1 = Instant::now();
    let result = session.finish().await.map_err(|e| e.to_string())?;
    Ok((open_ms, t1.elapsed().as_secs_f64() * 1000.0, result.text))
}

fn fmt_ms(v: Option<f64>) -> String {
    v.map(|v| format!("{v:.0} ms")).unwrap_or_else(|| "—".into())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let manifest = std::fs::read_to_string(&args.manifest)
        .map_err(|e| format!("{}: {e}", args.manifest.display()))
        .and_then(|s| parse_manifest(&s));
    let mut utterances = match manifest {
        Ok(m) => m,
        Err(e) => {
            eprintln!("manifest: {e}");
            std::process::exit(2);
        }
    };
    if let Some(n) = args.limit {
        utterances.truncate(n);
    }
    // Load all audio up front so file I/O never lands inside a measurement.
    let mut audio = Vec::new();
    for u in &utterances {
        let a = match &args.audio_dir {
            Some(dir) => load_wav(dir, &u.id),
            None => Ok(synthetic_audio(u.words())),
        };
        match a {
            Ok(a) => audio.push(a),
            Err(e) => {
                eprintln!("audio: {e}");
                std::process::exit(2);
            }
        }
    }
    let token = match (&args.token, &args.jwt_secret) {
        (Some(t), _) => t.clone(),
        (None, Some(s)) => mint(s).unwrap_or_else(|e| {
            eprintln!("token: {e}");
            std::process::exit(2);
        }),
        (None, None) => unreachable!("checked in parse_args"),
    };
    let gateway = WsGateway::new(&args.gateway, Arc::new(move || Some(token.clone())));
    let settings = StartSettings {
        languages: vec![args.language.clone()],
        ..Default::default()
    };

    println!(
        "vocalis-latency: {} utterances against {} ({}, {}x real time)",
        utterances.len(),
        args.gateway,
        if args.synthetic {
            "synthetic audio"
        } else {
            "recorded speech"
        },
        args.speed
    );
    // Warm-up (connection setup, provider cold starts); not counted.
    if let Err(e) = measure(&gateway, &settings, &audio[0], args.speed).await {
        eprintln!("warm-up failed: {e}");
    }

    let mut samples = Vec::new();
    println!(
        "{:<10} {:<6} {:>5} {:>8} {:>9} {:>11} {:>6}",
        "id", "kind", "words", "audio", "open", "stop→final", "WER"
    );
    for (u, a) in utterances.iter().zip(&audio) {
        let audio_ms = a.len() as u64 * 1000 / RATE as u64;
        let r = measure(&gateway, &settings, a, args.speed).await;
        let s = match r {
            Ok((open_ms, final_ms, text)) => Sample {
                id: u.id.clone(),
                kind: u.kind,
                words: u.words(),
                audio_ms,
                open_ms: Some(open_ms),
                stop_to_final_ms: Some(final_ms),
                wer: (!args.synthetic).then(|| wer(&u.text, &text)),
                error: None,
            },
            Err(e) => Sample {
                id: u.id.clone(),
                kind: u.kind,
                words: u.words(),
                audio_ms,
                open_ms: None,
                stop_to_final_ms: None,
                wer: None,
                error: Some(e),
            },
        };
        println!(
            "{:<10} {:<6} {:>5} {:>6.1} s {:>9} {:>11} {:>6}",
            s.id,
            if s.kind == Kind::Short { "short" } else { "long" },
            s.words,
            s.audio_ms as f64 / 1000.0,
            fmt_ms(s.open_ms),
            s.error.clone().unwrap_or_else(|| fmt_ms(s.stop_to_final_ms)),
            s.wer
                .map(|w| format!("{:.1}%", w * 100.0))
                .unwrap_or_else(|| "—".into()),
        );
        samples.push(s);
    }

    let checks = evaluate(&samples);
    let opens: Vec<f64> = samples.iter().filter_map(|s| s.open_ms).collect();
    let wers: Vec<f64> = samples.iter().filter_map(|s| s.wer).collect();
    let failed = samples.iter().filter(|s| s.error.is_some()).count();
    println!("\nBudgets (§5.9, stop → final result):");
    for c in &checks {
        let verdict = match c.pass {
            Some(true) => "pass",
            Some(false) => "FAIL",
            None => "not measured",
        };
        println!(
            "  {:<16} {:>9}  limit {:>6.0} ms  {verdict}",
            c.name,
            fmt_ms(c.measured_ms),
            c.limit_ms
        );
    }
    println!(
        "  open (connect + start → ready) p50 {}",
        fmt_ms(percentile(&opens, 0.5))
    );
    if !wers.is_empty() {
        println!(
            "  mean WER {:.1}%",
            wers.iter().sum::<f64>() / wers.len() as f64 * 100.0
        );
    }
    println!("  failed dictations: {failed}");
    println!(
        "\nNote: stop → final covers the network, speech-to-text and cleanup. The local paste\n\
         (clipboard + keystroke, a few ms; the clipboard restore runs after) is not included;\n\
         the desktop E2E covers it."
    );

    let passed = gate_passes(&samples, &checks);
    if let Some(path) = &args.json {
        let report = serde_json::json!({
            "gateway": args.gateway,
            "synthetic": args.synthetic,
            "speed": args.speed,
            "samples": samples,
            "checks": checks,
            "openP50Ms": percentile(&opens, 0.5),
            "meanWer": (!wers.is_empty()).then(|| wers.iter().sum::<f64>() / wers.len() as f64),
            "passed": passed,
        });
        if let Err(e) = std::fs::write(path, serde_json::to_vec_pretty(&report).expect("serialize")) {
            eprintln!("{}: {e}", path.display());
        }
    }
    if args.gate && !passed {
        println!("\nGATE FAILED");
        std::process::exit(1);
    }
    if args.gate {
        println!("\nGATE PASSED");
    }
}
