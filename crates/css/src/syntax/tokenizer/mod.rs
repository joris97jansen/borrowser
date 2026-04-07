//! CSS tokenizer.
//!
//! This is the lexical source of truth for the CSS syntax layer. It converts
//! decoded source text into explicit `CssToken` values with stable spans and
//! deterministic malformed-input handling.

mod engine;
mod entry;
mod model;
mod scan;

#[cfg(test)]
mod tests;

pub use self::entry::{tokenize_str, tokenize_str_with_options};
pub use self::model::{CssTokenization, CssTokenizationStats};
