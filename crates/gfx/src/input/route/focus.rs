use super::super::{InteractionState, to_input_id};
use super::{control_focus_rect, editable_layout_box};
use crate::EguiTextMeasurer;
use egui::{Rect, Sense, Ui};
use input_core::InputStore;
use layout::{HitKind, LayoutBox, ReplacedKind};

pub(super) fn handle_focusable_pointer_press<S: InputStore + ?Sized>(
    ui: &mut Ui,
    layout_root: &LayoutBox<'_>,
    measurer: &EguiTextMeasurer,
    input_values: &mut S,
    interaction: &mut InteractionState,
    hit: &layout::hit_test::HitResult,
) {
    let prev_focus_kind = interaction.focused_kind;
    let focus_changed = interaction.focused_node_id != Some(hit.node_id);
    if focus_changed
        && let Some(prev_focus) = interaction.focused_node_id
        && matches!(prev_focus_kind, Some(HitKind::Input))
    {
        input_values.blur(to_input_id(prev_focus));
    }

    match hit.kind {
        HitKind::Input => {
            input_values.ensure_initial(to_input_id(hit.node_id), String::new());
        }
        HitKind::Checkbox | HitKind::Radio => {
            input_values.ensure_initial_checked(to_input_id(hit.node_id), false);
        }
        _ => {}
    }
    interaction.set_focus(hit.node_id, hit.kind, hit.fragment_rect);

    if focus_changed && matches!(hit.kind, HitKind::Input) {
        input_values.focus(to_input_id(hit.node_id));
    }

    let egui_focus_id = ui.make_persistent_id(("dom-input", hit.node_id));
    ui.memory_mut(|mem| mem.request_focus(egui_focus_id));

    if matches!(hit.kind, HitKind::Input)
        && let Some(lb) = editable_layout_box(layout_root, hit.node_id)
    {
        let selecting = ui.input(|i| i.modifiers.shift);

        match lb.replaced {
            Some(ReplacedKind::InputText) => super::text_input::place_caret_from_pointer_hit(
                input_values,
                hit,
                measurer,
                lb.style,
                selecting,
            ),
            Some(ReplacedKind::TextArea) => super::textarea::place_caret_from_pointer_hit(
                input_values,
                interaction,
                hit,
                measurer,
                lb.style,
                selecting,
            ),
            _ => {}
        }
    }
}

pub(super) fn finalize_focus_change_after_release<S: InputStore + ?Sized>(
    ui: &mut Ui,
    input_values: &mut S,
    prev_focus: Option<html::internal::Id>,
    prev_focus_kind: Option<layout::HitKind>,
    interaction: &InteractionState,
) {
    if prev_focus != interaction.focused_node_id {
        if let Some(old) = prev_focus
            && matches!(prev_focus_kind, Some(HitKind::Input))
        {
            input_values.blur(to_input_id(old));
        }

        if let Some(old) = prev_focus {
            let old_egui_id = ui.make_persistent_id(("dom-input", old));
            ui.memory_mut(|mem| mem.surrender_focus(old_egui_id));
        }
    }
}

pub(super) fn maintain_egui_focus_bridge(
    ui: &mut Ui,
    content_rect: Rect,
    origin: egui::Pos2,
    layout_root: &LayoutBox<'_>,
    interaction: &InteractionState,
) -> Option<egui::Id> {
    let focus_id = interaction.focused_node_id?;
    let egui_focus_id = ui.make_persistent_id(("dom-input", focus_id));
    let rect = control_focus_rect(content_rect, origin, layout_root, interaction);

    ui.interact(rect, egui_focus_id, Sense::click());
    ui.memory_mut(|mem| {
        mem.request_focus(egui_focus_id);
        mem.set_focus_lock_filter(
            egui_focus_id,
            egui::EventFilter {
                tab: true,
                horizontal_arrows: true,
                vertical_arrows: true,
                escape: true,
            },
        );
    });

    Some(egui_focus_id)
}
