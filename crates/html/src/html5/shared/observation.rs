//! Passive, bounded observation storage owned by the production parser.

#[cfg(test)]
use super::TransitionTokenSummary;
use super::{
    ImplementationDiagnosticEvent, InputCoordinateSpace, InputPosition, NormalizedInputPosition,
    NormalizedLineNumber, NormalizedScalarColumn, ObservedToken, ParseErrorEvent,
    PositionUnavailableReason, SourceBytePosition, SourcePositionUnavailableReason,
    TreeTransitionEvent, UnsupportedFeatureEvent,
};

const POSITION_CHECKPOINT_STRIDE: u64 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ObservationOccurrenceSequence {
    ParseErrors,
    ImplementationDiagnostics,
    TreeTransitions,
    UnsupportedFeatures,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ObservationSurface {
    Tokens,
    ParseErrors,
    ImplementationDiagnostics,
    TreeTransitions,
    UnsupportedFeatures,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParserObservationInvariant {
    OccurrenceSequenceOverflow(ObservationOccurrenceSequence),
    DroppedCountOverflow(ObservationSurface),
    NormalizedPositionOverflow,
    NormalizedPositionIndexDiscontinuity,
    NormalizedPositionIndexMissing,
    InvalidNormalizedPositionOffset,
    /// The exact count of semantic patch operations omitted after prefix
    /// capacity exhaustion could not be represented.
    #[cfg(any(test, feature = "parser-conformance"))]
    PatchDroppedCountOverflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnsupportedFeatureObservationFailure {
    TokenAttributeNameUnavailable,
    ExistingHtmlElementSemanticsUnavailable,
    ExistingBodyElementSemanticsUnavailable,
    ExistingElementIdentityContradiction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParserObservationCaptureFailure {
    TokenCanonicalization,
    TreeTransitionTokenCanonicalization,
    UnsupportedFeatureEligibility(UnsupportedFeatureObservationFailure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParserObservationFailure {
    Capture(ParserObservationCaptureFailure),
    Invariant(ParserObservationInvariant),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ObservationPositionSource {
    NormalizedOffset(usize),
    #[cfg_attr(not(test), allow(dead_code))]
    Unavailable(PositionUnavailableReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ObservationPositionResolution {
    Known(InputPosition),
    GenuinelyUnavailable(PositionUnavailableReason),
    InvariantFailure(ParserObservationInvariant),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(not(any(test, feature = "parser-conformance")), allow(dead_code))]
pub(crate) enum SurfaceCaptureRequest {
    #[default]
    NotRequested,
    Capture {
        capacity: usize,
    },
}

impl SurfaceCaptureRequest {
    fn capacity(self) -> Option<usize> {
        match self {
            Self::NotRequested => None,
            Self::Capture { capacity } => Some(capacity),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParserObservationConfig {
    pub(crate) tokens: SurfaceCaptureRequest,
    pub(crate) parse_errors: SurfaceCaptureRequest,
    pub(crate) implementation_diagnostics: SurfaceCaptureRequest,
    pub(crate) tree_transitions: SurfaceCaptureRequest,
    pub(crate) unsupported_features: SurfaceCaptureRequest,
}

impl ParserObservationConfig {
    pub(crate) fn is_requested(self) -> bool {
        !matches!(self.tokens, SurfaceCaptureRequest::NotRequested)
            || !matches!(self.parse_errors, SurfaceCaptureRequest::NotRequested)
            || !matches!(
                self.implementation_diagnostics,
                SurfaceCaptureRequest::NotRequested
            )
            || !matches!(self.tree_transitions, SurfaceCaptureRequest::NotRequested)
            || !matches!(
                self.unsupported_features,
                SurfaceCaptureRequest::NotRequested
            )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg(any(test, feature = "parser-conformance"))]
pub(crate) struct CapturedSurface<T> {
    pub(crate) requested: bool,
    pub(crate) items: Vec<T>,
    pub(crate) dropped: u64,
}

#[derive(Debug)]
struct BoundedCapture<T> {
    capacity: Option<usize>,
    items: Vec<T>,
    dropped: u64,
}

impl<T> BoundedCapture<T> {
    fn new(request: SurfaceCaptureRequest) -> Self {
        Self {
            capacity: request.capacity(),
            items: Vec::new(),
            dropped: 0,
        }
    }

    fn is_requested(&self) -> bool {
        self.capacity.is_some()
    }

    fn can_retain(&self) -> bool {
        self.capacity
            .is_some_and(|capacity| self.items.len() < capacity)
    }

    fn push_reserved(&mut self, item: T) {
        debug_assert!(self.can_retain());
        self.items.push(item);
    }

    fn record_drop(&mut self) -> bool {
        debug_assert!(self.is_requested());
        let Some(next) = self.dropped.checked_add(1) else {
            return false;
        };
        self.dropped = next;
        true
    }

    #[cfg(any(test, feature = "parser-conformance"))]
    fn finish(self) -> CapturedSurface<T> {
        CapturedSurface {
            requested: self.is_requested(),
            items: self.items,
            dropped: self.dropped,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ParserObservationRecorder {
    next_parse_error_occurrence: Option<u64>,
    next_implementation_diagnostic_occurrence: Option<u64>,
    next_tree_transition_occurrence: Option<u64>,
    next_unsupported_feature_occurrence: Option<u64>,
    tokens: BoundedCapture<ObservedToken>,
    parse_errors: BoundedCapture<ParseErrorEvent>,
    implementation_diagnostics: BoundedCapture<ImplementationDiagnosticEvent>,
    tree_transitions: BoundedCapture<TreeTransitionEvent>,
    unsupported_features: BoundedCapture<UnsupportedFeatureEvent>,
    position_index: Option<NormalizedPositionIndex>,
    failure: Option<ParserObservationFailure>,
}

impl ParserObservationRecorder {
    pub(crate) fn new(config: ParserObservationConfig) -> Option<Self> {
        if !config.is_requested() {
            return None;
        }
        let position_requested = config
            .parse_errors
            .capacity()
            .is_some_and(|capacity| capacity > 0)
            || config
                .implementation_diagnostics
                .capacity()
                .is_some_and(|capacity| capacity > 0);
        Some(Self {
            next_parse_error_occurrence: Some(1),
            next_implementation_diagnostic_occurrence: Some(1),
            next_tree_transition_occurrence: Some(1),
            next_unsupported_feature_occurrence: Some(1),
            tokens: BoundedCapture::new(config.tokens),
            parse_errors: BoundedCapture::new(config.parse_errors),
            implementation_diagnostics: BoundedCapture::new(config.implementation_diagnostics),
            tree_transitions: BoundedCapture::new(config.tree_transitions),
            unsupported_features: BoundedCapture::new(config.unsupported_features),
            position_index: position_requested.then(NormalizedPositionIndex::new),
            failure: None,
        })
    }

    pub(crate) fn tokens_can_retain(&self) -> bool {
        self.tokens.can_retain()
    }

    pub(crate) fn tokens_requested(&self) -> bool {
        self.tokens.is_requested()
    }

    pub(crate) fn retain_token(&mut self, token: ObservedToken) {
        self.tokens.push_reserved(token);
    }

    pub(crate) fn drop_token(&mut self) {
        if !self.tokens.record_drop() {
            self.record_invariant(ParserObservationInvariant::DroppedCountOverflow(
                ObservationSurface::Tokens,
            ));
        }
    }

    pub(crate) fn record_token_capture_failure(&mut self) {
        self.record_capture_failure(ParserObservationCaptureFailure::TokenCanonicalization);
    }

    /// Assign the next parse-error occurrence before applying capacity.
    pub(crate) fn reserve_parse_error(&mut self) -> Option<u64> {
        if !self.parse_errors.is_requested() {
            return None;
        }
        let Some(occurrence) = self.next_parse_error_occurrence else {
            self.record_invariant(ParserObservationInvariant::OccurrenceSequenceOverflow(
                ObservationOccurrenceSequence::ParseErrors,
            ));
            return None;
        };
        self.next_parse_error_occurrence = occurrence.checked_add(1);
        if self.next_parse_error_occurrence.is_none() {
            self.record_invariant(ParserObservationInvariant::OccurrenceSequenceOverflow(
                ObservationOccurrenceSequence::ParseErrors,
            ));
        }
        if self.parse_errors.can_retain() {
            Some(occurrence)
        } else {
            if !self.parse_errors.record_drop() {
                self.record_invariant(ParserObservationInvariant::DroppedCountOverflow(
                    ObservationSurface::ParseErrors,
                ));
            }
            self.disable_position_index_if_exhausted();
            None
        }
    }

    pub(crate) fn retain_parse_error(&mut self, event: ParseErrorEvent) {
        self.parse_errors.push_reserved(event);
        self.disable_position_index_if_exhausted();
    }

    /// Assign the next implementation-diagnostic occurrence before capacity.
    pub(crate) fn reserve_implementation_diagnostic(&mut self) -> Option<u64> {
        if !self.implementation_diagnostics.is_requested() {
            return None;
        }
        let Some(occurrence) = self.next_implementation_diagnostic_occurrence else {
            self.record_invariant(ParserObservationInvariant::OccurrenceSequenceOverflow(
                ObservationOccurrenceSequence::ImplementationDiagnostics,
            ));
            return None;
        };
        self.next_implementation_diagnostic_occurrence = occurrence.checked_add(1);
        if self.next_implementation_diagnostic_occurrence.is_none() {
            self.record_invariant(ParserObservationInvariant::OccurrenceSequenceOverflow(
                ObservationOccurrenceSequence::ImplementationDiagnostics,
            ));
        }
        if self.implementation_diagnostics.can_retain() {
            Some(occurrence)
        } else {
            if !self.implementation_diagnostics.record_drop() {
                self.record_invariant(ParserObservationInvariant::DroppedCountOverflow(
                    ObservationSurface::ImplementationDiagnostics,
                ));
            }
            self.disable_position_index_if_exhausted();
            None
        }
    }

    pub(crate) fn retain_implementation_diagnostic(
        &mut self,
        event: ImplementationDiagnosticEvent,
    ) {
        self.implementation_diagnostics.push_reserved(event);
        self.disable_position_index_if_exhausted();
    }

    /// Assign the next tree-transition occurrence before applying capacity.
    pub(crate) fn reserve_tree_transition(&mut self) -> Option<u64> {
        if !self.tree_transitions.is_requested() {
            return None;
        }
        let Some(occurrence) = self.next_tree_transition_occurrence else {
            self.record_invariant(ParserObservationInvariant::OccurrenceSequenceOverflow(
                ObservationOccurrenceSequence::TreeTransitions,
            ));
            return None;
        };
        self.next_tree_transition_occurrence = occurrence.checked_add(1);
        if self.next_tree_transition_occurrence.is_none() {
            self.record_invariant(ParserObservationInvariant::OccurrenceSequenceOverflow(
                ObservationOccurrenceSequence::TreeTransitions,
            ));
        }
        if self.tree_transitions.can_retain() {
            Some(occurrence)
        } else {
            if !self.tree_transitions.record_drop() {
                self.record_invariant(ParserObservationInvariant::DroppedCountOverflow(
                    ObservationSurface::TreeTransitions,
                ));
            }
            None
        }
    }

    pub(crate) fn retain_tree_transition(&mut self, event: TreeTransitionEvent) {
        self.tree_transitions.push_reserved(event);
    }

    pub(crate) fn unsupported_features_requested(&self) -> bool {
        self.unsupported_features.is_requested()
    }

    /// Assign the next unsupported-feature occurrence before applying capacity.
    pub(crate) fn reserve_unsupported_feature(&mut self) -> Option<u64> {
        if !self.unsupported_features.is_requested() {
            return None;
        }
        let Some(occurrence) = self.next_unsupported_feature_occurrence else {
            self.record_invariant(ParserObservationInvariant::OccurrenceSequenceOverflow(
                ObservationOccurrenceSequence::UnsupportedFeatures,
            ));
            return None;
        };
        self.next_unsupported_feature_occurrence = occurrence.checked_add(1);
        if self.next_unsupported_feature_occurrence.is_none() {
            self.record_invariant(ParserObservationInvariant::OccurrenceSequenceOverflow(
                ObservationOccurrenceSequence::UnsupportedFeatures,
            ));
        }
        if self.unsupported_features.can_retain() {
            Some(occurrence)
        } else {
            if !self.unsupported_features.record_drop() {
                self.record_invariant(ParserObservationInvariant::DroppedCountOverflow(
                    ObservationSurface::UnsupportedFeatures,
                ));
            }
            None
        }
    }

    pub(crate) fn retain_unsupported_feature(&mut self, event: UnsupportedFeatureEvent) {
        self.unsupported_features.push_reserved(event);
    }

    pub(crate) fn record_tree_transition_capture_failure(&mut self) {
        self.record_capture_failure(
            ParserObservationCaptureFailure::TreeTransitionTokenCanonicalization,
        );
    }

    pub(crate) fn record_unsupported_feature_observation_failure(
        &mut self,
        failure: UnsupportedFeatureObservationFailure,
    ) {
        self.record_capture_failure(
            ParserObservationCaptureFailure::UnsupportedFeatureEligibility(failure),
        );
    }

    pub(crate) fn position_index_mut(&mut self) -> Option<&mut NormalizedPositionIndex> {
        self.position_index.as_mut()
    }

    pub(crate) fn resolve_position(
        &mut self,
        input: &str,
        source: ObservationPositionSource,
    ) -> ObservationPositionResolution {
        let offset = match source {
            ObservationPositionSource::NormalizedOffset(offset) => offset,
            ObservationPositionSource::Unavailable(reason) => {
                return ObservationPositionResolution::GenuinelyUnavailable(reason);
            }
        };
        let Some(index) = self.position_index.as_ref() else {
            let invariant = ParserObservationInvariant::NormalizedPositionIndexMissing;
            self.record_invariant(invariant);
            return ObservationPositionResolution::InvariantFailure(invariant);
        };
        match index.position_at(input, offset) {
            Ok(normalized) => ObservationPositionResolution::Known(InputPosition {
                normalized,
                source_bytes: SourceBytePosition::Unavailable(
                    SourcePositionUnavailableReason::NoInputProvenanceMap,
                ),
            }),
            Err(invariant) => {
                self.record_invariant(invariant);
                ObservationPositionResolution::InvariantFailure(invariant)
            }
        }
    }

    fn disable_position_index_if_exhausted(&mut self) {
        if !self.parse_errors.can_retain()
            && !self.implementation_diagnostics.can_retain()
            && let Some(index) = self.position_index.take()
            && let Some(invariant) = index.invariant
        {
            self.record_invariant(invariant);
        }
    }

    fn record_invariant(&mut self, invariant: ParserObservationInvariant) {
        self.record_failure(ParserObservationFailure::Invariant(invariant));
    }

    fn record_capture_failure(&mut self, failure: ParserObservationCaptureFailure) {
        self.record_failure(ParserObservationFailure::Capture(failure));
    }

    fn record_failure(&mut self, failure: ParserObservationFailure) {
        if self.failure.is_none() {
            self.failure = Some(failure);
        }
    }

    #[cfg(any(test, feature = "parser-conformance"))]
    pub(crate) fn finish(mut self) -> ParserObservationCapture {
        if let Some(index) = self.position_index.take()
            && let Some(invariant) = index.invariant
        {
            self.record_invariant(invariant);
        }
        ParserObservationCapture {
            tokens: self.tokens.finish(),
            parse_errors: self.parse_errors.finish(),
            implementation_diagnostics: self.implementation_diagnostics.finish(),
            tree_transitions: self.tree_transitions.finish(),
            unsupported_features: self.unsupported_features.finish(),
            failure: self.failure,
        }
    }

    #[cfg(test)]
    pub(crate) fn position_checkpoint_count(&self) -> usize {
        self.position_index
            .as_ref()
            .map_or(0, |index| index.checkpoints.len())
    }

    #[cfg(test)]
    pub(crate) fn has_position_index(&self) -> bool {
        self.position_index.is_some()
    }

    #[cfg(test)]
    pub(crate) fn remove_position_index_for_test(&mut self) {
        self.position_index = None;
    }

    #[cfg(test)]
    pub(crate) fn set_next_parse_error_occurrence_for_test(&mut self, next: u64) {
        self.next_parse_error_occurrence = Some(next);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg(any(test, feature = "parser-conformance"))]
pub(crate) struct ParserObservationCapture {
    pub(crate) tokens: CapturedSurface<ObservedToken>,
    pub(crate) parse_errors: CapturedSurface<ParseErrorEvent>,
    pub(crate) implementation_diagnostics: CapturedSurface<ImplementationDiagnosticEvent>,
    pub(crate) tree_transitions: CapturedSurface<TreeTransitionEvent>,
    pub(crate) unsupported_features: CapturedSurface<UnsupportedFeatureEvent>,
    pub(crate) failure: Option<ParserObservationFailure>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NormalizedPositionCheckpoint {
    position: NormalizedInputPosition,
}

#[derive(Debug)]
pub(crate) struct NormalizedPositionIndex {
    checkpoints: Vec<NormalizedPositionCheckpoint>,
    terminal: NormalizedInputPosition,
    next_stride_boundary: u64,
    invariant: Option<ParserObservationInvariant>,
}

impl NormalizedPositionIndex {
    fn new() -> Self {
        let terminal = normalized_position(0, 1, 1);
        Self {
            checkpoints: vec![NormalizedPositionCheckpoint { position: terminal }],
            terminal,
            next_stride_boundary: POSITION_CHECKPOINT_STRIDE,
            invariant: None,
        }
    }

    pub(crate) fn append_scalar(&mut self, buffer_len_before: usize, scalar: char) {
        if self.invariant.is_some() {
            return;
        }
        let Ok(buffer_len_before) = u64::try_from(buffer_len_before) else {
            self.invariant = Some(ParserObservationInvariant::NormalizedPositionOverflow);
            return;
        };
        if self.terminal.utf8_byte_offset != buffer_len_before {
            self.invariant = Some(ParserObservationInvariant::NormalizedPositionIndexDiscontinuity);
            return;
        }
        let Some(utf8_byte_offset) = self
            .terminal
            .utf8_byte_offset
            .checked_add(scalar.len_utf8() as u64)
        else {
            self.invariant = Some(ParserObservationInvariant::NormalizedPositionOverflow);
            return;
        };
        self.terminal.utf8_byte_offset = utf8_byte_offset;
        if scalar == '\n' {
            let Some(line) = self.terminal.line.get().checked_add(1) else {
                self.invariant = Some(ParserObservationInvariant::NormalizedPositionOverflow);
                return;
            };
            self.terminal.line =
                NormalizedLineNumber::new(line).expect("checked one-based line remains non-zero");
            self.terminal.column = NormalizedScalarColumn::new(1).expect("one is non-zero");
        } else {
            let Some(column) = self.terminal.column.get().checked_add(1) else {
                self.invariant = Some(ParserObservationInvariant::NormalizedPositionOverflow);
                return;
            };
            self.terminal.column = NormalizedScalarColumn::new(column)
                .expect("checked one-based column remains non-zero");
        }

        if scalar == '\n' || self.terminal.utf8_byte_offset >= self.next_stride_boundary {
            self.checkpoints.push(NormalizedPositionCheckpoint {
                position: self.terminal,
            });
            while self.next_stride_boundary <= self.terminal.utf8_byte_offset {
                let Some(next) = self
                    .next_stride_boundary
                    .checked_add(POSITION_CHECKPOINT_STRIDE)
                else {
                    self.invariant = Some(ParserObservationInvariant::NormalizedPositionOverflow);
                    return;
                };
                self.next_stride_boundary = next;
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn terminal(&self) -> Option<NormalizedInputPosition> {
        self.invariant.is_none().then_some(self.terminal)
    }

    fn position_at(
        &self,
        input: &str,
        offset: usize,
    ) -> Result<NormalizedInputPosition, ParserObservationInvariant> {
        if let Some(invariant) = self.invariant {
            return Err(invariant);
        }
        if offset > input.len() || !input.is_char_boundary(offset) {
            return Err(ParserObservationInvariant::InvalidNormalizedPositionOffset);
        }
        let offset_u64 = u64::try_from(offset)
            .map_err(|_| ParserObservationInvariant::NormalizedPositionOverflow)?;
        let checkpoint_index = self
            .checkpoints
            .partition_point(|checkpoint| checkpoint.position.utf8_byte_offset <= offset_u64)
            .checked_sub(1)
            .expect("the zero checkpoint precedes every valid offset");
        let mut position = self.checkpoints[checkpoint_index].position;
        let start = usize::try_from(position.utf8_byte_offset)
            .map_err(|_| ParserObservationInvariant::NormalizedPositionOverflow)?;
        for scalar in input[start..offset].chars() {
            position.utf8_byte_offset = position
                .utf8_byte_offset
                .checked_add(scalar.len_utf8() as u64)
                .ok_or(ParserObservationInvariant::NormalizedPositionOverflow)?;
            if scalar == '\n' {
                let line = position
                    .line
                    .get()
                    .checked_add(1)
                    .ok_or(ParserObservationInvariant::NormalizedPositionOverflow)?;
                position.line = NormalizedLineNumber::new(line)
                    .expect("checked one-based line remains non-zero");
                position.column = NormalizedScalarColumn::new(1).expect("one is a valid column");
            } else {
                let column = position
                    .column
                    .get()
                    .checked_add(1)
                    .ok_or(ParserObservationInvariant::NormalizedPositionOverflow)?;
                position.column = NormalizedScalarColumn::new(column)
                    .expect("checked one-based column remains non-zero");
            }
        }
        Ok(position)
    }
}

fn normalized_position(utf8_byte_offset: u64, line: u64, column: u64) -> NormalizedInputPosition {
    NormalizedInputPosition {
        space: InputCoordinateSpace::NormalizedUtf8,
        utf8_byte_offset,
        line: NormalizedLineNumber::new(line).expect("line is one-based"),
        column: NormalizedScalarColumn::new(column).expect("column is one-based"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html5::shared::{
        DiagnosticEventMetadata, EventPosition, ImplementationDiagnosticEvent, ParseErrorCode,
        ParserStage, PositionUnavailableReason, Utf8ReplacementPayload, Utf8ReplacementReason,
        WhatwgParseErrorCode,
    };
    use std::num::NonZeroU64;

    fn parse_event(occurrence: u64, offset: u64) -> ParseErrorEvent {
        ParseErrorEvent {
            occurrence,
            stage: ParserStage::Tokenizer,
            code: ParseErrorCode::Standard(WhatwgParseErrorCode::UnexpectedNullCharacter),
            recovery: None,
            position: EventPosition::Unavailable(
                PositionUnavailableReason::ParserDidNotProvidePosition,
            ),
            context: None,
            description: Some(if offset == 0 { "first" } else { "later" }),
        }
    }

    fn implementation_event(occurrence: u64) -> ImplementationDiagnosticEvent {
        ImplementationDiagnosticEvent::InvalidUtf8Replaced {
            metadata: DiagnosticEventMetadata {
                occurrence,
                stage: ParserStage::InputPreprocessing(
                    super::super::InputPreprocessingStage::Utf8Decoding,
                ),
                position: EventPosition::Unavailable(
                    PositionUnavailableReason::ParserDidNotProvidePosition,
                ),
                context: None,
                description: None,
            },
            reason: Utf8ReplacementReason::InvalidSequence,
            payload: Utf8ReplacementPayload {
                affected_byte_count: NonZeroU64::new(1).unwrap(),
            },
        }
    }

    #[test]
    fn parse_and_implementation_occurrences_are_independent() {
        let mut recorder = ParserObservationRecorder::new(ParserObservationConfig {
            parse_errors: SurfaceCaptureRequest::Capture { capacity: 2 },
            implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 0 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        assert_eq!(recorder.reserve_parse_error(), Some(1));
        recorder.retain_parse_error(parse_event(1, 0));
        assert_eq!(recorder.reserve_implementation_diagnostic(), None);
        assert_eq!(recorder.reserve_parse_error(), Some(2));
        recorder.retain_parse_error(parse_event(2, 1));
        let capture = recorder.finish();
        assert_eq!(
            capture
                .parse_errors
                .items
                .iter()
                .map(|event| event.occurrence)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(capture.implementation_diagnostics.dropped, 1);
    }

    #[test]
    fn default_configuration_installs_no_recorder_or_position_index() {
        assert!(ParserObservationRecorder::new(ParserObservationConfig::default()).is_none());
    }

    #[test]
    fn capacity_retains_prefix_without_reordering() {
        let mut recorder = ParserObservationRecorder::new(ParserObservationConfig {
            parse_errors: SurfaceCaptureRequest::Capture { capacity: 2 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        for offset in [20, 5, 30] {
            let occurrence = recorder.reserve_parse_error();
            if let Some(occurrence) = occurrence {
                recorder.retain_parse_error(parse_event(occurrence, offset));
            }
        }
        let capture = recorder.finish();
        assert_eq!(capture.parse_errors.items[0].description, Some("later"));
        assert_eq!(capture.parse_errors.items[1].description, Some("later"));
        assert_eq!(capture.parse_errors.dropped, 1);
    }

    #[test]
    fn position_index_is_released_when_all_position_surfaces_fill() {
        let mut recorder = ParserObservationRecorder::new(ParserObservationConfig {
            parse_errors: SurfaceCaptureRequest::Capture { capacity: 1 },
            implementation_diagnostics: SurfaceCaptureRequest::NotRequested,
            ..ParserObservationConfig::default()
        })
        .unwrap();
        assert!(recorder.has_position_index());
        recorder.position_index_mut().unwrap().append_scalar(0, 'x');
        assert!(recorder.position_checkpoint_count() > 0);
        assert_eq!(recorder.reserve_parse_error(), Some(1));
        recorder.retain_parse_error(parse_event(1, 0));
        assert!(!recorder.has_position_index());
        assert_eq!(recorder.position_checkpoint_count(), 0);
        assert_eq!(recorder.reserve_parse_error(), None);
        assert_eq!(
            recorder.finish().failure,
            None,
            "intentional bounded-work index retirement is not corruption"
        );
    }

    #[test]
    fn parse_error_occurrence_exhaustion_latches_on_the_u64_max_reservation() {
        let mut recorder = ParserObservationRecorder::new(ParserObservationConfig {
            parse_errors: SurfaceCaptureRequest::Capture { capacity: 2 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        recorder.next_parse_error_occurrence = Some(u64::MAX);
        assert_eq!(recorder.reserve_parse_error(), Some(u64::MAX));
        recorder.retain_parse_error(parse_event(u64::MAX, 0));
        let capture = recorder.finish();
        assert_eq!(
            capture
                .parse_errors
                .items
                .iter()
                .map(|event| event.occurrence)
                .collect::<Vec<_>>(),
            vec![u64::MAX]
        );
        assert_eq!(
            capture.failure,
            Some(ParserObservationFailure::Invariant(
                ParserObservationInvariant::OccurrenceSequenceOverflow(
                    ObservationOccurrenceSequence::ParseErrors
                )
            ))
        );
    }

    #[test]
    fn implementation_occurrence_exhaustion_latches_on_the_u64_max_reservation() {
        let mut recorder = ParserObservationRecorder::new(ParserObservationConfig {
            implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 1 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        recorder.next_implementation_diagnostic_occurrence = Some(u64::MAX);
        assert_eq!(recorder.reserve_implementation_diagnostic(), Some(u64::MAX));
        recorder.retain_implementation_diagnostic(implementation_event(u64::MAX));
        let capture = recorder.finish();
        assert_eq!(
            capture
                .implementation_diagnostics
                .items
                .iter()
                .map(ImplementationDiagnosticEvent::occurrence)
                .collect::<Vec<_>>(),
            vec![u64::MAX]
        );
        assert_eq!(
            capture.failure,
            Some(ParserObservationFailure::Invariant(
                ParserObservationInvariant::OccurrenceSequenceOverflow(
                    ObservationOccurrenceSequence::ImplementationDiagnostics
                )
            ))
        );
    }

    #[test]
    fn occurrence_exhaustion_latches_even_when_capacity_is_zero() {
        let mut parse_recorder = ParserObservationRecorder::new(ParserObservationConfig {
            parse_errors: SurfaceCaptureRequest::Capture { capacity: 0 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        parse_recorder.next_parse_error_occurrence = Some(u64::MAX);
        assert_eq!(parse_recorder.reserve_parse_error(), None);
        let parse_capture = parse_recorder.finish();
        assert_eq!(parse_capture.parse_errors.dropped, 1);
        assert_eq!(
            parse_capture.failure,
            Some(ParserObservationFailure::Invariant(
                ParserObservationInvariant::OccurrenceSequenceOverflow(
                    ObservationOccurrenceSequence::ParseErrors
                )
            ))
        );

        let mut diagnostic_recorder = ParserObservationRecorder::new(ParserObservationConfig {
            implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 0 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        diagnostic_recorder.next_implementation_diagnostic_occurrence = Some(u64::MAX);
        assert_eq!(
            diagnostic_recorder.reserve_implementation_diagnostic(),
            None
        );
        let diagnostic_capture = diagnostic_recorder.finish();
        assert_eq!(diagnostic_capture.implementation_diagnostics.dropped, 1);
        assert_eq!(
            diagnostic_capture.failure,
            Some(ParserObservationFailure::Invariant(
                ParserObservationInvariant::OccurrenceSequenceOverflow(
                    ObservationOccurrenceSequence::ImplementationDiagnostics
                )
            ))
        );
    }

    #[test]
    fn first_invariant_wins_when_one_reservation_detects_two_failures() {
        let mut recorder = ParserObservationRecorder::new(ParserObservationConfig {
            parse_errors: SurfaceCaptureRequest::Capture { capacity: 0 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        recorder.next_parse_error_occurrence = Some(u64::MAX);
        recorder.parse_errors.dropped = u64::MAX;

        assert_eq!(recorder.reserve_parse_error(), None);
        assert_eq!(
            recorder.next_parse_error_occurrence, None,
            "u64::MAX reservation exhausts rather than wrapping or duplicating"
        );
        let capture = recorder.finish();
        assert_eq!(capture.parse_errors.items.len(), 0);
        assert_eq!(capture.parse_errors.dropped, u64::MAX);
        assert_eq!(
            capture.failure,
            Some(ParserObservationFailure::Invariant(
                ParserObservationInvariant::OccurrenceSequenceOverflow(
                    ObservationOccurrenceSequence::ParseErrors
                )
            ))
        );
    }

    #[test]
    fn previously_latched_invariant_is_not_replaced_by_sequence_exhaustion() {
        let mut recorder = ParserObservationRecorder::new(ParserObservationConfig {
            implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 1 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        recorder.record_invariant(ParserObservationInvariant::NormalizedPositionOverflow);
        recorder.next_implementation_diagnostic_occurrence = Some(u64::MAX);

        assert_eq!(recorder.reserve_implementation_diagnostic(), Some(u64::MAX));
        recorder.retain_implementation_diagnostic(implementation_event(u64::MAX));
        let capture = recorder.finish();
        assert_eq!(
            capture
                .implementation_diagnostics
                .items
                .iter()
                .map(ImplementationDiagnosticEvent::occurrence)
                .collect::<Vec<_>>(),
            vec![u64::MAX]
        );
        assert_eq!(
            capture.failure,
            Some(ParserObservationFailure::Invariant(
                ParserObservationInvariant::NormalizedPositionOverflow
            ))
        );
    }

    #[test]
    fn dropped_count_overflow_is_an_explicit_invariant() {
        let mut recorder = ParserObservationRecorder::new(ParserObservationConfig {
            parse_errors: SurfaceCaptureRequest::Capture { capacity: 0 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        recorder.parse_errors.dropped = u64::MAX;
        assert_eq!(recorder.reserve_parse_error(), None);
        assert_eq!(
            recorder.finish().failure,
            Some(ParserObservationFailure::Invariant(
                ParserObservationInvariant::DroppedCountOverflow(ObservationSurface::ParseErrors)
            ))
        );
    }

    #[test]
    fn normalized_coordinate_overflow_is_an_explicit_invariant() {
        let mut index = NormalizedPositionIndex::new();
        index.terminal.line = NormalizedLineNumber::new(u64::MAX).unwrap();
        index.append_scalar(0, '\n');
        assert_eq!(
            index.invariant,
            Some(ParserObservationInvariant::NormalizedPositionOverflow)
        );
        assert_eq!(index.terminal(), None);
    }

    #[test]
    fn invalid_normalized_offsets_are_invariants_not_unavailable_positions() {
        let mut index = NormalizedPositionIndex::new();
        index.append_scalar(0, 'é');
        assert_eq!(
            index.position_at("é", 1),
            Err(ParserObservationInvariant::InvalidNormalizedPositionOffset)
        );
        assert_eq!(
            index.position_at("é", 3),
            Err(ParserObservationInvariant::InvalidNormalizedPositionOffset)
        );

        let eof = index.position_at("é", "é".len()).expect("valid EOF offset");
        assert_eq!(eof.utf8_byte_offset, 2);
        assert_eq!(eof.line.get(), 1);
        assert_eq!(eof.column.get(), 2);
    }

    #[test]
    fn position_resolution_distinguishes_unavailable_from_invariant_failure() {
        let mut recorder = ParserObservationRecorder::new(ParserObservationConfig {
            parse_errors: SurfaceCaptureRequest::Capture { capacity: 1 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        recorder.position_index_mut().unwrap().append_scalar(0, 'é');
        assert_eq!(
            recorder.resolve_position(
                "é",
                ObservationPositionSource::Unavailable(
                    PositionUnavailableReason::ParserDidNotProvidePosition
                )
            ),
            ObservationPositionResolution::GenuinelyUnavailable(
                PositionUnavailableReason::ParserDidNotProvidePosition
            )
        );
        assert_eq!(
            recorder.resolve_position("é", ObservationPositionSource::NormalizedOffset(1)),
            ObservationPositionResolution::InvariantFailure(
                ParserObservationInvariant::InvalidNormalizedPositionOffset
            )
        );
        assert_eq!(
            recorder.finish().failure,
            Some(ParserObservationFailure::Invariant(
                ParserObservationInvariant::InvalidNormalizedPositionOffset
            ))
        );
    }

    #[test]
    fn missing_position_index_is_corruption_not_unavailable_position() {
        let mut recorder = ParserObservationRecorder::new(ParserObservationConfig {
            parse_errors: SurfaceCaptureRequest::Capture { capacity: 1 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        recorder.remove_position_index_for_test();
        let occurrence = recorder
            .reserve_parse_error()
            .expect("retaining reservation");
        assert_eq!(occurrence, 1);
        assert_eq!(
            recorder.resolve_position("", ObservationPositionSource::NormalizedOffset(0)),
            ObservationPositionResolution::InvariantFailure(
                ParserObservationInvariant::NormalizedPositionIndexMissing
            )
        );
        let capture = recorder.finish();
        assert!(capture.parse_errors.items.is_empty());
        assert_eq!(
            capture.failure,
            Some(ParserObservationFailure::Invariant(
                ParserObservationInvariant::NormalizedPositionIndexMissing
            ))
        );
    }

    #[test]
    fn genuinely_unavailable_position_can_be_retained_without_an_invariant() {
        let mut recorder = ParserObservationRecorder::new(ParserObservationConfig {
            parse_errors: SurfaceCaptureRequest::Capture { capacity: 1 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        let occurrence = recorder.reserve_parse_error().unwrap();
        let position = match recorder.resolve_position(
            "",
            ObservationPositionSource::Unavailable(
                PositionUnavailableReason::ParserDidNotProvidePosition,
            ),
        ) {
            ObservationPositionResolution::GenuinelyUnavailable(reason) => {
                EventPosition::Unavailable(reason)
            }
            other => panic!("expected genuine unavailability, got {other:?}"),
        };
        let mut event = parse_event(occurrence, 0);
        event.position = position;
        recorder.retain_parse_error(event);

        let capture = recorder.finish();
        assert_eq!(capture.parse_errors.items.len(), 1);
        assert_eq!(capture.failure, None);
    }

    #[test]
    fn transition_and_unsupported_surfaces_have_independent_prefix_capacity() {
        let mut recorder = ParserObservationRecorder::new(ParserObservationConfig {
            tree_transitions: SurfaceCaptureRequest::Capture { capacity: 1 },
            unsupported_features: SurfaceCaptureRequest::Capture { capacity: 0 },
            ..ParserObservationConfig::default()
        })
        .unwrap();

        let transition_occurrence = recorder.reserve_tree_transition().unwrap();
        recorder.retain_tree_transition(TreeTransitionEvent {
            occurrence: transition_occurrence,
            token: std::sync::Arc::new(TransitionTokenSummary::Eof),
            insertion_mode_before: super::super::ObservedInsertionMode::InBody,
            dispatch_path: super::super::TreeDispatchPath::HtmlInsertionMode(
                super::super::ObservedInsertionMode::InBody,
            ),
            insertion_mode_after: super::super::ObservedInsertionMode::InBody,
            reprocessed: false,
        });
        assert_eq!(recorder.reserve_unsupported_feature(), None);
        assert_eq!(recorder.reserve_tree_transition(), None);
        assert_eq!(recorder.reserve_unsupported_feature(), None);

        let capture = recorder.finish();
        assert_eq!(capture.tree_transitions.items.len(), 1);
        assert_eq!(capture.tree_transitions.items[0].occurrence, 1);
        assert_eq!(capture.tree_transitions.dropped, 1);
        assert!(capture.unsupported_features.items.is_empty());
        assert_eq!(capture.unsupported_features.dropped, 2);
        assert_eq!(capture.failure, None);
    }

    #[test]
    fn one_failure_slot_preserves_cross_category_detection_order() {
        let mut capture_first = ParserObservationRecorder::new(ParserObservationConfig {
            tree_transitions: SurfaceCaptureRequest::Capture { capacity: 0 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        capture_first.record_tree_transition_capture_failure();
        capture_first.next_tree_transition_occurrence = Some(u64::MAX);
        assert_eq!(capture_first.reserve_tree_transition(), None);
        assert_eq!(
            capture_first.finish().failure,
            Some(ParserObservationFailure::Capture(
                ParserObservationCaptureFailure::TreeTransitionTokenCanonicalization
            ))
        );

        let mut invariant_first = ParserObservationRecorder::new(ParserObservationConfig {
            tree_transitions: SurfaceCaptureRequest::Capture { capacity: 0 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        invariant_first.next_tree_transition_occurrence = Some(u64::MAX);
        assert_eq!(invariant_first.reserve_tree_transition(), None);
        invariant_first.record_tree_transition_capture_failure();
        assert_eq!(
            invariant_first.finish().failure,
            Some(ParserObservationFailure::Invariant(
                ParserObservationInvariant::OccurrenceSequenceOverflow(
                    ObservationOccurrenceSequence::TreeTransitions
                )
            ))
        );
    }

    #[test]
    fn capture_failure_precedes_later_dropped_count_overflow() {
        let mut recorder = ParserObservationRecorder::new(ParserObservationConfig {
            tree_transitions: SurfaceCaptureRequest::Capture { capacity: 0 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        recorder.record_tree_transition_capture_failure();
        recorder.tree_transitions.dropped = u64::MAX;
        assert_eq!(recorder.reserve_tree_transition(), None);
        assert_eq!(
            recorder.finish().failure,
            Some(ParserObservationFailure::Capture(
                ParserObservationCaptureFailure::TreeTransitionTokenCanonicalization
            ))
        );
    }

    #[test]
    fn transition_and_unsupported_overflows_use_exact_surface_identities() {
        let mut transition = ParserObservationRecorder::new(ParserObservationConfig {
            tree_transitions: SurfaceCaptureRequest::Capture { capacity: 0 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        transition.next_tree_transition_occurrence = Some(u64::MAX);
        assert_eq!(transition.reserve_tree_transition(), None);
        assert_eq!(
            transition.finish().failure,
            Some(ParserObservationFailure::Invariant(
                ParserObservationInvariant::OccurrenceSequenceOverflow(
                    ObservationOccurrenceSequence::TreeTransitions
                )
            ))
        );

        let mut transition_drop = ParserObservationRecorder::new(ParserObservationConfig {
            tree_transitions: SurfaceCaptureRequest::Capture { capacity: 0 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        transition_drop.tree_transitions.dropped = u64::MAX;
        assert_eq!(transition_drop.reserve_tree_transition(), None);
        assert_eq!(
            transition_drop.finish().failure,
            Some(ParserObservationFailure::Invariant(
                ParserObservationInvariant::DroppedCountOverflow(
                    ObservationSurface::TreeTransitions
                )
            ))
        );

        let mut unsupported_occurrence = ParserObservationRecorder::new(ParserObservationConfig {
            unsupported_features: SurfaceCaptureRequest::Capture { capacity: 0 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        unsupported_occurrence.next_unsupported_feature_occurrence = Some(u64::MAX);
        assert_eq!(unsupported_occurrence.reserve_unsupported_feature(), None);
        assert_eq!(
            unsupported_occurrence.finish().failure,
            Some(ParserObservationFailure::Invariant(
                ParserObservationInvariant::OccurrenceSequenceOverflow(
                    ObservationOccurrenceSequence::UnsupportedFeatures
                )
            ))
        );

        let mut unsupported_drop = ParserObservationRecorder::new(ParserObservationConfig {
            unsupported_features: SurfaceCaptureRequest::Capture { capacity: 0 },
            ..ParserObservationConfig::default()
        })
        .unwrap();
        unsupported_drop.unsupported_features.dropped = u64::MAX;
        assert_eq!(unsupported_drop.reserve_unsupported_feature(), None);
        assert_eq!(
            unsupported_drop.finish().failure,
            Some(ParserObservationFailure::Invariant(
                ParserObservationInvariant::DroppedCountOverflow(
                    ObservationSurface::UnsupportedFeatures
                )
            ))
        );
    }

    #[test]
    fn valid_positions_after_crlf_normalization_use_normalized_coordinates() {
        let mut input = super::super::Input::new();
        let mut index = NormalizedPositionIndex::new();
        input.push_str_observed("a\r\né", Some(&mut index));
        assert_eq!(input.as_str(), "a\né");

        let before_multibyte = index
            .position_at(input.as_str(), 2)
            .expect("valid normalized boundary");
        assert_eq!(before_multibyte.utf8_byte_offset, 2);
        assert_eq!(before_multibyte.line.get(), 2);
        assert_eq!(before_multibyte.column.get(), 1);

        let eof = index
            .position_at(input.as_str(), input.as_str().len())
            .expect("valid normalized EOF");
        assert_eq!(eof.utf8_byte_offset, 4);
        assert_eq!(eof.line.get(), 2);
        assert_eq!(eof.column.get(), 2);
    }
}
