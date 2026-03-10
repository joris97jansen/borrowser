use super::fixtures::{Fixture, FixtureStatus, fixture_dir, write_expected_patch_file};
use super::runner::{ExecutionMode, PatchRunResult};
use html_test_support::diff_lines;

pub(crate) const BATCH_MARKER_PREFIX: &str = "Batch index=";

pub(crate) fn parse_batch_marker_line(line: &str) -> Option<(usize, usize)> {
    let rest = line.strip_prefix(BATCH_MARKER_PREFIX)?;
    let (index_str, size_str) = rest.split_once(" size=")?;
    let index = index_str.parse::<usize>().ok()?;
    let size = size_str.parse::<usize>().ok()?;
    Some((index, size))
}

fn is_batch_marker_line(line: &str) -> bool {
    parse_batch_marker_line(line).is_some()
}

fn batch_markers_filtered(lines: &[String]) -> impl Iterator<Item = &str> {
    lines
        .iter()
        .map(String::as_str)
        .filter(|line| !is_batch_marker_line(line))
}

pub(crate) fn filtered_lines_for_diff(lines: &[String]) -> Vec<String> {
    batch_markers_filtered(lines)
        .map(std::borrow::ToOwned::to_owned)
        .collect()
}

pub(crate) fn batch_partition_summary(lines: &[String]) -> String {
    let mut parts = Vec::new();
    for line in lines {
        if let Some((index, size)) = parse_batch_marker_line(line) {
            parts.push(format!("{index}:{size}"));
        }
    }
    if parts.is_empty() {
        "<none>".to_string()
    } else {
        parts.join(", ")
    }
}

pub(crate) fn lines_match(mode: ExecutionMode, actual: &[String], expected: &[String]) -> bool {
    if mode == ExecutionMode::WholeInput {
        actual == expected
    } else {
        batch_markers_filtered(actual).eq(batch_markers_filtered(expected))
    }
}

pub(crate) fn enforce_expected(
    fixture: &Fixture,
    actual: &PatchRunResult,
    mode: ExecutionMode,
    plan_label: Option<&str>,
    update: bool,
) {
    let label = match plan_label {
        Some(plan) => format!("{} ({})", mode.label(), plan),
        None => mode.label().to_string(),
    };

    if update && mode == ExecutionMode::WholeInput && plan_label.is_none() {
        if fixture.expected.status == FixtureStatus::Xfail {
            panic!(
                "refusing to update xfail fixture '{}' in update mode; resolve status first\npath: {}",
                fixture.name,
                fixture_dir(&fixture.name).display()
            );
        }
        match actual {
            PatchRunResult::Ok(lines) => {
                write_expected_patch_file(fixture, lines);
                return;
            }
            PatchRunResult::Err(err) => {
                panic!(
                    "refusing to update fixture '{}' because run failed: {err}\npath: {}",
                    fixture.name,
                    fixture_dir(&fixture.name).display()
                );
            }
        }
    }

    match fixture.expected.status {
        FixtureStatus::Active => match actual {
            PatchRunResult::Ok(lines) => {
                let matches_expected =
                    lines_match(mode, lines.as_slice(), fixture.expected.lines.as_slice());
                if !matches_expected {
                    panic!(
                        "patch mismatch in fixture '{}' [{label}]\npath: {}\n{}",
                        fixture.name,
                        fixture_dir(&fixture.name).display(),
                        diff_lines(&fixture.expected.lines, lines)
                    );
                }
            }
            PatchRunResult::Err(err) => {
                panic!("fixture '{}' [{label}] failed: {err}", fixture.name);
            }
        },
        FixtureStatus::Xfail => match actual {
            PatchRunResult::Ok(lines) => {
                let matches_expected =
                    lines_match(mode, lines.as_slice(), fixture.expected.lines.as_slice());
                if matches_expected {
                    panic!(
                        "fixture '{}' [{label}] matched expected patches but is marked xfail; reason: {}\npath: {}",
                        fixture.name,
                        fixture
                            .expected
                            .reason
                            .as_deref()
                            .unwrap_or("<missing reason>"),
                        fixture_dir(&fixture.name).display()
                    );
                }
            }
            PatchRunResult::Err(_) => {}
        },
    }
}

#[test]
fn batch_marker_parsing_accepts_exact_numeric_shape() {
    assert_eq!(
        parse_batch_marker_line("Batch index=0 size=13"),
        Some((0, 13))
    );
    assert!(is_batch_marker_line("Batch index=9 size=0"));
}

#[test]
fn batch_marker_parsing_rejects_non_contract_shapes() {
    assert_eq!(parse_batch_marker_line("Batch index=foo size=13"), None);
    assert_eq!(parse_batch_marker_line("Batch index=1 size=bar"), None);
    assert_eq!(parse_batch_marker_line("Batch index=1 size=13 extra"), None);
    assert_eq!(
        parse_batch_marker_line("CreateText key=1 text=\"Batch index=1 size=13\""),
        None
    );
    assert!(!is_batch_marker_line("Batch index=1 size=13 extra"));
}
