use crate::model::{
    CanvasColor, Comparison, Execution, Expectation, Expected, Fixture, IndependentExpectation,
};
use crate::projection::{Observation, ProjectedObservations};
use std::io::{self, Write};

pub(crate) struct CaseReport<'a> {
    fixture: &'a Fixture,
    execution: Execution,
    comparison: Option<Comparison>,
    expected: Option<ProjectedObservations>,
    pub(crate) relationship: Option<crate::relationship::Evaluation>,
}

pub(crate) fn evaluate(fixture: &Fixture, execution: Execution) -> CaseReport<'_> {
    let (mut execution, reference) = match execution {
        Execution::ReferenceObserved { test, reference } => {
            (Execution::Observed(test), Some(reference))
        }
        other => (other, None),
    };
    let mut validated_expected = None;
    let mut comparison = None;
    let mut relationship = None;
    if let Execution::Observed(actual) = &execution {
        let result = (|| {
            actual.observations.validate(fixture.request)?;
            if let Some(independent) = fixture.independent() {
                let expected = independent.expected.observations()?;
                expected.validate(fixture.request)?;
                comparison = Some(if actual.observations == expected {
                    Comparison::Match
                } else {
                    Comparison::Mismatch
                });
                validated_expected = Some(expected);
            }
            match (fixture.reference(), reference) {
                (Some((kind, _)), Some(reference)) => {
                    relationship = Some(crate::relationship::evaluate(
                        kind,
                        fixture.request,
                        &actual.observations,
                        &reference.observations,
                    )?)
                }
                (None, None) => {}
                _ => {
                    return Err(crate::model::HarnessError::new(
                        "relationship.required-data",
                        "reference capture correspondence",
                    ));
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            execution = Execution::Error(error);
            comparison = None;
        }
    }
    CaseReport {
        fixture,
        execution,
        comparison,
        expected: validated_expected,
        relationship,
    }
}

impl CaseReport<'_> {
    pub(crate) fn independent_status(&self) -> Option<&'static str> {
        let independent = self.fixture.independent()?;
        if matches!(self.execution, Execution::Error(_)) {
            return Some("ERROR");
        }
        if let Execution::Observed(capture) = &self.execution {
            if capture.observations.incomplete() && self.comparison == Some(Comparison::Match) {
                return Some("INCOMPLETE");
            }
            return Some(match (self.comparison, independent.disposition) {
                (Some(Comparison::Match), Expectation::Match) => "PASS",
                (Some(Comparison::Mismatch), Expectation::Match) => "FAIL",
                (Some(Comparison::Match), Expectation::KnownFailure { .. }) => "XPASS",
                (Some(Comparison::Mismatch), Expectation::KnownFailure { .. }) => "XFAIL",
                _ => "ERROR",
            });
        }
        None
    }
    pub(crate) fn status(&self) -> &'static str {
        match &self.execution {
            Execution::Error(_) => "ERROR",
            Execution::Unsupported { .. } => "UNSUPPORTED",
            Execution::Skipped { .. } => "SKIP",
            Execution::Observed(_) => {
                let independent = self.independent_status();
                if independent == Some("ERROR") {
                    return "ERROR";
                }
                if independent == Some("FAIL")
                    || self
                        .relationship
                        .as_ref()
                        .is_some_and(|r| r.status != "satisfied")
                {
                    return "FAIL";
                }
                if independent == Some("XPASS") {
                    return "XPASS";
                }
                if independent == Some("INCOMPLETE") || self.fixture.upstream.is_some() {
                    return "INCOMPLETE";
                }
                independent.unwrap_or("PASS")
            }
            Execution::ReferenceObserved { .. } => "ERROR",
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
        writeln!(out, "  source: {}", report.fixture.document.source)?;
        match (&report.execution, report.fixture.independent()) {
            (
                Execution::Observed(actual),
                Some(IndependentExpectation {
                    expected: Expected::Canvas(expected),
                    disposition: expectation,
                }),
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
                Some(IndependentExpectation {
                    expected: Expected::Projection(_),
                    disposition: expectation,
                }),
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
            (Execution::Observed(_), None) => {}
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
        if report.fixture.reference().is_some() {
            if let Some(status) = report.independent_status() {
                writeln!(out, "  independent expectation: {status}")?;
            }
            if let Some(relation) = &report.relationship {
                writeln!(
                    out,
                    "  Borrowser relationship: {}",
                    serde_json::to_string(relation)?
                )?;
            }
        }
        if let Some(upstream) = report.fixture.upstream {
            writeln!(
                out,
                "  upstream assertion: UNSUPPORTED {}: {}",
                upstream.form, upstream.unsupported_reason
            )?;
        }
        if let Some(provenance) = report.fixture.document.provenance {
            writeln!(out, "  provenance: {}", serde_json::to_string(&provenance)?)?;
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
