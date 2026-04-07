mod oneshot;
mod options;
mod output;
mod session;
mod types;

#[cfg(test)]
mod tests;

pub use self::oneshot::parse_document;
pub use self::options::{
    HtmlErrorPolicy, HtmlParseOptions, HtmlTokenizerLimits, HtmlTokenizerOptions,
    HtmlTreeBuilderLimits, HtmlTreeBuilderOptions,
};
pub use self::output::ParseOutput;
pub use self::session::HtmlParser;
pub use self::types::{HtmlParseCounters, HtmlParseError, HtmlParseEvent};
