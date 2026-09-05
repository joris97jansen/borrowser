use super::*;
use html::conformance::ObservedTemplateContents;
use writer::{Site, checked_length};
type Node = ObservedTreeNode;
fn doc(children: Vec<Node>) -> ObservedTree {
    ObservedTree {
        roots: vec![Node::Document { children }],
    }
}
fn text(data: &str) -> Node {
    Node::Text { data: data.into() }
}
fn el(
    ns: ElementNamespace,
    name: &str,
    attributes: Vec<ObservedDomAttribute>,
    children: Vec<Node>,
) -> Node {
    Node::Element {
        namespace: ns,
        local_name: name.into(),
        attributes,
        children,
    }
}
fn attr(
    ns: AttributeNamespace,
    prefix: Option<&str>,
    name: &str,
    value: &str,
) -> ObservedDomAttribute {
    ObservedDomAttribute {
        namespace: ns,
        prefix: prefix.map(str::to_owned),
        local_name: name.into(),
        value: value.into(),
    }
}
fn template(ordinary_children: Vec<Node>, children: Vec<Node>) -> Node {
    Node::HtmlTemplateElement {
        attributes: vec![],
        ordinary_children,
        contents: ObservedTemplateContents { children },
    }
}
fn golden(name: &str, tree: ObservedTree) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/contract-vectors/web-observable-dom-tree-v1");
    let expected = std::fs::read(root.join(name)).unwrap();
    assert_eq!(serialize(&tree).unwrap().bytes(), expected);
}
#[test]
fn web_observable_neutral_vectors() {
    golden(
        "nodes.txt",
        doc(vec![
            Node::DocumentType {
                name: Some("html".into()),
                public_id: None,
                system_id: None,
            },
            Node::DocumentType {
                name: Some("html".into()),
                public_id: Some("public".into()),
                system_id: Some("system".into()),
            },
            text("text"),
            Node::Comment {
                data: "comment".into(),
            },
            Node::ProcessingInstruction {
                target: "target".into(),
                data: "data".into(),
            },
        ]),
    );
    golden(
        "namespaces-attributes.txt",
        doc(vec![
            el(ElementNamespace::Html, "div", vec![], vec![]),
            el(
                ElementNamespace::Svg,
                "foreignObject",
                vec![
                    attr(AttributeNamespace::Xml, Some("xml"), "space", "preserve"),
                    attr(
                        AttributeNamespace::Xmlns,
                        None,
                        "xmlns",
                        "http://www.w3.org/2000/svg",
                    ),
                    attr(AttributeNamespace::Xml, Some("xml"), "lang", "en"),
                    attr(AttributeNamespace::None, None, "id", "plain"),
                    attr(
                        AttributeNamespace::Xmlns,
                        Some("xmlns"),
                        "xlink",
                        "http://www.w3.org/1999/xlink",
                    ),
                    attr(AttributeNamespace::XLink, Some("xlink"), "href", "#target"),
                ],
                vec![],
            ),
            el(ElementNamespace::MathMl, "mi", vec![], vec![]),
        ]),
    );
    golden(
        "templates.txt",
        doc(vec![template(
            vec![text("ordinary")],
            vec![
                Node::Comment {
                    data: "content".into(),
                },
                template(vec![], vec![text("nested")]),
            ],
        )]),
    );
    golden(
        "escaping-utf8-ordering.txt",
        doc(vec![
            text("\0\u{b}\u{1f}\u{7f}\r\n\t\"\\é😀\u{2028}\u{2029}"),
            el(
                ElementNamespace::Html,
                "div",
                vec![
                    attr(AttributeNamespace::None, None, "\u{10000}", "second"),
                    attr(AttributeNamespace::None, None, "\u{e000}", "first"),
                ],
                vec![],
            ),
        ]),
    );
    golden("static-document-different.txt", doc(vec![]));
    golden(
        "static-document.txt",
        doc(vec![
            Node::DocumentType {
                name: Some("html".into()),
                public_id: None,
                system_id: None,
            },
            el(
                ElementNamespace::Html,
                "html",
                vec![],
                vec![
                    el(ElementNamespace::Html, "head", vec![], vec![]),
                    el(
                        ElementNamespace::Html,
                        "body",
                        vec![],
                        vec![
                            text("lead"),
                            el(ElementNamespace::Html, "p", vec![], vec![text("ok")]),
                            text("\n"),
                        ],
                    ),
                ],
            ),
        ]),
    );
}
#[test]
fn web_observable_rejects_structural_and_attribute_states() {
    let cases = [
        ObservedTree::default(),
        ObservedTree {
            roots: vec![text("x")],
        },
        doc(vec![Node::Document { children: vec![] }]),
        doc(vec![Node::DocumentType {
            name: None,
            public_id: None,
            system_id: None,
        }]),
        doc(vec![el(ElementNamespace::Html, "template", vec![], vec![])]),
    ];
    for tree in cases {
        assert_eq!(serialize(&tree), Err(Error::InvalidStructure));
    }
    for (ns, prefix, name) in [
        (AttributeNamespace::None, Some("x"), "a"),
        (AttributeNamespace::Xml, None, "lang"),
        (AttributeNamespace::XLink, Some("xml"), "href"),
        (AttributeNamespace::Xmlns, None, "x"),
    ] {
        assert_eq!(
            serialize(&doc(vec![el(
                ElementNamespace::Svg,
                "svg",
                vec![attr(ns, prefix, name, "")],
                vec![]
            )])),
            Err(Error::InvalidAttribute)
        );
    }
    let a = attr(AttributeNamespace::None, None, "id", "one");
    assert_eq!(
        serialize(&doc(vec![el(
            ElementNamespace::Html,
            "p",
            vec![a.clone(), a],
            vec![]
        )])),
        Err(Error::DuplicateAttribute)
    );
}
struct Reject(Site);
impl Allocation for Reject {
    fn reserve<T>(&mut self, v: &mut Vec<T>, n: usize, site: Site) -> Result<(), Error> {
        if site == self.0 {
            Err(Error::Allocation)
        } else {
            v.try_reserve(n).map_err(|_| Error::Allocation)
        }
    }
}
#[test]
fn web_observable_exact_limit_and_failure_atomicity() {
    let overhead = serialize(&doc(vec![text("")])).unwrap().bytes().len();
    let n = MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1 as usize - overhead;
    let tree = doc(vec![text(&"a".repeat(n))]);
    assert_eq!(serialize(&tree).unwrap().bytes().len(), 8_388_608);
    assert_eq!(
        serialize(&doc(vec![text(&"a".repeat(n + 1))])),
        Err(Error::TooLarge)
    );
    assert_eq!(
        serialize(&doc(vec![text(&"\0".repeat(n / 6 + 1))])),
        Err(Error::TooLarge)
    );
    assert_eq!(
        checked_length(usize::MAX, 1, usize::MAX),
        Err(Error::Overflow)
    );
    let tree = doc(vec![el(
        ElementNamespace::Html,
        "div",
        vec![attr(AttributeNamespace::None, None, "id", "x")],
        vec![],
    )]);
    for site in [Site::Output, Site::Traversal, Site::Attributes] {
        assert_eq!(
            serialize_with(&tree, &mut Reject(site)),
            Err(Error::Allocation)
        );
    }
}
