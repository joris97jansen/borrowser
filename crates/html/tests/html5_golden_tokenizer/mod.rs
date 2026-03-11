use html::test_harness::shrink_chunk_plan_with_stats;
use html_test_support::diff_lines;
use std::env;

mod driver;
mod expect;
mod fixtures;
mod plans;

use driver::{run_tokenizer_chunked, run_tokenizer_whole};
use expect::{ExecutionMode, enforce_expected};
use fixtures::{FixtureStatus, env_u64, fixture_filter, load_fixtures};
use plans::build_tokenizer_chunk_plans;

const MIN_TOKENIZER_FIXTURE_COUNT: usize = 20;

#[test]
fn html5_golden_tokenizer_fixture_corpus_contract() {
    let fixtures = load_fixtures();
    assert!(
        fixtures.len() >= MIN_TOKENIZER_FIXTURE_COUNT,
        "tokenizer golden corpus too small: found {} fixtures, require at least {}",
        fixtures.len(),
        MIN_TOKENIZER_FIXTURE_COUNT
    );

    let mut has_tag = false;
    let mut has_attr = false;
    let mut has_comment = false;
    let mut has_doctype = false;
    let mut has_named_charref_input = false;
    let mut has_numeric_charref_input = false;
    let mut has_named_decode_effect = false;
    let mut has_numeric_decode_effect = false;
    let mut has_rawtext_style = false;
    for fixture in &fixtures {
        if fixture.expected.status == FixtureStatus::Skip {
            continue;
        }
        has_rawtext_style |= fixture.input.contains("<style");
        let has_amp_input = fixture.input.contains("&amp;");
        let has_lt_input = fixture.input.contains("&lt;");
        let has_gt_input = fixture.input.contains("&gt;");
        let has_literal_amp_in_input = fixture.input.replace("&amp;", "").contains('&');
        let has_literal_a_in_input = fixture.input.contains('A');
        has_named_charref_input |= has_amp_input || has_lt_input || has_gt_input;
        let has_numeric_input = fixture.input.contains("&#");
        has_numeric_charref_input |= has_numeric_input;

        let mut has_char_amp = false;
        let mut has_char_amp_literal = false;
        let mut has_char_lt = false;
        let mut has_char_lt_literal = false;
        let mut has_char_gt = false;
        let mut has_char_gt_literal = false;
        let mut has_char_a = false;
        let mut has_char_numeric_literal = false;
        for line in &fixture.expected.lines {
            if line.starts_with("CHAR ") {
                has_char_amp |= line.contains('&');
                has_char_amp_literal |= line.contains("&amp;");
                has_char_lt |= line.contains('<');
                has_char_lt_literal |= line.contains("&lt;");
                has_char_gt |= line.contains('>');
                has_char_gt_literal |= line.contains("&gt;");
                has_char_a |= line.contains("CHAR text=\"") && line.contains('A');
                has_char_numeric_literal |= line.contains("&#") || line.contains("&#x");
            }
            if line.starts_with("START ") || line.starts_with("END ") {
                has_tag = true;
            }
            if line.starts_with("START ") && !line.contains("attrs=[]") {
                has_attr = true;
            }
            if line.starts_with("COMMENT ") {
                has_comment = true;
            }
            if line.starts_with("DOCTYPE ") {
                has_doctype = true;
            }
        }
        if has_amp_input && !has_literal_amp_in_input && has_char_amp && !has_char_amp_literal {
            has_named_decode_effect = true;
        }
        if has_lt_input && has_char_lt && !has_char_lt_literal {
            has_named_decode_effect = true;
        }
        if has_gt_input && has_char_gt && !has_char_gt_literal {
            has_named_decode_effect = true;
        }
        if (fixture.input.contains("&#65;") || fixture.input.contains("&#x41;"))
            && !has_literal_a_in_input
            && has_char_a
            && !has_char_numeric_literal
        {
            has_numeric_decode_effect = true;
        }
    }

    assert!(has_tag, "tokenizer corpus missing tag coverage");
    assert!(has_attr, "tokenizer corpus missing attribute coverage");
    assert!(has_comment, "tokenizer corpus missing comment coverage");
    assert!(has_doctype, "tokenizer corpus missing doctype coverage");
    assert!(
        has_named_charref_input,
        "tokenizer corpus missing named-charref-like input coverage"
    );
    assert!(
        has_numeric_charref_input,
        "tokenizer corpus missing numeric-charref-like input coverage"
    );
    assert!(
        has_named_decode_effect,
        "tokenizer corpus missing named-charref decode effect in expected outputs"
    );
    assert!(
        has_numeric_decode_effect,
        "tokenizer corpus missing numeric-charref decode effect in expected outputs"
    );
    assert!(
        has_rawtext_style,
        "tokenizer corpus missing rawtext style coverage"
    );
}

#[test]
fn html5_golden_tokenizer_whole_input() {
    let fixtures = load_fixtures();
    let filter = fixture_filter();
    let mut ran = 0usize;
    for fixture in fixtures {
        if !filter.matches(&fixture.name) {
            continue;
        }
        ran += 1;
        if fixture.expected.status == FixtureStatus::Skip {
            continue;
        }
        let actual = run_tokenizer_whole(&fixture);
        enforce_expected(&fixture, &actual, ExecutionMode::WholeInput, None);
    }
    assert!(ran > 0, "no fixtures matched filter");
}

#[test]
fn html5_golden_tokenizer_chunked_input() {
    let fixtures = load_fixtures();
    let filter = fixture_filter();
    let mut fuzz_runs = env_u64("BORROWSER_HTML5_TOKEN_FUZZ_RUNS", 4) as usize;
    if env::var("CI").is_ok() && fuzz_runs == 0 {
        fuzz_runs = 1;
    }
    let fuzz_seed = env_u64("BORROWSER_HTML5_TOKEN_FUZZ_SEED", 0xC0FFEE);
    let mut ran = 0usize;
    for fixture in fixtures {
        if !filter.matches(&fixture.name) {
            continue;
        }
        ran += 1;
        if fixture.expected.status == FixtureStatus::Skip {
            continue;
        }
        let whole = run_tokenizer_whole(&fixture);
        let plans = build_tokenizer_chunk_plans(&fixture.input, fuzz_runs, fuzz_seed);
        for plan in plans {
            let actual = run_tokenizer_chunked(&fixture, &plan.plan, &plan.label);
            if fixture.expected.status == FixtureStatus::Active && actual != whole {
                let (shrunk, stats) =
                    shrink_chunk_plan_with_stats(&fixture.input, &plan.plan, |candidate| {
                        run_tokenizer_chunked(&fixture, candidate, "shrinking") != whole
                    });
                panic!(
                    "chunked output mismatch in fixture '{}'\nplan: {}\nshrunk: {}\nshrink stats: {:?}\n{}",
                    fixture.name,
                    plan.label,
                    shrunk,
                    stats,
                    diff_lines(&whole, &actual)
                );
            }
            enforce_expected(
                &fixture,
                &actual,
                ExecutionMode::ChunkedInput,
                Some(&plan.label),
            );
        }
    }
    assert!(ran > 0, "no fixtures matched filter");
}

#[test]
fn html5_golden_tokenizer_chunk_plan_generation_is_seed_deterministic() {
    let input = "<div a=\"x\">Tom&amp;Jerry</div>";
    let seed = 0xC0FFEE_u64;
    let runs = 4usize;
    let a = build_tokenizer_chunk_plans(input, runs, seed);
    let b = build_tokenizer_chunk_plans(input, runs, seed);
    assert_eq!(a.len(), b.len(), "chunk plan count must be deterministic");
    for (left, right) in a.iter().zip(b.iter()) {
        assert_eq!(left.label, right.label, "chunk plan labels must match");
        assert_eq!(left.plan, right.plan, "chunk plan definitions must match");
    }
}
