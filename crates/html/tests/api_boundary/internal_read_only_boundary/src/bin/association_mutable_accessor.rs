use html::internal;
use html_internal_read_only_boundary_probe::{approved_read_only_fragment, valid_template};

fn main() {
    let mut template = valid_template();
    let _ = approved_read_only_fragment(&template);
    let _association = internal::template_contents_mut(&mut template);
}
