//! Input level for the dictation bar waveform and the settings meter.

/// Root-mean-square of a frame.
pub fn rms(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt()
}

/// RMS in dBFS, floored at -100.
pub fn dbfs(frame: &[f32]) -> f32 {
    let r = rms(frame);
    if r <= 1e-5 {
        -100.0
    } else {
        (20.0 * r.log10()).max(-100.0)
    }
}

/// Maps frames to a 0–1 display level: fast attack, slow release, on a
/// perceptual (dB) scale between `floor_db` and 0 dBFS.
#[derive(Debug, Clone)]
pub struct LevelMeter {
    floor_db: f32,
    attack: f32,
    release: f32,
    value: f32,
}

impl Default for LevelMeter {
    fn default() -> Self {
        Self {
            floor_db: -60.0,
            attack: 0.6,
            release: 0.15,
            value: 0.0,
        }
    }
}

impl LevelMeter {
    pub fn push(&mut self, frame: &[f32]) -> f32 {
        let db = dbfs(frame);
        let target = ((db - self.floor_db) / -self.floor_db).clamp(0.0, 1.0);
        let k = if target > self.value { self.attack } else { self.release };
        self.value += (target - self.value) * k;
        self.value
    }

    pub fn value(&self) -> f32 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_and_db() {
        assert_eq!(rms(&[]), 0.0);
        assert!((rms(&[0.5, -0.5]) - 0.5).abs() < 1e-6);
        assert_eq!(dbfs(&[0.0; 10]), -100.0);
        assert!((dbfs(&[1.0, -1.0]) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn meter_attacks_fast_and_releases_slowly() {
        let mut m = LevelMeter::default();
        let loud = vec![0.5; 320];
        let silent = vec![0.0; 320];
        let up = m.push(&loud);
        assert!(up > 0.4, "attack {up}");
        for _ in 0..5 {
            m.push(&loud);
        }
        let peak = m.value();
        let down = m.push(&silent);
        assert!(down < peak && down > peak * 0.7, "release {down} from {peak}");
        for _ in 0..100 {
            m.push(&silent);
        }
        assert!(m.value() < 0.01);
    }
}
