mod oneshot;
mod options;
mod output;
mod session;
mod types;

#[cfg(test)]
mod tests;

pub use self::oneshot::parse_document;
#[cfg(all(
    feature = "parser-failure-injection",
    any(test, feature = "internal-api")
))]
pub(crate) use self::oneshot::parse_document_with_failure_injection;
pub use self::options::{
    HtmlErrorPolicy, HtmlParseOptions, HtmlTokenizerLimits, HtmlTokenizerOptions,
    HtmlTreeBuilderLimits, HtmlTreeBuilderOptions,
};
pub use self::output::ParseOutput;
pub use self::session::HtmlParser;
#[cfg(feature = "parser-conformance")]
pub(crate) use self::session::{
    ConformanceFinalizationError, ConformanceFinalizedOutput, PatchMaterializationWitness,
};
pub use self::types::{HtmlParseCounters, HtmlParseError, HtmlParseEvent};
