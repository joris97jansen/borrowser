use super::super::{OpenElement, OpenElementsStack};
use super::make_scope_tags;
use crate::dom_patch::PatchKey;
use crate::html5::shared::DocumentParseContext;

#[test]
fn clear_to_table_body_context_stops_at_row_group_root() {
    let mut ctx = DocumentParseContext::new();
    let tags = make_scope_tags(&mut ctx);
    let tbody = ctx.atoms.intern_ascii_folded("tbody").expect("atom");
    let thead = ctx.atoms.intern_ascii_folded("thead").expect("atom");
    let tfoot = ctx.atoms.intern_ascii_folded("tfoot").expect("atom");
    let tr = ctx.atoms.intern_ascii_folded("tr").expect("atom");
    let td = ctx.atoms.intern_ascii_folded("td").expect("atom");

    let mut stack = OpenElementsStack::default();
    stack.push(OpenElement::new(PatchKey(1), tags.html));
    stack.push(OpenElement::new(PatchKey(2), tags.table));
    stack.push(OpenElement::new(PatchKey(3), tbody));
    stack.push(OpenElement::new(PatchKey(4), tr));
    stack.push(OpenElement::new(PatchKey(5), td));

    let removed = stack.clear_to_table_body_context(tbody, thead, tfoot, &tags);
    assert_eq!(removed, 2);
    assert_eq!(stack.current().map(|entry| entry.key()), Some(PatchKey(3)));
}

#[test]
fn clear_to_table_row_context_stops_at_row_root() {
    let mut ctx = DocumentParseContext::new();
    let tags = make_scope_tags(&mut ctx);
    let tbody = ctx.atoms.intern_ascii_folded("tbody").expect("atom");
    let tr = ctx.atoms.intern_ascii_folded("tr").expect("atom");
    let td = ctx.atoms.intern_ascii_folded("td").expect("atom");
    let b = ctx.atoms.intern_ascii_folded("b").expect("atom");

    let mut stack = OpenElementsStack::default();
    stack.push(OpenElement::new(PatchKey(1), tags.html));
    stack.push(OpenElement::new(PatchKey(2), tags.table));
    stack.push(OpenElement::new(PatchKey(3), tbody));
    stack.push(OpenElement::new(PatchKey(4), tr));
    stack.push(OpenElement::new(PatchKey(5), td));
    stack.push(OpenElement::new(PatchKey(6), b));

    let removed = stack.clear_to_table_row_context(tr, &tags);
    assert_eq!(removed, 2);
    assert_eq!(stack.current().map(|entry| entry.key()), Some(PatchKey(4)));
}
