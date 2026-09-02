use std::fmt::Write;

use css::{
    CssWideResolvedSource, ParseOptions, PropertyId, PropertyNameKind, ResolvedValueSource, Rule,
    RuleCollection, SelectorListParseResult, SelectorMatchingEnvironment, SelectorMatchingLimits,
    ShorthandId, SpecifiedValueLimits, StyleProjection, StyleProjectionBuildError,
    StyleProjectionMatchError, StyleResolutionExecution, StyleResolutionLimits,
    StylesheetCollectionInput, StylesheetConditionInput, StylesheetOrder, StylesheetSourceId,
    parse_selector_source_with_limits, parse_specified_declaration_value_with_limits,
    parse_stylesheet_with_options,
};
use html::{
    HtmlErrorPolicy, HtmlParseError, HtmlParseOptions, HtmlParseSemanticCompleteness,
    HtmlParseSemanticDegradations, HtmlTokenizerLimits, HtmlTokenizerOptions,
    HtmlTreeBuilderLimits, HtmlTreeBuilderOptions,
};

use crate::fixture::{
    CssExecutionProfile, CssFixturePackage, CssHtmlInputKind, CssStylesheetOrigin,
};
use crate::target::{CssTargetResolutionFailure, resolve_target};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssExecutionPhase {
    HtmlDocumentParsing,
    TargetResolution,
    CssModelParsing,
    SelectorParsing,
    SelectorProjection,
    SelectorMatching,
    RuleCollection,
    Cascade,
    ResolvedStyleObservation,
    ComputedStyle,
    ObservationSerialization,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssExecutionFailure {
    HtmlParser(HtmlParseError),
    HtmlSemanticInputResourceLimited(HtmlParseSemanticDegradations),
    TargetResolution {
        label: String,
        failure: CssTargetResolutionFailure,
    },
    ResourceLimit {
        resource: CssExecutionResourceLimit,
    },
    SelectorProjection(StyleProjectionBuildError),
    SelectorMatching(StyleProjectionMatchError),
    RuleCollection(css::RuleCollectionBuildError),
    StyleResolution(css::StyleResolutionError),
    ProjectionArtifact(css::StyleProjectionArtifactError),
    ComputedMaterialization(css::ComputedStyleResolutionError),
    RequiredObservation(CssRequiredObservationFailure),
    StorageAllocation {
        storage: CssExecutionStorage,
    },
    ObservationLimitExceeded {
        actual: usize,
        maximum: usize,
    },
    ObservationAllocationFailure,
}

impl CssExecutionFailure {
    pub const fn stable_identity(&self) -> Option<&'static str> {
        match self {
            Self::HtmlParser(error) => Some(error.stable_label()),
            Self::SelectorProjection(error) => Some(error.stable_label()),
            Self::SelectorMatching(error) => Some(error.stable_label()),
            Self::RuleCollection(error) => Some(error.stable_label()),
            Self::StyleResolution(error) => Some(error.stable_label()),
            Self::ProjectionArtifact(error) => Some(error.stable_label()),
            Self::ComputedMaterialization(error) => Some(error.stable_label()),
            Self::RequiredObservation(failure) => Some(failure.stable_label()),
            Self::HtmlSemanticInputResourceLimited(_)
            | Self::TargetResolution { .. }
            | Self::ResourceLimit { .. }
            | Self::StorageAllocation { .. }
            | Self::ObservationLimitExceeded { .. }
            | Self::ObservationAllocationFailure => None,
        }
    }
}

/// CSS-owned classification used by subsystem-neutral conformance accounting.
///
/// This projection is deliberately closed over the typed failure hierarchy.
/// It does not replace the lossless failure and must not be inferred from a
/// diagnostic or stable label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssExecutionFailureClass {
    ResourceFailure,
    OtherExecutionFailure,
}

pub const fn classify_execution_failure(failure: &CssExecutionFailure) -> CssExecutionFailureClass {
    use CssExecutionFailureClass::{OtherExecutionFailure, ResourceFailure};

    match failure {
        CssExecutionFailure::HtmlParser(HtmlParseError::Fatal(error))
            if error.is_resource_exhaustion() =>
        {
            ResourceFailure
        }
        CssExecutionFailure::HtmlParser(_) => OtherExecutionFailure,
        CssExecutionFailure::HtmlSemanticInputResourceLimited(_)
        | CssExecutionFailure::ResourceLimit { .. }
        | CssExecutionFailure::StorageAllocation { .. } => ResourceFailure,
        CssExecutionFailure::SelectorProjection(error) => classify_projection_build_failure(error),
        CssExecutionFailure::SelectorMatching(error) => classify_projection_match_failure(error),
        CssExecutionFailure::RuleCollection(error) => classify_rule_collection_failure(error),
        CssExecutionFailure::StyleResolution(error) => classify_style_resolution_failure(error),
        CssExecutionFailure::ComputedMaterialization(error) => {
            classify_computed_style_failure(error)
        }
        CssExecutionFailure::TargetResolution { .. }
        | CssExecutionFailure::ProjectionArtifact(_)
        | CssExecutionFailure::RequiredObservation(_)
        | CssExecutionFailure::ObservationLimitExceeded { .. }
        | CssExecutionFailure::ObservationAllocationFailure => OtherExecutionFailure,
    }
}

const fn classify_projection_build_failure(
    failure: &StyleProjectionBuildError,
) -> CssExecutionFailureClass {
    match failure {
        StyleProjectionBuildError::SelectorDom(failure) => classify_selector_dom_failure(failure),
        StyleProjectionBuildError::ElementLimitExceeded { .. } => {
            CssExecutionFailureClass::ResourceFailure
        }
    }
}

const fn classify_projection_match_failure(
    failure: &StyleProjectionMatchError,
) -> CssExecutionFailureClass {
    match failure {
        StyleProjectionMatchError::ProjectionKey(_) => {
            CssExecutionFailureClass::OtherExecutionFailure
        }
        StyleProjectionMatchError::Matching(
            css::SelectorMatchingLimitError::AxisStepLimitExceeded { .. },
        ) => CssExecutionFailureClass::ResourceFailure,
    }
}

const fn classify_selector_dom_failure(
    failure: &css::SelectorDomBuildError,
) -> CssExecutionFailureClass {
    match failure {
        css::SelectorDomBuildError::ElementIdRepresentationExhausted { .. }
        | css::SelectorDomBuildError::ProjectionCapacityExceeded { .. }
        | css::SelectorDomBuildError::StorageReservationFailed { .. } => {
            CssExecutionFailureClass::ResourceFailure
        }
        css::SelectorDomBuildError::InvalidDocumentRoot { .. }
        | css::SelectorDomBuildError::NestedDocument { .. }
        | css::SelectorDomBuildError::MultipleDocumentElements { .. }
        | css::SelectorDomBuildError::NonCanonicalHtmlElementLocalName { .. } => {
            CssExecutionFailureClass::OtherExecutionFailure
        }
    }
}

pub const fn classify_rule_collection_failure(
    failure: &css::RuleCollectionBuildError,
) -> CssExecutionFailureClass {
    match failure {
        css::RuleCollectionBuildError::UnsupportedConfiguration { .. }
        | css::RuleCollectionBuildError::LimitExceeded { .. }
        | css::RuleCollectionBuildError::Reservation { .. } => {
            CssExecutionFailureClass::ResourceFailure
        }
        css::RuleCollectionBuildError::DuplicateSourceId { .. }
        | css::RuleCollectionBuildError::DuplicateStylesheetOrder { .. }
        | css::RuleCollectionBuildError::NonMonotonicStylesheetOrder { .. }
        | css::RuleCollectionBuildError::SelectorStateInvariant { .. }
        | css::RuleCollectionBuildError::Coordinate(_) => {
            CssExecutionFailureClass::OtherExecutionFailure
        }
    }
}

pub const fn classify_style_resolution_failure(
    failure: &css::StyleResolutionError,
) -> CssExecutionFailureClass {
    match failure {
        css::StyleResolutionError::SelectorDomBuild(failure) => {
            classify_selector_dom_failure(failure)
        }
        css::StyleResolutionError::LimitExceeded { .. }
        | css::StyleResolutionError::UnsupportedConfiguration { .. }
        | css::StyleResolutionError::SelectorMatching(
            css::SelectorMatchingLimitError::AxisStepLimitExceeded { .. },
        ) => CssExecutionFailureClass::ResourceFailure,
        css::StyleResolutionError::StylesheetInputBuild(failure) => match failure {
            css::StylesheetCollectionInputBuildError::Reservation => {
                CssExecutionFailureClass::ResourceFailure
            }
            css::StylesheetCollectionInputBuildError::Coordinate(_)
            | css::StylesheetCollectionInputBuildError::SourceIdentity(_) => {
                CssExecutionFailureClass::OtherExecutionFailure
            }
        },
        css::StyleResolutionError::RuleCollectionBuild(failure) => {
            classify_rule_collection_failure(failure)
        }
        css::StyleResolutionError::CascadeResolution(failure) => {
            classify_cascade_resolution_failure(failure)
        }
        css::StyleResolutionError::MatchingEnvironmentMismatch { .. }
        | css::StyleResolutionError::RuleInputBuild(_)
        | css::StyleResolutionError::SourceCoordinate(_) => {
            CssExecutionFailureClass::OtherExecutionFailure
        }
    }
}

const fn classify_cascade_resolution_failure(
    failure: &css::CascadeResolutionError,
) -> CssExecutionFailureClass {
    match failure {
        css::CascadeResolutionError::CandidateCeilingOverflow { .. }
        | css::CascadeResolutionError::RuleInputCeilingOverflow { .. }
        | css::CascadeResolutionError::UnsupportedLocatorLimit { .. }
        | css::CascadeResolutionError::CandidateLimitExceeded { .. }
        | css::CascadeResolutionError::WinnerWorkspaceReservationFailed { .. }
        | css::CascadeResolutionError::WinnerOutputReservationFailed { .. }
        | css::CascadeResolutionError::RuleInputStorageReservationFailed { .. } => {
            CssExecutionFailureClass::ResourceFailure
        }
        css::CascadeResolutionError::RuleInputSequenceInvariant { .. }
        | css::CascadeResolutionError::DeclarationSourceOrderInvariant { .. }
        | css::CascadeResolutionError::DuplicateCandidateIdentity { .. }
        | css::CascadeResolutionError::InconsistentCandidateIdentity { .. }
        | css::CascadeResolutionError::EqualPriorityDistinctCandidates { .. } => {
            CssExecutionFailureClass::OtherExecutionFailure
        }
    }
}

pub const fn classify_computed_style_failure(
    failure: &css::ComputedStyleResolutionError,
) -> CssExecutionFailureClass {
    match failure {
        css::ComputedStyleResolutionError::SelectorDomBuild(failure) => {
            classify_selector_dom_failure(failure)
        }
        css::ComputedStyleResolutionError::StyleResolution(failure) => {
            classify_style_resolution_failure(failure)
        }
        css::ComputedStyleResolutionError::ProjectionSourceRootMismatch
        | css::ComputedStyleResolutionError::ProjectionShapeMismatch { .. }
        | css::ComputedStyleResolutionError::MissingMatchingEnvironment
        | css::ComputedStyleResolutionError::MatchingEnvironmentMismatch { .. }
        | css::ComputedStyleResolutionError::MissingResolvedElement { .. }
        | css::ComputedStyleResolutionError::ResolvedElementNameMismatch { .. }
        | css::ComputedStyleResolutionError::ResolvedElementNamespaceMismatch { .. }
        | css::ComputedStyleResolutionError::MissingComputedParent { .. }
        | css::ComputedStyleResolutionError::MissingComputedElementStyle { .. }
        | css::ComputedStyleResolutionError::ComputedElementNameMismatch { .. }
        | css::ComputedStyleResolutionError::ComputedElementNamespaceMismatch { .. }
        | css::ComputedStyleResolutionError::ComputedElementIdentityMismatch { .. }
        | css::ComputedStyleResolutionError::ExtraComputedElementStyle { .. }
        | css::ComputedStyleResolutionError::MissingResolvedProperty { .. }
        | css::ComputedStyleResolutionError::MissingInheritedParent { .. }
        | css::ComputedStyleResolutionError::NonInheritedPropertyMarkedInherited { .. }
        | css::ComputedStyleResolutionError::InitialValueMismatch { .. }
        | css::ComputedStyleResolutionError::WinnerMissingSpecifiedValue { .. }
        | css::ComputedStyleResolutionError::WinnerPropertyMismatch { .. }
        | css::ComputedStyleResolutionError::Normalization(_)
        | css::ComputedStyleResolutionError::Build(_) => {
            CssExecutionFailureClass::OtherExecutionFailure
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssExecutionStorage {
    ParsedStylesheets,
    StylesheetInputs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssRequiredObservationFailure {
    PropertyRuleCoordinateMissing,
    PropertyCoordinateIsAtRule,
    PropertyDeclarationCoordinateMissing,
    PropertyNameUnresolved,
    SelectorSnapshotFormattingInvariant,
    TargetMissingFromProjection,
    TargetMissingFromStyleProjection,
    ComputedTargetMissing,
    ResolvedTargetMissing,
    SelectedPropertyUnsupported,
    SelectedResolvedPropertyMissing,
    ObservationFormattingInvariant,
}

impl CssRequiredObservationFailure {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::PropertyRuleCoordinateMissing => "property-rule-coordinate-missing",
            Self::PropertyCoordinateIsAtRule => "property-coordinate-is-at-rule",
            Self::PropertyDeclarationCoordinateMissing => "property-declaration-coordinate-missing",
            Self::PropertyNameUnresolved => "property-name-unresolved",
            Self::SelectorSnapshotFormattingInvariant => "selector-snapshot-formatting-invariant",
            Self::TargetMissingFromProjection => "target-missing-from-projection",
            Self::TargetMissingFromStyleProjection => "target-missing-from-style-projection",
            Self::ComputedTargetMissing => "computed-target-missing",
            Self::ResolvedTargetMissing => "resolved-target-missing",
            Self::SelectedPropertyUnsupported => "selected-property-unsupported",
            Self::SelectedResolvedPropertyMissing => "selected-resolved-property-missing",
            Self::ObservationFormattingInvariant => "observation-formatting-invariant",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssExecutionResourceLimit {
    StylesheetModelParsing,
    SelectorParsing,
    SpecifiedValueParsing,
    ShorthandExpansion,
}

impl CssExecutionResourceLimit {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::StylesheetModelParsing => "stylesheet-model-parsing",
            Self::SelectorParsing => "selector-parsing",
            Self::SpecifiedValueParsing => "specified-value-parsing",
            Self::ShorthandExpansion => "shorthand-expansion",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssObservedExecutionOutcome {
    SemanticPass,
    ExpectationMismatch {
        difference: String,
    },
    ExecutionFailure {
        phase: CssExecutionPhase,
        failure: CssExecutionFailure,
    },
    IncompleteObservation {
        phase: CssExecutionPhase,
        failure: CssExecutionFailure,
    },
    FinalInvariantFailure {
        phase: CssExecutionPhase,
        failure: CssExecutionFailure,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssFixtureEvaluation {
    /// Standards fragment parsing is representable but not currently a
    /// production engine capability. AG eligibility must keep such a package
    /// out of execution.
    NotAttemptedFragmentCapabilityUnavailable,
    Attempted {
        outcome: CssObservedExecutionOutcome,
        observation: Option<String>,
    },
}

pub fn evaluate_fixture(fixture: &CssFixturePackage) -> CssFixtureEvaluation {
    if fixture.html_kind() == Some(CssHtmlInputKind::Fragment) {
        return CssFixtureEvaluation::NotAttemptedFragmentCapabilityUnavailable;
    }
    let result = match fixture.profile {
        CssExecutionProfile::PropertyValue => property_value(fixture),
        CssExecutionProfile::SelectorParsing => selector_parsing(fixture),
        CssExecutionProfile::SelectorSpecificity => selector_specificity(fixture),
        CssExecutionProfile::SelectorMatching => selector_matching(fixture),
        CssExecutionProfile::CascadeWinner
        | CssExecutionProfile::InheritanceCssWide
        | CssExecutionProfile::ComputedStyle => combined_style(fixture),
    };
    match result {
        Ok(actual) => {
            let outcome = if actual == fixture.expected {
                CssObservedExecutionOutcome::SemanticPass
            } else {
                CssObservedExecutionOutcome::ExpectationMismatch {
                    difference: first_difference(&fixture.expected, &actual),
                }
            };
            CssFixtureEvaluation::Attempted {
                outcome,
                observation: Some(actual),
            }
        }
        Err((phase, failure)) => CssFixtureEvaluation::Attempted {
            outcome: observed_failure(phase, failure),
            observation: None,
        },
    }
}

type ExecutionResult<T = String> = Result<T, (CssExecutionPhase, CssExecutionFailure)>;

fn observed_failure(
    phase: CssExecutionPhase,
    failure: CssExecutionFailure,
) -> CssObservedExecutionOutcome {
    match &failure {
        CssExecutionFailure::ObservationLimitExceeded { .. }
        | CssExecutionFailure::ObservationAllocationFailure => {
            CssObservedExecutionOutcome::IncompleteObservation { phase, failure }
        }
        CssExecutionFailure::ProjectionArtifact(_)
        | CssExecutionFailure::RequiredObservation(
            CssRequiredObservationFailure::SelectorSnapshotFormattingInvariant
            | CssRequiredObservationFailure::TargetMissingFromProjection
            | CssRequiredObservationFailure::TargetMissingFromStyleProjection
            | CssRequiredObservationFailure::ComputedTargetMissing
            | CssRequiredObservationFailure::ResolvedTargetMissing
            | CssRequiredObservationFailure::SelectedPropertyUnsupported
            | CssRequiredObservationFailure::SelectedResolvedPropertyMissing
            | CssRequiredObservationFailure::ObservationFormattingInvariant,
        ) => CssObservedExecutionOutcome::FinalInvariantFailure { phase, failure },
        CssExecutionFailure::HtmlParser(_)
        | CssExecutionFailure::HtmlSemanticInputResourceLimited(_)
        | CssExecutionFailure::TargetResolution { .. }
        | CssExecutionFailure::ResourceLimit { .. }
        | CssExecutionFailure::SelectorProjection(_)
        | CssExecutionFailure::SelectorMatching(_)
        | CssExecutionFailure::RuleCollection(_)
        | CssExecutionFailure::StyleResolution(_)
        | CssExecutionFailure::ComputedMaterialization(_)
        | CssExecutionFailure::StorageAllocation { .. }
        | CssExecutionFailure::RequiredObservation(
            CssRequiredObservationFailure::PropertyRuleCoordinateMissing
            | CssRequiredObservationFailure::PropertyCoordinateIsAtRule
            | CssRequiredObservationFailure::PropertyDeclarationCoordinateMissing
            | CssRequiredObservationFailure::PropertyNameUnresolved,
        ) => CssObservedExecutionOutcome::ExecutionFailure { phase, failure },
    }
}

fn property_value(fixture: &CssFixturePackage) -> ExecutionResult {
    let source = fixture
        .property_stylesheet
        .as_deref()
        .expect("validated property stylesheet");
    let parse = parse_stylesheet_with_options(source, &ParseOptions::stylesheet());
    ensure_stylesheet_semantically_complete(&parse)?;
    let coordinate = fixture.property.expect("validated property coordinate");
    let rule = parse
        .stylesheet
        .rules
        .get(coordinate.rule_index)
        .ok_or_else(|| required(CssRequiredObservationFailure::PropertyRuleCoordinateMissing))?;
    let Rule::Style(rule) = rule else {
        return Err(required(
            CssRequiredObservationFailure::PropertyCoordinateIsAtRule,
        ));
    };
    let declaration = rule
        .declarations
        .declarations
        .get(coordinate.declaration_index)
        .ok_or_else(|| {
            required(CssRequiredObservationFailure::PropertyDeclarationCoordinateMissing)
        })?;
    let name = declaration
        .name
        .text
        .as_deref()
        .ok_or_else(|| required(CssRequiredObservationFailure::PropertyNameUnresolved))?;
    let mut out = ObservationText::new(fixture.limits.max_expected_bytes())?;
    writeln!(out, "format: borrowser-css-property-value-observation-v1").map_err(format_failure)?;
    writeln!(out, "property: {name}").map_err(format_failure)?;
    match declaration.name.kind {
        PropertyNameKind::Custom => {
            writeln!(out, "result: unsupported-custom-property").map_err(format_failure)?
        }
        PropertyNameKind::Invalid => {
            writeln!(out, "result: invalid-property-name").map_err(format_failure)?
        }
        PropertyNameKind::Standard => {
            if let Some(property) = PropertyId::from_name(name) {
                writeln!(out, "classification: longhand").map_err(format_failure)?;
                match parse_specified_declaration_value_with_limits(
                    property,
                    &declaration.value,
                    &SpecifiedValueLimits::default(),
                ) {
                    Ok(value) => {
                        writeln!(out, "result: parsed").map_err(format_failure)?;
                        writeln!(out, "specified: {}", value.to_css_text())
                            .map_err(format_failure)?;
                    }
                    Err(error) => {
                        if error.kind() == css::SpecifiedValueParseErrorKind::ResourceLimitExceeded
                        {
                            return Err(resource_limit(
                                CssExecutionPhase::CssModelParsing,
                                CssExecutionResourceLimit::SpecifiedValueParsing,
                            ));
                        }
                        writeln!(out, "result: rejected").map_err(format_failure)?;
                        writeln!(out, "reason: {}", error.kind().as_debug_label())
                            .map_err(format_failure)?;
                    }
                }
            } else if let Some(shorthand) = ShorthandId::from_name(name) {
                writeln!(out, "classification: shorthand").map_err(format_failure)?;
                match css::expand_shorthand_declaration(shorthand, &declaration.value) {
                    Ok(expansion) => {
                        writeln!(out, "result: parsed").map_err(format_failure)?;
                        for (index, longhand) in expansion.longhands().iter().enumerate() {
                            let specified = css::parse_specified_declaration_value_with_limits(
                                longhand.property(),
                                longhand.value(),
                                &SpecifiedValueLimits::default(),
                            );
                            if matches!(
                                specified.as_ref().map_err(|error| error.kind()),
                                Err(css::SpecifiedValueParseErrorKind::ResourceLimitExceeded)
                            ) {
                                return Err(resource_limit(
                                    CssExecutionPhase::CssModelParsing,
                                    CssExecutionResourceLimit::SpecifiedValueParsing,
                                ));
                            }
                            writeln!(
                                out,
                                "longhand[{index}]: {}={}",
                                longhand.property().name(),
                                specified
                                    .map(|value| value.to_css_text())
                                    .unwrap_or_else(|_| "rejected".to_owned())
                            )
                            .map_err(format_failure)?;
                        }
                    }
                    Err(error) => {
                        if shorthand_error_is_resource_limit(error.kind()) {
                            return Err(resource_limit(
                                CssExecutionPhase::CssModelParsing,
                                CssExecutionResourceLimit::ShorthandExpansion,
                            ));
                        }
                        writeln!(out, "result: rejected").map_err(format_failure)?;
                        writeln!(out, "reason: {}", error.kind().as_debug_label())
                            .map_err(format_failure)?;
                    }
                }
            } else {
                writeln!(out, "result: unsupported-property").map_err(format_failure)?;
            }
        }
    }
    out.finish()
}

fn selector_parsing(fixture: &CssFixturePackage) -> ExecutionResult {
    let result = parse_selector_source_with_limits(
        fixture
            .selector_list
            .as_deref()
            .expect("validated selector source"),
        &css::SyntaxLimits::default(),
    );
    ensure_selector_parse_semantically_complete(&result)?;
    css::serialize_selector_parse_result_for_snapshot_bounded(
        &result,
        fixture.limits.max_expected_bytes(),
    )
    .map_err(|error| {
        let failure = match error {
            css::SelectorSnapshotSerializationError::LimitExceeded { maximum, observed } => {
                CssExecutionFailure::ObservationLimitExceeded {
                    actual: observed,
                    maximum,
                }
            }
            css::SelectorSnapshotSerializationError::ReservationFailure { .. } => {
                CssExecutionFailure::ObservationAllocationFailure
            }
            css::SelectorSnapshotSerializationError::FormattingInvariant => {
                CssExecutionFailure::RequiredObservation(
                    CssRequiredObservationFailure::SelectorSnapshotFormattingInvariant,
                )
            }
        };
        (CssExecutionPhase::ObservationSerialization, failure)
    })
}

fn selector_specificity(fixture: &CssFixturePackage) -> ExecutionResult {
    let result = parse_selector_source_with_limits(
        fixture
            .selector_list
            .as_deref()
            .expect("validated selector source"),
        &css::SyntaxLimits::default(),
    );
    ensure_selector_parse_semantically_complete(&result)?;
    let mut out = ObservationText::new(fixture.limits.max_expected_bytes())?;
    writeln!(
        out,
        "format: borrowser-css-selector-specificity-observation-v1"
    )
    .map_err(format_failure)?;
    match result {
        SelectorListParseResult::Parsed(list) => {
            writeln!(out, "result: parsed").map_err(format_failure)?;
            for (index, selector) in list.selectors().iter().enumerate() {
                let specificity = selector.specificity();
                writeln!(
                    out,
                    "selector[{index}]: ({},{},{})",
                    specificity.a(),
                    specificity.b(),
                    specificity.c()
                )
                .map_err(format_failure)?;
            }
        }
        SelectorListParseResult::Unsupported(list) => {
            writeln!(out, "result: unsupported").map_err(format_failure)?;
            for (index, feature) in list.features().iter().enumerate() {
                writeln!(out, "feature[{index}]: {}", feature.stable_label())
                    .map_err(format_failure)?;
            }
        }
        SelectorListParseResult::Invalid(list) => {
            writeln!(out, "result: invalid").map_err(format_failure)?;
            writeln!(out, "reason: {}", list.reason().stable_label()).map_err(format_failure)?;
        }
    }
    out.finish()
}

fn selector_matching(fixture: &CssFixturePackage) -> ExecutionResult {
    let output = parse_html(fixture)?;
    let selectors = parse_selector_source_with_limits(
        fixture
            .selector_list
            .as_deref()
            .expect("validated selectors"),
        &css::SyntaxLimits::default(),
    );
    ensure_selector_parse_semantically_complete(&selectors)?;
    let environment = SelectorMatchingEnvironment::new(output.document_mode);
    let projection = StyleProjection::try_from_document_with_element_limit(
        &output.document,
        environment,
        StyleResolutionLimits::default().max_styled_elements_per_document,
    )
    .map_err(projection_failure)?;
    let mut out = ObservationText::new(fixture.limits.max_expected_bytes())?;
    writeln!(
        out,
        "format: borrowser-css-selector-matching-observation-v1"
    )
    .map_err(format_failure)?;
    writeln!(out, "document-mode: {}", output.document_mode).map_err(format_failure)?;
    for address in &fixture.targets {
        let element = resolve_target(&output.document, address).map_err(|failure| {
            (
                CssExecutionPhase::TargetResolution,
                CssExecutionFailure::TargetResolution {
                    label: address.label().to_owned(),
                    failure,
                },
            )
        })?;
        let key = projection.key_for_element(element).ok_or({
            (
                CssExecutionPhase::SelectorProjection,
                CssExecutionFailure::RequiredObservation(
                    CssRequiredObservationFailure::TargetMissingFromProjection,
                ),
            )
        })?;
        let matches = projection
            .match_selector_list_checked(&key, &selectors, SelectorMatchingLimits::default())
            .map_err(match_failure)?;
        writeln!(out, "target: {}", address.label()).map_err(format_failure)?;
        writeln!(
            out,
            "  matchability: {}",
            matchability_label(matches.matchability())
        )
        .map_err(format_failure)?;
        for matched in matches.matched_selectors() {
            let specificity = matched.specificity();
            writeln!(
                out,
                "  matched-selector[{}]: ({},{},{})",
                matched.selector_index(),
                specificity.a(),
                specificity.b(),
                specificity.c()
            )
            .map_err(format_failure)?;
        }
    }
    out.finish()
}

fn combined_style(fixture: &CssFixturePackage) -> ExecutionResult {
    let output = parse_html(fixture)?;
    let mut sheets = Vec::new();
    sheets.try_reserve(fixture.stylesheets.len()).map_err(|_| {
        (
            CssExecutionPhase::CssModelParsing,
            CssExecutionFailure::StorageAllocation {
                storage: CssExecutionStorage::ParsedStylesheets,
            },
        )
    })?;
    for stylesheet in &fixture.stylesheets {
        let parsed = parse_stylesheet_with_options(&stylesheet.source, &ParseOptions::stylesheet());
        ensure_stylesheet_semantically_complete(&parsed)?;
        sheets.push(parsed);
    }
    let limits = StyleResolutionLimits::default();
    let mut inputs = Vec::new();
    inputs.try_reserve(sheets.len()).map_err(|_| {
        (
            CssExecutionPhase::RuleCollection,
            CssExecutionFailure::StorageAllocation {
                storage: CssExecutionStorage::StylesheetInputs,
            },
        )
    })?;
    for (authored, parsed) in fixture.stylesheets.iter().zip(&sheets) {
        let source = match authored.origin {
            CssStylesheetOrigin::UserAgent => StylesheetSourceId::built_in_user_agent(),
            CssStylesheetOrigin::User | CssStylesheetOrigin::Author => {
                StylesheetSourceId::in_memory_generation_index(authored.source_index)
            }
        };
        let input = match authored.origin {
            CssStylesheetOrigin::UserAgent => StylesheetCollectionInput::user_agent_for_namespace(
                source,
                StylesheetOrder::new(authored.order),
                parsed,
                authored
                    .namespace
                    .expect("validated UA namespace")
                    .as_element_namespace(),
            ),
            CssStylesheetOrigin::User => StylesheetCollectionInput::user(
                source,
                StylesheetOrder::new(authored.order),
                parsed,
                StylesheetConditionInput::None,
            ),
            CssStylesheetOrigin::Author => StylesheetCollectionInput::author(
                source,
                StylesheetOrder::new(authored.order),
                parsed,
                StylesheetConditionInput::None,
            ),
        };
        inputs.push(input);
    }
    let collection = RuleCollection::try_new(&inputs, &limits).map_err(|error| {
        (
            CssExecutionPhase::RuleCollection,
            CssExecutionFailure::RuleCollection(error),
        )
    })?;
    let environment = SelectorMatchingEnvironment::new(output.document_mode);
    let execution =
        StyleResolutionExecution::try_new(&output.document, environment, &collection, &limits)
            .map_err(|error| {
                (
                    CssExecutionPhase::SelectorProjection,
                    CssExecutionFailure::StyleResolution(error),
                )
            })?;
    let projection_resolved = execution
        .resolve_document_styles_with_projection()
        .map_err(|error| {
            (
                CssExecutionPhase::Cascade,
                CssExecutionFailure::StyleResolution(error),
            )
        })?;
    let computed = if fixture.profile == CssExecutionProfile::ComputedStyle {
        Some(
            execution
                .compute_document_styles_from_projection_resolved(&projection_resolved)
                .map_err(|error| {
                    (
                        CssExecutionPhase::ComputedStyle,
                        CssExecutionFailure::ComputedMaterialization(error),
                    )
                })?,
        )
    } else {
        None
    };
    let mut out = ObservationText::new(fixture.limits.max_expected_bytes())?;
    writeln!(
        out,
        "format: {}",
        match fixture.profile {
            CssExecutionProfile::CascadeWinner => "borrowser-css-cascade-winner-observation-v1",
            CssExecutionProfile::InheritanceCssWide =>
                "borrowser-css-resolved-style-observation-v1",
            CssExecutionProfile::ComputedStyle => "borrowser-css-computed-style-observation-v1",
            _ => unreachable!(),
        }
    )
    .map_err(format_failure)?;
    writeln!(out, "document-mode: {}", output.document_mode).map_err(format_failure)?;
    for address in &fixture.targets {
        let element = resolve_target(&output.document, address).map_err(|failure| {
            (
                CssExecutionPhase::TargetResolution,
                CssExecutionFailure::TargetResolution {
                    label: address.label().to_owned(),
                    failure,
                },
            )
        })?;
        let key = execution.projection_key_for_element(element).ok_or({
            (
                CssExecutionPhase::SelectorProjection,
                CssExecutionFailure::RequiredObservation(
                    CssRequiredObservationFailure::TargetMissingFromStyleProjection,
                ),
            )
        })?;
        writeln!(out, "target: {}", address.label()).map_err(format_failure)?;
        if fixture.profile == CssExecutionProfile::ComputedStyle {
            let entry = execution
                .computed_style_for_key(computed.as_ref().expect("computed"), &key)
                .map_err(|error| {
                    (
                        CssExecutionPhase::ComputedStyle,
                        CssExecutionFailure::ProjectionArtifact(error),
                    )
                })?
                .ok_or_else(|| required(CssRequiredObservationFailure::ComputedTargetMissing))?;
            for name in &fixture.selected_properties {
                let property = PropertyId::from_name(name).ok_or_else(|| {
                    required(CssRequiredObservationFailure::SelectedPropertyUnsupported)
                })?;
                writeln!(
                    out,
                    "  {name}: {}",
                    entry.style().get(property).value().to_debug_label()
                )
                .map_err(format_failure)?;
            }
        } else {
            let entry = execution
                .resolved_style_for_key(&projection_resolved, &key)
                .map_err(|error| {
                    (
                        CssExecutionPhase::ResolvedStyleObservation,
                        CssExecutionFailure::ProjectionArtifact(error),
                    )
                })?
                .ok_or_else(|| required(CssRequiredObservationFailure::ResolvedTargetMissing))?;
            for name in &fixture.selected_properties {
                let property = PropertyId::from_name(name).ok_or_else(|| {
                    required(CssRequiredObservationFailure::SelectedPropertyUnsupported)
                })?;
                let resolved_entry = entry.style().get(property).ok_or_else(|| {
                    required(CssRequiredObservationFailure::SelectedResolvedPropertyMissing)
                })?;
                if fixture.profile == CssExecutionProfile::CascadeWinner {
                    match resolved_entry.source() {
                        ResolvedValueSource::Winner(winner) => {
                            writeln!(
                                out,
                                "  {name}: winner source={} specificity={} value={}",
                                if winner
                                    .priority
                                    .declaration_precedence()
                                    .is_element_attached()
                                {
                                    "inline-style"
                                } else {
                                    "stylesheet"
                                },
                                specificity_or_none(winner.priority.specificity()),
                                winner
                                    .value
                                    .to_css_text()
                                    .as_deref()
                                    .unwrap_or("unresolved")
                            )
                            .map_err(format_failure)?;
                        }
                        source => writeln!(
                            out,
                            "  {name}: no-winner source={}",
                            resolved_source_label(source)
                        )
                        .map_err(format_failure)?,
                    }
                } else {
                    writeln!(
                        out,
                        "  {name}: {}",
                        resolved_source_label(resolved_entry.source())
                    )
                    .map_err(format_failure)?;
                }
            }
        }
    }
    out.finish()
}

fn parse_html(
    fixture: &CssFixturePackage,
) -> Result<html::ParseOutput, (CssExecutionPhase, CssExecutionFailure)> {
    let input = fixture.html.as_ref().expect("validated combined HTML");
    let options = HtmlParseOptions {
        tokenizer: HtmlTokenizerOptions {
            emit_eof: true,
            limits: HtmlTokenizerLimits::default(),
        },
        tree_builder: HtmlTreeBuilderOptions {
            coalesce_text: false,
            limits: HtmlTreeBuilderLimits::default(),
        },
        error_policy: HtmlErrorPolicy {
            track: true,
            max_stored: 128,
            debug_only: false,
            track_counters: true,
        },
    };
    let output = html::parse_document(&input.source, options).map_err(|error| {
        (
            CssExecutionPhase::HtmlDocumentParsing,
            CssExecutionFailure::HtmlParser(error),
        )
    })?;
    if let HtmlParseSemanticCompleteness::Degraded(degraded) = output.semantic_completeness {
        return Err((
            CssExecutionPhase::HtmlDocumentParsing,
            CssExecutionFailure::HtmlSemanticInputResourceLimited(degraded),
        ));
    }
    Ok(output)
}

fn ensure_stylesheet_semantically_complete(parse: &css::StylesheetParse) -> ExecutionResult<()> {
    if parse.stats.hit_limit {
        Err(resource_limit(
            CssExecutionPhase::CssModelParsing,
            CssExecutionResourceLimit::StylesheetModelParsing,
        ))
    } else {
        Ok(())
    }
}

fn ensure_selector_parse_semantically_complete(
    result: &SelectorListParseResult,
) -> ExecutionResult<()> {
    if matches!(
        result,
        SelectorListParseResult::Invalid(invalid)
            if invalid.reason() == css::InvalidSelectorReason::ResourceLimitExceeded
    ) {
        Err(resource_limit(
            CssExecutionPhase::SelectorParsing,
            CssExecutionResourceLimit::SelectorParsing,
        ))
    } else {
        Ok(())
    }
}

fn shorthand_error_is_resource_limit(kind: &css::ShorthandExpansionErrorKind) -> bool {
    matches!(
        kind,
        css::ShorthandExpansionErrorKind::ResourceLimitExceeded
            | css::ShorthandExpansionErrorKind::LonghandValueRejected {
                kind: css::SpecifiedValueParseErrorKind::ResourceLimitExceeded,
                ..
            }
    )
}

fn resource_limit(
    phase: CssExecutionPhase,
    resource: CssExecutionResourceLimit,
) -> (CssExecutionPhase, CssExecutionFailure) {
    (phase, CssExecutionFailure::ResourceLimit { resource })
}

fn resolved_source_label(source: &ResolvedValueSource) -> String {
    match source {
        ResolvedValueSource::Winner(winner) => format!(
            "winner:{}",
            winner
                .value
                .to_css_text()
                .as_deref()
                .unwrap_or("unresolved")
        ),
        ResolvedValueSource::Inherited => "inherited".to_owned(),
        ResolvedValueSource::Initial(_) => "initial".to_owned(),
        ResolvedValueSource::CssWideKeyword(CssWideResolvedSource::Initial { keyword, .. }) => {
            format!("css-wide:{}->initial", keyword.as_css_keyword())
        }
        ResolvedValueSource::CssWideKeyword(CssWideResolvedSource::Inherited {
            keyword, ..
        }) => format!("css-wide:{}->inherited", keyword.as_css_keyword()),
    }
}

fn specificity_or_none(value: Option<css::Specificity>) -> String {
    value
        .map(|s| format!("({},{},{})", s.a(), s.b(), s.c()))
        .unwrap_or_else(|| "none".to_owned())
}
fn matchability_label(value: css::SelectorMatchability) -> &'static str {
    if value.is_parsed() {
        "parsed"
    } else if value.is_unsupported() {
        "unsupported"
    } else {
        "invalid"
    }
}
fn projection_failure(
    error: StyleProjectionBuildError,
) -> (CssExecutionPhase, CssExecutionFailure) {
    (
        CssExecutionPhase::SelectorProjection,
        CssExecutionFailure::SelectorProjection(error),
    )
}
fn match_failure(error: StyleProjectionMatchError) -> (CssExecutionPhase, CssExecutionFailure) {
    (
        CssExecutionPhase::SelectorMatching,
        CssExecutionFailure::SelectorMatching(error),
    )
}
fn required(failure: CssRequiredObservationFailure) -> (CssExecutionPhase, CssExecutionFailure) {
    (
        CssExecutionPhase::ObservationSerialization,
        CssExecutionFailure::RequiredObservation(failure),
    )
}
fn first_difference(expected: &str, actual: &str) -> String {
    let line = expected
        .lines()
        .zip(actual.lines())
        .position(|(left, right)| left != right)
        .map(|index| index + 1)
        .unwrap_or_else(|| expected.lines().count().min(actual.lines().count()) + 1);
    format!(
        "first mismatch at line {line}; expected-bytes={} actual-bytes={}",
        expected.len(),
        actual.len()
    )
}

struct ObservationText {
    bytes: String,
    failure: Option<CssExecutionFailure>,
    maximum: usize,
}
impl ObservationText {
    fn new(maximum: usize) -> Result<Self, (CssExecutionPhase, CssExecutionFailure)> {
        let mut bytes = String::new();
        bytes.try_reserve(1024).map_err(|_| {
            (
                CssExecutionPhase::ObservationSerialization,
                CssExecutionFailure::ObservationAllocationFailure,
            )
        })?;
        Ok(Self {
            bytes,
            failure: None,
            maximum,
        })
    }
    fn finish(self) -> ExecutionResult {
        match self.failure {
            Some(failure) => Err((CssExecutionPhase::ObservationSerialization, failure)),
            None => Ok(self.bytes),
        }
    }
}
impl std::fmt::Write for ObservationText {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        if self.failure.is_some() {
            return Ok(());
        }
        let maximum = self.maximum;
        let Some(actual) = self.bytes.len().checked_add(value.len()) else {
            self.failure = Some(CssExecutionFailure::ObservationLimitExceeded {
                actual: usize::MAX,
                maximum,
            });
            return Ok(());
        };
        if actual > maximum {
            self.failure = Some(CssExecutionFailure::ObservationLimitExceeded { actual, maximum });
            return Ok(());
        }
        if self.bytes.try_reserve(value.len()).is_err() {
            self.failure = Some(CssExecutionFailure::ObservationAllocationFailure);
            return Ok(());
        }
        self.bytes.push_str(value);
        Ok(())
    }
}
fn format_failure(_: std::fmt::Error) -> (CssExecutionPhase, CssExecutionFailure) {
    (
        CssExecutionPhase::ObservationSerialization,
        CssExecutionFailure::RequiredObservation(
            CssRequiredObservationFailure::ObservationFormattingInvariant,
        ),
    )
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_classifier_is_exhaustive_over_css_failure_families() {
        use CssExecutionFailureClass::{OtherExecutionFailure, ResourceFailure};

        let resources = [
            CssExecutionFailure::ResourceLimit {
                resource: CssExecutionResourceLimit::SelectorParsing,
            },
            CssExecutionFailure::StorageAllocation {
                storage: CssExecutionStorage::ParsedStylesheets,
            },
            CssExecutionFailure::SelectorProjection(
                StyleProjectionBuildError::ElementLimitExceeded {
                    limit: 1,
                    observed: 2,
                },
            ),
            CssExecutionFailure::SelectorProjection(StyleProjectionBuildError::SelectorDom(
                css::SelectorDomBuildError::ElementIdRepresentationExhausted { maximum: 1 },
            )),
            CssExecutionFailure::SelectorMatching(StyleProjectionMatchError::Matching(
                css::SelectorMatchingLimitError::AxisStepLimitExceeded { limit: 1 },
            )),
            CssExecutionFailure::RuleCollection(css::RuleCollectionBuildError::Reservation {
                storage: css::RuleCollectionStorage::Rules,
            }),
            CssExecutionFailure::StyleResolution(css::StyleResolutionError::LimitExceeded {
                limit: css::StyleResolutionLimit::MatchedRulesPerElement,
                configured: 1,
            }),
            CssExecutionFailure::StyleResolution(css::StyleResolutionError::StylesheetInputBuild(
                css::StylesheetCollectionInputBuildError::Reservation,
            )),
            CssExecutionFailure::StyleResolution(css::StyleResolutionError::CascadeResolution(
                css::CascadeResolutionError::WinnerWorkspaceReservationFailed { requested: 1 },
            )),
            CssExecutionFailure::ComputedMaterialization(
                css::ComputedStyleResolutionError::SelectorDomBuild(
                    css::SelectorDomBuildError::ProjectionCapacityExceeded {
                        storage: css::SelectorDomBuildStorage::ElementRecords,
                    },
                ),
            ),
        ];
        assert!(
            resources
                .iter()
                .all(|failure| classify_execution_failure(failure) == ResourceFailure)
        );

        let other = [
            CssExecutionFailure::HtmlParser(HtmlParseError::Decode),
            CssExecutionFailure::SelectorProjection(StyleProjectionBuildError::SelectorDom(
                css::SelectorDomBuildError::NestedDocument { depth: 1 },
            )),
            CssExecutionFailure::SelectorMatching(StyleProjectionMatchError::ProjectionKey(
                css::StyleProjectionKeyError::RootMismatch,
            )),
            CssExecutionFailure::StyleResolution(css::StyleResolutionError::SelectorDomBuild(
                css::SelectorDomBuildError::NestedDocument { depth: 1 },
            )),
            CssExecutionFailure::ComputedMaterialization(
                css::ComputedStyleResolutionError::MissingMatchingEnvironment,
            ),
            CssExecutionFailure::RequiredObservation(
                CssRequiredObservationFailure::PropertyNameUnresolved,
            ),
            CssExecutionFailure::ObservationAllocationFailure,
        ];
        assert!(
            other
                .iter()
                .all(|failure| classify_execution_failure(failure) == OtherExecutionFailure)
        );
    }

    fn test_limits() -> crate::CssFixtureLimits {
        crate::CssFixtureLimits::try_new(
            16 * 1024,
            1024 * 1024,
            css::StyleResolutionLimits::default()
                .max_stylesheets_per_style_pass
                .min(16),
        )
        .expect("test fixture limits")
    }

    #[test]
    fn observation_builder_accepts_exact_limit_and_rejects_plus_one_without_partial_success() {
        let maximum = test_limits().max_expected_bytes();
        let mut exact = ObservationText {
            bytes: "x".repeat(maximum - 1),
            failure: None,
            maximum,
        };
        assert!(exact.write_str("x").is_ok());
        assert_eq!(exact.bytes.len(), maximum);
        assert!(exact.write_str("x").is_ok());
        assert_eq!(exact.bytes.len(), maximum);
        assert!(matches!(
            exact.finish(),
            Err((
                CssExecutionPhase::ObservationSerialization,
                CssExecutionFailure::ObservationLimitExceeded {
                    actual,
                    maximum
                }
            )) if actual == maximum + 1 && maximum == test_limits().max_expected_bytes()
        ));
    }

    fn selector_fixture(source: String, expected: String) -> CssFixturePackage {
        CssFixturePackage {
            id: "selector-fixture".to_owned(),
            profile: CssExecutionProfile::SelectorParsing,
            selector_list: Some(source),
            property_stylesheet: None,
            stylesheets: vec![],
            html: None,
            property: None,
            targets: vec![],
            selected_properties: vec![],
            expected,
            primary_input_path: "selectors.txt".to_owned(),
            referenced_paths: vec!["expected.txt".to_owned(), "selectors.txt".to_owned()],
            limits: test_limits(),
        }
    }

    fn html_carrier(source: String) -> CssFixturePackage {
        CssFixturePackage {
            id: "html-carrier".to_owned(),
            profile: CssExecutionProfile::SelectorMatching,
            selector_list: Some("div".to_owned()),
            property_stylesheet: None,
            stylesheets: vec![],
            html: Some(crate::fixture::CssHtmlInput {
                request: crate::fixture::CssHtmlRequest::Document,
                source,
            }),
            property: None,
            targets: vec![],
            selected_properties: vec![],
            expected: String::new(),
            primary_input_path: "selectors.txt".to_owned(),
            referenced_paths: vec![],
            limits: test_limits(),
        }
    }

    #[test]
    fn css_html_input_integrity_uses_typed_completeness_not_parse_error_counters() {
        let malformed = html_carrier("<!doctype html><p><div>".to_owned());
        assert!(parse_html(&malformed).is_ok());

        let dropped_errors = html_carrier("&bogus;".repeat(256));
        let output = parse_html(&dropped_errors).expect("complete parser-created DOM");
        assert!(output.counters.errors_dropped > 0);
        assert_eq!(
            output.semantic_completeness,
            HtmlParseSemanticCompleteness::Complete
        );

        let degraded = html_carrier(format!("<{}>", "a".repeat(1025)));
        assert!(matches!(
            parse_html(&degraded),
            Err((
                CssExecutionPhase::HtmlDocumentParsing,
                CssExecutionFailure::HtmlSemanticInputResourceLimited(reasons)
            )) if reasons.contains(html::HtmlParseSemanticDegradationReason::TagNameTruncated)
        ));
    }

    #[test]
    fn unsupported_selector_is_a_semantic_observation_not_an_execution_failure() {
        let expected = css::serialize_selector_parse_result_for_snapshot(
            &css::parse_selector_source("a:hover"),
        );
        let fixture = selector_fixture("a:hover".to_owned(), expected);
        assert!(matches!(
            evaluate_fixture(&fixture),
            CssFixtureEvaluation::Attempted {
                outcome: CssObservedExecutionOutcome::SemanticPass,
                ..
            }
        ));
    }

    #[test]
    fn selector_resource_exhaustion_is_not_an_authored_invalid_observation() {
        let selector_list = (0..=css::SyntaxLimits::default().max_selectors_per_rule)
            .map(|index| format!(".selector-{index}"))
            .collect::<Vec<_>>()
            .join(",");
        let fixture = selector_fixture(selector_list, String::new());
        assert!(matches!(
            evaluate_fixture(&fixture),
            CssFixtureEvaluation::Attempted {
                outcome: CssObservedExecutionOutcome::ExecutionFailure {
                    phase: CssExecutionPhase::SelectorParsing,
                    failure: CssExecutionFailure::ResourceLimit {
                        resource: CssExecutionResourceLimit::SelectorParsing
                    }
                },
                observation: None,
            }
        ));
    }

    #[test]
    fn stylesheet_and_specified_value_limits_have_closed_execution_categories() {
        let options = ParseOptions {
            limits: css::SyntaxLimits {
                max_rules: 0,
                ..css::SyntaxLimits::default()
            },
            ..ParseOptions::stylesheet()
        };
        let parse = parse_stylesheet_with_options("a {}", &options);
        assert!(matches!(
            ensure_stylesheet_semantically_complete(&parse),
            Err((
                CssExecutionPhase::CssModelParsing,
                CssExecutionFailure::ResourceLimit {
                    resource: CssExecutionResourceLimit::StylesheetModelParsing
                }
            ))
        ));
        assert!(shorthand_error_is_resource_limit(
            &css::ShorthandExpansionErrorKind::ResourceLimitExceeded
        ));
        assert!(shorthand_error_is_resource_limit(
            &css::ShorthandExpansionErrorKind::LonghandValueRejected {
                property: PropertyId::Color,
                kind: css::SpecifiedValueParseErrorKind::ResourceLimitExceeded,
            }
        ));
        assert!(!shorthand_error_is_resource_limit(
            &css::ShorthandExpansionErrorKind::UnsupportedComponent
        ));
    }
}
