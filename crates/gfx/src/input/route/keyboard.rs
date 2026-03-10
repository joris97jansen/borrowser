use super::super::{InteractionState, to_input_id};
use crate::EguiTextMeasurer;
use crate::text_control::consume_focus_nav_keys;
use egui::{Event, Key, Ui};
use input_core::InputStore;
use layout::{HitKind, LayoutBox, Rectangle, ReplacedKind};

pub(super) fn handle_focused_keyboard_input<
    S: InputStore + ?Sized,
    F: super::FormControlHandler<S>,
>(
    ui: &mut Ui,
    layout_root: &LayoutBox<'_>,
    measurer: &EguiTextMeasurer,
    input_values: &mut S,
    form_controls: &F,
    interaction: &mut InteractionState,
) -> bool {
    let Some(focus_id) = interaction.focused_node_id else {
        return false;
    };

    let mut value_changed = false;
    let mut caret_or_selection_changed = false;
    let mut non_text_state_changed = false;
    let mut handled_activation = false;

    let focused_kind = interaction.focused_kind;
    let focused_replaced_kind = crate::text_control::find_layout_box_by_id(layout_root, focus_id)
        .and_then(|lb| lb.replaced);
    let is_textarea = matches!(focused_replaced_kind, Some(ReplacedKind::TextArea));

    let mut enter_pressed = false;
    let mut saw_text_newline = false;
    let activation_rect = interaction
        .focused_input_rect
        .or_else(|| {
            crate::text_control::find_layout_box_by_id(layout_root, focus_id).map(|lb| lb.rect)
        })
        .unwrap_or(Rectangle {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        });

    ui.input_mut(|i| {
        consume_focus_nav_keys(i);

        if matches!(focused_kind, Some(HitKind::Checkbox | HitKind::Radio)) {
            i.consume_key(egui::Modifiers::NONE, Key::Space);
            i.consume_key(egui::Modifiers::SHIFT, Key::Space);
        }
    });

    ui.input(|i| {
        for evt in &i.events {
            match focused_kind {
                Some(HitKind::Input) => match evt {
                    Event::Text(t) => {
                        if is_textarea {
                            interaction.textarea.clear_preferred_x();
                            saw_text_newline |= t.contains('\n') || t.contains('\r');
                            input_values.insert_text_multiline(to_input_id(focus_id), t.as_str());
                        } else {
                            input_values.insert_text(to_input_id(focus_id), t.as_str());
                        }
                        value_changed = true;
                    }
                    Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        if is_textarea {
                            let (value, caret, enter) = super::textarea::handle_linear_key_event(
                                input_values,
                                interaction,
                                focus_id,
                                *key,
                                *modifiers,
                            );
                            value_changed |= value;
                            caret_or_selection_changed |= caret;
                            enter_pressed |= enter;
                            if !caret && matches!(key, Key::ArrowUp | Key::ArrowDown) {
                                let delta = if *key == Key::ArrowUp { -1 } else { 1 };
                                caret_or_selection_changed |=
                                    super::textarea::move_caret_vertically(
                                        input_values,
                                        interaction,
                                        layout_root,
                                        focus_id,
                                        delta,
                                        measurer,
                                        *modifiers,
                                    );
                            }
                        } else {
                            let (value, caret) = super::text_input::handle_key_event(
                                input_values,
                                focus_id,
                                *key,
                                *modifiers,
                            );
                            value_changed |= value;
                            caret_or_selection_changed |= caret;
                        }
                    }
                    _ => {}
                },
                Some(HitKind::Checkbox) => {
                    if handled_activation {
                        continue;
                    }
                    match evt {
                        Event::Text(t) if t == " " => {
                            handled_activation = true;
                            non_text_state_changed |= super::actions::activate_checkbox(
                                input_values,
                                focus_id,
                                interaction,
                                activation_rect,
                            );
                        }
                        Event::Key {
                            key: Key::Space,
                            pressed: true,
                            ..
                        } => {
                            handled_activation = true;
                            non_text_state_changed |= super::actions::activate_checkbox(
                                input_values,
                                focus_id,
                                interaction,
                                activation_rect,
                            );
                        }
                        _ => {}
                    }
                }
                Some(HitKind::Radio) => {
                    if handled_activation {
                        continue;
                    }
                    match evt {
                        Event::Text(t) if t == " " => {
                            handled_activation = true;
                            non_text_state_changed |= super::actions::activate_radio(
                                form_controls,
                                input_values,
                                focus_id,
                                interaction,
                                activation_rect,
                            );
                        }
                        Event::Key {
                            key: Key::Space,
                            pressed: true,
                            ..
                        } => {
                            handled_activation = true;
                            non_text_state_changed |= super::actions::activate_radio(
                                form_controls,
                                input_values,
                                focus_id,
                                interaction,
                                activation_rect,
                            );
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    });

    if is_textarea && enter_pressed && !saw_text_newline {
        interaction.textarea.clear_preferred_x();
        input_values.insert_text_multiline(to_input_id(focus_id), "\n");
        value_changed = true;
    }

    let changed = value_changed || caret_or_selection_changed || non_text_state_changed;
    let needs_text_scroll_sync = value_changed || caret_or_selection_changed;

    if changed
        && needs_text_scroll_sync
        && matches!(focused_kind, Some(HitKind::Input))
        && let Some(lb) =
            crate::text_control::find_layout_box_by_id(layout_root, focus_id).filter(|lb| {
                matches!(
                    lb.replaced,
                    Some(ReplacedKind::InputText | ReplacedKind::TextArea)
                )
            })
    {
        let viewport = interaction.focused_input_rect.unwrap_or(lb.rect);
        match lb.replaced {
            Some(ReplacedKind::InputText) => super::text_input::sync_after_edit(
                input_values,
                focus_id,
                viewport.width,
                measurer,
                lb.style,
            ),
            Some(ReplacedKind::TextArea) => super::textarea::sync_after_edit(
                input_values,
                interaction,
                focus_id,
                viewport,
                measurer,
                lb.style,
            ),
            _ => {}
        }
    }

    changed
}
