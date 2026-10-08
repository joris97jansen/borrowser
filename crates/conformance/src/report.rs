use crate::model::{
    CanvasColor, Comparison, Execution, Expectation, Fixture, FixtureKind, compare,
};
use std::io::{self, Write};

pub(crate) struct CaseReport<'a> {
    fixture: &'a Fixture,
    execution: Execution,
    comparison: Option<Comparison>,
}

pub(crate) fn evaluate(fixture: &Fixture, execution: Execution) -> CaseReport<'_> {
    let comparison = match (&execution, fixture.kind) {
        (Execution::Observed(actual), FixtureKind::Runnable { expected, .. }) => {
            Some(compare(*actual, expected))
        }
        _ => None,
    };
    CaseReport {
        fixture,
        execution,
        comparison,
    }
}

impl CaseReport<'_> {
    pub(crate) fn status(&self) -> &'static str {
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
    const STATUSES: [&str; 7] = [
        "PASS",
        "FAIL",
        "XFAIL",
        "XPASS",
        "UNSUPPORTED",
        "SKIP",
        "ERROR",
    ];
    let mut counts = [0; 7];
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
                    expected,
                    expectation,
                    ..
                },
            ) => {
                writeln!(out, "  sample: (32.5, 32.5) CSS px")?;
                writeln!(out, "  expected: {}", color(expected))?;
                writeln!(out, "  actual:   {}", color(*actual))?;
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
    } else {
        0
    }
}
