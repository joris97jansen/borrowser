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
    addressing_expected: Option<crate::addressing::observations::AddressingObservation>,
    networking_expected: Option<crate::networking::observations::NetworkingObservation>,
    networking_differences: Vec<String>,
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
    let mut addressing_expected = None;
    let mut comparison = None;
    let mut relationship = None;
    let mut networking_expected = None;
    let mut networking_differences = Vec::new();
    if let Execution::Observed(actual) = &execution {
        let result = (|| {
            actual.observations.validate(fixture.request())?;
            if let Some(independent) = fixture.independent() {
                let expected = independent.expected.observations()?;
                expected.validate(fixture.request())?;
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
                        fixture.request(),
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
    if let Execution::AddressingObserved(actual) = &execution {
        let result = (|| {
            let crate::model::FixtureInput::Addressing(input) = fixture.input else {
                return Err(crate::model::HarnessError::new(
                    "addressing.required-data",
                    "addressing result for phase input",
                ));
            };
            let case = input.case()?;
            actual.validate(&case)?;
            let Some(independent) = fixture.independent() else {
                return Err(crate::model::HarnessError::new(
                    "addressing.required-data",
                    "missing independent expectation",
                ));
            };
            let Expected::Addressing(json) = independent.expected else {
                return Err(crate::model::HarnessError::new(
                    "addressing.required-data",
                    "missing addressing expectation",
                ));
            };
            let expected = crate::addressing::expected(json, &case)?;
            comparison = Some(if *actual == expected {
                Comparison::Match
            } else {
                Comparison::Mismatch
            });
            addressing_expected = Some(expected);
            Ok(())
        })();
        if let Err(error) = result {
            execution = Execution::Error(error);
            comparison = None;
        }
    }
    if let Execution::NetworkingObserved(actual) = &execution {
        let result = (|| {
            let crate::model::FixtureInput::Networking(input) = fixture.input else {
                return Err(crate::networking::observations::error(
                    "networking input required",
                ));
            };
            let case = input.case()?;
            let independent = fixture.independent().ok_or_else(|| {
                crate::networking::observations::error("independent expectation required")
            })?;
            let Expected::Networking(json) = independent.expected else {
                return Err(crate::networking::observations::error(
                    "networking expectation required",
                ));
            };
            let expected = crate::networking::expected(json, &case)?;
            actual.validate(expected.rows.len())?;
            if actual.rows.iter().zip(&expected.rows).any(|(a, b)| {
                a.storage().saturating_add(b.storage()) > crate::networking::bounds::ROW_STORAGE
            }) || actual.storage().saturating_add(expected.storage())
                > crate::networking::bounds::FIXTURE_STORAGE
            {
                return Err(crate::networking::observations::error(
                    "combined expectation/actual storage budget exceeded",
                ));
            }
            comparison = Some(if *actual == expected {
                Comparison::Match
            } else {
                Comparison::Mismatch
            });
            networking_differences =
                crate::networking::observations::differences(&expected, actual)?;
            networking_expected = Some(expected);
            Ok(())
        })();
        if let Err(e) = result {
            execution = Execution::Error(e);
            comparison = None;
        }
    }
    CaseReport {
        fixture,
        execution,
        comparison,
        expected: validated_expected,
        addressing_expected,
        networking_expected,
        networking_differences,
        relationship,
    }
}

impl CaseReport<'_> {
    pub(crate) fn independent_status(&self) -> Option<&'static str> {
        let independent = self.fixture.independent()?;
        if matches!(
            self.execution,
            Execution::Error(_) | Execution::NetworkingError(_)
        ) {
            return Some("ERROR");
        }
        if matches!(
            &self.execution,
            Execution::AddressingObserved(_) | Execution::NetworkingObserved(_)
        ) {
            return Some(match (self.comparison, independent.disposition) {
                (Some(Comparison::Match), Expectation::Match) => "PASS",
                (Some(Comparison::Mismatch), Expectation::Match) => "FAIL",
                (Some(Comparison::Match), Expectation::KnownFailure { .. }) => "XPASS",
                (Some(Comparison::Mismatch), Expectation::KnownFailure { .. }) => "XFAIL",
                _ => "ERROR",
            });
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
            Execution::Error(_) | Execution::NetworkingError(_) => "ERROR",
            Execution::Unsupported { .. } => "UNSUPPORTED",
            Execution::Skipped { .. } => "SKIP",
            Execution::Observed(_)
            | Execution::AddressingObserved(_)
            | Execution::NetworkingObserved(_) => {
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

/// Derived aggregate case totals, never counts of documents or comparison facts.
#[derive(Default, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct CaseCounts {
    pub(crate) passed: usize,
    pub(crate) failed: usize,
    pub(crate) expected_failed: usize,
    pub(crate) unexpected_passed: usize,
    pub(crate) unsupported: usize,
    pub(crate) skipped: usize,
    pub(crate) errored: usize,
    pub(crate) incomplete: usize,
}
impl CaseCounts {
    pub(crate) fn record(&mut self, status: &str) {
        let count = match status {
            "PASS" => &mut self.passed,
            "FAIL" => &mut self.failed,
            "XFAIL" => &mut self.expected_failed,
            "XPASS" => &mut self.unexpected_passed,
            "UNSUPPORTED" => &mut self.unsupported,
            "SKIP" => &mut self.skipped,
            "ERROR" => &mut self.errored,
            "INCOMPLETE" => &mut self.incomplete,
            _ => unreachable!("existing evaluated case status"),
        };
        *count += 1;
    }
    fn fields(&self) -> [(&'static str, usize); 8] {
        [
            ("PASS", self.passed),
            ("FAIL", self.failed),
            ("XFAIL", self.expected_failed),
            ("XPASS", self.unexpected_passed),
            ("UNSUPPORTED", self.unsupported),
            ("SKIP", self.skipped),
            ("ERROR", self.errored),
            ("INCOMPLETE", self.incomplete),
        ]
    }
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(crate) fn reproduction(mode: &str, suite: Option<crate::fixtures::Suite>, id: &str) -> String {
    let mut command = "cargo run -q -p borrowser-conformance --locked --".to_string();
    if !mode.is_empty() {
        command.push(' ');
        command.push_str(mode);
    }
    if let Some(suite) = suite {
        command.push_str(&format!(" --suite {}", suite.name()));
    }
    match mode {
        "--compare" | "--chromium" => command.push_str(" --chromium-executable \"$BORROWSER_CHROMIUM_EXECUTABLE\""),
        "--compare-aws" => command.push_str(" --config \"$AG4_CONFIG\" --artifact-dir \"$AG4_ARTIFACT_DIR\" --state-dir \"$AG4_STATE_DIR\""),
        _ => {}
    }
    command.push(' ');
    command.push_str(&quote(id));
    command
}

pub(crate) fn write_run_error(
    out: &mut impl Write,
    origin: &str,
    code: &str,
    publication: &str,
) -> io::Result<()> {
    writeln!(
        out,
        "RUN_ERROR scope=invocation errors=1 origin={origin} code={code} case_results={publication}"
    )
}

pub(crate) fn write_invocation_reproduction(
    args: &[String],
    out: &mut impl Write,
) -> io::Result<()> {
    writeln!(
        out,
        "REPRO command: cargo run -q -p borrowser-conformance --locked -- {}",
        args.iter().map(|s| quote(s)).collect::<Vec<_>>().join(" ")
    )?;
    if matches!(
        args.first().map(String::as_str),
        Some("--compare" | "--chromium")
    ) && !args.iter().any(|arg| arg == "--chromium-executable")
    {
        writeln!(
            out,
            "REPRO BORROWSER_CHROMIUM_EXECUTABLE: {:?}",
            std::env::var_os("BORROWSER_CHROMIUM_EXECUTABLE")
        )?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn write_report(reports: &[CaseReport<'_>], out: &mut impl Write) -> io::Result<()> {
    write_suite_report(reports, None, out)
}

pub(crate) fn write_suite_report(
    reports: &[CaseReport<'_>],
    suite: Option<crate::fixtures::Suite>,
    out: &mut impl Write,
) -> io::Result<()> {
    let mut counts = CaseCounts::default();
    let mut networking_counts = CaseCounts::default();
    let mut has_networking = false;
    for report in reports {
        let status = report.status();
        counts.record(status);
        if matches!(
            report.fixture.input,
            crate::model::FixtureInput::Networking(_)
        ) {
            has_networking = true;
            networking_counts.record(status);
        }
        writeln!(out, "{status} {}", report.fixture.id.0)?;
        writeln!(out, "  source: {}", report.fixture.source())?;
        writeln!(out, "  metadata: {}", report.fixture.metadata_source)?;
        writeln!(out, "  suite: {}", suite.map_or("catalogue", |s| s.name()))?;
        writeln!(
            out,
            "  reproduce: {}",
            reproduction("", suite, report.fixture.id.0)
        )?;
        if let Some((_, reference)) = report.fixture.reference() {
            writeln!(out, "  reference source: {}", reference.source)?;
        }
        match (&report.execution, report.fixture.independent()) {
            (Execution::NetworkingObserved(actual), Some(independent)) => {
                writeln!(
                    out,
                    "  actual snapshot: {}",
                    String::from_utf8(
                        crate::networking::bounds::json(actual, crate::networking::bounds::REPORT)
                            .map_err(io::Error::other)?
                    )
                    .map_err(io::Error::other)?
                )?;
                for difference in &report.networking_differences {
                    writeln!(out, "  {difference}")?;
                }
                if let Expectation::KnownFailure { reason } = independent.disposition {
                    writeln!(out, "  known failure: {reason}")?;
                }
            }
            (Execution::NetworkingError(e), _) => {
                writeln!(
                    out,
                    "  error: {}: {}; producer_closure={:?} worker_termination=unverified",
                    e.primary.code, e.primary.detail, e.closure
                )?;
            }

            (
                Execution::AddressingObserved(actual),
                Some(IndependentExpectation {
                    expected: Expected::Addressing(_),
                    disposition,
                }),
            ) => {
                writeln!(
                    out,
                    "  actual snapshot: {}",
                    serde_json::to_string_pretty(actual)?
                )?;
                let expected = serde_json::to_value(
                    report
                        .addressing_expected
                        .as_ref()
                        .expect("validated addressing expectation"),
                )?;
                let actual = serde_json::to_value(actual)?;
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
                if let Expectation::KnownFailure { reason } = disposition {
                    writeln!(out, "  known failure: {reason}")?;
                }
            }

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
        if let Some(provenance) = report.fixture.provenance() {
            writeln!(out, "  provenance: {}", serde_json::to_string(&provenance)?)?;
        }
    }
    if reports
        .iter()
        .any(|r| matches!(r.fixture.input, crate::model::FixtureInput::Phases { .. }))
    {
        writeln!(
            out,
            "ENVIRONMENT viewport={}x{} device_scale_factor=1 sample_pixel=({}, {}) document_url={}",
            crate::environment::WIDTH,
            crate::environment::HEIGHT,
            crate::environment::SAMPLE[0],
            crate::environment::SAMPLE[1],
            crate::environment::URL
        )?;
    }
    writeln!(
        out,
        "INDEPENDENT_EXPECTATIONS XFAIL={}",
        reports
            .iter()
            .filter(|r| r.independent_status() == Some("XFAIL"))
            .count()
    )?;
    writeln!(out, "RUN harness_errors=0")?;
    if has_networking {
        write!(out, "GROUP networking/fetch")?;
        for (status, count) in networking_counts.fields() {
            write!(out, " {status}={count}")?;
        }
        writeln!(out)?;
    }
    write!(out, "SUMMARY")?;
    for (status, count) in counts.fields() {
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

impl CaseReport<'_> {
    pub(crate) fn networking_storage(&self) -> usize {
        self.networking_expected.as_ref().map_or(0, |v| v.storage())
            + match &self.execution {
                Execution::NetworkingObserved(v) => v.storage(),
                _ => 0,
            }
    }
}

impl CaseReport<'_> {
    pub(crate) fn into_networking(
        self,
    ) -> Result<
        (
            crate::networking::observations::NetworkingObservation,
            crate::networking::observations::NetworkingObservation,
            Vec<String>,
            &'static str,
        ),
        crate::model::HarnessError,
    > {
        let status = self.status();
        match (self.execution, self.networking_expected) {
            (Execution::NetworkingObserved(actual), Some(expected)) => {
                Ok((actual, expected, self.networking_differences, status))
            }
            (Execution::NetworkingError(e), _) => Err(crate::model::HarnessError::new(
                e.primary.code,
                format!(
                    "{}; producer_closure={:?}; worker_termination=unverified",
                    e.primary.detail, e.closure
                ),
            )),
            (Execution::Error(e), _) => Err(e),
            _ => Err(crate::networking::observations::error(
                "missing networking comparison evidence",
            )),
        }
    }
}
