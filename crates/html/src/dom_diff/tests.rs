use super::{DomPatch, diff_dom_stateless};
use crate::dom_snapshot::{DomSnapshotOptions, assert_dom_eq};
use crate::golden_corpus::fixtures;
use crate::test_support::patch_apply::TestPatchArena;
use crate::traverse::assign_missing_ids_allow_collisions;
use crate::types::{Id, Node};
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
            Node::Element {
                id: Id(2),
                name: Arc::from("div"),
                attributes: Vec::new(),
                style: Vec::new(),
                children: vec![Node::Element {
                    id: Id(3),
                    name: Arc::from("span"),
                    attributes: Vec::new(),
                    style: Vec::new(),
                    children: Vec::new(),
                }],
            },
            Node::Element {
                id: Id(4),
                name: Arc::from("p"),
                attributes: Vec::new(),
                style: Vec::new(),
                children: Vec::new(),
            },
        ],
    };

    let next = Node::Document {
        id: Id(1),
        doctype: None,
        children: vec![
            Node::Element {
                id: Id(2),
                name: Arc::from("div"),
                attributes: Vec::new(),
                style: Vec::new(),
                children: Vec::new(),
            },
            Node::Element {
                id: Id(4),
                name: Arc::from("p"),
                attributes: Vec::new(),
                style: Vec::new(),
                children: vec![Node::Element {
                    id: Id(3),
                    name: Arc::from("span"),
                    attributes: Vec::new(),
                    style: Vec::new(),
                    children: Vec::new(),
                }],
            },
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
