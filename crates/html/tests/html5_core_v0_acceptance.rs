#![cfg(all(feature = "html5", feature = "test-harness"))]

//! Core-v0 contract inventory scaffold.
//!
//! Most entries in this file are intentionally ignored skeletons that map
//! acceptance IDs to evidence locations and contract anchors. Non-ignored
//! tests are the live subset of contract enforcement promoted from inventory
//! to CI-backed assertions.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const CONTRACT_DOC: &str = "docs/html5/html5-core-v0.md";

const ANCHOR_INPUT_AND_STREAMING_MODEL: &str = "input-and-streaming-model";
const ANCHOR_TOKENIZER_STATE_FAMILIES: &str = "tokenizer-state-families";
const ANCHOR_TREE_BUILDER_MODES_AND_ALGORITHMS: &str = "tree-builder-modes-and-algorithms";
const ANCHOR_SUPPORTED_TAGS_AND_CONTEXTS_BASELINE: &str = "supported-tags-and-contexts-baseline";
const ANCHOR_ATTRIBUTE_RULES_BASELINE: &str = "attribute-rules-baseline";
const ANCHOR_DOCTYPE_AND_QUIRKS_STANCE: &str = "doctype-and-quirks-stance";
const ANCHOR_TABLES_STANCE: &str = "tables-stance";
const ANCHOR_UNSPECIFIED_BEHAVIOR_HANDLING: &str =
    "unspecified-behavior-handling-fail-safe-contract";
const ANCHOR_CORE_V0_GATE_AND_EVIDENCE_MODEL: &str = "core-v0-gate-and-evidence-model";

#[derive(Clone, Copy, Debug)]
enum ExpectedOutput {
    TokensV1,
    DomV1,
}

#[derive(Clone, Copy, Debug)]
enum FixtureState {
    Existing,
    MissingByDesign,
}

#[derive(Clone, Copy, Debug)]
enum AcceptanceKind {
    Tokenizer,
    TreeBuilder,
}

fn expected_format(output: ExpectedOutput) -> &'static str {
    match output {
        ExpectedOutput::TokensV1 => "html5-token-v1",
        ExpectedOutput::DomV1 => "html5-dom-v1",
    }
}

fn fixture_state_label(state: FixtureState) -> &'static str {
    match state {
        FixtureState::Existing => "existing",
        FixtureState::MissingByDesign => "missing-by-design",
    }
}

fn kind_label(kind: AcceptanceKind) -> &'static str {
    match kind {
        AcceptanceKind::Tokenizer => "tokenizer",
        AcceptanceKind::TreeBuilder => "tree_builder",
    }
}

fn kind_fixture_subdir(kind: AcceptanceKind) -> &'static str {
    match kind {
        AcceptanceKind::Tokenizer => "tokenizer",
        AcceptanceKind::TreeBuilder => "tree_builder",
    }
}

fn fixture_dir(kind_subdir: &str, fixture_id: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("html5")
        .join(kind_subdir)
        .join(fixture_id)
}

fn evidence_location<'a>(kind: AcceptanceKind, evidence_ref: &'a str) -> Cow<'a, str> {
    Cow::Owned(
        fixture_dir(kind_fixture_subdir(kind), evidence_ref)
            .display()
            .to_string(),
    )
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate manifest dir should resolve to repo root")
        .to_path_buf()
}

fn parse_tokens_headers(path: &Path) -> (BTreeMap<String, String>, Vec<String>) {
    let content =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path:?}: {err}"));
    let mut headers = BTreeMap::new();
    let mut lines = Vec::new();
    for raw_line in content.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            continue;
        }
        if let Some(stripped) = line.strip_prefix('#') {
            let header = stripped.trim();
            if header.is_empty() {
                continue;
            }
            let (key, value) = header
                .split_once(':')
                .unwrap_or_else(|| panic!("invalid header in {path:?}: '{line}'"));
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            assert!(
                headers.insert(key.clone(), value).is_none(),
                "duplicate header '{key}' in {path:?}"
            );
        } else {
            lines.push(line.to_string());
        }
    }
    (headers, lines)
}

fn collect_contract_ids(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut ids = BTreeSet::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let candidate = if bytes[i..].starts_with(b"TOK-") {
            Some("TOK-")
        } else if bytes[i..].starts_with(b"TB-") {
            Some("TB-")
        } else {
            None
        };
        let Some(_prefix) = candidate else {
            i += 1;
            continue;
        };

        let start = i;
        let mut end = i;
        while end < bytes.len() {
            let b = bytes[end];
            if b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-' {
                end += 1;
            } else {
                break;
            }
        }
        if end > start + 4 {
            ids.insert(text[start..end].to_string());
        }
        i = end;
    }
    ids
}

fn collect_matrix_ids(path: &Path) -> BTreeSet<String> {
    let content =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path:?}: {err}"));
    let mut ids = BTreeSet::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let mut cells = trimmed.split('|');
        let _leading = cells.next();
        let Some(first_cell) = cells.next() else {
            continue;
        };
        let cell = first_cell.trim();
        let Some(cell) = cell
            .strip_prefix('`')
            .and_then(|cell| cell.strip_suffix('`'))
        else {
            continue;
        };
        if cell.starts_with("TOK-") || cell.starts_with("TB-") {
            ids.insert(cell.to_string());
        }
    }
    ids
}

fn validate_kind_output(kind: AcceptanceKind, output: ExpectedOutput) {
    match (kind, output) {
        (AcceptanceKind::Tokenizer, ExpectedOutput::TokensV1)
        | (AcceptanceKind::TreeBuilder, ExpectedOutput::DomV1) => {}
        (AcceptanceKind::Tokenizer, ExpectedOutput::DomV1) => {
            panic!("PENDING_ACCEPTANCE_CONFIG: tokenizer acceptance must use TokensV1 output")
        }
        (AcceptanceKind::TreeBuilder, ExpectedOutput::TokensV1) => {
            panic!("PENDING_ACCEPTANCE_CONFIG: tree-builder acceptance must use DomV1 output")
        }
    }
}

fn pending_acceptance(
    kind: AcceptanceKind,
    acceptance_id: &str,
    evidence_ref: &str,
    fixture_state: FixtureState,
    contract_anchor: &'static str,
    output: ExpectedOutput,
    note: Option<&str>,
) -> ! {
    validate_kind_output(kind, output);
    let evidence_location = evidence_location(kind, evidence_ref);
    let evidence_mapping = if acceptance_id == evidence_ref {
        evidence_ref.to_string()
    } else {
        format!("{acceptance_id} -> {evidence_ref}")
    };
    let note = note.unwrap_or("none");
    panic!(
        "PENDING_ACCEPTANCE: D4 acceptance skeleton pending implementation.\n\
         acceptance_id: {acceptance_id}\n\
         kind: {}\n\
         contract: {CONTRACT_DOC}#{contract_anchor}\n\
         evidence_mapping: {evidence_mapping}\n\
         fixture_state: {}\n\
         expected_output_format: {}\n\
         evidence_location: {evidence_location}\n\
         note: {note}\n\
         todo: implement whole-input and UTF-8 chunked assertions.",
        kind_label(kind),
        fixture_state_label(fixture_state),
        expected_format(output),
    );
}

macro_rules! pending_test {
    ($name:ident, $kind:expr, $acceptance_id:literal, $evidence_ref:literal, $fixture_state:expr, $anchor:expr, $output:expr) => {
        #[test]
        #[ignore = "D4 skeleton: acceptance definition only; implement assertions when parser behavior lands"]
        fn $name() {
            pending_acceptance(
                $kind,
                $acceptance_id,
                $evidence_ref,
                $fixture_state,
                $anchor,
                $output,
                None,
            );
        }
    };
    ($name:ident, $kind:expr, $acceptance_id:literal, $evidence_ref:literal, $fixture_state:expr, $anchor:expr, $output:expr, $note:literal) => {
        #[test]
        #[ignore = "D4 skeleton: acceptance definition only; implement assertions when parser behavior lands"]
        fn $name() {
            pending_acceptance(
                $kind,
                $acceptance_id,
                $evidence_ref,
                $fixture_state,
                $anchor,
                $output,
                Some($note),
            );
        }
    };
}

// Expected output definition for Core v0:
// - Tokenizer acceptance uses token snapshots in `html5-token-v1`.
// - Tree-builder acceptance uses DOM snapshots in `html5-dom-v1`.
// - Patch logs remain debug material and are not the primary acceptance oracle.

// -------------------------
// Tokenizer Core v0 coverage
// -------------------------

pending_test!(
    tok_empty_eof_contract,
    AcceptanceKind::Tokenizer,
    "tok-empty-eof",
    "tok-empty-eof",
    FixtureState::Existing,
    ANCHOR_TOKENIZER_STATE_FAMILIES,
    ExpectedOutput::TokensV1
);

pending_test!(
    tok_basic_text_contract,
    AcceptanceKind::Tokenizer,
    "tok-basic-text",
    "tok-basic-text",
    FixtureState::Existing,
    ANCHOR_TOKENIZER_STATE_FAMILIES,
    ExpectedOutput::TokensV1
);

pending_test!(
    tok_simple_tags_contract,
    AcceptanceKind::Tokenizer,
    "tok-simple-tags",
    "tok-simple-tags",
    FixtureState::Existing,
    ANCHOR_TOKENIZER_STATE_FAMILIES,
    ExpectedOutput::TokensV1
);

pending_test!(
    tok_attrs_core_contract,
    AcceptanceKind::Tokenizer,
    "tok-attrs-core",
    "tok-attrs-core",
    FixtureState::Existing,
    ANCHOR_ATTRIBUTE_RULES_BASELINE,
    ExpectedOutput::TokensV1
);

pending_test!(
    tok_before_attr_value_transitions_contract,
    AcceptanceKind::Tokenizer,
    "tok-before-attr-value-transitions",
    "tok-before-attr-value-transitions",
    FixtureState::Existing,
    ANCHOR_ATTRIBUTE_RULES_BASELINE,
    ExpectedOutput::TokensV1
);

pending_test!(
    tok_attr_value_quoted_contract,
    AcceptanceKind::Tokenizer,
    "tok-attr-value-quoted",
    "tok-attr-value-quoted",
    FixtureState::Existing,
    ANCHOR_ATTRIBUTE_RULES_BASELINE,
    ExpectedOutput::TokensV1
);

pending_test!(
    tok_attr_value_unquoted_contract,
    AcceptanceKind::Tokenizer,
    "tok-attr-value-unquoted",
    "tok-attr-value-unquoted",
    FixtureState::Existing,
    ANCHOR_ATTRIBUTE_RULES_BASELINE,
    ExpectedOutput::TokensV1
);

pending_test!(
    tok_comment_core_contract,
    AcceptanceKind::Tokenizer,
    "tok-comment-core",
    "tok-comment-core",
    FixtureState::Existing,
    ANCHOR_TOKENIZER_STATE_FAMILIES,
    ExpectedOutput::TokensV1
);

pending_test!(
    tok_bogus_comment_contract,
    AcceptanceKind::Tokenizer,
    "tok-bogus-comment",
    "tok-bogus-comment",
    FixtureState::Existing,
    ANCHOR_TOKENIZER_STATE_FAMILIES,
    ExpectedOutput::TokensV1
);

pending_test!(
    tok_doctype_core_contract,
    AcceptanceKind::Tokenizer,
    "tok-doctype-core",
    "tok-doctype-core",
    FixtureState::Existing,
    ANCHOR_DOCTYPE_AND_QUIRKS_STANCE,
    ExpectedOutput::TokensV1
);

pending_test!(
    tok_doctype_public_system_contract,
    AcceptanceKind::Tokenizer,
    "tok-doctype-public-system",
    "tok-doctype-public-system",
    FixtureState::Existing,
    ANCHOR_DOCTYPE_AND_QUIRKS_STANCE,
    ExpectedOutput::TokensV1
);

pending_test!(
    tok_doctype_quirks_missing_name_contract,
    AcceptanceKind::Tokenizer,
    "tok-doctype-quirks-missing-name",
    "tok-doctype-quirks-missing-name",
    FixtureState::Existing,
    ANCHOR_DOCTYPE_AND_QUIRKS_STANCE,
    ExpectedOutput::TokensV1
);

pending_test!(
    tok_charrefs_text_contract,
    AcceptanceKind::Tokenizer,
    "tok-charrefs-text",
    "tok-charrefs-text",
    FixtureState::Existing,
    ANCHOR_TOKENIZER_STATE_FAMILIES,
    ExpectedOutput::TokensV1
);

pending_test!(
    tok_charrefs_attr_contract,
    AcceptanceKind::Tokenizer,
    "tok-charrefs-attr",
    "tok-charrefs-attr",
    FixtureState::Existing,
    ANCHOR_TOKENIZER_STATE_FAMILIES,
    ExpectedOutput::TokensV1
);

pending_test!(
    tok_chunked_split_boundaries_contract,
    AcceptanceKind::Tokenizer,
    "tok-chunked-split-boundaries",
    "tok-simple-tags",
    FixtureState::Existing,
    ANCHOR_INPUT_AND_STREAMING_MODEL,
    ExpectedOutput::TokensV1,
    "Uses tok-simple-tags as the canonical chunking equivalence probe for tokenizer until a dedicated tok-chunked-split-boundaries fixture lands."
);

pending_test!(
    tok_malformed_recovery_contract,
    AcceptanceKind::Tokenizer,
    "tok-malformed-recovery",
    "tok-doctype-comment-smoke",
    FixtureState::Existing,
    ANCHOR_UNSPECIFIED_BEHAVIOR_HANDLING,
    ExpectedOutput::TokensV1
);

pending_test!(
    tok_output_format_contract,
    AcceptanceKind::Tokenizer,
    "tok-output-format",
    "tok-empty-eof",
    FixtureState::Existing,
    ANCHOR_CORE_V0_GATE_AND_EVIDENCE_MODEL,
    ExpectedOutput::TokensV1
);

#[test]
fn tok_script_data_escaped_comment_family_fixture_is_active() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("html5")
        .join("tokenizer")
        .join("tok-script-data-escaped-comment-family");
    assert!(
        fixture_dir.is_dir(),
        "missing escaped script-data fixture dir: {fixture_dir:?}"
    );

    let input_path = fixture_dir.join("input.html");
    let tokens_path = fixture_dir.join("tokens.txt");
    assert!(
        input_path.is_file(),
        "missing escaped script-data fixture input: {input_path:?}"
    );
    assert!(
        tokens_path.is_file(),
        "missing escaped script-data fixture tokens: {tokens_path:?}"
    );

    let (headers, lines) = parse_tokens_headers(&tokens_path);
    assert_eq!(
        headers.get("format").map(String::as_str),
        Some("html5-token-v1"),
        "escaped script-data fixture tokens must use html5-token-v1 format"
    );
    assert_eq!(
        headers.get("status").map(String::as_str),
        None,
        "escaped script-data fixture should be active now that G5 landed"
    );
    assert!(
        !lines.is_empty(),
        "escaped script-data fixture must contain actual token lines"
    );
    assert_eq!(
        lines.first().map(String::as_str),
        Some("START name=script attrs=[] self_closing=false")
    );
    assert_eq!(lines.last().map(String::as_str), Some("EOF"));
    assert!(
        lines
            .iter()
            .any(|line| line.contains("document.write(\\\"<script>nested</script>\\\")")),
        "escaped script-data fixture should preserve nested script text"
    );
}

// ---------------------------
// Tree-builder Core v0 coverage
// ---------------------------

pending_test!(
    tb_initial_doctype_contract,
    AcceptanceKind::TreeBuilder,
    "tb-initial-doctype",
    "tb-initial-doctype",
    FixtureState::MissingByDesign,
    ANCHOR_TREE_BUILDER_MODES_AND_ALGORITHMS,
    ExpectedOutput::DomV1
);

pending_test!(
    tb_before_html_implicit_root_contract,
    AcceptanceKind::TreeBuilder,
    "tb-before-html-implicit-root",
    "tb-before-html-implicit-root",
    FixtureState::MissingByDesign,
    ANCHOR_TREE_BUILDER_MODES_AND_ALGORITHMS,
    ExpectedOutput::DomV1
);

pending_test!(
    tb_before_head_implicit_head_contract,
    AcceptanceKind::TreeBuilder,
    "tb-before-head-implicit-head",
    "tb-before-head-implicit-head",
    FixtureState::MissingByDesign,
    ANCHOR_TREE_BUILDER_MODES_AND_ALGORITHMS,
    ExpectedOutput::DomV1
);

pending_test!(
    tb_in_head_core_contract,
    AcceptanceKind::TreeBuilder,
    "tb-in-head-core",
    "tb-in-head-core",
    FixtureState::MissingByDesign,
    ANCHOR_SUPPORTED_TAGS_AND_CONTEXTS_BASELINE,
    ExpectedOutput::DomV1
);

pending_test!(
    tb_after_head_body_bootstrap_contract,
    AcceptanceKind::TreeBuilder,
    "tb-after-head-body-bootstrap",
    "tb-after-head-body-bootstrap",
    FixtureState::MissingByDesign,
    ANCHOR_TREE_BUILDER_MODES_AND_ALGORITHMS,
    ExpectedOutput::DomV1
);

pending_test!(
    tb_in_body_core_contract,
    AcceptanceKind::TreeBuilder,
    "tb-in-body-core",
    "tb-in-body-core",
    FixtureState::MissingByDesign,
    ANCHOR_TREE_BUILDER_MODES_AND_ALGORITHMS,
    ExpectedOutput::DomV1
);

pending_test!(
    tb_soe_core_contract,
    AcceptanceKind::TreeBuilder,
    "tb-soe-core",
    "tb-soe-core",
    FixtureState::MissingByDesign,
    ANCHOR_TREE_BUILDER_MODES_AND_ALGORITHMS,
    ExpectedOutput::DomV1
);

pending_test!(
    tb_afe_core_contract,
    AcceptanceKind::TreeBuilder,
    "tb-afe-core",
    "tb-afe-core",
    FixtureState::MissingByDesign,
    ANCHOR_TREE_BUILDER_MODES_AND_ALGORITHMS,
    ExpectedOutput::DomV1
);

pending_test!(
    tb_reprocess_core_contract,
    AcceptanceKind::TreeBuilder,
    "tb-reprocess-core",
    "tb-reprocess-core",
    FixtureState::MissingByDesign,
    ANCHOR_TREE_BUILDER_MODES_AND_ALGORITHMS,
    ExpectedOutput::DomV1
);

pending_test!(
    tb_quirks_from_doctype_contract,
    AcceptanceKind::TreeBuilder,
    "tb-quirks-from-doctype",
    "tb-quirks-from-doctype",
    FixtureState::MissingByDesign,
    ANCHOR_DOCTYPE_AND_QUIRKS_STANCE,
    ExpectedOutput::DomV1
);

pending_test!(
    tb_table_tags_dont_explode_contract,
    AcceptanceKind::TreeBuilder,
    "tb-table-tags-dont-explode",
    "tb-table-tags-dont-explode",
    FixtureState::MissingByDesign,
    ANCHOR_TABLES_STANCE,
    ExpectedOutput::DomV1
);

pending_test!(
    tb_chunked_split_boundaries_contract,
    AcceptanceKind::TreeBuilder,
    "tb-chunked-split-boundaries",
    "tb-in-body-core",
    FixtureState::MissingByDesign,
    ANCHOR_INPUT_AND_STREAMING_MODEL,
    ExpectedOutput::DomV1,
    "Uses tb-in-body-core as the canonical chunking equivalence probe for tree builder until a dedicated tb-chunked-split-boundaries fixture lands."
);

pending_test!(
    tb_malformed_recovery_contract,
    AcceptanceKind::TreeBuilder,
    "tb-simple-element-smoke",
    "simple-element",
    FixtureState::Existing,
    ANCHOR_UNSPECIFIED_BEHAVIOR_HANDLING,
    ExpectedOutput::DomV1,
    "Uses existing simple-element fixture for smoke malformed-recovery baseline until dedicated tb-simple-element-smoke directory lands."
);

pending_test!(
    tb_output_format_contract,
    AcceptanceKind::TreeBuilder,
    "tb-output-format",
    "tb-in-body-core",
    FixtureState::MissingByDesign,
    ANCHOR_CORE_V0_GATE_AND_EVIDENCE_MODEL,
    ExpectedOutput::DomV1
);

#[test]
fn policy_id_drift_guard_matches_matrix_id_columns() {
    let repo_root = repo_root();
    let contract_path = repo_root
        .join("docs")
        .join("html5")
        .join("html5-core-v0.md");
    let tokenizer_matrix_path = repo_root
        .join("docs")
        .join("html5")
        .join("spec-matrix-tokenizer.md");
    let tree_builder_matrix_path = repo_root
        .join("docs")
        .join("html5")
        .join("spec-matrix-treebuilder.md");

    let contract_text = fs::read_to_string(&contract_path)
        .unwrap_or_else(|err| panic!("failed to read {contract_path:?}: {err}"));
    let referenced_ids = collect_contract_ids(&contract_text);
    assert!(
        !referenced_ids.is_empty(),
        "expected html5-core-v0.md to reference TOK-/TB-* IDs"
    );

    let mut matrix_ids = collect_matrix_ids(&tokenizer_matrix_path);
    matrix_ids.extend(collect_matrix_ids(&tree_builder_matrix_path));
    assert!(
        !matrix_ids.is_empty(),
        "expected tokenizer/tree-builder matrices to expose TOK-/TB-* ID columns"
    );

    let missing = referenced_ids
        .difference(&matrix_ids)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "contract drift: ids referenced in html5-core-v0.md are missing from matrix ID columns: {}",
        missing.join(", ")
    );
}
