use crate::{AggregateRun, ReconciledExternalAdvisoryEvidence, SelectedDomAdvisoryOperation};

use super::{BaselineEvidence, BaselineSealError, SealedBaselineEvidence};

pub fn seal_baseline_without_evaluation<'e, 'run>(
    run: &'run AggregateRun,
    evidence: &'e ReconciledExternalAdvisoryEvidence<'run>,
) -> Result<SealedBaselineEvidence<'e, 'run>, BaselineSealError> {
    if !std::ptr::eq(run, evidence.originating_run()) {
        return Err(BaselineSealError::OriginatingRunMismatch);
    }
    Ok(SealedBaselineEvidence {
        run,
        evidence: BaselineEvidence::NotRequested(evidence),
    })
}

pub fn seal_baseline_from_selected_operation<'e, 'run>(
    operation: &'e SelectedDomAdvisoryOperation<'run>,
) -> Result<SealedBaselineEvidence<'e, 'run>, BaselineSealError> {
    let run = operation.evidence().originating_run();
    Ok(SealedBaselineEvidence {
        run,
        evidence: BaselineEvidence::Selected(operation),
    })
}
