//! Product identity constants (ADR-0001). Keep in sync with `packages/config/product.json`.

pub const PRODUCT_NAME: &str = "Vocalis";
pub const WAKE_WORD: &str = "Vocalis";
pub const CLI_NAME: &str = "vocalis";
pub const DEEP_LINK_SCHEME: &str = "vocalis";
pub const FREE_WORDS_PER_WEEK: u32 = 2000;

/// Hard cap for a single recording (spec 4.1).
pub const MAX_RECORDING_MS: u64 = 10 * 60 * 1000;
/// When the "nearing limit" warning appears (§5.2: 9:30).
pub const RECORDING_WARNING_MS: u64 = 9 * 60 * 1000 + 30 * 1000;
