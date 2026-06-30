use super::super::{
    CascadeDeclarationInput, CascadeImportance, CascadeOrigin, CascadePropertyId, CascadeRuleInput,
    InlineStyleRuleRef, cascade_evaluation_debug_snapshot,
};
use super::support::{
    inline_declaration_source, matched_rule, parse_error, parsed_value, preserved_value,
    stylesheet_declaration_source,
};
use crate::selectors::Specificity;

#[test]
fn cascade_evaluation_debug_snapshot_covers_filtering_ordering_and_winners() {
    let stylesheet_rule = CascadeRuleInput::from_stylesheet_match(
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
            CascadeDeclarationInput::unsupported_property(
                stylesheet_declaration_source(0, 0, 1),
                1,
                CascadeImportance::Normal,
                "zoom",
                parsed_value("zoom: 2"),
            ),
            CascadeDeclarationInput::supported(
                stylesheet_declaration_source(0, 0, 2),
                2,
                CascadeImportance::Important,
                CascadePropertyId::Color,
                parsed_value("color: blue"),
            ),
            CascadeDeclarationInput::invalid_value(
                stylesheet_declaration_source(0, 0, 3),
                3,
                CascadeImportance::Normal,
                CascadePropertyId::Display,
                parse_error(CascadePropertyId::Display, "display: grid"),
                preserved_value("display: grid"),
            ),
        ],
    )
    .expect("valid stylesheet rule")
    .expect("matched rule");
    let inline_style = InlineStyleRuleRef::new(3);
    let inline_rule = CascadeRuleInput::from_inline_style(
        inline_style,
        1,
        vec![CascadeDeclarationInput::supported(
            inline_declaration_source(inline_style, 0),
            0,
            CascadeImportance::Normal,
            CascadePropertyId::Width,
            parsed_value("width: 20px"),
        )],
    )
    .expect("valid inline rule");

    assert_eq!(
        cascade_evaluation_debug_snapshot(&[stylesheet_rule, inline_rule]),
        concat!(
            "version: 1\n",
            "cascade-evaluation\n",
            "rule-inputs: 2\n",
            "  rule-input[0]: source=stylesheet[0/0] origin=author specificity=selector(0,0,1) rule-order=0 declarations=4\n",
            "    declaration[0]: source=stylesheet[0/0]/declaration[0] declaration-order=0 importance=normal property=supported(color) applicability=supported(color) value=\"red\"\n",
            "    declaration[1]: source=stylesheet[0/0]/declaration[1] declaration-order=1 importance=normal property=unsupported(\"zoom\") applicability=unsupported-property value=\"2\"\n",
            "    declaration[2]: source=stylesheet[0/0]/declaration[2] declaration-order=2 importance=important property=supported(color) applicability=supported(color) value=\"blue\"\n",
            "    declaration[3]: source=stylesheet[0/0]/declaration[3] declaration-order=3 importance=normal property=invalid-value(display) applicability=invalid-value(display) value=\"grid\" invalid-reason=unsupported-display-keyword\n",
            "  rule-input[1]: source=inline-style[3] origin=author specificity=inline-style rule-order=1 declarations=1\n",
            "    declaration[0]: source=inline-style[3]/declaration[0] declaration-order=0 importance=normal property=supported(width) applicability=supported(width) value=\"20px\"\n",
            "candidates-source-order: 3\n",
            "  candidate[0]: property=color source=stylesheet[0/0]/declaration[0] band=author-normal specificity=selector(0,0,1) rule-order=0 declaration-order=0 value=\"red\"\n",
            "  candidate[1]: property=color source=stylesheet[0/0]/declaration[2] band=author-important specificity=selector(0,0,1) rule-order=0 declaration-order=2 value=\"blue\"\n",
            "  candidate[2]: property=width source=inline-style[3]/declaration[0] band=author-normal specificity=inline-style rule-order=1 declaration-order=0 value=\"20px\"\n",
            "candidates-cascade-order: 3\n",
            "  candidate[0]: property=color source=stylesheet[0/0]/declaration[0] band=author-normal specificity=selector(0,0,1) rule-order=0 declaration-order=0 value=\"red\"\n",
            "  candidate[1]: property=color source=stylesheet[0/0]/declaration[2] band=author-important specificity=selector(0,0,1) rule-order=0 declaration-order=2 value=\"blue\"\n",
            "  candidate[2]: property=width source=inline-style[3]/declaration[0] band=author-normal specificity=inline-style rule-order=1 declaration-order=0 value=\"20px\"\n",
            "winners: 2\n",
            "  color: winner(source=stylesheet[0/0]/declaration[2], band=author-important, specificity=selector(0,0,1), rule-order=0, declaration-order=2, value=\"blue\")\n",
            "  width: winner(source=inline-style[3]/declaration[0], band=author-normal, specificity=inline-style, rule-order=1, declaration-order=0, value=\"20px\")\n",
        )
    );
}
