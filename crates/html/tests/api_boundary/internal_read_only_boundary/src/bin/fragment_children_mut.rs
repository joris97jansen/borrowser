use html::internal;
use html_internal_read_only_boundary_probe::{approved_read_only_fragment, valid_template};

fn main() {
    let template = valid_template();
    let fragment = approved_read_only_fragment(&template);
    assert_eq!(internal::fragment_children(fragment).len(), 1);
    fragment.children_mut().clear();
}
