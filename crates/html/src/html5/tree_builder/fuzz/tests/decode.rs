use super::super::config::{
    TreeBuilderFuzzConfig, TreeBuilderFuzzTermination, derive_tree_builder_fuzz_seed,
};
use super::super::decode::{
    SYNTHETIC_TOKEN_DECODER_V2_MARKER, SyntheticTokenDecoderVersion, TAG_NAME_CATALOG_V1,
    decode_token_stream, decoder_version_for_input,
};
use super::super::driver::run_seeded_token_stream_fuzz_case;
use crate::html5::shared::{AtomTable, Token};

// Independent pre-AE10 oracle. Do not derive this from the production decoder
// table: equality pins V1's exact order, length, and modulo mapping.
const PRE_AE10_TAG_NAME_CATALOG: [&str; 30] = [
    "html", "head", "body", "title", "textarea", "style", "script", "table", "tbody", "thead",
    "tfoot", "tr", "td", "th", "caption", "colgroup", "col", "template", "p", "div", "span", "a",
    "b", "i", "nobr", "applet", "object", "form", "frameset", "br",
];

fn v2_input(payload: &[u8]) -> Vec<u8> {
    let mut bytes = SYNTHETIC_TOKEN_DECODER_V2_MARKER.to_vec();
    bytes.extend_from_slice(payload);
    bytes
}

fn decoded_start_tag_names(bytes: &[u8]) -> Vec<String> {
    let mut atoms = AtomTable::new();
    let decoded = decode_token_stream(bytes, &mut atoms, TreeBuilderFuzzConfig::default())
        .expect("synthetic token stream must decode");
    decoded
        .tokens
        .iter()
        .map(|token| match token {
            Token::StartTag { name, .. } => atoms.resolve(*name).expect("valid atom").to_string(),
            other => panic!("expected only start tags, got {other:?}"),
        })
        .collect()
}

fn decode(bytes: &[u8], config: TreeBuilderFuzzConfig) -> super::super::decode::DecodedTokenStream {
    let mut atoms = AtomTable::new();
    decode_token_stream(bytes, &mut atoms, config).expect("synthetic token stream must decode")
}

#[test]
fn decoder_v1_catalog_is_the_exact_pre_ae10_catalog() {
    assert_eq!(TAG_NAME_CATALOG_V1.len(), 30);
    assert_eq!(*TAG_NAME_CATALOG_V1, PRE_AE10_TAG_NAME_CATALOG);
}

#[test]
fn every_catalog_backed_selector_byte_keeps_its_pre_ae10_v1_mapping() {
    for selector in (0_u8..=u8::MAX).filter(|selector| selector & 1 == 0) {
        // Start tag, catalog selector, zero attributes.
        let input = [1, selector, 0];
        let names = decoded_start_tag_names(&input);
        assert_eq!(
            names,
            [PRE_AE10_TAG_NAME_CATALOG[selector as usize % 30]],
            "selector={selector}"
        );
    }
}

#[test]
fn unmarked_and_marker_like_inputs_have_deterministic_version_selection() {
    assert_eq!(
        decoder_version_for_input(b"ordinary legacy bytes"),
        SyntheticTokenDecoderVersion::V1
    );
    assert_eq!(
        decoder_version_for_input(SYNTHETIC_TOKEN_DECODER_V2_MARKER),
        SyntheticTokenDecoderVersion::V2
    );

    // Only an exact complete V2 marker has metadata meaning. Truncated and
    // unknown versions remain V1 and all their bytes remain token data.
    for bytes in [
        b"T".as_slice(),
        b"TB-FUZZ-V2".as_slice(),
        b"TB-FUZZ-V3\n".as_slice(),
    ] {
        assert_eq!(
            decoder_version_for_input(bytes),
            SyntheticTokenDecoderVersion::V1
        );
        let _ = decode(bytes, TreeBuilderFuzzConfig::default());
    }
}

#[test]
fn decoder_v2_marker_is_metadata_and_reaches_all_ae10_tags() {
    let marker_only = decode(
        SYNTHETIC_TOKEN_DECODER_V2_MARKER,
        TreeBuilderFuzzConfig::default(),
    );
    assert!(marker_only.tokens.is_empty());
    assert_eq!(marker_only.tokens_generated, 0);
    assert_eq!(marker_only.attrs_generated, 0);
    assert_eq!(marker_only.string_bytes_generated, 0);
    assert_eq!(marker_only.termination, None);

    let mut payload = Vec::new();
    // Even selectors use the catalog. These select V2 indices 30..=34 while
    // every final zero requests no attributes.
    for selector in [30, 66, 32, 68, 34] {
        payload.extend_from_slice(&[1, selector, 0]);
    }
    assert_eq!(
        decoded_start_tag_names(&v2_input(&payload)),
        ["select", "option", "optgroup", "input", "hr"]
    );
}

#[test]
fn decoder_version_prefix_changes_only_version_and_prefix_accounting() {
    // One catalog-backed start tag with one owned-valued attribute. The tag
    // selector maps inside V1's shared prefix under both versions.
    let payload = [1, 2, 1, 0, 1, 3, 0, 1, 2];
    let v1 = decode(&payload, TreeBuilderFuzzConfig::default());
    let v2 = decode(&v2_input(&payload), TreeBuilderFuzzConfig::default());
    assert_eq!(v2, v1);
    assert_eq!(v1.tokens_generated, 1);
    assert_eq!(v1.attrs_generated, 1);
    assert_eq!(v1.string_bytes_generated, 3);
    assert_eq!(v1.termination, None);

    for config in [
        TreeBuilderFuzzConfig {
            max_tokens_generated: 0,
            ..TreeBuilderFuzzConfig::default()
        },
        TreeBuilderFuzzConfig {
            max_total_attrs: 0,
            ..TreeBuilderFuzzConfig::default()
        },
        TreeBuilderFuzzConfig {
            max_string_bytes_generated: 2,
            ..TreeBuilderFuzzConfig::default()
        },
    ] {
        assert_eq!(
            decode(&v2_input(&payload), config),
            decode(&payload, config)
        );
    }

    assert_eq!(
        decode(
            &payload,
            TreeBuilderFuzzConfig {
                max_tokens_generated: 0,
                ..TreeBuilderFuzzConfig::default()
            }
        )
        .termination,
        Some(TreeBuilderFuzzTermination::RejectedMaxTokensGenerated)
    );
}

#[test]
fn v2_framing_counts_as_raw_input_but_not_decoded_work() {
    let marker = SYNTHETIC_TOKEN_DECODER_V2_MARKER;
    let raw_seed = derive_tree_builder_fuzz_seed(marker);
    let marker_only = run_seeded_token_stream_fuzz_case(
        marker,
        TreeBuilderFuzzConfig {
            seed: raw_seed,
            max_input_bytes: marker.len(),
            // No decoded tokens means the only processing step is synthetic EOF.
            max_processing_steps: 1,
            ..TreeBuilderFuzzConfig::default()
        },
    )
    .expect("marker-only V2 input must remain within the decoded-step budget");
    assert_eq!(
        marker_only.termination,
        TreeBuilderFuzzTermination::Completed
    );
    assert_eq!(marker_only.seed, raw_seed);
    assert_eq!(marker_only.input_bytes, marker.len());
    assert_eq!(marker_only.tokens_generated, 0);
    assert_eq!(marker_only.attrs_generated, 0);
    assert_eq!(marker_only.string_bytes_generated, 0);

    let rejected = run_seeded_token_stream_fuzz_case(
        marker,
        TreeBuilderFuzzConfig {
            max_input_bytes: marker.len() - 1,
            ..TreeBuilderFuzzConfig::default()
        },
    )
    .expect("raw input limit rejection is a normal fuzz termination");
    assert_eq!(
        rejected.termination,
        TreeBuilderFuzzTermination::RejectedMaxInputBytes
    );
    assert_eq!(rejected.input_bytes, marker.len());
    assert_eq!(rejected.tokens_generated, 0);

    // This catalog-backed tag selects the same shared-prefix name in V1 and V2.
    let payload = [1, 2, 0];
    let versioned = v2_input(&payload);
    let v1 = run_seeded_token_stream_fuzz_case(
        &payload,
        TreeBuilderFuzzConfig {
            max_processing_steps: 2,
            ..TreeBuilderFuzzConfig::default()
        },
    )
    .expect("V1 shared-prefix payload");
    let v2 = run_seeded_token_stream_fuzz_case(
        &versioned,
        TreeBuilderFuzzConfig {
            max_processing_steps: 2,
            ..TreeBuilderFuzzConfig::default()
        },
    )
    .expect("V2 shared-prefix payload");
    assert_eq!(v1.tokens_generated, v2.tokens_generated);
    assert_eq!(v1.attrs_generated, v2.attrs_generated);
    assert_eq!(v1.string_bytes_generated, v2.string_bytes_generated);
    assert_eq!(v1.tokens_generated, 1);
    assert_eq!(v1.attrs_generated, 0);
    assert_eq!(v1.string_bytes_generated, 0);

    assert_ne!(
        derive_tree_builder_fuzz_seed(&payload),
        derive_tree_builder_fuzz_seed(&versioned),
        "the raw V2 marker must domain-separate the fuzz input seed"
    );
}

#[test]
fn replaying_v1_and_v2_decoder_inputs_is_deterministic() {
    let v1 = b"legacy deterministic token bytes";
    let v2 = v2_input(&[1, 30, 0, 2, 30]);

    for bytes in [v1.as_slice(), v2.as_slice()] {
        assert_eq!(
            decode(bytes, TreeBuilderFuzzConfig::default()),
            decode(bytes, TreeBuilderFuzzConfig::default())
        );
    }
}
