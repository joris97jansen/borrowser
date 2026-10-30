//! Frame preparation and execution orchestration.

use crate::form_controls::FormControlIndex;
use crate::input_state::DocumentInputState;
use crate::page::PageState;
use css::{ComputedStyleResolutionError, StylePhaseOutput};
use egui::Ui;
use gfx::input::PageAction;
use gfx::paint::{ImageProvider, PaintArtifact};
use gfx::viewport::{
    ViewportCtx, ViewportPaintArtifactAction, ViewportRepaintPolicy, ViewportRepaintScope,
    ViewportResourceInputs, ViewportRetainedLayout, ViewportRetainedPaint, execute_viewport_frame,
};
use layout::{RetainedLayoutArtifact, RetainedLayoutFrameResult, RetainedLayoutKeySeed};

use super::debug::{
    RenderFrameExecutionTrace, RenderPhaseExecutionKind, RenderPhaseExecutionTrace,
};
use super::invalidation::{
    IntrinsicRenderInvalidationSource, PendingRenderWork, PhaseRerunSource,
    RenderInvalidationRequest, render_intrinsic_invalidation_request,
};
use super::page_background::find_page_background_color;
use super::types::{PaintInvalidationScope, RepaintExecutionPlan, RepaintExecutionScope};
use super::types::{RenderRebuildTrigger, RenderingPhase};
use super::work_plan::{RenderWorkPlan, RepaintExecution};
use super::{RetainedPaintArtifactKeySeed, RetainedPaintFrameAction, RetainedPaintFrameResult};

pub(crate) struct OrchestratedFrameOutcome {
    pub(crate) action: Option<PageAction>,
    pub(crate) followup_render_request: Option<RenderInvalidationRequest>,
    pub(crate) trace: RenderFrameExecutionTrace,
    pub(crate) retained_layout_result: Option<RetainedLayoutFrameResult>,
    pub(crate) retained_paint_result: Option<RetainedPaintFrameResult>,
}

pub(crate) struct PreparedPageFrame<'a> {
    #[cfg(feature = "conformance-observation")]
    style_key: super::RetainedStyleArtifactKey,
    pub(crate) style_output: StylePhaseOutput<'a>,
    pub(crate) page_background: Option<(u8, u8, u8, u8)>,
    pub(crate) work_plan: RenderWorkPlan,
    retained_layout_key_seed: RetainedLayoutKeySeed,
    retained_layout_artifact: Option<RetainedLayoutArtifact>,
    retained_paint_key_seed: RetainedPaintArtifactKeySeed,
    retained_paint_artifact: Option<PaintArtifact>,
    pending_work: PendingRenderWork,
    style_dirty_before_frame: bool,
    image_sources:
        &'a std::collections::HashMap<html::internal::Id, crate::page::ImageSourceBinding>,
    form_controls: FormControlIndex,
}

#[derive(Default)]
struct PhaseReasonAccumulator {
    direct_triggers: Vec<RenderRebuildTrigger>,
    cascaded_from: Vec<RenderingPhase>,
}

impl PhaseReasonAccumulator {
    fn record(&mut self, source: PhaseRerunSource) {
        match source {
            PhaseRerunSource::None => {}
            PhaseRerunSource::Direct(trigger) => push_unique(&mut self.direct_triggers, trigger),
            PhaseRerunSource::CascadedFrom(phase) => push_unique(&mut self.cascaded_from, phase),
        }
    }

    fn has_requested_work(&self) -> bool {
        !self.direct_triggers.is_empty() || !self.cascaded_from.is_empty()
    }
}

pub(crate) fn prepare_page_frame(
    page: &mut PageState,
    pending_work: PendingRenderWork,
) -> Result<Option<PreparedPageFrame<'_>>, ComputedStyleResolutionError> {
    let style_dirty_before_frame = page.style_dirty_for_rendering();
    let form_controls = page.form_controls.clone();

    let prepared_style = match page.prepare_style_phase_for_frame(&pending_work)? {
        Some(prepared_style) => prepared_style,
        None => return Ok(None),
    };
    let page_background = find_page_background_color(&prepared_style.style_output);

    Ok(Some(PreparedPageFrame {
        #[cfg(feature = "conformance-observation")]
        style_key: prepared_style.style_key,
        style_output: prepared_style.style_output,
        page_background,
        work_plan: prepared_style.work_plan,
        retained_layout_key_seed: prepared_style.retained_layout_key_seed,
        retained_layout_artifact: prepared_style.retained_layout_artifact,
        retained_paint_key_seed: prepared_style.retained_paint_key_seed,
        retained_paint_artifact: prepared_style.retained_paint_artifact,
        pending_work,
        style_dirty_before_frame,
        image_sources: prepared_style.image_sources,
        form_controls,
    }))
}

struct BrowserReplacedElementInfo<'a, R> {
    image_sources:
        &'a std::collections::HashMap<html::internal::Id, crate::page::ImageSourceBinding>,
    resources: &'a R,
}

impl<R: ImageProvider> layout::ReplacedElementInfoProvider for BrowserReplacedElementInfo<'_, R> {
    fn resolve_image_source(&self, node_id: html::internal::Id, source: &str) -> Option<String> {
        crate::page::selected_image_source(self.image_sources, node_id, source)
    }

    fn intrinsic_for_img(
        &self,
        image: &layout::ImagePresentation,
    ) -> Option<layout::replaced::intrinsic::IntrinsicSize> {
        let url = image.resolved_source()?;
        let (width, height) = self.resources.image_intrinsic_size_px(url)?;
        Some(layout::replaced::intrinsic::IntrinsicSize::from_w_h(
            Some(width as f32),
            Some(height as f32),
        ))
    }
}

pub(crate) fn execute_prepared_page_frame<R: ImageProvider>(
    ui: &mut Ui,
    prepared: PreparedPageFrame<'_>,
    input_state: &mut DocumentInputState,
    resources: &R,
    #[cfg(feature = "conformance-observation")] observation: Option<
        &mut Option<crate::tab::observation::FrameDraft>,
    >,
) -> OrchestratedFrameOutcome {
    let PreparedPageFrame {
        #[cfg(feature = "conformance-observation")]
        style_key,
        style_output,
        page_background: _page_background,
        work_plan,
        retained_layout_key_seed,
        retained_layout_artifact,
        retained_paint_key_seed,
        retained_paint_artifact,
        pending_work,
        style_dirty_before_frame,
        image_sources,
        form_controls,
    } = prepared;
    let replaced_info = BrowserReplacedElementInfo {
        image_sources,
        resources,
    };
    let repaint_policy = viewport_repaint_policy(&pending_work);
    let viewport_result = execute_viewport_frame(
        ViewportCtx::new(
            ui,
            &style_output,
            None, // Return authored link intent; Browser owns URL resolution.
            ViewportResourceInputs::new(resources, &replaced_info),
            &mut input_state.input_values,
            &form_controls,
            &mut input_state.interaction,
        )
        .with_repaint_policy(repaint_policy)
        .with_retained_layout(ViewportRetainedLayout {
            key_seed: retained_layout_key_seed,
            retained: retained_layout_artifact.as_ref(),
            reuse_allowed: matches!(
                work_plan.relayout_execution,
                super::work_plan::RelayoutExecution::ReuseRetained
            ),
            conservative_dirty_fallback: matches!(
                work_plan.relayout_execution,
                super::work_plan::RelayoutExecution::ConservativeDocumentFallback { .. }
            ),
        })
        .with_retained_paint(ViewportRetainedPaint {
            retained: retained_paint_artifact.as_ref(),
            reuse_allowed: matches!(work_plan.repaint_execution, RepaintExecution::ReuseRetained),
        }),
    );
    let retained_paint_result = match (
        viewport_result.retained_paint_result,
        viewport_result.retained_layout_result.as_ref(),
    ) {
        (Some(paint_result), Some(layout_result)) => {
            let key = retained_paint_key_seed.for_layout_key(layout_result.key);
            let action = match paint_result.action {
                ViewportPaintArtifactAction::Reused => {
                    debug_assert!(
                        matches!(
                            layout_result.action,
                            layout::RetainedLayoutFrameAction::Reused
                        ),
                        "retained paint reuse requires actual retained layout reuse"
                    );
                    RetainedPaintFrameAction::Reused
                }
                ViewportPaintArtifactAction::Recomputed => {
                    retained_paint_action_for_recompute(&work_plan)
                }
            };
            Some(RetainedPaintFrameResult {
                key,
                action,
                artifact: paint_result.artifact,
            })
        }
        _ => None,
    };

    let trace = build_render_frame_execution_trace(
        &pending_work,
        style_dirty_before_frame,
        viewport_result.viewport_changed,
        viewport_result
            .retained_layout_result
            .as_ref()
            .is_some_and(|result| {
                matches!(result.action, layout::RetainedLayoutFrameAction::Reused)
            }),
        retained_paint_result
            .as_ref()
            .is_some_and(|result| matches!(result.action, RetainedPaintFrameAction::Reused)),
    );
    debug_assert_eq!(
        trace.repaint_execution.scope,
        repaint_execution_scope_from_viewport(viewport_result.repaint_scope)
    );
    let followup_render_request = viewport_result.requested_followup_render.then(|| {
        render_intrinsic_invalidation_request(IntrinsicRenderInvalidationSource::InputStateChanged)
    });

    #[cfg(feature = "conformance-observation")]
    if let Some(observation) = observation {
        *observation = match (
            &viewport_result.retained_layout_result,
            &retained_paint_result,
        ) {
            (Some(layout), Some(paint)) if layout.key == layout.artifact.key() => {
                Some(crate::tab::observation::FrameDraft::from_frame(
                    &style_output,
                    style_key,
                    layout.artifact.clone(),
                    paint.key,
                    viewport_result.layout_viewport_width,
                ))
            }
            _ => None,
        };
    }
    OrchestratedFrameOutcome {
        action: viewport_result.action,
        followup_render_request,
        trace,
        retained_layout_result: viewport_result.retained_layout_result,
        retained_paint_result,
    }
}

fn retained_paint_action_for_recompute(work_plan: &RenderWorkPlan) -> RetainedPaintFrameAction {
    match work_plan.repaint_execution {
        RepaintExecution::ConservativeDocumentFallback { .. } => {
            RetainedPaintFrameAction::ConservativeDocumentFallback
        }
        RepaintExecution::ConservativeViewportFallback { .. } | RepaintExecution::Viewport => {
            RetainedPaintFrameAction::ConservativeViewportFallback
        }
        RepaintExecution::None
        | RepaintExecution::ReuseRetained
        | RepaintExecution::FullDocument => RetainedPaintFrameAction::Recomputed,
    }
}

pub(crate) fn build_render_frame_execution_trace(
    pending_work: &PendingRenderWork,
    style_dirty_before_frame: bool,
    viewport_changed: bool,
    layout_reused_from_retained_artifacts: bool,
    paint_reused_from_retained_artifacts: bool,
) -> RenderFrameExecutionTrace {
    let mut triggered_entry_points = pending_work
        .requests()
        .iter()
        .map(|request| request.entry_point())
        .collect::<Vec<_>>();
    let mut style = PhaseReasonAccumulator::default();
    let mut layout = PhaseReasonAccumulator::default();
    let mut paint = PhaseReasonAccumulator::default();
    let mut frame_orchestration = PhaseReasonAccumulator::default();

    for request in pending_work.requests() {
        let requested_work = request.requested_work();
        style.record(requested_work.style());
        layout.record(requested_work.layout());
        paint.record(requested_work.paint());
        frame_orchestration.record(requested_work.frame_orchestration());
    }

    if viewport_changed {
        let request = render_intrinsic_invalidation_request(
            IntrinsicRenderInvalidationSource::ViewportChanged,
        );
        push_unique(&mut triggered_entry_points, request.entry_point());
        let requested_work = request.requested_work();
        style.record(requested_work.style());
        layout.record(requested_work.layout());
        paint.record(requested_work.paint());
        frame_orchestration.record(requested_work.frame_orchestration());
    }

    let style_fallback = if style_dirty_before_frame {
        RenderPhaseExecutionKind::RequiredForCurrentFrame
    } else {
        RenderPhaseExecutionKind::MaterializedFromRetainedArtifacts
    };
    let layout_fallback = if layout_reused_from_retained_artifacts {
        RenderPhaseExecutionKind::MaterializedFromRetainedArtifacts
    } else {
        RenderPhaseExecutionKind::RequiredForCurrentFrame
    };
    let paint_fallback = if paint_reused_from_retained_artifacts {
        RenderPhaseExecutionKind::MaterializedFromRetainedArtifacts
    } else {
        RenderPhaseExecutionKind::RequiredForCurrentFrame
    };

    RenderFrameExecutionTrace {
        triggered_entry_points,
        style: phase_trace(RenderingPhase::Style, style, style_fallback),
        layout: phase_trace(RenderingPhase::Layout, layout, layout_fallback),
        paint: phase_trace(RenderingPhase::Paint, paint, paint_fallback),
        frame_orchestration: phase_trace(
            RenderingPhase::FrameOrchestration,
            frame_orchestration,
            RenderPhaseExecutionKind::RequiredForCurrentFrame,
        ),
        repaint_execution: super::debug::RepaintExecutionTrace {
            scope: RepaintExecutionPlan::from_frame_inputs(pending_work, viewport_changed).scope,
        },
        semantic_phase_order: vec![
            RenderingPhase::Style,
            RenderingPhase::Layout,
            RenderingPhase::Paint,
        ],
    }
}

fn phase_trace(
    phase: RenderingPhase,
    reasons: PhaseReasonAccumulator,
    fallback_kind: RenderPhaseExecutionKind,
) -> RenderPhaseExecutionTrace {
    let kind = if reasons.has_requested_work() {
        RenderPhaseExecutionKind::Requested
    } else {
        fallback_kind
    };
    RenderPhaseExecutionTrace {
        phase,
        kind,
        direct_triggers: reasons.direct_triggers,
        cascaded_from: reasons.cascaded_from,
    }
}

fn push_unique<T: Copy + PartialEq>(items: &mut Vec<T>, item: T) {
    if !items.contains(&item) {
        items.push(item);
    }
}

fn viewport_repaint_policy(pending_work: &PendingRenderWork) -> ViewportRepaintPolicy {
    let paint = pending_work.paint_invalidations();
    let pending_scope = paint
        .effective_scope()
        .map(viewport_repaint_scope_from_paint);
    ViewportRepaintPolicy::from_pending_scope(pending_scope)
}

fn viewport_repaint_scope_from_paint(scope: PaintInvalidationScope) -> ViewportRepaintScope {
    match scope {
        PaintInvalidationScope::Viewport => ViewportRepaintScope::Viewport,
        PaintInvalidationScope::Document => ViewportRepaintScope::Document,
    }
}

fn repaint_execution_scope_from_viewport(scope: ViewportRepaintScope) -> RepaintExecutionScope {
    match scope {
        ViewportRepaintScope::Viewport => RepaintExecutionScope::Viewport,
        ViewportRepaintScope::Document => RepaintExecutionScope::Document,
    }
}

#[cfg(test)]
mod image_association_tests {
    use super::*;
    use layout::ReplacedElementInfoProvider;
    use std::cell::RefCell;

    struct Measure;
    impl layout::TextMeasurer for Measure {
        fn measure(&self, text: &str, _style: &css::ComputedStyle) -> f32 {
            text.len() as f32 * 8.0
        }
        fn line_height(&self, _style: &css::ComputedStyle) -> f32 {
            16.0
        }
    }
    struct Sizes(RefCell<Vec<String>>);
    impl ImageProvider for Sizes {
        fn image_state_by_url(&self, _url: &str) -> gfx::paint::ImageState {
            gfx::paint::ImageState::Missing
        }
        fn image_intrinsic_size_px(&self, url: &str) -> Option<(u32, u32)> {
            self.0.borrow_mut().push(url.into());
            Some((if url.contains("/old/") { 11 } else { 22 }, 10))
        }
    }

    #[test]
    fn resource_associations_layout_and_intrinsic_lookup_use_selected_owner_urls() {
        let mut page = PageState::new();
        page.start_nav("https://example.test/old/page");
        let parse = |source| {
            let mut output = html::parse_document(source, Default::default()).unwrap();
            let mut store = crate::dom_store::DomStore::new();
            let handle = core_types::DomHandle(1);
            store.create(handle).unwrap();
            store
                .apply(
                    handle,
                    core_types::DomVersion::INITIAL,
                    core_types::DomVersion(1),
                    &output.patches,
                )
                .unwrap();
            output.document = store.materialize_owned(handle).unwrap();
            output
        };
        let initial = parse("<!doctype html><img src=same.png><img src=''><img src='http://['>");
        page.document_mode = Some(initial.document_mode);
        page.dom = Some(Box::new(initial.document));
        page.reconcile_image_sources();
        page.addresses
            .as_mut()
            .unwrap()
            .select(Some((html::PatchKey(999), "/new/".into())))
            .unwrap();
        let next = parse("<!doctype html><img src=same.png><img src=same.png><img src='http://['>");
        page.dom = Some(Box::new(next.document));
        page.reconcile_image_sources();
        let resources = Sizes(RefCell::new(Vec::new()));
        let first_id;
        {
            let prepared = prepare_page_frame(&mut page, PendingRenderWork::default())
                .unwrap()
                .unwrap();
            let provider = BrowserReplacedElementInfo {
                image_sources: prepared.image_sources,
                resources: &resources,
            };
            let output = layout::layout_document(layout::LayoutPhaseInput::from_style_output(
                &prepared.style_output,
                320.0,
                &Measure,
                Some(&provider),
            ));
            fn images(
                node: &layout::LayoutBox<'_, '_>,
                out: &mut Vec<(html::internal::Id, Option<String>, Option<f32>)>,
            ) {
                if let Some(layout::ReplacedElementPresentation::Image(image)) =
                    node.replaced_presentation()
                {
                    out.push((
                        node.node_id(),
                        image.resolved_source().map(str::to_owned),
                        node.replaced_intrinsic.and_then(|size| size.width),
                    ));
                }
                for child in &node.children {
                    images(child, out);
                }
            }
            let mut actual = Vec::new();
            images(output.root(), &mut actual);
            assert_eq!(actual.len(), 3);
            first_id = actual[0].0;
            assert_eq!(
                actual[0].1.as_deref(),
                Some("https://example.test/old/same.png")
            );
            assert_eq!(actual[0].2, Some(11.0));
            assert_eq!(
                actual[1].1.as_deref(),
                Some("https://example.test/new/same.png")
            );
            assert_eq!(actual[1].2, Some(22.0));
            assert_eq!(actual[2].1, None);
            assert_eq!(
                &*resources.0.borrow(),
                &[
                    "https://example.test/old/same.png",
                    "https://example.test/new/same.png"
                ]
            );
            assert_eq!(
                provider.resolve_image_source(first_id, "mismatch.png"),
                None
            );
        }
        page.image_sources.remove(&first_id);
        let provider = BrowserReplacedElementInfo {
            image_sources: &page.image_sources,
            resources: &resources,
        };
        assert_eq!(
            provider.resolve_image_source(first_id, "same.png"),
            None,
            "missing association must not resolve against /new/"
        );
    }
}
