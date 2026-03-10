use super::super::{ActiveTarget, InputDragState, InteractionState, PageAction};
use super::{FragmentRects, focus};
use crate::EguiTextMeasurer;
use egui::{Pos2, Rect, Response, Ui};
use input_core::InputStore;
use layout::{
    HitKind, LayoutBox, ReplacedKind,
    hit_test::{HitResult, hit_test},
};

pub(super) struct PointerCtx<'a, 'layout> {
    pub(super) ui: &'a mut Ui,
    pub(super) resp: &'a Response,
    pub(super) content_rect: Rect,
    pub(super) origin: Pos2,
    pub(super) layout_root: &'a LayoutBox<'layout>,
    pub(super) measurer: &'a EguiTextMeasurer,
}

pub(super) struct PointerReleaseOutcome {
    pub(super) action: Option<PageAction>,
    pub(super) request_repaint: bool,
}

pub(super) fn pointer_pos(resp: &Response, ui: &Ui, allow_latest_pos: bool) -> Option<Pos2> {
    // Prefer response-scoped positions when available, fall back to the global pointer.
    resp.interact_pointer_pos()
        .or_else(|| resp.hover_pos())
        .or_else(|| {
            ui.input(|i| {
                if allow_latest_pos {
                    i.pointer.interact_pos().or(i.pointer.latest_pos())
                } else {
                    i.pointer.interact_pos()
                }
            })
        })
}

pub(super) fn hit_at_pointer(
    resp: &Response,
    ui: &Ui,
    content_rect: Rect,
    origin: Pos2,
    layout_root: &LayoutBox<'_>,
    measurer: &EguiTextMeasurer,
    allow_latest_pos: bool,
) -> Option<HitResult> {
    let pos = pointer_pos(resp, ui, allow_latest_pos)?;
    if !content_rect.contains(pos) {
        return None;
    }
    let lx = pos.x - origin.x;
    let ly = pos.y - origin.y;
    hit_test(layout_root, (lx, ly), measurer)
}

pub(super) fn handle_pointer_press<S: InputStore + ?Sized>(
    ctx: PointerCtx<'_, '_>,
    input_values: &mut S,
    interaction: &mut InteractionState,
) -> bool {
    let PointerCtx {
        ui,
        resp,
        content_rect,
        origin,
        layout_root,
        measurer,
    } = ctx;
    if !ui.input(|i| i.pointer.primary_pressed()) {
        return false;
    }

    let pressed_hit = hit_at_pointer(resp, ui, content_rect, origin, layout_root, measurer, true);
    interaction.active = pressed_hit.as_ref().map(|h| ActiveTarget {
        id: h.node_id,
        kind: h.kind,
    });
    interaction.input_drag = None;

    if let Some(hit) = pressed_hit
        && matches!(
            hit.kind,
            HitKind::Input | HitKind::Checkbox | HitKind::Radio
        )
    {
        focus::handle_focusable_pointer_press(
            ui,
            layout_root,
            measurer,
            input_values,
            interaction,
            &hit,
        );

        if matches!(hit.kind, HitKind::Input) {
            interaction.input_drag = Some(InputDragState {
                input_id: hit.node_id,
                rect: hit.fragment_rect,
            });
        }
        return true;
    }

    false
}

pub(super) fn handle_pointer_drag<S: InputStore + ?Sized>(
    ctx: PointerCtx<'_, '_>,
    layout_changed: bool,
    fragment_rects: &FragmentRects,
    input_values: &mut S,
    interaction: &mut InteractionState,
) -> bool {
    let PointerCtx {
        ui,
        resp,
        origin,
        layout_root,
        measurer,
        ..
    } = ctx;
    if !ui.input(|i| i.pointer.primary_down()) {
        return false;
    }

    let focused_id = interaction.focused_node_id;
    let focused_rect = interaction.focused_input_rect;
    let Some(pos) = pointer_pos(resp, ui, true) else {
        return false;
    };
    let Some((drag_input_id, prev_rect)) = interaction
        .input_drag
        .as_ref()
        .map(|d| (d.input_id, d.rect))
    else {
        return false;
    };

    let rect = if layout_changed {
        fragment_rects
            .borrow()
            .get(&drag_input_id)
            .copied()
            .or(focused_rect.filter(|_| focused_id == Some(drag_input_id)))
            .unwrap_or(prev_rect)
    } else {
        prev_rect
    };
    if let Some(drag) = interaction.input_drag.as_mut() {
        drag.rect = rect;
    }

    let lx = pos.x - origin.x;
    let local_x = (lx - rect.x).clamp(0.0, rect.width);
    let ly = pos.y - origin.y;
    let local_y = (ly - rect.y).clamp(0.0, rect.height);

    let Some(lb) =
        crate::text_control::find_layout_box_by_id(layout_root, drag_input_id).filter(|lb| {
            matches!(
                lb.replaced,
                Some(ReplacedKind::InputText | ReplacedKind::TextArea)
            )
        })
    else {
        return false;
    };

    match lb.replaced {
        Some(ReplacedKind::InputText) => {
            super::text_input::drag_selection(
                input_values,
                drag_input_id,
                local_x,
                rect.width,
                measurer,
                lb.style,
            );
            true
        }
        Some(ReplacedKind::TextArea) => {
            super::textarea::drag_selection(
                input_values,
                interaction,
                super::textarea::TextareaDragParams {
                    input_id: drag_input_id,
                    local_x,
                    local_y,
                    viewport_width: rect.width,
                    viewport_height: rect.height,
                    style: lb.style,
                },
                measurer,
            );
            true
        }
        _ => false,
    }
}

pub(super) fn handle_pointer_release<S: InputStore + ?Sized, F: super::FormControlHandler<S>>(
    ctx: PointerCtx<'_, '_>,
    base_url: Option<&str>,
    input_values: &mut S,
    form_controls: &F,
    interaction: &mut InteractionState,
) -> PointerReleaseOutcome {
    let PointerCtx {
        ui,
        resp,
        content_rect,
        origin,
        layout_root,
        measurer,
    } = ctx;
    if !ui.input(|i| i.pointer.primary_released()) {
        return PointerReleaseOutcome {
            action: None,
            request_repaint: false,
        };
    }

    let prev_focus = interaction.focused_node_id;
    let prev_focus_kind = interaction.focused_kind;
    let drag_input_id = interaction.input_drag.as_ref().map(|d| d.input_id);
    interaction.input_drag = None;

    let release_hit = hit_at_pointer(resp, ui, content_rect, origin, layout_root, measurer, false);
    let was_active = interaction.active;
    let gesture_started_in_text_input = matches!(
        was_active,
        Some(ActiveTarget {
            kind: HitKind::Input,
            ..
        })
    ) || drag_input_id.is_some();
    let gesture_started_in_toggle_control = matches!(
        was_active,
        Some(ActiveTarget {
            kind: HitKind::Checkbox | HitKind::Radio,
            ..
        })
    );

    let mut request_repaint = false;
    let mut action = None;

    if !gesture_started_in_text_input {
        match release_hit {
            None => {
                if !gesture_started_in_toggle_control {
                    interaction.clear_focus();
                }
            }
            Some(hit) => {
                let down_matches_up =
                    was_active.is_some_and(|a| a.id == hit.node_id && a.kind == hit.kind);

                if down_matches_up {
                    let activation = super::actions::activate_release_target(
                        hit,
                        base_url,
                        input_values,
                        form_controls,
                        interaction,
                    );
                    request_repaint |= activation.request_repaint;
                    action = activation.action;
                } else if !gesture_started_in_toggle_control
                    && !matches!(
                        hit.kind,
                        HitKind::Input | HitKind::Checkbox | HitKind::Radio
                    )
                {
                    interaction.clear_focus();
                }
            }
        }
    }

    focus::finalize_focus_change_after_release(
        ui,
        input_values,
        prev_focus,
        prev_focus_kind,
        interaction,
    );
    interaction.active = None;

    PointerReleaseOutcome {
        action,
        request_repaint,
    }
}
