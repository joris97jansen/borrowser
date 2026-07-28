//! Shared parser-owned diagnostic fanout.

use super::{
    Counters, DiagnosticEventMetadata, ErrorOrigin, ErrorPolicy, EventPosition,
    ImplementationDiagnosticEvent, Input, InputPreprocessingStage, LegacyParseErrorCode,
    ObservationPositionResolution, ObservationPositionSource, ParseError, ParseErrorCode,
    ParseErrorEvent, ParserContextSummary, ParserGuardrail, ParserGuardrailPayload,
    ParserObservationRecorder, ParserRecoveryAction, ParserResourceLimit,
    ParserResourceLimitPayload, ParserStage, PositionUnavailableReason,
    TreeConstructionImplementationDiagnosticCode, Utf8ReplacementPayload, Utf8ReplacementReason,
};
use std::collections::VecDeque;

#[derive(Clone, Copy)]
pub(crate) struct LegacyDiagnosticProjection {
    pub(crate) origin: ErrorOrigin,
    pub(crate) code: LegacyParseErrorCode,
    pub(crate) position: usize,
    pub(crate) detail: Option<&'static str>,
    pub(crate) aux: Option<u32>,
}

pub(crate) struct ParserDiagnosticSink<'a> {
    counters: &'a mut Counters,
    error_policy: ErrorPolicy,
    errors: &'a mut Option<VecDeque<ParseError>>,
    observations: &'a mut Option<ParserObservationRecorder>,
}

impl<'a> ParserDiagnosticSink<'a> {
    pub(crate) fn new(
        counters: &'a mut Counters,
        error_policy: ErrorPolicy,
        errors: &'a mut Option<VecDeque<ParseError>>,
        observations: &'a mut Option<ParserObservationRecorder>,
    ) -> Self {
        Self {
            counters,
            error_policy,
            errors,
            observations,
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the shared fanout keeps canonical metadata and the optional legacy projection explicit"
    )]
    pub(crate) fn record_parse_error_known(
        &mut self,
        input: &Input,
        stage: ParserStage,
        code: ParseErrorCode,
        position: usize,
        recovery: Option<ParserRecoveryAction>,
        context: Option<ParserContextSummary>,
        description: Option<&'static str>,
        legacy: Option<LegacyDiagnosticProjection>,
    ) {
        self.record_parse_error(
            stage,
            code,
            recovery,
            context,
            description,
            |recorder| match recorder.resolve_position(
                input.as_str(),
                ObservationPositionSource::NormalizedOffset(position),
            ) {
                ObservationPositionResolution::Known(position) => {
                    Some(EventPosition::Known(position))
                }
                ObservationPositionResolution::GenuinelyUnavailable(reason) => {
                    Some(EventPosition::Unavailable(reason))
                }
                ObservationPositionResolution::InvariantFailure(_) => None,
            },
            legacy,
        );
    }

    pub(crate) fn record_tree_parse_error(
        &mut self,
        code: ParseErrorCode,
        recovery: Option<ParserRecoveryAction>,
        context: ParserContextSummary,
        description: Option<&'static str>,
    ) {
        self.record_parse_error(
            ParserStage::TreeConstruction,
            code,
            recovery,
            Some(context),
            description,
            |_| {
                Some(EventPosition::Unavailable(
                    PositionUnavailableReason::ParserDidNotProvidePosition,
                ))
            },
            None,
        );
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the sole fanout keeps canonical source metadata and optional exact-position legacy projection explicit"
    )]
    fn record_parse_error(
        &mut self,
        stage: ParserStage,
        code: ParseErrorCode,
        recovery: Option<ParserRecoveryAction>,
        context: Option<ParserContextSummary>,
        description: Option<&'static str>,
        resolve_position: impl FnOnce(&mut ParserObservationRecorder) -> Option<EventPosition>,
        legacy: Option<LegacyDiagnosticProjection>,
    ) {
        if self.error_policy.track_counters {
            self.counters.parse_errors = self.counters.parse_errors.saturating_add(1);
        }

        if let Some(recorder) = self.observations.as_mut()
            && let Some(occurrence) = recorder.reserve_parse_error()
            && let Some(position) = resolve_position(recorder)
        {
            recorder.retain_parse_error(ParseErrorEvent {
                occurrence,
                stage,
                code,
                recovery,
                position,
                context,
                description,
            });
        }

        if let Some(legacy) = legacy {
            self.record_legacy(ParseError {
                origin: legacy.origin,
                code: legacy.code,
                position: legacy.position,
                detail: legacy.detail,
                aux: legacy.aux,
            });
        }
    }

    pub(crate) fn record_tree_implementation_diagnostic(
        &mut self,
        code: TreeConstructionImplementationDiagnosticCode,
        context: ParserContextSummary,
        description: Option<&'static str>,
    ) {
        self.record_implementation_diagnostic(
            |_| {
                Some(EventPosition::Unavailable(
                    PositionUnavailableReason::ParserDidNotProvidePosition,
                ))
            },
            |occurrence, position| ImplementationDiagnosticEvent::TreeConstruction {
                metadata: DiagnosticEventMetadata {
                    occurrence,
                    stage: ParserStage::TreeConstruction,
                    position,
                    context: Some(context),
                    description,
                },
                code,
            },
        );
    }

    pub(crate) fn record_tree_resource_limit(
        &mut self,
        limit: ParserResourceLimit,
        configured_limit: usize,
        context: ParserContextSummary,
        description: Option<&'static str>,
    ) {
        self.record_implementation_diagnostic(
            |_| {
                Some(EventPosition::Unavailable(
                    PositionUnavailableReason::ParserDidNotProvidePosition,
                ))
            },
            |occurrence, position| ImplementationDiagnosticEvent::ParserResourceLimitActivated {
                metadata: DiagnosticEventMetadata {
                    occurrence,
                    stage: ParserStage::TreeConstruction,
                    position,
                    context: Some(context),
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

    pub(crate) fn record_tokenizer_resource_limit(
        &mut self,
        input: &Input,
        limit: ParserResourceLimit,
        configured_limit: usize,
        position: usize,
        description: Option<&'static str>,
        legacy: LegacyDiagnosticProjection,
    ) {
        self.record_implementation_diagnostic_known(
            input,
            position,
            |occurrence, event_position| {
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
                }
            },
        );
        self.record_legacy(ParseError {
            origin: legacy.origin,
            code: legacy.code,
            position: legacy.position,
            detail: legacy.detail,
            aux: legacy.aux,
        });
    }

    pub(crate) fn record_tokenizer_guardrail(
        &mut self,
        input: &Input,
        guardrail: ParserGuardrail,
        consecutive_steps: usize,
        position: usize,
        description: Option<&'static str>,
        legacy: LegacyDiagnosticProjection,
    ) {
        self.record_implementation_diagnostic_known(
            input,
            position,
            |occurrence, event_position| ImplementationDiagnosticEvent::ParserGuardrailActivated {
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
        self.record_legacy(ParseError {
            origin: legacy.origin,
            code: legacy.code,
            position: legacy.position,
            detail: legacy.detail,
            aux: legacy.aux,
        });
    }

    pub(crate) fn record_utf8_replacement(
        &mut self,
        input: &Input,
        position: usize,
        reason: Utf8ReplacementReason,
        payload: Utf8ReplacementPayload,
    ) {
        self.counters.decode_errors = self.counters.decode_errors.saturating_add(1);
        self.record_implementation_diagnostic_known(
            input,
            position,
            |occurrence, event_position| ImplementationDiagnosticEvent::InvalidUtf8Replaced {
                metadata: DiagnosticEventMetadata {
                    occurrence,
                    stage: ParserStage::InputPreprocessing(InputPreprocessingStage::Utf8Decoding),
                    position: event_position,
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
            },
        );
    }

    fn record_implementation_diagnostic_known(
        &mut self,
        input: &Input,
        position: usize,
        make_event: impl FnOnce(u64, EventPosition) -> ImplementationDiagnosticEvent,
    ) {
        self.record_implementation_diagnostic(
            |recorder| match recorder.resolve_position(
                input.as_str(),
                ObservationPositionSource::NormalizedOffset(position),
            ) {
                ObservationPositionResolution::Known(position) => {
                    Some(EventPosition::Known(position))
                }
                ObservationPositionResolution::GenuinelyUnavailable(reason) => {
                    Some(EventPosition::Unavailable(reason))
                }
                ObservationPositionResolution::InvariantFailure(_) => None,
            },
            make_event,
        );
    }

    fn record_implementation_diagnostic(
        &mut self,
        resolve_position: impl FnOnce(&mut ParserObservationRecorder) -> Option<EventPosition>,
        make_event: impl FnOnce(u64, EventPosition) -> ImplementationDiagnosticEvent,
    ) {
        let Some(recorder) = self.observations.as_mut() else {
            return;
        };
        let Some(occurrence) = recorder.reserve_implementation_diagnostic() else {
            return;
        };
        let Some(position) = resolve_position(recorder) else {
            return;
        };
        recorder.retain_implementation_diagnostic(make_event(occurrence, position));
    }

    fn record_legacy(&mut self, error: ParseError) {
        if !self.error_policy.track
            || (self.error_policy.debug_only && !cfg!(debug_assertions))
            || self.error_policy.max_stored == 0
        {
            return;
        }
        let errors = self.errors.get_or_insert_with(VecDeque::new);
        if errors.len() >= self.error_policy.max_stored {
            errors.pop_front();
            self.counters.errors_dropped = self.counters.errors_dropped.saturating_add(1);
        }
        errors.push_back(error);
    }
}
