use std::fs;
use std::path::{Path, PathBuf};

pub struct TestRepository {
    temporary: tempfile::TempDir,
}

impl TestRepository {
    pub fn new() -> Self {
        let temporary = tempfile::tempdir().expect("temporary repository");
        fs::create_dir_all(temporary.path().join("tests/conformance/fixtures"))
            .expect("fixture root");
        Self { temporary }
    }

    pub fn root(&self) -> &Path {
        self.temporary.path()
    }

    pub fn fixture_root(&self) -> PathBuf {
        self.root().join("tests/conformance/fixtures")
    }

    pub fn bundle(&self, relative: &str, descriptor: &str, payloads: &[(&str, &[u8])]) {
        let root = self.fixture_root().join(relative);
        fs::create_dir_all(&root).expect("bundle root");
        fs::write(root.join("fixture.toml"), descriptor).expect("fixture descriptor");
        for (path, bytes) in payloads {
            let path = root.join(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("payload parent");
            }
            fs::write(path, bytes).expect("fixture payload");
        }
    }

    pub fn repository(&self) -> conformance_test_support::InventoryRepository {
        conformance_test_support::InventoryRepository::new(self.root(), self.fixture_root())
    }
}

pub fn descriptor(id: &str, observation: &str, test_path: &str) -> String {
    format!(
        r#"format = "borrowser-conformance-fixture-v1"
id = "{id}"
scope = "static-html-css-no-js"
observation = "{observation}"
test_path = "{test_path}"

[source]
kind = "native"

[metadata]
description = "Temporary inventory fixture."
"#
    )
}

pub fn descriptor_with_reference(
    id: &str,
    observation: &str,
    test_path: &str,
    reference_kind: &str,
    reference_path: &str,
) -> String {
    format!(
        r#"format = "borrowser-conformance-fixture-v1"
id = "{id}"
scope = "static-html-css-no-js"
observation = "{observation}"
test_path = "{test_path}"

[source]
kind = "native"

[reference]
kind = "{reference_kind}"
path = "{reference_path}"

[metadata]
description = "Temporary reference inventory fixture."
"#
    )
}
