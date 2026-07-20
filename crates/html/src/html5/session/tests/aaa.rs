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
            "#dom-snapshot-v2".to_string(),
            "#document".to_string(),
            "  <!doctype html>".to_string(),
            "  element ns=html local=\"html\" attrs=[]".to_string(),
            "    element ns=html local=\"head\" attrs=[]".to_string(),
            "    element ns=html local=\"body\" attrs=[]".to_string(),
            "      element ns=html local=\"b\" attrs=[]".to_string(),
            "        element ns=html local=\"i\" attrs=[]".to_string(),
            "          \"one\"".to_string(),
            "      element ns=html local=\"i\" attrs=[]".to_string(),
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
        create_counts.get(&PatchKey(7)),
        Some(&1),
        "moved furthest block should retain its original PatchKey"
    );
    assert_eq!(
        create_counts.get(&PatchKey(8)),
        Some(&1),
        "moved text should retain its original PatchKey"
    );
    assert!(
        whole.contains(&DomPatch::AppendChild {
            parent: PatchKey(5),
            child: PatchKey(7),
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
            "#dom-snapshot-v2".to_string(),
            "#document".to_string(),
            "  <!doctype html>".to_string(),
            "  element ns=html local=\"html\" attrs=[]".to_string(),
            "    element ns=html local=\"head\" attrs=[]".to_string(),
            "    element ns=html local=\"body\" attrs=[]".to_string(),
            "      element ns=html local=\"a\" attrs=[]".to_string(),
            "      element ns=html local=\"p\" attrs=[]".to_string(),
            "        element ns=html local=\"a\" attrs=[]".to_string(),
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
        create_counts.get(&PatchKey(10)),
        Some(&1),
        "foster-parented node should retain its original PatchKey"
    );
    assert_eq!(
        create_counts.get(&PatchKey(11)),
        Some(&1),
        "moved text should retain its original PatchKey"
    );
    assert!(
        whole.contains(&DomPatch::InsertBefore {
            parent: PatchKey(5),
            child: PatchKey(10),
            before: PatchKey(6),
        }),
        "AAA foster-parent reparenting must emit the canonical InsertBefore move"
    );
    assert!(
        whole.contains(&DomPatch::AppendChild {
            parent: PatchKey(10),
            child: PatchKey(11),
        }),
        "text insertion must use the reconstructed non-table current node"
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
            "#dom-snapshot-v2".to_string(),
            "#document".to_string(),
            "  <!doctype html>".to_string(),
            "  element ns=html local=\"html\" attrs=[]".to_string(),
            "    element ns=html local=\"head\" attrs=[]".to_string(),
            "    element ns=html local=\"body\" attrs=[]".to_string(),
            "      element ns=html local=\"a\" attrs=[]".to_string(),
            "      element ns=html local=\"a\" attrs=[]".to_string(),
            "        \"x\"".to_string(),
            "      element ns=html local=\"table\" attrs=[]".to_string(),
            "        element ns=html local=\"tbody\" attrs=[]".to_string(),
            "          element ns=html local=\"tr\" attrs=[]".to_string(),
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
