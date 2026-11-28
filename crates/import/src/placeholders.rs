//! Rewriting another tool's placeholders into Vocalis tags.
//!
//! Vocalis has three tags (`{clipboard}`, `{datetime}`, `{timezone}`). A
//! source placeholder is either mapped to one of them, removed (cursor
//! markers, which only make sense while typing), or unsupported, in which case
//! the whole snippet is skipped rather than imported with a hole in it.

/// What to do with one placeholder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Replace with this text; `note` explains a lossy conversion.
    Replace { with: String, note: Option<String> },
    /// The tool has a feature we can't reproduce: skip the snippet.
    Unsupported(String),
    /// Not a placeholder after all: keep the source text as written.
    Literal,
}

impl Action {
    pub fn tag(tag: &str) -> Self {
        Action::Replace {
            with: tag.to_owned(),
            note: None,
        }
    }

    pub fn tag_with_note(tag: &str, note: impl Into<String>) -> Self {
        Action::Replace {
            with: tag.to_owned(),
            note: Some(note.into()),
        }
    }

    pub fn removed(note: impl Into<String>) -> Self {
        Action::Replace {
            with: String::new(),
            note: Some(note.into()),
        }
    }
}

/// The result of rewriting one text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rewritten {
    Ok { text: String, notes: Vec<String> },
    Unsupported(String),
}

/// Rewrite single-brace placeholders `{name…}` (Alfred, Raycast). `decide`
/// receives the text between the braces. Braces that span lines or run long
/// are never placeholders.
pub fn rewrite_braces(text: &str, decide: impl Fn(&str) -> Action) -> Rewritten {
    let mut out = String::with_capacity(text.len());
    let mut notes = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after
            .find('}')
            .filter(|&c| c <= 200 && !after[..c].contains(['\n', '{']));
        let Some(close) = close else {
            out.push('{');
            rest = after;
            continue;
        };
        let inner = &after[..close];
        match decide(inner) {
            Action::Replace { with, note } => {
                out.push_str(&with);
                if let Some(n) = note {
                    push_note(&mut notes, n);
                }
            }
            Action::Unsupported(what) => return Rewritten::Unsupported(what),
            Action::Literal => {
                out.push('{');
                out.push_str(inner);
                out.push('}');
            }
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    Rewritten::Ok { text: out, notes }
}

pub fn push_note(notes: &mut Vec<String>, n: String) {
    if !notes.contains(&n) {
        notes.push(n);
    }
}

/// Split a placeholder body into its leading name (letters, digits, `-`, `_`)
/// and the rest, e.g. `date format="d MMM"` → (`date`, ` format="d MMM"`).
pub fn split_name(inner: &str) -> (&str, &str) {
    let end = inner
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(inner.len());
    (&inner[..end], &inner[end..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decide(inner: &str) -> Action {
        match inner {
            "clipboard" => Action::tag("{clipboard}"),
            "cursor" => Action::removed("cursor removed"),
            "bad" => Action::Unsupported("bad".into()),
            _ => Action::Literal,
        }
    }

    #[test]
    fn rewrites_and_keeps_unknown_braces() {
        assert_eq!(
            rewrite_braces("a {clipboard} b {x} {cursor}{cursor} }{ {", decide),
            Rewritten::Ok {
                text: "a {clipboard} b {x}  }{ {".into(),
                notes: vec!["cursor removed".into()]
            }
        );
    }

    #[test]
    fn unsupported_stops() {
        assert_eq!(
            rewrite_braces("x {bad} {clipboard}", decide),
            Rewritten::Unsupported("bad".into())
        );
    }

    #[test]
    fn multiline_braces_are_literal() {
        let code = "fn main() {\n  println!(\"{clipboard}\");\n}";
        assert_eq!(
            rewrite_braces(code, decide),
            Rewritten::Ok {
                text: code.into(),
                notes: vec![]
            }
        );
    }

    #[test]
    fn splits_names() {
        assert_eq!(split_name("date format=\"d\""), ("date", " format=\"d\""));
        assert_eq!(split_name("clipboard:1"), ("clipboard", ":1"));
        assert_eq!(split_name("browser-tab"), ("browser-tab", ""));
    }
}
