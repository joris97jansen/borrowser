//! CSS cascade resolved-style engine plus the legacy compatibility projection.
//!
//! The Milestone R cascade engine resolves structured declaration winners into
//! deterministic resolved-style objects. The core per-element contract is
//! defined by the `contract` submodule below; this module adds the current
//! document-level integration path that consumes DOM selector matches and
//! stylesheet model data.
//!
//! `attach_styles` remains only as a legacy projection from structured
//! resolved styles into `html::Node::style` so the pre-R computed-style and
//! layout path can continue to run while the computed-value cutover is still in
//! progress.

mod contract;

use crate::model::{self, PropertyNameKind};
use crate::selectors::{SelectorDomElementId, SelectorDomIndex, SelectorMatchingContext};
use crate::syntax::ParseOptions;
use html::Node;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::Arc;

pub use contract::{
    CascadeDeclarationApplicability, CascadeDeclarationCandidate, CascadeDeclarationCandidateKey,
    CascadeDeclarationInput, CascadeDeclarationProperty, CascadeDeclarationSource,
    CascadeImportance, CascadeInheritance, CascadeOrigin, CascadeOriginBand, CascadePriority,
    CascadePropertyId, CascadePropertyMetadata, CascadeRuleContext, CascadeRuleInput,
    CascadeRuleInputBuildError, CascadeRuleMatch, CascadeRuleSource, CascadeSpecificity,
    CascadeSpecifiedValue, CascadeWinner, CascadeWinnerEntry, CascadeWinnerSet,
    CurrentScopeCascadePriorityBand, InitialStyleValue, InlineStyleDeclarationRef,
    InlineStyleRuleRef, ResolvedStyle, ResolvedStyleBuildError, ResolvedStyleBuilder,
    ResolvedStyleEntry, ResolvedValueSource, StylesheetDeclarationRef, StylesheetRuleRef,
    cascade_evaluation_debug_snapshot, resolve_cascade_style,
    resolve_cascade_style_from_rule_inputs, resolve_cascade_winners,
    resolve_cascade_winners_from_rule_inputs, resolve_initial_style,
    sort_candidates_by_cascade_order,
};

/// Resolved cascade output for one DOM element in a document style pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedElementStyle {
    selector_element_id: SelectorDomElementId,
    element_name: String,
    style: ResolvedStyle,
}

impl ResolvedElementStyle {
    pub fn selector_element_id(&self) -> SelectorDomElementId {
        self.selector_element_id
    }

    pub fn element_name(&self) -> &str {
        &self.element_name
    }

    pub fn style(&self) -> &ResolvedStyle {
        &self.style
    }
}

/// Document-order resolved-style output for the element set selector matching
/// can address.
///
/// This is the structured cascade result for the current runtime integration
/// path. It is independent of `html::Node::style` mutation; the legacy bridge
/// projects from this object only after cascade has already resolved winners,
/// inheritance, and defaults.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResolvedDocumentStyle {
    entries: Vec<ResolvedElementStyle>,
}

impl ResolvedDocumentStyle {
    pub fn entries(&self) -> &[ResolvedElementStyle] {
        &self.entries
    }

    pub fn get(&self, element: SelectorDomElementId) -> Option<&ResolvedElementStyle> {
        self.entries
            .iter()
            .find(|entry| entry.selector_element_id == element)
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "resolved-document-style").expect("write snapshot");
        for (index, entry) in self.entries.iter().enumerate() {
            writeln!(
                &mut out,
                "element[{index}]: selector-id={} name=\"{}\"",
                entry.selector_element_id.get(),
                entry.element_name
            )
            .expect("write snapshot");
            for line in entry.style.to_debug_snapshot().lines().skip(1) {
                writeln!(&mut out, "  {line}").expect("write snapshot");
            }
        }
        out
    }
}

pub fn is_css(ct: &Option<String>) -> bool {
    ct.as_deref()
        .map(|s| s.to_ascii_lowercase().starts_with("text/css"))
        .unwrap_or(false)
}

// If the element has an inline style attribute, return its value
pub fn get_inline_style(attributes: &[(Arc<str>, Option<String>)]) -> Option<&str> {
    attributes
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("style"))
        .and_then(|(_, v)| v.as_deref())
}

/// Resolves structured cascade output for every element in `root`.
///
/// The output is ordered by selector-DOM document order and does not mutate the
/// DOM. Stylesheet declarations, inline style attributes, selector match
/// outcomes, winner resolution, inheritance, and initial/default fill all flow
/// through the Milestone R structured cascade pipeline.
pub fn resolve_document_styles(
    root: &Node,
    sheets: &[model::StylesheetParse],
) -> ResolvedDocumentStyle {
    let index = SelectorDomIndex::from_root(root);
    let context = SelectorMatchingContext::new(&index);
    let mut entries = Vec::with_capacity(index.len());
    let mut styles_by_element = BTreeMap::new();

    for element in index.elements() {
        let parent_style = context
            .parent_element(element)
            .and_then(|parent| styles_by_element.get(&parent));
        let rule_inputs = rule_inputs_for_element(&context, element, sheets);
        let style = resolve_cascade_style_from_rule_inputs(&rule_inputs, parent_style);

        styles_by_element.insert(element, style.clone());
        entries.push(ResolvedElementStyle {
            selector_element_id: element,
            element_name: context.element_name(element).to_string(),
            style,
        });
    }

    ResolvedDocumentStyle { entries }
}

/// Stable debug snapshot for document-level cascade style resolution.
///
/// This trace composes the per-element candidate evaluation snapshot with the
/// final resolved style for each element. It is intended for regression tests
/// and triage of cascade ordering, inheritance, and defaulting behavior.
pub fn resolve_document_styles_debug_snapshot(
    root: &Node,
    sheets: &[model::StylesheetParse],
) -> String {
    let index = SelectorDomIndex::from_root(root);
    let context = SelectorMatchingContext::new(&index);
    let mut styles_by_element = BTreeMap::new();
    let mut out = String::new();

    writeln!(&mut out, "version: 1").expect("write snapshot");
    writeln!(&mut out, "document-style-resolution").expect("write snapshot");

    for (element_index, element) in index.elements().enumerate() {
        let parent_style = context
            .parent_element(element)
            .and_then(|parent| styles_by_element.get(&parent));
        let rule_inputs = rule_inputs_for_element(&context, element, sheets);
        let mut cascade_debug = String::new();
        let winners = contract::append_cascade_evaluation_debug_snapshot(
            &mut cascade_debug,
            &rule_inputs,
            false,
        );
        let style = resolve_cascade_style(&winners, parent_style);

        writeln!(
            &mut out,
            "element[{element_index}]: selector-id={} name=\"{}\"",
            element.get(),
            context.element_name(element)
        )
        .expect("write snapshot");
        for line in cascade_debug.lines() {
            writeln!(&mut out, "  {line}").expect("write snapshot");
        }
        for line in style.to_debug_snapshot().lines().skip(1) {
            writeln!(&mut out, "  {line}").expect("write snapshot");
        }

        styles_by_element.insert(element, style);
    }

    out
}

/// Legacy DOM-attached style bridge.
///
/// Cascade itself is no longer driven by this mutation path. The bridge first
/// resolves the structured document style output, then projects authored winner
/// values back into `Node::Element::style` for the pre-computed-values runtime
/// path that still consumes string declarations.
pub fn attach_styles(dom: &mut Node, sheets: &[model::StylesheetParse]) {
    let resolved_styles = resolve_document_styles(dom, sheets);
    let mut entries = resolved_styles.entries().iter();
    project_resolved_styles_to_dom(dom, &mut entries);
    debug_assert!(
        entries.next().is_none(),
        "resolved document style must contain exactly one entry per element"
    );
}

fn rule_inputs_for_element(
    context: &SelectorMatchingContext<'_, SelectorDomIndex<'_>>,
    element: SelectorDomElementId,
    sheets: &[model::StylesheetParse],
) -> Vec<CascadeRuleInput> {
    let mut rule_inputs = Vec::new();
    let mut rule_order = 0u32;

    for (stylesheet_index, sheet) in sheets.iter().enumerate() {
        let stylesheet_index = u32_index(stylesheet_index, "stylesheet");
        for (rule_index, rule) in sheet.stylesheet.rules.iter().enumerate() {
            let rule_index = u32_index(rule_index, "rule");
            let model::Rule::Style(rule) = rule else {
                continue;
            };
            let current_rule_order = rule_order;
            rule_order = rule_order
                .checked_add(1)
                .expect("stylesheet rule order exceeds u32 range");

            let rule_match = CascadeRuleMatch {
                stylesheet_index,
                rule_index,
                outcome: context.match_selector_list(element, &rule.selectors),
            };
            if !rule_match.contributes_candidates() {
                continue;
            }

            let declarations = stylesheet_declaration_inputs(
                stylesheet_index,
                rule_index,
                &rule.declarations.declarations,
            );
            if declarations.is_empty() {
                continue;
            }

            if let Some(rule_input) = CascadeRuleInput::from_stylesheet_match(
                &rule_match,
                CascadeOrigin::Author,
                current_rule_order,
                declarations,
            )
            .expect("stylesheet declarations must belong to their stylesheet rule")
            {
                rule_inputs.push(rule_input);
            }
        }
    }

    let inline_rule_order = rule_order;
    if let Some(inline_style) = context.attribute_value(element, "style")
        && let Some(rule_input) = inline_style_rule_input(element, inline_rule_order, inline_style)
    {
        rule_inputs.push(rule_input);
    }

    rule_inputs
}

fn stylesheet_declaration_inputs(
    stylesheet_index: u32,
    rule_index: u32,
    declarations: &[model::Declaration],
) -> Vec<CascadeDeclarationInput> {
    declarations
        .iter()
        .enumerate()
        .map(|(declaration_index, declaration)| {
            let declaration_index = u32_index(declaration_index, "declaration");
            declaration_input_from_model(
                CascadeDeclarationSource::Stylesheet(StylesheetDeclarationRef {
                    stylesheet_index,
                    rule_index,
                    declaration_index,
                }),
                declaration_index,
                declaration,
            )
        })
        .collect()
}

fn inline_style_rule_input(
    element: SelectorDomElementId,
    rule_order: u32,
    inline_style_text: &str,
) -> Option<CascadeRuleInput> {
    if inline_style_text.trim().is_empty() {
        return None;
    }

    let inline_style = InlineStyleRuleRef::new(element.get());
    let declarations = inline_style_declaration_inputs(inline_style, inline_style_text);
    if declarations.is_empty() {
        return None;
    }

    Some(
        CascadeRuleInput::from_inline_style(inline_style, rule_order, declarations)
            .expect("inline declarations must belong to their inline style rule"),
    )
}

fn inline_style_declaration_inputs(
    inline_style: InlineStyleRuleRef,
    inline_style_text: &str,
) -> Vec<CascadeDeclarationInput> {
    // The model layer does not yet expose a first-class declaration-list parse
    // entrypoint. Keep the wrapper localized here so inline style attributes
    // still flow through structured model declarations rather than the legacy
    // string-vector projection.
    let wrapped_rule = format!("* {{ {inline_style_text} }}");
    let parse = model::parse_stylesheet_with_options(&wrapped_rule, &ParseOptions::stylesheet());
    let Some(model::Rule::Style(rule)) = parse.stylesheet.rules.first() else {
        return Vec::new();
    };

    rule.declarations
        .declarations
        .iter()
        .enumerate()
        .map(|(declaration_index, declaration)| {
            let declaration_index = u32_index(declaration_index, "inline declaration");
            declaration_input_from_model(
                CascadeDeclarationSource::InlineStyle(InlineStyleDeclarationRef {
                    inline_style,
                    declaration_index,
                }),
                declaration_index,
                declaration,
            )
        })
        .collect()
}

fn declaration_input_from_model(
    source: CascadeDeclarationSource,
    declaration_order: u32,
    declaration: &model::Declaration,
) -> CascadeDeclarationInput {
    let importance = if declaration.important.is_some() {
        CascadeImportance::Important
    } else {
        CascadeImportance::Normal
    };
    let value = CascadeSpecifiedValue::from_declaration_value(&declaration.value);

    match declaration.name.kind {
        PropertyNameKind::Standard => {
            let Some(property_name) = declaration.name.text.as_deref() else {
                return CascadeDeclarationInput::invalid_property_name(
                    source,
                    declaration_order,
                    importance,
                    value,
                );
            };
            if let Some(property) = CascadePropertyId::from_name(property_name) {
                CascadeDeclarationInput::supported(
                    source,
                    declaration_order,
                    importance,
                    property,
                    value,
                )
            } else {
                CascadeDeclarationInput::unsupported_property(
                    source,
                    declaration_order,
                    importance,
                    property_name,
                    value,
                )
            }
        }
        PropertyNameKind::Custom => {
            let Some(property_name) = declaration.name.text.as_deref() else {
                return CascadeDeclarationInput::invalid_property_name(
                    source,
                    declaration_order,
                    importance,
                    value,
                );
            };
            CascadeDeclarationInput::custom_property(
                source,
                declaration_order,
                importance,
                property_name,
                value,
            )
        }
        PropertyNameKind::Invalid => CascadeDeclarationInput::invalid_property_name(
            source,
            declaration_order,
            importance,
            value,
        ),
    }
}

fn project_resolved_styles_to_dom<'a>(
    node: &mut Node,
    entries: &mut std::slice::Iter<'a, ResolvedElementStyle>,
) {
    match node {
        Node::Document { children, .. } => {
            for child in children {
                project_resolved_styles_to_dom(child, entries);
            }
        }
        Node::Element {
            style, children, ..
        } => {
            let resolved = entries
                .next()
                .expect("resolved document style missing element entry");
            project_resolved_style_to_legacy_vector(resolved.style(), style);
            for child in children {
                project_resolved_styles_to_dom(child, entries);
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
}

fn project_resolved_style_to_legacy_vector(
    resolved_style: &ResolvedStyle,
    target: &mut Vec<(String, String)>,
) {
    target.clear();
    for entry in resolved_style.entries() {
        let Some(winner) = entry.winner() else {
            continue;
        };
        let Some(value) = winner.value.to_css_text() else {
            continue;
        };
        target.push((entry.property().name().to_string(), value));
    }
}

fn u32_index(index: usize, label: &str) -> u32 {
    u32::try_from(index).unwrap_or_else(|_| panic!("{label} index exceeds u32 range"))
}

#[cfg(test)]
mod tests {
    use super::{attach_styles, resolve_document_styles, resolve_document_styles_debug_snapshot};
    use crate::{
        CascadePropertyId, CascadeSpecificity, ParseOptions, ResolvedValueSource,
        parse_stylesheet_with_options,
    };
    use html::{Node, internal::Id};
    use std::sync::Arc;

    #[test]
    fn resolve_document_styles_produces_structured_output_without_mutating_dom() {
        let stylesheets = vec![parse_stylesheet_with_options(
            "main .hero { color: blue; } div { color: red; }",
            &ParseOptions::stylesheet(),
        )];
        let dom = Node::Element {
            id: Id::INVALID,
            name: Arc::from("main"),
            attributes: Vec::new(),
            style: Vec::new(),
            children: vec![Node::Element {
                id: Id::INVALID,
                name: Arc::from("div"),
                attributes: vec![(Arc::from("class"), Some("hero".to_string()))],
                style: Vec::new(),
                children: Vec::new(),
            }],
        };

        let resolved = resolve_document_styles(&dom, &stylesheets);

        let Node::Element {
            style, children, ..
        } = &dom
        else {
            panic!("expected element");
        };
        assert!(style.is_empty());
        let Node::Element {
            style: child_style, ..
        } = &children[0]
        else {
            panic!("expected child element");
        };
        assert!(child_style.is_empty());

        assert_eq!(resolved.entries().len(), 2);
        assert_eq!(resolved.entries()[0].element_name(), "main");
        assert_eq!(resolved.entries()[1].element_name(), "div");
        assert_eq!(
            resolved.entries()[1]
                .style()
                .get(CascadePropertyId::Color)
                .and_then(|entry| entry.winner())
                .and_then(|winner| winner.value.to_css_text())
                .as_deref(),
            Some("blue")
        );
        assert_eq!(
            resolved.entries()[1]
                .style()
                .get(CascadePropertyId::Display)
                .expect("display")
                .source(),
            &ResolvedValueSource::Initial(crate::InitialStyleValue::DisplayInline)
        );
    }

    #[test]
    fn resolve_document_styles_threads_parent_style_for_inheritance() {
        let stylesheets = vec![parse_stylesheet_with_options(
            "section { color: red; }",
            &ParseOptions::stylesheet(),
        )];
        let dom = Node::Element {
            id: Id::INVALID,
            name: Arc::from("section"),
            attributes: Vec::new(),
            style: Vec::new(),
            children: vec![Node::Element {
                id: Id::INVALID,
                name: Arc::from("span"),
                attributes: Vec::new(),
                style: Vec::new(),
                children: Vec::new(),
            }],
        };

        let resolved = resolve_document_styles(&dom, &stylesheets);

        assert_eq!(
            resolved.entries()[0]
                .style()
                .get(CascadePropertyId::Color)
                .and_then(|entry| entry.winner())
                .and_then(|winner| winner.value.to_css_text())
                .as_deref(),
            Some("red")
        );
        assert_eq!(
            resolved.entries()[1]
                .style()
                .get(CascadePropertyId::Color)
                .expect("child color")
                .source(),
            &ResolvedValueSource::Inherited
        );
        assert_eq!(
            resolved.entries()[1]
                .style()
                .get(CascadePropertyId::BackgroundColor)
                .expect("child background")
                .source(),
            &ResolvedValueSource::Initial(crate::InitialStyleValue::TransparentColor)
        );
    }

    #[test]
    fn resolve_document_styles_integrates_inline_style_as_structured_author_output() {
        let stylesheets = vec![parse_stylesheet_with_options(
            ".hero { color: red; width: 10px; }",
            &ParseOptions::stylesheet(),
        )];
        let dom = Node::Element {
            id: Id::INVALID,
            name: Arc::from("div"),
            attributes: vec![
                (Arc::from("class"), Some("hero".to_string())),
                (
                    Arc::from("style"),
                    Some("color: blue; width: 20px;".to_string()),
                ),
            ],
            style: Vec::new(),
            children: Vec::new(),
        };

        let resolved = resolve_document_styles(&dom, &stylesheets);
        let style = resolved.entries()[0].style();

        assert_eq!(
            style
                .get(CascadePropertyId::Color)
                .and_then(|entry| entry.winner())
                .and_then(|winner| winner.value.to_css_text())
                .as_deref(),
            Some("blue")
        );
        assert_eq!(
            style
                .get(CascadePropertyId::Width)
                .and_then(|entry| entry.winner())
                .and_then(|winner| winner.value.to_css_text())
                .as_deref(),
            Some("20px")
        );
        let color_winner = style
            .get(CascadePropertyId::Color)
            .and_then(|entry| entry.winner())
            .expect("inline color winner");
        assert_eq!(
            color_winner.priority.specificity,
            CascadeSpecificity::InlineStyle
        );
        assert_eq!(color_winner.priority.rule_order, 1);
    }

    #[test]
    fn resolved_document_style_debug_snapshot_is_stable() {
        let stylesheets = vec![parse_stylesheet_with_options(
            "div { color: red; }",
            &ParseOptions::stylesheet(),
        )];
        let dom = Node::Element {
            id: Id::INVALID,
            name: Arc::from("div"),
            attributes: Vec::new(),
            style: Vec::new(),
            children: Vec::new(),
        };

        let resolved = resolve_document_styles(&dom, &stylesheets);

        assert_eq!(
            resolved.to_debug_snapshot(),
            concat!(
                "version: 1\n",
                "resolved-document-style\n",
                "element[0]: selector-id=1 name=\"div\"\n",
                "  resolved-style\n",
                "    background-color: initial(transparent)\n",
                "    color: winner(source=stylesheet[0/0]/declaration[0], band=author-normal, specificity=selector(0,0,1), rule-order=0, declaration-order=0, value=\"red\")\n",
                "    display: initial(inline)\n",
                "    font-size: initial(16px)\n",
                "    height: initial(auto)\n",
                "    margin-bottom: initial(0px)\n",
                "    margin-left: initial(0px)\n",
                "    margin-right: initial(0px)\n",
                "    margin-top: initial(0px)\n",
                "    max-width: initial(none)\n",
                "    min-width: initial(auto)\n",
                "    padding-bottom: initial(0px)\n",
                "    padding-left: initial(0px)\n",
                "    padding-right: initial(0px)\n",
                "    padding-top: initial(0px)\n",
                "    width: initial(auto)\n",
            )
        );
    }

    #[test]
    fn document_style_resolution_debug_snapshot_covers_override_inheritance_and_defaults() {
        let stylesheets = vec![parse_stylesheet_with_options(
            "section { color: red; } div { color: green; } .hero { color: blue !important; }",
            &ParseOptions::stylesheet(),
        )];
        let dom = Node::Element {
            id: Id::INVALID,
            name: Arc::from("section"),
            attributes: Vec::new(),
            style: Vec::new(),
            children: vec![Node::Element {
                id: Id::INVALID,
                name: Arc::from("div"),
                attributes: vec![(Arc::from("class"), Some("hero".to_string()))],
                style: Vec::new(),
                children: Vec::new(),
            }],
        };

        assert_eq!(
            resolve_document_styles_debug_snapshot(&dom, &stylesheets),
            concat!(
                "version: 1\n",
                "document-style-resolution\n",
                "element[0]: selector-id=1 name=\"section\"\n",
                "  cascade-evaluation\n",
                "  rule-inputs: 1\n",
                "    rule-input[0]: source=stylesheet[0/0] origin=author specificity=selector(0,0,1) rule-order=0 declarations=1\n",
                "      declaration[0]: source=stylesheet[0/0]/declaration[0] declaration-order=0 importance=normal property=supported(color) applicability=supported(color) value=\"red\"\n",
                "  candidates-source-order: 1\n",
                "    candidate[0]: property=color source=stylesheet[0/0]/declaration[0] band=author-normal specificity=selector(0,0,1) rule-order=0 declaration-order=0 value=\"red\"\n",
                "  candidates-cascade-order: 1\n",
                "    candidate[0]: property=color source=stylesheet[0/0]/declaration[0] band=author-normal specificity=selector(0,0,1) rule-order=0 declaration-order=0 value=\"red\"\n",
                "  winners: 1\n",
                "    color: winner(source=stylesheet[0/0]/declaration[0], band=author-normal, specificity=selector(0,0,1), rule-order=0, declaration-order=0, value=\"red\")\n",
                "  resolved-style\n",
                "    background-color: initial(transparent)\n",
                "    color: winner(source=stylesheet[0/0]/declaration[0], band=author-normal, specificity=selector(0,0,1), rule-order=0, declaration-order=0, value=\"red\")\n",
                "    display: initial(inline)\n",
                "    font-size: initial(16px)\n",
                "    height: initial(auto)\n",
                "    margin-bottom: initial(0px)\n",
                "    margin-left: initial(0px)\n",
                "    margin-right: initial(0px)\n",
                "    margin-top: initial(0px)\n",
                "    max-width: initial(none)\n",
                "    min-width: initial(auto)\n",
                "    padding-bottom: initial(0px)\n",
                "    padding-left: initial(0px)\n",
                "    padding-right: initial(0px)\n",
                "    padding-top: initial(0px)\n",
                "    width: initial(auto)\n",
                "element[1]: selector-id=2 name=\"div\"\n",
                "  cascade-evaluation\n",
                "  rule-inputs: 2\n",
                "    rule-input[0]: source=stylesheet[0/1] origin=author specificity=selector(0,0,1) rule-order=1 declarations=1\n",
                "      declaration[0]: source=stylesheet[0/1]/declaration[0] declaration-order=0 importance=normal property=supported(color) applicability=supported(color) value=\"green\"\n",
                "    rule-input[1]: source=stylesheet[0/2] origin=author specificity=selector(0,1,0) rule-order=2 declarations=1\n",
                "      declaration[0]: source=stylesheet[0/2]/declaration[0] declaration-order=0 importance=important property=supported(color) applicability=supported(color) value=\"blue\"\n",
                "  candidates-source-order: 2\n",
                "    candidate[0]: property=color source=stylesheet[0/1]/declaration[0] band=author-normal specificity=selector(0,0,1) rule-order=1 declaration-order=0 value=\"green\"\n",
                "    candidate[1]: property=color source=stylesheet[0/2]/declaration[0] band=author-important specificity=selector(0,1,0) rule-order=2 declaration-order=0 value=\"blue\"\n",
                "  candidates-cascade-order: 2\n",
                "    candidate[0]: property=color source=stylesheet[0/1]/declaration[0] band=author-normal specificity=selector(0,0,1) rule-order=1 declaration-order=0 value=\"green\"\n",
                "    candidate[1]: property=color source=stylesheet[0/2]/declaration[0] band=author-important specificity=selector(0,1,0) rule-order=2 declaration-order=0 value=\"blue\"\n",
                "  winners: 1\n",
                "    color: winner(source=stylesheet[0/2]/declaration[0], band=author-important, specificity=selector(0,1,0), rule-order=2, declaration-order=0, value=\"blue\")\n",
                "  resolved-style\n",
                "    background-color: initial(transparent)\n",
                "    color: winner(source=stylesheet[0/2]/declaration[0], band=author-important, specificity=selector(0,1,0), rule-order=2, declaration-order=0, value=\"blue\")\n",
                "    display: initial(inline)\n",
                "    font-size: inherited\n",
                "    height: initial(auto)\n",
                "    margin-bottom: initial(0px)\n",
                "    margin-left: initial(0px)\n",
                "    margin-right: initial(0px)\n",
                "    margin-top: initial(0px)\n",
                "    max-width: initial(none)\n",
                "    min-width: initial(auto)\n",
                "    padding-bottom: initial(0px)\n",
                "    padding-left: initial(0px)\n",
                "    padding-right: initial(0px)\n",
                "    padding-top: initial(0px)\n",
                "    width: initial(auto)\n",
            )
        );
    }

    #[test]
    fn attach_styles_projects_structured_winners_into_legacy_dom_style_vector() {
        let stylesheets = vec![parse_stylesheet_with_options(
            "div { color: blue !important; color: red; }",
            &ParseOptions::stylesheet(),
        )];
        let mut dom = Node::Element {
            id: Id::INVALID,
            name: Arc::from("div"),
            attributes: Vec::new(),
            style: Vec::new(),
            children: Vec::new(),
        };

        attach_styles(&mut dom, &stylesheets);

        let Node::Element { style, .. } = dom else {
            panic!("expected element");
        };
        assert_eq!(style, vec![("color".to_string(), "blue".to_string())]);
    }
}
