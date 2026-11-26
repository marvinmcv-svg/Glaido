//! Energy-based voice activity detection with an adaptive noise floor.
//!
//! Good enough for "did the user say anything?" (skip empty recordings, the
//! silent-mic warning) and for trimming. A neural VAD can replace it behind the
//! same interface later.

use crate::level::dbfs;

/// Highest starting noise-floor estimate, in dBFS.
const MAX_INITIAL_FLOOR_DB: f32 = -50.0;

#[derive(Debug, Clone)]
pub struct VadConfig {
    /// Speech must be this many dB above the noise floor.
    pub margin_db: f32,
    /// Absolute floor: anything quieter is never speech.
    pub min_speech_db: f32,
    /// Consecutive speech frames needed to enter the speech state.
    pub attack_frames: u32,
    /// Frames to stay in speech after energy drops (hangover).
    pub hangover_frames: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        // 20 ms frames: 3 frames = 60 ms attack, 15 frames = 300 ms hangover.
        Self {
            margin_db: 10.0,
            min_speech_db: -50.0,
            attack_frames: 3,
            hangover_frames: 15,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Vad {
    cfg: VadConfig,
    /// Running noise-floor estimate; seeded from the first frame.
    noise_db: Option<f32>,
    run: u32,
    hang: u32,
    speaking: bool,
    speech_frames: u64,
    total_frames: u64,
}

impl Vad {
    pub fn new(cfg: VadConfig) -> Self {
        Self {
            cfg,
            noise_db: None,
            run: 0,
            hang: 0,
            speaking: false,
            speech_frames: 0,
            total_frames: 0,
        }
    }

    /// Feed one frame (20 ms recommended). Returns whether it is speech.
    pub fn push(&mut self, frame: &[f32]) -> bool {
        let db = dbfs(frame);
        self.total_frames += 1;
        // Seed from the first frame, but never above MAX_INITIAL_FLOOR_DB: with
        // push-to-talk the first frame is often already speech.
        let noise = *self.noise_db.get_or_insert(db.min(MAX_INITIAL_FLOOR_DB));
        let loud = db >= self.cfg.min_speech_db && db >= noise + self.cfg.margin_db;
        if loud {
            self.run += 1;
            if self.run >= self.cfg.attack_frames {
                self.speaking = true;
                self.hang = self.cfg.hangover_frames;
            }
        } else {
            self.run = 0;
            if self.hang > 0 {
                self.hang -= 1;
            } else {
                self.speaking = false;
            }
        }
        // Minimum-tracking noise floor: fall fast toward quieter frames, rise slowly
        // otherwise so steady background noise (fans, hum) is learned. The rise is
        // much slower during speech, so long continuous dictation is not mistaken
        // for background noise (a steady hum takes up to ~1 minute to be learned).
        let k = if db < noise {
            0.3
        } else if self.speaking {
            0.0005
        } else {
            0.01
        };
        self.noise_db = Some(noise + (db - noise) * k);
        if self.speaking {
            self.speech_frames += 1;
        }
        self.speaking
    }

    pub fn is_speaking(&self) -> bool {
        self.speaking
    }

    /// Total detected speech, in frames.
    pub fn speech_frames(&self) -> u64 {
        self.speech_frames
    }

    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }

    /// Whether the recording contains enough speech to be worth transcribing.
    pub fn heard_speech(&self, min_frames: u64) -> bool {
        self.speech_frames >= min_frames
    }
}

impl Default for Vad {
    fn default() -> Self {
        Self::new(VadConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(amp: f32, n: usize, seed: &mut u32) -> Vec<f32> {
        (0..n)
            .map(|_| {
                *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                ((*seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0) * amp
            })
            .collect()
    }

    fn tone(amp: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| (i as f32 * 0.2).sin() * amp).collect()
    }

    #[test]
    fn silence_is_not_speech() {
        let mut v = Vad::default();
        let mut seed = 1;
        for _ in 0..200 {
            assert!(!v.push(&noise(0.002, 320, &mut seed)));
        }
        assert!(!v.heard_speech(10));
    }

    #[test]
    fn detects_speech_over_noise_with_attack_and_hangover() {
        let mut v = Vad::default();
        let mut seed = 7;
        for _ in 0..50 {
            v.push(&noise(0.01, 320, &mut seed));
        }
        let flags: Vec<bool> = (0..10).map(|_| v.push(&tone(0.3, 320))).collect();
        assert_eq!(&flags[..3], &[false, false, true], "attack after 3 frames");
        assert!(flags[3..].iter().all(|&f| f));
        // Hangover keeps speech for 15 frames of noise, then releases.
        let tail: Vec<bool> = (0..20).map(|_| v.push(&noise(0.01, 320, &mut seed))).collect();
        assert!(tail[..15].iter().all(|&f| f));
        assert!(!tail[16]);
        assert!(v.heard_speech(8));
    }

    #[test]
    fn speech_from_the_very_first_frame_is_detected() {
        let mut v = Vad::default();
        let flags: Vec<bool> = (0..20).map(|_| v.push(&tone(0.3, 320))).collect();
        assert!(
            flags[2..].iter().all(|&f| f),
            "push-to-talk users start talking at once"
        );
    }

    #[test]
    fn noisy_room_from_the_start_is_not_speech() {
        let mut v = Vad::default();
        let mut seed = 3;
        // Background at about -45 dBFS: above the absolute floor, but steady.
        for _ in 0..200 {
            assert!(!v.push(&noise(0.01, 320, &mut seed)));
        }
    }

    #[test]
    fn long_continuous_speech_stays_speech() {
        let mut v = Vad::default();
        let mut seed = 5;
        for _ in 0..25 {
            v.push(&noise(0.005, 320, &mut seed));
        }
        // 20 seconds of uninterrupted loud speech.
        let flags: Vec<bool> = (0..1000).map(|_| v.push(&tone(0.3, 320))).collect();
        assert!(flags[3..].iter().all(|&f| f));
    }

    #[test]
    fn steady_hum_is_learned_as_noise() {
        let mut v = Vad::default();
        // A constant loud-ish hum: after adapting, it shouldn't count as speech forever.
        let hum = tone(0.02, 320);
        let mut speech_late = 0;
        for i in 0..4000 {
            if v.push(&hum) && i > 3000 {
                speech_late += 1;
            }
        }
        assert_eq!(speech_late, 0);
    }
}
