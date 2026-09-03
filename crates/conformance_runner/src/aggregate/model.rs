use std::cmp::Ordering;

use conformance_test_support::{
    InventoryScope, LanePolicyScope, ObservationSurface, ReferenceKind, ReferenceRelation,
    SubsystemOwner, TestId, ValidatedFixture,
};
use rendering_test_support::RenderingExecutionVariantId;

use crate::{
    AgCaseState, CssCaseResult, DerivedPolicyResult, ExecutionVariantId, NormalizedCaseResult,
    RenderingVariantResult, SingletonExecutionVariant,
};

use super::accounting::{AccountingError, build_accounting};
use super::identity::{member_digest, source_set_digest};
use super::projection::{
    css_attempt, parser_attempt, rendering_attempt, rendering_comparison_kind,
};
use super::{
    AggregateAccounting, AggregateIdentityError, AggregateLogicalCaseMemberDigest,
    AggregateLogicalCaseSourceSetDigest, AggregateLogicalSourceIdentity,
};

/// One named AG lane execution request. Stage 1 deliberately uses only the
/// existing empty execution-environment assessment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AggregateExecutionRequest {
    pub lane: LanePolicyScope,
}

/// The execution-environment assessment policy that was actually applied to
/// one aggregate run. AG9a intentionally exposes only the current empty V1
/// assessment rather than a caller-constructible general assessment API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateEnvironmentAssessmentMode {
    EmptyV1,
}

impl AggregateEnvironmentAssessmentMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmptyV1 => "ag9-empty-assessment-v1",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AggregateExecutionVariantId {
    Singleton(ExecutionVariantId<SingletonExecutionVariant>),
    Rendering(ExecutionVariantId<RenderingExecutionVariantId>),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AggregateVariantKey {
    pub test_id: TestId,
    pub observation: ObservationSurface,
    pub variant: AggregateExecutionVariantId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaneSelection {
    NotApplicable,
    Selected {
        lane: LanePolicyScope,
    },
    Excluded {
        lane: LanePolicyScope,
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AggregateTerminalOutcome {
    SemanticPass,
    SemanticFail,
    ExecutionFailure,
    ResourceFailure,
    IncompleteObservation,
    InvariantFailure,
    /// Reserved by the AG9 contract. No Stage 1 adapter produces this value.
    Timeout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateNotAttemptedReason {
    Eligibility,
    LaneExcluded,
    ParserPreAttemptEvaluation,
    CssFragmentCapabilityUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AggregateExecutionAttempt {
    NotAttempted { reason: AggregateNotAttemptedReason },
    Attempted { outcome: AggregateTerminalOutcome },
}

impl AggregateExecutionAttempt {
    pub const fn terminal_outcome(&self) -> Option<AggregateTerminalOutcome> {
        match self {
            Self::NotAttempted { .. } => None,
            Self::Attempted { outcome } => Some(*outcome),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AggregateComparisonKind {
    AuthoredExpectedObservation,
    StaticDocumentReference {
        reference_kind: ReferenceKind,
        relation: ReferenceRelation,
    },
}

/// Lossless subsystem result retained alongside one execution-variant
/// projection. Aggregate outcomes never replace these authoritative typed
/// values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AggregateSubsystemResult {
    Parser(NormalizedCaseResult),
    Css(CssCaseResult),
    Rendering(RenderingVariantResult),
}

/// Authoritative case-level metadata retained from the originating rendering
/// result. It is kept once per logical rendering case, independently of the
/// number of materialized execution variants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateRenderingCaseEvidence {
    originating_ag: AgCaseState,
}

impl AggregateRenderingCaseEvidence {
    pub(crate) const fn new(originating_ag: AgCaseState) -> Self {
        Self { originating_ag }
    }

    pub const fn originating_ag(&self) -> &AgCaseState {
        &self.originating_ag
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateVariantResult {
    pub key: AggregateVariantKey,
    pub selection: LaneSelection,
    pub execution: AggregateExecutionAttempt,
    pub policy: DerivedPolicyResult,
    pub comparison: AggregateComparisonKind,
    pub subsystem: AggregateSubsystemResult,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateCaseResult {
    pub fixture: ValidatedFixture,
    pub source_identity: AggregateLogicalSourceIdentity,
    pub member_digest: AggregateLogicalCaseMemberDigest,
    pub owner: SubsystemOwner,
    pub ag: AgCaseState,
    pub(crate) rendering_evidence: Option<AggregateRenderingCaseEvidence>,
    pub variants: Vec<AggregateVariantResult>,
}

impl AggregateCaseResult {
    /// Returns the case-level originating rendering metadata when this logical
    /// case is owned by the layout or paint adapter.
    pub const fn rendering_evidence(&self) -> Option<&AggregateRenderingCaseEvidence> {
        self.rendering_evidence.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateRun {
    inventory_scope: InventoryScope,
    request: AggregateExecutionRequest,
    environment_assessment_mode: AggregateEnvironmentAssessmentMode,
    cases: Vec<AggregateCaseResult>,
    accounting: AggregateAccounting,
    logical_case_source_set_digest: AggregateLogicalCaseSourceSetDigest,
}

impl AggregateRun {
    /// Canonically seals primary aggregate case state. Accounting and root
    /// population identity are always derived from these exact cases and can
    /// never be supplied independently by a caller.
    pub(crate) fn try_seal(
        inventory_scope: InventoryScope,
        request: AggregateExecutionRequest,
        environment_assessment_mode: AggregateEnvironmentAssessmentMode,
        cases: Vec<AggregateCaseResult>,
    ) -> Result<Self, AggregateRunSealError> {
        validate_case_population(inventory_scope, request, &cases)?;
        let accounting = build_accounting(&cases).map_err(AggregateRunSealError::Accounting)?;

        let mut members = Vec::new();
        members
            .try_reserve(cases.len())
            .map_err(|_| AggregateRunSealError::Allocation {
                storage: "logical-source-membership",
                requested: cases.len(),
            })?;
        members.extend(
            cases
                .iter()
                .map(|case| (&case.ag.test_id, case.member_digest)),
        );
        let logical_case_source_set_digest = source_set_digest(inventory_scope, &members)
            .map_err(AggregateRunSealError::Identity)?;

        Ok(Self {
            inventory_scope,
            request,
            environment_assessment_mode,
            cases,
            accounting,
            logical_case_source_set_digest,
        })
    }

    pub const fn inventory_scope(&self) -> InventoryScope {
        self.inventory_scope
    }

    pub const fn request(&self) -> AggregateExecutionRequest {
        self.request
    }

    pub const fn environment_assessment_mode(&self) -> AggregateEnvironmentAssessmentMode {
        self.environment_assessment_mode
    }

    pub fn cases(&self) -> &[AggregateCaseResult] {
        &self.cases
    }

    pub const fn accounting(&self) -> &AggregateAccounting {
        &self.accounting
    }

    pub const fn logical_case_source_set_digest(&self) -> AggregateLogicalCaseSourceSetDigest {
        self.logical_case_source_set_digest
    }

    pub(crate) fn validate_ag3_projection_invariants(
        &self,
    ) -> Result<(), AggregateRunInvariantError> {
        for case in &self.cases {
            validate_ag3_case_state(&case.ag)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AggregateRunInvariantError {
    NotYetClassifiedHasClassifiedDimensions {
        test_id: String,
    },
    NotYetClassifiedHasEstablishedExpectation {
        test_id: String,
    },
    ClassifiedLacksRequiredDimensions {
        test_id: String,
    },
    ClassifiedLacksEstablishedExpectation {
        test_id: String,
    },
    DuplicateLaneExclusion {
        test_id: String,
        lane: LanePolicyScope,
    },
    WrongFixtureScope {
        test_id: String,
        expected: InventoryScope,
        actual: InventoryScope,
    },
    FixtureTestIdMismatch {
        fixture_test_id: String,
        aggregate_test_id: String,
    },
    FixtureObservationMismatch {
        test_id: String,
        expected: ObservationSurface,
        actual: ObservationSurface,
    },
    SourceIdentityMismatch {
        test_id: String,
    },
    OwnerObservationMismatch {
        test_id: String,
        expected: SubsystemOwner,
        actual: SubsystemOwner,
    },
    DuplicateLogicalTestId {
        test_id: String,
    },
    MemberDigestMismatch {
        test_id: String,
    },
    VariantTestIdMismatch {
        logical_test_id: String,
        variant_test_id: String,
    },
    VariantObservationMismatch {
        test_id: String,
        expected: ObservationSurface,
        actual: ObservationSurface,
    },
    VariantSelectionMismatch {
        key: AggregateVariantKey,
    },
    InvalidSelectionAttempt {
        key: AggregateVariantKey,
        problem: &'static str,
    },
    VariantProjectionMismatch {
        key: AggregateVariantKey,
        field: &'static str,
    },
    MissingRenderingCaseEvidence {
        test_id: String,
    },
    UnexpectedRenderingCaseEvidence {
        test_id: String,
    },
    RenderingCaseMetadataMismatch {
        test_id: String,
    },
    DuplicateVariantKey {
        key: AggregateVariantKey,
    },
    PopulationSizeOverflow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AggregateRunSealError {
    Invariant(AggregateRunInvariantError),
    Accounting(AccountingError),
    Identity(AggregateIdentityError),
    Allocation {
        storage: &'static str,
        requested: usize,
    },
}

impl From<AggregateRunInvariantError> for AggregateRunSealError {
    fn from(error: AggregateRunInvariantError) -> Self {
        Self::Invariant(error)
    }
}

pub(crate) fn validate_ag3_case_state(ag: &AgCaseState) -> Result<(), AggregateRunInvariantError> {
    match &ag.classification {
        crate::ClassificationCompleteness::NotYetClassified { .. } => {
            if ag.capability.is_some()
                || ag.harness.is_some()
                || ag.stability.is_some()
                || !ag.requirements.is_empty()
                || !ag.environment_requirements.is_empty()
                || !ag.lane_exclusions.is_empty()
            {
                return Err(
                    AggregateRunInvariantError::NotYetClassifiedHasClassifiedDimensions {
                        test_id: ag.test_id.as_str().to_owned(),
                    },
                );
            }
            if !matches!(&ag.expectation, crate::AgExpectation::NotEstablished) {
                return Err(
                    AggregateRunInvariantError::NotYetClassifiedHasEstablishedExpectation {
                        test_id: ag.test_id.as_str().to_owned(),
                    },
                );
            }
        }
        crate::ClassificationCompleteness::Classified => {
            if ag.capability.is_none() || ag.harness.is_none() || ag.stability.is_none() {
                return Err(
                    AggregateRunInvariantError::ClassifiedLacksRequiredDimensions {
                        test_id: ag.test_id.as_str().to_owned(),
                    },
                );
            }
            if matches!(&ag.expectation, crate::AgExpectation::NotEstablished) {
                return Err(
                    AggregateRunInvariantError::ClassifiedLacksEstablishedExpectation {
                        test_id: ag.test_id.as_str().to_owned(),
                    },
                );
            }
        }
    }
    for (index, exclusion) in ag.lane_exclusions.iter().enumerate() {
        if ag.lane_exclusions[..index]
            .iter()
            .any(|earlier| earlier.policy == exclusion.policy)
        {
            return Err(AggregateRunInvariantError::DuplicateLaneExclusion {
                test_id: ag.test_id.as_str().to_owned(),
                lane: exclusion.policy,
            });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExpectedLaneSelection<'a> {
    NotApplicable,
    Selected,
    Excluded { reason: &'a str },
}

pub(crate) fn expected_lane_selection(
    ag: &AgCaseState,
    lane: LanePolicyScope,
) -> ExpectedLaneSelection<'_> {
    if !matches!(ag.eligibility, crate::Eligibility::Runnable) {
        return ExpectedLaneSelection::NotApplicable;
    }
    match ag
        .lane_exclusions
        .iter()
        .find(|exclusion| exclusion.policy == lane)
    {
        Some(exclusion) => ExpectedLaneSelection::Excluded {
            reason: &exclusion.reason,
        },
        None => ExpectedLaneSelection::Selected,
    }
}

pub(crate) const fn owner_for_surface(surface: ObservationSurface) -> SubsystemOwner {
    match surface {
        ObservationSurface::HtmlTokenizer
        | ObservationSurface::HtmlTreeConstruction
        | ObservationSurface::DomTree => SubsystemOwner::HtmlParser,
        ObservationSurface::CssParsing
        | ObservationSurface::CssSelectors
        | ObservationSurface::CssCascade
        | ObservationSurface::ComputedStyle => SubsystemOwner::Css,
        ObservationSurface::LayoutGeometry => SubsystemOwner::Layout,
        ObservationSurface::PaintOperations => SubsystemOwner::Paint,
        ObservationSurface::BrowserRuntimeSemantic => SubsystemOwner::BrowserRuntime,
    }
}

pub(crate) fn aggregate_variant_result_cmp(
    left: &AggregateVariantResult,
    right: &AggregateVariantResult,
) -> Ordering {
    aggregate_variant_key_cmp(&left.key, &right.key)
}

fn aggregate_variant_key_cmp(left: &AggregateVariantKey, right: &AggregateVariantKey) -> Ordering {
    left.test_id
        .as_str()
        .as_bytes()
        .cmp(right.test_id.as_str().as_bytes())
        .then_with(|| {
            left.observation
                .as_str()
                .as_bytes()
                .cmp(right.observation.as_str().as_bytes())
        })
        .then_with(|| {
            variant_identity_key(&left.variant).cmp(&variant_identity_key(&right.variant))
        })
}

fn variant_identity_key(value: &AggregateExecutionVariantId) -> (u8, &[u8], u32) {
    match value {
        AggregateExecutionVariantId::Singleton(_) => (0, b"", 0),
        AggregateExecutionVariantId::Rendering(rendering) => (
            1,
            rendering.value().stable_environment_label().as_bytes(),
            rendering.value().available_width_css_px.get(),
        ),
    }
}

fn validate_case_population(
    inventory_scope: InventoryScope,
    request: AggregateExecutionRequest,
    cases: &[AggregateCaseResult],
) -> Result<(), AggregateRunSealError> {
    let variant_count = cases.iter().try_fold(0_usize, |total, case| {
        total.checked_add(case.variants.len())
    });
    let variant_count = variant_count.ok_or(AggregateRunInvariantError::PopulationSizeOverflow)?;

    let mut logical_ids = Vec::new();
    logical_ids
        .try_reserve(cases.len())
        .map_err(|_| AggregateRunSealError::Allocation {
            storage: "logical-identity-validation",
            requested: cases.len(),
        })?;
    let mut variant_keys = Vec::new();
    variant_keys
        .try_reserve(variant_count)
        .map_err(|_| AggregateRunSealError::Allocation {
            storage: "variant-identity-validation",
            requested: variant_count,
        })?;

    for case in cases {
        validate_case(inventory_scope, request, case)?;
        logical_ids.push(&case.ag.test_id);
        variant_keys.extend(case.variants.iter().map(|variant| &variant.key));
    }

    logical_ids
        .sort_unstable_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
    if let Some(pair) = logical_ids.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(AggregateRunInvariantError::DuplicateLogicalTestId {
            test_id: pair[0].as_str().to_owned(),
        }
        .into());
    }

    variant_keys.sort_unstable_by(|left, right| aggregate_variant_key_cmp(left, right));
    if let Some(pair) = variant_keys
        .windows(2)
        .find(|pair| aggregate_variant_key_cmp(pair[0], pair[1]) == Ordering::Equal)
    {
        return Err(AggregateRunInvariantError::DuplicateVariantKey {
            key: pair[0].clone(),
        }
        .into());
    }
    Ok(())
}

fn validate_case(
    inventory_scope: InventoryScope,
    request: AggregateExecutionRequest,
    case: &AggregateCaseResult,
) -> Result<(), AggregateRunSealError> {
    validate_ag3_case_state(&case.ag)?;
    if case.fixture.scope() != inventory_scope {
        return Err(AggregateRunInvariantError::WrongFixtureScope {
            test_id: case.fixture.id().as_str().to_owned(),
            expected: inventory_scope,
            actual: case.fixture.scope(),
        }
        .into());
    }
    if case.fixture.id() != &case.ag.test_id {
        return Err(AggregateRunInvariantError::FixtureTestIdMismatch {
            fixture_test_id: case.fixture.id().as_str().to_owned(),
            aggregate_test_id: case.ag.test_id.as_str().to_owned(),
        }
        .into());
    }
    if case.fixture.observation() != case.ag.observation {
        return Err(AggregateRunInvariantError::FixtureObservationMismatch {
            test_id: case.ag.test_id.as_str().to_owned(),
            expected: case.fixture.observation(),
            actual: case.ag.observation,
        }
        .into());
    }
    validate_source_identity(case)?;
    let expected_owner = owner_for_surface(case.ag.observation);
    if case.owner != expected_owner {
        return Err(AggregateRunInvariantError::OwnerObservationMismatch {
            test_id: case.ag.test_id.as_str().to_owned(),
            expected: expected_owner,
            actual: case.owner,
        }
        .into());
    }
    validate_rendering_case_evidence(case)?;
    let expected_member = member_digest(&case.fixture, &case.source_identity)
        .map_err(AggregateRunSealError::Identity)?;
    if case.member_digest != expected_member {
        return Err(AggregateRunInvariantError::MemberDigestMismatch {
            test_id: case.ag.test_id.as_str().to_owned(),
        }
        .into());
    }
    for variant in &case.variants {
        validate_variant(request, case, variant)?;
    }
    Ok(())
}

fn validate_rendering_case_evidence(
    case: &AggregateCaseResult,
) -> Result<(), AggregateRunInvariantError> {
    let is_rendering_case = matches!(case.owner, SubsystemOwner::Layout | SubsystemOwner::Paint);
    match (is_rendering_case, case.rendering_evidence()) {
        (true, Some(evidence)) if evidence.originating_ag() == &case.ag => Ok(()),
        (true, Some(_)) => Err(AggregateRunInvariantError::RenderingCaseMetadataMismatch {
            test_id: case.ag.test_id.as_str().to_owned(),
        }),
        (true, None) => Err(AggregateRunInvariantError::MissingRenderingCaseEvidence {
            test_id: case.ag.test_id.as_str().to_owned(),
        }),
        (false, Some(_)) => Err(
            AggregateRunInvariantError::UnexpectedRenderingCaseEvidence {
                test_id: case.ag.test_id.as_str().to_owned(),
            },
        ),
        (false, None) => Ok(()),
    }
}

fn validate_source_identity(case: &AggregateCaseResult) -> Result<(), AggregateRunSealError> {
    if !case.source_identity.matches_fixture_source(&case.fixture) {
        return Err(AggregateRunInvariantError::SourceIdentityMismatch {
            test_id: case.ag.test_id.as_str().to_owned(),
        }
        .into());
    }
    Ok(())
}

fn validate_variant(
    request: AggregateExecutionRequest,
    case: &AggregateCaseResult,
    variant: &AggregateVariantResult,
) -> Result<(), AggregateRunSealError> {
    if variant.key.test_id != case.ag.test_id {
        return Err(AggregateRunInvariantError::VariantTestIdMismatch {
            logical_test_id: case.ag.test_id.as_str().to_owned(),
            variant_test_id: variant.key.test_id.as_str().to_owned(),
        }
        .into());
    }
    if variant.key.observation != case.ag.observation {
        return Err(AggregateRunInvariantError::VariantObservationMismatch {
            test_id: case.ag.test_id.as_str().to_owned(),
            expected: case.ag.observation,
            actual: variant.key.observation,
        }
        .into());
    }
    let expected_selection = expected_lane_selection(&case.ag, request.lane);
    if !selection_matches(&variant.selection, request.lane, expected_selection) {
        return Err(AggregateRunInvariantError::VariantSelectionMismatch {
            key: variant.key.clone(),
        }
        .into());
    }
    validate_selection_attempt(
        &variant.key,
        &case.ag.eligibility,
        &variant.selection,
        &variant.execution,
    )?;
    validate_subsystem_projection(case, variant)
}

fn selection_matches(
    actual: &LaneSelection,
    lane: LanePolicyScope,
    expected: ExpectedLaneSelection<'_>,
) -> bool {
    match (actual, expected) {
        (LaneSelection::NotApplicable, ExpectedLaneSelection::NotApplicable) => true,
        (LaneSelection::Selected { lane: actual }, ExpectedLaneSelection::Selected) => {
            *actual == lane
        }
        (
            LaneSelection::Excluded {
                lane: actual,
                reason: actual_reason,
            },
            ExpectedLaneSelection::Excluded { reason },
        ) => *actual == lane && actual_reason == reason,
        _ => false,
    }
}

pub(crate) fn validate_selection_attempt(
    key: &AggregateVariantKey,
    eligibility: &crate::Eligibility,
    selection: &LaneSelection,
    execution: &AggregateExecutionAttempt,
) -> Result<(), AggregateRunInvariantError> {
    let valid = matches!(
        (eligibility, selection, execution),
        (
            crate::Eligibility::Runnable,
            LaneSelection::Selected { .. },
            AggregateExecutionAttempt::Attempted { .. }
                | AggregateExecutionAttempt::NotAttempted {
                    reason: AggregateNotAttemptedReason::ParserPreAttemptEvaluation
                        | AggregateNotAttemptedReason::CssFragmentCapabilityUnavailable,
                },
        ) | (
            crate::Eligibility::Runnable,
            LaneSelection::Excluded { .. },
            AggregateExecutionAttempt::NotAttempted {
                reason: AggregateNotAttemptedReason::LaneExcluded,
            },
        ) | (
            crate::Eligibility::NotRunnable { .. } | crate::Eligibility::NotYetEstablished { .. },
            LaneSelection::NotApplicable,
            AggregateExecutionAttempt::NotAttempted {
                reason: AggregateNotAttemptedReason::Eligibility,
            },
        )
    );
    if !valid {
        return Err(AggregateRunInvariantError::InvalidSelectionAttempt {
            key: key.clone(),
            problem: "eligibility, lane selection, and execution-attempt state disagree",
        });
    }
    Ok(())
}

fn validate_subsystem_projection(
    case: &AggregateCaseResult,
    variant: &AggregateVariantResult,
) -> Result<(), AggregateRunSealError> {
    let mismatch = |field| {
        AggregateRunSealError::Invariant(AggregateRunInvariantError::VariantProjectionMismatch {
            key: variant.key.clone(),
            field,
        })
    };
    match (&variant.subsystem, &variant.key.variant) {
        (AggregateSubsystemResult::Parser(result), AggregateExecutionVariantId::Singleton(id)) => {
            if case.owner != SubsystemOwner::HtmlParser || result.ag != case.ag {
                return Err(mismatch("parser case metadata"));
            }
            if id != &result.variant {
                return Err(mismatch("parser variant identity"));
            }
            if variant.execution != parser_attempt(&result.execution) {
                return Err(mismatch("parser execution projection"));
            }
            if variant.policy != result.policy {
                return Err(mismatch("parser derived policy"));
            }
            if variant.comparison != AggregateComparisonKind::AuthoredExpectedObservation {
                return Err(mismatch("parser comparison kind"));
            }
        }
        (AggregateSubsystemResult::Css(result), AggregateExecutionVariantId::Singleton(id)) => {
            if case.owner != SubsystemOwner::Css || result.ag != case.ag {
                return Err(mismatch("CSS case metadata"));
            }
            if id != &result.variant {
                return Err(mismatch("CSS variant identity"));
            }
            if variant.execution != css_attempt(&result.execution) {
                return Err(mismatch("CSS execution projection"));
            }
            if variant.policy != result.policy {
                return Err(mismatch("CSS derived policy"));
            }
            if variant.comparison != AggregateComparisonKind::AuthoredExpectedObservation {
                return Err(mismatch("CSS comparison kind"));
            }
        }
        (
            AggregateSubsystemResult::Rendering(result),
            AggregateExecutionVariantId::Rendering(id),
        ) => {
            if id != &result.variant {
                return Err(mismatch("rendering variant identity"));
            }
            if variant.execution != rendering_attempt(&result.execution) {
                return Err(mismatch("rendering execution projection"));
            }
            if variant.policy != result.policy {
                return Err(mismatch("rendering derived policy"));
            }
            if variant.comparison != rendering_comparison_kind(result.oracle) {
                return Err(mismatch("rendering comparison kind"));
            }
        }
        (AggregateSubsystemResult::Parser(_), AggregateExecutionVariantId::Rendering(_))
        | (AggregateSubsystemResult::Css(_), AggregateExecutionVariantId::Rendering(_))
        | (AggregateSubsystemResult::Rendering(_), AggregateExecutionVariantId::Singleton(_)) => {
            return Err(mismatch("subsystem variant kind"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use conformance_test_support::SourceKind;

    use super::*;
    use crate::run_repository_aggregate;

    fn repository_run() -> AggregateRun {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        run_repository_aggregate(
            root,
            AggregateExecutionRequest {
                lane: LanePolicyScope::NormalCi,
            },
        )
        .unwrap()
    }

    fn reseal(
        run: &AggregateRun,
        cases: Vec<AggregateCaseResult>,
    ) -> Result<AggregateRun, AggregateRunSealError> {
        AggregateRun::try_seal(
            run.inventory_scope(),
            run.request(),
            run.environment_assessment_mode(),
            cases,
        )
    }

    #[test]
    fn sealing_derives_accounting_and_source_set_from_the_exact_case_population() {
        let run = repository_run();
        let original_accounting = run.accounting().clone();
        let original_source_set = run.logical_case_source_set_digest();
        let mut cases = run.cases().to_vec();
        let removed = cases.remove(0);
        let expected_accounting = build_accounting(&cases).unwrap();
        let members = cases
            .iter()
            .map(|case| (&case.ag.test_id, case.member_digest))
            .collect::<Vec<_>>();
        let expected_source_set = source_set_digest(run.inventory_scope(), &members).unwrap();

        let resealed = reseal(&run, cases).unwrap();
        assert_eq!(resealed.accounting(), &expected_accounting);
        assert_eq!(
            resealed.logical_case_source_set_digest(),
            expected_source_set
        );
        assert_eq!(
            resealed.accounting().logical.total_tests,
            original_accounting.logical.total_tests - 1
        );
        assert_eq!(
            resealed.accounting().variants.materialized_variants,
            original_accounting.variants.materialized_variants
                - u64::try_from(removed.variants.len()).unwrap()
        );
        assert_ne!(
            resealed.logical_case_source_set_digest(),
            original_source_set
        );
    }

    #[test]
    fn sealing_rejects_stale_member_and_structural_case_state() {
        let run = repository_run();

        let mut cases = run.cases().to_vec();
        cases[0].member_digest = cases[1].member_digest;
        assert!(matches!(
            reseal(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::MemberDigestMismatch { .. }
            ))
        ));

        let mut cases = run.cases().to_vec();
        cases[0].ag.test_id = TestId::parse("different-logical-id").unwrap();
        assert!(matches!(
            reseal(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::FixtureTestIdMismatch { .. }
            ))
        ));

        let mut cases = run.cases().to_vec();
        cases[0].ag.observation = ObservationSurface::CssParsing;
        assert!(matches!(
            reseal(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::FixtureObservationMismatch { .. }
            ))
        ));

        let mut cases = run.cases().to_vec();
        cases[0].owner = SubsystemOwner::Paint;
        assert!(matches!(
            reseal(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::OwnerObservationMismatch { .. }
            ))
        ));

        let mut cases = run.cases().to_vec();
        cases.push(cases[0].clone());
        assert!(matches!(
            reseal(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::DuplicateLogicalTestId { .. }
            ))
        ));
    }

    #[test]
    fn sealing_rejects_stale_variant_keys_selection_and_projection() {
        let run = repository_run();
        let case_index = run
            .cases()
            .iter()
            .position(|case| !case.variants.is_empty())
            .unwrap();

        let mut cases = run.cases().to_vec();
        cases[case_index].variants[0].key.test_id = TestId::parse("wrong-variant-id").unwrap();
        assert!(matches!(
            reseal(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::VariantTestIdMismatch { .. }
            ))
        ));

        let mut cases = run.cases().to_vec();
        cases[case_index].variants[0].key.observation = ObservationSurface::PaintOperations;
        assert!(matches!(
            reseal(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::VariantObservationMismatch { .. }
            ))
        ));

        let mut cases = run.cases().to_vec();
        cases[case_index].variants[0].selection = LaneSelection::Selected {
            lane: LanePolicyScope::ManualExtended,
        };
        assert!(matches!(
            reseal(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::VariantSelectionMismatch { .. }
            ))
        ));

        let mut cases = run.cases().to_vec();
        cases[case_index].variants[0].execution = AggregateExecutionAttempt::NotAttempted {
            reason: AggregateNotAttemptedReason::Eligibility,
        };
        assert!(matches!(
            reseal(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::InvalidSelectionAttempt { .. }
            ))
        ));

        let mut cases = run.cases().to_vec();
        cases[case_index].variants[0].policy = DerivedPolicyResult::UnexpectedOutcome;
        assert!(matches!(
            reseal(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::VariantProjectionMismatch { .. }
            ))
        ));

        let mut cases = run.cases().to_vec();
        let duplicate = cases[case_index].variants[0].clone();
        cases[case_index].variants.push(duplicate);
        assert!(matches!(
            reseal(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::DuplicateVariantKey { .. }
            ))
        ));
    }

    #[test]
    fn multi_variant_rendering_retains_one_case_evidence_and_every_variant() {
        let run = repository_run();
        let rendering_index = run
            .cases()
            .iter()
            .position(|case| {
                case.variants.len() > 1
                    && case.variants.iter().all(|variant| {
                        matches!(variant.subsystem, AggregateSubsystemResult::Rendering(_))
                    })
            })
            .expect("multi-variant rendering case");
        let case = &run.cases()[rendering_index];

        let evidence = case
            .rendering_evidence()
            .expect("one case-level rendering evidence record");
        assert_eq!(evidence.originating_ag(), &case.ag);
        assert!(case.variants.len() > 1);
        assert_eq!(
            case.variants
                .iter()
                .filter(|variant| matches!(
                    variant.subsystem,
                    AggregateSubsystemResult::Rendering(_)
                ))
                .count(),
            case.variants.len()
        );

        for variant_index in 0..case.variants.len() {
            let mut cases = run.cases().to_vec();
            let policy = &mut cases[rendering_index].variants[variant_index].policy;
            *policy = if *policy == DerivedPolicyResult::NotRun {
                DerivedPolicyResult::ExpectedPass
            } else {
                DerivedPolicyResult::NotRun
            };
            assert!(matches!(
                reseal(&run, cases),
                Err(AggregateRunSealError::Invariant(
                    AggregateRunInvariantError::VariantProjectionMismatch {
                        field: "rendering derived policy",
                        ..
                    }
                ))
            ));
        }
    }

    #[test]
    fn sealing_binds_rendering_case_to_the_authoritative_case_metadata() {
        let run = repository_run();
        let rendering_index = run
            .cases()
            .iter()
            .position(|case| {
                case.variants.iter().any(|variant| {
                    matches!(variant.subsystem, AggregateSubsystemResult::Rendering(_))
                        && variant.policy == DerivedPolicyResult::ExpectedPass
                })
            })
            .expect("attempted expected-pass rendering case");

        let unchanged = reseal(&run, run.cases().to_vec()).unwrap();
        assert_eq!(unchanged, run);

        let mut cases = run.cases().to_vec();
        cases[rendering_index].ag.expectation = crate::AgExpectation::ExpectedFail {
            failure: conformance_test_support::ExpectedFailureClassification::SemanticMismatch,
            reason: "sealing must require XPASS for this changed expectation".to_owned(),
        };
        assert!(matches!(
            reseal(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::RenderingCaseMetadataMismatch { .. }
            ))
        ));

        let mut cases = run.cases().to_vec();
        cases[rendering_index].ag.stability = Some(match cases[rendering_index].ag.stability {
            Some(crate::Stability::Stable) => crate::Stability::Flaky {
                reason: "structural metadata drift proof".to_owned(),
            },
            _ => crate::Stability::Stable,
        });
        assert!(matches!(
            reseal(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::RenderingCaseMetadataMismatch { .. }
            ))
        ));

        let mut cases = run.cases().to_vec();
        cases[rendering_index].rendering_evidence = None;
        assert!(matches!(
            reseal(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::MissingRenderingCaseEvidence { .. }
            ))
        ));
    }

    #[test]
    fn sealing_rejects_source_identity_branch_drift() {
        let run = repository_run();
        let controlled_index = run
            .cases()
            .iter()
            .position(|case| case.source_identity.source_kind() == SourceKind::ControlledStaticPage)
            .unwrap();
        let native_identity = run
            .cases()
            .iter()
            .find(|case| case.source_identity.source_kind() == SourceKind::Native)
            .unwrap()
            .source_identity
            .clone();
        let mut cases = run.cases().to_vec();
        cases[controlled_index].source_identity = native_identity;
        assert!(matches!(
            reseal(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::SourceIdentityMismatch { .. }
            ))
        ));
    }
}
