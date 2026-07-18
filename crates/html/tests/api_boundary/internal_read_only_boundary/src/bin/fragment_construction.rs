use html::Node;
use html::internal::{self, DocumentFragmentNode, Id, ParserCreatedFragmentKind};
use html_internal_read_only_boundary_probe::{approved_read_only_fragment, valid_template};

fn main() {
    let template = valid_template();
    let fragment = approved_read_only_fragment(&template);
    assert_eq!(internal::fragment_id(fragment), Id(2));

    let _detached = DocumentFragmentNode {
        id: Id(9),
        kind: ParserCreatedFragmentKind::TemplateContents,
        children: Vec::<Node>::new(),
    };
}
