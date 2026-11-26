//! Dynamic tags usable in dictionary corrections and snippet content (spec 4.5.1).
//!
//! The desktop supplies already-formatted values; this module only substitutes.

use serde::{Deserialize, Serialize};

pub const TAG_CLIPBOARD: &str = "{clipboard}";
pub const TAG_DATETIME: &str = "{datetime}";
pub const TAG_TIMEZONE: &str = "{timezone}";
pub const ALL_TAGS: [&str; 3] = [TAG_CLIPBOARD, TAG_DATETIME, TAG_TIMEZONE];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TagContext {
    /// Current plain-text clipboard, if any. Missing clipboard expands to "".
    pub clipboard: Option<String>,
    /// Local date and time, formatted by the client for the user's locale.
    pub datetime: String,
    /// IANA zone name or a display name such as "Europe/Berlin".
    pub timezone: String,
}

/// True when the text contains any dynamic tag.
pub fn contains_tag(s: &str) -> bool {
    ALL_TAGS.iter().any(|t| s.contains(t))
}

/// Replace every dynamic tag in a single pass, so tag-like text inside an
/// expanded value (e.g. a clipboard containing "{datetime}") is not expanded again.
pub fn expand(s: &str, ctx: &TagContext) -> String {
    if !s.contains('{') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    'outer: while let Some(pos) = rest.find('{') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        for tag in ALL_TAGS {
            if let Some(after) = tail.strip_prefix(tag) {
                let value = match tag {
                    TAG_CLIPBOARD => ctx.clipboard.as_deref().unwrap_or(""),
                    TAG_DATETIME => ctx.datetime.as_str(),
                    _ => ctx.timezone.as_str(),
                };
                out.push_str(value);
                rest = after;
                continue 'outer;
            }
        }
        out.push('{');
        rest = &tail[1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> TagContext {
        TagContext {
            clipboard: Some("{datetime} copied".into()),
            datetime: "2026-11-26 14:05".into(),
            timezone: "Europe/Berlin".into(),
        }
    }

    #[test]
    fn expands_all_tags_once() {
        assert_eq!(
            expand("At {datetime} ({timezone}): {clipboard}", &ctx()),
            "At 2026-11-26 14:05 (Europe/Berlin): {datetime} copied"
        );
    }

    #[test]
    fn leaves_other_braces() {
        assert_eq!(expand("fn x() { {unknown} }", &ctx()), "fn x() { {unknown} }");
    }

    #[test]
    fn missing_clipboard_is_empty() {
        let c = TagContext {
            clipboard: None,
            ..ctx()
        };
        assert_eq!(expand("[{clipboard}]", &c), "[]");
    }

    #[test]
    fn detects_tags() {
        assert!(contains_tag("paste {clipboard}"));
        assert!(!contains_tag("no tags {here}"));
    }
}
