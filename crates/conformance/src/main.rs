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

// CLI selection is a harness concern. Chromium receives CaptureInput values;
// neither the native adapter nor AWS transport knows named suite policy.
struct Options<'a> {
    suite: Option<fixtures::Suite>,
    test_id: Option<&'a str>,
    executable: Option<std::path::PathBuf>,
}

fn options(args: &[String], chromium: bool) -> Result<Options<'_>, model::HarnessError> {
    let invalid = |detail| model::HarnessError::new("command", detail);
    let mut options = Options {
        suite: None,
        test_id: None,
        executable: if chromium {
            std::env::var_os("BORROWSER_CHROMIUM_EXECUTABLE").map(Into::into)
        } else {
            None
        },
    };
    let mut explicit_executable = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--suite" if options.suite.is_none() => {
                options.suite = Some(match args.next().map(String::as_str) {
                    Some("ci") => fixtures::Suite::Ci,
                    Some("extended") => fixtures::Suite::Extended,
                    _ => return Err(invalid("--suite requires ci or extended")),
                });
            }
            "--chromium-executable" if chromium && !explicit_executable => {
                let value = args
                    .next()
                    .filter(|v| !v.starts_with('-'))
                    .ok_or_else(|| invalid("--chromium-executable requires a path"))?;
                options.executable = Some(value.into());
                explicit_executable = true;
            }
            _ if !arg.starts_with('-') && options.test_id.is_none() => {
                options.test_id = Some(arg);
            }
            _ => {
                return Err(invalid(
                    "unexpected or duplicate option; expected at most one test identity",
                ));
            }
        }
    }
    Ok(options)
}
impl Options<'_> {
    fn select_fixtures(&self) -> Result<Vec<&'static Fixture>, model::HarnessError> {
        fixtures::select_suite(self.suite, self.test_id)
    }
}

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
                let test = execute::execute_fixture(fixture.document.html, fixture.request)
                    .map_err(|e| e.with_document(fixture.id, "test", fixture.document.source))?;
                Ok(if let Some((_, reference)) = fixture.reference() {
                    Execution::ReferenceObserved {
                        test,
                        reference: Box::new(
                            execute::execute_fixture(reference.html, fixture.request).map_err(
                                |e| e.with_document(fixture.id, "reference", reference.source),
                            )?,
                        ),
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
    let result = run_selected(args, out);
    if matches!(result, Ok(2) | Err(_))
        && !matches!(
            args.first().map(String::as_str),
            Some("--ag4-eice-proxy" | "--ag4-artifact-info" | "--aws-cleanup")
        )
    {
        // Known CLI arguments only, shell quoted. Never dump the environment.
        let _ = report::write_invocation_reproduction(args, &mut io::stderr().lock());
    }
    result
}

fn run_selected(args: &[String], out: &mut impl Write) -> io::Result<u8> {
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
        return run_chromium(&args[1..], out, &mut io::stderr().lock());
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
            "Usage: borrowser-conformance [--suite ci|extended] [TEST_ID]\nRun the embedded discovered fixture set, a bounded named suite, or one exact test identity.\n       borrowser-conformance --chromium [--suite ci|extended] [--chromium-executable PATH] [TEST_ID]\nCapture only, using the pinned Chrome for Testing (or BORROWSER_CHROMIUM_EXECUTABLE).\n       borrowser-conformance --compare [--suite ci|extended] [--chromium-executable PATH] [TEST_ID]\nCompare the requested static DOM, computed-style, fixed-layout, and guarded canvas observations.\n       borrowser-conformance --compare-aws --config PATH --artifact-dir PATH --state-dir PATH [TEST_ID]\n       borrowser-conformance --aws-cleanup --operation PATH --aws-cli PATH\nSee docs/conformance/ag4-disposable-aws-chromium-runner.md for AG4 prerequisites and qualification."
        )?;
        return Ok(0);
    }
    let options = match options(args, false) {
        Ok(options) => options,
        Err(error) => {
            writeln!(out, "ERROR {}: {}", error.code, error.detail)?;
            report::write_run_error(
                &mut io::stderr().lock(),
                "harness",
                error.code,
                "not_published",
            )?;
            return Ok(2);
        }
    };
    let selected = match options.select_fixtures() {
        Ok(selected) => selected,
        Err(error) => {
            let code = if error.code == "fixture.selection" {
                "command"
            } else {
                error.code
            };
            writeln!(out, "ERROR {code}: {}", error.detail)?;
            report::write_run_error(
                &mut io::stderr().lock(),
                "harness",
                error.code,
                "not_published",
            )?;
            return Ok(2);
        }
    };
    let reports: Vec<_> = selected
        .into_iter()
        .map(|fixture| report::evaluate(fixture, run_fixture(fixture)))
        .collect();
    report::write_suite_report(&reports, options.suite, out)?;
    Ok(report::exit_code(&reports))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_chromium(
    args: &[String],
    out: &mut impl Write,
    diagnostics: &mut impl Write,
) -> io::Result<u8> {
    let selected = options(args, true).and_then(|options| {
        let fixtures = options.select_fixtures()?;
        Ok((options, chromium::inputs(&fixtures)))
    });
    match selected {
        Ok((options, inputs)) => {
            chromium::run(&inputs, options.executable.as_deref(), out, diagnostics)
        }
        Err(error) => {
            writeln!(diagnostics, "ERROR {}: {}", error.code, error.detail)?;
            report::write_run_error(diagnostics, "harness", error.code, "not_published")?;
            Ok(2)
        }
    }
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
            let _ = report::write_run_error(
                &mut io::stderr().lock(),
                "harness",
                "command",
                "not_published",
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
            let _ = report::write_run_error(
                &mut io::stderr().lock(),
                "harness",
                "report.write",
                "publication_failed",
            );
            ExitCode::from(2)
        }
    }
}
