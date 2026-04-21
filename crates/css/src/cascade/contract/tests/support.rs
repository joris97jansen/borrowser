use super::super::{
    CascadeDeclarationSource, CascadePropertyId, CascadeRuleMatch, CascadeSpecifiedValue,
    InlineStyleDeclarationRef, InlineStyleRuleRef, ResolvedStyleBuilder, StylesheetDeclarationRef,
};
use crate::selectors::{SelectorListMatchOutcome, Specificity};
use crate::specified::SpecifiedValueParseError;
use crate::{
    ParseOptions, PropertyNameKind, Rule, parse_stylesheet_with_options, property_registry,
};

pub(super) fn matched_rule(
    stylesheet_index: u32,
    rule_index: u32,
    specificities: &[Specificity],
) -> CascadeRuleMatch {
    let mut builder = SelectorListMatchOutcome::builder();
    for (selector_index, specificity) in specificities.iter().copied().enumerate() {
        builder.record_match(selector_index, specificity);
    }
    CascadeRuleMatch {
        stylesheet_index,
        rule_index,
        outcome: builder.build(),
    }
}

pub(super) fn builder_with_initials_except(skip: &[CascadePropertyId]) -> ResolvedStyleBuilder {
    let mut builder = ResolvedStyleBuilder::new();
    for property in CascadePropertyId::ALL {
        if skip.contains(&property) {
            continue;
        }
        builder.record_initial(property);
    }
    builder
}

pub(super) fn parsed_value(declaration: &str) -> CascadeSpecifiedValue {
    let declaration = test_declaration(declaration);
    if declaration.name.kind == PropertyNameKind::Standard
        && let Some(property_name) = declaration.name.text.as_deref()
        && let Some(property) = property_registry().lookup_id(property_name)
    {
        return CascadeSpecifiedValue::parse(property, &declaration.value).unwrap_or_else(
            |error| panic!("failed to parse test declaration {declaration:?}: {error}"),
        );
    }

    CascadeSpecifiedValue::preserved(&declaration.value)
}

pub(super) fn preserved_value(declaration: &str) -> CascadeSpecifiedValue {
    let declaration = test_declaration(declaration);
    CascadeSpecifiedValue::preserved(&declaration.value)
}

pub(super) fn parse_error(
    property: CascadePropertyId,
    declaration: &str,
) -> SpecifiedValueParseError {
    let declaration = test_declaration(declaration);
    CascadeSpecifiedValue::parse(property, &declaration.value)
        .expect_err("test declaration must be invalid for property")
}

fn test_declaration(declaration: &str) -> crate::Declaration {
    let parse = parse_stylesheet_with_options(
        &format!("div {{ {declaration}; }}"),
        &ParseOptions::stylesheet(),
    );
    let Rule::Style(rule) = &parse.stylesheet.rules[0] else {
        panic!("expected style rule");
    };

    rule.declarations.declarations[0].clone()
}

pub(super) fn stylesheet_declaration_source(
    stylesheet_index: u32,
    rule_index: u32,
    declaration_index: u32,
) -> CascadeDeclarationSource {
    CascadeDeclarationSource::Stylesheet(StylesheetDeclarationRef {
        stylesheet_index,
        rule_index,
        declaration_index,
    })
}

pub(super) fn inline_declaration_source(
    inline_style: InlineStyleRuleRef,
    declaration_index: u32,
) -> CascadeDeclarationSource {
    CascadeDeclarationSource::InlineStyle(InlineStyleDeclarationRef {
        inline_style,
        declaration_index,
    })
}
