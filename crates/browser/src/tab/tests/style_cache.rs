use super::super::Tab;
use super::support::{
    current_element_color_by_id, find_styled_node_id, initial_patch_document,
    no_quirks_patch_publication, two_paragraph_patch_document,
};
use crate::page::{
    StyleRecalcKind, dependency_artifact_build_count, reset_rule_collection_build_count,
    rule_collection_build_count, style_execution_build_count,
};
use crate::rendering::RetainedStyleArtifactAction;
use bus::CoreEvent;
use core_types::{DomHandle, DomVersion};
use css::ComputedStyleReuseStats;
use html::{DomPatch, PatchKey, internal::Id};

#[test]
fn attribute_mutation_without_existing_style_cache_falls_back_to_full_recompute() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 28;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(280);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 28,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document(".hot { color: red; } p { color: black; }", Some("p")),
        ),
    });

    assert_eq!(current_element_color_by_id(&mut tab, Id(7)), (0, 0, 0, 255));
    tab.page.clear_style_cache_for_tests();
    reset_rule_collection_build_count();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 28,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            vec![DomPatch::SetAttributes {
                key: PatchKey(7),
                attributes: vec![html::internal::unqualified_attribute("class", "hot")],
            }],
        ),
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
    assert_eq!(
        tab.page
            .retained_render_state_debug_snapshot()
            .style_artifacts
            .last_action,
        RetainedStyleArtifactAction::FallbackFullRecompute,
        "CSS suffix eligibility must remain distinguishable from the runtime full fallback"
    );
    assert_eq!(
        rule_collection_build_count(),
        1,
        "incremental-unavailable execution and full fallback share one collection"
    );
    assert_eq!(
        style_execution_build_count(),
        1,
        "incremental-unavailable execution and full fallback share one selector DOM"
    );
}

#[test]
fn keyed_class_id_and_attribute_dependencies_authorize_suffix_recompute() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 291;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(2910);
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 291,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document(
                ".hot { color: red; } #hero { color: blue; } [data-kind=promo] { color: green; }",
                Some("p"),
            ),
        ),
    });
    assert_eq!(current_element_color_by_id(&mut tab, Id(7)), (0, 0, 0, 255));
    let dependencies = tab
        .page
        .style_dependency_debug_snapshot()
        .expect("retained CSS dependency summary");
    assert!(dependencies.starts_with("version: 1\naf9-style-dependencies\n"));
    assert!(dependencies.contains("trigger=id(\"hero\")"));
    assert!(dependencies.contains("trigger=class(\"hot\")"));
    assert!(dependencies.contains("trigger=attribute-html(\"data-kind\" = \"promo\")"));

    let cases = [
        ("class", "hot", (255, 0, 0, 255)),
        ("id", "hero", (0, 0, 255, 255)),
        ("data-kind", "promo", (0, 128, 0, 255)),
    ];
    for (index, (name, value, expected)) in cases.into_iter().enumerate() {
        let from = DomVersion(1 + index as u64);
        let to = DomVersion(2 + index as u64);
        tab.on_core_event(CoreEvent::DomPatchUpdate {
            tab_id: tab.tab_id,
            request_id: 291,
            publication: no_quirks_patch_publication(
                handle,
                from,
                to,
                vec![DomPatch::SetAttributes {
                    key: PatchKey(7),
                    attributes: vec![html::internal::unqualified_attribute(name, value)],
                }],
            ),
        });
        assert_eq!(current_element_color_by_id(&mut tab, Id(7)), expected);
        assert!(matches!(
            tab.page.last_style_recalc(),
            Some(StyleRecalcKind::IncrementalSuffix { .. })
        ));
        let decision = tab
            .page
            .last_style_invalidation_decision_debug_snapshot()
            .expect("CSS decision snapshot");
        assert!(decision.contains("reason: selector-dependency-matched"));
        assert!(decision.contains("selected-plan: scope: document-suffix node-ids: [7]"));
    }
}

#[test]
fn irrelevant_attribute_change_reuses_computed_style_without_style_generation_change() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 292;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(2920);
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 292,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document(".hot { color: red; }", Some("p")),
        ),
    });
    assert_eq!(current_element_color_by_id(&mut tab, Id(7)), (0, 0, 0, 255));
    let before = tab.page.style_generations();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 292,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            vec![DomPatch::SetAttributes {
                key: PatchKey(7),
                attributes: vec![html::internal::unqualified_attribute("title", "neutral")],
            }],
        ),
    });

    assert_eq!(
        tab.page.style_generations().style_inputs,
        before.style_inputs
    );
    assert_eq!(current_element_color_by_id(&mut tab, Id(7)), (0, 0, 0, 255));
    assert_eq!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::ReusedCache)
    );
    assert!(
        tab.page
            .last_style_invalidation_decision_debug_snapshot()
            .is_some_and(|snapshot| snapshot.contains("reason: no-style-effect"))
    );
}

#[test]
fn inline_style_is_a_direct_cascade_dependency_without_style_selector() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 293;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(2930);
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 293,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document("p { color: black; }", Some("p")),
        ),
    });
    assert_eq!(current_element_color_by_id(&mut tab, Id(7)), (0, 0, 0, 255));

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 293,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            vec![DomPatch::SetAttributes {
                key: PatchKey(7),
                attributes: vec![html::internal::unqualified_attribute("style", "color: red")],
            }],
        ),
    });

    assert_eq!(
        current_element_color_by_id(&mut tab, Id(7)),
        (255, 0, 0, 255)
    );
    assert!(
        tab.page
            .last_style_invalidation_decision_debug_snapshot()
            .is_some_and(|snapshot| {
                snapshot.contains("reason: inline-cascade-input-changed")
                    && snapshot.contains("inline-style-changed: true")
            })
    );
}

#[test]
fn structural_full_recompute_reuses_compatible_dependency_artifact() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 294;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(2940);
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 294,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document("body > p:first-child { color: red; }", Some("p")),
        ),
    });
    assert_eq!(
        current_element_color_by_id(&mut tab, Id(7)),
        (255, 0, 0, 255)
    );
    reset_rule_collection_build_count();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 294,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            vec![
                DomPatch::CreateElement {
                    key: PatchKey(9),
                    name: html::internal::html_name("div"),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(6),
                    child: PatchKey(9),
                },
            ],
        ),
    });
    let _ = current_element_color_by_id(&mut tab, Id(7));
    assert!(matches!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::Full { .. })
    ));
    assert_eq!(dependency_artifact_build_count(), 0);
    assert!(
        tab.page
            .last_style_invalidation_decision_debug_snapshot()
            .is_some_and(
                |snapshot| snapshot.contains("reason: structural-mutation-requires-full-rebuild")
            )
    );
}

#[test]
fn multiple_attribute_operations_classify_committed_old_to_final_state() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 295;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(2950);
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 295,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document(".hot { color: red; }", Some("p")),
        ),
    });
    assert_eq!(current_element_color_by_id(&mut tab, Id(7)), (0, 0, 0, 255));
    let before = tab.page.style_generations();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 295,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            vec![
                DomPatch::SetAttributes {
                    key: PatchKey(7),
                    attributes: vec![html::internal::unqualified_attribute("class", "hot")],
                },
                DomPatch::SetAttributes {
                    key: PatchKey(7),
                    attributes: Vec::new(),
                },
            ],
        ),
    });

    assert_eq!(
        tab.page.style_generations().style_inputs,
        before.style_inputs
    );
    assert!(
        tab.page.last_dom_mutation_debug_snapshot().is_some_and(
            |snapshot| snapshot.contains("node=7 namespace=html before=0 after=0 no-op=true")
        )
    );
    assert!(
        tab.page
            .last_style_invalidation_decision_debug_snapshot()
            .is_some_and(|snapshot| snapshot.contains("reason: no-style-effect"))
    );
}

#[test]
fn text_without_empty_dependency_skips_css_style_but_keeps_intrinsic_work() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 296;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(2960);
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 296,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document("p { color: black; }", Some("p")),
        ),
    });
    assert_eq!(current_element_color_by_id(&mut tab, Id(7)), (0, 0, 0, 255));
    let before = tab.page.style_generations();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 296,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            vec![DomPatch::SetText {
                key: PatchKey(8),
                text: "World".to_string(),
            }],
        ),
    });

    assert_eq!(
        tab.page.style_generations().style_inputs,
        before.style_inputs
    );
    assert!(tab.pending_render_work.requests().iter().any(|request| {
        request.entry_point() == crate::rendering::RenderInvalidationEntryPoint::DomTextChanged
    }));
    assert!(
        tab.page
            .last_style_invalidation_decision_debug_snapshot()
            .is_some_and(|snapshot| snapshot.contains("reason: no-style-effect"))
    );
    let _ = current_element_color_by_id(&mut tab, Id(7));
    assert_eq!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::ReusedCache)
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
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            two_paragraph_patch_document("p { color: red; }"),
        ),
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
    assert_eq!(
        tab.page
            .retained_render_state_debug_snapshot()
            .style_artifacts
            .last_action,
        RetainedStyleArtifactAction::Reused
    );
    assert_eq!(
        tab.page
            .retained_render_state_debug_snapshot()
            .style_artifacts
            .stats
            .reuse_count,
        1
    );
}
