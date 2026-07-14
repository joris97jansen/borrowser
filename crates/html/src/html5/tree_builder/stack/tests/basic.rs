use super::super::{OpenElement, OpenElementsStack};
use crate::dom_patch::PatchKey;
use crate::html5::shared::DocumentParseContext;

#[test]
fn open_elements_push_pop_and_current_are_deterministic() {
    let mut ctx = DocumentParseContext::new();
    let div = ctx.atoms.intern_ascii_folded("div").expect("atom");
    let span = ctx.atoms.intern_ascii_folded("span").expect("atom");
    let mut stack = OpenElementsStack::default();

    assert!(stack.current().is_none());
    stack.push(OpenElement::new(PatchKey(1), div));
    assert_eq!(stack.current().map(|n| n.key()), Some(PatchKey(1)));
    stack.push(OpenElement::new(PatchKey(2), span));
    assert_eq!(stack.current().map(|n| n.key()), Some(PatchKey(2)));
    assert_eq!(stack.pop().map(|n| n.key()), Some(PatchKey(2)));
    assert_eq!(stack.current().map(|n| n.key()), Some(PatchKey(1)));
}

#[test]
fn exact_key_removal_preserves_same_name_identity_order_counts_and_foster_cache() {
    use super::make_scope_tags;
    use crate::html5::tree_builder::stack::FosterParentingAnchorIndices;

    let mut ctx = DocumentParseContext::new();
    let tags = make_scope_tags(&mut ctx);
    let form = ctx.atoms.intern_ascii_folded("form").expect("atom");
    let mut stack = OpenElementsStack::default();

    stack.push(OpenElement::new(PatchKey(1), tags.html));
    stack.push(OpenElement::new(PatchKey(2), tags.table));
    stack.push(OpenElement::new(PatchKey(3), form));
    stack.push(OpenElement::new(PatchKey(4), form));
    assert_eq!(
        stack.foster_parenting_anchor_indices(tags.html, tags.table, tags.template),
        FosterParentingAnchorIndices {
            html_index: Some(0),
            table_index: Some(1),
            template_index: None,
        }
    );
    assert_eq!(stack.foster_parenting_scan_calls(), 1);
    let before_pop_ops = stack.pop_ops();

    let removed = stack
        .remove_exact_key(PatchKey(3))
        .expect("first form identity should be removable");
    assert_eq!(removed.removed.key(), PatchKey(3));
    assert_eq!(removed.removed.name(), form);
    assert_eq!(removed.index, 2);
    assert!(!removed.was_current);
    assert_eq!(
        stack.iter_keys().collect::<Vec<_>>(),
        vec![PatchKey(1), PatchKey(2), PatchKey(4)],
        "only the requested same-name identity may be removed"
    );
    assert!(stack.contains_key(PatchKey(4)));
    assert_eq!(
        stack.name_counts,
        vec![(tags.html, 1), (tags.table, 1), (form, 1)]
    );
    assert_eq!(stack.pop_ops(), before_pop_ops + 1);

    assert_eq!(
        stack.foster_parenting_anchor_indices(tags.html, tags.table, tags.template),
        FosterParentingAnchorIndices {
            html_index: Some(0),
            table_index: Some(1),
            template_index: None,
        },
        "exact removal must leave foster anchors coherent after deterministic invalidation"
    );
    assert_eq!(
        stack.foster_parenting_scan_calls(),
        2,
        "exact removal invalidates the cached anchors rather than reusing stale indices"
    );
}
