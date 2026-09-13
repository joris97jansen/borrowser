use crate::{CaptureError as E, Result, limits::CONFIG_BYTES, wire};
use external_test_provenance::sha256;
use std::path::Path;
pub const FORMAT: &str = "web-observable-dom-tree-v1-isolated-expression";
pub const INSPECTOR_PATH: &str = "tools/conformance/web-observable-dom-tree-v1.mjs";
pub const PACKAGER_PATH: &str = "crates/external_browser_capture/src/packaging.rs";
const INSPECTOR: &[u8] =
    include_bytes!("../../../tools/conformance/web-observable-dom-tree-v1.mjs");
const PREFIX: &str = "(() => {\n\"use strict\";\n";
const SUFFIX: &str = "\nif (document.readyState !== \"complete\" ||\n    document.contentType !== \"text/html\" ||\n    document.characterSet !== \"UTF-8\") {\n  throw new InspectionFailure(\"capture-document-context\");\n}\nreturn new TextDecoder(\"utf-8\", { fatal: true })\n  .decode(inspectWebObservableDomTreeV1(document));\n})()\n";
const SITES: [&str; 5] = [
    "export const algorithm =",
    "export const MAX_BYTES =",
    "export class InspectionFailure",
    "export function inspectWebObservableDomTreeV1(",
    "export function captureWebObservableDomTreeV1(",
];

pub struct InspectorExpressionV1 {
    text: String,
    digest: String,
}
#[cfg(all(test, feature = "chromium-cdp"))]
pub(crate) fn specimen() -> InspectorExpressionV1 {
    InspectorExpressionV1::package(
        INSPECTOR,
        &sha256(INSPECTOR).to_string(),
        "316a83bad2374261833aa9890399f8fae6417742b0350406f452c52cd3936f0b",
    )
    .unwrap()
}
impl InspectorExpressionV1 {
    pub fn load(
        root: &Path,
        source_sha256: &str,
        packager_sha256: &str,
        expression_sha256: &str,
    ) -> Result<Self> {
        let source = wire::read(root, INSPECTOR_PATH, CONFIG_BYTES)?;
        let packager = wire::read(root, PACKAGER_PATH, CONFIG_BYTES)?;
        if packager != include_bytes!("packaging.rs")
            || sha256(&packager).to_string() != packager_sha256
        {
            return Err(E::Source);
        }
        Self::package(&source, source_sha256, expression_sha256)
    }
    pub fn package(source: &[u8], source_sha256: &str, expected_expression: &str) -> Result<Self> {
        if source.len() > CONFIG_BYTES
            || source != INSPECTOR
            || sha256(source).to_string() != source_sha256
        {
            return Err(E::Source);
        }
        let source = std::str::from_utf8(source).map_err(|_| E::Source)?;
        let mut found = [false; 5];
        let mut text = String::new();
        text.try_reserve(source.len() + PREFIX.len() + SUFFIX.len())
            .map_err(|_| E::Allocation)?;
        text.push_str(PREFIX);
        for line in source.split_inclusive('\n') {
            if let Some(i) = SITES.iter().position(|site| line.starts_with(site)) {
                if found[i] {
                    return Err(E::Source);
                }
                found[i] = true;
                text.push_str(&line[7..]);
            } else {
                text.push_str(line);
            }
        }
        if found != [true; 5] || text.len() != PREFIX.len() + source.len() - 35 {
            return Err(E::Source);
        }
        text.push_str(SUFFIX);
        let digest = sha256(text.as_bytes()).to_string();
        if digest != expected_expression {
            return Err(E::Digest);
        }
        Ok(Self { text, digest })
    }
    pub fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn sha256(&self) -> &str {
        &self.digest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_packaging_and_digest_rejection() {
        let mut expected = String::from(PREFIX);
        for line in std::str::from_utf8(INSPECTOR)
            .unwrap()
            .split_inclusive('\n')
        {
            expected.push_str(line.strip_prefix("export ").unwrap_or(line));
        }
        expected.push_str(SUFFIX);
        let digest = sha256(expected.as_bytes()).to_string();
        let source = sha256(INSPECTOR).to_string();
        let result = InspectorExpressionV1::package(INSPECTOR, &source, &digest).unwrap();
        assert_eq!(result.bytes(), expected.as_bytes());
        assert_eq!(
            result.bytes().len(),
            INSPECTOR.len() - 35 + PREFIX.len() + SUFFIX.len()
        );
        assert!(matches!(
            InspectorExpressionV1::package(INSPECTOR, "wrong", &digest),
            Err(E::Source)
        ));
        assert!(matches!(
            InspectorExpressionV1::package(INSPECTOR, &source, "wrong"),
            Err(E::Digest)
        ));
        let json = serde_json::to_vec(&serde_json::json!({"expression":result.text()})).unwrap();
        let decoded: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(
            decoded["expression"].as_str().unwrap().as_bytes(),
            result.bytes()
        );
    }
}
