use html::{ElementNode, Node};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::fixture::CssHostNamespace;

pub const CSS_NESTED_MAX_TARGET_LABEL_BYTES: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CssTargetLabel(String);

impl CssTargetLabel {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn parse(value: String) -> Result<Self, ()> {
        if value.is_empty()
            || value.len() > CSS_NESTED_MAX_TARGET_LABEL_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(());
        }
        Ok(Self(value))
    }
}

impl<'de> Deserialize<'de> for CssTargetLabel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(|()| D::Error::custom("invalid CSS target label"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CssTargetAddressStep {
    pub(crate) child_index: usize,
    pub(crate) expected_namespace: CssHostNamespace,
    pub(crate) expected_local_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CssTargetAddress {
    pub(crate) label: CssTargetLabel,
    /// Structural assertions over the complete ordinary child list, starting
    /// at the parser-created document root.
    pub(crate) steps: Vec<CssTargetAddressStep>,
}

impl CssTargetAddress {
    pub fn label(&self) -> &str {
        self.label.as_str()
    }

    pub fn steps(&self) -> &[CssTargetAddressStep] {
        &self.steps
    }

    pub(crate) fn validate(&self) -> Result<(), ()> {
        if self.steps.is_empty() {
            return Err(());
        }
        self.steps.iter().try_for_each(|step| {
            valid_local_name(&step.expected_local_name)
                .then_some(())
                .ok_or(())
        })
    }
}

impl CssTargetAddressStep {
    pub fn child_index(&self) -> usize {
        self.child_index
    }

    pub fn expected_namespace(&self) -> CssHostNamespace {
        self.expected_namespace
    }

    pub fn expected_local_name(&self) -> &str {
        &self.expected_local_name
    }
}

fn valid_local_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= html::HtmlTokenizerLimits::default().max_tag_name_bytes
        && !value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssTargetChildKind {
    Document,
    DocumentType,
    Text,
    Comment,
    ProcessingInstruction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssTargetResolutionFailure {
    EmptyAddress,
    ChildMissing {
        depth: usize,
        child_index: usize,
    },
    ChildIsNotElement {
        depth: usize,
        child_index: usize,
        actual: CssTargetChildKind,
    },
    NamespaceMismatch {
        depth: usize,
        child_index: usize,
        expected: CssHostNamespace,
        actual: html::ElementNamespace,
    },
    LocalNameMismatch {
        depth: usize,
        child_index: usize,
        expected: String,
        actual: String,
    },
}

pub(crate) fn resolve_target<'a>(
    root: &'a Node,
    address: &CssTargetAddress,
) -> Result<&'a ElementNode, CssTargetResolutionFailure> {
    let mut current = root;
    let mut target = None;
    for (depth, step) in address.steps.iter().enumerate() {
        let child = current
            .children()
            .and_then(|children| children.get(step.child_index))
            .ok_or(CssTargetResolutionFailure::ChildMissing {
                depth,
                child_index: step.child_index,
            })?;
        let element = match child {
            Node::Element { element } => element,
            other => {
                return Err(CssTargetResolutionFailure::ChildIsNotElement {
                    depth,
                    child_index: step.child_index,
                    actual: child_kind(other),
                });
            }
        };
        if element.namespace() != step.expected_namespace.as_element_namespace() {
            return Err(CssTargetResolutionFailure::NamespaceMismatch {
                depth,
                child_index: step.child_index,
                expected: step.expected_namespace,
                actual: element.namespace(),
            });
        }
        if element.name() != step.expected_local_name {
            return Err(CssTargetResolutionFailure::LocalNameMismatch {
                depth,
                child_index: step.child_index,
                expected: step.expected_local_name.clone(),
                actual: element.name().to_owned(),
            });
        }
        current = child;
        target = Some(element);
    }
    target.ok_or(CssTargetResolutionFailure::EmptyAddress)
}

fn child_kind(node: &Node) -> CssTargetChildKind {
    match node {
        Node::Document { .. } => CssTargetChildKind::Document,
        Node::DocumentType { .. } => CssTargetChildKind::DocumentType,
        Node::Element { .. } => unreachable!("element handled before child-kind classification"),
        Node::Text { .. } => CssTargetChildKind::Text,
        Node::Comment { .. } => CssTargetChildKind::Comment,
        Node::ProcessingInstruction { .. } => CssTargetChildKind::ProcessingInstruction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(child_index: usize, local_name: &str) -> CssTargetAddressStep {
        CssTargetAddressStep {
            child_index,
            expected_namespace: CssHostNamespace::Html,
            expected_local_name: local_name.to_owned(),
        }
    }

    fn address(steps: Vec<CssTargetAddressStep>) -> CssTargetAddress {
        CssTargetAddress {
            label: CssTargetLabel::parse("target".to_owned()).expect("label"),
            steps,
        }
    }

    fn document(source: &str) -> Node {
        html::parse_document(source, html::HtmlParseOptions::default())
            .expect("document")
            .document
    }

    #[test]
    fn exact_parser_created_structure_resolves_from_the_document_root() {
        let document = document(
            "<!doctype html><html><head></head><body><main><article></article></main></body></html>",
        );
        let target = resolve_target(
            &document,
            &address(vec![
                step(1, "html"),
                step(1, "body"),
                step(0, "main"),
                step(0, "article"),
            ]),
        )
        .expect("structural target");
        assert_eq!(target.name(), "article");
    }

    #[test]
    fn text_and_comment_nodes_invalidate_an_old_structural_address() {
        let old = address(vec![step(1, "html"), step(1, "body"), step(0, "article")]);
        let text = document(
            "<!doctype html><html><head></head><body>text<article></article></body></html>",
        );
        assert!(matches!(
            resolve_target(&text, &old),
            Err(CssTargetResolutionFailure::ChildIsNotElement {
                depth: 2,
                child_index: 0,
                actual: CssTargetChildKind::Text,
            })
        ));

        let comment = document(
            "<!doctype html><html><head></head><body><!-- marker --><article></article></body></html>",
        );
        assert!(matches!(
            resolve_target(&comment, &old),
            Err(CssTargetResolutionFailure::ChildIsNotElement {
                depth: 2,
                child_index: 0,
                actual: CssTargetChildKind::Comment,
            })
        ));
    }

    #[test]
    fn intermediate_element_substitution_fails_before_the_final_target() {
        let document = document(
            "<!doctype html><html><head></head><body><section><article></article></section></body></html>",
        );
        let old = address(vec![
            step(1, "html"),
            step(1, "body"),
            step(0, "main"),
            step(0, "article"),
        ]);
        assert!(matches!(
            resolve_target(&document, &old),
            Err(CssTargetResolutionFailure::LocalNameMismatch {
                depth: 2,
                child_index: 0,
                ref expected,
                ref actual,
            }) if expected == "main" && actual == "section"
        ));
    }

    #[test]
    fn namespace_mismatch_retains_typed_step_identity() {
        let document = document("<!doctype html><html><body></body></html>");
        let mut root_step = step(1, "html");
        root_step.expected_namespace = CssHostNamespace::Svg;
        assert!(matches!(
            resolve_target(&document, &address(vec![root_step])),
            Err(CssTargetResolutionFailure::NamespaceMismatch {
                depth: 0,
                child_index: 1,
                expected: CssHostNamespace::Svg,
                actual: html::ElementNamespace::Html,
            })
        ));
    }

    #[test]
    fn missing_child_retains_typed_depth_and_ordinary_child_index() {
        let document = document("<!doctype html><html><head></head><body></body></html>");
        let address = address(vec![step(1, "html"), step(1, "body"), step(0, "article")]);
        assert!(matches!(
            resolve_target(&document, &address),
            Err(CssTargetResolutionFailure::ChildMissing {
                depth: 2,
                child_index: 0,
            })
        ));
    }

    #[test]
    fn target_address_grammar_rejects_empty_and_invalid_steps() {
        assert!(CssTargetLabel::parse("x".repeat(CSS_NESTED_MAX_TARGET_LABEL_BYTES)).is_ok());
        assert!(CssTargetLabel::parse("x".repeat(CSS_NESTED_MAX_TARGET_LABEL_BYTES + 1)).is_err());
        assert!(address(Vec::new()).validate().is_err());
        assert!(address(vec![step(0, "div")]).validate().is_ok());
        assert!(address(vec![step(0, "bad name")]).validate().is_err());
        let maximum = html::HtmlTokenizerLimits::default().max_tag_name_bytes;
        assert!(
            address(vec![step(0, &"x".repeat(maximum))])
                .validate()
                .is_ok()
        );
        assert!(
            address(vec![step(0, &"x".repeat(maximum + 1))])
                .validate()
                .is_err()
        );
    }
}
