use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const CONTRACT_DOC: &str = "docs/html5/html5-core-v0.md";

pub(crate) mod anchors {
    pub(crate) const INPUT_AND_STREAMING_MODEL: &str = "input-and-streaming-model";
    pub(crate) const TOKENIZER_STATE_FAMILIES: &str = "tokenizer-state-families";
    pub(crate) const TREE_BUILDER_MODES_AND_ALGORITHMS: &str = "tree-builder-modes-and-algorithms";
    pub(crate) const SUPPORTED_TAGS_AND_CONTEXTS_BASELINE: &str =
        "supported-tags-and-contexts-baseline";
    pub(crate) const ATTRIBUTE_RULES_BASELINE: &str = "attribute-rules-baseline";
    pub(crate) const DOCTYPE_AND_QUIRKS_STANCE: &str = "doctype-and-quirks-stance";
    pub(crate) const TABLES_STANCE: &str = "tables-stance";
    pub(crate) const UNSPECIFIED_BEHAVIOR_HANDLING: &str =
        "unspecified-behavior-handling-fail-safe-contract";
    pub(crate) const CORE_V0_GATE_AND_EVIDENCE_MODEL: &str = "core-v0-gate-and-evidence-model";
}

const KNOWN_CONTRACT_ANCHORS: &[&str] = &[
    anchors::INPUT_AND_STREAMING_MODEL,
    anchors::TOKENIZER_STATE_FAMILIES,
    anchors::TREE_BUILDER_MODES_AND_ALGORITHMS,
    anchors::SUPPORTED_TAGS_AND_CONTEXTS_BASELINE,
    anchors::ATTRIBUTE_RULES_BASELINE,
    anchors::DOCTYPE_AND_QUIRKS_STANCE,
    anchors::TABLES_STANCE,
    anchors::UNSPECIFIED_BEHAVIOR_HANDLING,
    anchors::CORE_V0_GATE_AND_EVIDENCE_MODEL,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExpectedOutput {
    TokensV1,
    DomV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FixtureState {
    Existing,
    MissingByDesign,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AcceptanceKind {
    Tokenizer,
    TreeBuilder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AcceptanceCase {
    pub(crate) acceptance_id: &'static str,
    pub(crate) evidence_ref: &'static str,
    pub(crate) kind: AcceptanceKind,
    pub(crate) fixture_state: FixtureState,
    pub(crate) contract_anchor: &'static str,
    pub(crate) output: ExpectedOutput,
    pub(crate) note: Option<&'static str>,
}

pub(crate) fn expected_format(output: ExpectedOutput) -> &'static str {
    match output {
        ExpectedOutput::TokensV1 => "html5-token-v1",
        ExpectedOutput::DomV1 => "html5-dom-v1",
    }
}

pub(crate) fn fixture_state_label(state: FixtureState) -> &'static str {
    match state {
        FixtureState::Existing => "existing",
        FixtureState::MissingByDesign => "missing-by-design",
    }
}

pub(crate) fn kind_label(kind: AcceptanceKind) -> &'static str {
    match kind {
        AcceptanceKind::Tokenizer => "tokenizer",
        AcceptanceKind::TreeBuilder => "tree_builder",
    }
}

pub(crate) fn kind_fixture_subdir(kind: AcceptanceKind) -> &'static str {
    match kind {
        AcceptanceKind::Tokenizer => "tokenizer",
        AcceptanceKind::TreeBuilder => "tree_builder",
    }
}

pub(crate) fn fixture_dir(kind: AcceptanceKind, fixture_id: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("html5")
        .join(kind_fixture_subdir(kind))
        .join(fixture_id)
}

pub(crate) fn evidence_location<'a>(kind: AcceptanceKind, evidence_ref: &'a str) -> Cow<'a, str> {
    Cow::Owned(fixture_dir(kind, evidence_ref).display().to_string())
}

pub(crate) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate manifest dir should resolve to repo root")
        .to_path_buf()
}

pub(crate) fn parse_tokens_headers(path: &Path) -> (BTreeMap<String, String>, Vec<String>) {
    let content =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path:?}: {err}"));
    let mut headers = BTreeMap::new();
    let mut lines = Vec::new();
    for raw_line in content.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            continue;
        }
        if let Some(stripped) = line.strip_prefix('#') {
            let header = stripped.trim();
            if header.is_empty() {
                continue;
            }
            let (key, value) = header
                .split_once(':')
                .unwrap_or_else(|| panic!("invalid header in {path:?}: '{line}'"));
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            assert!(
                headers.insert(key.clone(), value).is_none(),
                "duplicate header '{key}' in {path:?}"
            );
        } else {
            lines.push(line.to_string());
        }
    }
    (headers, lines)
}

pub(crate) fn collect_contract_ids(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut ids = BTreeSet::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let candidate = if bytes[i..].starts_with(b"TOK-") {
            Some("TOK-")
        } else if bytes[i..].starts_with(b"TB-") {
            Some("TB-")
        } else {
            None
        };
        let Some(_prefix) = candidate else {
            i += 1;
            continue;
        };

        let start = i;
        let mut end = i;
        while end < bytes.len() {
            let b = bytes[end];
            if b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-' {
                end += 1;
            } else {
                break;
            }
        }
        if end > start + 4 {
            ids.insert(text[start..end].to_string());
        }
        i = end;
    }
    ids
}

pub(crate) fn collect_matrix_ids(path: &Path) -> BTreeSet<String> {
    let content =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path:?}: {err}"));
    let mut ids = BTreeSet::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let mut cells = trimmed.split('|');
        let _leading = cells.next();
        let Some(first_cell) = cells.next() else {
            continue;
        };
        let cell = first_cell.trim();
        let Some(cell) = cell
            .strip_prefix('`')
            .and_then(|cell| cell.strip_suffix('`'))
        else {
            continue;
        };
        if cell.starts_with("TOK-") || cell.starts_with("TB-") {
            ids.insert(cell.to_string());
        }
    }
    ids
}

pub(crate) fn validate_kind_output(kind: AcceptanceKind, output: ExpectedOutput) {
    match (kind, output) {
        (AcceptanceKind::Tokenizer, ExpectedOutput::TokensV1)
        | (AcceptanceKind::TreeBuilder, ExpectedOutput::DomV1) => {}
        (AcceptanceKind::Tokenizer, ExpectedOutput::DomV1) => {
            panic!("PENDING_ACCEPTANCE_CONFIG: tokenizer acceptance must use TokensV1 output")
        }
        (AcceptanceKind::TreeBuilder, ExpectedOutput::TokensV1) => {
            panic!("PENDING_ACCEPTANCE_CONFIG: tree-builder acceptance must use DomV1 output")
        }
    }
}

pub(crate) fn assert_acceptance_case_is_well_formed(case: &AcceptanceCase) {
    validate_kind_output(case.kind, case.output);
    let kind = kind_label(case.kind);
    let expected_output = expected_format(case.output);
    let fixture_state = fixture_state_label(case.fixture_state);

    assert!(
        !case.acceptance_id.trim().is_empty(),
        "acceptance id must not be empty: {case:?}"
    );
    assert!(
        !case.evidence_ref.trim().is_empty(),
        "evidence ref must not be empty for {}",
        case.acceptance_id
    );
    assert!(
        KNOWN_CONTRACT_ANCHORS.contains(&case.contract_anchor),
        "unknown contract anchor '{}' for {} in {CONTRACT_DOC}",
        case.contract_anchor,
        case.acceptance_id
    );
    match case.kind {
        AcceptanceKind::Tokenizer => assert!(
            case.acceptance_id.starts_with("tok-"),
            "{kind} acceptance id must start with tok-: {}",
            case.acceptance_id
        ),
        AcceptanceKind::TreeBuilder => assert!(
            case.acceptance_id.starts_with("tb-"),
            "{kind} acceptance id must start with tb-: {}",
            case.acceptance_id
        ),
    }

    if let Some(note) = case.note {
        assert!(
            !note.trim().is_empty(),
            "note must not be empty for {}",
            case.acceptance_id
        );
    }

    let fixture_dir = fixture_dir(case.kind, case.evidence_ref);
    match case.fixture_state {
        FixtureState::Existing => assert!(
            fixture_dir.is_dir(),
            "acceptance {} ({kind}, {expected_output}, fixture_state={fixture_state}) expects fixture dir to exist: {}",
            case.acceptance_id,
            evidence_location(case.kind, case.evidence_ref)
        ),
        FixtureState::MissingByDesign => assert!(
            !fixture_dir.exists(),
            "acceptance {} ({kind}, {expected_output}, fixture_state={fixture_state}) is marked missing-by-design, but fixture dir exists: {}",
            case.acceptance_id,
            evidence_location(case.kind, case.evidence_ref)
        ),
    }
}
