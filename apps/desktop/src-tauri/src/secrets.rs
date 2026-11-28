//! Per-install encryption keys and the sign-in session, in the OS keychain
//! (§5.2: "a per-install key in the keychain").
//!
//! If no keychain is reachable (e.g. a Linux session without a Secret
//! Service), values fall back to files readable only by the user, and a
//! warning is logged. Two rules keep History readable across changes:
//!
//! * **Migration.** When a keychain shows up later, values found in fallback
//!   files are moved into it (written, read back, then the file is removed),
//!   instead of new keys being created.
//! * **No forked keys.** A `keys/store` marker records where the keys live.
//!   If it says "keychain" and the keychain can't be reached (locked, service
//!   still starting), loading fails with [`KeyError::KeychainUnavailable`]
//!   rather than inventing new keys that would make History unreadable.

use std::path::Path;

const SERVICE: &str = "app.vocalis.desktop";
const MARKER: &str = "store";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeyError {
    #[error("Your system keychain couldn't be opened. Unlock it (or sign in to your desktop session), then restart Vocalis.")]
    KeychainUnavailable,
}

/// The OS keychain, behind a trait so the rules above can be tested.
pub trait Vault {
    /// `Ok(None)`: reachable, no such entry. `Err`: not reachable.
    fn get(&self, name: &str) -> Result<Option<String>, ()>;
    fn set(&self, name: &str, value: &str) -> bool;
    fn delete(&self, name: &str);
}

pub struct OsVault;

impl Vault for OsVault {
    fn get(&self, name: &str) -> Result<Option<String>, ()> {
        let entry = keyring::Entry::new(SERVICE, name).map_err(|_| ())?;
        match entry.get_password() {
            Ok(s) => Ok(Some(s)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(()),
        }
    }
    fn set(&self, name: &str, value: &str) -> bool {
        keyring::Entry::new(SERVICE, name).is_ok_and(|e| e.set_password(value).is_ok())
    }
    fn delete(&self, name: &str) {
        if let Ok(e) = keyring::Entry::new(SERVICE, name) {
            let _ = e.delete_credential();
        }
    }
}

fn to_hex(k: &[u8; 32]) -> String {
    k.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(s: &str) -> Option<[u8; 32]> {
    let s = s.trim();
    if s.len() != 64 {
        return None;
    }
    let mut k = [0u8; 32];
    for (i, byte) in k.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(k)
}

fn read_marker(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join(MARKER))
        .ok()
        .map(|s| s.trim().to_owned())
}

fn write_marker(dir: &Path, where_: &str) {
    write_private(&dir.join(MARKER), where_.as_bytes());
}

/// Move a fallback file's value into the vault. Returns `true` if it is now
/// safely in the vault (and the file is gone).
fn migrate(vault: &dyn Vault, name: &str, value: &str, file: &Path) -> bool {
    if vault.set(name, value) && vault.get(name) == Ok(Some(value.to_owned())) {
        let _ = std::fs::remove_file(file);
        tracing::info!(target: "secrets", key = name, "moved from the fallback file into the keychain");
        true
    } else {
        false
    }
}

/// Load the named key, creating it on first use.
pub fn key(name: &str, dir: &Path) -> Result<[u8; 32], KeyError> {
    key_in(&OsVault, name, dir)
}

fn key_in(vault: &dyn Vault, name: &str, dir: &Path) -> Result<[u8; 32], KeyError> {
    let file = dir.join(format!("{name}.key"));
    let from_file = std::fs::read_to_string(&file).ok().and_then(|s| from_hex(&s));
    match vault.get(name) {
        Ok(Some(hex)) if from_hex(&hex).is_some() => {
            write_marker(dir, "keychain");
            Ok(from_hex(&hex).expect("checked"))
        }
        // Reachable but empty (or garbage): adopt the fallback file if there
        // is one, else create a key.
        Ok(_) => {
            let k = from_file.unwrap_or_else(vocalis_audio::journal::generate_key);
            if migrate(vault, name, &to_hex(&k), &file) {
                write_marker(dir, "keychain");
            } else {
                tracing::warn!(target: "secrets", key = name, "keychain refused the key; using a user-only key file");
                write_private(&file, to_hex(&k).as_bytes());
                write_marker(dir, "file");
            }
            Ok(k)
        }
        Err(()) => {
            if let Some(k) = from_file {
                return Ok(k);
            }
            if read_marker(dir).as_deref() == Some("keychain") {
                return Err(KeyError::KeychainUnavailable);
            }
            tracing::warn!(target: "secrets", key = name, "OS keychain unavailable; using a user-only key file");
            let k = vocalis_audio::journal::generate_key();
            write_private(&file, to_hex(&k).as_bytes());
            write_marker(dir, "file");
            Ok(k)
        }
    }
}

/// A string secret (the sign-in session), in the keychain when one is
/// reachable, else in a user-only file under `fallback_dir`.
pub fn get_secret(name: &str, fallback_dir: &Path) -> Option<String> {
    get_secret_in(&OsVault, name, fallback_dir)
}

fn get_secret_in(vault: &dyn Vault, name: &str, dir: &Path) -> Option<String> {
    let file = dir.join(format!("{name}.secret"));
    match vault.get(name) {
        Ok(Some(s)) => Some(s),
        Ok(None) => {
            let s = std::fs::read_to_string(&file).ok()?;
            migrate(vault, name, &s, &file);
            Some(s)
        }
        Err(()) => std::fs::read_to_string(&file).ok(),
    }
}

pub fn set_secret(name: &str, value: &str, fallback_dir: &Path) {
    set_secret_in(&OsVault, name, value, fallback_dir);
}

fn set_secret_in(vault: &dyn Vault, name: &str, value: &str, dir: &Path) {
    let file = dir.join(format!("{name}.secret"));
    if vault.set(name, value) {
        let _ = std::fs::remove_file(file);
        return;
    }
    tracing::warn!(target: "secrets", key = name, "OS keychain unavailable; using a user-only file");
    write_private(&file, value.as_bytes());
}

pub fn delete_secret(name: &str, fallback_dir: &Path) {
    OsVault.delete(name);
    let _ = std::fs::remove_file(fallback_dir.join(format!("{name}.secret")));
}

fn write_private(path: &Path, bytes: &[u8]) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
        {
            let _ = f.write_all(bytes);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = std::fs::write(path, bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// In-memory keychain that can be switched off.
    #[derive(Default)]
    struct FakeVault {
        items: RefCell<HashMap<String, String>>,
        down: RefCell<bool>,
    }

    impl FakeVault {
        fn down() -> Self {
            let v = Self::default();
            *v.down.borrow_mut() = true;
            v
        }
        fn up(&self) {
            *self.down.borrow_mut() = false;
        }
    }

    impl Vault for FakeVault {
        fn get(&self, name: &str) -> Result<Option<String>, ()> {
            if *self.down.borrow() {
                return Err(());
            }
            Ok(self.items.borrow().get(name).cloned())
        }
        fn set(&self, name: &str, value: &str) -> bool {
            if *self.down.borrow() {
                return false;
            }
            self.items.borrow_mut().insert(name.into(), value.into());
            true
        }
        fn delete(&self, name: &str) {
            self.items.borrow_mut().remove(name);
        }
    }

    #[test]
    fn hex_round_trip() {
        let k = [0xABu8; 32];
        assert_eq!(from_hex(&to_hex(&k)), Some(k));
        assert_eq!(from_hex("zz"), None);
    }

    #[test]
    fn keychain_keys_are_created_once_and_reused() {
        let dir = tempfile::tempdir().unwrap();
        let v = FakeVault::default();
        let a = key_in(&v, "audio", dir.path()).unwrap();
        assert_eq!(key_in(&v, "audio", dir.path()).unwrap(), a);
        assert!(
            !dir.path().join("audio.key").exists(),
            "no file when the keychain works"
        );
        assert_eq!(read_marker(dir.path()).as_deref(), Some("keychain"));
    }

    #[test]
    fn file_fallback_is_stable_while_no_keychain() {
        let dir = tempfile::tempdir().unwrap();
        let v = FakeVault::down();
        let a = key_in(&v, "audio", dir.path()).unwrap();
        assert_eq!(key_in(&v, "audio", dir.path()).unwrap(), a);
        assert!(dir.path().join("audio.key").exists());
        assert_eq!(read_marker(dir.path()).as_deref(), Some("file"));
    }

    #[test]
    fn a_keychain_that_appears_later_adopts_the_file_key() {
        let dir = tempfile::tempdir().unwrap();
        let v = FakeVault::down();
        let original = key_in(&v, "history", dir.path()).unwrap();
        v.up();
        assert_eq!(
            key_in(&v, "history", dir.path()).unwrap(),
            original,
            "same key, History stays readable"
        );
        assert!(!dir.path().join("history.key").exists(), "file removed after migration");
        assert_eq!(v.items.borrow().get("history"), Some(&to_hex(&original)));
        assert_eq!(read_marker(dir.path()).as_deref(), Some("keychain"));
    }

    #[test]
    fn a_locked_keychain_never_forks_new_keys() {
        let dir = tempfile::tempdir().unwrap();
        let v = FakeVault::default();
        let k = key_in(&v, "history", dir.path()).unwrap();
        *v.down.borrow_mut() = true;
        assert_eq!(key_in(&v, "history", dir.path()), Err(KeyError::KeychainUnavailable));
        assert!(!dir.path().join("history.key").exists());
        v.up();
        assert_eq!(key_in(&v, "history", dir.path()).unwrap(), k);
    }

    #[test]
    fn session_secret_migrates_into_a_new_keychain() {
        let dir = tempfile::tempdir().unwrap();
        let v = FakeVault::down();
        set_secret_in(&v, "session", "{\"refreshToken\":\"r\"}", dir.path());
        assert!(dir.path().join("session.secret").exists());
        v.up();
        assert_eq!(
            get_secret_in(&v, "session", dir.path()).as_deref(),
            Some("{\"refreshToken\":\"r\"}")
        );
        assert!(!dir.path().join("session.secret").exists());
        assert_eq!(
            get_secret_in(&v, "session", dir.path()).as_deref(),
            Some("{\"refreshToken\":\"r\"}")
        );
    }
}
