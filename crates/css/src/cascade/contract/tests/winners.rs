use super::super::{
    CandidateDataMismatch, CascadeDeclarationInput, CascadeImportance, CascadeOrigin,
    CascadePropertyId, CascadeResolutionBudget, CascadeResolutionError, CascadeRuleContext,
    CascadeRuleInput, CascadeRuleSource, InlineStyleRuleRef, StyleRulePosition, StylesheetOrder,
    StylesheetRuleOrder, StylesheetRuleRef, ValidatedCascadeRuleInputs,
};
use super::support::{
    inline_declaration_source, parse_error, parsed_value, preserved_value, resolve_rule_inputs,
    stylesheet_declaration_source,
};
use crate::selectors::Specificity;

fn style_rule(
    stylesheet: u32,
    raw_rule: u32,
    stylesheet_order: u32,
    style_position: u32,
    origin: CascadeOrigin,
    specificity: Specificity,
    declarations: Vec<CascadeDeclarationInput>,
) -> CascadeRuleInput<'static> {
    let rule = StylesheetRuleRef::new(
        crate::cascade::StylesheetSourceId::compatibility_generation_index(stylesheet),
        crate::cascade::RawRuleIndex::new(raw_rule),
    );
    CascadeRuleInput::new(
        CascadeRuleSource::Stylesheet(rule),
        CascadeRuleContext::for_stylesheet(
            origin,
            specificity,
            StylesheetRuleOrder::new(
                StylesheetOrder::new(stylesheet_order),
                StyleRulePosition::new(style_position),
            ),
        ),
        declarations,
    )
    .expect("test stylesheet rule source owns its declarations")
}

fn inline_rule(
    inline: InlineStyleRuleRef,
    declarations: Vec<CascadeDeclarationInput>,
) -> CascadeRuleInput<'static> {
    CascadeRuleInput::new(
        CascadeRuleSource::InlineStyle(inline),
        CascadeRuleContext::for_inline_style(),
        declarations,
    )
    .expect("test inline source owns its declarations")
}

fn supported(
    source: super::super::CascadeDeclarationSource,
    order: u32,
    importance: CascadeImportance,
    property: CascadePropertyId,
    text: &str,
) -> CascadeDeclarationInput {
    CascadeDeclarationInput::supported(source, order, importance, property, parsed_value(text))
}

#[test]
fn origin_and_importance_ordering_matches_supported_level_five_bands() {
    let rules = vec![
        style_rule(
            0,
            0,
            0,
            0,
            CascadeOrigin::UserAgent,
            Specificity::ZERO,
            vec![supported(
                stylesheet_declaration_source(0, 0, 0),
                0,
                CascadeImportance::Important,
                CascadePropertyId::Color,
                "color: black",
            )],
        ),
        style_rule(
            1,
            0,
            1,
            0,
            CascadeOrigin::Author,
            Specificity::new(9, 9, 9),
            vec![supported(
                stylesheet_declaration_source(1, 0, 0),
                0,
                CascadeImportance::Important,
                CascadePropertyId::Color,
                "color: red",
            )],
        ),
        style_rule(
            2,
            0,
            2,
            0,
            CascadeOrigin::User,
            Specificity::ZERO,
            vec![supported(
                stylesheet_declaration_source(2, 0, 0),
                0,
                CascadeImportance::Important,
                CascadePropertyId::Color,
                "color: blue",
            )],
        ),
    ];
    let winners = resolve_rule_inputs(rules).expect("valid origin inputs resolve");
    assert_eq!(
        winners
            .get(CascadePropertyId::Color)
            .expect("color winner")
            .value
            .to_css_text()
            .as_deref(),
        Some("black"),
        "UA important outranks user and author important in the supported band model"
    );
}

#[test]
fn element_attachment_is_a_distinct_precedence_step() {
    let inline = InlineStyleRuleRef::new(7);
    let stylesheet_normal = style_rule(
        0,
        0,
        0,
        0,
        CascadeOrigin::Author,
        Specificity::new(100, 100, 100),
        vec![supported(
            stylesheet_declaration_source(0, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            "color: red",
        )],
    );
    let inline_normal = inline_rule(
        inline,
        vec![supported(
            inline_declaration_source(inline, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            "color: blue",
        )],
    );
    let winners = resolve_rule_inputs(vec![stylesheet_normal, inline_normal])
        .expect("valid element-attached inputs resolve");
    let winner = winners.get(CascadePropertyId::Color).expect("color winner");
    assert_eq!(winner.value.to_css_text().as_deref(), Some("blue"));
    assert!(
        winner
            .priority
            .declaration_precedence()
            .is_element_attached()
    );

    let stylesheet_important = style_rule(
        1,
        0,
        1,
        0,
        CascadeOrigin::Author,
        Specificity::ZERO,
        vec![supported(
            stylesheet_declaration_source(1, 0, 0),
            0,
            CascadeImportance::Important,
            CascadePropertyId::Color,
            "color: green",
        )],
    );
    let winners = resolve_rule_inputs(vec![
        stylesheet_important,
        inline_rule(
            inline,
            vec![supported(
                inline_declaration_source(inline, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                "color: blue",
            )],
        ),
    ])
    .expect("importance inputs resolve");
    assert_eq!(
        winners
            .get(CascadePropertyId::Color)
            .expect("color winner")
            .value
            .to_css_text()
            .as_deref(),
        Some("green")
    );

    let winners = resolve_rule_inputs(vec![
        style_rule(
            2,
            0,
            2,
            0,
            CascadeOrigin::Author,
            Specificity::new(100, 100, 100),
            vec![supported(
                stylesheet_declaration_source(2, 0, 0),
                0,
                CascadeImportance::Important,
                CascadePropertyId::Color,
                "color: green",
            )],
        ),
        inline_rule(
            inline,
            vec![supported(
                inline_declaration_source(inline, 0),
                0,
                CascadeImportance::Important,
                CascadePropertyId::Color,
                "color: blue",
            )],
        ),
    ])
    .expect("important attachment inputs resolve");
    assert_eq!(
        winners
            .get(CascadePropertyId::Color)
            .expect("color winner")
            .value
            .to_css_text()
            .as_deref(),
        Some("blue"),
        "element attachment resolves the tie only after the important band"
    );
}

#[test]
fn specificity_then_stylesheet_and_declaration_order_resolve_remaining_ties() {
    let higher_specificity = style_rule(
        0,
        0,
        0,
        0,
        CascadeOrigin::Author,
        Specificity::B,
        vec![supported(
            stylesheet_declaration_source(0, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            "color: red",
        )],
    );
    let later_lower = style_rule(
        1,
        0,
        1,
        0,
        CascadeOrigin::Author,
        Specificity::C,
        vec![supported(
            stylesheet_declaration_source(1, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            "color: blue",
        )],
    );
    let winners =
        resolve_rule_inputs(vec![higher_specificity, later_lower]).expect("inputs resolve");
    assert_eq!(
        winners
            .get(CascadePropertyId::Color)
            .expect("color winner")
            .value
            .to_css_text()
            .as_deref(),
        Some("red")
    );

    let declarations = vec![
        supported(
            stylesheet_declaration_source(2, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            "color: red",
        ),
        supported(
            stylesheet_declaration_source(2, 0, 1),
            1,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            "color: blue",
        ),
    ];
    let winners = resolve_rule_inputs(vec![style_rule(
        2,
        0,
        2,
        0,
        CascadeOrigin::Author,
        Specificity::C,
        declarations,
    )])
    .expect("declaration order input resolves");
    assert_eq!(
        winners
            .get(CascadePropertyId::Color)
            .expect("color winner")
            .value
            .to_css_text()
            .as_deref(),
        Some("blue")
    );
}

#[test]
fn duplicate_and_inconsistent_candidate_identities_are_errors() {
    let source = stylesheet_declaration_source(0, 0, 0);
    let duplicate = style_rule(
        0,
        0,
        0,
        0,
        CascadeOrigin::Author,
        Specificity::C,
        vec![
            supported(
                source,
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                "color: red",
            ),
            supported(
                source,
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                "color: red",
            ),
        ],
    );
    let duplicate_result = resolve_rule_inputs(vec![duplicate]);
    assert!(
        matches!(
            &duplicate_result,
            Err(CascadeResolutionError::DuplicateCandidateIdentity { .. })
        ),
        "unexpected duplicate result: {duplicate_result:?}"
    );
    let duplicate_message = duplicate_result
        .as_ref()
        .expect_err("duplicate identity remains an error")
        .to_string();
    assert!(duplicate_message.contains("property=color"));
    assert!(duplicate_message.contains("first-source=stylesheet["));
    assert!(duplicate_message.contains("second-source=stylesheet["));
    assert!(duplicate_message.contains("first-priority=author-normal:style-rule:"));
    assert!(duplicate_message.contains("second-priority=author-normal:style-rule:"));
    let (first_priority, second_priority) = match duplicate_result
        .as_ref()
        .expect_err("duplicate identity remains an error")
    {
        CascadeResolutionError::DuplicateCandidateIdentity {
            first_priority,
            second_priority,
            ..
        } => (*first_priority, *second_priority),
        other => panic!("unexpected duplicate error: {other:?}"),
    };
    assert_eq!(first_priority, second_priority);

    let inconsistent_priority = style_rule(
        0,
        0,
        0,
        0,
        CascadeOrigin::Author,
        Specificity::C,
        vec![
            supported(
                source,
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                "color: red",
            ),
            supported(
                source,
                0,
                CascadeImportance::Important,
                CascadePropertyId::Color,
                "color: red",
            ),
        ],
    );
    let inconsistent_priority_result = resolve_rule_inputs(vec![inconsistent_priority]);
    match inconsistent_priority_result
        .as_ref()
        .expect_err("inconsistent priority remains an error")
    {
        CascadeResolutionError::InconsistentCandidateIdentity {
            first_priority,
            second_priority,
            mismatch: CandidateDataMismatch::Priority,
            ..
        } => assert_ne!(first_priority, second_priority),
        other => panic!("unexpected inconsistent-priority error: {other:?}"),
    }

    let inconsistent_value = style_rule(
        0,
        0,
        0,
        0,
        CascadeOrigin::Author,
        Specificity::C,
        vec![
            supported(
                source,
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                "color: red",
            ),
            supported(
                source,
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                "color: blue",
            ),
        ],
    );
    assert!(matches!(
        resolve_rule_inputs(vec![inconsistent_value]),
        Err(CascadeResolutionError::InconsistentCandidateIdentity {
            mismatch: CandidateDataMismatch::Value,
            ..
        })
    ));

    let inconsistent_expansion_metadata = style_rule(
        0,
        0,
        0,
        0,
        CascadeOrigin::Author,
        Specificity::C,
        vec![
            CascadeDeclarationInput::supported_with_expansion_order(
                source,
                0,
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: red"),
            ),
            CascadeDeclarationInput::supported_with_expansion_order(
                source,
                0,
                1,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: red"),
            ),
        ],
    );
    assert!(matches!(
        resolve_rule_inputs(vec![inconsistent_expansion_metadata]),
        Err(CascadeResolutionError::InconsistentCandidateIdentity {
            mismatch: CandidateDataMismatch::ExpansionMetadata,
            ..
        })
    ));
}

#[test]
fn distinct_sources_with_equal_complete_priority_are_rejected_by_checked_test_boundary() {
    let context = CascadeRuleContext::for_stylesheet(
        CascadeOrigin::Author,
        Specificity::C,
        StylesheetRuleOrder::new(StylesheetOrder::new(0), StyleRulePosition::new(0)),
    );
    let first_rule = StylesheetRuleRef::new(
        crate::cascade::StylesheetSourceId::compatibility_generation_index(0),
        crate::cascade::RawRuleIndex::new(0),
    );
    let second_rule = StylesheetRuleRef::new(
        crate::cascade::StylesheetSourceId::compatibility_generation_index(1),
        crate::cascade::RawRuleIndex::new(0),
    );
    let first = CascadeRuleInput::new(
        CascadeRuleSource::Stylesheet(first_rule),
        context,
        vec![supported(
            stylesheet_declaration_source(0, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            "color: red",
        )],
    )
    .unwrap();
    let second = CascadeRuleInput::new(
        CascadeRuleSource::Stylesheet(second_rule),
        context,
        vec![supported(
            stylesheet_declaration_source(1, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            "color: blue",
        )],
    )
    .unwrap();
    assert!(matches!(
        resolve_rule_inputs(vec![first, second]),
        Err(CascadeResolutionError::EqualPriorityDistinctCandidates { .. })
    ));
}

#[test]
fn checked_test_boundary_rejects_identity_reuse_across_rule_inputs() {
    let source = stylesheet_declaration_source(0, 0, 0);
    let first = style_rule(
        0,
        0,
        0,
        0,
        CascadeOrigin::Author,
        Specificity::C,
        vec![supported(
            source,
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            "color: red",
        )],
    );
    let repeated_with_priority = CascadeRuleInput::new(
        first.source(),
        CascadeRuleContext::for_stylesheet(
            CascadeOrigin::Author,
            Specificity::B,
            StylesheetRuleOrder::new(StylesheetOrder::new(1), StyleRulePosition::new(0)),
        ),
        vec![supported(
            source,
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            "color: red",
        )],
    )
    .unwrap();
    assert!(matches!(
        resolve_rule_inputs(vec![first.clone(), first.clone()]),
        Err(CascadeResolutionError::DuplicateCandidateIdentity { .. })
    ));
    assert!(matches!(
        resolve_rule_inputs(vec![first.clone(), repeated_with_priority]),
        Err(CascadeResolutionError::InconsistentCandidateIdentity {
            mismatch: CandidateDataMismatch::Priority,
            ..
        })
    ));

    let repeated_with_data = CascadeRuleInput::new(
        first.source(),
        first.context(),
        vec![supported(
            source,
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            "color: blue",
        )],
    )
    .unwrap();
    assert!(matches!(
        resolve_rule_inputs(vec![first, repeated_with_data]),
        Err(CascadeResolutionError::InconsistentCandidateIdentity {
            mismatch: CandidateDataMismatch::Value,
            ..
        })
    ));
}

#[test]
fn shorthand_source_may_emit_distinct_longhands_but_not_the_same_longhand_twice() {
    let source = stylesheet_declaration_source(0, 0, 0);
    let valid = style_rule(
        0,
        0,
        0,
        0,
        CascadeOrigin::Author,
        Specificity::C,
        vec![
            supported(
                source,
                0,
                CascadeImportance::Normal,
                CascadePropertyId::MarginTop,
                "margin-top: 1px",
            ),
            supported(
                source,
                0,
                CascadeImportance::Normal,
                CascadePropertyId::MarginRight,
                "margin-right: 2px",
            ),
        ],
    );
    assert_eq!(
        resolve_rule_inputs(vec![valid])
            .expect("distinct shorthand longhands are valid")
            .entries()
            .len(),
        2
    );
}

#[test]
fn invalid_and_unsupported_declarations_do_not_participate() {
    let inline = InlineStyleRuleRef::new(3);
    let rule = inline_rule(
        inline,
        vec![
            CascadeDeclarationInput::unsupported_property(
                inline_declaration_source(inline, 0),
                0,
                CascadeImportance::Important,
                "zoom",
                preserved_value("zoom: 2"),
            ),
            CascadeDeclarationInput::custom_property(
                inline_declaration_source(inline, 1),
                1,
                CascadeImportance::Important,
                "--brand",
                preserved_value("--brand: red"),
            ),
            CascadeDeclarationInput::invalid_property_name(
                inline_declaration_source(inline, 2),
                2,
                CascadeImportance::Important,
                preserved_value("zoom: red"),
            ),
            supported(
                inline_declaration_source(inline, 3),
                3,
                CascadeImportance::Normal,
                CascadePropertyId::Width,
                "width: 10px",
            ),
        ],
    );
    let winners = resolve_rule_inputs(vec![rule]).expect("filtered inputs resolve");
    assert_eq!(winners.entries().len(), 1);
    assert_eq!(winners.entries()[0].property(), CascadePropertyId::Width);
}

#[test]
fn admitted_count_and_candidate_budget_ignore_filtered_declarations() {
    let inline = InlineStyleRuleRef::new(9);
    let mut declarations = Vec::new();
    for index in 0..128 {
        declarations.push(CascadeDeclarationInput::unsupported_property(
            inline_declaration_source(inline, index),
            index,
            CascadeImportance::Important,
            "zoom",
            preserved_value("zoom: 2"),
        ));
    }
    declarations.push(supported(
        inline_declaration_source(inline, 128),
        128,
        CascadeImportance::Normal,
        CascadePropertyId::Width,
        "width: 10px",
    ));
    let budget = CascadeResolutionBudget::try_new(0, 1, 0).unwrap();
    let validated = ValidatedCascadeRuleInputs::try_from_checked_inputs(
        vec![inline_rule(inline, declarations)],
        budget,
    )
    .expect("only the supported declaration consumes candidate budget");
    assert_eq!(validated.admitted_candidate_count(), 1);
}

#[test]
fn malformed_non_candidate_ordering_reports_a_property_independent_source_error() {
    let inline = InlineStyleRuleRef::new(10);
    let fixtures = vec![
        CascadeDeclarationInput::unsupported_property(
            inline_declaration_source(inline, 0),
            1,
            CascadeImportance::Normal,
            "zoom",
            preserved_value("zoom: 2"),
        ),
        CascadeDeclarationInput::invalid_value(
            inline_declaration_source(inline, 0),
            1,
            CascadeImportance::Normal,
            CascadePropertyId::Display,
            parse_error(CascadePropertyId::Display, "display: grid"),
            preserved_value("display: grid"),
        ),
        CascadeDeclarationInput::custom_property(
            inline_declaration_source(inline, 0),
            1,
            CascadeImportance::Normal,
            "--brand",
            preserved_value("--brand: red"),
        ),
        CascadeDeclarationInput::invalid_property_name(
            inline_declaration_source(inline, 0),
            1,
            CascadeImportance::Normal,
            preserved_value("color: red"),
        ),
    ];

    for non_candidate in fixtures {
        let current_source = inline_declaration_source(inline, 1);
        let error = resolve_rule_inputs(vec![inline_rule(
            inline,
            vec![
                non_candidate,
                supported(
                    current_source,
                    0,
                    CascadeImportance::Normal,
                    CascadePropertyId::Color,
                    "color: red",
                ),
            ],
        )])
        .expect_err("declaration sources must appear in increasing authored order");
        assert!(matches!(
            error,
            CascadeResolutionError::DeclarationSourceOrderInvariant {
                rule_source: CascadeRuleSource::InlineStyle(source),
                previous_source,
                current_source: observed_current,
                previous_order,
                current_order,
            } if source == inline
                && previous_source == inline_declaration_source(inline, 0)
                && observed_current == current_source
                && previous_order.get() == 1
                && current_order.get() == 0
        ));
        let rendered = error.to_string();
        assert!(rendered.contains("previous-order=1 current-order=0"));
        assert!(!rendered.contains("property="));
    }
}
