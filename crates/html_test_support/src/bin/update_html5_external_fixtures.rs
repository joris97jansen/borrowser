use html_test_support::external_wpt::{
    ExternalAdapterOutput, ExternalCaseClassification, adapt_allowlisted_subset,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operation {
    Check,
    Update,
}

fn main() {
    let operation = match std::env::args().nth(1).as_deref() {
        None | Some("--check") => Operation::Check,
        Some("--update") => Operation::Update,
        Some(value) => {
            eprintln!("unsupported operation {value}; use --check or --update");
            std::process::exit(2);
        }
    };
    if let Err(error) = run(operation) {
        eprintln!("external fixture operation failed: {error}");
        std::process::exit(1);
    }
}

fn run(operation: Operation) -> Result<(), String> {
    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw_root = repository_root.join("tests/wpt/external/raw");
    let allowlist = repository_root.join("tests/wpt/external/allowlist.toml");
    let output_root = repository_root.join("crates/html/tests/fixtures/html5/external-wpt");
    validate_output_root(&repository_root, &output_root)?;
    let adapted =
        adapt_allowlisted_subset(&raw_root, &allowlist).map_err(|error| error.to_string())?;
    let desired = generated_files(&adapted)?;
    let report = compare_output(&output_root, &desired)?;
    println!("external WPT fixture report:");
    println!("  added: {}", report.added.len());
    for name in &report.added {
        println!("    + {name}");
    }
    println!("  removed: {}", report.removed.len());
    for name in &report.removed {
        println!("    - {name}");
    }
    println!("  changed: {}", report.changed.len());
    for name in &report.changed {
        println!("    ~ {name}");
    }
    println!("  unchanged: {}", report.unchanged.len());
    for name in &report.unchanged {
        println!("    = {name}");
    }
    println!(
        "  unsupported records: {}",
        adapted.artifacts().len() - desired.len()
    );
    for artifact in adapted.artifacts() {
        if let ExternalCaseClassification::Unsupported(capability) = artifact.classification() {
            println!("    ! {}: {}", artifact.case_identity(), capability.name());
        }
    }

    if operation == Operation::Check {
        if report.has_changes() {
            return Err("generated external fixtures are stale; rerun with --update".to_string());
        }
        return Ok(());
    }
    apply_update(&output_root, &desired, &report)
}

fn validate_output_root(repository_root: &Path, output_root: &Path) -> Result<(), String> {
    if !output_root.starts_with(repository_root)
        || output_root.file_name().and_then(|name| name.to_str()) != Some("external-wpt")
    {
        return Err(format!(
            "refusing output root outside restricted external-wpt directory: {}",
            output_root.display()
        ));
    }
    if output_root.exists()
        && fs::symlink_metadata(output_root)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
    {
        return Err(format!(
            "refusing symlinked generated output root: {}",
            output_root.display()
        ));
    }
    Ok(())
}

fn generated_files(
    adapted: &ExternalAdapterOutput,
) -> Result<BTreeMap<String, BTreeMap<String, Vec<u8>>>, String> {
    let mut desired = BTreeMap::new();
    for artifact in adapted.artifacts() {
        if artifact.classification() == &ExternalCaseClassification::Eligible
            && desired
                .insert(artifact.bundle_name().to_string(), artifact.files().clone())
                .is_some()
        {
            return Err(format!(
                "duplicate generated bundle {}",
                artifact.bundle_name()
            ));
        }
        for relative in artifact.files().keys() {
            ValidatedArtifactPath::parse(relative)?;
        }
    }
    Ok(desired)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ValidatedArtifactPath(String);

impl ValidatedArtifactPath {
    fn parse(value: &str) -> Result<Self, String> {
        if value.is_empty()
            || value.contains('\\')
            || value.contains(':')
            || value.split('/').any(|component| component.is_empty())
        {
            return Err(format!("invalid generated artifact path: {value:?}"));
        }
        let path = Path::new(value);
        if path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::Prefix(_)
                        | std::path::Component::RootDir
                        | std::path::Component::CurDir
                        | std::path::Component::ParentDir
                )
            })
        {
            return Err(format!("invalid generated artifact path: {value:?}"));
        }
        Ok(Self(value.to_string()))
    }

    fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}

fn validate_bundle_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains(':')
    {
        return Err(format!("invalid generated bundle name: {name:?}"));
    }
    Ok(())
}

#[derive(Default)]
struct Report {
    added: Vec<String>,
    removed: Vec<String>,
    changed: Vec<String>,
    unchanged: Vec<String>,
}

impl Report {
    fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty() || !self.changed.is_empty()
    }
}

fn compare_output(
    output_root: &Path,
    desired: &BTreeMap<String, BTreeMap<String, Vec<u8>>>,
) -> Result<Report, String> {
    let existing = existing_bundles(output_root)?;
    let mut report = Report::default();
    for name in desired.keys() {
        match existing.get(name) {
            None => report.added.push(name.clone()),
            Some(path) => {
                let Some(bundle) = desired.get(name) else {
                    return Err(format!("desired bundle disappeared: {name}"));
                };
                if bundle_matches(path, bundle)? {
                    report.unchanged.push(name.clone());
                } else {
                    report.changed.push(name.clone());
                }
            }
        }
    }
    for name in existing.keys() {
        if !desired.contains_key(name) {
            report.removed.push(name.clone());
        }
    }
    Ok(report)
}

fn existing_bundles(output_root: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut bundles = BTreeMap::new();
    if !output_root.exists() {
        return Ok(bundles);
    }
    let entries = fs::read_dir(output_root).map_err(|error| error.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "symlinked generated bundle is not allowed: {}",
                path.display()
            ));
        }
        if metadata.is_dir() {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "generated bundle path is not UTF-8".to_string())?;
            bundles.insert(name, path);
        } else {
            return Err(format!(
                "unexpected file in generated output root: {}",
                path.display()
            ));
        }
    }
    Ok(bundles)
}

fn bundle_matches(path: &Path, desired: &BTreeMap<String, Vec<u8>>) -> Result<bool, String> {
    let mut actual = BTreeMap::new();
    collect_files(path, path, &mut actual)?;
    if actual.len() != desired.len() {
        return Ok(false);
    }
    for (relative, expected) in desired {
        if actual.get(relative) != Some(expected) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let entries = fs::read_dir(directory).map_err(|error| error.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "symlinked generated artifact is not allowed: {}",
                path.display()
            ));
        }
        if metadata.is_dir() {
            collect_files(root, &path, files)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let relative = ValidatedArtifactPath::parse(&relative)?.0;
            files.insert(relative, fs::read(path).map_err(|error| error.to_string())?);
        }
    }
    Ok(())
}

fn apply_update(
    output_root: &Path,
    desired: &BTreeMap<String, BTreeMap<String, Vec<u8>>>,
    report: &Report,
) -> Result<(), String> {
    fs::create_dir_all(output_root).map_err(|error| error.to_string())?;
    let output_metadata = fs::symlink_metadata(output_root).map_err(|error| error.to_string())?;
    if output_metadata.file_type().is_symlink() || !output_metadata.is_dir() {
        return Err(format!(
            "refusing non-directory or symlinked generated output root: {}",
            output_root.display()
        ));
    }

    for name in report
        .removed
        .iter()
        .chain(report.added.iter())
        .chain(report.changed.iter())
    {
        validate_bundle_name(name)?;
    }

    let stage_root = output_root.join(format!(".ae13e-stage-{}", std::process::id()));
    if stage_root.exists() {
        return Err(format!(
            "staging path already exists: {}",
            stage_root.display()
        ));
    }
    fs::create_dir(&stage_root).map_err(|error| error.to_string())?;

    let build_result = (|| {
        for name in report.added.iter().chain(report.changed.iter()) {
            let Some(bundle) = desired.get(name) else {
                return Err(format!("desired bundle disappeared: {name}"));
            };
            let bundle_root = stage_root.join(name);
            fs::create_dir(&bundle_root).map_err(|error| error.to_string())?;
            for (relative, bytes) in bundle {
                let validated = ValidatedArtifactPath::parse(relative)?;
                let path = bundle_root.join(validated.as_path());
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                }
                fs::write(path, bytes).map_err(|error| error.to_string())?;
            }
        }
        Ok::<(), String>(())
    })();
    if let Err(error) = build_result {
        let _ = fs::remove_dir_all(&stage_root);
        return Err(error);
    }

    for name in &report.removed {
        let path = output_root.join(name);
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            let _ = fs::remove_dir_all(&stage_root);
            return Err(format!(
                "refusing unsafe generated bundle removal: {}",
                path.display()
            ));
        }
        fs::remove_dir_all(path).map_err(|error| error.to_string())?;
    }

    for name in &report.changed {
        let target = output_root.join(name);
        let staged = stage_root.join(name);
        let backup = output_root.join(format!(".ae13e-backup-{}-{name}", std::process::id()));
        if backup.exists() {
            let _ = fs::remove_dir_all(&stage_root);
            return Err(format!("backup path already exists: {}", backup.display()));
        }
        fs::rename(&target, &backup).map_err(|error| error.to_string())?;
        if let Err(error) = fs::rename(&staged, &target) {
            let _ = fs::rename(&backup, &target);
            let _ = fs::remove_dir_all(&stage_root);
            return Err(error.to_string());
        }
        fs::remove_dir_all(backup).map_err(|error| error.to_string())?;
    }
    for name in &report.added {
        fs::rename(stage_root.join(name), output_root.join(name))
            .map_err(|error| error.to_string())?;
    }
    fs::remove_dir_all(stage_root).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_and_update_are_sorted_and_replace_changed_bundles() {
        let temporary = tempfile::tempdir().expect("temporary output root");
        let output_root = temporary.path().join("external-wpt");
        fs::create_dir_all(output_root.join("changed")).expect("changed bundle");
        fs::create_dir_all(output_root.join("unchanged")).expect("unchanged bundle");
        fs::create_dir_all(output_root.join("removed")).expect("removed bundle");
        fs::write(output_root.join("changed/obsolete.txt"), b"old").expect("obsolete file");
        fs::write(output_root.join("unchanged/tree.txt"), b"same").expect("same file");

        let mut desired = BTreeMap::new();
        desired.insert(
            "added".to_string(),
            BTreeMap::from([("tree.txt".to_string(), b"new".to_vec())]),
        );
        desired.insert(
            "changed".to_string(),
            BTreeMap::from([("tree.txt".to_string(), b"replacement".to_vec())]),
        );
        desired.insert(
            "unchanged".to_string(),
            BTreeMap::from([("tree.txt".to_string(), b"same".to_vec())]),
        );

        let report = compare_output(&output_root, &desired).expect("compare output");
        assert_eq!(report.added, vec!["added"]);
        assert_eq!(report.removed, vec!["removed"]);
        assert_eq!(report.changed, vec!["changed"]);
        assert_eq!(report.unchanged, vec!["unchanged"]);

        apply_update(&output_root, &desired, &report).expect("apply explicit update");
        assert!(!output_root.join("removed").exists());
        assert!(!output_root.join("changed/obsolete.txt").exists());
        assert_eq!(
            fs::read(output_root.join("changed/tree.txt")).expect("replacement file"),
            b"replacement"
        );
        assert_eq!(
            fs::read(output_root.join("unchanged/tree.txt")).expect("unchanged file"),
            b"same"
        );
    }

    #[test]
    fn artifact_paths_reject_escape_and_platform_forms_before_joining() {
        for value in [
            "../outside",
            "nested/../../outside",
            "/absolute",
            "C:/absolute",
            "C:\\absolute",
            "",
            ".",
            "nested//file",
        ] {
            assert!(ValidatedArtifactPath::parse(value).is_err(), "{value:?}");
        }
        assert!(ValidatedArtifactPath::parse("nested/file.txt").is_ok());
    }

    #[test]
    fn invalid_artifact_cannot_write_outside_or_partially_replace_a_bundle() {
        let temporary = tempfile::tempdir().expect("temporary output root");
        let output_root = temporary.path().join("external-wpt");
        let changed = output_root.join("changed");
        fs::create_dir_all(&changed).expect("changed bundle");
        fs::write(changed.join("tree.txt"), b"old").expect("old artifact");

        let desired = BTreeMap::from([(
            "changed".to_string(),
            BTreeMap::from([("../outside".to_string(), b"bad".to_vec())]),
        )]);
        let report = Report {
            changed: vec!["changed".to_string()],
            ..Report::default()
        };
        assert!(apply_update(&output_root, &desired, &report).is_err());
        assert_eq!(
            fs::read(changed.join("tree.txt")).expect("old artifact remains"),
            b"old"
        );
        assert!(!temporary.path().join("outside").exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_generated_artifacts_are_rejected() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary output root");
        let output_root = temporary.path().join("external-wpt");
        let bundle = output_root.join("bundle");
        fs::create_dir_all(&bundle).expect("bundle");
        let target = temporary.path().join("outside.txt");
        fs::write(&target, b"outside").expect("target");
        symlink(&target, bundle.join("tree.txt")).expect("artifact symlink");
        let desired = BTreeMap::from([(
            "bundle".to_string(),
            BTreeMap::from([("tree.txt".to_string(), b"replacement".to_vec())]),
        )]);
        assert!(compare_output(&output_root, &desired).is_err());

        let linked_root = temporary.path().join("linked-root");
        symlink(&output_root, &linked_root).expect("output root symlink");
        assert!(validate_output_root(temporary.path(), &linked_root).is_err());
    }
}
