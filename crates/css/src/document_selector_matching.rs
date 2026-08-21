//! Bounded, CSS-owned document/stylesheet selector matching diagnostics.
//!
//! This integration layer sits above the core selector matcher. It knows
//! stylesheet/rule identity and invokes the authoritative checked matcher, but
//! it does not collect cascade candidates or order cascade winners.
//! It remains selector-conformance-only: selectors may still be evaluated for
//! condition-inactive sheets, but every record carries an explicit CSS
//! condition and cascade-eligibility state so a selector match cannot be read
//! as active cascade participation.

use std::fmt::{self, Write};

use html::{AttributeNamespace, DocumentMode, Node};

use crate::cascade::{
    CascadeOrigin, StylesheetCollectionInput, StylesheetConditionStatus, StylesheetOrder,
    StylesheetSourceId,
};
use crate::model::Rule;
use crate::selectors::{
    BoundedSelectorDomConstructionError, InvalidSelectorReason, SelectorDomBuildError,
    SelectorDomBuildStorage, SelectorListParseResult, SelectorMatchDom, SelectorMatchability,
    SelectorMatchingContext, SelectorMatchingEnvironment, SelectorMatchingLimitError,
    SelectorMatchingLimits, SelectorNamespaceConstraint, UnsupportedSelectorFeature,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentSelectorMatchingDiagnosticLimits {
    pub max_stylesheets: usize,
    pub max_stylesheet_rules: usize,
    pub max_elements: usize,
    pub max_selector_evaluations: usize,
    pub max_report_records: usize,
    pub max_report_storage_bytes: usize,
    pub max_serialized_bytes: usize,
    pub selector_matching: SelectorMatchingLimits,
}

impl Default for DocumentSelectorMatchingDiagnosticLimits {
    fn default() -> Self {
        Self {
            max_stylesheets: 128,
            max_stylesheet_rules: 16_384,
            max_elements: 65_536,
            max_selector_evaluations: 1_000_000,
            max_report_records: 1_000_000,
            max_report_storage_bytes: 64 * 1024 * 1024,
            max_serialized_bytes: 64 * 1024 * 1024,
            selector_matching: SelectorMatchingLimits::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentSelectorMatchingDiagnosticLimit {
    Stylesheets,
    StylesheetRules,
    Elements,
    SelectorEvaluations,
    ReportRecords,
    ReportStorageBytes,
    SerializedBytes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentSelectorMatchingDiagnosticStorage {
    ReportRecords,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentSelectorMatchingDiagnosticFailure {
    LimitExceeded {
        limit: DocumentSelectorMatchingDiagnosticLimit,
        configured: usize,
        observed_at_least: usize,
    },
    SelectorDomBuild(SelectorDomBuildError),
    SelectorMatching {
        element_index: usize,
        stylesheet_source_id: StylesheetSourceId,
        stylesheet_order: StylesheetOrder,
        condition: SelectorDiagnosticCondition,
        rule_index: usize,
        selector_index: usize,
        error: SelectorMatchingLimitError,
    },
    StorageReservationFailed {
        storage: DocumentSelectorMatchingDiagnosticStorage,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentSelectorMatchingDiagnostic {
    Complete(DocumentSelectorMatchingSnapshot),
    Failed(DocumentSelectorMatchingDiagnosticFailure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentSelectorMatchingSnapshot {
    environment: SelectorMatchingEnvironment,
    stylesheet_count: usize,
    stylesheet_rule_count: usize,
    element_count: usize,
    selector_evaluation_count: usize,
    records: Vec<DocumentSelectorMatchingRecord>,
    serialized: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DocumentSelectorMatchingRecord {
    element_id: u32,
    parent_id: Option<u32>,
    previous_sibling_id: Option<u32>,
    namespace: html::ElementNamespace,
    local_name: String,
    id_attribute: Option<String>,
    stylesheet_source_id: StylesheetSourceId,
    stylesheet_order: StylesheetOrder,
    origin: CascadeOrigin,
    namespace_constraint: SelectorNamespaceConstraint,
    condition: SelectorDiagnosticCondition,
    rule_index: usize,
    selector_index: Option<usize>,
    matchability: SelectorMatchability,
    matched: bool,
    specificity: Option<crate::Specificity>,
    outcome_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorDiagnosticCondition {
    Active,
    InactiveDeferredUnsupported,
}

impl DocumentSelectorMatchingDiagnostic {
    #[must_use]
    pub fn to_debug_snapshot(&self) -> String {
        match self {
            Self::Complete(snapshot) => snapshot.serialized.clone(),
            Self::Failed(failure) => serialize_failure(*failure),
        }
    }

    #[must_use]
    pub fn failure(&self) -> Option<DocumentSelectorMatchingDiagnosticFailure> {
        match self {
            Self::Complete(_) => None,
            Self::Failed(failure) => Some(*failure),
        }
    }
}

/// Produces one bounded, deterministic AF4 selector-matching trace.
pub fn document_selector_matching_diagnostic(
    root: &Node,
    environment: SelectorMatchingEnvironment,
    stylesheets: &[StylesheetCollectionInput<'_>],
    limits: DocumentSelectorMatchingDiagnosticLimits,
) -> DocumentSelectorMatchingDiagnostic {
    if stylesheets.len() > limits.max_stylesheets {
        return failed_limit(
            DocumentSelectorMatchingDiagnosticLimit::Stylesheets,
            limits.max_stylesheets,
            stylesheets.len(),
        );
    }

    let stylesheet_rule_count = stylesheets.iter().try_fold(0usize, |count, input| {
        count.checked_add(input.stylesheet().stylesheet.rules.len())
    });
    let Some(stylesheet_rule_count) = stylesheet_rule_count else {
        return failed_limit(
            DocumentSelectorMatchingDiagnosticLimit::StylesheetRules,
            limits.max_stylesheet_rules,
            usize::MAX,
        );
    };
    if stylesheet_rule_count > limits.max_stylesheet_rules {
        return failed_limit(
            DocumentSelectorMatchingDiagnosticLimit::StylesheetRules,
            limits.max_stylesheet_rules,
            stylesheet_rule_count,
        );
    }

    let index = match crate::selectors::SelectorDomIndex::try_from_document_with_element_limit(
        root,
        limits.max_elements,
    ) {
        Ok(index) => index,
        Err(BoundedSelectorDomConstructionError::Build(error)) => {
            return DocumentSelectorMatchingDiagnostic::Failed(
                DocumentSelectorMatchingDiagnosticFailure::SelectorDomBuild(error),
            );
        }
        Err(BoundedSelectorDomConstructionError::ElementLimitExceeded { limit, observed }) => {
            return failed_limit(
                DocumentSelectorMatchingDiagnosticLimit::Elements,
                limit,
                observed,
            );
        }
    };

    let mut records = Vec::new();
    let mut report_storage_bytes = 0usize;
    let mut selector_evaluation_count = 0usize;
    for (element_index, element) in index.elements().enumerate() {
        for input in stylesheets.iter().copied() {
            let condition = match input.condition().classify() {
                StylesheetConditionStatus::Active => SelectorDiagnosticCondition::Active,
                StylesheetConditionStatus::DeferredUnsupported { .. } => {
                    SelectorDiagnosticCondition::InactiveDeferredUnsupported
                }
            };
            let context =
                SelectorMatchingContext::with_limits(&index, environment, limits.selector_matching)
                    .with_namespace_constraint(input.namespace_constraint());
            for (rule_index, rule) in input.stylesheet().stylesheet.rules.iter().enumerate() {
                let Rule::Style(rule) = rule else {
                    continue;
                };
                match &rule.selectors {
                    SelectorListParseResult::Parsed(list) => {
                        for (selector_index, selector) in list.iter().enumerate() {
                            selector_evaluation_count = match selector_evaluation_count
                                .checked_add(1)
                            {
                                Some(count) => count,
                                None => {
                                    return failed_limit(
                                            DocumentSelectorMatchingDiagnosticLimit::SelectorEvaluations,
                                            limits.max_selector_evaluations,
                                            usize::MAX,
                                        );
                                }
                            };
                            if selector_evaluation_count > limits.max_selector_evaluations {
                                return failed_limit(
                                    DocumentSelectorMatchingDiagnosticLimit::SelectorEvaluations,
                                    limits.max_selector_evaluations,
                                    selector_evaluation_count,
                                );
                            }
                            let matched = match context.matches_complex_selector(element, selector)
                            {
                                Ok(matched) => matched,
                                Err(error) => {
                                    return DocumentSelectorMatchingDiagnostic::Failed(
                                        DocumentSelectorMatchingDiagnosticFailure::SelectorMatching {
                                            element_index,
                                            stylesheet_source_id: input.source_id(),
                                            stylesheet_order: input.order(),
                                            condition,
                                            rule_index,
                                            selector_index,
                                            error,
                                        },
                                    );
                                }
                            };
                            let record = match record_context(
                                &index,
                                element,
                                input,
                                rule_index,
                                condition,
                                Some(selector_index),
                                SelectorMatchability::Parsed,
                                matched,
                                Some(selector.specificity()),
                                None,
                                limits.max_report_storage_bytes,
                            ) {
                                Ok(record) => record,
                                Err(failure) => {
                                    return DocumentSelectorMatchingDiagnostic::Failed(failure);
                                }
                            };
                            if let Err(failure) = push_record(
                                &mut records,
                                &mut report_storage_bytes,
                                record,
                                &limits,
                            ) {
                                return DocumentSelectorMatchingDiagnostic::Failed(failure);
                            }
                        }
                    }
                    SelectorListParseResult::Unsupported(unsupported) => {
                        let reason = unsupported
                            .features()
                            .iter()
                            .map(|feature| unsupported_feature_label(*feature))
                            .collect::<Vec<_>>()
                            .join(",");
                        let record = match record_context(
                            &index,
                            element,
                            input,
                            rule_index,
                            condition,
                            None,
                            SelectorMatchability::Unsupported,
                            false,
                            None,
                            Some(reason),
                            limits.max_report_storage_bytes,
                        ) {
                            Ok(record) => record,
                            Err(failure) => {
                                return DocumentSelectorMatchingDiagnostic::Failed(failure);
                            }
                        };
                        if let Err(failure) =
                            push_record(&mut records, &mut report_storage_bytes, record, &limits)
                        {
                            return DocumentSelectorMatchingDiagnostic::Failed(failure);
                        }
                    }
                    SelectorListParseResult::Invalid(invalid) => {
                        let record = match record_context(
                            &index,
                            element,
                            input,
                            rule_index,
                            condition,
                            None,
                            SelectorMatchability::Invalid,
                            false,
                            None,
                            Some(invalid_reason_label(invalid.reason()).into()),
                            limits.max_report_storage_bytes,
                        ) {
                            Ok(record) => record,
                            Err(failure) => {
                                return DocumentSelectorMatchingDiagnostic::Failed(failure);
                            }
                        };
                        if let Err(failure) =
                            push_record(&mut records, &mut report_storage_bytes, record, &limits)
                        {
                            return DocumentSelectorMatchingDiagnostic::Failed(failure);
                        }
                    }
                }
            }
        }
    }

    let mut snapshot = DocumentSelectorMatchingSnapshot {
        environment,
        stylesheet_count: stylesheets.len(),
        stylesheet_rule_count,
        element_count: index.len(),
        selector_evaluation_count,
        records,
        serialized: String::new(),
    };
    snapshot.serialized = match serialize_complete(&snapshot, limits.max_serialized_bytes) {
        Ok(serialized) => serialized,
        Err(observed_at_least) => {
            return failed_limit(
                DocumentSelectorMatchingDiagnosticLimit::SerializedBytes,
                limits.max_serialized_bytes,
                observed_at_least,
            );
        }
    };
    DocumentSelectorMatchingDiagnostic::Complete(snapshot)
}

fn push_record(
    records: &mut Vec<DocumentSelectorMatchingRecord>,
    report_storage_bytes: &mut usize,
    record: DocumentSelectorMatchingRecord,
    limits: &DocumentSelectorMatchingDiagnosticLimits,
) -> Result<(), DocumentSelectorMatchingDiagnosticFailure> {
    let observed = records.len().saturating_add(1);
    if observed > limits.max_report_records {
        return Err(DocumentSelectorMatchingDiagnosticFailure::LimitExceeded {
            limit: DocumentSelectorMatchingDiagnosticLimit::ReportRecords,
            configured: limits.max_report_records,
            observed_at_least: observed,
        });
    }
    let record_storage = record_storage_size(
        record.local_name.len(),
        record.id_attribute.as_ref().map_or(0, String::len),
        record.outcome_reason.as_ref().map_or(0, String::len),
    );
    let observed_storage = report_storage_bytes
        .checked_add(record_storage)
        .unwrap_or(usize::MAX);
    if observed_storage > limits.max_report_storage_bytes {
        return Err(DocumentSelectorMatchingDiagnosticFailure::LimitExceeded {
            limit: DocumentSelectorMatchingDiagnosticLimit::ReportStorageBytes,
            configured: limits.max_report_storage_bytes,
            observed_at_least: observed_storage,
        });
    }
    try_reserve_report_records(records, 1)?;
    records.push(record);
    *report_storage_bytes = observed_storage;
    Ok(())
}

fn try_reserve_report_records(
    records: &mut Vec<DocumentSelectorMatchingRecord>,
    additional: usize,
) -> Result<(), DocumentSelectorMatchingDiagnosticFailure> {
    records.try_reserve(additional).map_err(|_| {
        DocumentSelectorMatchingDiagnosticFailure::StorageReservationFailed {
            storage: DocumentSelectorMatchingDiagnosticStorage::ReportRecords,
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn record_context(
    index: &crate::selectors::SelectorDomIndex<'_>,
    element: crate::selectors::SelectorDomElementId,
    input: StylesheetCollectionInput<'_>,
    rule_index: usize,
    condition: SelectorDiagnosticCondition,
    selector_index: Option<usize>,
    matchability: SelectorMatchability,
    matched: bool,
    specificity: Option<crate::Specificity>,
    outcome_reason: Option<String>,
    max_report_storage_bytes: usize,
) -> Result<DocumentSelectorMatchingRecord, DocumentSelectorMatchingDiagnosticFailure> {
    let local_name = index.element_local_name(element);
    let id_attribute = index
        .attributes(element)
        .find(|attribute| {
            attribute.namespace() == AttributeNamespace::None
                && attribute.local_name().eq_ignore_ascii_case("id")
        })
        .map(|attribute| attribute.value());
    let observed_at_least = record_storage_size(
        local_name.len(),
        id_attribute.map_or(0, str::len),
        outcome_reason.as_ref().map_or(0, String::len),
    );
    if observed_at_least > max_report_storage_bytes {
        return Err(DocumentSelectorMatchingDiagnosticFailure::LimitExceeded {
            limit: DocumentSelectorMatchingDiagnosticLimit::ReportStorageBytes,
            configured: max_report_storage_bytes,
            observed_at_least,
        });
    }
    Ok(DocumentSelectorMatchingRecord {
        element_id: element.get(),
        parent_id: index.parent_element(element).map(|id| id.get()),
        previous_sibling_id: index.previous_sibling_element(element).map(|id| id.get()),
        namespace: index.element_namespace(element),
        local_name: local_name.to_string(),
        id_attribute: id_attribute.map(str::to_string),
        stylesheet_source_id: input.source_id(),
        stylesheet_order: input.order(),
        origin: input.origin(),
        namespace_constraint: input.namespace_constraint(),
        condition,
        rule_index,
        selector_index,
        matchability,
        matched,
        specificity,
        outcome_reason,
    })
}

fn record_storage_size(local_name: usize, id_attribute: usize, outcome_reason: usize) -> usize {
    std::mem::size_of::<DocumentSelectorMatchingRecord>()
        .checked_add(local_name)
        .and_then(|size| size.checked_add(id_attribute))
        .and_then(|size| size.checked_add(outcome_reason))
        .unwrap_or(usize::MAX)
}

fn failed_limit(
    limit: DocumentSelectorMatchingDiagnosticLimit,
    configured: usize,
    observed_at_least: usize,
) -> DocumentSelectorMatchingDiagnostic {
    DocumentSelectorMatchingDiagnostic::Failed(
        DocumentSelectorMatchingDiagnosticFailure::LimitExceeded {
            limit,
            configured,
            observed_at_least,
        },
    )
}

fn serialize_complete(
    snapshot: &DocumentSelectorMatchingSnapshot,
    max_serialized_bytes: usize,
) -> Result<String, usize> {
    let mut out = BoundedReport::new(max_serialized_bytes);
    writeln!(&mut out, "version: 2").map_err(|_| out.observed_at_least())?;
    writeln!(&mut out, "document-selector-matching").map_err(|_| out.observed_at_least())?;
    writeln!(&mut out, "status: complete").map_err(|_| out.observed_at_least())?;
    writeln!(
        &mut out,
        "environment: document-mode={}",
        document_mode_label(snapshot.environment.document_mode())
    )
    .map_err(|_| out.observed_at_least())?;
    writeln!(&mut out, "stylesheets: {}", snapshot.stylesheet_count)
        .map_err(|_| out.observed_at_least())?;
    writeln!(
        &mut out,
        "stylesheet-rules: {}",
        snapshot.stylesheet_rule_count
    )
    .map_err(|_| out.observed_at_least())?;
    writeln!(&mut out, "elements: {}", snapshot.element_count)
        .map_err(|_| out.observed_at_least())?;
    writeln!(
        &mut out,
        "selector-evaluations: {}",
        snapshot.selector_evaluation_count
    )
    .map_err(|_| out.observed_at_least())?;
    writeln!(&mut out, "records: {}", snapshot.records.len())
        .map_err(|_| out.observed_at_least())?;
    for (index, record) in snapshot.records.iter().enumerate() {
        write!(
            &mut out,
            "  record[{index}]: element={} parent={} previous-sibling={} namespace={} local=",
            record.element_id,
            optional_u32(record.parent_id),
            optional_u32(record.previous_sibling_id),
            record.namespace.snapshot_name(),
        )
        .map_err(|_| out.observed_at_least())?;
        write_quoted(&mut out, &record.local_name).map_err(|_| out.observed_at_least())?;
        write!(&mut out, " id-attribute=").map_err(|_| out.observed_at_least())?;
        if let Some(id) = &record.id_attribute {
            write_quoted(&mut out, id).map_err(|_| out.observed_at_least())?;
        } else {
            out.write_str("none").map_err(|_| out.observed_at_least())?;
        }
        write!(
            &mut out,
            " stylesheet-source={} stylesheet-order={} origin={} namespace-constraint={} condition={} rule={} selector={} matchability={} selector-state={} cascade-state={} specificity={} reason=",
            record.stylesheet_source_id.get(),
            record.stylesheet_order.get(),
            origin_label(record.origin),
            namespace_constraint_label(record.namespace_constraint),
            selector_condition_label(record.condition),
            record.rule_index,
            optional_usize(record.selector_index),
            matchability_label(record.matchability),
            if record.matched { "matched" } else { "not-matched" },
            selector_cascade_state_label(record.condition),
            specificity_label(record.specificity),
        )
        .map_err(|_| out.observed_at_least())?;
        if let Some(reason) = &record.outcome_reason {
            write_quoted(&mut out, reason).map_err(|_| out.observed_at_least())?;
        } else {
            out.write_str("none").map_err(|_| out.observed_at_least())?;
        }
        out.write_char('\n').map_err(|_| out.observed_at_least())?;
    }
    Ok(out.finish())
}

fn selector_condition_label(condition: SelectorDiagnosticCondition) -> &'static str {
    match condition {
        SelectorDiagnosticCondition::Active => "active",
        SelectorDiagnosticCondition::InactiveDeferredUnsupported => "deferred-unsupported",
    }
}

fn selector_cascade_state_label(condition: SelectorDiagnosticCondition) -> &'static str {
    match condition {
        SelectorDiagnosticCondition::Active => "eligible",
        SelectorDiagnosticCondition::InactiveDeferredUnsupported => "inactive-condition",
    }
}

struct BoundedReport {
    output: String,
    limit: usize,
    observed_at_least: usize,
}

impl BoundedReport {
    fn new(limit: usize) -> Self {
        Self {
            output: String::with_capacity(limit.min(4096)),
            limit,
            observed_at_least: limit.saturating_add(1),
        }
    }

    fn observed_at_least(&self) -> usize {
        self.observed_at_least
    }

    fn finish(self) -> String {
        self.output
    }
}

impl Write for BoundedReport {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(observed) = self.output.len().checked_add(value.len()) else {
            self.observed_at_least = usize::MAX;
            return Err(fmt::Error);
        };
        if observed > self.limit {
            self.observed_at_least = observed;
            return Err(fmt::Error);
        }
        self.output.push_str(value);
        Ok(())
    }
}

fn serialize_failure(failure: DocumentSelectorMatchingDiagnosticFailure) -> String {
    let mut out = String::new();
    writeln!(&mut out, "version: 2").expect("write diagnostic");
    writeln!(&mut out, "document-selector-matching").expect("write diagnostic");
    writeln!(&mut out, "status: failed").expect("write diagnostic");
    match failure {
        DocumentSelectorMatchingDiagnosticFailure::LimitExceeded {
            limit,
            configured,
            observed_at_least,
        } => writeln!(
            &mut out,
            "failure: kind=limit-exceeded limit={} configured={} observed-at-least={}",
            diagnostic_limit_label(limit),
            configured,
            observed_at_least
        )
        .expect("write diagnostic"),
        DocumentSelectorMatchingDiagnosticFailure::SelectorDomBuild(error) => writeln!(
            &mut out,
            "failure: kind=selector-dom-build reason={}",
            selector_dom_build_error_label(error)
        )
        .expect("write diagnostic"),
        DocumentSelectorMatchingDiagnosticFailure::SelectorMatching {
            element_index,
            stylesheet_source_id,
            stylesheet_order,
            condition,
            rule_index,
            selector_index,
            error,
        } => writeln!(
            &mut out,
            "failure: kind=selector-matching element-index={} stylesheet-source={} stylesheet-order={} condition={} cascade-state={} rule={} selector={} reason={}",
            element_index,
            stylesheet_source_id.get(),
            stylesheet_order.get(),
            selector_condition_label(condition),
            selector_cascade_state_label(condition),
            rule_index,
            selector_index,
            selector_matching_error_label(error)
        )
        .expect("write diagnostic"),
        DocumentSelectorMatchingDiagnosticFailure::StorageReservationFailed { storage } => {
            writeln!(
                &mut out,
                "failure: kind=storage-reservation-failed storage={}",
                diagnostic_storage_label(storage)
            )
            .expect("write diagnostic");
        }
    }
    out
}

fn write_quoted(out: &mut impl Write, value: &str) -> fmt::Result {
    out.write_char('"')?;
    for character in value.chars() {
        match character {
            '"' => out.write_str("\\\"")?,
            '\\' => out.write_str("\\\\")?,
            '\n' => out.write_str("\\n")?,
            '\r' => out.write_str("\\r")?,
            '\t' => out.write_str("\\t")?,
            character if character.is_control() => {
                write!(out, "\\u{{{:X}}}", character as u32)?;
            }
            character => out.write_char(character)?,
        }
    }
    out.write_char('"')?;
    Ok(())
}

fn optional_u32(value: Option<u32>) -> String {
    value.map_or_else(|| "none".into(), |value| value.to_string())
}

fn optional_usize(value: Option<usize>) -> String {
    value.map_or_else(|| "none".into(), |value| value.to_string())
}

fn specificity_label(value: Option<crate::Specificity>) -> String {
    value.map_or_else(
        || "none".into(),
        |value| format!("{},{},{}", value.a(), value.b(), value.c()),
    )
}

const fn document_mode_label(mode: DocumentMode) -> &'static str {
    match mode {
        DocumentMode::NoQuirks => "no-quirks",
        DocumentMode::LimitedQuirks => "limited-quirks",
        DocumentMode::Quirks => "quirks",
    }
}

const fn origin_label(origin: CascadeOrigin) -> &'static str {
    match origin {
        CascadeOrigin::UserAgent => "user-agent",
        CascadeOrigin::User => "user",
        CascadeOrigin::Author => "author",
    }
}

const fn namespace_constraint_label(constraint: SelectorNamespaceConstraint) -> &'static str {
    match constraint {
        SelectorNamespaceConstraint::Unconstrained => "unconstrained",
        SelectorNamespaceConstraint::Exact(html::ElementNamespace::Html) => "html",
        SelectorNamespaceConstraint::Exact(html::ElementNamespace::Svg) => "svg",
        SelectorNamespaceConstraint::Exact(html::ElementNamespace::MathMl) => "mathml",
    }
}

const fn matchability_label(value: SelectorMatchability) -> &'static str {
    match value {
        SelectorMatchability::Parsed => "parsed",
        SelectorMatchability::Unsupported => "unsupported",
        SelectorMatchability::Invalid => "invalid",
    }
}

const fn diagnostic_limit_label(limit: DocumentSelectorMatchingDiagnosticLimit) -> &'static str {
    match limit {
        DocumentSelectorMatchingDiagnosticLimit::Stylesheets => "stylesheets",
        DocumentSelectorMatchingDiagnosticLimit::StylesheetRules => "stylesheet-rules",
        DocumentSelectorMatchingDiagnosticLimit::Elements => "elements",
        DocumentSelectorMatchingDiagnosticLimit::SelectorEvaluations => "selector-evaluations",
        DocumentSelectorMatchingDiagnosticLimit::ReportRecords => "report-records",
        DocumentSelectorMatchingDiagnosticLimit::ReportStorageBytes => "report-storage-bytes",
        DocumentSelectorMatchingDiagnosticLimit::SerializedBytes => "serialized-bytes",
    }
}

const fn diagnostic_storage_label(
    storage: DocumentSelectorMatchingDiagnosticStorage,
) -> &'static str {
    match storage {
        DocumentSelectorMatchingDiagnosticStorage::ReportRecords => "report-records",
    }
}

const fn invalid_reason_label(reason: InvalidSelectorReason) -> &'static str {
    match reason {
        InvalidSelectorReason::EmptySelectorList => "empty-selector-list",
        InvalidSelectorReason::EmptyCompoundSelector => "empty-compound-selector",
        InvalidSelectorReason::LeadingCombinator => "leading-combinator",
        InvalidSelectorReason::TrailingCombinator => "trailing-combinator",
        InvalidSelectorReason::RepeatedCombinator => "repeated-combinator",
        InvalidSelectorReason::MultipleTypeSelectors => "multiple-type-selectors",
        InvalidSelectorReason::MissingAttributeName => "missing-attribute-name",
        InvalidSelectorReason::MissingAttributeValue => "missing-attribute-value",
        InvalidSelectorReason::UnexpectedComponentValue => "unexpected-component-value",
        InvalidSelectorReason::InvariantViolation => "invariant-violation",
        InvalidSelectorReason::ResourceLimitExceeded => "resource-limit-exceeded",
    }
}

const fn unsupported_feature_label(feature: UnsupportedSelectorFeature) -> &'static str {
    match feature {
        UnsupportedSelectorFeature::Namespace => "namespace",
        UnsupportedSelectorFeature::AttributeCaseModifier => "attribute-case-modifier",
        UnsupportedSelectorFeature::PseudoClass => "pseudo-class",
        UnsupportedSelectorFeature::FunctionalPseudoClass => "functional-pseudo-class",
        UnsupportedSelectorFeature::PseudoElement => "pseudo-element",
        UnsupportedSelectorFeature::RelativeSelector => "relative-selector",
        UnsupportedSelectorFeature::NestingSelector => "nesting-selector",
        UnsupportedSelectorFeature::ColumnCombinator => "column-combinator",
        UnsupportedSelectorFeature::ForgivingSelectorList => "forgiving-selector-list",
    }
}

fn selector_matching_error_label(error: SelectorMatchingLimitError) -> String {
    match error {
        SelectorMatchingLimitError::AxisStepLimitExceeded { limit } => {
            format!("axis-step-limit-exceeded:{limit}")
        }
    }
}

fn selector_dom_build_error_label(error: SelectorDomBuildError) -> String {
    match error {
        SelectorDomBuildError::InvalidDocumentRoot { actual } => {
            format!("invalid-document-root:{actual}")
        }
        SelectorDomBuildError::NestedDocument { depth } => format!("nested-document:{depth}"),
        SelectorDomBuildError::MultipleDocumentElements {
            first_child_index,
            second_child_index,
        } => format!("multiple-document-elements:{first_child_index}:{second_child_index}"),
        SelectorDomBuildError::NonCanonicalHtmlElementLocalName { element_index } => {
            format!("noncanonical-html-local-name:{element_index}")
        }
        SelectorDomBuildError::ElementIdRepresentationExhausted { maximum } => {
            format!("element-id-representation-exhausted:{maximum}")
        }
        SelectorDomBuildError::ProjectionCapacityExceeded { storage } => {
            format!("projection-capacity-exceeded:{}", storage_label(storage))
        }
        SelectorDomBuildError::StorageReservationFailed { storage } => {
            format!("storage-reservation-failed:{}", storage_label(storage))
        }
    }
}

const fn storage_label(storage: SelectorDomBuildStorage) -> &'static str {
    match storage {
        SelectorDomBuildStorage::PreflightTraversalStack => "preflight-traversal-stack",
        SelectorDomBuildStorage::MaterializationTraversalStack => "materialization-traversal-stack",
        SelectorDomBuildStorage::ElementRecords => "element-records",
        SelectorDomBuildStorage::DirectTextChildren => "direct-text-children",
    }
}

#[cfg(test)]
mod tests;
