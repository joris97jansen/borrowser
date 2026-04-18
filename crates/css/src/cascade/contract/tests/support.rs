use super::super::{
    CascadeDeclarationSource, CascadePropertyId, CascadeRuleMatch, CascadeSpecifiedValue,
    InlineStyleDeclarationRef, InlineStyleRuleRef, ResolvedStyleBuilder, StylesheetDeclarationRef,
};
use crate::selectors::{SelectorListMatchOutcome, Specificity};
use crate::{ParseOptions, Rule, parse_stylesheet_with_options};

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
    let parse = parse_stylesheet_with_options(
        &format!("div {{ {declaration}; }}"),
        &ParseOptions::stylesheet(),
    );
    let Rule::Style(rule) = &parse.stylesheet.rules[0] else {
        panic!("expected style rule");
    };
    CascadeSpecifiedValue::from_declaration_value(&rule.declarations.declarations[0].value)
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
