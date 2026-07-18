use super::{DomPatch, diff_dom_stateless};
use crate::dom_snapshot::{DomSnapshotOptions, assert_dom_eq};
use crate::golden_corpus::fixtures;
use crate::test_support::patch_apply::TestPatchArena;
use crate::traverse::assign_missing_ids_allow_collisions;
use crate::types::{DocumentFragmentNode, Id, Node};
use crate::{HtmlParseOptions, parse_document};
use std::sync::Arc;

fn build(input: &str) -> Node {
    let mut dom = parse_document(input, HtmlParseOptions::default())
        .expect("html5 parse should succeed")
        .document;
    assign_missing_ids_allow_collisions(&mut dom);
    dom
}

#[test]
fn diff_roundtrip_golden_fixtures() {
    let base = build("");
    let opts = DomSnapshotOptions {
        ignore_ids: true,
        ignore_empty_style: true,
    };
    for fixture in fixtures() {
        let next = build(fixture.input);
        let patches = diff_dom_stateless(&base, &next).expect("diff failed");
        let mut arena = TestPatchArena::from_dom(&base).expect("arena init failed");
        arena.apply(&patches).expect("apply failed");
        let materialized = arena.materialize().expect("materialize failed");
        assert_dom_eq(&next, &materialized, opts);
    }
}

#[test]
fn diff_is_deterministic() {
    let prev = build("<div><span>hi</span></div>");
    let next = build("<div><span>hi</span><em>ok</em></div>");
    let a = diff_dom_stateless(&prev, &next).expect("diff a failed");
    let b = diff_dom_stateless(&prev, &next).expect("diff b failed");
    assert_eq!(a, b, "expected deterministic patch output");
}

#[test]
fn diff_triggers_reset_on_midlist_insert() {
    let prev = build("<div><span>hi</span></div>");
    let next = build("<div><em>yo</em><span>hi</span></div>");
    let patches = diff_dom_stateless(&prev, &next).expect("diff failed");
    assert!(matches!(patches.first(), Some(DomPatch::Clear)));
}

#[test]
fn diff_resets_on_reparented_node() {
    let prev = Node::Document {
        id: Id(1),
        doctype: None,
        children: vec![
            crate::Node::from_element_parts(
                Id(2),
                Arc::from("div"),
                Vec::new(),
                Vec::new(),
                None,
                vec![crate::Node::from_element_parts(
                    Id(3),
                    Arc::from("span"),
                    Vec::new(),
                    Vec::new(),
                    None,
                    Vec::new(),
                )],
            ),
            crate::Node::from_element_parts(
                Id(4),
                Arc::from("p"),
                Vec::new(),
                Vec::new(),
                None,
                Vec::new(),
            ),
        ],
    };

    let next = Node::Document {
        id: Id(1),
        doctype: None,
        children: vec![
            crate::Node::from_element_parts(
                Id(2),
                Arc::from("div"),
                Vec::new(),
                Vec::new(),
                None,
                Vec::new(),
            ),
            crate::Node::from_element_parts(
                Id(4),
                Arc::from("p"),
                Vec::new(),
                Vec::new(),
                None,
                vec![crate::Node::from_element_parts(
                    Id(3),
                    Arc::from("span"),
                    Vec::new(),
                    Vec::new(),
                    None,
                    Vec::new(),
                )],
            ),
        ],
    };

    let patches = diff_dom_stateless(&prev, &next).expect("diff failed");
    assert!(
        matches!(patches.first(), Some(DomPatch::Clear)),
        "expected reset on reparent"
    );
}

#[test]
fn diff_reset_clears_allocation_state() {
    let prev = build("<div><span>hi</span></div>");
    let next = build("<div><em>yo</em><span>hi</span></div>");
    let mut arena = TestPatchArena::from_dom(&prev).expect("arena init failed");
    let patches = diff_dom_stateless(&prev, &next).expect("diff failed");
    arena.apply(&patches).expect("apply failed");
    let materialized = arena.materialize().expect("materialize failed");
    assert_dom_eq(
        &next,
        &materialized,
        DomSnapshotOptions {
            ignore_ids: true,
            ignore_empty_style: true,
        },
    );
}

fn template_doc(fragment_id: Id, text_id: Id, text: &str) -> Node {
    Node::Document {
        id: Id(1),
        doctype: None,
        children: vec![crate::Node::from_element_parts(
            Id(2),
            Arc::from("template"),
            Vec::new(),
            Vec::new(),
            Some(Box::new(DocumentFragmentNode::new_template_contents(
                fragment_id,
                vec![Node::Text {
                    id: text_id,
                    text: text.to_string(),
                }],
            ))),
            Vec::new(),
        )],
    }
}

#[test]
fn diff_emits_typed_template_association_and_fragment_child_edges() {
    let prev = Node::Document {
        id: Id(1),
        doctype: None,
        children: Vec::new(),
    };
    let next = template_doc(Id(3), Id(4), "inert");
    let patches = diff_dom_stateless(&prev, &next).expect("template diff should succeed");
    assert!(patches.iter().any(|patch| matches!(
        patch,
        DomPatch::CreateTemplateContents {
            host: crate::dom_patch::PatchKey(2),
            contents: crate::dom_patch::PatchKey(3)
        }
    )));
    assert!(patches.iter().any(|patch| matches!(
        patch,
        DomPatch::AppendChild {
            parent: crate::dom_patch::PatchKey(3),
            child: crate::dom_patch::PatchKey(4)
        }
    )));

    let mut arena = TestPatchArena::from_dom(&prev).expect("arena init failed");
    arena
        .apply(&patches)
        .expect("template patches should apply");
    let actual = arena
        .materialize()
        .expect("template DOM should materialize");
    assert_dom_eq(
        &next,
        &actual,
        DomSnapshotOptions {
            ignore_ids: true,
            ignore_empty_style: true,
        },
    );
}

#[test]
fn changing_fragment_identity_beneath_a_surviving_host_forces_reset() {
    let prev = template_doc(Id(3), Id(4), "inert");
    let next = template_doc(Id(30), Id(4), "inert");
    let patches = diff_dom_stateless(&prev, &next).expect("template diff should succeed");
    assert!(
        matches!(patches.first(), Some(DomPatch::Clear)),
        "association replacement must reset instead of attempting live reassociation"
    );
}

#[test]
fn duplicate_identity_across_template_host_fragment_and_descendants_is_rejected() {
    let previous = Node::Document {
        id: Id(1),
        doctype: None,
        children: Vec::new(),
    };
    for invalid in [
        template_doc(Id(2), Id(4), "fragment collides with host"),
        template_doc(Id(3), Id(3), "descendant collides with fragment"),
    ] {
        assert!(
            diff_dom_stateless(&previous, &invalid).is_err(),
            "full-model identity collection must reject collisions"
        );
    }
}
