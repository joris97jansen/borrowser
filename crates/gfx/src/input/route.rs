use super::{ActiveTarget, InputDragState, InputValueStore, InteractionState, PageAction};
use crate::EguiTextMeasurer;
use crate::text_control::{
    consume_focus_nav_keys, find_layout_box_by_id, sync_input_scroll_for_caret,
};
use crate::textarea::{
    TextareaVerticalMoveCtx, sync_textarea_scroll_for_caret, textarea_caret_for_x_in_lines,
    textarea_line_index_from_y, textarea_move_caret_vertically,
};
use crate::util::{input_text_padding, resolve_relative_url};
use egui::{CursorIcon, Event, Key, Pos2, Rect, Sense, Ui, Vec2};
use html::Id;
use layout::{
    HitKind, LayoutBox, Rectangle, ReplacedKind, TextMeasurer,
    hit_test::{HitResult, hit_test},
};
use std::cell::RefCell;
use std::collections::HashMap;

pub trait FormControlHandler {
    fn on_radio_clicked(&self, store: &mut InputValueStore, radio_id: Id) -> bool;
}

pub(crate) struct FrameInputCtx<'a, 'layout, F> {
    pub ui: &'a mut Ui,
    pub resp: egui::Response,
    pub content_rect: Rect,
    pub origin: Pos2,
    pub layout_root: &'a LayoutBox<'layout>,
    pub measurer: &'a EguiTextMeasurer,
    pub layout_changed: bool,
    pub fragment_rects: &'a RefCell<HashMap<Id, Rectangle>>,
    pub base_url: Option<&'a str>,
    pub input_values: &'a mut InputValueStore,
    pub form_controls: &'a F,
    pub interaction: &'a mut InteractionState,
}

pub(crate) fn route_frame_input<F: FormControlHandler>(
    ctx: FrameInputCtx<'_, '_, F>,
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
    let mut action: Option<PageAction> = None;

    // Prefer the painted fragment rect for the focused control when available.
    if let Some(focus_id) = interaction.focused_node_id
        && let Some(r) = fragment_rects.borrow().get(&focus_id).copied()
    {
        interaction.focused_input_rect = Some(r);
    }

    let pointer_pos = |ui: &Ui, allow_latest_pos: bool| -> Option<Pos2> {
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
    };

    // Helper: hit-test at current pointer position (layout coords).
    // On release we avoid `latest_pos()` to prevent stale-position clicks.
    let hit_at_pointer = |ui: &Ui, allow_latest_pos: bool| -> Option<HitResult> {
        let pos = pointer_pos(ui, allow_latest_pos)?;

        if !content_rect.contains(pos) {
            return None;
        }

        let lx = pos.x - origin.x;
        let ly = pos.y - origin.y;
        hit_test(layout_root, (lx, ly), measurer)
    };

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

    // Cursor icon hint
    if let Some(kind) = hover_hit
        .as_ref()
        .map(|h| h.kind)
        .or(interaction.hover_kind)
    {
        match kind {
            HitKind::Link => {
                ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
            }
            HitKind::Input => {
                ui.output_mut(|o| o.cursor_icon = CursorIcon::Text);
            }
            HitKind::Checkbox | HitKind::Radio => {
                ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
            }
            HitKind::Button => {
                ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
            }
            _ => {}
        }
    }

    // Pointer down -> active (+ focus)
    if ui.input(|i| i.pointer.primary_pressed()) {
        let pressed_hit = hit_at_pointer(ui, true);
        interaction.active = pressed_hit.as_ref().map(|h| ActiveTarget {
            id: h.node_id,
            kind: h.kind,
        });
        interaction.input_drag = None;

        if let Some(h) = pressed_hit
            && matches!(h.kind, HitKind::Input | HitKind::Checkbox | HitKind::Radio)
        {
            let prev_focus_kind = interaction.focused_kind;
            let focus_changed = interaction.focused_node_id != Some(h.node_id);
            if focus_changed
                && let Some(prev_focus) = interaction.focused_node_id
                && matches!(prev_focus_kind, Some(HitKind::Input))
            {
                input_values.blur(prev_focus);
            }

            match h.kind {
                HitKind::Input => {
                    input_values.ensure_initial(h.node_id, String::new());
                }
                HitKind::Checkbox | HitKind::Radio => {
                    input_values.ensure_initial_checked(h.node_id, false);
                }
                _ => {}
            }
            interaction.set_focus(h.node_id, h.kind, h.fragment_rect);

            if focus_changed && matches!(h.kind, HitKind::Input) {
                input_values.focus(h.node_id);
            }

            let egui_focus_id = ui.make_persistent_id(("dom-input", h.node_id));
            ui.memory_mut(|mem| mem.request_focus(egui_focus_id));

            if matches!(h.kind, HitKind::Input) {
                if let Some(lb) = find_layout_box_by_id(layout_root, h.node_id).filter(|lb| {
                    matches!(
                        lb.replaced,
                        Some(ReplacedKind::InputText | ReplacedKind::TextArea)
                    )
                }) {
                    let style = lb.style;
                    let selecting = ui.input(|i| i.modifiers.shift);

                    match lb.replaced {
                        Some(ReplacedKind::InputText) => {
                            let (pad_l, _pad_r, _pad_t, _pad_b) = input_text_padding(style);

                            let x_in_viewport = (h.local_pos.0 - pad_l).max(0.0);
                            input_values.set_caret_from_viewport_x(
                                h.node_id,
                                x_in_viewport,
                                selecting,
                                |s| measurer.measure(s, style),
                            );
                            sync_input_scroll_for_caret(
                                input_values,
                                h.node_id,
                                h.fragment_rect.width.max(1.0),
                                measurer,
                                style,
                            );
                        }
                        Some(ReplacedKind::TextArea) => {
                            let (pad_l, pad_r, pad_t, _pad_b) = input_text_padding(style);

                            let available_text_w = (h.fragment_rect.width - pad_l - pad_r).max(0.0);
                            {
                                let lines = interaction.textarea.ensure_layout_cache(
                                    &*input_values,
                                    h.node_id,
                                    available_text_w,
                                    measurer,
                                    style,
                                );

                                let caret = {
                                    let (value, scroll_y) = input_values
                                        .get_state(h.node_id)
                                        .map(|(v, _c, _sel, _sx, sy)| (v, sy))
                                        .unwrap_or(("", 0.0));

                                    let y_in_viewport = (h.local_pos.1 - pad_t).max(0.0);
                                    let y_in_text = y_in_viewport + scroll_y;

                                    let line_h = measurer.line_height(style);
                                    let line_idx =
                                        textarea_line_index_from_y(lines, y_in_text, line_h);

                                    let x_in_viewport = (h.local_pos.0 - pad_l).max(0.0);
                                    textarea_caret_for_x_in_lines(
                                        lines,
                                        value,
                                        line_idx,
                                        x_in_viewport,
                                    )
                                };

                                input_values.set_caret(h.node_id, caret, selecting);

                                sync_textarea_scroll_for_caret(
                                    input_values,
                                    h.node_id,
                                    h.fragment_rect.height.max(1.0),
                                    lines,
                                    measurer,
                                    style,
                                );
                            }
                        }
                        _ => {}
                    }
                }

                interaction.input_drag = Some(InputDragState {
                    input_id: h.node_id,
                    rect: h.fragment_rect,
                });
            }

            request_repaint = true;
        }
    }

    // Pointer drag -> selection update for focused input
    if ui.input(|i| i.pointer.primary_down()) {
        let focused_id = interaction.focused_node_id;
        let focused_rect = interaction.focused_input_rect;

        if let Some(pos) = pointer_pos(ui, true)
            && let Some((drag_input_id, prev_rect)) = interaction
                .input_drag
                .as_ref()
                .map(|d| (d.input_id, d.rect))
        {
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

            if let Some(lb) = find_layout_box_by_id(layout_root, drag_input_id).filter(|lb| {
                matches!(
                    lb.replaced,
                    Some(ReplacedKind::InputText | ReplacedKind::TextArea)
                )
            }) {
                let style = lb.style;

                match lb.replaced {
                    Some(ReplacedKind::InputText) => {
                        let (pad_l, _pad_r, _pad_t, _pad_b) = input_text_padding(style);

                        input_values.set_caret_from_viewport_x(
                            drag_input_id,
                            (local_x - pad_l).max(0.0),
                            true,
                            |s| measurer.measure(s, style),
                        );
                        sync_input_scroll_for_caret(
                            input_values,
                            drag_input_id,
                            rect.width.max(1.0),
                            measurer,
                            style,
                        );

                        request_repaint = true;
                    }
                    Some(ReplacedKind::TextArea) => {
                        interaction.textarea.clear_preferred_x();
                        let (pad_l, pad_r, pad_t, _pad_b) = input_text_padding(style);

                        let available_text_w = (rect.width - pad_l - pad_r).max(0.0);
                        {
                            let lines = interaction.textarea.ensure_layout_cache(
                                &*input_values,
                                drag_input_id,
                                available_text_w,
                                measurer,
                                style,
                            );

                            let caret = {
                                let (value, scroll_y) = input_values
                                    .get_state(drag_input_id)
                                    .map(|(v, _c, _sel, _sx, sy)| (v, sy))
                                    .unwrap_or(("", 0.0));

                                let y_in_viewport = (local_y - pad_t).max(0.0);
                                let y_in_text = y_in_viewport + scroll_y;

                                let line_h = measurer.line_height(style);
                                let line_idx = textarea_line_index_from_y(lines, y_in_text, line_h);

                                let x_in_viewport = (local_x - pad_l).max(0.0);
                                textarea_caret_for_x_in_lines(lines, value, line_idx, x_in_viewport)
                            };

                            input_values.set_caret(drag_input_id, caret, true);

                            sync_textarea_scroll_for_caret(
                                input_values,
                                drag_input_id,
                                rect.height.max(1.0),
                                lines,
                                measurer,
                                style,
                            );
                        }

                        request_repaint = true;
                    }
                    _ => {}
                }
            }
        }
    }

    // Pointer up -> action/focus (if released on same target)
    if ui.input(|i| i.pointer.primary_released()) {
        let prev_focus = interaction.focused_node_id;
        let prev_focus_kind = interaction.focused_kind;

        let drag_input_id = interaction.input_drag.as_ref().map(|d| d.input_id);
        interaction.input_drag = None;

        let release_hit = hit_at_pointer(ui, false);

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

        if !gesture_started_in_text_input {
            match release_hit {
                None => {
                    // Released outside: keep focus if we started on a focusable control.
                    if !gesture_started_in_toggle_control {
                        interaction.clear_focus();
                    }
                }
                Some(h) => {
                    let down_matches_up =
                        was_active.is_some_and(|a| a.id == h.node_id && a.kind == h.kind);

                    if down_matches_up {
                        match h.kind {
                            HitKind::Link => {
                                if let Some(href) = h.href.as_deref() {
                                    if let Some(url) = resolve_relative_url(base_url, href) {
                                        action = Some(PageAction::Navigate(url));
                                    }
                                } else {
                                    // debug: link hit but no href
                                    #[cfg(debug_assertions)]
                                    eprintln!("Link hit {:?} but no href in HitResult", h.node_id);
                                }
                                // Clicking a link should clear input focus (browser-like)
                                interaction.clear_focus();
                            }

                            HitKind::Checkbox => {
                                let changed = input_values.toggle_checked(h.node_id);

                                // Checkbox remains focused after activation (browser-like)
                                interaction.set_focus(h.node_id, h.kind, h.fragment_rect);
                                if changed {
                                    request_repaint = true;
                                }
                            }

                            HitKind::Radio => {
                                let changed =
                                    form_controls.on_radio_clicked(input_values, h.node_id);

                                // Radio remains focused after activation (browser-like)
                                interaction.set_focus(h.node_id, h.kind, h.fragment_rect);
                                if changed {
                                    request_repaint = true;
                                }
                            }

                            HitKind::Button => {
                                #[cfg(debug_assertions)]
                                eprintln!("button click: {:?}", h.node_id);

                                // Clicking a button should blur input focus (browser-like)
                                interaction.clear_focus();

                                request_repaint = true;
                            }

                            _ => {
                                interaction.clear_focus();
                            }
                        }
                    } else {
                        // If the pointer gesture did *not* begin on the current release target,
                        // we still blur when releasing on non-input content, but never blur just
                        // because the mouse-up happened to land inside an input.
                        if !gesture_started_in_toggle_control
                            && !matches!(
                                h.kind,
                                HitKind::Input | HitKind::Checkbox | HitKind::Radio
                            )
                        {
                            interaction.clear_focus();
                        }
                    }
                }
            }
        }

        if prev_focus != interaction.focused_node_id {
            // If focus changed due to this pointer release, clear selection on the old input.
            if let Some(old) = prev_focus
                && matches!(prev_focus_kind, Some(HitKind::Input))
            {
                input_values.blur(old);
            }

            if let Some(old) = prev_focus {
                let old_egui_id = ui.make_persistent_id(("dom-input", old));
                ui.memory_mut(|mem| mem.surrender_focus(old_egui_id));
            }
        }

        // Default behavior: any pointer release clears active.
        interaction.active = None;
    }

    // --- keep an egui focus target alive for the focused DOM control (MUST be before key handling)
    if let Some(focus_id) = interaction.focused_node_id {
        let egui_focus_id = ui.make_persistent_id(("dom-input", focus_id));

        // Default fallback: keep focusable alive on the whole content rect
        let mut r = content_rect;

        // Prefer the painted inline fragment rect, fall back to the layout box rect.
        if let Some(fr) = interaction.focused_input_rect {
            r = Rect::from_min_size(
                Pos2 {
                    x: origin.x + fr.x,
                    y: origin.y + fr.y,
                },
                Vec2 {
                    x: fr.width.max(1.0),
                    y: fr.height.max(1.0),
                },
            );
        } else if let Some(lb) = find_layout_box_by_id(layout_root, focus_id).filter(|lb| {
            matches!(
                lb.replaced,
                Some(
                    ReplacedKind::InputText
                        | ReplacedKind::TextArea
                        | ReplacedKind::InputCheckbox
                        | ReplacedKind::InputRadio
                )
            )
        }) {
            r = Rect::from_min_size(
                Pos2 {
                    x: origin.x + lb.rect.x,
                    y: origin.y + lb.rect.y,
                },
                Vec2 {
                    x: lb.rect.width.max(1.0),
                    y: lb.rect.height.max(1.0),
                },
            );
        }

        ui.interact(r, egui_focus_id, Sense::click());

        // Keep egui focus "sticky" while a DOM input is focused, and lock focus
        // navigation keys (arrows/tab/escape) so egui doesn't move focus to e.g. the URL bar.
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

        // --- Key input -> focused input (AFTER interact exists)
        if ui.memory(|mem| mem.has_focus(egui_focus_id)) {
            let mut value_changed = false;
            let mut caret_or_selection_changed = false;
            let mut non_text_state_changed = false;
            let mut handled_activation = false;

            let focused_kind = interaction.focused_kind;
            let focused_replaced_kind =
                find_layout_box_by_id(layout_root, focus_id).and_then(|lb| lb.replaced);
            let is_textarea = matches!(focused_replaced_kind, Some(ReplacedKind::TextArea));

            let mut enter_pressed = false;
            let mut saw_text_newline = false;

            // 1) consume nav keys first
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
                                    input_values.insert_text_multiline(focus_id, t.as_str());
                                } else {
                                    input_values.insert_text(focus_id, t.as_str());
                                }
                                value_changed = true;
                            }
                            Event::Key {
                                key,
                                pressed: true,
                                modifiers,
                                ..
                            } => match key {
                                Key::Enter => {
                                    if is_textarea {
                                        enter_pressed = true;
                                    }
                                }
                                Key::Backspace => {
                                    if is_textarea {
                                        interaction.textarea.clear_preferred_x();
                                    }
                                    input_values.backspace(focus_id);
                                    value_changed = true;
                                }
                                Key::Delete => {
                                    if is_textarea {
                                        interaction.textarea.clear_preferred_x();
                                    }
                                    input_values.delete(focus_id);
                                    value_changed = true;
                                }
                                Key::ArrowLeft => {
                                    if is_textarea {
                                        interaction.textarea.clear_preferred_x();
                                    }
                                    input_values.move_caret_left(focus_id, modifiers.shift);
                                    caret_or_selection_changed = true;
                                }
                                Key::ArrowRight => {
                                    if is_textarea {
                                        interaction.textarea.clear_preferred_x();
                                    }
                                    input_values.move_caret_right(focus_id, modifiers.shift);
                                    caret_or_selection_changed = true;
                                }
                                Key::ArrowUp => {
                                    if is_textarea
                                        && let Some(lb) = find_layout_box_by_id(
                                            layout_root,
                                            focus_id,
                                        )
                                        .filter(|lb| {
                                            matches!(lb.replaced, Some(ReplacedKind::TextArea))
                                        })
                                    {
                                        let viewport =
                                            interaction.focused_input_rect.unwrap_or(lb.rect);
                                        let (pad_l, pad_r, _pad_t, _pad_b) =
                                            input_text_padding(lb.style);
                                        let available_text_w =
                                            (viewport.width - pad_l - pad_r).max(0.0);
                                        let preferred_x = interaction.textarea.preferred_x();
                                        let new_preferred_x = {
                                            let lines = interaction.textarea.ensure_layout_cache(
                                                &*input_values,
                                                focus_id,
                                                available_text_w,
                                                measurer,
                                                lb.style,
                                            );
                                            let ctx = TextareaVerticalMoveCtx {
                                                lines,
                                                measurer,
                                                style: lb.style,
                                            };
                                            textarea_move_caret_vertically(
                                                input_values,
                                                focus_id,
                                                -1,
                                                preferred_x,
                                                ctx,
                                                modifiers.shift,
                                            )
                                        };
                                        interaction.textarea.set_preferred_x(new_preferred_x);
                                        caret_or_selection_changed = true;
                                    }
                                }
                                Key::ArrowDown => {
                                    if is_textarea
                                        && let Some(lb) = find_layout_box_by_id(
                                            layout_root,
                                            focus_id,
                                        )
                                        .filter(|lb| {
                                            matches!(lb.replaced, Some(ReplacedKind::TextArea))
                                        })
                                    {
                                        let viewport =
                                            interaction.focused_input_rect.unwrap_or(lb.rect);
                                        let (pad_l, pad_r, _pad_t, _pad_b) =
                                            input_text_padding(lb.style);
                                        let available_text_w =
                                            (viewport.width - pad_l - pad_r).max(0.0);
                                        let preferred_x = interaction.textarea.preferred_x();
                                        let new_preferred_x = {
                                            let lines = interaction.textarea.ensure_layout_cache(
                                                &*input_values,
                                                focus_id,
                                                available_text_w,
                                                measurer,
                                                lb.style,
                                            );
                                            let ctx = TextareaVerticalMoveCtx {
                                                lines,
                                                measurer,
                                                style: lb.style,
                                            };
                                            textarea_move_caret_vertically(
                                                input_values,
                                                focus_id,
                                                1,
                                                preferred_x,
                                                ctx,
                                                modifiers.shift,
                                            )
                                        };
                                        interaction.textarea.set_preferred_x(new_preferred_x);
                                        caret_or_selection_changed = true;
                                    }
                                }
                                Key::Home => {
                                    if is_textarea {
                                        interaction.textarea.clear_preferred_x();
                                    }
                                    input_values.move_caret_to_start(focus_id, modifiers.shift);
                                    caret_or_selection_changed = true;
                                }
                                Key::End => {
                                    if is_textarea {
                                        interaction.textarea.clear_preferred_x();
                                    }
                                    input_values.move_caret_to_end(focus_id, modifiers.shift);
                                    caret_or_selection_changed = true;
                                }
                                Key::A if modifiers.command || modifiers.ctrl => {
                                    if is_textarea {
                                        interaction.textarea.clear_preferred_x();
                                    }
                                    input_values.select_all(focus_id);
                                    caret_or_selection_changed = true;
                                }
                                _ => {}
                            },
                            _ => {}
                        },

                        Some(HitKind::Checkbox) => {
                            if handled_activation {
                                continue;
                            }
                            match evt {
                                Event::Text(t) if t == " " => {
                                    handled_activation = true;
                                    non_text_state_changed |= input_values.toggle_checked(focus_id);
                                }
                                Event::Key {
                                    key: Key::Space,
                                    pressed: true,
                                    ..
                                } => {
                                    handled_activation = true;
                                    non_text_state_changed |= input_values.toggle_checked(focus_id);
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
                                    non_text_state_changed |=
                                        form_controls.on_radio_clicked(input_values, focus_id);
                                }
                                Event::Key {
                                    key: Key::Space,
                                    pressed: true,
                                    ..
                                } => {
                                    handled_activation = true;
                                    non_text_state_changed |=
                                        form_controls.on_radio_clicked(input_values, focus_id);
                                }
                                _ => {}
                            }
                        }

                        _ => {}
                    }
                }
            });

            // If egui reported an Enter keypress without a corresponding `Event::Text("\n")`,
            // treat it as a newline insertion for `<textarea>`.
            if is_textarea && enter_pressed && !saw_text_newline {
                interaction.textarea.clear_preferred_x();
                input_values.insert_text_multiline(focus_id, "\n");
                value_changed = true;
            }

            let changed = value_changed || caret_or_selection_changed || non_text_state_changed;
            let needs_text_scroll_sync = value_changed || caret_or_selection_changed;

            if changed {
                if needs_text_scroll_sync
                    && matches!(focused_kind, Some(HitKind::Input))
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
                                input_values,
                                focus_id,
                                viewport.width.max(1.0),
                                measurer,
                                lb.style,
                            );
                        }
                        Some(ReplacedKind::TextArea) => {
                            let (pad_l, pad_r, _pad_t, _pad_b) = input_text_padding(lb.style);
                            let available_text_w = (viewport.width - pad_l - pad_r).max(0.0);
                            let lines = interaction.textarea.ensure_layout_cache(
                                &*input_values,
                                focus_id,
                                available_text_w,
                                measurer,
                                lb.style,
                            );

                            sync_textarea_scroll_for_caret(
                                input_values,
                                focus_id,
                                viewport.height.max(1.0),
                                lines,
                                measurer,
                                lb.style,
                            );
                        }
                        _ => {}
                    }
                }
                request_repaint = true;
            }
        }
    }

    if request_repaint {
        ui.ctx().request_repaint();
    }
    action
}
