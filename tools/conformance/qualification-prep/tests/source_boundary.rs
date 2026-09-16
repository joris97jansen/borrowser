use qualification_prep::{canonical, source_identity as source};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap()
}
fn git_file(root: &Path, path: &str) -> Vec<u8> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["show", &format!("{}:{path}", source::REVISION)])
        .output()
        .unwrap();
    assert!(out.status.success());
    out.stdout
}
#[test]
fn frozen_manifests_and_all_source_members_are_unchanged() {
    let root = root();
    source::verify_sources(&root).unwrap();
    for p in [
        "Cargo.toml",
        "Cargo.lock",
        source::COLLECTOR_PATH,
        source::QUALIFICATION_PATH,
    ] {
        assert_eq!(std::fs::read(root.join(p)).unwrap(), git_file(&root, p));
    }
    for p in [source::COLLECTOR_PATH, source::QUALIFICATION_PATH] {
        let m: toml::Value =
            toml::from_str(&std::fs::read_to_string(root.join(p)).unwrap()).unwrap();
        for entry in m["entries"].as_array().unwrap() {
            let p = entry["path"].as_str().unwrap();
            assert_eq!(
                std::fs::read(root.join(p)).unwrap(),
                git_file(&root, p),
                "{p}"
            );
        }
    }
}
#[test]
fn workspace_and_dependency_boundaries() {
    let root = root();
    let helper = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (manifest, expected) in [
        (root.join("Cargo.toml"), root.clone()),
        (helper.join("Cargo.toml"), helper.to_owned()),
    ] {
        let o = Command::new(env!("CARGO"))
            .args([
                "metadata",
                "--offline",
                "--locked",
                "--no-deps",
                "--format-version",
                "1",
                "--manifest-path",
            ])
            .arg(manifest)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let m: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(Path::new(m["workspace_root"].as_str().unwrap()), expected);
        let names: Vec<_> = m["packages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        if expected == root {
            assert!(!names.contains(&"qualification-prep"));
        } else {
            assert_eq!(names, ["qualification-prep"]);
            for d in m["packages"][0]["dependencies"].as_array().unwrap() {
                assert!(d.get("path").is_none());
            }
        }
    }
    for entry in std::fs::read_dir(helper.join("src")).unwrap() {
        let s = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        assert!(
            !s.contains("#[path")
                && !s.contains("include!(")
                && !s.contains("include_bytes!(")
                && !s.contains("include_str!(")
        );
    }
    assert!(helper.join("Cargo.lock").is_file());
}
#[test]
fn wrong_checkout_and_source_bytes_fail() {
    let root = root();
    let tmp = tempfile::tempdir().unwrap();
    assert!(source::verify_checkout(tmp.path()).is_err());
    let m: toml::Value =
        toml::from_str(&std::fs::read_to_string(root.join(source::COLLECTOR_PATH)).unwrap())
            .unwrap();
    let q: toml::Value =
        toml::from_str(&std::fs::read_to_string(root.join(source::QUALIFICATION_PATH)).unwrap())
            .unwrap();
    let mut paths = vec![
        source::COLLECTOR_PATH.to_owned(),
        source::QUALIFICATION_PATH.to_owned(),
    ];
    for manifest in [m, q] {
        for e in manifest["entries"].as_array().unwrap() {
            paths.push(e["path"].as_str().unwrap().into());
        }
    }
    paths.sort();
    paths.dedup();
    for p in paths {
        let dest = tmp.path().join(&p);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::copy(root.join(p), dest).unwrap();
    }
    source::verify_sources(tmp.path()).unwrap();
    for p in [
        source::COLLECTOR_PATH,
        source::QUALIFICATION_PATH,
        source::INSPECTOR_PATH,
        source::PACKAGING_PATH,
        "crates/external_browser_capture/src/transaction.rs",
    ] {
        let bytes = std::fs::read(tmp.path().join(p)).unwrap();
        std::fs::write(tmp.path().join(p), b"wrong").unwrap();
        assert!(source::verify_sources(tmp.path()).is_err(), "{p}");
        std::fs::write(tmp.path().join(p), bytes).unwrap();
    }
    let raw = std::fs::read(root.join(source::INSPECTOR_PATH)).unwrap();
    let expression = source::expression(&raw).unwrap();
    assert_eq!(canonical::hash(&expression), source::EXPRESSION_HASH);
    let mut modified = raw.clone();
    modified.push(b' ');
    assert_ne!(
        canonical::hash(&source::expression(&modified).unwrap()),
        source::EXPRESSION_HASH
    );
    assert!(source::expression(b"export const algorithm = 1;\n").is_err());
}

#[test]
fn historical_base_is_not_the_frozen_revision() {
    source::verify_revision(format!("{}\n", source::REVISION).as_bytes()).unwrap();
    assert!(source::verify_revision(b"619828f08fc97f3aa0f5d1913802dd8e08a00759\n").is_err());
}
