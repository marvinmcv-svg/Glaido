//! Desktop settings that the setup wizard and Settings write (Appendix B
//! keys), stored as `settings.json` in the app data directory. Hotkeys are
//! stored per platform, so a synced settings file works on every OS.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use vocalis_input::Chord;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Hotkeys {
    pub dictation: String,
    pub dictation_handsfree: String,
}

impl Hotkeys {
    /// Appendix A defaults for this OS.
    pub fn defaults() -> Self {
        let (dictation, handsfree) = if cfg!(target_os = "macos") {
            ("fn", "fn+space")
        } else if cfg!(target_os = "windows") {
            ("left_ctrl+left_meta", "left_ctrl+left_meta+shift")
        } else {
            ("left_ctrl+space", "left_ctrl+shift+space")
        };
        Self {
            dictation: dictation.into(),
            dictation_handsfree: handsfree.into(),
        }
    }

    /// Both chords, validated and distinct.
    pub fn chords(&self) -> Result<(Chord, Chord), String> {
        let a = Chord::parse(&self.dictation).map_err(|e| e.to_string())?;
        let b = Chord::parse(&self.dictation_handsfree).map_err(|e| e.to_string())?;
        if a == b {
            return Err("Dictation and hands-free dictation need different shortcuts.".into());
        }
        Ok((a, b))
    }
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self::defaults()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// `dictation.languages[]`: ISO 639-1 codes. Several = auto-detect among them.
    pub languages: Vec<String>,
    /// `dictation.microphone`: a device name, or `None` for the system default.
    pub microphone: Option<String>,
    /// `hotkeys.<platform>.*`, keyed by `macos` / `windows` / `linux`.
    pub hotkeys: std::collections::BTreeMap<String, Hotkeys>,
    /// The first-run wizard was finished (or skipped).
    pub setup_completed: bool,
    /// `dictation.paste_into_starting_app`: if the user switched apps while
    /// speaking, bring the starting app back and paste there.
    pub paste_into_starting_app: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            languages: system_languages(),
            microphone: None,
            hotkeys: Default::default(),
            setup_completed: false,
            paste_into_starting_app: true,
        }
    }
}

pub fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

/// The system locale's language (e.g. `de_DE.UTF-8` → `de`), else English.
pub fn system_languages() -> Vec<String> {
    let from_env = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty());
    vec![language_of(from_env.as_deref()).unwrap_or_else(|| "en".into())]
}

fn language_of(locale: Option<&str>) -> Option<String> {
    let lang = locale?.split(['_', '.', '-', '@']).next()?.to_ascii_lowercase();
    (lang.len() == 2 && lang.chars().all(|c| c.is_ascii_lowercase()) && lang != "c").then_some(lang)
}

fn valid_language(code: &str) -> bool {
    (2..=3).contains(&code.len()) && code.chars().all(|c| c.is_ascii_lowercase())
}

impl Settings {
    pub fn hotkeys(&self) -> Hotkeys {
        self.hotkeys.get(platform()).cloned().unwrap_or_default()
    }

    pub fn set_hotkeys(&mut self, h: Hotkeys) -> Result<(), String> {
        h.chords()?;
        self.hotkeys.insert(platform().into(), h);
        Ok(())
    }

    pub fn set_languages(&mut self, langs: Vec<String>) -> Result<(), String> {
        let mut clean: Vec<String> = Vec::new();
        for l in langs {
            let l = l.trim().to_ascii_lowercase();
            if !valid_language(&l) {
                return Err(format!("\"{l}\" is not a language code."));
            }
            if !clean.contains(&l) {
                clean.push(l);
            }
        }
        if clean.is_empty() {
            return Err("Keep at least one language selected.".into());
        }
        self.languages = clean;
        Ok(())
    }

    /// Fix anything a hand-edited or older file got wrong.
    fn sanitize(mut self) -> Self {
        if self.set_languages(self.languages.clone()).is_err() {
            self.languages = system_languages();
        }
        if self.hotkeys().chords().is_err() {
            self.hotkeys.remove(platform());
        }
        self
    }
}

pub struct SettingsFile {
    path: PathBuf,
}

impl SettingsFile {
    pub fn new(dir: &Path) -> Self {
        Self {
            path: dir.join("settings.json"),
        }
    }

    /// Missing or unreadable files give defaults; a damaged file is kept aside.
    pub fn load(&self) -> Settings {
        match std::fs::read(&self.path) {
            Ok(bytes) => match serde_json::from_slice::<Settings>(&bytes) {
                Ok(s) => s.sanitize(),
                Err(e) => {
                    tracing::warn!(target: "settings", error = %e, "settings file is damaged; using defaults");
                    let _ = std::fs::rename(&self.path, self.path.with_extension("json.damaged"));
                    Settings::default()
                }
            },
            Err(_) => Settings::default(),
        }
    }

    /// Atomic write: temp file then rename.
    pub fn save(&self, s: &Settings) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(s).expect("settings serialize"))?;
        std::fs::rename(tmp, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_to_language() {
        assert_eq!(language_of(Some("de_DE.UTF-8")).as_deref(), Some("de"));
        assert_eq!(language_of(Some("pt-BR")).as_deref(), Some("pt"));
        assert_eq!(language_of(Some("C.UTF-8")), None);
        assert_eq!(language_of(Some("POSIX")), None);
        assert_eq!(language_of(None), None);
    }

    #[test]
    fn defaults_are_valid_and_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let f = SettingsFile::new(dir.path());
        let s = f.load();
        assert!(!s.setup_completed);
        assert_eq!(s.languages.len(), 1);
        assert!(s.hotkeys().chords().is_ok());

        let mut s2 = s.clone();
        s2.setup_completed = true;
        s2.set_languages(vec!["EN".into(), "de".into(), "en".into()]).unwrap();
        s2.set_hotkeys(Hotkeys {
            dictation: "right_alt".into(),
            dictation_handsfree: "right_alt+space".into(),
        })
        .unwrap();
        s2.microphone = Some("USB Mic".into());
        f.save(&s2).unwrap();
        let back = f.load();
        assert_eq!(back, s2);
        assert_eq!(back.languages, vec!["en", "de"]);
    }

    #[test]
    fn rejects_bad_values() {
        let mut s = Settings::default();
        assert!(s.set_languages(vec![]).is_err());
        assert!(s.set_languages(vec!["english!".into()]).is_err());
        let same = Hotkeys {
            dictation: "fn".into(),
            dictation_handsfree: "fn".into(),
        };
        assert!(s.set_hotkeys(same).is_err());
        let bad = Hotkeys {
            dictation: "k".into(),
            dictation_handsfree: "fn".into(),
        };
        assert!(s.set_hotkeys(bad).is_err());
        assert_eq!(s.hotkeys(), Hotkeys::defaults());
    }

    #[test]
    fn damaged_or_hand_edited_files_recover() {
        let dir = tempfile::tempdir().unwrap();
        let f = SettingsFile::new(dir.path());
        std::fs::write(dir.path().join("settings.json"), b"{ not json").unwrap();
        assert_eq!(f.load(), Settings::default());
        assert!(dir.path().join("settings.json.damaged").exists());

        let edited = format!(
            r#"{{"languages":[],"hotkeys":{{"{}":{{"dictation":"k","dictationHandsfree":"fn"}}}},"setupCompleted":true,"unknownKey":1}}"#,
            platform()
        );
        std::fs::write(dir.path().join("settings.json"), edited).unwrap();
        let s = f.load();
        assert!(s.setup_completed);
        assert_eq!(s.languages, system_languages());
        assert_eq!(s.hotkeys(), Hotkeys::defaults());
    }
}
