//! Dictation orchestrator (spec 4.1 lifecycle, master prompt §5.2).
//!
//! One [`Controller`] owns at most one recording at a time:
//!
//! 1. `start`: open an encrypted journal and, in the background, a gateway
//!    session; show the bar; play the start sound.
//! 2. `audio`: append to the journal, meter for the bar, run the VAD, stream to
//!    the gateway as soon as it is open (buffered until then). The 10-minute
//!    cap warns at 9:30 and asks the caller to stop at 10:00.
//! 3. `stop`: close the journal; if nothing was said, drop it silently.
//!    Otherwise finish the gateway session in a background task (bar shows
//!    "slow connection" after 3 s), then paste, or keep the result in History
//!    if the user has started another dictation meanwhile.
//! 4. Every failure (no connection, server error, timeout) becomes a failed
//!    record that keeps its audio for Retry. `cancel` (Esc) keeps the audio
//!    too; `discard` (an accidental tap) keeps nothing.

pub mod ports;
pub mod recovery;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::task::{JoinHandle, JoinSet};
use vocalis_audio::journal::{self, JournalWriter, Key};
use vocalis_audio::level::LevelMeter;
use vocalis_audio::vad::Vad;
use vocalis_audio::{FRAME_SAMPLES, TARGET_RATE};
use vocalis_core::product::{MAX_RECORDING_MS, RECORDING_WARNING_MS};

pub use ports::*;

/// Error code for recordings cut short by a lost microphone.
pub const MIC_DISCONNECTED: &str = "MIC_DISCONNECTED";
/// Error code for recordings cut short by a screen lock or sleep.
pub const SCREEN_LOCKED: &str = "SCREEN_LOCKED";

#[derive(Clone)]
pub struct Config {
    pub journal_dir: PathBuf,
    pub key: Key,
    /// Speech needed to send a recording at all (20 ms frames): 10 = 200 ms.
    pub min_speech_frames: u64,
    /// Silent recordings longer than this warn that the microphone may be muted.
    pub silent_warning_ms: u64,
    pub finish_timeout: Duration,
    pub slow_after: Duration,
}

impl Config {
    pub fn new(journal_dir: PathBuf, key: Key) -> Self {
        Self {
            journal_dir,
            key,
            min_speech_frames: 10,
            silent_warning_ms: 3_000,
            finish_timeout: Duration::from_secs(25),
            slow_after: Duration::from_secs(3),
        }
    }
}

/// What the caller should do after feeding audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSignal {
    Continue,
    /// The 10-minute cap was reached: call [`Controller::stop`].
    AutoStop,
}

enum Link {
    Opening {
        task: JoinHandle<Result<Box<dyn GatewaySession>, GatewayError>>,
        buffer: Vec<i16>,
    },
    Open(Box<dyn GatewaySession>),
    Broken(GatewayError),
}

struct Active {
    id: String,
    created_at: i64,
    settings: StartSettings,
    journal: Option<JournalWriter>,
    journal_path: PathBuf,
    link: Link,
    vad: Vad,
    meter: LevelMeter,
    frame: Vec<f32>,
    samples: u64,
    warned: bool,
}

impl Active {
    fn duration_ms(&self) -> u64 {
        self.samples * 1000 / TARGET_RATE as u64
    }
}

pub struct Controller {
    cfg: Config,
    gateway: Arc<dyn Gateway>,
    ui: Arc<dyn Ui>,
    history: Arc<dyn History>,
    active: Option<Active>,
    /// Bumped on every start, so late results know the user moved on.
    generation: Arc<AtomicU64>,
    tasks: JoinSet<()>,
}

impl Controller {
    pub fn new(cfg: Config, gateway: Arc<dyn Gateway>, ui: Arc<dyn Ui>, history: Arc<dyn History>) -> Self {
        Self {
            cfg,
            gateway,
            ui,
            history,
            active: None,
            generation: Arc::new(AtomicU64::new(0)),
            tasks: JoinSet::new(),
        }
    }

    pub fn is_recording(&self) -> bool {
        self.active.is_some()
    }

    /// Begin a recording. `now_ms` is wall-clock Unix time for the record.
    pub fn start(&mut self, settings: StartSettings, now_ms: i64) {
        if self.active.is_some() {
            return;
        }
        self.generation.fetch_add(1, Ordering::SeqCst);
        let id = uuid::Uuid::new_v4().to_string();
        let journal_path = self.cfg.journal_dir.join(format!("{id}.wav.enc"));
        let journal = match JournalWriter::create(
            &journal_path,
            &self.cfg.key,
            TARGET_RATE,
            now_ms as u64,
            TARGET_RATE as usize / 2,
        ) {
            Ok(j) => Some(j),
            Err(e) => {
                tracing::error!(target: "session", error = %e, "could not create audio journal");
                None
            }
        };
        let gateway = self.gateway.clone();
        let open_settings = settings.clone();
        let task = tokio::spawn(async move { gateway.open(&open_settings).await });
        self.active = Some(Active {
            id,
            created_at: now_ms,
            settings,
            journal,
            journal_path,
            link: Link::Opening {
                task,
                buffer: Vec::new(),
            },
            vad: Vad::default(),
            meter: LevelMeter::default(),
            frame: Vec::with_capacity(FRAME_SAMPLES),
            samples: 0,
            warned: false,
        });
        self.ui.bar(BarState::Recording);
        self.ui.sound(Sound::Start);
    }

    /// Feed 16 kHz mono samples.
    pub async fn audio(&mut self, pcm: &[i16]) -> AudioSignal {
        let Some(a) = self.active.as_mut() else {
            return AudioSignal::Continue;
        };
        let room = (MAX_RECORDING_MS * TARGET_RATE as u64 / 1000).saturating_sub(a.samples) as usize;
        let pcm = &pcm[..pcm.len().min(room)];
        if let Some(j) = a.journal.as_mut() {
            if let Err(e) = j.append(pcm) {
                tracing::error!(target: "session", error = %e, "journal write failed");
                a.journal = None;
            }
        }
        for &s in pcm {
            a.frame.push(s as f32 / i16::MAX as f32);
            if a.frame.len() == FRAME_SAMPLES {
                a.vad.push(&a.frame);
                let level = a.meter.push(&a.frame);
                a.frame.clear();
                self.ui.level(level);
            }
        }
        a.samples += pcm.len() as u64;

        // Stream to the gateway, or buffer until it is open.
        if let Link::Opening { task, buffer } = &mut a.link {
            buffer.extend_from_slice(pcm);
            if task.is_finished() {
                a.link = resolve(std::mem::replace(&mut a.link, Link::Broken(GatewayError::Connect))).await;
            }
        } else if let Link::Open(s) = &mut a.link {
            if let Err(e) = s.send_audio(pcm).await {
                a.link = Link::Broken(e);
            }
        }

        let ms = a.duration_ms();
        if ms >= RECORDING_WARNING_MS && !a.warned {
            a.warned = true;
            self.ui.bar(BarState::NearLimit);
            self.ui.notify(Notice::RecordingLimitNear);
        }
        if ms >= MAX_RECORDING_MS {
            AudioSignal::AutoStop
        } else {
            AudioSignal::Continue
        }
    }

    /// Stop and process. Returns immediately; processing runs in the background.
    pub fn stop(&mut self) {
        let Some(mut a) = self.active.take() else { return };
        self.ui.sound(Sound::Stop);
        finish_journal(&mut a);
        if !a.vad.heard_speech(self.cfg.min_speech_frames) {
            if a.duration_ms() >= self.cfg.silent_warning_ms {
                self.ui.notify(Notice::MicSilent);
            }
            self.ui.bar(BarState::Hidden);
            drop_recording(a);
            return;
        }
        self.ui.bar(BarState::Processing);
        let generation = self.generation.clone();
        let my_gen = generation.load(Ordering::SeqCst);
        let (ui, history, cfg, gateway) = (
            self.ui.clone(),
            self.history.clone(),
            self.cfg.clone(),
            self.gateway.clone(),
        );
        self.tasks.spawn(async move {
            let id = a.id.clone();
            let entry = base_entry(&a);
            let started = tokio::time::Instant::now();
            let result = match finish_link(a.link, &ui, cfg.finish_timeout, cfg.slow_after).await {
                // The connection dropped (or never opened) but the whole
                // recording is in the journal: send it once more on a fresh
                // connection before giving up, within the same overall deadline.
                Err(e @ (GatewayError::Connect | GatewayError::Disconnected)) => {
                    let left = cfg.finish_timeout.saturating_sub(started.elapsed());
                    tracing::info!(target: "session", error = %e, "connection lost; resending the recording");
                    if a.journal_path.exists() && !left.is_zero() {
                        resend(&gateway, &a.journal_path, &cfg.key, &a.settings, left).await
                    } else {
                        Err(e)
                    }
                }
                r => r,
            };
            let moved_on = generation.load(Ordering::SeqCst) != my_gen;
            if !moved_on {
                ui.bar(BarState::Hidden);
            }
            match result {
                Ok(r) => deliver(r, entry, &ui, &history, moved_on),
                Err(e) => {
                    let code = e.record_code().to_owned();
                    tracing::warn!(target: "session", error = %e, "dictation failed; audio kept for retry");
                    history.insert(NewEntry {
                        error_code: Some(code.clone()),
                        ..entry
                    });
                    if !moved_on {
                        ui.sound(Sound::Error);
                    }
                    ui.notify(Notice::TranscriptionFailed { record_id: id, code });
                }
            }
        });
    }

    /// Esc: nothing is pasted, but the audio is kept as a failed record for Retry.
    pub fn cancel(&mut self) {
        let Some(mut a) = self.active.take() else { return };
        finish_journal(&mut a);
        self.ui.bar(BarState::Hidden);
        let heard = a.vad.heard_speech(self.cfg.min_speech_frames);
        let entry = base_entry(&a);
        cancel_link(a.link);
        if heard {
            self.history.insert(NewEntry {
                error_code: Some("CANCELLED".into()),
                ..entry
            });
        } else {
            let _ = std::fs::remove_file(&a.journal_path);
        }
    }

    /// The microphone failed. Before any audio arrived it couldn't be opened;
    /// later it was unplugged or lost. Speech so far is kept as a failed
    /// `MIC_DISCONNECTED` record (spec 4.1: audio is preserved when the mic
    /// disconnects); a recording with no speech yet is dropped.
    pub fn mic_failed(&mut self, reason: &str) {
        let Some(mut a) = self.active.take() else { return };
        finish_journal(&mut a);
        self.ui.bar(BarState::Hidden);
        self.ui.sound(Sound::Error);
        if a.samples == 0 {
            drop_recording(a);
            self.ui.notify(Notice::MicUnavailable {
                reason: reason.to_owned(),
            });
            return;
        }
        let heard = a.vad.heard_speech(self.cfg.min_speech_frames);
        let entry = base_entry(&a);
        let id = a.id.clone();
        if heard {
            cancel_link(a.link);
            self.history.insert(NewEntry {
                error_code: Some(MIC_DISCONNECTED.into()),
                ..entry
            });
            self.ui.notify(Notice::MicDisconnected { record_id: Some(id) });
        } else {
            drop_recording(a);
            self.ui.notify(Notice::MicDisconnected { record_id: None });
        }
    }

    /// The screen locked or the computer slept mid-recording (spec 4.1: audio
    /// is preserved when the screen locks). Nobody is at the screen, so nothing
    /// is sent or pasted: speech so far becomes a failed `SCREEN_LOCKED`
    /// record with Retry; a recording with no speech is dropped silently.
    pub fn interrupted(&mut self) {
        let Some(mut a) = self.active.take() else { return };
        finish_journal(&mut a);
        self.ui.bar(BarState::Hidden);
        if !a.vad.heard_speech(self.cfg.min_speech_frames) {
            drop_recording(a);
            return;
        }
        let entry = base_entry(&a);
        let id = a.id.clone();
        cancel_link(a.link);
        self.history.insert(NewEntry {
            error_code: Some(SCREEN_LOCKED.into()),
            ..entry
        });
        self.ui.notify(Notice::TranscriptionFailed {
            record_id: id,
            code: SCREEN_LOCKED.into(),
        });
    }

    /// Accidental tap: drop everything, no record.
    pub fn discard(&mut self) {
        let Some(mut a) = self.active.take() else { return };
        finish_journal(&mut a);
        self.ui.bar(BarState::Hidden);
        drop_recording(a);
    }

    /// Re-transcribe a failed record from its journal. Never pastes (spec 4.6).
    pub fn retry(&mut self, record_id: String, journal_path: PathBuf, settings: StartSettings) {
        let (gateway, ui, history, cfg) = (
            self.gateway.clone(),
            self.ui.clone(),
            self.history.clone(),
            self.cfg.clone(),
        );
        self.tasks.spawn(async move {
            match retry_inner(&gateway, &journal_path, &cfg.key, &settings, cfg.finish_timeout).await {
                Ok(r) => {
                    history.complete(&record_id, &r.text, r.words, r.wpm);
                    ui.notify(Notice::RetrySucceeded { record_id });
                }
                Err(code) => {
                    history.fail(&record_id, &code);
                    ui.notify(Notice::TranscriptionFailed { record_id, code });
                }
            }
        });
    }

    /// Wait for all background processing (tests, shutdown).
    pub async fn settle(&mut self) {
        while self.tasks.join_next().await.is_some() {}
    }
}

fn base_entry(a: &Active) -> NewEntry {
    NewEntry {
        id: a.id.clone(),
        is_command: false,
        text: String::new(),
        app_name: Some(a.settings.app.name.clone()).filter(|s| !s.is_empty()),
        app_id: Some(a.settings.app.app_id.clone()).filter(|s| !s.is_empty()),
        url: a.settings.app.url.clone(),
        words: 0,
        duration_ms: a.duration_ms(),
        wpm: 0,
        audio_path: path_string(&a.journal_path),
        error_code: None,
        created_at: a.created_at,
    }
}

fn path_string(p: &Path) -> Option<String> {
    p.exists().then(|| p.to_string_lossy().into_owned())
}

fn finish_journal(a: &mut Active) {
    if let Some(j) = a.journal.take() {
        if let Err(e) = j.finish() {
            tracing::error!(target: "session", error = %e, "journal finish failed");
        }
    }
}

fn drop_recording(a: Active) {
    cancel_link(a.link);
    let _ = std::fs::remove_file(&a.journal_path);
}

fn cancel_link(link: Link) {
    tokio::spawn(async move {
        match link {
            Link::Opening { task, .. } => {
                if let Ok(Ok(s)) = task.await {
                    s.cancel().await;
                }
            }
            Link::Open(s) => s.cancel().await,
            Link::Broken(_) => {}
        }
    });
}

/// Turn a finished open-task into an open session with the buffer flushed.
async fn resolve(link: Link) -> Link {
    let Link::Opening { task, buffer } = link else {
        return link;
    };
    match task.await {
        Ok(Ok(mut s)) => match s.send_audio(&buffer).await {
            Ok(()) => Link::Open(s),
            Err(e) => Link::Broken(e),
        },
        Ok(Err(e)) => Link::Broken(e),
        Err(_) => Link::Broken(GatewayError::Connect),
    }
}

async fn finish_link(
    link: Link,
    ui: &Arc<dyn Ui>,
    timeout: Duration,
    slow_after: Duration,
) -> Result<FinalResult, GatewayError> {
    let work = async {
        let link = resolve(link).await;
        match link {
            Link::Open(s) => s.finish().await,
            Link::Broken(e) => Err(e),
            Link::Opening { .. } => unreachable!("resolved above"),
        }
    };
    tokio::pin!(work);
    let slow = tokio::time::sleep(slow_after);
    tokio::pin!(slow);
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);
    let mut warned = false;
    loop {
        tokio::select! {
            r = &mut work => return r,
            _ = &mut slow, if !warned => {
                warned = true;
                ui.bar(BarState::SlowConnection);
            }
            _ = &mut deadline => return Err(GatewayError::Timeout),
        }
    }
}

fn deliver(r: FinalResult, entry: NewEntry, ui: &Arc<dyn Ui>, history: &Arc<dyn History>, moved_on: bool) {
    let id = entry.id.clone();
    match r.plan {
        PlanKind::Empty => {
            if let Some(p) = &entry.audio_path {
                let _ = std::fs::remove_file(p);
            }
        }
        PlanKind::Command | PlanKind::OpenCommand => {
            history.insert(NewEntry {
                is_command: true,
                text: r.text.clone(),
                ..entry
            });
            let req = (r.plan == PlanKind::Command).then_some(r.text.as_str());
            ui.command(req, &id);
        }
        PlanKind::Paste => {
            history.insert(NewEntry {
                text: r.text.clone(),
                words: r.words,
                wpm: r.wpm,
                ..entry
            });
            if moved_on {
                return; // The user started something else; the text waits in History.
            }
            match ui.paste(&r.text) {
                PasteOutcome::Pasted => {}
                PasteOutcome::NowhereToPaste => ui.notify(Notice::NowhereToPaste { record_id: id }),
                PasteOutcome::Failed => ui.notify(Notice::PasteFailed { record_id: id }),
            }
        }
    }
}

/// Send a whole journal on a fresh gateway session.
async fn resend(
    gateway: &Arc<dyn Gateway>,
    path: &Path,
    key: &Key,
    settings: &StartSettings,
    timeout: Duration,
) -> Result<FinalResult, GatewayError> {
    let recovered = journal::read(path, key).map_err(|_| GatewayError::Server {
        code: "AUDIO_UNREADABLE".into(),
        retryable: false,
    })?;
    let work = async {
        let mut s = gateway.open(settings).await?;
        for chunk in recovered.samples.chunks(TARGET_RATE as usize) {
            s.send_audio(chunk).await?;
        }
        s.finish().await
    };
    tokio::time::timeout(timeout, work)
        .await
        .unwrap_or(Err(GatewayError::Timeout))
}

async fn retry_inner(
    gateway: &Arc<dyn Gateway>,
    path: &Path,
    key: &Key,
    settings: &StartSettings,
    timeout: Duration,
) -> Result<FinalResult, String> {
    resend(gateway, path, key, settings, timeout)
        .await
        .map_err(|e| match e {
            GatewayError::Server { code, .. } if code == "AUDIO_UNREADABLE" => code,
            e => e.record_code().to_owned(),
        })
}

#[cfg(test)]
mod tests;
