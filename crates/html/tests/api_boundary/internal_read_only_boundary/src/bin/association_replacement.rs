use html::{Node, internal};
use html_internal_read_only_boundary_probe::{approved_read_only_fragment, valid_template};

fn main() {
    let mut template = valid_template();
    let _ = approved_read_only_fragment(&template);
    let Node::Element { element } = &mut template else {
        unreachable!("controlled constructor returns an element")
    };
    let _detached = std::mem::replace(&mut element.template_contents, None);
    let _ = internal::template_contents(&template);
}
