//! Hotkeys (master prompt §5.2 "Hotkeys", Appendix A).
//!
//! [`Key`] is a platform-neutral key with left/right sides kept apart, so "Right
//! Option" and "Left Ctrl" can be bound. [`ChordTracker`] turns the stream of key
//! downs and ups into presses and releases of configured [`Chord`]s, which feed
//! [`vocalis_core::gesture::Recognizer`]. The OS listener (feature `listen`)
//! only translates native events into [`Key`]s.

pub mod chord;
pub mod key;

#[cfg(feature = "listen")]
pub mod listen;

pub use chord::{Chord, ChordError, ChordRecorder, ChordTracker, KeyEvent, Recorded};
pub use key::Key;
