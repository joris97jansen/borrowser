mod actions;
mod focus;
mod hover;
mod keyboard;
mod pointer;
mod text_input;
mod textarea;
mod types;

pub use types::FormControlHandler;
pub(crate) use types::FrameInputCtx;

use super::{InteractionState, PageAction};
use egui::Rect;
use html::internal::Id;
use input_core::InputStore;
use layout::{LayoutBox, Rectangle};
use std::cell::RefCell;
use std::collections::HashMap;

#[cfg(test)]
mod tests;

pub(crate) fn route_frame_input<S: InputStore + ?Sized, F: FormControlHandler<S>>(
    ctx: FrameInputCtx<'_, '_, S, F>,
) -> Option<PageAction> {
    let FrameInputCtx {
        ui,
        resp,
        content_rect,
        origin,
        layout_root,
        measurer,
        layout_changed,
        fragment_rects,
        base_url,
        input_values,
        form_controls,
        interaction,
    } = ctx;

    let mut request_repaint = false;

    refresh_focused_input_rect(interaction, fragment_rects, layout_changed);
    hover::update_hover_and_cursor(hover::HoverCtx {
        ui,
        resp: &resp,
        content_rect,
        origin,
        layout_root,
        measurer,
        layout_changed,
        interaction,
    });

    request_repaint |= pointer::handle_pointer_press(
        pointer::PointerCtx {
            ui,
            resp: &resp,
            content_rect,
            origin,
            layout_root,
            measurer,
        },
        input_values,
        interaction,
    );

    request_repaint |= pointer::handle_pointer_drag(
        pointer::PointerCtx {
            ui,
            resp: &resp,
            content_rect,
            origin,
            layout_root,
            measurer,
        },
        layout_changed,
        fragment_rects,
        input_values,
        interaction,
    );

    let release = pointer::handle_pointer_release(
        pointer::PointerCtx {
            ui,
            resp: &resp,
            content_rect,
            origin,
            layout_root,
            measurer,
        },
        base_url,
        input_values,
        form_controls,
        interaction,
    );
    request_repaint |= release.request_repaint;
    let action = release.action;

    if let Some(egui_focus_id) =
        focus::maintain_egui_focus_bridge(ui, content_rect, origin, layout_root, interaction)
        && ui.memory(|mem| mem.has_focus(egui_focus_id))
    {
        request_repaint |= keyboard::handle_focused_keyboard_input(
            ui,
            layout_root,
            measurer,
            input_values,
            form_controls,
            interaction,
        );
    }

    if request_repaint {
        ui.ctx().request_repaint();
    }
    action
}

pub(super) fn refresh_focused_input_rect(
    interaction: &mut InteractionState,
    fragment_rects: &RefCell<HashMap<Id, Rectangle>>,
    layout_changed: bool,
) {
    // Prefer the painted fragment rect for the focused control when available.
    if let Some(focus_id) = interaction.focused_node_id {
        if let Some(r) = fragment_rects.borrow().get(&focus_id).copied() {
            interaction.focused_input_rect = Some(r);
        } else if layout_changed {
            interaction.focused_input_rect = None;
        }
    }
}

pub(super) fn editable_layout_box<'a>(
    layout_root: &'a LayoutBox<'a>,
    node_id: Id,
) -> Option<&'a LayoutBox<'a>> {
    crate::text_control::find_layout_box_by_id(layout_root, node_id).filter(|lb| {
        matches!(
            lb.replaced,
            Some(layout::ReplacedKind::InputText | layout::ReplacedKind::TextArea)
        )
    })
}

pub(super) fn control_focus_rect(
    content_rect: Rect,
    origin: egui::Pos2,
    layout_root: &LayoutBox<'_>,
    interaction: &InteractionState,
) -> Rect {
    if let Some(fr) = interaction.focused_input_rect {
        return Rect::from_min_size(
            egui::Pos2 {
                x: origin.x + fr.x,
                y: origin.y + fr.y,
            },
            egui::Vec2 {
                x: fr.width.max(1.0),
                y: fr.height.max(1.0),
            },
        );
    }

    if let Some(focus_id) = interaction.focused_node_id
        && let Some(lb) =
            crate::text_control::find_layout_box_by_id(layout_root, focus_id).filter(|lb| {
                matches!(
                    lb.replaced,
                    Some(
                        layout::ReplacedKind::InputText
                            | layout::ReplacedKind::TextArea
                            | layout::ReplacedKind::InputCheckbox
                            | layout::ReplacedKind::InputRadio
                    )
                )
            })
    {
        return Rect::from_min_size(
            egui::Pos2 {
                x: origin.x + lb.rect.x,
                y: origin.y + lb.rect.y,
            },
            egui::Vec2 {
                x: lb.rect.width.max(1.0),
                y: lb.rect.height.max(1.0),
            },
        );
    }

    content_rect
}

pub(super) type FragmentRects = RefCell<HashMap<Id, Rectangle>>;
