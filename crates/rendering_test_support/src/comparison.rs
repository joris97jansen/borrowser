use crate::{CanonicalRenderingCapture, RenderingObservationProfile, RenderingProfileObservation};

pub const REFERENCE_DIFFERENCE_EXCERPT_UTF8_BYTES_V1: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingOracleVerdict {
    Equivalent,
    Different,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingOracleComparison {
    Equivalent,
    Different {
        first_difference: RenderingDifferenceLocator,
    },
}

impl RenderingOracleComparison {
    pub const fn verdict(self) -> RenderingOracleVerdict {
        match self {
            Self::Equivalent => RenderingOracleVerdict::Equivalent,
            Self::Different { .. } => RenderingOracleVerdict::Different,
        }
    }

    pub const fn first_difference(self) -> Option<RenderingDifferenceLocator> {
        match self {
            Self::Equivalent => None,
            Self::Different { first_difference } => Some(first_difference),
        }
    }
}

/// An allocation-free locator into two already-validated captures.
///
/// This is oracle output. It deliberately contains no report excerpt bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderingDifferenceLocator {
    observation_index: usize,
    profile: RenderingObservationProfile,
    zero_based_line: usize,
}

impl RenderingDifferenceLocator {
    pub const fn profile(self) -> RenderingObservationProfile {
        self.profile
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderingFirstDifference {
    pub profile: RenderingObservationProfile,
    pub one_based_line: u64,
    pub test_observation_bytes: u64,
    pub reference_observation_bytes: u64,
    pub test_line: RenderingDifferenceLine,
    pub reference_line: RenderingDifferenceLine,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderingDifferenceLine {
    Missing,
    Present {
        original_bytes: u64,
        excerpt: String,
        truncated: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingComparisonFailure {
    VariantIdentityMismatch,
    ObservationSetMismatch,
    ObservationOrderMismatch,
}

impl RenderingComparisonFailure {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::VariantIdentityMismatch => "variant-identity-mismatch",
            Self::ObservationSetMismatch => "observation-set-mismatch",
            Self::ObservationOrderMismatch => "observation-order-mismatch",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingDifferenceEvidenceFailure {
    LocatorDoesNotMatchCapture,
    ByteLengthDoesNotFit,
    LineNumberDoesNotFit,
    ExcerptAllocation,
}

impl RenderingDifferenceEvidenceFailure {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::LocatorDoesNotMatchCapture => "locator-does-not-match-capture",
            Self::ByteLengthDoesNotFit => "byte-length-does-not-fit",
            Self::LineNumberDoesNotFit => "line-number-does-not-fit",
            Self::ExcerptAllocation => "excerpt-allocation",
        }
    }
}

impl std::fmt::Display for RenderingDifferenceEvidenceFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.stable_label())
    }
}

impl std::error::Error for RenderingDifferenceEvidenceFailure {}

/// Compares complete owner-produced bytes and establishes only oracle truth.
///
/// No owned diagnostic excerpt is allocated here. The first-difference locator
/// is sufficient to materialize bounded evidence afterwards.
pub fn compare_canonical_rendering_captures(
    test: &CanonicalRenderingCapture,
    reference: &CanonicalRenderingCapture,
) -> Result<RenderingOracleComparison, RenderingComparisonFailure> {
    if test.variant != reference.variant {
        return Err(RenderingComparisonFailure::VariantIdentityMismatch);
    }
    if test.observations.len() != reference.observations.len() {
        return Err(RenderingComparisonFailure::ObservationSetMismatch);
    }
    if !profiles_are_in_canonical_order(&test.observations)
        || !profiles_are_in_canonical_order(&reference.observations)
    {
        return Err(RenderingComparisonFailure::ObservationOrderMismatch);
    }
    for (observation_index, (test_observation, reference_observation)) in test
        .observations
        .iter()
        .zip(&reference.observations)
        .enumerate()
    {
        if test_observation.profile != reference_observation.profile {
            return Err(RenderingComparisonFailure::ObservationOrderMismatch);
        }
        if test_observation.bytes != reference_observation.bytes {
            return Ok(RenderingOracleComparison::Different {
                first_difference: first_difference_locator(
                    observation_index,
                    test_observation,
                    reference_observation,
                ),
            });
        }
    }
    Ok(RenderingOracleComparison::Equivalent)
}

pub fn materialize_rendering_first_difference(
    test: &CanonicalRenderingCapture,
    reference: &CanonicalRenderingCapture,
    locator: RenderingDifferenceLocator,
) -> Result<RenderingFirstDifference, RenderingDifferenceEvidenceFailure> {
    materialize_rendering_first_difference_with(test, reference, locator, difference_line)
}

fn profiles_are_in_canonical_order(observations: &[RenderingProfileObservation]) -> bool {
    observations
        .windows(2)
        .all(|pair| pair[0].profile < pair[1].profile)
}

fn first_difference_locator(
    observation_index: usize,
    test: &RenderingProfileObservation,
    reference: &RenderingProfileObservation,
) -> RenderingDifferenceLocator {
    let first_differing_byte = test
        .bytes
        .bytes()
        .zip(reference.bytes.bytes())
        .position(|(test_byte, reference_byte)| test_byte != reference_byte)
        .unwrap_or_else(|| test.bytes.len().min(reference.bytes.len()));
    let zero_based_line = test.bytes.as_bytes()[..first_differing_byte]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count();
    RenderingDifferenceLocator {
        observation_index,
        profile: test.profile,
        zero_based_line,
    }
}

fn materialize_rendering_first_difference_with<MaterializeLine>(
    test: &CanonicalRenderingCapture,
    reference: &CanonicalRenderingCapture,
    locator: RenderingDifferenceLocator,
    mut materialize_line: MaterializeLine,
) -> Result<RenderingFirstDifference, RenderingDifferenceEvidenceFailure>
where
    MaterializeLine:
        FnMut(Option<&str>) -> Result<RenderingDifferenceLine, RenderingDifferenceEvidenceFailure>,
{
    if test.variant != reference.variant {
        return Err(RenderingDifferenceEvidenceFailure::LocatorDoesNotMatchCapture);
    }
    let test_observation = test
        .observations
        .get(locator.observation_index)
        .ok_or(RenderingDifferenceEvidenceFailure::LocatorDoesNotMatchCapture)?;
    let reference_observation = reference
        .observations
        .get(locator.observation_index)
        .ok_or(RenderingDifferenceEvidenceFailure::LocatorDoesNotMatchCapture)?;
    if test_observation.profile != locator.profile
        || reference_observation.profile != locator.profile
    {
        return Err(RenderingDifferenceEvidenceFailure::LocatorDoesNotMatchCapture);
    }
    let test_line = test_observation
        .bytes
        .split_inclusive('\n')
        .nth(locator.zero_based_line);
    let reference_line = reference_observation
        .bytes
        .split_inclusive('\n')
        .nth(locator.zero_based_line);
    if test_line == reference_line {
        return Err(RenderingDifferenceEvidenceFailure::LocatorDoesNotMatchCapture);
    }
    let one_based_line = locator
        .zero_based_line
        .checked_add(1)
        .ok_or(RenderingDifferenceEvidenceFailure::LineNumberDoesNotFit)?;
    Ok(RenderingFirstDifference {
        profile: locator.profile,
        one_based_line: u64::try_from(one_based_line)
            .map_err(|_| RenderingDifferenceEvidenceFailure::LineNumberDoesNotFit)?,
        test_observation_bytes: u64::try_from(test_observation.bytes.len())
            .map_err(|_| RenderingDifferenceEvidenceFailure::ByteLengthDoesNotFit)?,
        reference_observation_bytes: u64::try_from(reference_observation.bytes.len())
            .map_err(|_| RenderingDifferenceEvidenceFailure::ByteLengthDoesNotFit)?,
        test_line: materialize_line(test_line)?,
        reference_line: materialize_line(reference_line)?,
    })
}

fn difference_line(
    line: Option<&str>,
) -> Result<RenderingDifferenceLine, RenderingDifferenceEvidenceFailure> {
    let Some(line) = line else {
        return Ok(RenderingDifferenceLine::Missing);
    };
    let original_bytes = u64::try_from(line.len())
        .map_err(|_| RenderingDifferenceEvidenceFailure::ByteLengthDoesNotFit)?;
    let mut end = line.len().min(REFERENCE_DIFFERENCE_EXCERPT_UTF8_BYTES_V1);
    while !line.is_char_boundary(end) {
        end = end
            .checked_sub(1)
            .ok_or(RenderingDifferenceEvidenceFailure::ByteLengthDoesNotFit)?;
    }
    let mut excerpt = String::new();
    excerpt
        .try_reserve(end)
        .map_err(|_| RenderingDifferenceEvidenceFailure::ExcerptAllocation)?;
    excerpt.push_str(&line[..end]);
    Ok(RenderingDifferenceLine::Present {
        original_bytes,
        excerpt,
        truncated: end < line.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AvailableWidthCssPx, PaintObservationProfile, RenderingExecutionVariantId,
        RenderingObservationProfile, SyntheticTextMetricsV1,
    };

    fn capture(profile: RenderingObservationProfile, bytes: String) -> CanonicalRenderingCapture {
        CanonicalRenderingCapture {
            variant: RenderingExecutionVariantId {
                environment: SyntheticTextMetricsV1::SyntheticTextMetricsV1,
                available_width_css_px: AvailableWidthCssPx::try_new(320).unwrap(),
            },
            observations: vec![RenderingProfileObservation { profile, bytes }],
        }
    }

    fn materialize(
        test: &CanonicalRenderingCapture,
        reference: &CanonicalRenderingCapture,
    ) -> RenderingFirstDifference {
        let comparison = compare_canonical_rendering_captures(test, reference).unwrap();
        assert_eq!(comparison.verdict(), RenderingOracleVerdict::Different);
        materialize_rendering_first_difference(
            test,
            reference,
            comparison.first_difference().unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn exact_owner_bytes_establish_different_before_evidence_materialization() {
        let profile = RenderingObservationProfile::Paint(PaintObservationProfile::PaintOperations);
        let test = capture(profile, "same\ntest\n".to_owned());
        let reference = capture(profile, "same\nreference\n".to_owned());
        let comparison = compare_canonical_rendering_captures(&test, &reference).unwrap();
        assert_eq!(comparison.verdict(), RenderingOracleVerdict::Different);

        let failure = materialize_rendering_first_difference_with(
            &test,
            &reference,
            comparison.first_difference().unwrap(),
            |_| Err(RenderingDifferenceEvidenceFailure::ExcerptAllocation),
        );
        assert_eq!(
            failure,
            Err(RenderingDifferenceEvidenceFailure::ExcerptAllocation)
        );
        assert_eq!(comparison.verdict(), RenderingOracleVerdict::Different);
    }

    #[test]
    fn exact_owner_bytes_decide_equivalence_and_first_difference() {
        let profile = RenderingObservationProfile::Paint(PaintObservationProfile::PaintOperations);
        let test = capture(profile, "same\ntest\n".to_owned());
        let reference = capture(profile, "same\nreference\n".to_owned());
        let difference = materialize(&test, &reference);
        assert_eq!(difference.one_based_line, 2);
        assert!(matches!(
            difference.test_line,
            RenderingDifferenceLine::Present { ref excerpt, truncated: false, .. }
                if excerpt == "test\n"
        ));
    }

    #[test]
    fn missing_lines_are_explicit() {
        let profile = RenderingObservationProfile::Paint(PaintObservationProfile::PaintOperations);
        let difference = materialize(
            &capture(profile, "one\ntwo\n".to_owned()),
            &capture(profile, "one\n".to_owned()),
        );
        assert_eq!(difference.one_based_line, 2);
        assert!(matches!(
            difference.reference_line,
            RenderingDifferenceLine::Missing
        ));
    }

    #[test]
    fn excerpts_truncate_at_a_stable_utf8_boundary() {
        let profile = RenderingObservationProfile::Paint(PaintObservationProfile::PaintOperations);
        let line = format!("{}é", "a".repeat(1023));
        let first = materialize(
            &capture(profile, line.clone()),
            &capture(profile, "different".to_owned()),
        );
        let second = materialize(
            &capture(profile, line),
            &capture(profile, "different".to_owned()),
        );
        assert_eq!(first, second);
        assert!(matches!(
            first.test_line,
            RenderingDifferenceLine::Present {
                original_bytes: 1025,
                ref excerpt,
                truncated: true,
            } if excerpt.len() == 1023
        ));
    }

    #[test]
    fn line_terminator_only_differences_remain_visible_in_evidence() {
        let profile = RenderingObservationProfile::Paint(PaintObservationProfile::PaintOperations);
        let difference = materialize(
            &capture(profile, "same".to_owned()),
            &capture(profile, "same\n".to_owned()),
        );
        assert!(matches!(
            difference.test_line,
            RenderingDifferenceLine::Present {
                original_bytes: 4,
                ref excerpt,
                truncated: false,
            } if excerpt == "same"
        ));
        assert!(matches!(
            difference.reference_line,
            RenderingDifferenceLine::Present {
                original_bytes: 5,
                ref excerpt,
                truncated: false,
            } if excerpt == "same\n"
        ));
    }

    #[test]
    fn comparator_invariants_remain_separately_typed() {
        let operations =
            RenderingObservationProfile::Paint(PaintObservationProfile::PaintOperations);
        let order = RenderingObservationProfile::Paint(PaintObservationProfile::PaintOrder);
        let mut test = capture(operations, "operations".to_owned());
        test.observations.push(RenderingProfileObservation {
            profile: order,
            bytes: "order".to_owned(),
        });
        let reference = test.clone();
        assert_eq!(
            compare_canonical_rendering_captures(&test, &reference),
            Err(RenderingComparisonFailure::ObservationOrderMismatch)
        );
    }

    #[test]
    fn first_difference_uses_canonical_profile_order() {
        let order = RenderingObservationProfile::Paint(PaintObservationProfile::PaintOrder);
        let operations =
            RenderingObservationProfile::Paint(PaintObservationProfile::PaintOperations);
        let variant = RenderingExecutionVariantId {
            environment: SyntheticTextMetricsV1::SyntheticTextMetricsV1,
            available_width_css_px: AvailableWidthCssPx::try_new(320).unwrap(),
        };
        let test = CanonicalRenderingCapture {
            variant,
            observations: vec![
                RenderingProfileObservation {
                    profile: order,
                    bytes: "test order".to_owned(),
                },
                RenderingProfileObservation {
                    profile: operations,
                    bytes: "test operations".to_owned(),
                },
            ],
        };
        let reference = CanonicalRenderingCapture {
            variant,
            observations: vec![
                RenderingProfileObservation {
                    profile: order,
                    bytes: "reference order".to_owned(),
                },
                RenderingProfileObservation {
                    profile: operations,
                    bytes: "reference operations".to_owned(),
                },
            ],
        };
        assert_eq!(
            compare_canonical_rendering_captures(&test, &reference)
                .unwrap()
                .first_difference()
                .unwrap()
                .profile(),
            order
        );
    }
}
