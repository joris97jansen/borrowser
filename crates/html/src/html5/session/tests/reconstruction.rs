use super::support::run_session_collect_patches;

#[test]
fn session_reconstruction_is_chunk_equivalent_after_generic_ancestor_pop() {
    let whole = run_session_collect_patches(
        &["<!doctype html><div><b class=\"x\">one</div>two"],
        "reconstruction",
    );
    let chunked = run_session_collect_patches(
        &[
            "<!doctype html><div><b class=\"x\">",
            "one</div>",
            "tw",
            "o",
        ],
        "reconstruction",
    );

    assert_eq!(
        whole, chunked,
        "reconstruction must preserve exact patch order and key allocation across chunking"
    );

    #[cfg(feature = "dom-snapshot")]
    {
        let whole_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
                .expect("whole reconstruction patches should materialize");
        let chunked_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
                .expect("chunked reconstruction patches should materialize");

        let whole_lines = crate::html5::serialize_dom_for_test(&whole_dom);
        let chunked_lines = crate::html5::serialize_dom_for_test(&chunked_dom);
        let expected_lines = vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <div>".to_string(),
            "        <b class=\"x\">".to_string(),
            "          \"one\"".to_string(),
            "      <b class=\"x\">".to_string(),
            "        \"two\"".to_string(),
        ];

        assert_eq!(
            whole_lines, expected_lines,
            "whole-input reconstruction should build the expected DOM"
        );
        assert_eq!(
            chunked_lines, expected_lines,
            "chunked reconstruction should build the expected DOM"
        );
    }
}

#[test]
fn session_reconstruction_of_multiple_missing_formatting_elements_is_chunk_equivalent() {
    let whole = run_session_collect_patches(
        &["<!doctype html><div><b class=\"x\"><i class=\"y\">one</div>two"],
        "multi-element reconstruction",
    );
    let chunked = run_session_collect_patches(
        &[
            "<!doctype html><div><b class=\"x\">",
            "<i class=\"y\">one",
            "</div>t",
            "wo",
        ],
        "multi-element reconstruction",
    );

    assert_eq!(
        whole, chunked,
        "multi-element reconstruction must preserve exact patch order and key allocation across chunking"
    );

    #[cfg(feature = "dom-snapshot")]
    {
        let whole_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
                .expect("whole multi-element reconstruction patches should materialize");
        let chunked_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
                .expect("chunked multi-element reconstruction patches should materialize");

        let whole_lines = crate::html5::serialize_dom_for_test(&whole_dom);
        let chunked_lines = crate::html5::serialize_dom_for_test(&chunked_dom);
        let expected_lines = vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <div>".to_string(),
            "        <b class=\"x\">".to_string(),
            "          <i class=\"y\">".to_string(),
            "            \"one\"".to_string(),
            "      <b class=\"x\">".to_string(),
            "        <i class=\"y\">".to_string(),
            "          \"two\"".to_string(),
        ];

        assert_eq!(
            whole_lines, expected_lines,
            "whole-input multi-element reconstruction should build the expected DOM"
        );
        assert_eq!(
            chunked_lines, expected_lines,
            "chunked multi-element reconstruction should build the expected DOM"
        );
    }
}

#[test]
fn session_special_anchor_recovery_is_chunk_equivalent() {
    let whole = run_session_collect_patches(&["<!doctype html><a>one<a>two"], "anchor recovery");
    let chunked =
        run_session_collect_patches(&["<!doctype html><a>", "one<a>", "two"], "anchor recovery");

    assert_eq!(
        whole, chunked,
        "special anchor recovery must preserve exact patch order and key allocation across chunking"
    );

    #[cfg(feature = "dom-snapshot")]
    {
        let whole_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
                .expect("whole anchor recovery patches should materialize");
        let chunked_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
                .expect("chunked anchor recovery patches should materialize");

        let whole_lines = crate::html5::serialize_dom_for_test(&whole_dom);
        let chunked_lines = crate::html5::serialize_dom_for_test(&chunked_dom);
        let expected_lines = vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <a>".to_string(),
            "        \"one\"".to_string(),
            "      <a>".to_string(),
            "        \"two\"".to_string(),
        ];

        assert_eq!(
            whole_lines, expected_lines,
            "whole-input special anchor recovery should build the expected DOM"
        );
        assert_eq!(
            chunked_lines, expected_lines,
            "chunked special anchor recovery should build the expected DOM"
        );
    }
}

#[test]
fn session_special_nobr_recovery_is_chunk_equivalent() {
    let whole =
        run_session_collect_patches(&["<!doctype html><nobr>one<nobr>two"], "nobr recovery");
    let chunked = run_session_collect_patches(
        &["<!doctype html><nobr>", "one<nobr>", "two"],
        "nobr recovery",
    );

    assert_eq!(
        whole, chunked,
        "special nobr recovery must preserve exact patch order and key allocation across chunking"
    );

    #[cfg(feature = "dom-snapshot")]
    {
        let whole_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
                .expect("whole nobr recovery patches should materialize");
        let chunked_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
                .expect("chunked nobr recovery patches should materialize");

        let whole_lines = crate::html5::serialize_dom_for_test(&whole_dom);
        let chunked_lines = crate::html5::serialize_dom_for_test(&chunked_dom);
        let expected_lines = vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <nobr>".to_string(),
            "        \"one\"".to_string(),
            "      <nobr>".to_string(),
            "        \"two\"".to_string(),
        ];

        assert_eq!(
            whole_lines, expected_lines,
            "whole-input special nobr recovery should build the expected DOM"
        );
        assert_eq!(
            chunked_lines, expected_lines,
            "chunked special nobr recovery should build the expected DOM"
        );
    }
}
