//! Vocalis local history (spec 4.6, master prompt §5.2 "Local store" and §5.5).
//!
//! * Encrypted at rest with SQLCipher. The 32-byte key comes from the OS keychain.
//! * Transcripts and command records live only here, never on a server.
//! * Search: at least 2 characters, matches anywhere in the text (FTS5 trigram
//!   index, or `LIKE` for 2-character queries), newest first, at most 50 results,
//!   failed records excluded.
//! * Audio files (`.wav.enc`) are kept for the most recent 100 recordings; older
//!   records keep their text only.
//! * On open: `PRAGMA integrity_check`. A damaged database is restored from the
//!   newest healthy rotating backup, or rebuilt empty, and the caller is told
//!   so it can notify the user.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Records that keep their audio file.
pub const AUDIO_RETENTION: usize = 100;
/// Maximum search results.
pub const SEARCH_LIMIT: usize = 50;
/// Rotating backups kept next to the database.
pub const BACKUPS_KEPT: usize = 3;

pub type Key = [u8; 32];

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("record not found")]
    NotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordType {
    Dictation,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordStatus {
    Ok,
    Failed,
    Running,
}

impl RecordType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Dictation => "dictation",
            Self::Command => "command",
        }
    }
}

impl RecordStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failed => "failed",
            Self::Running => "running",
        }
    }
}

/// A history record. Times are Unix milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub id: String,
    pub kind: RecordType,
    pub status: RecordStatus,
    pub text: String,
    pub app_name: Option<String>,
    pub app_id: Option<String>,
    pub url: Option<String>,
    pub field_kind: Option<String>,
    pub words: u32,
    pub duration_ms: u64,
    pub wpm: u32,
    pub audio_path: Option<String>,
    pub error_code: Option<String>,
    pub created_at: i64,
}

/// Fields for a new record; id and defaults are filled in by the store.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewRecord {
    pub kind: Option<RecordType>,
    pub status: Option<RecordStatus>,
    pub text: String,
    pub app_name: Option<String>,
    pub app_id: Option<String>,
    pub url: Option<String>,
    pub field_kind: Option<String>,
    pub words: u32,
    pub duration_ms: u64,
    pub wpm: u32,
    pub audio_path: Option<String>,
    pub error_code: Option<String>,
    pub created_at: i64,
}

/// What happened when the database was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OpenReport {
    pub restored_from_backup: bool,
    pub rebuilt_empty: bool,
}

/// Records for one local calendar day, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayGroup {
    /// Local date as `YYYY-MM-DD`.
    pub date: String,
    pub records: Vec<Record>,
}

const MIGRATIONS: &[&str] = &[
    // v1
    "
    CREATE TABLE records (
      id TEXT PRIMARY KEY,
      kind TEXT NOT NULL CHECK (kind IN ('dictation','command')),
      status TEXT NOT NULL CHECK (status IN ('ok','failed','running')),
      text TEXT NOT NULL DEFAULT '',
      app_name TEXT, app_id TEXT, url TEXT, field_kind TEXT,
      words INTEGER NOT NULL DEFAULT 0,
      duration_ms INTEGER NOT NULL DEFAULT 0,
      wpm INTEGER NOT NULL DEFAULT 0,
      audio_path TEXT,
      error_code TEXT,
      created_at INTEGER NOT NULL
    );
    CREATE INDEX records_created ON records (created_at DESC);
    CREATE VIRTUAL TABLE records_fts USING fts5(text, content='records', content_rowid='rowid', tokenize='trigram');
    CREATE TRIGGER records_ai AFTER INSERT ON records BEGIN
      INSERT INTO records_fts(rowid, text) VALUES (new.rowid, new.text);
    END;
    CREATE TRIGGER records_ad AFTER DELETE ON records BEGIN
      INSERT INTO records_fts(records_fts, rowid, text) VALUES ('delete', old.rowid, old.text);
    END;
    CREATE TRIGGER records_au AFTER UPDATE OF text ON records BEGIN
      INSERT INTO records_fts(records_fts, rowid, text) VALUES ('delete', old.rowid, old.text);
      INSERT INTO records_fts(rowid, text) VALUES (new.rowid, new.text);
    END;
    ",
];

pub struct Store {
    conn: Connection,
    path: PathBuf,
    key: Key,
}

fn key_pragma(key: &Key) -> String {
    let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
    format!("PRAGMA key = \"x'{hex}'\";")
}

fn open_conn(path: &Path, key: &Key) -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE)?;
    conn.execute_batch(&key_pragma(key))?;
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 2000;")?;
    Ok(conn)
}

/// `true` when the file opens with this key and passes the integrity check.
fn healthy(conn: &Connection) -> bool {
    conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
        .is_ok_and(|s| s == "ok")
}

fn backup_path(path: &Path, n: usize) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(format!(".bak{n}"));
    PathBuf::from(s)
}

/// Move a damaged database (and its WAL/SHM files) aside instead of deleting it.
fn quarantine(path: &Path) -> std::io::Result<()> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    for suffix in ["", "-wal", "-shm"] {
        let mut from = path.as_os_str().to_owned();
        from.push(suffix);
        let mut to = path.as_os_str().to_owned();
        to.push(format!(".damaged-{stamp}{suffix}"));
        match std::fs::rename(PathBuf::from(from), PathBuf::from(to)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

impl Store {
    /// Open (or create) the encrypted history database, checking and repairing it.
    ///
    /// A file that cannot be opened with this key or fails the integrity check is
    /// moved aside as `<name>.damaged-<unix_ms>` (never deleted), then replaced by
    /// the newest healthy backup or a fresh empty database.
    pub fn open(path: &Path, key: &Key) -> Result<(Self, OpenReport), StoreError> {
        let mut report = OpenReport::default();
        let existing = path.exists();
        let conn = match open_conn(path, key) {
            Ok(c) if !existing || healthy(&c) => c,
            Err(e) if !existing => return Err(e.into()),
            // Existing file that won't open with this key, or fails the check.
            _ => {
                let restored = (0..BACKUPS_KEPT)
                    .map(|n| backup_path(path, n))
                    .find(|b| b.exists() && open_conn(b, key).map(|c| healthy(&c)).unwrap_or(false));
                quarantine(path)?;
                match restored {
                    Some(b) => {
                        std::fs::copy(&b, path)?;
                        report.restored_from_backup = true;
                    }
                    None => report.rebuilt_empty = true,
                }
                open_conn(path, key)?
            }
        };
        let store = Self {
            conn,
            path: path.to_owned(),
            key: *key,
        };
        store.migrate()?;
        Ok((store, report))
    }

    fn migrate(&self) -> Result<(), StoreError> {
        let version: usize = self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version) {
            let tx = self.conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
            tx.execute_batch(&format!("PRAGMA user_version = {}", i + 1))?;
            tx.commit()?;
        }
        Ok(())
    }

    /// Write a rotating encrypted backup (`.bak0` newest). Call about once a day.
    pub fn backup(&self) -> Result<(), StoreError> {
        for n in (0..BACKUPS_KEPT - 1).rev() {
            let from = backup_path(&self.path, n);
            if from.exists() {
                std::fs::rename(&from, backup_path(&self.path, n + 1))?;
            }
        }
        let target = backup_path(&self.path, 0);
        let _ = std::fs::remove_file(&target);
        let mut dst = open_conn(&target, &self.key)?;
        let b = rusqlite::backup::Backup::new(&self.conn, &mut dst)?;
        b.run_to_completion(256, std::time::Duration::ZERO, None)?;
        Ok(())
    }

    pub fn insert(&self, r: NewRecord) -> Result<Record, StoreError> {
        self.insert_with_id(&uuid::Uuid::new_v4().to_string(), r)
    }

    /// Insert with a caller-chosen id (the dictation session's id, which also
    /// names its audio file and appears in notifications).
    pub fn insert_with_id(&self, id: &str, r: NewRecord) -> Result<Record, StoreError> {
        self.conn.execute(
            "INSERT INTO records (id, kind, status, text, app_name, app_id, url, field_kind, words, duration_ms, wpm, audio_path, error_code, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                id,
                r.kind.unwrap_or(RecordType::Dictation).as_str(),
                r.status.unwrap_or(RecordStatus::Ok).as_str(),
                r.text,
                r.app_name,
                r.app_id,
                r.url,
                r.field_kind,
                r.words,
                r.duration_ms as i64,
                r.wpm,
                r.audio_path,
                r.error_code,
                r.created_at,
            ],
        )?;
        self.get(id)
    }

    pub fn get(&self, id: &str) -> Result<Record, StoreError> {
        self.conn
            .query_row(&format!("SELECT {COLUMNS} FROM records WHERE id = ?1"), [id], from_row)
            .optional()?
            .ok_or(StoreError::NotFound)
    }

    /// Whether a record with this id exists (crash recovery uses it to spot
    /// orphaned journals).
    pub fn contains(&self, id: &str) -> Result<bool, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT 1 FROM records WHERE id = ?1", [id], |_| Ok(()))
            .optional()?
            .is_some())
    }

    /// Delete one record (the reference can't; spec 4.6 BETTER). Returns its
    /// audio path, if any, for the caller to delete.
    pub fn delete(&self, id: &str) -> Result<Option<String>, StoreError> {
        let audio = self.get(id)?.audio_path;
        self.conn.execute("DELETE FROM records WHERE id = ?1", [id])?;
        Ok(audio)
    }

    /// A retry or a late result succeeded: store the text and clear the error.
    pub fn complete(&self, id: &str, text: &str, words: u32, wpm: u32) -> Result<Record, StoreError> {
        let n = self.conn.execute(
            "UPDATE records SET status = 'ok', text = ?2, words = ?3, wpm = ?4, error_code = NULL WHERE id = ?1",
            params![id, text, words, wpm],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        self.get(id)
    }

    /// Mark a record failed (the audio stays, so it can be retried).
    pub fn fail(&self, id: &str, error_code: &str) -> Result<Record, StoreError> {
        let n = self.conn.execute(
            "UPDATE records SET status = 'failed', error_code = ?2 WHERE id = ?1",
            params![id, error_code],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        self.get(id)
    }

    /// Newest records, grouped by local day. `utc_offset_minutes` is the user's
    /// current offset (e.g. +60 for CET). `before` pages backwards.
    pub fn recent_by_day(
        &self,
        limit: usize,
        before: Option<i64>,
        utc_offset_minutes: i32,
    ) -> Result<Vec<DayGroup>, StoreError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM records WHERE created_at < ?1 ORDER BY created_at DESC LIMIT ?2"
        ))?;
        let rows = stmt.query_map(params![before.unwrap_or(i64::MAX), limit as i64], from_row)?;
        let mut groups: Vec<DayGroup> = Vec::new();
        for r in rows {
            let r = r?;
            let date = local_date(r.created_at, utc_offset_minutes);
            match groups.last_mut() {
                Some(g) if g.date == date => g.records.push(r),
                _ => groups.push(DayGroup { date, records: vec![r] }),
            }
        }
        Ok(groups)
    }

    /// Search transcripts. Queries shorter than 2 characters return nothing.
    pub fn search(&self, query: &str) -> Result<Vec<Record>, StoreError> {
        let q = query.trim();
        let len = q.chars().count();
        if len < 2 {
            return Ok(Vec::new());
        }
        let rows = if len >= 3 {
            // Trigram FTS: a quoted phrase matches as a substring anywhere.
            let phrase = format!("\"{}\"", q.replace('"', "\"\""));
            let mut stmt = self.conn.prepare(&format!(
                "SELECT {COLUMNS_R} FROM records_fts f JOIN records r ON r.rowid = f.rowid
                 WHERE records_fts MATCH ?1 AND r.status = 'ok' ORDER BY r.created_at DESC LIMIT ?2"
            ))?;
            let rows = stmt.query_map(params![phrase, SEARCH_LIMIT as i64], from_row)?;
            rows.collect::<Result<Vec<_>, _>>()?
        } else {
            let pattern = format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
            let mut stmt = self.conn.prepare(&format!(
                "SELECT {COLUMNS} FROM records WHERE status = 'ok' AND text LIKE ?1 ESCAPE '\\'
                 ORDER BY created_at DESC LIMIT ?2"
            ))?;
            let rows = stmt.query_map(params![pattern, SEARCH_LIMIT as i64], from_row)?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    /// Keep audio for the newest `AUDIO_RETENTION` recordings. Failed records always
    /// keep theirs, since it is what Retry needs. Returns the paths whose files the
    /// caller must delete; those records keep their text.
    pub fn enforce_audio_retention(&self) -> Result<Vec<String>, StoreError> {
        const EXPIRED: &str = "audio_path IS NOT NULL AND status != 'failed' AND id NOT IN
            (SELECT id FROM records WHERE audio_path IS NOT NULL ORDER BY created_at DESC LIMIT ?1)";
        let tx = self.conn.unchecked_transaction()?;
        let paths: Vec<String> = {
            let mut stmt = tx.prepare(&format!("SELECT audio_path FROM records WHERE {EXPIRED}"))?;
            let rows = stmt.query_map([AUDIO_RETENTION as i64], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        tx.execute(
            &format!("UPDATE records SET audio_path = NULL WHERE {EXPIRED}"),
            [AUDIO_RETENTION as i64],
        )?;
        tx.commit()?;
        Ok(paths)
    }

    /// Delete everything. Returns audio paths for the caller to delete.
    pub fn clear_all(&self) -> Result<Vec<String>, StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        let paths: Vec<String> = {
            let mut stmt = tx.prepare("SELECT audio_path FROM records WHERE audio_path IS NOT NULL")?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        tx.execute("DELETE FROM records", [])?;
        tx.commit()?;
        self.conn
            .execute_batch("INSERT INTO records_fts(records_fts) VALUES ('optimize'); VACUUM;")?;
        Ok(paths)
    }
}

const COLUMNS: &str = "id, kind, status, text, app_name, app_id, url, field_kind, words, duration_ms, wpm, audio_path, error_code, created_at";
const COLUMNS_R: &str = "r.id, r.kind, r.status, r.text, r.app_name, r.app_id, r.url, r.field_kind, r.words, r.duration_ms, r.wpm, r.audio_path, r.error_code, r.created_at";

fn from_row(row: &Row<'_>) -> rusqlite::Result<Record> {
    let kind: String = row.get(1)?;
    let status: String = row.get(2)?;
    Ok(Record {
        id: row.get(0)?,
        kind: if kind == "command" {
            RecordType::Command
        } else {
            RecordType::Dictation
        },
        status: match status.as_str() {
            "failed" => RecordStatus::Failed,
            "running" => RecordStatus::Running,
            _ => RecordStatus::Ok,
        },
        text: row.get(3)?,
        app_name: row.get(4)?,
        app_id: row.get(5)?,
        url: row.get(6)?,
        field_kind: row.get(7)?,
        words: row.get(8)?,
        duration_ms: row.get::<_, i64>(9)? as u64,
        wpm: row.get(10)?,
        audio_path: row.get(11)?,
        error_code: row.get(12)?,
        created_at: row.get(13)?,
    })
}

/// `YYYY-MM-DD` for a Unix-ms timestamp at a fixed UTC offset (civil-from-days).
pub fn local_date(ms: i64, utc_offset_minutes: i32) -> String {
    let secs = ms.div_euclid(1000) + utc_offset_minutes as i64 * 60;
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests;
