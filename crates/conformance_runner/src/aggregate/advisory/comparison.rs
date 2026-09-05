use super::{
    difference::{DifferenceBudget, first_difference},
    model::*,
    sources::{CaptureSourceError, VerifiedCaptureSourcesV1},
};
use crate::{
    ExternalRegistryDiagnostic, ReconciledExternalAdvisoryEvidence,
    load_repository_external_advisory_evidence,
};
use external_test_provenance::ValidatedExternalCaptureV1;
use std::path::Path;

#[derive(Debug)]
pub enum SelectedDomOperationError {
    Registry(ExternalRegistryDiagnostic),
    Sources(CaptureSourceError),
    Allocation,
    Invariant,
}
impl SelectedDomOperationRun {
    /// Prepare one explicit local operation AFTER sealing ordinary aggregate
    /// truth. No external input is consulted during ordinary execution.
    #[allow(
        clippy::result_large_err,
        reason = "Retain the complete AG9b diagnostic without allocating on the error path"
    )]
    pub fn compare_external(
        &self,
        root: &Path,
    ) -> Result<SelectedDomAdvisoryOperation<'_>, SelectedDomOperationError> {
        let evidence = load_repository_external_advisory_evidence(root, &self.run)
            .map_err(SelectedDomOperationError::Registry)?;
        let sources =
            VerifiedCaptureSourcesV1::load(root).map_err(SelectedDomOperationError::Sources)?;
        compare_selected(self, evidence, &sources, &mut |_| {
            Err(AdvisoryComparisonFailure::UnsupportedCaptureContext)
        })
    }
}

// Admission is private. Tests can exercise independent synthetic captures
// without providing a public escape hatch for unproven real-browser contexts.
#[allow(
    clippy::result_large_err,
    reason = "Retain the complete AG9b diagnostic without allocating on the error path"
)]
fn compare_selected<'a>(
    operation: &'a SelectedDomOperationRun,
    evidence: ReconciledExternalAdvisoryEvidence<'a>,
    sources: &VerifiedCaptureSourcesV1,
    admit_context: &mut impl FnMut(&ValidatedExternalCaptureV1) -> Result<(), AdvisoryComparisonFailure>,
) -> Result<SelectedDomAdvisoryOperation<'a>, SelectedDomOperationError> {
    let in_scope = evidence
        .attachments()
        .iter()
        .filter(|a| {
            a.aggregate_variant().key == operation.selected
                && a.comparable() == operation.comparable()
        })
        .count();
    let mut comparisons = Vec::new();
    comparisons
        .try_reserve(in_scope)
        .map_err(|_| SelectedDomOperationError::Allocation)?;
    let mut budget = DifferenceBudget::default();
    for (index, attachment) in evidence.attachments().iter().enumerate() {
        if attachment.aggregate_variant().key != operation.selected
            || attachment.comparable() != operation.comparable()
        {
            continue;
        }
        let capture = evidence
            .captures()
            .iter()
            .find(|c| c.capture().id() == attachment.capture_id())
            .ok_or(SelectedDomOperationError::Invariant)?
            .capture();
        let result = compare_attachment(operation, capture, sources, admit_context, &mut budget);
        comparisons.push(AdvisoryAttachmentComparison {
            attachment_index: index,
            capture_id: capture.id(),
            result,
        });
    }
    Ok(SelectedDomAdvisoryOperation {
        selected: &operation.selected,
        evidence,
        comparisons,
        in_scope,
        difference_bytes: budget.bytes,
    })
}
fn compare_attachment(
    operation: &SelectedDomOperationRun,
    capture: &ValidatedExternalCaptureV1,
    sources: &VerifiedCaptureSourcesV1,
    admit_context: &mut impl FnMut(&ValidatedExternalCaptureV1) -> Result<(), AdvisoryComparisonFailure>,
    budget: &mut DifferenceBudget,
) -> Result<AdvisoryVerdict, AdvisoryComparisonFailure> {
    let observation = operation
        .observation
        .as_ref()
        .map_err(|error| AdvisoryComparisonFailure::Observation(*error))?;
    match capture.provenance().artifact_format() {
        external_test_provenance::ExternalArtifactFormatV1::WebObservableDomTreeV1 => {}
    }
    sources.verify(capture.provenance())?;
    if capture.provenance().fixture_content_sha256() != observation.fixture_sha256 {
        return Err(AdvisoryComparisonFailure::FixtureMismatch);
    }
    admit_context(capture)?;
    compare_bytes(
        observation.bytes.bytes(),
        capture.artifact().bytes(),
        budget,
    )
}
fn compare_bytes(
    left: &[u8],
    right: &[u8],
    budget: &mut DifferenceBudget,
) -> Result<AdvisoryVerdict, AdvisoryComparisonFailure> {
    // Both are opaque validated V1 artifacts at the caller boundary. Never
    // reopen their paths or treat debug/report bytes as a comparable surface.
    if left == right {
        return Ok(AdvisoryVerdict::Equivalent);
    }
    let evidence = first_difference(left, right)?;
    budget.retain(evidence.retained_bytes()?)?;
    Ok(AdvisoryVerdict::Different { evidence })
}

#[cfg(test)]
mod tests;
