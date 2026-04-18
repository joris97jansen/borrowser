use super::metadata::parse_meta_file;
use html::html5::TextModeSpec;
use html_test_support::wpt_expected::parse_expected_tokens;
#[cfg(feature = "dom-snapshot")]
use html_test_support::wpt_expected::{ParsedExpectedDom, parse_expected_dom};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Fixture discovery and in-memory representation for rawtext/script HTML5
/// regression cases.

pub(crate) struct Fixture {
    pub(crate) name: String,
    pub(crate) dir: PathBuf,
    pub(crate) input: String,
    pub(crate) meta: FixtureMeta,
    pub(crate) expected_tokens: Option<Vec<String>>,
    #[cfg(feature = "dom-snapshot")]
    pub(crate) expected_dom: Option<ParsedExpectedDom>,
    #[cfg(not(feature = "dom-snapshot"))]
    pub(crate) expected_dom: Option<()>,
}

#[derive(Clone, Debug)]
pub(crate) struct FixtureMeta {
    pub(crate) tool: String,
    pub(crate) seed: String,
    pub(crate) date: String,
    pub(crate) issue: String,
    pub(crate) guard: String,
    pub(crate) source: Option<String>,
    pub(crate) mode: TextModeFixtureMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TextModeFixtureMode {
    ScriptData,
    RawtextStyle,
}

impl TextModeFixtureMode {
    pub(crate) fn from_header(value: &str, path: &Path) -> Self {
        match value {
            "script-data" => Self::ScriptData,
            "rawtext-style" => Self::RawtextStyle,
            other => panic!(
                "unsupported text-mode regression mode '{other}' in {}",
                path.display()
            ),
        }
    }

    pub(crate) fn tag_name(self) -> &'static str {
        match self {
            Self::ScriptData => "script",
            Self::RawtextStyle => "style",
        }
    }

    pub(crate) fn spec(self, tag: html::html5::AtomId) -> TextModeSpec {
        match self {
            Self::ScriptData => TextModeSpec::script_data(tag),
            Self::RawtextStyle => TextModeSpec::rawtext_style(tag),
        }
    }
}

pub(crate) struct FixtureFilter {
    raw: Option<String>,
}

impl FixtureFilter {
    pub(crate) fn matches(&self, name: &str) -> bool {
        let Some(filter) = &self.raw else {
            return true;
        };
        name.contains(filter)
    }
}

pub(crate) fn fixture_filter() -> FixtureFilter {
    FixtureFilter {
        raw: env::var("BORROWSER_HTML5_RAWTEXT_SCRIPT_REGRESSION").ok(),
    }
}

pub(crate) fn load_fixtures() -> Vec<Fixture> {
    let root = fixture_root();
    let mut entries = fs::read_dir(&root)
        .unwrap_or_else(|err| panic!("failed to read fixture root {}: {err}", root.display()))
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    let mut fixtures = Vec::new();
    for entry in entries {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let meta_path = dir.join("meta.txt");
        let input_path = dir.join("input.html");
        let tokens_path = dir.join("tokens.txt");
        let dom_path = dir.join("dom.txt");
        let input =
            trim_one_trailing_line_ending(&fs::read_to_string(&input_path).unwrap_or_else(|err| {
                panic!("failed to read input {}: {err}", input_path.display())
            }));
        let meta = parse_meta_file(&meta_path);
        let expected_tokens = tokens_path
            .exists()
            .then(|| parse_expected_tokens(&tokens_path));
        #[cfg(feature = "dom-snapshot")]
        let expected_dom = dom_path.exists().then(|| parse_expected_dom(&dom_path));
        #[cfg(not(feature = "dom-snapshot"))]
        let expected_dom = dom_path.exists().then_some(());
        fixtures.push(Fixture {
            name,
            dir,
            input,
            meta,
            expected_tokens,
            expected_dom,
        });
    }
    fixtures
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("regressions")
        .join("html5")
        .join("rawtext_script")
}

fn trim_one_trailing_line_ending(input: &str) -> String {
    if let Some(stripped) = input.strip_suffix("\r\n") {
        stripped.to_string()
    } else if let Some(stripped) = input.strip_suffix('\n') {
        stripped.to_string()
    } else {
        input.to_string()
    }
}
