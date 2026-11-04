//! Ephemeral authored CSS addressing. No requests, retained inventory, or style
//! participation decisions. Source provenance survives independently of outcomes.
use super::PageState;
use crate::document_style::StylesheetResourceBase;
use addressing::{
    BrowserUrl, ResourceAddress, ResourceKind, ResourceOperation, UrlDiagnostic, UrlFailure,
    UrlParseOutcome,
};
use css::resource_references::{
    self as css_refs, CssReferenceDisposition as Shape, CssResourceDiscovery, CssResourceReference,
    CssResourceRole,
};
use css::{ParseOptions, StylesheetCollectionInputBuildError, StylesheetSourceId};
use html::{Node, internal::Id};
use std::{collections::HashMap, fmt::Write};

pub enum CssSource {
    Stylesheet {
        slot: core_types::StylesheetSlotId,
        id: StylesheetSourceId,
    },
    Attribute,
}
pub enum CssBase<'a> {
    Document(Option<&'a BrowserUrl>),
    Response(Option<&'a Result<UrlParseOutcome, UrlFailure>>),
}
impl CssBase<'_> {
    fn url(&self) -> Option<&BrowserUrl> {
        match self {
            Self::Document(base) => *base,
            Self::Response(Some(Ok(parsed))) => Some(&parsed.url),
            _ => None,
        }
    }
}
pub enum CssReferenceOutcome {
    IncompleteSource,
    BlockedContext,
    InvalidCss,
    Empty,
    LocalFragment,
    UnresolvedBase,
    Resolved {
        address: ResourceAddress,
        diagnostics: Vec<UrlDiagnostic>,
    },
    Failed(UrlFailure),
}
impl std::fmt::Debug for CssReferenceOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IncompleteSource => f.write_str("IncompleteSource"),
            Self::BlockedContext => f.write_str("BlockedContext"),
            Self::InvalidCss => f.write_str("InvalidCss"),
            Self::Empty => f.write_str("Empty"),
            Self::LocalFragment => f.write_str("LocalFragment"),
            Self::UnresolvedBase => f.write_str("UnresolvedBase"),
            Self::Resolved {
                address,
                diagnostics,
            } => f
                .debug_struct("Resolved")
                .field("address", address)
                .field("diagnostics", diagnostics)
                .finish(),
            Self::Failed(failure) => f.debug_tuple("Failed").field(failure).finish(),
        }
    }
}

pub struct CssSourceReferences<'a> {
    pub(crate) owner: Id,
    pub(crate) source: CssSource,
    pub(crate) base: CssBase<'a>,
    pub(crate) condition_deferred: bool,
    // Keeps source-bound spans alive, including freshly parsed attributes.
    pub(crate) discovery: CssResourceDiscovery<'a>,
}
impl CssSourceReferences<'_> {
    pub fn outcome(&self, reference: &CssResourceReference) -> CssReferenceOutcome {
        if self.discovery.stats.hit_limit {
            return CssReferenceOutcome::IncompleteSource;
        }
        if !reference.contexts.is_empty() {
            return CssReferenceOutcome::BlockedContext;
        }
        let value = match &reference.disposition {
            Shape::Empty => return CssReferenceOutcome::Empty,
            Shape::LocalFragment(_) => return CssReferenceOutcome::LocalFragment,
            Shape::InvalidCss => return CssReferenceOutcome::InvalidCss,
            Shape::Literal(value) => value.value(),
        };
        let result = match self.base.url() {
            Some(base) => base.resolve(value).map(Some),
            None => BrowserUrl::parse_if_base_independent(value),
        };
        match result {
            Ok(Some(parsed)) => CssReferenceOutcome::Resolved {
                address: ResourceAddress::new(
                    parsed.url,
                    match reference.role {
                        CssResourceRole::Image
                        | CssResourceRole::Mask
                        | CssResourceRole::Cursor => ResourceKind::Image,
                        CssResourceRole::Font => ResourceKind::Font,
                        CssResourceRole::StylesheetImport => ResourceKind::Stylesheet,
                        CssResourceRole::Unknown => ResourceKind::Unknown,
                    },
                ),
                diagnostics: parsed.diagnostics,
            },
            Ok(None) => CssReferenceOutcome::UnresolvedBase,
            Err(failure) => CssReferenceOutcome::Failed(failure),
        }
    }
}
impl PageState {
    pub(crate) fn css_resource_references(
        &self,
    ) -> Result<Option<Vec<CssSourceReferences<'_>>>, StylesheetCollectionInputBuildError> {
        let Some(dom) = self.dom.as_deref() else {
            return Ok(None);
        };
        let styles = &self.rendering.document_styles;
        let mut by_owner: HashMap<_, _> = styles.resource_sources().map(|s| (s.owner, s)).collect();
        let mut sources = Vec::new();
        let mut stack = vec![dom];
        while let Some(node) = stack.pop() {
            match node {
                Node::Document { children, .. } => stack.extend(children.iter().rev()),
                Node::Element { element } => {
                    stack.extend(element.children().iter().rev());
                    let attributes = element.attributes().iter().map(|a| {
                        css::SelectorDomAttribute::new(
                            a.name().namespace(),
                            a.name().local_name(),
                            a.value(),
                        )
                    });
                    if let Some(text) = css::get_inline_style(element.namespace(), attributes) {
                        sources.push(CssSourceReferences {
                            owner: node.id(),
                            source: CssSource::Attribute,
                            base: CssBase::Document(self.active_base_url()),
                            condition_deferred: false,
                            discovery: css_refs::discover_style_attribute_resource_references(
                                text,
                                &ParseOptions::style_attribute(),
                            ),
                        });
                    }
                    if let Some(sheet) = by_owner.remove(&node.id()) {
                        let id = StylesheetSourceId::from_browser_slot(sheet.slot.0)?;
                        let condition_deferred =
                            css::StylesheetConditionInput::from_optional_raw_media(sheet.media)
                                .is_deferred();
                        sources.push(CssSourceReferences {
                            owner: sheet.owner,
                            source: CssSource::Stylesheet {
                                slot: sheet.slot,
                                id,
                            },
                            base: match sheet.base {
                                StylesheetResourceBase::Document => {
                                    CssBase::Document(self.active_base_url())
                                }
                                StylesheetResourceBase::Response(base) => CssBase::Response(base),
                            },
                            condition_deferred,
                            discovery: css_refs::discover_stylesheet_resource_references(
                                sheet.parse,
                                &ParseOptions::stylesheet(),
                            ),
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(Some(sources))
    }
    pub(crate) fn css_resource_address_debug_snapshot(&self) -> Option<String> {
        let sources = match self.css_resource_references() {
            Ok(value) => value?,
            Err(error) => {
                return Some(format!(
                    "CssResourceReferences {{ input_error: {:?} }}",
                    error.stable_label()
                ));
            }
        };
        let mut output = String::from("CssResourceReferences {\n");
        let mut owners = HashMap::new();
        let mut sheets = HashMap::new();
        for source in sources {
            let next = owners.len();
            let owner = *owners.entry(source.owner).or_insert(next);
            let kind = match source.source {
                CssSource::Attribute => "StyleAttribute".to_owned(),
                CssSource::Stylesheet { slot, id } => {
                    let next = sheets.len();
                    format!("Stylesheet[{}]", sheets.entry((slot, id)).or_insert(next))
                }
            };
            writeln!(output, "    source={kind} owner=element[{owner}] condition_deferred={} effective_participation=Unevaluated incomplete={} feature_notices_truncated={}", source.condition_deferred, source.discovery.stats.hit_limit, source.discovery.feature_notices_truncated).unwrap();
            match &source.base {
                CssBase::Document(base) => {
                    writeln!(output, "        base=DocumentActive({base:?})").unwrap()
                }
                CssBase::Response(base) => {
                    writeln!(output, "        base=AdmittedResponse({base:?})").unwrap()
                }
            }
            for diagnostic in &source.discovery.diagnostics {
                writeln!(
                    output,
                    "        syntax={:?} {:?}",
                    diagnostic.severity, diagnostic.kind
                )
                .unwrap();
            }
            for (_, feature) in &source.discovery.feature_notices {
                writeln!(output, "        feature={feature:?}").unwrap();
            }
            for (index, reference) in source.discovery.references.iter().enumerate() {
                let outcome = source.outcome(reference);
                writeln!(output, "        [{index}] role={:?} shape={} contexts={:?} acceptance={:?} selector={:?} outcome={outcome:?}", reference.role, reference.disposition.label(), reference.contexts, reference.declaration_applicability, reference.selector_matchability).unwrap();
                if let CssReferenceOutcome::Resolved { address, .. } = outcome {
                    writeln!(output, "            retrieval_scheme_capability={:?} navigation_scheme_capability={:?}", address.capability(ResourceOperation::Retrieval), address.capability(ResourceOperation::Navigation)).unwrap();
                }
            }
        }
        output.push('}');
        Some(output)
    }
}
