use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixtureStatus {
    Active,
    Xfail,
    Skip,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaseKind {
    Dom,
    Tokens,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiffKind {
    Tokens,
    Dom,
    Both,
    Skip,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct WptCase {
    pub id: String,
    pub path: PathBuf,
    pub expected: PathBuf,
    pub status: FixtureStatus,
    pub reason: Option<String>,
    pub kind: CaseKind,
    #[allow(dead_code)]
    pub diff: Option<DiffKind>,
}

pub fn load_manifest(path: &Path) -> Vec<WptCase> {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read WPT manifest {path:?}: {err}"));
    let mut format = None::<String>;
    let mut current: BTreeMap<String, String> = BTreeMap::new();
    let mut cases = Vec::new();

    let mut flush = |current: &mut BTreeMap<String, String>| {
        if current.is_empty() {
            return;
        }
        let id = current
            .remove("id")
            .unwrap_or_else(|| panic!("missing id in WPT manifest {path:?}"));
        let rel_path = current
            .remove("path")
            .unwrap_or_else(|| panic!("missing path for '{id}' in WPT manifest {path:?}"));
        let expected = current
            .remove("expected")
            .unwrap_or_else(|| panic!("missing expected for '{id}' in WPT manifest {path:?}"));
        let kind = match current.remove("kind").as_deref() {
            Some("tokens") => CaseKind::Tokens,
            Some("dom") | None => CaseKind::Dom,
            Some(other) => panic!("unsupported kind '{other}' for '{id}' in {path:?}"),
        };
        let status = match current.remove("status").as_deref() {
            Some("xfail") => FixtureStatus::Xfail,
            Some("skip") => FixtureStatus::Skip,
            Some("active") | None => FixtureStatus::Active,
            Some(other) => panic!("unsupported status '{other}' for '{id}' in {path:?}"),
        };
        let reason = current.remove("reason");
        match status {
            FixtureStatus::Active => {
                if reason.is_some() {
                    panic!("case '{id}' has reason but is not xfail/skip in {path:?}");
                }
            }
            FixtureStatus::Xfail | FixtureStatus::Skip => {
                if reason.as_deref().unwrap_or("").is_empty() {
                    panic!("case '{id}' with status '{status:?}' missing reason in {path:?}");
                }
            }
        }
        let diff = match current.remove("diff").as_deref() {
            Some("tokens") => Some(DiffKind::Tokens),
            Some("dom") => Some(DiffKind::Dom),
            Some("both") => Some(DiffKind::Both),
            Some("skip") => Some(DiffKind::Skip),
            Some(other) => panic!("unsupported diff '{other}' for '{id}' in {path:?}"),
            None => None,
        };
        if !current.is_empty() {
            let keys = current.keys().cloned().collect::<Vec<_>>();
            panic!("unknown keys for '{id}' in {path:?}: {keys:?}");
        }
        let root = path
            .parent()
            .unwrap_or_else(|| panic!("manifest has no parent directory"));
        let input_path = root.join(rel_path);
        let expected_path = root.join(expected);
        if !input_path.is_file() {
            panic!("WPT input file missing for '{id}': {input_path:?}");
        }
        if !expected_path.is_file() {
            panic!("WPT expected file missing for '{id}': {expected_path:?}");
        }
        let expected_name = expected_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| {
                panic!("expected snapshot filename is not valid UTF-8: {expected_path:?}")
            });
        match kind {
            CaseKind::Tokens => {
                if !expected_name.ends_with(".tokens.txt") {
                    panic!(
                        "token case '{id}' must use .tokens.txt expected file: {expected_path:?}"
                    );
                }
            }
            CaseKind::Dom => {
                if !expected_name.ends_with(".dom.txt") {
                    panic!("dom case '{id}' must use .dom.txt expected file: {expected_path:?}");
                }
            }
        }
        cases.push(WptCase {
            id,
            path: input_path,
            expected: expected_path,
            status,
            reason,
            kind,
            diff,
        });
        current.clear();
    };

    for raw_line in content.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            flush(&mut current);
            continue;
        }
        if let Some(stripped) = line.strip_prefix('#') {
            let header = stripped.trim();
            if let Some((key, value)) = header.split_once(':') {
                let key = key.trim().to_ascii_lowercase();
                let value = value.trim().to_string();
                if key == "format" {
                    format = Some(value);
                }
            }
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .unwrap_or_else(|| panic!("invalid manifest line in {path:?}: '{line}'"));
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().to_string();
        if current.insert(key.clone(), value).is_some() {
            panic!("duplicate key '{key}' in {path:?}");
        }
    }
    flush(&mut current);

    match format.as_deref() {
        Some("wpt-manifest-v1") => {}
        Some(other) => panic!("unsupported manifest format '{other}' in {path:?}"),
        None => panic!("missing manifest format header in {path:?}"),
    }

    cases
}
