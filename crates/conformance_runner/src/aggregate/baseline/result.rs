use super::super::binary_wire::{Reader, Writer};
use crate::{AdvisoryComparisonFailure, AdvisoryVerdict, DomObservationFailure};

use super::BaselineError;

pub(crate) const MAX_ADVISORY_RESULT_BYTES_V1: usize = 16_409;
pub(crate) const MAX_ADVISORY_RESULT_SLOT_PAYLOAD_BYTES_V1: usize = 16_418;

pub(super) fn encode_result(
    w: &mut Writer,
    result: &Result<AdvisoryVerdict, AdvisoryComparisonFailure>,
) -> Result<(), BaselineError> {
    match result {
        Ok(AdvisoryVerdict::Equivalent) => w.string("equivalent")?,
        Ok(AdvisoryVerdict::Different { evidence }) => {
            w.string("different")?;
            w.bytes(evidence.serialized_bytes())?
        }
        Err(e) => {
            w.string("failure")?;
            encode_failure(w, *e)?
        }
    }
    Ok(())
}
fn encode_failure(w: &mut Writer, e: AdvisoryComparisonFailure) -> Result<(), BaselineError> {
    match e {
        AdvisoryComparisonFailure::Observation(e) => {
            w.string("observation")?;
            encode_observation(w, e)?
        }
        AdvisoryComparisonFailure::UnsupportedCaptureContext => {
            w.string("unsupported-capture-context")?
        }
        AdvisoryComparisonFailure::SourceIdentityMismatch => {
            w.string("source-identity-mismatch")?
        }
        AdvisoryComparisonFailure::AlgorithmSourceMismatch => {
            w.string("algorithm-source-mismatch")?
        }
        AdvisoryComparisonFailure::ConfigurationSourceMismatch => {
            w.string("configuration-source-mismatch")?
        }
        AdvisoryComparisonFailure::FixtureMismatch => w.string("fixture-mismatch")?,
        AdvisoryComparisonFailure::IncompatibleSurface => w.string("incompatible-surface")?,
        AdvisoryComparisonFailure::InvalidArtifact => w.string("invalid-artifact")?,
        AdvisoryComparisonFailure::Invariant => w.string("invariant")?,
        AdvisoryComparisonFailure::Resource => w.string("resource")?,
        AdvisoryComparisonFailure::Allocation => w.string("allocation")?,
    }
    Ok(())
}
fn encode_observation(w: &mut Writer, e: DomObservationFailure) -> Result<(), BaselineError> {
    match e {
        DomObservationFailure::UnknownVariant => w.string("unknown-variant")?,
        DomObservationFailure::UnsupportedSelection => w.string("unsupported-selection")?,
        DomObservationFailure::NotAttempted => w.string("not-attempted")?,
        DomObservationFailure::DuplicateHandoff => w.string("duplicate-handoff")?,
        DomObservationFailure::Preparation(e) => {
            w.string("preparation")?;
            use html_test_support::parser_fixture::ComparableDomPreparationError as P;
            match e {
                P::Unavailable => w.string("unavailable")?,
                P::ExecutionFailure => w.string("execution-failure")?,
                P::Resource => w.string("resource")?,
                P::Incomplete => w.string("incomplete")?,
                P::Invariant => w.string("invariant")?,
                P::UnsupportedContext => w.string("unsupported-context")?,
                P::Serialization(e) => {
                    w.string("serialization")?;
                    use html_test_support::web_observable_dom::WebObservableDomSerializationError as S;
                    w.string(match e {
                        S::InvalidStructure => "invalid-structure",
                        S::InvalidAttribute => "invalid-attribute",
                        S::DuplicateAttribute => "duplicate-attribute",
                        S::TooLarge => "too-large",
                        S::Overflow => "overflow",
                        S::Allocation => "allocation",
                    })?
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate_result(bytes: &[u8]) -> Result<(), BaselineError> {
    if bytes.len() > MAX_ADVISORY_RESULT_BYTES_V1 {
        return Err(BaselineError::InvalidEvaluation);
    }
    let mut r = Reader::new(bytes);
    match r.string()? {
        "equivalent" => {}
        "different" => {
            let evidence = r.bytes()?;
            super::super::advisory::validate_first_difference_v1(evidence)
                .map_err(|_| BaselineError::InvalidEvidence)?
        }
        "failure" => validate_failure(&mut r)?,
        _ => return Err(BaselineError::InvalidEvaluation),
    }
    r.finish()?;
    Ok(())
}
fn validate_failure(r: &mut Reader<'_>) -> Result<(), BaselineError> {
    match r.string()? {
        "observation" => match r.string()? {
            "unknown-variant" | "unsupported-selection" | "not-attempted" | "duplicate-handoff" => {
            }
            "preparation" => match r.string()? {
                "unavailable"
                | "execution-failure"
                | "resource"
                | "incomplete"
                | "invariant"
                | "unsupported-context" => {}
                "serialization" => match r.string()? {
                    "invalid-structure"
                    | "invalid-attribute"
                    | "duplicate-attribute"
                    | "too-large"
                    | "overflow"
                    | "allocation" => {}
                    _ => return Err(BaselineError::InvalidEvaluation),
                },
                _ => return Err(BaselineError::InvalidEvaluation),
            },
            _ => return Err(BaselineError::InvalidEvaluation),
        },
        "unsupported-capture-context"
        | "source-identity-mismatch"
        | "algorithm-source-mismatch"
        | "configuration-source-mismatch"
        | "fixture-mismatch"
        | "incompatible-surface"
        | "invalid-artifact"
        | "invariant"
        | "resource"
        | "allocation" => {}
        _ => return Err(BaselineError::InvalidEvaluation),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use html_test_support::parser_fixture::ComparableDomPreparationError as Preparation;
    use html_test_support::web_observable_dom::WebObservableDomSerializationError as Serialization;

    use super::*;

    fn encoded(result: Result<AdvisoryVerdict, AdvisoryComparisonFailure>) -> Vec<u8> {
        let mut writer = Writer::new(16_418).unwrap();
        encode_result(&mut writer, &result).unwrap();
        writer.finish()
    }

    #[test]
    fn equivalent_has_independent_exact_bytes() {
        assert_eq!(
            encoded(Ok(AdvisoryVerdict::Equivalent)),
            [10_u64.to_be_bytes().as_slice(), b"equivalent"].concat()
        );
    }

    #[test]
    fn every_frozen_failure_label_round_trips_through_the_historical_validator() {
        let mut failures = vec![
            AdvisoryComparisonFailure::UnsupportedCaptureContext,
            AdvisoryComparisonFailure::SourceIdentityMismatch,
            AdvisoryComparisonFailure::AlgorithmSourceMismatch,
            AdvisoryComparisonFailure::ConfigurationSourceMismatch,
            AdvisoryComparisonFailure::FixtureMismatch,
            AdvisoryComparisonFailure::IncompatibleSurface,
            AdvisoryComparisonFailure::InvalidArtifact,
            AdvisoryComparisonFailure::Invariant,
            AdvisoryComparisonFailure::Resource,
            AdvisoryComparisonFailure::Allocation,
            AdvisoryComparisonFailure::Observation(DomObservationFailure::UnknownVariant),
            AdvisoryComparisonFailure::Observation(DomObservationFailure::UnsupportedSelection),
            AdvisoryComparisonFailure::Observation(DomObservationFailure::NotAttempted),
            AdvisoryComparisonFailure::Observation(DomObservationFailure::DuplicateHandoff),
        ];
        for preparation in [
            Preparation::Unavailable,
            Preparation::ExecutionFailure,
            Preparation::Resource,
            Preparation::Incomplete,
            Preparation::Invariant,
            Preparation::UnsupportedContext,
        ] {
            failures.push(AdvisoryComparisonFailure::Observation(
                DomObservationFailure::Preparation(preparation),
            ));
        }
        for serialization in [
            Serialization::InvalidStructure,
            Serialization::InvalidAttribute,
            Serialization::DuplicateAttribute,
            Serialization::TooLarge,
            Serialization::Overflow,
            Serialization::Allocation,
        ] {
            failures.push(AdvisoryComparisonFailure::Observation(
                DomObservationFailure::Preparation(Preparation::Serialization(serialization)),
            ));
        }
        for failure in failures {
            validate_result(&encoded(Err(failure))).unwrap();
        }
    }

    #[test]
    fn different_verdict_requires_a_valid_authoritative_first_difference_artifact() {
        let mut writer = Writer::new(64).unwrap();
        writer.string("different").unwrap();
        writer.bytes(b"not-first-difference\n").unwrap();
        assert!(matches!(
            validate_result(&writer.finish()),
            Err(BaselineError::InvalidEvidence)
        ));
    }
}
