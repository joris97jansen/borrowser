use crate::EguiTextMeasurer;
use crate::dom::{get_attr, resolve_relative_url};
use crate::input::{
    FormControlHandler, FrameInputCtx, InputValueStore, InteractionState, route_frame_input,
};
use crate::paint::{ImageProvider, PaintArgs, paint_page};
use crate::text_control::{
    ensure_textarea_layout_cache, find_layout_box_by_id, input_text_padding,
    sync_input_scroll_for_caret, sync_textarea_scroll_for_caret,
};
use css::StyledNode;
use egui::{Color32, ScrollArea, Sense, Stroke, Ui, Vec2};
use html::{Id, Node};
use layout::{Rectangle, ReplacedElementInfoProvider, ReplacedKind, layout_block_tree};
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum ViewportAction {
    Navigate(String),
}

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

pub struct ViewportCtx<'a, R, F> {
    pub ui: &'a mut Ui,
    pub style_root: &'a StyledNode<'a>,
    pub base_url: Option<&'a str>,
    pub resources: &'a R,
    pub input_values: &'a mut InputValueStore,
    pub form_controls: &'a F,
    pub interaction: &'a mut InteractionState,
    pub config: ViewportConfig,
}

impl<'a, R, F> ViewportCtx<'a, R, F> {
    pub fn new(
        ui: &'a mut Ui,
        style_root: &'a StyledNode<'a>,
        base_url: Option<&'a str>,
        resources: &'a R,
        input_values: &'a mut InputValueStore,
        form_controls: &'a F,
        interaction: &'a mut InteractionState,
    ) -> Self {
        Self {
            ui,
            style_root,
            base_url,
            resources,
            input_values,
            form_controls,
            interaction,
            config: ViewportConfig::default(),
        }
    }
}

pub fn page_viewport<R: ImageProvider, F: FormControlHandler>(
    ctx: ViewportCtx<'_, R, F>,
) -> Option<ViewportAction> {
    let ViewportCtx {
        ui,
        style_root,
        base_url,
        resources,
        input_values,
        form_controls,
        interaction,
        config,
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
            let layout_root =
                layout_block_tree(style_root, available_width, &measurer, Some(&replaced_info));

            let content_height = layout_root.rect.height.max(min_height);

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

            if layout_changed {
                interaction.focused_input_rect = None;
            }

            // Keep the focused text control's scroll stable across frames (e.g. resize)
            // and ensure the caret remains visible within the control viewport.
            if let Some(focus_id) = interaction.focused_node_id
                && let Some(lb) = find_layout_box_by_id(&layout_root, focus_id).filter(|lb| {
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
                            input_values,
                            focus_id,
                            viewport.width.max(1.0),
                            &measurer,
                            lb.style,
                        );
                    }
                    Some(ReplacedKind::TextArea) => {
                        let (pad_l, pad_r, _pad_t, _pad_b) = input_text_padding(lb.style);
                        let available_text_w = (viewport.width - pad_l - pad_r).max(0.0);
                        let lines = ensure_textarea_layout_cache(
                            interaction,
                            &*input_values,
                            focus_id,
                            available_text_w,
                            &measurer,
                            lb.style,
                        );

                        sync_textarea_scroll_for_caret(
                            input_values,
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

                let focused_textarea_lines = focused.and_then(|id| {
                    interaction
                        .textarea_layout_cache
                        .as_ref()
                        .filter(|c| c.input_id == id)
                        .map(|c| c.lines.as_slice())
                });

                let paint_args = PaintArgs {
                    painter: &painter,
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
                paint_page(&layout_root, paint_args);
            }

            if let Some(focus_id) = interaction.focused_node_id
                && let Some(r) = fragment_rects.borrow().get(&focus_id).copied()
            {
                interaction.focused_input_rect = Some(r);
            }

            let input_out = route_frame_input(FrameInputCtx {
                ui,
                resp,
                content_rect,
                origin,
                layout_root: &layout_root,
                measurer: &measurer,
                layout_changed,
                fragment_rects: &fragment_rects,
                base_url,
                input_values,
                form_controls,
                interaction,
            });

            if input_out.request_repaint {
                ui.ctx().request_repaint();
            }

            input_out.action
        })
        .inner
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
