use super::helpers::{
    run_chunks, run_chunks_with_config_and_errors, run_text_mode_chunks_with_config_and_errors,
};
use crate::html5::shared::ParseErrorCode;
use crate::html5::tokenizer::{TextModeSpec, TokenizerConfig, TokenizerLimits};

fn limit_config(limits: TokenizerLimits) -> TokenizerConfig {
    TokenizerConfig {
        limits,
        ..TokenizerConfig::default()
    }
}

fn assert_single_limit_error(
    errors: &[crate::html5::shared::ParseError],
    detail: &'static str,
    limit: usize,
) {
    assert_eq!(
        errors.len(),
        1,
        "expected exactly one limit error: {errors:?}"
    );
    let error = &errors[0];
    assert_eq!(error.code, ParseErrorCode::ResourceLimit);
    assert_eq!(error.detail, Some(detail));
    assert_eq!(error.aux, Some(limit as u32));
}

#[test]
fn token_batch_limit_yields_same_tokens_with_deterministic_limit_errors() {
    let input = "<a></a><b></b>";
    let baseline = run_chunks(&[input]);
    let limits = TokenizerLimits {
        max_tokens_per_batch: 1,
        ..TokenizerLimits::default()
    };
    let (tokens, errors) = run_chunks_with_config_and_errors(limit_config(limits), &[input]);

    assert_eq!(tokens, baseline);
    assert_eq!(
        errors.len(),
        3,
        "expected one limit error per forced intermediate yield"
    );
    for error in &errors {
        assert_eq!(error.code, ParseErrorCode::ResourceLimit);
        assert_eq!(error.detail, Some("token-batch-limit"));
        assert_eq!(error.aux, Some(1));
    }
}

#[test]
fn tag_name_limit_truncates_start_and_end_tags_deterministically() {
    let limits = TokenizerLimits {
        max_tag_name_bytes: 3,
        ..TokenizerLimits::default()
    };
    let (tokens, errors) =
        run_chunks_with_config_and_errors(limit_config(limits), &["<abcdef>body</abcdef>"]);

    assert_eq!(
        tokens,
        vec![
            "START name=abc attrs=[] self_closing=false".to_string(),
            "CHAR text=\"body\"".to_string(),
            "END name=abc".to_string(),
            "EOF".to_string(),
        ]
    );
    assert_eq!(errors.len(), 2);
    for error in &errors {
        assert_eq!(error.code, ParseErrorCode::ResourceLimit);
        assert_eq!(error.detail, Some("tag-name-truncated"));
        assert_eq!(error.aux, Some(3));
    }
}

#[test]
fn attribute_name_limit_truncates_attribute_name_deterministically() {
    let limits = TokenizerLimits {
        max_attribute_name_bytes: 3,
        ..TokenizerLimits::default()
    };
    let (tokens, errors) =
        run_chunks_with_config_and_errors(limit_config(limits), &["<div abcdef=1></div>"]);

    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[abc=\"1\"] self_closing=false".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
    assert_single_limit_error(&errors, "attribute-name-truncated", 3);
}

#[test]
fn attribute_value_limit_truncates_attribute_value_deterministically() {
    let limits = TokenizerLimits {
        max_attribute_value_bytes: 4,
        ..TokenizerLimits::default()
    };
    let (tokens, errors) =
        run_chunks_with_config_and_errors(limit_config(limits), &["<div data=abcdef></div>"]);

    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[data=\"abcd\"] self_closing=false".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
    assert_single_limit_error(&errors, "attribute-value-truncated", 4);
}

#[test]
fn attributes_per_tag_limit_drops_excess_attributes_deterministically() {
    let limits = TokenizerLimits {
        max_attributes_per_tag: 2,
        ..TokenizerLimits::default()
    };
    let (tokens, errors) =
        run_chunks_with_config_and_errors(limit_config(limits), &["<div a=1 b=2 c=3 d=4></div>"]);

    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[a=\"1\" b=\"2\"] self_closing=false".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
    assert_eq!(errors.len(), 2);
    for error in &errors {
        assert_eq!(error.code, ParseErrorCode::ResourceLimit);
        assert_eq!(error.detail, Some("attributes-per-tag-limit"));
        assert_eq!(error.aux, Some(2));
    }
}

#[test]
fn attributes_per_tag_limit_allows_zero_retained_attributes() {
    let limits = TokenizerLimits {
        max_attributes_per_tag: 0,
        ..TokenizerLimits::default()
    };
    let (tokens, errors) =
        run_chunks_with_config_and_errors(limit_config(limits), &["<div a=1 b=2></div>"]);

    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[] self_closing=false".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
    assert_eq!(errors.len(), 2);
    for error in &errors {
        assert_eq!(error.code, ParseErrorCode::ResourceLimit);
        assert_eq!(error.detail, Some("attributes-per-tag-limit"));
        assert_eq!(error.aux, Some(0));
    }
}

#[test]
fn comment_limit_truncates_emitted_comment_deterministically() {
    let limits = TokenizerLimits {
        max_comment_bytes: 4,
        ..TokenizerLimits::default()
    };
    let (tokens, errors) =
        run_chunks_with_config_and_errors(limit_config(limits), &["<!--abcdef-->"]);

    assert_eq!(
        tokens,
        vec!["COMMENT text=\"abcd\"".to_string(), "EOF".to_string()]
    );
    assert_single_limit_error(&errors, "comment-truncated", 4);
}

#[test]
fn doctype_limit_forces_quirks_without_corrupting_state() {
    let limits = TokenizerLimits {
        max_doctype_bytes: 10,
        ..TokenizerLimits::default()
    };
    let (tokens, errors) = run_chunks_with_config_and_errors(
        limit_config(limits),
        &["<!DOCTYPE html PUBLIC \"abcdef\"><p>"],
    );

    assert_eq!(
        tokens,
        vec![
            "DOCTYPE name=html public_id=null system_id=null force_quirks=true".to_string(),
            "START name=p attrs=[] self_closing=false".to_string(),
            "EOF".to_string(),
        ]
    );
    assert_single_limit_error(&errors, "doctype-limit", 10);
}

#[test]
fn end_tag_matcher_limit_treats_oversized_candidate_as_text_then_recovers() {
    let limits = TokenizerLimits {
        max_end_tag_match_scan_bytes: 8,
        ..TokenizerLimits::default()
    };
    let (tokens, _stats, errors) = run_text_mode_chunks_with_config_and_errors(
        limit_config(limits),
        &["<style>hello</style class=x></style>"],
        "style",
        TextModeSpec::rawtext_style,
    );

    assert_eq!(
        tokens,
        vec![
            "START name=style attrs=[] self_closing=false".to_string(),
            "CHAR text=\"hello</style class=x>\"".to_string(),
            "END name=style".to_string(),
            "EOF".to_string(),
        ]
    );
    assert_single_limit_error(&errors, "end-tag-matcher-limit", 8);
}
