use super::super::{FosterParentingAnchorIndices, OpenElement, OpenElementsStack};
use super::make_scope_tags;
use crate::dom_patch::PatchKey;
use crate::html5::shared::DocumentParseContext;

#[test]
fn foster_parenting_anchor_cache_survives_non_tracked_push_pop_churn() {
    let mut ctx = DocumentParseContext::new();
    let tags = make_scope_tags(&mut ctx);
    let div = ctx.atoms.intern_ascii_folded("div").expect("atom");
    let mut stack = OpenElementsStack::default();

    stack.push(OpenElement::new(PatchKey(1), tags.html));
    stack.push(OpenElement::new(PatchKey(2), tags.table));

    let first = stack.foster_parenting_anchor_indices(tags.html, tags.table, tags.template);
    assert_eq!(
        first,
        FosterParentingAnchorIndices {
            html_index: Some(0),
            table_index: Some(1),
            template_index: None,
        }
    );
    assert_eq!(stack.foster_parenting_scan_calls(), 1);

    stack.push(OpenElement::new(PatchKey(3), div));
    let _ = stack.pop();

    let second = stack.foster_parenting_anchor_indices(tags.html, tags.table, tags.template);
    assert_eq!(second, first);
    assert_eq!(
        stack.foster_parenting_scan_calls(),
        1,
        "non-tracked push/pop churn above the table should reuse cached foster anchors"
    );
}
