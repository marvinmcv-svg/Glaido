//! Dictionary and snippets (spec 4.5.1, 4.5.2), kept in the same encrypted
//! database as History. Validation comes from `vocalis-core`, so the rules are
//! the ones the gateway applies.
//!
//! Deletes are tombstones: the row keeps its id with `deleted_at` set, so sync
//! can tell other devices. Lists and duplicate checks ignore tombstones.

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use vocalis_core::dictionary::{self, Entry, EntryError};
use vocalis_core::snippets::{self, Snippet, SnippetError};

use crate::{Store, StoreError};

#[derive(Debug, Error)]
pub enum PersonalError {
    #[error(transparent)]
    Entry(#[from] EntryError),
    #[error(transparent)]
    Snippet(#[from] SnippetError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<rusqlite::Error> for PersonalError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Store(e.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryEntry {
    pub id: String,
    pub word: String,
    pub misspelling: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl DictionaryEntry {
    pub fn entry(&self) -> Entry {
        Entry {
            word: self.word.clone(),
            misspelling: self.misspelling.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetEntry {
    pub id: String,
    pub trigger: String,
    pub content: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl SnippetEntry {
    pub fn snippet(&self) -> Snippet {
        Snippet {
            trigger: self.trigger.clone(),
            content: self.content.clone(),
        }
    }
}

/// Trim both sides; an empty misspelling means "vocabulary".
fn tidy(e: Entry) -> Entry {
    Entry {
        word: e.word.trim().to_owned(),
        misspelling: e.misspelling.map(|m| m.trim().to_owned()).filter(|m| !m.is_empty()),
    }
}

fn dict_row(r: &Row<'_>) -> rusqlite::Result<DictionaryEntry> {
    Ok(DictionaryEntry {
        id: r.get(0)?,
        word: r.get(1)?,
        misspelling: r.get(2)?,
        created_at: r.get(3)?,
        updated_at: r.get(4)?,
    })
}

fn snippet_row(r: &Row<'_>) -> rusqlite::Result<SnippetEntry> {
    Ok(SnippetEntry {
        id: r.get(0)?,
        trigger: r.get(1)?,
        content: r.get(2)?,
        created_at: r.get(3)?,
        updated_at: r.get(4)?,
    })
}

const DICT_COLUMNS: &str = "id, word, misspelling, created_at, updated_at";
const SNIPPET_COLUMNS: &str = "id, trigger, content, created_at, updated_at";

impl Store {
    /// Live dictionary entries, newest first (the order the page shows).
    pub fn dictionary(&self) -> Result<Vec<DictionaryEntry>, StoreError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {DICT_COLUMNS} FROM dictionary WHERE deleted_at IS NULL ORDER BY created_at DESC, rowid DESC"
        ))?;
        let rows = stmt.query_map([], dict_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn dictionary_get(&self, id: &str) -> Result<DictionaryEntry, StoreError> {
        self.conn
            .query_row(
                &format!("SELECT {DICT_COLUMNS} FROM dictionary WHERE id = ?1 AND deleted_at IS NULL"),
                [id],
                dict_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)
    }

    /// Validate against the live entries (excluding `editing`) and return the tidied entry.
    fn dictionary_check(&self, entry: Entry, editing: Option<&str>) -> Result<Entry, PersonalError> {
        let entry = tidy(entry);
        let live = self.dictionary()?;
        let others: Vec<Entry> = live
            .iter()
            .filter(|e| Some(e.id.as_str()) != editing)
            .map(DictionaryEntry::entry)
            .collect();
        dictionary::validate(&entry, &others, None)?;
        Ok(entry)
    }

    pub fn dictionary_add(&self, entry: Entry, now: i64) -> Result<DictionaryEntry, PersonalError> {
        let entry = self.dictionary_check(entry, None)?;
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO dictionary (id, word, misspelling, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, entry.word, entry.misspelling, now],
        )?;
        Ok(self.dictionary_get(&id)?)
    }

    pub fn dictionary_update(&self, id: &str, entry: Entry, now: i64) -> Result<DictionaryEntry, PersonalError> {
        self.dictionary_get(id)?;
        let entry = self.dictionary_check(entry, Some(id))?;
        self.conn.execute(
            "UPDATE dictionary SET word = ?2, misspelling = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, entry.word, entry.misspelling, now],
        )?;
        Ok(self.dictionary_get(id)?)
    }

    pub fn dictionary_delete(&self, id: &str, now: i64) -> Result<(), StoreError> {
        let n = self.conn.execute(
            "UPDATE dictionary SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
            params![id, now],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// "Delete all words". Returns how many were deleted.
    pub fn dictionary_delete_all(&self, now: i64) -> Result<usize, StoreError> {
        Ok(self.conn.execute(
            "UPDATE dictionary SET deleted_at = ?1, updated_at = ?1 WHERE deleted_at IS NULL",
            [now],
        )?)
    }

    /// Live snippets in match order: oldest first, so the first one saved wins a shared trigger.
    pub fn snippets(&self) -> Result<Vec<SnippetEntry>, StoreError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SNIPPET_COLUMNS} FROM snippets WHERE deleted_at IS NULL ORDER BY created_at, rowid"
        ))?;
        let rows = stmt.query_map([], snippet_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn snippet_get(&self, id: &str) -> Result<SnippetEntry, StoreError> {
        self.conn
            .query_row(
                &format!("SELECT {SNIPPET_COLUMNS} FROM snippets WHERE id = ?1 AND deleted_at IS NULL"),
                [id],
                snippet_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)
    }

    fn snippet_check(s: Snippet) -> Result<Snippet, PersonalError> {
        // Keep the content's inner layout (it can be multi-line); trim only the trigger.
        let s = Snippet {
            trigger: s.trigger.trim().to_owned(),
            content: s.content,
        };
        snippets::validate(&s)?;
        Ok(s)
    }

    pub fn snippet_add(&self, s: Snippet, now: i64) -> Result<SnippetEntry, PersonalError> {
        let s = Self::snippet_check(s)?;
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO snippets (id, trigger, content, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, s.trigger, s.content, now],
        )?;
        Ok(self.snippet_get(&id)?)
    }

    pub fn snippet_update(&self, id: &str, s: Snippet, now: i64) -> Result<SnippetEntry, PersonalError> {
        self.snippet_get(id)?;
        let s = Self::snippet_check(s)?;
        self.conn.execute(
            "UPDATE snippets SET trigger = ?2, content = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, s.trigger, s.content, now],
        )?;
        Ok(self.snippet_get(id)?)
    }

    pub fn snippet_delete(&self, id: &str, now: i64) -> Result<(), StoreError> {
        let n = self.conn.execute(
            "UPDATE snippets SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
            params![id, now],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let (s, _) = Store::open(&dir.path().join("h.db"), &[7; 32]).unwrap();
        (dir, s)
    }

    #[test]
    fn dictionary_add_edit_delete_and_duplicates() {
        let (_d, s) = store();
        let a = s.dictionary_add(Entry::vocab("  Vocalis "), 1).unwrap();
        assert_eq!(a.word, "Vocalis");
        assert_eq!(a.misspelling, None);
        let b = s
            .dictionary_add(Entry::correction("postgres", "PostgreSQL"), 2)
            .unwrap();
        // An empty misspelling is vocabulary, and so a duplicate of "Vocalis".
        assert!(matches!(
            s.dictionary_add(Entry::correction(" ", "vocalis"), 3),
            Err(PersonalError::Entry(EntryError::Duplicate))
        ));
        // Newest first.
        let ids: Vec<_> = s.dictionary().unwrap().into_iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![b.id.clone(), a.id.clone()]);
        // Editing an entry to itself (other case) is fine; to another entry is a duplicate.
        s.dictionary_update(&a.id, Entry::vocab("VOCALIS"), 4).unwrap();
        assert!(matches!(
            s.dictionary_update(&a.id, Entry::correction("Postgres", "postgresql"), 5),
            Err(PersonalError::Entry(EntryError::Duplicate))
        ));
        assert!(matches!(
            s.dictionary_add(Entry::vocab("x".repeat(51)), 6),
            Err(PersonalError::Entry(EntryError::WordTooLong))
        ));
        // A deleted word can be added again.
        s.dictionary_delete(&a.id, 7).unwrap();
        assert!(matches!(s.dictionary_delete(&a.id, 8), Err(StoreError::NotFound)));
        s.dictionary_add(Entry::vocab("Vocalis"), 9).unwrap();
        assert_eq!(s.dictionary().unwrap().len(), 2);
        assert!(matches!(
            s.dictionary_update(&a.id, Entry::vocab("x"), 10),
            Err(PersonalError::Store(StoreError::NotFound))
        ));
    }

    #[test]
    fn dictionary_delete_all_leaves_tombstones() {
        let (_d, s) = store();
        s.dictionary_add(Entry::vocab("one"), 1).unwrap();
        s.dictionary_add(Entry::vocab("two"), 2).unwrap();
        assert_eq!(s.dictionary_delete_all(3).unwrap(), 2);
        assert!(s.dictionary().unwrap().is_empty());
        let tombstones: i64 = s
            .conn
            .query_row("SELECT count(*) FROM dictionary WHERE deleted_at = 3", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tombstones, 2);
    }

    #[test]
    fn snippets_keep_order_and_content() {
        let (_d, s) = store();
        let first = s
            .snippet_add(
                Snippet {
                    trigger: " my address ".into(),
                    content: "1 Main St\n  Springfield\n".into(),
                },
                1,
            )
            .unwrap();
        assert_eq!(first.trigger, "my address");
        assert_eq!(first.content, "1 Main St\n  Springfield\n");
        let second = s
            .snippet_add(
                Snippet {
                    trigger: "My address!".into(),
                    content: "second".into(),
                },
                2,
            )
            .unwrap();
        let list: Vec<Snippet> = s.snippets().unwrap().iter().map(SnippetEntry::snippet).collect();
        assert_eq!(
            snippets::find_match("my address", &list).unwrap().content,
            first.content
        );
        s.snippet_delete(&first.id, 3).unwrap();
        let list: Vec<Snippet> = s.snippets().unwrap().iter().map(SnippetEntry::snippet).collect();
        assert_eq!(snippets::find_match("my address", &list).unwrap().content, "second");
        assert!(matches!(
            s.snippet_update(
                &second.id,
                Snippet {
                    trigger: "...".into(),
                    content: "x".into()
                },
                4
            ),
            Err(PersonalError::Snippet(SnippetError::EmptyTrigger))
        ));
        let edited = s
            .snippet_update(
                &second.id,
                Snippet {
                    trigger: "sig".into(),
                    content: "Best".into(),
                },
                5,
            )
            .unwrap();
        assert_eq!((edited.trigger.as_str(), edited.updated_at), ("sig", 5));
    }

    #[test]
    fn survives_reopen_and_clear_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("h.db");
        {
            let (s, _) = Store::open(&path, &[7; 32]).unwrap();
            s.dictionary_add(Entry::vocab("Vocalis"), 1).unwrap();
            s.snippet_add(
                Snippet {
                    trigger: "sig".into(),
                    content: "Best".into(),
                },
                1,
            )
            .unwrap();
            // Clearing History does not touch personalization.
            s.clear_all().unwrap();
        }
        let (s, _) = Store::open(&path, &[7; 32]).unwrap();
        assert_eq!(s.dictionary().unwrap().len(), 1);
        assert_eq!(s.snippets().unwrap().len(), 1);
    }
}
