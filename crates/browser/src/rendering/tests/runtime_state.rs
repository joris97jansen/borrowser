use crate::page::{PageState, RestyleHint};
use crate::rendering::*;
use html::internal::Id;

use super::support::*;

#[test]
fn retained_render_state_initializes_with_epoch_zero() {
    let page = PageState::new();

    let snapshot = page.retained_render_state_debug_snapshot();
    assert_eq!(snapshot.render_epoch, RenderEpoch::initial());
    assert!(!snapshot.has_dom);
    assert_eq!(snapshot.resolved_styles, RenderArtifactState::Absent);
    assert_eq!(snapshot.computed_styles, RenderArtifactState::Absent);
    assert_eq!(snapshot.styled_tree, RenderArtifactState::Absent);
    assert_eq!(snapshot.layout_tree, RenderArtifactState::Absent);
    assert_eq!(snapshot.paint_output, RenderArtifactState::Absent);
    assert!(snapshot.style_dirty);
    assert!(snapshot.layout_dirty);
    assert!(snapshot.paint_dirty);
    assert_eq!(
        snapshot.dirty_state.entries,
        vec![
            DirtyEntry::new(
                DirtyPhase::Style,
                DirtyReason::ConservativeUnknownImpact,
                DirtyScope::Document,
            ),
            DirtyEntry::new(
                DirtyPhase::Layout,
                DirtyReason::CascadedFromStyle,
                DirtyScope::Document,
            ),
            DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::CascadedFromLayout,
                DirtyScope::Document,
            ),
        ]
    );
    assert_eq!(snapshot.style_invalidation, StyleInvalidationState::Full);
    assert_eq!(
        snapshot.retained_identity_domain,
        RetainedRenderIdentityDomain::initial()
    );
    assert!(snapshot.retained_identities.is_empty());
    assert_eq!(
        snapshot.layout_identity,
        FrameLocalIdentityState::NotRetained
    );
    assert_eq!(
        snapshot.paint_identity,
        FrameLocalIdentityState::NotRetained
    );
    assert_eq!(
        snapshot.stacking_identity,
        FrameLocalIdentityState::NotRetained
    );
    assert_eq!(
        snapshot.traversal_source_order_identity,
        FrameLocalIdentityState::NotRetained
    );
}

#[test]
fn retained_render_state_debug_snapshot_is_deterministic() {
    let page = PageState::new();

    assert_eq!(
        page.retained_render_state_debug_snapshot()
            .to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "retained-render-state\n",
            "render-epoch: 0\n",
            "has-dom: false\n",
            "artifacts:\n",
            "  resolved-styles: absent\n",
            "  computed-styles: absent\n",
            "  styled-tree: absent\n",
            "  layout-tree: absent\n",
            "  paint-output: absent\n",
            "dirty-state:\n",
            "  entries: 3\n",
            "    entry[0]: phase=style reason=conservative-unknown-impact scope=document\n",
            "    entry[1]: phase=layout reason=cascaded-from-style scope=document\n",
            "    entry[2]: phase=paint reason=cascaded-from-layout scope=document\n",
            "  style-dirty: true\n",
            "  layout-dirty: true\n",
            "  paint-dirty: true\n",
            "  style-invalidation: full\n",
            "retained-identities:\n",
            "  identity-domain: 0\n",
            "  render-artifacts: 0\n",
            "  frame-local-layout-ids: not-retained\n",
            "  frame-local-paint-ids: not-retained\n",
            "  frame-local-stacking-ids: not-retained\n",
            "  frame-local-traversal-source-order-ids: not-retained\n",
        )
    );
}

#[test]
fn retained_render_identities_allocate_deterministically_for_initial_document() {
    let page = page_with_node(doc_with_explicit_ids());

    let snapshot = page.retained_render_state_debug_snapshot();
    assert_eq!(snapshot.retained_identity_domain.value(), 1);
    assert_eq!(
        snapshot.retained_identities,
        vec![
            retained_identity(1, 1),
            retained_identity(2, 2),
            retained_identity(3, 3),
            retained_identity(4, 4),
            retained_identity(5, 5),
        ]
    );

    assert_eq!(
        snapshot.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "retained-render-state\n",
            "render-epoch: 1\n",
            "has-dom: true\n",
            "artifacts:\n",
            "  resolved-styles: absent\n",
            "  computed-styles: absent\n",
            "  styled-tree: borrow-backed-rebuilt-on-demand\n",
            "  layout-tree: frame-local-rebuilt-per-frame\n",
            "  paint-output: immediate-frame-output\n",
            "dirty-state:\n",
            "  entries: 3\n",
            "    entry[0]: phase=style reason=document-replaced scope=document\n",
            "    entry[1]: phase=layout reason=cascaded-from-style scope=document\n",
            "    entry[2]: phase=paint reason=cascaded-from-layout scope=document\n",
            "  style-dirty: true\n",
            "  layout-dirty: true\n",
            "  paint-dirty: true\n",
            "  style-invalidation: full\n",
            "retained-identities:\n",
            "  identity-domain: 1\n",
            "  render-artifacts: 5\n",
            "    - retained-render-id=1 kind=dom-backed-render-node anchor=dom-node(1)\n",
            "    - retained-render-id=2 kind=dom-backed-render-node anchor=dom-node(2)\n",
            "    - retained-render-id=3 kind=dom-backed-render-node anchor=dom-node(3)\n",
            "    - retained-render-id=4 kind=dom-backed-render-node anchor=dom-node(4)\n",
            "    - retained-render-id=5 kind=dom-backed-render-node anchor=dom-node(5)\n",
            "  frame-local-layout-ids: not-retained\n",
            "  frame-local-paint-ids: not-retained\n",
            "  frame-local-stacking-ids: not-retained\n",
            "  frame-local-traversal-source-order-ids: not-retained\n",
        )
    );
}

#[test]
fn equivalent_fresh_input_produces_deterministic_identity_output_without_cross_document_claims() {
    let first = page_with_node(doc_with_explicit_ids())
        .retained_render_state_debug_snapshot()
        .to_debug_snapshot();
    let second = page_with_node(doc_with_explicit_ids())
        .retained_render_state_debug_snapshot()
        .to_debug_snapshot();

    assert_eq!(first, second);
    assert!(first.contains("  identity-domain: 1\n"));
}

#[test]
fn debug_snapshot_reports_retained_style_artifacts_and_ephemeral_downstream_trees() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );

    let style_output = style_output_for_test(&mut page);
    assert_eq!(
        styled_element_color(style_output.root(), "p"),
        (255, 0, 0, 255)
    );
    drop(style_output);

    let snapshot = page.render_pipeline_debug_snapshot();
    assert_eq!(
        snapshot,
        RenderPipelineDebugSnapshot {
            has_dom: true,
            resolved_styles: RenderArtifactState::RetainedFresh,
            computed_styles: RenderArtifactState::RetainedFresh,
            styled_tree: RenderArtifactState::BorrowBackedRebuiltOnDemand,
            layout_tree: RenderArtifactState::FrameLocalRebuiltPerFrame,
            paint_output: RenderArtifactState::ImmediateFrameOutput,
            dirty_state: DirtyStateDebugSnapshot {
                entries: vec![
                    DirtyEntry::new(
                        DirtyPhase::Layout,
                        DirtyReason::CascadedFromStyle,
                        DirtyScope::Document,
                    ),
                    DirtyEntry::new(
                        DirtyPhase::Paint,
                        DirtyReason::CascadedFromLayout,
                        DirtyScope::Document,
                    ),
                ],
            },
            style_dirty: false,
            layout_dirty: true,
            paint_dirty: true,
            style_invalidation: StyleInvalidationState::None,
        }
    );
}

#[test]
fn retained_render_state_survives_noop_render_without_epoch_churn() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );

    let initial_style = style_output_for_test(&mut page);
    assert_eq!(
        styled_element_color(initial_style.root(), "p"),
        (255, 0, 0, 255)
    );
    drop(initial_style);

    let before_noop = page.retained_render_state_debug_snapshot();
    assert!(before_noop.render_epoch > RenderEpoch::initial());
    assert_eq!(
        before_noop.resolved_styles,
        RenderArtifactState::RetainedFresh
    );
    assert_eq!(
        before_noop.computed_styles,
        RenderArtifactState::RetainedFresh
    );
    assert!(!before_noop.style_dirty);

    let noop_style = style_output_for_test(&mut page);
    assert_eq!(
        styled_element_color(noop_style.root(), "p"),
        (255, 0, 0, 255)
    );
    drop(noop_style);

    let after_noop = page.retained_render_state_debug_snapshot();
    assert_eq!(after_noop, before_noop);
}

#[test]
fn noop_update_does_not_mark_clean_retained_dirty_state() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );
    let initial_style = style_output_for_test(&mut page);
    assert_eq!(
        styled_element_color(initial_style.root(), "p"),
        (255, 0, 0, 255)
    );
    drop(initial_style);

    page.clear_all_dirty_for_tests();
    assert!(
        page.retained_render_state_debug_snapshot()
            .dirty_state
            .entries
            .is_empty()
    );

    let noop_style = style_output_for_test(&mut page);
    assert_eq!(
        styled_element_color(noop_style.root(), "p"),
        (255, 0, 0, 255)
    );
    drop(noop_style);

    let after = page.retained_render_state_debug_snapshot();
    assert!(!after.style_dirty);
    assert!(!after.layout_dirty);
    assert!(!after.paint_dirty);
    assert!(after.dirty_state.entries.is_empty());
}

#[test]
fn same_document_text_update_preserves_surviving_retained_identity() {
    let mut page = page_with_node(doc_with_explicit_ids());
    let before = page.retained_render_state_debug_snapshot();
    let text_identity = identity_for_dom_anchor(&before, Id(5));

    replace_first_text(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        "Hello",
        "Goodbye",
    );
    page.mark_dom_changed_for_tests(RestyleHint::text_mutated());

    let after = page.retained_render_state_debug_snapshot();
    assert_eq!(
        after.retained_identity_domain,
        before.retained_identity_domain
    );
    assert_eq!(identity_for_dom_anchor(&after, Id(5)), text_identity);
}

#[test]
fn same_document_class_update_preserves_surviving_retained_identity() {
    let mut page = page_with_node(doc_with_explicit_ids());
    let before = page.retained_render_state_debug_snapshot();
    let element_identity = identity_for_dom_anchor(&before, Id(4));

    let dirty_id = set_first_element_attr(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        "p",
        "class",
        Some("hot".to_string()),
    );
    page.mark_dom_changed_for_tests(RestyleHint::attributes_changed(vec![dirty_id]));

    let after = page.retained_render_state_debug_snapshot();
    assert_eq!(
        after.retained_identity_domain,
        before.retained_identity_domain
    );
    assert_eq!(identity_for_dom_anchor(&after, Id(4)), element_identity);
}

#[test]
fn same_document_removal_prunes_stale_retained_identity() {
    let mut page = page_with_node(doc_with_explicit_ids());
    let before = page.retained_render_state_debug_snapshot();
    assert!(identity_for_dom_anchor_optional(&before, Id(4)).is_some());
    assert!(identity_for_dom_anchor_optional(&before, Id(5)).is_some());

    remove_first_element(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        "p",
    );
    page.mark_dom_changed_for_tests(RestyleHint::tree_mutated());

    let after = page.retained_render_state_debug_snapshot();
    assert_eq!(
        after.retained_identity_domain,
        before.retained_identity_domain
    );
    assert!(identity_for_dom_anchor_optional(&after, Id(4)).is_none());
    assert!(identity_for_dom_anchor_optional(&after, Id(5)).is_none());
    assert_eq!(
        after.retained_identities,
        vec![
            retained_identity(1, 1),
            retained_identity(2, 2),
            retained_identity(3, 3),
        ]
    );
}

#[test]
fn same_document_replacement_allocates_new_retained_identity_without_recycling_removed_ids() {
    let mut page = page_with_node(doc_with_explicit_ids());
    let before = page.retained_render_state_debug_snapshot();
    let removed_identity = identity_for_dom_anchor(&before, Id(4));

    replace_first_element(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        "p",
        paragraph_node(6, 7, "Replacement"),
    );
    page.mark_dom_changed_for_tests(RestyleHint::tree_mutated());

    let after = page.retained_render_state_debug_snapshot();
    assert_eq!(
        after.retained_identity_domain,
        before.retained_identity_domain
    );
    assert!(identity_for_dom_anchor_optional(&after, Id(4)).is_none());
    assert_ne!(identity_for_dom_anchor(&after, Id(6)), removed_identity);
    assert_eq!(identity_for_dom_anchor(&after, Id(6)).id.value(), 6);
    assert_eq!(identity_for_dom_anchor(&after, Id(7)).id.value(), 7);
}

#[test]
fn full_document_replacement_starts_new_identity_domain_even_when_dom_ids_match() {
    let mut page = page_with_node(doc_with_explicit_ids());
    let before = page.retained_render_state_debug_snapshot();
    assert_eq!(before.retained_identity_domain.value(), 1);

    let _ = page.replace_dom(
        Box::new(doc_with_explicit_ids()),
        RestyleHint::document_replaced(),
    );

    let after = page.retained_render_state_debug_snapshot();
    assert_eq!(after.retained_identity_domain.value(), 2);
    assert_eq!(
        after.retained_identities,
        vec![
            retained_identity(1, 1),
            retained_identity(2, 2),
            retained_identity(3, 3),
            retained_identity(4, 4),
            retained_identity(5, 5),
        ]
    );
    assert_ne!(
        after.retained_identity_domain, before.retained_identity_domain,
        "matching DOM anchor numbers across replace_dom must not prove retained continuity"
    );
}

#[test]
fn replace_dom_enforces_identity_boundary_even_with_non_document_replaced_hint() {
    let mut page = page_with_node(doc_with_explicit_ids());
    let before = page.retained_render_state_debug_snapshot();
    assert_eq!(before.retained_identity_domain.value(), 1);
    assert_eq!(identity_for_dom_anchor(&before, Id(4)).id.value(), 4);

    let _ = page.replace_dom(
        Box::new(doc_with_explicit_ids()),
        RestyleHint::text_mutated(),
    );

    let after = page.retained_render_state_debug_snapshot();
    assert_eq!(after.retained_identity_domain.value(), 2);
    assert_eq!(identity_for_dom_anchor(&after, Id(4)).id.value(), 4);
    assert_ne!(
        after.retained_identity_domain, before.retained_identity_domain,
        "replace_dom must isolate retained identities even if the caller supplies a non-boundary hint"
    );
}

#[test]
fn retained_render_epoch_advances_when_failed_recompute_consumes_pending_invalidation() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>div { width: 1e39px; }</style></head><body><div>bad</div></body></html>",
    );
    let before = page.retained_render_state_debug_snapshot();
    assert_eq!(before.style_invalidation, StyleInvalidationState::Full);

    let error = match page.build_style_phase_output() {
        Ok(_) => panic!("style recomputation must fail on out-of-range computed width"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("length-out-of-range"),
        "unexpected style recomputation error: {error}"
    );

    let after = page.retained_render_state_debug_snapshot();
    assert!(
        after.render_epoch > before.render_epoch,
        "consuming retained style invalidation must advance the render epoch even when recomputation fails"
    );
    assert!(after.style_dirty);
    assert_eq!(after.style_invalidation, StyleInvalidationState::None);
}

#[test]
fn retained_render_state_debug_snapshot_does_not_expose_frame_local_ids() {
    let mut page = page_with_node(doc_with_explicit_ids());
    let style_output = style_output_for_test(&mut page);
    drop(style_output);

    let snapshot = page
        .retained_render_state_debug_snapshot()
        .to_debug_snapshot();

    for forbidden in [
        "LayoutBox",
        "BoxId",
        "StackingContextId",
        "PaintId",
        "PaintPrimitiveId",
        "source-order-id=",
        "traversal-id=",
        "paint-operation-index",
        "paint-order-index",
    ] {
        assert!(
            !snapshot.contains(forbidden),
            "retained render state debug snapshot must not expose {forbidden}"
        );
    }

    assert!(snapshot.contains("  frame-local-layout-ids: not-retained\n"));
    assert!(snapshot.contains("  frame-local-paint-ids: not-retained\n"));
    assert!(snapshot.contains("  frame-local-stacking-ids: not-retained\n"));
    assert!(snapshot.contains("  frame-local-traversal-source-order-ids: not-retained\n"));
    assert!(snapshot.contains("anchor=dom-node(4)"));
}

#[test]
fn attribute_mutation_keeps_style_cache_but_marks_it_stale_until_restored() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>.hot { color: red; } p { color: black; }</style></head><body><p>Hello</p></body></html>",
    );
    let initial = style_output_for_test(&mut page);
    assert_eq!(styled_element_color(initial.root(), "p"), (0, 0, 0, 255));
    drop(initial);

    let p_id = set_first_element_attr(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        "p",
        "class",
        Some("hot".to_string()),
    );
    let hint = RestyleHint::attributes_changed(vec![p_id]);
    page.mark_dom_changed_for_tests(hint);

    let stale = page.render_pipeline_debug_snapshot();
    assert_eq!(stale.resolved_styles, RenderArtifactState::RetainedStale);
    assert_eq!(stale.computed_styles, RenderArtifactState::RetainedStale);
    assert_eq!(
        stale.style_invalidation,
        StyleInvalidationState::AttributeSuffix
    );
    assert!(stale.style_dirty);
    assert!(stale.layout_dirty);
    assert!(stale.paint_dirty);
    assert_eq!(
        stale.dirty_state.entries,
        vec![
            DirtyEntry::new(
                DirtyPhase::Style,
                DirtyReason::StyleInputChanged,
                DirtyScope::Document,
            ),
            DirtyEntry::new(
                DirtyPhase::Layout,
                DirtyReason::CascadedFromStyle,
                DirtyScope::Document,
            ),
            DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::CascadedFromLayout,
                DirtyScope::Document,
            ),
        ]
    );

    let restyled = style_output_for_test(&mut page);
    assert_eq!(styled_element_color(restyled.root(), "p"), (255, 0, 0, 255));
    drop(restyled);

    let refreshed = page.render_pipeline_debug_snapshot();
    assert_eq!(
        refreshed.resolved_styles,
        RenderArtifactState::RetainedFresh
    );
    assert_eq!(
        refreshed.computed_styles,
        RenderArtifactState::RetainedFresh
    );
    assert_eq!(refreshed.style_invalidation, StyleInvalidationState::None);
    assert!(!refreshed.style_dirty);
}

#[test]
fn text_mutation_dirties_layout_without_invalidating_computed_style() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );
    let initial = style_output_for_test(&mut page);
    assert_eq!(styled_element_color(initial.root(), "p"), (255, 0, 0, 255));
    drop(initial);
    page.clear_layout_dirty_for_tests();

    replace_first_text(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        "Hello",
        "Goodbye",
    );
    let hint = RestyleHint::text_mutated();
    page.mark_dom_changed_for_tests(hint);

    let snapshot = page.render_pipeline_debug_snapshot();
    assert_eq!(snapshot.resolved_styles, RenderArtifactState::RetainedFresh);
    assert_eq!(snapshot.computed_styles, RenderArtifactState::RetainedFresh);
    assert_eq!(snapshot.style_invalidation, StyleInvalidationState::None);
    assert!(!snapshot.style_dirty);
    assert!(snapshot.layout_dirty);
    assert!(snapshot.paint_dirty);
    assert_eq!(
        snapshot.dirty_state.entries,
        vec![
            DirtyEntry::new(
                DirtyPhase::Layout,
                DirtyReason::TextContentChanged,
                DirtyScope::Document,
            ),
            DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::CascadedFromLayout,
                DirtyScope::Document,
            ),
        ]
    );
}

#[test]
fn navigation_reset_clears_page_owned_retained_render_state() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );
    let style_output = style_output_for_test(&mut page);
    assert_eq!(
        styled_element_color(style_output.root(), "p"),
        (255, 0, 0, 255)
    );
    drop(style_output);

    page.start_nav("https://example.com/next.html");

    assert_eq!(
        page.render_pipeline_debug_snapshot(),
        RenderPipelineDebugSnapshot {
            has_dom: false,
            resolved_styles: RenderArtifactState::Absent,
            computed_styles: RenderArtifactState::Absent,
            styled_tree: RenderArtifactState::Absent,
            layout_tree: RenderArtifactState::Absent,
            paint_output: RenderArtifactState::Absent,
            dirty_state: DirtyStateDebugSnapshot {
                entries: vec![
                    DirtyEntry::new(
                        DirtyPhase::Style,
                        DirtyReason::ConservativeUnknownImpact,
                        DirtyScope::Document,
                    ),
                    DirtyEntry::new(
                        DirtyPhase::Layout,
                        DirtyReason::CascadedFromStyle,
                        DirtyScope::Document,
                    ),
                    DirtyEntry::new(
                        DirtyPhase::Paint,
                        DirtyReason::CascadedFromLayout,
                        DirtyScope::Document,
                    ),
                ],
            },
            style_dirty: true,
            layout_dirty: true,
            paint_dirty: true,
            style_invalidation: StyleInvalidationState::Full,
        }
    );
    assert_eq!(
        page.retained_render_state_debug_snapshot().render_epoch,
        RenderEpoch::initial()
    );
    assert_eq!(
        page.retained_render_state_debug_snapshot()
            .retained_identity_domain,
        RetainedRenderIdentityDomain::initial()
    );
    assert!(
        page.retained_render_state_debug_snapshot()
            .retained_identities
            .is_empty()
    );
}

fn retained_identity(retained_id: u64, dom_anchor: u32) -> RetainedRenderIdentity {
    RetainedRenderIdentity {
        id: RetainedRenderId::from_raw(retained_id),
        kind: RetainedRenderArtifactKind::DomBackedRenderNode,
        anchor: RetainedRenderAnchor::DomNode(Id(dom_anchor)),
    }
}

fn identity_for_dom_anchor(
    snapshot: &RetainedRenderStateDebugSnapshot,
    dom_anchor: Id,
) -> RetainedRenderIdentity {
    identity_for_dom_anchor_optional(snapshot, dom_anchor).expect("retained identity should exist")
}

fn identity_for_dom_anchor_optional(
    snapshot: &RetainedRenderStateDebugSnapshot,
    dom_anchor: Id,
) -> Option<RetainedRenderIdentity> {
    snapshot
        .retained_identities
        .iter()
        .copied()
        .find(|identity| identity.anchor == RetainedRenderAnchor::DomNode(dom_anchor))
}
