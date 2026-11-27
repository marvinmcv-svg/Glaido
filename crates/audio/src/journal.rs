//! Crash-safe encrypted audio journal (`.wav.enc`), master prompt §5.2 and spec 4.6.
//!
//! Audio is appended in small chunks while recording, so a crash, power loss or
//! kill loses at most the chunk in flight. The same file is the long-term
//! encrypted recording used for playback, download and Retry.
//!
//! ## Format (version 1)
//! ```text
//! header  = "VCJ1" | version:u8 | sample_rate:u32le | nonce_prefix:[u8;8] | created_ms:u64le   (25 bytes, plaintext)
//! chunk*  = len:u32le | AES-256-GCM(ciphertext ‖ tag)
//! nonce   = nonce_prefix ‖ counter:u32be
//! aad     = header ‖ counter:u32be ‖ final:u8
//! ```
//! Each chunk holds 16-bit little-endian PCM. The header is authenticated by
//! every chunk; the counter prevents reordering and dropping chunks in the middle;
//! the `final` flag on the last chunk tells a complete recording from a
//! truncated one. The 32-byte key comes from the OS keychain (one per install).

use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::RngCore;
use thiserror::Error;

const MAGIC: &[u8; 4] = b"VCJ1";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 25;
/// Upper bound for one chunk, to reject garbage lengths without allocating.
const MAX_CHUNK: usize = 4 * 1024 * 1024;

pub type Key = [u8; 32];

/// Generate a new random key (stored in the keychain by the caller).
pub fn generate_key() -> Key {
    let mut k = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut k);
    k
}

#[derive(Debug, Error)]
pub enum JournalError {
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),
    #[error("not an audio journal, or an unsupported version")]
    BadHeader,
    #[error("the key does not match or the file is corrupt")]
    Undecryptable,
    #[error("the journal is already finished")]
    Finished,
}

pub struct JournalWriter {
    out: BufWriter<File>,
    cipher: Aes256Gcm,
    header: [u8; HEADER_LEN],
    prefix: [u8; 8],
    counter: u32,
    pending: Vec<i16>,
    chunk_samples: usize,
    finished: bool,
}

impl JournalWriter {
    /// Create a new journal. `chunk_samples` controls how much audio can be lost
    /// on a crash (8000 samples = 0.5 s at 16 kHz).
    pub fn create(
        path: &Path,
        key: &Key,
        sample_rate: u32,
        created_ms: u64,
        chunk_samples: usize,
    ) -> Result<Self, JournalError> {
        let mut prefix = [0u8; 8];
        rand::thread_rng().fill_bytes(&mut prefix);
        let mut header = [0u8; HEADER_LEN];
        header[0..4].copy_from_slice(MAGIC);
        header[4] = VERSION;
        header[5..9].copy_from_slice(&sample_rate.to_le_bytes());
        header[9..17].copy_from_slice(&prefix);
        header[17..25].copy_from_slice(&created_ms.to_le_bytes());

        let file = OpenOptions::new().write(true).create_new(true).open(path)?;
        let mut out = BufWriter::new(file);
        out.write_all(&header)?;
        out.flush()?;
        out.get_ref().sync_data()?;
        Ok(Self {
            out,
            cipher: Aes256Gcm::new_from_slice(key).expect("32-byte key"),
            header,
            prefix,
            counter: 0,
            pending: Vec::with_capacity(chunk_samples),
            chunk_samples: chunk_samples.max(1),
            finished: false,
        })
    }

    /// Append samples; full chunks are encrypted and flushed to disk immediately.
    pub fn append(&mut self, samples: &[i16]) -> Result<(), JournalError> {
        if self.finished {
            return Err(JournalError::Finished);
        }
        self.pending.extend_from_slice(samples);
        while self.pending.len() >= self.chunk_samples {
            let rest = self.pending.split_off(self.chunk_samples);
            let chunk = std::mem::replace(&mut self.pending, rest);
            self.write_chunk(&chunk, false)?;
        }
        Ok(())
    }

    /// Write remaining samples as the final chunk and close the journal.
    pub fn finish(mut self) -> Result<(), JournalError> {
        let chunk = std::mem::take(&mut self.pending);
        self.write_chunk(&chunk, true)?;
        self.finished = true;
        Ok(())
    }

    fn write_chunk(&mut self, samples: &[i16], last: bool) -> Result<(), JournalError> {
        let plain: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let nonce = nonce(&self.prefix, self.counter);
        let aad = aad(&self.header, self.counter, last);
        let ct = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), Payload { msg: &plain, aad: &aad })
            .map_err(|_| JournalError::Undecryptable)?;
        self.out.write_all(&(ct.len() as u32).to_le_bytes())?;
        self.out.write_all(&ct)?;
        self.out.flush()?;
        self.out.get_ref().sync_data()?;
        self.counter += 1;
        Ok(())
    }
}

fn nonce(prefix: &[u8; 8], counter: u32) -> [u8; 12] {
    let mut n = [0u8; 12];
    n[..8].copy_from_slice(prefix);
    n[8..].copy_from_slice(&counter.to_be_bytes());
    n
}

fn aad(header: &[u8; HEADER_LEN], counter: u32, last: bool) -> Vec<u8> {
    let mut a = Vec::with_capacity(HEADER_LEN + 5);
    a.extend_from_slice(header);
    a.extend_from_slice(&counter.to_be_bytes());
    a.push(last as u8);
    a
}

/// What could be read back from a journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recovered {
    pub samples: Vec<i16>,
    pub sample_rate: u32,
    pub created_ms: u64,
    /// The final chunk was present: the recording ended normally.
    pub complete: bool,
    /// Bytes after the last good chunk were ignored (crash mid-write, or damage).
    pub had_trailing_garbage: bool,
}

/// Read a journal, recovering every intact chunk. A truncated or damaged tail is
/// dropped; damage before any chunk could be read means a wrong key or a corrupt
/// file, which is an error.
pub fn read(path: &Path, key: &Key) -> Result<Recovered, JournalError> {
    let mut bytes = Vec::new();
    File::open(path)?.read_to_end(&mut bytes)?;
    read_bytes(&bytes, key)
}

pub fn read_bytes(bytes: &[u8], key: &Key) -> Result<Recovered, JournalError> {
    if bytes.len() < HEADER_LEN || &bytes[0..4] != MAGIC || bytes[4] != VERSION {
        return Err(JournalError::BadHeader);
    }
    let header: [u8; HEADER_LEN] = bytes[..HEADER_LEN].try_into().unwrap();
    let sample_rate = u32::from_le_bytes(header[5..9].try_into().unwrap());
    let prefix: [u8; 8] = header[9..17].try_into().unwrap();
    let created_ms = u64::from_le_bytes(header[17..25].try_into().unwrap());
    let cipher = Aes256Gcm::new_from_slice(key).expect("32-byte key");

    let mut samples = Vec::new();
    let mut pos = HEADER_LEN;
    let mut counter = 0u32;
    let mut complete = false;
    let mut garbage = false;
    while pos < bytes.len() {
        let Some(len_bytes) = bytes.get(pos..pos + 4) else {
            garbage = true;
            break;
        };
        let len = u32::from_le_bytes(len_bytes.try_into().unwrap()) as usize;
        let Some(ct) = bytes.get(pos + 4..pos + 4 + len).filter(|_| len <= MAX_CHUNK) else {
            garbage = true;
            break;
        };
        let n = nonce(&prefix, counter);
        let plain = [false, true].into_iter().find_map(|last| {
            cipher
                .decrypt(
                    Nonce::from_slice(&n),
                    Payload {
                        msg: ct,
                        aad: &aad(&header, counter, last),
                    },
                )
                .ok()
                .map(|p| (p, last))
        });
        let Some((plain, last)) = plain else {
            if counter == 0 {
                return Err(JournalError::Undecryptable);
            }
            garbage = true;
            break;
        };
        samples.extend(plain.as_chunks::<2>().0.iter().map(|c| i16::from_le_bytes(*c)));
        pos += 4 + len;
        counter += 1;
        if last {
            complete = true;
            garbage = pos < bytes.len();
            break;
        }
    }
    Ok(Recovered {
        samples,
        sample_rate,
        created_ms,
        complete,
        had_trailing_garbage: garbage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(n: usize) -> Vec<i16> {
        (0..n).map(|i| ((i * 7919) % 60_000) as i16).collect()
    }

    fn write(dir: &Path, key: &Key, data: &[i16], finish: bool) -> std::path::PathBuf {
        let p = dir.join("rec.wav.enc");
        let mut w = JournalWriter::create(&p, key, 16_000, 1_700_000_000_000, 1000).unwrap();
        for c in data.chunks(333) {
            w.append(c).unwrap();
        }
        if finish {
            w.finish().unwrap();
        } else {
            drop(w); // simulate a crash: the partial chunk in memory is lost
        }
        p
    }

    #[test]
    fn round_trip_complete() {
        let dir = tempfile::tempdir().unwrap();
        let key = generate_key();
        let data = samples(4321);
        let r = read(&write(dir.path(), &key, &data, true), &key).unwrap();
        assert_eq!(r.samples, data);
        assert_eq!(r.sample_rate, 16_000);
        assert_eq!(r.created_ms, 1_700_000_000_000);
        assert!(r.complete && !r.had_trailing_garbage);
    }

    #[test]
    fn crash_keeps_flushed_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let key = generate_key();
        let data = samples(4321);
        let r = read(&write(dir.path(), &key, &data, false), &key).unwrap();
        assert_eq!(r.samples, data[..4000], "only whole chunks survive");
        assert!(!r.complete);
    }

    #[test]
    fn torn_write_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let key = generate_key();
        let p = write(dir.path(), &key, &samples(3000), true);
        let mut bytes = std::fs::read(&p).unwrap();
        bytes.truncate(bytes.len() - 10); // power loss during the final chunk
        let r = read_bytes(&bytes, &key).unwrap();
        assert_eq!(r.samples.len(), 3000);
        assert!(!r.complete && r.had_trailing_garbage);
    }

    #[test]
    fn wrong_key_and_tampering_are_detected() {
        let dir = tempfile::tempdir().unwrap();
        let key = generate_key();
        let p = write(dir.path(), &key, &samples(2500), true);
        assert!(matches!(read(&p, &generate_key()), Err(JournalError::Undecryptable)));

        let mut bytes = std::fs::read(&p).unwrap();
        bytes[20] ^= 1; // header is authenticated by every chunk
        assert!(matches!(read_bytes(&bytes, &key), Err(JournalError::Undecryptable)));

        let mut bytes = std::fs::read(&p).unwrap();
        let second_chunk = HEADER_LEN + 4 + 2000 + 16 + 4 + 5;
        bytes[second_chunk] ^= 1; // damage in the middle: keep what came before
        let r = read_bytes(&bytes, &key).unwrap();
        assert_eq!(r.samples.len(), 1000);
        assert!(!r.complete);
    }

    #[test]
    fn chunks_cannot_be_reordered() {
        let dir = tempfile::tempdir().unwrap();
        let key = generate_key();
        let p = write(dir.path(), &key, &samples(3000), true);
        let bytes = std::fs::read(&p).unwrap();
        let chunk = 4 + 2000 + 16;
        let mut swapped = bytes[..HEADER_LEN].to_vec();
        swapped.extend_from_slice(&bytes[HEADER_LEN + chunk..HEADER_LEN + 2 * chunk]);
        swapped.extend_from_slice(&bytes[HEADER_LEN..HEADER_LEN + chunk]);
        swapped.extend_from_slice(&bytes[HEADER_LEN + 2 * chunk..]);
        assert!(matches!(read_bytes(&swapped, &key), Err(JournalError::Undecryptable)));
    }

    #[test]
    fn empty_recording_and_no_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let key = generate_key();
        let p = write(dir.path(), &key, &[], true);
        let r = read(&p, &key).unwrap();
        assert!(r.samples.is_empty() && r.complete);
        assert!(matches!(
            JournalWriter::create(&p, &key, 16_000, 0, 10),
            Err(JournalError::Io(_))
        ));
        assert!(matches!(
            read_bytes(b"VCJ0garbage-garbage-garbage", &key),
            Err(JournalError::BadHeader)
        ));
    }

    #[test]
    fn plaintext_is_not_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let key = generate_key();
        let data = vec![0x1234i16; 2000];
        let bytes = std::fs::read(write(dir.path(), &key, &data, true)).unwrap();
        let needle = [0x34u8, 0x12, 0x34, 0x12, 0x34, 0x12, 0x34, 0x12];
        assert!(!bytes.windows(needle.len()).any(|w| w == needle));
    }
}
