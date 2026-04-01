use html::Node;

fn main() {
    #[allow(deprecated)]
    let stream = html::tokenize("<p>legacy</p>");
    #[allow(deprecated)]
    let dom = html::build_owned_dom(&stream);

    let Node::Document { children, .. } = &dom else {
        panic!("expected document root");
    };
    assert!(
        !children.is_empty(),
        "expected legacy parser to materialize document children"
    );
}
