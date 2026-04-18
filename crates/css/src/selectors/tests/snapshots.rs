use super::support::sample_selector_list;
use crate::syntax::CssInput;

#[test]
fn selector_list_snapshot_is_stable() {
    let input = CssInput::from("article.card > h1#hero[data-kind=\"promo\"]");
    let list = sample_selector_list(&input);

    assert_eq!(
        list.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-list\n",
            "span: @0..41\n",
            "selector[0] @0..41 specificity=(1,2,2)\n",
            "  compound[0] @0..12 specificity=(0,1,1)\n",
            "    - type(\"article\") node=@0..7 name=@0..7\n",
            "    - class(\"card\") node=@7..12 name=@8..12\n",
            "  combined[0] child @13..41\n",
            "    compound @15..41 specificity=(1,1,1)\n",
            "      - type(\"h1\") node=@15..17 name=@15..17\n",
            "      - id(\"hero\") node=@17..22 name=@18..22\n",
            "      - attribute-match(name=\"data-kind\", name_span=@23..32, matcher=exact, value=string(\"promo\", span=@33..40)) node=@22..41\n",
        )
    );
}
