use super::super::Tab;
use super::support::{
    FixedTextMeasurer, current_element_color, current_element_color_by_id,
    current_element_color_optional, find_styled_element, find_styled_node_id,
    initial_patch_document, two_paragraph_patch_document,
};
use crate::page::{RestyleTrigger, StyleRecalcKind};
use bus::CoreEvent;
use core_types::{DomHandle, DomVersion};
use css::Display;
use html::{DomPatch, PatchKey, internal::Id};
use layout::{LayoutPhaseInput, layout_document};

#[test]
fn dom_patch_attribute_change_triggers_restyle_through_computed_cache() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 19;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(190);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 19,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: initial_patch_document(".hot { color: red; } p { color: black; }", Some("p")),
    });

    assert_eq!(
        tab.page.last_restyle_trigger(),
        Some(RestyleTrigger::DocumentReplaced)
    );
    assert_eq!(current_element_color(&mut tab, "p"), (0, 0, 0, 255));
    let after_initial = tab.page.style_generations();
    assert!(!tab.page.style_dirty());

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 19,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![DomPatch::SetAttributes {
            key: PatchKey(7),
            attributes: vec![html::internal::unqualified_attribute("class", "hot")],
        }],
    });

    assert!(
        tab.page.style_dirty(),
        "attribute mutation must mark style dirty before restyle"
    );
    assert_eq!(
        tab.page.last_restyle_trigger(),
        Some(RestyleTrigger::AttributesChanged)
    );
    assert_eq!(tab.page.style_generations().dom, after_initial.dom + 1);
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    assert_eq!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::IncrementalSuffix {
            reused_prefix_len: 4,
            recomputed_len: 1,
        }),
        "attribute mutation on the last element should reuse the computed prefix"
    );
    assert!(
        !tab.page.style_dirty(),
        "style cache should be clean after recomputation"
    );
}

#[test]
fn dom_patch_node_insertion_triggers_restyle_for_inserted_subtree() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 20;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(200);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 20,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: initial_patch_document("span { color: blue; }", None),
    });

    assert!(
        current_element_color_optional(&mut tab, "span").is_none(),
        "initial document has no span"
    );

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 20,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![
            DomPatch::CreateElement {
                key: PatchKey(9),
                name: html::internal::html_name("span"),
                attributes: Vec::new(),
            },
            DomPatch::CreateText {
                key: PatchKey(10),
                text: "Inserted".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(9),
                child: PatchKey(10),
            },
            DomPatch::AppendChild {
                parent: PatchKey(6),
                child: PatchKey(9),
            },
        ],
    });

    assert!(
        tab.page.style_dirty(),
        "node insertion must mark style dirty before restyle"
    );
    assert_eq!(
        tab.page.last_restyle_trigger(),
        Some(RestyleTrigger::TreeMutated)
    );
    assert_eq!(
        current_element_color(&mut tab, "span"),
        (0, 0, 255, 255),
        "inserted element should receive computed style from existing stylesheet"
    );
    assert_eq!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::Full { elements: 5 }),
        "structural mutations must not use suffix reuse while selector ids can shift"
    );
}

#[test]
fn dom_patch_node_removal_triggers_restyle_and_removes_styled_node() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 21;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(210);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 21,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: initial_patch_document("p { color: red; }", Some("p")),
    });

    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 21,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![DomPatch::RemoveNode { key: PatchKey(7) }],
    });

    assert!(
        tab.page.style_dirty(),
        "node removal must mark style dirty before restyle"
    );
    assert_eq!(
        tab.page.last_restyle_trigger(),
        Some(RestyleTrigger::TreeMutated)
    );
    assert!(
        current_element_color_optional(&mut tab, "p").is_none(),
        "removed element must not remain in the rebuilt styled tree"
    );
}

#[test]
fn dom_patch_style_text_change_reconciles_stylesheet_slot_and_restyles() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 22;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(220);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 22,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: initial_patch_document("p { color: red; }", Some("p")),
    });

    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    let before = tab.page.style_generations();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 22,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![DomPatch::SetText {
            key: PatchKey(5),
            text: "p { color: blue; }".to_string(),
        }],
    });

    let after = tab.page.style_generations();
    assert_eq!(after.dom, before.dom + 1);
    assert_eq!(
        tab.page.last_restyle_trigger(),
        Some(RestyleTrigger::TextMutated)
    );
    assert_eq!(
        after.style_inputs, before.style_inputs,
        "style text changes should invalidate through stylesheet generation"
    );
    assert_eq!(
        after.stylesheets,
        before.stylesheets + 1,
        "style text mutation must update the document stylesheet generation"
    );
    assert_eq!(current_element_color(&mut tab, "p"), (0, 0, 255, 255));

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 22,
        handle,
        from: DomVersion(2),
        to: DomVersion(3),
        patches: vec![DomPatch::SetText {
            key: PatchKey(5),
            text: "p { display: none; }".to_string(),
        }],
    });

    let style_output = tab
        .page
        .build_style_phase_output()
        .expect("style phase output should build")
        .expect("document should be styled");
    let paragraph = find_styled_element(style_output.root(), "p").expect("p styled node");
    assert_eq!(
        paragraph.style.display(),
        Display::None,
        "style text mutation must invalidate style before reuse is allowed"
    );

    let measurer = FixedTextMeasurer;
    let layout_output = layout_document(LayoutPhaseInput::from_style_output(
        &style_output,
        320.0,
        &measurer,
        None,
    ));
    assert!(
        !layout_output
            .to_debug_snapshot()
            .contains("node=element(\"p\")"),
        "style text mutation to display:none must remove the paragraph from layout"
    );
}

#[test]
fn dom_patch_attribute_change_incrementally_restyles_following_sibling_suffix() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 26;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(260);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 26,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: two_paragraph_patch_document(".hot ~ p { color: blue; } p { color: black; }"),
    });

    assert_eq!(current_element_color_by_id(&mut tab, Id(9)), (0, 0, 0, 255));

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 26,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![DomPatch::SetAttributes {
            key: PatchKey(7),
            attributes: vec![html::internal::unqualified_attribute("class", "hot")],
        }],
    });

    {
        let style_output = tab
            .page
            .build_style_phase_output()
            .expect("style phase output should build")
            .expect("document should be styled");
        assert_eq!(
            find_styled_node_id(style_output.root(), Id(7))
                .expect("first paragraph")
                .style
                .color(),
            (0, 0, 0, 255)
        );
        assert_eq!(
            find_styled_node_id(style_output.root(), Id(9))
                .expect("second paragraph")
                .style
                .color(),
            (0, 0, 255, 255),
            "suffix restyle must include following siblings affected by sibling selectors"
        );
    }
    assert_eq!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::IncrementalSuffix {
            reused_prefix_len: 4,
            recomputed_len: 2,
        }),
        "first paragraph mutation should reuse html/head/style/body and recompute both paragraphs"
    );
}

#[test]
fn queued_attribute_mutations_merge_to_earliest_dirty_suffix() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 27;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(270);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 27,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: two_paragraph_patch_document(
            ".hot { color: red; } .cool { color: blue; } p { color: black; }",
        ),
    });

    assert_eq!(current_element_color_by_id(&mut tab, Id(7)), (0, 0, 0, 255));
    assert_eq!(current_element_color_by_id(&mut tab, Id(9)), (0, 0, 0, 255));

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 27,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![DomPatch::SetAttributes {
            key: PatchKey(7),
            attributes: vec![html::internal::unqualified_attribute("class", "hot")],
        }],
    });
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 27,
        handle,
        from: DomVersion(2),
        to: DomVersion(3),
        patches: vec![DomPatch::SetAttributes {
            key: PatchKey(9),
            attributes: vec![html::internal::unqualified_attribute("class", "cool")],
        }],
    });

    {
        let style_output = tab
            .page
            .build_style_phase_output()
            .expect("style phase output should build")
            .expect("document should be styled");
        assert_eq!(
            find_styled_node_id(style_output.root(), Id(7))
                .expect("first paragraph")
                .style
                .color(),
            (255, 0, 0, 255),
            "first queued attribute mutation must not be lost"
        );
        assert_eq!(
            find_styled_node_id(style_output.root(), Id(9))
                .expect("second paragraph")
                .style
                .color(),
            (0, 0, 255, 255),
            "second queued attribute mutation must also apply"
        );
    }
    assert_eq!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::IncrementalSuffix {
            reused_prefix_len: 4,
            recomputed_len: 2,
        }),
        "merged pending suffix must start at the earliest queued dirty element"
    );
}

#[test]
fn dom_patch_normal_text_change_dirties_layout_but_reuses_computed_style() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 23;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(230);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 23,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: initial_patch_document("p { color: red; }", Some("p")),
    });

    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    assert!(!tab.page.style_dirty());
    tab.page.clear_layout_dirty_for_tests();
    let before = tab.page.style_generations();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 23,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![DomPatch::SetText {
            key: PatchKey(8),
            text: "Goodbye".to_string(),
        }],
    });

    let after = tab.page.style_generations();
    assert_eq!(
        tab.page.last_restyle_trigger(),
        Some(RestyleTrigger::TextMutated)
    );
    assert_eq!(after.dom, before.dom + 1);
    assert_eq!(
        after.style_inputs, before.style_inputs,
        "normal text changes must not invalidate selector/cascade inputs"
    );
    assert_eq!(
        after.stylesheets, before.stylesheets,
        "normal text changes must not reconcile a new stylesheet set"
    );
    assert!(
        !tab.page.style_dirty(),
        "normal text changes should reuse cached computed style"
    );
    assert!(
        tab.page.layout_dirty(),
        "normal text changes still require downstream layout work"
    );
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
}

#[test]
fn empty_dom_patch_batch_does_not_trigger_restyle() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 25;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(250);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 25,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: initial_patch_document("p { color: red; }", Some("p")),
    });

    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    assert!(!tab.page.style_dirty());
    let before = tab.page.style_generations();
    let previous_trigger = tab.page.last_restyle_trigger();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 25,
        handle,
        from: DomVersion(1),
        to: DomVersion(1),
        patches: Vec::new(),
    });

    assert_eq!(
        tab.page.style_generations(),
        before,
        "empty patch batches must not advance DOM or style generations"
    );
    assert_eq!(
        tab.page.last_restyle_trigger(),
        previous_trigger,
        "empty patch batches must not record a synthetic restyle trigger"
    );
    assert!(
        !tab.page.style_dirty(),
        "empty patch batches must not invalidate cached computed style"
    );
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
}
