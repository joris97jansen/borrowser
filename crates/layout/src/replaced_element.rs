use html::Node;

use crate::replaced::intrinsic::IntrinsicSize;

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
    fn intrinsic_for_img(&self, node: &html::Node) -> Option<IntrinsicSize>;
}

/// Classify replaced elements for layout purposes.
pub(crate) fn classify_replaced_kind(node: &Node) -> Option<ReplacedKind> {
    match node {
        Node::Element { element } => {
            let name = element.name();
            let attributes = element.attributes();
            if name.eq_ignore_ascii_case("img") {
                return Some(ReplacedKind::Img);
            }

            if name.eq_ignore_ascii_case("input") {
                // Phase 1: basic <input> replaced controls.
                let mut ty: Option<&str> = None;
                for (k, v) in attributes {
                    if k.eq_ignore_ascii_case("type") {
                        ty = v.as_deref().map(str::trim).filter(|s| !s.is_empty());
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
