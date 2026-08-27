use std::path::PathBuf;

use conformance_test_support::{
    InventoryRepository, ManifestCheck, build_manifest, check_manifest, discover_inventory,
    update_manifest,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operation {
    Check,
    Update,
}

fn main() {
    let operation = match parse_operation() {
        Ok(operation) => operation,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    if let Err(error) = run(operation) {
        eprintln!("conformance manifest operation failed:\n{error}");
        std::process::exit(1);
    }
}

fn parse_operation() -> Result<Operation, String> {
    let mut arguments = std::env::args_os().skip(1);
    let operation = arguments.next();
    if arguments.next().is_some() {
        return Err(
            "unexpected trailing arguments; use no argument, --check, or --update".to_owned(),
        );
    }
    match operation.as_deref() {
        None => Ok(Operation::Check),
        Some(value) if value == "--check" => Ok(Operation::Check),
        Some(value) if value == "--update" => Ok(Operation::Update),
        Some(value) => match value.to_str() {
            Some(value) => Err(format!(
                "unsupported operation '{value}'; use --check or --update"
            )),
            None => Err("unsupported non-UTF-8 operation; use --check or --update".to_owned()),
        },
    }
}

fn run(operation: Operation) -> Result<(), String> {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repository_root = crate_root
        .parent()
        .and_then(|path| path.parent())
        .ok_or_else(|| "conformance crate is not under the repository crates directory".to_owned())?
        .to_path_buf();
    let fixture_root = repository_root.join("tests/conformance/fixtures");
    let output_path = repository_root.join("tests/conformance/manifest.toml");
    let inventory = discover_inventory(&InventoryRepository::new(
        repository_root.clone(),
        fixture_root,
    ))
    .map_err(|errors| errors.to_string())?;
    let manifest = build_manifest(&inventory);

    match operation {
        Operation::Check => match check_manifest(&repository_root, &output_path, &manifest)
            .map_err(|error| error.to_string())?
        {
            ManifestCheck::Current => {
                println!(
                    "conformance manifest is current: {} fixtures",
                    manifest.entries().len()
                );
                Ok(())
            }
            ManifestCheck::Stale => {
                Err("checked-in conformance manifest is stale; rerun with --update".to_owned())
            }
            ManifestCheck::Missing => {
                Err("checked-in conformance manifest is missing; rerun with --update".to_owned())
            }
        },
        Operation::Update => {
            update_manifest(&repository_root, &output_path, &manifest)
                .map_err(|error| error.to_string())?;
            println!(
                "updated conformance manifest: {} fixtures",
                manifest.entries().len()
            );
            Ok(())
        }
    }
}
