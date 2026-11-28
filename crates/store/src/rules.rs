//! Formatting rules (spec 4.5.3), kept in the same encrypted database as
//! History. Every change is checked with `RuleSet::validate` from
//! `vocalis-core` (one owner per app or site, All apps always on, prompts of
//! at most 500 characters), so the gateway never receives a set it rejects.
//!
//! * The table is seeded lazily with `RuleSet::default()`: All apps, plus the
//!   built-in Email rule with two webmail sites.
//! * All apps can be neither switched off nor deleted; Email can be switched
//!   off but not deleted.
//! * Deletes are tombstones (`deleted_at`), as for the dictionary, so sync can
//!   pass them on. The shape mirrors `public.formatting_rules`.
//! * Apps are stored with their display name, so the page can show "Slack"
//!   rather than `com.tinyspeck.slackmacgap` after the app is uninstalled.

use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use vocalis_core::formatting::{normalize_host, Rule, RuleError, RuleKind, RuleSet, Style};
use vocalis_core::presets::{self, Os, Preset, Skipped};
use vocalis_core::text::char_len;

use crate::{Store, StoreError};

/// Maximum characters in a rule name (matches the server's column check).
pub const MAX_NAME_CHARS: usize = 80;
/// "Most used" apps: how many, and over how many days of dictations.
pub const MOST_USED_LIMIT: usize = 30;
pub const MOST_USED_DAYS: i64 = 90;

#[derive(Debug, Error)]
pub enum RulesError {
    #[error(transparent)]
    Rule(#[from] RuleError),
    #[error("Give the rule a name.")]
    NameEmpty,
    #[error("Rule names can be at most {MAX_NAME_CHARS} characters.")]
    NameTooLong,
    #[error("\"{0}\" doesn't look like a website address.")]
    BadSite(String),
    #[error("The All apps rule can't be deleted.")]
    DeleteAllApps,
    #[error("The Email rule can be switched off, but not deleted.")]
    DeleteEmail,
    #[error("There is no preset called \"{0}\".")]
    UnknownPreset(String),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<rusqlite::Error> for RulesError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Store(e.into())
    }
}

/// An app a rule targets, with the name to show for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppRef {
    /// macOS bundle id, Windows exe name or X11 WM_CLASS class.
    pub id: String,
    pub name: String,
}

/// A stored rule. Times are Unix milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormattingRule {
    pub id: String,
    pub kind: RuleKind,
    pub name: String,
    pub enabled: bool,
    pub style: Style,
    pub raw: bool,
    pub prompt: String,
    pub apps: Vec<AppRef>,
    pub sites: Vec<String>,
    pub position: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl FormattingRule {
    /// The rule as the formatting pipeline sees it.
    pub fn rule(&self) -> Rule {
        Rule {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.kind,
            enabled: self.enabled,
            style: self.style,
            raw: self.raw,
            prompt: self.prompt.clone(),
            apps: self.apps.iter().map(|a| a.id.clone()).collect(),
            sites: self.sites.clone(),
        }
    }
}

/// The editable fields of a rule. For the built-in rules the name is fixed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RuleDraft {
    pub name: String,
    pub enabled: bool,
    pub style: Style,
    pub raw: bool,
    pub prompt: String,
    pub apps: Vec<AppRef>,
    pub sites: Vec<String>,
}

impl Default for RuleDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            enabled: true,
            style: Style::Standard,
            raw: false,
            prompt: String::new(),
            apps: vec![],
            sites: vec![],
        }
    }
}

/// What adding a preset did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetOutcome {
    /// The new rule, or the updated Email rule. `None` when every app and site
    /// the preset wanted already belongs to another rule.
    pub rule: Option<FormattingRule>,
    pub skipped: Vec<Skipped>,
    /// The Email rule's own prompt was kept.
    pub kept_prompt: bool,
}

/// An app from recent dictations, for the picker's "Most used" list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsedApp {
    pub id: String,
    pub name: String,
    pub count: u32,
}

fn kind_str(k: RuleKind) -> &'static str {
    match k {
        RuleKind::AllApps => "all_apps",
        RuleKind::Email => "email",
        RuleKind::Custom => "custom",
    }
}

fn kind_from(s: &str) -> RuleKind {
    match s {
        "all_apps" => RuleKind::AllApps,
        "email" => RuleKind::Email,
        _ => RuleKind::Custom,
    }
}

fn style_str(s: Style) -> &'static str {
    match s {
        Style::Standard => "standard",
        Style::Casual => "casual",
        Style::Lowercase => "lowercase",
    }
}

fn style_from(s: &str) -> Style {
    match s {
        "casual" => Style::Casual,
        "lowercase" => Style::Lowercase,
        _ => Style::Standard,
    }
}

fn json_col<T: serde::de::DeserializeOwned>(r: &Row<'_>, i: usize) -> rusqlite::Result<T> {
    let s: String = r.get(i)?;
    serde_json::from_str(&s)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(i, rusqlite::types::Type::Text, e.into()))
}

fn rule_row(r: &Row<'_>) -> rusqlite::Result<FormattingRule> {
    Ok(FormattingRule {
        id: r.get(0)?,
        kind: kind_from(&r.get::<_, String>(1)?),
        name: r.get(2)?,
        enabled: r.get(3)?,
        style: style_from(&r.get::<_, String>(4)?),
        raw: r.get(5)?,
        prompt: r.get(6)?,
        apps: json_col(r, 7)?,
        sites: json_col(r, 8)?,
        position: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
    })
}

const RULE_COLUMNS: &str = "id, kind, name, enabled, style, raw, prompt, apps, sites, position, created_at, updated_at";

/// A site entry as a bare host (`https://www.GitHub.com/x` → `github.com`).
fn tidy_site(s: &str) -> Result<Option<String>, RulesError> {
    if s.trim().is_empty() {
        return Ok(None);
    }
    let host = normalize_host(s);
    let ok = !host.is_empty()
        && !host.starts_with(['.', '-'])
        && host.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-')
        && !host.contains("..");
    if !ok {
        return Err(RulesError::BadSite(s.trim().to_owned()));
    }
    Ok(Some(host))
}

/// Trim, drop empties and duplicates (apps case-insensitively, sites by host).
fn tidy(d: RuleDraft) -> Result<RuleDraft, RulesError> {
    let mut apps: Vec<AppRef> = Vec::new();
    for a in d.apps {
        let id = a.id.trim().to_owned();
        if id.is_empty() || apps.iter().any(|x| x.id.eq_ignore_ascii_case(&id)) {
            continue;
        }
        let name = match a.name.trim() {
            "" => id.clone(),
            n => n.to_owned(),
        };
        apps.push(AppRef { id, name });
    }
    let mut sites: Vec<String> = Vec::new();
    for s in d.sites {
        if let Some(h) = tidy_site(&s)? {
            if !sites.contains(&h) {
                sites.push(h);
            }
        }
    }
    Ok(RuleDraft {
        name: d.name.trim().to_owned(),
        prompt: d.prompt.trim().to_owned(),
        apps,
        sites,
        ..d
    })
}

fn check_name(name: &str) -> Result<(), RulesError> {
    if name.is_empty() {
        return Err(RulesError::NameEmpty);
    }
    if char_len(name) > MAX_NAME_CHARS {
        return Err(RulesError::NameTooLong);
    }
    Ok(())
}

/// Validate the whole set as it would be after a change.
fn check_set(rules: &[FormattingRule]) -> Result<(), RulesError> {
    RuleSet {
        rules: rules.iter().map(FormattingRule::rule).collect(),
    }
    .validate()?;
    Ok(())
}

impl Store {
    /// Insert the default rules the first time rules are read.
    fn seed_rules(&self, now: i64) -> Result<(), StoreError> {
        let count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM formatting_rules", [], |r| r.get(0))?;
        if count > 0 {
            return Ok(());
        }
        let tx = self.conn.unchecked_transaction()?;
        for (pos, r) in RuleSet::default().rules.iter().enumerate() {
            let apps: Vec<AppRef> = r
                .apps
                .iter()
                .map(|a| AppRef {
                    id: a.clone(),
                    name: a.clone(),
                })
                .collect();
            tx.execute(
                "INSERT INTO formatting_rules (id, kind, name, enabled, style, raw, prompt, apps, sites, position, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    kind_str(r.kind),
                    r.name,
                    r.enabled,
                    style_str(r.style),
                    r.raw,
                    r.prompt,
                    serde_json::to_string(&apps).expect("json"),
                    serde_json::to_string(&r.sites).expect("json"),
                    pos as i64,
                    now,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Live rules in page order: All apps, Email, then custom rules oldest first.
    pub fn formatting_rules(&self, now: i64) -> Result<Vec<FormattingRule>, StoreError> {
        self.seed_rules(now)?;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {RULE_COLUMNS} FROM formatting_rules WHERE deleted_at IS NULL
             ORDER BY CASE kind WHEN 'all_apps' THEN 0 WHEN 'email' THEN 1 ELSE 2 END, position, rowid"
        ))?;
        let rows = stmt.query_map([], rule_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The rules in the shape the formatting pipeline and the gateway use.
    pub fn rule_set(&self, now: i64) -> Result<RuleSet, StoreError> {
        Ok(RuleSet {
            rules: self.formatting_rules(now)?.iter().map(FormattingRule::rule).collect(),
        })
    }

    fn rule_write(&self, r: &FormattingRule, insert: bool) -> Result<(), StoreError> {
        let apps = serde_json::to_string(&r.apps).expect("json");
        let sites = serde_json::to_string(&r.sites).expect("json");
        if insert {
            self.conn.execute(
                "INSERT INTO formatting_rules (id, kind, name, enabled, style, raw, prompt, apps, sites, position, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    r.id,
                    kind_str(r.kind),
                    r.name,
                    r.enabled,
                    style_str(r.style),
                    r.raw,
                    r.prompt,
                    apps,
                    sites,
                    r.position,
                    r.created_at,
                    r.updated_at
                ],
            )?;
        } else {
            self.conn.execute(
                "UPDATE formatting_rules SET name = ?2, enabled = ?3, style = ?4, raw = ?5, prompt = ?6,
                 apps = ?7, sites = ?8, updated_at = ?9 WHERE id = ?1 AND deleted_at IS NULL",
                params![
                    r.id,
                    r.name,
                    r.enabled,
                    style_str(r.style),
                    r.raw,
                    r.prompt,
                    apps,
                    sites,
                    r.updated_at
                ],
            )?;
        }
        Ok(())
    }

    /// Add a custom rule at the end of the list.
    pub fn rule_add(&self, draft: RuleDraft, now: i64) -> Result<FormattingRule, RulesError> {
        let d = tidy(draft)?;
        check_name(&d.name)?;
        let mut all = self.formatting_rules(now)?;
        let position = all.iter().map(|r| r.position).max().unwrap_or(0) + 1;
        let rule = FormattingRule {
            id: uuid::Uuid::new_v4().to_string(),
            kind: RuleKind::Custom,
            name: d.name,
            enabled: d.enabled,
            style: d.style,
            raw: d.raw,
            prompt: d.prompt,
            apps: d.apps,
            sites: d.sites,
            position,
            created_at: now,
            updated_at: now,
        };
        all.push(rule.clone());
        check_set(&all)?;
        self.rule_write(&rule, true)?;
        Ok(rule)
    }

    /// Replace a rule's editable fields. Built-in rules keep their names.
    pub fn rule_update(&self, id: &str, draft: RuleDraft, now: i64) -> Result<FormattingRule, RulesError> {
        let d = tidy(draft)?;
        let mut all = self.formatting_rules(now)?;
        let r = all.iter_mut().find(|r| r.id == id).ok_or(StoreError::NotFound)?;
        if r.kind == RuleKind::Custom {
            check_name(&d.name)?;
            r.name = d.name;
        }
        r.enabled = d.enabled;
        r.style = d.style;
        r.raw = d.raw;
        r.prompt = d.prompt;
        r.apps = d.apps;
        r.sites = d.sites;
        r.updated_at = now;
        let rule = r.clone();
        check_set(&all)?;
        self.rule_write(&rule, false)?;
        Ok(rule)
    }

    /// The per-rule on/off switch. All apps is always on.
    pub fn rule_set_enabled(&self, id: &str, enabled: bool, now: i64) -> Result<FormattingRule, RulesError> {
        let mut all = self.formatting_rules(now)?;
        let r = all.iter_mut().find(|r| r.id == id).ok_or(StoreError::NotFound)?;
        r.enabled = enabled;
        r.updated_at = now;
        let rule = r.clone();
        check_set(&all)?;
        self.rule_write(&rule, false)?;
        Ok(rule)
    }

    /// Delete a custom rule (a tombstone, for sync).
    pub fn rule_delete(&self, id: &str, now: i64) -> Result<(), RulesError> {
        let all = self.formatting_rules(now)?;
        let r = all.iter().find(|r| r.id == id).ok_or(StoreError::NotFound)?;
        match r.kind {
            RuleKind::AllApps => return Err(RulesError::DeleteAllApps),
            RuleKind::Email => return Err(RulesError::DeleteEmail),
            RuleKind::Custom => {}
        }
        self.conn.execute(
            "UPDATE formatting_rules SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
            params![id, now],
        )?;
        Ok(())
    }

    /// Add a preset from the gallery for apps on `os`. Apps and sites that
    /// another rule owns are skipped and reported; if nothing is left, no rule
    /// is added. The Email preset fills in the built-in Email rule.
    pub fn rule_add_preset(&self, preset_id: &str, os: Option<Os>, now: i64) -> Result<PresetOutcome, RulesError> {
        let preset: &Preset = presets::find(preset_id).ok_or_else(|| RulesError::UnknownPreset(preset_id.into()))?;
        let all = self.formatting_rules(now)?;
        let set = RuleSet {
            rules: all.iter().map(FormattingRule::rule).collect(),
        };
        let applied = presets::apply(preset, &set, os, &uuid::Uuid::new_v4().to_string());
        let existing = all.iter().find(|r| r.id == applied.rule.id);
        if existing.is_none() && applied.is_empty() {
            return Ok(PresetOutcome {
                rule: None,
                skipped: applied.skipped,
                kept_prompt: false,
            });
        }
        let mut apps = existing.map(|r| r.apps.clone()).unwrap_or_default();
        apps.extend(applied.added_apps.iter().map(|(id, name)| AppRef {
            id: id.clone(),
            name: name.clone(),
        }));
        let r = &applied.rule;
        let draft = RuleDraft {
            name: r.name.clone(),
            enabled: r.enabled,
            style: r.style,
            raw: r.raw,
            prompt: r.prompt.clone(),
            apps,
            sites: r.sites.clone(),
        };
        let rule = match existing {
            Some(e) => self.rule_update(&e.id.clone(), draft, now)?,
            None => self.rule_add(draft, now)?,
        };
        Ok(PresetOutcome {
            rule: Some(rule),
            skipped: applied.skipped,
            kept_prompt: applied.kept_prompt,
        })
    }

    /// Up to 30 apps from the last 90 days of records, most dictations first.
    /// Browsers and this app are left out; checking that an app is still
    /// installed is up to the caller (it is platform-specific).
    pub fn most_used_apps(&self, now: i64) -> Result<Vec<UsedApp>, StoreError> {
        let since = now - MOST_USED_DAYS * 86_400_000;
        // With MAX(), SQLite takes the bare `app_name` from the newest row.
        let mut stmt = self.conn.prepare(
            "SELECT app_id, app_name, count(*) AS n, MAX(created_at) FROM records
             WHERE created_at >= ?1 AND app_id IS NOT NULL AND trim(app_id) != ''
             GROUP BY app_id COLLATE NOCASE ORDER BY n DESC, MAX(created_at) DESC",
        )?;
        let rows = stmt.query_map([since], |r| {
            let id: String = r.get(0)?;
            let name: Option<String> = r.get(1)?;
            Ok(UsedApp {
                name: name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| id.clone()),
                id,
                count: r.get(2)?,
            })
        })?;
        let mut out = Vec::new();
        for a in rows {
            let a = a?;
            if vocalis_core::apps::is_excluded(&a.id, &a.name) {
                continue;
            }
            out.push(a);
            if out.len() == MOST_USED_LIMIT {
                break;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NewRecord;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let (s, _) = Store::open(&dir.path().join("h.db"), &[7; 32]).unwrap();
        (dir, s)
    }

    fn app(id: &str, name: &str) -> AppRef {
        AppRef {
            id: id.into(),
            name: name.into(),
        }
    }

    fn draft(name: &str) -> RuleDraft {
        RuleDraft {
            name: name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn seeded_lazily_with_the_defaults() {
        let (_d, s) = store();
        let rules = s.formatting_rules(1).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!((rules[0].kind, rules[0].name.as_str()), (RuleKind::AllApps, "All apps"));
        assert_eq!(rules[1].kind, RuleKind::Email);
        assert_eq!(rules[1].sites, ["mail.google.com", "outlook.live.com"]);
        assert!(rules[1].apps.is_empty());
        // Reading again does not seed twice.
        assert_eq!(s.formatting_rules(2).unwrap(), rules);
        let set = s.rule_set(3).unwrap();
        set.validate().unwrap();
        assert_eq!(set.rules.len(), 2);
    }

    #[test]
    fn add_update_toggle_delete() {
        let (_d, s) = store();
        let mut d = draft("  Code  ");
        d.apps = vec![app(" com.microsoft.VSCode ", ""), app("COM.MICROSOFT.VSCODE", "dup")];
        d.sites = vec!["https://www.GitHub.com/org".into(), "github.com".into(), " ".into()];
        d.prompt = " Identifiers as code. ".into();
        let r = s.rule_add(d, 10).unwrap();
        assert_eq!(r.name, "Code");
        assert_eq!(r.apps, [app("com.microsoft.VSCode", "com.microsoft.VSCode")]);
        assert_eq!(r.sites, ["github.com"]);
        assert_eq!(r.prompt, "Identifiers as code.");
        assert_eq!(s.formatting_rules(11).unwrap().last().unwrap(), &r);

        let mut d2 = draft("Code editors");
        d2.style = Style::Lowercase;
        d2.raw = true;
        let u = s.rule_update(&r.id, d2, 12).unwrap();
        assert_eq!(
            (u.name.as_str(), u.style, u.raw, u.apps.len()),
            ("Code editors", Style::Lowercase, true, 0)
        );
        assert_eq!(u.updated_at, 12);
        assert_eq!(u.created_at, 10);

        let off = s.rule_set_enabled(&r.id, false, 13).unwrap();
        assert!(!off.enabled);
        assert!(!s.rule_set(14).unwrap().rules[2].enabled);

        s.rule_delete(&r.id, 15).unwrap();
        assert_eq!(s.formatting_rules(16).unwrap().len(), 2);
        assert!(matches!(
            s.rule_delete(&r.id, 17),
            Err(RulesError::Store(StoreError::NotFound))
        ));
        let tomb: i64 = s
            .conn
            .query_row("SELECT count(*) FROM formatting_rules WHERE deleted_at = 15", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(tomb, 1);
    }

    #[test]
    fn one_owner_per_app_and_site() {
        let (_d, s) = store();
        let mut a = draft("Chat");
        a.apps = vec![app("Slack", "Slack")];
        s.rule_add(a, 1).unwrap();
        let mut b = draft("Other");
        b.apps = vec![app("slack", "Slack")];
        let err = s.rule_add(b, 2).unwrap_err();
        assert_eq!(err.to_string(), "slack is already used by \"Chat\"");
        // The built-in Email rule owns its webmail sites.
        let mut c = draft("Gmail");
        c.sites = vec!["mail.google.com/mail/u/0".into()];
        assert!(matches!(
            s.rule_add(c, 3),
            Err(RulesError::Rule(RuleError::AlreadyUsed { ref owner, .. })) if owner == "Email"
        ));
        assert_eq!(s.formatting_rules(4).unwrap().len(), 3);
    }

    #[test]
    fn builtin_rules_are_protected() {
        let (_d, s) = store();
        let rules = s.formatting_rules(1).unwrap();
        let (all, email) = (&rules[0], &rules[1]);
        assert!(matches!(
            s.rule_set_enabled(&all.id, false, 2),
            Err(RulesError::Rule(RuleError::AllAppsDisabled))
        ));
        assert!(matches!(s.rule_delete(&all.id, 2), Err(RulesError::DeleteAllApps)));
        assert!(matches!(s.rule_delete(&email.id, 2), Err(RulesError::DeleteEmail)));
        // Email can be switched off; built-in names don't change.
        assert!(!s.rule_set_enabled(&email.id, false, 3).unwrap().enabled);
        let mut d = draft("Renamed");
        d.prompt = "Be brief.".into();
        d.style = Style::Casual;
        let u = s.rule_update(&all.id, d, 4).unwrap();
        assert_eq!(
            (u.name.as_str(), u.prompt.as_str(), u.style),
            ("All apps", "Be brief.", Style::Casual)
        );
        assert_eq!(s.formatting_rules(5).unwrap().len(), 2);
    }

    #[test]
    fn prompt_names_and_sites_are_checked() {
        let (_d, s) = store();
        let mut d = draft("Long");
        d.prompt = "x".repeat(501);
        assert!(matches!(
            s.rule_add(d.clone(), 1),
            Err(RulesError::Rule(RuleError::PromptTooLong(_)))
        ));
        d.prompt = "é".repeat(500);
        s.rule_add(d, 1).unwrap();
        assert!(matches!(s.rule_add(draft("   "), 2), Err(RulesError::NameEmpty)));
        assert!(matches!(
            s.rule_add(draft(&"n".repeat(81)), 2),
            Err(RulesError::NameTooLong)
        ));
        let mut d = draft("Sites");
        d.sites = vec!["not a site".into()];
        assert!(matches!(s.rule_add(d, 3), Err(RulesError::BadSite(ref x)) if x == "not a site"));
        assert!(matches!(
            s.rule_update("missing", draft("x"), 4),
            Err(RulesError::Store(StoreError::NotFound))
        ));
    }

    #[test]
    fn presets_respect_ownership_and_fill_email() {
        let (_d, s) = store();
        let ai = s.rule_add_preset("ai-chat", Some(Os::Macos), 1).unwrap();
        let rule = ai.rule.unwrap();
        assert_eq!(rule.name, "AI chat");
        assert_eq!(rule.apps, [app("com.openai.chat", "ChatGPT")]);
        assert!(ai.skipped.is_empty());

        let ff = s.rule_add_preset("friendly-filter", Some(Os::Macos), 2).unwrap();
        assert!(ff.rule.is_none());
        assert_eq!(ff.skipped.len(), 4);
        assert!(ff.skipped.iter().all(|k| k.owner == "AI chat"));
        assert_eq!(s.formatting_rules(3).unwrap().len(), 3);

        let email = s.rule_add_preset("email", Some(Os::Macos), 4).unwrap().rule.unwrap();
        assert_eq!(email.kind, RuleKind::Email);
        assert_eq!(email.sites, ["mail.google.com", "outlook.live.com"]);
        assert_eq!(email.apps[0], app("com.apple.mail", "Mail"));
        assert_eq!(s.formatting_rules(5).unwrap().len(), 3);

        let t = s.rule_add_preset("terminal", Some(Os::Linux), 6).unwrap().rule.unwrap();
        assert!(t.raw);
        assert!(t.apps.iter().any(|a| a.id == "konsole"));
        s.rule_set(7).unwrap().validate().unwrap();
        assert!(matches!(
            s.rule_add_preset("nope", None, 8),
            Err(RulesError::UnknownPreset(_))
        ));
    }

    #[test]
    fn most_used_apps_last_90_days_without_browsers() {
        let (_d, s) = store();
        const DAY: i64 = 86_400_000;
        let now = 200 * DAY;
        let add = |id: &str, name: &str, at: i64| {
            s.insert(NewRecord {
                text: "x".into(),
                app_id: Some(id.into()),
                app_name: Some(name.into()),
                created_at: at,
                ..Default::default()
            })
            .unwrap();
        };
        for i in 0..3 {
            add("com.tinyspeck.slackmacgap", "Slack", now - i * DAY);
        }
        add("COM.TINYSPECK.SLACKMACGAP", "Slack Beta", now + 1);
        add("com.microsoft.VSCode", "Code", now - DAY);
        add("com.microsoft.VSCode", "Code", now - 2 * DAY);
        add("com.google.Chrome", "Google Chrome", now);
        add("com.google.Chrome", "Google Chrome", now);
        add("com.google.Chrome", "Google Chrome", now);
        add("com.google.Chrome", "Google Chrome", now);
        add("com.google.Chrome", "Google Chrome", now);
        add("app.vocalis.desktop", "Vocalis", now);
        add("com.apple.Notes", "Notes", now - 91 * DAY);
        let used = s.most_used_apps(now).unwrap();
        let names: Vec<_> = used.iter().map(|a| (a.name.as_str(), a.count)).collect();
        assert_eq!(names, [("Slack Beta", 4), ("Code", 2)]);

        for i in 0..40 {
            add(&format!("com.example.app{i}"), &format!("App {i}"), now);
        }
        assert_eq!(s.most_used_apps(now).unwrap().len(), MOST_USED_LIMIT);
    }

    #[test]
    fn rules_survive_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("h.db");
        let id = {
            let (s, _) = Store::open(&path, &[7; 32]).unwrap();
            let mut d = draft("Keep");
            d.apps = vec![app("Slack", "Slack")];
            s.rule_add(d, 1).unwrap().id
        };
        let (s, _) = Store::open(&path, &[7; 32]).unwrap();
        let rules = s.formatting_rules(2).unwrap();
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[2].id, id);
    }
}
