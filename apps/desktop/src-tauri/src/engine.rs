//! The dictation engine: hotkeys → gestures → controller, microphone →
//! resampler → controller. Runs on Tauri's async runtime plus two OS threads
//! (the key listener and the audio capture loop).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Runtime};
use tokio::sync::mpsc;
use vocalis_audio::dsp::{to_i16, Resampler};
use vocalis_audio::TARGET_RATE;
use vocalis_core::gesture::{self, Action, Binding, Recognizer};
use vocalis_input::{Chord, ChordTracker, KeyEvent};
use vocalis_session::{AudioSignal, Controller, StartSettings};

use crate::gateway_ws::{TokenSource, WsGateway};

pub struct EngineConfig {
    pub data_dir: PathBuf,
    pub gateway_url: String,
    pub token: TokenSource,
    pub primary: Chord,
    pub hands_free: Chord,
    pub gesture: gesture::Config,
    pub languages: Vec<String>,
    pub keep_on_clipboard: bool,
    /// Development and E2E: feed this 16 kHz mono WAV instead of the microphone.
    pub dev_wav: Option<PathBuf>,
}

enum Event {
    Key(KeyEvent),
    Audio(Vec<i16>),
    CaptureFailed(String),
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Start the engine. Returns immediately; work continues in the background.
pub fn start<R: Runtime>(app: AppHandle<R>, cfg: EngineConfig) -> Result<(), String> {
    let journal_dir = cfg.data_dir.join("recordings");
    std::fs::create_dir_all(&journal_dir).map_err(|e| e.to_string())?;
    let key_dir = cfg.data_dir.join("keys");
    let audio_key = crate::secrets::key("audio", &key_dir);
    let db_key = crate::secrets::key("history", &key_dir);
    let (store, report) =
        vocalis_store::Store::open(&cfg.data_dir.join("history.db"), &db_key).map_err(|e| e.to_string())?;
    if report.restored_from_backup || report.rebuilt_empty {
        tracing::warn!(target: "engine", ?report, "history database was repaired");
    }

    let ui = Arc::new(crate::adapters::TauriUi::new(app.clone(), cfg.keep_on_clipboard));
    let history = Arc::new(crate::adapters::StoreHistory {
        store: std::sync::Mutex::new(store),
    });
    let gateway = Arc::new(WsGateway::new(&cfg.gateway_url, cfg.token.clone()));
    let controller = Controller::new(
        vocalis_session::Config::new(journal_dir, audio_key),
        gateway,
        ui,
        history,
    );
    let tracker = ChordTracker::new(vec![
        (Binding::Primary, cfg.primary.clone()),
        (Binding::HandsFree, cfg.hands_free.clone()),
    ])
    .map_err(|e| e.to_string())?;

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

    let settings = StartSettings {
        languages: cfg.languages.clone(),
        ..Default::default()
    };
    tauri::async_runtime::spawn(run(
        rx,
        tx,
        controller,
        tracker,
        Recognizer::new(cfg.gesture),
        settings,
        epoch,
        cfg.dev_wav,
    ));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run(
    mut rx: mpsc::Receiver<Event>,
    tx: mpsc::Sender<Event>,
    mut controller: Controller,
    mut tracker: ChordTracker,
    mut recognizer: Recognizer,
    settings: StartSettings,
    epoch: Instant,
    dev_wav: Option<PathBuf>,
) {
    let mut capture: Option<Arc<AtomicBool>> = None;
    let mut tick = tokio::time::interval(Duration::from_millis(20));
    loop {
        let actions: Vec<Action> = tokio::select! {
            ev = rx.recv() => match ev {
                None => return,
                Some(Event::Key(k)) => tracker.handle(k).into_iter().flat_map(|i| recognizer.handle(i)).collect(),
                Some(Event::Audio(pcm)) => {
                    if controller.audio(&pcm).await == AudioSignal::AutoStop {
                        stop_capture(&mut capture);
                        controller.stop();
                        recognizer = Recognizer::new(recognizer.config());
                    }
                    continue;
                }
                Some(Event::CaptureFailed(e)) => {
                    tracing::error!(target: "engine", error = %e, "microphone unavailable");
                    stop_capture(&mut capture);
                    controller.cancel();
                    recognizer = Recognizer::new(recognizer.config());
                    continue;
                }
            },
            _ = tick.tick() => match recognizer.deadline() {
                Some(d) if epoch.elapsed().as_millis() as u64 >= d => recognizer.tick(epoch.elapsed().as_millis() as u64),
                _ => continue,
            },
        };
        for action in actions {
            match action {
                Action::Start(_) => {
                    controller.start(settings.clone(), now_ms());
                    capture = Some(start_capture(tx.clone(), dev_wav.clone()));
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
fn start_capture(tx: mpsc::Sender<Event>, dev_wav: Option<PathBuf>) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let _ = std::thread::Builder::new()
        .name("vocalis-capture".into())
        .spawn(move || {
            if let Some(path) = dev_wav {
                return feed_wav(&path, &tx, &flag);
            }
            let cap = match vocalis_audio::capture::start(None) {
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
