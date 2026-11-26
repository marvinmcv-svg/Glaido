use super::*;

fn key() -> Key {
    [7u8; 32]
}

fn open(dir: &Path) -> (Store, OpenReport) {
    Store::open(&dir.join("history.db"), &key()).unwrap()
}

fn rec(text: &str, at: i64) -> NewRecord {
    NewRecord {
        text: text.into(),
        words: count(text),
        created_at: at,
        ..Default::default()
    }
}

fn count(s: &str) -> u32 {
    s.split_whitespace().count() as u32
}

const DAY: i64 = 86_400_000;
const T0: i64 = 1_764_115_200_000; // 2025-11-26T00:00:00Z

#[test]
fn insert_get_and_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let (s, report) = open(dir.path());
    assert_eq!(report, OpenReport::default());
    let r = s.insert(rec("hello world", T0)).unwrap();
    assert_eq!(r.kind, RecordType::Dictation);
    assert_eq!(r.status, RecordStatus::Ok);
    assert_eq!(s.get(&r.id).unwrap(), r);
    assert!(matches!(s.get("nope"), Err(StoreError::NotFound)));
}

#[test]
fn failed_then_retry_completes() {
    let dir = tempfile::tempdir().unwrap();
    let (s, _) = open(dir.path());
    let r = s
        .insert(NewRecord {
            status: Some(RecordStatus::Failed),
            error_code: Some("TRANSCRIPTION_FAILED".into()),
            audio_path: Some("a.wav.enc".into()),
            created_at: T0,
            ..Default::default()
        })
        .unwrap();
    assert!(s.search("retried text").unwrap().is_empty());
    let done = s.complete(&r.id, "retried text works", 3, 120).unwrap();
    assert_eq!(done.status, RecordStatus::Ok);
    assert_eq!(done.error_code, None);
    assert_eq!(s.search("retried").unwrap().len(), 1, "FTS follows text updates");
    assert_eq!(s.fail(&r.id, "QUOTA_EXCEEDED").unwrap().status, RecordStatus::Failed);
}

#[test]
fn search_rules() {
    let dir = tempfile::tempdir().unwrap();
    let (s, _) = open(dir.path());
    s.insert(rec("Ship the PostgreSQL migration today", T0)).unwrap();
    s.insert(rec("Lunch at noon? 100% yes", T0 + 1)).unwrap();
    s.insert(NewRecord {
        status: Some(RecordStatus::Failed),
        ..rec("PostgreSQL failed one", T0 + 2)
    })
    .unwrap();
    assert!(s.search("p").unwrap().is_empty(), "under 2 chars");
    assert_eq!(
        s.search("gresq").unwrap().len(),
        1,
        "substring anywhere, failed excluded"
    );
    assert_eq!(s.search("POSTGRESQL").unwrap().len(), 1, "case-insensitive");
    assert_eq!(s.search("on").unwrap().len(), 2, "2-char LIKE path: migration, noon");
    assert_eq!(s.search("0%").unwrap().len(), 1, "LIKE wildcards are escaped");
    assert_eq!(s.search("__").unwrap().len(), 0);
    assert!(
        s.search("\"quoted\" OR x").unwrap().is_empty(),
        "FTS syntax is neutralized"
    );
}

#[test]
fn search_caps_at_50_newest_first() {
    let dir = tempfile::tempdir().unwrap();
    let (s, _) = open(dir.path());
    for i in 0..70 {
        s.insert(rec(&format!("note number {i}"), T0 + i)).unwrap();
    }
    let hits = s.search("note").unwrap();
    assert_eq!(hits.len(), SEARCH_LIMIT);
    assert_eq!(hits[0].text, "note number 69");
}

#[test]
fn groups_by_local_day_and_pages() {
    let dir = tempfile::tempdir().unwrap();
    let (s, _) = open(dir.path());
    // 23:30 UTC on day 0 is already day 1 at UTC+1.
    s.insert(rec("a", T0 + 23 * 3_600_000 + 30 * 60_000)).unwrap();
    s.insert(rec("b", T0 + 10 * 3_600_000)).unwrap();
    s.insert(rec("c", T0 - DAY + 1)).unwrap();
    let utc = s.recent_by_day(10, None, 0).unwrap();
    assert_eq!(
        utc.iter()
            .map(|g| (g.date.as_str(), g.records.len()))
            .collect::<Vec<_>>(),
        vec![("2025-11-26", 2), ("2025-11-25", 1)]
    );
    let cet = s.recent_by_day(10, None, 60).unwrap();
    assert_eq!(cet[0].date, "2025-11-27");
    let page2 = s.recent_by_day(10, Some(T0), 0).unwrap();
    assert_eq!(page2.len(), 1);
    assert_eq!(page2[0].records[0].text, "c");
}

#[test]
fn local_date_math() {
    assert_eq!(local_date(0, 0), "1970-01-01");
    assert_eq!(local_date(951_782_400_000, 0), "2000-02-29");
    assert_eq!(local_date(T0, -60), "2025-11-25");
    assert_eq!(local_date(-1, 0), "1969-12-31");
}

#[test]
fn audio_retention_keeps_newest_100_and_failed() {
    let dir = tempfile::tempdir().unwrap();
    let (s, _) = open(dir.path());
    let failed = s
        .insert(NewRecord {
            status: Some(RecordStatus::Failed),
            audio_path: Some("old-failed.wav.enc".into()),
            created_at: T0 - 1,
            ..Default::default()
        })
        .unwrap();
    for i in 0..105 {
        s.insert(NewRecord {
            audio_path: Some(format!("{i}.wav.enc")),
            ..rec("x", T0 + i)
        })
        .unwrap();
    }
    let expired = s.enforce_audio_retention().unwrap();
    let mut sorted = expired.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        vec!["0.wav.enc", "1.wav.enc", "2.wav.enc", "3.wav.enc", "4.wav.enc"]
    );
    assert_eq!(
        s.get(&failed.id).unwrap().audio_path.as_deref(),
        Some("old-failed.wav.enc")
    );
    assert!(s.enforce_audio_retention().unwrap().is_empty(), "idempotent");
}

#[test]
fn clear_all_returns_audio() {
    let dir = tempfile::tempdir().unwrap();
    let (s, _) = open(dir.path());
    s.insert(NewRecord {
        audio_path: Some("a".into()),
        ..rec("one", T0)
    })
    .unwrap();
    s.insert(rec("two", T0 + 1)).unwrap();
    assert_eq!(s.clear_all().unwrap(), vec!["a"]);
    assert!(s.recent_by_day(10, None, 0).unwrap().is_empty());
    assert!(s.search("one").unwrap().is_empty());
}

#[test]
fn encrypted_at_rest_and_wrong_key_rebuilds() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.db");
    {
        let (s, _) = Store::open(&path, &key()).unwrap();
        s.insert(rec("secret-transcript-marker", T0)).unwrap();
        s.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").unwrap();
    }
    let bytes = std::fs::read(&path).unwrap();
    assert!(!bytes.windows(13).any(|w| w == b"SQLite format"), "no plaintext header");
    assert!(
        !bytes.windows(24).any(|w| w == b"secret-transcript-marker"),
        "no plaintext content"
    );

    let (s, report) = Store::open(&path, &[9u8; 32]).unwrap();
    assert!(report.rebuilt_empty, "wrong key looks like damage and is rebuilt");
    assert!(s.recent_by_day(10, None, 0).unwrap().is_empty());
    // The unreadable original is kept aside, not deleted.
    let kept: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("history.db.damaged-"))
        .collect();
    assert!(!kept.is_empty());
}

#[test]
fn corruption_restores_from_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.db");
    {
        let (s, _) = Store::open(&path, &key()).unwrap();
        s.insert(rec("kept by backup", T0)).unwrap();
        s.backup().unwrap();
        s.insert(rec("after backup", T0 + 1)).unwrap();
        s.backup().unwrap(); // rotates: .bak1 = first, .bak0 = second
        s.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").unwrap();
    }
    assert!(backup_path(&path, 1).exists());
    // Damage the main file.
    let mut bytes = std::fs::read(&path).unwrap();
    for b in bytes.iter_mut().skip(1024).take(4096) {
        *b ^= 0xA5;
    }
    std::fs::write(&path, bytes).unwrap();

    let (s, report) = Store::open(&path, &key()).unwrap();
    assert!(report.restored_from_backup);
    let texts: Vec<String> = s
        .recent_by_day(10, None, 0)
        .unwrap()
        .into_iter()
        .flat_map(|g| g.records)
        .map(|r| r.text)
        .collect();
    assert_eq!(texts, vec!["after backup", "kept by backup"]);
}
