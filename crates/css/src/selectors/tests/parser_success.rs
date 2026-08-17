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

#[test]
fn parser_accepts_quoted_empty_values_for_every_supported_attribute_operator() {
    for source in [
        r#"[data-x=""]"#,
        r#"[data-x~=""]"#,
        r#"[data-x|=""]"#,
        r#"[data-x^=""]"#,
        r#"[data-x$=""]"#,
        r#"[data-x*=""]"#,
    ] {
        assert!(
            parse_selector_result(source).parsed().is_some(),
            "expected {source:?} to remain syntactically valid",
        );
    }
}

#[test]
fn parser_supports_tree_structural_pseudos_with_ascii_case_insensitive_keywords() {
    for source in [
        ":root",
        ":ROOT",
        ":RoOt",
        ":empty",
        ":Empty",
        ":FIRST-CHILD",
        ":last-CHILD",
        ":Only-Child",
    ] {
        assert!(
            parse_selector_result(source).parsed().is_some(),
            "expected {source:?} to parse",
        );
    }

    let result = parse_selector_result("section:FIRST-CHILD:Empty > p:ONLY-child, :RoOt");
    assert_eq!(
        result.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: parsed\n",
            "span: @0..48\n",
            "selector[0] @0..40 specificity=(0,3,2)\n",
            "  compound[0] @0..25 specificity=(0,2,1)\n",
            "    - type(\"section\") node=@0..7 name=@0..7\n",
            "    - tree-structural-pseudo-class(first-child) node=@7..19\n",
            "    - tree-structural-pseudo-class(empty) node=@19..25\n",
            "  combined[0] child @26..40\n",
            "    compound @28..40 specificity=(0,1,1)\n",
            "      - type(\"p\") node=@28..29 name=@28..29\n",
            "      - tree-structural-pseudo-class(only-child) node=@29..40\n",
            "selector[1] @42..47 specificity=(0,1,0)\n",
            "  compound[0] @42..47 specificity=(0,1,0)\n",
            "    - tree-structural-pseudo-class(root) node=@42..47\n",
        )
    );
}
