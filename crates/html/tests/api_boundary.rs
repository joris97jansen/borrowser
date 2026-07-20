use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/api_boundary")
        .join(name)
        .join("Cargo.toml")
}

fn check_fixture(name: &str, target: &str, extra_args: &[&str]) -> Output {
    let target = std::env::temp_dir()
        .join("borrowser-ae10-api-boundary")
        .join(format!("{}-{}-{}", name, target, std::process::id()));
    let mut command = Command::new(env!("CARGO"));
    command
        .args(["check", "--offline", "--locked", "--manifest-path"])
        .arg(fixture(name));
    command
        .args(extra_args)
        .env("CARGO_TARGET_DIR", target)
        .output()
        .expect("API-boundary cargo check should run")
}

fn check_package(name: &str) -> Output {
    check_fixture(name, "package", &[])
}

fn check_bin(name: &str, bin: &str) -> Output {
    check_fixture(name, bin, &["--bin", bin])
}

fn assert_operation_specific_compile_failure(
    output: Output,
    bin: &str,
    error_code: &str,
    diagnostic: &str,
) {
    assert!(!output.status.success(), "{bin} unexpectedly compiled");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(error_code)
            && stderr.contains(diagnostic)
            && stderr.contains(&format!("src/bin/{bin}.rs")),
        "{bin} must fail for its intended operation:\n{stderr}"
    );
    for unrelated in ["E0432", "E0433", "E0282"] {
        assert!(
            !stderr.contains(unrelated),
            "{bin} failure was masked by unrelated {unrelated}:\n{stderr}"
        );
    }
}

#[test]
fn ordinary_consumer_without_internal_api_compiles() {
    let output = check_package("ordinary_consumer");
    assert!(
        output.status.success(),
        "ordinary consumer must compile without internal-api:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn template_association_mutation_without_internal_api_does_not_compile() {
    let output = check_package("no_internal_mutation");
    assert!(
        !output.status.success(),
        "association mutation probe unexpectedly compiled"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not have a field named `template_contents`")
            || stderr.contains("has no field named `template_contents`"),
        "probe must fail at the opaque association boundary:\n{stderr}"
    );
    assert!(
        stderr.contains("DocumentFragmentNode")
            && stderr.contains("ParserCreatedFragmentKind")
            && stderr.contains("could not find `internal` in `html`"),
        "fragment types and the engine interface must be unavailable without internal-api:\n{stderr}"
    );
}

#[test]
fn internal_engine_consumer_has_controlled_construction_and_read_access() {
    let output = check_package("internal_consumer");
    assert!(
        output.status.success(),
        "internal engine consumer must compile:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn internal_test_harness_consumer_has_read_only_access() {
    let output = check_fixture("internal_read_only_boundary", "library", &["--lib"]);
    assert!(
        output.status.success(),
        "internal test-harness consumer must retain approved construction and read access:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn qualified_attribute_valid_public_construction_remains_available() {
    let output = check_fixture("internal_read_only_boundary", "library", &["--lib"]);
    assert!(
        output.status.success(),
        "valid unqualified construction must compile:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn qualified_attribute_internal_state_is_not_constructible() {
    assert_operation_specific_compile_failure(
        check_bin("internal_read_only_boundary", "qualified_private_state"),
        "qualified_private_state",
        "error[E0451]",
        "field `kind`",
    );
}

#[test]
fn qualified_attribute_foreign_smart_constructors_remain_parser_owned() {
    assert_operation_specific_compile_failure(
        check_bin("internal_read_only_boundary", "qualified_xml_constructor"),
        "qualified_xml_constructor",
        "error[E0624]",
        "associated function `xml` is private",
    );
}

#[test]
fn qualified_attribute_raw_namespace_prefix_bypass_does_not_exist() {
    assert_operation_specific_compile_failure(
        check_bin("internal_read_only_boundary", "qualified_raw_bypass"),
        "qualified_raw_bypass",
        "error[E0599]",
        "no function or associated item named `from_parts`",
    );
}

#[test]
fn qualified_attribute_no_namespace_prefix_shape_is_not_constructible() {
    assert_operation_specific_compile_failure(
        check_bin("internal_read_only_boundary", "qualified_none_with_prefix"),
        "qualified_none_with_prefix",
        "error[E0599]",
        "no function or associated item named `from_parts`",
    );
}

#[test]
fn qualified_attribute_malformed_xmlns_shape_is_not_constructible() {
    assert_operation_specific_compile_failure(
        check_bin("internal_read_only_boundary", "qualified_malformed_xmlns"),
        "qualified_malformed_xmlns",
        "error[E0599]",
        "no function or associated item named `from_parts`",
    );
}

#[test]
fn qualified_attribute_internal_state_cannot_be_mutated() {
    assert_operation_specific_compile_failure(
        check_bin("internal_read_only_boundary", "qualified_state_mutation"),
        "qualified_state_mutation",
        "error[E0616]",
        "field `kind`",
    );
}

#[test]
fn internal_consumer_cannot_mutate_fragment_identity() {
    assert_operation_specific_compile_failure(
        check_bin("internal_read_only_boundary", "fragment_set_id"),
        "fragment_set_id",
        "error[E0624]",
        "method `set_id` is private",
    );
}

#[test]
fn internal_consumer_cannot_mutate_fragment_children() {
    assert_operation_specific_compile_failure(
        check_bin("internal_read_only_boundary", "fragment_children_mut"),
        "fragment_children_mut",
        "error[E0624]",
        "method `children_mut` is private",
    );
}

#[test]
fn internal_consumer_cannot_construct_arbitrary_fragment() {
    assert_operation_specific_compile_failure(
        check_bin("internal_read_only_boundary", "fragment_construction"),
        "fragment_construction",
        "error[E0451]",
        "private",
    );
}

#[test]
fn internal_consumer_cannot_replace_template_association() {
    assert_operation_specific_compile_failure(
        check_bin("internal_read_only_boundary", "association_replacement"),
        "association_replacement",
        "error[E0616]",
        "field `template_contents`",
    );
}

#[test]
fn internal_consumer_has_no_mutable_association_accessor() {
    assert_operation_specific_compile_failure(
        check_bin(
            "internal_read_only_boundary",
            "association_mutable_accessor",
        ),
        "association_mutable_accessor",
        "error[E0425]",
        "template_contents_mut",
    );
}
