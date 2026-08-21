use super::super::integration::{
    CollectedRule, InactiveStyleRuleReason, StylesheetConditionStatus,
    declaration_classification_count, reset_declaration_classification_count,
};
use super::support::{document_element, element, matching_environment, stylesheet};
use crate::{
    AtRuleSkipReason, CascadeDeclarationApplicability, CascadeDeclarationSource, CascadeImportance,
    CascadeOrigin, CascadePropertyId, DiagnosticCondition, DiagnosticDeclarationProperty,
    RuleCollection, RuleCollectionBuildError, RuleCollectionDiagnosticFailure,
    RuleCollectionDiagnosticLimit, RuleCollectionDiagnosticLimits, RuleCollectionDiagnosticRecord,
    StyleResolutionError, StyleResolutionLimit, StyleResolutionLimits, StylesheetCollectionInput,
    StylesheetConditionInput, StylesheetOrder, StylesheetSourceId, rule_collection_diagnostic,
    try_resolve_document_styles_from_rule_collection_with_limits,
};

fn author_input<'a>(
    id: u32,
    order: u32,
    sheet: &'a crate::StylesheetParse,
) -> StylesheetCollectionInput<'a> {
    StylesheetCollectionInput::author(
        StylesheetSourceId::in_memory_generation_index(id),
        StylesheetOrder::new(order),
        sheet,
        StylesheetConditionInput::None,
    )
}

#[test]
fn collection_keeps_raw_and_style_positions_distinct_and_flattens_declarations() {
    let sheet = stylesheet(concat!(
        "div { color: red; outline: blue solid 2px; }",
        "@media screen { div { color: green; } }",
        ":hover { color: black; }",
        "> p { color: black; }",
        "div { color: blue !important; malformed; width: 10px; }",
    ));
    let collection = RuleCollection::try_new(
        &[author_input(1, 7, &sheet)],
        &StyleResolutionLimits::default(),
    )
    .expect("collection builds");

    assert_eq!(collection.stylesheets()[0].order().get(), 7);
    assert_eq!(collection.rules().len(), 5);
    let CollectedRule::ActiveStyle(first) = &collection.rules()[0] else {
        panic!()
    };
    assert_eq!(first.rule_ref().raw_rule_index().get(), 0);
    assert_eq!(first.style_position().get(), 0);
    assert_eq!(first.source_order().stylesheet().get(), 7);
    assert_eq!(collection.declarations_for_rule(first).len(), 4);

    let CollectedRule::SkippedAtRule(at_rule) = &collection.rules()[1] else {
        panic!()
    };
    assert_eq!(at_rule.rule_ref().raw_rule_index().get(), 1);
    assert_eq!(at_rule.reason(), AtRuleSkipReason::MediaDeferred);

    let CollectedRule::InactiveStyle(unsupported) = &collection.rules()[2] else {
        panic!()
    };
    assert_eq!(unsupported.style_position().get(), 1);
    assert!(matches!(
        unsupported.reason(),
        InactiveStyleRuleReason::UnsupportedSelector { .. }
    ));
    let CollectedRule::InactiveStyle(invalid) = &collection.rules()[3] else {
        panic!()
    };
    assert_eq!(invalid.style_position().get(), 2);
    assert!(matches!(
        invalid.reason(),
        InactiveStyleRuleReason::InvalidSelector { .. }
    ));

    let CollectedRule::ActiveStyle(last) = &collection.rules()[4] else {
        panic!()
    };
    assert_eq!(last.rule_ref().raw_rule_index().get(), 4);
    assert_eq!(last.style_position().get(), 3);
    let declarations = collection.declarations_for_rule(last);
    assert_eq!(
        declarations.len(),
        2,
        "parser-discarded malformed syntax is not fabricated"
    );
    assert_eq!(declarations[0].importance(), CascadeImportance::Important);
    assert_eq!(declarations[0].declaration_order().get(), 0);
    assert_eq!(declarations[1].declaration_order().get(), 1);
}

#[test]
fn equal_specificity_uses_explicit_rule_then_sparse_stylesheet_order() {
    let first = stylesheet("div { color: red; } div { color: green; }");
    let second = stylesheet("div { color: blue; }");
    let inputs = [author_input(1, 2, &first), author_input(2, 9, &second)];
    let collection = RuleCollection::try_new(&inputs, &StyleResolutionLimits::default()).unwrap();
    let orders = collection
        .rules()
        .iter()
        .map(|rule| match rule {
            CollectedRule::ActiveStyle(rule) => (
                rule.source_order().stylesheet().get(),
                rule.source_order().rule().get(),
            ),
            _ => panic!("all test rules are active"),
        })
        .collect::<Vec<_>>();
    assert_eq!(orders, vec![(2, 0), (2, 1), (9, 0)]);

    let resolved = try_resolve_document_styles_from_rule_collection_with_limits(
        &document_element("div", Vec::new(), Vec::new()),
        matching_environment(),
        &collection,
        &StyleResolutionLimits::default(),
    )
    .unwrap();
    let winner = resolved.entries()[0]
        .style()
        .get(CascadePropertyId::Color)
        .and_then(|entry| entry.winner())
        .expect("equal-specificity color winner");
    let CascadeDeclarationSource::Stylesheet(source) = winner.source else {
        panic!("winner must be a stylesheet declaration")
    };
    assert_eq!(source.source_id(), inputs[1].source_id());
    assert_eq!(
        winner.priority.source_order(),
        crate::CascadeSourceOrder::Stylesheet(crate::StylesheetRuleOrder::new(
            StylesheetOrder::new(9),
            crate::StyleRulePosition::new(0)
        ))
    );
}

#[test]
fn inactive_condition_classifies_no_declarations_and_retains_bounded_diagnostic_text() {
    let sheet = stylesheet("div { color: red; outline: blue solid 2px; }");
    let input = StylesheetCollectionInput::author(
        StylesheetSourceId::in_memory_generation_index(4),
        StylesheetOrder::new(3),
        &sheet,
        StylesheetConditionInput::RawMedia(" screen "),
    );
    let collection = RuleCollection::try_new(&[input], &StyleResolutionLimits::default())
        .expect("unsupported media is a normal inactive collection state");
    assert!(collection.declarations().is_empty());
    assert!(matches!(
        collection.rules()[0],
        CollectedRule::InactiveStyle(_)
    ));

    let diagnostic = rule_collection_diagnostic(
        &document_element("div", Vec::new(), Vec::new()),
        matching_environment(),
        &[input],
        &StyleResolutionLimits::default(),
        RuleCollectionDiagnosticLimits {
            max_condition_text_bytes: 3,
            ..Default::default()
        },
    );
    let Some(RuleCollectionDiagnosticRecord::Stylesheet { condition, .. }) =
        diagnostic.records().first()
    else {
        panic!()
    };
    let DiagnosticCondition::DeferredUnsupported(text) = condition else {
        panic!()
    };
    assert_eq!(text.original_bytes, 8);
    assert!(text.truncated);
}

#[test]
fn invalid_and_unsupported_selector_rules_never_classify_declarations() {
    let sheet = stylesheet(concat!(
        ":hover { outline: red solid 2px; }",
        "> div { color: red; width: 2px; }",
    ));
    reset_declaration_classification_count();
    let collection = RuleCollection::try_new(
        &[author_input(14, 0, &sheet)],
        &StyleResolutionLimits::default(),
    )
    .unwrap();
    assert!(collection.declarations().is_empty());
    assert_eq!(declaration_classification_count(), 0);
    assert!(
        collection
            .rules()
            .iter()
            .all(|rule| matches!(rule, CollectedRule::InactiveStyle(_)))
    );
}

#[test]
fn collection_rejects_duplicate_identity_and_non_monotonic_order_but_accepts_sparse_order() {
    let first = stylesheet("div { color: red; }");
    let second = stylesheet("div { color: blue; }");
    let duplicate = [author_input(1, 0, &first), author_input(1, 2, &second)];
    assert!(matches!(
        RuleCollection::try_new(&duplicate, &StyleResolutionLimits::default()),
        Err(RuleCollectionBuildError::DuplicateSourceId { .. })
    ));
    let non_monotonic = [author_input(1, 3, &first), author_input(2, 2, &second)];
    assert!(matches!(
        RuleCollection::try_new(&non_monotonic, &StyleResolutionLimits::default()),
        Err(RuleCollectionBuildError::NonMonotonicStylesheetOrder { .. })
    ));
    let duplicate_order = [author_input(1, 3, &first), author_input(2, 3, &second)];
    assert!(matches!(
        RuleCollection::try_new(&duplicate_order, &StyleResolutionLimits::default()),
        Err(RuleCollectionBuildError::DuplicateStylesheetOrder { .. })
    ));
    let sparse = [author_input(1, 0, &first), author_input(2, 9, &second)];
    let collection = RuleCollection::try_new(&sparse, &StyleResolutionLimits::default()).unwrap();
    assert_eq!(collection.stylesheets()[1].order().get(), 9);
}

#[test]
fn source_id_domains_and_compact_coordinates_are_checked_without_aliasing() {
    let ua = StylesheetSourceId::built_in_user_agent();
    let browser = StylesheetSourceId::from_browser_slot(0).unwrap();
    let in_memory = StylesheetSourceId::in_memory_generation_index(0);
    let compatibility = StylesheetSourceId::compatibility_generation_index(0);
    assert_ne!(ua, browser);
    assert_ne!(browser, in_memory);
    assert_ne!(in_memory, compatibility);
    assert!(StylesheetSourceId::from_browser_slot(u64::MAX).is_err());

    if usize::BITS > u32::BITS {
        let unrepresentable = (u32::MAX as usize).checked_add(1).unwrap();
        assert!(StylesheetOrder::from_usize(unrepresentable).is_err());
        assert!(crate::RawRuleIndex::from_usize(unrepresentable).is_err());
        assert!(crate::StyleRulePosition::from_usize(unrepresentable).is_err());
        assert!(crate::DeclarationSourceIndex::from_usize(unrepresentable).is_err());
        assert!(crate::DeclarationOrder::from_usize(unrepresentable).is_err());
    }
}

#[test]
fn collection_limits_count_at_rules_and_post_expansion_declarations() {
    let rules = stylesheet("@unknown; div { color: red; }");
    let rule_limits = StyleResolutionLimits {
        max_top_level_rules_per_document: 1,
        ..Default::default()
    };
    assert!(matches!(
        RuleCollection::try_new(&[author_input(1, 0, &rules)], &rule_limits),
        Err(RuleCollectionBuildError::LimitExceeded {
            limit: StyleResolutionLimit::TopLevelRulesPerDocument,
            observed: 2,
            ..
        })
    ));

    let shorthand = stylesheet("div { outline: red solid 2px; }");
    let declaration_limits = StyleResolutionLimits {
        max_collected_declaration_inputs_per_document: 2,
        ..Default::default()
    };
    assert!(matches!(
        RuleCollection::try_new(&[author_input(1, 0, &shorthand)], &declaration_limits),
        Err(RuleCollectionBuildError::LimitExceeded {
            limit: StyleResolutionLimit::CollectedDeclarationInputsPerDocument,
            observed: 3,
            ..
        })
    ));
}

#[test]
fn matched_declaration_budget_failure_is_an_element_limit_not_a_collection_failure() {
    let sheet = stylesheet("div { color: red; } div { width: 10px; }");
    let collection = RuleCollection::try_new(
        &[author_input(1, 0, &sheet)],
        &StyleResolutionLimits::default(),
    )
    .expect("both declaration inputs are collectable");
    let limits = StyleResolutionLimits {
        max_declaration_inputs_per_element: 1,
        ..Default::default()
    };
    let error = try_resolve_document_styles_from_rule_collection_with_limits(
        &document_element("div", Vec::new(), Vec::new()),
        matching_environment(),
        &collection,
        &limits,
    )
    .expect_err("the second matched declaration exceeds the per-element budget");
    assert_eq!(
        error,
        StyleResolutionError::LimitExceeded {
            limit: StyleResolutionLimit::DeclarationInputsPerElement,
            configured: 1,
        }
    );
}

#[test]
fn all_internal_at_rules_skip_without_flattening_and_whitespace_media_is_active() {
    let sheet = stylesheet(concat!(
        "@media screen { div { color: red; } }",
        "@supports (display: block) { div { color: red; } }",
        "@import url(other.css);",
        "@future token { div { color: red; } }",
        "div { color: blue; }",
    ));
    let input = StylesheetCollectionInput::author(
        StylesheetSourceId::in_memory_generation_index(12),
        StylesheetOrder::new(4),
        &sheet,
        StylesheetConditionInput::RawMedia(" \t\n"),
    );
    let collection = RuleCollection::try_new(&[input], &StyleResolutionLimits::default()).unwrap();
    assert!(matches!(
        collection.stylesheets()[0].condition(),
        StylesheetConditionStatus::Active
    ));
    let reasons = collection.rules()[..4]
        .iter()
        .map(|rule| match rule {
            CollectedRule::SkippedAtRule(rule) => rule.reason(),
            _ => panic!("conditional contents must not be flattened"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reasons,
        vec![
            AtRuleSkipReason::MediaDeferred,
            AtRuleSkipReason::SupportsDeferred,
            AtRuleSkipReason::ImportDeferred,
            AtRuleSkipReason::Unknown,
        ]
    );
    assert!(matches!(
        collection.rules()[4],
        CollectedRule::ActiveStyle(_)
    ));
}

#[test]
fn af5_diagnostic_uses_exact_match_outcome_and_is_bounded_before_winners() {
    let sheet = stylesheet("#missing, div { color: red; } div { display: grid; }");
    let dom = document_element(
        "div",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );
    let input = author_input(2, 5, &sheet);
    let diagnostic = rule_collection_diagnostic(
        &dom,
        matching_environment(),
        &[input],
        &StyleResolutionLimits::default(),
        RuleCollectionDiagnosticLimits::default(),
    );
    let matches = diagnostic
        .records()
        .iter()
        .filter_map(|record| match record {
            RuleCollectionDiagnosticRecord::Match {
                outcome,
                effective_specificity,
                ..
            } => Some((outcome, effective_specificity)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        4,
        "two active rules are observed for two elements"
    );
    assert_eq!(matches[0].0.matched_selectors().len(), 1);
    assert_eq!(*matches[0].1, matches[0].0.highest_specificity());
    assert!(
        diagnostic
            .to_debug_snapshot()
            .starts_with("version: 1\naf5-rule-collection\nstatus: complete\n")
    );
    let repeated = rule_collection_diagnostic(
        &dom,
        matching_environment(),
        &[input],
        &StyleResolutionLimits::default(),
        RuleCollectionDiagnosticLimits::default(),
    );
    assert_eq!(diagnostic, repeated);
    assert_eq!(diagnostic.to_debug_snapshot(), repeated.to_debug_snapshot());

    let bounded = rule_collection_diagnostic(
        &dom,
        matching_environment(),
        &[input],
        &StyleResolutionLimits::default(),
        RuleCollectionDiagnosticLimits {
            max_records: 1,
            ..Default::default()
        },
    );
    assert!(bounded.failure().is_some());

    let serialized_bounded = rule_collection_diagnostic(
        &dom,
        matching_environment(),
        &[input],
        &StyleResolutionLimits::default(),
        RuleCollectionDiagnosticLimits {
            max_serialized_bytes: 1,
            ..Default::default()
        },
    );
    assert!(matches!(
        serialized_bounded.failure(),
        Some(crate::RuleCollectionDiagnosticFailure::LimitExceeded {
            limit: crate::RuleCollectionDiagnosticLimit::SerializedBytes,
            ..
        })
    ));
}

#[test]
fn af5_diagnostic_reports_inline_importance_and_declaration_coordinates() {
    let dom = document_element(
        "div",
        vec![("style", Some("color: red; color: blue !important"))],
        Vec::new(),
    );
    let diagnostic = rule_collection_diagnostic(
        &dom,
        matching_environment(),
        &[],
        &StyleResolutionLimits::default(),
        RuleCollectionDiagnosticLimits::default(),
    );
    let inline = diagnostic
        .records()
        .iter()
        .filter_map(|record| match record {
            RuleCollectionDiagnosticRecord::InlineDeclaration {
                declaration_source_index,
                declaration_order,
                importance,
                ..
            } => Some((
                declaration_source_index.get(),
                declaration_order.get(),
                *importance,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        inline,
        vec![
            (0, 0, CascadeImportance::Normal),
            (1, 1, CascadeImportance::Important),
        ]
    );
}

#[test]
fn af5_declaration_diagnostic_exposes_classification_values_expansion_and_invalidity() {
    let sheet = stylesheet(concat!(
        "div {",
        "color: red;",
        "display: grid;",
        "future-property-long: preserved-value-long;",
        "--custom-property-long: custom-value-long;",
        "outline: red wavy 2px;",
        "outline: blue solid 2px !important;",
        "}",
    ));
    let input = author_input(6, 4, &sheet);
    let diagnostic = rule_collection_diagnostic(
        &document_element("div", Vec::new(), Vec::new()),
        matching_environment(),
        &[input],
        &StyleResolutionLimits::default(),
        RuleCollectionDiagnosticLimits::default(),
    );
    let declarations = diagnostic
        .records()
        .iter()
        .filter(|record| matches!(record, RuleCollectionDiagnosticRecord::Declaration { .. }))
        .collect::<Vec<_>>();
    assert_eq!(declarations.len(), 8);
    let snapshot = diagnostic.to_debug_snapshot();
    assert!(
        snapshot.contains(
            "property=supported(color) value=\"red\" applicability=supported invalid=none"
        )
    );
    assert!(snapshot.contains(
        "property=invalid-value(display) value=\"grid\" applicability=invalid-value invalid=unsupported-display-keyword"
    ));
    assert!(snapshot.contains(
        "property=unsupported(\"future-property-long\") value=\"preserved-value-long\" applicability=unsupported-property invalid=none"
    ));
    assert!(snapshot.contains(
        "property=custom(\"--custom-property-long\") value=\"custom-value-long\" applicability=custom-property invalid=none"
    ));
    assert!(snapshot.contains("property=invalid-shorthand(outline)"));
    assert!(snapshot.contains("applicability=invalid-shorthand-value invalid="));
    assert!(
        snapshot
            .contains("order=5 expansion=0 importance=important property=supported(outline-color)")
    );
    assert!(
        snapshot
            .contains("order=5 expansion=1 importance=important property=supported(outline-style)")
    );
    assert!(
        snapshot
            .contains("order=5 expansion=2 importance=important property=supported(outline-width)")
    );

    let typed = declarations
        .iter()
        .filter_map(|record| match record {
            RuleCollectionDiagnosticRecord::Declaration { property, .. } => Some(property),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        typed[0],
        DiagnosticDeclarationProperty::Supported { name: "color" }
    ));
    assert!(matches!(
        typed[1],
        DiagnosticDeclarationProperty::InvalidValue { name: "display" }
    ));
    assert!(matches!(
        typed[2],
        DiagnosticDeclarationProperty::Unsupported { .. }
    ));
    assert!(matches!(
        typed[3],
        DiagnosticDeclarationProperty::Custom { .. }
    ));
    assert!(matches!(
        typed[4],
        DiagnosticDeclarationProperty::InvalidShorthand { name: "outline" }
    ));
}

#[test]
fn af5_declaration_diagnostic_bounds_property_value_storage_and_serialization() {
    const AT_RULE_NAME: &str = "extraordinarily-long-at-rule";
    const UNSUPPORTED_NAME: &str = "future-property-long";
    const CUSTOM_NAME: &str = "--custom-property-long";
    let sheet = stylesheet(&format!(
        "@{AT_RULE_NAME}; div {{ {UNSUPPORTED_NAME}: preserved-value-long; {CUSTOM_NAME}: custom-value-long; }}"
    ));
    let input = author_input(7, 0, &sheet);
    let dom = document_element("div", Vec::new(), Vec::new());
    let diagnostic = rule_collection_diagnostic(
        &dom,
        matching_environment(),
        &[input],
        &StyleResolutionLimits::default(),
        RuleCollectionDiagnosticLimits {
            max_at_rule_name_text_bytes: 7,
            max_declaration_property_text_bytes: 6,
            max_declaration_value_text_bytes: 5,
            ..Default::default()
        },
    );
    let snapshot = diagnostic.to_debug_snapshot();
    assert!(snapshot.contains(&format!(
        "name=\"extraor\"[original-bytes={}]",
        AT_RULE_NAME.len()
    )));
    assert!(snapshot.contains(&format!(
        "property=unsupported(\"future\"[original-bytes={}])",
        UNSUPPORTED_NAME.len()
    )));
    assert!(snapshot.contains("value=\"prese\"[original-bytes=20]"));
    assert!(snapshot.contains(&format!(
        "property=custom(\"--cust\"[original-bytes={}])",
        CUSTOM_NAME.len()
    )));
    assert!(snapshot.contains("value=\"custo\"[original-bytes=17]"));

    let unsupported = diagnostic.records().iter().find_map(|record| match record {
        RuleCollectionDiagnosticRecord::Declaration {
            property: DiagnosticDeclarationProperty::Unsupported { name },
            ..
        } => Some(name),
        _ => None,
    });
    let unsupported = unsupported.expect("unsupported property diagnostic exists");
    assert_eq!(unsupported.text, "future");
    assert_eq!(unsupported.original_bytes, UNSUPPORTED_NAME.len());
    assert!(unsupported.truncated);

    let custom = diagnostic.records().iter().find_map(|record| match record {
        RuleCollectionDiagnosticRecord::Declaration {
            property: DiagnosticDeclarationProperty::Custom { name },
            ..
        } => Some(name),
        _ => None,
    });
    let custom = custom.expect("custom property diagnostic exists");
    assert_eq!(custom.text, "--cust");
    assert_eq!(custom.original_bytes, CUSTOM_NAME.len());
    assert!(custom.truncated);

    let at_rule = diagnostic.records().iter().find_map(|record| match record {
        RuleCollectionDiagnosticRecord::Rule {
            state:
                crate::DiagnosticRuleState::SkippedAtRule {
                    name: Some(name), ..
                },
            ..
        } => Some(name),
        _ => None,
    });
    let at_rule = at_rule.expect("skipped at-rule diagnostic exists");
    assert_eq!(at_rule.text, "extraor");
    assert_eq!(at_rule.original_bytes, AT_RULE_NAME.len());
    assert!(at_rule.truncated);

    let repeated = rule_collection_diagnostic(
        &dom,
        matching_environment(),
        &[input],
        &StyleResolutionLimits::default(),
        RuleCollectionDiagnosticLimits {
            max_at_rule_name_text_bytes: 7,
            max_declaration_property_text_bytes: 6,
            max_declaration_value_text_bytes: 5,
            ..Default::default()
        },
    );
    assert_eq!(snapshot, repeated.to_debug_snapshot());

    let storage_limited = rule_collection_diagnostic(
        &dom,
        matching_environment(),
        &[input],
        &StyleResolutionLimits::default(),
        RuleCollectionDiagnosticLimits {
            max_storage_bytes: 1,
            ..Default::default()
        },
    );
    assert!(matches!(
        storage_limited.failure(),
        Some(RuleCollectionDiagnosticFailure::LimitExceeded {
            limit: RuleCollectionDiagnosticLimit::StorageBytes,
            ..
        })
    ));

    let serialization_limited = rule_collection_diagnostic(
        &dom,
        matching_environment(),
        &[input],
        &StyleResolutionLimits::default(),
        RuleCollectionDiagnosticLimits {
            max_serialized_bytes: 1,
            ..Default::default()
        },
    );
    assert!(matches!(
        serialization_limited.failure(),
        Some(RuleCollectionDiagnosticFailure::LimitExceeded {
            limit: RuleCollectionDiagnosticLimit::SerializedBytes,
            ..
        })
    ));
}

#[test]
fn collected_hot_path_types_remain_compact_and_origin_is_existing_cascade_origin() {
    assert!(std::mem::size_of::<crate::StylesheetRuleRef>() <= 16);
    assert!(std::mem::size_of::<crate::StylesheetDeclarationRef>() <= 16);
    assert!(std::mem::size_of::<super::super::integration::ActiveCollectedStyleRule<'_>>() <= 48);
    assert!(std::mem::size_of::<crate::CascadePriority>() <= 32);
    assert!(std::mem::size_of::<crate::CascadeDeclarationInput>() <= 160);
    assert!(std::mem::size_of::<crate::CascadeDeclarationCandidate>() <= 144);
    assert!(std::mem::size_of::<crate::CascadeWinner>() <= 144);
    assert!(std::mem::size_of::<crate::MatchedStylesheetRuleInput<'_>>() <= 96);
    let sheet = stylesheet("div { color: red; }");
    let user = StylesheetCollectionInput::user(
        StylesheetSourceId::in_memory_generation_index(8),
        StylesheetOrder::new(0),
        &sheet,
        StylesheetConditionInput::None,
    );
    let collection = RuleCollection::try_new(&[user], &StyleResolutionLimits::default()).unwrap();
    assert_eq!(collection.stylesheets()[0].origin(), CascadeOrigin::User);
    let CollectedRule::ActiveStyle(rule) = &collection.rules()[0] else {
        panic!()
    };
    assert!(matches!(
        collection.declarations_for_rule(rule)[0].applicability(),
        CascadeDeclarationApplicability::Supported(_)
    ));
}

#[test]
fn declaration_classification_happens_once_when_collection_serves_multiple_elements() {
    super::super::integration::reset_declaration_classification_count();
    let sheet = stylesheet("div { color: red; width: 10px; }");
    let collection = RuleCollection::try_new(
        &[author_input(1, 0, &sheet)],
        &StyleResolutionLimits::default(),
    )
    .unwrap();
    assert_eq!(
        super::super::integration::declaration_classification_count(),
        2
    );
    let dom = document_element(
        "div",
        Vec::new(),
        vec![
            element("div", Vec::new(), Vec::new()),
            element("div", Vec::new(), Vec::new()),
        ],
    );
    let resolved = try_resolve_document_styles_from_rule_collection_with_limits(
        &dom,
        matching_environment(),
        &collection,
        &StyleResolutionLimits::default(),
    )
    .unwrap();
    assert_eq!(resolved.entries().len(), 3);
    assert_eq!(
        super::super::integration::declaration_classification_count(),
        2,
        "matching multiple elements must borrow preclassified declarations"
    );
}
