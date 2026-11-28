//! History commands for the main window (spec 4.6): the day-grouped list,
//! search, copy, playback, Retry and per-record delete.

use tauri::ipc::Response;
use tauri::{AppHandle, Manager, Runtime, State};
use vocalis_store::{DayGroup, Record};

use crate::engine::EngineHandle;

/// Records per page of the History list.
const PAGE: usize = 100;

fn engine<R: Runtime>(app: &AppHandle<R>) -> Result<State<'_, EngineHandle>, String> {
    app.try_state::<EngineHandle>()
        .ok_or_else(|| "History is unavailable because the dictation engine did not start.".to_owned())
}

/// Newest records grouped by local day. `before` (Unix ms) pages backwards;
/// `utc_offset_minutes` is the browser's offset, east positive.
#[tauri::command]
pub fn history_list<R: Runtime>(
    app: AppHandle<R>,
    before: Option<i64>,
    utc_offset_minutes: i32,
) -> Result<Vec<DayGroup>, String> {
    engine(&app)?
        .history
        .lock()
        .recent_by_day(PAGE, before, utc_offset_minutes)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn history_search<R: Runtime>(app: AppHandle<R>, query: String) -> Result<Vec<Record>, String> {
    engine(&app)?.history.lock().search(&query).map_err(|e| e.to_string())
}

/// Re-transcribe a failed record. The result arrives as a `notice` event and a
/// `history-changed` event; it is never pasted.
#[tauri::command]
pub fn history_retry<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    engine(&app)?.retry(id)
}

#[tauri::command]
pub fn history_delete<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let engine = engine(&app)?;
    let audio = engine.history.lock().delete(&id).map_err(|e| e.to_string())?;
    if let Some(p) = audio {
        let _ = std::fs::remove_file(p);
    }
    engine.history.changed();
    Ok(())
}

/// The record's audio, decrypted to a WAV file in memory for the player. It is
/// never written to disk unencrypted.
#[tauri::command]
pub fn history_audio<R: Runtime>(app: AppHandle<R>, id: String) -> Result<Response, String> {
    let engine = engine(&app)?;
    let path = engine
        .history
        .lock()
        .get(&id)
        .map_err(|e| e.to_string())?
        .audio_path
        .ok_or("This recording's audio is no longer kept.")?;
    let rec = vocalis_audio::journal::read(std::path::Path::new(&path), &engine.audio_key)
        .map_err(|_| "The audio file could not be read.".to_owned())?;
    Ok(Response::new(vocalis_audio::wav::encode(&rec.samples, rec.sample_rate)))
}

/// Copy a record's text (History row "Copy", notification "Copy").
#[tauri::command]
pub fn history_copy<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let engine = engine(&app)?;
    let text = engine.history.lock().get(&id).map_err(|e| e.to_string())?.text;
    if text.is_empty() {
        return Err("This record has no text to copy yet.".into());
    }
    (engine.copier)(&text)
}
