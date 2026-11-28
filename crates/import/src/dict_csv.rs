//! Dictionary CSV files (Wispr Flow, Superwhisper) and Wispr Flow's snippets JSON.
//!
//! **Wispr Flow** documents its bulk-import files publicly (ledger IMP-05):
//! a dictionary CSV with one column (a word) or two (`misspelling,correction`),
//! no header, and a snippets JSON array of `{ "name", "text" }`. We follow the
//! same row rules: surrounding whitespace and invisible characters are
//! removed, empty fields are dropped (so `word,` is a word and `,fix` is the
//! word `fix`), blank lines are ignored, and rows with more than two non-empty
//! fields are skipped. Wispr Flow has no documented *export*, so this reads the
//! file a user prepared for that import (or the same layout from anywhere).
//!
//! **Superwhisper**'s own vocabulary file layout is not publicly documented
//! (ledger IMP-06, UNKNOWN), so we accept a conservative CSV: one column for a
//! word, two for `original,replacement`, optionally with a header row naming
//! the columns.

use serde_json::Value;

use crate::{Format, ImportError, Parsed};
use vocalis_core::dictionary::Entry;

/// Characters that are invisible but sometimes surround copied words.
fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}' | '\u{00AD}' | '\u{180E}'
    )
}

fn clean(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || is_invisible(c))
}

/// Column names we recognise in a header row. `true` = the column holds the
/// spoken/wrong form (misspelling), `false` = the correct word.
fn header_role(cell: &str) -> Option<bool> {
    match cell.to_lowercase().as_str() {
        "misspelling" | "misspelled" | "original" | "from" | "wrong" | "spoken" | "phrase" | "find" | "heard" => {
            Some(true)
        }
        "word" | "words" | "vocabulary" | "correction" | "corrected" | "correct" | "replacement" | "replace"
        | "replace with" | "to" | "term" => Some(false),
        _ => None,
    }
}

fn records(text: &str, format: Format) -> Result<Vec<(usize, Vec<String>)>, ImportError> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| ImportError::malformed(format, e.to_string()))?;
        let line = rec.position().map(|p| p.line() as usize).unwrap_or(0);
        rows.push((line, rec.iter().map(str::to_owned).collect()));
    }
    Ok(rows)
}

fn parse_dictionary(text: &str, format: Format) -> Result<Parsed, ImportError> {
    let mut out = Parsed::default();
    let rows = records(text, format)?;
    // (misspelling column, word column) for two-column rows; default is misspelling first.
    let mut order = (0usize, 1usize);
    for (i, (line, cells)) in rows.iter().enumerate() {
        let cells: Vec<&str> = cells.iter().map(|c| clean(c)).filter(|c| !c.is_empty()).collect();
        let where_ = format!("line {line}");
        if i == 0 && !cells.is_empty() && cells.iter().all(|c| header_role(c).is_some()) {
            if cells.len() == 2 {
                if let (Some(false), Some(true)) = (header_role(cells[0]), header_role(cells[1])) {
                    order = (1, 0);
                }
            }
            out.convert(&cells.join(","), "header row skipped");
            continue;
        }
        match cells.len() {
            0 => {}
            1 => out.entry(Entry::vocab(cells[0]))?,
            2 => out.entry(Entry::correction(cells[order.0], cells[order.1]))?,
            _ => out.invalid(&where_, "more than two columns"),
        }
    }
    Ok(out)
}

fn parse_snippets_json(text: &str) -> Result<Parsed, ImportError> {
    let v: Value = serde_json::from_str(text).map_err(|e| ImportError::malformed(Format::WisprFlow, e.to_string()))?;
    let Value::Array(items) = v else {
        return Err(ImportError::malformed(Format::WisprFlow, "expected a list of snippets"));
    };
    let mut out = Parsed::default();
    for (i, item) in items.iter().enumerate() {
        let fallback = format!("item {}", i + 1);
        let name = item.get("name").and_then(Value::as_str).map(clean);
        let content = item.get("text").and_then(Value::as_str).map(clean);
        match (name, content) {
            (Some(n), Some(t)) if !n.is_empty() && !t.is_empty() => out.snippet(n.to_owned(), t.to_owned())?,
            (Some(n), _) if !n.is_empty() => out.invalid(n, "no snippet text"),
            _ => out.invalid(&fallback, "needs a name and a text"),
        }
    }
    Ok(out)
}

/// Wispr Flow: the snippets JSON if the file is JSON, otherwise the dictionary CSV.
pub fn parse_wispr(text: &str) -> Result<Parsed, ImportError> {
    let t = text.trim_start();
    if t.starts_with('[') || t.starts_with('{') {
        parse_snippets_json(text)
    } else {
        parse_dictionary(text, Format::WisprFlow)
    }
}

pub fn parse_superwhisper(text: &str) -> Result<Parsed, ImportError> {
    parse_dictionary(text, Format::Superwhisper)
}

/// TextExpander when some row has a non-empty third column and none has more;
/// otherwise unknown (one- and two-column files fit both dictionary formats).
pub fn guess_csv(text: &str) -> Option<Format> {
    let rows = records(text, Format::TextExpander).ok()?;
    let rows = &rows[..rows.len().min(200)];
    let three = rows
        .iter()
        .any(|(_, c)| c.len() == 3 && !clean(&c[2]).is_empty() && !clean(&c[0]).is_empty());
    let wider = rows
        .iter()
        .any(|(_, c)| c.iter().filter(|x| !clean(x).is_empty()).count() > 3);
    (three && !wider).then_some(Format::TextExpander)
}
