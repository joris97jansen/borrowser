use super::fixtures::{Fixture, FixtureStatus, fixture_dir};
use html_test_support::diff_lines;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecutionMode {
    WholeInput,
    ChunkedInput,
}

impl ExecutionMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            ExecutionMode::WholeInput => "whole",
            ExecutionMode::ChunkedInput => "chunked",
        }
    }
}

pub(crate) fn enforce_expected(
    fixture: &Fixture,
    actual: &[String],
    mode: ExecutionMode,
    plan_label: Option<&str>,
) {
    let mismatch = actual != fixture.expected.lines;
    let label = match plan_label {
        Some(plan) => format!("{} ({})", mode.label(), plan),
        None => mode.label().to_string(),
    };
    match fixture.expected.status {
        FixtureStatus::Active => {
            if mismatch {
                panic!(
                    "token mismatch in fixture '{}' [{label}]\npath: {}\n{}",
                    fixture.name,
                    fixture_dir(&fixture.name).display(),
                    diff_lines(&fixture.expected.lines, actual)
                );
            }
        }
        FixtureStatus::Xfail => {
            if !mismatch {
                panic!(
                    "fixture '{}' [{label}] matched expected tokens but is marked xfail; reason: {}\npath: {}",
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
        FixtureStatus::Skip => {}
    }
}
