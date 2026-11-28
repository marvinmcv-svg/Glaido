//! The dictation engine: hotkeys → gestures → controller, microphone →
//! resampler → controller. Runs on Tauri's async runtime plus two OS threads
//! (the key listener and the audio capture loop).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::mpsc;
use vocalis_audio::dsp::{to_i16, Resampler};
use vocalis_audio::TARGET_RATE;
use vocalis_core::gesture::{self, Action, Binding, Recognizer};
use vocalis_input::{Chord, ChordRecorder, ChordTracker, KeyEvent, Recorded};
use vocalis_session::{AudioSignal, Controller, History, StartSettings};
use vocalis_store::RecordStatus;

use crate::adapters::StoreHistory;
use crate::settings::Settings;

pub type SharedSettings = Arc<std::sync::RwLock<Settings>>;

use vocalis_client::{TokenSource, WsGateway};

pub struct EngineConfig {
    pub data_dir: PathBuf,
    pub gateway_url: String,
    pub token: TokenSource,
    /// Read at every start (languages, microphone) and on `Event::Hotkeys`.
    pub settings: SharedSettings,
    pub gesture: gesture::Config,
    pub keep_on_clipboard: bool,
    /// Development and E2E: feed this 16 kHz mono WAV instead of the microphone.
    pub dev_wav: Option<PathBuf>,
    /// Development and E2E: while this file exists, the screen counts as locked.
    pub dev_lock_file: Option<PathBuf>,
}

pub(crate) enum Event {
    Key(KeyEvent),
    Audio(Vec<i16>),
    CaptureFailed(String),
    /// Re-transcribe a failed record from its saved audio.
    Retry(String),
    /// The hotkeys in settings changed: rebuild the chord tracker.
    Hotkeys,
    /// Hotkey picker: report the next combination pressed (`None` = cancelled).
    Record(RecordReply),
}

/// Puts text on the clipboard.
pub type Copier = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

/// Reads the clipboard's plain text (the `{clipboard}` tag).
pub type ClipboardReader = Arc<dyn Fn() -> Option<String> + Send + Sync>;

/// Where the hotkey picker's result goes (`None` = cancelled).
pub(crate) type RecordReply = tokio::sync::oneshot::Sender<Option<Result<Chord, String>>>;

/// What the UI commands need from a running engine (Tauri managed state).
pub struct EngineHandle {
    pub history: Arc<StoreHistory>,
    pub audio_key: [u8; 32],
    pub(crate) tx: mpsc::Sender<Event>,
    pub copier: Copier,
}

impl EngineHandle {
    pub fn retry(&self, id: String) -> Result<(), String> {
        self.send(Event::Retry(id))
    }

    pub fn hotkeys_changed(&self) -> Result<(), String> {
        self.send(Event::Hotkeys)
    }

    /// Wait for the user to press and release a combination.
    pub async fn record_chord(&self) -> Result<Option<Chord>, String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.send(Event::Record(tx))?;
        match rx.await {
            Ok(Some(r)) => r.map(Some),
            Ok(None) | Err(_) => Ok(None),
        }
    }

    fn send(&self, ev: Event) -> Result<(), String> {
        self.tx
            .try_send(ev)
            .map_err(|_| "the dictation engine is busy".to_owned())
    }
}

fn tracker_for(settings: &SharedSettings) -> Result<ChordTracker, String> {
    let (primary, hands_free) = settings.read().expect("settings lock").hotkeys().chords()?;
    ChordTracker::new(vec![(Binding::Primary, primary), (Binding::HandsFree, hands_free)]).map_err(|e| e.to_string())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Start the engine. Returns immediately; work continues in the background.
pub fn start<R: Runtime>(app: AppHandle<R>, cfg: EngineConfig) -> Result<EngineHandle, String> {
    let journal_dir = cfg.data_dir.join("recordings");
    std::fs::create_dir_all(&journal_dir).map_err(|e| e.to_string())?;
    let key_dir = cfg.data_dir.join("keys");
    // A keychain that is still unlocking at login gets a few seconds.
    let load = |name: &str| {
        let mut last = None;
        for attempt in 0..5 {
            match crate::secrets::key(name, &key_dir) {
                Ok(k) => return Ok(k),
                Err(e) => last = Some(e),
            }
            std::thread::sleep(Duration::from_millis(500 * (attempt + 1)));
        }
        Err(last.expect("at least one attempt").to_string())
    };
    let audio_key = load("audio")?;
    let db_key = load("history")?;
    let (store, report) =
        vocalis_store::Store::open(&cfg.data_dir.join("history.db"), &db_key).map_err(|e| e.to_string())?;
    if report.restored_from_backup || report.rebuilt_empty {
        tracing::warn!(target: "engine", ?report, "history database was repaired");
    }

    let context = Arc::new(crate::adapters::AppTracker::new(vocalis_context::os::desktop()));
    let ui = Arc::new(crate::adapters::TauriUi::new(
        app.clone(),
        cfg.keep_on_clipboard,
        context.clone(),
        cfg.settings.clone(),
    ));
    let notify_app = app.clone();
    let history = Arc::new(StoreHistory::new(store, move || {
        let _ = notify_app.emit_to("main", "history-changed", ());
    }));

    // Audio left behind by a crash or forced quit becomes failed records with Retry.
    let found = vocalis_session::recovery::scan(&journal_dir, &audio_key, |id| {
        history.lock().contains(id).unwrap_or(true)
    });
    if !found.recovered.is_empty() || found.unreadable > 0 {
        tracing::warn!(
            target: "engine",
            recovered = found.recovered.len(),
            unreadable = found.unreadable,
            "recovered interrupted recordings"
        );
    }
    for entry in found.recovered {
        history.insert(entry);
    }

    let gateway = Arc::new(WsGateway::new(&cfg.gateway_url, cfg.token.clone()));
    let controller = Controller::new(
        vocalis_session::Config::new(journal_dir, audio_key),
        gateway,
        ui.clone(),
        history.clone(),
    );
    let tracker = tracker_for(&cfg.settings)?;

    let (tx, rx) = mpsc::channel::<Event>(256);
    let epoch = Instant::now();

    // Key listener thread → async channel.
    let keys = vocalis_input::listen::start(epoch);
    let key_tx = tx.clone();
    std::thread::Builder::new()
        .name("vocalis-key-bridge".into())
        .spawn(move || {
            while let Ok(ev) = keys.recv() {
                if key_tx.blocking_send(Event::Key(ev)).is_err() {
                    break;
                }
            }
        })
        .map_err(|e| e.to_string())?;

    let reader_ui = ui.clone();
    let read_clipboard: ClipboardReader = Arc::new(move || reader_ui.read_text());
    let handle = EngineHandle {
        history: history.clone(),
        audio_key,
        tx: tx.clone(),
        copier: Arc::new(move |text: &str| ui.copy(text)),
    };
    tauri::async_runtime::spawn(run(
        rx,
        tx,
        history,
        controller,
        tracker,
        Recognizer::new(cfg.gesture),
        cfg.settings,
        context,
        epoch,
        cfg.dev_wav,
        cfg.dev_lock_file,
        read_clipboard,
    ));
    tracing::info!(target: "engine", "dictation engine ready");
    Ok(handle)
}

#[allow(clippy::too_many_arguments)]
async fn run(
    mut rx: mpsc::Receiver<Event>,
    tx: mpsc::Sender<Event>,
    history: Arc<StoreHistory>,
    mut controller: Controller,
    mut tracker: ChordTracker,
    mut recognizer: Recognizer,
    settings: SharedSettings,
    context: Arc<crate::adapters::AppTracker>,
    epoch: Instant,
    dev_wav: Option<PathBuf>,
    dev_lock_file: Option<PathBuf>,
    read_clipboard: ClipboardReader,
) {
    // Dictionary, snippets and tag values (spec 4.5.1, 4.5.2) are read fresh
    // for every start and retry; the clipboard only when a tag uses it.
    let start_settings = || {
        let personal = tokio::task::block_in_place(|| crate::personal_cmds::load(&history.lock(), || read_clipboard()));
        StartSettings {
            languages: settings.read().expect("settings lock").languages.clone(),
            dictionary: personal.dictionary,
            snippets: personal.snippets,
            tags: personal.tags,
            ..Default::default()
        }
    };
    let microphone = || settings.read().expect("settings lock").microphone.clone();
    let mut capture: Option<Arc<AtomicBool>> = None;
    let mut recording: Option<(ChordRecorder, RecordReply)> = None;
    let mut tick = tokio::time::interval(Duration::from_millis(20));
    // Lock/sleep checks: every 500 ms while recording (spec 4.1).
    let mut last_check = (Instant::now(), SystemTime::now());
    // While locked, keys (including auto-repeat of keys held at lock time) are
    // ignored; the tracker is cleared again once the session unlocks.
    let mut locked = false;
    let locked_now = |context: &crate::adapters::AppTracker| {
        dev_lock_file.as_ref().is_some_and(|p| p.exists())
            || tokio::task::block_in_place(|| context.desktop.screen_locked()) == Some(true)
    };
    loop {
        let actions: Vec<Action> = tokio::select! {
            ev = rx.recv() => match ev {
                None => return,
                Some(Event::Key(k)) => {
                    if locked {
                        continue;
                    }
                    if let Some((rec, _)) = recording.as_mut() {
                        let outcome = match rec.handle(k) {
                            Recorded::Pending => continue,
                            Recorded::Cancelled => None,
                            Recorded::Done(r) => Some(r.map_err(|e| e.to_string())),
                        };
                        if let Some((_, reply)) = recording.take() {
                            let _ = reply.send(outcome);
                        }
                        // The keys used to record must not also start a dictation.
                        let _ = tracker.reset(0);
                        continue;
                    }
                    tracker.handle(k).into_iter().flat_map(|i| recognizer.handle(i)).collect()
                }
                Some(Event::Hotkeys) => {
                    match tracker_for(&settings) {
                        Ok(t) => {
                            tracker = t;
                            recognizer = Recognizer::new(recognizer.config());
                        }
                        Err(e) => tracing::warn!(target: "engine", error = %e, "hotkeys not applied"),
                    }
                    continue;
                }
                Some(Event::Record(reply)) => {
                    if controller.is_recording() {
                        let _ = reply.send(Some(Err("Finish the current dictation first.".into())));
                    } else {
                        recording = Some((ChordRecorder::new(), reply));
                    }
                    continue;
                }
                Some(Event::Audio(pcm)) => {
                    if controller.audio(&pcm).await == AudioSignal::AutoStop {
                        stop_capture(&mut capture);
                        controller.stop();
                        recognizer = Recognizer::new(recognizer.config());
                    }
                    continue;
                }
                Some(Event::Retry(id)) => {
                    let record = history.lock().get(&id);
                    match record {
                        Ok(r) if r.status == RecordStatus::Failed => match r.audio_path {
                            Some(p) => controller.retry(id, PathBuf::from(p), start_settings()),
                            None => tracing::warn!(target: "engine", "retry skipped: the record has no audio"),
                        },
                        Ok(_) => tracing::info!(target: "engine", "retry skipped: the record did not fail"),
                        Err(e) => tracing::warn!(target: "engine", error = %e, "retry skipped"),
                    }
                    continue;
                }
                Some(Event::CaptureFailed(e)) => {
                    tracing::error!(target: "engine", error = %e, "microphone unavailable");
                    stop_capture(&mut capture);
                    controller.mic_failed(&e);
                    recognizer = Recognizer::new(recognizer.config());
                    continue;
                }
            },
            _ = tick.tick() => {
                if last_check.0.elapsed() >= Duration::from_millis(500) {
                    let mono = last_check.0.elapsed().as_millis() as u64;
                    let wall = last_check.1.elapsed().map(|d| d.as_millis() as i64).unwrap_or(0);
                    last_check = (Instant::now(), SystemTime::now());
                    let slept = vocalis_context::slept(wall, mono);
                    let check = locked || slept || controller.is_recording();
                    let now_locked = check && locked_now(&context);
                    if slept || now_locked != locked {
                        // Keys released while locked never reach us: forget them.
                        let _ = tracker.reset(epoch.elapsed().as_millis() as u64);
                        recognizer = Recognizer::new(recognizer.config());
                    }
                    if locked && !now_locked {
                        tracing::info!(target: "engine", "session unlocked");
                    }
                    locked = now_locked;
                    if (slept || now_locked) && controller.is_recording() {
                        tracing::warn!(target: "engine", slept, "recording interrupted by a screen lock or sleep");
                        stop_capture(&mut capture);
                        controller.interrupted();
                        continue;
                    }
                }
                match recognizer.deadline() {
                    Some(d) if epoch.elapsed().as_millis() as u64 >= d => recognizer.tick(epoch.elapsed().as_millis() as u64),
                    _ => continue,
                }
            },
        };
        for action in actions {
            match action {
                Action::Start(_) => {
                    // Which app the words are for: history, paste target, field kind.
                    let front = context.capture();
                    let start = StartSettings {
                        app: vocalis_context::app_context(front.as_ref()),
                        ..start_settings()
                    };
                    controller.start(start, now_ms());
                    capture = Some(start_capture(tx.clone(), dev_wav.clone(), microphone()));
                }
                Action::Promote(_) => {}
                Action::Stop => {
                    stop_capture(&mut capture);
                    controller.stop();
                }
                Action::Cancel => {
                    stop_capture(&mut capture);
                    controller.cancel();
                }
                Action::Discard => {
                    stop_capture(&mut capture);
                    controller.discard();
                }
            }
        }
    }
}

fn stop_capture(capture: &mut Option<Arc<AtomicBool>>) {
    if let Some(flag) = capture.take() {
        flag.store(true, Ordering::SeqCst);
    }
}

/// Capture on a dedicated thread until the returned flag is set. The cpal
/// stream is created and dropped on that thread (it is not `Send` everywhere).
fn start_capture(tx: mpsc::Sender<Event>, dev_wav: Option<PathBuf>, device: Option<String>) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let _ = std::thread::Builder::new()
        .name("vocalis-capture".into())
        .spawn(move || {
            if let Some(path) = dev_wav {
                return feed_wav(&path, &tx, &flag);
            }
            // A saved microphone that is unplugged falls back to the default.
            let opened = vocalis_audio::capture::start(device.as_deref()).or_else(|e| {
                if device.is_some() {
                    vocalis_audio::capture::start(None)
                } else {
                    Err(e)
                }
            });
            let cap = match opened {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.blocking_send(Event::CaptureFailed(e.to_string()));
                    return;
                }
            };
            let mut resampler = Resampler::new(cap.sample_rate, TARGET_RATE);
            while !flag.load(Ordering::SeqCst) {
                match cap.blocks.recv_timeout(Duration::from_millis(30)) {
                    Ok(block) => {
                        let pcm = to_i16(&resampler.process(&block));
                        if !pcm.is_empty() && tx.blocking_send(Event::Audio(pcm)).is_err() {
                            return;
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(_) => {
                        let _ = tx.blocking_send(Event::CaptureFailed("microphone disconnected".into()));
                        return;
                    }
                }
                if let Ok(e) = cap.errors.try_recv() {
                    let _ = tx.blocking_send(Event::CaptureFailed(e));
                    return;
                }
            }
        });
    stop
}

/// Replays a WAV at real-time pace, then silence, until stopped.
fn feed_wav(path: &std::path::Path, tx: &mpsc::Sender<Event>, stop: &AtomicBool) {
    let samples = std::fs::read(path)
        .ok()
        .and_then(|b| vocalis_audio::wav::decode(&b).ok())
        .filter(|(_, rate)| *rate == TARGET_RATE)
        .map(|(s, _)| s);
    let Some(samples) = samples else {
        let _ = tx.blocking_send(Event::CaptureFailed("dev WAV must be 16 kHz mono PCM".into()));
        return;
    };
    let mut chunks = samples.chunks(1600);
    while !stop.load(Ordering::SeqCst) {
        let block = chunks.next().map(<[i16]>::to_vec).unwrap_or_else(|| vec![0; 1600]);
        if tx.blocking_send(Event::Audio(block)).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
