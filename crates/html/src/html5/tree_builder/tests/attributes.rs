use super::helpers::EmptyResolver;
use crate::html5::shared::{Attribute, AttributeValue, DocumentParseContext, Token};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig};

#[test]
fn parser_created_attributes_are_first_wins_and_encounter_ordered() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder =
        Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).expect("tree builder init");

    let div = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");
    let first = ctx
        .atoms
        .intern_ascii_folded("data-first")
        .expect("atom interning");
    let duplicated = ctx
        .atoms
        .intern_ascii_folded("DATA-DUP")
        .expect("atom interning");
    let duplicated_lower = ctx
        .atoms
        .intern_ascii_folded("data-dup")
        .expect("atom interning");
    let empty = ctx
        .atoms
        .intern_ascii_folded("data-empty")
        .expect("atom interning");
    let boolean = ctx
        .atoms
        .intern_ascii_folded("disabled")
        .expect("atom interning");

    assert_eq!(
        duplicated, duplicated_lower,
        "attribute name atomization should normalize duplicate-name identity"
    );

    let _ = builder
        .process(
            &Token::StartTag {
                name: div,
                attrs: vec![
                    Attribute {
                        name: first,
                        value: AttributeValue::Owned("1".to_string()),
                    },
                    Attribute {
                        name: duplicated,
                        value: AttributeValue::Owned("keep".to_string()),
                    },
                    Attribute {
                        name: empty,
                        value: AttributeValue::Owned(String::new()),
                    },
                    Attribute {
                        name: duplicated_lower,
                        value: AttributeValue::Owned("drop".to_string()),
                    },
                    Attribute {
                        name: boolean,
                        value: AttributeValue::Owned(String::new()),
                    },
                ],
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("start tag should process");
    let _ = builder
        .process(&Token::Eof, &ctx.atoms, &resolver)
        .expect("EOF should process");

    let patches = builder.drain_patches();
    let dom = crate::test_harness::materialize_patch_batches(&[patches]).expect("materialize DOM");
    let div = find_element(&dom, "div").expect("div element should materialize");
    let crate::Node::Element { element } = div else {
        panic!("expected div element");
    };

    let actual = element
        .attributes()
        .iter()
        .map(|attribute| (attribute.local_name(), attribute.value()))
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            ("data-first", "1"),
            ("data-dup", "keep"),
            ("data-empty", ""),
            ("disabled", ""),
        ],
        "stored parser-created attributes should be first-wins and encounter ordered"
    );
}

fn find_element<'a>(node: &'a crate::Node, name: &str) -> Option<&'a crate::Node> {
    match node {
        crate::Node::Element { element } if element.expanded_name().is_html(name) => Some(node),
        crate::Node::Document { children, .. } => {
            children.iter().find_map(|child| find_element(child, name))
        }
        crate::Node::Element { element } => element
            .children()
            .iter()
            .find_map(|child| find_element(child, name)),
        crate::Node::DocumentType { .. }
        | crate::Node::Text { .. }
        | crate::Node::Comment { .. } => None,
    }
}
