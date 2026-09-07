use std::{fs, path::Path};

pub fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

pub fn repository() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temp = tempfile::tempdir().unwrap();
    copy_tree(
        &root.join("tests/conformance"),
        &temp.path().join("tests/conformance"),
    );
    copy_tree(&root.join("docs"), &temp.path().join("docs"));
    // Copy the two reviewed repository-owned AG9c source files.
    for relative in [
        conformance_runner::CAPTURE_ALGORITHM_PATH_V1,
        conformance_runner::CAPTURE_CONFIGURATION_PATH_V1,
    ] {
        let target = temp.path().join(relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(root.join(relative), target).unwrap();
    }
    temp
}
