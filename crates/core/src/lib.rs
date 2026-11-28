//! Vocalis domain logic shared by the desktop app and the gateway.
//!
//! Everything in this crate is pure and deterministic so it can be unit- and
//! property-tested without a microphone, a display or a network.
//! Behavior is specified in `docs/spec/4-1-dictation.md` and
//! `docs/spec/4-5-personalization.md`; design choices are in `docs/DECISIONS.md`.

pub mod apps;
pub mod dictionary;
pub mod formatting;
pub mod gesture;
pub mod presets;
pub mod product;
pub mod snippets;
pub mod suggest;
pub mod tags;
pub mod text;
pub mod wake;
