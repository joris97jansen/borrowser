use crate::model::{
    CanvasColor, Comparison, Execution, Expectation, Expected, Fixture, FixtureKind,
};
use crate::projection::{Observation, ProjectedObservations};
use std::io::{self, Write};

pub(crate) struct CaseReport<'a> {
    fixture: &'a Fixture,
    execution: Execution,
    comparison: Option<Comparison>,
    expected: Option<ProjectedObservations>,
}

pub(crate) fn evaluate(fixture: &Fixture, mut execution: Execution) -> CaseReport<'_> {
    let mut validated_expected = None;
    let comparison = match (&execution, fixture.kind) {
        (Execution::Observed(actual), FixtureKind::Runnable { expected, .. }) => {
            match expected.observations().and_then(|expected| {
                actual.observations.validate(fixture.request)?;
                expected.validate(fixture.request)?;
                Ok(expected)
            }) {
                Ok(expected) => {
                    let comparison = if actual.observations == expected {
                        Comparison::Match
                    } else {
                        Comparison::Mismatch
                    };
                    validated_expected = Some(expected);
                    Some(comparison)
                }
                Err(error) => {
                    execution = Execution::Error(error);
                    None
                }
            }
        }
        _ => None,
    };
    CaseReport {
        fixture,
        execution,
        comparison,
        expected: validated_expected,
    }
}

impl CaseReport<'_> {
    pub(crate) fn status(&self) -> &'static str {
        if let Execution::Observed(capture) = &self.execution
            && capture.observations.incomplete()
            && self.comparison == Some(Comparison::Match)
        {
            return "INCOMPLETE";
        }
        match (&self.execution, self.comparison, self.fixture.kind) {
            (
                Execution::Observed(_),
                Some(Comparison::Match),
                FixtureKind::Runnable {
                    expectation: Expectation::Match,
                    ..
                },
            ) => "PASS",
            (
                Execution::Observed(_),
                Some(Comparison::Mismatch),
                FixtureKind::Runnable {
                    expectation: Expectation::Match,
                    ..
                },
            ) => "FAIL",
            (
                Execution::Observed(_),
                Some(Comparison::Mismatch),
                FixtureKind::Runnable {
                    expectation: Expectation::KnownFailure { .. },
                    ..
                },
            ) => "XFAIL",
            (
                Execution::Observed(_),
                Some(Comparison::Match),
                FixtureKind::Runnable {
                    expectation: Expectation::KnownFailure { .. },
                    ..
                },
            ) => "XPASS",
            (Execution::Unsupported { .. }, None, _) => "UNSUPPORTED",
            (Execution::Skipped { .. }, None, _) => "SKIP",
            _ => "ERROR",
        }
    }
}

fn color(CanvasColor([r, g, b]): CanvasColor) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

pub(crate) fn write_report(reports: &[CaseReport<'_>], out: &mut impl Write) -> io::Result<()> {
    const STATUSES: [&str; 8] = [
        "PASS",
        "FAIL",
        "XFAIL",
        "XPASS",
        "UNSUPPORTED",
        "SKIP",
        "ERROR",
        "INCOMPLETE",
    ];
    let mut counts = [0; 8];
    for report in reports {
        let status = report.status();
        counts[STATUSES
            .iter()
            .position(|item| *item == status)
            .expect("report status")] += 1;
        writeln!(out, "{status} {}", report.fixture.id.0)?;
        writeln!(out, "  source: {}", report.fixture.source)?;
        match (&report.execution, report.fixture.kind) {
            (
                Execution::Observed(actual),
                FixtureKind::Runnable {
                    expected: Expected::Canvas(expected),
                    expectation,
                    ..
                },
            ) => {
                let crate::projection::Observation::Observed(actual) = actual.observations.canvas
                else {
                    writeln!(out, "  error: missing expected canvas")?;
                    continue;
                };
                writeln!(out, "  sample: (32.5, 32.5) CSS px")?;
                writeln!(out, "  expected: {}", color(expected))?;
                writeln!(out, "  actual:   {}", color(actual))?;
                if report.comparison == Some(Comparison::Mismatch) {
                    for (index, name) in ["red", "green", "blue"].iter().enumerate() {
                        if expected.0[index] != actual.0[index] {
                            writeln!(
                                out,
                                "  difference: {name} expected {}, actual {}",
                                expected.0[index], actual.0[index]
                            )?;
                        }
                    }
                }
                if let Expectation::KnownFailure { reason } = expectation {
                    writeln!(out, "  known failure: {reason}")?;
                }
            }
            (
                Execution::Observed(actual),
                FixtureKind::Runnable {
                    expected: Expected::Projection(_),
                    expectation,
                    ..
                },
            ) => {
                let expected =
                    serde_json::to_value(report.expected.as_ref().expect("validated expectation"))?;
                if actual.observations.incomplete() {
                    writeln!(
                        out,
                        "  coverage: incomplete (unsupported observations or no compared fields)"
                    )?;
                }
                fn unsupported<T>(
                    out: &mut impl Write,
                    id: &str,
                    value: &Observation<T>,
                ) -> io::Result<()> {
                    if let Observation::Unsupported(reason) = value {
                        writeln!(out, "  unsupported: {id}: {reason}")?;
                    }
                    Ok(())
                }
                unsupported(out, "dom", &actual.observations.dom)?;
                for field in &actual.observations.styles {
                    unsupported(
                        out,
                        &format!("style:#{}/{}", field.target, field.property),
                        &field.value,
                    )?;
                }
                unsupported(out, "layout", &actual.observations.layout)?;
                unsupported(out, "canvas_color_rgb8", &actual.observations.canvas)?;
                let actual = serde_json::to_value(&actual.observations)?;
                for difference in crate::projection::differences("", &expected, &actual) {
                    writeln!(
                        out,
                        "  difference: {} expected {}, actual {}",
                        difference.path,
                        difference
                            .left
                            .map_or_else(|| "<absent>".into(), |v| v.to_string()),
                        difference
                            .right
                            .map_or_else(|| "<absent>".into(), |v| v.to_string())
                    )?;
                }
                if let Expectation::KnownFailure { reason } = expectation {
                    writeln!(out, "  known failure: {reason}")?;
                }
            }
            (Execution::Unsupported { reason } | Execution::Skipped { reason }, _) => {
                writeln!(out, "  reason: {reason}")?
            }
            (Execution::Error(error), _) => {
                writeln!(out, "  error: {}: {}", error.code, error.detail)?
            }
            _ => writeln!(
                out,
                "  error: report.invalid: observation has no runnable expectation"
            )?,
        }
    }
    write!(out, "SUMMARY")?;
    for (status, count) in STATUSES.into_iter().zip(counts) {
        if status == "INCOMPLETE" && count == 0 {
            continue;
        }
        write!(out, " {status}={count}")?;
    }
    writeln!(out)
}

pub(crate) fn exit_code(reports: &[CaseReport<'_>]) -> u8 {
    if reports.iter().any(|report| report.status() == "ERROR") {
        2
    } else if reports
        .iter()
        .any(|report| matches!(report.status(), "FAIL" | "XPASS"))
    {
        1
    } else if reports.iter().any(|r| r.status() == "INCOMPLETE") {
        3
    } else {
        0
    }
}
