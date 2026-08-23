use std::fmt::{self, Write};

use super::collection::{
    CollectedRule, InactiveStyleRuleReason, RuleCollection, RuleCollectionBuildError,
};
use super::limits::{StyleResolutionError, StyleResolutionLimits};
use super::rule_inputs::rule_inputs_for_element_with_observer;
use super::selector_dom::build_document_selector_dom_with_element_limit;
use super::source::{StylesheetCollectionInput, StylesheetConditionStatus};
use crate::cascade::contract::{
    CascadeDeclarationApplicability, CascadeDeclarationInput, CascadeDeclarationProperty,
    CascadeDeclarationSource, CascadeImportance, CascadeOrigin, CascadeResolutionBudget,
    CascadeRuleInput, DeclarationOrder, DeclarationSourceIndex, RawRuleIndex, StyleRulePosition,
    StylesheetOrder, StylesheetRuleOrder, StylesheetSourceId,
};
use crate::selectors::{
    InvalidSelectorReason, SelectorListMatchOutcome, SelectorMatchingContext,
    SelectorMatchingEnvironment, SelectorNamespaceConstraint, Specificity,
    UnsupportedSelectorFeature,
};
use html::Node;

pub const RULE_COLLECTION_DIAGNOSTIC_VERSION: u16 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleCollectionDiagnosticLimits {
    pub max_records: usize,
    pub max_storage_bytes: usize,
    pub max_serialized_bytes: usize,
    pub max_condition_text_bytes: usize,
    pub max_at_rule_name_text_bytes: usize,
    pub max_declaration_property_text_bytes: usize,
    pub max_declaration_value_text_bytes: usize,
}

impl Default for RuleCollectionDiagnosticLimits {
    fn default() -> Self {
        Self {
            max_records: 1_000_000,
            max_storage_bytes: 64 * 1024 * 1024,
            max_serialized_bytes: 64 * 1024 * 1024,
            max_condition_text_bytes: 4 * 1024,
            max_at_rule_name_text_bytes: 256,
            max_declaration_property_text_bytes: 1_024,
            max_declaration_value_text_bytes: 4 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleCollectionDiagnosticLimit {
    Records,
    StorageBytes,
    SerializedBytes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleCollectionDiagnosticStorage {
    Records,
    SerializedOutput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleCollectionDiagnosticFailure {
    Collection(RuleCollectionBuildError),
    Resolution(StyleResolutionError),
    LimitExceeded {
        limit: RuleCollectionDiagnosticLimit,
        configured: usize,
        observed_at_least: usize,
    },
    Reservation {
        storage: RuleCollectionDiagnosticStorage,
    },
}

impl RuleCollectionDiagnosticFailure {
    pub const fn stable_label(&self) -> &'static str {
        match self {
            Self::Collection(_) => "collection",
            Self::Resolution(_) => "resolution",
            Self::LimitExceeded { .. } => "limit-exceeded",
            Self::Reservation { .. } => "reservation",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleCollectionDiagnostic {
    Complete(RuleCollectionDiagnosticSnapshot),
    Failed(RuleCollectionDiagnosticFailure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleCollectionDiagnosticSnapshot {
    records: Vec<RuleCollectionDiagnosticRecord>,
    serialized: String,
}

impl RuleCollectionDiagnosticSnapshot {
    pub fn records(&self) -> &[RuleCollectionDiagnosticRecord] {
        &self.records
    }
}

impl RuleCollectionDiagnostic {
    pub fn failure(&self) -> Option<&RuleCollectionDiagnosticFailure> {
        match self {
            Self::Complete(_) => None,
            Self::Failed(failure) => Some(failure),
        }
    }

    pub fn records(&self) -> &[RuleCollectionDiagnosticRecord] {
        match self {
            Self::Complete(snapshot) => snapshot.records(),
            Self::Failed(_) => &[],
        }
    }

    pub fn to_debug_snapshot(&self) -> String {
        match self {
            Self::Complete(snapshot) => snapshot.serialized.clone(),
            Self::Failed(failure) => format!(
                "version: {RULE_COLLECTION_DIAGNOSTIC_VERSION}\naf5-rule-collection\nstatus: failed\nfailure: {}\n",
                failure_label(failure)
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleCollectionDiagnosticRecord {
    Stylesheet {
        source_id: StylesheetSourceId,
        order: StylesheetOrder,
        origin: CascadeOrigin,
        namespace_constraint: SelectorNamespaceConstraint,
        condition: DiagnosticCondition,
    },
    Rule {
        source_id: StylesheetSourceId,
        raw_rule_index: RawRuleIndex,
        style_position: Option<StyleRulePosition>,
        source_order: Option<StylesheetRuleOrder>,
        state: DiagnosticRuleState,
    },
    Declaration {
        source_id: StylesheetSourceId,
        raw_rule_index: RawRuleIndex,
        declaration_source_index: DeclarationSourceIndex,
        declaration_order: DeclarationOrder,
        expansion_order: u16,
        importance: CascadeImportance,
        property: DiagnosticDeclarationProperty,
        value: Option<BoundedDiagnosticText>,
        applicability: CascadeDeclarationApplicability,
        invalid_reason: Option<&'static str>,
    },
    InlineDeclaration {
        element_id: u32,
        declaration_source_index: DeclarationSourceIndex,
        declaration_order: DeclarationOrder,
        expansion_order: u16,
        importance: CascadeImportance,
        property: DiagnosticDeclarationProperty,
        value: Option<BoundedDiagnosticText>,
        applicability: CascadeDeclarationApplicability,
        invalid_reason: Option<&'static str>,
    },
    Match {
        element_id: u32,
        source_id: StylesheetSourceId,
        raw_rule_index: RawRuleIndex,
        outcome: SelectorListMatchOutcome,
        effective_specificity: Option<Specificity>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiagnosticCondition {
    Active,
    DeferredUnsupported(BoundedDiagnosticText),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundedDiagnosticText {
    pub text: String,
    pub original_bytes: usize,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiagnosticDeclarationProperty {
    Supported { name: &'static str },
    InvalidValue { name: &'static str },
    InvalidShorthand { name: &'static str },
    Unsupported { name: BoundedDiagnosticText },
    Custom { name: BoundedDiagnosticText },
    InvalidName,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiagnosticRuleState {
    Active,
    InactiveCondition,
    InvalidSelector {
        reason: InvalidSelectorReason,
    },
    UnsupportedSelector {
        features: Vec<UnsupportedSelectorFeature>,
    },
    SkippedAtRule {
        reason: super::collection::AtRuleSkipReason,
        name: Option<BoundedDiagnosticText>,
    },
}

pub fn rule_collection_diagnostic(
    root: &Node,
    environment: SelectorMatchingEnvironment,
    inputs: &[StylesheetCollectionInput<'_>],
    style_limits: &StyleResolutionLimits,
    diagnostic_limits: RuleCollectionDiagnosticLimits,
) -> RuleCollectionDiagnostic {
    let collection = match RuleCollection::try_new(inputs, style_limits) {
        Ok(collection) => collection,
        Err(error) => {
            return RuleCollectionDiagnostic::Failed(RuleCollectionDiagnosticFailure::Collection(
                error,
            ));
        }
    };
    let index = match build_document_selector_dom_with_element_limit(
        root,
        style_limits.max_styled_elements_per_document,
    ) {
        Ok(index) => index,
        Err(error) => {
            return RuleCollectionDiagnostic::Failed(RuleCollectionDiagnosticFailure::Resolution(
                error,
            ));
        }
    };

    let mut records = DiagnosticRecordBuilder::new(diagnostic_limits);
    for stylesheet in collection.stylesheets() {
        let condition = match stylesheet.condition() {
            StylesheetConditionStatus::Active => DiagnosticCondition::Active,
            StylesheetConditionStatus::DeferredUnsupported { raw } => {
                match bounded_text(raw, diagnostic_limits.max_condition_text_bytes) {
                    Ok(text) => DiagnosticCondition::DeferredUnsupported(text),
                    Err(failure) => return RuleCollectionDiagnostic::Failed(failure),
                }
            }
        };
        if let Err(failure) = records.push(RuleCollectionDiagnosticRecord::Stylesheet {
            source_id: stylesheet.source_id(),
            order: stylesheet.order(),
            origin: stylesheet.origin(),
            namespace_constraint: stylesheet.namespace_constraint(),
            condition,
        }) {
            return RuleCollectionDiagnostic::Failed(failure);
        }
    }

    for rule in collection.rules() {
        let record = match rule {
            CollectedRule::ActiveStyle(rule) => RuleCollectionDiagnosticRecord::Rule {
                source_id: rule.rule_ref().source_id(),
                raw_rule_index: rule.rule_ref().raw_rule_index(),
                style_position: Some(rule.style_position()),
                source_order: Some(rule.source_order()),
                state: DiagnosticRuleState::Active,
            },
            CollectedRule::InactiveStyle(rule) => RuleCollectionDiagnosticRecord::Rule {
                source_id: rule.rule_ref().source_id(),
                raw_rule_index: rule.rule_ref().raw_rule_index(),
                style_position: Some(rule.style_position()),
                source_order: None,
                state: match rule.reason() {
                    InactiveStyleRuleReason::StylesheetConditionDeferred { .. } => {
                        DiagnosticRuleState::InactiveCondition
                    }
                    InactiveStyleRuleReason::InvalidSelector { reason } => {
                        DiagnosticRuleState::InvalidSelector { reason: *reason }
                    }
                    InactiveStyleRuleReason::UnsupportedSelector { features } => {
                        DiagnosticRuleState::UnsupportedSelector {
                            features: features.to_vec(),
                        }
                    }
                },
            },
            CollectedRule::SkippedAtRule(rule) => RuleCollectionDiagnosticRecord::Rule {
                source_id: rule.rule_ref().source_id(),
                raw_rule_index: rule.rule_ref().raw_rule_index(),
                style_position: None,
                source_order: None,
                state: DiagnosticRuleState::SkippedAtRule {
                    reason: rule.reason(),
                    name: match rule.name() {
                        Some(name) => {
                            match bounded_text(name, diagnostic_limits.max_at_rule_name_text_bytes)
                            {
                                Ok(name) => Some(name),
                                Err(failure) => return RuleCollectionDiagnostic::Failed(failure),
                            }
                        }
                        None => None,
                    },
                },
            },
        };
        if let Err(failure) = records.push(record) {
            return RuleCollectionDiagnostic::Failed(failure);
        }
    }

    for declaration in collection.declarations() {
        let crate::cascade::contract::CascadeDeclarationSource::Stylesheet(source) =
            declaration.source()
        else {
            continue;
        };
        let (property, value, invalid_reason) =
            match diagnostic_declaration_projection(declaration, diagnostic_limits) {
                Ok(projection) => projection,
                Err(failure) => return RuleCollectionDiagnostic::Failed(failure),
            };
        if let Err(failure) = records.push(RuleCollectionDiagnosticRecord::Declaration {
            source_id: source.source_id(),
            raw_rule_index: source.raw_rule_index(),
            declaration_source_index: source.declaration_index(),
            declaration_order: declaration.declaration_order(),
            expansion_order: declaration.expansion_order(),
            importance: declaration.importance(),
            property,
            value,
            applicability: declaration.applicability(),
            invalid_reason,
        }) {
            return RuleCollectionDiagnostic::Failed(failure);
        }
    }

    let context =
        SelectorMatchingContext::with_limits(&index, environment, style_limits.selector_matching);
    let cascade_budget = match CascadeResolutionBudget::try_new(
        style_limits.max_declaration_inputs_per_element,
        style_limits.max_inline_declarations_per_element,
        style_limits.max_matched_rules_per_element,
    ) {
        Ok(budget) => budget,
        Err(error) => {
            return RuleCollectionDiagnostic::Failed(RuleCollectionDiagnosticFailure::Resolution(
                StyleResolutionError::CascadeResolution(error),
            ));
        }
    };
    for element in index.elements() {
        let mut observer_failure = None;
        let result = rule_inputs_for_element_with_observer(
            &index,
            &context,
            element,
            &collection,
            style_limits,
            cascade_budget,
            |rule, outcome| {
                if observer_failure.is_none() {
                    observer_failure = records
                        .push(RuleCollectionDiagnosticRecord::Match {
                            element_id: element.get(),
                            source_id: rule.rule_ref().source_id(),
                            raw_rule_index: rule.rule_ref().raw_rule_index(),
                            outcome: outcome.clone(),
                            effective_specificity: outcome.highest_specificity(),
                        })
                        .err();
                }
            },
        );
        let rule_inputs = match result {
            Ok(rule_inputs) => rule_inputs,
            Err(error) => {
                return RuleCollectionDiagnostic::Failed(
                    RuleCollectionDiagnosticFailure::Resolution(error),
                );
            }
        };
        if let Some(failure) = observer_failure {
            return RuleCollectionDiagnostic::Failed(failure);
        }
        for input in rule_inputs.inputs() {
            let CascadeRuleInput::Inline(_) = input else {
                continue;
            };
            for declaration in input.declarations() {
                let CascadeDeclarationSource::InlineStyle(source) = declaration.source() else {
                    continue;
                };
                let (property, value, invalid_reason) =
                    match diagnostic_declaration_projection(declaration, diagnostic_limits) {
                        Ok(projection) => projection,
                        Err(failure) => return RuleCollectionDiagnostic::Failed(failure),
                    };
                if let Err(failure) =
                    records.push(RuleCollectionDiagnosticRecord::InlineDeclaration {
                        element_id: element.get(),
                        declaration_source_index: source.declaration_index(),
                        declaration_order: declaration.declaration_order(),
                        expansion_order: declaration.expansion_order(),
                        importance: declaration.importance(),
                        property,
                        value,
                        applicability: declaration.applicability(),
                        invalid_reason,
                    })
                {
                    return RuleCollectionDiagnostic::Failed(failure);
                }
            }
        }
    }

    let records = records.records;
    let serialized = match serialize_records(&records, diagnostic_limits.max_serialized_bytes) {
        Ok(serialized) => serialized,
        Err(failure) => return RuleCollectionDiagnostic::Failed(failure),
    };
    RuleCollectionDiagnostic::Complete(RuleCollectionDiagnosticSnapshot {
        records,
        serialized,
    })
}

struct DiagnosticRecordBuilder {
    records: Vec<RuleCollectionDiagnosticRecord>,
    limits: RuleCollectionDiagnosticLimits,
    heap_storage_bytes: usize,
}

impl DiagnosticRecordBuilder {
    fn new(limits: RuleCollectionDiagnosticLimits) -> Self {
        Self {
            records: Vec::new(),
            limits,
            heap_storage_bytes: 0,
        }
    }

    fn push(
        &mut self,
        record: RuleCollectionDiagnosticRecord,
    ) -> Result<(), RuleCollectionDiagnosticFailure> {
        let Some(observed) = self.records.len().checked_add(1) else {
            return Err(limit_failure(
                RuleCollectionDiagnosticLimit::Records,
                self.limits.max_records,
                self.limits.max_records,
            ));
        };
        if observed > self.limits.max_records {
            return Err(limit_failure(
                RuleCollectionDiagnosticLimit::Records,
                self.limits.max_records,
                observed,
            ));
        }
        let heap_bytes = record_heap_bytes(&record).ok_or_else(|| {
            limit_failure(
                RuleCollectionDiagnosticLimit::StorageBytes,
                self.limits.max_storage_bytes,
                self.limits.max_storage_bytes,
            )
        })?;
        let Some(heap_storage_bytes) = self.heap_storage_bytes.checked_add(heap_bytes) else {
            return Err(limit_failure(
                RuleCollectionDiagnosticLimit::StorageBytes,
                self.limits.max_storage_bytes,
                self.limits.max_storage_bytes,
            ));
        };
        let minimum_record_bytes = observed
            .checked_mul(std::mem::size_of::<RuleCollectionDiagnosticRecord>())
            .and_then(|bytes| bytes.checked_add(heap_storage_bytes))
            .ok_or_else(|| {
                limit_failure(
                    RuleCollectionDiagnosticLimit::StorageBytes,
                    self.limits.max_storage_bytes,
                    self.limits.max_storage_bytes,
                )
            })?;
        if minimum_record_bytes > self.limits.max_storage_bytes {
            return Err(limit_failure(
                RuleCollectionDiagnosticLimit::StorageBytes,
                self.limits.max_storage_bytes,
                minimum_record_bytes,
            ));
        }
        self.records
            .try_reserve(1)
            .map_err(|_| RuleCollectionDiagnosticFailure::Reservation {
                storage: RuleCollectionDiagnosticStorage::Records,
            })?;
        let retained_storage_bytes = self
            .records
            .capacity()
            .checked_mul(std::mem::size_of::<RuleCollectionDiagnosticRecord>())
            .and_then(|bytes| bytes.checked_add(heap_storage_bytes))
            .ok_or_else(|| {
                limit_failure(
                    RuleCollectionDiagnosticLimit::StorageBytes,
                    self.limits.max_storage_bytes,
                    self.limits.max_storage_bytes,
                )
            })?;
        if retained_storage_bytes > self.limits.max_storage_bytes {
            return Err(limit_failure(
                RuleCollectionDiagnosticLimit::StorageBytes,
                self.limits.max_storage_bytes,
                retained_storage_bytes,
            ));
        }
        self.records.push(record);
        self.heap_storage_bytes = heap_storage_bytes;
        Ok(())
    }
}

fn record_heap_bytes(record: &RuleCollectionDiagnosticRecord) -> Option<usize> {
    match record {
        RuleCollectionDiagnosticRecord::Stylesheet {
            condition: DiagnosticCondition::DeferredUnsupported(text),
            ..
        } => Some(text.text.capacity()),
        RuleCollectionDiagnosticRecord::Rule {
            state: DiagnosticRuleState::UnsupportedSelector { features },
            ..
        } => features
            .capacity()
            .checked_mul(std::mem::size_of::<UnsupportedSelectorFeature>()),
        RuleCollectionDiagnosticRecord::Rule {
            state:
                DiagnosticRuleState::SkippedAtRule {
                    name: Some(name), ..
                },
            ..
        } => Some(name.text.capacity()),
        RuleCollectionDiagnosticRecord::Declaration {
            property, value, ..
        }
        | RuleCollectionDiagnosticRecord::InlineDeclaration {
            property, value, ..
        } => diagnostic_property_heap_bytes(property)?
            .checked_add(value.as_ref().map_or(0, |value| value.text.capacity())),
        RuleCollectionDiagnosticRecord::Match { outcome, .. } => {
            outcome.retained_match_storage_bytes()
        }
        _ => Some(0),
    }
}

fn diagnostic_property_heap_bytes(property: &DiagnosticDeclarationProperty) -> Option<usize> {
    Some(match property {
        DiagnosticDeclarationProperty::Unsupported { name }
        | DiagnosticDeclarationProperty::Custom { name } => name.text.capacity(),
        DiagnosticDeclarationProperty::Supported { .. }
        | DiagnosticDeclarationProperty::InvalidValue { .. }
        | DiagnosticDeclarationProperty::InvalidShorthand { .. }
        | DiagnosticDeclarationProperty::InvalidName => 0,
    })
}

fn diagnostic_declaration_projection(
    declaration: &CascadeDeclarationInput,
    limits: RuleCollectionDiagnosticLimits,
) -> Result<
    (
        DiagnosticDeclarationProperty,
        Option<BoundedDiagnosticText>,
        Option<&'static str>,
    ),
    RuleCollectionDiagnosticFailure,
> {
    let property = match declaration.property() {
        CascadeDeclarationProperty::Supported(property) => {
            DiagnosticDeclarationProperty::Supported {
                name: property.name(),
            }
        }
        CascadeDeclarationProperty::InvalidValue(property) => {
            DiagnosticDeclarationProperty::InvalidValue {
                name: property.name(),
            }
        }
        CascadeDeclarationProperty::InvalidShorthandValue(shorthand) => {
            DiagnosticDeclarationProperty::InvalidShorthand {
                name: shorthand.name(),
            }
        }
        CascadeDeclarationProperty::Unsupported(name) => {
            DiagnosticDeclarationProperty::Unsupported {
                name: bounded_text(name, limits.max_declaration_property_text_bytes)?,
            }
        }
        CascadeDeclarationProperty::Custom(name) => DiagnosticDeclarationProperty::Custom {
            name: bounded_text(name, limits.max_declaration_property_text_bytes)?,
        },
        CascadeDeclarationProperty::Invalid => DiagnosticDeclarationProperty::InvalidName,
    };
    let mut writer = BoundedTextWriter::new(limits.max_declaration_value_text_bytes);
    let has_value = declaration
        .value()
        .write_css_text(&mut writer)
        .map_err(|_| RuleCollectionDiagnosticFailure::Reservation {
            storage: RuleCollectionDiagnosticStorage::Records,
        })?;
    let value = has_value.then(|| writer.finish());
    let invalid_reason = declaration
        .invalid_value_error()
        .map(|error| error.kind().as_debug_label())
        .or_else(|| {
            declaration
                .invalid_shorthand_error()
                .map(|error| error.kind().as_debug_label())
        });
    Ok((property, value, invalid_reason))
}

fn bounded_text(
    text: &str,
    maximum: usize,
) -> Result<BoundedDiagnosticText, RuleCollectionDiagnosticFailure> {
    let mut writer = BoundedTextWriter::new(maximum);
    writer
        .write_str(text)
        .map_err(|_| RuleCollectionDiagnosticFailure::Reservation {
            storage: RuleCollectionDiagnosticStorage::Records,
        })?;
    Ok(writer.finish())
}

struct BoundedTextWriter {
    text: String,
    maximum: usize,
    original_bytes: usize,
}

impl BoundedTextWriter {
    fn new(maximum: usize) -> Self {
        Self {
            text: String::new(),
            maximum,
            original_bytes: 0,
        }
    }

    fn finish(self) -> BoundedDiagnosticText {
        BoundedDiagnosticText {
            truncated: self.text.len() < self.original_bytes,
            text: self.text,
            original_bytes: self.original_bytes,
        }
    }
}

impl Write for BoundedTextWriter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.original_bytes = self
            .original_bytes
            .checked_add(text.len())
            .ok_or(fmt::Error)?;
        let remaining = self.maximum.saturating_sub(self.text.len());
        if remaining == 0 {
            return Ok(());
        }
        let end = if text.len() <= remaining {
            text.len()
        } else {
            text.char_indices()
                .map(|(index, _)| index)
                .take_while(|index| *index <= remaining)
                .last()
                .unwrap_or(0)
        };
        self.text.try_reserve(end).map_err(|_| fmt::Error)?;
        self.text.push_str(&text[..end]);
        Ok(())
    }
}

fn serialize_records(
    records: &[RuleCollectionDiagnosticRecord],
    maximum: usize,
) -> Result<String, RuleCollectionDiagnosticFailure> {
    let mut counter = CountingWriter { bytes: 0, maximum };
    write_snapshot(&mut counter, records).map_err(|_| {
        limit_failure(
            RuleCollectionDiagnosticLimit::SerializedBytes,
            maximum,
            counter.bytes,
        )
    })?;

    let mut output = String::new();
    output.try_reserve_exact(counter.bytes).map_err(|_| {
        RuleCollectionDiagnosticFailure::Reservation {
            storage: RuleCollectionDiagnosticStorage::SerializedOutput,
        }
    })?;
    let mut writer = BoundedWriter {
        output,
        maximum: counter.bytes,
    };
    write_snapshot(&mut writer, records).map_err(|_| serialized_limit(maximum))?;
    Ok(writer.output)
}

fn write_snapshot(
    writer: &mut impl Write,
    records: &[RuleCollectionDiagnosticRecord],
) -> fmt::Result {
    writeln!(writer, "version: {RULE_COLLECTION_DIAGNOSTIC_VERSION}")?;
    writeln!(writer, "af5-rule-collection")?;
    writeln!(writer, "status: complete")?;
    for (index, record) in records.iter().enumerate() {
        write!(writer, "record[{index}]: ")?;
        write_record(writer, record)?;
        writeln!(writer)?;
    }
    Ok(())
}

fn write_record(writer: &mut impl Write, record: &RuleCollectionDiagnosticRecord) -> fmt::Result {
    match record {
        RuleCollectionDiagnosticRecord::Stylesheet {
            source_id,
            order,
            origin,
            namespace_constraint,
            condition,
        } => {
            write!(
                writer,
                "stylesheet source={} order={} origin={} namespace={} condition=",
                source_id.get(),
                order.get(),
                origin_label(*origin),
                namespace_label(*namespace_constraint)
            )?;
            match condition {
                DiagnosticCondition::Active => writer.write_str("active"),
                DiagnosticCondition::DeferredUnsupported(text) => {
                    writer.write_str("deferred-unsupported text=")?;
                    write_bounded_diagnostic_text(writer, text)
                }
            }
        }
        RuleCollectionDiagnosticRecord::Rule {
            source_id,
            raw_rule_index,
            style_position,
            source_order,
            state,
        } => {
            write!(
                writer,
                "rule source={} raw={} style-position=",
                source_id.get(),
                raw_rule_index.get()
            )?;
            write_optional_u32(writer, style_position.map(|value| value.get()))?;
            writer.write_str(" source-order=")?;
            match source_order {
                Some(order) => write!(
                    writer,
                    "{}/{}",
                    order.stylesheet().get(),
                    order.rule().get()
                )?,
                None => writer.write_str("none")?,
            }
            write!(writer, " state={}", rule_state_label(state))?;
            match state {
                DiagnosticRuleState::InvalidSelector { reason } => {
                    write!(writer, " reason={}", invalid_selector_reason_label(*reason))
                }
                DiagnosticRuleState::UnsupportedSelector { features } => {
                    writer.write_str(" features=[")?;
                    for (index, feature) in features.iter().copied().enumerate() {
                        if index != 0 {
                            writer.write_char(',')?;
                        }
                        writer.write_str(unsupported_selector_feature_label(feature))?;
                    }
                    writer.write_char(']')
                }
                DiagnosticRuleState::SkippedAtRule { name, .. } => {
                    writer.write_str(" name=")?;
                    match name {
                        Some(name) => write_bounded_diagnostic_text(writer, name),
                        None => writer.write_str("none"),
                    }
                }
                DiagnosticRuleState::Active | DiagnosticRuleState::InactiveCondition => Ok(()),
            }
        }
        RuleCollectionDiagnosticRecord::Declaration {
            source_id,
            raw_rule_index,
            declaration_source_index,
            declaration_order,
            expansion_order,
            importance,
            property,
            value,
            applicability,
            invalid_reason,
        } => {
            write!(
                writer,
                "declaration source={} raw={} source-index={} order={} expansion={} importance={} property=",
                source_id.get(),
                raw_rule_index.get(),
                declaration_source_index.get(),
                declaration_order.get(),
                expansion_order,
                importance_label(*importance)
            )?;
            write_diagnostic_property(writer, property)?;
            writer.write_str(" value=")?;
            write_optional_bounded_text(writer, value.as_ref())?;
            write!(
                writer,
                " applicability={} invalid=",
                applicability_label(*applicability)
            )?;
            match invalid_reason {
                Some(reason) => writer.write_str(reason),
                None => writer.write_str("none"),
            }
        }
        RuleCollectionDiagnosticRecord::InlineDeclaration {
            element_id,
            declaration_source_index,
            declaration_order,
            expansion_order,
            importance,
            property,
            value,
            applicability,
            invalid_reason,
        } => {
            write!(
                writer,
                "inline-declaration element={element_id} source-index={} order={} expansion={} importance={} property=",
                declaration_source_index.get(),
                declaration_order.get(),
                expansion_order,
                importance_label(*importance)
            )?;
            write_diagnostic_property(writer, property)?;
            writer.write_str(" value=")?;
            write_optional_bounded_text(writer, value.as_ref())?;
            write!(
                writer,
                " applicability={} invalid=",
                applicability_label(*applicability)
            )?;
            match invalid_reason {
                Some(reason) => writer.write_str(reason),
                None => writer.write_str("none"),
            }
        }
        RuleCollectionDiagnosticRecord::Match {
            element_id,
            source_id,
            raw_rule_index,
            outcome,
            effective_specificity,
        } => {
            write!(
                writer,
                "match element={element_id} source={} raw={} matchability={} matched=[",
                source_id.get(),
                raw_rule_index.get(),
                matchability_label(outcome.matchability())
            )?;
            for (index, matched) in outcome.matched_selectors().iter().enumerate() {
                if index != 0 {
                    writer.write_char(',')?;
                }
                let specificity = matched.specificity();
                write!(
                    writer,
                    "{}:{}/{}/{}",
                    matched.selector_index(),
                    specificity.a(),
                    specificity.b(),
                    specificity.c()
                )?;
            }
            writer.write_str("] specificity=")?;
            match effective_specificity {
                Some(specificity) => write!(
                    writer,
                    "{}/{}/{}",
                    specificity.a(),
                    specificity.b(),
                    specificity.c()
                ),
                None => writer.write_str("none"),
            }
        }
    }
}

fn write_diagnostic_property(
    writer: &mut impl Write,
    property: &DiagnosticDeclarationProperty,
) -> fmt::Result {
    match property {
        DiagnosticDeclarationProperty::Supported { name } => {
            write!(writer, "supported({name})")
        }
        DiagnosticDeclarationProperty::InvalidValue { name } => {
            write!(writer, "invalid-value({name})")
        }
        DiagnosticDeclarationProperty::InvalidShorthand { name } => {
            write!(writer, "invalid-shorthand({name})")
        }
        DiagnosticDeclarationProperty::Unsupported { name } => {
            writer.write_str("unsupported(")?;
            write_bounded_diagnostic_text(writer, name)?;
            writer.write_char(')')
        }
        DiagnosticDeclarationProperty::Custom { name } => {
            writer.write_str("custom(")?;
            write_bounded_diagnostic_text(writer, name)?;
            writer.write_char(')')
        }
        DiagnosticDeclarationProperty::InvalidName => writer.write_str("invalid-name"),
    }
}

fn write_optional_bounded_text(
    writer: &mut impl Write,
    value: Option<&BoundedDiagnosticText>,
) -> fmt::Result {
    match value {
        Some(value) => write_bounded_diagnostic_text(writer, value),
        None => writer.write_str("none"),
    }
}

fn write_bounded_diagnostic_text(
    writer: &mut impl Write,
    text: &BoundedDiagnosticText,
) -> fmt::Result {
    write_quoted(writer, &text.text)?;
    if text.truncated {
        write!(writer, "[original-bytes={}]", text.original_bytes)?;
    }
    Ok(())
}

fn origin_label(origin: CascadeOrigin) -> &'static str {
    match origin {
        CascadeOrigin::UserAgent => "user-agent",
        CascadeOrigin::User => "user",
        CascadeOrigin::Author => "author",
    }
}

fn namespace_label(namespace: SelectorNamespaceConstraint) -> &'static str {
    match namespace {
        SelectorNamespaceConstraint::Unconstrained => "unconstrained",
        SelectorNamespaceConstraint::Exact(namespace) => namespace.snapshot_name(),
    }
}

fn rule_state_label(state: &DiagnosticRuleState) -> &'static str {
    match state {
        DiagnosticRuleState::Active => "active",
        DiagnosticRuleState::InactiveCondition => "inactive-condition",
        DiagnosticRuleState::InvalidSelector { .. } => "invalid-selector",
        DiagnosticRuleState::UnsupportedSelector { .. } => "unsupported-selector",
        DiagnosticRuleState::SkippedAtRule { reason, .. } => at_rule_reason_label(*reason),
    }
}

fn at_rule_reason_label(reason: super::collection::AtRuleSkipReason) -> &'static str {
    match reason {
        super::collection::AtRuleSkipReason::MediaDeferred => "skipped-at-media",
        super::collection::AtRuleSkipReason::SupportsDeferred => "skipped-at-supports",
        super::collection::AtRuleSkipReason::ImportDeferred => "skipped-at-import",
        super::collection::AtRuleSkipReason::LayerDeferred => "skipped-at-layer",
        super::collection::AtRuleSkipReason::ScopeDeferred => "skipped-at-scope",
        super::collection::AtRuleSkipReason::Unknown => "skipped-at-unknown",
        super::collection::AtRuleSkipReason::UnresolvedName => "skipped-at-unresolved",
    }
}

fn invalid_selector_reason_label(reason: InvalidSelectorReason) -> &'static str {
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

fn unsupported_selector_feature_label(feature: UnsupportedSelectorFeature) -> &'static str {
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

fn importance_label(importance: CascadeImportance) -> &'static str {
    match importance {
        CascadeImportance::Normal => "normal",
        CascadeImportance::Important => "important",
    }
}

fn applicability_label(applicability: CascadeDeclarationApplicability) -> &'static str {
    match applicability {
        CascadeDeclarationApplicability::Supported(_) => "supported",
        CascadeDeclarationApplicability::InvalidValue(_) => "invalid-value",
        CascadeDeclarationApplicability::InvalidShorthandValue(_) => "invalid-shorthand-value",
        CascadeDeclarationApplicability::UnsupportedProperty => "unsupported-property",
        CascadeDeclarationApplicability::CustomProperty => "custom-property",
        CascadeDeclarationApplicability::InvalidPropertyName => "invalid-property-name",
    }
}

fn matchability_label(matchability: crate::selectors::SelectorMatchability) -> &'static str {
    match matchability {
        crate::selectors::SelectorMatchability::Parsed => "parsed",
        crate::selectors::SelectorMatchability::Unsupported => "unsupported",
        crate::selectors::SelectorMatchability::Invalid => "invalid",
    }
}

fn write_optional_u32(writer: &mut impl Write, value: Option<u32>) -> fmt::Result {
    match value {
        Some(value) => write!(writer, "{value}"),
        None => writer.write_str("none"),
    }
}

fn write_quoted(writer: &mut impl Write, text: &str) -> fmt::Result {
    writer.write_char('"')?;
    for character in text.chars() {
        match character {
            '\\' => writer.write_str("\\\\")?,
            '"' => writer.write_str("\\\"")?,
            '\n' => writer.write_str("\\n")?,
            '\r' => writer.write_str("\\r")?,
            '\t' => writer.write_str("\\t")?,
            character => writer.write_char(character)?,
        }
    }
    writer.write_char('"')
}

struct BoundedWriter {
    output: String,
    maximum: usize,
}

struct CountingWriter {
    bytes: usize,
    maximum: usize,
}

impl Write for CountingWriter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let new_len = self.bytes.checked_add(text.len()).ok_or(fmt::Error)?;
        self.bytes = new_len;
        if new_len > self.maximum {
            return Err(fmt::Error);
        }
        Ok(())
    }
}

impl Write for BoundedWriter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let new_len = self
            .output
            .len()
            .checked_add(text.len())
            .ok_or(fmt::Error)?;
        if new_len > self.maximum {
            return Err(fmt::Error);
        }
        self.output.push_str(text);
        Ok(())
    }
}

fn limit_failure(
    limit: RuleCollectionDiagnosticLimit,
    configured: usize,
    observed_at_least: usize,
) -> RuleCollectionDiagnosticFailure {
    RuleCollectionDiagnosticFailure::LimitExceeded {
        limit,
        configured,
        observed_at_least,
    }
}

fn serialized_limit(maximum: usize) -> RuleCollectionDiagnosticFailure {
    limit_failure(
        RuleCollectionDiagnosticLimit::SerializedBytes,
        maximum,
        maximum,
    )
}

fn failure_label(failure: &RuleCollectionDiagnosticFailure) -> String {
    match failure {
        RuleCollectionDiagnosticFailure::Collection(error) => format!(
            "{} kind={} detail={error}",
            failure.stable_label(),
            error.stable_label()
        ),
        RuleCollectionDiagnosticFailure::Resolution(error) => format!(
            "{} kind={} detail={error}",
            failure.stable_label(),
            error.stable_label()
        ),
        RuleCollectionDiagnosticFailure::LimitExceeded {
            limit,
            configured,
            observed_at_least,
        } => format!(
            "{} limit={} configured={configured} observed-at-least={observed_at_least}",
            failure.stable_label(),
            diagnostic_limit_label(*limit)
        ),
        RuleCollectionDiagnosticFailure::Reservation { storage } => {
            format!(
                "{} storage={}",
                failure.stable_label(),
                diagnostic_storage_label(*storage)
            )
        }
    }
}

fn diagnostic_limit_label(limit: RuleCollectionDiagnosticLimit) -> &'static str {
    match limit {
        RuleCollectionDiagnosticLimit::Records => "records",
        RuleCollectionDiagnosticLimit::StorageBytes => "storage-bytes",
        RuleCollectionDiagnosticLimit::SerializedBytes => "serialized-bytes",
    }
}

fn diagnostic_storage_label(storage: RuleCollectionDiagnosticStorage) -> &'static str {
    match storage {
        RuleCollectionDiagnosticStorage::Records => "records",
        RuleCollectionDiagnosticStorage::SerializedOutput => "serialized-output",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cascade::contract::SourceCoordinateError;

    fn failed_snapshot(failure: RuleCollectionDiagnosticFailure) -> String {
        RuleCollectionDiagnostic::Failed(failure).to_debug_snapshot()
    }

    #[test]
    fn bounded_text_truncates_only_at_utf8_boundaries_and_reports_original_bytes() {
        let text = bounded_text("éclair", 2).expect("bounded diagnostic text builds");
        assert_eq!(text.text, "é");
        assert_eq!(text.original_bytes, "éclair".len());
        assert!(text.truncated);

        let mut first = String::new();
        write_bounded_diagnostic_text(&mut first, &text).expect("bounded text serializes");
        let mut second = String::new();
        write_bounded_diagnostic_text(&mut second, &text).expect("bounded text repeats");
        assert_eq!(first, "\"é\"[original-bytes=7]");
        assert_eq!(first, second);
    }

    #[test]
    fn every_rule_collection_build_failure_has_a_stable_non_debug_snapshot() {
        let source_id = StylesheetSourceId::in_memory_generation_index(7);
        let cases = [
            RuleCollectionBuildError::UnsupportedConfiguration {
                limit: crate::cascade::StyleResolutionLimit::TopLevelRulesPerDocument,
                configured: 9,
                maximum: 8,
            },
            RuleCollectionBuildError::LimitExceeded {
                limit: crate::cascade::StyleResolutionLimit::TopLevelRulesPerDocument,
                configured: 1,
                observed: 2,
            },
            RuleCollectionBuildError::DuplicateSourceId { source_id },
            RuleCollectionBuildError::DuplicateStylesheetOrder {
                order: StylesheetOrder::new(4),
            },
            RuleCollectionBuildError::NonMonotonicStylesheetOrder {
                previous: StylesheetOrder::new(4),
                current: StylesheetOrder::new(2),
            },
            RuleCollectionBuildError::SelectorStateInvariant {
                source_id,
                raw_rule_index: RawRuleIndex::new(3),
            },
            RuleCollectionBuildError::Coordinate(SourceCoordinateError::Unrepresentable {
                coordinate: "raw-rule-index",
                value: 9,
                maximum: 8,
            }),
            RuleCollectionBuildError::Coordinate(SourceCoordinateError::CounterExhausted {
                coordinate: "style-rule-position",
            }),
            RuleCollectionBuildError::Reservation {
                storage: super::super::collection::RuleCollectionStorage::Stylesheets,
            },
            RuleCollectionBuildError::Reservation {
                storage: super::super::collection::RuleCollectionStorage::Rules,
            },
            RuleCollectionBuildError::Reservation {
                storage: super::super::collection::RuleCollectionStorage::Declarations,
            },
        ];
        let snapshots = cases
            .into_iter()
            .map(|error| failed_snapshot(RuleCollectionDiagnosticFailure::Collection(error)))
            .collect::<Vec<_>>();
        assert_eq!(
            snapshots,
            vec![
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: collection kind=unsupported-configuration detail=rule collection configured top-level-rules-per-document limit 9 above representable maximum 8\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: collection kind=limit-exceeded detail=rule collection observed 2 entries above top-level-rules-per-document limit 1\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: collection kind=duplicate-source-id detail=duplicate stylesheet source id 31\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: collection kind=duplicate-stylesheet-order detail=duplicate stylesheet order 4\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: collection kind=non-monotonic-stylesheet-order detail=stylesheet order 2 follows non-earlier order 4\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: collection kind=selector-state-invariant detail=stylesheet source 31 raw rule 3 has no classified selector state\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: collection kind=unrepresentable detail=raw-rule-index value 9 exceeds representable maximum 8\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: collection kind=counter-exhausted detail=style-rule-position counter exhausted\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: collection kind=reservation detail=failed to reserve rule collection stylesheets storage\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: collection kind=reservation detail=failed to reserve rule collection rules storage\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: collection kind=reservation detail=failed to reserve rule collection declarations storage\n",
            ]
        );
    }

    #[test]
    fn every_af5_diagnostic_failure_variant_has_a_stable_label_and_snapshot() {
        let cases = [
            RuleCollectionDiagnosticFailure::Collection(
                RuleCollectionBuildError::DuplicateStylesheetOrder {
                    order: StylesheetOrder::new(2),
                },
            ),
            RuleCollectionDiagnosticFailure::Resolution(StyleResolutionError::LimitExceeded {
                limit: crate::cascade::StyleResolutionLimit::MatchedRulesPerElement,
                configured: 4,
            }),
            RuleCollectionDiagnosticFailure::Resolution(StyleResolutionError::SourceCoordinate(
                SourceCoordinateError::CounterExhausted {
                    coordinate: "inline-declaration-order",
                },
            )),
            RuleCollectionDiagnosticFailure::LimitExceeded {
                limit: RuleCollectionDiagnosticLimit::Records,
                configured: 1,
                observed_at_least: 2,
            },
            RuleCollectionDiagnosticFailure::LimitExceeded {
                limit: RuleCollectionDiagnosticLimit::StorageBytes,
                configured: 3,
                observed_at_least: 4,
            },
            RuleCollectionDiagnosticFailure::LimitExceeded {
                limit: RuleCollectionDiagnosticLimit::SerializedBytes,
                configured: 5,
                observed_at_least: 6,
            },
            RuleCollectionDiagnosticFailure::Reservation {
                storage: RuleCollectionDiagnosticStorage::Records,
            },
            RuleCollectionDiagnosticFailure::Reservation {
                storage: RuleCollectionDiagnosticStorage::SerializedOutput,
            },
        ];
        let snapshots = cases.into_iter().map(failed_snapshot).collect::<Vec<_>>();
        assert_eq!(
            snapshots,
            vec![
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: collection kind=duplicate-stylesheet-order detail=duplicate stylesheet order 2\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: resolution kind=limit-exceeded detail=style resolution exceeded matched-rules-per-element limit 4\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: resolution kind=counter-exhausted detail=style execution source coordinate: inline-declaration-order counter exhausted\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: limit-exceeded limit=records configured=1 observed-at-least=2\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: limit-exceeded limit=storage-bytes configured=3 observed-at-least=4\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: limit-exceeded limit=serialized-bytes configured=5 observed-at-least=6\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: reservation storage=records\n",
                "version: 2\naf5-rule-collection\nstatus: failed\nfailure: reservation storage=serialized-output\n",
            ]
        );
    }
}
