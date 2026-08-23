use std::fmt::Write;

use html::Node;

use super::collection::RuleCollection;
use super::limits::{StyleResolutionError, StyleResolutionLimits};
use super::rule_inputs::rule_inputs_for_element_with_limits;
use super::selector_dom::build_document_selector_dom_with_element_limit;
use super::source::StylesheetCollectionInput;
use crate::cascade::contract::{
    CascadeCandidateObservationIndex, CascadeDeclarationCandidate, CascadeDeclarationSource,
    CascadeEvaluationFailure, CascadeEvaluationObserver, CascadePriority, CascadePropertyId,
    CascadeResolutionBudget, CascadeResolutionWorkspace,
    resolve_cascade_winners_from_validated_inputs,
};
use crate::selectors::{
    SelectorDomElementId, SelectorMatchingContext, SelectorMatchingEnvironment,
};

pub const CASCADE_EVALUATION_DIAGNOSTIC_VERSION: u16 = 1;

/// Candidate and winner record vectors start at eight entries and then double.
///
/// Eight records keeps the first allocation small while ensuring that a
/// diagnostic with several elements does not ask the allocator to grow for
/// every observed candidate or winner. The checked bounded-growth planner may
/// clamp this target to a lower record-count or live-heap limit.
const MINIMUM_DIAGNOSTIC_RECORD_CAPACITY: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CascadeEvaluationDiagnosticLimits {
    pub max_candidate_records: usize,
    pub max_winner_records: usize,
    pub max_retained_bytes: usize,
    pub max_serialized_bytes: usize,
    pub max_source_text_bytes: usize,
    pub max_property_text_bytes: usize,
    pub max_value_text_bytes: usize,
}

impl Default for CascadeEvaluationDiagnosticLimits {
    fn default() -> Self {
        Self {
            max_candidate_records: 65_536,
            max_winner_records: 16_384,
            max_retained_bytes: 8 * 1024 * 1024,
            max_serialized_bytes: 16 * 1024 * 1024,
            max_source_text_bytes: 256,
            max_property_text_bytes: 128,
            max_value_text_bytes: 4 * 1024,
        }
    }
}

impl CascadeEvaluationDiagnosticLimits {
    pub fn validate(self) -> Result<Self, CascadeEvaluationDiagnosticFailure> {
        if self.max_candidate_records > u32::MAX as usize {
            return Err(
                CascadeEvaluationDiagnosticFailure::UnsupportedConfiguration {
                    limit: CascadeEvaluationDiagnosticLimit::CandidateRecords,
                    configured: self.max_candidate_records,
                    maximum: u32::MAX as usize,
                },
            );
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeEvaluationDiagnosticLimit {
    CandidateRecords,
    WinnerRecords,
    RetainedBytes,
    SerializedBytes,
    SourceTextBytes,
    PropertyTextBytes,
    ValueTextBytes,
}

impl CascadeEvaluationDiagnosticLimit {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::CandidateRecords => "candidate-records",
            Self::WinnerRecords => "winner-records",
            Self::RetainedBytes => "retained-bytes",
            Self::SerializedBytes => "serialized-bytes",
            Self::SourceTextBytes => "source-text-bytes",
            Self::PropertyTextBytes => "property-text-bytes",
            Self::ValueTextBytes => "value-text-bytes",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CascadeDiagnosticCandidateId(u32);

impl CascadeDiagnosticCandidateId {
    fn try_from_usize(value: usize) -> Result<Self, CascadeEvaluationObserverError> {
        u32::try_from(value)
            .map(Self)
            .map_err(|_| CascadeEvaluationObserverError::CandidateIdExhausted { required: value })
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeDiagnosticText(String);

impl CascadeDiagnosticText {
    fn measure(
        maximum: usize,
        limit: CascadeEvaluationDiagnosticLimit,
        write: impl FnOnce(&mut BoundedDiagnosticTextCounter) -> std::fmt::Result,
    ) -> Result<usize, CascadeEvaluationObserverError> {
        let mut counter = BoundedDiagnosticTextCounter::new(maximum, limit);
        if write(&mut counter).is_err() {
            return Err(counter.failure.unwrap_or(
                CascadeEvaluationObserverError::SerializationFailed {
                    stage: "diagnostic-text-measurement",
                },
            ));
        }
        Ok(counter.observed)
    }

    fn try_write_exact(
        measured: usize,
        write: impl FnOnce(&mut String) -> std::fmt::Result,
    ) -> Result<Self, CascadeEvaluationObserverError> {
        let mut text = String::new();
        text.try_reserve_exact(measured).map_err(|_| {
            CascadeEvaluationObserverError::ReservationFailed {
                storage: "diagnostic-text",
                requested: measured,
            }
        })?;
        write(&mut text).map_err(|_| CascadeEvaluationObserverError::SerializationFailed {
            stage: "diagnostic-text-materialization",
        })?;
        if text.len() != measured {
            return Err(CascadeEvaluationObserverError::SerializationFailed {
                stage: "diagnostic-text-length-invariant",
            });
        }
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

struct BoundedDiagnosticTextCounter {
    maximum: usize,
    limit: CascadeEvaluationDiagnosticLimit,
    observed: usize,
    failure: Option<CascadeEvaluationObserverError>,
}

impl BoundedDiagnosticTextCounter {
    fn new(maximum: usize, limit: CascadeEvaluationDiagnosticLimit) -> Self {
        Self {
            maximum,
            limit,
            observed: 0,
            failure: None,
        }
    }
}

impl std::fmt::Write for BoundedDiagnosticTextCounter {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.observed = self.observed.checked_add(value.len()).ok_or_else(|| {
            self.failure = Some(CascadeEvaluationObserverError::LimitExceeded {
                limit: self.limit,
                configured: self.maximum,
                observed: usize::MAX,
            });
            std::fmt::Error
        })?;
        if self.observed > self.maximum {
            self.failure = Some(CascadeEvaluationObserverError::LimitExceeded {
                limit: self.limit,
                configured: self.maximum,
                observed: self.observed,
            });
            return Err(std::fmt::Error);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeEvaluationCandidateRecord {
    id: CascadeDiagnosticCandidateId,
    element: SelectorDomElementId,
    property: CascadePropertyId,
    source: CascadeDeclarationSource,
    priority: CascadePriority,
    source_text: CascadeDiagnosticText,
    property_text: CascadeDiagnosticText,
    value_text: CascadeDiagnosticText,
    winner: bool,
}

impl CascadeEvaluationCandidateRecord {
    pub const fn id(&self) -> CascadeDiagnosticCandidateId {
        self.id
    }
    pub const fn element(&self) -> SelectorDomElementId {
        self.element
    }
    pub const fn property(&self) -> CascadePropertyId {
        self.property
    }
    pub const fn source(&self) -> CascadeDeclarationSource {
        self.source
    }
    pub const fn priority(&self) -> CascadePriority {
        self.priority
    }
    pub fn source_text(&self) -> &str {
        self.source_text.as_str()
    }
    pub fn property_text(&self) -> &str {
        self.property_text.as_str()
    }
    pub fn value_text(&self) -> &str {
        self.value_text.as_str()
    }
    pub const fn is_winner(&self) -> bool {
        self.winner
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeEvaluationWinnerRecord {
    element: SelectorDomElementId,
    property: CascadePropertyId,
    candidate: CascadeDiagnosticCandidateId,
}

impl CascadeEvaluationWinnerRecord {
    pub const fn element(&self) -> SelectorDomElementId {
        self.element
    }
    pub const fn property(&self) -> CascadePropertyId {
        self.property
    }
    pub const fn candidate(&self) -> CascadeDiagnosticCandidateId {
        self.candidate
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeEvaluationDiagnosticSnapshot {
    candidates: Vec<CascadeEvaluationCandidateRecord>,
    winners: Vec<CascadeEvaluationWinnerRecord>,
    serialized: String,
    retained_bytes: usize,
    peak_live_bytes: usize,
    finalization_work_units: usize,
}

impl CascadeEvaluationDiagnosticSnapshot {
    pub const fn version(&self) -> u16 {
        CASCADE_EVALUATION_DIAGNOSTIC_VERSION
    }
    pub fn candidates(&self) -> &[CascadeEvaluationCandidateRecord] {
        &self.candidates
    }
    pub fn winners(&self) -> &[CascadeEvaluationWinnerRecord] {
        &self.winners
    }
    pub const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub const fn peak_live_bytes(&self) -> usize {
        self.peak_live_bytes
    }
    pub fn serialized(&self) -> &str {
        &self.serialized
    }
    pub fn serialized_bytes(&self) -> usize {
        self.serialized.len()
    }
    pub fn to_debug_snapshot(&self) -> String {
        self.serialized.clone()
    }

    #[cfg(test)]
    const fn finalization_work_units(&self) -> usize {
        self.finalization_work_units
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CascadeEvaluationDiagnostic {
    Complete(CascadeEvaluationDiagnosticSnapshot),
    Failed(CascadeEvaluationDiagnosticFailure),
}

impl CascadeEvaluationDiagnostic {
    pub fn to_debug_snapshot(&self) -> String {
        match self {
            Self::Complete(snapshot) => snapshot.to_debug_snapshot(),
            Self::Failed(failure) => failure_debug_snapshot(failure),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CascadeEvaluationDiagnosticFailure {
    StyleExecution(StyleResolutionError),
    LimitExceeded {
        limit: CascadeEvaluationDiagnosticLimit,
        configured: usize,
        observed: usize,
    },
    UnsupportedConfiguration {
        limit: CascadeEvaluationDiagnosticLimit,
        configured: usize,
        maximum: usize,
    },
    ReservationFailed {
        storage: &'static str,
        requested: usize,
    },
    CandidateIdExhausted {
        required: usize,
    },
    SerializationFailed {
        stage: &'static str,
    },
    RecordCapacityGrowthOverflow {
        storage: &'static str,
        current_capacity: usize,
        required: usize,
    },
}

impl CascadeEvaluationDiagnosticFailure {
    pub const fn stable_label(&self) -> &'static str {
        match self {
            Self::StyleExecution(_) => "style-execution",
            Self::LimitExceeded { .. } => "limit-exceeded",
            Self::UnsupportedConfiguration { .. } => "unsupported-configuration",
            Self::ReservationFailed { .. } => "reservation-failed",
            Self::CandidateIdExhausted { .. } => "candidate-id-exhausted",
            Self::SerializationFailed { .. } => "serialization-failed",
            Self::RecordCapacityGrowthOverflow { .. } => "record-capacity-growth-overflow",
        }
    }
}

impl std::fmt::Display for CascadeEvaluationDiagnosticFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "AF6 cascade diagnostic failure: {}",
            self.stable_label()
        )
    }
}

impl std::error::Error for CascadeEvaluationDiagnosticFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::StyleExecution(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CascadeEvaluationObserverError {
    LimitExceeded {
        limit: CascadeEvaluationDiagnosticLimit,
        configured: usize,
        observed: usize,
    },
    ReservationFailed {
        storage: &'static str,
        requested: usize,
    },
    CandidateIdExhausted {
        required: usize,
    },
    SerializationFailed {
        stage: &'static str,
    },
    RecordCapacityGrowthOverflow {
        storage: &'static str,
        current_capacity: usize,
        required: usize,
    },
}

impl From<CascadeEvaluationObserverError> for CascadeEvaluationDiagnosticFailure {
    fn from(error: CascadeEvaluationObserverError) -> Self {
        match error {
            CascadeEvaluationObserverError::LimitExceeded {
                limit,
                configured,
                observed,
            } => Self::LimitExceeded {
                limit,
                configured,
                observed,
            },
            CascadeEvaluationObserverError::ReservationFailed { storage, requested } => {
                Self::ReservationFailed { storage, requested }
            }
            CascadeEvaluationObserverError::CandidateIdExhausted { required } => {
                Self::CandidateIdExhausted { required }
            }
            CascadeEvaluationObserverError::SerializationFailed { stage } => {
                Self::SerializationFailed { stage }
            }
            CascadeEvaluationObserverError::RecordCapacityGrowthOverflow {
                storage,
                current_capacity,
                required,
            } => Self::RecordCapacityGrowthOverflow {
                storage,
                current_capacity,
                required,
            },
        }
    }
}

#[derive(Clone, Copy)]
struct BoundedRecordGrowthRequest {
    storage: &'static str,
    record_limit: CascadeEvaluationDiagnosticLimit,
    current_len: usize,
    current_capacity: usize,
    required_len: usize,
    preferred_len: usize,
    configured_record_limit: usize,
    element_size: usize,
    current_live_heap_capacity: usize,
    max_retained_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BoundedRecordGrowthPlan {
    target_capacity: usize,
    non_record_live_bytes: usize,
}

fn plan_bounded_record_growth(
    request: BoundedRecordGrowthRequest,
) -> Result<Option<BoundedRecordGrowthPlan>, CascadeEvaluationObserverError> {
    if request.current_len > request.current_capacity || request.element_size == 0 {
        return Err(record_capacity_growth_overflow(request));
    }
    if request.required_len > request.configured_record_limit {
        return Err(CascadeEvaluationObserverError::LimitExceeded {
            limit: request.record_limit,
            configured: request.configured_record_limit,
            observed: request.required_len,
        });
    }

    let current_record_bytes = request
        .current_capacity
        .checked_mul(request.element_size)
        .ok_or_else(|| record_capacity_growth_overflow(request))?;
    let non_record_live_bytes = request
        .current_live_heap_capacity
        .checked_sub(current_record_bytes)
        .ok_or_else(|| record_capacity_growth_overflow(request))?;
    if non_record_live_bytes > request.max_retained_bytes {
        return Err(retained_capacity_limit_error(request, usize::MAX));
    }
    let capacity_permitted_by_heap = request
        .max_retained_bytes
        .checked_sub(non_record_live_bytes)
        .ok_or_else(|| retained_capacity_limit_error(request, usize::MAX))?
        / request.element_size;
    let maximum_capacity = request
        .configured_record_limit
        .min(capacity_permitted_by_heap);
    if request.required_len > maximum_capacity {
        let required_live_bytes = request
            .required_len
            .checked_mul(request.element_size)
            .and_then(|bytes| non_record_live_bytes.checked_add(bytes))
            .unwrap_or(usize::MAX);
        return Err(retained_capacity_limit_error(request, required_live_bytes));
    }

    let preferred_len = request
        .preferred_len
        .max(request.required_len)
        .min(maximum_capacity);
    if preferred_len <= request.current_capacity {
        return Ok(None);
    }

    let mut target_capacity = if request.current_capacity == 0 {
        MINIMUM_DIAGNOSTIC_RECORD_CAPACITY.min(maximum_capacity)
    } else {
        request.current_capacity
    };
    while target_capacity < preferred_len {
        target_capacity = target_capacity
            .checked_mul(2)
            .ok_or_else(|| record_capacity_growth_overflow(request))?
            .min(maximum_capacity);
        if target_capacity < request.required_len && target_capacity == maximum_capacity {
            return Err(retained_capacity_limit_error(request, usize::MAX));
        }
    }

    let prospective_live_bytes = target_capacity
        .checked_mul(request.element_size)
        .and_then(|bytes| non_record_live_bytes.checked_add(bytes))
        .ok_or_else(|| retained_capacity_limit_error(request, usize::MAX))?;
    if prospective_live_bytes > request.max_retained_bytes {
        return Err(retained_capacity_limit_error(
            request,
            prospective_live_bytes,
        ));
    }
    Ok(Some(BoundedRecordGrowthPlan {
        target_capacity,
        non_record_live_bytes,
    }))
}

fn try_grow_bounded_records<T>(
    records: &mut Vec<T>,
    mut request: BoundedRecordGrowthRequest,
) -> Result<Option<BoundedRecordGrowthPlan>, CascadeEvaluationObserverError> {
    request.current_len = records.len();
    request.current_capacity = records.capacity();
    request.element_size = std::mem::size_of::<T>();
    let Some(mut plan) = plan_bounded_record_growth(request)? else {
        return Ok(None);
    };
    let additional = plan
        .target_capacity
        .checked_sub(records.len())
        .ok_or_else(|| record_capacity_growth_overflow(request))?;
    records.try_reserve(additional).map_err(|_| {
        CascadeEvaluationObserverError::ReservationFailed {
            storage: request.storage,
            requested: additional,
        }
    })?;

    let actual_capacity = records.capacity();
    let actual_live_bytes = verify_actual_bounded_record_capacity(
        request,
        plan.non_record_live_bytes,
        actual_capacity,
    )?;
    plan.target_capacity = actual_capacity;
    debug_assert!(actual_live_bytes <= request.max_retained_bytes);
    Ok(Some(plan))
}

fn verify_actual_bounded_record_capacity(
    request: BoundedRecordGrowthRequest,
    non_record_live_bytes: usize,
    actual_capacity: usize,
) -> Result<usize, CascadeEvaluationObserverError> {
    if actual_capacity < request.required_len {
        return Err(record_capacity_growth_overflow(request));
    }
    let actual_live_bytes = actual_capacity
        .checked_mul(request.element_size)
        .and_then(|bytes| non_record_live_bytes.checked_add(bytes))
        .ok_or_else(|| retained_capacity_limit_error(request, usize::MAX))?;
    if actual_live_bytes > request.max_retained_bytes {
        return Err(retained_capacity_limit_error(request, actual_live_bytes));
    }
    Ok(actual_live_bytes)
}

fn record_capacity_growth_overflow(
    request: BoundedRecordGrowthRequest,
) -> CascadeEvaluationObserverError {
    CascadeEvaluationObserverError::RecordCapacityGrowthOverflow {
        storage: request.storage,
        current_capacity: request.current_capacity,
        required: request.required_len,
    }
}

fn retained_capacity_limit_error(
    request: BoundedRecordGrowthRequest,
    observed: usize,
) -> CascadeEvaluationObserverError {
    CascadeEvaluationObserverError::LimitExceeded {
        limit: CascadeEvaluationDiagnosticLimit::RetainedBytes,
        configured: request.max_retained_bytes,
        observed,
    }
}

struct BoundedCascadeObserver {
    limits: CascadeEvaluationDiagnosticLimits,
    current_element: Option<SelectorDomElementId>,
    current_element_candidate_start: usize,
    candidates: Vec<CascadeEvaluationCandidateRecord>,
    winners: Vec<CascadeEvaluationWinnerRecord>,
    peak_live_bytes: usize,
}

impl BoundedCascadeObserver {
    fn new(limits: CascadeEvaluationDiagnosticLimits) -> Self {
        Self {
            limits,
            current_element: None,
            current_element_candidate_start: 0,
            candidates: Vec::new(),
            winners: Vec::new(),
            peak_live_bytes: 0,
        }
    }

    fn begin_element(
        &mut self,
        element: SelectorDomElementId,
        admitted_candidates: usize,
    ) -> Result<(), CascadeEvaluationObserverError> {
        self.current_element = Some(element);
        self.current_element_candidate_start = self.candidates.len();
        let observed = self
            .candidates
            .len()
            .checked_add(admitted_candidates)
            .ok_or_else(|| self.retained_limit_error(usize::MAX))?;
        if observed > self.limits.max_candidate_records {
            return Err(CascadeEvaluationObserverError::LimitExceeded {
                limit: CascadeEvaluationDiagnosticLimit::CandidateRecords,
                configured: self.limits.max_candidate_records,
                observed,
            });
        }
        self.try_reserve_candidates(observed)?;
        self.try_reserve_winner_hint(admitted_candidates)
    }

    fn finish(
        mut self,
        serialized_limit: usize,
    ) -> Result<CascadeEvaluationDiagnosticSnapshot, CascadeEvaluationDiagnosticFailure> {
        self.candidates.sort_unstable_by(|left, right| {
            left.element
                .cmp(&right.element)
                .then_with(|| left.property.cmp(&right.property))
                .then_with(|| left.priority.cmp(&right.priority))
                .then_with(|| left.source_text.0.cmp(&right.source_text.0))
                .then_with(|| left.id.cmp(&right.id))
        });
        let candidate_count = self.candidates.len();
        let mut remap = Vec::new();
        self.try_reserve_finalization_storage::<CascadeDiagnosticCandidateId>(
            &mut remap,
            candidate_count,
            0,
            "candidate-id-remap",
        )?;
        remap.resize(candidate_count, CascadeDiagnosticCandidateId(0));
        self.record_peak_with_scratch(remap.capacity(), 0)?;

        for (final_index, candidate) in self.candidates.iter_mut().enumerate() {
            let final_id = CascadeDiagnosticCandidateId::try_from_usize(final_index)
                .map_err(CascadeEvaluationDiagnosticFailure::from)?;
            remap[candidate.id.get() as usize] = final_id;
            candidate.id = final_id;
        }

        let mut winner_marks = Vec::new();
        self.try_reserve_finalization_storage::<u8>(
            &mut winner_marks,
            candidate_count,
            remap.capacity(),
            "winner-marking",
        )?;
        winner_marks.resize(candidate_count, 0);
        self.record_peak_with_scratch(remap.capacity(), winner_marks.capacity())?;

        for winner in &mut self.winners {
            let final_id = remap[winner.candidate.get() as usize];
            winner.candidate = final_id;
            winner_marks[final_id.get() as usize] = 1;
        }
        for candidate in &mut self.candidates {
            candidate.winner = winner_marks[candidate.id.get() as usize] != 0;
        }

        let finalization_work_units = candidate_count
            .checked_add(self.winners.len())
            .and_then(|value| value.checked_add(candidate_count))
            .ok_or_else(|| self.diagnostic_retained_failure(usize::MAX))?;
        drop(winner_marks);
        drop(remap);

        let base_retained_bytes = self
            .live_heap_bytes(self.candidates.capacity(), self.winners.capacity(), 0)
            .ok_or_else(|| self.diagnostic_retained_failure(usize::MAX))?;
        let serialized = serialize_snapshot_bounded(
            &self.candidates,
            &self.winners,
            serialized_limit,
            self.limits.max_retained_bytes,
            base_retained_bytes,
        )?;
        let retained_bytes = base_retained_bytes
            .checked_add(serialized.capacity())
            .ok_or_else(|| self.diagnostic_retained_failure(usize::MAX))?;
        if retained_bytes > self.limits.max_retained_bytes {
            return Err(self.diagnostic_retained_failure(retained_bytes));
        }
        self.peak_live_bytes = self.peak_live_bytes.max(retained_bytes);
        Ok(CascadeEvaluationDiagnosticSnapshot {
            candidates: self.candidates,
            winners: self.winners,
            serialized,
            retained_bytes,
            peak_live_bytes: self.peak_live_bytes,
            finalization_work_units,
        })
    }

    fn retained_limit_error(&self, observed: usize) -> CascadeEvaluationObserverError {
        CascadeEvaluationObserverError::LimitExceeded {
            limit: CascadeEvaluationDiagnosticLimit::RetainedBytes,
            configured: self.limits.max_retained_bytes,
            observed,
        }
    }

    fn diagnostic_retained_failure(&self, observed: usize) -> CascadeEvaluationDiagnosticFailure {
        CascadeEvaluationDiagnosticFailure::LimitExceeded {
            limit: CascadeEvaluationDiagnosticLimit::RetainedBytes,
            configured: self.limits.max_retained_bytes,
            observed,
        }
    }

    fn try_reserve_candidates(
        &mut self,
        required_len: usize,
    ) -> Result<(), CascadeEvaluationObserverError> {
        let current_live_heap_capacity = self
            .live_heap_bytes(self.candidates.capacity(), self.winners.capacity(), 0)
            .ok_or_else(|| self.retained_limit_error(usize::MAX))?;
        try_grow_bounded_records(
            &mut self.candidates,
            BoundedRecordGrowthRequest {
                storage: "candidate-records",
                record_limit: CascadeEvaluationDiagnosticLimit::CandidateRecords,
                current_len: 0,
                current_capacity: 0,
                required_len,
                preferred_len: required_len,
                configured_record_limit: self.limits.max_candidate_records,
                element_size: 0,
                current_live_heap_capacity,
                max_retained_bytes: self.limits.max_retained_bytes,
            },
        )?;
        self.record_current_peak()
    }

    fn try_reserve_winner_hint(
        &mut self,
        admitted_candidates: usize,
    ) -> Result<(), CascadeEvaluationObserverError> {
        let maximum_new_winners =
            admitted_candidates.min(crate::property_registry().entries().len());
        if maximum_new_winners == 0 {
            return self.record_current_peak();
        }
        let required_len = self.winners.len().checked_add(1).ok_or(
            CascadeEvaluationObserverError::RecordCapacityGrowthOverflow {
                storage: "winner-records",
                current_capacity: self.winners.capacity(),
                required: usize::MAX,
            },
        )?;
        let preferred_len = self.winners.len().checked_add(maximum_new_winners).ok_or(
            CascadeEvaluationObserverError::RecordCapacityGrowthOverflow {
                storage: "winner-records",
                current_capacity: self.winners.capacity(),
                required: usize::MAX,
            },
        )?;
        self.try_reserve_winners(required_len, preferred_len)
    }

    fn try_reserve_winner(
        &mut self,
        required_len: usize,
    ) -> Result<(), CascadeEvaluationObserverError> {
        self.try_reserve_winners(required_len, required_len)
    }

    fn try_reserve_winners(
        &mut self,
        required_len: usize,
        preferred_len: usize,
    ) -> Result<(), CascadeEvaluationObserverError> {
        let current_live_heap_capacity = self
            .live_heap_bytes(self.candidates.capacity(), self.winners.capacity(), 0)
            .ok_or_else(|| self.retained_limit_error(usize::MAX))?;
        try_grow_bounded_records(
            &mut self.winners,
            BoundedRecordGrowthRequest {
                storage: "winner-records",
                record_limit: CascadeEvaluationDiagnosticLimit::WinnerRecords,
                current_len: 0,
                current_capacity: 0,
                required_len,
                preferred_len,
                configured_record_limit: self.limits.max_winner_records,
                element_size: 0,
                current_live_heap_capacity,
                max_retained_bytes: self.limits.max_retained_bytes,
            },
        )?;
        self.record_current_peak()
    }

    fn preflight_candidate_text(
        &self,
        source_bytes: usize,
        property_bytes: usize,
        value_bytes: usize,
    ) -> Result<(), CascadeEvaluationObserverError> {
        let additional = source_bytes
            .checked_add(property_bytes)
            .and_then(|value| value.checked_add(value_bytes))
            .ok_or_else(|| self.retained_limit_error(usize::MAX))?;
        let prospective = self
            .live_heap_bytes(
                self.candidates.capacity(),
                self.winners.capacity(),
                additional,
            )
            .ok_or_else(|| self.retained_limit_error(usize::MAX))?;
        self.ensure_observer_retained_limit(prospective)
    }

    fn ensure_observer_retained_limit(
        &self,
        observed: usize,
    ) -> Result<(), CascadeEvaluationObserverError> {
        if observed > self.limits.max_retained_bytes {
            Err(self.retained_limit_error(observed))
        } else {
            Ok(())
        }
    }

    fn record_current_peak(&mut self) -> Result<(), CascadeEvaluationObserverError> {
        let observed = self
            .live_heap_bytes(self.candidates.capacity(), self.winners.capacity(), 0)
            .ok_or_else(|| self.retained_limit_error(usize::MAX))?;
        self.ensure_observer_retained_limit(observed)?;
        self.peak_live_bytes = self.peak_live_bytes.max(observed);
        Ok(())
    }

    fn try_reserve_finalization_storage<T>(
        &mut self,
        storage: &mut Vec<T>,
        required: usize,
        other_scratch_capacity: usize,
        label: &'static str,
    ) -> Result<(), CascadeEvaluationDiagnosticFailure> {
        let scratch_bytes = required
            .checked_mul(std::mem::size_of::<T>())
            .and_then(|value| {
                value.checked_add(
                    other_scratch_capacity
                        .checked_mul(std::mem::size_of::<CascadeDiagnosticCandidateId>())?,
                )
            })
            .ok_or_else(|| self.diagnostic_retained_failure(usize::MAX))?;
        let prospective = self
            .live_heap_bytes(
                self.candidates.capacity(),
                self.winners.capacity(),
                scratch_bytes,
            )
            .ok_or_else(|| self.diagnostic_retained_failure(usize::MAX))?;
        if prospective > self.limits.max_retained_bytes {
            return Err(self.diagnostic_retained_failure(prospective));
        }
        try_reserve_final_records(storage, required, label)
    }

    fn record_peak_with_scratch(
        &mut self,
        remap_capacity: usize,
        winner_mark_capacity: usize,
    ) -> Result<(), CascadeEvaluationDiagnosticFailure> {
        let scratch_bytes = remap_capacity
            .checked_mul(std::mem::size_of::<CascadeDiagnosticCandidateId>())
            .and_then(|value| value.checked_add(winner_mark_capacity))
            .ok_or_else(|| self.diagnostic_retained_failure(usize::MAX))?;
        let observed = self
            .live_heap_bytes(
                self.candidates.capacity(),
                self.winners.capacity(),
                scratch_bytes,
            )
            .ok_or_else(|| self.diagnostic_retained_failure(usize::MAX))?;
        if observed > self.limits.max_retained_bytes {
            return Err(self.diagnostic_retained_failure(observed));
        }
        self.peak_live_bytes = self.peak_live_bytes.max(observed);
        Ok(())
    }

    fn live_heap_bytes(
        &self,
        candidate_capacity: usize,
        winner_capacity: usize,
        additional_bytes: usize,
    ) -> Option<usize> {
        candidate_capacity
            .checked_mul(std::mem::size_of::<CascadeEvaluationCandidateRecord>())?
            .checked_add(
                winner_capacity.checked_mul(std::mem::size_of::<CascadeEvaluationWinnerRecord>())?,
            )?
            .checked_add(self.candidate_text_capacity_bytes()?)?
            .checked_add(additional_bytes)
    }

    fn candidate_text_capacity_bytes(&self) -> Option<usize> {
        self.candidates.iter().try_fold(0usize, |bytes, candidate| {
            bytes
                .checked_add(candidate.source_text.0.capacity())?
                .checked_add(candidate.property_text.0.capacity())?
                .checked_add(candidate.value_text.0.capacity())
        })
    }
}

impl CascadeEvaluationObserver for BoundedCascadeObserver {
    type Error = CascadeEvaluationObserverError;

    fn candidate(
        &mut self,
        observation_index: CascadeCandidateObservationIndex,
        candidate: CascadeDeclarationCandidate<'_>,
    ) -> Result<(), Self::Error> {
        let source_bytes = CascadeDiagnosticText::measure(
            self.limits.max_source_text_bytes,
            CascadeEvaluationDiagnosticLimit::SourceTextBytes,
            |writer| write_source_label(writer, candidate.source()),
        )?;
        let property_bytes = CascadeDiagnosticText::measure(
            self.limits.max_property_text_bytes,
            CascadeEvaluationDiagnosticLimit::PropertyTextBytes,
            |writer| writer.write_str(candidate.property().name()),
        )?;
        let value_bytes = CascadeDiagnosticText::measure(
            self.limits.max_value_text_bytes,
            CascadeEvaluationDiagnosticLimit::ValueTextBytes,
            |writer| {
                if candidate.value().write_css_text(writer)? {
                    Ok(())
                } else {
                    writer.write_str("<unresolved-value>")
                }
            },
        )?;
        self.preflight_candidate_text(source_bytes, property_bytes, value_bytes)?;
        let source_text = CascadeDiagnosticText::try_write_exact(source_bytes, |writer| {
            write_source_label(writer, candidate.source())
        })?;
        let property_text = CascadeDiagnosticText::try_write_exact(property_bytes, |writer| {
            writer.write_str(candidate.property().name())
        })?;
        let value_text = CascadeDiagnosticText::try_write_exact(value_bytes, |writer| {
            if candidate.value().write_css_text(writer)? {
                Ok(())
            } else {
                writer.write_str("<unresolved-value>")
            }
        })?;
        let provisional_index = self
            .current_element_candidate_start
            .checked_add(observation_index.get())
            .ok_or(CascadeEvaluationObserverError::CandidateIdExhausted {
                required: usize::MAX,
            })?;
        if provisional_index != self.candidates.len() {
            return Err(CascadeEvaluationObserverError::SerializationFailed {
                stage: "candidate-observation-sequence",
            });
        }
        let observation_id = CascadeDiagnosticCandidateId::try_from_usize(provisional_index)?;
        self.candidates.push(CascadeEvaluationCandidateRecord {
            id: observation_id,
            element: self
                .current_element
                .expect("diagnostic observer begins an element before evaluation"),
            property: candidate.property(),
            source: candidate.source(),
            priority: candidate.priority(),
            source_text,
            property_text,
            value_text,
            winner: false,
        });
        self.record_current_peak()
    }

    fn final_winner(
        &mut self,
        property: CascadePropertyId,
        observation_index: CascadeCandidateObservationIndex,
        _candidate: CascadeDeclarationCandidate<'_>,
    ) -> Result<(), Self::Error> {
        let observed = self
            .winners
            .len()
            .checked_add(1)
            .ok_or_else(|| self.retained_limit_error(usize::MAX))?;
        if observed > self.limits.max_winner_records {
            return Err(CascadeEvaluationObserverError::LimitExceeded {
                limit: CascadeEvaluationDiagnosticLimit::WinnerRecords,
                configured: self.limits.max_winner_records,
                observed,
            });
        }
        self.try_reserve_winner(observed)?;
        let provisional_index = self
            .current_element_candidate_start
            .checked_add(observation_index.get())
            .ok_or(CascadeEvaluationObserverError::CandidateIdExhausted {
                required: usize::MAX,
            })?;
        self.winners.push(CascadeEvaluationWinnerRecord {
            element: self
                .current_element
                .expect("diagnostic observer begins an element before evaluation"),
            property,
            candidate: CascadeDiagnosticCandidateId::try_from_usize(provisional_index)?,
        });
        self.record_current_peak()
    }
}

pub fn cascade_evaluation_diagnostic(
    root: &Node,
    environment: SelectorMatchingEnvironment,
    sheets: &[StylesheetCollectionInput<'_>],
    style_limits: &StyleResolutionLimits,
    diagnostic_limits: CascadeEvaluationDiagnosticLimits,
) -> CascadeEvaluationDiagnostic {
    let diagnostic_limits = match diagnostic_limits.validate() {
        Ok(limits) => limits,
        Err(failure) => return CascadeEvaluationDiagnostic::Failed(failure),
    };
    let collection = match RuleCollection::try_new(sheets, style_limits) {
        Ok(collection) => collection,
        Err(error) => {
            return CascadeEvaluationDiagnostic::Failed(
                CascadeEvaluationDiagnosticFailure::StyleExecution(
                    StyleResolutionError::RuleCollectionBuild(error),
                ),
            );
        }
    };
    let index = match build_document_selector_dom_with_element_limit(
        root,
        style_limits.max_styled_elements_per_document,
    ) {
        Ok(index) => index,
        Err(error) => {
            return CascadeEvaluationDiagnostic::Failed(
                CascadeEvaluationDiagnosticFailure::StyleExecution(error),
            );
        }
    };
    let budget = match CascadeResolutionBudget::try_new(
        style_limits.max_declaration_inputs_per_element,
        style_limits.max_inline_declarations_per_element,
        style_limits.max_matched_rules_per_element,
    ) {
        Ok(budget) => budget,
        Err(error) => {
            return CascadeEvaluationDiagnostic::Failed(
                CascadeEvaluationDiagnosticFailure::StyleExecution(
                    StyleResolutionError::CascadeResolution(error),
                ),
            );
        }
    };
    let mut workspace = match CascadeResolutionWorkspace::try_new(budget) {
        Ok(workspace) => workspace,
        Err(error) => {
            return CascadeEvaluationDiagnostic::Failed(
                CascadeEvaluationDiagnosticFailure::StyleExecution(
                    StyleResolutionError::CascadeResolution(error),
                ),
            );
        }
    };
    let context =
        SelectorMatchingContext::with_limits(&index, environment, style_limits.selector_matching);
    let mut observer = BoundedCascadeObserver::new(diagnostic_limits);
    for element in index.elements() {
        let inputs = match rule_inputs_for_element_with_limits(
            &index,
            &context,
            element,
            &collection,
            style_limits,
            budget,
        ) {
            Ok(inputs) => inputs,
            Err(error) => {
                return CascadeEvaluationDiagnostic::Failed(
                    CascadeEvaluationDiagnosticFailure::StyleExecution(error),
                );
            }
        };
        if let Err(error) = observer.begin_element(element, inputs.admitted_candidate_count()) {
            return CascadeEvaluationDiagnostic::Failed(error.into());
        }
        match resolve_cascade_winners_from_validated_inputs(
            &inputs,
            budget,
            &mut workspace,
            &mut observer,
        ) {
            Ok(_) => {}
            Err(CascadeEvaluationFailure::Cascade(error)) => {
                return CascadeEvaluationDiagnostic::Failed(
                    CascadeEvaluationDiagnosticFailure::StyleExecution(
                        StyleResolutionError::CascadeResolution(error),
                    ),
                );
            }
            Err(CascadeEvaluationFailure::Observer(error)) => {
                return CascadeEvaluationDiagnostic::Failed(error.into());
            }
        }
    }
    match observer.finish(diagnostic_limits.max_serialized_bytes) {
        Ok(snapshot) => CascadeEvaluationDiagnostic::Complete(snapshot),
        Err(failure) => CascadeEvaluationDiagnostic::Failed(failure),
    }
}

fn failure_debug_snapshot(failure: &CascadeEvaluationDiagnosticFailure) -> String {
    let mut out = String::new();
    writeln!(&mut out, "version: {CASCADE_EVALUATION_DIAGNOSTIC_VERSION}")
        .expect("String formatting is infallible");
    writeln!(&mut out, "cascade-evaluation-diagnostic").expect("String formatting is infallible");
    write!(&mut out, "failure: kind={}", failure.stable_label())
        .expect("String formatting is infallible");
    match failure {
        CascadeEvaluationDiagnosticFailure::StyleExecution(error) => {
            write!(&mut out, " style-kind={}", error.stable_label())
                .expect("String formatting is infallible");
        }
        CascadeEvaluationDiagnosticFailure::LimitExceeded {
            limit,
            configured,
            observed,
        } => {
            write!(
                &mut out,
                " limit={} configured={configured} observed={observed}",
                limit.stable_label()
            )
            .expect("String formatting is infallible");
        }
        CascadeEvaluationDiagnosticFailure::UnsupportedConfiguration {
            limit,
            configured,
            maximum,
        } => {
            write!(
                &mut out,
                " limit={} configured={configured} maximum={maximum}",
                limit.stable_label()
            )
            .expect("String formatting is infallible");
        }
        CascadeEvaluationDiagnosticFailure::ReservationFailed { storage, requested } => {
            write!(&mut out, " storage={storage} requested={requested}")
                .expect("String formatting is infallible");
        }
        CascadeEvaluationDiagnosticFailure::CandidateIdExhausted { required } => {
            write!(&mut out, " required={required}").expect("String formatting is infallible");
        }
        CascadeEvaluationDiagnosticFailure::SerializationFailed { stage } => {
            write!(&mut out, " stage={stage}").expect("String formatting is infallible");
        }
        CascadeEvaluationDiagnosticFailure::RecordCapacityGrowthOverflow {
            storage,
            current_capacity,
            required,
        } => {
            write!(
                &mut out,
                " storage={storage} current-capacity={current_capacity} required={required}"
            )
            .expect("String formatting is infallible");
        }
    }
    out.push('\n');
    out
}

fn serialize_snapshot_bounded(
    candidates: &[CascadeEvaluationCandidateRecord],
    winners: &[CascadeEvaluationWinnerRecord],
    maximum_serialized: usize,
    maximum_retained: usize,
    retained_before_serialized: usize,
) -> Result<String, CascadeEvaluationDiagnosticFailure> {
    let mut counter = SerializedByteCounter::default();
    write_snapshot(&mut counter, candidates, winners).map_err(|_| {
        CascadeEvaluationDiagnosticFailure::LimitExceeded {
            limit: CascadeEvaluationDiagnosticLimit::SerializedBytes,
            configured: maximum_serialized,
            observed: usize::MAX,
        }
    })?;
    if counter.bytes > maximum_serialized {
        return Err(CascadeEvaluationDiagnosticFailure::LimitExceeded {
            limit: CascadeEvaluationDiagnosticLimit::SerializedBytes,
            configured: maximum_serialized,
            observed: counter.bytes,
        });
    }
    let retained_with_serialized = retained_before_serialized
        .checked_add(counter.bytes)
        .ok_or(CascadeEvaluationDiagnosticFailure::LimitExceeded {
            limit: CascadeEvaluationDiagnosticLimit::RetainedBytes,
            configured: maximum_retained,
            observed: usize::MAX,
        })?;
    if retained_with_serialized > maximum_retained {
        return Err(CascadeEvaluationDiagnosticFailure::LimitExceeded {
            limit: CascadeEvaluationDiagnosticLimit::RetainedBytes,
            configured: maximum_retained,
            observed: retained_with_serialized,
        });
    }
    let mut out = String::new();
    try_reserve_serialized_bytes(&mut out, counter.bytes)?;
    write_snapshot(&mut out, candidates, winners).map_err(|_| {
        CascadeEvaluationDiagnosticFailure::SerializationFailed {
            stage: "snapshot-materialization",
        }
    })?;
    if out.len() != counter.bytes {
        return Err(CascadeEvaluationDiagnosticFailure::SerializationFailed {
            stage: "snapshot-length-invariant",
        });
    }
    Ok(out)
}

fn try_reserve_final_records<T>(
    records: &mut Vec<T>,
    additional: usize,
    storage: &'static str,
) -> Result<(), CascadeEvaluationDiagnosticFailure> {
    records.try_reserve_exact(additional).map_err(|_| {
        CascadeEvaluationDiagnosticFailure::ReservationFailed {
            storage,
            requested: additional,
        }
    })
}

fn try_reserve_serialized_bytes(
    output: &mut String,
    requested: usize,
) -> Result<(), CascadeEvaluationDiagnosticFailure> {
    output.try_reserve_exact(requested).map_err(|_| {
        CascadeEvaluationDiagnosticFailure::ReservationFailed {
            storage: "serialized-bytes",
            requested,
        }
    })
}

#[derive(Default)]
struct SerializedByteCounter {
    bytes: usize,
}

impl std::fmt::Write for SerializedByteCounter {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.bytes = self.bytes.checked_add(value.len()).ok_or(std::fmt::Error)?;
        Ok(())
    }
}

fn write_snapshot(
    out: &mut impl std::fmt::Write,
    candidates: &[CascadeEvaluationCandidateRecord],
    winners: &[CascadeEvaluationWinnerRecord],
) -> std::fmt::Result {
    writeln!(out, "version: {}", CASCADE_EVALUATION_DIAGNOSTIC_VERSION)?;
    writeln!(out, "cascade-evaluation-diagnostic")?;
    writeln!(out, "candidates: {}", candidates.len())?;
    for candidate in candidates {
        write!(
            out,
            "  candidate[{}]: element={} property={} source={} band={} attachment={} specificity=",
            candidate.id.get(),
            candidate.element.get(),
            candidate.property_text.as_str(),
            candidate.source_text.as_str(),
            candidate.priority.band().as_debug_label(),
            if candidate
                .priority
                .declaration_precedence()
                .is_element_attached()
            {
                "element-attached"
            } else {
                "style-rule"
            },
        )?;
        if let Some(specificity) = candidate.priority.specificity() {
            write!(
                out,
                "{},{},{}",
                specificity.a(),
                specificity.b(),
                specificity.c()
            )?;
        } else {
            out.write_str("not-applicable")?;
        }
        out.write_str(" source-order=")?;
        if let Some(source_order) = candidate.priority.source_order() {
            write!(
                out,
                "{}/{}",
                source_order.stylesheet().get(),
                source_order.rule().get()
            )?;
        } else {
            out.write_str("not-applicable")?;
        }
        write!(
            out,
            " declaration-order={} winner={} value=",
            candidate.priority.declaration_order().get(),
            candidate.winner,
        )?;
        write_diagnostic_quoted(out, candidate.value_text.as_str())?;
        out.write_char('\n')?;
    }
    writeln!(out, "winners: {}", winners.len())?;
    for winner in winners {
        writeln!(
            out,
            "  element={} property={} candidate={}",
            winner.element.get(),
            winner.property.name(),
            winner.candidate.get(),
        )?;
    }
    Ok(())
}

/// Writes AF6 diagnostic quoted text.
///
/// Grammar: the value is enclosed in ASCII double quotes. Double quote,
/// backslash, LF, CR, and TAB are escaped as `\"`, `\\`, `\n`, `\r`, and
/// `\t`. Other C0 controls and DEL use lowercase `\u{hex}`; every other
/// Unicode scalar is emitted unchanged.
fn write_diagnostic_quoted(out: &mut impl std::fmt::Write, value: &str) -> std::fmt::Result {
    out.write_char('"')?;
    for character in value.chars() {
        match character {
            '"' => out.write_str("\\\"")?,
            '\\' => out.write_str("\\\\")?,
            '\n' => out.write_str("\\n")?,
            '\r' => out.write_str("\\r")?,
            '\t' => out.write_str("\\t")?,
            '\u{0000}'..='\u{001f}' | '\u{007f}' => {
                write!(out, "\\u{{{:x}}}", character as u32)?;
            }
            _ => out.write_char(character)?,
        }
    }
    out.write_char('"')
}

fn write_source_label(
    writer: &mut impl std::fmt::Write,
    source: CascadeDeclarationSource,
) -> std::fmt::Result {
    match source {
        CascadeDeclarationSource::Stylesheet(source) => write!(
            writer,
            "stylesheet[{}/{}]/declaration[{}]",
            source.source_id().get(),
            source.raw_rule_index().get(),
            source.declaration_index().get()
        ),
        CascadeDeclarationSource::InlineStyle(source) => match source.inline_style().element() {
            Some(element) => write!(
                writer,
                "inline-style[element={}]/declaration[{}]",
                element.get(),
                source.declaration_index().get()
            ),
            None => write!(
                writer,
                "inline-style/declaration[{}]",
                source.declaration_index().get()
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ParseOptions, StylesheetOrder, StylesheetSourceId, parse_stylesheet_with_options,
        try_resolve_document_styles_from_cascade_inputs_with_limits,
    };

    fn fixture() -> (Node, crate::StylesheetParse) {
        let dom = html::parse_document(
            "<!doctype html><html><body><div class=target style='width: 9px'></div></body></html>",
            html::HtmlParseOptions::default(),
        )
        .expect("diagnostic DOM parses")
        .document;
        let sheet = parse_stylesheet_with_options(
            ".target { color: red; } div { color: blue; width: 3px; }",
            &ParseOptions::stylesheet(),
        );
        (dom, sheet)
    }

    fn input(sheet: &crate::StylesheetParse) -> StylesheetCollectionInput<'_> {
        StylesheetCollectionInput::author(
            StylesheetSourceId::in_memory_generation_index(0),
            StylesheetOrder::new(0),
            sheet,
            super::super::source::StylesheetConditionInput::None,
        )
    }

    fn complete(limits: CascadeEvaluationDiagnosticLimits) -> CascadeEvaluationDiagnosticSnapshot {
        let (dom, sheet) = fixture();
        match cascade_evaluation_diagnostic(
            &dom,
            SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks),
            &[input(&sheet)],
            &StyleResolutionLimits::default(),
            limits,
        ) {
            CascadeEvaluationDiagnostic::Complete(snapshot) => snapshot,
            CascadeEvaluationDiagnostic::Failed(failure) => {
                panic!("expected complete diagnostic, got {failure:?}")
            }
        }
    }

    #[test]
    fn bounded_diagnostic_uses_typed_ids_and_production_winners() {
        let (dom, sheet) = fixture();
        let inputs = [input(&sheet)];
        let limits = StyleResolutionLimits::default();
        let resolved = try_resolve_document_styles_from_cascade_inputs_with_limits(
            &dom,
            SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks),
            &inputs,
            &limits,
        )
        .expect("production cascade resolves");
        let diagnostic = match cascade_evaluation_diagnostic(
            &dom,
            SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks),
            &inputs,
            &limits,
            CascadeEvaluationDiagnosticLimits::default(),
        ) {
            CascadeEvaluationDiagnostic::Complete(snapshot) => snapshot,
            CascadeEvaluationDiagnostic::Failed(failure) => panic!("{failure:?}"),
        };

        for winner in diagnostic.winners() {
            let candidate = &diagnostic.candidates()[winner.candidate().get() as usize];
            assert_eq!(candidate.id(), winner.candidate());
            assert_eq!(candidate.element(), winner.element());
            assert_eq!(candidate.property(), winner.property());
            assert!(candidate.is_winner());
            let production = resolved
                .entries()
                .iter()
                .find(|entry| entry.selector_element_id() == winner.element())
                .and_then(|entry| entry.style().get(winner.property()))
                .expect("diagnostic winner has production resolved entry");
            if let crate::ResolvedValueSource::Winner(production) = production.source() {
                assert_eq!(production.source, candidate.source());
                assert_eq!(production.priority, candidate.priority());
                assert_eq!(
                    production.value.to_css_text().as_deref(),
                    Some(candidate.value_text())
                );
            }
        }
        assert_eq!(diagnostic.version(), 1);
        assert_eq!(diagnostic.serialized(), diagnostic.to_debug_snapshot());
        assert_eq!(diagnostic.serialized_bytes(), diagnostic.serialized().len());
    }

    #[test]
    fn diagnostic_limits_fail_in_their_own_typed_domain() {
        let base = CascadeEvaluationDiagnosticLimits::default();
        let cases = [
            (
                CascadeEvaluationDiagnosticLimits {
                    max_candidate_records: 0,
                    ..base
                },
                CascadeEvaluationDiagnosticLimit::CandidateRecords,
            ),
            (
                CascadeEvaluationDiagnosticLimits {
                    max_winner_records: 0,
                    ..base
                },
                CascadeEvaluationDiagnosticLimit::WinnerRecords,
            ),
            (
                CascadeEvaluationDiagnosticLimits {
                    max_retained_bytes: 0,
                    ..base
                },
                CascadeEvaluationDiagnosticLimit::RetainedBytes,
            ),
            (
                CascadeEvaluationDiagnosticLimits {
                    max_serialized_bytes: 1,
                    ..base
                },
                CascadeEvaluationDiagnosticLimit::SerializedBytes,
            ),
            (
                CascadeEvaluationDiagnosticLimits {
                    max_value_text_bytes: 1,
                    ..base
                },
                CascadeEvaluationDiagnosticLimit::ValueTextBytes,
            ),
            (
                CascadeEvaluationDiagnosticLimits {
                    max_source_text_bytes: 1,
                    ..base
                },
                CascadeEvaluationDiagnosticLimit::SourceTextBytes,
            ),
            (
                CascadeEvaluationDiagnosticLimits {
                    max_property_text_bytes: 1,
                    ..base
                },
                CascadeEvaluationDiagnosticLimit::PropertyTextBytes,
            ),
        ];
        for (limits, expected) in cases {
            let (dom, sheet) = fixture();
            let failure = match cascade_evaluation_diagnostic(
                &dom,
                SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks),
                &[input(&sheet)],
                &StyleResolutionLimits::default(),
                limits,
            ) {
                CascadeEvaluationDiagnostic::Failed(failure) => failure,
                CascadeEvaluationDiagnostic::Complete(_) => panic!("limit must fail"),
            };
            assert!(matches!(
                failure,
                CascadeEvaluationDiagnosticFailure::LimitExceeded { limit, .. }
                    if limit == expected
            ));
        }
    }

    #[test]
    fn diagnostic_id_configuration_is_checked_separately_from_style_budget() {
        let invalid = CascadeEvaluationDiagnosticLimits {
            max_candidate_records: u32::MAX as usize + 1,
            ..CascadeEvaluationDiagnosticLimits::default()
        };
        assert_eq!(
            invalid.validate(),
            Err(
                CascadeEvaluationDiagnosticFailure::UnsupportedConfiguration {
                    limit: CascadeEvaluationDiagnosticLimit::CandidateRecords,
                    configured: u32::MAX as usize + 1,
                    maximum: u32::MAX as usize,
                }
            )
        );

        CascadeResolutionBudget::try_new(1, 1, 1)
            .expect("production budget is independent from diagnostic identity bounds");
    }

    #[test]
    fn cascade_failure_has_one_canonical_diagnostic_representation() {
        let (dom, sheet) = fixture();
        let style_limits = StyleResolutionLimits {
            max_declaration_inputs_per_element: usize::MAX,
            max_inline_declarations_per_element: 1,
            ..StyleResolutionLimits::default()
        };
        let diagnostic = cascade_evaluation_diagnostic(
            &dom,
            SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks),
            &[input(&sheet)],
            &style_limits,
            CascadeEvaluationDiagnosticLimits::default(),
        );
        assert!(matches!(
            diagnostic,
            CascadeEvaluationDiagnostic::Failed(
                CascadeEvaluationDiagnosticFailure::StyleExecution(
                    StyleResolutionError::CascadeResolution(
                        crate::CascadeResolutionError::CandidateCeilingOverflow { .. }
                    )
                )
            )
        ));
        assert_eq!(
            diagnostic.to_debug_snapshot(),
            concat!(
                "version: 1\n",
                "cascade-evaluation-diagnostic\n",
                "failure: kind=style-execution style-kind=candidate-ceiling-overflow\n",
            )
        );
    }

    #[test]
    fn diagnostic_output_is_stable_after_candidate_presentation_sorting() {
        let first = complete(CascadeEvaluationDiagnosticLimits::default());
        let second = complete(CascadeEvaluationDiagnosticLimits::default());
        assert_eq!(first, second);
        assert_eq!(first.to_debug_snapshot(), second.to_debug_snapshot());
        assert!(
            first
                .candidates()
                .windows(2)
                .all(|pair| pair[0].id().get() + 1 == pair[1].id().get())
        );
    }

    #[test]
    fn complete_diagnostic_retains_authoritative_bounded_serialization() {
        let mut diagnostic = complete(CascadeEvaluationDiagnosticLimits::default());
        let retained = diagnostic.serialized().to_string();
        assert_eq!(diagnostic.serialized_bytes(), retained.len());

        diagnostic.candidates[0].winner = !diagnostic.candidates[0].winner;
        assert_eq!(
            diagnostic.to_debug_snapshot(),
            retained,
            "the accessor clones the retained artifact instead of reserializing records"
        );
    }

    #[test]
    fn diagnostic_quoting_grammar_is_explicit_and_stable() {
        let mut serialized = String::new();
        write_diagnostic_quoted(
            &mut serialized,
            "quote\" slash\\ newline\nreturn\rtab\tcontrol\u{0007}",
        )
        .expect("String writer is infallible");
        assert_eq!(
            serialized,
            "\"quote\\\" slash\\\\ newline\\nreturn\\rtab\\tcontrol\\u{7}\""
        );

        let mut counter = SerializedByteCounter::default();
        write_diagnostic_quoted(&mut counter, "\"\\\n\r\t").expect("counter accepts quoted text");
        let mut materialized = String::new();
        write_diagnostic_quoted(&mut materialized, "\"\\\n\r\t")
            .expect("String accepts quoted text");
        assert_eq!(counter.bytes, materialized.len());
    }

    #[test]
    fn retained_storage_accounting_covers_capacities_serialization_and_peak_scratch() {
        let diagnostic = complete(CascadeEvaluationDiagnosticLimits::default());
        let record_bytes = diagnostic.candidates.capacity()
            * std::mem::size_of::<CascadeEvaluationCandidateRecord>();
        let winner_bytes =
            diagnostic.winners.capacity() * std::mem::size_of::<CascadeEvaluationWinnerRecord>();
        let text_bytes = diagnostic
            .candidates
            .iter()
            .map(|candidate| {
                candidate.source_text.0.capacity()
                    + candidate.property_text.0.capacity()
                    + candidate.value_text.0.capacity()
            })
            .sum::<usize>();
        let expected_retained =
            record_bytes + winner_bytes + text_bytes + diagnostic.serialized.capacity();
        assert_eq!(diagnostic.retained_bytes(), expected_retained);
        assert!(diagnostic.peak_live_bytes() >= diagnostic.retained_bytes());

        let exact_peak = diagnostic.peak_live_bytes();
        let exact = complete(CascadeEvaluationDiagnosticLimits {
            max_retained_bytes: exact_peak,
            ..CascadeEvaluationDiagnosticLimits::default()
        });
        assert!(exact.peak_live_bytes() <= exact_peak);

        let (dom, sheet) = fixture();
        let bounded = cascade_evaluation_diagnostic(
            &dom,
            SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks),
            &[input(&sheet)],
            &StyleResolutionLimits::default(),
            CascadeEvaluationDiagnosticLimits {
                max_retained_bytes: exact_peak - 1,
                ..CascadeEvaluationDiagnosticLimits::default()
            },
        );
        assert!(matches!(
            bounded,
            CascadeEvaluationDiagnostic::Failed(
                CascadeEvaluationDiagnosticFailure::LimitExceeded {
                    limit: CascadeEvaluationDiagnosticLimit::RetainedBytes,
                    ..
                }
            )
        ));
    }

    #[test]
    fn candidate_and_winner_finalization_has_linear_indexed_remap_work() {
        let diagnostic = complete(CascadeEvaluationDiagnosticLimits::default());
        assert_eq!(
            diagnostic.finalization_work_units(),
            diagnostic.candidates().len() * 2 + diagnostic.winners().len()
        );
        for winner in diagnostic.winners() {
            let final_index = winner.candidate().get() as usize;
            assert!(final_index < diagnostic.candidates().len());
            assert_eq!(
                diagnostic.candidates()[final_index].id(),
                winner.candidate()
            );
            assert!(diagnostic.candidates()[final_index].is_winner());
        }
    }

    fn growth_request(
        current_capacity: usize,
        required_len: usize,
        preferred_len: usize,
        configured_record_limit: usize,
        element_size: usize,
        max_retained_bytes: usize,
    ) -> BoundedRecordGrowthRequest {
        BoundedRecordGrowthRequest {
            storage: "test-records",
            record_limit: CascadeEvaluationDiagnosticLimit::CandidateRecords,
            current_len: current_capacity,
            current_capacity,
            required_len,
            preferred_len,
            configured_record_limit,
            element_size,
            current_live_heap_capacity: current_capacity
                .checked_mul(element_size)
                .expect("test growth request current capacity is representable"),
            max_retained_bytes,
        }
    }

    #[test]
    fn record_growth_policy_uses_minimum_chunk_checked_doubling_and_bounded_clamps() {
        let first = plan_bounded_record_growth(growth_request(0, 1, 1, 64, 16, 1_024))
            .expect("first record growth is representable")
            .expect("empty storage needs capacity");
        assert_eq!(first.target_capacity, MINIMUM_DIAGNOSTIC_RECORD_CAPACITY);
        assert_eq!(first.target_capacity * 16, 8 * 16);

        let doubled = plan_bounded_record_growth(growth_request(8, 9, 9, 64, 16, 1_024))
            .expect("doubling is representable")
            .expect("ninth record needs capacity");
        assert_eq!(doubled.target_capacity, 16);

        let record_clamped = plan_bounded_record_growth(growth_request(8, 9, 64, 10, 16, 1_024))
            .expect("record-limit clamp is representable")
            .expect("preferred capacity extends current storage");
        assert_eq!(record_clamped.target_capacity, 10);

        let heap_clamped = plan_bounded_record_growth(growth_request(0, 1, 64, 64, 16, 48))
            .expect("heap-limit clamp is representable")
            .expect("one required record fits");
        assert_eq!(heap_clamped.target_capacity, 3);
        assert_eq!(heap_clamped.target_capacity * 16, 48);
    }

    #[test]
    fn record_growth_failures_are_typed_for_count_heap_actual_capacity_and_overflow() {
        assert!(matches!(
            plan_bounded_record_growth(growth_request(0, 3, 3, 2, 16, 1_024)),
            Err(CascadeEvaluationObserverError::LimitExceeded {
                limit: CascadeEvaluationDiagnosticLimit::CandidateRecords,
                configured: 2,
                observed: 3,
            })
        ));
        assert!(matches!(
            plan_bounded_record_growth(growth_request(0, 3, 3, 64, 16, 32)),
            Err(CascadeEvaluationObserverError::LimitExceeded {
                limit: CascadeEvaluationDiagnosticLimit::RetainedBytes,
                configured: 32,
                observed: 48,
            })
        ));

        let actual_capacity_request = growth_request(0, 1, 1, 64, 16, 32);
        assert!(matches!(
            verify_actual_bounded_record_capacity(actual_capacity_request, 0, 3),
            Err(CascadeEvaluationObserverError::LimitExceeded {
                limit: CascadeEvaluationDiagnosticLimit::RetainedBytes,
                configured: 32,
                observed: 48,
            })
        ));

        let near_overflow = usize::MAX / 2 + 1;
        let overflow = plan_bounded_record_growth(growth_request(
            near_overflow,
            near_overflow + 1,
            near_overflow + 1,
            usize::MAX,
            1,
            usize::MAX,
        ))
        .expect_err("geometric overflow is not replaced with exact growth");
        assert_eq!(
            overflow,
            CascadeEvaluationObserverError::RecordCapacityGrowthOverflow {
                storage: "test-records",
                current_capacity: near_overflow,
                required: near_overflow + 1,
            }
        );
        let diagnostic_failure = CascadeEvaluationDiagnosticFailure::from(overflow);
        assert_eq!(
            diagnostic_failure.stable_label(),
            "record-capacity-growth-overflow"
        );
        assert_eq!(
            failure_debug_snapshot(&diagnostic_failure),
            format!(
                "version: 1\ncascade-evaluation-diagnostic\nfailure: kind=record-capacity-growth-overflow storage=test-records current-capacity={near_overflow} required={}\n",
                near_overflow + 1
            )
        );
    }

    #[test]
    fn repeated_record_growth_is_explicitly_amortized() {
        let mut records = Vec::<u64>::new();
        let mut capacity_growths = 0usize;
        for required_len in 1..=1_024 {
            let current_capacity = records.capacity();
            if try_grow_bounded_records(
                &mut records,
                growth_request(
                    current_capacity,
                    required_len,
                    required_len,
                    1_024,
                    std::mem::size_of::<u64>(),
                    1_024 * std::mem::size_of::<u64>(),
                ),
            )
            .expect("bounded record growth succeeds")
            .is_some()
            {
                capacity_growths += 1;
            }
            records.push(0);
        }
        assert!(
            capacity_growths <= 8,
            "minimum-eight doubling needs at most eight growths for 1,024 records, got {capacity_growths}"
        );
    }

    #[test]
    fn begin_element_reserves_candidate_count_and_bounded_winner_upper_hint() {
        let (dom, _) = fixture();
        let index = build_document_selector_dom_with_element_limit(&dom, 64)
            .expect("fixture selector projection builds");
        let element = index.elements().next().expect("fixture has elements");
        let property_count = crate::property_registry().entries().len();
        let mut observer =
            BoundedCascadeObserver::new(CascadeEvaluationDiagnosticLimits::default());
        observer
            .begin_element(element, property_count)
            .expect("candidate and winner upper bounds fit defaults");
        assert!(observer.candidates.capacity() >= property_count);
        assert!(observer.winners.capacity() >= property_count);

        let mut record_limited = BoundedCascadeObserver::new(CascadeEvaluationDiagnosticLimits {
            max_winner_records: 1,
            ..CascadeEvaluationDiagnosticLimits::default()
        });
        record_limited
            .begin_element(element, property_count)
            .expect("winner hint clamps without treating an upper bound as observed winners");
    }

    #[test]
    fn diagnostic_reservation_sites_return_only_diagnostic_failures() {
        let mut observer_records = Vec::<u8>::new();
        assert!(matches!(
            try_grow_bounded_records(
                &mut observer_records,
                growth_request(
                    0,
                    isize::MAX as usize,
                    isize::MAX as usize,
                    usize::MAX,
                    1,
                    usize::MAX,
                ),
            ),
            Err(CascadeEvaluationObserverError::ReservationFailed {
                storage: "test-records",
                ..
            })
        ));

        let mut final_records = Vec::<u8>::new();
        assert_eq!(
            try_reserve_final_records(&mut final_records, usize::MAX, "final-candidate-records"),
            Err(CascadeEvaluationDiagnosticFailure::ReservationFailed {
                storage: "final-candidate-records",
                requested: usize::MAX,
            })
        );

        let mut serialized = String::new();
        assert_eq!(
            try_reserve_serialized_bytes(&mut serialized, usize::MAX),
            Err(CascadeEvaluationDiagnosticFailure::ReservationFailed {
                storage: "serialized-bytes",
                requested: usize::MAX,
            })
        );
    }
}
