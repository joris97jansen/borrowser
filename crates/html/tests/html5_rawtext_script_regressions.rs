#![cfg(feature = "html5")]

mod rawtext_script_regressions;

use html_test_support::diff_lines;
use rawtext_script_regressions::{
    assert_expected_lines, fixture_filter, load_fixtures, run_token_fixture_every_boundary,
    run_token_fixture_whole,
};
#[cfg(feature = "dom-snapshot")]
use rawtext_script_regressions::{run_dom_fixture_every_boundary, run_dom_fixture_whole};

const MIN_REGRESSION_FIXTURES: usize = 1;

#[test]
fn html5_rawtext_script_regression_fixture_contract() {
    let fixtures = load_fixtures();
    assert!(
        fixtures.len() >= MIN_REGRESSION_FIXTURES,
        "rawtext/script regression corpus too small: found {} fixtures, require at least {}",
        fixtures.len(),
        MIN_REGRESSION_FIXTURES
    );

    for fixture in fixtures {
        assert!(
            fixture.name.starts_with("rs-"),
            "fixture '{}' must use the rs-<mode>-<slug>-<date> naming convention",
            fixture.name
        );
        assert!(
            fixture
                .name
                .bytes()
                .all(|b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "fixture '{}' must be lowercase ASCII kebab-case",
            fixture.name
        );
        assert!(
            fixture.expected_tokens.is_some() || fixture.expected_dom.is_some(),
            "fixture '{}' must declare at least one stable oracle (tokens.txt or dom.txt)",
            fixture.name
        );
        assert!(
            !fixture.meta.guard.trim().is_empty(),
            "fixture '{}' must explain what it guards",
            fixture.name
        );
    }
}

#[test]
fn html5_rawtext_script_regressions_tokens_whole_input() {
    let filter = fixture_filter();
    let fixtures = load_fixtures();
    let mut ran = 0usize;
    for fixture in fixtures
        .iter()
        .filter(|fixture| filter.matches(&fixture.name))
    {
        let Some(expected_tokens) = fixture.expected_tokens.as_ref() else {
            continue;
        };
        ran = ran.saturating_add(1);
        let actual = run_token_fixture_whole(fixture);
        assert_expected_lines(fixture, expected_tokens, &actual, "tokens whole");
    }
    assert!(
        ran > 0,
        "no rawtext/script token regressions matched filter"
    );
}

#[test]
fn html5_rawtext_script_regressions_tokens_every_boundary_chunked() {
    let filter = fixture_filter();
    let fixtures = load_fixtures();
    let mut ran = 0usize;
    for fixture in fixtures
        .iter()
        .filter(|fixture| filter.matches(&fixture.name))
    {
        let Some(expected_tokens) = fixture.expected_tokens.as_ref() else {
            continue;
        };
        ran = ran.saturating_add(1);
        let whole = run_token_fixture_whole(fixture);
        let chunked = run_token_fixture_every_boundary(fixture);
        if chunked != whole {
            panic!(
                "chunked token output diverged from whole input in rawtext/script regression '{}' [every-boundary]\npath: {}\nguard: {}\n{}",
                fixture.name,
                fixture.dir.display(),
                fixture.meta.guard,
                diff_lines(&whole, &chunked)
            );
        }
        assert_expected_lines(fixture, expected_tokens, &chunked, "tokens every-boundary");
    }
    assert!(
        ran > 0,
        "no rawtext/script token regressions matched filter"
    );
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn html5_rawtext_script_regressions_dom_whole_input() {
    let filter = fixture_filter();
    let fixtures = load_fixtures();
    let mut ran = 0usize;
    for fixture in fixtures
        .iter()
        .filter(|fixture| filter.matches(&fixture.name))
    {
        let Some(expected_dom) = fixture.expected_dom.as_ref() else {
            continue;
        };
        ran = ran.saturating_add(1);
        let actual = run_dom_fixture_whole(fixture);
        assert_expected_lines(fixture, &expected_dom.lines, &actual, "dom whole");
    }
    assert!(ran > 0, "no rawtext/script DOM regressions matched filter");
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn html5_rawtext_script_regressions_dom_every_boundary_chunked() {
    let filter = fixture_filter();
    let fixtures = load_fixtures();
    let mut ran = 0usize;
    for fixture in fixtures
        .iter()
        .filter(|fixture| filter.matches(&fixture.name))
    {
        let Some(expected_dom) = fixture.expected_dom.as_ref() else {
            continue;
        };
        ran = ran.saturating_add(1);
        let whole = run_dom_fixture_whole(fixture);
        let chunked = run_dom_fixture_every_boundary(fixture);
        if chunked != whole {
            panic!(
                "chunked DOM output diverged from whole input in rawtext/script regression '{}' [every-boundary]\npath: {}\nguard: {}\n{}",
                fixture.name,
                fixture.dir.display(),
                fixture.meta.guard,
                diff_lines(&whole, &chunked)
            );
        }
        assert_expected_lines(fixture, &expected_dom.lines, &chunked, "dom every-boundary");
    }
    assert!(ran > 0, "no rawtext/script DOM regressions matched filter");
}
