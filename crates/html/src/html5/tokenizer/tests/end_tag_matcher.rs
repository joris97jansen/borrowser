use crate::html5::tokenizer::scan::{IncrementalEndTagMatch, IncrementalEndTagMatcher};

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
            had_attributes: false,
            self_closing: false,
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
            had_attributes: false,
            self_closing: false,
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
            had_attributes: false,
            self_closing: false,
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
            had_attributes: false,
            self_closing: false,
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
fn incremental_end_tag_matcher_consumes_attribute_like_continuations() {
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(b"</style class=x>", b"style"),
        IncrementalEndTagMatch::Matched {
            cursor_after: 16,
            had_attributes: true,
            self_closing: false,
        }
    );
}

#[test]
fn incremental_end_tag_matcher_consumes_self_closing_continuations() {
    assert_eq!(
        IncrementalEndTagMatcher::new(0).advance(b"</title/>", b"title"),
        IncrementalEndTagMatch::Matched {
            cursor_after: 9,
            had_attributes: false,
            self_closing: true,
        }
    );
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
            had_attributes: true,
            self_closing: false,
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
            had_attributes: true,
            self_closing: false,
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
            had_attributes: true,
            self_closing: false,
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
            had_attributes: true,
            self_closing: false,
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
            had_attributes: true,
            self_closing: true,
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
            had_attributes: false,
            self_closing: false,
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
            had_attributes: true,
            self_closing: false,
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
            had_attributes: true,
            self_closing: false,
        }
    );
}
