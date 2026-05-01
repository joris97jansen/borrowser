use crate::input_state::DocumentInputState;
use crate::page::PageState;
use crate::rendering::{
    OrchestratedFrameOutcome, PendingRenderWork, execute_prepared_page_frame, prepare_page_frame,
};
use crate::resources::ResourceManager;
use egui::{
    Align2, Area, CentralPanel, Color32, Context, CornerRadius, Frame, Id, Margin, Order, RichText,
    Stroke, vec2,
};
pub use gfx::input::PageAction;

pub(crate) struct ViewContentOutcome {
    pub(crate) action: Option<PageAction>,
    pub(crate) followup_render_request: Option<crate::rendering::RenderInvalidationRequest>,
    pub(crate) trace: Option<crate::rendering::RenderFrameExecutionTrace>,
}

pub(crate) fn content(
    ctx: &Context,
    page: &mut PageState,
    input_state: &mut DocumentInputState,
    resources: &ResourceManager,
    status: Option<&String>,
    loading: bool,
    pending_work: PendingRenderWork,
) -> ViewContentOutcome {
    if page.dom.is_none() {
        let visuals = ctx.style().visuals.clone();
        CentralPanel::default()
            .frame(Frame::default().fill(visuals.panel_fill))
            .show(ctx, |ui| {
                if loading {
                    ui.label("⏳ Loading…");
                }
                if let Some(s) = status {
                    ui.label(s);
                }
            });
        return ViewContentOutcome {
            action: None,
            followup_render_request: None,
            trace: None,
        };
    }

    let prepared_frame = match prepare_page_frame(page, pending_work) {
        Ok(Some(prepared_frame)) => prepared_frame,
        Ok(None) => {
            show_status_overlay(ctx, loading, status.map(|status| status.as_str()));
            return ViewContentOutcome {
                action: None,
                followup_render_request: None,
                trace: None,
            };
        }
        Err(error) => {
            let visuals = ctx.style().visuals.clone();
            CentralPanel::default()
                .frame(Frame::default().fill(visuals.panel_fill))
                .show(ctx, |ui| {
                    ui.label(format!("Style computation failed: {error}"));
                });
            show_status_overlay(ctx, loading, status.map(|status| status.as_str()));
            return ViewContentOutcome {
                action: None,
                followup_render_request: None,
                trace: None,
            };
        }
    };
    let base_fill = if let Some((r, g, b, a)) = prepared_frame.page_background {
        Color32::from_rgba_unmultiplied(r, g, b, a)
    } else {
        Color32::WHITE
    };
    let frame_outcome = CentralPanel::default()
        .frame(Frame::default().fill(base_fill))
        .show(ctx, |ui| {
            execute_prepared_page_frame(ui, prepared_frame, input_state, resources)
        })
        .inner;
    let OrchestratedFrameOutcome {
        action,
        followup_render_request,
        trace,
    } = frame_outcome;
    show_status_overlay(ctx, loading, status.map(|status| status.as_str()));
    ViewContentOutcome {
        action,
        followup_render_request,
        trace: Some(trace),
    }
}

fn show_status_overlay(ctx: &Context, loading: bool, status: Option<&str>) {
    let lines = overlay_lines(loading, status);
    if lines.is_empty() {
        return;
    }

    Area::new(Id::new("page_status_overlay"))
        .order(Order::Foreground)
        .anchor(Align2::RIGHT_TOP, vec2(-16.0, 16.0))
        .interactable(false)
        .show(ctx, |ui| {
            Frame::new()
                .fill(Color32::from_black_alpha(208))
                .stroke(Stroke::new(1.0, Color32::from_white_alpha(40)))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(8))
                .show(ui, |ui| {
                    ui.set_max_width(420.0);
                    for (idx, line) in lines.iter().enumerate() {
                        let text = if idx == 0 && loading {
                            RichText::new(line).strong().color(Color32::WHITE)
                        } else {
                            RichText::new(line).color(Color32::WHITE)
                        };
                        ui.label(text);
                    }
                });
        });
}

fn overlay_lines(loading: bool, status: Option<&str>) -> Vec<String> {
    let mut lines = Vec::new();
    if loading {
        lines.push("Loading…".to_string());
    }
    if let Some(status) = status {
        lines.push(status.to_string());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::overlay_lines;

    #[test]
    fn overlay_lines_include_loading_and_status() {
        let lines = overlay_lines(true, Some("Document parsed • HTTP 200"));
        assert_eq!(lines, vec!["Loading…", "Document parsed • HTTP 200"]);
    }

    #[test]
    fn overlay_lines_are_empty_without_loading_or_status() {
        assert!(overlay_lines(false, None).is_empty());
    }
}
