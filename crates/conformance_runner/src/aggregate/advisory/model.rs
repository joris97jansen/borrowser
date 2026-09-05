use super::difference::AdvisoryFirstDifference;
use crate::{
    AggregateRun, AggregateVariantKey, ComparableObservationSurface,
    ReconciledExternalAdvisoryEvidence, ReconciledExternalAttachment,
};
use external_test_provenance::{ExternalCaptureId, Sha256Digest};
use html_test_support::parser_fixture::ComparableDomPreparationError;
use html_test_support::web_observable_dom::WebObservableDomTreeV1;

/// Exactly one existing variant requested for this local operation. This does
/// not restrict the aggregate population or change its execution selection.
#[derive(Debug)]
pub struct SelectedDomOperationRequest {
    pub selected: AggregateVariantKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedDomOperationScope {
    SelectedVariantOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomObservationFailure {
    UnknownVariant,
    UnsupportedSelection,
    NotAttempted,
    Preparation(ComparableDomPreparationError),
    DuplicateHandoff,
}
#[derive(Debug)]
pub(super) struct ProducedObservation {
    pub bytes: WebObservableDomTreeV1,
    pub fixture_sha256: Sha256Digest,
}

/// A sealed ordinary run plus one independent local observation result.
/// Private construction binds observation and selection to that same execution.
pub struct SelectedDomOperationRun {
    pub(super) run: AggregateRun,
    pub(super) selected: AggregateVariantKey,
    pub(super) observation: Result<ProducedObservation, DomObservationFailure>,
}
impl SelectedDomOperationRun {
    pub fn run(&self) -> &AggregateRun {
        &self.run
    }
    pub fn selected(&self) -> &AggregateVariantKey {
        &self.selected
    }
    pub const fn scope(&self) -> SelectedDomOperationScope {
        SelectedDomOperationScope::SelectedVariantOnly
    }
    pub const fn comparable(&self) -> ComparableObservationSurface {
        ComparableObservationSurface::WebObservableDomTreeV1
    }
    pub fn observation(&self) -> Result<&WebObservableDomTreeV1, DomObservationFailure> {
        self.observation
            .as_ref()
            .map(|value| &value.bytes)
            .map_err(|error| *error)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvisoryComparisonFailure {
    Observation(DomObservationFailure),
    UnsupportedCaptureContext,
    SourceIdentityMismatch,
    AlgorithmSourceMismatch,
    ConfigurationSourceMismatch,
    FixtureMismatch,
    IncompatibleSurface,
    InvalidArtifact,
    Invariant,
    Resource,
    Allocation,
}
#[derive(Debug, PartialEq, Eq)]
pub enum AdvisoryVerdict {
    Equivalent,
    Different { evidence: AdvisoryFirstDifference },
}

/// One exact attachment+track in the operation's immutable reconciled evidence.
/// The index never escapes as an independently constructible attachment key.
#[derive(Debug)]
pub struct AdvisoryAttachmentComparison {
    pub(super) attachment_index: usize,
    pub(super) capture_id: ExternalCaptureId,
    pub(super) result: Result<AdvisoryVerdict, AdvisoryComparisonFailure>,
}
impl AdvisoryAttachmentComparison {
    pub fn result(&self) -> &Result<AdvisoryVerdict, AdvisoryComparisonFailure> {
        &self.result
    }
    pub const fn capture_id(&self) -> ExternalCaptureId {
        self.capture_id
    }
}

/// Selected/partial operation, NEVER a complete aggregate advisory population.
/// Out-of-scope attachments remain in evidence, with no fabricated result.
pub struct SelectedDomAdvisoryOperation<'run> {
    pub(super) selected: &'run AggregateVariantKey,
    pub(super) evidence: ReconciledExternalAdvisoryEvidence<'run>,
    pub(super) comparisons: Vec<AdvisoryAttachmentComparison>,
    pub(super) in_scope: usize,
    pub(super) difference_bytes: usize,
}
impl<'run> SelectedDomAdvisoryOperation<'run> {
    pub const fn scope(&self) -> SelectedDomOperationScope {
        SelectedDomOperationScope::SelectedVariantOnly
    }
    pub fn selected(&self) -> &AggregateVariantKey {
        self.selected
    }
    pub const fn comparable(&self) -> ComparableObservationSurface {
        ComparableObservationSurface::WebObservableDomTreeV1
    }
    pub fn evidence(&self) -> &ReconciledExternalAdvisoryEvidence<'run> {
        &self.evidence
    }
    pub fn total_attachment_count(&self) -> usize {
        self.evidence.attachments().len()
    }
    pub fn in_scope_attachment_count(&self) -> usize {
        self.in_scope
    }
    pub fn outside_scope_attachment_count(&self) -> usize {
        self.total_attachment_count() - self.in_scope
    }
    pub fn evaluated(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &ReconciledExternalAttachment<'run>,
            &AdvisoryAttachmentComparison,
        ),
    > {
        self.comparisons.iter().map(|comparison| {
            (
                &self.evidence.attachments()[comparison.attachment_index],
                comparison,
            )
        })
    }
    pub fn outside_scope(&self) -> impl Iterator<Item = &ReconciledExternalAttachment<'run>> {
        self.evidence.attachments().iter().filter(|attachment| {
            &attachment.aggregate_variant().key != self.selected
                || attachment.comparable() != self.comparable()
        })
    }
    pub fn retained_difference_bytes(&self) -> usize {
        self.difference_bytes
    }
}
