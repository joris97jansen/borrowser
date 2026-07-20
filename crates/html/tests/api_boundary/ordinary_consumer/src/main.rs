use html::{AtomTable, ElementNamespace, ElementNode, HtmlParseOptions, Node, parse_document};

fn main() {
    let output = parse_document(
        "<div class=ordinary><span>x</span></div>",
        HtmlParseOptions::default(),
    )
    .expect("ordinary consumer input parses");

    let mut stack = vec![&output.document];
    while let Some(node) = stack.pop() {
        let _identity = node.id();
        if let Some(element) = node.element() {
            let _name = element.name();
            let _attributes = element.attributes();
            let _style = element.style();
        }
        if let Some(children) = node.children() {
            stack.extend(children.iter().rev());
        }
    }

    let mut names = AtomTable::new();
    let section = names
        .intern_ascii_folded("section")
        .expect("synthetic name interning");
    let section = names
        .expanded_name(ElementNamespace::Html, section)
        .expect("synthetic expanded name");
    let ordinary = Node::new_element(section, Vec::new(), Vec::new(), Vec::new());
    assert_eq!(
        ordinary.element().map(ElementNode::name).map(AsRef::as_ref),
        Some("section")
    );
}
