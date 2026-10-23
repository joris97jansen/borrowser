#[cfg(any(target_os = "linux", target_os = "macos"))]
mod aws;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod chromium;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod comparison;
mod environment;
mod execute;
mod fixtures;
mod metadata_links;
mod model;
mod projection;
mod relationship;
mod report;
#[cfg(test)]
mod tests;

use model::{Execution, Fixture, FixtureKind};
use std::{
    io::{self, Write},
    process::ExitCode,
};

fn run_fixture(fixture: &Fixture) -> Execution {
    match fixture.kind {
        FixtureKind::Runnable { .. } => {
            let result = (|| {
                if fixture.requires_javascript {
                    return Err(model::HarnessError::new(
                        "fixture.configuration",
                        "runnable JavaScript requirement",
                    ));
                }
                let test = execute::execute_fixture(fixture.document.html, fixture.request)?;
                Ok(if let Some((_, reference)) = fixture.reference() {
                    Execution::ReferenceObserved {
                        test,
                        reference: Box::new(execute::execute_fixture(
                            reference.html,
                            fixture.request,
                        )?),
                    }
                } else {
                    Execution::Observed(test)
                })
            })();
            result.unwrap_or_else(Execution::Error)
        }
        FixtureKind::Unsupported { reason } => Execution::Unsupported { reason },
        FixtureKind::Skipped { reason } => Execution::Skipped { reason },
    }
}

fn run(args: &[String], out: &mut impl Write) -> io::Result<u8> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    match args.first().map(String::as_str) {
        Some("--ag4-eice-proxy") => return aws::eice_proxy(&args[1..]),
        Some("--compare-aws") => return aws::run(&args[1..], out, &mut io::stderr().lock()),
        Some("--aws-cleanup") => return aws::cleanup(&args[1..], &mut io::stderr().lock()),
        Some("--ag4-artifact-info") if args.len() == 1 => return aws::artifact_info(out),
        _ => {}
    }
    if args.first().map(String::as_str) == Some("--compare") {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        return comparison::run(&args[1..], out, &mut io::stderr().lock());
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            writeln!(
                io::stderr().lock(),
                "ERROR chromium.platform: local comparison requires native Linux x86-64"
            )?;
            return Ok(2);
        }
    }
    if args.first().map(String::as_str) == Some("--chromium") {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        return chromium::run(&args[1..], out);
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            writeln!(
                io::stderr().lock(),
                "ERROR chromium.platform: local capture requires native Linux x86-64"
            )?;
            return Ok(2);
        }
    }
    if args == ["--help"] {
        writeln!(
            out,
            "Usage: borrowser-conformance [TEST_ID]\nRun the embedded discovered fixture set, or one exact test identity.\n       borrowser-conformance --chromium [--chromium-executable PATH] [TEST_ID]\nCapture only, using the pinned Chrome for Testing (or BORROWSER_CHROMIUM_EXECUTABLE).\n       borrowser-conformance --compare [--chromium-executable PATH] [TEST_ID]\nCompare the requested static DOM, computed-style, fixed-layout, and guarded canvas observations.\n       borrowser-conformance --compare-aws --config PATH --artifact-dir PATH --state-dir PATH [TEST_ID]\n       borrowser-conformance --aws-cleanup --operation PATH --aws-cli PATH\nSee docs/conformance/ag4-disposable-aws-chromium-runner.md for AG4 prerequisites and qualification."
        )?;
        return Ok(0);
    }
    let id = match args {
        [] => None,
        [id] => Some(id.as_str()),
        _ => {
            writeln!(out, "ERROR command: expected at most one test identity")?;
            return Ok(2);
        }
    };
    let selected = match fixtures::select(id) {
        Ok(selected) => selected,
        Err(error) => {
            let code = if error.code == "fixture.selection" {
                "command"
            } else {
                error.code
            };
            writeln!(out, "ERROR {code}: {}", error.detail)?;
            return Ok(2);
        }
    };
    let reports: Vec<_> = selected
        .into_iter()
        .map(|fixture| report::evaluate(fixture, run_fixture(fixture)))
        .collect();
    report::write_report(&reports, out)?;
    Ok(report::exit_code(&reports))
}

fn main() -> ExitCode {
    let args: Result<Vec<_>, _> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.into_string())
        .collect();
    let result = match args {
        Ok(args) => run(&args, &mut io::stdout().lock()),
        Err(_) => {
            // Terminal diagnostics are best effort; failed stderr must not
            // replace the required error exit with a panic.
            let _ = writeln!(
                io::stderr().lock(),
                "ERROR command: test identity must be UTF-8"
            );
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(_) => {
            // Inner helpers remain fallible. At this final boundary, preserve
            // exit 2 even when the diagnostic destination has also failed.
            let _ = writeln!(
                io::stderr().lock(),
                "ERROR report.write: unable to write report"
            );
            ExitCode::from(2)
        }
    }
}
