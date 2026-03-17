use super::Mode;
use super::fixtures::{Fixture, FixtureStatus, fixture_dir};
use super::runner::RunOutput;
use html_test_support::diff_lines;

pub(super) fn enforce_expected(
    fixture: &Fixture,
    actual: &RunOutput,
    mode: Mode,
    plan_label: Option<&str>,
) {
    let label = match plan_label {
        Some(plan) => format!("{} ({})", mode.label(), plan),
        None => mode.label().to_string(),
    };
    match fixture.expected.status {
        FixtureStatus::Active => match actual {
            RunOutput::Ok(lines) => {
                if lines.as_slice() != fixture.expected.lines.as_slice() {
                    panic!(
                        "DOM mismatch in fixture '{}' [{label}]\npath: {}\n{}",
                        fixture.name,
                        fixture_dir(&fixture.name).display(),
                        diff_lines(&fixture.expected.lines, lines)
                    );
                }
            }
            RunOutput::Err(err) => {
                panic!("fixture '{}' [{label}] failed: {err}", fixture.name);
            }
        },
        FixtureStatus::Xfail => match actual {
            RunOutput::Ok(lines) => {
                if lines.as_slice() == fixture.expected.lines.as_slice() {
                    panic!(
                        "fixture '{}' [{label}] matched expected DOM but is marked xfail; reason: {}\npath: {}",
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
            RunOutput::Err(_) => {
                // Expected to fail; keep xfail until implementation lands.
            }
        },
    }
}
