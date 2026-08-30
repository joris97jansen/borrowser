//! Document-level parse context and parser-owned observation fanout.

#[cfg(all(
    feature = "parser-failure-injection",
    any(test, feature = "internal-api")
))]
use super::ParserFailureInjection;
#[cfg(any(test, feature = "parser-conformance"))]
use super::ParserObservationCapture;
use super::{
    AtomTable, Counters, ErrorOrigin, ErrorPolicy, Input, LegacyParseErrorCode,
    NormalizedPositionIndex, ParseError, ParseErrorCode, ParserEventSink, ParserGuardrail,
    ParserObservationConfig, ParserObservationRecorder, ParserRecoveryAction, ParserResourceLimit,
    ParserStage, Utf8ReplacementPayload, Utf8ReplacementReason, WhatwgParseErrorCode,
};
use super::{
    HtmlParseSemanticCompleteness, HtmlParseSemanticCompletenessTracker, guardrail_degradation,
    resource_limit_degradation,
};
use super::{ParserReservationController, ParserReservationSite, ParserResourceExhaustion};
use std::collections::VecDeque;

/// Document-level resources shared by the production tokenizer and tree builder.
#[derive(Debug)]
pub struct DocumentParseContext {
    pub atoms: AtomTable,
    pub counters: Counters,
    pub error_policy: ErrorPolicy,
    pub errors: Option<VecDeque<ParseError>>,
    pub(crate) observations: Option<ParserObservationRecorder>,
    pub(crate) reservations: ParserReservationController,
    pub(crate) semantic_completeness: HtmlParseSemanticCompletenessTracker,
}

impl Default for DocumentParseContext {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentParseContext {
    pub fn new() -> Self {
        Self::with_error_policy(ErrorPolicy::default())
    }

    pub fn with_error_policy(error_policy: ErrorPolicy) -> Self {
        Self::build(error_policy, ParserObservationConfig::default())
    }

    #[cfg(any(test, feature = "parser-conformance"))]
    pub(crate) fn with_observations(
        error_policy: ErrorPolicy,
        observations: ParserObservationConfig,
    ) -> Self {
        Self::build(error_policy, observations)
    }

    fn build(error_policy: ErrorPolicy, observations: ParserObservationConfig) -> Self {
        let mut ctx = Self {
            atoms: AtomTable::default(),
            counters: Counters::default(),
            error_policy,
            errors: None,
            observations: ParserObservationRecorder::new(observations),
            reservations: ParserReservationController::default(),
            semantic_completeness: HtmlParseSemanticCompletenessTracker::default(),
        };
        if ctx.error_policy.track
            && ctx.error_policy.max_stored != 0
            && (!ctx.error_policy.debug_only || cfg!(debug_assertions))
        {
            ctx.errors = Some(VecDeque::new());
        }
        ctx
    }

    #[cfg(all(
        feature = "parser-failure-injection",
        any(test, feature = "internal-api")
    ))]
    pub(crate) fn with_failure_injection(
        error_policy: ErrorPolicy,
        injection: ParserFailureInjection,
    ) -> Self {
        let mut ctx = Self::build(error_policy, ParserObservationConfig::default());
        ctx.reservations = ParserReservationController::with_failure(injection);
        ctx
    }

    #[cfg(all(
        test,
        feature = "parser-conformance",
        feature = "parser-failure-injection"
    ))]
    pub(crate) fn with_observations_and_failure_injection(
        error_policy: ErrorPolicy,
        observations: ParserObservationConfig,
        injection: ParserFailureInjection,
    ) -> Self {
        let mut ctx = Self::build(error_policy, observations);
        ctx.reservations = ParserReservationController::with_failure(injection);
        ctx
    }

    #[inline]
    pub(crate) fn before_reservation(
        &mut self,
        site: ParserReservationSite,
    ) -> Result<(), ParserResourceExhaustion> {
        self.reservations.before_reservation(site)
    }

    /// Record one exact recoverable tokenizer error at its production source.
    pub(crate) fn record_tokenizer_parse_error(
        &mut self,
        input: &Input,
        code: ParseErrorCode,
        position: usize,
        recovery: Option<ParserRecoveryAction>,
        description: Option<&'static str>,
        legacy_aux: Option<u32>,
    ) {
        self.record_parse_error_internal(
            input,
            ParserStage::Tokenizer,
            ErrorOrigin::Tokenizer,
            code,
            position,
            recovery,
            description,
            legacy_aux,
            None,
        );
    }

    /// Preserve a pre-existing broad facade category when the exact Standard
    /// identity is more specific than the legacy model. The override affects
    /// only legacy retention; canonical identity is retained directly from
    /// `code`.
    pub(crate) fn record_tokenizer_parse_error_with_legacy_projection(
        &mut self,
        input: &Input,
        code: ParseErrorCode,
        position: usize,
        recovery: Option<ParserRecoveryAction>,
        description: Option<&'static str>,
        legacy_code: LegacyParseErrorCode,
    ) {
        self.record_parse_error_internal(
            input,
            ParserStage::Tokenizer,
            ErrorOrigin::Tokenizer,
            code,
            position,
            recovery,
            description,
            None,
            Some(legacy_code),
        );
    }

    /// Shared parser-error fanout used by exact-position tokenizer sources.
    ///
    /// Tree construction reaches the same fanout through its narrow process
    /// context, with an explicitly unavailable canonical position and no
    /// inexact legacy projection.
    #[allow(
        clippy::too_many_arguments,
        reason = "the core fanout keeps canonical source metadata and legacy projection inputs explicit"
    )]
    fn record_parse_error_internal(
        &mut self,
        input: &Input,
        stage: ParserStage,
        legacy_origin: ErrorOrigin,
        code: ParseErrorCode,
        position: usize,
        recovery: Option<ParserRecoveryAction>,
        description: Option<&'static str>,
        legacy_aux: Option<u32>,
        legacy_code_override: Option<LegacyParseErrorCode>,
    ) {
        let mut sink = ParserEventSink::new(
            &mut self.counters,
            self.error_policy,
            &mut self.errors,
            &mut self.observations,
        );
        sink.record_parse_error_known(
            input,
            stage,
            code,
            position,
            recovery,
            None,
            description,
            Some(super::LegacyDiagnosticProjection {
                origin: legacy_origin,
                code: legacy_code_override.unwrap_or_else(|| legacy_parse_error_code(code)),
                position,
                detail: description,
                aux: legacy_aux,
            }),
        );
    }

    pub(crate) fn record_resource_limit(
        &mut self,
        input: &Input,
        limit: ParserResourceLimit,
        configured_limit: usize,
        position: usize,
        description: Option<&'static str>,
    ) {
        if let Some(reason) = resource_limit_degradation(limit) {
            self.semantic_completeness.record(reason);
        }
        let mut sink = ParserEventSink::new(
            &mut self.counters,
            self.error_policy,
            &mut self.errors,
            &mut self.observations,
        );
        sink.record_tokenizer_resource_limit(
            input,
            limit,
            configured_limit,
            position,
            description,
            super::LegacyDiagnosticProjection {
                origin: ErrorOrigin::Tokenizer,
                code: LegacyParseErrorCode::ResourceLimit,
                position,
                detail: description,
                aux: Some(configured_limit.min(u32::MAX as usize) as u32),
            },
        );
    }

    pub(crate) fn record_guardrail(
        &mut self,
        input: &Input,
        guardrail: ParserGuardrail,
        consecutive_steps: usize,
        position: usize,
        description: Option<&'static str>,
    ) {
        self.semantic_completeness
            .record(guardrail_degradation(guardrail));
        let mut sink = ParserEventSink::new(
            &mut self.counters,
            self.error_policy,
            &mut self.errors,
            &mut self.observations,
        );
        sink.record_tokenizer_guardrail(
            input,
            guardrail,
            consecutive_steps,
            position,
            description,
            super::LegacyDiagnosticProjection {
                origin: ErrorOrigin::Tokenizer,
                code: LegacyParseErrorCode::ImplementationGuardrail,
                position,
                detail: description,
                aux: Some(consecutive_steps.min(u32::MAX as usize) as u32),
            },
        );
    }

    pub(crate) fn semantic_completeness(&self) -> HtmlParseSemanticCompleteness {
        self.semantic_completeness.status()
    }

    pub(crate) fn record_utf8_replacement(
        &mut self,
        input: &Input,
        position: usize,
        reason: Utf8ReplacementReason,
        payload: Utf8ReplacementPayload,
    ) {
        let mut sink = ParserEventSink::new(
            &mut self.counters,
            self.error_policy,
            &mut self.errors,
            &mut self.observations,
        );
        sink.record_utf8_replacement(input, position, reason, payload);
    }

    /// Mandatory accounting for decoder replacements when canonical
    /// observation is not installed.
    pub(crate) fn record_decode_replacements(&mut self, replacements: u64) {
        self.counters.decode_errors = self.counters.decode_errors.saturating_add(replacements);
    }

    pub(crate) fn observation_position_index_mut(
        &mut self,
    ) -> Option<&mut NormalizedPositionIndex> {
        self.observations
            .as_mut()
            .and_then(ParserObservationRecorder::position_index_mut)
    }

    pub(crate) fn observation_enabled(&self) -> bool {
        self.observations.is_some()
    }

    #[cfg(test)]
    pub(crate) fn remove_observation_position_index_for_test(&mut self) {
        self.observations
            .as_mut()
            .expect("test requires observation recorder")
            .remove_position_index_for_test();
    }

    #[cfg(test)]
    pub(crate) fn enable_tree_observations_for_test(&mut self) {
        if self.observations.is_none() {
            self.observations = ParserObservationRecorder::new(ParserObservationConfig {
                tokens: super::SurfaceCaptureRequest::NotRequested,
                parse_errors: super::SurfaceCaptureRequest::Capture { capacity: 4_096 },
                implementation_diagnostics: super::SurfaceCaptureRequest::Capture {
                    capacity: 4_096,
                },
                ..ParserObservationConfig::default()
            });
        }
    }

    /// Explicit unit-test setup for direct tree-builder diagnostics.
    ///
    /// Unlike `TreeBuilderProcessContext::new`, this helper intentionally
    /// installs the production recorder with finite per-surface capacity.
    #[cfg(test)]
    pub(crate) fn with_tree_observations_for_test() -> Self {
        let mut context = Self::new();
        context.enable_tree_observations_for_test();
        context
    }

    #[cfg(test)]
    pub(crate) fn set_next_parse_error_occurrence_for_test(&mut self, next: u64) {
        self.observations
            .as_mut()
            .expect("test requires an observation recorder")
            .set_next_parse_error_occurrence_for_test(next);
    }

    #[cfg(test)]
    pub(crate) fn take_tree_parse_errors_for_test(&mut self) -> Vec<super::ParseErrorEvent> {
        self.take_observations()
            .map(|capture| capture.parse_errors.items)
            .unwrap_or_default()
            .into_iter()
            .filter(|event| event.stage == ParserStage::TreeConstruction)
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn take_tree_parse_error_descriptions_for_test(&mut self) -> Vec<&'static str> {
        self.take_tree_parse_errors_for_test()
            .into_iter()
            .filter_map(|event| event.description)
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn take_tree_implementation_diagnostic_descriptions_for_test(
        &mut self,
    ) -> Vec<&'static str> {
        self.take_observations()
            .map(|capture| capture.implementation_diagnostics.items)
            .unwrap_or_default()
            .into_iter()
            .filter(|event| event.metadata().stage == ParserStage::TreeConstruction)
            .filter_map(|event| event.metadata().description)
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn take_tree_diagnostic_descriptions_for_test(
        &mut self,
    ) -> (Vec<&'static str>, Vec<&'static str>) {
        let Some(capture) = self.take_observations() else {
            return (Vec::new(), Vec::new());
        };
        let parse_errors = capture
            .parse_errors
            .items
            .into_iter()
            .filter(|event| event.stage == ParserStage::TreeConstruction)
            .filter_map(|event| event.description)
            .collect();
        let implementation_diagnostics = capture
            .implementation_diagnostics
            .items
            .into_iter()
            .filter(|event| event.metadata().stage == ParserStage::TreeConstruction)
            .filter_map(|event| event.metadata().description)
            .collect();
        (parse_errors, implementation_diagnostics)
    }

    #[cfg(any(test, feature = "parser-conformance"))]
    pub(crate) fn take_observations(&mut self) -> Option<ParserObservationCapture> {
        self.observations
            .take()
            .map(ParserObservationRecorder::finish)
    }

    pub fn errors(&self) -> Vec<ParseError> {
        self.errors
            .as_ref()
            .map(|errors| errors.iter().cloned().collect())
            .unwrap_or_default()
    }
}

fn legacy_parse_error_code(code: ParseErrorCode) -> LegacyParseErrorCode {
    match code {
        ParseErrorCode::TokenizerExtension(code) => match code {
            super::TokenizerExtensionParseErrorCode::MalformedNumericCharacterReference => {
                LegacyParseErrorCode::InvalidCharacterReference
            }
            super::TokenizerExtensionParseErrorCode::DroppedGraveAccentBeforeAttributeName
            | super::TokenizerExtensionParseErrorCode::GraveAccentInAttributeName
            | super::TokenizerExtensionParseErrorCode::DroppedQuestionMarkBeforeAttributeName
            | super::TokenizerExtensionParseErrorCode::
                TerminatedUnquotedAttributeValueBeforeQuestionMark => {
                LegacyParseErrorCode::Other
            }
        },
        ParseErrorCode::TreeConstruction(_) => LegacyParseErrorCode::Other,
        ParseErrorCode::Standard(code) => legacy_standard_parse_error_code(code),
    }
}

fn legacy_standard_parse_error_code(code: WhatwgParseErrorCode) -> LegacyParseErrorCode {
    match code {
        WhatwgParseErrorCode::UnexpectedNullCharacter => {
            LegacyParseErrorCode::UnexpectedNullCharacter
        }
        WhatwgParseErrorCode::EofBeforeTagName
        | WhatwgParseErrorCode::EofInTag
        | WhatwgParseErrorCode::EofInComment
        | WhatwgParseErrorCode::EofInDoctype
        | WhatwgParseErrorCode::EofInCdata
        | WhatwgParseErrorCode::EofInProcessingInstruction => LegacyParseErrorCode::UnexpectedEof,
        WhatwgParseErrorCode::MissingSemicolonAfterCharacterReference
        | WhatwgParseErrorCode::UnknownNamedCharacterReference
        | WhatwgParseErrorCode::AbsenceOfDigitsInNumericCharacterReference
        | WhatwgParseErrorCode::NullCharacterReference
        | WhatwgParseErrorCode::CharacterReferenceOutsideUnicodeRange
        | WhatwgParseErrorCode::SurrogateCharacterReference
        | WhatwgParseErrorCode::NoncharacterCharacterReference
        | WhatwgParseErrorCode::ControlCharacterReference => {
            LegacyParseErrorCode::InvalidCharacterReference
        }
        _ => LegacyParseErrorCode::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html5::shared::{
        ByteStreamDecoder, ImplementationDiagnosticCode, ImplementationDiagnosticEvent,
        SurfaceCaptureRequest,
    };

    fn observed_context(
        parse_capacity: usize,
        implementation_capacity: usize,
    ) -> DocumentParseContext {
        DocumentParseContext::with_observations(
            ErrorPolicy {
                debug_only: false,
                ..ErrorPolicy::default()
            },
            ParserObservationConfig {
                parse_errors: SurfaceCaptureRequest::Capture {
                    capacity: parse_capacity,
                },
                implementation_diagnostics: SurfaceCaptureRequest::Capture {
                    capacity: implementation_capacity,
                },
                ..ParserObservationConfig::default()
            },
        )
    }

    #[test]
    fn exact_identity_is_not_reconstructed_from_legacy_metadata() {
        let mut input = Input::new();
        input.push_str("<x>");
        let mut ctx = observed_context(4, 0);
        ctx.record_tokenizer_parse_error(
            &input,
            ParseErrorCode::Standard(WhatwgParseErrorCode::DuplicateAttribute),
            1,
            Some(ParserRecoveryAction::DropDuplicateAttribute),
            Some("unexpected-eof"),
            Some(u32::MAX),
        );

        let legacy = ctx.errors();
        assert_eq!(legacy[0].code, LegacyParseErrorCode::Other);
        let capture = ctx.take_observations().unwrap();
        assert_eq!(
            capture.parse_errors.items[0].code,
            ParseErrorCode::Standard(WhatwgParseErrorCode::DuplicateAttribute)
        );
        assert_eq!(capture.parse_errors.items[0].occurrence, 1);
    }

    #[test]
    fn missing_position_index_prevents_parse_and_diagnostic_retention() {
        let mut input = Input::new();
        input.push_str("<x>");

        let mut parse_ctx = observed_context(1, 0);
        parse_ctx.remove_observation_position_index_for_test();
        parse_ctx.record_tokenizer_parse_error(
            &input,
            ParseErrorCode::Standard(WhatwgParseErrorCode::UnexpectedNullCharacter),
            1,
            Some(ParserRecoveryAction::ReplaceInvalidInput),
            Some("test-parse-position"),
            None,
        );
        let parse_capture = parse_ctx.take_observations().expect("parse capture");
        assert!(parse_capture.parse_errors.items.is_empty());
        assert_eq!(
            parse_capture.failure,
            Some(crate::html5::shared::ParserObservationFailure::Invariant(
                crate::html5::shared::ParserObservationInvariant::NormalizedPositionIndexMissing
            ))
        );

        let mut diagnostic_ctx = observed_context(0, 1);
        diagnostic_ctx.remove_observation_position_index_for_test();
        diagnostic_ctx.record_resource_limit(
            &input,
            ParserResourceLimit::TagNameBytes,
            8,
            1,
            Some("test-diagnostic-position"),
        );
        let diagnostic_capture = diagnostic_ctx
            .take_observations()
            .expect("diagnostic capture");
        assert!(
            diagnostic_capture
                .implementation_diagnostics
                .items
                .is_empty()
        );
        assert_eq!(
            diagnostic_capture.failure,
            Some(crate::html5::shared::ParserObservationFailure::Invariant(
                crate::html5::shared::ParserObservationInvariant::NormalizedPositionIndexMissing
            ))
        );
    }

    #[test]
    fn implementation_diagnostics_do_not_increment_parse_error_counter() {
        let mut input = Input::new();
        input.push_str("<abcdef>");
        let mut ctx = observed_context(0, 4);
        ctx.record_resource_limit(
            &input,
            ParserResourceLimit::TagNameBytes,
            3,
            4,
            Some("tag-name-truncated"),
        );
        ctx.record_guardrail(
            &input,
            ParserGuardrail::TokenizerStallRecovery,
            7,
            5,
            Some("tokenizer-stall-recovery"),
        );

        assert_eq!(ctx.counters.parse_errors, 0);
        assert_eq!(
            ctx.errors()
                .iter()
                .map(|error| (error.code, error.aux))
                .collect::<Vec<_>>(),
            vec![
                (LegacyParseErrorCode::ResourceLimit, Some(3)),
                (LegacyParseErrorCode::ImplementationGuardrail, Some(7)),
            ]
        );
        let capture = ctx.take_observations().unwrap();
        assert_eq!(
            capture
                .implementation_diagnostics
                .items
                .iter()
                .map(ImplementationDiagnosticEvent::code)
                .collect::<Vec<_>>(),
            vec![
                ImplementationDiagnosticCode::ParserResourceLimitActivated(
                    ParserResourceLimit::TagNameBytes
                ),
                ImplementationDiagnosticCode::ParserGuardrailActivated(
                    ParserGuardrail::TokenizerStallRecovery
                ),
            ]
        );
        assert_eq!(
            capture
                .implementation_diagnostics
                .items
                .iter()
                .map(ImplementationDiagnosticEvent::occurrence)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn legacy_aux_saturates_without_narrowing_canonical_diagnostic_payloads() {
        let mut input = Input::new();
        input.push_str("<x>");
        let mut ctx = observed_context(0, 2);
        let resource_value = u32::MAX as usize + 41;
        let guardrail_value = u32::MAX as usize + 73;
        ctx.record_resource_limit(
            &input,
            ParserResourceLimit::TagNameBytes,
            resource_value,
            1,
            Some("tag-name-truncated"),
        );
        ctx.record_guardrail(
            &input,
            ParserGuardrail::TokenizerStallRecovery,
            guardrail_value,
            2,
            Some("tokenizer-stall-recovery"),
        );

        assert_eq!(
            ctx.errors()
                .iter()
                .map(|error| error.aux)
                .collect::<Vec<_>>(),
            vec![Some(u32::MAX), Some(u32::MAX)]
        );
        // The compatibility buffer is deliberately independent and lossy.
        ctx.errors.as_mut().expect("tracked legacy errors")[0].aux = Some(0);
        ctx.errors.as_mut().expect("tracked legacy errors")[1].aux = None;

        let capture = ctx.take_observations().expect("canonical capture");
        let [
            ImplementationDiagnosticEvent::ParserResourceLimitActivated {
                payload: resource_payload,
                ..
            },
            ImplementationDiagnosticEvent::ParserGuardrailActivated {
                payload: guardrail_payload,
                ..
            },
        ] = capture.implementation_diagnostics.items.as_slice()
        else {
            panic!("expected resource-limit then guardrail diagnostics");
        };
        assert_eq!(
            resource_payload.configured_limit,
            u64::try_from(resource_value).unwrap()
        );
        assert_eq!(
            guardrail_payload.consecutive_stall_steps.get(),
            u64::try_from(guardrail_value).unwrap()
        );
        assert_eq!(ctx.counters.parse_errors, 0);
    }

    #[test]
    fn decoder_resource_and_guardrail_share_implementation_occurrence_sequence() {
        let mut ctx = observed_context(0, 3);
        let mut input = Input::new();
        let mut decoder = ByteStreamDecoder::new();
        let _ = decoder.push_bytes_with_context(&[0xFF], &mut input, &mut ctx);
        ctx.record_resource_limit(
            &input,
            ParserResourceLimit::TagNameBytes,
            3,
            input.as_str().len(),
            Some("tag-name-truncated"),
        );
        ctx.record_guardrail(
            &input,
            ParserGuardrail::TokenizerStallRecovery,
            4,
            input.as_str().len(),
            Some("tokenizer-stall-recovery"),
        );
        let capture = ctx.take_observations().unwrap();
        assert_eq!(
            capture
                .implementation_diagnostics
                .items
                .iter()
                .map(ImplementationDiagnosticEvent::occurrence)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(ctx.counters.decode_errors, 1);
        assert_eq!(ctx.counters.parse_errors, 0);
    }

    #[test]
    fn canonical_overflow_is_independent_of_legacy_retention_and_counters() {
        let mut input = Input::new();
        input.push_str("\0\0\0");
        let mut ctx = observed_context(1, 0);
        for position in 0..3 {
            ctx.record_tokenizer_parse_error(
                &input,
                ParseErrorCode::Standard(WhatwgParseErrorCode::UnexpectedNullCharacter),
                position,
                Some(ParserRecoveryAction::ReplaceInvalidInput),
                Some("unexpected-null-character"),
                None,
            );
        }
        assert_eq!(ctx.counters.parse_errors, 3);
        assert_eq!(ctx.counters.errors_dropped, 0);
        assert_eq!(ctx.errors().len(), 3);
        let capture = ctx.take_observations().unwrap();
        assert_eq!(capture.parse_errors.items.len(), 1);
        assert_eq!(capture.parse_errors.items[0].occurrence, 1);
        assert_eq!(capture.parse_errors.dropped, 2);
    }

    #[test]
    fn exhausted_diagnostic_surfaces_stop_normalized_position_index_growth() {
        let mut ctx = observed_context(1, 1);
        let mut input = Input::new();
        input.push_str_observed("start", ctx.observation_position_index_mut());
        ctx.record_tokenizer_parse_error(
            &input,
            ParseErrorCode::Standard(WhatwgParseErrorCode::DuplicateAttribute),
            0,
            Some(ParserRecoveryAction::DropDuplicateAttribute),
            Some("duplicate-attribute"),
            None,
        );
        ctx.record_resource_limit(
            &input,
            ParserResourceLimit::TagNameBytes,
            4,
            4,
            Some("tag-name-truncated"),
        );
        assert!(
            !ctx.observations
                .as_ref()
                .expect("recorder")
                .has_position_index()
        );

        let long_tail = "x".repeat(4 * 1024);
        input.push_str_observed(&long_tail, ctx.observation_position_index_mut());
        assert_eq!(
            ctx.observations
                .as_ref()
                .expect("recorder")
                .position_checkpoint_count(),
            0
        );
        ctx.record_tokenizer_parse_error(
            &input,
            ParseErrorCode::Standard(WhatwgParseErrorCode::DuplicateAttribute),
            input.as_str().len(),
            Some(ParserRecoveryAction::DropDuplicateAttribute),
            Some("duplicate-attribute"),
            None,
        );
        let capture = ctx.take_observations().unwrap();
        assert_eq!(capture.parse_errors.dropped, 1);
    }

    #[test]
    fn invalid_positions_retain_no_false_parse_or_implementation_events() {
        let mut input = Input::new();
        let mut ctx = observed_context(1, 2);
        input.push_str_observed("é", ctx.observation_position_index_mut());

        ctx.record_tokenizer_parse_error(
            &input,
            ParseErrorCode::Standard(WhatwgParseErrorCode::UnexpectedNullCharacter),
            1,
            Some(ParserRecoveryAction::ReplaceInvalidInput),
            Some("invalid-position-test"),
            None,
        );
        ctx.record_resource_limit(
            &input,
            ParserResourceLimit::TagNameBytes,
            8,
            1,
            Some("invalid-position-test"),
        );
        ctx.record_guardrail(
            &input,
            ParserGuardrail::TokenizerStallRecovery,
            1,
            1,
            Some("invalid-position-test"),
        );

        let capture = ctx.take_observations().expect("requested capture");
        assert!(capture.parse_errors.items.is_empty());
        assert!(capture.implementation_diagnostics.items.is_empty());
        assert_eq!(
            capture.failure,
            Some(super::super::ParserObservationFailure::Invariant(
                super::super::ParserObservationInvariant::InvalidNormalizedPositionOffset
            ))
        );
    }
}
