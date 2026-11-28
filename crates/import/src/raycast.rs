//! Raycast snippets export: a JSON array of `{ "name", "text", "keyword" }`.
//!
//! The keyword becomes the trigger; a snippet without one uses its name.
//! Raycast's dynamic placeholders map as follows:
//!
//! | Raycast | Vocalis |
//! |---|---|
//! | `{clipboard}` | `{clipboard}` |
//! | `{date}`, `{time}`, `{datetime}`, `{day}` (with or without `format=`) | `{datetime}`, formatted by Vocalis |
//! | `{cursor}` | removed |
//! | anything with `offset=` or a `\|` modifier, `{selection}`, `{argument}`, `{uuid}`, `{snippet}`, `{browser-tab}` and other named placeholders | snippet skipped |
//! | other braces (code, JSON) | kept as written |

use serde_json::Value;

use crate::placeholders::{rewrite_braces, split_name, Action, Rewritten};
use crate::{Format, ImportError, Parsed};

const KNOWN_UNSUPPORTED: [&str; 9] = [
    "selection",
    "argument",
    "uuid",
    "snippet",
    "browser-tab",
    "random",
    "focused-window",
    "location",
    "weather",
];

fn decide(inner: &str) -> Action {
    let (name, rest) = split_name(inner.trim());
    let rest = rest.trim();
    let is_date = matches!(name, "date" | "time" | "datetime" | "day");
    if !(is_date || name == "clipboard" || name == "cursor" || KNOWN_UNSUPPORTED.contains(&name)) {
        return Action::Literal;
    }
    if rest.contains('|') {
        return Action::Unsupported(format!("the {{{name}}} placeholder with a modifier"));
    }
    if rest.contains("offset") {
        return Action::Unsupported(format!("the {{{name}}} placeholder with an offset"));
    }
    match name {
        "clipboard" if rest.is_empty() => Action::tag("{clipboard}"),
        "clipboard" => Action::Unsupported("the {clipboard} placeholder with options".into()),
        "cursor" => Action::removed("cursor position removed"),
        _ if is_date && rest.is_empty() => Action::tag("{datetime}"),
        _ if is_date => Action::tag_with_note("{datetime}", "custom date format replaced by Vocalis's date and time"),
        _ => Action::Unsupported(format!("the {{{name}}} placeholder")),
    }
}

pub fn parse(text: &str) -> Result<Parsed, ImportError> {
    let v: Value = serde_json::from_str(text).map_err(|e| ImportError::malformed(Format::Raycast, e.to_string()))?;
    let Value::Array(items) = v else {
        return Err(ImportError::malformed(Format::Raycast, "expected a list of snippets"));
    };
    let mut out = Parsed::default();
    for (i, item) in items.iter().enumerate() {
        let get = |k: &str| {
            item.get(k)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
        };
        let fallback = format!("item {}", i + 1);
        let name = get("name");
        let Some(content) = item.get("text").and_then(Value::as_str) else {
            out.invalid(name.unwrap_or(&fallback), "no snippet text");
            continue;
        };
        let trigger = match (get("keyword"), name) {
            (Some(k), _) => k.to_owned(),
            (None, Some(n)) => {
                out.convert(n, "no keyword, so the name is the trigger");
                n.to_owned()
            }
            (None, None) => {
                out.invalid(&fallback, "no keyword or name");
                continue;
            }
        };
        match rewrite_braces(content, decide) {
            Rewritten::Ok { text, notes } => {
                for n in notes {
                    out.convert(&trigger, n);
                }
                out.snippet(trigger, text)?;
            }
            Rewritten::Unsupported(what) => out.unsupported(&trigger, what),
        }
    }
    Ok(out)
}
