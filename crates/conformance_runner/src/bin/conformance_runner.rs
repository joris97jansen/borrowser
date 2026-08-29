use std::path::Path;

fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            2
        }
    };
    std::process::exit(code);
}

#[derive(Debug)]
enum CliError {
    Arguments,
    Run(conformance_runner::ParserRunError),
    Report(conformance_runner::ReportPublicationError),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Arguments => f.write_str("usage: conformance-runner [--check]"),
            Self::Run(error) => write!(f, "conformance execution failed: {error}"),
            Self::Report(error) => write!(f, "conformance report publication failed: {error}"),
        }
    }
}

fn run() -> Result<i32, CliError> {
    let mut check = false;
    for argument in std::env::args().skip(1) {
        if argument == "--check" && !check {
            check = true;
        } else {
            return Err(CliError::Arguments);
        }
    }
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is a direct workspace child");
    let summary =
        conformance_runner::run_repository_parser_cases(repository_root).map_err(CliError::Run)?;
    // The complete bounded report is constructed before stdout publication.
    // A transport failure may still occur after stdout accepted a prefix.
    conformance_runner::build_and_write_report(summary.cases(), &mut std::io::stdout().lock())
        .map_err(CliError::Report)?;
    Ok(if check && summary.has_unexpected_results() {
        1
    } else {
        0
    })
}
