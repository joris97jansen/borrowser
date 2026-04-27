mod config;
mod error;
mod matching;
mod parser;
mod source;

pub use self::config::{
    SelectorMatchingFuzzConfig, SelectorMatchingFuzzSummary, SelectorMatchingFuzzTermination,
    SelectorParserFuzzConfig, SelectorParserFuzzSummary, SelectorParserFuzzTermination,
};
pub use self::error::SelectorFuzzError;
pub use self::matching::run_seeded_selector_matching_fuzz_case;
pub use self::parser::run_seeded_selector_parser_fuzz_case;

#[cfg(test)]
mod tests;
