use crate::dom_patch::{DomPatch, DomPatchBatch};
use crate::html5::Html5ParseSession;
use crate::html5::ParserFatalError;
use crate::html5::shared::DocumentParseContext;
#[cfg(all(
    feature = "parser-failure-injection",
    any(test, feature = "internal-api")
))]
use crate::html5::shared::ParserFailureInjection;
#[cfg(feature = "parser-conformance")]
use crate::html5::shared::{ParserObservationCapture, ParserObservationConfig};
use crate::patch_validation::PatchValidationArena;

use super::options::HtmlParseOptions;
use super::output::ParseOutput;
use super::types::{HtmlParseCounters, HtmlParseError, HtmlParseEvent};

/// Stable engine-level HTML parser API backed exclusively by the HTML5 pipeline.
///
/// If internal patch-mirror validation fails while draining emitted patches, the
/// parser transitions into a terminal poisoned state. Subsequent mutating or
/// draining operations return a fatal engine-invariant identity deterministically
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
        Self::with_context(options, ctx)
    }

    #[cfg(all(
        feature = "parser-failure-injection",
        any(test, feature = "internal-api")
    ))]
    pub(crate) fn new_with_failure_injection(
        options: HtmlParseOptions,
        injection: ParserFailureInjection,
    ) -> Result<Self, HtmlParseError> {
        let ctx =
            DocumentParseContext::with_failure_injection(options.error_policy.into(), injection);
        Self::with_context(options, ctx)
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn new_with_observations(
        options: HtmlParseOptions,
        observations: ParserObservationConfig,
    ) -> Result<Self, HtmlParseError> {
        let ctx =
            DocumentParseContext::with_observations(options.error_policy.into(), observations);
        Self::with_context(options, ctx)
    }

    #[cfg(all(
        test,
        feature = "parser-conformance",
        feature = "parser-failure-injection"
    ))]
    pub(crate) fn new_with_observations_and_failure_injection(
        options: HtmlParseOptions,
        observations: ParserObservationConfig,
        injection: ParserFailureInjection,
    ) -> Result<Self, HtmlParseError> {
        let ctx = DocumentParseContext::with_observations_and_failure_injection(
            options.error_policy.into(),
            observations,
            injection,
        );
        Self::with_context(options, ctx)
    }

    fn with_context(
        options: HtmlParseOptions,
        ctx: DocumentParseContext,
    ) -> Result<Self, HtmlParseError> {
        let session =
            Html5ParseSession::new(options.tokenizer.into(), options.tree_builder.into(), ctx)?;
        Ok(Self {
            session,
            arena: PatchValidationArena::default(),
            patches_drained_before_output: false,
            poisoned: false,
        })
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn take_observations_for_conformance(
        &mut self,
    ) -> Result<Option<ParserObservationCapture>, HtmlParseError> {
        self.ensure_not_poisoned()?;
        Ok(self.session.take_observations_for_conformance()?)
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn document_mode_for_conformance(&self) -> crate::DocumentMode {
        self.session.document_mode_for_conformance()
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn tokenizer_invariant_for_conformance(
        &self,
    ) -> Option<crate::html5::tokenizer::TokenizerInvariantKind> {
        self.session.tokenizer_invariant_for_conformance()
    }

    #[cfg(all(test, feature = "parser-conformance"))]
    pub(crate) fn inject_patch_for_conformance_test(&mut self, patch: DomPatch) {
        self.session.inject_patch_for_test(patch);
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
        let patches = self.session.take_patches()?;
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

    /// Return the currently retained exact-position legacy parse events
    /// without exposing backend `html5::*` types.
    ///
    /// This facade retains only events whose production source supplied an
    /// exact input position. Tree-construction errors with unavailable
    /// positions still increment `counters().parse_errors` and can be captured
    /// through the typed conformance observation surface, but are omitted here.
    /// Consequently callers must not assume that
    /// `counters().parse_errors == parse_errors().len()`. Omitting an
    /// unrepresentable tree event does not increment `errors_dropped`.
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

    #[cfg(test)]
    pub(crate) fn normalized_input_for_test(&self) -> &str {
        self.session.normalized_input_for_test()
    }

    #[cfg(test)]
    pub(crate) fn force_self_closing_flag_without_solidus_for_test(&mut self) {
        self.session
            .force_self_closing_flag_without_solidus_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_missing_doctype_name_start_for_test(&mut self) {
        self.session.force_missing_doctype_name_start_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_name_start_after_cursor_for_test(&mut self) {
        self.session
            .force_doctype_name_start_after_cursor_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_empty_doctype_name_range_for_test(&mut self) {
        self.session.force_empty_doctype_name_range_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_resource_start_after_cursor_for_test(&mut self) {
        self.session
            .force_doctype_resource_start_after_cursor_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_comment_end_bang_state_for_test(&mut self) {
        self.session.force_comment_end_bang_state_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_comment_state_without_pending_start_for_test(
        &mut self,
        state: crate::html5::tokenizer::TokenizerState,
    ) {
        self.session
            .force_comment_state_without_pending_start_for_test(state);
    }

    #[cfg(test)]
    pub(crate) fn force_comment_start_after_cursor_for_test(&mut self) {
        self.session.force_comment_start_after_cursor_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_cdata_end_state_for_test(
        &mut self,
        pending_text_start: Option<usize>,
        cursor: usize,
    ) {
        self.session
            .force_cdata_end_state_for_test(pending_text_start, cursor);
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_ascii_prefix_range_invalid_for_test(&mut self) {
        self.session
            .force_doctype_ascii_prefix_range_invalid_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_quoted_tail_range_invalid_for_test(&mut self) {
        self.session
            .force_doctype_quoted_tail_range_invalid_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_processing_instruction_metadata_missing_for_test(&mut self) {
        self.session
            .force_processing_instruction_metadata_missing_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_text_mode_end_tag_evidence_for_test(
        &mut self,
        candidate_start: usize,
        cursor_after: usize,
        attribute_error_position: Option<usize>,
        trailing_solidus_position: Option<usize>,
    ) {
        self.session.force_text_mode_end_tag_evidence_for_test(
            candidate_start,
            cursor_after,
            attribute_error_position,
            trailing_solidus_position,
        );
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

    pub(super) fn apply_patches(&mut self, patches: &[DomPatch]) -> Result<(), HtmlParseError> {
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
        let Some(batch) = self.session.take_patch_batch()? else {
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
            return Err(HtmlParseError::Fatal(ParserFatalError::EngineInvariant));
        }
        Ok(())
    }
}
