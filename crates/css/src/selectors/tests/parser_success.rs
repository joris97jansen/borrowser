use super::super::Combinator;
use super::support::{parse_selector_result, parsed_selector_list};

#[test]
fn parser_builds_ir_for_supported_selector_lists() {
    let result = parse_selector_result("article.card > h1#hero[data-kind=\"promo\"], aside");

    assert_eq!(
        result.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: parsed\n",
            "span: @0..49\n",
            "selector[0] @0..41 specificity=(1,2,2)\n",
            "  compound[0] @0..12 specificity=(0,1,1)\n",
            "    - type(\"article\") node=@0..7 name=@0..7\n",
            "    - class(\"card\") node=@7..12 name=@8..12\n",
            "  combined[0] child @13..41\n",
            "    compound @15..41 specificity=(1,1,1)\n",
            "      - type(\"h1\") node=@15..17 name=@15..17\n",
            "      - id(\"hero\") node=@17..22 name=@18..22\n",
            "      - attribute-match(name=\"data-kind\", name_span=@23..32, matcher=exact, value=string(\"promo\", span=@34..39)) node=@22..41\n",
            "selector[1] @43..48 specificity=(0,0,1)\n",
            "  compound[0] @43..48 specificity=(0,0,1)\n",
            "    - type(\"aside\") node=@43..48 name=@43..48\n",
        )
    );
}

#[test]
fn parser_distinguishes_comments_from_descendant_whitespace() {
    let compact = parsed_selector_list("div/**/.card");
    let compact_selector = compact.iter().next().expect("compact selector");
    assert!(compact_selector.tail().is_empty());
    assert_eq!(compact_selector.head().subclasses().len(), 1);

    let descendant = parsed_selector_list("div /* gap */ .card");
    let descendant_selector = descendant.iter().next().expect("descendant selector");
    assert_eq!(descendant_selector.tail().len(), 1);
    assert_eq!(
        descendant_selector.tail()[0].combinator(),
        Combinator::Descendant
    );
}
