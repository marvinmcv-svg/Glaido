//! Hotkey gesture recognizer (spec 4.1 "Recording modes", master prompt §5.2).
//!
//! The platform input layer (`crates/input`) resolves raw key events into
//! presses and releases of *bindings* (a binding is a whole chord such as
//! `Fn` or `Ctrl+Win`). This state machine turns those into session actions.
//!
//! Supported gestures:
//! * **Hold** (push-to-talk): press the dictation key to start, release to stop.
//! * **Hands-free** binding: press to start, press again to stop.
//! * **Double-tap toggle** on the dictation key: a quick tap followed by a
//!   second press within the tap window turns the session hands-free; a later
//!   single tap stops it. The same key still works for hold-to-talk.
//! * **Double-tap and hold**: a single press does nothing; tap then press and
//!   hold within the window starts push-to-talk (for keys that are also used
//!   on their own).
//! * **Esc** cancels any active session (audio is kept by the caller).
//! * **Enter** stops a hands-free session when "Enter to stop and paste" is on.
//!
//! Recording starts on the first key-down so the start latency budget
//! (< 80 ms, §5.9) is never spent waiting for the tap window. When a quick tap
//! turns out not to be part of a double tap, the recognizer emits
//! [`Action::Discard`] so the caller drops that sliver of audio without
//! creating a record.
//!
//! Timestamps are milliseconds from any monotonic clock. Call [`Recognizer::tick`]
//! at or after [`Recognizer::deadline`] to resolve pending taps.

use serde::{Deserialize, Serialize};

/// Default tap window (§5.2).
pub const DEFAULT_TAP_WINDOW_MS: u64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Binding {
    /// The main dictation key (hold-to-talk, optionally double-tap).
    Primary,
    /// The dedicated hands-free (toggle) chord.
    HandsFree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoubleTap {
    /// Hold only.
    #[default]
    Off,
    /// Hold to talk, and double-tap to start/stop hands-free.
    Toggle,
    /// Tap, then press and hold to talk. A single hold does nothing.
    HoldAfterTap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub tap_window_ms: u64,
    pub double_tap: DoubleTap,
    pub enter_to_stop: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tap_window_ms: DEFAULT_TAP_WINDOW_MS,
            double_tap: DoubleTap::Off,
            enter_to_stop: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Input {
    Pressed(Binding, u64),
    Released(Binding, u64),
    Escape(u64),
    Enter(u64),
    Tick(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    PushToTalk,
    HandsFree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "action", content = "mode")]
pub enum Action {
    /// Begin capturing audio and show the bar.
    Start(Mode),
    /// The current capture continues, but the session is now hands-free.
    Promote(Mode),
    /// Stop capturing and process (transcribe, format, paste).
    Stop,
    /// User cancelled. Nothing is pasted; the caller keeps the audio.
    Cancel,
    /// Accidental tap: drop the audio silently, no record.
    Discard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    /// Primary held, recording push-to-talk. `since` = press time.
    Holding {
        since: u64,
    },
    /// Quick tap released while recording; waiting to see if a second press follows.
    TapPending {
        released_at: u64,
    },
    /// Hands-free recording. `primary_down` tracks a press that must be ignored on release.
    HandsFree {
        primary_down: bool,
    },
    /// HoldAfterTap: first press seen, not recording.
    ArmFirstDown {
        since: u64,
    },
    /// HoldAfterTap: first tap released, waiting for the second press.
    ArmAwaitSecond {
        released_at: u64,
    },
    /// A press that must be swallowed until release (e.g. a too-long first press).
    Swallow,
}

#[derive(Debug, Clone)]
pub struct Recognizer {
    cfg: Config,
    state: State,
}

impl Recognizer {
    pub fn new(cfg: Config) -> Self {
        Self {
            cfg,
            state: State::Idle,
        }
    }

    pub fn config(&self) -> Config {
        self.cfg
    }

    /// Whether a session is currently capturing audio.
    pub fn is_recording(&self) -> bool {
        matches!(
            self.state,
            State::Holding { .. } | State::TapPending { .. } | State::HandsFree { .. }
        )
    }

    /// The next time `tick` must be called to resolve a pending tap, if any.
    pub fn deadline(&self) -> Option<u64> {
        let w = self.cfg.tap_window_ms;
        match self.state {
            State::TapPending { released_at } | State::ArmAwaitSecond { released_at } => Some(released_at + w),
            State::ArmFirstDown { since } => Some(since + w),
            _ => None,
        }
    }

    pub fn handle(&mut self, input: Input) -> Vec<Action> {
        use Action::*;
        use Binding::*;
        let w = self.cfg.tap_window_ms;
        let mut out = Vec::new();

        // Esc cancels anything that is recording, and resets arming states.
        if let Input::Escape(_) = input {
            if self.is_recording() {
                out.push(match self.state {
                    // A pending tap was never a real dictation.
                    State::TapPending { .. } => Discard,
                    _ => Cancel,
                });
            }
            self.state = State::Idle;
            return out;
        }

        self.state = match (self.state, input) {
            // ---------- Idle ----------
            (State::Idle, Input::Pressed(Primary, t)) => match self.cfg.double_tap {
                DoubleTap::HoldAfterTap => State::ArmFirstDown { since: t },
                _ => {
                    out.push(Start(Mode::PushToTalk));
                    State::Holding { since: t }
                }
            },
            (State::Idle, Input::Pressed(HandsFree, _)) => {
                out.push(Start(Mode::HandsFree));
                State::HandsFree { primary_down: false }
            }

            // ---------- Push-to-talk ----------
            (State::Holding { since }, Input::Released(Primary, t)) => {
                if self.cfg.double_tap == DoubleTap::Toggle && t.saturating_sub(since) < w {
                    State::TapPending { released_at: t }
                } else {
                    out.push(Stop);
                    State::Idle
                }
            }
            // Pressing the hands-free chord while holding promotes the session.
            (State::Holding { .. }, Input::Pressed(HandsFree, _)) => {
                out.push(Promote(Mode::HandsFree));
                State::HandsFree { primary_down: true }
            }

            // ---------- Double-tap toggle ----------
            (State::TapPending { released_at }, Input::Pressed(Primary, t)) if t <= released_at + w => {
                out.push(Promote(Mode::HandsFree));
                State::HandsFree { primary_down: true }
            }
            (State::TapPending { released_at }, Input::Pressed(Primary, t)) => {
                // Window already passed but no tick arrived: resolve, then treat as a fresh press.
                let _ = released_at;
                out.push(Discard);
                out.push(Start(Mode::PushToTalk));
                State::Holding { since: t }
            }
            (State::TapPending { released_at }, Input::Tick(t)) if t >= released_at + w => {
                out.push(Discard);
                State::Idle
            }
            (State::TapPending { .. }, Input::Pressed(HandsFree, _)) => {
                out.push(Promote(Mode::HandsFree));
                State::HandsFree { primary_down: false }
            }

            // ---------- Hands-free ----------
            (State::HandsFree { primary_down: true }, Input::Released(Primary, _)) => {
                State::HandsFree { primary_down: false }
            }
            (State::HandsFree { primary_down: false }, Input::Pressed(Primary, _)) => {
                out.push(Stop);
                State::Swallow
            }
            (State::HandsFree { .. }, Input::Pressed(HandsFree, _)) => {
                out.push(Stop);
                State::Idle
            }
            (State::HandsFree { primary_down }, Input::Enter(_)) if self.cfg.enter_to_stop => {
                out.push(Stop);
                if primary_down {
                    State::Swallow
                } else {
                    State::Idle
                }
            }

            // ---------- Double-tap and hold ----------
            (State::ArmFirstDown { since }, Input::Released(Primary, t)) => {
                if t.saturating_sub(since) < w {
                    State::ArmAwaitSecond { released_at: t }
                } else {
                    State::Idle
                }
            }
            (State::ArmFirstDown { since }, Input::Tick(t)) if t >= since + w => State::Swallow,
            (State::ArmAwaitSecond { released_at }, Input::Pressed(Primary, t)) if t <= released_at + w => {
                out.push(Start(Mode::PushToTalk));
                // `since: 0` makes any release a Stop (no tap logic in this mode).
                State::Holding { since: 0 }
            }
            (State::ArmAwaitSecond { .. }, Input::Pressed(Primary, t)) => State::ArmFirstDown { since: t },
            (State::ArmAwaitSecond { released_at }, Input::Tick(t)) if t >= released_at + w => State::Idle,

            // ---------- Swallow until the key comes up ----------
            (State::Swallow, Input::Released(Primary, _)) => State::Idle,

            // Everything else is ignored.
            (s, _) => s,
        };
        out
    }

    /// Convenience for `handle(Input::Tick(now))`.
    pub fn tick(&mut self, now: u64) -> Vec<Action> {
        self.handle(Input::Tick(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Action::*;
    use Binding::*;
    use Input::*;

    fn run(cfg: Config, inputs: &[Input]) -> Vec<Action> {
        let mut r = Recognizer::new(cfg);
        inputs.iter().flat_map(|i| r.handle(*i)).collect()
    }

    fn cfg(dt: DoubleTap) -> Config {
        Config {
            double_tap: dt,
            ..Config::default()
        }
    }

    #[test]
    fn hold_to_talk() {
        let a = run(Config::default(), &[Pressed(Primary, 0), Released(Primary, 1500)]);
        assert_eq!(a, vec![Start(Mode::PushToTalk), Stop]);
    }

    #[test]
    fn short_hold_without_double_tap_still_stops() {
        let a = run(Config::default(), &[Pressed(Primary, 0), Released(Primary, 50)]);
        assert_eq!(a, vec![Start(Mode::PushToTalk), Stop]);
    }

    #[test]
    fn hands_free_binding_toggles() {
        let a = run(
            Config::default(),
            &[Pressed(HandsFree, 0), Released(HandsFree, 80), Pressed(HandsFree, 4000)],
        );
        assert_eq!(a, vec![Start(Mode::HandsFree), Stop]);
    }

    #[test]
    fn escape_cancels_hands_free_and_hold() {
        assert_eq!(
            run(Config::default(), &[Pressed(HandsFree, 0), Escape(900)]),
            vec![Start(Mode::HandsFree), Cancel]
        );
        assert_eq!(
            run(Config::default(), &[Pressed(Primary, 0), Escape(900)]),
            vec![Start(Mode::PushToTalk), Cancel]
        );
    }

    #[test]
    fn escape_when_idle_does_nothing() {
        assert!(run(Config::default(), &[Escape(0)]).is_empty());
    }

    #[test]
    fn enter_stops_only_when_enabled() {
        let on = Config {
            enter_to_stop: true,
            ..Config::default()
        };
        assert_eq!(
            run(on, &[Pressed(HandsFree, 0), Enter(500)]),
            vec![Start(Mode::HandsFree), Stop]
        );
        assert_eq!(
            run(Config::default(), &[Pressed(HandsFree, 0), Enter(500)]),
            vec![Start(Mode::HandsFree)]
        );
    }

    #[test]
    fn enter_does_not_stop_push_to_talk() {
        let on = Config {
            enter_to_stop: true,
            ..Config::default()
        };
        assert_eq!(
            run(on, &[Pressed(Primary, 0), Enter(500)]),
            vec![Start(Mode::PushToTalk)]
        );
    }

    #[test]
    fn double_tap_toggle_promotes_then_single_tap_stops() {
        let a = run(
            cfg(DoubleTap::Toggle),
            &[
                Pressed(Primary, 0),
                Released(Primary, 100),
                Pressed(Primary, 250),
                Released(Primary, 320),
                Pressed(Primary, 5000),
                Released(Primary, 5080),
            ],
        );
        assert_eq!(a, vec![Start(Mode::PushToTalk), Promote(Mode::HandsFree), Stop]);
    }

    #[test]
    fn double_tap_toggle_single_tap_is_discarded_after_window() {
        let mut r = Recognizer::new(cfg(DoubleTap::Toggle));
        assert_eq!(r.handle(Pressed(Primary, 0)), vec![Start(Mode::PushToTalk)]);
        assert!(r.handle(Released(Primary, 100)).is_empty());
        assert_eq!(r.deadline(), Some(400));
        assert!(r.tick(399).is_empty());
        assert_eq!(r.tick(400), vec![Discard]);
        assert!(!r.is_recording());
    }

    #[test]
    fn double_tap_toggle_keeps_hold_to_talk() {
        let a = run(cfg(DoubleTap::Toggle), &[Pressed(Primary, 0), Released(Primary, 2000)]);
        assert_eq!(a, vec![Start(Mode::PushToTalk), Stop]);
    }

    #[test]
    fn late_second_press_without_tick_resolves_and_restarts() {
        let a = run(
            cfg(DoubleTap::Toggle),
            &[Pressed(Primary, 0), Released(Primary, 100), Pressed(Primary, 900)],
        );
        assert_eq!(a, vec![Start(Mode::PushToTalk), Discard, Start(Mode::PushToTalk)]);
    }

    #[test]
    fn escape_during_pending_tap_discards() {
        let a = run(
            cfg(DoubleTap::Toggle),
            &[Pressed(Primary, 0), Released(Primary, 100), Escape(150)],
        );
        assert_eq!(a, vec![Start(Mode::PushToTalk), Discard]);
    }

    #[test]
    fn hold_after_tap_requires_tap_first() {
        assert!(run(
            cfg(DoubleTap::HoldAfterTap),
            &[Pressed(Primary, 0), Released(Primary, 2000)]
        )
        .is_empty());
        let a = run(
            cfg(DoubleTap::HoldAfterTap),
            &[
                Pressed(Primary, 0),
                Released(Primary, 80),
                Pressed(Primary, 200),
                Released(Primary, 3000),
            ],
        );
        assert_eq!(a, vec![Start(Mode::PushToTalk), Stop]);
    }

    #[test]
    fn hold_after_tap_short_second_press_still_stops() {
        let a = run(
            cfg(DoubleTap::HoldAfterTap),
            &[
                Pressed(Primary, 0),
                Released(Primary, 80),
                Pressed(Primary, 200),
                Released(Primary, 250),
            ],
        );
        assert_eq!(a, vec![Start(Mode::PushToTalk), Stop]);
    }

    #[test]
    fn hold_after_tap_expires() {
        let mut r = Recognizer::new(cfg(DoubleTap::HoldAfterTap));
        r.handle(Pressed(Primary, 0));
        r.handle(Released(Primary, 80));
        assert!(r.tick(380).is_empty());
        assert!(r.handle(Pressed(Primary, 500)).is_empty());
        assert!(!r.is_recording());
    }

    #[test]
    fn hands_free_chord_promotes_hold() {
        let a = run(
            Config::default(),
            &[
                Pressed(Primary, 0),
                Pressed(HandsFree, 400),
                Released(Primary, 500),
                Released(HandsFree, 520),
                Pressed(HandsFree, 3000),
            ],
        );
        assert_eq!(a, vec![Start(Mode::PushToTalk), Promote(Mode::HandsFree), Stop]);
    }

    #[test]
    fn primary_press_stops_hands_free_and_release_is_swallowed() {
        let mut r = Recognizer::new(Config::default());
        assert_eq!(r.handle(Pressed(HandsFree, 0)), vec![Start(Mode::HandsFree)]);
        assert_eq!(r.handle(Pressed(Primary, 3000)), vec![Stop]);
        assert!(r.handle(Released(Primary, 3100)).is_empty());
        // Fresh press starts a new session.
        assert_eq!(r.handle(Pressed(Primary, 5000)), vec![Start(Mode::PushToTalk)]);
    }

    mod props {
        use super::*;
        use proptest::prelude::*;

        fn any_input() -> impl Strategy<Value = (u8, u64)> {
            (0u8..7, 0u64..700)
        }

        proptest! {
            /// Starts and terminations always alternate, whatever the key mashing.
            #[test]
            fn starts_and_ends_alternate(steps in proptest::collection::vec(any_input(), 0..60), dt in 0u8..3, enter in any::<bool>()) {
                let double_tap = [DoubleTap::Off, DoubleTap::Toggle, DoubleTap::HoldAfterTap][dt as usize];
                let mut r = Recognizer::new(Config { double_tap, enter_to_stop: enter, ..Config::default() });
                let mut t = 0u64;
                let mut active = false;
                for (kind, dtms) in steps {
                    t += dtms;
                    let input = match kind {
                        0 => Pressed(Primary, t), 1 => Released(Primary, t),
                        2 => Pressed(HandsFree, t), 3 => Released(HandsFree, t),
                        4 => Escape(t), 5 => Enter(t), _ => Tick(t),
                    };
                    for a in r.handle(input) {
                        match a {
                            Start(_) => { prop_assert!(!active); active = true; }
                            Promote(_) => prop_assert!(active),
                            Stop | Cancel | Discard => { prop_assert!(active); active = false; }
                        }
                    }
                    prop_assert_eq!(active, r.is_recording());
                }
            }
        }
    }
}
