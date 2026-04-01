use crate::Node;
use crate::dom_patch::{DomPatch, DomPatchBatch};
use crate::html5::shared::{
    Counters as Html5Counters, DocumentParseContext, ErrorOrigin, ErrorPolicy,
    ParseError as Html5ParseError, ParseErrorCode,
};
use crate::html5::tokenizer::{TokenizerConfig, TokenizerLimits};
use crate::html5::tree_builder::{TreeBuilderConfig, TreeBuilderLimits};
use crate::html5::{Html5ParseSession, Html5SessionError};
use crate::patch_validation::PatchValidationArena;

/// Stable origin classification for surfaced parse events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HtmlParseEventOrigin {
    Tokenizer,
    TreeBuilder,
}

impl From<ErrorOrigin> for HtmlParseEventOrigin {
    fn from(value: ErrorOrigin) -> Self {
        match value {
            ErrorOrigin::Tokenizer => Self::Tokenizer,
            ErrorOrigin::TreeBuilder => Self::TreeBuilder,
        }
    }
}

/// Stable event code classification for surfaced parse events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HtmlParseEventCode {
    UnexpectedNullCharacter,
    UnexpectedEof,
    InvalidCharacterReference,
    ResourceLimit,
    ImplementationGuardrail,
    Other,
}

impl From<ParseErrorCode> for HtmlParseEventCode {
    fn from(value: ParseErrorCode) -> Self {
        match value {
            ParseErrorCode::UnexpectedNullCharacter => Self::UnexpectedNullCharacter,
            ParseErrorCode::UnexpectedEof => Self::UnexpectedEof,
            ParseErrorCode::InvalidCharacterReference => Self::InvalidCharacterReference,
            ParseErrorCode::ResourceLimit => Self::ResourceLimit,
            ParseErrorCode::ImplementationGuardrail => Self::ImplementationGuardrail,
            ParseErrorCode::Other => Self::Other,
        }
    }
}

/// Stable engine-facing parse event record.
///
/// `detail` is diagnostic metadata and is not intended to be a hard stability
/// boundary for downstream decision logic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HtmlParseEvent {
    pub origin: HtmlParseEventOrigin,
    pub code: HtmlParseEventCode,
    pub position: usize,
    pub detail: Option<&'static str>,
    pub aux: Option<u32>,
}

impl From<Html5ParseError> for HtmlParseEvent {
    fn from(value: Html5ParseError) -> Self {
        Self {
            origin: value.origin.into(),
            code: value.code.into(),
            position: value.position,
            detail: value.detail,
            aux: value.aux,
        }
    }
}

/// Controls parse-error tracking on the stable parser facade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HtmlErrorPolicy {
    pub track: bool,
    pub max_stored: usize,
    pub debug_only: bool,
    pub track_counters: bool,
}

impl Default for HtmlErrorPolicy {
    fn default() -> Self {
        let policy = ErrorPolicy::default();
        Self {
            track: policy.track,
            max_stored: policy.max_stored,
            debug_only: policy.debug_only,
            track_counters: policy.track_counters,
        }
    }
}

impl From<HtmlErrorPolicy> for ErrorPolicy {
    fn from(value: HtmlErrorPolicy) -> Self {
        Self {
            track: value.track,
            max_stored: value.max_stored,
            debug_only: value.debug_only,
            track_counters: value.track_counters,
        }
    }
}

/// Stable parser counters surfaced by the HTML5-backed facade.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HtmlParseCounters {
    pub tokens_processed: u64,
    pub patches_emitted: u64,
    pub decode_errors: u64,
    pub adapter_invariant_violations: u64,
    pub tree_builder_invariant_errors: u64,
    pub parse_errors: u64,
    pub errors_dropped: u64,
    pub max_open_elements_depth: u32,
    pub max_active_formatting_depth: u32,
    pub soe_push_ops: u64,
    pub soe_pop_ops: u64,
    pub soe_scope_scan_calls: u64,
    pub soe_scope_scan_steps: u64,
    pub tree_builder_patches_emitted: u64,
    pub tree_builder_text_nodes_created: u64,
    pub tree_builder_text_appends: u64,
    pub tree_builder_text_coalescing_invalidations: u64,
}

impl From<Html5Counters> for HtmlParseCounters {
    fn from(value: Html5Counters) -> Self {
        Self {
            tokens_processed: value.tokens_processed,
            patches_emitted: value.patches_emitted,
            decode_errors: value.decode_errors,
            adapter_invariant_violations: value.adapter_invariant_violations,
            tree_builder_invariant_errors: value.tree_builder_invariant_errors,
            parse_errors: value.parse_errors,
            errors_dropped: value.errors_dropped,
            max_open_elements_depth: value.max_open_elements_depth,
            max_active_formatting_depth: value.max_active_formatting_depth,
            soe_push_ops: value.soe_push_ops,
            soe_pop_ops: value.soe_pop_ops,
            soe_scope_scan_calls: value.soe_scope_scan_calls,
            soe_scope_scan_steps: value.soe_scope_scan_steps,
            tree_builder_patches_emitted: value.tree_builder_patches_emitted,
            tree_builder_text_nodes_created: value.tree_builder_text_nodes_created,
            tree_builder_text_appends: value.tree_builder_text_appends,
            tree_builder_text_coalescing_invalidations: value
                .tree_builder_text_coalescing_invalidations,
        }
    }
}

/// Tokenizer resource limits for the stable parser facade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HtmlTokenizerLimits {
    pub max_tokens_per_batch: usize,
    pub max_tag_name_bytes: usize,
    pub max_attribute_name_bytes: usize,
    pub max_attribute_value_bytes: usize,
    pub max_attributes_per_tag: usize,
    pub max_comment_bytes: usize,
    pub max_doctype_bytes: usize,
    pub max_end_tag_match_scan_bytes: usize,
}

impl Default for HtmlTokenizerLimits {
    fn default() -> Self {
        let limits = TokenizerLimits::default();
        Self {
            max_tokens_per_batch: limits.max_tokens_per_batch,
            max_tag_name_bytes: limits.max_tag_name_bytes,
            max_attribute_name_bytes: limits.max_attribute_name_bytes,
            max_attribute_value_bytes: limits.max_attribute_value_bytes,
            max_attributes_per_tag: limits.max_attributes_per_tag,
            max_comment_bytes: limits.max_comment_bytes,
            max_doctype_bytes: limits.max_doctype_bytes,
            max_end_tag_match_scan_bytes: limits.max_end_tag_match_scan_bytes,
        }
    }
}

impl From<HtmlTokenizerLimits> for TokenizerLimits {
    fn from(value: HtmlTokenizerLimits) -> Self {
        Self {
            max_tokens_per_batch: value.max_tokens_per_batch,
            max_tag_name_bytes: value.max_tag_name_bytes,
            max_attribute_name_bytes: value.max_attribute_name_bytes,
            max_attribute_value_bytes: value.max_attribute_value_bytes,
            max_attributes_per_tag: value.max_attributes_per_tag,
            max_comment_bytes: value.max_comment_bytes,
            max_doctype_bytes: value.max_doctype_bytes,
            max_end_tag_match_scan_bytes: value.max_end_tag_match_scan_bytes,
        }
    }
}

/// Tokenizer configuration for the stable parser facade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HtmlTokenizerOptions {
    pub emit_eof: bool,
    pub limits: HtmlTokenizerLimits,
}

impl Default for HtmlTokenizerOptions {
    fn default() -> Self {
        let config = TokenizerConfig::default();
        Self {
            emit_eof: config.emit_eof,
            limits: HtmlTokenizerLimits::default(),
        }
    }
}

impl From<HtmlTokenizerOptions> for TokenizerConfig {
    fn from(value: HtmlTokenizerOptions) -> Self {
        Self {
            emit_eof: value.emit_eof,
            limits: value.limits.into(),
        }
    }
}

/// Tree-builder resource limits for the stable parser facade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HtmlTreeBuilderLimits {
    pub max_open_elements_depth: usize,
    pub max_nodes_created: usize,
    pub max_children_per_node: usize,
}

impl Default for HtmlTreeBuilderLimits {
    fn default() -> Self {
        let limits = TreeBuilderLimits::default();
        Self {
            max_open_elements_depth: limits.max_open_elements_depth,
            max_nodes_created: limits.max_nodes_created,
            max_children_per_node: limits.max_children_per_node,
        }
    }
}

impl From<HtmlTreeBuilderLimits> for TreeBuilderLimits {
    fn from(value: HtmlTreeBuilderLimits) -> Self {
        Self {
            max_open_elements_depth: value.max_open_elements_depth,
            max_nodes_created: value.max_nodes_created,
            max_children_per_node: value.max_children_per_node,
        }
    }
}

/// Tree-builder configuration for the stable parser facade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HtmlTreeBuilderOptions {
    pub coalesce_text: bool,
    pub limits: HtmlTreeBuilderLimits,
}

impl Default for HtmlTreeBuilderOptions {
    fn default() -> Self {
        let config = TreeBuilderConfig::default();
        Self {
            coalesce_text: config.coalesce_text,
            limits: HtmlTreeBuilderLimits::default(),
        }
    }
}

impl From<HtmlTreeBuilderOptions> for TreeBuilderConfig {
    fn from(value: HtmlTreeBuilderOptions) -> Self {
        Self {
            coalesce_text: value.coalesce_text,
            limits: value.limits.into(),
        }
    }
}

/// Stable options for one-shot and streaming HTML parsing.
#[derive(Clone, Debug, Default)]
pub struct HtmlParseOptions {
    pub tokenizer: HtmlTokenizerOptions,
    pub tree_builder: HtmlTreeBuilderOptions,
    pub error_policy: HtmlErrorPolicy,
}

/// Stable error surface for the engine-facing parser facade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HtmlParseError {
    Decode,
    /// Terminal parser-state violation, including use after a poisoned
    /// patch-mirror failure.
    Invariant,
    PatchValidation(String),
}

impl core::fmt::Display for HtmlParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HtmlParseError::Decode => write!(f, "decode error"),
            HtmlParseError::Invariant => write!(f, "engine invariant violation"),
            HtmlParseError::PatchValidation(detail) => {
                write!(f, "patch validation error: {detail}")
            }
        }
    }
}

impl std::error::Error for HtmlParseError {}

impl From<Html5SessionError> for HtmlParseError {
    fn from(value: Html5SessionError) -> Self {
        match value {
            Html5SessionError::Decode => Self::Decode,
            Html5SessionError::Invariant => Self::Invariant,
        }
    }
}

/// Final parse result returned by [`parse_document`] or [`HtmlParser::into_output`].
#[derive(Debug)]
pub struct ParseOutput {
    pub document: Node,
    /// Patches drained by `into_output()`.
    ///
    /// For `parse_document(...)`, this is the full emitted patch history because
    /// no earlier draining is possible. For streaming use, if the caller has
    /// already consumed patches via `take_patches()` or `take_patch_batch()`,
    /// this contains only the undrained remainder.
    pub patches: Vec<DomPatch>,
    /// True when `patches` contains the full session patch history.
    pub contains_full_patch_history: bool,
    pub counters: HtmlParseCounters,
    pub parse_errors: Vec<HtmlParseEvent>,
}

/// Stable engine-level HTML parser API backed exclusively by the HTML5 pipeline.
///
/// If internal patch-mirror validation fails while draining emitted patches, the
/// parser transitions into a terminal poisoned state. Subsequent mutating or
/// draining operations return `HtmlParseError::Invariant` deterministically
/// rather than continuing with a partially updated mirror.
///
/// # Examples
///
/// ```no_run
/// use html::{HtmlParseOptions, HtmlParser};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut parser = HtmlParser::new(HtmlParseOptions::default())?;
///     parser.push_bytes(b"<div><span>hel")?;
///     parser.pump()?;
///     let _first_batch = parser.take_patch_batch()?;
///
///     parser.push_bytes(b"lo</span></div>")?;
///     parser.finish()?;
///     let output = parser.into_output()?;
///
///     assert!(!output.patches.is_empty());
///     Ok(())
/// }
/// ```
pub struct HtmlParser {
    session: Html5ParseSession,
    arena: PatchValidationArena,
    patches_drained_before_output: bool,
    poisoned: bool,
}

impl HtmlParser {
    /// Create a new streaming HTML parser backed by the HTML5 pipeline.
    pub fn new(options: HtmlParseOptions) -> Result<Self, HtmlParseError> {
        let ctx = DocumentParseContext::with_error_policy(options.error_policy.into());
        let session =
            Html5ParseSession::new(options.tokenizer.into(), options.tree_builder.into(), ctx)?;
        Ok(Self {
            session,
            arena: PatchValidationArena::default(),
            patches_drained_before_output: false,
            poisoned: false,
        })
    }

    /// Append raw bytes to the session decoder/input buffer.
    pub fn push_bytes(&mut self, bytes: &[u8]) -> Result<(), HtmlParseError> {
        self.ensure_not_poisoned()?;
        self.session.push_bytes(bytes)?;
        Ok(())
    }

    /// Append already-decoded UTF-8 text to the parser input.
    pub fn push_str(&mut self, text: &str) -> Result<(), HtmlParseError> {
        self.ensure_not_poisoned()?;
        self.session.push_str(text)?;
        Ok(())
    }

    /// Advance tokenization/tree building until the session needs more input or
    /// reaches a stable stop point.
    pub fn pump(&mut self) -> Result<(), HtmlParseError> {
        self.ensure_not_poisoned()?;
        self.session.pump()?;
        Ok(())
    }

    /// Signal end-of-input and run EOF-sensitive parser work exactly once.
    ///
    /// Callers using the streaming API must invoke this when no more input will
    /// arrive. Text-mode containers such as `<style>` and `<textarea>` may keep
    /// buffered content until `finish()` or an explicit closing tag is seen.
    pub fn finish(&mut self) -> Result<(), HtmlParseError> {
        self.ensure_not_poisoned()?;
        self.session.finish()?;
        Ok(())
    }

    /// Drain the currently available patches as one ordered vector.
    ///
    /// Draining patches updates the parser's internal DOM mirror. If non-empty
    /// patches are drained before `into_output()`, the final `ParseOutput`
    /// exposes only the undrained remainder in `patches`.
    pub fn take_patches(&mut self) -> Result<Vec<DomPatch>, HtmlParseError> {
        self.ensure_not_poisoned()?;
        let patches = self.session.take_patches();
        self.apply_patches(&patches)?;
        if !patches.is_empty() {
            self.patches_drained_before_output = true;
        }
        Ok(patches)
    }

    /// Drain the next available atomic patch batch.
    ///
    /// As with `take_patches()`, previously drained non-empty batches are not
    /// replayed by `into_output()`.
    pub fn take_patch_batch(&mut self) -> Result<Option<DomPatchBatch>, HtmlParseError> {
        self.take_patch_batch_internal(true)
    }

    /// Return the current parser counters without mutating parser state.
    pub fn counters(&self) -> HtmlParseCounters {
        self.session.counters().into()
    }

    /// Return the currently retained parse events without exposing backend
    /// `html5::*` types.
    pub fn parse_errors(&self) -> Vec<HtmlParseEvent> {
        self.session
            .parse_errors()
            .into_iter()
            .map(HtmlParseEvent::from)
            .collect()
    }

    /// Convenience accessor for `counters().tokens_processed`.
    pub fn tokens_processed(&self) -> u64 {
        self.counters().tokens_processed
    }

    /// Materialize the parser's current DOM mirror and return the undrained
    /// patch remainder.
    ///
    /// This consumes the parser. If earlier calls already drained non-empty
    /// patch batches, `ParseOutput::patches` contains only the remaining
    /// undrained patches and `contains_full_patch_history` is `false`.
    pub fn into_output(mut self) -> Result<ParseOutput, HtmlParseError> {
        let mut patches = Vec::new();
        while let Some(batch) = self.take_patch_batch_internal(false)? {
            patches.extend(batch.patches);
        }
        let document = self
            .arena
            .materialize()
            .map_err(|err| HtmlParseError::PatchValidation(err.to_string()))?;
        Ok(ParseOutput {
            document,
            patches,
            contains_full_patch_history: !self.patches_drained_before_output,
            counters: self.counters(),
            parse_errors: self.parse_errors(),
        })
    }

    fn apply_patches(&mut self, patches: &[DomPatch]) -> Result<(), HtmlParseError> {
        if patches.is_empty() {
            return Ok(());
        }
        if let Err(err) = self.arena.apply_batch_trusted(patches) {
            self.poisoned = true;
            return Err(HtmlParseError::PatchValidation(err.to_string()));
        }
        Ok(())
    }

    fn take_patch_batch_internal(
        &mut self,
        record_user_drain: bool,
    ) -> Result<Option<DomPatchBatch>, HtmlParseError> {
        self.ensure_not_poisoned()?;
        let Some(batch) = self.session.take_patch_batch() else {
            return Ok(None);
        };
        self.apply_patches(&batch.patches)?;
        if record_user_drain && !batch.patches.is_empty() {
            self.patches_drained_before_output = true;
        }
        Ok(Some(batch))
    }

    fn ensure_not_poisoned(&self) -> Result<(), HtmlParseError> {
        if self.poisoned {
            return Err(HtmlParseError::Invariant);
        }
        Ok(())
    }
}

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

#[cfg(test)]
mod tests {
    use super::{
        HtmlErrorPolicy, HtmlParseEventCode, HtmlParseEventOrigin, HtmlParseOptions, HtmlParser,
        parse_document,
    };
    use crate::{DomPatch, PatchKey};

    fn summarize(node: &crate::Node, out: &mut Vec<String>) {
        match node {
            crate::Node::Document {
                doctype, children, ..
            } => {
                out.push(format!("document:{:?}", doctype));
                for child in children {
                    summarize(child, out);
                }
            }
            crate::Node::Element {
                name,
                attributes,
                children,
                ..
            } => {
                out.push(format!("element:{name}:{}", attributes.len()));
                for child in children {
                    summarize(child, out);
                }
            }
            crate::Node::Text { text, .. } => out.push(format!("text:{text}")),
            crate::Node::Comment { text, .. } => out.push(format!("comment:{text}")),
        }
    }

    #[test]
    fn parse_document_materializes_html5_dom_and_patch_stream() {
        let output = parse_document(
            "<!doctype html><div class=hero>Hello</div>",
            HtmlParseOptions::default(),
        )
        .expect("one-shot parse should succeed");

        let mut summary = Vec::new();
        summarize(&output.document, &mut summary);

        assert!(summary.iter().any(|line| line == "element:div:1"));
        assert!(summary.iter().any(|line| line == "text:Hello"));
        assert!(output.contains_full_patch_history);
        assert!(
            output.patches.iter().any(|patch| matches!(
                patch,
                crate::DomPatch::CreateElement { name, .. } if name.as_ref() == "div"
            )),
            "expected a div create patch"
        );
    }

    #[test]
    fn chunked_parser_session_matches_one_shot_output() {
        let input = "<div><span>alpha</span><span>beta</span></div>";
        let mut parser = HtmlParser::new(HtmlParseOptions::default()).expect("session init");

        parser.push_bytes(b"<div><span>alpha").expect("first chunk");
        parser.pump().expect("first pump");
        let first_batch = parser
            .take_patch_batch()
            .expect("first batch drain should succeed");
        assert!(first_batch.is_some(), "expected patches after first chunk");

        parser
            .push_bytes(b"</span><span>beta</span></div>")
            .expect("second chunk");
        parser.finish().expect("finish");
        let chunked = parser.into_output().expect("chunked output");
        let whole = parse_document(input, HtmlParseOptions::default()).expect("whole output");

        let mut chunked_summary = Vec::new();
        summarize(&chunked.document, &mut chunked_summary);
        let mut whole_summary = Vec::new();
        summarize(&whole.document, &mut whole_summary);

        assert_eq!(chunked_summary, whole_summary);
        assert_eq!(
            chunked.counters.tokens_processed,
            whole.counters.tokens_processed
        );
        assert!(!chunked.contains_full_patch_history);
    }

    #[test]
    fn finish_is_required_to_flush_eof_sensitive_text_mode_content() {
        let mut parser = HtmlParser::new(HtmlParseOptions::default()).expect("session init");
        parser.push_str("<style>body{color:red").expect("push");
        parser.pump().expect("pump");

        let before_finish = parser.take_patches().expect("drain before finish");
        assert!(
            !before_finish
                .iter()
                .any(|patch| matches!(patch, crate::DomPatch::CreateText { .. })),
            "rawtext content should not be flushed before finish()"
        );

        parser.finish().expect("finish");
        let after_finish = parser.take_patches().expect("drain after finish");
        assert!(
            after_finish
                .iter()
                .any(|patch| matches!(patch, crate::DomPatch::CreateText { text, .. } if text == "body{color:red" )),
            "finish() must flush EOF-sensitive text-mode content"
        );
    }

    #[test]
    fn take_patches_and_take_patch_batch_materialize_the_same_dom() {
        let input = "<div><span>a</span><span>b</span><span>c</span></div>";

        let mut vec_parser = HtmlParser::new(HtmlParseOptions::default()).expect("vec parser init");
        vec_parser.push_bytes(input.as_bytes()).expect("vec push");
        vec_parser.finish().expect("vec finish");
        let drained = vec_parser.take_patches().expect("vec drain");
        assert!(!drained.is_empty(), "expected drained patches");
        let vec_output = vec_parser.into_output().expect("vec output");

        let mut batch_parser =
            HtmlParser::new(HtmlParseOptions::default()).expect("batch parser init");
        batch_parser
            .push_bytes(input.as_bytes())
            .expect("batch push");
        batch_parser.finish().expect("batch finish");
        let mut batch_count = 0usize;
        while let Some(batch) = batch_parser
            .take_patch_batch()
            .expect("batch drain should succeed")
        {
            batch_count += 1;
            assert!(
                !batch.patches.is_empty(),
                "empty batches must not be emitted"
            );
        }
        let batch_output = batch_parser.into_output().expect("batch output");

        let mut vec_summary = Vec::new();
        summarize(&vec_output.document, &mut vec_summary);
        let mut batch_summary = Vec::new();
        summarize(&batch_output.document, &mut batch_summary);

        assert_eq!(vec_summary, batch_summary);
        assert!(!vec_output.contains_full_patch_history);
        assert!(!batch_output.contains_full_patch_history);
        assert!(batch_count > 0, "expected at least one emitted batch");
    }

    #[test]
    fn into_output_only_returns_undrained_patch_remainder() {
        let input = "<div><span>alpha</span><span>beta</span></div>";
        let mut parser = HtmlParser::new(HtmlParseOptions::default()).expect("session init");

        parser.push_bytes(b"<div><span>alpha").expect("first chunk");
        parser.pump().expect("first pump");
        let drained_first = parser.take_patches().expect("first drain");
        assert!(!drained_first.is_empty(), "expected early patches");

        parser
            .push_bytes(b"</span><span>beta</span></div>")
            .expect("second chunk");
        parser.finish().expect("finish");
        let output = parser.into_output().expect("output");
        let full_output =
            parse_document(input, HtmlParseOptions::default()).expect("full one-shot output");

        assert!(
            output.patches.len() < full_output.patches.len(),
            "output patches should represent only the undrained remainder"
        );
        assert!(
            !output.contains_full_patch_history,
            "partial draining must mark output patch history as incomplete"
        );
    }

    #[test]
    fn parser_surface_exposes_parse_events_without_html5_types() {
        let mut options = HtmlParseOptions::default();
        options.tokenizer.limits.max_tag_name_bytes = 3;
        options.error_policy = HtmlErrorPolicy {
            track: true,
            max_stored: 16,
            debug_only: false,
            track_counters: true,
        };

        let output = parse_document("<abcdef>text</abcdef>", options).expect("parse should work");
        assert!(
            !output.parse_errors.is_empty(),
            "expected surfaced parse event"
        );
        assert_eq!(
            output.parse_errors[0].origin,
            HtmlParseEventOrigin::Tokenizer
        );
        assert_eq!(
            output.parse_errors[0].code,
            HtmlParseEventCode::ResourceLimit
        );
        assert_eq!(output.parse_errors[0].detail, Some("tag-name-truncated"));
    }

    #[test]
    fn patch_validation_failure_poisons_parser_for_future_mutation_and_drains() {
        let mut parser = HtmlParser::new(HtmlParseOptions::default()).expect("session init");

        let err = parser
            .apply_patches(&[DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            }])
            .expect_err("invalid patch batch should fail");
        assert!(
            matches!(err, crate::HtmlParseError::PatchValidation(_)),
            "expected patch validation failure, got {err:?}"
        );

        assert_eq!(
            parser.push_bytes(b"<div>").unwrap_err(),
            crate::HtmlParseError::Invariant
        );
        assert_eq!(
            parser.push_str("<span>").unwrap_err(),
            crate::HtmlParseError::Invariant
        );
        assert_eq!(parser.pump().unwrap_err(), crate::HtmlParseError::Invariant);
        assert_eq!(
            parser.finish().unwrap_err(),
            crate::HtmlParseError::Invariant
        );
        assert_eq!(
            parser.take_patches().unwrap_err(),
            crate::HtmlParseError::Invariant
        );
        assert_eq!(
            parser.take_patch_batch().unwrap_err(),
            crate::HtmlParseError::Invariant
        );
    }
}
