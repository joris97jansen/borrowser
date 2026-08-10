//! A deliberately narrow adapter for pinned WPT tree-construction records.
//!
//! This module only parses the external record format and emits ordinary
//! canonical fixture-v3 files. It never invokes the parser and never compares
//! parser output; those responsibilities remain at the canonical fixture
//! validation and execution boundary.

use ring::digest::{SHA256, digest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

pub const EXTERNAL_PROVENANCE_FORMAT: &str = "borrowser-external-provenance-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalCapability {
    FragmentDomApi,
    Scripting,
    DocumentWrite,
    DomBindings,
    Events,
    Navigation,
    ResourcesNetworking,
    Rendering,
    UnsupportedExpectationRepresentation,
    UnsupportedParserFeature,
    MalformedExternalRecord,
}

impl ExternalCapability {
    pub fn name(&self) -> &'static str {
        match self {
            Self::FragmentDomApi => "fragment-dom-api",
            Self::Scripting => "scripting",
            Self::DocumentWrite => "document-write",
            Self::DomBindings => "dom-bindings",
            Self::Events => "events",
            Self::Navigation => "navigation",
            Self::ResourcesNetworking => "resources-networking",
            Self::Rendering => "rendering",
            Self::UnsupportedExpectationRepresentation => "unsupported-expectation-representation",
            Self::UnsupportedParserFeature => "unsupported-parser-feature",
            Self::MalformedExternalRecord => "malformed-external-record",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalCaseClassification {
    Eligible,
    Unsupported(ExternalCapability),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalFixtureArtifact {
    bundle_name: String,
    case_identity: String,
    classification: ExternalCaseClassification,
    files: BTreeMap<String, Vec<u8>>,
}

impl ExternalFixtureArtifact {
    pub fn bundle_name(&self) -> &str {
        &self.bundle_name
    }

    pub fn case_identity(&self) -> &str {
        &self.case_identity
    }

    pub fn classification(&self) -> &ExternalCaseClassification {
        &self.classification
    }

    pub fn files(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.files
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalAdapterOutput {
    artifacts: Vec<ExternalFixtureArtifact>,
}

impl ExternalAdapterOutput {
    pub fn artifacts(&self) -> &[ExternalFixtureArtifact] {
        &self.artifacts
    }
}

#[derive(Debug)]
pub enum ExternalAdapterError {
    Io {
        path: PathBuf,
        message: String,
    },
    InvalidUtf8 {
        path: PathBuf,
    },
    InvalidToml {
        path: PathBuf,
        message: String,
    },
    InvalidAllowlist(String),
    InvalidRecord {
        path: String,
        ordinal: usize,
        message: String,
    },
    UnsupportedRecord {
        path: String,
        ordinal: usize,
        capability: ExternalCapability,
        message: String,
    },
    IntegrityMismatch {
        path: String,
        expected: String,
        actual: String,
    },
}

impl std::fmt::Display for ExternalAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
            Self::InvalidUtf8 { path } => write!(f, "{}: expected UTF-8", path.display()),
            Self::InvalidToml { path, message } => write!(f, "{}: {message}", path.display()),
            Self::InvalidAllowlist(message) => f.write_str(message),
            Self::InvalidRecord {
                path,
                ordinal,
                message,
            } => write!(f, "{path} record {ordinal}: {message}"),
            Self::UnsupportedRecord {
                path,
                ordinal,
                capability,
                message,
            } => write!(
                f,
                "{path} record {ordinal}: unsupported {}: {message}",
                capability.name()
            ),
            Self::IntegrityMismatch {
                path,
                expected,
                actual,
            } => write!(f, "{path}: expected SHA-256 {expected}, got {actual}"),
        }
    }
}

impl std::error::Error for ExternalAdapterError {}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowlistFile {
    format: String,
    upstream_project: String,
    upstream_revision: String,
    source_path: String,
    source_file_sha256: String,
    license_identifier: String,
    license_notice: String,
    attribution: String,
    cases: Vec<AllowlistCase>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowlistCase {
    source_path: Option<String>,
    source_file_sha256: Option<String>,
    ordinal: usize,
    record_sha256: String,
    display_name: String,
}

#[derive(Clone, Debug)]
struct DatRecord {
    ordinal: usize,
    raw: String,
    input: Vec<u8>,
    parse_error_count: u64,
    scripting: ScriptingMarker,
    fragment_context: Option<String>,
    tree: TreeNode,
}

#[derive(Clone, Debug)]
struct TreeAttribute {
    namespace: &'static str,
    prefix: Option<String>,
    local_name: String,
    value: String,
}

#[derive(Clone, Debug)]
struct TreeStackFrame {
    depth: usize,
    path: Vec<usize>,
    owns_attributes: bool,
}

#[derive(Serialize)]
struct GeneratedExternalProvenance<'a> {
    format: &'a str,
    upstream_project: &'a str,
    upstream_revision: &'a str,
    upstream_path: &'a str,
    source_record_ordinal: usize,
    source_record_sha256: &'a str,
    source_file_sha256: &'a str,
    license_identifier: &'a str,
    license_notice: &'a str,
    attribution: &'a str,
    adaptation: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScriptingMarker {
    ExplicitOff,
    ExplicitOn,
    Absent,
}

#[derive(Clone, Debug)]
enum TreeNode {
    Document {
        children: Vec<TreeNode>,
    },
    Element {
        namespace: &'static str,
        local_name: String,
        attributes: Vec<TreeAttribute>,
        children: Vec<TreeNode>,
    },
    Text(String),
    Comment(String),
    Doctype {
        name: String,
        public_id: Option<String>,
        system_id: Option<String>,
    },
    ProcessingInstruction {
        target: String,
        data: String,
    },
}

pub fn adapt_allowlisted_subset(
    raw_root: &Path,
    allowlist_path: &Path,
) -> Result<ExternalAdapterOutput, ExternalAdapterError> {
    let allowlist_bytes = read_file(allowlist_path)?;
    let allowlist_text =
        std::str::from_utf8(&allowlist_bytes).map_err(|_| ExternalAdapterError::InvalidUtf8 {
            path: allowlist_path.to_path_buf(),
        })?;
    let allowlist: AllowlistFile =
        toml::from_str(allowlist_text).map_err(|error| ExternalAdapterError::InvalidToml {
            path: allowlist_path.to_path_buf(),
            message: error.to_string(),
        })?;
    if allowlist.format != "borrowser-wpt-external-allowlist-v1" {
        return Err(ExternalAdapterError::InvalidAllowlist(
            "unsupported external allowlist format".to_string(),
        ));
    }
    if allowlist.cases.is_empty() {
        return Err(ExternalAdapterError::InvalidAllowlist(
            "external allowlist must select at least one record".to_string(),
        ));
    }
    validate_allowlist_metadata(&allowlist)?;

    let mut artifacts = Vec::with_capacity(allowlist.cases.len());
    for case in &allowlist.cases {
        let source_path = case
            .source_path
            .as_deref()
            .unwrap_or(&allowlist.source_path);
        let source_file_sha256 = case
            .source_file_sha256
            .as_deref()
            .unwrap_or(&allowlist.source_file_sha256);
        let source_path_value = Path::new(source_path);
        if source_path_value.is_absolute()
            || source_path_value
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(ExternalAdapterError::InvalidAllowlist(format!(
                "external source path is not repository-relative: {source_path}"
            )));
        }
        validate_source_path(source_path)?;
        validate_sha256_text(source_file_sha256, "source file")?;
        if case.ordinal == 0 {
            return Err(ExternalAdapterError::InvalidAllowlist(
                "external record ordinals are one-based".to_string(),
            ));
        }
        validate_sha256_text(&case.record_sha256, "record")?;
        if case.display_name.trim().is_empty() {
            return Err(ExternalAdapterError::InvalidAllowlist(
                "external display names must not be empty".to_string(),
            ));
        }
        let source_file = raw_root.join(source_path_value);
        let source_bytes = read_file(&source_file)?;
        verify_hash(source_path, source_file_sha256, &source_bytes)?;
        let source_text =
            std::str::from_utf8(&source_bytes).map_err(|_| ExternalAdapterError::InvalidUtf8 {
                path: source_file.clone(),
            })?;
        let raw_records = split_dat_records(source_text);
        let raw_record = raw_records
            .get(case.ordinal.saturating_sub(1))
            .ok_or_else(|| ExternalAdapterError::InvalidRecord {
                path: source_path.to_string(),
                ordinal: case.ordinal,
                message: "allowlisted ordinal is outside the source file".to_string(),
            })?;
        let actual_record_hash = sha256_hex(raw_record.as_bytes());
        if actual_record_hash != case.record_sha256 {
            return Err(ExternalAdapterError::IntegrityMismatch {
                path: format!("{source_path}#{}", case.ordinal),
                expected: case.record_sha256.clone(),
                actual: actual_record_hash,
            });
        }
        let record = match parse_dat_record(source_path, case.ordinal, raw_record.clone()) {
            Ok(record) => record,
            Err(error @ ExternalAdapterError::UnsupportedRecord { .. }) => {
                artifacts.push(ExternalFixtureArtifact {
                    bundle_name: format!("wpt-{}", sanitize_identifier(&case.display_name)),
                    case_identity: format!(
                        "{}:{source_path}:{}:{}",
                        allowlist.upstream_revision, case.ordinal, case.record_sha256
                    ),
                    classification: unsupported_classification(error),
                    files: BTreeMap::new(),
                });
                continue;
            }
            Err(ExternalAdapterError::InvalidRecord { .. }) => {
                artifacts.push(ExternalFixtureArtifact {
                    bundle_name: format!("wpt-{}", sanitize_identifier(&case.display_name)),
                    case_identity: format!(
                        "{}:{source_path}:{}:{}",
                        allowlist.upstream_revision, case.ordinal, case.record_sha256
                    ),
                    classification: ExternalCaseClassification::Unsupported(
                        ExternalCapability::MalformedExternalRecord,
                    ),
                    files: BTreeMap::new(),
                });
                continue;
            }
            Err(error) => return Err(error),
        };
        let case_identity = format!(
            "{}:{source_path}:{}:{actual_record_hash}",
            allowlist.upstream_revision, case.ordinal
        );
        let classification = classify_record(&record);
        let bundle_name = format!("wpt-{}", sanitize_identifier(&case.display_name));
        let files = if classification == ExternalCaseClassification::Eligible {
            generate_fixture_files(
                &record,
                &bundle_name,
                source_path,
                source_file_sha256,
                &case_identity,
                &allowlist,
            )?
        } else {
            BTreeMap::new()
        };
        artifacts.push(ExternalFixtureArtifact {
            bundle_name,
            case_identity,
            classification,
            files,
        });
    }
    artifacts.sort_by(|left, right| left.bundle_name.cmp(&right.bundle_name));
    Ok(ExternalAdapterOutput { artifacts })
}

fn unsupported_classification(error: ExternalAdapterError) -> ExternalCaseClassification {
    let ExternalAdapterError::UnsupportedRecord { capability, .. } = error else {
        unreachable!("only unsupported records reach this conversion")
    };
    ExternalCaseClassification::Unsupported(capability)
}

fn validate_allowlist_metadata(allowlist: &AllowlistFile) -> Result<(), ExternalAdapterError> {
    for (label, value) in [
        ("upstream project", allowlist.upstream_project.as_str()),
        ("upstream revision", allowlist.upstream_revision.as_str()),
        ("source path", allowlist.source_path.as_str()),
        ("licence identifier", allowlist.license_identifier.as_str()),
        ("licence notice", allowlist.license_notice.as_str()),
        ("attribution", allowlist.attribution.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(ExternalAdapterError::InvalidAllowlist(format!(
                "{label} must not be empty"
            )));
        }
    }
    validate_revision(&allowlist.upstream_revision)?;
    validate_source_path(&allowlist.source_path)?;
    validate_sha256_text(&allowlist.source_file_sha256, "source file")?;
    Ok(())
}

fn validate_revision(revision: &str) -> Result<(), ExternalAdapterError> {
    if revision.len() != 40
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ExternalAdapterError::InvalidAllowlist(
            "upstream revision must be a 40-character lowercase hexadecimal commit".to_string(),
        ));
    }
    Ok(())
}

fn validate_source_path(path: &str) -> Result<(), ExternalAdapterError> {
    let value = Path::new(path);
    if path.is_empty()
        || path.contains('\\')
        || path.contains(':')
        || value.is_absolute()
        || value.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(ExternalAdapterError::InvalidAllowlist(format!(
            "external source path is not a portable repository-relative path: {path}"
        )));
    }
    Ok(())
}

fn validate_sha256_text(value: &str, label: &str) -> Result<(), ExternalAdapterError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ExternalAdapterError::InvalidAllowlist(format!(
            "{label} SHA-256 must be 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn read_file(path: &Path) -> Result<Vec<u8>, ExternalAdapterError> {
    fs::read(path).map_err(|error| ExternalAdapterError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn verify_hash(path: &str, expected: &str, bytes: &[u8]) -> Result<(), ExternalAdapterError> {
    let actual = sha256_hex(bytes);
    if expected != actual {
        return Err(ExternalAdapterError::IntegrityMismatch {
            path: path.to_string(),
            expected: expected.to_string(),
            actual,
        });
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = digest(&SHA256, bytes);
    let mut output = String::with_capacity(64);
    for byte in digest.as_ref() {
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
fn parse_dat_records(path: &str, text: &str) -> Result<Vec<DatRecord>, ExternalAdapterError> {
    split_dat_records(text)
        .into_iter()
        .enumerate()
        .map(|(index, raw)| parse_dat_record(path, index + 1, raw))
        .collect()
}

fn split_dat_records(text: &str) -> Vec<String> {
    text.split("\n\n#data\n")
        .enumerate()
        .map(|(index, part)| {
            if index == 0 {
                part.to_string()
            } else {
                format!("#data\n{part}")
            }
        })
        .collect()
}

fn parse_dat_record(
    path: &str,
    ordinal: usize,
    raw: String,
) -> Result<DatRecord, ExternalAdapterError> {
    let mut cursor = 0;
    consume_marker(path, ordinal, &raw, &mut cursor, "#data")?;
    let errors_start = find_next_marker(path, ordinal, &raw, cursor, &["#errors"])?
        .ok_or_else(|| invalid_record(path, ordinal, "record has no #errors section"))?;
    let mut input = raw.as_bytes()[cursor..errors_start.0].to_vec();
    if input.last() == Some(&b'\n') {
        input.pop();
    }
    cursor = errors_start.1;
    let next_section = next_required_marker(path, ordinal, &raw, cursor)?;
    let mut parse_error_count = count_error_section(path, ordinal, &raw[cursor..next_section])?;
    cursor = next_section;

    if line_is_at(&raw, cursor, "#new-errors") {
        consume_marker(path, ordinal, &raw, &mut cursor, "#new-errors")?;
        let next = next_required_marker(path, ordinal, &raw, cursor)?;
        parse_error_count = parse_error_count.saturating_add(count_error_section(
            path,
            ordinal,
            &raw[cursor..next],
        )?);
        cursor = next;
    }

    let fragment_context = if line_is_at(&raw, cursor, "#document-fragment") {
        consume_marker(path, ordinal, &raw, &mut cursor, "#document-fragment")?;
        let (context, next) = consume_content_line(path, ordinal, &raw, cursor)?;
        if context.trim().is_empty() {
            return Err(invalid_record(
                path,
                ordinal,
                "document-fragment context must not be empty",
            ));
        }
        cursor = next;
        Some(context.to_string())
    } else {
        None
    };

    let scripting = if line_is_at(&raw, cursor, "#script-off") {
        consume_marker(path, ordinal, &raw, &mut cursor, "#script-off")?;
        ScriptingMarker::ExplicitOff
    } else if line_is_at(&raw, cursor, "#script-on") {
        consume_marker(path, ordinal, &raw, &mut cursor, "#script-on")?;
        ScriptingMarker::ExplicitOn
    } else {
        ScriptingMarker::Absent
    };
    consume_marker(path, ordinal, &raw, &mut cursor, "#document")?;
    let tree_text = &raw[cursor..];
    let tree = parse_tree(path, ordinal, tree_text)?;
    Ok(DatRecord {
        ordinal,
        raw,
        input,
        parse_error_count,
        scripting,
        fragment_context,
        tree,
    })
}

fn count_error_section(
    path: &str,
    ordinal: usize,
    text: &str,
) -> Result<u64, ExternalAdapterError> {
    if text.is_empty() {
        return Ok(0);
    }
    let Some(entries) = text.strip_suffix('\n') else {
        return Err(invalid_record(
            path,
            ordinal,
            "error section is not terminated before the next marker",
        ));
    };
    if entries.is_empty() {
        return Err(invalid_record(
            path,
            ordinal,
            "error section contains an empty error entry",
        ));
    }
    let mut count = 0;
    for entry in entries.split('\n') {
        if entry.is_empty() {
            return Err(invalid_record(
                path,
                ordinal,
                "error section contains an empty error entry",
            ));
        }
        count += 1;
    }
    Ok(count)
}

fn invalid_record(path: &str, ordinal: usize, message: &str) -> ExternalAdapterError {
    ExternalAdapterError::InvalidRecord {
        path: path.to_string(),
        ordinal,
        message: message.to_string(),
    }
}

fn classify_record(record: &DatRecord) -> ExternalCaseClassification {
    if record.fragment_context.is_some() {
        return ExternalCaseClassification::Unsupported(ExternalCapability::FragmentDomApi);
    }
    if record.scripting != ScriptingMarker::ExplicitOff {
        return ExternalCaseClassification::Unsupported(ExternalCapability::Scripting);
    }
    ExternalCaseClassification::Eligible
}

fn find_next_marker(
    path: &str,
    ordinal: usize,
    raw: &str,
    start: usize,
    markers: &[&str],
) -> Result<Option<(usize, usize)>, ExternalAdapterError> {
    let mut cursor = start;
    while cursor < raw.len() {
        let (line, next) = consume_content_line(path, ordinal, raw, cursor)?;
        if markers.contains(&line) {
            return Ok(Some((cursor, next)));
        }
        cursor = next;
    }
    Ok(None)
}

fn next_required_marker(
    path: &str,
    ordinal: usize,
    raw: &str,
    start: usize,
) -> Result<usize, ExternalAdapterError> {
    find_next_marker(
        path,
        ordinal,
        raw,
        start,
        &[
            "#new-errors",
            "#document-fragment",
            "#script-off",
            "#script-on",
            "#document",
        ],
    )?
    .map(|(start, _)| start)
    .ok_or_else(|| invalid_record(path, ordinal, "record has no following section marker"))
}

fn line_is_at(raw: &str, cursor: usize, marker: &str) -> bool {
    raw.get(cursor..)
        .and_then(|rest| rest.strip_prefix(marker))
        .is_some_and(|rest| rest.starts_with('\n') || rest.is_empty())
}

fn consume_marker(
    path: &str,
    ordinal: usize,
    raw: &str,
    cursor: &mut usize,
    marker: &str,
) -> Result<(), ExternalAdapterError> {
    let (line, next) = consume_content_line(path, ordinal, raw, *cursor)?;
    if line != marker {
        return Err(invalid_record(
            path,
            ordinal,
            &format!("expected {marker}, found {line:?}"),
        ));
    }
    *cursor = next;
    Ok(())
}

fn consume_content_line<'a>(
    path: &str,
    ordinal: usize,
    raw: &'a str,
    cursor: usize,
) -> Result<(&'a str, usize), ExternalAdapterError> {
    if cursor > raw.len() {
        return Err(invalid_record(
            path,
            ordinal,
            "section cursor is outside record",
        ));
    }
    let rest = &raw[cursor..];
    let Some(newline) = rest.find('\n') else {
        return Ok((rest, raw.len()));
    };
    Ok((&rest[..newline], cursor + newline + 1))
}

fn parse_tree(path: &str, ordinal: usize, text: &str) -> Result<TreeNode, ExternalAdapterError> {
    let mut root = TreeNode::Document {
        children: Vec::new(),
    };
    let mut stack: Vec<TreeStackFrame> = Vec::new();
    let mut previous_line_may_continue_quoted_value = false;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let Some(rest) = line.strip_prefix("| ") else {
            if previous_line_may_continue_quoted_value {
                return Err(ExternalAdapterError::UnsupportedRecord {
                    path: path.to_string(),
                    ordinal,
                    capability: ExternalCapability::UnsupportedExpectationRepresentation,
                    message: "multi-line WPT tree value is outside the AE13e adapter profile"
                        .to_string(),
                });
            }
            return invalid_tree(path, ordinal, "tree line lacks '| ' prefix");
        };
        let spaces = rest.bytes().take_while(|byte| *byte == b' ').count();
        if spaces % 2 != 0 {
            return invalid_tree(path, ordinal, "tree indentation is not two-space aligned");
        }
        let depth = spaces / 2;
        let node_text = &rest[spaces..];
        previous_line_may_continue_quoted_value = starts_unclosed_quoted_value(node_text);
        if previous_line_may_continue_quoted_value
            || ((node_text.starts_with("<!DOCTYPE ") || node_text.starts_with("DOCTYPE "))
                && has_unclosed_quote(node_text))
        {
            return Err(ExternalAdapterError::UnsupportedRecord {
                path: path.to_string(),
                ordinal,
                capability: ExternalCapability::UnsupportedExpectationRepresentation,
                message: "multi-line WPT tree value is outside the AE13e adapter profile"
                    .to_string(),
            });
        }
        if !node_text.starts_with('<')
            && !node_text.starts_with('"')
            && !node_text.starts_with("<!--")
            && !node_text.starts_with("DOCTYPE ")
            && !node_text.starts_with("<!DOCTYPE ")
            && !node_text.starts_with("<?")
        {
            let Some((name, value)) = node_text.split_once('=') else {
                return invalid_tree(path, ordinal, "WPT tree line is not a node or attribute");
            };
            let attribute = parse_tree_attribute(name, value)
                .map_err(|error| tree_error_to_adapter_error(path, ordinal, error))?;
            let Some(current_frame) = stack.last() else {
                return invalid_tree(path, ordinal, "WPT attribute has no element owner");
            };
            let Some(expected_attribute_depth) = current_frame.depth.checked_add(1) else {
                return invalid_tree(
                    path,
                    ordinal,
                    "WPT attribute owner depth cannot have an attribute child",
                );
            };
            if depth != expected_attribute_depth {
                return invalid_tree(
                    path,
                    ordinal,
                    "WPT attribute indentation does not belong to its element owner",
                );
            }
            if !current_frame.owns_attributes {
                return invalid_tree(path, ordinal, "WPT attribute owner is not an element");
            }
            let current = node_at_mut(&mut root, &current_frame.path).ok_or_else(|| {
                invalid_record(path, ordinal, "WPT attribute owner path is invalid")
            })?;
            let TreeNode::Element { attributes, .. } = current else {
                return invalid_tree(path, ordinal, "WPT attribute owner is not an element");
            };
            attributes.push(attribute);
            continue;
        }
        let node = parse_tree_node(node_text)
            .map_err(|error| tree_error_to_adapter_error(path, ordinal, error))?;
        while stack.last().is_some_and(|frame| frame.depth >= depth) {
            stack.pop();
        }
        if let Some(parent_frame) = stack.last() {
            if depth != parent_frame.depth.saturating_add(1) {
                return invalid_tree(
                    path,
                    ordinal,
                    "WPT tree indentation skips a structural parent level",
                );
            }
        } else if depth != 0 {
            return invalid_tree(path, ordinal, "WPT tree root is not at depth zero");
        }
        let parent_path = stack
            .last()
            .map(|frame| frame.path.clone())
            .unwrap_or_default();
        let parent = node_at_mut(&mut root, &parent_path)
            .ok_or_else(|| invalid_record(path, ordinal, "WPT parent path is invalid"))?;
        let owns_attributes = matches!(node, TreeNode::Element { .. });
        let child_index = parent
            .push_child(node)
            .map_err(|message| invalid_record(path, ordinal, message))?;
        let mut node_path = parent_path;
        node_path.push(child_index);
        stack.push(TreeStackFrame {
            depth,
            path: node_path,
            owns_attributes,
        });
    }
    Ok(root)
}

fn starts_unclosed_quoted_value(value: &str) -> bool {
    let Some(value) = value.strip_prefix('"') else {
        return false;
    };
    let mut escaped = false;
    for byte in value.bytes() {
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            return false;
        }
    }
    true
}

fn has_unclosed_quote(value: &str) -> bool {
    let mut escaped = false;
    let mut quoted = false;
    for byte in value.bytes() {
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            quoted = !quoted;
        }
    }
    quoted
}

fn invalid_tree(
    path: &str,
    ordinal: usize,
    message: &str,
) -> Result<TreeNode, ExternalAdapterError> {
    Err(ExternalAdapterError::InvalidRecord {
        path: path.to_string(),
        ordinal,
        message: message.to_string(),
    })
}

fn tree_error_to_adapter_error(
    path: &str,
    ordinal: usize,
    error: TreeParseError,
) -> ExternalAdapterError {
    match error {
        TreeParseError::Malformed(message) => invalid_record(path, ordinal, message),
        TreeParseError::UnsupportedExpectation(message) => {
            ExternalAdapterError::UnsupportedRecord {
                path: path.to_string(),
                ordinal,
                capability: ExternalCapability::UnsupportedExpectationRepresentation,
                message,
            }
        }
    }
}

impl TreeNode {
    fn push_child(&mut self, child: TreeNode) -> Result<usize, &'static str> {
        let children = match self {
            Self::Document { children } | Self::Element { children, .. } => children,
            _ => {
                return Err("tree node cannot own children");
            }
        };
        let index = children.len();
        children.push(child);
        Ok(index)
    }
}

fn node_at_mut<'a>(node: &'a mut TreeNode, path: &[usize]) -> Option<&'a mut TreeNode> {
    if let Some((first, rest)) = path.split_first() {
        let children = match node {
            TreeNode::Document { children } | TreeNode::Element { children, .. } => children,
            _ => return None,
        };
        children
            .get_mut(*first)
            .and_then(|child| node_at_mut(child, rest))
    } else {
        Some(node)
    }
}

#[derive(Debug)]
enum TreeParseError {
    Malformed(&'static str),
    UnsupportedExpectation(String),
}

fn parse_tree_node(text: &str) -> Result<TreeNode, TreeParseError> {
    if let Some(text) = text.strip_prefix('"') {
        return serde_json::from_str::<String>(&format!("\"{text}"))
            .map(TreeNode::Text)
            .map_err(|_| TreeParseError::Malformed("WPT text node is not a valid quoted string"));
    }
    if let Some(comment) = text
        .strip_prefix("<!-- ")
        .and_then(|text| text.strip_suffix(" -->"))
    {
        return Ok(TreeNode::Comment(comment.to_string()));
    }
    if let Some(doctype) = text
        .strip_prefix("<!DOCTYPE ")
        .and_then(|text| text.strip_suffix('>'))
    {
        return parse_doctype(doctype);
    }
    if let Some(pi) = text.strip_prefix("<?") {
        let Some(pi) = pi.strip_suffix("?>") else {
            return Err(TreeParseError::Malformed(
                "processing instruction is not closed",
            ));
        };
        let Some((target, data)) = pi.split_once(' ') else {
            return Err(TreeParseError::Malformed(
                "processing instruction requires a separator space",
            ));
        };
        if target.is_empty() {
            return Err(TreeParseError::Malformed(
                "processing instruction has no target",
            ));
        }
        return Ok(TreeNode::ProcessingInstruction {
            target: target.to_string(),
            data: data.to_string(),
        });
    }
    let Some(element) = text
        .strip_prefix('<')
        .and_then(|text| text.strip_suffix('>'))
    else {
        return Err(TreeParseError::Malformed(
            "WPT tree node is not well framed",
        ));
    };
    let (namespace, local_name) = parse_wpt_element_name(element)?;
    if local_name == "template" && namespace == "html" {
        return Err(TreeParseError::UnsupportedExpectation(
            "template content is not represented by the AE13e adapter".to_string(),
        ));
    }
    Ok(TreeNode::Element {
        namespace,
        local_name: local_name.to_string(),
        attributes: Vec::new(),
        children: Vec::new(),
    })
}

fn parse_wpt_element_name(value: &str) -> Result<(&'static str, &str), TreeParseError> {
    let fields = value.split(' ').collect::<Vec<_>>();
    match fields.as_slice() {
        [local_name] if !local_name.is_empty() && !contains_whitespace(local_name) => {
            Ok(("html", local_name))
        }
        [namespace, local_name] if !local_name.is_empty() && !contains_whitespace(local_name) => {
            match *namespace {
                "svg" => Ok(("svg", local_name)),
                "math" => Ok(("mathml", local_name)),
                _ => Err(TreeParseError::Malformed(
                    "unknown WPT element namespace designator",
                )),
            }
        }
        [_namespace, _local_name] => Err(TreeParseError::Malformed(
            "WPT element namespace designator requires a non-empty local name",
        )),
        _ => Err(TreeParseError::Malformed(
            "WPT element has malformed namespace/name fields",
        )),
    }
}

fn contains_whitespace(value: &str) -> bool {
    value.chars().any(char::is_whitespace)
}

fn parse_tree_attribute(name: &str, value: &str) -> Result<TreeAttribute, TreeParseError> {
    let value = serde_json::from_str::<String>(value.trim()).map_err(|_| {
        if value.trim_start().starts_with('"') && !value.trim_end().ends_with('"') {
            TreeParseError::UnsupportedExpectation(
                "multi-line or otherwise unrepresentable WPT attribute value".to_string(),
            )
        } else {
            TreeParseError::Malformed("WPT attribute value is not a quoted string")
        }
    })?;
    let fields = name.split(' ').collect::<Vec<_>>();
    let (namespace, prefix, local_name) = match fields.as_slice() {
        [local_name] if !local_name.is_empty() && !contains_whitespace(local_name) => {
            ("none", None, (*local_name).to_string())
        }
        [namespace, local_name] if !local_name.is_empty() && !contains_whitespace(local_name) => {
            if !matches!(*namespace, "xml" | "xmlns" | "xlink") {
                return Err(TreeParseError::Malformed(
                    "unknown WPT attribute namespace designator",
                ));
            }
            return Err(TreeParseError::UnsupportedExpectation(
                "WPT namespace-designated attribute cannot be represented without asserting a DOM prefix"
                    .to_string(),
            ));
        }
        [_namespace, _local_name] => {
            return Err(TreeParseError::Malformed(
                "WPT attribute namespace designator requires a non-empty local name",
            ));
        }
        _ => {
            return Err(TreeParseError::Malformed(
                "WPT attribute has malformed namespace/name fields",
            ));
        }
    };
    Ok(TreeAttribute {
        namespace,
        prefix: prefix.map(str::to_string),
        local_name,
        value,
    })
}

fn parse_doctype(doctype: &str) -> Result<TreeNode, TreeParseError> {
    let mut parts = doctype.splitn(2, char::is_whitespace);
    let Some(name) = parts.next().filter(|name| !name.is_empty()) else {
        return Err(TreeParseError::Malformed("DOCTYPE has no name"));
    };
    let rest = parts.next().unwrap_or("").trim_start();
    if rest.is_empty() {
        return Ok(TreeNode::Doctype {
            name: name.to_string(),
            public_id: None,
            system_id: None,
        });
    }
    let (public_id, rest) = take_quoted_value(rest)?;
    let (system_id, rest) = take_quoted_value(rest)?;
    if !rest.trim().is_empty() {
        return Err(TreeParseError::Malformed("DOCTYPE has trailing fields"));
    }
    Ok(TreeNode::Doctype {
        name: name.to_string(),
        public_id: Some(public_id),
        system_id: Some(system_id),
    })
}

fn take_quoted_value(value: &str) -> Result<(String, &str), TreeParseError> {
    let value = value.trim_start();
    let Some(value) = value.strip_prefix('"') else {
        return Err(TreeParseError::Malformed(
            "DOCTYPE identifier is not quoted",
        ));
    };
    let Some(end) = value.find('"') else {
        return Err(TreeParseError::Malformed(
            "DOCTYPE identifier is not closed",
        ));
    };
    Ok((value[..end].to_string(), &value[end + 1..]))
}

fn generate_fixture_files(
    record: &DatRecord,
    bundle_name: &str,
    source_path: &str,
    source_file_sha256: &str,
    case_identity: &str,
    allowlist: &AllowlistFile,
) -> Result<BTreeMap<String, Vec<u8>>, ExternalAdapterError> {
    let tree = serialize_tree(&record.tree);
    let source_record_sha256 = sha256_hex(record.raw.as_bytes());
    let adaptation = format!(
        "Representation-only translation of the pinned WPT #data, #errors count, and #document tree; no upstream diagnostic text is mapped to Borrowser error identities. Case identity: {case_identity}."
    );
    let provenance = toml::to_string(&GeneratedExternalProvenance {
        format: EXTERNAL_PROVENANCE_FORMAT,
        upstream_project: &allowlist.upstream_project,
        upstream_revision: &allowlist.upstream_revision,
        upstream_path: source_path,
        source_record_ordinal: record.ordinal,
        source_record_sha256: &source_record_sha256,
        source_file_sha256,
        license_identifier: &allowlist.license_identifier,
        license_notice: &allowlist.license_notice,
        attribution: &allowlist.attribution,
        adaptation: &adaptation,
    })
    .map_err(|error| {
        ExternalAdapterError::InvalidAllowlist(format!(
            "generated external provenance is not serializable: {error}"
        ))
    })?;
    let provenance_sha256 = sha256_hex(provenance.as_bytes());
    let input_sha256 = sha256_hex(&record.input);
    let fixture = format!(
        "format = \"borrowser-html-parser-fixture-v3\"\nid = \"{bundle_name}\"\n\n[source]\nkind = \"external\"\nprovenance_record = \"provenance.toml\"\nprovenance_sha256 = \"{provenance_sha256}\"\n\n[input]\npath = \"input.html\"\nkind = \"utf8-text\"\nsha256 = \"{input_sha256}\"\n\n[execution]\ntarget = {{ kind = \"document\", scripting = \"disabled\" }}\nreference_delivery = \"whole-unicode\"\n[[execution.deliveries]]\nname = \"whole-unicode\"\nunit = \"unicode-scalars\"\nstrategy = \"whole\"\n\n[expectations]\nparse_errors = {{ kind = \"count\", count = {} }}\ntree = \"tree.txt\"\n\n[disposition]\nstatus = \"active\"\n",
        record.parse_error_count,
    );
    let mut files = BTreeMap::new();
    files.insert("fixture.toml".to_string(), fixture.into_bytes());
    files.insert("input.html".to_string(), record.input.clone());
    files.insert("tree.txt".to_string(), tree.into_bytes());
    files.insert("provenance.toml".to_string(), provenance.into_bytes());
    Ok(files)
}

fn serialize_tree(root: &TreeNode) -> String {
    let mut output = String::from("# format: html5-dom-v3\n");
    serialize_tree_node(root, "/root[0]", &mut output);
    output
}

fn serialize_tree_node(node: &TreeNode, path: &str, output: &mut String) {
    match node {
        TreeNode::Document { children } => {
            let _ = writeln!(output, "NODE path={path} kind=document");
            serialize_children(children, path, output);
        }
        TreeNode::Element {
            namespace,
            local_name,
            attributes,
            children,
        } => {
            let _ = writeln!(
                output,
                "NODE path={path} kind=element namespace={namespace} local-name={}",
                quote(local_name)
            );
            for (index, attribute) in attributes.iter().enumerate() {
                let _ = writeln!(
                    output,
                    "ATTRIBUTE path={path} index={index} namespace={} prefix={} local-name={} value={}",
                    attribute.namespace,
                    attribute
                        .prefix
                        .as_deref()
                        .map_or("null".to_string(), quote),
                    quote(&attribute.local_name),
                    quote(&attribute.value)
                );
            }
            serialize_children(children, path, output);
        }
        TreeNode::Text(data) => {
            let _ = writeln!(output, "NODE path={path} kind=text data={}", quote(data));
        }
        TreeNode::Comment(data) => {
            let _ = writeln!(output, "NODE path={path} kind=comment data={}", quote(data));
        }
        TreeNode::Doctype {
            name,
            public_id,
            system_id,
        } => {
            let _ = writeln!(
                output,
                "NODE path={path} kind=document-type name={} public-id={} system-id={}",
                quote(name),
                public_id.as_deref().map_or("null".to_string(), quote),
                system_id.as_deref().map_or("null".to_string(), quote),
            );
        }
        TreeNode::ProcessingInstruction { target, data } => {
            let _ = writeln!(
                output,
                "NODE path={path} kind=processing-instruction target={} data={}",
                quote(target),
                quote(data)
            );
        }
    }
}

fn serialize_children(children: &[TreeNode], parent_path: &str, output: &mut String) {
    for (index, child) in children.iter().enumerate() {
        serialize_tree_node(child, &format!("{parent_path}/child[{index}]"), output);
    }
}

fn quote(value: &str) -> String {
    serde_json::to_string(value).expect("strings are serializable")
}

fn sanitize_identifier(value: &str) -> String {
    let mut result = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            result.push(character.to_ascii_lowercase());
        } else if !result.ends_with('-') {
            result.push('-');
        }
    }
    result.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser_fixture::{
        FixtureRepository, FixtureRepositoryPolicy, discover_and_load, run_fixture,
    };

    #[test]
    fn record_hash_identity_and_script_default_are_deterministic() {
        let records = parse_dat_records(
            "tests1.dat",
            "#data\n<p>x\n#errors\n(1,1): error\n#document\n| <html>\n|   <head>\n|   <body>\n\n#data\n<head><noscript></noscript>\n#errors\n(1,1): error\n#script-off\n#document\n| <html>\n|   <head>\n|     <noscript>\n|   <body>\n",
        )
        .expect("records parse");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].scripting, ScriptingMarker::Absent);
        assert_eq!(records[1].scripting, ScriptingMarker::ExplicitOff);
        assert_eq!(sha256_hex(records[1].raw.as_bytes()).len(), 64);
    }

    fn parse_minimal_record(data: &str) -> DatRecord {
        parse_dat_record(
            "inline.dat",
            1,
            format!("#data\n{data}#errors\n#document\n| <html>\n"),
        )
        .expect("minimal WPT record")
    }

    #[test]
    fn data_removes_only_the_structural_final_lf() {
        assert_eq!(
            parse_minimal_record("ordinary\n").input,
            b"ordinary".to_vec()
        );
        assert_eq!(parse_minimal_record("line\n\n").input, b"line\n".to_vec());
        assert_eq!(
            parse_minimal_record("line\n\n\n").input,
            b"line\n\n".to_vec()
        );
        assert!(parse_minimal_record("").input.is_empty());
    }

    #[test]
    fn marker_like_data_lines_remain_parser_input() {
        let data = "#document\n#script-off\n#document-fragment\npayload\n";
        let record = parse_minimal_record(data);
        assert_eq!(record.input, data.trim_end_matches('\n').as_bytes());
    }

    #[test]
    fn marker_like_error_and_tree_text_is_not_section_structure() {
        let record = parse_dat_record(
            "inline.dat",
            1,
            "#data\nx\n#errors\n(1,0): #document #script-off #document-fragment\n#document\n| <html>\n|   \"#document #script-off #document-fragment\"\n"
                .to_string(),
        )
        .expect("marker-like content is not a section marker");
        assert_eq!(record.parse_error_count, 1);
        assert!(serialize_tree(&record.tree).contains("#document #script-off"));
    }

    #[test]
    fn malformed_section_ordering_is_typed_without_panicking() {
        let error = parse_dat_record(
            "inline.dat",
            1,
            "#data\nx\n#errors\n#document-fragment\nctx\n#new-errors\n#document\n| <html>\n"
                .to_string(),
        )
        .expect_err("invalid section ordering");
        assert!(matches!(error, ExternalAdapterError::InvalidRecord { .. }));
    }

    #[test]
    fn parser_text_containing_document_write_does_not_require_the_api() {
        let record = parse_dat_record(
            "inline.dat",
            1,
            "#data\n<p>document.write</p>\n#errors\n#script-off\n#document\n| <html>\n".to_string(),
        )
        .expect("literal parser input");
        assert_eq!(
            classify_record(&record),
            ExternalCaseClassification::Eligible
        );
    }

    #[test]
    fn valid_unsupported_tree_construct_is_not_malformed() {
        let error = parse_dat_record(
            "inline.dat",
            1,
            "#data\nx\n#errors\n#script-off\n#document\n| <template>\n".to_string(),
        )
        .expect_err("template content is outside the adapter profile");
        assert!(matches!(
            error,
            ExternalAdapterError::UnsupportedRecord {
                capability: ExternalCapability::UnsupportedExpectationRepresentation,
                ..
            }
        ));
    }

    #[test]
    fn tree_namespace_attributes_and_lossless_nodes_are_structured() {
        let tree = parse_tree(
            "inline.dat",
            1,
            "| <html>\n|   <svg svg>\n|     xml:base=\"value\"\n|     xlink:href=\"value\"\n|     <!--  -->\n|     <?COMMENT ?>\n|     <!DOCTYPE html \"public\" \"system\">\n",
        )
        .expect("supported tree representation");
        let serialized = serialize_tree(&tree);
        assert!(serialized.contains("namespace=svg"));
        assert!(serialized.contains("namespace=none prefix=null local-name=\"xml:base\""));
        assert!(serialized.contains("namespace=none prefix=null local-name=\"xlink:href\""));
        assert!(serialized.contains("data=\"\""));
        assert!(serialized.contains("target=\"COMMENT\""));
        assert!(serialized.contains("public-id=\"public\""));
    }

    #[test]
    fn namespace_designated_attributes_are_unsupported_without_prefix_strengthening() {
        for name in ["xml lang", "xlink href", "xmlns foo"] {
            let error = parse_tree("inline.dat", 1, &format!("| <html>\n|   {name}=\"\"\n"))
                .expect_err("the canonical tree format has no unconstrained prefix field");
            assert!(matches!(
                error,
                ExternalAdapterError::UnsupportedRecord {
                    capability: ExternalCapability::UnsupportedExpectationRepresentation,
                    ..
                }
            ));
        }

        let attribute = parse_tree_attribute("xml:base", "\"value\"")
            .expect("a colon in a one-field WPT name is part of the local name");
        assert_eq!(attribute.namespace, "none");
        assert_eq!(attribute.prefix, None);
        assert_eq!(attribute.local_name, "xml:base");
    }

    #[test]
    fn invalid_namespace_and_name_fields_are_malformed_records() {
        for tree in [
            "| <bogus html>\n",
            "| <html one two>\n",
            "| <html>\n|   bogus href=\"\"\n",
            "| <html>\n|   xml lang extra=\"\"\n",
            "| <html>\n|   xlink href extra=\"\"\n",
        ] {
            let error = parse_dat_record(
                "inline.dat",
                1,
                format!("#data\nx\n#errors\n#script-off\n#document\n{tree}"),
            )
            .expect_err("invalid WPT tree fields must be malformed");
            assert!(matches!(error, ExternalAdapterError::InvalidRecord { .. }));
        }
    }

    #[test]
    fn pinned_webkit_namespace_records_are_explicitly_unsupported() {
        let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source = fs::read_to_string(
            repository_root
                .join("tests/wpt/external/raw/html/syntax/parsing/resources/webkit02.dat"),
        )
        .expect("pinned webkit source");
        let record = split_dat_records(&source)
            .get(22)
            .cloned()
            .expect("pinned namespace record");
        let error = parse_dat_record("webkit02.dat", 23, record)
            .expect_err("WPT namespace designators are outside the exact canonical profile");
        assert!(matches!(
            error,
            ExternalAdapterError::UnsupportedRecord {
                capability: ExternalCapability::UnsupportedExpectationRepresentation,
                ..
            }
        ));
    }

    #[test]
    fn tree_indentation_and_attribute_ownership_are_structural() {
        for valid in [
            "| <html>\n|   <body>\n|     <p>\n|       \"text\"\n",
            "| <html>\n|   <head>\n|   <body>\n|     <p>\n|     <div>\n|   <!-- comment -->\n",
            "| <html>\n|   <body>\n|     <p>\n|   <head>\n",
            "| <html>\n|   id=\"root\"\n",
        ] {
            parse_tree("inline.dat", 1, valid).expect("valid WPT tree indentation");
        }

        for malformed in [
            "| <html>\n|     <body>\n",
            "| <html>\n|   <body>\n| id=\"wrong-depth\"\n",
            "| id=\"no-owner\"\n",
            "| <html>\n|   <body>\n|       id=\"wrong-owner-depth\"\n",
        ] {
            assert!(matches!(
                parse_tree("inline.dat", 1, malformed),
                Err(ExternalAdapterError::InvalidRecord { .. })
            ));
        }
    }

    #[test]
    fn wpt_doctype_serialization_decodes_identifier_pairs_without_keywords() {
        let cases = [
            ("html", None, None),
            ("html \"public\" \"system\"", Some("public"), Some("system")),
            ("html \"\" \"system\"", Some(""), Some("system")),
            ("html \"public\" \"\"", Some("public"), Some("")),
        ];
        for (source, public_id, system_id) in cases {
            let TreeNode::Doctype {
                public_id: actual_public,
                system_id: actual_system,
                ..
            } = parse_doctype(source).expect("WPT doctype")
            else {
                panic!("expected doctype node");
            };
            assert_eq!(actual_public.as_deref(), public_id);
            assert_eq!(actual_system.as_deref(), system_id);
        }
    }

    #[test]
    fn non_wpt_doctype_forms_and_malformed_identifier_pairs_are_rejected() {
        for source in [
            "html PUBLIC \"public\" \"system\"",
            "html \"public\"",
            "html public \"system\"",
            "html \"public\" \"system\" trailing",
        ] {
            assert!(matches!(
                parse_doctype(source),
                Err(TreeParseError::Malformed(_))
            ));
        }
    }

    #[test]
    fn tree_doctype_requires_the_exact_wpt_framing() {
        for source in ["<!DOCTYPE html>", "<!DOCTYPE html \"public\" \"system\">"] {
            parse_tree_node(source).expect("framed WPT doctype");
        }
        for source in ["DOCTYPE html", "DOCTYPE html \"public\" \"system\""] {
            assert!(matches!(
                parse_tree_node(source),
                Err(TreeParseError::Malformed(_))
            ));
        }
    }

    #[test]
    fn malformed_tree_names_are_not_whitespace_normalized() {
        for source in ["<svg >", "<math >", "<svg  path>", "< svg>"] {
            assert!(matches!(
                parse_tree_node(source),
                Err(TreeParseError::Malformed(_))
            ));
        }
        for source in ["xml =\"\"", "xlink =\"\"", "bogus =\"\""] {
            assert!(matches!(
                parse_tree_attribute(source.split_once('=').unwrap().0, "\"\""),
                Err(TreeParseError::Malformed(_))
            ));
        }
    }

    #[test]
    fn valid_wpt_element_and_attribute_name_fields_remain_distinct() {
        for source in ["<html>", "<svg path>", "<math mi>"] {
            parse_tree_node(source).expect("valid WPT element name");
        }
        parse_tree_attribute("id", "\"value\"").expect("valid no-namespace attribute");
        for source in ["xml lang", "xlink href", "xmlns foo"] {
            assert!(matches!(
                parse_tree_attribute(source, "\"value\""),
                Err(TreeParseError::UnsupportedExpectation(_))
            ));
        }
        let attribute = parse_tree_attribute("xml:base", "\"value\"")
            .expect("colon remains part of a one-field local name");
        assert_eq!(attribute.namespace, "none");
        assert_eq!(attribute.local_name, "xml:base");
    }

    #[test]
    fn malformed_tree_names_reach_invalid_record_at_dat_boundary() {
        for tree in [
            "| DOCTYPE html\n",
            "| DOCTYPE html \"public\" \"system\"\n",
            "| <!DOCTYPE html\n",
            "| <svg >\n",
            "| <html>\n|   xml =\"\"\n",
        ] {
            let error = parse_dat_record(
                "inline.dat",
                1,
                format!("#data\nx\n#errors\n#script-off\n#document\n{tree}"),
            )
            .expect_err("malformed WPT tree serialization");
            assert!(matches!(error, ExternalAdapterError::InvalidRecord { .. }));
        }
    }

    #[test]
    fn comments_and_processing_instructions_consume_only_wpt_framing() {
        let TreeNode::Comment(empty) = parse_tree_node("<!--  -->").expect("empty comment") else {
            panic!("expected comment");
        };
        assert_eq!(empty, "");
        let TreeNode::Comment(ordinary) = parse_tree_node("<!-- X -->").expect("comment") else {
            panic!("expected comment");
        };
        assert_eq!(ordinary, "X");
        let TreeNode::Comment(spaced) = parse_tree_node("<!--  X  -->").expect("spaced comment")
        else {
            panic!("expected comment");
        };
        assert_eq!(spaced, " X ");

        let TreeNode::ProcessingInstruction { data: empty, .. } =
            parse_tree_node("<?target ?>").expect("empty PI")
        else {
            panic!("expected processing instruction");
        };
        assert_eq!(empty, "");
        let TreeNode::ProcessingInstruction { data: ordinary, .. } =
            parse_tree_node("<?target data?>").expect("PI")
        else {
            panic!("expected processing instruction");
        };
        assert_eq!(ordinary, "data");
        let TreeNode::ProcessingInstruction { data: leading, .. } =
            parse_tree_node("<?target  data?>").expect("PI with leading data space")
        else {
            panic!("expected processing instruction");
        };
        assert_eq!(leading, " data");
    }

    #[test]
    fn multiline_tree_values_are_explicitly_unsupported_not_malformed() {
        let error = parse_dat_record(
            "inline.dat",
            1,
            "#data\nx\n#errors\n#script-off\n#document\n| <html>\n|   \"first line\nsecond line\"\n"
                .to_string(),
        )
        .expect_err("multiline WPT value is outside the narrow adapter profile");
        assert!(matches!(
            error,
            ExternalAdapterError::UnsupportedRecord {
                capability: ExternalCapability::UnsupportedExpectationRepresentation,
                ..
            }
        ));
    }

    #[test]
    fn malformed_closed_tree_text_is_not_an_expectation_limitation() {
        let error = parse_dat_record(
            "inline.dat",
            1,
            "#data\nx\n#errors\n#script-off\n#document\n| <html>\n|   \"\\x\"\n".to_string(),
        )
        .expect_err("invalid quoted text is malformed WPT serialization");
        assert!(matches!(error, ExternalAdapterError::InvalidRecord { .. }));
    }

    #[test]
    fn error_sections_count_nonempty_entries_and_reject_blank_entries() {
        let zero = parse_dat_record(
            "inline.dat",
            1,
            "#data\nx\n#errors\n#script-off\n#document\n| <html>\n".to_string(),
        )
        .expect("zero errors");
        assert_eq!(zero.parse_error_count, 0);
        let multiple = parse_dat_record(
            "inline.dat",
            1,
            "#data\nx\n#errors\nfirst\nsecond\n#new-errors\nthird\n#script-off\n#document\n| <html>\n"
                .to_string(),
        )
        .expect("multiple errors");
        assert_eq!(multiple.parse_error_count, 3);
        let blank = parse_dat_record(
            "inline.dat",
            1,
            "#data\nx\n#errors\n\n#script-off\n#document\n| <html>\n".to_string(),
        )
        .expect_err("blank error entry");
        assert!(matches!(blank, ExternalAdapterError::InvalidRecord { .. }));
    }

    #[test]
    fn allowlist_revision_metadata_is_required_and_validated() {
        assert!(validate_revision("").is_err());
        assert!(validate_revision("not-a-commit").is_err());
        assert!(toml::from_str::<AllowlistFile>(
            "format = \"borrowser-wpt-external-allowlist-v1\"\nupstream_project = \"web-platform-tests/wpt\"\nsource_path = \"tests1.dat\"\nsource_file_sha256 = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\nlicense_identifier = \"BSD-3-Clause\"\nlicense_notice = \"notice\"\nattribution = \"attribution\"\n"
        )
        .is_err());
        let mut allowlist = AllowlistFile {
            format: "borrowser-wpt-external-allowlist-v1".to_string(),
            upstream_project: "web-platform-tests/wpt".to_string(),
            upstream_revision: "".to_string(),
            source_path: "tests1.dat".to_string(),
            source_file_sha256: "a".repeat(64),
            license_identifier: "BSD-3-Clause".to_string(),
            license_notice: "notice".to_string(),
            attribution: "attribution".to_string(),
            cases: Vec::new(),
        };
        assert!(validate_allowlist_metadata(&allowlist).is_err());
        allowlist.upstream_revision = "g".repeat(40);
        assert!(validate_allowlist_metadata(&allowlist).is_err());
    }

    #[test]
    fn unsupported_default_scripting_is_explicit() {
        let record = DatRecord {
            ordinal: 1,
            raw: String::new(),
            input: Vec::new(),
            parse_error_count: 0,
            scripting: ScriptingMarker::Absent,
            fragment_context: None,
            tree: TreeNode::Document {
                children: Vec::new(),
            },
        };
        assert_eq!(
            classify_record(&record),
            ExternalCaseClassification::Unsupported(ExternalCapability::Scripting)
        );
    }

    #[test]
    fn pinned_allowlist_adapts_real_records_and_classifies_default_scripting() {
        let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let webkit_source = fs::read_to_string(
            repository_root
                .join("tests/wpt/external/raw/html/syntax/parsing/resources/webkit02.dat"),
        )
        .expect("webkit source");
        let selected_record = split_dat_records(&webkit_source)
            .get(2)
            .cloned()
            .expect("selected webkit record");
        assert_eq!(
            parse_dat_record("webkit02.dat", 3, selected_record)
                .unwrap()
                .parse_error_count,
            1
        );
        let output = adapt_allowlisted_subset(
            &repository_root.join("tests/wpt/external/raw"),
            &repository_root.join("tests/wpt/external/allowlist.toml"),
        )
        .expect("pinned external records adapt");
        assert_eq!(output.artifacts().len(), 3);
        assert_eq!(
            output
                .artifacts()
                .iter()
                .filter(
                    |artifact| artifact.classification() == &ExternalCaseClassification::Eligible
                )
                .count(),
            1
        );
        assert!(output.artifacts().iter().any(|artifact| {
            artifact.classification()
                == &ExternalCaseClassification::Unsupported(ExternalCapability::Scripting)
        }));
        let eligible = output
            .artifacts()
            .iter()
            .find(|artifact| artifact.classification() == &ExternalCaseClassification::Eligible)
            .expect("one eligible record");
        assert!(eligible.files().contains_key("fixture.toml"));
        assert!(eligible.files().contains_key("provenance.toml"));
    }

    #[test]
    fn eligible_adapter_output_enters_the_canonical_validation_and_runner_path() {
        let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = adapt_allowlisted_subset(
            &repository_root.join("tests/wpt/external/raw"),
            &repository_root.join("tests/wpt/external/allowlist.toml"),
        )
        .expect("pinned external records adapt");
        let eligible = output
            .artifacts()
            .iter()
            .find(|artifact| artifact.classification() == &ExternalCaseClassification::Eligible)
            .expect("one eligible record");
        let generated_fixture = String::from_utf8(eligible.files()["fixture.toml"].clone())
            .expect("generated fixture is UTF-8");
        assert!(
            generated_fixture.contains("count = 1"),
            "{generated_fixture}"
        );
        let temporary = tempfile::tempdir().expect("temporary repository");
        let fixture_root = temporary.path().join("fixtures");
        let bundle_root = fixture_root.join(eligible.bundle_name());
        fs::create_dir_all(&bundle_root).expect("fixture bundle directory");
        for (relative, bytes) in eligible.files() {
            fs::write(bundle_root.join(relative), bytes).expect("generated fixture file");
        }
        let repository = FixtureRepository {
            repository_root: temporary.path().to_path_buf(),
            fixture_root,
            policy: FixtureRepositoryPolicy::AdaptedOrQuarantine,
        };
        let fixtures = discover_and_load(&repository).expect("canonical v3 validation");
        assert_eq!(fixtures.len(), 1);
        let report = run_fixture(&fixtures[0]).expect("canonical external fixture execution");
        assert!(report.result().is_some());
    }

    #[test]
    fn generated_provenance_serialization_preserves_arbitrary_metadata() {
        let record = parse_minimal_record("x\n");
        let allowlist = AllowlistFile {
            format: "borrowser-wpt-external-allowlist-v1".to_string(),
            upstream_project: "project \"quoted\"".to_string(),
            upstream_revision: "revision".to_string(),
            source_path: "tests1.dat".to_string(),
            source_file_sha256: "a".repeat(64),
            license_identifier: "BSD-3-Clause".to_string(),
            license_notice: "notice \"quoted\"\nsecond line".to_string(),
            attribution: "attribution \\ quoted".to_string(),
            cases: Vec::new(),
        };
        let files = generate_fixture_files(
            &record,
            "wpt-case",
            "tests1.dat",
            &allowlist.source_file_sha256,
            "revision:tests1.dat:1:hash",
            &allowlist,
        )
        .expect("typed provenance serialization");
        let provenance =
            std::str::from_utf8(&files["provenance.toml"]).expect("generated provenance is UTF-8");
        let value = toml::from_str::<toml::Value>(provenance).expect("valid generated TOML");
        assert_eq!(
            value["license_notice"].as_str(),
            Some("notice \"quoted\"\nsecond line")
        );
        assert_eq!(value["attribution"].as_str(), Some("attribution \\ quoted"));
    }

    #[test]
    fn generic_fixture_v3_provenance_accepts_non_git_revision_identifiers() {
        let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = adapt_allowlisted_subset(
            &repository_root.join("tests/wpt/external/raw"),
            &repository_root.join("tests/wpt/external/allowlist.toml"),
        )
        .expect("pinned external records adapt");
        let eligible = output
            .artifacts()
            .iter()
            .find(|artifact| artifact.classification() == &ExternalCaseClassification::Eligible)
            .expect("one eligible record");
        let temporary = tempfile::tempdir().expect("temporary repository");
        let fixture_root = temporary.path().join("fixtures");
        let bundle_root = fixture_root.join(eligible.bundle_name());
        fs::create_dir_all(&bundle_root).expect("fixture bundle directory");
        let old_revision = "2c705104a295c48053eeddf7fe0170d790a4e853";
        let mut files = eligible.files().clone();
        let provenance = String::from_utf8(files["provenance.toml"].clone())
            .expect("provenance UTF-8")
            .replace(old_revision, "source-release-2026");
        files.insert(
            "provenance.toml".to_string(),
            provenance.as_bytes().to_vec(),
        );
        let fixture = String::from_utf8(files["fixture.toml"].clone())
            .expect("fixture UTF-8")
            .replace(
                &format!(
                    "provenance_sha256 = \"{}\"",
                    sha256_hex(eligible.files()["provenance.toml"].as_slice())
                ),
                &format!(
                    "provenance_sha256 = \"{}\"",
                    sha256_hex(provenance.as_bytes())
                ),
            );
        files.insert("fixture.toml".to_string(), fixture.into_bytes());
        for (relative, bytes) in files {
            fs::write(bundle_root.join(relative), bytes).expect("fixture file");
        }
        let repository = FixtureRepository {
            repository_root: temporary.path().to_path_buf(),
            fixture_root,
            policy: FixtureRepositoryPolicy::AdaptedOrQuarantine,
        };
        assert_eq!(
            discover_and_load(&repository)
                .expect("source-agnostic provenance")
                .len(),
            1
        );
    }

    #[test]
    fn malformed_external_provenance_cannot_cross_canonical_validation() {
        let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = adapt_allowlisted_subset(
            &repository_root.join("tests/wpt/external/raw"),
            &repository_root.join("tests/wpt/external/allowlist.toml"),
        )
        .expect("pinned external records adapt");
        let eligible = output
            .artifacts()
            .iter()
            .find(|artifact| artifact.classification() == &ExternalCaseClassification::Eligible)
            .expect("one eligible record");
        let temporary = tempfile::tempdir().expect("temporary repository");
        let fixture_root = temporary.path().join("fixtures");
        let bundle_root = fixture_root.join(eligible.bundle_name());
        fs::create_dir_all(&bundle_root).expect("fixture bundle directory");
        let mut files = eligible.files().clone();
        let provenance = String::from_utf8(files["provenance.toml"].clone())
            .expect("provenance UTF-8")
            .replace("license_identifier = \"BSD-3-Clause\"\n", "");
        files.insert(
            "provenance.toml".to_string(),
            provenance.as_bytes().to_vec(),
        );
        let fixture = String::from_utf8(files["fixture.toml"].clone())
            .expect("fixture UTF-8")
            .replace(
                &format!(
                    "provenance_sha256 = \"{}\"",
                    sha256_hex(eligible.files()["provenance.toml"].as_slice())
                ),
                &format!(
                    "provenance_sha256 = \"{}\"",
                    sha256_hex(provenance.as_bytes())
                ),
            );
        files.insert("fixture.toml".to_string(), fixture.into_bytes());
        for (relative, bytes) in files {
            fs::write(bundle_root.join(relative), bytes).expect("fixture file");
        }
        let repository = FixtureRepository {
            repository_root: temporary.path().to_path_buf(),
            fixture_root,
            policy: FixtureRepositoryPolicy::AdaptedOrQuarantine,
        };
        let error = discover_and_load(&repository).expect_err("missing licence is rejected");
        assert!(
            error.to_string().contains("license_identifier"),
            "unexpected validation error: {error}"
        );
    }

    #[test]
    fn malformed_allowlisted_record_is_explicitly_classified() {
        let temporary = tempfile::tempdir().expect("temporary repository");
        let raw_root = temporary.path().join("raw");
        fs::create_dir_all(&raw_root).expect("raw directory");
        let source = "#data\nnot complete\n#document\n| <html>\n";
        let source_path = "malformed.dat";
        let record_hash = sha256_hex(source.as_bytes());
        let source_hash = sha256_hex(source.as_bytes());
        fs::write(raw_root.join(source_path), source).expect("raw source");
        let allowlist = format!(
            "format = \"borrowser-wpt-external-allowlist-v1\"\nupstream_project = \"web-platform-tests/wpt\"\nupstream_revision = \"2c705104a295c48053eeddf7fe0170d790a4e853\"\nsource_path = \"{source_path}\"\nsource_file_sha256 = \"{source_hash}\"\nlicense_identifier = \"BSD-3-Clause\"\nlicense_notice = \"notice\"\nattribution = \"attribution\"\n\n[[cases]]\nordinal = 1\nrecord_sha256 = \"{record_hash}\"\ndisplay_name = \"malformed\"\n"
        );
        let allowlist_path = temporary.path().join("allowlist.toml");
        fs::write(&allowlist_path, allowlist).expect("allowlist");
        let output = adapt_allowlisted_subset(&raw_root, &allowlist_path)
            .expect("malformed records are classified, not silently dropped");
        assert_eq!(output.artifacts().len(), 1);
        assert_eq!(
            output.artifacts()[0].classification(),
            &ExternalCaseClassification::Unsupported(ExternalCapability::MalformedExternalRecord)
        );
    }

    #[test]
    fn count_expectation_mismatch_has_distinct_deterministic_spelling() {
        let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = adapt_allowlisted_subset(
            &repository_root.join("tests/wpt/external/raw"),
            &repository_root.join("tests/wpt/external/allowlist.toml"),
        )
        .expect("pinned external records adapt");
        let eligible = output
            .artifacts()
            .iter()
            .find(|artifact| artifact.classification() == &ExternalCaseClassification::Eligible)
            .expect("one eligible record");
        let temporary = tempfile::tempdir().expect("temporary repository");
        let fixture_root = temporary.path().join("fixtures");
        let bundle_root = fixture_root.join(eligible.bundle_name());
        fs::create_dir_all(&bundle_root).expect("fixture bundle directory");
        for (relative, bytes) in eligible.files() {
            let mut bytes = bytes.clone();
            if relative == "fixture.toml" {
                let text = String::from_utf8(bytes).expect("fixture UTF-8");
                bytes = text.replace("count = 1", "count = 0").into_bytes();
            }
            fs::write(bundle_root.join(relative), bytes).expect("fixture file");
        }
        let repository = FixtureRepository {
            repository_root: temporary.path().to_path_buf(),
            fixture_root,
            policy: FixtureRepositoryPolicy::AdaptedOrQuarantine,
        };
        let fixture = discover_and_load(&repository)
            .expect("count expectation remains structurally valid")
            .remove(0);
        let error = run_fixture(&fixture).expect_err("wrong count must fail");
        assert!(error.to_string().contains("parse-error-count"));
    }
}
