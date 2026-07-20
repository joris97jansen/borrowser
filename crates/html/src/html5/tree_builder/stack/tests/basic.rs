use super::super::{OpenElement, OpenElementsStack};
use crate::dom_patch::PatchKey;
use crate::html5::shared::DocumentParseContext;
use crate::html5::tree_builder::stack::types::ExpandedNameKey;

#[test]
fn open_elements_push_pop_and_current_are_deterministic() {
    let mut ctx = DocumentParseContext::new();
    let div = ctx.atoms.intern_ascii_folded("div").expect("atom");
    let span = ctx.atoms.intern_ascii_folded("span").expect("atom");
    let mut stack = OpenElementsStack::default();

    assert!(stack.current().is_none());
    stack.push(OpenElement::new_html(PatchKey(1), div));
    assert_eq!(stack.current().map(|n| n.key()), Some(PatchKey(1)));
    stack.push(OpenElement::new_html(PatchKey(2), span));
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

    stack.push(OpenElement::new_html(PatchKey(1), tags.html));
    stack.push(OpenElement::new_html(PatchKey(2), tags.table));
    stack.push(OpenElement::new_html(PatchKey(3), form));
    stack.push(OpenElement::new_html(PatchKey(4), form));
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
        vec![
            (
                ExpandedNameKey::new(crate::ElementNamespace::Html, tags.html),
                1,
            ),
            (
                ExpandedNameKey::new(crate::ElementNamespace::Html, tags.table),
                1,
            ),
            (ExpandedNameKey::new(crate::ElementNamespace::Html, form), 1,),
        ]
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

#[test]
fn expanded_name_cache_separates_namespaces_and_preserves_exact_svg_case() {
    let mut ctx = DocumentParseContext::new();
    let html_title = ctx.atoms.intern_exact("title").expect("HTML atom");
    let svg_title = ctx.atoms.intern_exact("title").expect("SVG atom");
    let foreign_object = ctx
        .atoms
        .intern_exact("foreignObject")
        .expect("canonical SVG atom");
    let mut stack = OpenElementsStack::default();

    stack.push(OpenElement::new_html(PatchKey(1), html_title));
    stack.push(OpenElement::new_foreign(
        PatchKey(2),
        crate::ElementNamespace::Svg,
        svg_title,
    ));
    stack.push(OpenElement::new_foreign(
        PatchKey(3),
        crate::ElementNamespace::Svg,
        foreign_object,
    ));

    assert_eq!(stack.name_counts.len(), 3);
    assert!(stack.has_name_count(crate::ElementNamespace::Html, html_title));
    assert!(stack.has_name_count(crate::ElementNamespace::Svg, svg_title));
    assert!(stack.has_name_count(crate::ElementNamespace::Svg, foreign_object));
    assert!(!stack.has_name_count(crate::ElementNamespace::Html, foreign_object));
    assert_eq!(ctx.atoms.resolve(foreign_object), Some("foreignObject"));
    assert!(stack.name_cache_matches_stack());
}

#[test]
fn expanded_name_cache_keys_do_not_cross_parser_interner_domains() {
    let mut left = DocumentParseContext::new();
    let mut right = DocumentParseContext::new();
    let left_div = left.atoms.intern_exact("div").expect("left atom");
    let right_div = right.atoms.intern_exact("div").expect("right atom");

    assert_ne!(left_div, right_div);
    assert_ne!(
        ExpandedNameKey::new(crate::ElementNamespace::Html, left_div),
        ExpandedNameKey::new(crate::ElementNamespace::Html, right_div),
    );
    assert_eq!(left.atoms.resolve(right_div), None);
    assert_eq!(right.atoms.resolve(left_div), None);

    let mut stack = OpenElementsStack::new(left.atoms.id());
    stack.push(OpenElement::new_html(PatchKey(1), left_div));
    let before_items = stack.items.clone();
    let before_counts = stack.name_counts.clone();
    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = stack.replace_at(0, OpenElement::new_html(PatchKey(2), right_div));
    }));
    assert!(rejected.is_err());
    assert_eq!(stack.items, before_items);
    assert_eq!(stack.name_counts, before_counts);
    assert!(stack.name_cache_matches_stack());
}

#[test]
fn deterministic_stack_mutations_keep_expanded_name_counts_exact() {
    let mut ctx = DocumentParseContext::new();
    let names = ["div", "span", "table", "foreignObject"]
        .map(|name| ctx.atoms.intern_exact(name).expect("operation atom"));
    let mut stack = OpenElementsStack::default();
    let mut next_key = 1_u32;

    for step in 0..512_usize {
        match step % 5 {
            0 | 1 => {
                let name = names[step % names.len()];
                let namespace = if step % 7 == 0 {
                    crate::ElementNamespace::Svg
                } else {
                    crate::ElementNamespace::Html
                };
                let entry = if namespace == crate::ElementNamespace::Html {
                    OpenElement::new_html(PatchKey(next_key), name)
                } else {
                    OpenElement::new_foreign(PatchKey(next_key), namespace, name)
                };
                stack.push(entry);
                next_key += 1;
            }
            2 if !stack.is_empty() => {
                let _ = stack.pop();
            }
            3 if stack.len() > 1 => {
                let name = names[(step + 1) % names.len()];
                let replacement = OpenElement::new_html(PatchKey(next_key), name);
                let _ = stack.replace_at(stack.len() / 2, replacement);
                next_key += 1;
            }
            4 if stack.len() > 2 => {
                let _ = stack.remove_at(stack.len() / 3);
            }
            _ => {}
        }
        assert!(stack.name_cache_matches_stack(), "step={step}");
    }
}

#[test]
#[should_panic(expected = "SOE name count missing for popped element")]
fn missing_name_count_is_a_hard_invariant_failure() {
    let mut ctx = DocumentParseContext::new();
    let div = ctx.atoms.intern_exact("div").expect("atom");
    let mut stack = OpenElementsStack::default();
    stack.push(OpenElement::new_html(PatchKey(1), div));
    stack.name_counts.clear();
    let _ = stack.pop();
}

#[test]
#[should_panic(expected = "SOE name count underflow")]
fn zero_name_count_is_a_hard_invariant_failure() {
    let mut ctx = DocumentParseContext::new();
    let div = ctx.atoms.intern_exact("div").expect("atom");
    let mut stack = OpenElementsStack::default();
    stack.push(OpenElement::new_html(PatchKey(1), div));
    stack.name_counts[0].1 = 0;
    let _ = stack.pop();
}
