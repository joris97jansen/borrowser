//! Document-level parse context and parser-owned observation fanout.

#[cfg(any(test, feature = "parser-conformance"))]
use super::ParserObservationCapture;
use super::{
    AtomTable, Counters, DiagnosticEventMetadata, ErrorOrigin, ErrorPolicy, EventPosition,
    ImplementationDiagnosticEvent, Input, LegacyParseErrorCode, NormalizedPositionIndex,
    ObservationPositionResolution, ObservationPositionSource, ParseError, ParseErrorCode,
    ParseErrorEvent, ParserGuardrail, ParserGuardrailPayload, ParserObservationConfig,
    ParserObservationRecorder, ParserRecoveryAction, ParserResourceLimit,
    ParserResourceLimitPayload, ParserStage, Utf8ReplacementPayload, Utf8ReplacementReason,
    WhatwgParseErrorCode,
};
use std::collections::VecDeque;

/// Document-level resources shared by the production tokenizer and tree builder.
#[derive(Debug)]
pub struct DocumentParseContext {
    pub atoms: AtomTable,
    pub counters: Counters,
    pub error_policy: ErrorPolicy,
    pub errors: Option<VecDeque<ParseError>>,
    pub(crate) observations: Option<ParserObservationRecorder>,
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
        };
        if ctx.error_policy.track
            && ctx.error_policy.max_stored != 0
            && (!ctx.error_policy.debug_only || cfg!(debug_assertions))
        {
            ctx.errors = Some(VecDeque::new());
        }
        ctx
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

    /// Shared parser-error fanout. AE13b2 can add a tree-construction wrapper
    /// without introducing another occurrence sequence or retention policy.
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
        if self.error_policy.track_counters {
            self.counters.parse_errors = self.counters.parse_errors.saturating_add(1);
        }

        if let Some(occurrence) = self
            .observations
            .as_mut()
            .and_then(ParserObservationRecorder::reserve_parse_error)
            && let Some(event_position) = self.observation_position_at(input, position)
        {
            self.observations
                .as_mut()
                .expect("parse-error reservation requires an observation recorder")
                .retain_parse_error(ParseErrorEvent {
                    occurrence,
                    stage,
                    code,
                    recovery,
                    position: event_position,
                    context: None,
                    description,
                });
        }

        self.record_legacy(ParseError {
            origin: legacy_origin,
            code: legacy_code_override.unwrap_or_else(|| legacy_parse_error_code(code)),
            position,
            detail: description,
            aux: legacy_aux,
        });
    }

    pub(crate) fn record_resource_limit(
        &mut self,
        input: &Input,
        limit: ParserResourceLimit,
        configured_limit: usize,
        position: usize,
        description: Option<&'static str>,
    ) {
        if let Some(occurrence) = self.reserve_implementation_diagnostic()
            && let Some(event_position) = self.observation_position_at(input, position)
        {
            self.retain_implementation_diagnostic(
                ImplementationDiagnosticEvent::ParserResourceLimitActivated {
                    metadata: DiagnosticEventMetadata {
                        occurrence,
                        stage: ParserStage::Tokenizer,
                        position: event_position,
                        context: None,
                        description,
                    },
                    limit,
                    payload: ParserResourceLimitPayload {
                        configured_limit: u64::try_from(configured_limit)
                            .expect("supported Rust targets represent usize in u64"),
                    },
                },
            );
        }
        self.record_legacy(ParseError {
            origin: ErrorOrigin::Tokenizer,
            code: LegacyParseErrorCode::ResourceLimit,
            position,
            detail: description,
            aux: Some(configured_limit.min(u32::MAX as usize) as u32),
        });
    }

    pub(crate) fn record_guardrail(
        &mut self,
        input: &Input,
        guardrail: ParserGuardrail,
        consecutive_steps: usize,
        position: usize,
        description: Option<&'static str>,
    ) {
        if let Some(occurrence) = self.reserve_implementation_diagnostic()
            && let Some(event_position) = self.observation_position_at(input, position)
        {
            self.retain_implementation_diagnostic(
                ImplementationDiagnosticEvent::ParserGuardrailActivated {
                    metadata: DiagnosticEventMetadata {
                        occurrence,
                        stage: ParserStage::Tokenizer,
                        position: event_position,
                        context: None,
                        description,
                    },
                    guardrail,
                    payload: ParserGuardrailPayload {
                        consecutive_stall_steps: std::num::NonZeroU64::new(
                            u64::try_from(consecutive_steps)
                                .expect("supported Rust targets represent usize in u64"),
                        )
                        .expect("a triggered guardrail has a non-zero stall count"),
                    },
                },
            );
        }
        self.record_legacy(ParseError {
            origin: ErrorOrigin::Tokenizer,
            code: LegacyParseErrorCode::ImplementationGuardrail,
            position,
            detail: description,
            aux: Some(consecutive_steps.min(u32::MAX as usize) as u32),
        });
    }

    pub(crate) fn reserve_implementation_diagnostic(&mut self) -> Option<u64> {
        self.observations
            .as_mut()
            .and_then(ParserObservationRecorder::reserve_implementation_diagnostic)
    }

    /// Mandatory parser accounting for decoder-generated replacement scalars.
    /// This is deliberately independent of optional canonical observation.
    pub(crate) fn record_decode_replacements(&mut self, replacements: u64) {
        self.counters.decode_errors = self.counters.decode_errors.saturating_add(replacements);
    }

    pub(crate) fn retain_utf8_replacement(
        &mut self,
        occurrence: u64,
        reason: Utf8ReplacementReason,
        payload: Utf8ReplacementPayload,
        position: EventPosition,
    ) {
        self.retain_implementation_diagnostic(ImplementationDiagnosticEvent::InvalidUtf8Replaced {
            metadata: DiagnosticEventMetadata {
                occurrence,
                stage: ParserStage::InputPreprocessing(
                    super::InputPreprocessingStage::Utf8Decoding,
                ),
                position,
                context: None,
                description: Some(match reason {
                    Utf8ReplacementReason::InvalidSequence => "invalid-utf8-sequence-replaced",
                    Utf8ReplacementReason::IncompleteSequenceAtEof => {
                        "incomplete-utf8-sequence-at-eof-replaced"
                    }
                }),
            },
            reason,
            payload,
        });
    }

    fn retain_implementation_diagnostic(&mut self, event: ImplementationDiagnosticEvent) {
        self.observations
            .as_mut()
            .expect("diagnostic reservation requires an observation recorder")
            .retain_implementation_diagnostic(event);
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

    pub(crate) fn observation_position_at(
        &mut self,
        input: &Input,
        position: usize,
    ) -> Option<EventPosition> {
        let recorder = self
            .observations
            .as_mut()
            .expect("position resolution follows a successful observation reservation");
        match recorder.resolve_position(
            input.as_str(),
            ObservationPositionSource::NormalizedOffset(position),
        ) {
            ObservationPositionResolution::Known(position) => Some(EventPosition::Known(position)),
            ObservationPositionResolution::GenuinelyUnavailable(reason) => {
                Some(EventPosition::Unavailable(reason))
            }
            ObservationPositionResolution::InvariantFailure(_) => None,
        }
    }

    #[cfg(any(test, feature = "parser-conformance"))]
    pub(crate) fn take_observations(&mut self) -> Option<ParserObservationCapture> {
        self.observations
            .take()
            .map(ParserObservationRecorder::finish)
    }

    fn record_legacy(&mut self, error: ParseError) {
        if !self.error_policy.track {
            return;
        }
        if self.error_policy.debug_only && !cfg!(debug_assertions) {
            return;
        }
        if self.error_policy.max_stored == 0 {
            return;
        }
        let errors = self.errors.get_or_insert_with(VecDeque::new);
        if errors.len() >= self.error_policy.max_stored {
            errors.pop_front();
            self.counters.errors_dropped = self.counters.errors_dropped.saturating_add(1);
        }
        errors.push_back(error);
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
            super::TokenizerExtensionParseErrorCode::EofInTextMode => {
                LegacyParseErrorCode::UnexpectedEof
            }
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
        ByteStreamDecoder, ImplementationDiagnosticCode, SurfaceCaptureRequest,
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
            parse_capture.invariant,
            Some(crate::html5::shared::ParserObservationInvariant::NormalizedPositionIndexMissing)
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
            diagnostic_capture.invariant,
            Some(crate::html5::shared::ParserObservationInvariant::NormalizedPositionIndexMissing)
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
            capture.invariant,
            Some(super::super::ParserObservationInvariant::InvalidNormalizedPositionOffset)
        );
    }
}
