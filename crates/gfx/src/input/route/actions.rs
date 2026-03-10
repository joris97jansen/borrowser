use super::super::{InteractionState, PageAction, to_input_id};
use crate::util::resolve_relative_url;
use input_core::InputStore;
use layout::HitKind;
use layout::hit_test::HitResult;

pub(super) struct ActivationResult {
    pub(super) action: Option<PageAction>,
    pub(super) request_repaint: bool,
}

pub(super) fn activate_release_target<S: InputStore + ?Sized, F>(
    hit: HitResult,
    base_url: Option<&str>,
    input_values: &mut S,
    form_controls: &F,
    interaction: &mut InteractionState,
) -> ActivationResult
where
    F: super::FormControlHandler<S>,
{
    match hit.kind {
        HitKind::Link => {
            let action = hit
                .href
                .as_deref()
                .and_then(|href| resolve_relative_url(base_url, href).map(PageAction::Navigate));
            interaction.clear_focus();
            ActivationResult {
                action,
                request_repaint: false,
            }
        }
        HitKind::Checkbox => ActivationResult {
            action: None,
            request_repaint: activate_checkbox(
                input_values,
                hit.node_id,
                interaction,
                hit.fragment_rect,
            ),
        },
        HitKind::Radio => ActivationResult {
            action: None,
            request_repaint: activate_radio(
                form_controls,
                input_values,
                hit.node_id,
                interaction,
                hit.fragment_rect,
            ),
        },
        HitKind::Button => {
            interaction.clear_focus();
            ActivationResult {
                action: None,
                request_repaint: true,
            }
        }
        _ => {
            interaction.clear_focus();
            ActivationResult {
                action: None,
                request_repaint: false,
            }
        }
    }
}

pub(super) fn activate_checkbox<S: InputStore + ?Sized>(
    input_values: &mut S,
    node_id: html::internal::Id,
    interaction: &mut InteractionState,
    rect: layout::Rectangle,
) -> bool {
    let changed = input_values.toggle_checked(to_input_id(node_id));
    interaction.set_focus(node_id, HitKind::Checkbox, rect);
    changed
}

pub(super) fn activate_radio<S: InputStore + ?Sized, F: super::FormControlHandler<S>>(
    form_controls: &F,
    input_values: &mut S,
    node_id: html::internal::Id,
    interaction: &mut InteractionState,
    rect: layout::Rectangle,
) -> bool {
    let changed = form_controls.on_radio_clicked(input_values, to_input_id(node_id));
    interaction.set_focus(node_id, HitKind::Radio, rect);
    changed
}
