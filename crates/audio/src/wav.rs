//! Minimal 16-bit PCM mono WAV encoding and decoding (for retry uploads and downloads).

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WavError {
    #[error("not a 16-bit PCM mono WAV file")]
    Unsupported,
    #[error("the WAV file is truncated")]
    Truncated,
}

pub fn encode(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// Decode a file produced by [`encode`] (or any canonical 16-bit mono PCM WAV).
pub fn decode(bytes: &[u8]) -> Result<(Vec<i16>, u32), WavError> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(WavError::Unsupported);
    }
    let mut i = 12;
    let mut rate = None;
    while i + 8 <= bytes.len() {
        let id = &bytes[i..i + 4];
        let len = u32::from_le_bytes(bytes[i + 4..i + 8].try_into().unwrap()) as usize;
        let body = bytes.get(i + 8..i + 8 + len).ok_or(WavError::Truncated)?;
        match id {
            b"fmt " => {
                if len < 16 {
                    return Err(WavError::Unsupported);
                }
                let fmt = u16::from_le_bytes([body[0], body[1]]);
                let ch = u16::from_le_bytes([body[2], body[3]]);
                let bits = u16::from_le_bytes([body[14], body[15]]);
                if fmt != 1 || ch != 1 || bits != 16 {
                    return Err(WavError::Unsupported);
                }
                rate = Some(u32::from_le_bytes(body[4..8].try_into().unwrap()));
            }
            b"data" => {
                let rate = rate.ok_or(WavError::Unsupported)?;
                let samples = body.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
                return Ok((samples, rate));
            }
            _ => {}
        }
        i += 8 + len + (len & 1);
    }
    Err(WavError::Truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let s: Vec<i16> = (0..1000).map(|i| (i * 37 % 65_000 - 32_000) as i16).collect();
        let bytes = encode(&s, 16_000);
        assert_eq!(bytes.len(), 44 + 2000);
        assert_eq!(decode(&bytes), Ok((s, 16_000)));
    }

    #[test]
    fn rejects_garbage_and_truncation() {
        assert_eq!(decode(b"nope"), Err(WavError::Unsupported));
        let bytes = encode(&[1, 2, 3], 16_000);
        assert_eq!(decode(&bytes[..bytes.len() - 1]), Err(WavError::Truncated));
    }
}
