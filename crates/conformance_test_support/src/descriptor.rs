use serde::Deserialize;

use crate::diagnostic::InventoryDiagnosticKind;
use crate::model::{
    CONFORMANCE_FIXTURE_FORMAT_V1, InventoryScope, ObservationSurface, ReferenceKind, SourceKind,
    TestId, TestIdValidationError,
};

#[derive(Clone, Debug)]
pub(crate) struct ParsedDescriptor {
    pub test_path: String,
    pub scope: InventoryScope,
    pub observation: ObservationSurface,
    pub source_kind: SourceKind,
    pub reference: Option<ParsedReference>,
    pub description: String,
    pub test_id: TestId,
}

#[derive(Clone, Debug)]
pub(crate) struct ParsedReference {
    pub kind: ReferenceKind,
    pub path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DescriptorV1 {
    format: String,
    id: String,
    scope: String,
    observation: String,
    test_path: String,
    source: SourceV1,
    reference: Option<ReferenceV1>,
    metadata: MetadataV1,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceV1 {
    kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceV1 {
    kind: String,
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataV1 {
    description: String,
}

pub(crate) struct DescriptorParseResult {
    pub raw_id: Option<String>,
    pub descriptor: Option<ParsedDescriptor>,
    pub diagnostics: Vec<InventoryDiagnosticKind>,
}

pub(crate) fn parse_descriptor(bytes: &[u8]) -> DescriptorParseResult {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            return DescriptorParseResult {
                raw_id: None,
                descriptor: None,
                diagnostics: vec![InventoryDiagnosticKind::InvalidDescriptorShape],
            };
        }
    };
    let value = match toml::from_str::<toml::Value>(text) {
        Ok(value) => value,
        Err(_) => {
            return DescriptorParseResult {
                raw_id: None,
                descriptor: None,
                diagnostics: vec![InventoryDiagnosticKind::MalformedToml],
            };
        }
    };

    let Some(table) = value.as_table() else {
        return DescriptorParseResult {
            raw_id: None,
            descriptor: None,
            diagnostics: vec![InventoryDiagnosticKind::InvalidDescriptorShape],
        };
    };
    let raw_id = table
        .get("id")
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned);
    let format = table
        .get("format")
        .and_then(toml::Value::as_str)
        .unwrap_or("");
    if format != CONFORMANCE_FIXTURE_FORMAT_V1 {
        return DescriptorParseResult {
            raw_id,
            descriptor: None,
            diagnostics: vec![InventoryDiagnosticKind::UnsupportedDescriptorVersion {
                value: format.to_owned(),
            }],
        };
    }

    let unknown_fields = unknown_fields(table);
    if !unknown_fields.is_empty() {
        return DescriptorParseResult {
            raw_id,
            descriptor: None,
            diagnostics: unknown_fields
                .into_iter()
                .map(|field| InventoryDiagnosticKind::UnknownDescriptorField { field })
                .collect(),
        };
    }

    let wire = match toml::from_str::<DescriptorV1>(text) {
        Ok(wire) => wire,
        Err(_) => {
            return DescriptorParseResult {
                raw_id,
                descriptor: None,
                diagnostics: vec![InventoryDiagnosticKind::InvalidDescriptorShape],
            };
        }
    };
    debug_assert_eq!(wire.format, CONFORMANCE_FIXTURE_FORMAT_V1);
    validate_wire(wire)
}

fn unknown_fields(table: &toml::Table) -> Vec<String> {
    let mut fields = Vec::new();
    collect_unknown(
        table,
        "",
        &[
            "format",
            "id",
            "scope",
            "observation",
            "test_path",
            "source",
            "reference",
            "metadata",
        ],
        &mut fields,
    );
    if let Some(source) = table.get("source").and_then(toml::Value::as_table) {
        collect_unknown(source, "source.", &["kind"], &mut fields);
    }
    if let Some(reference) = table.get("reference").and_then(toml::Value::as_table) {
        collect_unknown(reference, "reference.", &["kind", "path"], &mut fields);
    }
    if let Some(metadata) = table.get("metadata").and_then(toml::Value::as_table) {
        collect_unknown(metadata, "metadata.", &["description"], &mut fields);
    }
    fields.sort();
    fields
}

fn collect_unknown(table: &toml::Table, prefix: &str, allowed: &[&str], fields: &mut Vec<String>) {
    for key in table.keys() {
        if !allowed.contains(&key.as_str()) {
            fields.push(format!("{prefix}{key}"));
        }
    }
}

fn validate_wire(wire: DescriptorV1) -> DescriptorParseResult {
    let raw_id = wire.id.clone();
    let mut diagnostics = Vec::new();
    let test_id = match TestId::parse(&wire.id) {
        Ok(id) => Some(id),
        Err(TestIdValidationError::TooLong) => {
            diagnostics.push(InventoryDiagnosticKind::TestIdTooLong {
                value: wire.id.clone(),
            });
            None
        }
        Err(TestIdValidationError::CaseUnsafe) => {
            diagnostics.push(InventoryDiagnosticKind::CaseUnsafeTestId {
                value: wire.id.clone(),
            });
            None
        }
        Err(TestIdValidationError::InvalidGrammar) => {
            diagnostics.push(InventoryDiagnosticKind::InvalidTestId {
                value: wire.id.clone(),
            });
            None
        }
    };
    let scope = match InventoryScope::parse(&wire.scope) {
        Some(scope) => Some(scope),
        None => {
            diagnostics.push(InventoryDiagnosticKind::InvalidScope {
                value: wire.scope.clone(),
            });
            None
        }
    };
    let observation = match ObservationSurface::parse(&wire.observation) {
        Some(observation) => Some(observation),
        None => {
            diagnostics.push(InventoryDiagnosticKind::InvalidObservation {
                value: wire.observation.clone(),
            });
            None
        }
    };
    let source_kind = match SourceKind::parse(&wire.source.kind) {
        Some(kind) => Some(kind),
        None => {
            diagnostics.push(InventoryDiagnosticKind::InvalidSourceKind {
                value: wire.source.kind.clone(),
            });
            None
        }
    };
    let reference = wire.reference.and_then(|reference| {
        ReferenceKind::parse(&reference.kind)
            .map(|kind| ParsedReference {
                kind,
                path: reference.path,
            })
            .or_else(|| {
                diagnostics.push(InventoryDiagnosticKind::InvalidReferenceKind {
                    value: reference.kind,
                });
                None
            })
    });
    if wire.metadata.description.trim().is_empty() {
        diagnostics.push(InventoryDiagnosticKind::EmptyDescription);
    }

    let descriptor = match (test_id, scope, observation, source_kind) {
        (Some(test_id), Some(scope), Some(observation), Some(source_kind))
            if diagnostics.is_empty() =>
        {
            Some(ParsedDescriptor {
                test_path: wire.test_path,
                scope,
                observation,
                source_kind,
                reference,
                description: wire.metadata.description,
                test_id,
            })
        }
        _ => None,
    };
    DescriptorParseResult {
        raw_id: Some(raw_id),
        descriptor,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
format = "borrowser-conformance-fixture-v1"
id = "html-tokenizer-basic"
scope = "static-html-css-no-js"
observation = "html-tokenizer"
test_path = "test.html"

[source]
kind = "native"

[metadata]
description = "A basic tokenizer inventory fixture."
"#;

    #[test]
    fn parses_strict_v1_descriptor() {
        let result = parse_descriptor(VALID.as_bytes());
        assert!(result.diagnostics.is_empty());
        let descriptor = result.descriptor.expect("validated descriptor");
        assert_eq!(descriptor.test_id.as_str(), "html-tokenizer-basic");
        assert_eq!(descriptor.observation, ObservationSurface::HtmlTokenizer);
    }

    #[test]
    fn reports_unknown_fields_with_stable_paths() {
        let text = VALID.replace(
            "kind = \"native\"",
            "kind = \"native\"\nfuture = \"not-accepted\"",
        );
        let result = parse_descriptor(text.as_bytes());
        assert_eq!(
            result.diagnostics,
            vec![InventoryDiagnosticKind::UnknownDescriptorField {
                field: "source.future".to_owned()
            }]
        );
    }
}
