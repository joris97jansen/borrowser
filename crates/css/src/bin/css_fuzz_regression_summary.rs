use css::fuzz_regressions::{
    CssFuzzRegressionProfile, CssFuzzRegressionTool,
    render_css_fuzz_regression_summary_with_profile,
};
use css::syntax::derive_css_fuzz_seed;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut tool = None;
    let mut input = None;
    let mut profile = None;
    let mut seed = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--tool" => tool = args.next(),
            "--input" => input = args.next().map(PathBuf::from),
            "--profile" => profile = args.next(),
            "--seed" => seed = args.next(),
            other => {
                return Err(format!(
                    "unexpected argument '{other}'. usage: cargo run -p css --features css-fuzzing --bin css_fuzz_regression_summary -- --tool <tool> --input <path> [--profile <default|selector-limit-zero>] [--seed <u64|derived>]"
                ));
            }
        }
    }

    let tool = tool
        .as_deref()
        .and_then(CssFuzzRegressionTool::parse)
        .ok_or_else(|| "missing or unsupported --tool".to_string())?;
    let profile = match profile.as_deref() {
        None => CssFuzzRegressionProfile::Default,
        Some(value) => CssFuzzRegressionProfile::parse(value)
            .ok_or_else(|| format!("unsupported --profile '{value}'"))?,
    };
    let input = input.ok_or_else(|| "missing --input".to_string())?;
    let bytes = fs::read(&input)
        .map_err(|err| format!("failed to read input {}: {err}", input.display()))?;
    let seed = match seed.as_deref() {
        None | Some("derived") => derive_css_fuzz_seed(&bytes),
        Some(raw) => raw
            .parse::<u64>()
            .map_err(|err| format!("invalid --seed value '{raw}': {err}"))?,
    };

    let summary = render_css_fuzz_regression_summary_with_profile(tool, profile, &bytes, seed)?;
    print!("{summary}");
    Ok(())
}
