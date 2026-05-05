use super::super::Tab;
use super::support::{
    current_element_color_by_id, find_styled_node_id, initial_patch_document,
    two_paragraph_patch_document,
};
use crate::page::StyleRecalcKind;
use bus::CoreEvent;
use core_types::{DomHandle, DomVersion};
use css::ComputedStyleReuseStats;
use html::{DomPatch, PatchKey, internal::Id};
use std::sync::Arc;

#[test]
fn attribute_mutation_without_existing_style_cache_falls_back_to_full_recompute() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 28;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(280);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 28,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: initial_patch_document(".hot { color: red; } p { color: black; }", Some("p")),
    });

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 28,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![DomPatch::SetAttributes {
            key: PatchKey(7),
            attributes: vec![(Arc::from("class"), Some("hot".to_string()))],
        }],
    });

    assert_eq!(
        current_element_color_by_id(&mut tab, Id(7)),
        (255, 0, 0, 255)
    );
    assert_eq!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::Full { elements: 5 }),
        "partial suffix reuse requires a validated previous style cache"
    );
}

#[test]
fn clean_style_cache_reuses_computed_document_without_recompute() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 29;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(290);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 29,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: two_paragraph_patch_document("p { color: red; }"),
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
            (255, 0, 0, 255)
        );
        assert_eq!(
            find_styled_node_id(style_output.root(), Id(9))
                .expect("second paragraph")
                .style
                .color(),
            (255, 0, 0, 255)
        );
    }
    assert_eq!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::Full { elements: 6 })
    );
    assert!(
        tab.page
            .last_style_reuse()
            .is_some_and(|stats| stats.hits > 0),
        "initial full pass should share identical sibling computed styles"
    );
    let before = tab.page.style_generations();

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
            (255, 0, 0, 255)
        );
    }

    assert_eq!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::ReusedCache),
        "clean style inputs should reuse PageState's cached computed document"
    );
    assert_eq!(tab.page.style_generations(), before);
    assert_eq!(
        tab.page.last_style_reuse(),
        Some(ComputedStyleReuseStats { hits: 0, misses: 0 }),
        "no per-pass sharing work should run when the page cache is reused"
    );
}
