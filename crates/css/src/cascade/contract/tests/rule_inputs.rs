use super::super::{
    CascadeDeclarationApplicability, CascadeDeclarationInput, CascadeDeclarationProperty,
    CascadeImportance, CascadeOrigin, CascadePropertyId, CascadeRuleInput, CascadeRuleSource,
    InlineStyleRuleRef, StylesheetRuleRef,
};
use super::support::{
    inline_declaration_source, matched_rule, parse_error, parsed_value, preserved_value,
    stylesheet_declaration_source,
};
use crate::SpecifiedValueParseErrorKind;
use crate::selectors::Specificity;

#[test]
fn cascade_rule_match_uses_highest_selector_specificity() {
    let mut builder = crate::selectors::SelectorListMatchOutcome::builder();
    builder.record_match(0, Specificity::TYPE);
    builder.record_match(2, Specificity::CLASS);

    let rule_match = super::super::CascadeRuleMatch {
        stylesheet_index: 0,
        rule_index: 1,
        outcome: builder.build(),
    };

    assert!(rule_match.contributes_candidates());
    assert_eq!(rule_match.effective_specificity(), Some(Specificity::CLASS));
}

#[test]
fn cascade_rule_input_materializes_supported_candidates_with_explicit_priority() {
    let rule_match = matched_rule(2, 5, &[Specificity::TYPE, Specificity::CLASS]);
    let source = CascadeRuleSource::Stylesheet(StylesheetRuleRef::from_rule_match(&rule_match));
    let rule = CascadeRuleInput::from_stylesheet_match(
        &rule_match,
        CascadeOrigin::Author,
        11,
        vec![
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(2, 5, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: red"),
            ),
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(2, 5, 1),
                1,
                CascadeImportance::Important,
                CascadePropertyId::Color,
                parsed_value("color: blue"),
            ),
        ],
    )
    .expect("valid matched stylesheet rule")
    .expect("matched rule contributes");

    let candidates = rule.candidates();
    let context = rule.context();
    assert_eq!(rule.source(), source);
    assert_eq!(rule.context(), context);
    assert_eq!(rule.declarations().len(), 2);
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].property(), CascadePropertyId::Color);
    assert_eq!(
        candidates[0].source(),
        stylesheet_declaration_source(2, 5, 0)
    );
    assert_eq!(
        candidates[0].priority(),
        context.priority_for_declaration(CascadeImportance::Normal, 0)
    );
    assert_eq!(
        candidates[1].priority(),
        context.priority_for_declaration(CascadeImportance::Important, 1)
    );
    assert_eq!(candidates[1].value().to_css_text().as_deref(), Some("blue"));

    let winner = candidates[1].to_winner();
    assert_eq!(winner.source, stylesheet_declaration_source(2, 5, 1));
    assert_eq!(
        winner.priority,
        context.priority_for_declaration(CascadeImportance::Important, 1)
    );
    assert_eq!(winner.value.to_css_text().as_deref(), Some("blue"));
}

#[test]
fn cascade_rule_input_keeps_declaration_filter_state_explicit() {
    let inline_style = InlineStyleRuleRef::new(7);
    let rule = CascadeRuleInput::from_inline_style(
        inline_style,
        0,
        vec![
            CascadeDeclarationInput::supported(
                inline_declaration_source(inline_style, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: red"),
            ),
            CascadeDeclarationInput::unsupported_property(
                inline_declaration_source(inline_style, 1),
                1,
                CascadeImportance::Normal,
                "zoom",
                parsed_value("zoom: 2"),
            ),
            CascadeDeclarationInput::custom_property(
                inline_declaration_source(inline_style, 2),
                2,
                CascadeImportance::Normal,
                "--brand",
                parsed_value("--brand: teal"),
            ),
            CascadeDeclarationInput::invalid_property_name(
                inline_declaration_source(inline_style, 3),
                3,
                CascadeImportance::Normal,
                preserved_value("color: green"),
            ),
            CascadeDeclarationInput::invalid_value(
                inline_declaration_source(inline_style, 4),
                4,
                CascadeImportance::Normal,
                CascadePropertyId::Display,
                parse_error(CascadePropertyId::Display, "display: grid"),
                preserved_value("display: grid"),
            ),
        ],
    )
    .expect("valid inline style rule");
    let context = rule.context();

    assert_eq!(
        rule.declarations()[0].applicability(),
        CascadeDeclarationApplicability::Supported(CascadePropertyId::Color)
    );
    assert_eq!(
        rule.declarations()[0].property(),
        &CascadeDeclarationProperty::Supported(CascadePropertyId::Color)
    );
    assert_eq!(rule.declarations()[1].property_name(), Some("zoom"));
    assert_eq!(
        rule.declarations()[1].applicability(),
        CascadeDeclarationApplicability::UnsupportedProperty
    );
    assert_eq!(
        rule.declarations()[1].property(),
        &CascadeDeclarationProperty::Unsupported("zoom".to_string())
    );
    assert_eq!(rule.declarations()[2].property_name(), Some("--brand"));
    assert_eq!(
        rule.declarations()[2].applicability(),
        CascadeDeclarationApplicability::CustomProperty
    );
    assert_eq!(
        rule.declarations()[2].property(),
        &CascadeDeclarationProperty::Custom("--brand".to_string())
    );
    assert_eq!(rule.declarations()[3].property_name(), None);
    assert_eq!(
        rule.declarations()[3].applicability(),
        CascadeDeclarationApplicability::InvalidPropertyName
    );
    assert_eq!(
        rule.declarations()[3].property(),
        &CascadeDeclarationProperty::Invalid
    );
    assert_eq!(rule.declarations()[4].property_name(), Some("display"));
    assert_eq!(
        rule.declarations()[4].applicability(),
        CascadeDeclarationApplicability::InvalidValue(CascadePropertyId::Display)
    );
    assert_eq!(
        rule.declarations()[4].property(),
        &CascadeDeclarationProperty::InvalidValue(CascadePropertyId::Display)
    );
    assert_eq!(
        rule.declarations()[4]
            .invalid_value_error()
            .expect("invalid value error")
            .kind(),
        SpecifiedValueParseErrorKind::UnsupportedDisplayKeyword
    );

    let candidates = rule.candidates();
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].source(),
        inline_declaration_source(inline_style, 0)
    );
    assert_eq!(
        candidates[0].priority(),
        context.priority_for_declaration(CascadeImportance::Normal, 0)
    );
}

#[test]
fn cascade_rule_input_rejects_declarations_from_a_different_inline_style_source() {
    let inline_style = InlineStyleRuleRef::new(1);
    let other_inline_style = InlineStyleRuleRef::new(2);
    let error = CascadeRuleInput::from_inline_style(
        inline_style,
        0,
        vec![CascadeDeclarationInput::supported(
            inline_declaration_source(other_inline_style, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: red"),
        )],
    )
    .expect_err("mismatched inline source");

    assert_eq!(
        error.rule_source(),
        CascadeRuleSource::InlineStyle(inline_style)
    );
    assert_eq!(
        error.declaration_source(),
        inline_declaration_source(other_inline_style, 0)
    );
    assert_eq!(error.declaration_position(), 0);
}
