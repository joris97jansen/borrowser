use super::super::{OpenElement, OpenElementsStack, ScopeKind};
use super::make_scope_tags;
use crate::dom_patch::PatchKey;
use crate::html5::shared::DocumentParseContext;

#[test]
fn open_elements_in_scope_checks_follow_core_v0_boundaries() {
    let mut ctx = DocumentParseContext::new();
    let tags = make_scope_tags(&mut ctx);
    let p = ctx.atoms.intern_ascii_folded("p").expect("atom");
    let li = ctx.atoms.intern_ascii_folded("li").expect("atom");

    let mut stack = OpenElementsStack::default();
    stack.push(OpenElement::new(PatchKey(1), tags.html));
    stack.push(OpenElement::new(PatchKey(2), p));
    assert!(stack.has_in_scope(p, ScopeKind::InScope, &tags));

    stack.push(OpenElement::new(PatchKey(3), tags.table));
    assert!(!stack.has_in_scope(li, ScopeKind::InScope, &tags));

    let mut list_stack = OpenElementsStack::default();
    list_stack.push(OpenElement::new(PatchKey(1), tags.html));
    list_stack.push(OpenElement::new(PatchKey(2), li));
    assert!(list_stack.has_in_scope(li, ScopeKind::ListItem, &tags));
    list_stack.push(OpenElement::new(PatchKey(3), tags.ul));
    assert!(!list_stack.has_in_scope(li, ScopeKind::ListItem, &tags));

    let mut button_stack = OpenElementsStack::default();
    button_stack.push(OpenElement::new(PatchKey(1), tags.html));
    button_stack.push(OpenElement::new(PatchKey(2), p));
    assert!(button_stack.has_in_scope(p, ScopeKind::Button, &tags));
    button_stack.push(OpenElement::new(PatchKey(3), tags.button));
    assert!(!button_stack.has_in_scope(p, ScopeKind::Button, &tags));

    let mut table_scope_stack = OpenElementsStack::default();
    table_scope_stack.push(OpenElement::new(PatchKey(1), tags.html));
    table_scope_stack.push(OpenElement::new(PatchKey(2), p));
    assert!(table_scope_stack.has_in_scope(p, ScopeKind::Table, &tags));
    table_scope_stack.push(OpenElement::new(PatchKey(3), tags.table));
    assert!(!table_scope_stack.has_in_scope(p, ScopeKind::Table, &tags));
}

#[test]
fn open_elements_target_before_boundary_is_visible() {
    let mut ctx = DocumentParseContext::new();
    let tags = make_scope_tags(&mut ctx);
    let p = ctx.atoms.intern_ascii_folded("p").expect("atom");
    let mut stack = OpenElementsStack::default();

    stack.push(OpenElement::new(PatchKey(1), tags.html));
    stack.push(OpenElement::new(PatchKey(2), tags.table));
    stack.push(OpenElement::new(PatchKey(3), p));
    assert!(stack.has_in_scope(p, ScopeKind::InScope, &tags));

    stack.push(OpenElement::new(PatchKey(4), tags.template));
    assert!(!stack.has_in_scope(p, ScopeKind::InScope, &tags));
}

#[test]
fn pop_until_including_in_scope_returns_matched_element() {
    let mut ctx = DocumentParseContext::new();
    let tags = make_scope_tags(&mut ctx);
    let div = ctx.atoms.intern_ascii_folded("div").expect("atom");
    let span = ctx.atoms.intern_ascii_folded("span").expect("atom");
    let mut stack = OpenElementsStack::default();
    stack.push(OpenElement::new(PatchKey(1), tags.html));
    stack.push(OpenElement::new(PatchKey(2), div));
    stack.push(OpenElement::new(PatchKey(3), span));

    let popped = stack.pop_until_including_in_scope(div, ScopeKind::InScope, &tags);
    assert_eq!(popped.map(|entry| entry.key()), Some(PatchKey(2)));
    assert_eq!(popped.map(|entry| entry.name()), Some(div));
    assert_eq!(stack.current().map(|entry| entry.key()), Some(PatchKey(1)));

    let not_found = stack.pop_until_including_in_scope(div, ScopeKind::InScope, &tags);
    assert!(not_found.is_none());
}

#[test]
fn pop_until_including_in_scope_does_not_mutate_when_boundary_hides_target() {
    let mut ctx = DocumentParseContext::new();
    let tags = make_scope_tags(&mut ctx);
    let div = ctx.atoms.intern_ascii_folded("div").expect("atom");
    let section = ctx.atoms.intern_ascii_folded("section").expect("atom");
    let mut stack = OpenElementsStack::default();
    stack.push(OpenElement::new(PatchKey(1), tags.html));
    stack.push(OpenElement::new(PatchKey(2), div));
    stack.push(OpenElement::new(PatchKey(3), tags.table));
    stack.push(OpenElement::new(PatchKey(4), section));

    let before: Vec<_> = stack.iter_keys().collect();
    let not_found = stack.pop_until_including_in_scope(div, ScopeKind::InScope, &tags);
    let after: Vec<_> = stack.iter_keys().collect();
    assert!(not_found.is_none());
    assert_eq!(
        after, before,
        "failed in-scope pop must not partially mutate SOE"
    );
}

#[test]
fn pop_until_including_in_scope_respects_button_scope_boundary() {
    let mut ctx = DocumentParseContext::new();
    let tags = make_scope_tags(&mut ctx);
    let p = ctx.atoms.intern_ascii_folded("p").expect("atom");
    let span = ctx.atoms.intern_ascii_folded("span").expect("atom");
    let mut stack = OpenElementsStack::default();
    stack.push(OpenElement::new(PatchKey(1), tags.html));
    stack.push(OpenElement::new(PatchKey(2), p));
    stack.push(OpenElement::new(PatchKey(3), tags.button));
    stack.push(OpenElement::new(PatchKey(4), span));

    let before: Vec<_> = stack.iter_keys().collect();
    let not_found = stack.pop_until_including_in_scope(p, ScopeKind::Button, &tags);
    let after: Vec<_> = stack.iter_keys().collect();
    assert!(not_found.is_none());
    assert_eq!(
        after, before,
        "button-scope boundary should block pops below <button>"
    );
}

#[test]
fn pop_until_including_in_scope_respects_list_item_scope_boundary() {
    let mut ctx = DocumentParseContext::new();
    let tags = make_scope_tags(&mut ctx);
    let li = ctx.atoms.intern_ascii_folded("li").expect("atom");
    let span = ctx.atoms.intern_ascii_folded("span").expect("atom");
    let mut stack = OpenElementsStack::default();
    stack.push(OpenElement::new(PatchKey(1), tags.html));
    stack.push(OpenElement::new(PatchKey(2), li));
    stack.push(OpenElement::new(PatchKey(3), tags.ul));
    stack.push(OpenElement::new(PatchKey(4), span));

    let before: Vec<_> = stack.iter_keys().collect();
    let not_found = stack.pop_until_including_in_scope(li, ScopeKind::ListItem, &tags);
    let after: Vec<_> = stack.iter_keys().collect();
    assert!(not_found.is_none());
    assert_eq!(
        after, before,
        "list-item-scope boundary should block pops below <ol>/<ul>"
    );
}
