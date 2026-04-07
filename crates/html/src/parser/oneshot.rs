use super::options::HtmlParseOptions;
use super::output::ParseOutput;
use super::session::HtmlParser;
use super::types::HtmlParseError;

/// Parse a complete HTML document in one shot through the HTML5-backed facade.
///
/// This is the preferred engine-level entrypoint when the full input is already
/// available. The returned [`ParseOutput`] always contains the full patch
/// history for the parse.
///
/// # Examples
///
/// ```no_run
/// use html::{HtmlParseOptions, parse_document};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let output = parse_document(
///         "<!doctype html><p>Hello</p>",
///         HtmlParseOptions::default(),
///     )?;
///
///     assert!(output.contains_full_patch_history);
///     Ok(())
/// }
/// ```
pub fn parse_document(
    input: impl AsRef<[u8]>,
    options: HtmlParseOptions,
) -> Result<ParseOutput, HtmlParseError> {
    #[cfg(feature = "parse-guards")]
    crate::parse_guards::record_full_parse_entry();

    let mut parser = HtmlParser::new(options)?;
    parser.push_bytes(input.as_ref())?;
    parser.finish()?;

    #[cfg(feature = "parse-guards")]
    crate::parse_guards::record_full_parse_output();

    parser.into_output()
}
