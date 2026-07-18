use html::internal::{DocumentFragmentNode, Id, ParserCreatedFragmentKind};
use html::{Node, internal};

pub fn valid_template() -> Node {
    internal::template_element_from_parts(
        Id(1),
        Vec::new(),
        Vec::new(),
        Id(2),
        vec![Node::Text {
            id: Id(3),
            text: "inert".to_string(),
        }],
        Vec::new(),
    )
}

pub fn approved_read_only_fragment(template: &Node) -> &DocumentFragmentNode {
    let fragment = internal::template_contents(template).expect("typed contents association");
    assert_eq!(internal::fragment_id(fragment), Id(2));
    assert_eq!(
        internal::fragment_kind(fragment),
        ParserCreatedFragmentKind::TemplateContents
    );
    assert_eq!(internal::fragment_children(fragment).len(), 1);
    fragment
}
