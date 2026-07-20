use super::make_scope_tags;
use crate::dom_patch::PatchKey;
use crate::html5::shared::{DocumentParseContext, EngineInvariantError};
use crate::html5::tree_builder::stack::types::OpenElementMatch;
use crate::html5::tree_builder::stack::{InBodyEndTagScan, OpenElement, OpenElementsStack};

fn push(stack: &mut OpenElementsStack, key: u32, name: crate::html5::shared::AtomId) {
    stack.push(OpenElement::new_html(PatchKey(key), name));
}

#[test]
fn generic_end_scan_matches_through_non_special_without_scope_metrics() {
    let mut ctx = DocumentParseContext::new();
    let tags = make_scope_tags(&mut ctx);
    let option = ctx.atoms.intern_ascii_folded("option").expect("atom");
    let span = ctx.atoms.intern_ascii_folded("span").expect("atom");
    let mut stack = OpenElementsStack::default();
    push(&mut stack, 1, tags.html);
    push(&mut stack, 2, option);
    push(&mut stack, 3, span);

    let before_scope_calls = stack.scope_scan_calls();
    let before_scope_steps = stack.scope_scan_steps();
    let result = stack
        .scan_in_body_any_other_end_tag(option, &ctx.atoms)
        .expect("rooted stack");
    assert_eq!(
        result,
        InBodyEndTagScan::Matched(OpenElementMatch {
            index: 1,
            element: OpenElement::new_html(PatchKey(2), option),
        })
    );
    assert_eq!(stack.end_tag_scan_calls(), 1);
    assert_eq!(stack.end_tag_scan_steps(), 2);
    assert_eq!(stack.scope_scan_calls(), before_scope_calls);
    assert_eq!(stack.scope_scan_steps(), before_scope_steps);

    let InBodyEndTagScan::Matched(matched) = result else {
        panic!("expected match");
    };
    let removed = stack
        .pop_suffix_from_match(matched)
        .expect("stable match must be removable");
    assert_eq!(removed.name(), option);
    assert_eq!(stack.current().map(OpenElement::name), Some(tags.html));
}

#[test]
fn generic_end_scan_is_blocked_by_corrected_special_names() {
    for blocker_name in ["object", "noscript", "keygen", "select"] {
        let mut ctx = DocumentParseContext::new();
        let tags = make_scope_tags(&mut ctx);
        let option = ctx.atoms.intern_ascii_folded("option").expect("atom");
        let blocker = ctx.atoms.intern_ascii_folded(blocker_name).expect("atom");
        let mut stack = OpenElementsStack::default();
        push(&mut stack, 1, tags.html);
        push(&mut stack, 2, option);
        push(&mut stack, 3, blocker);

        assert_eq!(
            stack
                .scan_in_body_any_other_end_tag(option, &ctx.atoms)
                .expect("rooted stack"),
            InBodyEndTagScan::BlockedBySpecial {
                index: 2,
                element: OpenElement::new_html(PatchKey(3), blocker),
            },
            "{blocker_name} must block the target"
        );
    }
}

#[test]
fn missing_target_is_normally_blocked_by_html_root() {
    let mut ctx = DocumentParseContext::new();
    let tags = make_scope_tags(&mut ctx);
    let option = ctx.atoms.intern_ascii_folded("option").expect("atom");
    let span = ctx.atoms.intern_ascii_folded("span").expect("atom");
    let mut stack = OpenElementsStack::default();
    push(&mut stack, 1, tags.html);
    push(&mut stack, 2, span);

    assert_eq!(
        stack
            .scan_in_body_any_other_end_tag(option, &ctx.atoms)
            .expect("rooted stack"),
        InBodyEndTagScan::BlockedBySpecial {
            index: 0,
            element: OpenElement::new_html(PatchKey(1), tags.html),
        }
    );
}

#[test]
fn exhausting_stack_without_html_root_is_invariant_error() {
    let mut ctx = DocumentParseContext::new();
    let option = ctx.atoms.intern_ascii_folded("option").expect("atom");
    let span = ctx.atoms.intern_ascii_folded("span").expect("atom");
    let mut stack = OpenElementsStack::default();
    push(&mut stack, 1, span);

    assert!(matches!(
        stack.scan_in_body_any_other_end_tag(option, &ctx.atoms),
        Err(EngineInvariantError)
    ));
}

#[test]
fn stale_generic_end_match_fails_before_mutating_the_stack_suffix() {
    let mut ctx = DocumentParseContext::new();
    let tags = make_scope_tags(&mut ctx);
    let option = ctx.atoms.intern_ascii_folded("option").expect("atom");
    let span = ctx.atoms.intern_ascii_folded("span").expect("atom");
    let div = ctx.atoms.intern_ascii_folded("div").expect("atom");
    let mut stack = OpenElementsStack::default();
    push(&mut stack, 1, tags.html);
    push(&mut stack, 2, option);
    push(&mut stack, 3, span);

    let InBodyEndTagScan::Matched(matched) = stack
        .scan_in_body_any_other_end_tag(option, &ctx.atoms)
        .expect("rooted stack")
    else {
        panic!("option must be matched before the HTML root");
    };

    assert_eq!(stack.pop().map(OpenElement::key), Some(PatchKey(3)));
    assert_eq!(stack.pop().map(OpenElement::key), Some(PatchKey(2)));
    stack.push(OpenElement::new_html(PatchKey(4), div));
    let before_items = stack.items.clone();
    let before_name_counts = stack.name_counts.clone();
    let before_pop_ops = stack.pop_ops();

    assert!(matches!(
        stack.pop_suffix_from_match(matched),
        Err(EngineInvariantError)
    ));
    assert_eq!(stack.items, before_items);
    assert_eq!(stack.name_counts, before_name_counts);
    assert_eq!(stack.pop_ops(), before_pop_ops);
}
