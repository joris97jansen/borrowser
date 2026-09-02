use external_test_provenance::{
    EXTERNAL_PROVENANCE_FORMAT_V1, parse_external_provenance_v1, serialize_external_provenance_v1,
};

const EXISTING_AE13E: &str = r#"format = "borrowser-external-provenance-v1"
upstream_project = "web-platform-tests/wpt"
upstream_revision = "2c705104a295c48053eeddf7fe0170d790a4e853"
upstream_path = "html/syntax/parsing/resources/webkit02.dat"
source_record_ordinal = 3
source_record_sha256 = "451124de0b3a67a5773b7fee11e4a83716cb26a08abb965452c1232e86a56ab9"
source_file_sha256 = "03b215350d352faf110df2cc6eac23a44a7f70945b4ea962f0b17bed103459f7"
license_identifier = "BSD-3-Clause"
license_notice = "The 3-Clause BSD License; see tests/wpt/external/LICENSE-3-Clause.txt."
attribution = "Copyright © web-platform-tests contributors"
adaptation = "Representation-only translation."
"#;

#[test]
fn existing_v1_wire_shape_round_trips_byte_for_byte() {
    let parsed = parse_external_provenance_v1(EXISTING_AE13E.as_bytes()).unwrap();
    assert_eq!(
        parsed.case_identity(),
        "2c705104a295c48053eeddf7fe0170d790a4e853:html/syntax/parsing/resources/webkit02.dat:3:451124de0b3a67a5773b7fee11e4a83716cb26a08abb965452c1232e86a56ab9"
    );
    assert_eq!(
        serialize_external_provenance_v1(&parsed).unwrap(),
        EXISTING_AE13E.as_bytes()
    );
    assert_eq!(
        EXTERNAL_PROVENANCE_FORMAT_V1,
        "borrowser-external-provenance-v1"
    );
}

#[test]
fn v1_is_closed_and_default_deny() {
    let unknown = EXISTING_AE13E.replace(
        "adaptation = \"Representation-only translation.\"",
        "adaptation = \"Representation-only translation.\"\nunknown = true",
    );
    assert!(parse_external_provenance_v1(unknown.as_bytes()).is_err());
}
