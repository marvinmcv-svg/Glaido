//! Crash recovery (spec 4.1 "Audio is preserved if the app crashes").
//!
//! Every journal the controller keeps ends up referenced by a history record,
//! and every journal it does not keep is deleted. So at startup, before any new
//! recording begins, a `.wav.enc` file with no record is the audio of a
//! recording the app never finished: a crash, a forced quit or a power loss.
//! Each one becomes a failed record that keeps its audio for Retry.

use std::path::Path;

use vocalis_audio::journal::{self, Key};

use crate::NewEntry;

/// Error code stored on records rebuilt from an orphaned journal.
pub const INTERRUPTED: &str = "INTERRUPTED";

/// What a startup scan found.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RecoveryReport {
    /// Failed records to insert, one per recoverable journal.
    pub recovered: Vec<NewEntry>,
    /// Journals removed because they held no audio at all.
    pub removed_empty: usize,
    /// Journals that could not be read (wrong key, damaged header). They are
    /// left on disk untouched so nothing is ever lost silently.
    pub unreadable: usize,
}

/// Scan `dir` for journals whose id (`<id>.wav.enc`) `is_known` does not
/// recognise. Call once at startup, before the controller records anything.
pub fn scan(dir: &Path, key: &Key, is_known: impl Fn(&str) -> bool) -> RecoveryReport {
    let mut report = RecoveryReport::default();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return report;
    };
    let mut paths: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        let Some(id) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".wav.enc"))
            .map(str::to_owned)
        else {
            continue;
        };
        if is_known(&id) {
            continue;
        }
        let empty_file = std::fs::metadata(&path).map(|m| m.len() == 0).unwrap_or(false);
        match journal::read(&path, key) {
            Ok(r) if r.samples.is_empty() => {
                let _ = std::fs::remove_file(&path);
                report.removed_empty += 1;
            }
            Ok(r) => {
                let duration_ms = r.samples.len() as u64 * 1000 / u64::from(r.sample_rate.max(1));
                report.recovered.push(NewEntry {
                    id,
                    is_command: false,
                    text: String::new(),
                    app_name: None,
                    app_id: None,
                    url: None,
                    words: 0,
                    duration_ms,
                    wpm: 0,
                    audio_path: Some(path.to_string_lossy().into_owned()),
                    error_code: Some(INTERRUPTED.into()),
                    created_at: r.created_ms as i64,
                });
            }
            // A crash before the header was written leaves a zero-byte file.
            Err(_) if empty_file => {
                let _ = std::fs::remove_file(&path);
                report.removed_empty += 1;
            }
            Err(e) => {
                tracing::warn!(target: "recovery", error = %e, "orphaned journal is unreadable; left in place");
                report.unreadable += 1;
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use vocalis_audio::journal::JournalWriter;
    use vocalis_audio::TARGET_RATE;

    const KEY: Key = [7u8; 32];

    fn write(dir: &Path, id: &str, samples: usize, finish: bool) {
        let mut j = JournalWriter::create(&dir.join(format!("{id}.wav.enc")), &KEY, TARGET_RATE, 1_000, 1600).unwrap();
        if samples > 0 {
            j.append(&vec![1000i16; samples]).unwrap();
        }
        if finish {
            j.finish().unwrap();
        } else {
            // Simulate a crash: the writer is dropped without the final chunk.
            std::mem::forget(j);
        }
    }

    #[test]
    fn orphans_become_failed_records_and_known_ones_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "known", 16_000, true);
        write(dir.path(), "crashed", 32_000, false);
        std::fs::write(dir.path().join("notes.txt"), b"ignore me").unwrap();

        let report = scan(dir.path(), &KEY, |id| id == "known");
        assert_eq!(report.recovered.len(), 1);
        let e = &report.recovered[0];
        assert_eq!(e.id, "crashed");
        assert_eq!(e.error_code.as_deref(), Some(INTERRUPTED));
        assert_eq!(e.created_at, 1_000);
        assert!(e.duration_ms >= 1_900, "most of the 2 s survived: {}", e.duration_ms);
        assert!(Path::new(e.audio_path.as_deref().unwrap()).exists());
        assert_eq!(report.unreadable, 0);
    }

    #[test]
    fn empty_journals_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "nothing", 0, true);
        std::fs::write(dir.path().join("zero.wav.enc"), b"").unwrap();
        let report = scan(dir.path(), &KEY, |_| false);
        assert!(report.recovered.is_empty());
        assert_eq!(report.removed_empty, 2);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn unreadable_journals_are_never_deleted() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "other-key", 16_000, true);
        let report = scan(dir.path(), &[9u8; 32], |_| false);
        assert!(report.recovered.is_empty());
        assert_eq!(report.unreadable, 1);
        assert!(dir.path().join("other-key.wav.enc").exists());
    }

    #[test]
    fn a_missing_directory_is_not_an_error() {
        let report = scan(Path::new("/definitely/not/here"), &KEY, |_| false);
        assert_eq!(report, RecoveryReport::default());
    }
}
