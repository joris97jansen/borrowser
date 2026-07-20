use html::Node;

use crate::replaced::intrinsic::IntrinsicSize;

/// Layout-owned presentation data for an HTML image replaced element.
///
/// The source is resolved by Browser-owned resource integration while Layout
/// builds this handoff. Paint/GFX does not inspect DOM names or attributes, or
/// perform base-URL resolution, to recover this information.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImagePresentation {
    resolved_source: Option<String>,
    alternative_text: Option<String>,
}

impl ImagePresentation {
    pub fn resolved_source(&self) -> Option<&str> {
        self.resolved_source.as_deref()
    }

    pub fn alternative_text(&self) -> Option<&str> {
        self.alternative_text.as_deref()
    }
}

/// Layout-owned generated presentation data for an HTML text control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextControlPresentation {
    placeholder: Option<String>,
}

impl TextControlPresentation {
    pub fn placeholder(&self) -> Option<&str> {
        self.placeholder.as_deref()
    }
}

/// Typed DOM-to-layout presentation handoff for supported replaced elements.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplacedElementPresentation {
    Image(ImagePresentation),
    TextControl(TextControlPresentation),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplacedKind {
    Img,
    InputText,
    TextArea,
    InputCheckbox,
    InputRadio,
    Button,
}

/// Optional, host-provided info for replaced elements (e.g. decoded image sizes).
pub trait ReplacedElementInfoProvider {
    /// Resolve the exact stored HTML `src` value through the browser-owned
    /// document/resource context before it becomes paint-facing layout data.
    /// URL-specific preprocessing belongs to that Browser integration.
    fn resolve_image_source(&self, source: &str) -> Option<String>;

    fn intrinsic_for_img(&self, image: &ImagePresentation) -> Option<IntrinsicSize>;
}

/// Resolve the supported HTML attribute semantics needed by generated layout
/// and paint-facing metadata. Foreign lookalikes never reach this handoff.
pub(crate) fn replaced_element_presentation(
    node: &Node,
    kind: ReplacedKind,
    replaced_info: Option<&dyn ReplacedElementInfoProvider>,
) -> Option<ReplacedElementPresentation> {
    if !matches!(
        node,
        Node::Element { element } if element.namespace() == html::ElementNamespace::Html
    ) {
        return None;
    }
    match kind {
        ReplacedKind::Img => {
            let resolved_source = exact_html_attribute(node, "src").and_then(|source| {
                replaced_info.and_then(|provider| provider.resolve_image_source(source))
            });
            Some(ReplacedElementPresentation::Image(ImagePresentation {
                resolved_source,
                alternative_text: exact_html_attribute(node, "alt").map(str::to_owned),
            }))
        }
        ReplacedKind::InputText | ReplacedKind::TextArea => Some(
            ReplacedElementPresentation::TextControl(TextControlPresentation {
                placeholder: exact_html_attribute(node, "placeholder").map(str::to_owned),
            }),
        ),
        ReplacedKind::InputCheckbox | ReplacedKind::InputRadio | ReplacedKind::Button => None,
    }
}

fn exact_html_attribute<'a>(node: &'a Node, name: &str) -> Option<&'a str> {
    let Node::Element { element } = node else {
        return None;
    };
    if element.namespace() != html::ElementNamespace::Html {
        return None;
    }
    element
        .attributes()
        .iter()
        .find(|attribute| {
            attribute.namespace() == html::AttributeNamespace::None
                && attribute.local_name().eq_ignore_ascii_case(name)
        })
        .map(html::ParserCreatedAttribute::value)
}

/// Classify replaced elements for layout purposes.
pub(crate) fn classify_replaced_kind(node: &Node) -> Option<ReplacedKind> {
    match node {
        Node::Element { element } => {
            if element.namespace() != html::ElementNamespace::Html {
                return None;
            }
            let name = element.name();
            let attributes = element.attributes();
            if name.eq_ignore_ascii_case("img") {
                return Some(ReplacedKind::Img);
            }

            if name.eq_ignore_ascii_case("input") {
                // Phase 1: basic <input> replaced controls.
                let mut ty: Option<&str> = None;
                for attribute in attributes {
                    if attribute.namespace() == html::AttributeNamespace::None
                        && attribute.local_name().eq_ignore_ascii_case("type")
                    {
                        ty = Some(str::trim(attribute.value())).filter(|s| !s.is_empty());
                        break;
                    }
                }

                match ty {
                    None => return Some(ReplacedKind::InputText),
                    Some(t) if t.eq_ignore_ascii_case("text") => {
                        return Some(ReplacedKind::InputText);
                    }
                    Some(t) if t.eq_ignore_ascii_case("checkbox") => {
                        return Some(ReplacedKind::InputCheckbox);
                    }
                    Some(t) if t.eq_ignore_ascii_case("radio") => {
                        return Some(ReplacedKind::InputRadio);
                    }
                    _ => {}
                }
            }

            if name.eq_ignore_ascii_case("textarea") {
                return Some(ReplacedKind::TextArea);
            }

            if name.eq_ignore_ascii_case("button") {
                return Some(ReplacedKind::Button);
            }

            None
        }
        _ => None,
    }
}
