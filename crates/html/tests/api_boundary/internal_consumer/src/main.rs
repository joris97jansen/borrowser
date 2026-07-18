use html::internal::{Id, ParserCreatedFragmentKind};
use html::{Node, internal};

fn main() {
    let template = internal::template_element_from_parts(
        Id(1),
        Vec::new(),
        Vec::new(),
        Id(2),
        vec![Node::Text {
            id: Id(3),
            text: "inert".to_string(),
        }],
        Vec::new(),
    );

    let host = template.element().expect("template host");
    assert_eq!(host.name().as_ref(), "template");
    assert!(host.children().is_empty());
    let contents = internal::template_contents(&template).expect("typed contents association");
    assert_eq!(internal::fragment_id(contents), Id(2));
    assert_eq!(
        internal::fragment_kind(contents),
        ParserCreatedFragmentKind::TemplateContents
    );
    assert_eq!(internal::fragment_children(contents).len(), 1);
}
