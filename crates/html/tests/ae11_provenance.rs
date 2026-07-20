use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const EXPECTED_PROFILE: &str = "AE11-WHATWG-supported-token-profile-v1";
const EXPECTED_WHATWG_COMMIT: &str = "85b40db7c40436be8d459e8f4ca2120e823c34f0";
const EXPECTED_WPT_COMMIT: &str = "e4ea1706fa708c3ac4523c534a65160d1ab20db8";

#[test]
fn ae11_supported_profile_uses_immutable_complete_upstream_revisions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let provenance_path = root.join("tests/wpt/provenance/ae11-supported-profile.provenance.txt");
    let provenance = fs::read_to_string(&provenance_path)
        .unwrap_or_else(|error| panic!("failed to read {provenance_path:?}: {error}"));
    let fields = parse_fields(&provenance);

    assert_eq!(fields.get("profile"), Some(&EXPECTED_PROFILE));
    assert_eq!(
        fields.get("whatwg-html-commit"),
        Some(&EXPECTED_WHATWG_COMMIT)
    );
    assert_eq!(fields.get("wpt-commit"), Some(&EXPECTED_WPT_COMMIT));
    assert_full_sha("whatwg-html-commit", fields["whatwg-html-commit"]);
    assert_full_sha("wpt-commit", fields["wpt-commit"]);

    for field in [
        "whatwg-html-source",
        "whatwg-html-source-sha256",
        "wpt-tests10-source",
        "wpt-tests10-sha256",
        "wpt-svg-source",
        "wpt-svg-sha256",
        "wpt-math-source",
        "wpt-math-sha256",
        "wpt-namespace-sensitivity-source",
        "wpt-namespace-sensitivity-sha256",
        "tests10-imported-cases",
        "tests10-reviewed-local-coverage-cases",
        "adaptation",
        "deviation-processing-instructions",
        "deviation-svg-script",
        "deviation-fragment-parsing",
    ] {
        assert!(
            fields
                .get(field)
                .is_some_and(|value| !value.trim().is_empty()),
            "AE11 provenance field must be present and non-empty: {field}"
        );
    }

    for field in [
        "whatwg-html-source-sha256",
        "wpt-tests10-sha256",
        "wpt-svg-sha256",
        "wpt-math-sha256",
        "wpt-namespace-sensitivity-sha256",
    ] {
        assert_sha256(field, fields[field]);
    }

    let contract_path = root.join("docs/html5/ae11-foreign-content-tree-construction-contract.md");
    let contract = fs::read_to_string(&contract_path)
        .unwrap_or_else(|error| panic!("failed to read {contract_path:?}: {error}"));
    for required in [
        EXPECTED_PROFILE,
        EXPECTED_WHATWG_COMMIT,
        EXPECTED_WPT_COMMIT,
        "duplicate-attribute",
        "Noah's Ark comparison",
        "adjusted current node",
        "CDATA section, bracket, and end states",
        "foreign characters and frameset-ok",
        "foreign end tags",
        "static SVG script end handling",
    ] {
        assert!(
            contract.contains(required),
            "AE11 contract is missing pinned provenance item: {required}"
        );
    }
}

fn parse_fields(input: &str) -> BTreeMap<&str, &str> {
    input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split_once(": ")
                .unwrap_or_else(|| panic!("invalid AE11 provenance line: {line:?}"))
        })
        .collect()
}

fn assert_full_sha(label: &str, value: &str) {
    assert_eq!(value.len(), 40, "{label} must be a full 40-character SHA");
    assert!(
        value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{label} must contain only hexadecimal characters"
    );
    assert!(
        !value.contains("TODO") && !value.contains("placeholder"),
        "{label} must not be a placeholder"
    );
}

fn assert_sha256(label: &str, value: &str) {
    assert_eq!(value.len(), 64, "{label} must be a complete SHA-256");
    assert!(
        value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{label} must contain only hexadecimal characters"
    );
}
