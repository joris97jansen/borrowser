use std::fs;
use std::path::Path;

use super::support::{collect_contract_ids, collect_matrix_ids, parse_tokens_headers, repo_root};

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
