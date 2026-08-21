use super::super::Tab;
use super::support::{
    FixedTextMeasurer, current_element_color, current_element_color_by_id,
    current_element_color_optional, find_styled_element, find_styled_node_id,
    initial_patch_document, no_quirks_patch_publication, two_paragraph_patch_document,
};
use crate::page::StyleRecalcKind;
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
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document(".hot { color: red; } p { color: black; }", Some("p")),
        ),
    });

    assert!(
        tab.page
            .last_dom_mutation_facts()
            .is_some_and(|facts| facts.document_replaced())
    );
    assert_eq!(current_element_color(&mut tab, "p"), (0, 0, 0, 255));
    let after_initial = tab.page.style_generations();
    assert!(!tab.page.style_dirty());

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 19,
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

    assert!(
        tab.page.style_dirty(),
        "attribute mutation must mark style dirty before restyle"
    );
    assert!(
        tab.page
            .last_dom_mutation_facts()
            .is_some_and(|facts| facts.attributes().changed())
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
fn same_handle_clear_uses_neutral_replacement_fact_for_retained_identity_boundary() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 1901;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(1901);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 1901,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document("p { color: red; }", Some("p")),
        ),
    });

    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    let retained_before = tab.page.retained_render_state_debug_snapshot();
    let identity_before = retained_before
        .retained_identities
        .iter()
        .copied()
        .find(|identity| identity.anchor == crate::rendering::RetainedRenderAnchor::DomNode(Id(7)))
        .expect("initial paragraph retained identity");
    assert_eq!(
        retained_before
            .style_artifacts
            .key
            .expect("initial retained style artifact")
            .identity_domain,
        retained_before.retained_identity_domain
    );
    let generations_before = tab.page.style_generations();
    tab.clear_render_orchestration_state();
    tab.page.clear_all_dirty_for_tests();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 1901,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            initial_patch_document("p { color: blue; }", Some("p")),
        ),
    });

    let facts = tab
        .page
        .last_dom_mutation_facts()
        .expect("same-handle Clear publication facts");
    assert!(facts.document_replaced());
    let retained_after_publication = tab.page.retained_render_state_debug_snapshot();
    let identity_after = retained_after_publication
        .retained_identities
        .iter()
        .copied()
        .find(|identity| identity.anchor == crate::rendering::RetainedRenderAnchor::DomNode(Id(7)))
        .expect("replacement paragraph retained identity");
    assert_eq!(identity_after.anchor, identity_before.anchor);
    assert_eq!(identity_after.id, identity_before.id);
    assert_ne!(
        retained_after_publication.retained_identity_domain,
        retained_before.retained_identity_domain,
        "equal local DOM and retained-render numbers must not imply continuity across Clear"
    );
    assert_eq!(retained_after_publication.style_artifacts.key, None);
    assert_eq!(
        tab.page.style_generations().style_inputs,
        generations_before.style_inputs + 1,
        "the replacement publication must be classified and applied once by CSS"
    );

    let entry_points = tab
        .pending_render_work
        .requests()
        .iter()
        .map(|request| request.entry_point())
        .collect::<Vec<_>>();
    assert_eq!(
        entry_points
            .iter()
            .filter(|entry_point| {
                **entry_point == crate::rendering::RenderInvalidationEntryPoint::DocumentReplaced
            })
            .count(),
        1
    );
    assert_eq!(
        entry_points
            .iter()
            .filter(|entry_point| {
                **entry_point
                    == crate::rendering::RenderInvalidationEntryPoint::DomPublicationStyleInvalidated
            })
            .count(),
        1
    );

    assert_eq!(current_element_color(&mut tab, "p"), (0, 0, 255, 255));
    let retained_after_recompute = tab.page.retained_render_state_debug_snapshot();
    assert_eq!(
        retained_after_recompute
            .style_artifacts
            .key
            .expect("replacement retained style artifact")
            .identity_domain,
        retained_after_recompute.retained_identity_domain
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
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document("span { color: blue; }", None),
        ),
    });

    assert!(
        current_element_color_optional(&mut tab, "span").is_none(),
        "initial document has no span"
    );

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 20,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            vec![
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
        ),
    });

    assert!(
        tab.page.style_dirty(),
        "node insertion must mark style dirty before restyle"
    );
    assert!(
        tab.page
            .last_dom_mutation_facts()
            .is_some_and(|facts| facts.tree_topology_or_order_operation())
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
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document("p { color: red; }", Some("p")),
        ),
    });

    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 21,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            vec![DomPatch::RemoveNode { key: PatchKey(7) }],
        ),
    });

    assert!(
        tab.page.style_dirty(),
        "node removal must mark style dirty before restyle"
    );
    assert!(
        tab.page
            .last_dom_mutation_facts()
            .is_some_and(|facts| facts.tree_topology_or_order_operation())
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
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document("p { color: red; }", Some("p")),
        ),
    });

    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    let before = tab.page.style_generations();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 22,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            vec![DomPatch::SetText {
                key: PatchKey(5),
                text: "p { color: blue; }".to_string(),
            }],
        ),
    });

    let after = tab.page.style_generations();
    assert_eq!(after.dom, before.dom + 1);
    assert!(
        tab.page
            .last_dom_mutation_facts()
            .is_some_and(|facts| facts.text().changed())
    );
    assert_eq!(
        after.style_inputs,
        before.style_inputs + 1,
        "CSS must authorize a style-input generation advance for the text mutation"
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
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(2),
            DomVersion(3),
            vec![DomPatch::SetText {
                key: PatchKey(5),
                text: "p { display: none; }".to_string(),
            }],
        ),
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
fn dom_patch_style_media_change_invalidates_stylesheet_generation() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 2201;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(2201);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 2201,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document("p { color: red; }", Some("p")),
        ),
    });
    let before = tab.page.style_generations();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 2201,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            vec![DomPatch::SetAttributes {
                key: PatchKey(4),
                attributes: vec![html::internal::unqualified_attribute("media", "screen")],
            }],
        ),
    });

    assert_eq!(
        tab.page.style_generations().stylesheets,
        before.stylesheets + 1
    );
    assert!(tab.page.style_dirty());
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
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            two_paragraph_patch_document(".hot ~ p { color: blue; } p { color: black; }"),
        ),
    });

    assert_eq!(current_element_color_by_id(&mut tab, Id(9)), (0, 0, 0, 255));

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 26,
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
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            two_paragraph_patch_document(
                ".hot { color: red; } .cool { color: blue; } p { color: black; }",
            ),
        ),
    });

    assert_eq!(current_element_color_by_id(&mut tab, Id(7)), (0, 0, 0, 255));
    assert_eq!(current_element_color_by_id(&mut tab, Id(9)), (0, 0, 0, 255));

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 27,
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
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 27,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(2),
            DomVersion(3),
            vec![DomPatch::SetAttributes {
                key: PatchKey(9),
                attributes: vec![html::internal::unqualified_attribute("class", "cool")],
            }],
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
fn dom_patch_normal_text_change_conservatively_restyles_and_dirties_layout() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 23;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(230);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 23,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document("p { color: red; }", Some("p")),
        ),
    });

    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    assert!(!tab.page.style_dirty());
    tab.page.clear_layout_dirty_for_tests();
    let before = tab.page.style_generations();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 23,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            vec![DomPatch::SetText {
                key: PatchKey(8),
                text: "Goodbye".to_string(),
            }],
        ),
    });

    let after = tab.page.style_generations();
    assert!(
        tab.page
            .last_dom_mutation_facts()
            .is_some_and(|facts| facts.text().changed())
    );
    assert_eq!(after.dom, before.dom + 1);
    assert_eq!(
        after.style_inputs,
        before.style_inputs + 1,
        "AF4d conservatively invalidates style because text can change :empty matching"
    );
    assert_eq!(
        after.stylesheets, before.stylesheets,
        "normal text changes must not reconcile a new stylesheet set"
    );
    assert!(
        tab.page.style_dirty(),
        "CSS-authorized text invalidation must schedule style work"
    );
    assert!(
        tab.page.layout_dirty(),
        "normal text changes still require downstream layout work"
    );
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
}

#[test]
fn published_text_mutation_restyles_retained_empty_selector_without_losing_layout_cause() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 230;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(2300);
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 230,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document("p { color: black; } p:empty { color: red; }", Some("p")),
        ),
    });

    assert_eq!(current_element_color(&mut tab, "p"), (0, 0, 0, 255));
    let retained_before = tab.page.retained_render_state_debug_snapshot();
    assert_eq!(
        retained_before.computed_styles,
        crate::rendering::RenderArtifactState::RetainedFresh
    );
    tab.page.clear_layout_dirty_for_tests();
    let before = tab.page.style_generations();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 230,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            vec![DomPatch::SetText {
                key: PatchKey(8),
                text: String::new(),
            }],
        ),
    });

    let after = tab.page.style_generations();
    assert_eq!(after.style_inputs, before.style_inputs + 1);
    assert!(tab.page.style_dirty());
    assert!(tab.page.layout_dirty());
    let entry_points = tab
        .pending_render_work
        .requests()
        .iter()
        .map(|request| request.entry_point())
        .collect::<Vec<_>>();
    assert!(entry_points.contains(&crate::rendering::RenderInvalidationEntryPoint::DomTextChanged));
    assert_eq!(
        entry_points
            .iter()
            .filter(|entry| {
                **entry
                    == crate::rendering::RenderInvalidationEntryPoint::DomPublicationStyleInvalidated
            })
            .count(),
        1
    );
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    assert_eq!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::Full { elements: 5 })
    );
}

#[test]
fn browser_selector_debug_uses_the_bounded_authoritative_css_surface() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 232;
    tab.page.start_nav("https://example.com/index.html");
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 232,
        publication: no_quirks_patch_publication(
            DomHandle(2320),
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document("p, :hover, > p {}", Some("p")),
        ),
    });

    let diagnostic = tab
        .page
        .selector_matching_debug_snapshot(css::DocumentSelectorMatchingDiagnosticLimits {
            max_elements: 0,
            ..Default::default()
        })
        .expect("selector diagnostic input construction succeeds")
        .expect("published document has a selector diagnostic");
    assert!(matches!(
        diagnostic.failure(),
        Some(
            css::DocumentSelectorMatchingDiagnosticFailure::LimitExceeded {
                limit: css::DocumentSelectorMatchingDiagnosticLimit::Elements,
                ..
            }
        )
    ));
    assert!(
        diagnostic
            .to_debug_snapshot()
            .contains("status: failed\nfailure: kind=limit-exceeded limit=elements")
    );
}

#[test]
fn browser_rule_collection_debug_uses_production_handoff_and_bounded_af5_surface() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 233;
    tab.page.start_nav("https://example.com/index.html");
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 233,
        publication: no_quirks_patch_publication(
            DomHandle(2330),
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document("p { color: red !important; }", Some("p")),
        ),
    });

    let diagnostic = tab
        .page
        .rule_collection_debug_snapshot(
            &css::StyleResolutionLimits::default(),
            css::RuleCollectionDiagnosticLimits::default(),
        )
        .expect("AF5 diagnostic input construction succeeds")
        .expect("published document has an AF5 diagnostic");
    assert!(diagnostic.failure().is_none());
    assert!(diagnostic.records().iter().any(|record| matches!(
        record,
        css::RuleCollectionDiagnosticRecord::Declaration {
            importance: css::CascadeImportance::Important,
            ..
        }
    )));

    let bounded = tab
        .page
        .rule_collection_debug_snapshot(
            &css::StyleResolutionLimits::default(),
            css::RuleCollectionDiagnosticLimits {
                max_records: 0,
                ..Default::default()
            },
        )
        .expect("bounded AF5 diagnostic input construction succeeds")
        .expect("published document has a bounded AF5 diagnostic");
    assert!(matches!(
        bounded.failure(),
        Some(css::RuleCollectionDiagnosticFailure::LimitExceeded {
            limit: css::RuleCollectionDiagnosticLimit::Records,
            ..
        })
    ));
}

#[test]
fn mixed_attribute_and_text_publication_preserves_both_identities_and_one_css_authorization() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 231;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(2310);
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 231,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document(
                "p { color: black; } p.hot:empty { color: blue; }",
                Some("p"),
            ),
        ),
    });
    assert_eq!(current_element_color(&mut tab, "p"), (0, 0, 0, 255));
    tab.page.clear_layout_dirty_for_tests();
    let before = tab.page.style_generations();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 231,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            vec![
                DomPatch::SetAttributes {
                    key: PatchKey(7),
                    attributes: vec![html::internal::unqualified_attribute("class", "hot")],
                },
                DomPatch::SetText {
                    key: PatchKey(8),
                    text: String::new(),
                },
            ],
        ),
    });

    let facts = tab
        .page
        .last_dom_mutation_facts()
        .expect("mixed neutral publication facts");
    assert!(facts.attributes().changed());
    assert_eq!(facts.attributes().live_node_ids(), [Id(7)]);
    assert_eq!(facts.attributes().historical_target_count(), 0);
    assert!(facts.text().changed());
    assert_eq!(facts.text().live_node_ids(), [Id(8)]);
    assert_eq!(facts.text().historical_target_count(), 0);
    assert_eq!(
        tab.page.style_generations().style_inputs,
        before.style_inputs + 1
    );
    let requests = tab.pending_render_work.requests();
    assert_eq!(
        requests
            .iter()
            .filter(|request| {
                request.entry_point()
                    == crate::rendering::RenderInvalidationEntryPoint::DomPublicationStyleInvalidated
            })
            .count(),
        1
    );
    let text = requests
        .iter()
        .find(|request| {
            request.entry_point() == crate::rendering::RenderInvalidationEntryPoint::DomTextChanged
        })
        .expect("text intrinsic request survives mixed publication");
    assert_eq!(
        text.requested_work().style(),
        crate::rendering::PhaseRerunSource::None
    );
    assert_eq!(
        text.requested_work().layout(),
        crate::rendering::PhaseRerunSource::Direct(
            crate::rendering::RenderRebuildTrigger::DomTextChanged
        )
    );
    assert_eq!(current_element_color(&mut tab, "p"), (0, 0, 255, 255));
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
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document("p { color: red; }", Some("p")),
        ),
    });

    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    assert!(!tab.page.style_dirty());
    let before = tab.page.style_generations();
    let previous_facts = tab.page.last_dom_mutation_facts().cloned();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 25,
        publication: no_quirks_patch_publication(handle, DomVersion(1), DomVersion(1), Vec::new()),
    });

    assert_eq!(
        tab.page.style_generations(),
        before,
        "empty patch batches must not advance DOM or style generations"
    );
    assert_eq!(
        tab.page.last_dom_mutation_facts(),
        previous_facts.as_ref(),
        "empty patch batches must not record a synthetic restyle trigger"
    );
    assert!(
        !tab.page.style_dirty(),
        "empty patch batches must not invalidate cached computed style"
    );
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
}

#[test]
fn invalid_publication_preserves_committed_browser_state_and_reason() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 27;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(270);
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 27,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document("p { color: red; }", Some("p")),
        ),
    });
    let before_dom = tab.page.dom.as_ref().map(|dom| format!("{dom:?}"));
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 27,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            vec![DomPatch::SetAttributes {
                key: PatchKey(9999),
                attributes: Vec::new(),
            }],
        ),
    });
    assert_eq!(tab.dom_handle, Some(handle));
    assert_eq!(tab.dom_version, DomVersion(1));
    assert_eq!(
        tab.page.dom.as_ref().map(|dom| format!("{dom:?}")),
        before_dom
    );
    assert!(
        tab.last_status
            .as_deref()
            .unwrap_or_default()
            .contains("InvalidPayload")
    );
}

#[test]
fn staged_identity_resolution_failure_rolls_back_the_complete_publication() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 271;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(2710);
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 271,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial_patch_document("p { color: red; }", Some("p")),
        ),
    });
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    let version_before = tab.dom_version;
    let handle_before = tab.dom_handle;
    let outline_before = tab.page.outline(100);
    let pipeline_before = tab.page.render_pipeline_debug_snapshot();
    let retained_before = tab.page.retained_render_state_debug_snapshot();
    let facts_before = tab.page.last_dom_mutation_facts().cloned();
    let pending_before = tab.pending_render_work.clone();

    let failure = tab
        .commit_document_publication_with_forced_identity_failure_for_tests(
            no_quirks_patch_publication(
                handle,
                DomVersion(1),
                DomVersion(2),
                vec![DomPatch::SetText {
                    key: PatchKey(8),
                    text: "must not publish".into(),
                }],
            ),
            271,
            PatchKey(8),
        )
        .expect_err("forced staged identity failure");
    assert_eq!(failure, bus::DocumentPublicationFailure::InvariantViolation);
    assert_eq!(tab.dom_version, version_before);
    assert_eq!(tab.dom_handle, handle_before);
    assert_eq!(tab.page.outline(100), outline_before);
    assert_eq!(tab.page.render_pipeline_debug_snapshot(), pipeline_before);
    assert_eq!(
        tab.page.retained_render_state_debug_snapshot(),
        retained_before
    );
    assert_eq!(tab.page.last_dom_mutation_facts(), facts_before.as_ref());
    assert_eq!(tab.pending_render_work, pending_before);
}

#[test]
fn same_handle_mode_mismatch_preserves_committed_state() {
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
            initial_patch_document("p { color: red; }", Some("p")),
        ),
    });
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 28,
        publication: bus::DocumentPublication {
            handle,
            document_mode: html::DocumentMode::Quirks,
            payload: bus::DocumentPublicationPayload::Patch {
                from: DomVersion(1),
                to: DomVersion(2),
                patches: vec![DomPatch::SetText {
                    key: PatchKey(9),
                    text: "changed".to_string(),
                }],
            },
        },
    });
    assert_eq!(tab.dom_version, DomVersion(1));
    assert_eq!(tab.page.document_mode, Some(html::DocumentMode::NoQuirks));
    assert!(
        tab.last_status
            .as_deref()
            .unwrap_or_default()
            .contains("DocumentModeChanged")
    );
}

#[test]
fn inert_template_contents_publication_commits_without_restyle() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 26;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(260);
    let initial = vec![
        DomPatch::Clear,
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: html::internal::html_name("html"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: html::internal::html_name("template"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
    ];
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 26,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion::INITIAL,
            DomVersion(1),
            initial,
        ),
    });
    let before = tab.page.style_generations();
    let before_dirty = tab.page.style_dirty();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 26,
        publication: no_quirks_patch_publication(
            handle,
            DomVersion(1),
            DomVersion(2),
            vec![DomPatch::CreateTemplateContents {
                host: PatchKey(3),
                contents: PatchKey(4),
            }],
        ),
    });
    assert_eq!(tab.dom_version, DomVersion(2));
    let after = tab.page.style_generations();
    assert_eq!(after.dom, before.dom + 1);
    assert_eq!(after.style_inputs, before.style_inputs);
    assert_eq!(after.stylesheets, before.stylesheets);
    assert_eq!(tab.page.style_dirty(), before_dirty);
}
