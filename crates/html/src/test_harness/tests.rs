use super::{
    BoundaryPolicy, ChunkPlan, materialize_patch_batches, run_chunked, run_full,
    shrink_chunk_plan_with_stats,
};
use crate::DomPatch;
use crate::dom_patch::PatchKey;
use crate::dom_snapshot::{DomSnapshotOptions, assert_dom_eq};
use crate::tokenizer::Tokenizer;
use crate::types::{Id, Node};
use std::sync::Arc;

#[test]
fn chunked_fixed_matches_full() {
    let input = "<p>café &amp; crème</p>";
    let full = run_full(input);
    let chunked = run_chunked(input, &ChunkPlan::fixed_unaligned(1));
    assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
}

#[test]
fn chunked_boundary_plan_allows_unaligned_splits_in_ascii_prefix() {
    let input = "<p>é</p>";
    let boundaries = vec![1, 2];
    let full = run_full(input);
    let chunked = run_chunked(input, &ChunkPlan::boundaries_unaligned(boundaries));
    assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
}

#[test]
fn chunked_boundary_splits_utf8_codepoint() {
    let input = "<p>é</p>";
    let boundaries = vec![4];
    let full = run_full(input);
    let chunked = run_chunked(input, &ChunkPlan::boundaries_unaligned(boundaries));
    assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
}

#[test]
fn chunked_boundary_splits_comment_terminator() {
    let input = "<!--x-->";
    let boundaries = vec!["<!--x--".len()];
    let full = run_full(input);
    let chunked = run_chunked(input, &ChunkPlan::boundaries(boundaries));
    assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
}

#[test]
fn chunked_boundary_splits_rawtext_close_tag() {
    let input = "<script>hi</script>";
    let boundaries = vec!["<script>hi</scr".len()];
    let full = run_full(input);
    let chunked = run_chunked(input, &ChunkPlan::boundaries(boundaries));
    assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
}

#[test]
fn chunked_draining_leaves_no_tokens_behind() {
    let input = "<div>ok</div><!--x-->";
    let bytes = input.as_bytes();
    let sizes = [2, 3, 1];
    let mut tokenizer = Tokenizer::new();
    let mut tokens = Vec::new();
    let mut offset = 0usize;

    for size in sizes {
        if offset >= bytes.len() {
            break;
        }
        let end = (offset + size).min(bytes.len());
        tokenizer.feed(&bytes[offset..end]);
        tokenizer.drain_into(&mut tokens);
        offset = end;
    }
    if offset < bytes.len() {
        tokenizer.feed(&bytes[offset..]);
    }
    tokenizer.finish();
    tokenizer.drain_into(&mut tokens);

    assert!(
        tokenizer.drain_tokens().is_empty(),
        "expected tokenizer to have no buffered tokens after draining"
    );

    let (atoms, source, text_pool) = tokenizer.into_parts();
    let stream = crate::TokenStream::new(tokens, atoms, source, text_pool);
    let expected = crate::tokenize(input);
    assert_eq!(
        crate::test_utils::token_snapshot(&expected),
        crate::test_utils::token_snapshot(&stream),
        "expected drained tokens to match full tokenize() snapshot"
    );
}

#[test]
fn shrinker_reduces_boundary_count() {
    let input = "<p>abcd</p>";
    let plan = ChunkPlan::Boundaries {
        indices: vec![1, 2, 3, 4, 5, 6, 7],
        policy: BoundaryPolicy::ByteStream,
    };
    let (minimized, _) = shrink_chunk_plan_with_stats(input, &plan, |candidate| match candidate {
        ChunkPlan::Boundaries { indices, .. } => indices.len() > 2,
        _ => false,
    });
    let minimized_len = match minimized {
        ChunkPlan::Boundaries { indices, .. } => indices.len(),
        _ => 0,
    };
    assert!(
        minimized_len > 0 && minimized_len < 7,
        "expected shrinker to reduce boundary count, got {minimized_len}"
    );
}

fn element(name: &str, children: Vec<Node>) -> Node {
    Node::Element {
        id: Id::INVALID,
        name: Arc::from(name),
        attributes: Vec::new(),
        style: Vec::new(),
        children,
    }
}

fn text(value: &str) -> Node {
    Node::Text {
        id: Id::INVALID,
        text: value.to_string(),
    }
}

#[test]
fn materialize_patch_batches_supports_cross_parent_reparenting() {
    let dom = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("div"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("p"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: Arc::from("span"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(3),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(4),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(4),
        },
    ]])
    .expect("cross-parent reparenting should materialize");

    let expected = Node::Document {
        id: Id::INVALID,
        doctype: None,
        children: vec![
            element("div", Vec::new()),
            element("p", vec![element("span", Vec::new())]),
        ],
    };
    assert_dom_eq(&expected, &dom, DomSnapshotOptions::default());
}

#[test]
fn materialize_patch_batches_supports_same_parent_reordering_and_noops() {
    let dom = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("ul"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("li"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: Arc::from("li"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(5),
            name: Arc::from("li"),
            attributes: Vec::new(),
        },
        DomPatch::CreateText {
            key: PatchKey(6),
            text: "a".to_string(),
        },
        DomPatch::CreateText {
            key: PatchKey(7),
            text: "b".to_string(),
        },
        DomPatch::CreateText {
            key: PatchKey(8),
            text: "c".to_string(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(4),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(5),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(6),
        },
        DomPatch::AppendChild {
            parent: PatchKey(4),
            child: PatchKey(7),
        },
        DomPatch::AppendChild {
            parent: PatchKey(5),
            child: PatchKey(8),
        },
        DomPatch::InsertBefore {
            parent: PatchKey(2),
            child: PatchKey(5),
            before: PatchKey(3),
        },
        DomPatch::InsertBefore {
            parent: PatchKey(2),
            child: PatchKey(3),
            before: PatchKey(4),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(4),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(4),
        },
    ]])
    .expect("same-parent reorder/no-op sequence should materialize");

    let expected = Node::Document {
        id: Id::INVALID,
        doctype: None,
        children: vec![element(
            "ul",
            vec![
                element("li", vec![text("c")]),
                element("li", vec![text("a")]),
                element("li", vec![text("b")]),
            ],
        )],
    };
    assert_dom_eq(&expected, &dom, DomSnapshotOptions::default());
}

#[test]
fn materialize_patch_batches_supports_same_parent_append_child_move_to_end() {
    let dom = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("ul"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("li"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: Arc::from("li"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(5),
            name: Arc::from("li"),
            attributes: Vec::new(),
        },
        DomPatch::CreateText {
            key: PatchKey(6),
            text: "a".to_string(),
        },
        DomPatch::CreateText {
            key: PatchKey(7),
            text: "b".to_string(),
        },
        DomPatch::CreateText {
            key: PatchKey(8),
            text: "c".to_string(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(4),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(5),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(6),
        },
        DomPatch::AppendChild {
            parent: PatchKey(4),
            child: PatchKey(7),
        },
        DomPatch::AppendChild {
            parent: PatchKey(5),
            child: PatchKey(8),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
    ]])
    .expect("same-parent append-child move should materialize");

    let expected = Node::Document {
        id: Id::INVALID,
        doctype: None,
        children: vec![element(
            "ul",
            vec![
                element("li", vec![text("b")]),
                element("li", vec![text("c")]),
                element("li", vec![text("a")]),
            ],
        )],
    };
    assert_dom_eq(&expected, &dom, DomSnapshotOptions::default());
}

#[test]
fn materialize_patch_batches_supports_cross_parent_insert_before_move() {
    let dom = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("div"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("p"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: Arc::from("span"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(5),
            name: Arc::from("em"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(3),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(4),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(5),
        },
        DomPatch::InsertBefore {
            parent: PatchKey(3),
            child: PatchKey(4),
            before: PatchKey(5),
        },
    ]])
    .expect("cross-parent insert-before move should materialize");

    let expected = Node::Document {
        id: Id::INVALID,
        doctype: None,
        children: vec![
            element("div", Vec::new()),
            element(
                "p",
                vec![element("span", Vec::new()), element("em", Vec::new())],
            ),
        ],
    };
    assert_dom_eq(&expected, &dom, DomSnapshotOptions::default());
}

#[test]
fn materialize_patch_batches_supports_same_parent_insert_before_move() {
    let dom = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("ul"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("li"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: Arc::from("li"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(5),
            name: Arc::from("li"),
            attributes: Vec::new(),
        },
        DomPatch::CreateText {
            key: PatchKey(6),
            text: "a".to_string(),
        },
        DomPatch::CreateText {
            key: PatchKey(7),
            text: "b".to_string(),
        },
        DomPatch::CreateText {
            key: PatchKey(8),
            text: "c".to_string(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(4),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(5),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(6),
        },
        DomPatch::AppendChild {
            parent: PatchKey(4),
            child: PatchKey(7),
        },
        DomPatch::AppendChild {
            parent: PatchKey(5),
            child: PatchKey(8),
        },
        DomPatch::InsertBefore {
            parent: PatchKey(2),
            child: PatchKey(5),
            before: PatchKey(3),
        },
    ]])
    .expect("same-parent insert-before move should materialize");

    let expected = Node::Document {
        id: Id::INVALID,
        doctype: None,
        children: vec![element(
            "ul",
            vec![
                element("li", vec![text("c")]),
                element("li", vec![text("a")]),
                element("li", vec![text("b")]),
            ],
        )],
    };
    assert_dom_eq(&expected, &dom, DomSnapshotOptions::default());
}

#[test]
fn materialize_patch_batches_rejects_moves_of_removed_nodes() {
    let error = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("div"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("span"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
        DomPatch::RemoveNode { key: PatchKey(3) },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
    ]])
    .expect_err("moving a removed node should be rejected");
    assert!(
        error.contains("missing node") || error.contains("missing child"),
        "unexpected removed-node move error: {error}"
    );
}

#[test]
fn materialize_patch_batches_rejects_moves_of_removed_subtree_descendants() {
    let error = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("div"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("section"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: Arc::from("span"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(4),
        },
        DomPatch::RemoveNode { key: PatchKey(3) },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(4),
        },
    ]])
    .expect_err("moving a descendant of a removed subtree should be rejected");
    assert!(
        error.contains("missing node") || error.contains("missing child"),
        "unexpected removed-subtree descendant move error: {error}"
    );
}

#[test]
fn materialize_patch_batches_rejects_document_and_root_element_moves() {
    let patches = vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("html"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("body"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(1),
        },
    ];
    let document_error =
        materialize_patch_batches(&[patches]).expect_err("document move should be rejected");
    assert!(
        document_error.contains("document node"),
        "unexpected document-move error: {document_error}"
    );

    let root_move_error = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("html"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("body"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(2),
        },
    ]])
    .expect_err("document root element move should be rejected");
    assert!(
        root_move_error.contains("document root element"),
        "unexpected root-element error: {root_move_error}"
    );

    let insert_before_document_error = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("html"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("body"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: Arc::from("anchor"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(4),
        },
        DomPatch::InsertBefore {
            parent: PatchKey(3),
            child: PatchKey(1),
            before: PatchKey(4),
        },
    ]])
    .expect_err("insert-before document move should be rejected");
    assert!(
        insert_before_document_error.contains("document node"),
        "unexpected insert-before document error: {insert_before_document_error}"
    );

    let insert_before_root_error = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("html"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("body"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: Arc::from("anchor"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(4),
        },
        DomPatch::InsertBefore {
            parent: PatchKey(3),
            child: PatchKey(2),
            before: PatchKey(4),
        },
    ]])
    .expect_err("insert-before document root move should be rejected");
    assert!(
        insert_before_root_error.contains("document root element"),
        "unexpected insert-before root error: {insert_before_root_error}"
    );
}
