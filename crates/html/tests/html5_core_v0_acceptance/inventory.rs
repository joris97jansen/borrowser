use std::collections::BTreeSet;

use super::support::{
    AcceptanceCase, AcceptanceKind, ExpectedOutput, FixtureState, anchors,
    assert_acceptance_case_is_well_formed,
};

const fn tokenizer_case(
    acceptance_id: &'static str,
    evidence_ref: &'static str,
    fixture_state: FixtureState,
    contract_anchor: &'static str,
    note: Option<&'static str>,
) -> AcceptanceCase {
    AcceptanceCase {
        acceptance_id,
        evidence_ref,
        kind: AcceptanceKind::Tokenizer,
        fixture_state,
        contract_anchor,
        output: ExpectedOutput::TokensV1,
        note,
    }
}

const fn tree_builder_case(
    acceptance_id: &'static str,
    evidence_ref: &'static str,
    fixture_state: FixtureState,
    contract_anchor: &'static str,
    note: Option<&'static str>,
) -> AcceptanceCase {
    AcceptanceCase {
        acceptance_id,
        evidence_ref,
        kind: AcceptanceKind::TreeBuilder,
        fixture_state,
        contract_anchor,
        output: ExpectedOutput::DomV1,
        note,
    }
}

const TOKENIZER_ACCEPTANCE_CASES: &[AcceptanceCase] = &[
    tokenizer_case(
        "tok-empty-eof",
        "tok-empty-eof",
        FixtureState::Existing,
        anchors::TOKENIZER_STATE_FAMILIES,
        None,
    ),
    tokenizer_case(
        "tok-basic-text",
        "tok-basic-text",
        FixtureState::Existing,
        anchors::TOKENIZER_STATE_FAMILIES,
        None,
    ),
    tokenizer_case(
        "tok-simple-tags",
        "tok-simple-tags",
        FixtureState::Existing,
        anchors::TOKENIZER_STATE_FAMILIES,
        None,
    ),
    tokenizer_case(
        "tok-attrs-core",
        "tok-attrs-core",
        FixtureState::Existing,
        anchors::ATTRIBUTE_RULES_BASELINE,
        None,
    ),
    tokenizer_case(
        "tok-before-attr-value-transitions",
        "tok-before-attr-value-transitions",
        FixtureState::Existing,
        anchors::ATTRIBUTE_RULES_BASELINE,
        None,
    ),
    tokenizer_case(
        "tok-attr-value-quoted",
        "tok-attr-value-quoted",
        FixtureState::Existing,
        anchors::ATTRIBUTE_RULES_BASELINE,
        None,
    ),
    tokenizer_case(
        "tok-attr-value-unquoted",
        "tok-attr-value-unquoted",
        FixtureState::Existing,
        anchors::ATTRIBUTE_RULES_BASELINE,
        None,
    ),
    tokenizer_case(
        "tok-comment-core",
        "tok-comment-core",
        FixtureState::Existing,
        anchors::TOKENIZER_STATE_FAMILIES,
        None,
    ),
    tokenizer_case(
        "tok-bogus-comment",
        "tok-bogus-comment",
        FixtureState::Existing,
        anchors::TOKENIZER_STATE_FAMILIES,
        None,
    ),
    tokenizer_case(
        "tok-doctype-core",
        "tok-doctype-core",
        FixtureState::Existing,
        anchors::DOCTYPE_AND_QUIRKS_STANCE,
        None,
    ),
    tokenizer_case(
        "tok-doctype-public-system",
        "tok-doctype-public-system",
        FixtureState::Existing,
        anchors::DOCTYPE_AND_QUIRKS_STANCE,
        None,
    ),
    tokenizer_case(
        "tok-doctype-quirks-missing-name",
        "tok-doctype-quirks-missing-name",
        FixtureState::Existing,
        anchors::DOCTYPE_AND_QUIRKS_STANCE,
        None,
    ),
    tokenizer_case(
        "tok-charrefs-text",
        "tok-charrefs-text",
        FixtureState::Existing,
        anchors::TOKENIZER_STATE_FAMILIES,
        None,
    ),
    tokenizer_case(
        "tok-charrefs-attr",
        "tok-charrefs-attr",
        FixtureState::Existing,
        anchors::TOKENIZER_STATE_FAMILIES,
        None,
    ),
    tokenizer_case(
        "tok-chunked-split-boundaries",
        "tok-simple-tags",
        FixtureState::Existing,
        anchors::INPUT_AND_STREAMING_MODEL,
        Some(
            "Uses tok-simple-tags as the canonical chunking equivalence probe for tokenizer until a dedicated tok-chunked-split-boundaries fixture lands.",
        ),
    ),
    tokenizer_case(
        "tok-malformed-recovery",
        "tok-doctype-comment-smoke",
        FixtureState::Existing,
        anchors::UNSPECIFIED_BEHAVIOR_HANDLING,
        None,
    ),
    tokenizer_case(
        "tok-output-format",
        "tok-empty-eof",
        FixtureState::Existing,
        anchors::CORE_V0_GATE_AND_EVIDENCE_MODEL,
        None,
    ),
];

const TREE_BUILDER_ACCEPTANCE_CASES: &[AcceptanceCase] = &[
    tree_builder_case(
        "tb-initial-doctype",
        "tb-initial-doctype",
        FixtureState::MissingByDesign,
        anchors::TREE_BUILDER_MODES_AND_ALGORITHMS,
        None,
    ),
    tree_builder_case(
        "tb-before-html-implicit-root",
        "tb-before-html-implicit-root",
        FixtureState::MissingByDesign,
        anchors::TREE_BUILDER_MODES_AND_ALGORITHMS,
        None,
    ),
    tree_builder_case(
        "tb-before-head-implicit-head",
        "tb-before-head-implicit-head",
        FixtureState::MissingByDesign,
        anchors::TREE_BUILDER_MODES_AND_ALGORITHMS,
        None,
    ),
    tree_builder_case(
        "tb-in-head-core",
        "tb-in-head-core",
        FixtureState::MissingByDesign,
        anchors::SUPPORTED_TAGS_AND_CONTEXTS_BASELINE,
        None,
    ),
    tree_builder_case(
        "tb-after-head-body-bootstrap",
        "tb-after-head-body-bootstrap",
        FixtureState::MissingByDesign,
        anchors::TREE_BUILDER_MODES_AND_ALGORITHMS,
        None,
    ),
    tree_builder_case(
        "tb-in-body-core",
        "tb-in-body-core",
        FixtureState::MissingByDesign,
        anchors::TREE_BUILDER_MODES_AND_ALGORITHMS,
        None,
    ),
    tree_builder_case(
        "tb-soe-core",
        "tb-soe-core",
        FixtureState::MissingByDesign,
        anchors::TREE_BUILDER_MODES_AND_ALGORITHMS,
        None,
    ),
    tree_builder_case(
        "tb-afe-core",
        "tb-afe-core",
        FixtureState::MissingByDesign,
        anchors::TREE_BUILDER_MODES_AND_ALGORITHMS,
        None,
    ),
    tree_builder_case(
        "tb-reprocess-core",
        "tb-reprocess-core",
        FixtureState::MissingByDesign,
        anchors::TREE_BUILDER_MODES_AND_ALGORITHMS,
        None,
    ),
    tree_builder_case(
        "tb-quirks-from-doctype",
        "tb-quirks-from-doctype",
        FixtureState::MissingByDesign,
        anchors::DOCTYPE_AND_QUIRKS_STANCE,
        None,
    ),
    tree_builder_case(
        "tb-table-tags-dont-explode",
        "tb-table-tags-dont-explode",
        FixtureState::MissingByDesign,
        anchors::TABLES_STANCE,
        None,
    ),
    tree_builder_case(
        "tb-chunked-split-boundaries",
        "tb-in-body-core",
        FixtureState::MissingByDesign,
        anchors::INPUT_AND_STREAMING_MODEL,
        Some(
            "Uses tb-in-body-core as the canonical chunking equivalence probe for tree builder until a dedicated tb-chunked-split-boundaries fixture lands.",
        ),
    ),
    tree_builder_case(
        "tb-simple-element-smoke",
        "simple-element",
        FixtureState::Existing,
        anchors::UNSPECIFIED_BEHAVIOR_HANDLING,
        Some(
            "Uses existing simple-element fixture for smoke malformed-recovery baseline until dedicated tb-simple-element-smoke directory lands.",
        ),
    ),
    tree_builder_case(
        "tb-output-format",
        "tb-in-body-core",
        FixtureState::MissingByDesign,
        anchors::CORE_V0_GATE_AND_EVIDENCE_MODEL,
        None,
    ),
];

fn assert_inventory_is_well_formed(expected_kind: AcceptanceKind, cases: &[AcceptanceCase]) {
    let mut ids = BTreeSet::new();
    for case in cases {
        assert_eq!(
            case.kind, expected_kind,
            "inventory case kind mismatch for {}",
            case.acceptance_id
        );
        assert!(
            ids.insert(case.acceptance_id),
            "duplicate acceptance id in grouped inventory: {}",
            case.acceptance_id
        );
        assert_acceptance_case_is_well_formed(case);
    }
}

#[test]
fn tokenizer_acceptance_inventory_is_well_formed() {
    assert_inventory_is_well_formed(AcceptanceKind::Tokenizer, TOKENIZER_ACCEPTANCE_CASES);
}

#[test]
fn tree_builder_acceptance_inventory_is_well_formed() {
    assert_inventory_is_well_formed(AcceptanceKind::TreeBuilder, TREE_BUILDER_ACCEPTANCE_CASES);
}

#[test]
fn acceptance_inventory_ids_are_unique_across_kinds() {
    let mut ids = BTreeSet::new();
    for case in TOKENIZER_ACCEPTANCE_CASES
        .iter()
        .chain(TREE_BUILDER_ACCEPTANCE_CASES.iter())
    {
        assert!(
            ids.insert(case.acceptance_id),
            "duplicate acceptance id across inventories: {}",
            case.acceptance_id
        );
    }
}
