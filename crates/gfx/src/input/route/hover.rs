use super::super::InteractionState;
use crate::EguiTextMeasurer;
use egui::{CursorIcon, Rect, Response, Ui, Vec2};
use layout::{HitKind, LayoutBox, hit_test::hit_test};

pub(super) struct HoverCtx<'a, 'layout> {
    pub(super) ui: &'a mut Ui,
    pub(super) resp: &'a Response,
    pub(super) content_rect: Rect,
    pub(super) origin: egui::Pos2,
    pub(super) layout_root: &'a LayoutBox<'layout>,
    pub(super) measurer: &'a EguiTextMeasurer,
    pub(super) layout_changed: bool,
    pub(super) interaction: &'a mut InteractionState,
}

pub(super) fn update_hover_and_cursor(ctx: HoverCtx<'_, '_>) {
    let HoverCtx {
        ui,
        resp,
        content_rect,
        origin,
        layout_root,
        measurer,
        layout_changed,
        interaction,
    } = ctx;
    // Hover hit-testing can be expensive (inline layout), so only recompute when needed.
    let hover_pos = resp.hover_pos().filter(|pos| content_rect.contains(*pos));
    let hover_needs_update = layout_changed
        || ui.input(|i| {
            i.pointer.delta() != Vec2::ZERO
                || i.pointer.motion().is_some_and(|m| m != Vec2::ZERO)
                || i.raw_scroll_delta != Vec2::ZERO
                || i.smooth_scroll_delta != Vec2::ZERO
        });

    let hover_hit = if hover_needs_update {
        hover_pos.and_then(|pos| {
            let lx = pos.x - origin.x;
            let ly = pos.y - origin.y;
            hit_test(layout_root, (lx, ly), measurer)
        })
    } else {
        None
    };

    if hover_needs_update {
        interaction.hover = hover_hit.as_ref().map(|h| h.node_id);
        interaction.hover_kind = hover_hit.as_ref().map(|h| h.kind);
    } else if hover_pos.is_none() {
        interaction.hover = None;
        interaction.hover_kind = None;
    }

    if let Some(kind) = hover_hit
        .as_ref()
        .map(|h| h.kind)
        .or(interaction.hover_kind)
    {
        apply_cursor_icon(ui, kind);
    }
}

fn apply_cursor_icon(ui: &mut Ui, kind: HitKind) {
    match kind {
        HitKind::Link | HitKind::Checkbox | HitKind::Radio | HitKind::Button => {
            ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
        }
        HitKind::Input => {
            ui.output_mut(|o| o.cursor_icon = CursorIcon::Text);
        }
        _ => {}
    }
}
