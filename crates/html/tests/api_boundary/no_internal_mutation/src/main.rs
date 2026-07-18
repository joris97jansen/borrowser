use html::{
    DocumentFragmentNode, HtmlParseOptions, Node, ParserCreatedFragmentKind, parse_document,
};

fn main() {
    let _unavailable_fragment_type: Option<DocumentFragmentNode> = None;
    let _unavailable_fragment_kind = ParserCreatedFragmentKind::TemplateContents;
    let _unavailable_constructor = html::internal::template_element_from_parts;

    let mut output = parse_document(
        "<template><span>x</span></template>",
        HtmlParseOptions::default(),
    )
    .expect("probe input parses");

    if let Node::Element {
        template_contents, ..
    } = &mut output.document
    {
        if let Some(contents) = template_contents.as_deref_mut() {
            let id = contents.id();
            contents.set_id(id);
            contents.children_mut().clear();
        }
        let _detached = template_contents.take();
    }
}
