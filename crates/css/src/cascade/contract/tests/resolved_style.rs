use super::super::{
    CascadeDeclarationInput, CascadeDeclarationSource, CascadeImportance, CascadeOrigin,
    CascadePriority, CascadePropertyId, CascadeRuleContext, CascadeRuleInput, CascadeRuleSource,
    CascadeWinner, CascadeWinnerSet, CssWideResolvedSource, InheritanceParentPresence,
    InitialStyleValue, InlineStyleDeclarationRef, InlineStyleRuleRef, ResolvedStyleBuilder,
    ResolvedValueSource, StylesheetDeclarationRef, resolve_cascade_style,
    resolve_cascade_style_from_rule_inputs, resolve_cascade_style_owned, resolve_initial_style,
};
use super::support::{
    builder_with_initials_except, matched_rule, parsed_value, resolve_rule_inputs,
    stylesheet_declaration_source,
};
use crate::selectors::Specificity;

fn style_priority(rule_order: u32, declaration_order: u32) -> CascadePriority {
    CascadePriority::from_rule_context(
        CascadeRuleContext::for_stylesheet(
            CascadeOrigin::Author,
            Specificity::C,
            rule_order.into(),
        ),
        CascadeImportance::Normal,
        declaration_order,
    )
}

fn winners_for_rule(
    rule_index: u32,
    specificity: Specificity,
    declarations: Vec<CascadeDeclarationInput>,
) -> CascadeWinnerSet {
    let rule_ref = super::super::StylesheetRuleRef::new(
        crate::cascade::StylesheetSourceId::compatibility_generation_index(0),
        crate::cascade::RawRuleIndex::new(rule_index),
    );
    let rule = CascadeRuleInput::new(
        CascadeRuleSource::Stylesheet(rule_ref),
        CascadeRuleContext::for_stylesheet(CascadeOrigin::Author, specificity, rule_index.into()),
        declarations,
    )
    .expect("test rule source owns declarations");
    resolve_rule_inputs(vec![rule]).expect("test winners resolve")
}

#[test]
fn resolve_cascade_style_marks_inherited_properties_only_when_parent_is_present() {
    let mut parent_builder = builder_with_initials_except(&[CascadePropertyId::Color]);
    parent_builder.record_winner(
        CascadePropertyId::Color,
        CascadeWinner {
            source: stylesheet_declaration_source(0, 0, 0),
            priority: style_priority(0, 0),
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
            "version: 3\n",
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
            "  outline-color: initial(transparent)\n",
            "  outline-style: initial(none)\n",
            "  outline-width: initial(0px)\n",
            "  padding-bottom: initial(0px)\n",
            "  padding-left: initial(0px)\n",
            "  padding-right: initial(0px)\n",
            "  padding-top: initial(0px)\n",
            "  position: initial(static)\n",
            "  text-decoration-line: initial(none)\n",
            "  width: initial(auto)\n",
            "  z-index: initial(auto)\n",
        )
    );
}

#[test]
fn resolve_cascade_style_parent_dependency_is_presence_only() {
    let first_parent = resolve_initial_style();

    let mut second_parent_builder =
        builder_with_initials_except(&[CascadePropertyId::Color, CascadePropertyId::Display]);
    second_parent_builder.record_winner(
        CascadePropertyId::Color,
        CascadeWinner {
            source: stylesheet_declaration_source(0, 0, 0),
            priority: style_priority(0, 0),
            value: parsed_value("color: red"),
        },
    );
    second_parent_builder.record_winner(
        CascadePropertyId::Display,
        CascadeWinner {
            source: stylesheet_declaration_source(0, 0, 1),
            priority: style_priority(0, 1),
            value: parsed_value("display: block"),
        },
    );
    let second_parent = second_parent_builder.build().expect("total parent style");
    assert_ne!(
        first_parent, second_parent,
        "parents must be materially different"
    );

    let winners = winners_for_rule(
        1,
        Specificity::C,
        vec![CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 1, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Width,
            parsed_value("width: 40px"),
        )],
    );

    let with_first_parent = resolve_cascade_style(&winners, Some(&first_parent));
    let with_second_parent = resolve_cascade_style(&winners, Some(&second_parent));
    let without_parent = resolve_cascade_style(&winners, None);

    assert_eq!(with_first_parent, with_second_parent);
    assert_ne!(with_first_parent, without_parent);
    assert_eq!(
        with_first_parent
            .get(CascadePropertyId::Color)
            .expect("color")
            .source(),
        &ResolvedValueSource::Inherited
    );
    assert_eq!(
        without_parent
            .get(CascadePropertyId::Color)
            .expect("color")
            .source(),
        &ResolvedValueSource::Initial(InitialStyleValue::ColorBlack)
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
            "version: 3\n",
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
            "  outline-color: initial(transparent)\n",
            "  outline-style: initial(none)\n",
            "  outline-width: initial(0px)\n",
            "  padding-bottom: initial(0px)\n",
            "  padding-left: initial(0px)\n",
            "  padding-right: initial(0px)\n",
            "  padding-top: initial(0px)\n",
            "  position: initial(static)\n",
            "  text-decoration-line: initial(none)\n",
            "  width: initial(auto)\n",
            "  z-index: initial(auto)\n",
        )
    );
}

#[test]
fn resolved_style_resolves_every_registered_property_exactly_once() {
    let winners = winners_for_rule(
        0,
        Specificity::C,
        vec![CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Width,
            parsed_value("width: 40px"),
        )],
    );
    let style = resolve_cascade_style(&winners, Some(&resolve_initial_style()));
    let expected = crate::property_registry().ids().collect::<Vec<_>>();
    let actual = style
        .entries()
        .iter()
        .map(|entry| entry.property())
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
    for property in expected {
        assert!(style.get(property).is_some(), "{}", property.name());
    }
}

#[test]
fn resolve_cascade_style_defaults_missing_properties_to_the_initial_contract() {
    let winners = winners_for_rule(
        0,
        Specificity::C,
        vec![CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Width,
            parsed_value("width: 40px"),
        )],
    );

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
fn resolve_cascade_style_resolves_explicit_css_wide_keywords_after_winner_selection() {
    let mut parent_builder =
        builder_with_initials_except(&[CascadePropertyId::Color, CascadePropertyId::Display]);
    parent_builder.record_winner(
        CascadePropertyId::Color,
        CascadeWinner {
            source: stylesheet_declaration_source(0, 0, 0),
            priority: style_priority(0, 0),
            value: parsed_value("color: red"),
        },
    );
    parent_builder.record_winner(
        CascadePropertyId::Display,
        CascadeWinner {
            source: stylesheet_declaration_source(0, 0, 1),
            priority: style_priority(0, 1),
            value: parsed_value("display: block"),
        },
    );
    let parent_style = parent_builder.build().expect("total parent style");
    let child_winners = winners_for_rule(
        1,
        Specificity::C,
        vec![
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 1, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: unset"),
            ),
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 1, 1),
                1,
                CascadeImportance::Normal,
                CascadePropertyId::Display,
                parsed_value("display: inherit"),
            ),
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 1, 2),
                2,
                CascadeImportance::Normal,
                CascadePropertyId::Width,
                parsed_value("width: unset"),
            ),
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 1, 3),
                3,
                CascadeImportance::Normal,
                CascadePropertyId::FontSize,
                parsed_value("font-size: inherit"),
            ),
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 1, 4),
                4,
                CascadeImportance::Normal,
                CascadePropertyId::BackgroundColor,
                parsed_value("background-color: initial"),
            ),
        ],
    );

    let child = resolve_cascade_style(&child_winners, Some(&parent_style));

    let ResolvedValueSource::CssWideKeyword(CssWideResolvedSource::Inherited {
        keyword: color_keyword,
        ..
    }) = child.get(CascadePropertyId::Color).expect("color").source()
    else {
        panic!("color unset should resolve to explicit CSS-wide inheritance");
    };
    assert_eq!(color_keyword.as_css_keyword(), "unset");

    let ResolvedValueSource::CssWideKeyword(CssWideResolvedSource::Inherited {
        keyword: display_keyword,
        ..
    }) = child
        .get(CascadePropertyId::Display)
        .expect("display")
        .source()
    else {
        panic!("display inherit should resolve to explicit CSS-wide inheritance");
    };
    assert_eq!(display_keyword.as_css_keyword(), "inherit");

    let ResolvedValueSource::CssWideKeyword(CssWideResolvedSource::Initial {
        keyword: width_keyword,
        initial,
        ..
    }) = child.get(CascadePropertyId::Width).expect("width").source()
    else {
        panic!("width unset should resolve to explicit CSS-wide initial");
    };
    assert_eq!(width_keyword.as_css_keyword(), "unset");
    assert_eq!(*initial, InitialStyleValue::AutoKeyword);
    assert!(
        child
            .get(CascadePropertyId::Width)
            .expect("width")
            .winner()
            .is_none(),
        "explicit CSS-wide source must not look like an ordinary authored winner"
    );

    let ResolvedValueSource::CssWideKeyword(CssWideResolvedSource::Inherited {
        keyword: font_size_keyword,
        ..
    }) = child
        .get(CascadePropertyId::FontSize)
        .expect("font-size")
        .source()
    else {
        panic!("font-size inherit should resolve to explicit CSS-wide inheritance");
    };
    assert_eq!(*font_size_keyword, crate::CssWideKeyword::Inherit);

    let ResolvedValueSource::CssWideKeyword(CssWideResolvedSource::Initial {
        keyword: background_keyword,
        initial: background_initial,
        ..
    }) = child
        .get(CascadePropertyId::BackgroundColor)
        .expect("background-color")
        .source()
    else {
        panic!("background-color initial should resolve to explicit CSS-wide initial");
    };
    assert_eq!(*background_keyword, crate::CssWideKeyword::Initial);
    assert_eq!(*background_initial, InitialStyleValue::TransparentColor);

    let snapshot = child.to_debug_snapshot();
    assert!(snapshot.contains("color: css-wide-inherited(keyword=unset, winner("));
    assert!(snapshot.contains("display: css-wide-inherited(keyword=inherit, winner("));
    assert!(snapshot.contains("width: css-wide-initial(keyword=unset, winner("));
    assert!(snapshot.contains("font-size: css-wide-inherited(keyword=inherit, winner("));
    assert!(snapshot.contains("background-color: css-wide-initial(keyword=initial, winner("));
}

#[test]
fn borrowed_and_owned_resolution_paths_are_identical() {
    let winners = winners_for_rule(
        0,
        Specificity::C,
        vec![
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 0, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: unset"),
            ),
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 0, 1),
                1,
                CascadeImportance::Normal,
                CascadePropertyId::Width,
                parsed_value("width: 40px"),
            ),
        ],
    );
    let parent = resolve_initial_style();

    let borrowed = resolve_cascade_style(&winners, Some(&parent));
    let owned = resolve_cascade_style_owned(winners, InheritanceParentPresence::Present);

    assert_eq!(borrowed, owned);
}

#[test]
fn resolve_cascade_style_resolves_root_css_wide_inherit_and_unset_to_initial() {
    let root_winners = winners_for_rule(
        0,
        Specificity::C,
        vec![
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 0, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: inherit"),
            ),
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 0, 1),
                1,
                CascadeImportance::Normal,
                CascadePropertyId::FontSize,
                parsed_value("font-size: unset"),
            ),
        ],
    );

    let root = resolve_cascade_style(&root_winners, None);

    let ResolvedValueSource::CssWideKeyword(CssWideResolvedSource::Initial {
        keyword: color_keyword,
        initial: color_initial,
        ..
    }) = root.get(CascadePropertyId::Color).expect("color").source()
    else {
        panic!("root color inherit should resolve to explicit CSS-wide initial");
    };
    assert_eq!(color_keyword.as_css_keyword(), "inherit");
    assert_eq!(*color_initial, InitialStyleValue::ColorBlack);

    let ResolvedValueSource::CssWideKeyword(CssWideResolvedSource::Initial {
        keyword: font_keyword,
        initial: font_initial,
        ..
    }) = root
        .get(CascadePropertyId::FontSize)
        .expect("font-size")
        .source()
    else {
        panic!("root font-size unset should resolve to explicit CSS-wide initial");
    };
    assert_eq!(font_keyword.as_css_keyword(), "unset");
    assert_eq!(*font_initial, InitialStyleValue::FontSizePx16);
}

#[test]
fn resolve_cascade_style_explicit_winner_overrides_parent_inheritance_and_defaults() {
    let mut parent_builder =
        builder_with_initials_except(&[CascadePropertyId::Color, CascadePropertyId::Display]);
    parent_builder.record_winner(
        CascadePropertyId::Color,
        CascadeWinner {
            source: stylesheet_declaration_source(0, 0, 0),
            priority: style_priority(0, 0),
            value: parsed_value("color: red"),
        },
    );
    parent_builder.record_winner(
        CascadePropertyId::Display,
        CascadeWinner {
            source: stylesheet_declaration_source(0, 0, 1),
            priority: style_priority(0, 1),
            value: parsed_value("display: block"),
        },
    );
    let parent_style = parent_builder.build().expect("total parent style");

    let child_winners = winners_for_rule(
        1,
        Specificity::B,
        vec![CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 1, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: blue"),
        )],
    );

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
        &matched_rule(0, 0, &[Specificity::B]),
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
            source: CascadeDeclarationSource::Stylesheet(StylesheetDeclarationRef::new(
                crate::cascade::StylesheetSourceId::compatibility_generation_index(0),
                crate::cascade::RawRuleIndex::new(0),
                crate::cascade::DeclarationSourceIndex::new(1),
            )),
            priority: style_priority(0, 1),
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
            "version: 3\n",
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
            "  display: winner(source=stylesheet[2/0]/declaration[1], band=author-normal, attachment=style-rule, specificity=selector(0,0,1), source-order=stylesheet[0/0], declaration-order=1, value=\"block\")\n",
            "  font-size: initial(16px)\n",
            "  height: initial(auto)\n",
            "  margin-bottom: initial(0px)\n",
            "  margin-left: initial(0px)\n",
            "  margin-right: initial(0px)\n",
            "  margin-top: initial(0px)\n",
            "  max-width: initial(none)\n",
            "  min-width: initial(auto)\n",
            "  overflow: initial(visible)\n",
            "  outline-color: initial(transparent)\n",
            "  outline-style: initial(none)\n",
            "  outline-width: initial(0px)\n",
            "  padding-bottom: initial(0px)\n",
            "  padding-left: initial(0px)\n",
            "  padding-right: initial(0px)\n",
            "  padding-top: initial(0px)\n",
            "  position: initial(static)\n",
            "  text-decoration-line: initial(none)\n",
            "  width: initial(auto)\n",
            "  z-index: initial(auto)\n",
        )
    );
}

#[test]
fn resolved_style_snapshot_formats_inline_winners() {
    let mut builder = builder_with_initials_except(&[CascadePropertyId::Color]);
    builder.record_winner(
        CascadePropertyId::Color,
        CascadeWinner {
            source: CascadeDeclarationSource::InlineStyle(InlineStyleDeclarationRef::new(
                InlineStyleRuleRef::new(9),
                crate::cascade::DeclarationSourceIndex::new(2),
            )),
            priority: CascadePriority::from_rule_context(
                CascadeRuleContext::for_inline_style(),
                CascadeImportance::Normal,
                2,
            ),
            value: parsed_value("color: red"),
        },
    );

    let snapshot = builder.build().expect("total style").to_debug_snapshot();
    assert!(snapshot.contains(
        "winner(source=inline-style[compatibility=9]/declaration[2], band=author-normal, attachment=element-attached, specificity=not-applicable, source-order=not-applicable, declaration-order=2, value=\"red\")"
    ));
}
