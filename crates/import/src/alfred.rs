//! Alfred snippet collections: a `.alfredsnippets` file is a zip holding one
//! JSON file per snippet, `{ "alfredsnippet": { "name", "keyword", "snippet", "uid" } }`,
//! and an optional `info.plist` with a keyword prefix and suffix for the
//! whole collection. A single unzipped snippet JSON is accepted too.
//!
//! The trigger is prefix + keyword + suffix; a snippet without a keyword uses
//! its name. Placeholders:
//!
//! | Alfred | Vocalis |
//! |---|---|
//! | `{clipboard}` | `{clipboard}` |
//! | `{date}`, `{time}`, `{datetime}`, `{isodate}` (any style such as `{date:short}`) | `{datetime}` |
//! | `{cursor}` | removed |
//! | date arithmetic (`{date +1d}`), `{clipboard:N}`, `{random…}`, `{snippet:…}`, `{var:…}`, `{uuid}`, `{query}` | snippet skipped |
//! | other braces | kept as written |

use std::io::{Cursor, Read};

use serde_json::Value;

use crate::placeholders::{rewrite_braces, split_name, Action, Rewritten};
use crate::{Format, ImportError, Parsed, MAX_FILE_BYTES, MAX_ITEMS};

/// Total bytes we are willing to unpack from one collection.
const MAX_UNPACKED_BYTES: u64 = 4 * MAX_FILE_BYTES as u64;

const KNOWN_UNSUPPORTED: [&str; 8] = [
    "random",
    "snippet",
    "var",
    "uuid",
    "query",
    "selection",
    "allvars",
    "calc",
];

fn decide(inner: &str) -> Action {
    let (name, rest) = split_name(inner.trim());
    let is_date = matches!(name, "date" | "time" | "datetime" | "isodate");
    if !(is_date || name == "clipboard" || name == "cursor" || KNOWN_UNSUPPORTED.contains(&name)) {
        return Action::Literal;
    }
    match name {
        "clipboard" if rest.is_empty() => Action::tag("{clipboard}"),
        "clipboard" => Action::Unsupported("an earlier clipboard item ({clipboard:N})".into()),
        "cursor" if rest.is_empty() => Action::removed("cursor position removed"),
        _ if is_date && (rest.contains(" +") || rest.contains(" -")) => Action::Unsupported("date arithmetic".into()),
        _ if is_date && rest.is_empty() => Action::tag("{datetime}"),
        _ if is_date => Action::tag_with_note("{datetime}", "date style replaced by Vocalis's date and time"),
        _ => Action::Unsupported(format!("the {{{name}}} placeholder")),
    }
}

/// Affixes from `info.plist`: `<key>snippetkeywordprefix</key><string>…</string>`.
fn plist_string(plist: &str, key: &str) -> String {
    let needle = format!("<key>{key}</key>");
    let Some(at) = plist.find(&needle) else {
        return String::new();
    };
    let after = plist[at + needle.len()..].trim_start();
    after
        .strip_prefix("<string>")
        .and_then(|s| s.find("</string>").map(|end| &s[..end]))
        .map(unescape_xml)
        .unwrap_or_default()
}

fn unescape_xml(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

pub fn parse(data: &[u8]) -> Result<Parsed, ImportError> {
    let mut out = Parsed::default();
    if !data.starts_with(b"PK") {
        // A single snippet JSON file.
        let text = std::str::from_utf8(data).map_err(|_| ImportError::NotText)?;
        let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .map_err(|e| ImportError::malformed(Format::Alfred, e.to_string()))?;
        one(&v, "snippet", "", "", &mut out)?;
        return Ok(out);
    }
    let mut zip =
        zip::ZipArchive::new(Cursor::new(data)).map_err(|e| ImportError::malformed(Format::Alfred, e.to_string()))?;
    if zip.len() > MAX_ITEMS + 1 {
        return Err(ImportError::TooManyItems);
    }
    let mut budget = MAX_UNPACKED_BYTES;
    let mut files: Vec<(String, String)> = Vec::new();
    for i in 0..zip.len() {
        let mut f = zip
            .by_index(i)
            .map_err(|e| ImportError::malformed(Format::Alfred, e.to_string()))?;
        let name = f.name().to_owned();
        let base = name.rsplit('/').next().unwrap_or(&name).to_owned();
        let wanted = !f.is_dir()
            && !name.starts_with("__MACOSX/")
            && !base.starts_with("._")
            && (base.ends_with(".json") || base == "info.plist");
        if !wanted {
            continue;
        }
        // Read at most the remaining budget (+1 to notice overflow); never trust the header size.
        let mut buf = Vec::new();
        (&mut f)
            .take(budget + 1)
            .read_to_end(&mut buf)
            .map_err(|e| ImportError::malformed(Format::Alfred, e.to_string()))?;
        if buf.len() as u64 > budget {
            return Err(ImportError::TooLarge);
        }
        budget -= buf.len() as u64;
        match String::from_utf8(buf) {
            Ok(s) => files.push((name, s)),
            Err(_) => out.invalid(&base, "not UTF-8 text"),
        }
    }
    let plist = files
        .iter()
        .find(|(n, _)| n.ends_with("info.plist"))
        .map(|(_, s)| s.as_str());
    let prefix = plist
        .map(|p| plist_string(p, "snippetkeywordprefix"))
        .unwrap_or_default();
    let suffix = plist
        .map(|p| plist_string(p, "snippetkeywordsuffix"))
        .unwrap_or_default();
    let mut found = false;
    let mut jsons: Vec<&(String, String)> = files.iter().filter(|(n, _)| n.ends_with(".json")).collect();
    jsons.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, text) in jsons {
        found = true;
        let base = name.rsplit('/').next().unwrap_or(name);
        match serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) {
            Ok(v) => one(&v, base, &prefix, &suffix, &mut out)?,
            Err(_) => out.invalid(base, "not valid JSON"),
        }
    }
    if !found {
        return Err(ImportError::malformed(Format::Alfred, "no snippets in the collection"));
    }
    Ok(out)
}

fn one(v: &Value, file: &str, prefix: &str, suffix: &str, out: &mut Parsed) -> Result<(), ImportError> {
    let Some(s) = v.get("alfredsnippet") else {
        out.invalid(file, "not an Alfred snippet");
        return Ok(());
    };
    let get = |k: &str| {
        s.get(k)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|x| !x.is_empty())
    };
    let name = get("name");
    let Some(content) = s.get("snippet").and_then(Value::as_str) else {
        out.invalid(name.unwrap_or(file), "no snippet text");
        return Ok(());
    };
    let trigger = match (get("keyword"), name) {
        (Some(k), _) => format!("{prefix}{k}{suffix}"),
        (None, Some(n)) => {
            out.convert(n, "no keyword, so the name is the trigger");
            n.to_owned()
        }
        (None, None) => {
            out.invalid(file, "no keyword or name");
            return Ok(());
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
    Ok(())
}
