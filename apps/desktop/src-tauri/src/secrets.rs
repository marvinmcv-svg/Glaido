//! Per-install encryption keys in the OS keychain (§5.2: "a per-install key in the keychain").
//!
//! If no keychain service is reachable (e.g. a Linux session without a Secret
//! Service), the key falls back to a file readable only by the user, and a
//! warning is logged. That fallback is weaker and is surfaced in settings (P2).

use std::path::Path;

const SERVICE: &str = "app.vocalis.desktop";

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

/// Load the named key, creating it on first use.
pub fn key(name: &str, fallback_dir: &Path) -> [u8; 32] {
    if let Ok(entry) = keyring::Entry::new(SERVICE, name) {
        match entry.get_password() {
            Ok(s) => {
                if let Some(k) = from_hex(&s) {
                    return k;
                }
            }
            Err(keyring::Error::NoEntry) => {
                let k = vocalis_audio::journal::generate_key();
                if entry.set_password(&to_hex(&k)).is_ok() {
                    return k;
                }
            }
            Err(_) => {}
        }
    }
    tracing::warn!(target: "secrets", key = name, "OS keychain unavailable; using a user-only key file");
    file_key(name, fallback_dir)
}

fn file_key(name: &str, dir: &Path) -> [u8; 32] {
    let path = dir.join(format!("{name}.key"));
    if let Some(k) = std::fs::read_to_string(&path).ok().and_then(|s| from_hex(&s)) {
        return k;
    }
    let k = vocalis_audio::journal::generate_key();
    let _ = std::fs::create_dir_all(dir);
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
        {
            let _ = f.write_all(to_hex(&k).as_bytes());
        }
    }
    #[cfg(not(unix))]
    {
        let _ = std::fs::write(&path, to_hex(&k));
    }
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let k = [0xABu8; 32];
        assert_eq!(from_hex(&to_hex(&k)), Some(k));
        assert_eq!(from_hex("zz"), None);
    }

    #[test]
    fn file_key_is_stable() {
        let dir = std::env::temp_dir().join(format!("vocalis-keytest-{}", std::process::id()));
        let a = file_key("t", &dir);
        assert_eq!(a, file_key("t", &dir));
        let _ = std::fs::remove_dir_all(dir);
    }
}
