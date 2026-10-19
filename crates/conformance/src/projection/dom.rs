use super::{HarnessError, error};
use html::{AttributeNamespace, ElementNamespace, ElementNode, Node};
use serde::{Deserialize, Serialize};
const HTML: &str = "http://www.w3.org/1999/xhtml";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DomNode {
    Document {
        children: Vec<DomNode>,
    },
    Doctype {
        name: String,
        public_id: String,
        system_id: String,
    },
    Element {
        namespace: String,
        name: String,
        attributes: Vec<Attribute>,
        children: Vec<DomNode>,
        template: Option<Box<DomNode>>,
    },
    Text {
        value: String,
    },
    Comment {
        value: String,
    },
    Fragment {
        children: Vec<DomNode>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Attribute {
    pub(crate) namespace: String,
    pub(crate) name: String,
    pub(crate) prefix: String,
    pub(crate) value: String,
}
#[derive(Default)]
struct Budget {
    nodes: usize,
    bytes: usize,
}
impl Budget {
    fn node(&mut self, depth: usize) -> Result<(), HarnessError> {
        self.nodes += 1;
        if self.nodes > 128 || depth > 32 {
            Err(error("DOM tree limit"))
        } else {
            Ok(())
        }
    }
    fn text(&mut self, s: &str) -> Result<String, HarnessError> {
        self.bytes += s.len();
        if self.bytes > 8192 {
            Err(error("DOM string limit"))
        } else {
            Ok(s.into())
        }
    }
}
pub(crate) fn borrowser(node: &Node) -> Result<DomNode, HarnessError> {
    fn walk(n: &Node, depth: usize, b: &mut Budget) -> Result<DomNode, HarnessError> {
        b.node(depth)?;
        let mut children = |nodes: &[Node]| {
            nodes
                .iter()
                .map(|n| walk(n, depth + 1, b))
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(match n {
            Node::Document { children: c, .. } => DomNode::Document {
                children: children(c)?,
            },
            Node::DocumentType {
                name,
                public_id,
                system_id,
                ..
            } => DomNode::Doctype {
                name: b.text(name.as_deref().unwrap_or(""))?,
                public_id: b.text(public_id.as_deref().unwrap_or(""))?,
                system_id: b.text(system_id.as_deref().unwrap_or(""))?,
            },
            Node::Text { text, .. } => DomNode::Text {
                value: b.text(text)?,
            },
            Node::Comment { text, .. } => DomNode::Comment {
                value: b.text(text)?,
            },
            Node::Element { element: e } => {
                if e.attributes().len() > 32 {
                    return Err(error("DOM attribute limit"));
                }
                let namespace = b.text(match e.namespace() {
                    ElementNamespace::Html => HTML,
                    ElementNamespace::Svg => "http://www.w3.org/2000/svg",
                    ElementNamespace::MathMl => "http://www.w3.org/1998/Math/MathML",
                })?;
                let name = b.text(e.name())?;
                let mut attributes = e
                    .attributes()
                    .iter()
                    .map(|a| {
                        Ok(Attribute {
                            namespace: b.text(match a.namespace() {
                                AttributeNamespace::None => "",
                                AttributeNamespace::Xml => "http://www.w3.org/XML/1998/namespace",
                                AttributeNamespace::Xmlns => "http://www.w3.org/2000/xmlns/",
                                AttributeNamespace::XLink => "http://www.w3.org/1999/xlink",
                            })?,
                            name: b.text(a.local_name())?,
                            prefix: b.text(a.prefix().unwrap_or(""))?,
                            value: b.text(a.value())?,
                        })
                    })
                    .collect::<Result<Vec<_>, HarnessError>>()?;
                attributes.sort(); // Comparison only; production order is untouched.
                let children = e
                    .children()
                    .iter()
                    .map(|n| walk(n, depth + 1, b))
                    .collect::<Result<_, _>>()?;
                let template = html::internal::template_contents(n)
                    .map(|fragment| {
                        b.node(depth + 1)?;
                        let children = html::internal::fragment_children(fragment)
                            .iter()
                            .map(|n| walk(n, depth + 2, b))
                            .collect::<Result<_, _>>()?;
                        Ok::<_, HarnessError>(Box::new(DomNode::Fragment { children }))
                    })
                    .transpose()?;
                DomNode::Element {
                    namespace,
                    name,
                    attributes,
                    children,
                    template,
                }
            }
            Node::ProcessingInstruction { .. } => {
                return Err(error(
                    "processing instructions outside selected HTML profile",
                ));
            }
        })
    }
    walk(node, 0, &mut Budget::default())
}
pub(crate) fn target<'a>(
    root: &'a Node,
    id: &str,
) -> Result<Option<&'a ElementNode>, HarnessError> {
    fn walk<'a>(
        n: &'a Node,
        id: &str,
        result: &mut Option<&'a ElementNode>,
        count: &mut usize,
        depth: usize,
    ) -> Result<(), HarnessError> {
        *count += 1;
        if *count > 128 || depth > 32 {
            return Err(error("target lookup tree limit"));
        }
        if let Node::Element { element } = n
            && element.attributes().iter().any(|a| {
                a.namespace() == AttributeNamespace::None
                    && a.local_name() == "id"
                    && a.value() == id
            })
        {
            if element.namespace() != ElementNamespace::Html {
                return Err(error("target namespace outside HTML profile"));
            }
            if result.replace(element).is_some() {
                return Err(error(format!("duplicate target #{id}")));
            }
        }
        for child in n.children().unwrap_or(&[]) {
            walk(child, id, result, count, depth + 1)?;
        }
        Ok(())
    }
    let mut result = None;
    walk(root, id, &mut result, &mut 0, 0)?;
    Ok(result)
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn chromium(
    node: &crate::chromium::inspection::NativeNode,
) -> Result<DomNode, HarnessError> {
    use crate::chromium::inspection::NativeNode;
    fn walk(n: &NativeNode, depth: usize, b: &mut Budget) -> Result<DomNode, HarnessError> {
        b.node(depth)?;
        let children = n
            .children
            .iter()
            .map(|n| walk(n, depth + 1, b))
            .collect::<Result<Vec<_>, _>>()?;
        // Reject contradictory or omitted required fields rather than ignoring
        // extra facts which would otherwise disappear in the common projection.
        if n.node_type != 10 && (!n.public_id.is_empty() || !n.system_id.is_empty()) {
            return Err(error("unexpected doctype fields"));
        }
        if ![1, 10].contains(&n.node_type) && !n.name.is_empty() {
            return Err(error("unexpected native node name"));
        }
        if ![3, 8].contains(&n.node_type) && !n.value.is_empty() {
            return Err(error("unexpected native character data"));
        }
        if n.children
            .iter()
            .any(|c| c.node_type == 9 || c.node_type == 11)
        {
            return Err(error("invalid native ordinary child kind"));
        }
        if n.template.as_ref().is_some_and(|t| t.node_type != 11) {
            return Err(error("invalid native template fragment"));
        }
        if n.node_type != 1
            && (!n.attributes.is_empty() || n.template.is_some() || !n.namespace.is_empty())
        {
            return Err(error("invalid native node fields"));
        }
        if ![1, 9, 11].contains(&n.node_type) && !children.is_empty() {
            return Err(error("native leaf has children"));
        }
        Ok(match n.node_type {
            9 => DomNode::Document { children },
            11 => DomNode::Fragment { children },
            10 => DomNode::Doctype {
                name: b.text(&n.name)?,
                public_id: b.text(&n.public_id)?,
                system_id: b.text(&n.system_id)?,
            },
            3 => DomNode::Text {
                value: b.text(&n.value)?,
            },
            8 => DomNode::Comment {
                value: b.text(&n.value)?,
            },
            1 => {
                if n.name.is_empty()
                    || ![
                        HTML,
                        "http://www.w3.org/2000/svg",
                        "http://www.w3.org/1998/Math/MathML",
                    ]
                    .contains(&n.namespace.as_str())
                {
                    return Err(error("unsupported native expanded element name"));
                }
                if n.attributes.len() > 32 {
                    return Err(error("native attribute limit"));
                }
                if (n.namespace == HTML && n.name == "template") != n.template.is_some() {
                    return Err(error("missing or unexpected template content"));
                }
                let mut attributes = n
                    .attributes
                    .iter()
                    .map(|a| {
                        Ok(Attribute {
                            namespace: b.text(&a.namespace)?,
                            name: b.text(&a.name)?,
                            prefix: b.text(&a.prefix)?,
                            value: b.text(&a.value)?,
                        })
                    })
                    .collect::<Result<Vec<_>, HarnessError>>()?;
                attributes.sort();
                if attributes
                    .windows(2)
                    .any(|a| a[0].namespace == a[1].namespace && a[0].name == a[1].name)
                {
                    return Err(error("duplicate native attribute"));
                }
                DomNode::Element {
                    namespace: b.text(&n.namespace)?,
                    name: b.text(&n.name)?,
                    attributes,
                    children,
                    template: n
                        .template
                        .as_ref()
                        .map(|n| walk(n, depth + 1, b).map(Box::new))
                        .transpose()?,
                }
            }
            _ => return Err(error("unsupported native node kind")),
        })
    }
    if node.node_type != 9 {
        return Err(error("missing native document root"));
    }
    walk(node, 0, &mut Budget::default())
}
