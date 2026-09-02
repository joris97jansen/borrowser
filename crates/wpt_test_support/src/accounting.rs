use conformance_test_support::{
    AccountedDerivedAdaptation, AccountedExternalSource, AssessmentEvidence, CapabilityRequirement,
    DerivedAdaptationDecision, GenericAssertionRequirement, GenericHarnessRequirement,
    SelectionPolicyAssessment, SelectionPolicyState, SourceRequirementsBuilder,
    ValidatedExternalAssessmentProfile, account_malformed_external_source,
    assess_derived_adaptation, assess_external_source,
};

use crate::selection_policy::{evaluate_derived_filter, evaluate_wpt_filter};
use crate::{
    AccountedWptRecord, InterpretedWptRecord, ValidatedWptSelectionPolicy, ValidatedWptSourceSet,
    WptFilterAssessment, WptFilterDimension, WptFilterFact, WptFilterOutcome,
    WptInterpretationStatus, WptSelectionPolicyError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WptAccountingError {
    InvalidEvidence,
    InvalidLineage,
    InvalidDerivedRequirements,
    SelectionPolicy(WptSelectionPolicyError),
    ConservationFailure,
}

pub fn account_wpt_source_set(
    set: &ValidatedWptSourceSet,
    selection_policy: &ValidatedWptSelectionPolicy,
    assessment_profile: &ValidatedExternalAssessmentProfile,
    interpreted: Vec<InterpretedWptRecord>,
) -> Result<Vec<AccountedWptRecord>, WptAccountingError> {
    if interpreted.len() != set.records().len() {
        return Err(WptAccountingError::ConservationFailure);
    }
    let mut output = Vec::new();
    for record in interpreted {
        if record.interpretation_status() == WptInterpretationStatus::MalformedOrUnimportable {
            let evidence = record
                .interpretation_evidence()
                .first()
                .ok_or(WptAccountingError::InvalidEvidence)?;
            let generic = account_malformed_external_source(
                record.source_record_id().clone(),
                record.generic_requirements().clone(),
                AssessmentEvidence::parse(evidence.value())
                    .map_err(|_| WptAccountingError::InvalidEvidence)?,
            );
            output.push(AccountedWptRecord::new(
                record,
                generic,
                unresolved_filter("malformed source form cannot be filtered"),
                Vec::new(),
            ));
            continue;
        }
        if record.interpretation_status() == WptInterpretationStatus::NotYetClassifiable {
            let policy = SelectionPolicyAssessment::new(
                SelectionPolicyState::NotYetEstablished,
                vec![static_evidence(
                    "The source form is not yet honestly classifiable by the bounded WPT interpreter.",
                )?],
            );
            let generic = assess_external_source(
                record.source_record_id().clone(),
                record.generic_requirements(),
                assessment_profile.profiles(),
                policy,
            );
            output.push(AccountedWptRecord::new(
                record,
                generic,
                unresolved_filter("source form is not yet classifiable"),
                Vec::new(),
            ));
            continue;
        }
        let filter = evaluate_wpt_filter(selection_policy, &record)
            .map_err(WptAccountingError::SelectionPolicy)?;
        let direct_policy = generic_policy_assessment(&filter)?;
        let generic = assess_external_source(
            record.source_record_id().clone(),
            record.generic_requirements(),
            assessment_profile.profiles(),
            direct_policy,
        );
        let mut derived_adaptations = Vec::new();
        for policy in selection_policy.derived().filter(|policy| {
            record.interpretation_status() == WptInterpretationStatus::Complete
                && policy.source_record() == record.source_record_id()
        }) {
            let lineage = set
                .lineages()
                .iter()
                .find(|lineage| lineage.id() == policy.lineage_id())
                .ok_or(WptAccountingError::InvalidLineage)?;
            let mut builder = SourceRequirementsBuilder::new();
            builder
                .requirement_tag(policy.capability_kind().requirement_tag())
                .capability(
                    CapabilityRequirement::new(
                        policy.capability_kind(),
                        Some(policy.capability_feature().clone()),
                    )
                    .map_err(|_| WptAccountingError::InvalidDerivedRequirements)?,
                )
                .harness(GenericHarnessRequirement::SubsystemAdapter(
                    policy.harness_adapter().clone(),
                ))
                .assertion(GenericAssertionRequirement::SemanticObservation(
                    policy.representation().clone(),
                ));
            for resource in record.generic_requirements().resources() {
                builder.resource(resource.clone());
            }
            let requirements = builder
                .build()
                .map_err(|_| WptAccountingError::InvalidDerivedRequirements)?;
            let policy_assessment = evaluate_derived_filter(selection_policy, policy, &record)
                .map_err(WptAccountingError::SelectionPolicy)?;
            derived_adaptations.push(assess_derived_adaptation(
                lineage.id().clone(),
                &requirements,
                assessment_profile.profiles(),
                policy_assessment,
            ));
        }
        output.push(AccountedWptRecord::new(
            record,
            generic,
            filter,
            derived_adaptations,
        ));
    }
    output.sort_by(|a, b| {
        a.interpreted()
            .source_record_id()
            .cmp(b.interpreted().source_record_id())
    });
    if output.len() != set.records().len()
        || output
            .iter()
            .map(|record| record.interpreted().source_record_id())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != set.records().len()
    {
        return Err(WptAccountingError::ConservationFailure);
    }
    Ok(output)
}

fn generic_policy_assessment(
    filter: &WptFilterAssessment,
) -> Result<SelectionPolicyAssessment, WptAccountingError> {
    let state = if filter
        .facts()
        .iter()
        .any(|fact| fact.outcome() == WptFilterOutcome::Excluded)
    {
        SelectionPolicyState::Excluded
    } else if filter
        .facts()
        .iter()
        .any(|fact| fact.outcome() == WptFilterOutcome::NotYetEstablished)
    {
        SelectionPolicyState::NotYetEstablished
    } else {
        SelectionPolicyState::Included
    };
    let evidence = filter
        .facts()
        .iter()
        .map(|fact| AssessmentEvidence::parse(fact.evidence()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| WptAccountingError::InvalidEvidence)?;
    Ok(SelectionPolicyAssessment::new(state, evidence))
}

fn unresolved_filter(evidence: &str) -> WptFilterAssessment {
    WptFilterAssessment::new(vec![WptFilterFact::new(
        WptFilterDimension::TestType,
        WptFilterOutcome::NotYetEstablished,
        evidence.to_owned(),
    )])
}

fn static_evidence(value: &str) -> Result<AssessmentEvidence, WptAccountingError> {
    AssessmentEvidence::parse(value).map_err(|_| WptAccountingError::InvalidEvidence)
}

pub fn selected_derived_adaptations(
    records: &[AccountedWptRecord],
) -> impl Iterator<Item = &AccountedDerivedAdaptation> {
    records
        .iter()
        .flat_map(AccountedWptRecord::derived_adaptations)
        .filter(|adaptation| adaptation.decision() == &DerivedAdaptationDecision::Selected)
}

pub fn directly_selected_records(
    records: &[AccountedWptRecord],
) -> impl Iterator<Item = &AccountedExternalSource> {
    records
        .iter()
        .map(AccountedWptRecord::generic_accounting)
        .filter(|record| {
            matches!(
                record.decision(),
                conformance_test_support::SourceSelectionDecision::SelectedForDirectExecution
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InterpretationEvidence;

    #[test]
    fn integrity_verified_unimportable_source_is_an_accounted_decision() {
        let record = InterpretedWptRecord::malformed(
            conformance_test_support::SourceRecordId::parse("malformed-proof").unwrap(),
            InterpretationEvidence::new(
                "malformed-source-form",
                "integrity-verified source cannot be imported by the bounded interpreter",
            ),
        );
        let evidence =
            AssessmentEvidence::parse(record.interpretation_evidence()[0].value()).unwrap();
        let accounted = account_malformed_external_source(
            record.source_record_id().clone(),
            record.generic_requirements().clone(),
            evidence,
        );
        assert!(matches!(
            accounted.decision(),
            conformance_test_support::SourceSelectionDecision::MalformedSourceForm { .. }
        ));
    }
}
