mod execute;
mod fixtures;
mod model;
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
        FixtureKind::Runnable { html, .. } => match execute::execute_html(html) {
            Ok(color) => Execution::Observed(color),
            Err(error) => Execution::Error(error),
        },
        FixtureKind::Unsupported { reason } => Execution::Unsupported { reason },
        FixtureKind::Skipped { reason } => Execution::Skipped { reason },
    }
}

fn run(args: &[String], out: &mut impl Write) -> io::Result<u8> {
    if args == ["--help"] {
        writeln!(
            out,
            "Usage: borrowser-conformance [TEST_ID]\nRun the explicit fixture set, or one exact test identity."
        )?;
        return Ok(0);
    }
    let selected: Vec<_> = match args {
        [] => fixtures::FIXTURES.iter().collect(),
        [id] => match fixtures::FIXTURES.iter().find(|fixture| fixture.id.0 == id) {
            Some(fixture) => vec![fixture],
            None => {
                writeln!(out, "ERROR command: unknown test identity {id:?}")?;
                return Ok(2);
            }
        },
        _ => {
            writeln!(out, "ERROR command: expected at most one test identity")?;
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
            eprintln!("ERROR command: test identity must be UTF-8");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(_) => {
            eprintln!("ERROR report.write: unable to write report");
            ExitCode::from(2)
        }
    }
}
