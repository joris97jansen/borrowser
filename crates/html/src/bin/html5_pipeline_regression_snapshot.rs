#[cfg(all(feature = "html5", feature = "html5-fuzzing", feature = "dom-snapshot"))]
fn main() {
    if let Err(err) = real_main() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

#[cfg(all(feature = "html5", feature = "html5-fuzzing", feature = "dom-snapshot"))]
fn real_main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let input_path = args.next().ok_or_else(|| {
        "usage: cargo run -p html --features \"html5 html5-fuzzing dom-snapshot parser_invariants\" --bin html5_pipeline_regression_snapshot -- <input-path> [label]".to_string()
    })?;
    let label = match args.next() {
        Some(label) => label,
        None => std::path::Path::new(&input_path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("html5-pipeline-regression")
            .to_string(),
    };
    if args.next().is_some() {
        return Err("expected at most two arguments: <input-path> [label]".to_string());
    }

    let bytes = std::fs::read(&input_path)
        .map_err(|err| format!("failed to read regression input {input_path}: {err}"))?;
    let snapshot = html::html5::render_html5_pipeline_regression_snapshot(&bytes, &label)
        .map_err(|err| format!("failed to render regression snapshot for {input_path}: {err}"))?;
    print!("{snapshot}");
    Ok(())
}

#[cfg(not(all(feature = "html5", feature = "html5-fuzzing", feature = "dom-snapshot")))]
fn main() {
    eprintln!(
        "html5_pipeline_regression_snapshot requires features: html5 html5-fuzzing dom-snapshot"
    );
    std::process::exit(1);
}
