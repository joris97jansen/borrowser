use std::io::Write;
use std::path::PathBuf;

use conformance_test_support::{
    InventoryRepository, discover_inventory, load_expected_results,
    serialize_expected_results_summary,
};

fn main() {
    if let Err(message) = parse_operation() {
        eprintln!("{message}");
        std::process::exit(2);
    }
    if let Err(error) = run() {
        eprintln!("conformance expected-results check failed:\n{error}");
        std::process::exit(1);
    }
}

fn parse_operation() -> Result<(), String> {
    let mut arguments = std::env::args_os().skip(1);
    let operation = arguments.next();
    if arguments.next().is_some() {
        return Err("unexpected trailing arguments; use no argument or --check".to_owned());
    }
    match operation.as_deref() {
        None => Ok(()),
        Some(value) if value == "--check" => Ok(()),
        Some(value) => match value.to_str() {
            Some(value) => Err(format!(
                "unsupported operation '{value}'; use no argument or --check"
            )),
            None => Err("unsupported non-UTF-8 operation; use no argument or --check".to_owned()),
        },
    }
}

fn run() -> Result<(), String> {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repository_root = crate_root
        .parent()
        .and_then(|path| path.parent())
        .ok_or_else(|| "conformance crate is not under the repository crates directory".to_owned())?
        .to_path_buf();
    let fixture_root = repository_root.join("tests/conformance/fixtures");
    let inventory = discover_inventory(&InventoryRepository::new(
        repository_root.clone(),
        fixture_root,
    ))
    .map_err(|errors| errors.to_string())?;
    let expected_results =
        load_expected_results(&repository_root, &inventory).map_err(|errors| errors.to_string())?;
    let summary = serialize_expected_results_summary(&expected_results);
    std::io::stdout()
        .write_all(&summary)
        .map_err(|error| format!("failed to write deterministic summary: {error}"))
}
