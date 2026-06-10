use super::super::{
    CascadeDeclarationInput, CascadeDeclarationSource, CascadeImportance, CascadeOrigin,
    CascadeOriginBand, CascadePriority, CascadePropertyId, CascadeRuleContext, CascadeRuleInput,
    CascadeSpecificity, CascadeWinner, CascadeWinnerSet, InitialStyleValue,
    InlineStyleDeclarationRef, InlineStyleRuleRef, ResolvedStyleBuilder, ResolvedValueSource,
    StylesheetDeclarationRef, resolve_cascade_style, resolve_cascade_style_from_rule_inputs,
    resolve_cascade_winners, resolve_initial_style,
};
use super::support::{
    builder_with_initials_except, matched_rule, parsed_value, stylesheet_declaration_source,
};
use crate::selectors::Specificity;

#[test]
fn resolve_cascade_style_marks_inherited_properties_only_when_parent_is_present() {
    let mut parent_builder = builder_with_initials_except(&[CascadePropertyId::Color]);
    parent_builder.record_winner(
        CascadePropertyId::Color,
        CascadeWinner {
            source: stylesheet_declaration_source(0, 0, 0),
            priority: CascadePriority::new(
                CascadeOriginBand::AuthorNormal,
                CascadeSpecificity::Selector(Specificity::TYPE),
                0,
                0,
            ),
            value: parsed_value("color: red"),
        },
    );
    let parent_style = parent_builder.build().expect("total parent style");

    let child = resolve_cascade_style(&CascadeWinnerSet::default(), Some(&parent_style));

    assert_eq!(
        child.get(CascadePropertyId::Color).expect("color").source(),
        &ResolvedValueSource::Inherited
    );
    assert_eq!(
        child
            .get(CascadePropertyId::FontSize)
            .expect("font-size")
            .source(),
        &ResolvedValueSource::Inherited
    );
    assert_eq!(
        child
            .get(CascadePropertyId::Display)
            .expect("display")
            .source(),
        &ResolvedValueSource::Initial(InitialStyleValue::DisplayInline)
    );
    assert_eq!(
        child.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "resolved-style\n",
            "  background-color: initial(transparent)\n",
            "  border-bottom-color: initial(transparent)\n",
            "  border-bottom-style: initial(none)\n",
            "  border-bottom-width: initial(0px)\n",
            "  border-left-color: initial(transparent)\n",
            "  border-left-style: initial(none)\n",
            "  border-left-width: initial(0px)\n",
            "  border-right-color: initial(transparent)\n",
            "  border-right-style: initial(none)\n",
            "  border-right-width: initial(0px)\n",
            "  border-top-color: initial(transparent)\n",
            "  border-top-style: initial(none)\n",
            "  border-top-width: initial(0px)\n",
            "  color: inherited\n",
            "  display: initial(inline)\n",
            "  font-size: inherited\n",
            "  height: initial(auto)\n",
            "  margin-bottom: initial(0px)\n",
            "  margin-left: initial(0px)\n",
            "  margin-right: initial(0px)\n",
            "  margin-top: initial(0px)\n",
            "  max-width: initial(none)\n",
            "  min-width: initial(auto)\n",
            "  overflow: initial(visible)\n",
            "  padding-bottom: initial(0px)\n",
            "  padding-left: initial(0px)\n",
            "  padding-right: initial(0px)\n",
            "  padding-top: initial(0px)\n",
            "  position: initial(static)\n",
            "  width: initial(auto)\n",
        )
    );
}

#[test]
fn resolve_cascade_style_uses_initial_for_inherited_properties_at_the_root() {
    let root_style = resolve_cascade_style(&CascadeWinnerSet::default(), None);

    assert_eq!(root_style, resolve_initial_style());
    assert_eq!(
        root_style
            .get(CascadePropertyId::Color)
            .expect("color")
            .source(),
        &ResolvedValueSource::Initial(InitialStyleValue::ColorBlack)
    );
    assert_eq!(
        root_style
            .get(CascadePropertyId::FontSize)
            .expect("font-size")
            .source(),
        &ResolvedValueSource::Initial(InitialStyleValue::FontSizePx16)
    );
}

#[test]
fn resolve_initial_style_materializes_total_canonical_initial_style() {
    let initial_style = resolve_initial_style();

    assert_eq!(initial_style.entries().len(), CascadePropertyId::ALL.len());
    for entry in initial_style.entries() {
        assert_eq!(
            entry.source(),
            &ResolvedValueSource::Initial(entry.property().initial_value()),
            "{}",
            entry.property().name()
        );
    }
    assert_eq!(
        initial_style.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "resolved-style\n",
            "  background-color: initial(transparent)\n",
            "  border-bottom-color: initial(transparent)\n",
            "  border-bottom-style: initial(none)\n",
            "  border-bottom-width: initial(0px)\n",
            "  border-left-color: initial(transparent)\n",
            "  border-left-style: initial(none)\n",
            "  border-left-width: initial(0px)\n",
            "  border-right-color: initial(transparent)\n",
            "  border-right-style: initial(none)\n",
            "  border-right-width: initial(0px)\n",
            "  border-top-color: initial(transparent)\n",
            "  border-top-style: initial(none)\n",
            "  border-top-width: initial(0px)\n",
            "  color: initial(black)\n",
            "  display: initial(inline)\n",
            "  font-size: initial(16px)\n",
            "  height: initial(auto)\n",
            "  margin-bottom: initial(0px)\n",
            "  margin-left: initial(0px)\n",
            "  margin-right: initial(0px)\n",
            "  margin-top: initial(0px)\n",
            "  max-width: initial(none)\n",
            "  min-width: initial(auto)\n",
            "  overflow: initial(visible)\n",
            "  padding-bottom: initial(0px)\n",
            "  padding-left: initial(0px)\n",
            "  padding-right: initial(0px)\n",
            "  padding-top: initial(0px)\n",
            "  position: initial(static)\n",
            "  width: initial(auto)\n",
        )
    );
}

#[test]
fn resolve_cascade_style_defaults_missing_properties_to_the_initial_contract() {
    let winners = resolve_cascade_winners(&[CascadeDeclarationInput::supported(
        stylesheet_declaration_source(0, 0, 0),
        0,
        CascadeImportance::Normal,
        CascadePropertyId::Width,
        parsed_value("width: 40px"),
    )
    .candidate(CascadeRuleContext::new(
        CascadeOrigin::Author,
        CascadeSpecificity::Selector(Specificity::TYPE),
        0,
    ))
    .expect("supported candidate")]);

    let style = resolve_cascade_style(&winners, None);

    assert_eq!(
        style
            .get(CascadePropertyId::Width)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("40px")
    );
    assert_eq!(
        style
            .get(CascadePropertyId::BackgroundColor)
            .expect("background-color")
            .source(),
        &ResolvedValueSource::Initial(InitialStyleValue::TransparentColor)
    );
    assert_eq!(
        style.get(CascadePropertyId::Color).expect("color").source(),
        &ResolvedValueSource::Initial(InitialStyleValue::ColorBlack)
    );
    assert_eq!(
        style
            .get(CascadePropertyId::MaxWidth)
            .expect("max-width")
            .source(),
        &ResolvedValueSource::Initial(InitialStyleValue::NoneKeyword)
    );
}

#[test]
fn resolve_cascade_style_explicit_winner_overrides_parent_inheritance_and_defaults() {
    let mut parent_builder =
        builder_with_initials_except(&[CascadePropertyId::Color, CascadePropertyId::Display]);
    parent_builder.record_winner(
        CascadePropertyId::Color,
        CascadeWinner {
            source: stylesheet_declaration_source(0, 0, 0),
            priority: CascadePriority::new(
                CascadeOriginBand::AuthorNormal,
                CascadeSpecificity::Selector(Specificity::TYPE),
                0,
                0,
            ),
            value: parsed_value("color: red"),
        },
    );
    parent_builder.record_winner(
        CascadePropertyId::Display,
        CascadeWinner {
            source: stylesheet_declaration_source(0, 0, 1),
            priority: CascadePriority::new(
                CascadeOriginBand::AuthorNormal,
                CascadeSpecificity::Selector(Specificity::TYPE),
                0,
                1,
            ),
            value: parsed_value("display: block"),
        },
    );
    let parent_style = parent_builder.build().expect("total parent style");

    let child_winners = resolve_cascade_winners(&[CascadeDeclarationInput::supported(
        stylesheet_declaration_source(0, 1, 0),
        0,
        CascadeImportance::Normal,
        CascadePropertyId::Color,
        parsed_value("color: blue"),
    )
    .candidate(CascadeRuleContext::new(
        CascadeOrigin::Author,
        CascadeSpecificity::Selector(Specificity::CLASS),
        1,
    ))
    .expect("supported candidate")]);

    let child = resolve_cascade_style(&child_winners, Some(&parent_style));

    assert_eq!(
        child
            .get(CascadePropertyId::Color)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("blue")
    );
    assert_eq!(
        child
            .get(CascadePropertyId::FontSize)
            .expect("font-size")
            .source(),
        &ResolvedValueSource::Inherited
    );
    assert_eq!(
        child
            .get(CascadePropertyId::Display)
            .expect("display")
            .source(),
        &ResolvedValueSource::Initial(InitialStyleValue::DisplayInline)
    );
}

#[test]
fn resolve_cascade_style_from_rule_inputs_applies_inheritance_without_rederiving_priority() {
    let parent_style = resolve_cascade_style(&CascadeWinnerSet::default(), None);
    let child_rule = CascadeRuleInput::from_stylesheet_match(
        &matched_rule(0, 0, &[Specificity::CLASS]),
        CascadeOrigin::Author,
        0,
        vec![CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: blue"),
        )],
    )
    .expect("valid rule")
    .expect("matching rule");

    let child_style = resolve_cascade_style_from_rule_inputs(&[child_rule], Some(&parent_style));

    assert_eq!(
        child_style
            .get(CascadePropertyId::Color)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("blue")
    );
    assert_eq!(
        child_style
            .get(CascadePropertyId::FontSize)
            .expect("font-size")
            .source(),
        &ResolvedValueSource::Inherited
    );
    assert_eq!(
        child_style
            .get(CascadePropertyId::BackgroundColor)
            .expect("background-color")
            .source(),
        &ResolvedValueSource::Initial(InitialStyleValue::TransparentColor)
    );
}

#[test]
fn resolved_style_builder_rejects_missing_supported_properties() {
    let error = ResolvedStyleBuilder::new()
        .build()
        .expect_err("partial style");
    assert_eq!(
        error.missing_properties(),
        CascadePropertyId::ALL.as_slice()
    );
}

#[test]
#[should_panic(expected = "resolved style must not record the same property twice")]
fn resolved_style_builder_rejects_duplicate_property_insertion_in_all_builds() {
    let mut builder = ResolvedStyleBuilder::new();
    builder.record_initial(CascadePropertyId::Color);
    builder.record_initial(CascadePropertyId::Color);
}

#[test]
#[should_panic(expected = "only inherited properties may resolve through inheritance")]
fn resolved_style_builder_rejects_inherited_source_for_non_inherited_property_in_all_builds() {
    let mut builder = ResolvedStyleBuilder::new();
    builder.record_inherited(CascadePropertyId::Display);
}

#[test]
fn resolved_style_builder_record_initial_uses_property_initial_value_contract() {
    let mut builder = ResolvedStyleBuilder::new();
    for property in CascadePropertyId::ALL {
        builder.record_initial(property);
    }

    let style = builder.build().expect("total style");

    for property in CascadePropertyId::ALL {
        assert_eq!(
            style
                .get(property)
                .unwrap_or_else(|| panic!("{}", property.name()))
                .source(),
            &ResolvedValueSource::Initial(property.initial_value()),
            "{}",
            property.name()
        );
    }
}

#[test]
fn resolved_style_builder_is_deterministic_and_property_sorted() {
    let mut builder =
        builder_with_initials_except(&[CascadePropertyId::Color, CascadePropertyId::Display]);
    builder.record_winner(
        CascadePropertyId::Display,
        CascadeWinner {
            source: CascadeDeclarationSource::Stylesheet(StylesheetDeclarationRef {
                stylesheet_index: 0,
                rule_index: 0,
                declaration_index: 1,
            }),
            priority: CascadePriority::new(
                CascadeOriginBand::AuthorNormal,
                CascadeSpecificity::Selector(Specificity::TYPE),
                0,
                1,
            ),
            value: parsed_value("display: block"),
        },
    );
    builder.record_inherited(CascadePropertyId::Color);

    let style = builder.build().expect("total style");

    assert_eq!(
        style.entries()[0].property(),
        CascadePropertyId::BackgroundColor
    );
    assert_eq!(
        style.entries()[1].property(),
        CascadePropertyId::BorderBottomColor
    );
    assert_eq!(style.entries()[13].property(), CascadePropertyId::Color);
    assert_eq!(style.entries()[14].property(), CascadePropertyId::Display);
    assert_eq!(
        style.get(CascadePropertyId::Width).expect("width").source(),
        &ResolvedValueSource::Initial(InitialStyleValue::AutoKeyword)
    );
    assert_eq!(
        style.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "resolved-style\n",
            "  background-color: initial(transparent)\n",
            "  border-bottom-color: initial(transparent)\n",
            "  border-bottom-style: initial(none)\n",
            "  border-bottom-width: initial(0px)\n",
            "  border-left-color: initial(transparent)\n",
            "  border-left-style: initial(none)\n",
            "  border-left-width: initial(0px)\n",
            "  border-right-color: initial(transparent)\n",
            "  border-right-style: initial(none)\n",
            "  border-right-width: initial(0px)\n",
            "  border-top-color: initial(transparent)\n",
            "  border-top-style: initial(none)\n",
            "  border-top-width: initial(0px)\n",
            "  color: inherited\n",
            "  display: winner(source=stylesheet[0/0]/declaration[1], band=author-normal, specificity=selector(0,0,1), rule-order=0, declaration-order=1, value=\"block\")\n",
            "  font-size: initial(16px)\n",
            "  height: initial(auto)\n",
            "  margin-bottom: initial(0px)\n",
            "  margin-left: initial(0px)\n",
            "  margin-right: initial(0px)\n",
            "  margin-top: initial(0px)\n",
            "  max-width: initial(none)\n",
            "  min-width: initial(auto)\n",
            "  overflow: initial(visible)\n",
            "  padding-bottom: initial(0px)\n",
            "  padding-left: initial(0px)\n",
            "  padding-right: initial(0px)\n",
            "  padding-top: initial(0px)\n",
            "  position: initial(static)\n",
            "  width: initial(auto)\n",
        )
    );
}

#[test]
fn resolved_style_snapshot_formats_inline_winners() {
    let mut builder = builder_with_initials_except(&[CascadePropertyId::Color]);
    builder.record_winner(
        CascadePropertyId::Color,
        CascadeWinner {
            source: CascadeDeclarationSource::InlineStyle(InlineStyleDeclarationRef {
                inline_style: InlineStyleRuleRef::new(9),
                declaration_index: 2,
            }),
            priority: CascadePriority::new(
                CascadeOriginBand::AuthorNormal,
                CascadeSpecificity::InlineStyle,
                0,
                2,
            ),
            value: parsed_value("color: red"),
        },
    );

    let snapshot = builder.build().expect("total style").to_debug_snapshot();
    assert!(snapshot.contains(
        "winner(source=inline-style[9]/declaration[2], band=author-normal, specificity=inline-style, rule-order=0, declaration-order=2, value=\"red\")"
    ));
}
