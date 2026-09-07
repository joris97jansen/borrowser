use conformance_test_support::{
    CapabilityFeatureId, EngineCapabilityKind, EnvironmentProfileId, EnvironmentRequirementKind,
    ExternalAdapterVersion, ExternalLineageId, HarnessFeatureId, HarnessLimitationKind,
    InventoryScope, ObservationSurface, ReferenceKind, ReferenceRelation, RequirementTag,
    SourceRecordId, TestId,
};
use external_test_provenance::Sha256Digest;

use super::model::*;
use crate::aggregate::AggregateComparisonKind;
use crate::aggregate::accounting::FixedAggregateAccountingProjection;
use crate::aggregate::identity::{
    historical_member_digest, source_set_digest_from_labels as calculate_source_set_digest,
};
use crate::aggregate::model::{
    AggregateAttemptProjection, AggregateEligibilityProjection, AggregateNotAttemptedReason,
    AggregateSelectionProjection, AggregateTerminalOutcome, owner_for_surface,
};
use crate::model::{DerivedPolicyResult, PolicyExpectationClass};

const OWNERS: [&str; 5] = ["html-parser", "css", "layout", "paint", "browser-runtime"];
const SURFACES: [&str; 10] = [
    "html-tokenizer",
    "html-tree-construction",
    "dom-tree",
    "css-parsing",
    "css-selectors",
    "css-cascade",
    "computed-style",
    "layout-geometry",
    "paint-operations",
    "browser-runtime-semantic",
];
const COMPARISONS: [(&str, Option<&str>, Option<&str>); 5] = [
    ("authored-expected-observation", None, None),
    ("static-document-reference", Some("semantic"), Some("match")),
    (
        "static-document-reference",
        Some("semantic"),
        Some("mismatch"),
    ),
    (
        "static-document-reference",
        Some("structural"),
        Some("match"),
    ),
    (
        "static-document-reference",
        Some("structural"),
        Some("mismatch"),
    ),
];
const TERMINALS: [&str; 7] = [
    "semantic-pass",
    "semantic-fail",
    "execution-failure",
    "resource-failure",
    "incomplete-observation",
    "invariant-failure",
    "timeout",
];

struct Declared {
    logical: [u64; 8],
    variants: [u64; 9],
    owners: [[u64; 2]; 5],
    surfaces: [[u64; 2]; 10],
    comparisons: [u64; 5],
    terminals: [u64; 7],
}

pub(crate) fn decode_aggregate_detail_v1(
    bytes: &[u8],
) -> Result<HistoricalDetail, HistoricalDetailError> {
    if bytes.len() > super::super::report::AGGREGATE_DETAIL_MAX_BYTES_V1
        || !bytes.ends_with(b"\n")
        || bytes.windows(2).any(|w| w == b"\r\n")
    {
        return Err(HistoricalDetailError::InvalidFraming);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| HistoricalDetailError::InvalidUtf8)?;
    let mut p = Parser::new(text)?;
    p.exact_string("format", super::super::report::AGGREGATE_DETAIL_FORMAT_V1)?;
    let inventory_scope = p.string("inventory-scope")?;
    if inventory_scope != "static-html-css-no-js" {
        return Err(HistoricalDetailError::InvalidVocabulary);
    }
    let aggregate_contract = p.string("aggregate-granularity-contract")?;
    p.exact_string_value(
        &aggregate_contract,
        super::super::report::AGGREGATE_GRANULARITY_CONTRACT_V1,
    )?;
    let named_lane = p.string("named-lane")?;
    require_one(
        &named_lane,
        &[
            "normal-ci",
            "local-extended",
            "scheduled-extended",
            "manual-extended",
        ],
    )?;
    let environment_assessment = p.string("environment-assessment")?;
    p.exact_string_value(&environment_assessment, "ag9-empty-assessment-v1")?;
    let population_identity_contract = p.string("population-identity-contract")?;
    p.exact_string_value(
        &population_identity_contract,
        super::super::report::AGGREGATE_POPULATION_IDENTITY_CONTRACT_V1,
    )?;
    let source_set_digest = p.digest("logical-case-source-set-digest")?;
    p.exact_bool("headline-counts-overlap", true)?;
    p.exact_string("logical-case-population", "logical-case")?;
    p.exact_string("execution-variant-population", "execution-variant")?;
    p.exact_number("accounting-count-field-count", 59)?;
    p.exact_number("subsystem-row-count", 5)?;
    p.exact_number("observation-row-count", 10)?;
    p.exact_number("comparison-row-count", 5)?;
    p.exact_number("terminal-row-count", 7)?;
    let declared = parse_declared(&mut p)?;
    let count = p.number("logical-case-detail-count")?;
    let count = usize::try_from(count).map_err(|_| HistoricalDetailError::Overflow)?;
    let mut cases = Vec::new();
    let mut derived = FixedAggregateAccountingProjection::default();
    for _ in 0..count {
        cases
            .try_reserve(1)
            .map_err(|_| HistoricalDetailError::Allocation)?;
        cases.push(parse_case(&mut p, &named_lane, &mut derived)?);
    }
    p.finish()?;
    if cases
        .windows(2)
        .any(|w| w[0].test_id.as_bytes() >= w[1].test_id.as_bytes())
    {
        return Err(if cases.windows(2).any(|w| w[0].test_id == w[1].test_id) {
            HistoricalDetailError::DuplicateIdentity
        } else {
            HistoricalDetailError::NonCanonicalOrder
        });
    }
    validate_declared(&declared, &derived)?;
    let scope = InventoryScope::StaticHtmlCssNoJs;
    let mut members = Vec::new();
    members
        .try_reserve_exact(cases.len())
        .map_err(|_| HistoricalDetailError::Allocation)?;
    for case in &cases {
        members.push((
            case.test_id.as_str(),
            crate::AggregateLogicalCaseMemberDigest::from_sha256(case.member_digest),
        ));
    }
    let calculated = calculate_source_set_digest(scope, &members).map_err(|e| match e {
        crate::AggregateIdentityError::AllocationFailure => HistoricalDetailError::Allocation,
        _ => HistoricalDetailError::InvalidState,
    })?;
    if calculated.as_sha256() != &source_set_digest {
        return Err(HistoricalDetailError::SourceSetDigestMismatch);
    }
    Ok(HistoricalDetail {
        inventory_scope,
        aggregate_contract,
        named_lane,
        environment_assessment,
        population_identity_contract,
        source_set_digest,
        cases,
    })
}

fn parse_declared(p: &mut Parser<'_>) -> Result<Declared, HistoricalDetailError> {
    p.marker("BEGIN logical-accounting")?;
    let logical = read_numbers::<8>(
        p,
        [
            "total",
            "pass",
            "fail",
            "expected-fail",
            "unsupported",
            "skipped",
            "flaky",
            "unclassified",
        ],
    )?;
    p.marker("END logical-accounting")?;
    p.marker("BEGIN execution-variant-accounting")?;
    let variants = read_numbers::<9>(
        p,
        [
            "materialized",
            "runnable",
            "not-runnable",
            "eligibility-not-established",
            "selected",
            "excluded",
            "selection-not-applicable",
            "attempted",
            "not-attempted",
        ],
    )?;
    p.marker("END execution-variant-accounting")?;
    let mut owners = [[0; 2]; 5];
    for (i, name) in OWNERS.iter().enumerate() {
        p.marker("BEGIN subsystem")?;
        p.exact_string("owner", name)?;
        p.exact_string("logical-domain", "logical-case")?;
        p.exact_string("variant-domain", "execution-variant")?;
        owners[i] = [p.number("logical-cases")?, p.number("execution-variants")?];
        p.marker("END subsystem")?;
    }
    let mut surfaces = [[0; 2]; 10];
    for (i, name) in SURFACES.iter().enumerate() {
        p.marker("BEGIN observation")?;
        p.exact_string("surface", name)?;
        p.exact_string("logical-domain", "logical-case")?;
        p.exact_string("variant-domain", "execution-variant")?;
        surfaces[i] = [p.number("logical-cases")?, p.number("execution-variants")?];
        p.marker("END observation")?;
    }
    let mut comparisons = [0; 5];
    for (i, (kind, rk, rr)) in COMPARISONS.iter().enumerate() {
        p.marker("BEGIN comparison")?;
        p.exact_string("comparison-kind", kind)?;
        p.exact_optional_string("reference-kind", *rk)?;
        p.exact_optional_string("reference-relation", *rr)?;
        comparisons[i] = p.number("execution-variants")?;
        p.marker("END comparison")?;
    }
    let mut terminals = [0; 7];
    for (i, name) in TERMINALS.iter().enumerate() {
        p.marker("BEGIN terminal")?;
        p.exact_string("outcome", name)?;
        terminals[i] = p.number("attempted-variants")?;
        p.marker("END terminal")?;
    }
    Ok(Declared {
        logical,
        variants,
        owners,
        surfaces,
        comparisons,
        terminals,
    })
}
fn read_numbers<const N: usize>(
    p: &mut Parser<'_>,
    names: [&str; N],
) -> Result<[u64; N], HistoricalDetailError> {
    let mut out = [0; N];
    for (i, n) in names.iter().enumerate() {
        out[i] = p.number(n)?;
    }
    Ok(out)
}

fn parse_case(
    p: &mut Parser<'_>,
    lane: &str,
    accounting: &mut FixedAggregateAccountingProjection,
) -> Result<HistoricalLogicalCase, HistoricalDetailError> {
    let start = p.marker_start("BEGIN logical-case")?;
    let test_id = p.string("test-id")?;
    if !TestId::is_valid(&test_id) {
        return Err(HistoricalDetailError::InvalidValue);
    }
    let member_digest = p.digest("logical-case-member-digest")?;
    let source_kind = p.string("source-kind")?;
    require_one(
        &source_kind,
        &["native", "controlled-static-page", "external-derived"],
    )?;
    let source_record = p.optional_string("external-source-record")?;
    let lineage = p.optional_string("external-lineage")?;
    let adapter = p.optional_string("external-adapter")?;
    let adapter_version = p.optional_string("external-adapter-version")?;
    let external = source_kind == "external-derived";
    if [
        source_record.is_some(),
        lineage.is_some(),
        adapter.is_some(),
        adapter_version.is_some(),
    ]
    .iter()
    .any(|v| *v != external)
    {
        return Err(HistoricalDetailError::InvalidState);
    }
    if external
        && (!SourceRecordId::is_valid(
            source_record
                .as_deref()
                .ok_or(HistoricalDetailError::InvalidState)?,
        ) || !ExternalLineageId::is_valid(
            lineage
                .as_deref()
                .ok_or(HistoricalDetailError::InvalidState)?,
        ) || !HarnessFeatureId::is_valid(
            adapter
                .as_deref()
                .ok_or(HistoricalDetailError::InvalidState)?,
        ) || !ExternalAdapterVersion::is_valid(
            adapter_version
                .as_deref()
                .ok_or(HistoricalDetailError::InvalidState)?,
        ))
    {
        return Err(HistoricalDetailError::InvalidValue);
    }
    let persisted_owner = p.string("subsystem-owner")?;
    let observation = p.string("observation-surface")?;
    let observation_enum =
        ObservationSurface::parse(&observation).ok_or(HistoricalDetailError::InvalidVocabulary)?;
    let owner = owner_for_surface(observation_enum);
    if persisted_owner != owner.as_str() {
        return Err(HistoricalDetailError::InvalidState);
    }
    let classification = p.string("classification")?;
    require_one(&classification, &["classified", "not-yet-classified"])?;
    if classification == "not-yet-classified" {
        p.require_present_string("classification-reason")?;
        for key in [
            "requirements",
            "capability",
            "capability-missing-count",
            "harness",
            "harness-limitation-count",
            "environment-requirement-count",
            "stability",
            "stability-reason",
            "lane-exclusion-count",
        ] {
            p.require_null(key)?;
        }
    } else {
        p.require_null("classification-reason")?;
        p.sorted_requirements("requirements")?;
    }
    let capability = if classification == "classified" {
        let value = p.string("capability")?;
        require_one(&value, &["available", "unavailable", "not-yet-established"])?;
        let n = p.number("capability-missing-count")?;
        if (value == "unavailable") != (n > 0) {
            return Err(HistoricalDetailError::InvalidState);
        }
        let mut previous = None;
        for _ in 0..n {
            let fields =
                parse_simple_record(p, "capability-missing", &["kind", "feature", "reason"])?;
            let kind = EngineCapabilityKind::parse(
                fields[0]
                    .as_deref()
                    .ok_or(HistoricalDetailError::InvalidState)?,
            )
            .ok_or(HistoricalDetailError::InvalidVocabulary)?;
            let feature = fields[1].as_deref();
            if feature.is_some_and(|value| !CapabilityFeatureId::is_valid(value))
                || kind.requires_feature() != feature.is_some()
            {
                return Err(HistoricalDetailError::InvalidState);
            }
            if previous.as_ref().is_some_and(|value| value >= &fields) {
                return Err(HistoricalDetailError::NonCanonicalOrder);
            }
            previous = Some(fields);
        }
        Some(value)
    } else {
        None
    };
    if classification == "classified" {
        let harness = p.string("harness")?;
        require_one(&harness, &["ready", "not-ready", "not-yet-established"])?;
        let n = p.number("harness-limitation-count")?;
        if (harness == "not-ready") != (n > 0) {
            return Err(HistoricalDetailError::InvalidState);
        }
        let mut previous = None;
        for _ in 0..n {
            let fields = parse_simple_record(p, "harness-limitation", &["kind", "reason"])?;
            if HarnessLimitationKind::parse(
                fields[0]
                    .as_deref()
                    .ok_or(HistoricalDetailError::InvalidState)?,
            )
            .is_none()
            {
                return Err(HistoricalDetailError::InvalidVocabulary);
            }
            if previous.as_ref().is_some_and(|value| value >= &fields) {
                return Err(HistoricalDetailError::NonCanonicalOrder);
            }
            previous = Some(fields);
        }
        let n = p.number("environment-requirement-count")?;
        let mut previous = None;
        for _ in 0..n {
            let fields =
                parse_simple_record(p, "environment-requirement", &["kind", "profile", "reason"])?;
            if EnvironmentRequirementKind::parse(
                fields[0]
                    .as_deref()
                    .ok_or(HistoricalDetailError::InvalidState)?,
            )
            .is_none()
                || !EnvironmentProfileId::is_valid(
                    fields[1]
                        .as_deref()
                        .ok_or(HistoricalDetailError::InvalidState)?,
                )
            {
                return Err(HistoricalDetailError::InvalidVocabulary);
            }
            if previous.as_ref().is_some_and(|value| value >= &fields) {
                return Err(HistoricalDetailError::NonCanonicalOrder);
            }
            previous = Some(fields);
        }
    }
    let stability = if classification == "classified" {
        let v = p.string("stability")?;
        require_one(&v, &["stable", "flaky", "not-yet-established"])?;
        let r = p.optional_string("stability-reason")?;
        if (v == "flaky") != r.is_some() {
            return Err(HistoricalDetailError::InvalidState);
        }
        let n = p.number("lane-exclusion-count")?;
        let mut prev = None;
        for _ in 0..n {
            p.marker("BEGIN lane-exclusion")?;
            let l = p.string("lane")?;
            require_one(
                &l,
                &[
                    "normal-ci",
                    "local-extended",
                    "scheduled-extended",
                    "manual-extended",
                ],
            )?;
            if prev
                .as_ref()
                .is_some_and(|x: &String| lane_rank(x) >= lane_rank(&l))
            {
                return Err(HistoricalDetailError::NonCanonicalOrder);
            }
            prev = Some(l);
            p.require_present_string("reason")?;
            p.marker("END lane-exclusion")?;
        }
        Some(v)
    } else {
        None
    };
    let eligibility = p.string("eligibility")?;
    require_one(
        &eligibility,
        &["runnable", "not-runnable", "not-yet-established"],
    )?;
    let blockers = p.number("eligibility-blocker-count")?;
    let unresolved = p.number("eligibility-unresolved-count")?;
    if eligibility == "runnable" && (blockers != 0 || unresolved != 0)
        || eligibility == "not-yet-established" && blockers != 0
        || eligibility == "not-runnable" && blockers == 0
    {
        return Err(HistoricalDetailError::InvalidState);
    }
    let eligibility_projection = match eligibility.as_str() {
        "runnable" => AggregateEligibilityProjection::Runnable,
        "not-runnable" => AggregateEligibilityProjection::NotRunnable,
        "not-yet-established" => AggregateEligibilityProjection::NotYetEstablished,
        _ => return Err(HistoricalDetailError::InvalidVocabulary),
    };
    parse_eligibility_facts(p, "blocker", blockers)?;
    parse_eligibility_facts(p, "unresolved", unresolved)?;
    let expectation = p.string("expectation")?;
    require_one(
        &expectation,
        &["expected-pass", "expected-fail", "not-established"],
    )?;
    let failure = p.optional_string("expected-failure-kind")?;
    let reason = p.optional_string("expectation-reason")?;
    if (expectation == "expected-fail") != (failure.is_some() && reason.is_some())
        || expectation != "expected-fail" && (failure.is_some() || reason.is_some())
    {
        return Err(HistoricalDetailError::InvalidState);
    }
    if failure
        .as_deref()
        .is_some_and(|value| value != "semantic-mismatch")
    {
        return Err(HistoricalDetailError::InvalidVocabulary);
    }
    if (classification == "classified") == (expectation == "not-established") {
        return Err(HistoricalDetailError::InvalidState);
    }
    let expectation_projection = match expectation.as_str() {
        "expected-pass" => PolicyExpectationClass::ExpectedPass,
        "expected-fail" => PolicyExpectationClass::ExpectedFailSemanticMismatch,
        "not-established" => PolicyExpectationClass::NotEstablished,
        _ => return Err(HistoricalDetailError::InvalidVocabulary),
    };
    let metadata_end = p.position();
    let n = p.number("execution-variant-count")?;
    let n = usize::try_from(n).map_err(|_| HistoricalDetailError::Overflow)?;
    let mut variants = Vec::new();
    for _ in 0..n {
        variants
            .try_reserve(1)
            .map_err(|_| HistoricalDetailError::Allocation)?;
        variants.push(parse_variant(
            p,
            &test_id,
            &observation,
            lane,
            eligibility_projection,
            expectation_projection,
        )?);
    }
    p.marker("END logical-case")?;
    if variants.windows(2).any(|w| w[0].key >= w[1].key) {
        return Err(if variants.windows(2).any(|w| w[0].key == w[1].key) {
            HistoricalDetailError::DuplicateIdentity
        } else {
            HistoricalDetailError::NonCanonicalOrder
        });
    }
    let calculated = historical_member_digest(
        InventoryScope::StaticHtmlCssNoJs,
        &test_id,
        observation_enum,
        &source_kind,
        source_record.as_deref(),
        lineage.as_deref(),
        adapter.as_deref(),
        adapter_version.as_deref(),
    )
    .map_err(|_| HistoricalDetailError::InvalidState)?;
    if calculated.as_sha256() != &member_digest {
        return Err(HistoricalDetailError::MemberDigestMismatch);
    }
    crate::aggregate::accounting::accumulate_case_projection(
        accounting,
        crate::aggregate::accounting::LogicalCaseAccountingProjection {
            owner,
            observation: observation_enum,
            eligibility: eligibility_projection,
            expected_fail: expectation_projection
                == PolicyExpectationClass::ExpectedFailSemanticMismatch,
            unsupported: capability.as_deref() == Some("unavailable"),
            flaky: stability.as_deref() == Some("flaky"),
            unclassified: classification == "not-yet-classified",
        },
        variants.iter().map(
            |variant| crate::aggregate::accounting::VariantAccountingProjection {
                comparison: variant.comparison,
                selection: variant.selection,
                attempt: variant.attempt,
            },
        ),
    )
    .map_err(map_accounting_error)?;
    Ok(HistoricalLogicalCase {
        test_id,
        member_digest,
        canonical_metadata_prefix: copy_bytes(p.slice(start, metadata_end)?)?,
        variants,
    })
}

fn parse_variant(
    p: &mut Parser<'_>,
    test_id: &str,
    observation: &str,
    lane: &str,
    eligibility: AggregateEligibilityProjection,
    expectation: PolicyExpectationClass,
) -> Result<HistoricalVariant, HistoricalDetailError> {
    let start = p.marker_start("BEGIN execution-variant")?;
    let kind = p.string("variant-kind")?;
    let env = p.optional_string("rendering-environment")?;
    let width = p.optional_number("available-width-css-px")?;
    let variant = match (kind.as_str(), env, width) {
        ("singleton", None, None) => HistoricalVariantIdentity::Singleton,
        ("rendering", Some(e), Some(w))
            if e == "synthetic-text-metrics-v1" && (1..=16_777_216).contains(&w) =>
        {
            HistoricalVariantIdentity::Rendering {
                environment: e,
                available_width_css_px: u32::try_from(w)
                    .map_err(|_| HistoricalDetailError::Overflow)?,
            }
        }
        _ => return Err(HistoricalDetailError::InvalidState),
    };
    let rendering_surface = matches!(observation, "layout-geometry" | "paint-operations");
    if rendering_surface != matches!(variant, HistoricalVariantIdentity::Rendering { .. }) {
        return Err(HistoricalDetailError::InvalidState);
    }
    let comparison = p.string("comparison-kind")?;
    let rk = p.optional_string("reference-kind")?;
    let rr = p.optional_string("reference-relation")?;
    let comparison = parse_comparison(&comparison, rk.as_deref(), rr.as_deref())?;
    let selection = p.string("lane-selection")?;
    require_one(&selection, &["not-applicable", "selected", "excluded"])?;
    let sl = p.optional_string("selection-lane")?;
    let sr = p.optional_string("lane-selection-reason")?;
    match selection.as_str() {
        "not-applicable" if sl.is_none() && sr.is_none() => {}
        "selected" if sl.as_deref() == Some(lane) && sr.is_none() => {}
        "excluded" if sl.as_deref() == Some(lane) && sr.is_some() => {}
        _ => return Err(HistoricalDetailError::InvalidState),
    }
    let selection_projection = match selection.as_str() {
        "not-applicable" => AggregateSelectionProjection::NotApplicable,
        "selected" => AggregateSelectionProjection::Selected,
        "excluded" => AggregateSelectionProjection::Excluded,
        _ => return Err(HistoricalDetailError::InvalidVocabulary),
    };
    let attempt = p.string("attempt")?;
    require_one(&attempt, &["attempted", "not-attempted"])?;
    let nar = p.optional_string("not-attempted-reason")?;
    let terminal = p.optional_string("terminal-outcome")?;
    if let Some(ref t) = terminal {
        index(t, &TERMINALS)?;
    }
    let attempt_projection = match (attempt.as_str(), nar.as_deref(), terminal.as_deref()) {
        ("attempted", None, Some(terminal)) => {
            AggregateAttemptProjection::Attempted(parse_terminal(terminal)?)
        }
        ("not-attempted", Some(reason), None) => {
            AggregateAttemptProjection::NotAttempted(parse_not_attempted_reason(reason)?)
        }
        _ => return Err(HistoricalDetailError::InvalidState),
    };
    let expected_policy = super::policy::historical_policy(
        eligibility,
        selection_projection,
        attempt_projection,
        expectation,
    )
    .ok_or(HistoricalDetailError::InvalidState)?;
    let policy = parse_policy(&p.string("derived-policy")?)?;
    if policy != expected_policy {
        return Err(HistoricalDetailError::InvalidState);
    }
    p.marker("END execution-variant")?;
    let end = p.position();
    Ok(HistoricalVariant {
        key: HistoricalVariantKey {
            test_id: copy_string(test_id)?,
            observation: copy_string(observation)?,
            variant,
        },
        canonical_record: copy_bytes(p.slice(start, end)?)?,
        comparison,
        selection: selection_projection,
        attempt: attempt_projection,
    })
}

fn validate_declared(
    a: &Declared,
    b: &FixedAggregateAccountingProjection,
) -> Result<(), HistoricalDetailError> {
    let logical = [
        b.logical.total_tests,
        b.logical.pass_count,
        b.logical.fail_count,
        b.logical.expected_fail_count,
        b.logical.unsupported_count,
        b.logical.skipped_count,
        b.logical.flaky_count,
        b.logical.unclassified_count,
    ];
    let variants = [
        b.variants.materialized_variants,
        b.variants.runnable_variants,
        b.variants.not_runnable_variants,
        b.variants.eligibility_not_established_variants,
        b.variants.selected_variants,
        b.variants.excluded_variants,
        b.variants.selection_not_applicable_variants,
        b.variants.attempted_variants,
        b.variants.not_attempted_variants,
    ];
    let owners = b.owners;
    let surfaces = b.surfaces;
    let comparisons = b.comparisons;
    let terminals = terminal_counts(b);
    if a.logical != logical
        || a.variants != variants
        || a.owners != owners
        || a.surfaces != surfaces
        || a.comparisons != comparisons
        || a.terminals != terminals
    {
        return Err(HistoricalDetailError::AccountingMismatch);
    }
    crate::aggregate::accounting::validate_fixed_accounting(b).map_err(map_accounting_error)
}

fn terminal_counts(accounting: &FixedAggregateAccountingProjection) -> [u64; 7] {
    [
        accounting.terminals.semantic_pass,
        accounting.terminals.semantic_fail,
        accounting.terminals.execution_failure,
        accounting.terminals.resource_failure,
        accounting.terminals.incomplete_observation,
        accounting.terminals.invariant_failure,
        accounting.terminals.timeout,
    ]
}

fn map_accounting_error(
    error: crate::aggregate::accounting::AccountingError,
) -> HistoricalDetailError {
    match error {
        crate::aggregate::accounting::AccountingError::Overflow => HistoricalDetailError::Overflow,
        crate::aggregate::accounting::AccountingError::Invariant(_) => {
            HistoricalDetailError::AccountingMismatch
        }
    }
}

fn parse_comparison(
    comparison: &str,
    reference_kind: Option<&str>,
    relation: Option<&str>,
) -> Result<AggregateComparisonKind, HistoricalDetailError> {
    Ok(match (comparison, reference_kind, relation) {
        ("authored-expected-observation", None, None) => {
            AggregateComparisonKind::AuthoredExpectedObservation
        }
        ("static-document-reference", Some(kind), Some(relation)) => {
            AggregateComparisonKind::StaticDocumentReference {
                reference_kind: match kind {
                    "semantic" => ReferenceKind::Semantic,
                    "structural" => ReferenceKind::Structural,
                    _ => return Err(HistoricalDetailError::InvalidVocabulary),
                },
                relation: match relation {
                    "match" => ReferenceRelation::Match,
                    "mismatch" => ReferenceRelation::Mismatch,
                    _ => return Err(HistoricalDetailError::InvalidVocabulary),
                },
            }
        }
        _ => return Err(HistoricalDetailError::InvalidState),
    })
}

fn parse_terminal(value: &str) -> Result<AggregateTerminalOutcome, HistoricalDetailError> {
    Ok(match value {
        "semantic-pass" => AggregateTerminalOutcome::SemanticPass,
        "semantic-fail" => AggregateTerminalOutcome::SemanticFail,
        "execution-failure" => AggregateTerminalOutcome::ExecutionFailure,
        "resource-failure" => AggregateTerminalOutcome::ResourceFailure,
        "incomplete-observation" => AggregateTerminalOutcome::IncompleteObservation,
        "invariant-failure" => AggregateTerminalOutcome::InvariantFailure,
        "timeout" => AggregateTerminalOutcome::Timeout,
        _ => return Err(HistoricalDetailError::InvalidVocabulary),
    })
}

fn parse_not_attempted_reason(
    value: &str,
) -> Result<AggregateNotAttemptedReason, HistoricalDetailError> {
    Ok(match value {
        "eligibility" => AggregateNotAttemptedReason::Eligibility,
        "lane-excluded" => AggregateNotAttemptedReason::LaneExcluded,
        "parser-pre-attempt-evaluation" => AggregateNotAttemptedReason::ParserPreAttemptEvaluation,
        "css-fragment-capability-unavailable" => {
            AggregateNotAttemptedReason::CssFragmentCapabilityUnavailable
        }
        _ => return Err(HistoricalDetailError::InvalidVocabulary),
    })
}

fn parse_policy(value: &str) -> Result<DerivedPolicyResult, HistoricalDetailError> {
    Ok(match value {
        "expected-pass" => DerivedPolicyResult::ExpectedPass,
        "unexpected-fail" => DerivedPolicyResult::UnexpectedFail,
        "xfail" => DerivedPolicyResult::ExpectedFail,
        "xpass" => DerivedPolicyResult::UnexpectedPass,
        "not-run" => DerivedPolicyResult::NotRun,
        "not-yet-established" => DerivedPolicyResult::NotYetEstablished,
        "unexpected-outcome" => DerivedPolicyResult::UnexpectedOutcome,
        _ => return Err(HistoricalDetailError::InvalidVocabulary),
    })
}
fn parse_simple_record(
    p: &mut Parser<'_>,
    name: &str,
    fields: &[&str],
) -> Result<Vec<Option<String>>, HistoricalDetailError> {
    let (begin, end) = match name {
        "capability-missing" => ("BEGIN capability-missing", "END capability-missing"),
        "harness-limitation" => ("BEGIN harness-limitation", "END harness-limitation"),
        "environment-requirement" => (
            "BEGIN environment-requirement",
            "END environment-requirement",
        ),
        _ => return Err(HistoricalDetailError::InvalidState),
    };
    p.marker(begin)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(fields.len())
        .map_err(|_| HistoricalDetailError::Allocation)?;
    for f in fields {
        let value = if *f == "feature" {
            p.optional_string(f)?
        } else {
            Some(p.present_string(f)?)
        };
        values.push(value);
    }
    p.marker(end)?;
    Ok(values)
}

type EligibilityFactKey = (u8, String, String, String, String);

fn parse_eligibility_facts(
    p: &mut Parser<'_>,
    role: &str,
    count: u64,
) -> Result<(), HistoricalDetailError> {
    let mut previous: Option<EligibilityFactKey> = None;
    for _ in 0..count {
        let key = parse_eligibility_fact(p, role)?;
        if previous.as_ref().is_some_and(|value| value >= &key) {
            return Err(HistoricalDetailError::NonCanonicalOrder);
        }
        previous = Some(key);
    }
    Ok(())
}

fn parse_eligibility_fact(
    p: &mut Parser<'_>,
    role: &str,
) -> Result<EligibilityFactKey, HistoricalDetailError> {
    p.marker("BEGIN eligibility-fact")?;
    p.exact_string("role", role)?;
    let kind = p.string("kind")?;
    let key = match kind.as_str() {
        "engine-capability" => {
            let capability = p.present_string("capability-kind")?;
            let parsed = EngineCapabilityKind::parse(&capability)
                .ok_or(HistoricalDetailError::InvalidVocabulary)?;
            let feature = p.optional_string("feature")?;
            if feature
                .as_deref()
                .is_some_and(|value| !CapabilityFeatureId::is_valid(value))
                || parsed.requires_feature() != feature.is_some()
            {
                return Err(HistoricalDetailError::InvalidState);
            }
            let reason = p.present_string("reason")?;
            (
                0,
                capability,
                feature.unwrap_or_default(),
                reason,
                String::new(),
            )
        }
        "harness" => {
            let harness = p.present_string("harness-kind")?;
            if HarnessLimitationKind::parse(&harness).is_none() {
                return Err(HistoricalDetailError::InvalidVocabulary);
            }
            let reason = p.present_string("reason")?;
            (1, harness, reason, String::new(), String::new())
        }
        "environment" => {
            let environment = p.present_string("environment-kind")?;
            if EnvironmentRequirementKind::parse(&environment).is_none() {
                return Err(HistoricalDetailError::InvalidVocabulary);
            }
            let profile = p.present_string("profile")?;
            if !EnvironmentProfileId::is_valid(&profile) {
                return Err(HistoricalDetailError::InvalidValue);
            }
            let requirement_reason = p.present_string("requirement-reason")?;
            let assessment_reason = p.present_string("assessment-reason")?;
            (
                2,
                environment,
                profile,
                requirement_reason,
                assessment_reason,
            )
        }
        "classification" => (
            3,
            p.present_string("reason")?,
            String::new(),
            String::new(),
            String::new(),
        ),
        "engine-capability-availability" => (
            4,
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ),
        "harness-readiness" => (
            5,
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ),
        "environment-requirement" => {
            let environment = p.present_string("environment-kind")?;
            if EnvironmentRequirementKind::parse(&environment).is_none() {
                return Err(HistoricalDetailError::InvalidVocabulary);
            }
            let profile = p.present_string("profile")?;
            if !EnvironmentProfileId::is_valid(&profile) {
                return Err(HistoricalDetailError::InvalidValue);
            }
            let reason = p.present_string("reason")?;
            (6, environment, profile, reason, String::new())
        }
        _ => return Err(HistoricalDetailError::InvalidVocabulary),
    };
    p.marker("END eligibility-fact")?;
    Ok(key)
}
fn index<const N: usize>(v: &str, values: &[&str; N]) -> Result<usize, HistoricalDetailError> {
    values
        .iter()
        .position(|x| *x == v)
        .ok_or(HistoricalDetailError::InvalidVocabulary)
}
fn require_one(v: &str, values: &[&str]) -> Result<(), HistoricalDetailError> {
    values
        .contains(&v)
        .then_some(())
        .ok_or(HistoricalDetailError::InvalidVocabulary)
}
fn lane_rank(v: &str) -> u8 {
    match v {
        "normal-ci" => 0,
        "local-extended" => 1,
        "scheduled-extended" => 2,
        _ => 3,
    }
}
struct Parser<'a> {
    text: &'a str,
    offset: usize,
}
impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Result<Self, HistoricalDetailError> {
        if !text.ends_with('\n') {
            return Err(HistoricalDetailError::InvalidFraming);
        }
        Ok(Self { text, offset: 0 })
    }
    fn position(&self) -> usize {
        self.offset
    }
    fn slice(&self, s: usize, e: usize) -> Result<&[u8], HistoricalDetailError> {
        self.text
            .as_bytes()
            .get(s..e)
            .ok_or(HistoricalDetailError::InvalidFraming)
    }
    fn line(&mut self) -> Result<&'a str, HistoricalDetailError> {
        let remainder = self
            .text
            .as_bytes()
            .get(self.offset..)
            .ok_or(HistoricalDetailError::InvalidFraming)?;
        let relative_end = remainder
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or(HistoricalDetailError::InvalidFraming)?;
        let start = self.offset;
        let end = start
            .checked_add(relative_end)
            .ok_or(HistoricalDetailError::Overflow)?;
        self.offset = end.checked_add(1).ok_or(HistoricalDetailError::Overflow)?;
        self.text
            .get(start..end)
            .ok_or(HistoricalDetailError::InvalidFraming)
    }
    fn marker_start(&mut self, m: &str) -> Result<usize, HistoricalDetailError> {
        let s = self.position();
        self.marker(m)?;
        Ok(s)
    }
    fn marker(&mut self, m: &str) -> Result<(), HistoricalDetailError> {
        (self.line()? == m)
            .then_some(())
            .ok_or(HistoricalDetailError::InvalidFraming)
    }
    fn raw_value(&mut self, key: &str) -> Result<&'a str, HistoricalDetailError> {
        let l = self.line()?;
        l.strip_prefix(key)
            .and_then(|v| v.strip_prefix(" = "))
            .ok_or(HistoricalDetailError::InvalidFraming)
    }
    fn string(&mut self, key: &str) -> Result<String, HistoricalDetailError> {
        decode_string(self.raw_value(key)?)
    }
    fn optional_string(&mut self, key: &str) -> Result<Option<String>, HistoricalDetailError> {
        let v = self.raw_value(key)?;
        if v == "null" {
            Ok(None)
        } else {
            Ok(Some(decode_string(v)?))
        }
    }
    fn require_present_string(&mut self, key: &str) -> Result<(), HistoricalDetailError> {
        self.present_string(key).map(|_| ())
    }
    fn present_string(&mut self, key: &str) -> Result<String, HistoricalDetailError> {
        let value = self.string(key)?;
        if value.is_empty() {
            return Err(HistoricalDetailError::InvalidValue);
        }
        Ok(value)
    }
    fn require_null(&mut self, key: &str) -> Result<(), HistoricalDetailError> {
        (self.raw_value(key)? == "null")
            .then_some(())
            .ok_or(HistoricalDetailError::InvalidValue)
    }
    fn number(&mut self, key: &str) -> Result<u64, HistoricalDetailError> {
        parse_number(self.raw_value(key)?)
    }
    fn optional_number(&mut self, key: &str) -> Result<Option<u64>, HistoricalDetailError> {
        let v = self.raw_value(key)?;
        if v == "null" {
            Ok(None)
        } else {
            Ok(Some(parse_number(v)?))
        }
    }
    fn digest(&mut self, key: &str) -> Result<Sha256Digest, HistoricalDetailError> {
        let v = self.string(key)?;
        let h = v
            .strip_prefix("sha256:")
            .ok_or(HistoricalDetailError::InvalidValue)?;
        Sha256Digest::parse(h).map_err(|_| HistoricalDetailError::InvalidValue)
    }
    fn exact_string(&mut self, key: &str, e: &str) -> Result<(), HistoricalDetailError> {
        let v = self.string(key)?;
        self.exact_string_value(&v, e)
    }
    fn exact_string_value(&self, v: &str, e: &str) -> Result<(), HistoricalDetailError> {
        (v == e)
            .then_some(())
            .ok_or(HistoricalDetailError::InvalidVocabulary)
    }
    fn exact_optional_string(
        &mut self,
        key: &str,
        e: Option<&str>,
    ) -> Result<(), HistoricalDetailError> {
        (self.optional_string(key)?.as_deref() == e)
            .then_some(())
            .ok_or(HistoricalDetailError::InvalidVocabulary)
    }
    fn exact_number(&mut self, key: &str, e: u64) -> Result<(), HistoricalDetailError> {
        (self.number(key)? == e)
            .then_some(())
            .ok_or(HistoricalDetailError::InvalidValue)
    }
    fn exact_bool(&mut self, key: &str, e: bool) -> Result<(), HistoricalDetailError> {
        (self.raw_value(key)? == if e { "true" } else { "false" })
            .then_some(())
            .ok_or(HistoricalDetailError::InvalidValue)
    }
    fn sorted_requirements(&mut self, key: &str) -> Result<(), HistoricalDetailError> {
        let v = self.raw_value(key)?;
        if !v.starts_with('[') || !v.ends_with(']') {
            return Err(HistoricalDetailError::InvalidValue);
        }
        let inner = &v[1..v.len() - 1];
        if inner.is_empty() {
            return Ok(());
        }
        let mut out = Vec::new();
        for item in inner.split(", ") {
            out.try_reserve(1)
                .map_err(|_| HistoricalDetailError::Allocation)?;
            let requirement = decode_string(item)?;
            if RequirementTag::parse(&requirement).is_none() {
                return Err(HistoricalDetailError::InvalidVocabulary);
            }
            out.push(requirement);
        }
        if out.windows(2).any(|w| w[0].as_bytes() >= w[1].as_bytes()) {
            return Err(HistoricalDetailError::NonCanonicalOrder);
        }
        Ok(())
    }
    fn finish(&self) -> Result<(), HistoricalDetailError> {
        (self.offset == self.text.len())
            .then_some(())
            .ok_or(HistoricalDetailError::InvalidFraming)
    }
}
fn parse_number(v: &str) -> Result<u64, HistoricalDetailError> {
    if v.is_empty() || (v.len() > 1 && v.starts_with('0')) || !v.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(HistoricalDetailError::InvalidValue);
    }
    v.parse().map_err(|_| HistoricalDetailError::Overflow)
}
fn decode_string(v: &str) -> Result<String, HistoricalDetailError> {
    if !v.starts_with('"') || !v.ends_with('"') {
        return Err(HistoricalDetailError::InvalidValue);
    }
    let inner = &v[1..v.len() - 1];
    let mut out = String::new();
    out.try_reserve(inner.len())
        .map_err(|_| HistoricalDetailError::Allocation)?;
    let mut c = inner.chars();
    while let Some(ch) = c.next() {
        if ch == '\\' {
            match c.next().ok_or(HistoricalDetailError::InvalidValue)? {
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'u' => {
                    if c.next() != Some('{') {
                        return Err(HistoricalDetailError::InvalidValue);
                    }
                    let mut value = 0_u32;
                    let mut digits = 0_usize;
                    let mut leading_zero = false;
                    loop {
                        let h = c.next().ok_or(HistoricalDetailError::InvalidValue)?;
                        if h == '}' {
                            break;
                        }
                        if !h.is_ascii_hexdigit() || h.is_ascii_lowercase() || digits >= 6 {
                            return Err(HistoricalDetailError::InvalidValue);
                        }
                        if digits == 0 {
                            leading_zero = h == '0';
                        }
                        value = value
                            .checked_mul(16)
                            .and_then(|value| value.checked_add(h.to_digit(16)?))
                            .ok_or(HistoricalDetailError::InvalidValue)?;
                        digits += 1;
                    }
                    if digits == 0 || digits > 1 && leading_zero {
                        return Err(HistoricalDetailError::InvalidValue);
                    }
                    if value > 0x1f || matches!(value, 9 | 10 | 13) {
                        return Err(HistoricalDetailError::InvalidValue);
                    }
                    out.push(char::from_u32(value).ok_or(HistoricalDetailError::InvalidValue)?);
                }
                _ => return Err(HistoricalDetailError::InvalidValue),
            }
        } else {
            if ch == '"' || ch.is_control() {
                return Err(HistoricalDetailError::InvalidValue);
            }
            out.push(ch)
        }
    }
    Ok(out)
}
fn copy_string(value: &str) -> Result<String, HistoricalDetailError> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| HistoricalDetailError::Allocation)?;
    owned.push_str(value);
    Ok(owned)
}
fn copy_bytes(value: &[u8]) -> Result<Vec<u8>, HistoricalDetailError> {
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| HistoricalDetailError::Allocation)?;
    owned.extend_from_slice(value);
    Ok(owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repository_golden_decodes_and_rejects_mutation() {
        let b = include_bytes!("../../../tests/data/aggregate-detail-v1.txt");
        let d = decode_aggregate_detail_v1(b).unwrap();
        assert_eq!(d.cases.len(), 25);
        assert_eq!(d.cases.iter().map(|c| c.variants.len()).sum::<usize>(), 25);
        let mut bad = b.to_vec();
        let p = std::str::from_utf8(&bad)
            .unwrap()
            .find("total = 25")
            .unwrap()
            + 8;
        bad[p] = b'4';
        assert_eq!(
            decode_aggregate_detail_v1(&bad).unwrap_err(),
            HistoricalDetailError::AccountingMismatch
        );
    }
    #[test]
    fn malformed_framing_and_member_digest_fail_closed() {
        let b = include_bytes!("../../../tests/data/aggregate-detail-v1.txt");
        assert_eq!(
            decode_aggregate_detail_v1(&b[..b.len() - 1]).unwrap_err(),
            HistoricalDetailError::InvalidFraming
        );
        let mut bad = b.to_vec();
        let p = std::str::from_utf8(&bad).unwrap().find("fc500a").unwrap();
        bad[p] = b'0';
        assert_eq!(
            decode_aggregate_detail_v1(&bad).unwrap_err(),
            HistoricalDetailError::MemberDigestMismatch
        );
    }

    #[test]
    fn closed_metadata_vocabulary_and_canonical_requirement_order_are_enforced() {
        let bytes = include_bytes!("../../../tests/data/aggregate-detail-v1.txt");
        let text = std::str::from_utf8(bytes).unwrap();

        let unknown = text.replacen("\"no-js\"", "\"unknown-requirement\"", 1);
        assert_eq!(
            decode_aggregate_detail_v1(unknown.as_bytes()).unwrap_err(),
            HistoricalDetailError::InvalidVocabulary
        );

        let reordered = text.replacen(
            "[\"no-js\", \"requires-css-feature\", \"requires-html-parser-feature\"]",
            "[\"requires-css-feature\", \"no-js\", \"requires-html-parser-feature\"]",
            1,
        );
        assert_eq!(
            decode_aggregate_detail_v1(reordered.as_bytes()).unwrap_err(),
            HistoricalDetailError::NonCanonicalOrder
        );

        let wrong_variant_domain = text
            .replacen(
                "variant-kind = \"singleton\"",
                "variant-kind = \"rendering\"",
                1,
            )
            .replacen(
                "rendering-environment = null",
                "rendering-environment = \"synthetic-text-metrics-v1\"",
                1,
            )
            .replacen(
                "available-width-css-px = null",
                "available-width-css-px = 1",
                1,
            );
        assert_eq!(
            decode_aggregate_detail_v1(wrong_variant_domain.as_bytes()).unwrap_err(),
            HistoricalDetailError::InvalidState
        );
    }

    #[test]
    fn duplicate_logical_and_variant_identities_fail_before_accounting_can_hide_them() {
        let bytes = include_bytes!("../../../tests/data/aggregate-detail-v1.txt");
        let text = std::str::from_utf8(bytes).unwrap();

        let last_case_start = text.rfind("BEGIN logical-case\n").unwrap();
        let last_case_end = text[last_case_start..]
            .find("END logical-case\n")
            .map(|offset| last_case_start + offset + "END logical-case\n".len())
            .unwrap();
        let mut duplicate_case = text.replacen(
            "logical-case-detail-count = 25",
            "logical-case-detail-count = 26",
            1,
        );
        duplicate_case.push_str(&text[last_case_start..last_case_end]);
        assert_eq!(
            decode_aggregate_detail_v1(duplicate_case.as_bytes()).unwrap_err(),
            HistoricalDetailError::DuplicateIdentity
        );

        let count = text.find("execution-variant-count = 1\n").unwrap();
        let variant_start = text[count..]
            .find("BEGIN execution-variant\n")
            .map(|offset| count + offset)
            .unwrap();
        let variant_end = text[variant_start..]
            .find("END execution-variant\n")
            .map(|offset| variant_start + offset + "END execution-variant\n".len())
            .unwrap();
        let mut duplicate_variant = text.replacen(
            "execution-variant-count = 1",
            "execution-variant-count = 2",
            1,
        );
        duplicate_variant.insert_str(variant_end, &text[variant_start..variant_end]);
        assert_eq!(
            decode_aggregate_detail_v1(duplicate_variant.as_bytes()).unwrap_err(),
            HistoricalDetailError::DuplicateIdentity
        );
    }

    #[test]
    fn newline_dense_transport_does_not_allocate_a_per_line_index() {
        assert_eq!(
            std::mem::size_of::<Parser<'_>>(),
            std::mem::size_of::<&str>() + std::mem::size_of::<usize>()
        );
        let dense = vec![b'\n'; super::super::super::report::AGGREGATE_DETAIL_MAX_BYTES_V1];
        assert_eq!(
            decode_aggregate_detail_v1(&dense).unwrap_err(),
            HistoricalDetailError::InvalidFraming
        );
        let over = vec![b'\n'; super::super::super::report::AGGREGATE_DETAIL_MAX_BYTES_V1 + 1];
        assert_eq!(
            decode_aggregate_detail_v1(&over).unwrap_err(),
            HistoricalDetailError::InvalidFraming
        );
    }
}
