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
#[allow(
    dead_code,
    reason = "some variants require an explicitly enabled adapter"
)]
enum CliError {
    Arguments,
    Feature(&'static str),
    Run(String),
    Report(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Arguments => {
                f.write_str("usage: conformance-runner [--css|--rendering] [--check]")
            }
            Self::Feature(feature) => write!(
                f,
                "conformance runner adapter feature is not enabled: {feature}"
            ),
            Self::Run(error) => write!(f, "conformance execution failed: {error}"),
            Self::Report(error) => write!(f, "conformance report publication failed: {error}"),
        }
    }
}

fn run() -> Result<i32, CliError> {
    let mut check = false;
    let mut css = false;
    let mut rendering = false;
    for argument in std::env::args().skip(1) {
        if argument == "--check" && !check {
            check = true;
        } else if argument == "--css" && !css {
            css = true;
        } else if argument == "--rendering" && !rendering {
            rendering = true;
        } else {
            return Err(CliError::Arguments);
        }
    }
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is a direct workspace child");
    if css && rendering {
        Err(CliError::Arguments)
    } else if rendering {
        run_rendering(repository_root, check)
    } else if css {
        run_css(repository_root, check)
    } else {
        run_parser(repository_root, check)
    }
}

#[cfg(feature = "html-parser")]
fn run_parser(repository_root: &Path, check: bool) -> Result<i32, CliError> {
    let summary = conformance_runner::run_repository_parser_cases(repository_root)
        .map_err(|error| CliError::Run(error.to_string()))?;
    conformance_runner::build_and_write_report(summary.cases(), &mut std::io::stdout().lock())
        .map_err(|error| CliError::Report(error.to_string()))?;
    Ok(i32::from(check && summary.has_unexpected_results()))
}
#[cfg(not(feature = "html-parser"))]
fn run_parser(_: &Path, _: bool) -> Result<i32, CliError> {
    Err(CliError::Feature("html-parser"))
}

#[cfg(feature = "css")]
fn run_css(repository_root: &Path, check: bool) -> Result<i32, CliError> {
    let summary = conformance_runner::run_repository_css_cases(repository_root)
        .map_err(|error| CliError::Run(error.to_string()))?;
    conformance_runner::build_and_write_css_report(summary.cases(), &mut std::io::stdout().lock())
        .map_err(|error| CliError::Report(error.to_string()))?;
    Ok(i32::from(check && summary.has_unexpected_results()))
}
#[cfg(not(feature = "css"))]
fn run_css(_: &Path, _: bool) -> Result<i32, CliError> {
    Err(CliError::Feature("css"))
}

#[cfg(feature = "rendering")]
fn run_rendering(repository_root: &Path, check: bool) -> Result<i32, CliError> {
    let summary = conformance_runner::run_repository_rendering_cases(repository_root)
        .map_err(|error| CliError::Run(error.to_string()))?;
    conformance_runner::build_and_write_rendering_report(
        summary.cases(),
        &mut std::io::stdout().lock(),
    )
    .map_err(|error| CliError::Report(error.to_string()))?;
    Ok(i32::from(check && summary.has_unexpected_results()))
}
#[cfg(not(feature = "rendering"))]
fn run_rendering(_: &Path, _: bool) -> Result<i32, CliError> {
    Err(CliError::Feature("rendering"))
}
