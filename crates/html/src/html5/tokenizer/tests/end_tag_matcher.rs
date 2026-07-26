use crate::html5::tokenizer::TokenizerInvariantKind;
use crate::html5::tokenizer::scan::{
    IncrementalEndTagMatch, IncrementalEndTagMatcher, validate_completed_text_mode_end_tag_evidence,
};

#[test]
fn incremental_end_tag_matcher_resumes_across_name_and_space_boundaries() {
    let matcher = IncrementalEndTagMatcher::new(0);
    let matcher = match matcher.advance(b"</sty", b"style") {
        IncrementalEndTagMatch::NeedMoreInput(matcher) => matcher,
        other => panic!("expected partial name match, got {other:?}"),
    };
    let matcher = match matcher.advance(b"</style \t", b"style") {
        IncrementalEndTagMatch::NeedMoreInput(matcher) => matcher,
        other => panic!("expected partial trailing-space match, got {other:?}"),
    };
    assert_eq!(
        matcher.advance(b"</style \t>", b"style"),
        IncrementalEndTagMatch::Matched {
            cursor_after: 10,
            attribute_error_position: None,
            trailing_solidus_position: None,
        }
    );
}

#[test]
fn incremental_end_tag_matcher_rejects_false_positive_after_prefix_match() {
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(b"</stylex>", b"style"),
        IncrementalEndTagMatch::NoMatch
    );
}

#[test]
fn incremental_end_tag_matcher_handles_split_prefix_from_first_byte() {
    let matcher = match IncrementalEndTagMatcher::new(0).advance(b"<", b"script") {
        IncrementalEndTagMatch::NeedMoreInput(matcher) => matcher,
        other => panic!("expected partial '<' match, got {other:?}"),
    };
    let matcher = match matcher.advance(b"</scr", b"script") {
        IncrementalEndTagMatch::NeedMoreInput(matcher) => matcher,
        other => panic!("expected partial '</scr' match, got {other:?}"),
    };
    assert_eq!(
        matcher.advance(b"</script>", b"script"),
        IncrementalEndTagMatch::Matched {
            cursor_after: 9,
            attribute_error_position: None,
            trailing_solidus_position: None,
        }
    );
}

#[test]
fn incremental_end_tag_matcher_handles_non_zero_start_offsets() {
    let matcher = match IncrementalEndTagMatcher::new(3).advance(b"abc</scr", b"script") {
        IncrementalEndTagMatch::NeedMoreInput(matcher) => matcher,
        other => panic!("expected partial match at non-zero start, got {other:?}"),
    };
    assert_eq!(matcher.start(), 3);
    assert_eq!(matcher.cursor_for_test(), 8);
    assert_eq!(matcher.matched_name_len_for_test(), 3);
    assert_eq!(
        matcher.advance(b"abc</script>", b"script"),
        IncrementalEndTagMatch::Matched {
            cursor_after: 12,
            attribute_error_position: None,
            trailing_solidus_position: None,
        }
    );
}

#[test]
fn incremental_end_tag_matcher_trailing_space_growth_is_incremental_and_linear() {
    let start = 5usize;
    let mut buffer = b"lead:</script".to_vec();
    let mut matcher = match IncrementalEndTagMatcher::new(start).advance(&buffer, b"script") {
        IncrementalEndTagMatch::NeedMoreInput(matcher) => matcher,
        other => panic!("expected name-complete partial match, got {other:?}"),
    };

    assert_eq!(matcher.start(), start);
    assert_eq!(matcher.cursor_for_test(), buffer.len());
    assert_eq!(matcher.matched_name_len_for_test(), b"script".len());

    for _ in 0..4096 {
        buffer.push(b' ');
        let previous_cursor = matcher.cursor_for_test();
        matcher = match matcher.advance(&buffer, b"script") {
            IncrementalEndTagMatch::NeedMoreInput(matcher) => matcher,
            other => panic!("expected trailing-space growth to remain resumable, got {other:?}"),
        };
        assert_eq!(matcher.matched_name_len_for_test(), b"script".len());
        assert_eq!(matcher.cursor_for_test(), buffer.len());
        assert_eq!(matcher.cursor_for_test(), previous_cursor + 1);
    }

    buffer.push(b'>');
    assert_eq!(
        matcher.advance(&buffer, b"script"),
        IncrementalEndTagMatch::Matched {
            cursor_after: buffer.len(),
            attribute_error_position: None,
            trailing_solidus_position: None,
        }
    );
}

#[test]
fn incremental_end_tag_matcher_false_start_candidates_fail_deterministically() {
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(b"<<<<<<<<<<", b"script"),
        IncrementalEndTagMatch::NoMatch
    );
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(b"</s<", b"script"),
        IncrementalEndTagMatch::NoMatch
    );
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(b"</scriptx>", b"script"),
        IncrementalEndTagMatch::NoMatch
    );
}

#[test]
fn incremental_end_tag_matcher_partial_prefix_growth_preserves_progress_until_mismatch() {
    let matcher = match IncrementalEndTagMatcher::new(0).advance(b"</s", b"script") {
        IncrementalEndTagMatch::NeedMoreInput(matcher) => matcher,
        other => panic!("expected partial '</s' match, got {other:?}"),
    };
    assert_eq!(matcher.cursor_for_test(), 3);
    assert_eq!(matcher.matched_name_len_for_test(), 1);

    let matcher = match matcher.advance(b"</sc", b"script") {
        IncrementalEndTagMatch::NeedMoreInput(matcher) => matcher,
        other => panic!("expected partial '</sc' match, got {other:?}"),
    };
    assert_eq!(matcher.cursor_for_test(), 4);
    assert_eq!(matcher.matched_name_len_for_test(), 2);

    assert_eq!(
        matcher.advance(b"</scx", b"script"),
        IncrementalEndTagMatch::NoMatch
    );
}

#[test]
fn incremental_end_tag_matcher_respects_scan_window_limit() {
    let mut progress_bytes = 0u64;
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance_counted_limited(
            b"</script class=x>",
            b"script",
            &mut progress_bytes,
            8,
        ),
        IncrementalEndTagMatch::LimitExceeded
    );
    assert_eq!(progress_bytes, 8);
}

#[test]
fn incremental_end_tag_matcher_consumes_attribute_like_continuations() {
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(b"</style class=x>", b"style"),
        IncrementalEndTagMatch::Matched {
            cursor_after: 16,
            attribute_error_position: Some(15),
            trailing_solidus_position: None,
        }
    );
}

#[test]
fn incremental_end_tag_matcher_consumes_self_closing_continuations() {
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(b"</title/>", b"title"),
        IncrementalEndTagMatch::Matched {
            cursor_after: 9,
            attribute_error_position: None,
            trailing_solidus_position: Some(7),
        }
    );
}

#[test]
fn incremental_end_tag_matcher_carries_attribute_and_solidus_positions_across_every_split() {
    let input = b"</title a=1 />";
    let expected = IncrementalEndTagMatch::Matched {
        cursor_after: input.len(),
        attribute_error_position: Some(input.len() - 1),
        trailing_solidus_position: Some(input.len() - 2),
    };
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(input, b"title"),
        expected
    );

    for split in 1..input.len() {
        let matcher = match IncrementalEndTagMatcher::new(0).advance(&input[..split], b"title") {
            IncrementalEndTagMatch::NeedMoreInput(matcher) => matcher,
            other => panic!("split={split} should pause matcher, got {other:?}"),
        };
        assert_eq!(matcher.advance(input, b"title"), expected, "split={split}");
    }
}

#[test]
fn incremental_end_tag_matcher_resumes_across_attribute_value_growth() {
    let matcher = match IncrementalEndTagMatcher::new(0).advance(b"</script type=\"te", b"script") {
        IncrementalEndTagMatch::NeedMoreInput(matcher) => matcher,
        other => panic!("expected partial attribute value match, got {other:?}"),
    };
    assert!(matcher.had_attributes_for_test());
    let matcher = match matcher.advance(b"</script type=\"text/plain", b"script") {
        IncrementalEndTagMatch::NeedMoreInput(matcher) => matcher,
        other => panic!("expected continued attribute value match, got {other:?}"),
    };
    assert!(matcher.had_attributes_for_test());
    assert_eq!(
        matcher.advance(b"</script type=\"text/plain\">", b"script"),
        IncrementalEndTagMatch::Matched {
            cursor_after: 27,
            attribute_error_position: Some(26),
            trailing_solidus_position: None,
        }
    );
}

#[test]
fn incremental_end_tag_matcher_consumes_unquoted_attribute_value_tails() {
    let input = b"</script foo=bar>";
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(input, b"script"),
        IncrementalEndTagMatch::Matched {
            cursor_after: input.len(),
            attribute_error_position: Some(input.len() - 1),
            trailing_solidus_position: None,
        }
    );
}

#[test]
fn incremental_end_tag_matcher_consumes_single_quoted_attribute_value_tails() {
    let input = b"</script foo='bar'>";
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(input, b"script"),
        IncrementalEndTagMatch::Matched {
            cursor_after: input.len(),
            attribute_error_position: Some(input.len() - 1),
            trailing_solidus_position: None,
        }
    );
}

#[test]
fn incremental_end_tag_matcher_consumes_empty_attribute_value_tails() {
    let input = b"</script foo=>";
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(input, b"script"),
        IncrementalEndTagMatch::Matched {
            cursor_after: input.len(),
            attribute_error_position: Some(input.len() - 1),
            trailing_solidus_position: None,
        }
    );
}

#[test]
fn incremental_end_tag_matcher_consumes_attribute_value_before_self_closing_tail() {
    let input = b"</script foo=bar/>";
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(input, b"script"),
        IncrementalEndTagMatch::Matched {
            cursor_after: input.len(),
            attribute_error_position: Some(input.len() - 1),
            trailing_solidus_position: None,
        }
    );
}

#[test]
fn incremental_end_tag_matcher_recovers_from_space_after_self_closing_solidus() {
    let input = b"</script / >";
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(input, b"script"),
        IncrementalEndTagMatch::Matched {
            cursor_after: input.len(),
            attribute_error_position: None,
            trailing_solidus_position: None,
        }
    );
}

#[test]
fn incremental_end_tag_matcher_treats_post_quoted_name_continuation_as_another_attribute() {
    let input = b"</script type=\"x\"foo>";
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(input, b"script"),
        IncrementalEndTagMatch::Matched {
            cursor_after: input.len(),
            attribute_error_position: Some(input.len() - 1),
            trailing_solidus_position: None,
        }
    );
}

#[test]
fn incremental_end_tag_matcher_consumes_quoted_name_like_tail_bytes() {
    let input = b"</script \"x\">";
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(input, b"script"),
        IncrementalEndTagMatch::Matched {
            cursor_after: input.len(),
            attribute_error_position: Some(input.len() - 1),
            trailing_solidus_position: None,
        }
    );
}

#[test]
fn completed_text_mode_end_tag_evidence_rejects_corrupt_positions() {
    let bytes = b"prefix</title a=1/>";
    let start = 6;
    let cursor_after = bytes.len();
    let closing = cursor_after - 1;
    let solidus = closing - 1;

    for attribute_position in [Some(0), Some(start), Some(solidus)] {
        assert_eq!(
            validate_completed_text_mode_end_tag_evidence(
                bytes,
                start,
                cursor_after,
                attribute_position,
                None,
            ),
            Err(TokenizerInvariantKind::TextModeEndTagAttributePositionInvalid)
        );
    }

    for solidus_position in [Some(0), Some(start), Some(closing)] {
        assert_eq!(
            validate_completed_text_mode_end_tag_evidence(
                bytes,
                start,
                cursor_after,
                None,
                solidus_position,
            ),
            Err(TokenizerInvariantKind::TextModeEndTagSolidusPositionInvalid)
        );
    }

    assert_eq!(
        validate_completed_text_mode_end_tag_evidence(
            bytes,
            start,
            cursor_after,
            Some(closing),
            Some(closing),
        ),
        Err(TokenizerInvariantKind::TextModeEndTagSolidusPositionInvalid)
    );
    assert_eq!(
        validate_completed_text_mode_end_tag_evidence(
            bytes,
            start,
            cursor_after,
            Some(closing),
            Some(solidus),
        ),
        Ok(())
    );
}

#[test]
fn text_mode_candidate_range_is_validated_without_diagnostic_evidence() {
    let valid = b"</title>";
    assert_eq!(
        validate_completed_text_mode_end_tag_evidence(valid, 0, valid.len(), None, None),
        Ok(())
    );

    for (bytes, start, cursor_after) in [
        (valid.as_slice(), 4, 3),
        (valid.as_slice(), valid.len() + 1, valid.len()),
        (valid.as_slice(), 0, valid.len() + 1),
        (b"x/title>".as_slice(), 0, 8),
        (b"<xtitle>".as_slice(), 0, 8),
        (b"<x</title>".as_slice(), 0, 10),
        (b"x<".as_slice(), 1, 2),
        (b"</titlex".as_slice(), 0, 8),
    ] {
        assert_eq!(
            validate_completed_text_mode_end_tag_evidence(bytes, start, cursor_after, None, None,),
            Err(TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid)
        );
    }

    assert_eq!(
        IncrementalEndTagMatcher::new(0)
            .force_candidate_range_for_test(4, 3)
            .advance(valid, b"title"),
        IncrementalEndTagMatch::InvariantFailure(
            TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid
        )
    );
    assert_eq!(
        IncrementalEndTagMatcher::new(0)
            .force_candidate_range_for_test(valid.len() + 1, valid.len() + 1)
            .advance(valid, b"title"),
        IncrementalEndTagMatch::InvariantFailure(
            TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid
        )
    );
}

#[test]
fn text_mode_candidate_requires_the_retained_end_tag_opener() {
    let invalid = b"<xtitle>";
    assert_eq!(
        IncrementalEndTagMatcher::new(0)
            .force_candidate_range_for_test(0, 2)
            .force_name_phase_for_test()
            .validate_live_candidate_range(invalid),
        Err(TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid)
    );

    let invalid_with_attribute_evidence = b"<xtitle a=1>";
    assert_eq!(
        validate_completed_text_mode_end_tag_evidence(
            invalid_with_attribute_evidence,
            0,
            invalid_with_attribute_evidence.len(),
            Some(invalid_with_attribute_evidence.len() - 1),
            None,
        ),
        Err(TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid)
    );

    let invalid_with_solidus_evidence = b"<xtitle/>";
    assert_eq!(
        validate_completed_text_mode_end_tag_evidence(
            invalid_with_solidus_evidence,
            0,
            invalid_with_solidus_evidence.len(),
            None,
            Some(invalid_with_solidus_evidence.len() - 2),
        ),
        Err(TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid)
    );
}

#[test]
fn live_text_mode_matcher_validates_accepted_solidus_evidence() {
    let bytes = b"prefix</title/";
    let matcher = match IncrementalEndTagMatcher::new(6).advance(bytes, b"title") {
        IncrementalEndTagMatch::NeedMoreInput(matcher) => matcher,
        other => panic!("expected live self-closing candidate, got {other:?}"),
    };
    assert_eq!(matcher.validate_live_diagnostic_evidence(bytes), Ok(()));
    assert_eq!(
        matcher
            .force_live_solidus_position_for_test(0)
            .validate_live_diagnostic_evidence(bytes),
        Err(TokenizerInvariantKind::TextModeEndTagSolidusPositionInvalid)
    );
    assert_eq!(
        matcher
            .force_live_solidus_position_for_test(6)
            .validate_live_diagnostic_evidence(bytes),
        Err(TokenizerInvariantKind::TextModeEndTagSolidusPositionInvalid)
    );
}
