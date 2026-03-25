use super::support::{
    assert_no_remove_node_moves, create_count_by_key, run_session_collect_patches,
};
use crate::dom_patch::{DomPatch, PatchKey};

#[test]
fn session_integrated_aaa_misnested_formatting_is_chunk_equivalent() {
    let whole =
        run_session_collect_patches(&["<!doctype html><b><i>one</b>two</i>"], "AAA misnesting");
    let chunked = run_session_collect_patches(
        &["<!doctype html><b>", "<i>one</b>", "two</i>"],
        "AAA misnesting",
    );

    assert_eq!(
        whole, chunked,
        "integrated AAA end-tag dispatch must preserve exact patch order and key allocation across chunking"
    );

    #[cfg(feature = "dom-snapshot")]
    {
        let whole_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
                .expect("whole AAA misnesting patches should materialize");
        let chunked_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
                .expect("chunked AAA misnesting patches should materialize");

        let whole_lines = crate::html5::serialize_dom_for_test(&whole_dom);
        let chunked_lines = crate::html5::serialize_dom_for_test(&chunked_dom);
        let expected_lines = vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <b>".to_string(),
            "        <i>".to_string(),
            "          \"one\"".to_string(),
            "      <i>".to_string(),
            "        \"two\"".to_string(),
        ];

        assert_eq!(
            whole_lines, expected_lines,
            "whole-input integrated AAA misnesting should build the expected DOM"
        );
        assert_eq!(
            chunked_lines, expected_lines,
            "chunked integrated AAA misnesting should build the expected DOM"
        );
    }
}

#[test]
fn session_aaa_furthest_block_reparenting_is_chunk_equivalent() {
    let whole = run_session_collect_patches(
        &["<!doctype html><a><p>one</a>"],
        "AAA furthest-block reparenting",
    );
    let chunked = run_session_collect_patches(
        &["<!doctype html><a>", "<p>one</a>"],
        "AAA furthest-block reparenting",
    );

    assert_eq!(
        whole, chunked,
        "AAA furthest-block reparenting must preserve exact patch order and key allocation across chunking"
    );

    let create_counts = create_count_by_key(&whole);
    assert_no_remove_node_moves(&whole, "AAA furthest-block reparenting");
    assert_eq!(
        create_counts.get(&PatchKey(6)),
        Some(&1),
        "moved furthest block should retain its original PatchKey"
    );
    assert_eq!(
        create_counts.get(&PatchKey(7)),
        Some(&1),
        "moved text should retain its original PatchKey"
    );
    assert!(
        whole.contains(&DomPatch::AppendChild {
            parent: PatchKey(4),
            child: PatchKey(6),
        }),
        "AAA furthest-block reparenting must emit the canonical AppendChild move"
    );

    #[cfg(feature = "dom-snapshot")]
    {
        let whole_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
                .expect("whole AAA furthest-block patches should materialize");
        let chunked_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
                .expect("chunked AAA furthest-block patches should materialize");

        let expected_lines = vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <a>".to_string(),
            "      <p>".to_string(),
            "        <a>".to_string(),
            "          \"one\"".to_string(),
        ];

        assert_eq!(
            crate::html5::serialize_dom_for_test(&whole_dom),
            expected_lines,
            "whole-input AAA furthest-block reparenting should build the expected DOM"
        );
        assert_eq!(
            crate::html5::serialize_dom_for_test(&chunked_dom),
            expected_lines,
            "chunked AAA furthest-block reparenting should build the expected DOM"
        );
    }
}

#[test]
fn session_aaa_foster_parent_insert_before_is_chunk_equivalent() {
    let whole = run_session_collect_patches(
        &["<!doctype html><table><a><tr>x</a>"],
        "AAA foster-parent reparenting",
    );
    let chunked = run_session_collect_patches(
        &["<!doctype html><table><a>", "<tr>x</a>"],
        "AAA foster-parent reparenting",
    );

    assert_eq!(
        whole, chunked,
        "AAA foster-parent reparenting must preserve exact patch order and key allocation across chunking"
    );

    let create_counts = create_count_by_key(&whole);
    assert_no_remove_node_moves(&whole, "AAA foster-parent reparenting");
    assert_eq!(
        create_counts.get(&PatchKey(9)),
        Some(&1),
        "foster-parented node should retain its original PatchKey"
    );
    assert_eq!(
        create_counts.get(&PatchKey(10)),
        Some(&1),
        "moved text should retain its original PatchKey"
    );
    assert!(
        whole.contains(&DomPatch::InsertBefore {
            parent: PatchKey(4),
            child: PatchKey(9),
            before: PatchKey(5),
        }),
        "AAA foster-parent reparenting must emit the canonical InsertBefore move"
    );

    #[cfg(feature = "dom-snapshot")]
    {
        let whole_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
                .expect("whole AAA foster-parent patches should materialize");
        let chunked_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
                .expect("chunked AAA foster-parent patches should materialize");

        let expected_lines = vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <a>".to_string(),
            "      <a>".to_string(),
            "      \"x\"".to_string(),
            "      <table>".to_string(),
            "        <tbody>".to_string(),
            "          <tr>".to_string(),
        ];

        assert_eq!(
            crate::html5::serialize_dom_for_test(&whole_dom),
            expected_lines,
            "whole-input AAA foster-parent reparenting should build the expected DOM"
        );
        assert_eq!(
            crate::html5::serialize_dom_for_test(&chunked_dom),
            expected_lines,
            "chunked AAA foster-parent reparenting should build the expected DOM"
        );
    }
}
