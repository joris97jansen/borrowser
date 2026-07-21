use crate::{BoxKind, ListMarker, ReplacedKind};
use css::Display;
use html::internal::Id;

use super::super::*;
use super::support::*;

#[test]
fn supported_display_values_map_to_principal_box_behavior() {
    let dom = doc(vec![element(
        2,
        "html",
        Vec::new(),
        vec![element(
            3,
            "body",
            Vec::new(),
            vec![
                element(4, "div", vec![("display", "block")], Vec::new()),
                element(5, "span", vec![("display", "inline")], Vec::new()),
                element(6, "span", vec![("display", "inline-block")], Vec::new()),
                element(7, "li", vec![("display", "list-item")], Vec::new()),
                element(8, "span", vec![("display", "none")], Vec::new()),
                element(9, "input", vec![("display", "inline-block")], Vec::new()),
                element(10, "section", vec![("display", "flex")], Vec::new()),
            ],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    assert_eq!(
        box_by_node_id(&tree, Id(1)).display_behavior(),
        DisplayBoxBehavior::DocumentRoot
    );
    assert_eq!(
        box_by_node_id(&tree, Id(2)).display_behavior(),
        DisplayBoxBehavior::DocumentElement
    );

    let block = box_by_node_id(&tree, Id(4));
    assert_eq!(block.kind(), BoxKind::Block);
    assert_eq!(block.display(), Display::Block);
    assert_eq!(block.display_behavior(), DisplayBoxBehavior::Block);

    let inline = box_by_node_id(&tree, Id(5));
    assert_eq!(inline.kind(), BoxKind::Inline);
    assert_eq!(inline.display(), Display::Inline);
    assert_eq!(inline.display_behavior(), DisplayBoxBehavior::Inline);

    let inline_block = box_by_node_id(&tree, Id(6));
    assert_eq!(inline_block.kind(), BoxKind::InlineBlock);
    assert_eq!(inline_block.display(), Display::InlineBlock);
    assert_eq!(
        inline_block.display_behavior(),
        DisplayBoxBehavior::InlineBlock
    );

    let list_item = box_by_node_id(&tree, Id(7));
    assert_eq!(list_item.kind(), BoxKind::Block);
    assert_eq!(list_item.display(), Display::ListItem);
    assert_eq!(list_item.display_behavior(), DisplayBoxBehavior::ListItem);

    assert!(
        tree.nodes()
            .iter()
            .all(|node| node.direct_node_id() != Some(Id(8)))
    );

    let input = box_by_node_id(&tree, Id(9));
    assert_eq!(input.kind(), BoxKind::ReplacedInline);
    assert_eq!(input.display(), Display::InlineBlock);
    assert_eq!(input.display_behavior(), DisplayBoxBehavior::ReplacedInline);
    assert_eq!(input.replaced(), Some(ReplacedKind::InputText));

    let flex = box_by_node_id(&tree, Id(10));
    assert_eq!(flex.kind(), BoxKind::Block);
    assert_eq!(flex.display(), Display::Flex);
    assert_eq!(flex.display_behavior(), DisplayBoxBehavior::FlexContainer);
}

#[test]
fn parser_created_template_host_and_contents_generate_no_boxes() {
    let template = html::internal::template_element_from_parts(
        Id(4),
        html::internal::html_name("template"),
        Vec::new(),
        Vec::new(),
        Id(5),
        vec![element(6, "div", Vec::new(), vec![text(7, "inert")])],
        Vec::new(),
    );

    let dom = doc_with_body(vec![
        template,
        element(8, "p", Vec::new(), vec![text(9, "active")]),
    ]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    for inert_id in [Id(4), Id(5), Id(6), Id(7)] {
        assert!(
            tree.nodes()
                .iter()
                .all(|node| node.direct_node_id() != Some(inert_id)),
            "parser-created template identity {inert_id:?} entered layout"
        );
    }
    assert_eq!(box_by_node_id(&tree, Id(8)).kind(), BoxKind::Block);
}

#[test]
fn processing_instruction_is_preserved_by_style_and_suppressed_by_box_generation() {
    let dom = doc_with_body(vec![
        text(4, "before"),
        processing_instruction(5, "Exact-Target", "data"),
        element(6, "span", Vec::new(), vec![text(7, "after")]),
    ]);
    let styled = css::build_style_tree(&dom, None);
    let style_snapshot = css::StylePhaseOutput::new(styled).to_debug_snapshot();
    assert!(
        style_snapshot
            .contains("kind=processing-instruction target=\"Exact-Target\" data=\"data\"")
    );

    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);
    assert!(
        tree.nodes()
            .iter()
            .all(|node| node.direct_node_id() != Some(Id(5))),
        "PI DOM identity must not acquire layout identity"
    );
    assert_eq!(
        box_by_node_id(&tree, Id(4)).role(),
        BoxGenerationRole::TextRun
    );
    assert_eq!(
        box_by_node_id(&tree, Id(6)).role(),
        BoxGenerationRole::OrdinaryElement
    );
    assert_eq!(
        box_by_node_id(&tree, Id(7)).role(),
        BoxGenerationRole::TextRun
    );
}

#[test]
fn unsupported_foreign_namespace_suppresses_its_complete_layout_subtree() {
    let svg = namespaced_element(
        4,
        html::ElementNamespace::Svg,
        "svg",
        Vec::new(),
        vec![
            namespaced_element(
                5,
                html::ElementNamespace::Svg,
                "foreignObject",
                Vec::new(),
                vec![element(6, "div", Vec::new(), vec![text(7, "preserved")])],
            ),
            namespaced_element(
                8,
                html::ElementNamespace::Svg,
                "circle",
                Vec::new(),
                Vec::new(),
            ),
        ],
    );
    let math = namespaced_element(
        9,
        html::ElementNamespace::MathMl,
        "math",
        Vec::new(),
        vec![namespaced_element(
            10,
            html::ElementNamespace::MathMl,
            "mi",
            Vec::new(),
            vec![text(11, "x")],
        )],
    );
    let dom = doc_with_body(vec![
        svg,
        math,
        element(12, "p", Vec::new(), vec![text(13, "after")]),
    ]);

    let styled = css::build_style_tree(&dom, None);
    assert!(
        styled
            .children
            .iter()
            .flat_map(|node| node.children.iter())
            .count()
            > 0
    );
    let tree = BoxTree::generate(&styled, None);

    for suppressed in 4..=11 {
        assert!(
            tree.nodes()
                .iter()
                .all(|node| node.direct_node_id() != Some(Id(suppressed))),
            "foreign subtree identity {suppressed} must not enter layout"
        );
    }
    assert_eq!(box_by_node_id(&tree, Id(12)).kind(), BoxKind::Block);
    assert!(
        tree.nodes()
            .iter()
            .any(|node| node.direct_node_id() == Some(Id(13)))
    );
}

#[test]
fn layout_resolves_typed_replaced_presentation_before_gfx() {
    struct PresentationProvider {
        seen_sources: std::cell::RefCell<Vec<String>>,
    }

    impl crate::ReplacedElementInfoProvider for PresentationProvider {
        fn resolve_image_source(&self, source: &str) -> Option<String> {
            self.seen_sources.borrow_mut().push(source.to_string());
            match source {
                " hero.png " => Some("https://example.test/hero.png".to_string()),
                "plain.png" => Some("https://example.test/plain.png".to_string()),
                "plain-ordinary.png" => Some("https://example.test/plain-ordinary.png".to_string()),
                "" => None,
                unexpected => panic!("unexpected exact image source {unexpected:?}"),
            }
        }

        fn intrinsic_for_img(
            &self,
            _image: &crate::ImagePresentation,
        ) -> Option<crate::replaced::intrinsic::IntrinsicSize> {
            None
        }
    }

    let provider = PresentationProvider {
        seen_sources: std::cell::RefCell::new(Vec::new()),
    };

    let dom = doc_with_body(vec![
        element_with_attributes(
            4,
            html::ElementNamespace::Html,
            "img",
            vec![("src", " hero.png "), ("alt", " Hero ")],
            Vec::new(),
            Vec::new(),
        ),
        element_with_attributes(
            5,
            html::ElementNamespace::Html,
            "img",
            vec![("src", ""), ("alt", "")],
            Vec::new(),
            Vec::new(),
        ),
        element_with_attributes(
            6,
            html::ElementNamespace::Html,
            "img",
            vec![("src", "plain.png")],
            Vec::new(),
            Vec::new(),
        ),
        element_with_attributes(
            7,
            html::ElementNamespace::Html,
            "input",
            vec![("placeholder", " Search ")],
            Vec::new(),
            Vec::new(),
        ),
        element_with_attributes(
            8,
            html::ElementNamespace::Html,
            "input",
            vec![("placeholder", "")],
            Vec::new(),
            Vec::new(),
        ),
        element_with_attributes(
            9,
            html::ElementNamespace::Html,
            "input",
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        element_with_attributes(
            10,
            html::ElementNamespace::Html,
            "textarea",
            vec![("placeholder", " Multi word ")],
            Vec::new(),
            Vec::new(),
        ),
        element_with_attributes(
            11,
            html::ElementNamespace::Html,
            "textarea",
            vec![("placeholder", "")],
            Vec::new(),
            Vec::new(),
        ),
        element_with_attributes(
            12,
            html::ElementNamespace::Html,
            "textarea",
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        element_with_attributes(
            13,
            html::ElementNamespace::Svg,
            "img",
            vec![("src", "foreign.png"), ("alt", "foreign")],
            Vec::new(),
            Vec::new(),
        ),
        element_with_attributes(
            14,
            html::ElementNamespace::MathMl,
            "input",
            vec![("placeholder", "foreign")],
            Vec::new(),
            Vec::new(),
        ),
        element_with_attributes(
            15,
            html::ElementNamespace::Svg,
            "textarea",
            vec![("placeholder", "foreign")],
            Vec::new(),
            Vec::new(),
        ),
        element_with_attributes(
            16,
            html::ElementNamespace::Html,
            "img",
            vec![("src", "plain-ordinary.png"), ("alt", "Hero")],
            Vec::new(),
            Vec::new(),
        ),
        element_with_attributes(
            17,
            html::ElementNamespace::Html,
            "input",
            vec![("placeholder", "Search")],
            Vec::new(),
            Vec::new(),
        ),
        element_with_attributes(
            18,
            html::ElementNamespace::Html,
            "textarea",
            vec![("placeholder", "Multi word")],
            Vec::new(),
            Vec::new(),
        ),
    ]);
    let foreign_nodes = dom.children().expect("document children")[0]
        .children()
        .expect("html children")[0]
        .children()
        .expect("body children");
    assert_eq!(
        crate::replaced_element_presentation(&foreign_nodes[9], ReplacedKind::Img, Some(&provider),),
        None
    );
    assert_eq!(
        crate::replaced_element_presentation(
            &foreign_nodes[10],
            ReplacedKind::InputText,
            Some(&provider),
        ),
        None
    );
    assert_eq!(
        crate::replaced_element_presentation(
            &foreign_nodes[11],
            ReplacedKind::TextArea,
            Some(&provider),
        ),
        None
    );
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, Some(&provider));

    let image = box_by_node_id(&tree, Id(4));
    let Some(crate::ReplacedElementPresentation::Image(image)) = image.replaced_presentation()
    else {
        panic!("HTML img must carry typed image presentation");
    };
    assert_eq!(
        image.resolved_source(),
        Some("https://example.test/hero.png")
    );
    assert_eq!(image.alternative_text(), Some(" Hero "));

    let empty_image = box_by_node_id(&tree, Id(5));
    let Some(crate::ReplacedElementPresentation::Image(empty_image)) =
        empty_image.replaced_presentation()
    else {
        panic!("HTML img must carry typed image presentation");
    };
    assert_eq!(empty_image.resolved_source(), None);
    assert_eq!(empty_image.alternative_text(), Some(""));

    let absent_alt_image = box_by_node_id(&tree, Id(6));
    let Some(crate::ReplacedElementPresentation::Image(absent_alt_image)) =
        absent_alt_image.replaced_presentation()
    else {
        panic!("HTML img must carry typed image presentation");
    };
    assert_eq!(
        absent_alt_image.resolved_source(),
        Some("https://example.test/plain.png")
    );
    assert_eq!(absent_alt_image.alternative_text(), None);

    let ordinary_image = box_by_node_id(&tree, Id(16));
    let Some(crate::ReplacedElementPresentation::Image(ordinary_image)) =
        ordinary_image.replaced_presentation()
    else {
        panic!("HTML img must carry typed image presentation");
    };
    assert_eq!(ordinary_image.alternative_text(), Some("Hero"));

    let input = box_by_node_id(&tree, Id(7));
    let Some(crate::ReplacedElementPresentation::TextControl(input)) =
        input.replaced_presentation()
    else {
        panic!("HTML input must carry typed control presentation");
    };
    assert_eq!(input.placeholder(), Some(" Search "));

    for (id, expected) in [
        (8, Some("")),
        (9, None),
        (10, Some(" Multi word ")),
        (11, Some("")),
        (12, None),
        (17, Some("Search")),
        (18, Some("Multi word")),
    ] {
        let control = box_by_node_id(&tree, Id(id));
        let Some(crate::ReplacedElementPresentation::TextControl(control)) =
            control.replaced_presentation()
        else {
            panic!("HTML text control {id} must carry typed presentation");
        };
        assert_eq!(control.placeholder(), expected, "text control {id}");
    }

    assert_eq!(
        &*provider.seen_sources.borrow(),
        &[" hero.png ", "", "plain.png", "plain-ordinary.png"],
        "Layout must pass exact stored src values to Browser integration"
    );

    for foreign in [Id(13), Id(14), Id(15)] {
        assert!(
            tree.nodes()
                .iter()
                .all(|node| node.direct_node_id() != Some(foreign)),
            "foreign lookalikes must neither classify nor generate presentation boxes"
        );
    }
}

#[test]
fn unsupported_display_keywords_do_not_reach_box_generation_as_deferred_modes() {
    for (node_id, display) in [(4, "grid"), (5, "inline-flex")] {
        let dom = doc(vec![element(
            2,
            "html",
            Vec::new(),
            vec![element(
                3,
                "body",
                Vec::new(),
                vec![element(
                    node_id,
                    "span",
                    vec![("display", display)],
                    Vec::new(),
                )],
            )],
        )]);
        let styled = css::build_style_tree(&dom, None);
        let tree = BoxTree::generate(&styled, None);

        let unsupported = box_by_node_id(&tree, Id(node_id));
        assert_eq!(unsupported.display(), Display::Inline, "{display}");
        assert_eq!(unsupported.kind(), BoxKind::Inline, "{display}");
        assert_eq!(
            unsupported.display_behavior(),
            DisplayBoxBehavior::Inline,
            "{display}"
        );
    }
}

#[test]
fn list_item_marker_metadata_is_assigned_from_box_tree_parent_context() {
    let dom = doc(vec![
        element(
            2,
            "ul",
            Vec::new(),
            vec![
                element(3, "li", Vec::new(), vec![text(4, "a")]),
                element(5, "li", Vec::new(), vec![text(6, "b")]),
            ],
        ),
        element(
            7,
            "ol",
            Vec::new(),
            vec![
                element(8, "li", Vec::new(), vec![text(9, "one")]),
                element(10, "li", Vec::new(), vec![text(11, "two")]),
            ],
        ),
    ]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    let markers = tree
        .nodes()
        .iter()
        .filter_map(|node| {
            node.list_marker()
                .map(|marker| (node.direct_node_id(), marker))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        markers,
        vec![
            (Some(Id(3)), ListMarker::Unordered),
            (Some(Id(5)), ListMarker::Unordered),
            (Some(Id(8)), ListMarker::Ordered(1)),
            (Some(Id(10)), ListMarker::Ordered(2)),
        ]
    );
}

#[test]
fn list_marker_assignment_uses_display_box_behavior() {
    let dom = doc(vec![element(
        2,
        "ul",
        Vec::new(),
        vec![element(3, "li", vec![("display", "list-item")], Vec::new())],
    )]);

    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);
    let item = box_by_node_id(&tree, Id(3));

    assert_eq!(item.display(), Display::ListItem);
    assert_eq!(item.display_behavior(), DisplayBoxBehavior::ListItem);
    assert_eq!(item.list_marker(), Some(ListMarker::Unordered));
}
