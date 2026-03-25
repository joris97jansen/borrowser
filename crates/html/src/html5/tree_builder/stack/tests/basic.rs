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
