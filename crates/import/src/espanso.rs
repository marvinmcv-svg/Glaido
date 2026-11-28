//! Espanso match files (`match/*.yml`):
//!
//! ```yaml
//! matches:
//!   - trigger: ":sig"
//!     replace: "Best,\nSam"
//!   - triggers: [":hi", ":hello"]
//!     replace: "Hello {{name}}"
//!     vars:
//!       - name: name
//!         type: echo
//!         params: { echo: "there" }
//! ```
//!
//! Each trigger becomes one snippet. Variables are written `{{name}}`:
//!
//! | Espanso variable | Vocalis |
//! |---|---|
//! | `type: date` | `{datetime}` (the `format` is not kept) |
//! | `type: clipboard` | `{clipboard}` |
//! | `type: echo` | its text, inlined |
//! | `shell`, `script`, `random`, `choice`, `form`, `match` and other types, or an undefined variable | snippet skipped |
//!
//! `$|$` (cursor) is removed. `regex` triggers, `form:`, `image_path:` and
//! `html:` matches are skipped. `markdown:` content is imported as plain text.
//! `global_vars` in the same file are honoured. Nothing is ever run.

use std::collections::HashMap;

use yaml_rust2::{Yaml, YamlLoader};

use crate::placeholders::push_note;
use crate::{Format, ImportError, Parsed};
use vocalis_core::tags::{TAG_CLIPBOARD, TAG_DATETIME};

/// What a variable expands to, or why it can't.
#[derive(Debug, Clone)]
enum Var {
    Text { with: String, note: Option<&'static str> },
    Unsupported(String),
}

fn scalar(y: &Yaml) -> Option<String> {
    match y {
        Yaml::String(s) | Yaml::Real(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

fn vars(list: &Yaml, into: &mut HashMap<String, Var>) {
    let Some(list) = list.as_vec() else { return };
    for v in list {
        let Some(name) = scalar(&v["name"]) else { continue };
        let kind = v["type"].as_str().unwrap_or("");
        let var = match kind {
            "date" => Var::Text {
                with: TAG_DATETIME.into(),
                note: Some("date variable replaced by Vocalis's date and time"),
            },
            "clipboard" => Var::Text {
                with: TAG_CLIPBOARD.into(),
                note: None,
            },
            "echo" => match scalar(&v["params"]["echo"]) {
                Some(text) => Var::Text { with: text, note: None },
                None => Var::Unsupported(format!("the echo variable \"{name}\" without text")),
            },
            "" => Var::Unsupported(format!("the variable \"{name}\" without a type")),
            other => Var::Unsupported(format!("the {other} variable \"{name}\"")),
        };
        into.insert(name, var);
    }
}

/// Replace `{{name}}` occurrences and the `$|$` cursor marker.
fn expand(replace: &str, vars: &HashMap<String, Var>) -> Result<(String, Vec<String>), String> {
    let mut out = String::with_capacity(replace.len());
    let mut notes = Vec::new();
    let mut rest = replace;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let Some(close) = after.find("}}") else {
            out.push_str("{{");
            rest = after;
            continue;
        };
        let name = after[..close].trim();
        match vars.get(name) {
            Some(Var::Text { with, note }) => {
                out.push_str(with);
                if let Some(n) = note {
                    push_note(&mut notes, (*n).to_owned());
                }
            }
            Some(Var::Unsupported(what)) => return Err(what.clone()),
            None => return Err(format!("the undefined variable \"{name}\"")),
        }
        rest = &after[close + 2..];
    }
    out.push_str(rest);
    if out.contains("$|$") {
        out = out.replace("$|$", "");
        push_note(&mut notes, "cursor position removed".into());
    }
    Ok((out, notes))
}

pub fn parse(text: &str) -> Result<Parsed, ImportError> {
    let docs = YamlLoader::load_from_str(text).map_err(|e| ImportError::malformed(Format::Espanso, e.to_string()))?;
    let mut out = Parsed::default();
    let mut found = false;
    for doc in &docs {
        if doc.is_null() {
            continue;
        }
        if doc.as_hash().is_none() {
            return Err(ImportError::malformed(Format::Espanso, "expected a match file"));
        }
        let mut globals = HashMap::new();
        vars(&doc["global_vars"], &mut globals);
        let Some(matches) = doc["matches"].as_vec() else {
            if !doc["matches"].is_badvalue() {
                return Err(ImportError::malformed(Format::Espanso, "\"matches\" must be a list"));
            }
            continue;
        };
        found = true;
        for (i, m) in matches.iter().enumerate() {
            one(m, i, &globals, &mut out)?;
        }
    }
    if !found {
        return Err(ImportError::malformed(Format::Espanso, "no \"matches\" list"));
    }
    Ok(out)
}

fn one(m: &Yaml, index: usize, globals: &HashMap<String, Var>, out: &mut Parsed) -> Result<(), ImportError> {
    let mut triggers: Vec<String> = Vec::new();
    if let Some(t) = scalar(&m["trigger"]) {
        triggers.push(t);
    }
    if let Some(list) = m["triggers"].as_vec() {
        triggers.extend(list.iter().filter_map(scalar));
    }
    let fallback = format!("match {}", index + 1);
    let name = triggers.first().cloned().unwrap_or(fallback);
    if triggers.is_empty() {
        if !m["regex"].is_badvalue() {
            out.unsupported(&name, "regex triggers");
        } else {
            out.invalid(&name, "no trigger");
        }
        return Ok(());
    }
    for (key, what) in [("form", "forms"), ("image_path", "images"), ("html", "HTML content")] {
        if !m[key].is_badvalue() {
            out.unsupported(&name, what);
            return Ok(());
        }
    }
    let (replace, markdown) = match (scalar(&m["replace"]), scalar(&m["markdown"])) {
        (Some(r), _) => (r, false),
        (None, Some(md)) => (md, true),
        (None, None) => {
            out.invalid(&name, "no replace text");
            return Ok(());
        }
    };
    let mut scope = globals.clone();
    vars(&m["vars"], &mut scope);
    let (content, mut notes) = match expand(&replace, &scope) {
        Ok(r) => r,
        Err(what) => {
            out.unsupported(&name, what);
            return Ok(());
        }
    };
    if markdown {
        notes.push("Markdown imported as plain text".into());
    }
    if triggers.len() > 1 {
        notes.push(format!("one snippet per trigger ({} triggers)", triggers.len()));
    }
    for t in triggers {
        for n in &notes {
            out.convert(&t, n.clone());
        }
        out.snippet(t, content.clone())?;
    }
    Ok(())
}
