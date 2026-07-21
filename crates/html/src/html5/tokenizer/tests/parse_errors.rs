use super::helpers::run_chunks_with_config_and_errors;
use crate::html5::shared::{ErrorOrigin, ParseErrorCode};
use crate::html5::tokenizer::TokenizerConfig;

#[test]
fn duplicate_attribute_is_a_tokenizer_parse_error_and_remains_first_wins() {
    let (tokens, errors) =
        run_chunks_with_config_and_errors(TokenizerConfig::default(), &["<div a=1 A=2 b>"]);
    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[a=\"1\" b=\"\"] self_closing=false".to_string(),
            "EOF".to_string(),
        ]
    );
    assert_eq!(
        errors
            .iter()
            .filter(|error| error.detail == Some("duplicate-attribute"))
            .count(),
        1
    );
}

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

#[test]
fn valid_processing_instruction_does_not_report_the_old_invalid_tag_open_error() {
    let (tokens, errors) =
        run_chunks_with_config_and_errors(TokenizerConfig::default(), &["<?bogus?>"]);
    assert_eq!(
        tokens,
        vec![
            "PI target=\"bogus\" data=\"\"".to_string(),
            "EOF".to_string()
        ]
    );
    assert!(errors.is_empty());
}

#[test]
fn malformed_attribute_recovery_reports_stable_details() {
    let (tokens, errors) =
        run_chunks_with_config_and_errors(TokenizerConfig::default(), &["<div a=foo\"bar></div>"]);
    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[a=\"foo\" bar=\"\"] self_closing=false".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
    let details: Vec<_> = errors.iter().filter_map(|error| error.detail).collect();
    assert_eq!(
        details,
        vec![
            super::super::normalization::ERROR_DETAIL_INVALID_ATTRIBUTE_VALUE,
            super::super::normalization::ERROR_DETAIL_INVALID_ATTRIBUTE_NAME,
        ]
    );
    assert!(
        errors
            .iter()
            .all(|error| error.origin == ErrorOrigin::Tokenizer
                && error.code == ParseErrorCode::Other)
    );
}

#[test]
fn unfinished_tag_and_attribute_states_report_eof_without_partial_tokens() {
    let cases = [
        (
            "<div",
            super::super::normalization::ERROR_DETAIL_EOF_IN_TAG_NAME,
        ),
        (
            "</div",
            super::super::normalization::ERROR_DETAIL_EOF_IN_TAG_NAME,
        ),
        (
            "</",
            super::super::normalization::ERROR_DETAIL_EOF_IN_END_TAG_OPEN,
        ),
        (
            "<div a",
            super::super::normalization::ERROR_DETAIL_EOF_IN_ATTRIBUTE,
        ),
        (
            "<div a=\"x",
            super::super::normalization::ERROR_DETAIL_EOF_IN_ATTRIBUTE,
        ),
        (
            "<br/",
            super::super::normalization::ERROR_DETAIL_EOF_IN_SELF_CLOSING_START_TAG,
        ),
    ];
    for (input, detail) in cases {
        let (tokens, errors) =
            run_chunks_with_config_and_errors(TokenizerConfig::default(), &[input]);
        assert_eq!(tokens, vec!["EOF".to_string()], "input={input:?}");
        assert_eq!(errors.len(), 1, "input={input:?}");
        assert_eq!(errors[0].origin, ErrorOrigin::Tokenizer);
        assert_eq!(errors[0].code, ParseErrorCode::UnexpectedEof);
        assert_eq!(errors[0].detail, Some(detail), "input={input:?}");
    }
}

#[test]
fn partial_markup_declaration_eof_recovers_as_bogus_comment() {
    let (tokens, errors) = run_chunks_with_config_and_errors(TokenizerConfig::default(), &["<!D"]);
    assert_eq!(
        tokens,
        vec!["COMMENT text=\"D\"".to_string(), "EOF".to_string()]
    );
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].origin, ErrorOrigin::Tokenizer);
    assert_eq!(errors[0].code, ParseErrorCode::UnexpectedEof);
    assert_eq!(
        errors[0].detail,
        Some(super::super::normalization::ERROR_DETAIL_EOF_IN_MARKUP_DECLARATION)
    );
}

#[test]
fn malformed_doctype_reports_error_and_emits_quirks_token() {
    let (tokens, errors) =
        run_chunks_with_config_and_errors(TokenizerConfig::default(), &["<!DOCTYPE>"]);
    assert_eq!(
        tokens,
        vec![
            "DOCTYPE name=null public_id=null system_id=null force_quirks=true".to_string(),
            "EOF".to_string(),
        ]
    );
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].origin, ErrorOrigin::Tokenizer);
    assert_eq!(errors[0].code, ParseErrorCode::Other);
    assert_eq!(
        errors[0].detail,
        Some(super::super::normalization::ERROR_DETAIL_MALFORMED_DOCTYPE)
    );
}
