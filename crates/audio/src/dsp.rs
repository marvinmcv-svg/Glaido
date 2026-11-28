//! Channel downmix, sample-rate conversion and format conversion.

/// Average interleaved channels into mono.
pub fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Streaming linear-interpolation resampler. Keeps state across calls so
/// chunk boundaries don't produce clicks or drift.
///
/// Linear interpolation is adequate for speech going to STT (which band-limits
/// to ~8 kHz anyway). A simple moving-average pre-filter reduces aliasing when
/// downsampling by more than 2×.
#[derive(Debug, Clone)]
pub struct Resampler {
    step: f64,
    /// Read position in the working buffer `[carry] + input`.
    pos: f64,
    /// Last input sample of the previous chunk (index 0 of the next working buffer).
    carry: Option<f32>,
    taps: usize,
    history: Vec<f32>,
}

impl Resampler {
    pub fn new(from_rate: u32, to_rate: u32) -> Self {
        assert!(from_rate > 0 && to_rate > 0, "sample rates must be positive");
        let step = from_rate as f64 / to_rate as f64;
        let taps = if step > 2.0 { step.floor() as usize } else { 1 };
        Self {
            step,
            pos: 0.0,
            carry: None,
            taps,
            history: Vec::new(),
        }
    }

    pub fn is_passthrough(&self) -> bool {
        (self.step - 1.0).abs() < f64::EPSILON
    }

    fn prefilter(&mut self, input: &[f32]) -> Vec<f32> {
        if self.taps <= 1 {
            return input.to_vec();
        }
        input
            .iter()
            .map(|&x| {
                self.history.push(x);
                if self.history.len() > self.taps {
                    self.history.remove(0);
                }
                self.history.iter().sum::<f32>() / self.history.len() as f32
            })
            .collect()
    }

    /// Convert one chunk of mono samples.
    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if self.is_passthrough() {
            return input.to_vec();
        }
        let filtered = self.prefilter(input);
        if filtered.is_empty() {
            return Vec::new();
        }
        let mut buf = Vec::with_capacity(filtered.len() + 1);
        buf.extend(self.carry);
        buf.extend_from_slice(&filtered);

        let mut out = Vec::with_capacity((buf.len() as f64 / self.step) as usize + 1);
        let last = buf.len() - 1;
        loop {
            let i = self.pos.floor() as usize;
            let frac = (self.pos - i as f64) as f32;
            if i > last || (i == last && frac > 0.0) {
                break; // the next chunk is needed to interpolate
            }
            let a = buf[i];
            let b = if i < last { buf[i + 1] } else { a };
            out.push(a + (b - a) * frac);
            self.pos += self.step;
        }
        // The last sample becomes index 0 of the next working buffer.
        self.pos -= last as f64;
        self.carry = Some(buf[last]);
        out
    }
}

/// Convert float samples in [-1, 1] to 16-bit PCM with clipping.
pub fn to_i16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16)
        .collect()
}

/// Convert 16-bit PCM to floats in [-1, 1].
pub fn from_i16(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|&s| s as f32 / i16::MAX as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn sine(rate: u32, freq: f32, secs: f32) -> Vec<f32> {
        let n = (rate as f32 * secs) as usize;
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin() * 0.5)
            .collect()
    }

    #[test]
    fn downmix_averages() {
        assert_eq!(downmix(&[1.0, 0.0, 0.5, 0.5], 2), vec![0.5, 0.5]);
        assert_eq!(downmix(&[0.1, 0.2], 1), vec![0.1, 0.2]);
    }

    #[test]
    fn resample_length_matches_ratio() {
        for from in [44_100u32, 48_000, 32_000, 8_000, 16_000] {
            let input = sine(from, 440.0, 1.0);
            let mut r = Resampler::new(from, 16_000);
            let out = r.process(&input);
            let expected = 16_000.0;
            assert!((out.len() as f64 - expected).abs() <= 2.0, "{from}: got {}", out.len());
        }
    }

    #[test]
    fn chunked_equals_whole() {
        let input = sine(48_000, 300.0, 0.5);
        let whole = Resampler::new(48_000, 16_000).process(&input);
        let mut r = Resampler::new(48_000, 16_000);
        let chunked: Vec<f32> = input.chunks(441).flat_map(|c| r.process(c)).collect();
        assert!((whole.len() as isize - chunked.len() as isize).abs() <= 1);
        let n = whole.len().min(chunked.len());
        let max_diff = whole[..n]
            .iter()
            .zip(&chunked[..n])
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(max_diff < 1e-3, "max diff {max_diff}");
    }

    #[test]
    fn preserves_tone_frequency() {
        // Count zero crossings of a 440 Hz tone after 44.1k → 16k.
        let out = Resampler::new(44_100, 16_000).process(&sine(44_100, 440.0, 1.0));
        let crossings = out.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        assert!((crossings as i32 - 880).abs() <= 4, "crossings {crossings}");
    }

    #[test]
    fn pcm_round_trip_and_clipping() {
        assert_eq!(to_i16(&[2.0, -2.0, 0.0]), vec![i16::MAX, -i16::MAX, 0]);
        let back = from_i16(&to_i16(&[0.25]));
        assert!((back[0] - 0.25).abs() < 1e-4);
    }

    proptest! {
        #[test]
        fn output_is_bounded(chunks in proptest::collection::vec(proptest::collection::vec(-1.0f32..1.0, 0..500), 0..20), from in 8_000u32..96_000) {
            let mut r = Resampler::new(from, 16_000);
            for c in chunks {
                for s in r.process(&c) {
                    prop_assert!((-1.0..=1.0).contains(&s));
                }
            }
        }
    }
}
