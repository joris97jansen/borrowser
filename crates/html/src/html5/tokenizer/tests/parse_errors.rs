use super::helpers::run_chunks_with_config_and_errors;
use crate::html5::shared::{ErrorOrigin, ParseErrorCode};
use crate::html5::tokenizer::TokenizerConfig;

#[test]
fn null_in_data_state_is_replaced_and_reported() {
    let (tokens, errors) = run_chunks_with_config_and_errors(TokenizerConfig::default(), &["a\0b"]);
    let replacement = '\u{FFFD}';
    assert_eq!(
        tokens,
        vec![format!("CHAR text=\"a{replacement}b\""), "EOF".to_string()]
    );
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].origin, ErrorOrigin::Tokenizer);
    assert_eq!(errors[0].code, ParseErrorCode::UnexpectedNullCharacter);
    assert_eq!(errors[0].position, 1);
    assert_eq!(
        errors[0].detail,
        Some(super::super::normalization::ERROR_DETAIL_UNEXPECTED_NULL_CHARACTER)
    );
    assert_eq!(errors[0].aux, Some(0));
}

#[test]
fn null_in_supported_tag_and_attribute_fields_is_replaced_and_reported() {
    let (tokens, errors) =
        run_chunks_with_config_and_errors(TokenizerConfig::default(), &["<Di\0v a\0=1 b=x\0>"]);
    let replacement = '\u{FFFD}';
    assert_eq!(
        tokens,
        vec![
            format!(
                "START name=di{replacement}v attrs=[a{replacement}=\"1\" b=\"x{replacement}\"] self_closing=false"
            ),
            "EOF".to_string()
        ]
    );
    let mut positions: Vec<_> = errors.iter().map(|error| error.position).collect();
    positions.sort_unstable();
    assert_eq!(positions, vec![3, 7, 14]);
    assert!(
        errors
            .iter()
            .all(|error| error.origin == ErrorOrigin::Tokenizer
                && error.code == ParseErrorCode::UnexpectedNullCharacter
                && error.detail
                    == Some(super::super::normalization::ERROR_DETAIL_UNEXPECTED_NULL_CHARACTER))
    );
}

#[test]
fn unterminated_comment_reports_eof_and_null_without_aborting() {
    let (tokens, errors) =
        run_chunks_with_config_and_errors(TokenizerConfig::default(), &["<!--a\0"]);
    let replacement = '\u{FFFD}';
    assert_eq!(
        tokens,
        vec![
            format!("COMMENT text=\"a{replacement}\""),
            "EOF".to_string()
        ]
    );
    let codes: Vec<_> = errors.iter().map(|error| error.code).collect();
    assert_eq!(
        codes,
        vec![
            ParseErrorCode::UnexpectedEof,
            ParseErrorCode::UnexpectedNullCharacter
        ]
    );
    assert_eq!(
        errors[0].detail,
        Some(super::super::normalization::ERROR_DETAIL_EOF_IN_COMMENT)
    );
    assert_eq!(errors[1].position, 5);
}

#[test]
fn unfinished_doctype_reports_eof_and_still_emits_recoverable_tokens() {
    let (tokens, errors) = run_chunks_with_config_and_errors(
        TokenizerConfig::default(),
        &["<!DOCTYPE html PUBLIC \"x"],
    );
    assert_eq!(
        tokens,
        vec![
            "DOCTYPE name=html public_id=null system_id=null force_quirks=true".to_string(),
            "EOF".to_string()
        ]
    );
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].origin, ErrorOrigin::Tokenizer);
    assert_eq!(errors[0].code, ParseErrorCode::UnexpectedEof);
    assert_eq!(
        errors[0].detail,
        Some(super::super::normalization::ERROR_DETAIL_EOF_IN_DOCTYPE)
    );
}

#[test]
fn lonely_tag_open_at_eof_reports_recoverable_parse_error() {
    let (tokens, errors) = run_chunks_with_config_and_errors(TokenizerConfig::default(), &["<"]);
    assert_eq!(tokens, vec!["EOF".to_string()]);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].origin, ErrorOrigin::Tokenizer);
    assert_eq!(errors[0].code, ParseErrorCode::UnexpectedEof);
    assert_eq!(
        errors[0].detail,
        Some(super::super::normalization::ERROR_DETAIL_EOF_IN_TAG_OPEN)
    );
}
