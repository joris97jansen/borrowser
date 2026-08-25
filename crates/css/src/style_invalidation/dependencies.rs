use std::fmt::{self, Write};

use crate::cascade::{CollectedRule, RuleCollection};
use crate::selectors::SelectorDomAttribute;
use crate::selectors::{
    AttributeMatcher, AttributeSelector, AttributeValue, Combinator, ComplexSelector,
    SelectorMatchingEnvironment, SelectorNamespaceConstraint, SubclassSelector,
    TreeStructuralPseudoClass, TypeSelector,
};
use crate::{CascadeDeclarationApplicability, StyleResolutionLimit, StyleResolutionLimits};

pub const STYLE_DEPENDENCY_ARTIFACT_DEBUG_VERSION: u32 = 1;
const STYLE_DEPENDENCY_DEBUG_MAX_RECORDS: usize = 4_096;
const STYLE_DEPENDENCY_DEBUG_MAX_SERIALIZED_BYTES: usize = 512 * 1024;
const STYLE_DEPENDENCY_DEBUG_RESERVED_METADATA_BYTES: usize = 2 * 1024;

/// Owned CSS selector/cascade dependency metadata for one active stylesheet
/// input generation. Browser may retain this value but cannot inspect its
/// semantic index.
#[derive(Clone, PartialEq, Eq)]
pub struct StyleDependencyArtifact {
    matching_environment: SelectorMatchingEnvironment,
    state: StyleDependencyArtifactState,
    summary: StyleDependencySummary,
    max_transition_evaluations_per_publication: usize,
}

impl fmt::Debug for StyleDependencyArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("StyleDependencyArtifact");
        debug
            .field("matching_environment", &self.matching_environment)
            .field(
                "state",
                &match &self.state {
                    StyleDependencyArtifactState::Complete(_) => "complete",
                    StyleDependencyArtifactState::ConservativeUnavailable(_) => {
                        "conservative-unavailable"
                    }
                },
            )
            .field(
                "dependency_record_count",
                &match &self.state {
                    StyleDependencyArtifactState::Complete(index) => Some(index.record_count),
                    StyleDependencyArtifactState::ConservativeUnavailable(_) => None,
                },
            )
            .field("participating_rules", &self.summary.participating_rules)
            .field(
                "participating_selectors",
                &self.summary.participating_selectors,
            )
            .field(
                "active_without_supported_declarations",
                &self.summary.active_without_supported_declarations,
            )
            .field(
                "max_transition_evaluations_per_publication",
                &self.max_transition_evaluations_per_publication,
            );
        if let StyleDependencyArtifactState::ConservativeUnavailable(failure) = &self.state {
            debug.field("failure_kind", &failure.stable_label());
        }
        debug.finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StyleDependencyArtifactState {
    Complete(SelectorDependencyIndex),
    ConservativeUnavailable(StyleDependencyBuildFailure),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct StyleDependencySummary {
    participating_rules: usize,
    participating_selectors: usize,
    active_without_supported_declarations: usize,
    inactive_invalid_rules: usize,
    inactive_unsupported_rules: usize,
    inactive_deferred_rules: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StyleDependencyBuildFailure {
    LimitExceeded {
        limit: StyleResolutionLimit,
        configured: usize,
        observed: usize,
    },
    CounterExhausted {
        counter: &'static str,
    },
    Reservation {
        storage: StyleDependencyStorage,
    },
}

impl StyleDependencyBuildFailure {
    const fn stable_label(&self) -> &'static str {
        match self {
            Self::LimitExceeded { .. } => "limit-exceeded",
            Self::CounterExhausted { .. } => "counter-exhausted",
            Self::Reservation { .. } => "reservation",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StyleDependencyStorage {
    Records,
    IndexGroups,
    IndexEffects,
    SubjectPath,
    SelectorText,
}

impl StyleDependencyStorage {
    const fn stable_label(self) -> &'static str {
        match self {
            Self::Records => "records",
            Self::IndexGroups => "index-groups",
            Self::IndexEffects => "index-effects",
            Self::SubjectPath => "subject-path",
            Self::SelectorText => "selector-text",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct SelectorDependencyIndex {
    type_names: Vec<KeyedDependencyGroup<NameDependencyKey>>,
    id_values: Vec<KeyedDependencyGroup<String>>,
    class_tokens: Vec<KeyedDependencyGroup<String>>,
    attributes: Vec<AttributeDependencyGroup>,
    structural: Vec<KeyedDependencyGroup<StructuralDependencyKind>>,
    relationships: Vec<KeyedDependencyGroup<RelationshipDependencyKind>>,
    record_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SelectorDependencyRecord {
    trigger: SelectorDependencyTrigger,
    effect: SelectorDependencyEffect,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum SelectorDependencyTrigger {
    TypeName(NameDependencyKey),
    IdValue(String),
    ClassToken(String),
    Attribute(AttributeDependencyKey),
    Structural(StructuralDependencyKind),
    Relationship(RelationshipDependencyKind),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum NameDependencyKey {
    HtmlAsciiLowercase(String),
    Exact(String),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct AttributeDependencyKey {
    name: NameDependencyKey,
    predicate: AttributeDependencyPredicate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct KeyedDependencyGroup<K> {
    key: K,
    effects: Vec<SelectorDependencyEffect>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AttributeDependencyGroup {
    name: NameDependencyKey,
    predicates: Vec<AttributePredicateDependencyGroup>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AttributePredicateDependencyGroup {
    predicate: AttributeDependencyPredicate,
    effects: Vec<SelectorDependencyEffect>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum AttributeDependencyPredicate {
    Exists,
    Match {
        matcher: AttributeMatcher,
        value: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum StructuralDependencyKind {
    Root,
    EmptyDirectContent,
    FirstChildOrder,
    LastChildOrder,
    OnlyChildOrder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum RelationshipDependencyKind {
    DescendantAncestry,
    DirectParentChild,
    AdjacentSibling,
    FollowingSiblings,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SelectorDependencyEffect {
    namespace_constraint: SelectorNamespaceConstraint,
    subject_path: SelectorSubjectPath,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct SelectorSubjectPath {
    steps: Vec<SelectorEffectStep>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum SelectorEffectStep {
    Descendants,
    DirectChildren,
    NextSibling,
    FollowingSiblings,
}

impl StyleDependencyArtifact {
    pub fn from_rule_collection(
        collection: &RuleCollection<'_>,
        matching_environment: SelectorMatchingEnvironment,
        limits: &StyleResolutionLimits,
    ) -> Self {
        let mut builder = DependencyBuilder::new(limits, matching_environment);
        let result = builder.collect(collection).and_then(|()| {
            builder.records.sort_unstable();
            builder.records.dedup();
            SelectorDependencyIndex::try_from_records(std::mem::take(&mut builder.records))
        });
        let summary = builder.summary;
        let state = match result {
            Ok(index) => StyleDependencyArtifactState::Complete(index),
            Err(error) => StyleDependencyArtifactState::ConservativeUnavailable(error),
        };
        Self {
            matching_environment,
            state,
            summary,
            max_transition_evaluations_per_publication: limits
                .max_selector_dependency_evaluations_per_publication,
        }
    }

    /// CSS-owned compatibility query used by Browser only for retained
    /// artifact lifecycle validation.
    pub fn matches_environment(&self, environment: SelectorMatchingEnvironment) -> bool {
        self.matching_environment == environment
    }

    pub(super) fn complete_index(&self) -> Option<&SelectorDependencyIndex> {
        match &self.state {
            StyleDependencyArtifactState::Complete(index) => Some(index),
            StyleDependencyArtifactState::ConservativeUnavailable(_) => None,
        }
    }

    pub(super) fn classification_budget(&self) -> DependencyEvaluationBudget {
        DependencyEvaluationBudget::new(self.max_transition_evaluations_per_publication)
    }

    pub(super) fn classify_attribute_transition(
        &self,
        element_namespace: html::ElementNamespace,
        before: &[html::ParserCreatedAttribute],
        after: &[html::ParserCreatedAttribute],
        budget: &mut DependencyEvaluationBudget,
    ) -> Result<AttributeDependencyMatch, DependencyClassificationFailure> {
        let index = self
            .complete_index()
            .ok_or(DependencyClassificationFailure::ArtifactUnavailable)?;
        let mut matched = AttributeDependencyMatch::default();
        self.classify_id_transition(
            index,
            element_namespace,
            before,
            after,
            budget,
            &mut matched,
        )?;
        self.classify_class_transition(
            index,
            element_namespace,
            before,
            after,
            budget,
            &mut matched,
        )?;
        self.classify_named_attribute_transitions(
            index,
            element_namespace,
            before,
            after,
            budget,
            &mut matched,
        )?;
        matched.any = matched.id || matched.class || matched.attribute;
        Ok(matched)
    }

    fn classify_id_transition(
        &self,
        index: &SelectorDependencyIndex,
        element_namespace: html::ElementNamespace,
        before: &[html::ParserCreatedAttribute],
        after: &[html::ParserCreatedAttribute],
        budget: &mut DependencyEvaluationBudget,
        matched: &mut AttributeDependencyMatch,
    ) -> Result<(), DependencyClassificationFailure> {
        if index.id_values.is_empty() {
            return Ok(());
        }
        let document_mode = self.matching_environment.document_mode();
        let before_value = effective_unqualified_attribute(before, element_namespace, "id")
            .map(SelectorDomAttribute::value);
        let after_value = effective_unqualified_attribute(after, element_namespace, "id")
            .map(SelectorDomAttribute::value);
        if crate::selectors::matching::id_and_class_selector_values_equal(
            document_mode,
            before_value,
            after_value,
        ) {
            return Ok(());
        }
        for candidate in before_value.into_iter().chain(after_value) {
            budget.consume(1)?;
            record_candidate_lookup(CandidateLookupKind::Id);
            let Some(group) = find_selector_value_group(&index.id_values, candidate, document_mode)
            else {
                continue;
            };
            if !group_has_applicable_effect(group, element_namespace, budget)? {
                continue;
            }
            budget.consume(1)?;
            let before_matches = crate::selectors::matching::matches_id_in_attributes(
                element_namespace,
                attribute_views(before),
                document_mode,
                &group.key,
            );
            let after_matches = crate::selectors::matching::matches_id_in_attributes(
                element_namespace,
                attribute_views(after),
                document_mode,
                &group.key,
            );
            if before_matches != after_matches {
                matched.id = true;
                break;
            }
        }
        Ok(())
    }

    fn classify_class_transition(
        &self,
        index: &SelectorDependencyIndex,
        element_namespace: html::ElementNamespace,
        before: &[html::ParserCreatedAttribute],
        after: &[html::ParserCreatedAttribute],
        budget: &mut DependencyEvaluationBudget,
        matched: &mut AttributeDependencyMatch,
    ) -> Result<(), DependencyClassificationFailure> {
        if index.class_tokens.is_empty() {
            return Ok(());
        }
        let document_mode = self.matching_environment.document_mode();
        let before_value = effective_unqualified_attribute(before, element_namespace, "class")
            .map(SelectorDomAttribute::value);
        let after_value = effective_unqualified_attribute(after, element_namespace, "class")
            .map(SelectorDomAttribute::value);
        if crate::selectors::matching::id_and_class_selector_values_equal(
            document_mode,
            before_value,
            after_value,
        ) {
            return Ok(());
        }
        let mut previous_candidate = None;
        for value in before_value.into_iter().chain(after_value) {
            record_class_value_tokenization();
            for candidate in crate::selectors::matching::css_whitespace_separated_tokens(value) {
                budget.consume(1)?;
                record_class_token_visit();
                if previous_candidate.is_some_and(|previous| {
                    crate::selectors::matching::compare_id_and_class_selector_values(
                        document_mode,
                        previous,
                        candidate,
                    )
                    .is_eq()
                }) {
                    continue;
                }
                previous_candidate = Some(candidate);
                record_candidate_lookup(CandidateLookupKind::Class);
                let Some(group) =
                    find_selector_value_group(&index.class_tokens, candidate, document_mode)
                else {
                    continue;
                };
                if !group_has_applicable_effect(group, element_namespace, budget)? {
                    continue;
                }
                budget.consume(1)?;
                let before_matches = crate::selectors::matching::matches_class_in_attributes(
                    element_namespace,
                    attribute_views(before),
                    document_mode,
                    &group.key,
                );
                let after_matches = crate::selectors::matching::matches_class_in_attributes(
                    element_namespace,
                    attribute_views(after),
                    document_mode,
                    &group.key,
                );
                if before_matches != after_matches {
                    matched.class = true;
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    fn classify_named_attribute_transitions(
        &self,
        index: &SelectorDependencyIndex,
        element_namespace: html::ElementNamespace,
        before: &[html::ParserCreatedAttribute],
        after: &[html::ParserCreatedAttribute],
        budget: &mut DependencyEvaluationBudget,
        matched: &mut AttributeDependencyMatch,
    ) -> Result<(), DependencyClassificationFailure> {
        if index.attributes.is_empty() {
            return Ok(());
        }
        for attribute in before.iter().chain(after) {
            budget.consume(1)?;
            if attribute.namespace() != html::AttributeNamespace::None {
                continue;
            }
            record_candidate_lookup(CandidateLookupKind::Attribute);
            let Some(group) = find_attribute_group_for_dom_name(
                &index.attributes,
                attribute.local_name(),
                element_namespace,
            ) else {
                continue;
            };
            for predicate in &group.predicates {
                if !effects_apply(&predicate.effects, element_namespace, budget)? {
                    continue;
                }
                budget.consume(1)?;
                let selector_name = name_dependency_text(&group.name);
                let predicate_input = match &predicate.predicate {
                    AttributeDependencyPredicate::Exists => None,
                    AttributeDependencyPredicate::Match { matcher, value } => {
                        Some((*matcher, value.as_str()))
                    }
                };
                let before_matches = crate::selectors::matching::matches_attribute_in_attributes(
                    element_namespace,
                    attribute_views(before),
                    selector_name,
                    predicate_input,
                );
                let after_matches = crate::selectors::matching::matches_attribute_in_attributes(
                    element_namespace,
                    attribute_views(after),
                    selector_name,
                    predicate_input,
                );
                if before_matches != after_matches {
                    matched.attribute = true;
                    break;
                }
            }
            if matched.attribute {
                return Ok(());
            }
        }
        Ok(())
    }

    pub(super) fn has_empty_dependency_for_namespace(
        &self,
        element_namespace: html::ElementNamespace,
        budget: &mut DependencyEvaluationBudget,
    ) -> Result<bool, DependencyClassificationFailure> {
        let index = self
            .complete_index()
            .ok_or(DependencyClassificationFailure::ArtifactUnavailable)?;
        let group = find_keyed_group(
            &index.structural,
            &StructuralDependencyKind::EmptyDirectContent,
        );
        group.map_or(Ok(false), |group| {
            effects_apply(&group.effects, element_namespace, budget)
        })
    }

    pub fn to_debug_snapshot(&self) -> String {
        serialize_dependency_debug_snapshot(self)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct AttributeDependencyMatch {
    pub(super) any: bool,
    pub(super) id: bool,
    pub(super) class: bool,
    pub(super) attribute: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum DependencyClassificationFailure {
    ArtifactUnavailable,
    EvaluationLimitExceeded { configured: usize, observed: usize },
    CounterExhausted { counter: &'static str },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DependencyEvaluationBudget {
    configured: usize,
    consumed: usize,
}

impl DependencyEvaluationBudget {
    fn new(configured: usize) -> Self {
        Self {
            configured,
            consumed: 0,
        }
    }

    fn consume(&mut self, count: usize) -> Result<(), DependencyClassificationFailure> {
        let observed = self.consumed.checked_add(count).ok_or(
            DependencyClassificationFailure::CounterExhausted {
                counter: "selector-dependency-evaluation-count",
            },
        )?;
        if observed > self.configured {
            return Err(DependencyClassificationFailure::EvaluationLimitExceeded {
                configured: self.configured,
                observed,
            });
        }
        self.consumed = observed;
        Ok(())
    }

    #[cfg(test)]
    fn consumed(self) -> usize {
        self.consumed
    }
}

impl SelectorDependencyIndex {
    fn try_from_records(
        records: Vec<SelectorDependencyRecord>,
    ) -> Result<Self, StyleDependencyBuildFailure> {
        let mut index = Self::default();
        for record in records {
            match record.trigger {
                SelectorDependencyTrigger::TypeName(key) => {
                    push_keyed_effect(&mut index.type_names, key, record.effect)?;
                }
                SelectorDependencyTrigger::IdValue(key) => {
                    push_keyed_effect(&mut index.id_values, key, record.effect)?;
                }
                SelectorDependencyTrigger::ClassToken(key) => {
                    push_keyed_effect(&mut index.class_tokens, key, record.effect)?;
                }
                SelectorDependencyTrigger::Attribute(key) => {
                    push_attribute_effect(
                        &mut index.attributes,
                        key.name,
                        key.predicate,
                        record.effect,
                    )?;
                }
                SelectorDependencyTrigger::Structural(key) => {
                    push_keyed_effect(&mut index.structural, key, record.effect)?;
                }
                SelectorDependencyTrigger::Relationship(key) => {
                    push_keyed_effect(&mut index.relationships, key, record.effect)?;
                }
            }
            index.record_count = index.record_count.checked_add(1).ok_or(
                StyleDependencyBuildFailure::CounterExhausted {
                    counter: "selector-dependency-index-record-count",
                },
            )?;
        }
        Ok(index)
    }

    fn visit_records(&self, mut visitor: impl FnMut(SelectorDependencyRecordView<'_>) -> bool) {
        for group in &self.type_names {
            for effect in &group.effects {
                if !visitor(SelectorDependencyRecordView::TypeName(&group.key, effect)) {
                    return;
                }
            }
        }
        for group in &self.id_values {
            for effect in &group.effects {
                if !visitor(SelectorDependencyRecordView::IdValue(&group.key, effect)) {
                    return;
                }
            }
        }
        for group in &self.class_tokens {
            for effect in &group.effects {
                if !visitor(SelectorDependencyRecordView::ClassToken(&group.key, effect)) {
                    return;
                }
            }
        }
        for group in &self.attributes {
            for predicate in &group.predicates {
                for effect in &predicate.effects {
                    if !visitor(SelectorDependencyRecordView::Attribute(
                        &group.name,
                        &predicate.predicate,
                        effect,
                    )) {
                        return;
                    }
                }
            }
        }
        for group in &self.structural {
            for effect in &group.effects {
                if !visitor(SelectorDependencyRecordView::Structural(group.key, effect)) {
                    return;
                }
            }
        }
        for group in &self.relationships {
            for effect in &group.effects {
                if !visitor(SelectorDependencyRecordView::Relationship(
                    group.key, effect,
                )) {
                    return;
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum SelectorDependencyRecordView<'a> {
    TypeName(&'a NameDependencyKey, &'a SelectorDependencyEffect),
    IdValue(&'a str, &'a SelectorDependencyEffect),
    ClassToken(&'a str, &'a SelectorDependencyEffect),
    Attribute(
        &'a NameDependencyKey,
        &'a AttributeDependencyPredicate,
        &'a SelectorDependencyEffect,
    ),
    Structural(StructuralDependencyKind, &'a SelectorDependencyEffect),
    Relationship(RelationshipDependencyKind, &'a SelectorDependencyEffect),
}

fn push_keyed_effect<K: Eq>(
    groups: &mut Vec<KeyedDependencyGroup<K>>,
    key: K,
    effect: SelectorDependencyEffect,
) -> Result<(), StyleDependencyBuildFailure> {
    if groups.last().is_some_and(|group| group.key == key) {
        let effects = &mut groups.last_mut().expect("last group exists").effects;
        effects
            .try_reserve(1)
            .map_err(|_| StyleDependencyBuildFailure::Reservation {
                storage: StyleDependencyStorage::IndexEffects,
            })?;
        effects.push(effect);
        return Ok(());
    }
    let mut effects = Vec::new();
    effects
        .try_reserve_exact(1)
        .map_err(|_| StyleDependencyBuildFailure::Reservation {
            storage: StyleDependencyStorage::IndexEffects,
        })?;
    effects.push(effect);
    groups
        .try_reserve(1)
        .map_err(|_| StyleDependencyBuildFailure::Reservation {
            storage: StyleDependencyStorage::IndexGroups,
        })?;
    groups.push(KeyedDependencyGroup { key, effects });
    Ok(())
}

fn push_attribute_effect(
    groups: &mut Vec<AttributeDependencyGroup>,
    name: NameDependencyKey,
    predicate: AttributeDependencyPredicate,
    effect: SelectorDependencyEffect,
) -> Result<(), StyleDependencyBuildFailure> {
    if groups.last().is_none_or(|group| group.name != name) {
        groups
            .try_reserve(1)
            .map_err(|_| StyleDependencyBuildFailure::Reservation {
                storage: StyleDependencyStorage::IndexGroups,
            })?;
        groups.push(AttributeDependencyGroup {
            name,
            predicates: Vec::new(),
        });
    }
    let group = groups.last_mut().expect("attribute group exists");
    if group
        .predicates
        .last()
        .is_some_and(|group| group.predicate == predicate)
    {
        let effects = &mut group
            .predicates
            .last_mut()
            .expect("attribute predicate group exists")
            .effects;
        effects
            .try_reserve(1)
            .map_err(|_| StyleDependencyBuildFailure::Reservation {
                storage: StyleDependencyStorage::IndexEffects,
            })?;
        effects.push(effect);
        return Ok(());
    }
    let mut effects = Vec::new();
    effects
        .try_reserve_exact(1)
        .map_err(|_| StyleDependencyBuildFailure::Reservation {
            storage: StyleDependencyStorage::IndexEffects,
        })?;
    effects.push(effect);
    group
        .predicates
        .try_reserve(1)
        .map_err(|_| StyleDependencyBuildFailure::Reservation {
            storage: StyleDependencyStorage::IndexGroups,
        })?;
    group
        .predicates
        .push(AttributePredicateDependencyGroup { predicate, effects });
    Ok(())
}

struct DependencyBuilder<'a> {
    limits: &'a StyleResolutionLimits,
    matching_environment: SelectorMatchingEnvironment,
    records: Vec<SelectorDependencyRecord>,
    raw_records: usize,
    owned_bytes: usize,
    path_steps: usize,
    summary: StyleDependencySummary,
}

impl<'a> DependencyBuilder<'a> {
    fn new(
        limits: &'a StyleResolutionLimits,
        matching_environment: SelectorMatchingEnvironment,
    ) -> Self {
        Self {
            limits,
            matching_environment,
            records: Vec::new(),
            raw_records: 0,
            owned_bytes: 0,
            path_steps: 0,
            summary: StyleDependencySummary::default(),
        }
    }

    fn collect(
        &mut self,
        collection: &RuleCollection<'_>,
    ) -> Result<(), StyleDependencyBuildFailure> {
        for rule in collection.rules() {
            match rule {
                CollectedRule::ActiveStyle(rule) => {
                    let participates =
                        collection
                            .declarations_for_rule(rule)
                            .iter()
                            .any(|declaration| {
                                matches!(
                                    declaration.applicability(),
                                    CascadeDeclarationApplicability::Supported(_)
                                )
                            });
                    if !participates {
                        self.summary.active_without_supported_declarations = checked_increment(
                            self.summary.active_without_supported_declarations,
                            "active-without-supported-declarations-count",
                        )?;
                        continue;
                    }
                    self.summary.participating_rules = checked_increment(
                        self.summary.participating_rules,
                        "participating-rule-count",
                    )?;
                    for selector in rule.selectors().iter() {
                        self.summary.participating_selectors = checked_increment(
                            self.summary.participating_selectors,
                            "participating-selector-count",
                        )?;
                        self.collect_selector(selector, rule.namespace_constraint())?;
                    }
                }
                CollectedRule::InactiveStyle(rule) => match rule.reason() {
                    crate::cascade::InactiveStyleRuleReason::InvalidSelector { .. } => {
                        self.summary.inactive_invalid_rules = checked_increment(
                            self.summary.inactive_invalid_rules,
                            "inactive-invalid-rule-count",
                        )?;
                    }
                    crate::cascade::InactiveStyleRuleReason::UnsupportedSelector { .. } => {
                        self.summary.inactive_unsupported_rules = checked_increment(
                            self.summary.inactive_unsupported_rules,
                            "inactive-unsupported-rule-count",
                        )?;
                    }
                    crate::cascade::InactiveStyleRuleReason::StylesheetConditionDeferred {
                        ..
                    } => {
                        self.summary.inactive_deferred_rules = checked_increment(
                            self.summary.inactive_deferred_rules,
                            "inactive-deferred-rule-count",
                        )?;
                    }
                },
                CollectedRule::SkippedAtRule(_) => {}
            }
        }
        Ok(())
    }

    fn collect_selector(
        &mut self,
        selector: &ComplexSelector,
        namespace_constraint: SelectorNamespaceConstraint,
    ) -> Result<(), StyleDependencyBuildFailure> {
        let mut compounds = Vec::new();
        compounds
            .try_reserve_exact(selector.tail().len().checked_add(1).ok_or(
                StyleDependencyBuildFailure::CounterExhausted {
                    counter: "selector-compound-count",
                },
            )?)
            .map_err(|_| StyleDependencyBuildFailure::Reservation {
                storage: StyleDependencyStorage::SubjectPath,
            })?;
        compounds.push(selector.head());
        compounds.extend(selector.tail().iter().map(|combined| combined.selector()));

        let mut combinators = Vec::new();
        combinators
            .try_reserve_exact(selector.tail().len())
            .map_err(|_| StyleDependencyBuildFailure::Reservation {
                storage: StyleDependencyStorage::SubjectPath,
            })?;
        combinators.extend(selector.tail().iter().map(|combined| combined.combinator()));

        for (compound_index, compound) in compounds.iter().enumerate() {
            let subject_path = &combinators[compound_index..];
            if let Some(TypeSelector::Named(named)) = compound.type_selector() {
                self.push_type_name_records(
                    named.name().text(),
                    namespace_constraint,
                    subject_path,
                )?;
            }
            for subclass in compound.subclasses() {
                let trigger = match subclass {
                    SubclassSelector::Id(selector) => SelectorDependencyTrigger::IdValue(
                        self.copy_selector_value_key(selector.name().text())?,
                    ),
                    SubclassSelector::Class(selector) => SelectorDependencyTrigger::ClassToken(
                        self.copy_selector_value_key(selector.name().text())?,
                    ),
                    SubclassSelector::Attribute(selector) => {
                        self.push_attribute_records(selector, namespace_constraint, subject_path)?;
                        continue;
                    }
                    SubclassSelector::TreeStructuralPseudoClass(selector) => {
                        SelectorDependencyTrigger::Structural(match selector.pseudo_class() {
                            TreeStructuralPseudoClass::Root => StructuralDependencyKind::Root,
                            TreeStructuralPseudoClass::Empty => {
                                StructuralDependencyKind::EmptyDirectContent
                            }
                            TreeStructuralPseudoClass::FirstChild => {
                                StructuralDependencyKind::FirstChildOrder
                            }
                            TreeStructuralPseudoClass::LastChild => {
                                StructuralDependencyKind::LastChildOrder
                            }
                            TreeStructuralPseudoClass::OnlyChild => {
                                StructuralDependencyKind::OnlyChildOrder
                            }
                        })
                    }
                };
                self.push_record_with_path(trigger, namespace_constraint, subject_path)?;
            }
            if let Some(combinator) = combinators.get(compound_index).copied() {
                self.push_record_with_path(
                    SelectorDependencyTrigger::Relationship(relationship_kind(combinator)),
                    namespace_constraint,
                    subject_path,
                )?;
            }
        }
        Ok(())
    }

    fn push_type_name_records(
        &mut self,
        name: &str,
        namespace_constraint: SelectorNamespaceConstraint,
        subject_path: &[Combinator],
    ) -> Result<(), StyleDependencyBuildFailure> {
        match namespace_constraint {
            SelectorNamespaceConstraint::Exact(html::ElementNamespace::Html) => {
                let name = NameDependencyKey::HtmlAsciiLowercase(
                    self.copy_selector_text_with_ascii_lowercase(name)?,
                );
                self.push_record_with_path(
                    SelectorDependencyTrigger::TypeName(name),
                    namespace_constraint,
                    subject_path,
                )
            }
            SelectorNamespaceConstraint::Exact(
                html::ElementNamespace::Svg | html::ElementNamespace::MathMl,
            ) => {
                let name = NameDependencyKey::Exact(self.copy_selector_text(name)?);
                self.push_record_with_path(
                    SelectorDependencyTrigger::TypeName(name),
                    namespace_constraint,
                    subject_path,
                )
            }
            SelectorNamespaceConstraint::Unconstrained => {
                let html_name = NameDependencyKey::HtmlAsciiLowercase(
                    self.copy_selector_text_with_ascii_lowercase(name)?,
                );
                self.push_record_with_path(
                    SelectorDependencyTrigger::TypeName(html_name),
                    namespace_constraint,
                    subject_path,
                )?;
                let exact_name = NameDependencyKey::Exact(self.copy_selector_text(name)?);
                self.push_record_with_path(
                    SelectorDependencyTrigger::TypeName(exact_name),
                    namespace_constraint,
                    subject_path,
                )
            }
        }
    }

    fn push_attribute_records(
        &mut self,
        selector: &AttributeSelector,
        namespace_constraint: SelectorNamespaceConstraint,
        subject_path: &[Combinator],
    ) -> Result<(), StyleDependencyBuildFailure> {
        let selector_name = match selector {
            AttributeSelector::Exists(selector) => selector.name().text(),
            AttributeSelector::Match(selector) => selector.name().text(),
        };
        match namespace_constraint {
            SelectorNamespaceConstraint::Exact(html::ElementNamespace::Html) => {
                let name = NameDependencyKey::HtmlAsciiLowercase(
                    self.copy_selector_text_with_ascii_lowercase(selector_name)?,
                );
                let predicate = self.copy_attribute_predicate(selector)?;
                self.push_record_with_path(
                    SelectorDependencyTrigger::Attribute(AttributeDependencyKey {
                        name,
                        predicate,
                    }),
                    namespace_constraint,
                    subject_path,
                )
            }
            SelectorNamespaceConstraint::Exact(
                html::ElementNamespace::Svg | html::ElementNamespace::MathMl,
            ) => {
                let name = NameDependencyKey::Exact(self.copy_selector_text(selector_name)?);
                let predicate = self.copy_attribute_predicate(selector)?;
                self.push_record_with_path(
                    SelectorDependencyTrigger::Attribute(AttributeDependencyKey {
                        name,
                        predicate,
                    }),
                    namespace_constraint,
                    subject_path,
                )
            }
            SelectorNamespaceConstraint::Unconstrained => {
                let html_name = NameDependencyKey::HtmlAsciiLowercase(
                    self.copy_selector_text_with_ascii_lowercase(selector_name)?,
                );
                let html_predicate = self.copy_attribute_predicate(selector)?;
                self.push_record_with_path(
                    SelectorDependencyTrigger::Attribute(AttributeDependencyKey {
                        name: html_name,
                        predicate: html_predicate,
                    }),
                    namespace_constraint,
                    subject_path,
                )?;
                let exact_name = NameDependencyKey::Exact(self.copy_selector_text(selector_name)?);
                let exact_predicate = self.copy_attribute_predicate(selector)?;
                self.push_record_with_path(
                    SelectorDependencyTrigger::Attribute(AttributeDependencyKey {
                        name: exact_name,
                        predicate: exact_predicate,
                    }),
                    namespace_constraint,
                    subject_path,
                )
            }
        }
    }

    fn copy_attribute_predicate(
        &mut self,
        selector: &AttributeSelector,
    ) -> Result<AttributeDependencyPredicate, StyleDependencyBuildFailure> {
        match selector {
            AttributeSelector::Exists(_) => Ok(AttributeDependencyPredicate::Exists),
            AttributeSelector::Match(selector) => Ok(AttributeDependencyPredicate::Match {
                matcher: selector.matcher(),
                value: self.copy_selector_text(attribute_value(selector.value()))?,
            }),
        }
    }

    fn copy_selector_value_key(
        &mut self,
        text: &str,
    ) -> Result<String, StyleDependencyBuildFailure> {
        if self.matching_environment.document_mode() == html::DocumentMode::Quirks {
            self.copy_selector_text_with_ascii_lowercase(text)
        } else {
            self.copy_selector_text(text)
        }
    }

    fn push_record_with_path(
        &mut self,
        trigger: SelectorDependencyTrigger,
        namespace_constraint: SelectorNamespaceConstraint,
        combinators: &[Combinator],
    ) -> Result<(), StyleDependencyBuildFailure> {
        let subject_path = self.copy_path(combinators)?;
        self.push_record(
            trigger,
            SelectorDependencyEffect {
                namespace_constraint,
                subject_path,
            },
        )
    }

    fn copy_path(
        &mut self,
        combinators: &[Combinator],
    ) -> Result<SelectorSubjectPath, StyleDependencyBuildFailure> {
        let observed = self.path_steps.checked_add(combinators.len()).ok_or(
            StyleDependencyBuildFailure::CounterExhausted {
                counter: "selector-dependency-path-step-count",
            },
        )?;
        if observed > self.limits.max_selector_dependency_path_steps_per_document {
            return Err(StyleDependencyBuildFailure::LimitExceeded {
                limit: StyleResolutionLimit::SelectorDependencyPathStepsPerDocument,
                configured: self.limits.max_selector_dependency_path_steps_per_document,
                observed,
            });
        }
        let mut steps = Vec::new();
        steps.try_reserve_exact(combinators.len()).map_err(|_| {
            StyleDependencyBuildFailure::Reservation {
                storage: StyleDependencyStorage::SubjectPath,
            }
        })?;
        steps.extend(combinators.iter().copied().map(effect_step));
        self.path_steps = observed;
        Ok(SelectorSubjectPath { steps })
    }

    fn copy_selector_text(&mut self, text: &str) -> Result<String, StyleDependencyBuildFailure> {
        let observed = self.owned_bytes.checked_add(text.len()).ok_or(
            StyleDependencyBuildFailure::CounterExhausted {
                counter: "selector-dependency-byte-count",
            },
        )?;
        if observed > self.limits.max_selector_dependency_bytes_per_document {
            return Err(StyleDependencyBuildFailure::LimitExceeded {
                limit: StyleResolutionLimit::SelectorDependencyBytesPerDocument,
                configured: self.limits.max_selector_dependency_bytes_per_document,
                observed,
            });
        }
        let mut owned = String::new();
        owned.try_reserve_exact(text.len()).map_err(|_| {
            StyleDependencyBuildFailure::Reservation {
                storage: StyleDependencyStorage::SelectorText,
            }
        })?;
        owned.push_str(text);
        self.owned_bytes = observed;
        Ok(owned)
    }

    fn copy_selector_text_with_ascii_lowercase(
        &mut self,
        text: &str,
    ) -> Result<String, StyleDependencyBuildFailure> {
        let mut owned = self.copy_selector_text(text)?;
        owned.make_ascii_lowercase();
        Ok(owned)
    }

    fn push_record(
        &mut self,
        trigger: SelectorDependencyTrigger,
        effect: SelectorDependencyEffect,
    ) -> Result<(), StyleDependencyBuildFailure> {
        let observed = self.raw_records.checked_add(1).ok_or(
            StyleDependencyBuildFailure::CounterExhausted {
                counter: "selector-dependency-record-count",
            },
        )?;
        if observed > self.limits.max_selector_dependency_records_per_document {
            return Err(StyleDependencyBuildFailure::LimitExceeded {
                limit: StyleResolutionLimit::SelectorDependencyRecordsPerDocument,
                configured: self.limits.max_selector_dependency_records_per_document,
                observed,
            });
        }
        self.records
            .try_reserve(1)
            .map_err(|_| StyleDependencyBuildFailure::Reservation {
                storage: StyleDependencyStorage::Records,
            })?;
        self.records
            .push(SelectorDependencyRecord { trigger, effect });
        self.raw_records = observed;
        Ok(())
    }
}

fn checked_increment(
    value: usize,
    counter: &'static str,
) -> Result<usize, StyleDependencyBuildFailure> {
    value
        .checked_add(1)
        .ok_or(StyleDependencyBuildFailure::CounterExhausted { counter })
}

fn effective_unqualified_attribute<'a>(
    attributes: &'a [html::ParserCreatedAttribute],
    element_namespace: html::ElementNamespace,
    requested_name: &str,
) -> Option<SelectorDomAttribute<'a>> {
    crate::dom_attributes::first_effective_unqualified_attribute(
        element_namespace,
        attribute_views(attributes),
        requested_name,
    )
}

fn find_selector_value_group<'a>(
    groups: &'a [KeyedDependencyGroup<String>],
    key: &str,
    document_mode: html::DocumentMode,
) -> Option<&'a KeyedDependencyGroup<String>> {
    groups
        .binary_search_by(|group| {
            crate::selectors::matching::compare_id_and_class_selector_values(
                document_mode,
                group.key.as_str(),
                key,
            )
        })
        .ok()
        .map(|index| &groups[index])
}

fn find_keyed_group<'a, K: Ord>(
    groups: &'a [KeyedDependencyGroup<K>],
    key: &K,
) -> Option<&'a KeyedDependencyGroup<K>> {
    groups
        .binary_search_by(|group| group.key.cmp(key))
        .ok()
        .map(|index| &groups[index])
}

fn find_attribute_group_for_dom_name<'a>(
    groups: &'a [AttributeDependencyGroup],
    dom_local_name: &str,
    element_namespace: html::ElementNamespace,
) -> Option<&'a AttributeDependencyGroup> {
    let wants_html_key = element_namespace == html::ElementNamespace::Html;
    groups
        .binary_search_by(|group| match (&group.name, wants_html_key) {
            (NameDependencyKey::HtmlAsciiLowercase(name), true)
            | (NameDependencyKey::Exact(name), false) => name.as_str().cmp(dom_local_name),
            (NameDependencyKey::HtmlAsciiLowercase(_), false) => std::cmp::Ordering::Less,
            (NameDependencyKey::Exact(_), true) => std::cmp::Ordering::Greater,
        })
        .ok()
        .map(|index| &groups[index])
}

#[derive(Clone, Copy)]
enum CandidateLookupKind {
    Id,
    Class,
    Attribute,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct DependencyClassificationTestCounters {
    class_values_tokenized: usize,
    class_tokens_visited: usize,
    id_candidate_lookups: usize,
    class_candidate_lookups: usize,
    attribute_candidate_lookups: usize,
}

#[cfg(test)]
thread_local! {
    static DEPENDENCY_CLASSIFICATION_TEST_COUNTERS:
        std::cell::Cell<DependencyClassificationTestCounters> =
            const { std::cell::Cell::new(DependencyClassificationTestCounters {
                class_values_tokenized: 0,
                class_tokens_visited: 0,
                id_candidate_lookups: 0,
                class_candidate_lookups: 0,
                attribute_candidate_lookups: 0,
            }) };
}

fn record_candidate_lookup(kind: CandidateLookupKind) {
    #[cfg(test)]
    DEPENDENCY_CLASSIFICATION_TEST_COUNTERS.with(|cell| {
        let mut counters = cell.get();
        match kind {
            CandidateLookupKind::Id => increment_test_counter(&mut counters.id_candidate_lookups),
            CandidateLookupKind::Class => {
                increment_test_counter(&mut counters.class_candidate_lookups)
            }
            CandidateLookupKind::Attribute => {
                increment_test_counter(&mut counters.attribute_candidate_lookups)
            }
        }
        cell.set(counters);
    });
    #[cfg(not(test))]
    let _ = kind;
}

fn record_class_value_tokenization() {
    #[cfg(test)]
    DEPENDENCY_CLASSIFICATION_TEST_COUNTERS.with(|cell| {
        let mut counters = cell.get();
        increment_test_counter(&mut counters.class_values_tokenized);
        cell.set(counters);
    });
}

fn record_class_token_visit() {
    #[cfg(test)]
    DEPENDENCY_CLASSIFICATION_TEST_COUNTERS.with(|cell| {
        let mut counters = cell.get();
        increment_test_counter(&mut counters.class_tokens_visited);
        cell.set(counters);
    });
}

#[cfg(test)]
fn increment_test_counter(counter: &mut usize) {
    *counter = counter.checked_add(1).expect("test counter exhausted");
}

#[cfg(test)]
fn reset_dependency_classification_test_counters() {
    DEPENDENCY_CLASSIFICATION_TEST_COUNTERS.with(|cell| {
        cell.set(DependencyClassificationTestCounters::default());
    });
}

#[cfg(test)]
fn dependency_classification_test_counters() -> DependencyClassificationTestCounters {
    DEPENDENCY_CLASSIFICATION_TEST_COUNTERS.with(std::cell::Cell::get)
}

fn group_has_applicable_effect<K>(
    group: &KeyedDependencyGroup<K>,
    element_namespace: html::ElementNamespace,
    budget: &mut DependencyEvaluationBudget,
) -> Result<bool, DependencyClassificationFailure> {
    effects_apply(&group.effects, element_namespace, budget)
}

fn effects_apply(
    effects: &[SelectorDependencyEffect],
    element_namespace: html::ElementNamespace,
    budget: &mut DependencyEvaluationBudget,
) -> Result<bool, DependencyClassificationFailure> {
    for effect in effects {
        budget.consume(1)?;
        if namespace_applies(effect.namespace_constraint, element_namespace) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn name_dependency_text(key: &NameDependencyKey) -> &str {
    match key {
        NameDependencyKey::HtmlAsciiLowercase(name) | NameDependencyKey::Exact(name) => name,
    }
}

fn attribute_views(
    attributes: &[html::ParserCreatedAttribute],
) -> impl ExactSizeIterator<Item = SelectorDomAttribute<'_>> {
    attributes.iter().map(|attribute| {
        SelectorDomAttribute::new(
            attribute.namespace(),
            attribute.local_name(),
            attribute.value(),
        )
    })
}

fn namespace_applies(
    constraint: SelectorNamespaceConstraint,
    namespace: html::ElementNamespace,
) -> bool {
    match constraint {
        SelectorNamespaceConstraint::Unconstrained => true,
        SelectorNamespaceConstraint::Exact(expected) => expected == namespace,
    }
}

fn attribute_value(value: &AttributeValue) -> &str {
    match value {
        AttributeValue::Ident(value) => value.text(),
        AttributeValue::String(value) => value.value(),
    }
}

fn effect_step(combinator: Combinator) -> SelectorEffectStep {
    match combinator {
        Combinator::Descendant => SelectorEffectStep::Descendants,
        Combinator::Child => SelectorEffectStep::DirectChildren,
        Combinator::NextSibling => SelectorEffectStep::NextSibling,
        Combinator::SubsequentSibling => SelectorEffectStep::FollowingSiblings,
    }
}

fn relationship_kind(combinator: Combinator) -> RelationshipDependencyKind {
    match combinator {
        Combinator::Descendant => RelationshipDependencyKind::DescendantAncestry,
        Combinator::Child => RelationshipDependencyKind::DirectParentChild,
        Combinator::NextSibling => RelationshipDependencyKind::AdjacentSibling,
        Combinator::SubsequentSibling => RelationshipDependencyKind::FollowingSiblings,
    }
}

fn serialize_dependency_debug_snapshot(artifact: &StyleDependencyArtifact) -> String {
    let (total_records, visible_records, visible_record_bytes, truncated) = match &artifact.state {
        StyleDependencyArtifactState::Complete(index) => {
            let record_byte_budget = STYLE_DEPENDENCY_DEBUG_MAX_SERIALIZED_BYTES
                .checked_sub(STYLE_DEPENDENCY_DEBUG_RESERVED_METADATA_BYTES)
                .expect("dependency debug metadata reservation fits configured byte limit");
            let mut visible = 0usize;
            let mut serialized_record_bytes = 0usize;
            index.visit_records(|record| {
                if visible == STYLE_DEPENDENCY_DEBUG_MAX_RECORDS {
                    return false;
                }
                let mut counter = CountingWriter::default();
                write_dependency_record(&mut counter, visible, record)
                    .expect("counting dependency record cannot fail");
                let Some(observed) = serialized_record_bytes.checked_add(counter.bytes) else {
                    return false;
                };
                if observed > record_byte_budget {
                    return false;
                }
                serialized_record_bytes = observed;
                visible += 1;
                true
            });
            (
                index.record_count,
                visible,
                serialized_record_bytes,
                visible < index.record_count,
            )
        }
        StyleDependencyArtifactState::ConservativeUnavailable(_) => (0, 0, 0, false),
    };

    let mut out = String::new();
    let initial_capacity = visible_record_bytes
        .checked_add(STYLE_DEPENDENCY_DEBUG_RESERVED_METADATA_BYTES)
        .unwrap_or(STYLE_DEPENDENCY_DEBUG_MAX_SERIALIZED_BYTES)
        .min(STYLE_DEPENDENCY_DEBUG_MAX_SERIALIZED_BYTES);
    if out.try_reserve_exact(initial_capacity).is_err() {
        return "version: 1\naf9-style-dependencies\nstate: diagnostic-unavailable\ntruncated: true\n"
            .to_string();
    }
    writeln!(
        &mut out,
        "version: {STYLE_DEPENDENCY_ARTIFACT_DEBUG_VERSION}"
    )
    .expect("write dependency artifact snapshot");
    writeln!(&mut out, "af9-style-dependencies").expect("write dependency artifact snapshot");
    writeln!(
        &mut out,
        "environment: document-mode={}",
        document_mode_label(artifact.matching_environment.document_mode())
    )
    .expect("write dependency artifact snapshot");
    writeln!(
        &mut out,
        "rules: participating={} selectors={} inactive-invalid={} inactive-unsupported={} inactive-deferred={}",
        artifact.summary.participating_rules,
        artifact.summary.participating_selectors,
        artifact.summary.inactive_invalid_rules,
        artifact.summary.inactive_unsupported_rules,
        artifact.summary.inactive_deferred_rules,
    )
    .expect("write dependency artifact snapshot");
    writeln!(
        &mut out,
        "active-without-supported-declarations: {}",
        artifact.summary.active_without_supported_declarations,
    )
    .expect("write dependency artifact snapshot");
    writeln!(
        &mut out,
        "classification-limit: evaluations-per-publication={}",
        artifact.max_transition_evaluations_per_publication,
    )
    .expect("write dependency artifact snapshot");
    match &artifact.state {
        StyleDependencyArtifactState::Complete(index) => {
            writeln!(&mut out, "state: complete").expect("write dependency artifact snapshot");
            writeln!(
                &mut out,
                "records: total={total_records} visible={visible_records}"
            )
            .expect("write dependency artifact snapshot");
            writeln!(&mut out, "truncated: {truncated}")
                .expect("write dependency artifact snapshot");
            let mut position = 0usize;
            index.visit_records(|record| {
                if position == visible_records {
                    return false;
                }
                write_dependency_record(&mut out, position, record)
                    .expect("write dependency artifact record");
                position += 1;
                true
            });
        }
        StyleDependencyArtifactState::ConservativeUnavailable(error) => {
            writeln!(&mut out, "state: conservative-unavailable")
                .expect("write dependency artifact snapshot");
            writeln!(&mut out, "records: total=0 visible=0")
                .expect("write dependency artifact snapshot");
            writeln!(&mut out, "truncated: false").expect("write dependency artifact snapshot");
            write!(&mut out, "failure: kind={}", error.stable_label())
                .expect("write dependency artifact snapshot");
            match error {
                StyleDependencyBuildFailure::LimitExceeded {
                    limit,
                    configured,
                    observed,
                } => write!(
                    &mut out,
                    " limit={} configured={configured} observed={observed}",
                    limit.stable_label()
                )
                .expect("write dependency artifact snapshot"),
                StyleDependencyBuildFailure::CounterExhausted { counter } => {
                    write!(&mut out, " counter={counter}")
                        .expect("write dependency artifact snapshot")
                }
                StyleDependencyBuildFailure::Reservation { storage } => {
                    write!(&mut out, " storage={}", storage.stable_label())
                        .expect("write dependency artifact snapshot")
                }
            }
            writeln!(&mut out).expect("write dependency artifact snapshot");
        }
    }
    debug_assert!(out.len() <= STYLE_DEPENDENCY_DEBUG_MAX_SERIALIZED_BYTES);
    out
}

#[derive(Default)]
struct CountingWriter {
    bytes: usize,
}

impl Write for CountingWriter {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.bytes = self.bytes.checked_add(text.len()).ok_or(std::fmt::Error)?;
        Ok(())
    }
}

fn write_dependency_record(
    out: &mut impl Write,
    position: usize,
    record: SelectorDependencyRecordView<'_>,
) -> std::fmt::Result {
    write!(out, "  record[{position}]: trigger=")?;
    write_dependency_trigger(out, record)?;
    let effect = dependency_record_effect(record);
    write!(
        out,
        " namespace={} effect=",
        namespace_constraint_label(effect.namespace_constraint)
    )?;
    write_subject_path(out, &effect.subject_path)?;
    writeln!(out)
}

fn write_dependency_trigger(
    out: &mut impl Write,
    record: SelectorDependencyRecordView<'_>,
) -> std::fmt::Result {
    match record {
        SelectorDependencyRecordView::TypeName(name, _) => {
            write!(
                out,
                "type-{}({:?})",
                name_dependency_kind(name),
                name_dependency_text(name)
            )
        }
        SelectorDependencyRecordView::IdValue(value, _) => write!(out, "id({value:?})"),
        SelectorDependencyRecordView::ClassToken(value, _) => write!(out, "class({value:?})"),
        SelectorDependencyRecordView::Attribute(name, predicate, _) => {
            write!(
                out,
                "attribute-{}({:?}",
                name_dependency_kind(name),
                name_dependency_text(name)
            )?;
            if let AttributeDependencyPredicate::Match { matcher, value } = predicate {
                write!(out, " {} {value:?}", matcher_label(*matcher))?;
            }
            write!(out, ")")
        }
        SelectorDependencyRecordView::Structural(kind, _) => {
            write!(out, "structural({})", structural_label(kind))
        }
        SelectorDependencyRecordView::Relationship(kind, _) => {
            write!(out, "relationship({})", relationship_label(kind))
        }
    }
}

fn dependency_record_effect(record: SelectorDependencyRecordView<'_>) -> &SelectorDependencyEffect {
    match record {
        SelectorDependencyRecordView::TypeName(_, effect)
        | SelectorDependencyRecordView::IdValue(_, effect)
        | SelectorDependencyRecordView::ClassToken(_, effect)
        | SelectorDependencyRecordView::Attribute(_, _, effect)
        | SelectorDependencyRecordView::Structural(_, effect)
        | SelectorDependencyRecordView::Relationship(_, effect) => effect,
    }
}

fn name_dependency_kind(key: &NameDependencyKey) -> &'static str {
    match key {
        NameDependencyKey::HtmlAsciiLowercase(_) => "html",
        NameDependencyKey::Exact(_) => "exact",
    }
}

fn write_subject_path(out: &mut impl Write, path: &SelectorSubjectPath) -> std::fmt::Result {
    if path.steps.is_empty() {
        return write!(out, "self");
    }
    for (index, step) in path.steps.iter().enumerate() {
        if index > 0 {
            write!(out, " -> ")?;
        }
        write!(
            out,
            "{}",
            match step {
                SelectorEffectStep::Descendants => "descendants",
                SelectorEffectStep::DirectChildren => "direct-children",
                SelectorEffectStep::NextSibling => "next-sibling",
                SelectorEffectStep::FollowingSiblings => "following-siblings",
            }
        )?;
    }
    Ok(())
}

fn document_mode_label(mode: html::DocumentMode) -> &'static str {
    match mode {
        html::DocumentMode::NoQuirks => "no-quirks",
        html::DocumentMode::LimitedQuirks => "limited-quirks",
        html::DocumentMode::Quirks => "quirks",
    }
}

fn namespace_constraint_label(constraint: SelectorNamespaceConstraint) -> &'static str {
    match constraint {
        SelectorNamespaceConstraint::Unconstrained => "unconstrained",
        SelectorNamespaceConstraint::Exact(html::ElementNamespace::Html) => "html",
        SelectorNamespaceConstraint::Exact(html::ElementNamespace::Svg) => "svg",
        SelectorNamespaceConstraint::Exact(html::ElementNamespace::MathMl) => "mathml",
    }
}

fn matcher_label(matcher: AttributeMatcher) -> &'static str {
    match matcher {
        AttributeMatcher::Exact => "=",
        AttributeMatcher::Includes => "~=",
        AttributeMatcher::DashMatch => "|=",
        AttributeMatcher::Prefix => "^=",
        AttributeMatcher::Suffix => "$=",
        AttributeMatcher::Substring => "*=",
    }
}

fn structural_label(kind: StructuralDependencyKind) -> &'static str {
    match kind {
        StructuralDependencyKind::Root => "root",
        StructuralDependencyKind::EmptyDirectContent => "empty-direct-content",
        StructuralDependencyKind::FirstChildOrder => "first-child-order",
        StructuralDependencyKind::LastChildOrder => "last-child-order",
        StructuralDependencyKind::OnlyChildOrder => "only-child-order",
    }
}

fn relationship_label(kind: RelationshipDependencyKind) -> &'static str {
    match kind {
        RelationshipDependencyKind::DescendantAncestry => "descendant-ancestry",
        RelationshipDependencyKind::DirectParentChild => "direct-parent-child",
        RelationshipDependencyKind::AdjacentSibling => "adjacent-sibling",
        RelationshipDependencyKind::FollowingSiblings => "following-siblings",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ParseOptions, StylesheetCollectionInput, StylesheetConditionInput, StylesheetOrder,
        StylesheetSourceId, parse_stylesheet_with_options,
    };

    fn artifact(source: &str, limits: &StyleResolutionLimits) -> StyleDependencyArtifact {
        artifact_with_environment(
            source,
            limits,
            SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks),
        )
    }

    fn artifact_with_environment(
        source: &str,
        limits: &StyleResolutionLimits,
        environment: SelectorMatchingEnvironment,
    ) -> StyleDependencyArtifact {
        let sheet = parse_stylesheet_with_options(source, &ParseOptions::stylesheet());
        let input = StylesheetCollectionInput::author(
            StylesheetSourceId::in_memory_generation_index(1),
            StylesheetOrder::new(0),
            &sheet,
            StylesheetConditionInput::None,
        );
        let collection = RuleCollection::try_new(&[input], limits).expect("collection");
        StyleDependencyArtifact::from_rule_collection(&collection, environment, limits)
    }

    #[test]
    fn extraction_preserves_composed_subject_paths_and_relationships() {
        let artifact = artifact(
            ".a + .b .c { color: red; } * > * { width: 1px; }",
            &StyleResolutionLimits::default(),
        );
        let snapshot = artifact.to_debug_snapshot();
        assert_eq!(
            snapshot,
            concat!(
                "version: 1\n",
                "af9-style-dependencies\n",
                "environment: document-mode=no-quirks\n",
                "rules: participating=2 selectors=2 inactive-invalid=0 inactive-unsupported=0 inactive-deferred=0\n",
                "active-without-supported-declarations: 0\n",
                "classification-limit: evaluations-per-publication=4194304\n",
                "state: complete\n",
                "records: total=6 visible=6\n",
                "truncated: false\n",
                "  record[0]: trigger=class(\"a\") namespace=unconstrained effect=next-sibling -> descendants\n",
                "  record[1]: trigger=class(\"b\") namespace=unconstrained effect=descendants\n",
                "  record[2]: trigger=class(\"c\") namespace=unconstrained effect=self\n",
                "  record[3]: trigger=relationship(descendant-ancestry) namespace=unconstrained effect=descendants\n",
                "  record[4]: trigger=relationship(direct-parent-child) namespace=unconstrained effect=direct-children\n",
                "  record[5]: trigger=relationship(adjacent-sibling) namespace=unconstrained effect=next-sibling -> descendants\n",
            )
        );
    }

    #[test]
    fn raw_debug_is_compact_and_does_not_expose_semantic_dependencies() {
        const RAW_DEBUG_MAX_BYTES: usize = 512;
        const SENTINEL_CLASS: &str = "af9-raw-debug-sentinel-class";
        const SENTINEL_ID: &str = "af9-raw-debug-sentinel-id";
        const SENTINEL_ATTRIBUTE: &str = "data-af9-raw-debug-sentinel";

        let mut source = format!(
            ".{SENTINEL_CLASS} + #{SENTINEL_ID} [{SENTINEL_ATTRIBUTE}=sentinel-value] .subject {{ color: red; }}"
        );
        for index in 0..512 {
            source.push_str(&format!(".bulk-debug-key-{index} {{ color: blue; }}"));
        }
        let artifact = artifact(&source, &StyleResolutionLimits::default());

        let raw = format!("{artifact:?}");
        eprintln!(
            "AF9 compact raw dependency Debug: bytes={} semantic-records={}",
            raw.len(),
            artifact
                .complete_index()
                .expect("compact Debug fixture has a complete index")
                .record_count,
        );
        assert!(raw.starts_with("StyleDependencyArtifact"));
        assert!(raw.contains("state: \"complete\""));
        assert!(raw.contains("dependency_record_count: Some("));
        assert!(raw.contains("matching_environment:"));
        assert!(raw.len() <= RAW_DEBUG_MAX_BYTES, "raw debug was {raw:?}");
        for semantic_text in [
            SENTINEL_CLASS,
            SENTINEL_ID,
            SENTINEL_ATTRIBUTE,
            "sentinel-value",
            "bulk-debug-key-511",
            "SelectorDependencyIndex",
            "next-sibling",
            "record[",
        ] {
            assert!(
                !raw.contains(semantic_text),
                "raw debug exposed semantic dependency text {semantic_text:?}: {raw}"
            );
        }

        let detailed = artifact.to_debug_snapshot();
        assert!(detailed.starts_with("version: 1\naf9-style-dependencies\n"));
        assert!(detailed.contains(SENTINEL_CLASS));
        assert!(detailed.contains(SENTINEL_ID));
        assert!(detailed.contains(SENTINEL_ATTRIBUTE));
        assert!(detailed.contains("effect=next-sibling -> descendants"));
        assert!(detailed.len() <= STYLE_DEPENDENCY_DEBUG_MAX_SERIALIZED_BYTES);
    }

    #[test]
    fn inactive_and_non_candidate_rules_do_not_create_active_dependencies() {
        let artifact = artifact(
            concat!(
                ".active { color: red; }",
                ".unsupported:hover { color: blue; }",
                ".custom { --x: red; }",
                ".invalid-value { width: nope; }",
            ),
            &StyleResolutionLimits::default(),
        );
        let snapshot = artifact.to_debug_snapshot();
        assert!(snapshot.contains("class(\"active\")"));
        assert!(!snapshot.contains("class(\"custom\")"));
        assert!(!snapshot.contains("class(\"invalid-value\")"));
        assert!(snapshot.contains("inactive-unsupported=1"));
        assert!(snapshot.contains("active-without-supported-declarations: 2"));
    }

    #[test]
    fn dependency_limit_produces_typed_conservative_artifact() {
        let limits = StyleResolutionLimits {
            max_selector_dependency_records_per_document: 1,
            ..StyleResolutionLimits::default()
        };
        let artifact = artifact(".a.b { color: red; }", &limits);
        let snapshot = artifact.to_debug_snapshot();
        assert!(snapshot.contains("state: conservative-unavailable"));
        assert!(snapshot.contains("limit=selector-dependency-records-per-document"));
    }

    #[test]
    fn every_dependency_storage_dimension_has_a_typed_limit_failure() {
        let cases = [
            (
                ".a.b { color: red; }",
                StyleResolutionLimits {
                    max_selector_dependency_records_per_document: 1,
                    ..StyleResolutionLimits::default()
                },
                StyleResolutionLimit::SelectorDependencyRecordsPerDocument,
            ),
            (
                ".long-name { color: red; }",
                StyleResolutionLimits {
                    max_selector_dependency_bytes_per_document: 1,
                    ..StyleResolutionLimits::default()
                },
                StyleResolutionLimit::SelectorDependencyBytesPerDocument,
            ),
            (
                ".a.b .c { color: red; }",
                StyleResolutionLimits {
                    max_selector_dependency_path_steps_per_document: 2,
                    ..StyleResolutionLimits::default()
                },
                StyleResolutionLimit::SelectorDependencyPathStepsPerDocument,
            ),
        ];

        for (source, limits, expected_limit) in cases {
            let artifact = artifact(source, &limits);
            assert!(matches!(
                artifact.state,
                StyleDependencyArtifactState::ConservativeUnavailable(
                    StyleDependencyBuildFailure::LimitExceeded { limit, .. }
                ) if limit == expected_limit
            ));
        }
    }

    #[test]
    fn exact_class_id_and_attribute_transitions_use_matcher_semantics() {
        let artifact = artifact(
            ".hot, #hero, [data-kind~=promo] { color: red; }",
            &StyleResolutionLimits::default(),
        );
        let before = vec![
            html::internal::unqualified_attribute("class", "cold"),
            html::internal::unqualified_attribute("id", "other"),
            html::internal::unqualified_attribute("data-kind", "plain"),
        ];
        let after = vec![
            html::internal::unqualified_attribute("class", "cold hot"),
            html::internal::unqualified_attribute("id", "hero"),
            html::internal::unqualified_attribute("data-kind", "plain promo"),
        ];
        let mut budget = artifact.classification_budget();
        let matched = artifact
            .classify_attribute_transition(
                html::ElementNamespace::Html,
                &before,
                &after,
                &mut budget,
            )
            .expect("complete artifact");
        assert!(matched.any && matched.class && matched.id && matched.attribute);
    }

    #[test]
    fn dependency_transition_uses_the_authoritative_quirks_comparison() {
        let limits = StyleResolutionLimits::default();
        let quirks = artifact_with_environment(
            ".HOT { color: red; }",
            &limits,
            SelectorMatchingEnvironment::new(html::DocumentMode::Quirks),
        );
        let standards = artifact(".HOT { color: red; }", &limits);
        let before = Vec::new();
        let after = vec![html::internal::unqualified_attribute("class", "hot")];

        let mut quirks_budget = quirks.classification_budget();
        assert!(
            quirks
                .classify_attribute_transition(
                    html::ElementNamespace::Html,
                    &before,
                    &after,
                    &mut quirks_budget,
                )
                .is_ok_and(|matched| matched.class)
        );
        let mut standards_budget = standards.classification_budget();
        assert!(
            standards
                .classify_attribute_transition(
                    html::ElementNamespace::Html,
                    &before,
                    &after,
                    &mut standards_budget,
                )
                .is_ok_and(|matched| !matched.any)
        );
    }

    #[test]
    fn id_transition_uses_at_most_two_borrowed_environment_aware_probes() {
        let limits = StyleResolutionLimits::default();
        let quirks = artifact_with_environment(
            "#hero { color: red; }",
            &limits,
            SelectorMatchingEnvironment::new(html::DocumentMode::Quirks),
        );
        let standards = artifact("#hero { color: red; }", &limits);
        let before = vec![html::internal::unqualified_attribute("id", "HERO")];
        let after = vec![html::internal::unqualified_attribute("id", "hero")];

        reset_dependency_classification_test_counters();
        let mut quirks_budget = quirks.classification_budget();
        let quirks_match = quirks
            .classify_attribute_transition(
                html::ElementNamespace::Html,
                &before,
                &after,
                &mut quirks_budget,
            )
            .expect("quirks ID classification");
        let quirks_counters = dependency_classification_test_counters();
        assert!(!quirks_match.any);
        assert_eq!(quirks_counters.id_candidate_lookups, 0);

        reset_dependency_classification_test_counters();
        let mut standards_budget = standards.classification_budget();
        let standards_match = standards
            .classify_attribute_transition(
                html::ElementNamespace::Html,
                &before,
                &after,
                &mut standards_budget,
            )
            .expect("standards ID classification");
        let standards_counters = dependency_classification_test_counters();
        assert!(standards_match.id);
        assert_eq!(standards_counters.id_candidate_lookups, 2);
    }

    #[test]
    fn keyed_lookup_does_not_evaluate_unrelated_dependency_groups() {
        let mut source = String::new();
        for index in 0..2_000 {
            source.push_str(&format!(".unrelated-{index} {{ color: red; }}"));
        }
        source.push_str(".target { color: blue; }");
        let artifact = artifact(&source, &StyleResolutionLimits::default());
        let index = artifact.complete_index().expect("complete keyed index");
        assert!(index.record_count > 2_000);

        let before = Vec::new();
        let after = vec![html::internal::unqualified_attribute("class", "target")];
        let mut budget = artifact.classification_budget();
        let matched = artifact
            .classify_attribute_transition(
                html::ElementNamespace::Html,
                &before,
                &after,
                &mut budget,
            )
            .expect("key-directed classification");

        assert!(matched.class);
        assert!(
            budget.consumed() < 16,
            "classification work must be based on mutation candidates and the found key, not all retained records"
        );
    }

    #[test]
    fn unchanged_large_class_value_is_not_tokenized_for_an_attribute_transition() {
        let artifact = artifact(
            ".target, [title=new] { color: red; }",
            &StyleResolutionLimits::default(),
        );
        let large_class = "irrelevant ".repeat(100_000);
        let before = vec![
            html::internal::unqualified_attribute("class", large_class.clone()),
            html::internal::unqualified_attribute("title", "old"),
        ];
        let after = vec![
            html::internal::unqualified_attribute("class", large_class),
            html::internal::unqualified_attribute("title", "new"),
        ];

        reset_dependency_classification_test_counters();
        let mut budget = artifact.classification_budget();
        let matched = artifact
            .classify_attribute_transition(
                html::ElementNamespace::Html,
                &before,
                &after,
                &mut budget,
            )
            .expect("borrowed attribute classification");
        let counters = dependency_classification_test_counters();

        assert!(matched.attribute);
        assert_eq!(counters.class_values_tokenized, 0);
        assert_eq!(counters.class_tokens_visited, 0);
        assert_eq!(counters.class_candidate_lookups, 0);
        assert!(counters.attribute_candidate_lookups > 0);
    }

    #[test]
    fn repeated_class_tokens_use_borrowed_deduplicated_candidate_probes() {
        let artifact = artifact(".target { color: red; }", &StyleResolutionLimits::default());
        let before = Vec::new();
        let after = vec![html::internal::unqualified_attribute(
            "class",
            "irrelevant ".repeat(100_000),
        )];

        reset_dependency_classification_test_counters();
        let mut budget = artifact.classification_budget();
        let matched = artifact
            .classify_attribute_transition(
                html::ElementNamespace::Html,
                &before,
                &after,
                &mut budget,
            )
            .expect("bounded borrowed candidate classification");
        let counters = dependency_classification_test_counters();

        assert!(!matched.any);
        assert_eq!(counters.class_values_tokenized, 1);
        assert_eq!(counters.class_tokens_visited, 100_000);
        assert_eq!(counters.class_candidate_lookups, 1);
        assert_eq!(budget.consumed(), 100_000);
    }

    #[test]
    fn quirks_class_lookup_uses_borrowed_ascii_folded_probes() {
        let artifact = artifact_with_environment(
            ".target { color: red; }",
            &StyleResolutionLimits::default(),
            SelectorMatchingEnvironment::new(html::DocumentMode::Quirks),
        );
        let before = Vec::new();
        let after = vec![html::internal::unqualified_attribute(
            "class",
            "TaRgEt TARGET target",
        )];

        reset_dependency_classification_test_counters();
        let mut budget = artifact.classification_budget();
        let matched = artifact
            .classify_attribute_transition(
                html::ElementNamespace::Html,
                &before,
                &after,
                &mut budget,
            )
            .expect("allocation-free quirks lookup");
        let counters = dependency_classification_test_counters();

        assert!(matched.class);
        assert_eq!(counters.class_candidate_lookups, 1);
    }

    #[test]
    fn large_distinct_class_transition_exhausts_work_budget_conservatively() {
        let limits = StyleResolutionLimits {
            max_selector_dependency_evaluations_per_publication: 8,
            ..StyleResolutionLimits::default()
        };
        let artifact = artifact(".target { color: red; }", &limits);
        let before = Vec::new();
        let mut class_value = String::new();
        for index in 0..100 {
            class_value.push_str(&format!("irrelevant-{index} "));
        }
        let after = vec![html::internal::unqualified_attribute("class", class_value)];

        reset_dependency_classification_test_counters();
        let mut budget = artifact.classification_budget();
        let failure = artifact
            .classify_attribute_transition(
                html::ElementNamespace::Html,
                &before,
                &after,
                &mut budget,
            )
            .expect_err("candidate work must remain bounded");
        let counters = dependency_classification_test_counters();

        assert!(matches!(
            failure,
            DependencyClassificationFailure::EvaluationLimitExceeded {
                configured: 8,
                observed: 9
            }
        ));
        assert_eq!(counters.class_candidate_lookups, 8);
        assert_eq!(budget.consumed(), 8);
    }

    #[test]
    fn dependency_debug_projection_is_independently_bounded_and_explicitly_truncated() {
        let mut source = String::new();
        for index in 0..=STYLE_DEPENDENCY_DEBUG_MAX_RECORDS {
            source.push_str(&format!(".debug-{index} {{ color: red; }}"));
        }
        let snapshot = artifact(&source, &StyleResolutionLimits::default()).to_debug_snapshot();

        assert!(snapshot.contains("records: total=4097 visible=4096"));
        assert!(snapshot.contains("truncated: true"));
        assert!(snapshot.len() <= STYLE_DEPENDENCY_DEBUG_MAX_SERIALIZED_BYTES);
        assert!(snapshot.contains("record[0]"));
        assert!(snapshot.contains("record[4095]"));
        assert!(!snapshot.contains("record[4096]"));
    }

    #[test]
    fn dependency_debug_projection_truncates_by_serialized_bytes_before_semantic_limits() {
        let long_suffix = "x".repeat(2_048);
        let mut source = String::new();
        for index in 0..300 {
            source.push_str(&format!(".debug-{index}-{long_suffix} {{ color: red; }}"));
        }
        let snapshot = artifact(&source, &StyleResolutionLimits::default()).to_debug_snapshot();

        assert!(snapshot.contains("records: total=300 visible="));
        assert!(snapshot.contains("truncated: true"));
        assert!(snapshot.len() <= STYLE_DEPENDENCY_DEBUG_MAX_SERIALIZED_BYTES);
        assert!(!snapshot.contains("record[299]"));
    }
}
