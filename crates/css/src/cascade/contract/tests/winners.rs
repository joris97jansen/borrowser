use super::super::{
    CascadeDeclarationInput, CascadeImportance, CascadeOrigin, CascadePropertyId,
    CascadeRuleContext, CascadeRuleInput, CascadeSpecificity, CurrentScopeCascadePriorityBand,
    InlineStyleRuleRef, resolve_cascade_winners, resolve_cascade_winners_from_rule_inputs,
    sort_candidates_by_cascade_order,
};
use super::support::{
    inline_declaration_source, matched_rule, parsed_value, stylesheet_declaration_source,
};
use crate::selectors::Specificity;

#[test]
fn cascade_candidate_sort_key_is_property_first_then_priority() {
    let author_rule = CascadeRuleContext::new(
        CascadeOrigin::Author,
        CascadeSpecificity::Selector(Specificity::TYPE),
        4,
    );
    let inline_style = InlineStyleRuleRef::new(3);
    let inline_rule = CascadeRuleContext::for_inline_style(0);

    let mut candidates = vec![
        CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Width,
            parsed_value("width: 10px"),
        )
        .candidate(author_rule)
        .expect("supported candidate"),
        CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 0, 1),
            1,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: red"),
        )
        .candidate(author_rule)
        .expect("supported candidate"),
        CascadeDeclarationInput::supported(
            inline_declaration_source(inline_style, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: blue"),
        )
        .candidate(inline_rule)
        .expect("supported candidate"),
        CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 1, 0),
            0,
            CascadeImportance::Important,
            CascadePropertyId::Color,
            parsed_value("color: green"),
        )
        .candidate(author_rule)
        .expect("supported candidate"),
    ];

    sort_candidates_by_cascade_order(&mut candidates);

    assert_eq!(candidates[0].property(), CascadePropertyId::Color);
    assert_eq!(candidates[0].value().to_css_text().as_deref(), Some("red"));
    assert_eq!(candidates[1].property(), CascadePropertyId::Color);
    assert_eq!(candidates[1].value().to_css_text().as_deref(), Some("blue"));
    assert_eq!(candidates[2].property(), CascadePropertyId::Color);
    assert_eq!(
        candidates[2].value().to_css_text().as_deref(),
        Some("green")
    );
    assert_eq!(candidates[3].property(), CascadePropertyId::Width);
}

#[test]
fn cascade_candidate_sorting_preserves_incoming_order_for_equal_keys() {
    let context = CascadeRuleContext::new(
        CascadeOrigin::Author,
        CascadeSpecificity::Selector(Specificity::CLASS),
        4,
    );
    let mut candidates = vec![
        CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: red"),
        )
        .candidate(context)
        .expect("supported candidate"),
        CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 1, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: blue"),
        )
        .candidate(context)
        .expect("supported candidate"),
    ];

    sort_candidates_by_cascade_order(&mut candidates);

    assert_eq!(candidates[0].value().to_css_text().as_deref(), Some("red"));
    assert_eq!(candidates[1].value().to_css_text().as_deref(), Some("blue"));
}

#[test]
fn cascade_winner_resolution_prefers_higher_specificity_over_later_rule_order() {
    let high_specificity = CascadeDeclarationInput::supported(
        stylesheet_declaration_source(0, 0, 0),
        0,
        CascadeImportance::Normal,
        CascadePropertyId::Color,
        parsed_value("color: red"),
    )
    .candidate(CascadeRuleContext::new(
        CascadeOrigin::Author,
        CascadeSpecificity::Selector(Specificity::CLASS),
        0,
    ))
    .expect("supported candidate");
    let later_lower_specificity = CascadeDeclarationInput::supported(
        stylesheet_declaration_source(0, 1, 0),
        0,
        CascadeImportance::Normal,
        CascadePropertyId::Color,
        parsed_value("color: blue"),
    )
    .candidate(CascadeRuleContext::new(
        CascadeOrigin::Author,
        CascadeSpecificity::Selector(Specificity::TYPE),
        10,
    ))
    .expect("supported candidate");

    let winners = resolve_cascade_winners(&[later_lower_specificity, high_specificity]);
    let winner = winners.get(CascadePropertyId::Color).expect("color winner");

    assert_eq!(winner.value.to_css_text().as_deref(), Some("red"));
    assert_eq!(
        winner.priority.specificity,
        CascadeSpecificity::Selector(Specificity::CLASS)
    );
    assert_eq!(winner.priority.rule_order, 0);
}

#[test]
fn cascade_winner_resolution_prefers_author_over_user_over_user_agent_in_current_normal_scope() {
    let user_agent = CascadeDeclarationInput::supported(
        stylesheet_declaration_source(0, 0, 0),
        0,
        CascadeImportance::Normal,
        CascadePropertyId::Color,
        parsed_value("color: gray"),
    )
    .candidate(CascadeRuleContext::new(
        CascadeOrigin::UserAgent,
        CascadeSpecificity::Selector(Specificity::TYPE),
        0,
    ))
    .expect("supported candidate");
    let user = CascadeDeclarationInput::supported(
        stylesheet_declaration_source(0, 1, 0),
        0,
        CascadeImportance::Normal,
        CascadePropertyId::Color,
        parsed_value("color: green"),
    )
    .candidate(CascadeRuleContext::new(
        CascadeOrigin::User,
        CascadeSpecificity::Selector(Specificity::TYPE),
        0,
    ))
    .expect("supported candidate");
    let author = CascadeDeclarationInput::supported(
        stylesheet_declaration_source(0, 2, 0),
        0,
        CascadeImportance::Normal,
        CascadePropertyId::Color,
        parsed_value("color: red"),
    )
    .candidate(CascadeRuleContext::new(
        CascadeOrigin::Author,
        CascadeSpecificity::Selector(Specificity::TYPE),
        0,
    ))
    .expect("supported candidate");

    let winners = resolve_cascade_winners(&[user, author, user_agent]);
    let winner = winners.get(CascadePropertyId::Color).expect("color winner");

    assert_eq!(winner.value.to_css_text().as_deref(), Some("red"));
    assert_eq!(
        winner.priority.current_scope_band(),
        Some(CurrentScopeCascadePriorityBand::AuthorNormal)
    );
}

#[test]
fn cascade_winner_resolution_prefers_important_band_over_higher_specificity_normal_band() {
    let high_specificity_normal = CascadeDeclarationInput::supported(
        stylesheet_declaration_source(0, 0, 0),
        0,
        CascadeImportance::Normal,
        CascadePropertyId::Color,
        parsed_value("color: red"),
    )
    .candidate(CascadeRuleContext::new(
        CascadeOrigin::Author,
        CascadeSpecificity::Selector(Specificity::new(1, 0, 0)),
        10,
    ))
    .expect("supported candidate");
    let low_specificity_important = CascadeDeclarationInput::supported(
        stylesheet_declaration_source(0, 1, 0),
        0,
        CascadeImportance::Important,
        CascadePropertyId::Color,
        parsed_value("color: blue"),
    )
    .candidate(CascadeRuleContext::new(
        CascadeOrigin::Author,
        CascadeSpecificity::Selector(Specificity::TYPE),
        0,
    ))
    .expect("supported candidate");

    let winners = resolve_cascade_winners(&[high_specificity_normal, low_specificity_important]);
    let winner = winners.get(CascadePropertyId::Color).expect("color winner");

    assert_eq!(winner.value.to_css_text().as_deref(), Some("blue"));
    assert_eq!(
        winner.priority.current_scope_band(),
        Some(CurrentScopeCascadePriorityBand::AuthorImportant)
    );
}

#[test]
fn cascade_winner_resolution_prefers_user_important_over_author_important_in_current_scope() {
    let author_important = CascadeDeclarationInput::supported(
        stylesheet_declaration_source(0, 0, 0),
        0,
        CascadeImportance::Important,
        CascadePropertyId::Color,
        parsed_value("color: red"),
    )
    .candidate(CascadeRuleContext::new(
        CascadeOrigin::Author,
        CascadeSpecificity::Selector(Specificity::new(1, 0, 0)),
        10,
    ))
    .expect("supported candidate");
    let user_important = CascadeDeclarationInput::supported(
        stylesheet_declaration_source(0, 1, 0),
        0,
        CascadeImportance::Important,
        CascadePropertyId::Color,
        parsed_value("color: blue"),
    )
    .candidate(CascadeRuleContext::new(
        CascadeOrigin::User,
        CascadeSpecificity::Selector(Specificity::TYPE),
        0,
    ))
    .expect("supported candidate");

    let winners = resolve_cascade_winners(&[author_important, user_important]);
    let winner = winners.get(CascadePropertyId::Color).expect("color winner");

    assert_eq!(winner.value.to_css_text().as_deref(), Some("blue"));
    assert_eq!(
        winner.priority.current_scope_band(),
        Some(CurrentScopeCascadePriorityBand::UserImportant)
    );
}

#[test]
fn cascade_winner_resolution_prefers_later_rule_order_when_specificity_ties() {
    let earlier_rule = CascadeRuleInput::from_stylesheet_match(
        &matched_rule(0, 0, &[Specificity::CLASS]),
        CascadeOrigin::Author,
        0,
        vec![CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: red"),
        )],
    )
    .expect("valid rule")
    .expect("matching rule");
    let later_rule = CascadeRuleInput::from_stylesheet_match(
        &matched_rule(0, 1, &[Specificity::CLASS]),
        CascadeOrigin::Author,
        1,
        vec![CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 1, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: blue"),
        )],
    )
    .expect("valid rule")
    .expect("matching rule");

    let winners = resolve_cascade_winners_from_rule_inputs(&[later_rule, earlier_rule]);
    let winner = winners.get(CascadePropertyId::Color).expect("color winner");

    assert_eq!(winner.value.to_css_text().as_deref(), Some("blue"));
    assert_eq!(winner.priority.rule_order, 1);
}

#[test]
fn cascade_winner_resolution_prefers_later_declaration_order_within_one_rule() {
    let rule = CascadeRuleInput::from_stylesheet_match(
        &matched_rule(0, 0, &[Specificity::TYPE]),
        CascadeOrigin::Author,
        0,
        vec![
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 0, 0),
                0,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: red"),
            ),
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 0, 1),
                1,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: blue"),
            ),
        ],
    )
    .expect("valid rule")
    .expect("matching rule");

    let winners = resolve_cascade_winners_from_rule_inputs(&[rule]);
    let winner = winners.get(CascadePropertyId::Color).expect("color winner");

    assert_eq!(winner.value.to_css_text().as_deref(), Some("blue"));
    assert_eq!(winner.priority.declaration_order, 1);
}

#[test]
fn cascade_winner_resolution_ignores_unsupported_custom_and_invalid_declarations() {
    let inline_style = InlineStyleRuleRef::new(12);
    let rule = CascadeRuleInput::from_inline_style(
        inline_style,
        0,
        vec![
            CascadeDeclarationInput::unsupported_property(
                inline_declaration_source(inline_style, 0),
                0,
                CascadeImportance::Normal,
                "zoom",
                parsed_value("zoom: 2"),
            ),
            CascadeDeclarationInput::custom_property(
                inline_declaration_source(inline_style, 1),
                1,
                CascadeImportance::Normal,
                "--brand",
                parsed_value("--brand: teal"),
            ),
            CascadeDeclarationInput::invalid_property_name(
                inline_declaration_source(inline_style, 2),
                2,
                CascadeImportance::Normal,
                parsed_value("color: green"),
            ),
            CascadeDeclarationInput::supported(
                inline_declaration_source(inline_style, 3),
                3,
                CascadeImportance::Normal,
                CascadePropertyId::Color,
                parsed_value("color: red"),
            ),
        ],
    )
    .expect("valid inline rule");

    let winners = resolve_cascade_winners_from_rule_inputs(&[rule]);

    assert_eq!(winners.entries().len(), 1);
    assert_eq!(winners.entries()[0].property(), CascadePropertyId::Color);
    assert_eq!(
        winners.entries()[0].winner().value.to_css_text().as_deref(),
        Some("red")
    );
}

#[test]
fn cascade_winner_resolution_uses_later_input_for_equal_candidate_keys() {
    let context = CascadeRuleContext::new(
        CascadeOrigin::Author,
        CascadeSpecificity::Selector(Specificity::CLASS),
        4,
    );
    let first = CascadeDeclarationInput::supported(
        stylesheet_declaration_source(0, 0, 0),
        0,
        CascadeImportance::Normal,
        CascadePropertyId::Color,
        parsed_value("color: red"),
    )
    .candidate(context)
    .expect("supported candidate");
    let second = CascadeDeclarationInput::supported(
        stylesheet_declaration_source(0, 1, 0),
        0,
        CascadeImportance::Normal,
        CascadePropertyId::Color,
        parsed_value("color: blue"),
    )
    .candidate(context)
    .expect("supported candidate");

    let winners = resolve_cascade_winners(&[first, second]);
    let winner = winners.get(CascadePropertyId::Color).expect("color winner");

    assert_eq!(winner.value.to_css_text().as_deref(), Some("blue"));
    assert_eq!(
        winner.priority,
        context.priority_for_declaration(CascadeImportance::Normal, 0)
    );
}

#[test]
fn cascade_winner_set_is_property_sorted_and_snapshot_stable() {
    let winners = resolve_cascade_winners(&[
        CascadeDeclarationInput::supported(
            stylesheet_declaration_source(0, 0, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Width,
            parsed_value("width: 10px"),
        )
        .candidate(CascadeRuleContext::new(
            CascadeOrigin::Author,
            CascadeSpecificity::Selector(Specificity::TYPE),
            0,
        ))
        .expect("supported candidate"),
        CascadeDeclarationInput::supported(
            inline_declaration_source(InlineStyleRuleRef::new(15), 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Color,
            parsed_value("color: blue"),
        )
        .candidate(CascadeRuleContext::for_inline_style(0))
        .expect("supported candidate"),
    ]);

    assert_eq!(winners.entries()[0].property(), CascadePropertyId::Color);
    assert_eq!(winners.entries()[1].property(), CascadePropertyId::Width);
    assert_eq!(
        winners.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "cascade-winners\n",
            "  color: winner(source=inline-style[15]/declaration[0], band=author-normal, specificity=inline-style, rule-order=0, declaration-order=0, value=\"blue\")\n",
            "  width: winner(source=stylesheet[0/0]/declaration[0], band=author-normal, specificity=selector(0,0,1), rule-order=0, declaration-order=0, value=\"10px\")\n",
        )
    );
}
