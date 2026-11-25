//! Bounded production recoverable diagnostics. Coordinates are resolved only on retirement.
use super::observation::{BoundedCapture, SurfaceCaptureRequest};
use super::{
    EventPosition, Input, InputCoordinateSpace, InputPosition, NormalizedInputPosition,
    NormalizedLineNumber, NormalizedScalarColumn, ParseErrorCode, ParseErrorEvent,
    ParserContextSummary, ParserRecoveryAction, ParserStage, PositionUnavailableReason,
    SourceBytePosition, SourcePositionUnavailableReason,
};

pub(crate) const RUNTIME_DIAGNOSTIC_CAPACITY: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(test, feature = "internal-api")), allow(dead_code))]
pub enum ParserInputCompletion {
    PrefixOnly,
    Finalized,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(test, feature = "internal-api")), allow(dead_code))]
pub enum DiagnosticCaptureFailure {
    PendingStorageAllocation,
    OutputStorageAllocation,
    InvalidNormalizedOffset,
    PositionArithmeticOverflow,
    OccurrenceSequenceExhausted,
    TotalCountExhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(any(test, feature = "internal-api")), allow(dead_code))]
pub enum DiagnosticCaptureStatus {
    Complete,
    Truncated,
    Incomplete {
        first_failure: DiagnosticCaptureFailure,
        terminal_failure: Option<DiagnosticCaptureFailure>,
    },
}

/// Owned terminal evidence; deliberately not Clone. Counts become lower bounds on exhaustion.
#[cfg_attr(not(any(test, feature = "internal-api")), allow(dead_code))]
pub struct HtmlParserDiagnosticReport {
    completion: ParserInputCompletion,
    capture: DiagnosticCaptureStatus,
    normalized_input_bytes: usize,
    total: u64,
    dropped: u64,
    counts_exact: bool,
    events: Vec<ParseErrorEvent>,
}

#[cfg_attr(not(any(test, feature = "internal-api")), allow(dead_code))]
impl HtmlParserDiagnosticReport {
    pub fn completion(&self) -> ParserInputCompletion {
        self.completion
    }
    pub fn capture(&self) -> &DiagnosticCaptureStatus {
        &self.capture
    }
    pub fn normalized_input_bytes(&self) -> usize {
        self.normalized_input_bytes
    }
    pub fn total(&self) -> u64 {
        self.total
    }
    pub fn retained(&self) -> usize {
        self.events.len()
    }
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
    pub fn counts_exact(&self) -> bool {
        self.counts_exact
    }
    pub fn events(&self) -> &[ParseErrorEvent] {
        &self.events
    }
}

impl std::fmt::Debug for HtmlParserDiagnosticReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HtmlParserDiagnosticReport")
            .field("completion", &self.completion)
            .field("capture", &self.capture)
            .field("total", &self.total)
            .field("retained", &self.retained())
            .field("dropped", &self.dropped)
            .field("counts_exact", &self.counts_exact)
            .finish()
    }
}

pub(crate) enum ParseErrorPositionSource<'a> {
    Known { input: &'a Input, offset: usize },
    Unavailable(PositionUnavailableReason),
}

#[derive(Debug)]
#[cfg_attr(not(any(test, feature = "internal-api")), allow(dead_code))]
struct ValidatedNormalizedOffset(usize);

fn validate_at_emission(
    input: &Input,
    offset: usize,
) -> Result<ValidatedNormalizedOffset, DiagnosticCaptureFailure> {
    if offset > input.as_str().len() || !input.as_str().is_char_boundary(offset) {
        return Err(DiagnosticCaptureFailure::InvalidNormalizedOffset);
    }
    u64::try_from(offset).map_err(|_| DiagnosticCaptureFailure::PositionArithmeticOverflow)?;
    Ok(ValidatedNormalizedOffset(offset))
}

#[derive(Debug)]
#[cfg_attr(not(any(test, feature = "internal-api")), allow(dead_code))]
enum PendingDiagnosticPosition {
    KnownAtEmission(ValidatedNormalizedOffset),
    Unavailable(PositionUnavailableReason),
}

#[derive(Debug)]
#[cfg_attr(not(any(test, feature = "internal-api")), allow(dead_code))]
pub(super) struct PendingParseError {
    occurrence: u64,
    stage: ParserStage,
    code: ParseErrorCode,
    recovery: Option<ParserRecoveryAction>,
    context: Option<ParserContextSummary>,
    description: Option<&'static str>,
    position: PendingDiagnosticPosition,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(any(test, feature = "internal-api")), allow(dead_code))]
struct PositionRequest {
    offset: usize,
    event_index: usize,
    resolved: Option<NormalizedInputPosition>,
}

#[cfg(test)]
#[derive(Default, Debug)]
pub(crate) struct CaptureWork {
    pub pending_reservations: usize,
    pub output_reservations: usize,
    pub scans: usize,
    pub scalar_visits: usize,
    pub high_water: usize,
    pub requested_pending_bytes: usize,
    pub requested_output_bytes: usize,
    pub pending_capacity_bytes: usize,
    pub output_capacity_bytes: usize,
    pub peak_heap_bytes: usize,
    pub scratch_bytes: usize,
    pub position_index_present: bool,
    pub position_checkpoints: usize,
    pub unrelated_surfaces_requested: usize,
    pub fail_pending: bool,
    pub fail_output: bool,
    pub fail_after_scalars: Option<usize>,
}

#[derive(Debug)]
pub(super) struct RuntimeDiagnosticCapture {
    pending: BoundedCapture<PendingParseError>,
    first_failure: Option<DiagnosticCaptureFailure>,
    #[cfg(test)]
    pub work: CaptureWork,
}

impl RuntimeDiagnosticCapture {
    #[cfg_attr(not(any(test, feature = "internal-api")), allow(dead_code))]
    pub(super) fn new() -> Self {
        Self {
            pending: BoundedCapture::new(SurfaceCaptureRequest::Capture {
                capacity: RUNTIME_DIAGNOSTIC_CAPACITY,
            }),
            first_failure: None,
            #[cfg(test)]
            work: {
                let (pending, output, arithmetic) =
                    super::NEXT_CAPTURE_FAILURE.with(|v| v.replace((false, false, None)));
                CaptureWork {
                    fail_pending: pending,
                    fail_output: output,
                    fail_after_scalars: arithmetic,
                    ..Default::default()
                }
            },
        }
    }

    pub(super) fn fail(&mut self, failure: DiagnosticCaptureFailure) {
        self.first_failure.get_or_insert(failure);
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn record(
        &mut self,
        occurrence: u64,
        stage: ParserStage,
        code: ParseErrorCode,
        recovery: Option<ParserRecoveryAction>,
        context: Option<ParserContextSummary>,
        description: Option<&'static str>,
        source: ParseErrorPositionSource<'_>,
    ) {
        if self.first_failure.is_some() || !self.pending.can_retain() {
            return;
        }
        let position = match source {
            ParseErrorPositionSource::Known { input, offset } => {
                match validate_at_emission(input, offset) {
                    Ok(offset) => PendingDiagnosticPosition::KnownAtEmission(offset),
                    Err(failure) => {
                        self.fail(failure);
                        return;
                    }
                }
            }
            ParseErrorPositionSource::Unavailable(reason) => {
                PendingDiagnosticPosition::Unavailable(reason)
            }
        };
        if self.pending.items.capacity() == 0 {
            #[cfg(test)]
            {
                self.work.pending_reservations += 1;
                self.work.requested_pending_bytes =
                    RUNTIME_DIAGNOSTIC_CAPACITY * std::mem::size_of::<PendingParseError>();
            }
            #[cfg(test)]
            if self.work.fail_pending {
                self.fail(DiagnosticCaptureFailure::PendingStorageAllocation);
                return;
            }
            if self
                .pending
                .items
                .try_reserve_exact(RUNTIME_DIAGNOSTIC_CAPACITY)
                .is_err()
            {
                self.fail(DiagnosticCaptureFailure::PendingStorageAllocation);
                return;
            }
        }
        self.pending.push_reserved(PendingParseError {
            occurrence,
            stage,
            code,
            recovery,
            context,
            description,
            position,
        });
        #[cfg(test)]
        {
            self.work.high_water = self.work.high_water.max(self.pending.items.capacity());
            self.work.pending_capacity_bytes =
                self.pending.items.capacity() * std::mem::size_of::<PendingParseError>();
            self.work.peak_heap_bytes = self
                .work
                .peak_heap_bytes
                .max(self.work.pending_capacity_bytes);
        }
    }

    #[cfg_attr(not(any(test, feature = "internal-api")), allow(dead_code))]
    pub(super) fn finish(
        mut self,
        input: &str,
        finalized: bool,
        total: u64,
        sequence_exact: bool,
    ) -> HtmlParserDiagnosticReport {
        let counts_exact = sequence_exact && total != u64::MAX;
        if !counts_exact {
            self.fail(if !sequence_exact {
                DiagnosticCaptureFailure::OccurrenceSequenceExhausted
            } else {
                DiagnosticCaptureFailure::TotalCountExhausted
            });
        }
        let mut output = Vec::new();
        let mut terminal_failure = None;
        let count = self.pending.items.len();
        if count != 0 {
            #[cfg(test)]
            {
                self.work.output_reservations += 1;
                self.work.requested_output_bytes = count * std::mem::size_of::<ParseErrorEvent>();
            }
            #[cfg(test)]
            let forced_output_failure = self.work.fail_output;
            #[cfg(not(test))]
            let forced_output_failure = false;
            if forced_output_failure || output.try_reserve_exact(count).is_err() {
                terminal_failure = Some(DiagnosticCaptureFailure::OutputStorageAllocation);
            } else {
                #[cfg(test)]
                {
                    self.work.output_capacity_bytes =
                        output.capacity() * std::mem::size_of::<ParseErrorEvent>();
                    self.work.peak_heap_bytes =
                        self.work.pending_capacity_bytes + self.work.output_capacity_bytes;
                    self.work.scratch_bytes =
                        std::mem::size_of::<[PositionRequest; RUNTIME_DIAGNOSTIC_CAPACITY]>();
                }
                let mut requests = [PositionRequest {
                    offset: 0,
                    event_index: 0,
                    resolved: None,
                }; RUNTIME_DIAGNOSTIC_CAPACITY];
                let mut used = 0;
                for (event_index, event) in self.pending.items.iter().enumerate() {
                    if let PendingDiagnosticPosition::KnownAtEmission(offset) = &event.position {
                        requests[used] = PositionRequest {
                            offset: offset.0,
                            event_index,
                            resolved: None,
                        };
                        used += 1;
                    }
                }
                let requests = &mut requests[..used];
                requests.sort_unstable_by_key(|r| (r.offset, r.event_index));
                if !requests.is_empty() {
                    #[cfg(test)]
                    {
                        self.work.scans += 1;
                    }
                    let mut offset = 0usize;
                    let mut line = 1u64;
                    let mut column = 1u64;
                    'resolve: for request in requests.iter_mut() {
                        if request.offset > input.len() || !input.is_char_boundary(request.offset) {
                            terminal_failure =
                                Some(DiagnosticCaptureFailure::InvalidNormalizedOffset);
                            break;
                        }
                        while offset < request.offset {
                            #[cfg(test)]
                            if self.work.fail_after_scalars == Some(self.work.scalar_visits) {
                                terminal_failure =
                                    Some(DiagnosticCaptureFailure::PositionArithmeticOverflow);
                                break 'resolve;
                            }
                            // Input is valid UTF-8 and offset remains a scalar
                            // boundary. Coordinates need only LF and scalar width,
                            // not decoding and re-encoding every scalar value.
                            let leading = input.as_bytes()[offset];
                            #[cfg(test)]
                            {
                                self.work.scalar_visits += 1;
                            }
                            let next = if leading == b'\n' {
                                offset += 1;
                                line.checked_add(1).map(|v| (v, 1))
                            } else {
                                offset += if leading < 0x80 {
                                    1
                                } else if leading < 0xE0 {
                                    2
                                } else if leading < 0xF0 {
                                    3
                                } else {
                                    4
                                };
                                column.checked_add(1).map(|v| (line, v))
                            };
                            let Some((next_line, next_column)) = next else {
                                terminal_failure =
                                    Some(DiagnosticCaptureFailure::PositionArithmeticOverflow);
                                break 'resolve;
                            };
                            line = next_line;
                            column = next_column;
                        }
                        request.resolved = Some(NormalizedInputPosition {
                            space: InputCoordinateSpace::NormalizedUtf8,
                            utf8_byte_offset: request.offset as u64,
                            line: NormalizedLineNumber::new(line).expect("one based"),
                            column: NormalizedScalarColumn::new(column).expect("one based"),
                        });
                    }
                }
                requests.sort_unstable_by_key(|r| r.event_index);
                let mut positions = requests.iter_mut();
                for pending in self.pending.items.drain(..) {
                    let position = match pending.position {
                        PendingDiagnosticPosition::Unavailable(reason) => {
                            EventPosition::Unavailable(reason)
                        }
                        PendingDiagnosticPosition::KnownAtEmission(_) => {
                            let Some(normalized) = positions.next().and_then(|r| r.resolved.take())
                            else {
                                break;
                            };
                            EventPosition::Known(InputPosition {
                                normalized,
                                source_bytes: SourceBytePosition::Unavailable(
                                    SourcePositionUnavailableReason::NoInputProvenanceMap,
                                ),
                            })
                        }
                    };
                    output.push(ParseErrorEvent {
                        occurrence: pending.occurrence,
                        stage: pending.stage,
                        code: pending.code,
                        recovery: pending.recovery,
                        position,
                        context: pending.context,
                        description: pending.description,
                    });
                }
            }
        }
        let dropped = total.saturating_sub(output.len() as u64);
        let capture = match (self.first_failure, terminal_failure) {
            (Some(first_failure), terminal_failure) => DiagnosticCaptureStatus::Incomplete {
                first_failure,
                terminal_failure,
            },
            (None, Some(first_failure)) => DiagnosticCaptureStatus::Incomplete {
                first_failure,
                terminal_failure: None,
            },
            (None, None) if dropped != 0 => DiagnosticCaptureStatus::Truncated,
            (None, None) => DiagnosticCaptureStatus::Complete,
        };
        #[cfg(test)]
        LAST_WORK.with(|work| *work.borrow_mut() = self.work);
        HtmlParserDiagnosticReport {
            completion: if finalized {
                ParserInputCompletion::Finalized
            } else {
                ParserInputCompletion::PrefixOnly
            },
            capture,
            normalized_input_bytes: input.len(),
            total,
            dropped,
            counts_exact,
            events: output,
        }
    }
}

#[cfg(test)]
thread_local! { pub(crate) static LAST_WORK: std::cell::RefCell<CaptureWork> = std::cell::RefCell::new(CaptureWork::default()); }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html5::shared::{
        ObservationPositionResolution, ObservationPositionSource, ParserObservationConfig,
        ParserObservationRecorder, WhatwgParseErrorCode,
    };

    fn record(c: &mut RuntimeDiagnosticCapture, input: &Input, occurrence: u64, offset: usize) {
        c.record(
            occurrence,
            ParserStage::Tokenizer,
            ParseErrorCode::Standard(WhatwgParseErrorCode::UnexpectedNullCharacter),
            None,
            None,
            None,
            ParseErrorPositionSource::Known { input, offset },
        );
    }
    fn input(s: &str) -> Input {
        let mut input = Input::new();
        input.push_str(s);
        input
    }
    fn incomplete(
        r: &HtmlParserDiagnosticReport,
        first: DiagnosticCaptureFailure,
        terminal: Option<DiagnosticCaptureFailure>,
    ) {
        assert_eq!(
            r.capture(),
            &DiagnosticCaptureStatus::Incomplete {
                first_failure: first,
                terminal_failure: terminal
            }
        );
        assert_eq!(r.total(), r.retained() as u64 + r.dropped());
    }

    #[test]
    fn aj5_storage_capacities_and_refusal_work_are_type_bounded() {
        assert!(std::mem::size_of::<PendingParseError>() <= std::mem::size_of::<ParseErrorEvent>());
        for (count, fail_pending, fail_output) in [
            (0, false, false),
            (1, false, false),
            (128, false, false),
            (1000, false, false),
            (1000, true, false),
            (1000, false, true),
        ] {
            let input = input("x");
            let mut capture = RuntimeDiagnosticCapture::new();
            capture.work.fail_pending = fail_pending;
            capture.work.fail_output = fail_output;
            for occurrence in 1..=count {
                record(&mut capture, &input, occurrence, 1);
            }
            let report = capture.finish(input.as_str(), true, count, true);
            LAST_WORK.with(|work| {
                let w = work.borrow();
                let attempted = count != 0;
                let pending = attempted && !fail_pending;
                let output = pending && !fail_output;
                let pending_bytes = RUNTIME_DIAGNOSTIC_CAPACITY * std::mem::size_of::<PendingParseError>();
                let output_bytes = (count as usize).min(RUNTIME_DIAGNOSTIC_CAPACITY) * std::mem::size_of::<ParseErrorEvent>();
                assert_eq!(w.pending_reservations, usize::from(attempted));
                assert_eq!(w.output_reservations, usize::from(pending));
                assert_eq!(w.requested_pending_bytes, if attempted {pending_bytes} else {0});
                assert_eq!(w.requested_output_bytes, if pending {output_bytes} else {0});
                assert_eq!(w.pending_capacity_bytes, if pending {pending_bytes} else {0});
                assert_eq!(w.output_capacity_bytes, if output {output_bytes} else {0});
                assert_eq!(w.high_water, if pending {RUNTIME_DIAGNOSTIC_CAPACITY} else {0});
                assert_eq!(w.peak_heap_bytes, w.pending_capacity_bytes + w.output_capacity_bytes);
                assert_eq!(w.scratch_bytes, if output {std::mem::size_of::<[PositionRequest; RUNTIME_DIAGNOSTIC_CAPACITY]>()} else {0});
                assert_eq!(report.retained(), if output {(count as usize).min(RUNTIME_DIAGNOSTIC_CAPACITY)} else {0});
                if fail_output {assert_eq!(w.scans, 0);}
                eprintln!("AJ5_STORAGE count={count} fail_pending={fail_pending} fail_output={fail_output} work={w:?}");
            });
        }
    }

    #[test]
    fn aj5_invalid_emission_offset_cannot_be_rehabilitated_by_append() {
        let mut input = input("é");
        let mut c = RuntimeDiagnosticCapture::new();
        record(&mut c, &input, 1, 0);
        record(&mut c, &input, 2, 3);
        input.push_str("x");
        record(&mut c, &input, 3, 3);
        let report = c.finish(input.as_str(), true, 3, true);
        incomplete(
            &report,
            DiagnosticCaptureFailure::InvalidNormalizedOffset,
            None,
        );
        assert_eq!(report.retained(), 1);
        assert_eq!(report.dropped(), 2);
    }
    #[test]
    fn aj5_known_positions_allow_end_duplicates_and_nonmonotone_offsets() {
        let input = input("aé€😀\nZ");
        let offsets = [12, 0, 6, 3, 10, 1, 11, 6, 12];
        let mut c = RuntimeDiagnosticCapture::new();
        for (i, offset) in offsets.into_iter().enumerate() {
            record(&mut c, &input, i as u64 + 1, offset);
        }
        let report = c.finish(input.as_str(), true, offsets.len() as u64, true);
        let mut reference = ParserObservationRecorder::new(ParserObservationConfig {
            parse_errors: SurfaceCaptureRequest::Capture { capacity: 128 },
            ..Default::default()
        })
        .unwrap();
        for (offset, scalar) in input.as_str().char_indices() {
            reference
                .position_index_mut()
                .unwrap()
                .append_scalar(offset, scalar);
        }
        for (i, (event, offset)) in report.events().iter().zip(offsets).enumerate() {
            assert_eq!(event.occurrence, i as u64 + 1);
            let ObservationPositionResolution::Known(expected) = reference.resolve_position(
                input.as_str(),
                ObservationPositionSource::NormalizedOffset(offset),
            ) else {
                panic!("valid reference position");
            };
            assert_eq!(event.position, EventPosition::Known(expected));
        }
        LAST_WORK.with(|w| {
            let w = w.borrow();
            assert_eq!(w.scans, 1);
            assert_eq!(w.scalar_visits, 6);
        });
    }
    #[test]
    fn aj5_saturated_accounting_is_explicit_and_preserves_prefix() {
        let input = input("x");
        for sequence_exact in [true, false] {
            let mut c = RuntimeDiagnosticCapture::new();
            record(&mut c, &input, 0, 1);
            let r = c.finish(input.as_str(), true, u64::MAX, sequence_exact);
            assert!(!r.counts_exact());
            assert_eq!(r.total(), u64::MAX);
            assert_eq!(r.retained(), 1);
            assert_eq!(r.dropped(), u64::MAX - 1);
            assert!(matches!(
                r.capture(),
                DiagnosticCaptureStatus::Incomplete { .. }
            ));
        }
    }
    #[test]
    fn aj5_invalid_position_freezes_unused_capacity_and_survives_as_prefix() {
        let input = input("éx");
        let mut c = RuntimeDiagnosticCapture::new();
        record(&mut c, &input, 1, 0);
        record(&mut c, &input, 2, 1);
        record(&mut c, &input, 3, 2);
        let r = c.finish(input.as_str(), false, 3, true);
        assert_eq!((r.total(), r.retained(), r.dropped()), (3, 1, 2));
        assert!(matches!(
            r.capture(),
            DiagnosticCaptureStatus::Incomplete {
                first_failure: DiagnosticCaptureFailure::InvalidNormalizedOffset,
                ..
            }
        ));
        LAST_WORK.with(|w| {
            let w = w.borrow();
            assert_eq!(w.pending_reservations, 1);
            assert_eq!(w.output_reservations, 1);
            assert!(w.high_water <= 128);
        });
    }
    #[test]
    fn aj5_invalid_character_boundary_is_not_unavailable() {
        let input = input("é");
        let mut c = RuntimeDiagnosticCapture::new();
        record(&mut c, &input, 1, 1);
        let r = c.finish(input.as_str(), true, 1, true);
        incomplete(&r, DiagnosticCaptureFailure::InvalidNormalizedOffset, None);
        assert!(r.events().is_empty());
    }
    #[test]
    fn aj5_pending_reservation_failure_is_sticky_without_retries() {
        let input = input("x");
        let mut c = RuntimeDiagnosticCapture::new();
        c.work.fail_pending = true;
        for i in 1..1000 {
            record(&mut c, &input, i, 0);
        }
        let r = c.finish(input.as_str(), true, 999, true);
        incomplete(&r, DiagnosticCaptureFailure::PendingStorageAllocation, None);
        LAST_WORK.with(|w| {
            let w = w.borrow();
            assert_eq!(w.pending_reservations, 1);
            assert_eq!(w.output_reservations, 0);
            assert_eq!(w.scans, 0);
        });
    }
    #[test]
    fn aj5_output_reservation_failure_exports_no_rows_and_performs_no_scan() {
        let input = input("x");
        let mut c = RuntimeDiagnosticCapture::new();
        record(&mut c, &input, 1, 1);
        c.work.fail_output = true;
        let r = c.finish(input.as_str(), true, 1, true);
        incomplete(&r, DiagnosticCaptureFailure::OutputStorageAllocation, None);
        assert_eq!(r.retained(), 0);
        LAST_WORK.with(|w| assert_eq!(w.borrow().scans, 0));
    }
    #[test]
    fn aj5_coordinate_failure_exports_the_largest_contiguous_occurrence_prefix() {
        let input = input("abcd");
        let mut c = RuntimeDiagnosticCapture::new();
        for (i, offset) in [0, 4, 1].into_iter().enumerate() {
            record(&mut c, &input, i as u64 + 1, offset);
        }
        c.work.fail_after_scalars = Some(2);
        let r = c.finish(input.as_str(), true, 3, true);
        incomplete(
            &r,
            DiagnosticCaptureFailure::PositionArithmeticOverflow,
            None,
        );
        assert_eq!(r.retained(), 1);
        assert_eq!(r.events.capacity(), 3);
        LAST_WORK.with(|work| {
            let w = work.borrow();
            assert_eq!(
                w.requested_output_bytes,
                3 * std::mem::size_of::<ParseErrorEvent>()
            );
            assert_eq!(
                w.output_capacity_bytes,
                3 * std::mem::size_of::<ParseErrorEvent>()
            );
            assert_eq!(
                w.peak_heap_bytes,
                w.pending_capacity_bytes + w.output_capacity_bytes
            );
            assert_eq!((w.scans, w.scalar_visits), (1, 2));
            assert_eq!(
                w.scratch_bytes,
                std::mem::size_of::<[PositionRequest; RUNTIME_DIAGNOSTIC_CAPACITY]>()
            );
        });
    }
    #[test]
    fn aj5_first_capture_failure_survives_an_additional_terminal_failure() {
        let input = input("x");
        let mut c = RuntimeDiagnosticCapture::new();
        record(&mut c, &input, 1, 0);
        record(&mut c, &input, 2, 2);
        c.work.fail_output = true;
        let r = c.finish(input.as_str(), true, 2, true);
        incomplete(
            &r,
            DiagnosticCaptureFailure::InvalidNormalizedOffset,
            Some(DiagnosticCaptureFailure::OutputStorageAllocation),
        );
    }
    #[test]
    fn aj5_capacity_truncation_continues_exact_accounting() {
        for total in [0, 127, 128, 129, 1024] {
            let input = input("x");
            let mut c = RuntimeDiagnosticCapture::new();
            for i in 1..=total {
                record(&mut c, &input, i, 0);
            }
            let r = c.finish(input.as_str(), true, total, true);
            assert_eq!(r.retained(), (total as usize).min(128));
            assert!(r.counts_exact());
            assert_eq!(r.total(), r.retained() as u64 + r.dropped());
            assert_eq!(
                r.capture(),
                if total <= 128 {
                    &DiagnosticCaptureStatus::Complete
                } else {
                    &DiagnosticCaptureStatus::Truncated
                }
            );
            LAST_WORK.with(|w| assert!(w.borrow().high_water <= 128));
        }
    }
    #[test]
    fn aj5_clean_and_unavailable_only_capture_performs_no_position_scan() {
        for total in [0, 1] {
            let input = input(&"\n".repeat(1024 * 1024));
            let mut c = RuntimeDiagnosticCapture::new();
            if total != 0 {
                c.record(
                    1,
                    ParserStage::TreeConstruction,
                    ParseErrorCode::Standard(WhatwgParseErrorCode::UnexpectedNullCharacter),
                    None,
                    None,
                    None,
                    ParseErrorPositionSource::Unavailable(
                        PositionUnavailableReason::ParserDidNotProvidePosition,
                    ),
                );
            }
            let r = c.finish(input.as_str(), false, total, true);
            assert_eq!(r.completion(), ParserInputCompletion::PrefixOnly);
            LAST_WORK.with(|w| {
                let w = w.borrow();
                assert_eq!(w.scans, 0);
                assert_eq!(w.scalar_visits, 0);
            });
        }
    }
}

#[cfg(test)]
thread_local! { pub(crate) static PATH_WORK: std::cell::Cell<[usize;4]> = const { std::cell::Cell::new([0;4]) }; }
#[cfg(test)]
pub(crate) fn record_decoder_path_for_test(observed: bool) {
    PATH_WORK.with(|w| {
        let mut v = w.get();
        v[usize::from(observed)] += 1;
        w.set(v);
    });
}
#[cfg(test)]
pub(crate) fn record_token_path_for_test(observed: bool) {
    PATH_WORK.with(|w| {
        let mut v = w.get();
        v[2 + usize::from(observed)] += 1;
        w.set(v);
    });
}
