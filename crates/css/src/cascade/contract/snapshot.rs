use std::fmt::Write;

#[cfg(test)]
use super::DeclarationOrder;
use super::StylesheetRuleOrder;
use super::declarations::CascadeSpecifiedValue;
#[cfg(test)]
use super::declarations::{
    CascadeDeclarationApplicability, CascadeDeclarationInput, CascadeDeclarationProperty,
};
use super::priority::CascadePriority;
#[cfg(test)]
use super::priority::{CascadeImportance, CascadeOrigin};
use super::resolved_style::{CssWideResolvedSource, ResolvedStyle, ResolvedValueSource};
#[cfg(test)]
use super::rules::{CascadeRuleInput, ValidatedCascadeRuleInputs};
use super::sources::CascadeDeclarationSource;
#[cfg(test)]
use super::sources::CascadeRuleSource;
#[cfg(test)]
use super::winners::{
    CascadeCandidateObservationIndex, CascadeDeclarationCandidate, CascadeEvaluationFailure,
    CascadeEvaluationObserver, CascadeResolutionBudget, CascadeResolutionError,
    CascadeResolutionWorkspace, resolve_cascade_winners_from_validated_inputs,
};
use super::winners::{CascadeWinner, CascadeWinnerSet};

/// Maintenance-facing debug snapshots for the cascade contract.
///
/// This module owns debug formatting for rule inputs, winner sets, and
/// resolved-style output. It does not own CSS value serialization.
#[cfg(test)]
pub(crate) fn cascade_evaluation_debug_snapshot(
    rule_inputs: &[CascadeRuleInput<'_>],
) -> Result<String, CascadeResolutionError> {
    let mut out = String::new();
    let (maximum_stylesheet_declarations, maximum_inline_declarations) = rule_inputs
        .iter()
        .try_fold(
            (0usize, 0usize),
            |(stylesheet, inline), input| match input.context() {
                super::sources::CascadeRuleContext::Stylesheet { .. } => stylesheet
                    .checked_add(input.declarations().len())
                    .map(|stylesheet| (stylesheet, inline)),
                super::sources::CascadeRuleContext::InlineStyle => inline
                    .checked_add(input.declarations().len())
                    .map(|inline| (stylesheet, inline)),
            },
        )
        .ok_or(CascadeResolutionError::CandidateCeilingOverflow {
            stylesheet_limit: usize::MAX,
            inline_limit: usize::MAX,
        })?;
    let budget = CascadeResolutionBudget::try_new(
        maximum_stylesheet_declarations,
        maximum_inline_declarations,
        rule_inputs.len(),
    )?;
    let validated =
        ValidatedCascadeRuleInputs::try_from_checked_inputs(rule_inputs.to_vec(), budget)?;
    let mut workspace = CascadeResolutionWorkspace::try_new(budget)?;
    append_cascade_evaluation_debug_snapshot(&mut out, &validated, budget, &mut workspace, true)?;
    Ok(out)
}

#[cfg(test)]
pub(crate) fn append_cascade_evaluation_debug_snapshot(
    out: &mut String,
    rule_inputs: &ValidatedCascadeRuleInputs<'_>,
    budget: CascadeResolutionBudget,
    workspace: &mut CascadeResolutionWorkspace,
    include_version: bool,
) -> Result<CascadeWinnerSet, CascadeResolutionError> {
    let mut observer = SnapshotObserver::default();
    let winners = match resolve_cascade_winners_from_validated_inputs(
        rule_inputs,
        budget,
        workspace,
        &mut observer,
    ) {
        Ok(winners) => winners,
        Err(CascadeEvaluationFailure::Cascade(error)) => return Err(error),
        Err(CascadeEvaluationFailure::Observer(error)) => match error {},
    };
    let mut ordered_candidates = observer.candidates.iter().collect::<Vec<_>>();
    ordered_candidates.sort_by_key(|candidate| (candidate.property, candidate.priority));

    if include_version {
        writeln!(out, "version: 3").expect("write snapshot");
    }
    writeln!(out, "cascade-evaluation").expect("write snapshot");
    writeln!(out, "rule-inputs: {}", rule_inputs.inputs().len()).expect("write snapshot");
    for (rule_index, rule_input) in rule_inputs.inputs().iter().enumerate() {
        let context = rule_input.context();
        writeln!(
            out,
            "  rule-input[{rule_index}]: source={} origin={} attachment={} specificity={} source-order={} declarations={}",
            rule_source_label(rule_input.source()),
            origin_label(context.origin()),
            if matches!(context, super::sources::CascadeRuleContext::InlineStyle) {
                "element-attached"
            } else {
                "style-rule"
            },
            specificity_label(context.specificity()),
            source_order_label(context.source_order()),
            rule_input.declarations().len(),
        )
        .expect("write snapshot");
        for (declaration_index, declaration) in rule_input.declarations().iter().enumerate() {
            writeln!(
                out,
                "    declaration[{declaration_index}]: source={} declaration-order={} importance={} property={} applicability={} value={}{}",
                declaration_source_label(declaration.source()),
                declaration_order_label(declaration.declaration_order(), declaration.expansion_order()),
                importance_label(declaration.importance()),
                declaration_property_label(declaration.property()),
                applicability_label(declaration.applicability()),
                specified_value_label(declaration.value()),
                declaration_error_label(declaration),
            )
            .expect("write snapshot");
        }
    }

    writeln!(
        out,
        "candidates-source-order: {}",
        observer.candidates.len()
    )
    .expect("write snapshot");
    for (candidate_index, candidate) in observer.candidates.iter().enumerate() {
        writeln!(out, "  candidate[{candidate_index}]: {}", candidate.label,)
            .expect("write snapshot");
    }

    writeln!(
        out,
        "candidates-cascade-order: {}",
        ordered_candidates.len()
    )
    .expect("write snapshot");
    for (candidate_index, candidate) in ordered_candidates.iter().enumerate() {
        writeln!(out, "  candidate[{candidate_index}]: {}", candidate.label,)
            .expect("write snapshot");
    }

    writeln!(out, "winners: {}", winners.entries().len()).expect("write snapshot");
    for entry in winners.entries() {
        writeln!(
            out,
            "  {}: {}",
            entry.property().name(),
            winner_snapshot_label(entry.winner()),
        )
        .expect("write snapshot");
    }

    Ok(winners)
}

#[derive(Clone, Debug)]
#[cfg(test)]
struct CandidateSnapshotRecord {
    property: super::properties::CascadePropertyId,
    priority: CascadePriority,
    label: String,
}

#[cfg(test)]
#[derive(Default)]
struct SnapshotObserver {
    candidates: Vec<CandidateSnapshotRecord>,
}

#[cfg(test)]
impl CascadeEvaluationObserver for SnapshotObserver {
    type Error = std::convert::Infallible;

    fn candidate(
        &mut self,
        _observation_index: CascadeCandidateObservationIndex,
        candidate: CascadeDeclarationCandidate<'_>,
    ) -> Result<(), Self::Error> {
        self.candidates.push(CandidateSnapshotRecord {
            property: candidate.property(),
            priority: candidate.priority(),
            label: candidate_snapshot_label(candidate),
        });
        Ok(())
    }
}

impl CascadeWinnerSet {
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 3").expect("write snapshot");
        writeln!(&mut out, "cascade-winners").expect("write snapshot");
        for entry in self.entries() {
            writeln!(
                &mut out,
                "  {}: {}",
                entry.property().name(),
                winner_snapshot_label(entry.winner())
            )
            .expect("write snapshot");
        }
        out
    }
}

impl ResolvedStyle {
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 3").expect("write snapshot");
        writeln!(&mut out, "resolved-style").expect("write snapshot");
        for entry in self.entries() {
            writeln!(
                &mut out,
                "  {}: {}",
                entry.property().name(),
                source_snapshot_label(entry.source())
            )
            .expect("write snapshot");
        }
        out
    }
}

fn source_snapshot_label(source: &ResolvedValueSource) -> String {
    match source {
        ResolvedValueSource::Winner(winner) => winner_snapshot_label(winner),
        ResolvedValueSource::Inherited => "inherited".to_string(),
        ResolvedValueSource::Initial(initial) => {
            format!("initial({})", initial.as_debug_label())
        }
        ResolvedValueSource::CssWideKeyword(source) => css_wide_source_snapshot_label(source),
    }
}

fn css_wide_source_snapshot_label(source: &CssWideResolvedSource) -> String {
    match source {
        CssWideResolvedSource::Initial {
            keyword,
            winner,
            initial,
        } => format!(
            "css-wide-initial(keyword={}, {}, initial={})",
            keyword.as_css_keyword(),
            winner_snapshot_label(winner),
            initial.as_debug_label(),
        ),
        CssWideResolvedSource::Inherited { keyword, winner } => format!(
            "css-wide-inherited(keyword={}, {})",
            keyword.as_css_keyword(),
            winner_snapshot_label(winner),
        ),
    }
}

#[cfg(test)]
fn rule_source_label(source: CascadeRuleSource) -> String {
    match source {
        CascadeRuleSource::Stylesheet(source) => {
            format!(
                "stylesheet[{}/{}]",
                source.source_id().get(),
                source.raw_rule_index().get()
            )
        }
        CascadeRuleSource::InlineStyle(source) => inline_rule_source_label(source),
    }
}

#[cfg(test)]
fn origin_label(origin: CascadeOrigin) -> &'static str {
    match origin {
        CascadeOrigin::UserAgent => "user-agent",
        CascadeOrigin::User => "user",
        CascadeOrigin::Author => "author",
    }
}

#[cfg(test)]
fn importance_label(importance: CascadeImportance) -> &'static str {
    match importance {
        CascadeImportance::Normal => "normal",
        CascadeImportance::Important => "important",
    }
}

#[cfg(test)]
fn declaration_property_label(property: &CascadeDeclarationProperty) -> String {
    match property {
        CascadeDeclarationProperty::Supported(property) => {
            format!("supported({})", property.name())
        }
        CascadeDeclarationProperty::InvalidValue(property) => {
            format!("invalid-value({})", property.name())
        }
        CascadeDeclarationProperty::InvalidShorthandValue(shorthand) => {
            format!("invalid-shorthand-value({})", shorthand.name())
        }
        CascadeDeclarationProperty::Unsupported(name) => {
            format!("unsupported({})", quoted_snapshot_text(name))
        }
        CascadeDeclarationProperty::Custom(name) => {
            format!("custom({})", quoted_snapshot_text(name))
        }
        CascadeDeclarationProperty::Invalid => "invalid".to_string(),
    }
}

#[cfg(test)]
fn applicability_label(applicability: CascadeDeclarationApplicability) -> String {
    match applicability {
        CascadeDeclarationApplicability::Supported(property) => {
            format!("supported({})", property.name())
        }
        CascadeDeclarationApplicability::InvalidValue(property) => {
            format!("invalid-value({})", property.name())
        }
        CascadeDeclarationApplicability::InvalidShorthandValue(shorthand) => {
            format!("invalid-shorthand-value({})", shorthand.name())
        }
        CascadeDeclarationApplicability::UnsupportedProperty => "unsupported-property".to_string(),
        CascadeDeclarationApplicability::CustomProperty => "custom-property".to_string(),
        CascadeDeclarationApplicability::InvalidPropertyName => "invalid-property-name".to_string(),
    }
}

#[cfg(test)]
fn declaration_order_label(declaration_order: DeclarationOrder, expansion_order: u16) -> String {
    if expansion_order == 0 {
        declaration_order.get().to_string()
    } else {
        format!(
            "{} expansion-order={expansion_order}",
            declaration_order.get()
        )
    }
}

#[cfg(test)]
fn candidate_snapshot_label(candidate: CascadeDeclarationCandidate<'_>) -> String {
    format!(
        "property={} source={} band={} attachment={} specificity={} source-order={} declaration-order={} value={}",
        candidate.property().name(),
        declaration_source_label(candidate.source()),
        candidate.priority().band().as_debug_label(),
        declaration_precedence_label(candidate.priority()),
        specificity_label(candidate.priority().specificity()),
        source_order_label(candidate.priority().source_order()),
        candidate.priority().declaration_order().get(),
        specified_value_label(candidate.value()),
    )
}

fn winner_snapshot_label(winner: &CascadeWinner) -> String {
    format!(
        "winner(source={}, band={}, attachment={}, specificity={}, source-order={}, declaration-order={}, value={})",
        declaration_source_label(winner.source),
        winner.priority.band().as_debug_label(),
        declaration_precedence_label(winner.priority),
        specificity_label(winner.priority.specificity()),
        source_order_label(winner.priority.source_order()),
        winner.priority.declaration_order().get(),
        specified_value_label(&winner.value),
    )
}

fn declaration_source_label(source: CascadeDeclarationSource) -> String {
    match source {
        CascadeDeclarationSource::Stylesheet(source) => format!(
            "stylesheet[{}/{}]/declaration[{}]",
            source.source_id().get(),
            source.raw_rule_index().get(),
            source.declaration_index().get()
        ),
        CascadeDeclarationSource::InlineStyle(source) => format!(
            "{}/declaration[{}]",
            inline_rule_source_label(source.inline_style()),
            source.declaration_index().get()
        ),
    }
}

fn inline_rule_source_label(source: super::sources::InlineStyleRuleRef) -> String {
    #[cfg(test)]
    if let super::sources::InlineStyleRuleRef::CompatibilityScope(scope) = source {
        return format!("inline-style[compatibility={scope}]");
    }
    match source.element() {
        Some(element) => format!("inline-style[element={}]", element.get()),
        None => "inline-style[diagnostic]".to_string(),
    }
}

fn source_order_label(order: Option<StylesheetRuleOrder>) -> String {
    match order {
        Some(order) => format!(
            "stylesheet[{}/{}]",
            order.stylesheet().get(),
            order.rule().get()
        ),
        None => "not-applicable".to_string(),
    }
}

fn declaration_precedence_label(priority: CascadePriority) -> &'static str {
    if priority.declaration_precedence().is_element_attached() {
        "element-attached"
    } else {
        "style-rule"
    }
}

fn specified_value_label(value: &CascadeSpecifiedValue) -> String {
    let value = value
        .to_css_text()
        .unwrap_or_else(|| "<unresolved-value>".to_string());
    quoted_snapshot_text(&value)
}

#[cfg(test)]
fn declaration_error_label(declaration: &CascadeDeclarationInput) -> String {
    if let Some(error) = declaration.invalid_value_error() {
        return format!(" invalid-reason={}", error.kind().as_debug_label());
    }
    if let Some(error) = declaration.invalid_shorthand_error() {
        return format!(" invalid-reason={}", error.kind().as_debug_label());
    }
    String::new()
}

fn specificity_label(specificity: Option<crate::selectors::Specificity>) -> String {
    match specificity {
        Some(specificity) => format!(
            "selector({},{},{})",
            specificity.a(),
            specificity.b(),
            specificity.c()
        ),
        None => "not-applicable".to_string(),
    }
}

fn quoted_snapshot_text(text: &str) -> String {
    let mut out = String::new();
    out.push('"');
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}
