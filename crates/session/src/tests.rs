use std::sync::Mutex;

use async_trait::async_trait;

use super::*;

// ---------- fakes ----------

#[derive(Clone)]
enum OpenBehavior {
    Ok,
    Fail,
    Delay(Duration),
}

#[derive(Clone)]
struct FakeGateway {
    open: OpenBehavior,
    send_fails: bool,
    finish: Result<FinalResult, GatewayError>,
    finish_delay: Duration,
    received: Arc<Mutex<Vec<i16>>>,
    cancelled: Arc<Mutex<u32>>,
}

impl FakeGateway {
    fn ok(text: &str) -> Self {
        Self {
            open: OpenBehavior::Ok,
            send_fails: false,
            finish: Ok(FinalResult {
                plan: PlanKind::Paste,
                text: text.into(),
                words: 2,
                wpm: 120,
                duration_ms: 1000,
            }),
            finish_delay: Duration::ZERO,
            received: Default::default(),
            cancelled: Default::default(),
        }
    }
}

struct FakeSession(FakeGateway);

#[async_trait]
impl Gateway for FakeGateway {
    async fn open(&self, _: &StartSettings) -> Result<Box<dyn GatewaySession>, GatewayError> {
        match self.open {
            OpenBehavior::Ok => {}
            OpenBehavior::Fail => return Err(GatewayError::Connect),
            OpenBehavior::Delay(d) => tokio::time::sleep(d).await,
        }
        Ok(Box::new(FakeSession(self.clone())))
    }
}

#[async_trait]
impl GatewaySession for FakeSession {
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<(), GatewayError> {
        if self.0.send_fails {
            return Err(GatewayError::Disconnected);
        }
        self.0.received.lock().unwrap().extend_from_slice(pcm);
        Ok(())
    }
    async fn finish(self: Box<Self>) -> Result<FinalResult, GatewayError> {
        tokio::time::sleep(self.0.finish_delay).await;
        self.0.finish.clone()
    }
    async fn cancel(self: Box<Self>) {
        *self.0.cancelled.lock().unwrap() += 1;
    }
}

#[derive(Default)]
struct FakeUi {
    events: Mutex<Vec<String>>,
    paste_outcome: Mutex<Option<PasteOutcome>>,
}

impl FakeUi {
    fn events(&self) -> Vec<String> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| !e.starts_with("level"))
            .cloned()
            .collect()
    }
}

impl Ui for FakeUi {
    fn bar(&self, s: BarState) {
        self.events.lock().unwrap().push(format!("bar {s:?}"));
    }
    fn level(&self, l: f32) {
        self.events.lock().unwrap().push(format!("level {l:.2}"));
    }
    fn sound(&self, s: Sound) {
        self.events.lock().unwrap().push(format!("sound {s:?}"));
    }
    fn notify(&self, n: Notice) {
        let tag = match n {
            Notice::TranscriptionFailed { code, .. } => format!("failed {code}"),
            Notice::NowhereToPaste { .. } => "nowhere".into(),
            Notice::PasteFailed { .. } => "paste-failed".into(),
            Notice::RecordingLimitNear => "limit-near".into(),
            Notice::MicSilent => "mic-silent".into(),
            Notice::RetrySucceeded { .. } => "retry-ok".into(),
        };
        self.events.lock().unwrap().push(format!("notify {tag}"));
    }
    fn paste(&self, text: &str) -> PasteOutcome {
        self.events.lock().unwrap().push(format!("paste {text}"));
        self.paste_outcome.lock().unwrap().unwrap_or(PasteOutcome::Pasted)
    }
    fn command(&self, request: Option<&str>, _: &str) {
        self.events.lock().unwrap().push(format!("command {request:?}"));
    }
}

#[derive(Default)]
struct FakeHistory {
    inserted: Mutex<Vec<NewEntry>>,
    completed: Mutex<Vec<(String, String)>>,
    failed: Mutex<Vec<(String, String)>>,
}

impl History for FakeHistory {
    fn insert(&self, e: NewEntry) {
        self.inserted.lock().unwrap().push(e);
    }
    fn complete(&self, id: &str, text: &str, _: u32, _: u32) {
        self.completed.lock().unwrap().push((id.into(), text.into()));
    }
    fn fail(&self, id: &str, code: &str) {
        self.failed.lock().unwrap().push((id.into(), code.into()));
    }
}

struct Rig {
    c: Controller,
    ui: Arc<FakeUi>,
    history: Arc<FakeHistory>,
    gw: FakeGateway,
    _dir: tempfile::TempDir,
    dir: PathBuf,
}

fn rig(gw: FakeGateway) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_owned();
    let ui = Arc::new(FakeUi::default());
    let history = Arc::new(FakeHistory::default());
    let c = Controller::new(
        Config::new(path.clone(), [3u8; 32]),
        Arc::new(gw.clone()),
        ui.clone(),
        history.clone(),
    );
    Rig {
        c,
        ui,
        history,
        gw,
        _dir: dir,
        dir: path,
    }
}

/// `ms` of a loud tone (speech-like for the VAD), in 100 ms blocks.
fn speech(ms: u64) -> Vec<Vec<i16>> {
    let n = (ms * 16) as usize;
    let all: Vec<i16> = (0..n).map(|i| ((i as f32 * 0.2).sin() * 12_000.0) as i16).collect();
    all.chunks(1600).map(|c| c.to_vec()).collect()
}

fn silence(ms: u64) -> Vec<Vec<i16>> {
    vec![vec![0i16; 1600]; (ms / 100) as usize]
}

async fn record(r: &mut Rig, blocks: Vec<Vec<i16>>) {
    r.c.start(StartSettings::default(), 1_764_115_200_000);
    for b in blocks {
        r.c.audio(&b).await;
    }
}

fn journals(dir: &Path) -> usize {
    std::fs::read_dir(dir).unwrap().count()
}

// ---------- tests ----------

#[tokio::test]
async fn happy_path_streams_pastes_and_records() {
    let mut r = rig(FakeGateway::ok("Hello there."));
    record(&mut r, speech(1000)).await;
    r.c.stop();
    r.c.settle().await;
    assert_eq!(
        r.gw.received.lock().unwrap().len(),
        16_000,
        "every sample reaches the gateway, including those buffered before it opened"
    );
    assert_eq!(
        r.ui.events(),
        vec![
            "bar Recording",
            "sound Start",
            "sound Stop",
            "bar Processing",
            "bar Hidden",
            "paste Hello there."
        ]
    );
    let rec = &r.history.inserted.lock().unwrap()[0];
    assert_eq!(
        (rec.text.as_str(), rec.error_code.as_deref(), rec.duration_ms),
        ("Hello there.", None, 1000)
    );
    let audio = rec.audio_path.clone().expect("audio kept");
    let back = journal::read(Path::new(&audio), &[3u8; 32]).unwrap();
    assert_eq!(back.samples.len(), 16_000);
    assert!(back.complete);
}

#[tokio::test]
async fn silence_is_dropped_without_a_record() {
    let mut r = rig(FakeGateway::ok("x"));
    record(&mut r, silence(1000)).await;
    r.c.stop();
    r.c.settle().await;
    assert!(r.history.inserted.lock().unwrap().is_empty());
    assert_eq!(journals(&r.dir), 0, "journal deleted");
    assert!(
        !r.ui.events().contains(&"notify mic-silent".to_owned()),
        "short silence is not a mic problem"
    );
    tokio::task::yield_now().await;
    assert!(*r.gw.cancelled.lock().unwrap() <= 1);
}

#[tokio::test]
async fn long_silence_warns_about_the_microphone() {
    let mut r = rig(FakeGateway::ok("x"));
    record(&mut r, silence(4000)).await;
    r.c.stop();
    assert!(r.ui.events().contains(&"notify mic-silent".to_owned()));
}

#[tokio::test]
async fn offline_becomes_a_failed_record_with_audio() {
    let mut r = rig(FakeGateway {
        open: OpenBehavior::Fail,
        ..FakeGateway::ok("x")
    });
    record(&mut r, speech(800)).await;
    r.c.stop();
    r.c.settle().await;
    let rec = r.history.inserted.lock().unwrap()[0].clone();
    assert_eq!(rec.error_code.as_deref(), Some("NETWORK"));
    assert!(rec.audio_path.as_deref().is_some_and(|p| Path::new(p).exists()));
    assert!(r.ui.events().contains(&"notify failed NETWORK".to_owned()));
    assert!(r.ui.events().contains(&"sound Error".to_owned()));
}

#[tokio::test]
async fn connection_drop_mid_recording_still_keeps_audio() {
    let mut r = rig(FakeGateway {
        send_fails: true,
        ..FakeGateway::ok("x")
    });
    record(&mut r, speech(800)).await;
    r.c.stop();
    r.c.settle().await;
    let rec = r.history.inserted.lock().unwrap()[0].clone();
    assert_eq!(rec.error_code.as_deref(), Some("NETWORK"));
    let back = journal::read(Path::new(rec.audio_path.as_deref().unwrap()), &[3u8; 32]).unwrap();
    assert_eq!(back.samples.len(), 800 * 16);
}

#[tokio::test]
async fn server_errors_map_to_record_codes() {
    let gw = FakeGateway {
        finish: Err(GatewayError::Server {
            code: "QUOTA_EXCEEDED".into(),
            retryable: false,
        }),
        ..FakeGateway::ok("x")
    };
    let mut r = rig(gw);
    record(&mut r, speech(500)).await;
    r.c.stop();
    r.c.settle().await;
    assert_eq!(
        r.history.inserted.lock().unwrap()[0].error_code.as_deref(),
        Some("QUOTA_EXCEEDED")
    );
}

#[tokio::test(start_paused = true)]
async fn slow_connection_then_timeout() {
    let gw = FakeGateway {
        finish_delay: Duration::from_secs(60),
        ..FakeGateway::ok("late")
    };
    let mut r = rig(gw);
    record(&mut r, speech(500)).await;
    r.c.stop();
    r.c.settle().await;
    let ev = r.ui.events();
    assert!(ev.contains(&"bar SlowConnection".to_owned()), "{ev:?}");
    assert!(
        ev.contains(&"notify failed NETWORK".to_owned()),
        "timeout is a network failure: {ev:?}"
    );
    assert!(!ev.iter().any(|e| e.starts_with("paste")));
}

#[tokio::test(start_paused = true)]
async fn late_result_after_a_new_recording_goes_to_history_only() {
    let gw = FakeGateway {
        finish_delay: Duration::from_millis(1500),
        ..FakeGateway::ok("first")
    };
    let mut r = rig(gw);
    record(&mut r, speech(500)).await;
    r.c.stop();
    r.c.start(StartSettings::default(), 1_764_115_300_000); // user moved on
    r.c.settle().await;
    assert_eq!(r.history.inserted.lock().unwrap()[0].text, "first");
    assert!(
        !r.ui.events().iter().any(|e| e.starts_with("paste")),
        "not pasted into whatever the user is doing now"
    );
    assert!(r.c.is_recording());
}

#[tokio::test]
async fn nowhere_to_paste_notifies_and_keeps_text() {
    let mut r = rig(FakeGateway::ok("kept"));
    *r.ui.paste_outcome.lock().unwrap() = Some(PasteOutcome::NowhereToPaste);
    record(&mut r, speech(500)).await;
    r.c.stop();
    r.c.settle().await;
    assert!(r.ui.events().contains(&"notify nowhere".to_owned()));
    assert_eq!(r.history.inserted.lock().unwrap()[0].text, "kept");
}

#[tokio::test]
async fn wake_word_result_opens_command() {
    let gw = FakeGateway {
        finish: Ok(FinalResult {
            plan: PlanKind::Command,
            text: "summarize this".into(),
            words: 2,
            wpm: 0,
            duration_ms: 900,
        }),
        ..FakeGateway::ok("")
    };
    let mut r = rig(gw);
    record(&mut r, speech(500)).await;
    r.c.stop();
    r.c.settle().await;
    assert!(r.ui.events().contains(&"command Some(\"summarize this\")".to_owned()));
    assert!(r.history.inserted.lock().unwrap()[0].is_command);
}

#[tokio::test]
async fn cancel_keeps_audio_and_discard_keeps_nothing() {
    let mut r = rig(FakeGateway::ok("never"));
    record(&mut r, speech(600)).await;
    r.c.cancel();
    r.c.settle().await;
    let rec = r.history.inserted.lock().unwrap()[0].clone();
    assert_eq!(rec.error_code.as_deref(), Some("CANCELLED"));
    assert!(Path::new(rec.audio_path.as_deref().unwrap()).exists());
    assert!(!r.ui.events().iter().any(|e| e.starts_with("paste")));

    let before = journals(&r.dir);
    record(&mut r, speech(200)).await;
    r.c.discard();
    assert_eq!(journals(&r.dir), before, "tap leaves no file");
    assert_eq!(r.history.inserted.lock().unwrap().len(), 1, "and no record");
}

#[tokio::test]
async fn retry_completes_from_journal_without_pasting() {
    let mut r = rig(FakeGateway {
        open: OpenBehavior::Fail,
        ..FakeGateway::ok("x")
    });
    record(&mut r, speech(700)).await;
    r.c.stop();
    r.c.settle().await;
    let rec = r.history.inserted.lock().unwrap()[0].clone();

    // Network is back.
    let gw = FakeGateway::ok("Recovered text.");
    let received = gw.received.clone();
    r.c.gateway = Arc::new(gw);
    r.c.retry(
        rec.id.clone(),
        PathBuf::from(rec.audio_path.unwrap()),
        StartSettings::default(),
    );
    r.c.settle().await;
    assert_eq!(
        r.history.completed.lock().unwrap()[0],
        (rec.id, "Recovered text.".to_owned())
    );
    assert_eq!(
        received.lock().unwrap().len(),
        700 * 16,
        "the whole recording is resent"
    );
    assert!(r.ui.events().contains(&"notify retry-ok".to_owned()));
    assert!(!r.ui.events().iter().any(|e| e.starts_with("paste")));
}

#[tokio::test]
async fn retry_failure_marks_record() {
    let mut r = rig(FakeGateway {
        open: OpenBehavior::Fail,
        ..FakeGateway::ok("x")
    });
    r.c.retry("rec-1".into(), r.dir.join("missing.wav.enc"), StartSettings::default());
    r.c.settle().await;
    assert_eq!(
        r.history.failed.lock().unwrap()[0],
        ("rec-1".to_owned(), "AUDIO_UNREADABLE".to_owned())
    );
}

#[tokio::test]
async fn ten_minute_cap_warns_then_asks_to_stop() {
    let mut r = rig(FakeGateway::ok("long"));
    r.c.start(StartSettings::default(), 0);
    let block = vec![500i16; 16_000]; // 1 s
    let mut signals = Vec::new();
    for _ in 0..601 {
        signals.push(r.c.audio(&block).await);
    }
    assert!(r.ui.events().contains(&"notify limit-near".to_owned()));
    let first_stop = signals.iter().position(|s| *s == AudioSignal::AutoStop).unwrap();
    assert_eq!(first_stop, 599, "stop requested exactly at 10:00");
    assert_eq!(
        r.c.active.as_ref().unwrap().samples,
        600 * 16_000,
        "nothing beyond the cap is recorded"
    );
}

#[tokio::test(start_paused = true)]
async fn slow_open_buffers_audio_until_connected() {
    let gw = FakeGateway {
        open: OpenBehavior::Delay(Duration::from_millis(400)),
        ..FakeGateway::ok("ok")
    };
    let mut r = rig(gw);
    r.c.start(StartSettings::default(), 0);
    for b in speech(1000) {
        r.c.audio(&b).await;
        tokio::time::advance(Duration::from_millis(100)).await;
    }
    r.c.stop();
    r.c.settle().await;
    assert_eq!(r.gw.received.lock().unwrap().len(), 16_000);
}
