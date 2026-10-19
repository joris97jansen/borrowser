//! One bounded comparison, after a complete standalone Chromium capture phase.
mod report;
#[cfg(test)]
pub(crate) mod tests;

use crate::{
    chromium::{self, CaptureInput, CapturedFixture},
    execute,
    model::{CanvasColor, Fixture, FixtureKind, HarnessError},
};
use std::{
    io::{self, Write},
    path::Path,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CanvasObservation {
    Observed(CanvasColor),
    Unsupported { reason: &'static str },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum FieldStatus {
    Equal,
    Different,
    Unsupported,
}

fn compare(borrowser: CanvasObservation, chromium: CanvasObservation) -> FieldStatus {
    match (borrowser, chromium) {
        (CanvasObservation::Observed(left), CanvasObservation::Observed(right)) => {
            if left == right {
                FieldStatus::Equal
            } else {
                FieldStatus::Different
            }
        }
        _ => FieldStatus::Unsupported,
    }
}

#[derive(Debug)]
enum Failure {
    Chromium(chromium::Failure),
    Harness(HarnessError),
}
impl From<chromium::Failure> for Failure {
    fn from(value: chromium::Failure) -> Self {
        Self::Chromium(value)
    }
}
impl From<HarnessError> for Failure {
    fn from(value: HarnessError) -> Self {
        Self::Harness(value)
    }
}
impl Failure {
    fn write_diagnostics(&self, out: &mut impl Write) -> io::Result<()> {
        match self {
            Self::Chromium(failure) => failure.write_diagnostics(out),
            Self::Harness(error) => {
                writeln!(out, "ERROR comparison.{}: {}", error.code, error.detail)
            }
        }
    }
}

fn validate_selection(fixtures: &[&Fixture]) -> Result<(), HarnessError> {
    if fixtures.is_empty() {
        return Err(HarnessError::new(
            "configuration",
            "empty fixture selection",
        ));
    }
    for (index, fixture) in fixtures.iter().enumerate() {
        if fixtures[..index].iter().any(|other| other.id == fixture.id) {
            return Err(HarnessError::new(
                "configuration",
                "duplicate fixture identity",
            ));
        }
    }
    Ok(())
}

fn validate_captures(
    inputs: &[CaptureInput<'_>],
    captures: &[CapturedFixture],
) -> Result<(), HarnessError> {
    if inputs.len() != captures.len()
        || inputs
            .iter()
            .zip(captures)
            .any(|(input, capture)| input.id != capture.id || input.source != capture.source)
    {
        return Err(HarnessError::new(
            "required-data",
            "incomplete or mismatched Chromium capture correspondence",
        ));
    }
    Ok(())
}

fn execute_selected(
    fixtures: &[&Fixture],
    executable: Option<&Path>,
) -> Result<report::Report, Failure> {
    validate_selection(fixtures)?;
    let inputs: Vec<_> = fixtures
        .iter()
        .filter_map(|fixture| match fixture.kind {
            FixtureKind::Runnable { html, .. } => Some(CaptureInput {
                id: fixture.id,
                source: fixture.source,
                html,

                request: fixture.request,
            }),
            _ => None,
        })
        .collect();
    // Phase 1 is a whole-invocation boundary. Do not move this into the loop
    // below: execute_html spawns workers whose termination is not joined.
    let captures = if inputs.is_empty() {
        vec![]
    } else {
        let executable = executable.ok_or_else(|| {
            chromium::configuration("set BORROWSER_CHROMIUM_EXECUTABLE or --chromium-executable")
        })?;
        chromium::capture_batch(&inputs, executable)?
    };
    compare_captured(fixtures, captures)
}

// Both backends enter here only after their complete capture/runtime cleanup.
fn compare_captured(
    fixtures: &[&Fixture],
    captures: Vec<CapturedFixture>,
) -> Result<report::Report, Failure> {
    validate_selection(fixtures)?;
    let inputs: Vec<_> = fixtures
        .iter()
        .filter_map(|fixture| match fixture.kind {
            FixtureKind::Runnable { html, .. } => Some(CaptureInput {
                id: fixture.id,
                source: fixture.source,
                html,

                request: fixture.request,
            }),
            _ => None,
        })
        .collect();
    validate_captures(&inputs, &captures)?;
    let mut captures = captures.into_iter();
    let mut cases = Vec::with_capacity(fixtures.len());
    // Phase 2 never launches children or calls the native launcher/reaper.
    for fixture in fixtures {
        let case = match fixture.kind {
            FixtureKind::Runnable { html, .. } => {
                let capture = captures
                    .next()
                    .ok_or_else(|| HarnessError::new("required-data", "missing Chromium sample"))?;
                let observed = execute::execute_fixture(html, fixture.request)?;
                report::Case::observed(fixture, observed, capture.capture)?
            }
            FixtureKind::Unsupported { reason } => report::Case::unsupported(fixture, reason),
            FixtureKind::Skipped { reason } => report::Case::skipped(fixture, reason),
        };
        cases.push(case);
    }
    Ok(report::Report::new(cases))
}

pub(crate) fn publish_captured(
    fixtures: &[&Fixture],
    captures: Vec<CapturedFixture>,
    out: &mut impl Write,
    diagnostics: &mut impl Write,
) -> io::Result<u8> {
    publish(compare_captured(fixtures, captures), out, diagnostics)
}

// Execution, correspondence checks, construction and serialization all finish
// before the first output byte. Write/flush errors cannot retract prior bytes.
fn publish(
    result: Result<report::Report, Failure>,
    out: &mut impl Write,
    diagnostics: &mut impl Write,
) -> io::Result<u8> {
    let report = match result {
        Ok(report) => report,
        Err(failure) => {
            failure.write_diagnostics(diagnostics)?;
            return Ok(2);
        }
    };
    let code = report.exit_code();
    let mut bytes = match serde_json::to_vec_pretty(&report) {
        Ok(bytes) => bytes,
        Err(error) => {
            writeln!(diagnostics, "ERROR comparison.serialize: {error}")?;
            return Ok(2);
        }
    };
    bytes.push(b'\n');
    if let Err(error) = out.write_all(&bytes).and_then(|()| out.flush()) {
        writeln!(diagnostics, "ERROR comparison.write: {error}")?;
        return Ok(2);
    }
    Ok(code)
}

pub(crate) fn run(
    args: &[String],
    out: &mut impl Write,
    diagnostics: &mut impl Write,
) -> io::Result<u8> {
    let result = chromium::options(args)
        .map_err(Failure::from)
        .and_then(|options| {
            let fixtures = options.select_fixtures()?;
            execute_selected(&fixtures, options.executable.as_deref())
        });
    publish(result, out, diagnostics)
}
