//! TextExpander groups exported as CSV: `abbreviation,content,label` per row
//! (the label is optional and ignored). A header row naming those columns is
//! skipped.
//!
//! Macros in the content:
//!
//! | TextExpander | Vocalis |
//! |---|---|
//! | `%clipboard` | `{clipboard}` |
//! | a run of date/time macros (`%Y`, `%m`, `%d`, `%e`, `%B`, `%b`, `%A`, `%a`, `%y`, `%H`, `%I`, `%1H`, `%1I`, `%1m`, `%M`, `%S`, `%p`) and the separators between them | one `{datetime}` |
//! | `%\|` (cursor) | removed |
//! | `%%` | `%` |
//! | fill-ins, keys, nested snippets, date math, scripts (`%fill…`, `%key:…`, `%snippet:…`, `%@…`, `%(…)`, `%<…>`, `%^`) | snippet skipped |
//! | any other `%` | kept as written (so "50% off" survives) |

use vocalis_core::tags::{TAG_CLIPBOARD, TAG_DATETIME};

use crate::placeholders::push_note;
use crate::{Format, ImportError, Parsed};

const DATE_MACROS: [&str; 17] = [
    "1H", "1I", "1m", "Y", "y", "B", "b", "m", "d", "e", "A", "a", "H", "I", "M", "S", "p",
];

/// Length of the date macro at the start of `s` (after the `%`), if any.
fn date_macro(s: &str) -> Option<usize> {
    DATE_MACROS.iter().find(|m| s.starts_with(*m)).map(|m| m.len())
}

#[derive(Debug, PartialEq, Eq)]
enum Converted {
    Ok { text: String, notes: Vec<String> },
    Unsupported(String),
}

fn convert(content: &str) -> Converted {
    let mut out = String::with_capacity(content.len());
    let mut notes = Vec::new();
    let mut rest = content;
    while let Some(pos) = rest.find('%') {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 1..];
        if let Some(a) = after.strip_prefix('%') {
            out.push('%');
            rest = a;
        } else if let Some(a) = after.strip_prefix("clipboard") {
            out.push_str(TAG_CLIPBOARD);
            rest = a;
        } else if let Some(a) = after.strip_prefix('|') {
            push_note(&mut notes, "cursor position removed".into());
            rest = a;
        } else if let Some(len) = date_macro(after) {
            // Swallow the whole run, e.g. "%B %e, %Y" or "%H:%M".
            let mut tail = &after[len..];
            loop {
                let sep = tail
                    .find(|c: char| !matches!(c, ' ' | ',' | '.' | '/' | '-' | ':'))
                    .unwrap_or(tail.len());
                match tail[sep..]
                    .strip_prefix('%')
                    .and_then(|t| date_macro(t).map(|l| &t[l..]))
                {
                    Some(next) if sep <= 3 => tail = next,
                    _ => break,
                }
            }
            out.push_str(TAG_DATETIME);
            push_note(
                &mut notes,
                "date and time macros replaced by Vocalis's date and time".into(),
            );
            rest = tail;
        } else if ["fill", "key:", "snippet:", "@", "(", "<", "^"]
            .iter()
            .any(|p| after.starts_with(p))
        {
            let what = after
                .chars()
                .take_while(|c| !c.is_whitespace())
                .take(20)
                .collect::<String>();
            return Converted::Unsupported(format!("the %{what} macro"));
        } else {
            out.push('%');
            rest = after;
        }
    }
    out.push_str(rest);
    Converted::Ok { text: out, notes }
}

pub fn parse(text: &str) -> Result<Parsed, ImportError> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut out = Parsed::default();
    for (i, rec) in rdr.records().enumerate() {
        let rec = rec.map_err(|e| ImportError::malformed(Format::TextExpander, e.to_string()))?;
        let line = rec.position().map(|p| p.line()).unwrap_or(0);
        let abbr = rec.get(0).unwrap_or("").trim();
        let content = rec.get(1).unwrap_or("");
        if i == 0 && abbr.eq_ignore_ascii_case("abbreviation") {
            continue;
        }
        if rec.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        if abbr.is_empty() {
            out.invalid(&format!("line {line}"), "no abbreviation");
            continue;
        }
        if rec.len() < 2 {
            out.invalid(abbr, "no content");
            continue;
        }
        match convert(content) {
            Converted::Ok { text, notes } => {
                for n in notes {
                    out.convert(abbr, n);
                }
                out.snippet(abbr.to_owned(), text)?;
            }
            Converted::Unsupported(what) => out.unsupported(abbr, what),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(s: &str) -> String {
        match convert(s) {
            Converted::Ok { text, .. } => text,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn macros() {
        assert_eq!(ok("Today is %B %e, %Y."), "Today is {datetime}.");
        assert_eq!(ok("%H:%M - done"), "{datetime} - done");
        assert_eq!(ok("%A at %H:%M"), "{datetime} at {datetime}");
        assert_eq!(ok("50% off, 100%%"), "50% off, 100%");
        assert_eq!(ok("Hi %|there %clipboard"), "Hi there {clipboard}");
        assert_eq!(ok("trailing %"), "trailing %");
        assert_eq!(ok("10%-20% (approx)"), "10%-20% (approx)");
        assert!(matches!(convert("Dear %filltext:name=x%"), Converted::Unsupported(_)));
        assert!(matches!(convert("%key:enter%"), Converted::Unsupported(_)));
        assert!(matches!(convert("%snippet:sig%"), Converted::Unsupported(_)));
        assert!(matches!(convert("%@+1D%Y"), Converted::Unsupported(_)));
    }
}
