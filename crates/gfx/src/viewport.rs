use crate::EguiTextMeasurer;
use crate::input::{
    FormControlHandler, FrameInputCtx, InputValueStore, InteractionState, PageAction,
    route_frame_input,
};
use crate::paint::{ImageProvider, PaintArgs, PaintPhaseInput, paint_page};
use crate::text_control::{find_layout_box_by_id, sync_input_scroll_for_caret};
use crate::textarea::sync_textarea_scroll_for_caret;
use crate::util::{get_attr, input_text_padding, resolve_relative_url};
use css::StylePhaseOutput;
use egui::{Color32, Rect, ScrollArea, Sense, Stroke, Ui, Vec2};
use html::{Node, internal::Id};
use input_core::InputValueStore as CoreInputValueStore;
use layout::{
    LayoutPhaseInput, Rectangle, ReplacedElementInfoProvider, ReplacedKind, RetainedLayoutArtifact,
    RetainedLayoutFallbackReason, RetainedLayoutFrameAction, RetainedLayoutFrameResult,
    RetainedLayoutKeySeed, layout_document,
};
use std::cell::RefCell;
use std::collections::HashMap;

pub use crate::input::PageAction as ViewportAction;

#[derive(Clone, Copy, Debug)]
pub struct ViewportConfig {
    pub scroll_id_salt: &'static str,
    pub min_content_height: f32,
    pub auto_shrink: [bool; 2],
}

impl Default for ViewportConfig {
    fn default() -> Self {
        Self {
            scroll_id_salt: "page_viewport_scroll_area",
            min_content_height: 200.0,
            auto_shrink: [false, false],
        }
    }
}

pub struct ViewportCtx<'ui, 'style, R, F> {
    pub ui: &'ui mut Ui,
    pub style: &'ui StylePhaseOutput<'style>,
    pub base_url: Option<&'ui str>,
    pub resources: &'ui R,
    pub input_values: &'ui mut InputValueStore,
    pub form_controls: &'ui F,
    pub interaction: &'ui mut InteractionState,
    pub config: ViewportConfig,
    pub repaint_policy: ViewportRepaintPolicy,
    pub retained_layout: Option<ViewportRetainedLayout<'ui>>,
}

impl<'ui, 'style, R, F> ViewportCtx<'ui, 'style, R, F> {
    pub fn new(
        ui: &'ui mut Ui,
        style: &'ui StylePhaseOutput<'style>,
        base_url: Option<&'ui str>,
        resources: &'ui R,
        input_values: &'ui mut InputValueStore,
        form_controls: &'ui F,
        interaction: &'ui mut InteractionState,
    ) -> Self {
        Self {
            ui,
            style,
            base_url,
            resources,
            input_values,
            form_controls,
            interaction,
            config: ViewportConfig::default(),
            repaint_policy: ViewportRepaintPolicy::default(),
            retained_layout: None,
        }
    }

    pub fn with_repaint_policy(mut self, repaint_policy: ViewportRepaintPolicy) -> Self {
        self.repaint_policy = repaint_policy;
        self
    }

    pub fn with_retained_layout(mut self, retained_layout: ViewportRetainedLayout<'ui>) -> Self {
        self.retained_layout = Some(retained_layout);
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ViewportRetainedLayout<'a> {
    pub key_seed: RetainedLayoutKeySeed,
    pub retained: Option<&'a RetainedLayoutArtifact>,
    pub reuse_allowed: bool,
    pub conservative_dirty_fallback: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewportRepaintScope {
    Viewport,
    Document,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewportRepaintPolicy {
    pending_scope: Option<ViewportRepaintScope>,
}

impl ViewportRepaintPolicy {
    pub fn from_pending_scope(scope: Option<ViewportRepaintScope>) -> Self {
        Self {
            pending_scope: scope,
        }
    }

    pub fn scope_for_frame(self, viewport_changed: bool) -> ViewportRepaintScope {
        match (self.pending_scope, viewport_changed) {
            (Some(ViewportRepaintScope::Document), _) => ViewportRepaintScope::Document,
            (Some(ViewportRepaintScope::Viewport), _) => ViewportRepaintScope::Viewport,
            (None, true) => ViewportRepaintScope::Viewport,
            (None, false) => ViewportRepaintScope::Document,
        }
    }
}

pub struct ViewportFrameOutput {
    pub action: Option<PageAction>,
    pub viewport_changed: bool,
    pub requested_followup_render: bool,
    pub repaint_scope: ViewportRepaintScope,
    pub retained_layout_result: Option<RetainedLayoutFrameResult>,
}

pub fn execute_viewport_frame<R: ImageProvider, F: FormControlHandler<CoreInputValueStore>>(
    ctx: ViewportCtx<'_, '_, R, F>,
) -> ViewportFrameOutput {
    let ViewportCtx {
        ui,
        style,
        base_url,
        resources,
        input_values,
        form_controls,
        interaction,
        config,
        repaint_policy,
        retained_layout,
    } = ctx;

    ScrollArea::vertical()
        .id_salt(config.scroll_id_salt)
        .auto_shrink(config.auto_shrink)
        .show(ui, |ui| {
            let available_width = ui.available_width();
            let min_height = ui.available_height().max(config.min_content_height);

            let measurer = EguiTextMeasurer::new(ui.ctx());

            let replaced_info = ViewportReplacedInfo {
                base_url,
                resources,
            };
            let (layout_output, retained_layout_result) = match retained_layout {
                Some(retained_layout) => {
                    let key = retained_layout.key_seed.for_viewport_width(available_width);
                    let retained_attempt = retained_layout
                        .reuse_allowed
                        .then_some(retained_layout.retained)
                        .flatten()
                        .filter(|artifact| artifact.key() == key)
                        .map(|artifact| (artifact, artifact.materialize(style.root())));

                    match retained_attempt {
                        Some((artifact, Ok(output))) => (
                            output,
                            Some(RetainedLayoutFrameResult {
                                key,
                                action: RetainedLayoutFrameAction::Reused,
                                artifact: artifact.clone(),
                            }),
                        ),
                        Some((_artifact, Err(_))) => {
                            let output = layout_document(LayoutPhaseInput::from_style_output(
                                style,
                                available_width,
                                &measurer,
                                Some(&replaced_info),
                            ));
                            let artifact = RetainedLayoutArtifact::from_layout_output(key, &output);
                            (
                                output,
                                Some(RetainedLayoutFrameResult {
                                    key,
                                    action: RetainedLayoutFrameAction::ConservativeFallback(
                                        RetainedLayoutFallbackReason::MaterializationFailed,
                                    ),
                                    artifact,
                                }),
                            )
                        }
                        None => {
                            let output = layout_document(LayoutPhaseInput::from_style_output(
                                style,
                                available_width,
                                &measurer,
                                Some(&replaced_info),
                            ));
                            let artifact = RetainedLayoutArtifact::from_layout_output(key, &output);
                            let action = if retained_layout.conservative_dirty_fallback {
                                RetainedLayoutFrameAction::ConservativeFallback(
                                    RetainedLayoutFallbackReason::DirtyLayout,
                                )
                            } else if retained_layout.reuse_allowed
                                && retained_layout.retained.is_none()
                            {
                                RetainedLayoutFrameAction::ConservativeFallback(
                                    RetainedLayoutFallbackReason::MissingRetainedArtifact,
                                )
                            } else if retained_layout.reuse_allowed {
                                RetainedLayoutFrameAction::ConservativeFallback(
                                    RetainedLayoutFallbackReason::KeyMismatch,
                                )
                            } else {
                                RetainedLayoutFrameAction::Recomputed
                            };
                            (
                                output,
                                Some(RetainedLayoutFrameResult {
                                    key,
                                    action,
                                    artifact,
                                }),
                            )
                        }
                    }
                }
                None => (
                    layout_document(LayoutPhaseInput::from_style_output(
                        style,
                        available_width,
                        &measurer,
                        Some(&replaced_info),
                    )),
                    None,
                ),
            };
            let layout_root = layout_output.root();

            let content_height = layout_output.content_height().max(min_height);

            let (content_rect, resp) =
                ui.allocate_exact_size(Vec2::new(available_width, content_height), Sense::hover());

            let painter = ui.painter_at(content_rect);
            let origin = content_rect.min;

            let viewport_width_changed = interaction
                .last_viewport_width
                .map(|w| (w - available_width).abs() > 0.5)
                .unwrap_or(true);
            interaction.last_viewport_width = Some(available_width);

            let layout_root_size_changed = interaction
                .last_layout_root_size
                .map(|(w, h)| {
                    (w - layout_root.rect.width).abs() > 0.5
                        || (h - layout_root.rect.height).abs() > 0.5
                })
                .unwrap_or(true);
            interaction.last_layout_root_size =
                Some((layout_root.rect.width, layout_root.rect.height));

            let layout_changed = viewport_width_changed || layout_root_size_changed;
            let repaint_scope = repaint_policy.scope_for_frame(viewport_width_changed);

            if layout_changed {
                interaction.focused_input_rect = None;
            }

            // Keep the focused text control's scroll stable across frames (e.g. resize)
            // and ensure the caret remains visible within the control viewport.
            if let Some(focus_id) = interaction.focused_node_id
                && let Some(lb) = find_layout_box_by_id(layout_root, focus_id).filter(|lb| {
                    matches!(
                        lb.replaced,
                        Some(ReplacedKind::InputText | ReplacedKind::TextArea)
                    )
                })
            {
                let viewport = interaction.focused_input_rect.unwrap_or(lb.rect);

                match lb.replaced {
                    Some(ReplacedKind::InputText) => {
                        sync_input_scroll_for_caret(
                            input_values.inner_mut(),
                            focus_id,
                            viewport.width.max(1.0),
                            &measurer,
                            lb.style,
                        );
                    }
                    Some(ReplacedKind::TextArea) => {
                        let (pad_l, pad_r, _pad_t, _pad_b) = input_text_padding(lb.style);
                        let available_text_w = (viewport.width - pad_l - pad_r).max(0.0);
                        let lines = interaction.textarea.ensure_layout_cache(
                            input_values.inner(),
                            focus_id,
                            available_text_w,
                            &measurer,
                            lb.style,
                        );

                        sync_textarea_scroll_for_caret(
                            input_values.inner_mut(),
                            focus_id,
                            viewport.height.max(1.0),
                            lines,
                            &measurer,
                            lb.style,
                        );
                    }
                    _ => {}
                }
            }

            let fragment_rects: RefCell<HashMap<Id, Rectangle>> = RefCell::new(HashMap::new());

            // Paint
            let focused = interaction.focused_node_id;
            let active = interaction.active;
            {
                let selection = ui.visuals().selection;
                let bg = selection.bg_fill;
                let selection_bg_fill =
                    Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), bg.a().min(96));
                let selection_stroke =
                    Stroke::new(selection.stroke.width.max(2.0), selection.stroke.color);

                let focused_textarea_lines =
                    focused.and_then(|id| interaction.textarea.focused_lines(id));
                let viewport_clip =
                    viewport_repaint_clip(repaint_scope, content_rect, ui.clip_rect());
                let clipped_painter = viewport_clip.map(|clip| painter.with_clip_rect(clip));
                let paint_painter = clipped_painter.as_ref().unwrap_or(&painter);

                let paint_args = PaintArgs {
                    painter: paint_painter,
                    origin,
                    measurer: &measurer,
                    base_url,
                    resources,
                    input_values: &*input_values,
                    focused,
                    focused_textarea_lines,
                    active,
                    selection_bg_fill,
                    selection_stroke,
                    fragment_rects: Some(&fragment_rects),
                };
                paint_page(PaintPhaseInput::new(&layout_output), paint_args);
            }

            let input_result = route_frame_input(FrameInputCtx {
                ui,
                resp,
                content_rect,
                origin,
                layout_root,
                measurer: &measurer,
                layout_changed,
                fragment_rects: &fragment_rects,
                base_url,
                input_values: input_values.inner_mut(),
                form_controls,
                interaction,
            });

            ViewportFrameOutput {
                action: input_result.action,
                viewport_changed: viewport_width_changed,
                requested_followup_render: input_result.requested_followup_render,
                repaint_scope,
                retained_layout_result,
            }
        })
        .inner
}

fn viewport_repaint_clip(
    scope: ViewportRepaintScope,
    content_rect: Rect,
    viewport_rect: Rect,
) -> Option<Rect> {
    match scope {
        ViewportRepaintScope::Document => None,
        ViewportRepaintScope::Viewport => Some(content_rect.intersect(viewport_rect)),
    }
}

struct ViewportReplacedInfo<'a, R> {
    base_url: Option<&'a str>,
    resources: &'a R,
}

impl<R: ImageProvider> ReplacedElementInfoProvider for ViewportReplacedInfo<'_, R> {
    fn intrinsic_for_img(&self, node: &Node) -> Option<layout::replaced::intrinsic::IntrinsicSize> {
        let src = get_attr(node, "src")?;
        let url = resolve_relative_url(self.base_url, src)?;
        let (w, h) = self.resources.image_intrinsic_size_px(&url)?;
        Some(layout::replaced::intrinsic::IntrinsicSize::from_w_h(
            Some(w as f32),
            Some(h as f32),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, Vec2};

    #[test]
    fn repaint_policy_preserves_document_as_conservative_pending_scope() {
        let policy =
            ViewportRepaintPolicy::from_pending_scope(Some(ViewportRepaintScope::Document));

        assert_eq!(
            policy.scope_for_frame(false),
            ViewportRepaintScope::Document
        );
        assert_eq!(policy.scope_for_frame(true), ViewportRepaintScope::Document);
    }

    #[test]
    fn repaint_policy_uses_viewport_for_pending_or_synthesized_viewport_scope() {
        let pending_viewport =
            ViewportRepaintPolicy::from_pending_scope(Some(ViewportRepaintScope::Viewport));
        assert_eq!(
            pending_viewport.scope_for_frame(false),
            ViewportRepaintScope::Viewport
        );

        let no_pending = ViewportRepaintPolicy::from_pending_scope(None);
        assert_eq!(
            no_pending.scope_for_frame(true),
            ViewportRepaintScope::Viewport
        );
        assert_eq!(
            no_pending.scope_for_frame(false),
            ViewportRepaintScope::Document
        );
    }

    #[test]
    fn viewport_repaint_clip_distinguishes_viewport_from_document_scope() {
        let content = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(200.0, 600.0));
        let viewport = Rect::from_min_size(Pos2::new(0.0, 100.0), Vec2::new(200.0, 150.0));

        assert_eq!(
            viewport_repaint_clip(ViewportRepaintScope::Document, content, viewport),
            None
        );
        assert_eq!(
            viewport_repaint_clip(ViewportRepaintScope::Viewport, content, viewport),
            Some(viewport)
        );
    }
}
