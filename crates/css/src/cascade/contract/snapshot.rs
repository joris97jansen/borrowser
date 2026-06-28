use std::fmt::Write;

use super::declarations::{
    CascadeDeclarationApplicability, CascadeDeclarationProperty, CascadeSpecifiedValue,
};
use super::priority::{CascadeImportance, CascadeOrigin, CascadeSpecificity};
use super::resolved_style::{CssWideResolvedSource, ResolvedStyle, ResolvedValueSource};
use super::rules::CascadeRuleInput;
use super::sources::{CascadeDeclarationSource, CascadeRuleSource};
use super::winners::{
    CascadeDeclarationCandidate, CascadeWinner, CascadeWinnerSet, resolve_cascade_winners,
    sort_candidates_by_cascade_order,
};

/// Maintenance-facing debug snapshots for the cascade contract.
///
/// This module owns debug formatting for rule inputs, winner sets, and
/// resolved-style output. It does not own CSS value serialization.
pub fn cascade_evaluation_debug_snapshot(rule_inputs: &[CascadeRuleInput]) -> String {
    let mut out = String::new();
    append_cascade_evaluation_debug_snapshot(&mut out, rule_inputs, true);
    out
}

pub(crate) fn append_cascade_evaluation_debug_snapshot(
    out: &mut String,
    rule_inputs: &[CascadeRuleInput],
    include_version: bool,
) -> CascadeWinnerSet {
    let candidates = rule_inputs
        .iter()
        .flat_map(|rule_input| rule_input.candidates())
        .collect::<Vec<_>>();
    let mut ordered_candidates = candidates.clone();
    sort_candidates_by_cascade_order(&mut ordered_candidates);
    let winners = resolve_cascade_winners(&candidates);

    if include_version {
        writeln!(out, "version: 1").expect("write snapshot");
    }
    writeln!(out, "cascade-evaluation").expect("write snapshot");
    writeln!(out, "rule-inputs: {}", rule_inputs.len()).expect("write snapshot");
    for (rule_index, rule_input) in rule_inputs.iter().enumerate() {
        let context = rule_input.context();
        writeln!(
            out,
            "  rule-input[{rule_index}]: source={} origin={} specificity={} rule-order={} declarations={}",
            rule_source_label(rule_input.source()),
            origin_label(context.origin),
            specificity_label(context.specificity),
            context.rule_order,
            rule_input.declarations().len(),
        )
        .expect("write snapshot");
        for (declaration_index, declaration) in rule_input.declarations().iter().enumerate() {
            writeln!(
                out,
                "    declaration[{declaration_index}]: source={} declaration-order={} importance={} property={} applicability={} value={}",
                declaration_source_label(declaration.source()),
                declaration_order_label(declaration.declaration_order(), declaration.expansion_order()),
                importance_label(declaration.importance()),
                declaration_property_label(declaration.property()),
                applicability_label(declaration.applicability()),
                specified_value_label(declaration.value()),
            )
            .expect("write snapshot");
        }
    }

    writeln!(out, "candidates-source-order: {}", candidates.len()).expect("write snapshot");
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        writeln!(
            out,
            "  candidate[{candidate_index}]: {}",
            candidate_snapshot_label(candidate),
        )
        .expect("write snapshot");
    }

    writeln!(
        out,
        "candidates-cascade-order: {}",
        ordered_candidates.len()
    )
    .expect("write snapshot");
    for (candidate_index, candidate) in ordered_candidates.iter().enumerate() {
        writeln!(
            out,
            "  candidate[{candidate_index}]: {}",
            candidate_snapshot_label(candidate),
        )
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

    winners
}

impl CascadeWinnerSet {
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
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
        writeln!(&mut out, "version: 1").expect("write snapshot");
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

fn rule_source_label(source: CascadeRuleSource) -> String {
    match source {
        CascadeRuleSource::Stylesheet(source) => {
            format!(
                "stylesheet[{}/{}]",
                source.stylesheet_index, source.rule_index
            )
        }
        CascadeRuleSource::InlineStyle(source) => format!("inline-style[{}]", source.scope_id),
    }
}

fn origin_label(origin: CascadeOrigin) -> &'static str {
    match origin {
        CascadeOrigin::UserAgent => "user-agent",
        CascadeOrigin::User => "user",
        CascadeOrigin::Author => "author",
    }
}

fn importance_label(importance: CascadeImportance) -> &'static str {
    match importance {
        CascadeImportance::Normal => "normal",
        CascadeImportance::Important => "important",
    }
}

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

fn declaration_order_label(declaration_order: u32, expansion_order: u16) -> String {
    if expansion_order == 0 {
        declaration_order.to_string()
    } else {
        format!("{declaration_order} expansion-order={expansion_order}")
    }
}

fn candidate_snapshot_label(candidate: &CascadeDeclarationCandidate) -> String {
    format!(
        "property={} source={} band={} specificity={} rule-order={} declaration-order={} value={}",
        candidate.property().name(),
        declaration_source_label(candidate.source()),
        candidate.priority().band.as_debug_label(),
        specificity_label(candidate.priority().specificity),
        candidate.priority().rule_order,
        candidate.priority().declaration_order,
        specified_value_label(candidate.value()),
    )
}

fn winner_snapshot_label(winner: &CascadeWinner) -> String {
    format!(
        "winner(source={}, band={}, specificity={}, rule-order={}, declaration-order={}, value={})",
        declaration_source_label(winner.source),
        winner.priority.band.as_debug_label(),
        specificity_label(winner.priority.specificity),
        winner.priority.rule_order,
        winner.priority.declaration_order,
        specified_value_label(&winner.value),
    )
}

fn declaration_source_label(source: CascadeDeclarationSource) -> String {
    match source {
        CascadeDeclarationSource::Stylesheet(source) => format!(
            "stylesheet[{}/{}]/declaration[{}]",
            source.stylesheet_index, source.rule_index, source.declaration_index
        ),
        CascadeDeclarationSource::InlineStyle(source) => format!(
            "inline-style[{}]/declaration[{}]",
            source.inline_style.scope_id, source.declaration_index
        ),
    }
}

fn specified_value_label(value: &CascadeSpecifiedValue) -> String {
    let value = value
        .to_css_text()
        .unwrap_or_else(|| "<unresolved-value>".to_string());
    quoted_snapshot_text(&value)
}

fn specificity_label(specificity: CascadeSpecificity) -> String {
    match specificity {
        CascadeSpecificity::Selector(specificity) => format!(
            "selector({},{},{})",
            specificity.ids(),
            specificity.classes(),
            specificity.types()
        ),
        CascadeSpecificity::InlineStyle => "inline-style".to_string(),
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
