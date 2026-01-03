use crate::form_controls::FormControlIndex;
use crate::input_store::{InputValueStore, SelectionRange};
use crate::interactions::{
    ActiveTarget, InputDragState, InteractionState, TextareaCachedLine, TextareaCachedTextFragment,
    TextareaLayoutCache,
};
use crate::page::PageState;
use crate::resources::{ImageState, ResourceManager};
use crate::tab::Tab;

use html::{Id, Node, dom_utils::is_non_rendering_element};

use std::cell::RefCell;
use std::collections::HashMap;

use css::{ComputedStyle, Display, Length, StyledNode, build_style_tree};
use gfx::EguiTextMeasurer;
use layout::{
    BoxKind, LayoutBox, ListMarker, Rectangle, ReplacedElementInfoProvider, ReplacedKind,
    TextMeasurer, content_height, content_x_and_width, content_y,
    hit_test::{HitKind, HitResult, hit_test},
    inline::{
        InlineFragment, LineBox, button_label_from_layout, layout_inline_for_paint,
        layout_textarea_value_for_paint,
    },
    layout_block_tree,
};

use egui::{
    Align, Align2, Button, CentralPanel, Color32, Context, CursorIcon, Event, FontId, Frame, Key,
    Margin, Painter, Pos2, Rect, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, TopBottomPanel,
    Ui, Vec2,
};

pub enum NavigationAction {
    None,
    Back,
    Forward,
    Refresh,
    Navigate(String),
}

pub enum PageAction {
    Navigate(String),
}

pub fn top_bar(ctx: &Context, tab: &mut Tab) -> NavigationAction {
    let mut action = NavigationAction::None;

    const BAR_HEIGHT: f32 = 36.0;
    const BUTTON_WIDTH: f32 = 36.0;

    TopBottomPanel::top("topbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            let can_go_back = tab.history_index > 0;
            let can_go_forward = tab.history_index + 1 < tab.history.len();

            if ui
                .add_enabled(
                    can_go_back,
                    Button::new("⬅").min_size([BUTTON_WIDTH, BAR_HEIGHT].into()),
                )
                .clicked()
            {
                action = NavigationAction::Back;
            }
            if ui
                .add_enabled(
                    can_go_forward,
                    Button::new("➡").min_size([BUTTON_WIDTH, BAR_HEIGHT].into()),
                )
                .clicked()
            {
                action = NavigationAction::Forward;
            }
            if ui
                .add_sized([BUTTON_WIDTH, BAR_HEIGHT], Button::new("🔄"))
                .clicked()
            {
                action = NavigationAction::Refresh;
            }

            let response = Frame::new()
                .fill(ui.visuals().extreme_bg_color) // subtle background
                .stroke(Stroke::new(
                    1.0,
                    ui.visuals().widgets.inactive.bg_stroke.color,
                ))
                .corner_radius(6.0)
                .inner_margin(Margin::symmetric(4, 4))
                .show(ui, |ui| {
                    ui.add_sized(
                        [ui.available_width(), BAR_HEIGHT - 8.0],
                        TextEdit::singleline(&mut tab.url)
                            .hint_text("Enter URL")
                            .vertical_align(Align::Center),
                    )
                })
                .inner;

            if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                action = NavigationAction::Navigate(tab.url.clone());
            }
        });
    });
    action
}

pub fn content(
    ctx: &Context,
    page: &mut PageState,
    interaction: &mut InteractionState,
    resources: &ResourceManager,
    status: Option<&String>,
    loading: bool,
) -> Option<PageAction> {
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
        return None;
    }

    // IMPORTANT: borrow of page.dom is contained in this block and ends here.
    let base_fill = {
        let dom = page.dom.as_ref().unwrap();
        let style_root = build_style_tree(dom, None);
        let page_bg = find_page_background_color(&style_root);

        if let Some((r, g, b, a)) = page_bg {
            Color32::from_rgba_unmultiplied(r, g, b, a)
        } else {
            Color32::WHITE
        }
    };

    CentralPanel::default()
        .frame(Frame::default().fill(base_fill))
        .show(ctx, |ui| {
            // Rebuild style_root inside closure (needed anyway for layout/paint).
            let dom = page.dom.as_ref().unwrap();
            let style_root = build_style_tree(dom, None);

            // disjoint borrow: OK (dom is immutably borrowed, input_values mutably borrowed)
            let base_url = page.base_url.as_deref();
            let input_values = &mut page.input_values;
            let form_controls = &page.form_controls;

            let action = page_viewport(
                ui,
                &style_root,
                base_url,
                resources,
                input_values,
                form_controls,
                interaction,
            );

            if loading {
                ui.label("⏳ Loading…");
            }
            if let Some(s) = status {
                ui.label(s);
            }

            action
        })
        .inner
}

pub fn page_viewport(
    ui: &mut Ui,
    style_root: &StyledNode<'_>,
    base_url: Option<&str>,
    resources: &ResourceManager,
    input_values: &mut InputValueStore,
    form_controls: &FormControlIndex,
    interaction: &mut InteractionState,
) -> Option<PageAction> {
    ScrollArea::vertical()
        .id_salt("page_viewport_scroll_area")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let available_width = ui.available_width();
            let min_height = ui.available_height().max(200.0);

            let measurer = EguiTextMeasurer::new(ui.ctx());
            let replaced_info = BrowserReplacedInfo {
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

            // Paint first
            let focused = interaction.focused_node_id;
            let active = interaction.active;
            {
                let selection = ui.visuals().selection;
                let bg = selection.bg_fill;
                // Keep selection translucent so text stays readable without inverting text color (yet).
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

                let paint_ctx = PaintCtx {
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
                paint_layout_box(&layout_root, paint_ctx, true);
            }

            if let Some(focus_id) = interaction.focused_node_id
                && let Some(r) = fragment_rects.borrow().get(&focus_id).copied()
            {
                interaction.focused_input_rect = Some(r);
            }

            // ------- unified router output -------
            let mut action: Option<PageAction> = None;

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
                hit_test(&layout_root, (lx, ly), &measurer)
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
                    hit_test(&layout_root, (lx, ly), &measurer)
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
                        if let Some(lb) =
                            find_layout_box_by_id(&layout_root, h.node_id).filter(|lb| {
                                matches!(
                                    lb.replaced,
                                    Some(ReplacedKind::InputText | ReplacedKind::TextArea)
                                )
                            })
                        {
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
                                        &measurer,
                                        style,
                                    );
                                }
                                Some(ReplacedKind::TextArea) => {
                                    let (pad_l, pad_r, pad_t, _pad_b) = input_text_padding(style);

                                    let available_text_w =
                                        (h.fragment_rect.width - pad_l - pad_r).max(0.0);
                                    {
                                        let lines = ensure_textarea_layout_cache(
                                            interaction,
                                            &*input_values,
                                            h.node_id,
                                            available_text_w,
                                            &measurer,
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
                                            let line_idx = textarea_line_index_from_y(
                                                lines, y_in_text, line_h,
                                            );

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
                                            &measurer,
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

                    ui.ctx().request_repaint();
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

                    if let Some(lb) =
                        find_layout_box_by_id(&layout_root, drag_input_id).filter(|lb| {
                            matches!(
                                lb.replaced,
                                Some(ReplacedKind::InputText | ReplacedKind::TextArea)
                            )
                        })
                    {
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
                                    &measurer,
                                    style,
                                );

                                ui.ctx().request_repaint();
                            }
                            Some(ReplacedKind::TextArea) => {
                                interaction.textarea_preferred_x = None;
                                let (pad_l, pad_r, pad_t, _pad_b) = input_text_padding(style);

                                let available_text_w = (rect.width - pad_l - pad_r).max(0.0);
                                {
                                    let lines = ensure_textarea_layout_cache(
                                        interaction,
                                        &*input_values,
                                        drag_input_id,
                                        available_text_w,
                                        &measurer,
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
                                        let line_idx =
                                            textarea_line_index_from_y(lines, y_in_text, line_h);

                                        let x_in_viewport = (local_x - pad_l).max(0.0);
                                        textarea_caret_for_x_in_lines(
                                            lines,
                                            value,
                                            line_idx,
                                            x_in_viewport,
                                        )
                                    };

                                    input_values.set_caret(drag_input_id, caret, true);

                                    sync_textarea_scroll_for_caret(
                                        input_values,
                                        drag_input_id,
                                        rect.height.max(1.0),
                                        lines,
                                        &measurer,
                                        style,
                                    );
                                }

                                ui.ctx().request_repaint();
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
                                            if let Some(url) = resolve_relative_url(base_url, href)
                                            {
                                                action = Some(PageAction::Navigate(url));
                                            }
                                        } else {
                                            // debug: link hit but no href
                                            #[cfg(debug_assertions)]
                                            eprintln!(
                                                "Link hit {:?} but no href in HitResult",
                                                h.node_id
                                            );
                                        }
                                        // Clicking a link should clear input focus (browser-like)
                                        interaction.clear_focus();
                                    }

                                    HitKind::Checkbox => {
                                        let changed = input_values.toggle_checked(h.node_id);

                                        // Checkbox remains focused after activation (browser-like)
                                        interaction.set_focus(h.node_id, h.kind, h.fragment_rect);
                                        if changed {
                                            ui.ctx().request_repaint();
                                        }
                                    }

                                    HitKind::Radio => {
                                        let changed =
                                            form_controls.click_radio(input_values, h.node_id);

                                        // Radio remains focused after activation (browser-like)
                                        interaction.set_focus(h.node_id, h.kind, h.fragment_rect);
                                        if changed {
                                            ui.ctx().request_repaint();
                                        }
                                    }

                                    HitKind::Button => {
                                        #[cfg(debug_assertions)]
                                        eprintln!("button click: {:?}", h.node_id);

                                        // Clicking a button should blur input focus (browser-like)
                                        interaction.clear_focus();

                                        ui.ctx().request_repaint();
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
                } else if let Some(lb) =
                    find_layout_box_by_id(&layout_root, focus_id).filter(|lb| {
                        matches!(
                            lb.replaced,
                            Some(
                                ReplacedKind::InputText
                                    | ReplacedKind::TextArea
                                    | ReplacedKind::InputCheckbox
                                    | ReplacedKind::InputRadio
                            )
                        )
                    })
                {
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
                        find_layout_box_by_id(&layout_root, focus_id).and_then(|lb| lb.replaced);
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
                                            interaction.textarea_preferred_x = None;
                                            saw_text_newline |=
                                                t.contains('\n') || t.contains('\r');
                                            input_values
                                                .insert_text_multiline(focus_id, t.as_str());
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
                                                interaction.textarea_preferred_x = None;
                                            }
                                            input_values.backspace(focus_id);
                                            value_changed = true;
                                        }
                                        Key::Delete => {
                                            if is_textarea {
                                                interaction.textarea_preferred_x = None;
                                            }
                                            input_values.delete(focus_id);
                                            value_changed = true;
                                        }
                                        Key::ArrowLeft => {
                                            if is_textarea {
                                                interaction.textarea_preferred_x = None;
                                            }
                                            input_values.move_caret_left(focus_id, modifiers.shift);
                                            caret_or_selection_changed = true;
                                        }
                                        Key::ArrowRight => {
                                            if is_textarea {
                                                interaction.textarea_preferred_x = None;
                                            }
                                            input_values
                                                .move_caret_right(focus_id, modifiers.shift);
                                            caret_or_selection_changed = true;
                                        }
                                        Key::ArrowUp => {
                                            if is_textarea
                                                && let Some(lb) =
                                                    find_layout_box_by_id(&layout_root, focus_id)
                                                        .filter(|lb| {
                                                            matches!(
                                                                lb.replaced,
                                                                Some(ReplacedKind::TextArea)
                                                            )
                                                        })
                                            {
                                                let viewport = interaction
                                                    .focused_input_rect
                                                    .unwrap_or(lb.rect);
                                                let (pad_l, pad_r, _pad_t, _pad_b) =
                                                    input_text_padding(lb.style);
                                                let available_text_w =
                                                    (viewport.width - pad_l - pad_r).max(0.0);
                                                let preferred_x = interaction.textarea_preferred_x;
                                                let new_preferred_x = {
                                                    let lines = ensure_textarea_layout_cache(
                                                        interaction,
                                                        &*input_values,
                                                        focus_id,
                                                        available_text_w,
                                                        &measurer,
                                                        lb.style,
                                                    );
                                                    let ctx = TextareaVerticalMoveCtx {
                                                        lines,
                                                        measurer: &measurer,
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
                                                interaction.textarea_preferred_x = new_preferred_x;
                                                caret_or_selection_changed = true;
                                            }
                                        }
                                        Key::ArrowDown => {
                                            if is_textarea
                                                && let Some(lb) =
                                                    find_layout_box_by_id(&layout_root, focus_id)
                                                        .filter(|lb| {
                                                            matches!(
                                                                lb.replaced,
                                                                Some(ReplacedKind::TextArea)
                                                            )
                                                        })
                                            {
                                                let viewport = interaction
                                                    .focused_input_rect
                                                    .unwrap_or(lb.rect);
                                                let (pad_l, pad_r, _pad_t, _pad_b) =
                                                    input_text_padding(lb.style);
                                                let available_text_w =
                                                    (viewport.width - pad_l - pad_r).max(0.0);
                                                let preferred_x = interaction.textarea_preferred_x;
                                                let new_preferred_x = {
                                                    let lines = ensure_textarea_layout_cache(
                                                        interaction,
                                                        &*input_values,
                                                        focus_id,
                                                        available_text_w,
                                                        &measurer,
                                                        lb.style,
                                                    );
                                                    let ctx = TextareaVerticalMoveCtx {
                                                        lines,
                                                        measurer: &measurer,
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
                                                interaction.textarea_preferred_x = new_preferred_x;
                                                caret_or_selection_changed = true;
                                            }
                                        }
                                        Key::Home => {
                                            if is_textarea {
                                                interaction.textarea_preferred_x = None;
                                            }
                                            input_values
                                                .move_caret_to_start(focus_id, modifiers.shift);
                                            caret_or_selection_changed = true;
                                        }
                                        Key::End => {
                                            if is_textarea {
                                                interaction.textarea_preferred_x = None;
                                            }
                                            input_values
                                                .move_caret_to_end(focus_id, modifiers.shift);
                                            caret_or_selection_changed = true;
                                        }
                                        Key::A if modifiers.command || modifiers.ctrl => {
                                            if is_textarea {
                                                interaction.textarea_preferred_x = None;
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
                                            non_text_state_changed |=
                                                input_values.toggle_checked(focus_id);
                                        }
                                        Event::Key {
                                            key: Key::Space,
                                            pressed: true,
                                            ..
                                        } => {
                                            handled_activation = true;
                                            non_text_state_changed |=
                                                input_values.toggle_checked(focus_id);
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
                                                form_controls.click_radio(input_values, focus_id);
                                        }
                                        Event::Key {
                                            key: Key::Space,
                                            pressed: true,
                                            ..
                                        } => {
                                            handled_activation = true;
                                            non_text_state_changed |=
                                                form_controls.click_radio(input_values, focus_id);
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
                        interaction.textarea_preferred_x = None;
                        input_values.insert_text_multiline(focus_id, "\n");
                        value_changed = true;
                    }

                    let changed =
                        value_changed || caret_or_selection_changed || non_text_state_changed;
                    let needs_text_scroll_sync = value_changed || caret_or_selection_changed;

                    if changed {
                        if needs_text_scroll_sync
                            && matches!(focused_kind, Some(HitKind::Input))
                            && let Some(lb) =
                                find_layout_box_by_id(&layout_root, focus_id).filter(|lb| {
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
                                    let (pad_l, pad_r, _pad_t, _pad_b) =
                                        input_text_padding(lb.style);
                                    let available_text_w =
                                        (viewport.width - pad_l - pad_r).max(0.0);
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
                        ui.ctx().request_repaint();
                    }
                }
            }

            // Optional debug overlay
            if let Some(h) = hover_hit {
                let msg = format!("hover: {:?} on {:?}", h.kind, h.node_id);
                painter.text(
                    origin + Vec2::new(8.0, 8.0),
                    Align2::LEFT_TOP,
                    msg,
                    FontId::proportional(12.0),
                    Color32::from_rgb(80, 80, 80),
                );
            }

            action
        })
        .inner
}

#[derive(Clone, Copy)]
struct PaintCtx<'a> {
    painter: &'a Painter,
    origin: Pos2,
    measurer: &'a EguiTextMeasurer,
    base_url: Option<&'a str>,
    resources: &'a ResourceManager,
    input_values: &'a InputValueStore,
    focused: Option<Id>,
    focused_textarea_lines: Option<&'a [TextareaCachedLine]>,
    active: Option<ActiveTarget>,
    selection_bg_fill: Color32,
    selection_stroke: Stroke,
    fragment_rects: Option<&'a RefCell<HashMap<Id, Rectangle>>>,
}

impl<'a> PaintCtx<'a> {
    fn with_origin(self, origin: Pos2) -> Self {
        Self { origin, ..self }
    }
}

fn paint_line_boxes<'a>(lines: &[LineBox<'a>], ctx: PaintCtx<'_>) {
    let painter = ctx.painter;
    let origin = ctx.origin;
    let measurer = ctx.measurer;
    let base_url = ctx.base_url;
    let resources = ctx.resources;
    let input_values = ctx.input_values;
    let focused = ctx.focused;
    let focused_textarea_lines = ctx.focused_textarea_lines;
    let active = ctx.active;
    let selection_bg_fill = ctx.selection_bg_fill;
    let selection_stroke = ctx.selection_stroke;
    let fragment_rects = ctx.fragment_rects;

    for line in lines {
        for frag in &line.fragments {
            match &frag.kind {
                InlineFragment::Text { text, style, .. } => {
                    let (cr, cg, cb, ca) = style.color;
                    let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);

                    let Length::Px(font_px) = style.font_size;
                    let font_id = FontId::proportional(font_px);

                    let pos = Pos2 {
                        x: origin.x + frag.rect.x,
                        y: origin.y + frag.rect.y,
                    };

                    painter.text(pos, Align2::LEFT_TOP, text, font_id, text_color);
                }

                InlineFragment::Box { style, layout, .. } => {
                    if let Some(cache) = fragment_rects
                        && let Some(lb) = layout
                        && lb.replaced.is_some()
                    {
                        cache.borrow_mut().insert(lb.node_id(), frag.rect);
                    }

                    let rect = Rect::from_min_size(
                        Pos2 {
                            x: origin.x + frag.rect.x,
                            y: origin.y + frag.rect.y,
                        },
                        Vec2::new(frag.rect.width, frag.rect.height),
                    );

                    if let Some(child_box) = layout {
                        // Paint the inline-block's full content at this inline position.
                        // Compute an origin such that child's rect's top-left lands at `rect.min`.
                        let translated_origin = Pos2 {
                            x: rect.min.x - child_box.rect.x,
                            y: rect.min.y - child_box.rect.y,
                        };

                        // Paint the entire subtree of this inline-block here,
                        // including its background/border and its children.
                        paint_layout_box(
                            child_box,
                            ctx.with_origin(translated_origin),
                            false, // do NOT skip inline-block children inside this subtree
                        );
                    } else {
                        // Fallback: simple placeholder rectangle using the box style.
                        let (r, g, b, a) = style.background_color;
                        let color = if a > 0 {
                            Color32::from_rgba_unmultiplied(r, g, b, a)
                        } else {
                            Color32::from_rgba_unmultiplied(180, 180, 180, 255)
                        };

                        painter.rect_filled(rect, 0.0, color);
                    }
                }

                InlineFragment::Replaced {
                    style,
                    kind,
                    layout,
                    ..
                } => {
                    let rect = Rect::from_min_size(
                        Pos2 {
                            x: origin.x + frag.rect.x,
                            y: origin.y + frag.rect.y,
                        },
                        Vec2::new(frag.rect.width, frag.rect.height),
                    );

                    if let Some(cache) = fragment_rects
                        && let Some(lb) = layout
                    {
                        cache.borrow_mut().insert(lb.node_id(), frag.rect);
                    }

                    // --- BUTTON: pressed visual state (uses `active`) ---
                    if matches!(kind, ReplacedKind::Button) {
                        let id = layout.map(|lb| lb.node_id());
                        let is_pressed = id.is_some_and(|id| {
                            active.is_some_and(|a| a.id == id && matches!(a.kind, HitKind::Button))
                        });

                        let fill = if is_pressed {
                            Color32::from_rgb(200, 200, 200)
                        } else {
                            Color32::from_rgb(230, 230, 230)
                        };

                        painter.rect_filled(rect, 6.0, fill);

                        let stroke = if is_pressed {
                            Stroke::new(2.0, Color32::from_rgb(110, 110, 110))
                        } else {
                            Stroke::new(1.0, Color32::from_rgb(140, 140, 140))
                        };
                        painter.rect_stroke(rect, 6.0, stroke, StrokeKind::Outside);

                        let mut label = "Button".to_string();
                        if let Some(lb) = layout {
                            label = button_label_from_layout(lb);
                        }

                        let offset = if is_pressed {
                            Vec2::new(1.0, 1.0)
                        } else {
                            Vec2::ZERO
                        };

                        painter.text(
                            rect.center() + offset,
                            Align2::CENTER_CENTER,
                            label,
                            FontId::proportional(12.0),
                            Color32::from_rgb(60, 60, 60),
                        );

                        continue; // IMPORTANT: don't fall through to generic replaced painting
                    }

                    // --- INPUT CHECKBOX / RADIO ---
                    if matches!(kind, ReplacedKind::InputCheckbox | ReplacedKind::InputRadio) {
                        let id = layout.map(|lb| lb.node_id());
                        let is_checked = id.is_some_and(|id| input_values.is_checked(id));
                        let is_focused = id.is_some_and(|id| focused == Some(id));

                        let is_pressed = id.is_some_and(|id| {
                            active.is_some_and(|a| {
                                a.id == id && matches!(a.kind, HitKind::Checkbox | HitKind::Radio)
                            })
                        });

                        let side = rect.width().min(rect.height()).max(0.0);
                        if side > 0.0 {
                            let control_rect =
                                Rect::from_center_size(rect.center(), Vec2::splat(side));

                            let (br, bg, bb, ba) = style.background_color;
                            let base_fill = if ba > 0 {
                                Color32::from_rgba_unmultiplied(br, bg, bb, ba)
                            } else {
                                Color32::WHITE
                            };
                            let fill = if is_pressed {
                                base_fill.gamma_multiply(0.9)
                            } else {
                                base_fill
                            };

                            let border = if is_focused {
                                selection_stroke
                            } else {
                                Stroke::new(1.0, Color32::from_rgb(120, 120, 120))
                            };
                            let corner = (side * 0.2).min(4.0);

                            match kind {
                                ReplacedKind::InputCheckbox => {
                                    painter.rect_filled(control_rect, corner, fill);
                                    painter.rect_stroke(
                                        control_rect,
                                        corner,
                                        border,
                                        StrokeKind::Outside,
                                    );

                                    if is_checked {
                                        let (cr, cg, cb, ca) = style.color;
                                        let check_color =
                                            Color32::from_rgba_unmultiplied(cr, cg, cb, ca);
                                        let thickness = (side * 0.12).max(1.5);

                                        let a = Pos2 {
                                            x: control_rect.min.x + side * 0.25,
                                            y: control_rect.min.y + side * 0.55,
                                        };
                                        let b = Pos2 {
                                            x: control_rect.min.x + side * 0.45,
                                            y: control_rect.min.y + side * 0.75,
                                        };
                                        let c = Pos2 {
                                            x: control_rect.min.x + side * 0.80,
                                            y: control_rect.min.y + side * 0.30,
                                        };

                                        let stroke = Stroke::new(thickness, check_color);
                                        painter.line_segment([a, b], stroke);
                                        painter.line_segment([b, c], stroke);
                                    }
                                }

                                ReplacedKind::InputRadio => {
                                    let center = control_rect.center();
                                    let r = side * 0.5;
                                    painter.circle_filled(center, r, fill);
                                    painter.circle_stroke(center, r, border);

                                    if is_checked {
                                        let (cr, cg, cb, ca) = style.color;
                                        let dot = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);
                                        painter.circle_filled(center, r * 0.45, dot);
                                    }
                                }

                                _ => unreachable!("handled by match guard"),
                            }
                        }

                        continue; // don't fall through to generic replaced painting
                    }

                    // --- IMG: decoded texture (if ready) or accessibility-ish fallback ---
                    if matches!(kind, ReplacedKind::Img) {
                        let alt = layout
                            .and_then(|lb| get_attr(lb.node.node, "alt"))
                            .map(str::trim)
                            .filter(|s| !s.is_empty());

                        let img_url = layout
                            .and_then(|lb| get_attr(lb.node.node, "src"))
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .and_then(|src| resolve_relative_url(base_url, src));

                        let state = img_url
                            .as_deref()
                            .map(|url| resources.image_state_by_url(url))
                            .unwrap_or(ImageState::Missing);

                        match state {
                            ImageState::Ready(ready) => {
                                let uv = Rect::from_min_max(
                                    Pos2 { x: 0.0, y: 0.0 },
                                    Pos2 { x: 1.0, y: 1.0 },
                                );
                                painter.image(ready.texture_id, rect, uv, Color32::WHITE);
                            }
                            ImageState::Loading { .. } | ImageState::Decoding { .. } => {
                                paint_img_fallback_placeholder(
                                    painter,
                                    rect,
                                    style,
                                    measurer,
                                    ImgFallbackState::Loading,
                                    alt,
                                );
                            }
                            ImageState::Error { .. } => {
                                paint_img_fallback_placeholder(
                                    painter,
                                    rect,
                                    style,
                                    measurer,
                                    ImgFallbackState::Error,
                                    alt,
                                );
                            }
                            ImageState::Missing => {
                                paint_img_fallback_placeholder(
                                    painter,
                                    rect,
                                    style,
                                    measurer,
                                    ImgFallbackState::Missing,
                                    alt,
                                );
                            }
                        }

                        continue;
                    }

                    let is_focused_text_control =
                        matches!(kind, ReplacedKind::InputText | ReplacedKind::TextArea)
                            && layout.is_some_and(|lb| focused == Some(lb.node_id()));

                    // Fill + stroke (placeholder look)
                    let (r, g, b, a) = style.background_color;
                    let fill = if a > 0 {
                        Color32::from_rgba_unmultiplied(r, g, b, a)
                    } else {
                        Color32::from_rgba_unmultiplied(220, 220, 220, 255)
                    };

                    painter.rect_filled(rect, 2.0, fill);
                    let stroke = if is_focused_text_control {
                        selection_stroke
                    } else {
                        Stroke::new(1.0, Color32::from_rgb(120, 120, 120))
                    };

                    painter.rect_stroke(rect, 2.0, stroke, StrokeKind::Outside);

                    // Special case: <input type="text"> draws its value/placeholder inside the box
                    if matches!(kind, ReplacedKind::InputText) {
                        // Determine shown text: value first, else placeholder
                        let mut value = String::new();
                        let mut placeholder: Option<String> = None;
                        let mut caret: usize = 0;
                        let mut selection: Option<SelectionRange> = None;
                        let mut scroll_x: f32 = 0.0;

                        if let Some(lb) = layout {
                            let id = lb.node_id();
                            if let Some((v, c, sel, sx, _sy)) = input_values.get_state(id) {
                                value = v.to_string();
                                caret = c;
                                selection = sel;
                                scroll_x = sx;
                            }

                            placeholder = if value.is_empty() {
                                get_attr(lb.node.node, "placeholder")
                                    .map(str::trim)
                                    .filter(|ph| !ph.is_empty())
                                    .map(|ph| ph.to_string())
                            } else {
                                None
                            };
                        }

                        // Inner text area from padding (with sane minimums)
                        let (pad_l, pad_r, pad_t, pad_b) = input_text_padding(style);

                        let available_text_w = (rect.width() - pad_l - pad_r).max(0.0);

                        let line_h = measurer.line_height(style);
                        let inner_h = (rect.height() - pad_t - pad_b).max(0.0);
                        let caret_h = line_h.min(inner_h).max(1.0);
                        let extra_y = (inner_h - caret_h).max(0.0) * 0.5;
                        let text_y = rect.min.y + pad_t + extra_y;

                        // Paint in style color (placeholder uses a lighter tint).
                        let (cr, cg, cb, ca) = style.color;
                        let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);
                        let value_color = text_color;
                        let placeholder_color = text_color.gamma_multiply(0.6);
                        let Length::Px(font_px) = style.font_size;
                        let font_id = FontId::proportional(font_px);

                        let is_placeholder = value.is_empty();
                        let paint_color = if is_placeholder {
                            placeholder_color
                        } else {
                            value_color
                        };

                        let inner_min_x = rect.min.x + pad_l;
                        let inner_max_x = (rect.max.x - pad_r).max(inner_min_x);
                        let inner_min_y = rect.min.y + pad_t;
                        let inner_max_y = (rect.max.y - pad_b).max(inner_min_y);
                        let inner_rect = Rect::from_min_max(
                            Pos2 {
                                x: inner_min_x,
                                y: inner_min_y,
                            },
                            Pos2 {
                                x: inner_max_x,
                                y: inner_max_y,
                            },
                        );

                        if is_focused_text_control {
                            // Focused input: render the full value, clipped to the inner rect,
                            // with a caret and optional selection highlight.
                            let clip_painter = painter.with_clip_rect(inner_rect);

                            let caret = clamp_caret_to_boundary(&value, caret);

                            // Scroll horizontally to keep the caret visible.
                            let text_w = if is_placeholder {
                                0.0
                            } else {
                                measurer.measure(&value, style)
                            };
                            let caret_w = if is_placeholder {
                                0.0
                            } else {
                                measurer.measure(&value[..caret], style)
                            };

                            // `scroll_x` is persistent state in the store; clamp it to current bounds.
                            let scroll_max = if !is_placeholder && available_text_w > 0.0 {
                                (text_w - available_text_w).max(0.0)
                            } else {
                                0.0
                            };
                            scroll_x = scroll_x.clamp(0.0, scroll_max);

                            let text_x = inner_rect.min.x - scroll_x;

                            // Selection highlight (single-line).
                            if let (false, Some(sel)) =
                                (is_placeholder, selection.filter(|s| s.start < s.end))
                            {
                                let sel_start = sel.start.min(value.len());
                                let sel_end = sel.end.min(value.len());

                                if value.is_char_boundary(sel_start)
                                    && value.is_char_boundary(sel_end)
                                {
                                    let x0 = measurer.measure(&value[..sel_start], style);
                                    let x1 = measurer.measure(&value[..sel_end], style);
                                    let sel_rect = Rect::from_min_max(
                                        Pos2 {
                                            x: text_x + x0,
                                            y: text_y,
                                        },
                                        Pos2 {
                                            x: text_x + x1,
                                            y: text_y + caret_h,
                                        },
                                    );

                                    clip_painter.rect_filled(sel_rect, 0.0, selection_bg_fill);
                                }
                            }

                            // Text
                            let paint_text = if is_placeholder {
                                placeholder.as_deref().unwrap_or_default()
                            } else {
                                value.as_str()
                            };
                            clip_painter.text(
                                Pos2 {
                                    x: text_x,
                                    y: text_y,
                                },
                                Align2::LEFT_TOP,
                                paint_text,
                                font_id,
                                paint_color,
                            );

                            // Caret: 1px vertical line.
                            let caret_x = if is_placeholder {
                                inner_rect.min.x
                            } else {
                                inner_rect.min.x + caret_w - scroll_x
                            };
                            let caret_max_x =
                                (inner_rect.min.x + available_text_w - 1.0).max(inner_rect.min.x);
                            let caret_x = caret_x.clamp(inner_rect.min.x, caret_max_x).round();
                            let caret_rect = Rect::from_min_size(
                                Pos2 {
                                    x: caret_x,
                                    y: text_y,
                                },
                                Vec2 { x: 1.0, y: caret_h },
                            );
                            // Caret uses the actual text color, not placeholder styling.
                            clip_painter.rect_filled(caret_rect, 0.0, value_color);
                        } else {
                            // Unfocused input: show a simple truncated preview (no caret/selection).
                            let painted = if !is_placeholder {
                                truncate_to_fit(measurer, style, &value, available_text_w)
                            } else {
                                let ph = placeholder.as_deref().unwrap_or_default();
                                truncate_to_fit(measurer, style, ph, available_text_w)
                            };

                            painter.text(
                                Pos2 {
                                    x: inner_rect.min.x,
                                    y: text_y,
                                },
                                Align2::LEFT_TOP,
                                &painted,
                                font_id,
                                paint_color,
                            );
                        }

                        continue; // skip default label painting below
                    }

                    // Special case: <textarea> draws its multi-line value with wrapping.
                    if matches!(kind, ReplacedKind::TextArea) {
                        let mut value: &str = "";
                        let mut placeholder: Option<&str> = None;
                        let mut caret: usize = 0;
                        let mut selection: Option<SelectionRange> = None;
                        let mut scroll_y: f32 = 0.0;

                        if let Some(lb) = layout {
                            let id = lb.node_id();
                            if let Some((v, c, sel, _sx, sy)) = input_values.get_state(id) {
                                value = v;
                                caret = c;
                                selection = sel;
                                scroll_y = sy;
                            }

                            placeholder = if value.is_empty() {
                                get_attr(lb.node.node, "placeholder")
                                    .map(str::trim)
                                    .filter(|ph| !ph.is_empty())
                            } else {
                                None
                            };
                        }

                        // Inner text area from padding (with sane minimums)
                        let (pad_l, pad_r, pad_t, pad_b) = input_text_padding(style);

                        let inner_min_x = rect.min.x + pad_l;
                        let inner_max_x = (rect.max.x - pad_r).max(inner_min_x);
                        let inner_min_y = rect.min.y + pad_t;
                        let inner_max_y = (rect.max.y - pad_b).max(inner_min_y);
                        let inner_rect = Rect::from_min_max(
                            Pos2 {
                                x: inner_min_x,
                                y: inner_min_y,
                            },
                            Pos2 {
                                x: inner_max_x,
                                y: inner_max_y,
                            },
                        );

                        let available_text_w = inner_rect.width().max(0.0);
                        let available_text_h = inner_rect.height().max(0.0);

                        // Paint in style color (placeholder uses a lighter tint).
                        let (cr, cg, cb, ca) = style.color;
                        let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);
                        let value_color = text_color;
                        let placeholder_color = text_color.gamma_multiply(0.6);
                        let Length::Px(font_px) = style.font_size;
                        let font_id = FontId::proportional(font_px);

                        let is_placeholder = value.is_empty();
                        let paint_color = if is_placeholder {
                            placeholder_color
                        } else {
                            value_color
                        };

                        let paint_text = if is_placeholder {
                            placeholder.unwrap_or_default()
                        } else {
                            value
                        };

                        let mut owned_lines: Option<Vec<TextareaCachedLine>> = None;
                        let lines: &[TextareaCachedLine] = if is_placeholder {
                            owned_lines
                                .get_or_insert_with(|| {
                                    layout_textarea_cached_lines(
                                        measurer,
                                        style,
                                        available_text_w,
                                        paint_text,
                                        false,
                                    )
                                })
                                .as_slice()
                        } else if is_focused_text_control {
                            if let Some(cached) = focused_textarea_lines {
                                cached
                            } else {
                                owned_lines
                                    .get_or_insert_with(|| {
                                        layout_textarea_cached_lines(
                                            measurer,
                                            style,
                                            available_text_w,
                                            paint_text,
                                            false,
                                        )
                                    })
                                    .as_slice()
                            }
                        } else {
                            owned_lines
                                .get_or_insert_with(|| {
                                    layout_textarea_cached_lines(
                                        measurer,
                                        style,
                                        available_text_w,
                                        paint_text,
                                        false,
                                    )
                                })
                                .as_slice()
                        };

                        // Clamp scroll to the current text bounds.
                        let text_h = textarea_text_height(lines, measurer.line_height(style));
                        let scroll_max = if available_text_h > 0.0 {
                            (text_h - available_text_h).max(0.0)
                        } else {
                            0.0
                        };
                        scroll_y = scroll_y.clamp(0.0, scroll_max);

                        let clip_painter = painter.with_clip_rect(inner_rect);

                        // Multi-line selection highlight.
                        if is_focused_text_control
                            && let (false, Some(sel)) =
                                (is_placeholder, selection.filter(|s| s.start < s.end))
                        {
                            paint_textarea_selection(
                                &clip_painter,
                                lines,
                                value,
                                sel,
                                TextAreaSelectionPaintParams {
                                    inner_origin: inner_rect.min,
                                    scroll_y,
                                    measurer,
                                    style,
                                    selection_bg_fill,
                                },
                            );
                        }

                        // Text fragments
                        for line in lines {
                            for tfrag in &line.fragments {
                                let Some((start, end)) = tfrag.source_range else {
                                    continue;
                                };
                                if start > end || end > paint_text.len() {
                                    continue;
                                }
                                if !(paint_text.is_char_boundary(start)
                                    && paint_text.is_char_boundary(end))
                                {
                                    continue;
                                }

                                let mut s = &paint_text[start..end];
                                if s == " " || s == "\t" {
                                    s = "\u{00A0}";
                                }

                                clip_painter.text(
                                    Pos2 {
                                        x: inner_rect.min.x + tfrag.rect.x,
                                        y: inner_rect.min.y + tfrag.rect.y - scroll_y,
                                    },
                                    Align2::LEFT_TOP,
                                    s,
                                    font_id.clone(),
                                    paint_color,
                                );
                            }
                        }

                        // Caret: 1px vertical line.
                        if is_focused_text_control {
                            if is_placeholder {
                                let caret_h =
                                    measurer.line_height(style).min(available_text_h).max(1.0);
                                let caret_rect = Rect::from_min_size(
                                    Pos2 {
                                        x: inner_rect.min.x.round(),
                                        y: inner_rect.min.y.round(),
                                    },
                                    Vec2 { x: 1.0, y: caret_h },
                                );
                                clip_painter.rect_filled(caret_rect, 0.0, value_color);
                            } else {
                                let caret = clamp_caret_to_boundary(value, caret);
                                let (cx, cy, ch) =
                                    textarea_caret_geometry(lines, value, caret, measurer, style);
                                let caret_h = ch.min(available_text_h).max(1.0);
                                let caret_rect = Rect::from_min_size(
                                    Pos2 {
                                        x: (inner_rect.min.x + cx).round(),
                                        y: (inner_rect.min.y + cy - scroll_y).round(),
                                    },
                                    Vec2 { x: 1.0, y: caret_h },
                                );
                                clip_painter.rect_filled(caret_rect, 0.0, value_color);
                            }
                        }

                        continue; // skip default label painting below
                    }

                    // Default centered label for other replaced elements
                    let mut label = match kind {
                        ReplacedKind::Img => "IMG".to_string(),
                        ReplacedKind::Button => "BUTTON".to_string(),
                        ReplacedKind::InputText => unreachable!("handled above"),
                        ReplacedKind::TextArea => unreachable!("handled above"),
                        ReplacedKind::InputCheckbox => "CHECKBOX".to_string(),
                        ReplacedKind::InputRadio => "RADIO".to_string(),
                    };

                    // If <img alt="...">, show alt text instead
                    if let (ReplacedKind::Img, Some(alt)) =
                        (kind, layout.and_then(|lb| get_attr(lb.node.node, "alt")))
                    {
                        let alt = alt.trim();
                        if !alt.is_empty() {
                            label = alt.to_string();
                        }
                    }

                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::proportional(12.0),
                        Color32::from_rgb(60, 60, 60),
                    );
                }
            }
        }
    }
}

fn paint_layout_box<'a>(
    layout: &LayoutBox<'a>,
    ctx: PaintCtx<'_>,
    skip_inline_block_children: bool,
) {
    let painter = ctx.painter;
    let origin = ctx.origin;
    let measurer = ctx.measurer;

    // 0) Do not paint non-rendering elements (head, style, script, etc.)
    if is_non_rendering_element(layout.node.node) {
        for child in &layout.children {
            paint_layout_box(child, ctx, skip_inline_block_children);
        }
        return;
    }

    let rect = Rect::from_min_size(
        Pos2 {
            x: origin.x + layout.rect.x,
            y: origin.y + layout.rect.y,
        },
        Vec2 {
            x: layout.rect.width,
            y: layout.rect.height,
        },
    );

    // background
    let (r, g, b, a) = layout.style.background_color;
    if a > 0 {
        painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(r, g, b, a));
    }

    // 1) List marker (for display:list-item), if any.
    //    This does not affect layout; it's purely visual.
    if matches!(layout.style.display, Display::ListItem) {
        paint_list_marker(layout, painter, origin, measurer);
    }

    // 2) Inline content
    paint_inline_content(layout, ctx);

    // 3) Recurse into children
    for child in &layout.children {
        // ✅ Inline engine already painted inline-blocks AND replaced elements via fragments.
        if skip_inline_block_children
            && (matches!(child.kind, BoxKind::InlineBlock) || child.replaced.is_some())
        {
            continue;
        }

        paint_layout_box(child, ctx, skip_inline_block_children);
    }
}

fn paint_list_marker<'a>(
    layout: &LayoutBox<'a>,
    painter: &Painter,
    origin: Pos2,
    measurer: &dyn TextMeasurer,
) {
    let marker = match layout.list_marker {
        Some(m) => m,
        None => return, // nothing to paint
    };

    // Choose marker text: bullet or number.
    let marker_text = match marker {
        ListMarker::Unordered => "•".to_string(),
        ListMarker::Ordered(index) => format!("{index}."),
    };

    // Use the list item's text style for the marker.
    let style = layout.style;
    let (cr, cg, cb, ca) = style.color;
    let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);

    let Length::Px(font_px) = style.font_size;
    let font_id = FontId::proportional(font_px);

    // Position: slightly to the left of the content box (padding-left),
    // aligned with the top of the content. This doesn't change layout height.
    let bm = layout.style.box_metrics;

    // Content box x/y in layout coordinates (same as inline content start).
    let content_x = layout.rect.x + bm.padding_left;
    let content_y = layout.rect.y + bm.padding_top;

    // Measure marker width so we can place it just to the left of the content.
    let marker_width = measurer.measure(&marker_text, style);

    // How much gap between marker and content.
    let gap = 4.0;

    let marker_pos = Pos2 {
        x: origin.x + content_x - marker_width - gap,
        y: origin.y + content_y,
    };

    painter.text(
        marker_pos,
        Align2::LEFT_TOP,
        marker_text,
        font_id,
        text_color,
    );
}

// Paint a sequence of LineBox/LineFragment produced by the inline engine.
// Text fragments are painted directly; Box fragments (inline-blocks) are
// painted by translating the associated LayoutBox subtree into the fragment
// rect position.
fn paint_inline_content<'a>(layout: &LayoutBox<'a>, ctx: PaintCtx<'_>) {
    // ✅ Replaced elements (<textarea>, <input>, <img>, <button>) do NOT paint their DOM children.
    // They are painted by InlineFragment::Replaced in paint_line_boxes.
    if layout.replaced.is_some() {
        return;
    }

    let measurer = ctx.measurer;

    // Only block-like elements host their own inline formatting context.
    match layout.node.node {
        Node::Element { .. } => {
            // Inline elements do NOT establish their own block-level
            // inline formatting context; their text is handled by the
            // nearest block ancestor.
            if matches!(layout.style.display, Display::Inline) {
                return;
            }
        }
        // The Document node itself also does not host inline content;
        // its block children (html/body/etc.) will do that.
        Node::Document { .. } => return,
        _ => return,
    }

    // Compute the content box consistently with the layout engine.
    let (content_x, content_width) =
        content_x_and_width(layout.style, layout.rect.x, layout.rect.width);
    let content_y = content_y(layout.style, layout.rect.y);
    let content_height = content_height(layout.style, layout.rect.height);

    let block_rect = Rectangle {
        x: content_x,
        y: content_y,
        width: content_width,
        height: content_height,
    };

    // Use the painting-aware inline layout: text + inline-block boxes,
    // enumerated from the layout tree in DOM order. LineBox/LineFragment are
    // the source of truth for inline geometry here.
    let lines = layout_inline_for_paint(measurer, block_rect, layout);

    if lines.is_empty() {
        return;
    }

    paint_line_boxes(&lines, ctx);
}

fn find_page_background_color(root: &StyledNode<'_>) -> Option<(u8, u8, u8, u8)> {
    // We prefer <body> background if present and non-transparent.
    // If not, we fall back to <html>. Otherwise: None.
    fn is_non_transparent_rgba(rgba: (u8, u8, u8, u8)) -> bool {
        let (_r, _g, _b, a) = rgba;
        a > 0
    }

    fn from_elem(node: &StyledNode<'_>, want: &str) -> Option<(u8, u8, u8, u8)> {
        match node.node {
            Node::Element { name, .. } if name.eq_ignore_ascii_case(want) => {
                let rgba = node.style.background_color;
                if is_non_transparent_rgba(rgba) {
                    Some(rgba)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    // root.node is the Document. We look for <html> first-level children,
    // then <body> beneath those. This matches the usual structure.
    // Prefer <body>, fallback to <html>.
    let mut html_bg = None;
    let mut body_bg = None;

    for child in &root.children {
        if html_bg.is_none() {
            html_bg = from_elem(child, "html");
        }

        for gc in &child.children {
            if body_bg.is_none() {
                body_bg = from_elem(gc, "body");
            }
        }
    }

    body_bg.or(html_bg)
}

fn truncate_to_fit(
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
    text: &str,
    max_w: f32,
) -> String {
    if text.is_empty() || max_w <= 0.0 {
        return String::new();
    }
    if measurer.measure(text, style) <= max_w {
        return text.to_string();
    }

    // Simple ellipsis truncation.
    let ell = "…";
    let ell_w = measurer.measure(ell, style);
    if ell_w > max_w {
        return String::new();
    }

    // Binary search cut point.
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();

    while lo < hi {
        let mid = (lo + hi) / 2;
        let candidate: String = chars[..mid].iter().collect();
        let w = measurer.measure(&(candidate.clone() + ell), style);
        if w <= max_w {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }

    let cut = lo.saturating_sub(1);
    let mut s: String = chars[..cut].iter().collect();
    s.push_str(ell);
    s
}

fn clamp_caret_to_boundary(value: &str, caret: usize) -> usize {
    let mut caret = caret.min(value.len());
    while caret > 0 && !value.is_char_boundary(caret) {
        caret -= 1;
    }
    caret
}

fn input_text_padding(style: &ComputedStyle) -> (f32, f32, f32, f32) {
    let bm = style.box_metrics;
    let pad_l = bm.padding_left.max(4.0);
    let pad_r = bm.padding_right.max(4.0);
    let pad_t = bm.padding_top.max(2.0);
    let pad_b = bm.padding_bottom.max(2.0);
    (pad_l, pad_r, pad_t, pad_b)
}

fn build_textarea_fragment_hit_map(
    measurer: &EguiTextMeasurer,
    style: &ComputedStyle,
    value: &str,
    source_range: Option<(usize, usize)>,
    frag_width: f32,
) -> (Vec<usize>, Vec<f32>) {
    let Some((start, end)) = source_range else {
        return (Vec::new(), Vec::new());
    };
    if start > end || end > value.len() {
        return (Vec::new(), Vec::new());
    }
    if !(value.is_char_boundary(start) && value.is_char_boundary(end)) {
        return (Vec::new(), Vec::new());
    }

    let slice = &value[start..end];
    if slice.is_empty() {
        return (vec![start], vec![0.0]);
    }

    // Very common for textarea text to include whitespace fragments. These don't benefit from
    // per-char hit maps, and egui can be picky about measuring them; treat them as a single box.
    if slice == " " || slice == "\t" {
        let w = frag_width.max(0.0);
        return (vec![start, end], vec![0.0, w]);
    }

    let text_for_layout = slice.to_owned();

    let (r, g, b, a) = style.color;
    let color = Color32::from_rgba_unmultiplied(r, g, b, a);
    let Length::Px(font_px) = style.font_size;
    let font_id = FontId::proportional(font_px);

    let galley = measurer
        .context()
        .fonts(|f| f.layout_no_wrap(text_for_layout, font_id, color));

    if galley.rows.len() != 1 {
        return (Vec::new(), Vec::new());
    }

    let row = &galley.rows[0];
    let char_count = row.char_count_excluding_newline();

    let mut byte_positions = Vec::with_capacity(char_count + 1);
    byte_positions.push(start);
    for (byte_off, ch) in slice.char_indices() {
        byte_positions.push(start + byte_off + ch.len_utf8());
    }

    let mut x_advances = Vec::with_capacity(char_count + 1);
    for i in 0..=char_count {
        x_advances.push(row.x_offset(i).max(0.0));
    }

    if byte_positions.len() != x_advances.len() {
        return (Vec::new(), Vec::new());
    }

    (byte_positions, x_advances)
}

fn layout_textarea_cached_lines(
    measurer: &EguiTextMeasurer,
    style: &ComputedStyle,
    available_text_w: f32,
    text: &str,
    build_hit_maps: bool,
) -> Vec<TextareaCachedLine> {
    let available_text_w = available_text_w.max(0.0);

    let raw_lines = layout_textarea_value_for_paint(
        measurer,
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: available_text_w,
            height: 1_000_000.0,
        },
        style,
        text,
    );

    raw_lines
        .into_iter()
        .map(|line| {
            let fragments: Vec<TextareaCachedTextFragment> = line
                .fragments
                .into_iter()
                .filter(|f| matches!(f.kind, InlineFragment::Text { .. }))
                .map(|f| {
                    let (byte_positions, x_advances) = if build_hit_maps {
                        build_textarea_fragment_hit_map(
                            measurer,
                            style,
                            text,
                            f.source_range,
                            f.rect.width,
                        )
                    } else {
                        (Vec::new(), Vec::new())
                    };

                    TextareaCachedTextFragment {
                        rect: f.rect,
                        source_range: f.source_range,
                        byte_positions,
                        x_advances,
                    }
                })
                .collect();

            let source_range = line.source_range.or_else(|| {
                let mut start: Option<usize> = None;
                let mut end: Option<usize> = None;
                for frag in &fragments {
                    if let Some((s, e)) = frag.source_range {
                        start = Some(start.map(|x| x.min(s)).unwrap_or(s));
                        end = Some(end.map(|x| x.max(e)).unwrap_or(e));
                    }
                }
                match (start, end) {
                    (Some(s), Some(e)) if e >= s => Some((s, e)),
                    _ => None,
                }
            });

            TextareaCachedLine {
                rect: line.rect,
                source_range,
                fragments,
            }
        })
        .collect()
}

fn ensure_textarea_layout_cache<'a>(
    interaction: &'a mut InteractionState,
    input_values: &InputValueStore,
    input_id: Id,
    available_text_w: f32,
    measurer: &EguiTextMeasurer,
    style: &ComputedStyle,
) -> &'a [TextareaCachedLine] {
    let available_text_w = available_text_w.max(0.0);
    let value_rev = input_values.value_revision(input_id);
    let Length::Px(font_px) = style.font_size;

    let cache_valid = interaction.textarea_layout_cache.as_ref().is_some_and(|c| {
        c.input_id == input_id
            && (c.available_text_w - available_text_w).abs() <= 0.5
            && (c.font_px - font_px).abs() <= 0.01
            && c.value_rev == value_rev
    });

    if !cache_valid {
        let value = input_values.get(input_id).unwrap_or("");
        let lines = layout_textarea_cached_lines(measurer, style, available_text_w, value, true);

        interaction.textarea_layout_cache = Some(TextareaLayoutCache {
            input_id,
            available_text_w,
            font_px,
            value_rev,
            lines,
        });
    }

    interaction
        .textarea_layout_cache
        .as_ref()
        .filter(|c| c.input_id == input_id)
        .map(|c| c.lines.as_slice())
        .unwrap_or(&[])
}

fn sync_input_scroll_for_caret(
    input_values: &mut InputValueStore,
    input_id: Id,
    input_rect_w: f32,
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) {
    let (pad_l, pad_r, _pad_t, _pad_b) = input_text_padding(style);
    let available_text_w = (input_rect_w - pad_l - pad_r).max(0.0);

    let (caret_px, text_w) = match input_values.get_state(input_id) {
        Some((value, caret, _sel, _scroll_x, _scroll_y)) => {
            let caret = clamp_caret_to_boundary(value, caret);
            (
                measurer.measure(&value[..caret], style),
                measurer.measure(value, style),
            )
        }
        None => (0.0, 0.0),
    };

    input_values.update_scroll_for_caret(input_id, caret_px, text_w, available_text_w);
}

fn sync_textarea_scroll_for_caret(
    input_values: &mut InputValueStore,
    input_id: Id,
    control_rect_h: f32,
    lines: &[TextareaCachedLine],
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) {
    let (_pad_l, _pad_r, pad_t, pad_b) = input_text_padding(style);
    let available_text_h = (control_rect_h - pad_t - pad_b).max(0.0);

    let (caret_y, caret_h, text_h) = {
        let Some((value, caret, _sel, _scroll_x, _scroll_y)) = input_values.get_state(input_id)
        else {
            return;
        };

        let caret = clamp_caret_to_boundary(value, caret);
        let (_cx, caret_y, caret_h) = textarea_caret_geometry(lines, value, caret, measurer, style);
        let text_h = textarea_text_height(lines, measurer.line_height(style));

        (caret_y, caret_h, text_h)
    };

    input_values.update_scroll_for_caret_y(input_id, caret_y, caret_h, text_h, available_text_h);
}

fn textarea_text_height(lines: &[TextareaCachedLine], fallback_line_h: f32) -> f32 {
    lines
        .last()
        .map(|l| (l.rect.y + l.rect.height).max(0.0))
        .unwrap_or_else(|| fallback_line_h.max(0.0))
}

fn textarea_line_index_from_y(lines: &[TextareaCachedLine], y_in_text: f32, line_h: f32) -> usize {
    if lines.is_empty() {
        return 0;
    }

    let y = y_in_text.max(0.0);

    for (i, line) in lines.iter().enumerate() {
        let top = textarea_visual_line_top(line);
        let h = line.rect.height.max(line_h).max(1.0);
        if y < top + h {
            return i;
        }
    }

    lines.len() - 1
}

fn textarea_visual_line_top(line: &TextareaCachedLine) -> f32 {
    line.rect.y
}

fn textarea_line_index_for_caret(lines: &[TextareaCachedLine], caret: usize) -> usize {
    if lines.is_empty() {
        return 0;
    }

    let i = lines.partition_point(|l| {
        textarea_line_source_range(l).is_some_and(|(start, _end)| start <= caret)
    });
    i.saturating_sub(1).min(lines.len() - 1)
}

fn textarea_line_byte_range(
    lines: &[TextareaCachedLine],
    value: &str,
    line_idx: usize,
) -> (usize, usize) {
    if lines.is_empty() {
        return (0, value.len());
    }

    let i = line_idx.min(lines.len() - 1);
    let start = textarea_line_source_range(&lines[i])
        .map(|(s, _)| s)
        .unwrap_or(0);

    // Prefer the current line's explicit end when available (e.g. excludes the '\n' for hard breaks).
    let end = textarea_line_source_range(&lines[i])
        .map(|(_s, e)| e)
        .or_else(|| {
            if i + 1 < lines.len() {
                textarea_line_source_range(&lines[i + 1]).map(|(s, _e)| s)
            } else {
                None
            }
        })
        .unwrap_or(value.len());

    let end = end.max(start).min(value.len());
    let start = start.min(end);

    (start, end)
}

fn textarea_x_for_index_in_line(
    line: &TextareaCachedLine,
    value: &str,
    index: usize,
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) -> f32 {
    let index = clamp_caret_to_boundary(value, index);

    let mut x = 0.0;
    for frag in &line.fragments {
        let Some((start, end)) = frag.source_range else {
            continue;
        };

        if index <= start {
            return frag.rect.x;
        }
        if index >= end {
            x = frag.rect.x + frag.rect.width;
            continue;
        }

        if !frag.byte_positions.is_empty()
            && frag.byte_positions.len() == frag.x_advances.len()
            && frag.byte_positions.first().copied() == Some(start)
            && frag.byte_positions.last().copied() == Some(end)
        {
            let i = frag.byte_positions.partition_point(|&b| b <= index);
            let i = i.saturating_sub(1).min(frag.x_advances.len() - 1);
            let rel_x = frag.x_advances[i].clamp(0.0, frag.rect.width.max(0.0));
            x = frag.rect.x + rel_x;
        } else if value.is_char_boundary(start) && value.is_char_boundary(index) {
            x = frag.rect.x + measurer.measure(&value[start..index], style);
        } else {
            x = frag.rect.x;
        }
        break;
    }

    x
}

fn textarea_caret_for_x_in_fragment(
    value: &str,
    frag: &TextareaCachedTextFragment,
    x: f32,
    line_start: usize,
    line_end: usize,
) -> usize {
    let line_end = line_end.min(value.len());
    let line_start = line_start.min(line_end);

    let Some((frag_start, frag_end)) = frag.source_range else {
        return line_start;
    };

    let mut start = frag_start.clamp(line_start, line_end);
    let mut end = frag_end.clamp(start, line_end);

    start = clamp_caret_to_boundary(value, start).min(line_end);
    end = clamp_caret_to_boundary(value, end).max(start).min(line_end);

    if start >= end {
        return start;
    }

    let frag_w = frag.rect.width.max(0.0);
    let local_x = (x - frag.rect.x).clamp(0.0, frag_w);
    if local_x <= 0.0 {
        return start;
    }
    if local_x >= frag_w {
        return end;
    }

    if !frag.byte_positions.is_empty()
        && frag.byte_positions.len() == frag.x_advances.len()
        && frag.byte_positions.first().copied() == Some(frag_start)
        && frag.byte_positions.last().copied() == Some(frag_end)
    {
        let start_i = frag.byte_positions.partition_point(|&b| b < start);
        let end_i = frag.byte_positions.partition_point(|&b| b <= end);
        if start_i < end_i {
            let bytes = &frag.byte_positions[start_i..end_i];
            let xs = &frag.x_advances[start_i..end_i];
            if bytes.len() != xs.len() || xs.is_empty() {
                return start;
            }

            let i = xs.partition_point(|&ax| ax <= local_x);
            let left = i.saturating_sub(1).min(xs.len() - 1);
            let left_x = xs[left];
            let left_byte = bytes[left];

            if i < xs.len() {
                let right_x = xs[i];
                let right_byte = bytes[i];
                if local_x - left_x > right_x - local_x {
                    return right_byte;
                }
            }

            return left_byte;
        }
    }

    // Fallback: approximate by character index without shaping.
    let slice = &value[start..end];
    let char_count = slice.chars().count();
    if char_count == 0 {
        return start;
    }

    let t = (local_x / frag_w).clamp(0.0, 1.0);
    let target = (t * char_count as f32).round() as usize;
    if target == 0 {
        return start;
    }
    if target >= char_count {
        return end;
    }

    for (i, (byte_off, _ch)) in slice.char_indices().enumerate() {
        if i == target {
            return start + byte_off;
        }
    }

    end
}

fn textarea_caret_for_x_in_line(
    line: &TextareaCachedLine,
    value: &str,
    x: f32,
    line_start: usize,
    line_end: usize,
) -> usize {
    let x = x.max(0.0);
    let Some(first) = line.fragments.first() else {
        return line_start;
    };

    let mut prev: Option<&TextareaCachedTextFragment> = None;
    for frag in &line.fragments {
        let left = frag.rect.x;
        let right = frag.rect.x + frag.rect.width.max(0.0);

        if x < left {
            return match prev {
                None => textarea_caret_for_x_in_fragment(value, first, left, line_start, line_end),
                Some(prev) => {
                    let prev_right = prev.rect.x + prev.rect.width.max(0.0);
                    if left - x < x - prev_right {
                        textarea_caret_for_x_in_fragment(value, frag, left, line_start, line_end)
                    } else {
                        textarea_caret_for_x_in_fragment(
                            value, prev, prev_right, line_start, line_end,
                        )
                    }
                }
            };
        }

        if x <= right {
            return textarea_caret_for_x_in_fragment(value, frag, x, line_start, line_end);
        }

        prev = Some(frag);
    }

    // After the last fragment: snap to the end of it.
    let last = prev.unwrap_or(first);
    let last_right = last.rect.x + last.rect.width.max(0.0);
    textarea_caret_for_x_in_fragment(value, last, last_right, line_start, line_end)
}

fn textarea_caret_for_x_in_lines(
    lines: &[TextareaCachedLine],
    value: &str,
    line_idx: usize,
    x: f32,
) -> usize {
    if lines.is_empty() {
        return 0;
    }

    let i = line_idx.min(lines.len() - 1);
    let (line_start, line_end) = textarea_line_byte_range(lines, value, i);
    textarea_caret_for_x_in_line(&lines[i], value, x, line_start, line_end)
}

fn textarea_caret_geometry(
    lines: &[TextareaCachedLine],
    value: &str,
    caret: usize,
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) -> (f32, f32, f32) {
    let line_h = measurer.line_height(style);
    if lines.is_empty() {
        return (0.0, 0.0, line_h);
    }

    let caret = clamp_caret_to_boundary(value, caret);
    let line_idx = textarea_line_index_for_caret(lines, caret);
    let line = &lines[line_idx];

    let x = textarea_x_for_index_in_line(line, value, caret, measurer, style);
    let y = line.rect.y;
    let h = line.rect.height.max(line_h);

    (x, y, h)
}

#[derive(Clone, Copy)]
struct TextAreaSelectionPaintParams<'a> {
    inner_origin: Pos2,
    scroll_y: f32,
    measurer: &'a dyn TextMeasurer,
    style: &'a ComputedStyle,
    selection_bg_fill: Color32,
}

fn paint_textarea_selection(
    painter: &Painter,
    lines: &[TextareaCachedLine],
    value: &str,
    sel: SelectionRange,
    params: TextAreaSelectionPaintParams<'_>,
) {
    let TextAreaSelectionPaintParams {
        inner_origin,
        scroll_y,
        measurer,
        style,
        selection_bg_fill,
    } = params;

    if lines.is_empty() || value.is_empty() || sel.start >= sel.end {
        return;
    }

    let sel_start = sel.start.min(value.len());
    let sel_end = sel.end.min(value.len());

    if !(value.is_char_boundary(sel_start) && value.is_char_boundary(sel_end)) {
        return;
    }

    for line in lines {
        let Some((line_start, line_end_display)) = textarea_line_source_range(line) else {
            continue;
        };

        let a = sel_start.clamp(line_start, line_end_display);
        let b = sel_end.clamp(line_start, line_end_display);
        if a >= b {
            continue;
        }

        let x0 = textarea_x_for_index_in_line(line, value, a, measurer, style);
        let x1 = textarea_x_for_index_in_line(line, value, b, measurer, style);

        let y = inner_origin.y + line.rect.y - scroll_y;
        let h = line.rect.height.max(measurer.line_height(style)).max(1.0);

        let rect = Rect::from_min_max(
            Pos2 {
                x: inner_origin.x + x0,
                y,
            },
            Pos2 {
                x: inner_origin.x + x1,
                y: y + h,
            },
        );

        painter.rect_filled(rect, 0.0, selection_bg_fill);
    }
}

struct TextareaVerticalMoveCtx<'a> {
    lines: &'a [TextareaCachedLine],
    measurer: &'a dyn TextMeasurer,
    style: &'a ComputedStyle,
}

fn textarea_move_caret_vertically(
    input_values: &mut InputValueStore,
    input_id: Id,
    delta_lines: i32,
    preferred_x: Option<f32>,
    ctx: TextareaVerticalMoveCtx<'_>,
    selecting: bool,
) -> Option<f32> {
    let TextareaVerticalMoveCtx {
        lines,
        measurer,
        style,
    } = ctx;

    if delta_lines == 0 {
        return preferred_x;
    }

    let Some((value, caret)) = input_values
        .get_state(input_id)
        .map(|(value, caret, _sel, _sx, _sy)| (value, caret))
    else {
        return preferred_x;
    };

    if lines.is_empty() {
        return preferred_x;
    }

    let caret = clamp_caret_to_boundary(value, caret);

    // Keep the "column" stable across vertical moves.
    let x = preferred_x.unwrap_or_else(|| {
        let (x, _y, _h) = textarea_caret_geometry(lines, value, caret, measurer, style);
        x
    });

    let cur_line = textarea_line_index_for_caret(lines, caret);
    let last_line = lines.len() - 1;

    // --- NEW: boundary behavior like browsers ---
    if selecting {
        if delta_lines < 0 && cur_line == 0 {
            // Shift+Up at first line => go to start of first line
            let (line_start, _line_end) = textarea_line_byte_range(lines, value, cur_line);
            input_values.set_caret(input_id, line_start, true);
            return Some(x.max(0.0));
        }
        if delta_lines > 0 && cur_line == last_line {
            // Shift+Down at last line => go to end of last line
            let (_line_start, line_end) = textarea_line_byte_range(lines, value, cur_line);
            input_values.set_caret(input_id, line_end, true);
            return Some(x.max(0.0));
        }
    }

    // Normal vertical move (within bounds)
    let target_line = if delta_lines < 0 {
        cur_line.saturating_sub((-delta_lines) as usize)
    } else {
        (cur_line + (delta_lines as usize)).min(last_line)
    };

    let (line_start, line_end) = textarea_line_byte_range(lines, value, target_line);
    let line = &lines[target_line];
    let new_caret = textarea_caret_for_x_in_line(line, value, x, line_start, line_end);

    input_values.set_caret(input_id, new_caret, selecting);
    Some(x.max(0.0))
}

fn textarea_line_source_range(line: &TextareaCachedLine) -> Option<(usize, usize)> {
    if let Some(r) = line.source_range {
        return Some(r);
    }

    // Soft-wrapped lines may not have line.source_range set.
    // Derive it from fragment source ranges.
    let mut start: Option<usize> = None;
    let mut end: Option<usize> = None;

    for frag in &line.fragments {
        if let Some((s, e)) = frag.source_range {
            start = Some(start.map(|x| x.min(s)).unwrap_or(s));
            end = Some(end.map(|x| x.max(e)).unwrap_or(e));
        }
    }

    match (start, end) {
        (Some(s), Some(e)) if e >= s => Some((s, e)),
        _ => None,
    }
}

fn consume_focus_nav_keys(i: &mut egui::InputState) {
    // Prevent egui / other widgets from hijacking these while a DOM input is focused:
    i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
    i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab);
    i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
}

fn get_attr<'a>(node: &'a Node, name: &str) -> Option<&'a str> {
    match node {
        Node::Element { attributes, .. } => {
            for (k, v) in attributes {
                if k.eq_ignore_ascii_case(name) {
                    return v.as_deref();
                }
            }
            None
        }
        _ => None,
    }
}

fn find_layout_box_by_id<'a>(root: &'a LayoutBox<'a>, id: Id) -> Option<&'a LayoutBox<'a>> {
    if root.node_id() == id {
        return Some(root);
    }
    for c in &root.children {
        if let Some(found) = find_layout_box_by_id(c, id) {
            return Some(found);
        }
    }
    None
}

fn resolve_relative_url(base_url: Option<&str>, href: &str) -> Option<String> {
    // If no base_url (e.g. initial about:blank), just pass through.
    let Some(base) = base_url else {
        return Some(href.to_string());
    };

    let base = url::Url::parse(base).ok()?;
    base.join(href).ok().map(|u| u.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImgFallbackState {
    Missing,
    Loading,
    Error,
}

fn paint_img_fallback_placeholder(
    painter: &Painter,
    rect: Rect,
    style: &ComputedStyle,
    measurer: &dyn TextMeasurer,
    state: ImgFallbackState,
    alt: Option<&str>,
) {
    // Placeholder box
    let (r, g, b, a) = style.background_color;
    let fill = if a > 0 {
        Color32::from_rgba_unmultiplied(r, g, b, a)
    } else {
        Color32::from_rgba_unmultiplied(220, 220, 220, 255)
    };

    painter.rect_filled(rect, 2.0, fill);
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, Color32::from_rgb(120, 120, 120)),
        StrokeKind::Outside,
    );

    // Error decoration (subtle "broken" cross)
    if matches!(state, ImgFallbackState::Error) {
        let inset = 5.0;
        if rect.width() > inset * 2.0 && rect.height() > inset * 2.0 {
            let a = rect.min + Vec2::new(inset, inset);
            let b = rect.max - Vec2::new(inset, inset);
            let c = Pos2 { x: a.x, y: b.y };
            let d = Pos2 { x: b.x, y: a.y };

            let stroke = Stroke::new(1.5, Color32::from_rgba_unmultiplied(220, 80, 80, 140));
            painter.line_segment([a, b], stroke);
            painter.line_segment([c, d], stroke);
        }
    }

    // Text content (status + alt)
    let padding = 6.0;
    let inner = rect.shrink(padding);
    if inner.width() <= 1.0 || inner.height() <= 1.0 {
        return;
    }

    let clip_painter = painter.with_clip_rect(rect);

    let (cr, cg, cb, ca) = style.color;
    let base_text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);

    let status = match state {
        ImgFallbackState::Loading => Some("Loading…"),
        ImgFallbackState::Error => Some("Failed to load image"),
        ImgFallbackState::Missing => None,
    };

    let main_text = match (state, alt) {
        (ImgFallbackState::Error, Some(alt)) => Some(alt),
        (ImgFallbackState::Error, None) => Some("Broken image"),
        (ImgFallbackState::Loading, Some(alt)) => Some(alt),
        (ImgFallbackState::Loading, None) => None,
        (ImgFallbackState::Missing, Some(alt)) => Some(alt),
        (ImgFallbackState::Missing, None) => Some("IMG"),
    };

    let mut y = inner.min.y;
    let mut remaining_h = inner.height();

    if let Some(status) = status {
        let mut status_style = *style;
        let Length::Px(font_px) = style.font_size;
        status_style.font_size = Length::Px((font_px * 0.85).clamp(10.0, 12.0));

        let status_color = base_text_color.gamma_multiply(0.65);
        let font_id = match status_style.font_size {
            Length::Px(px) => FontId::proportional(px),
        };
        clip_painter.text(
            Pos2 { x: inner.min.x, y },
            Align2::LEFT_TOP,
            status,
            font_id,
            status_color,
        );

        let status_h = measurer.line_height(&status_style);
        y += status_h;
        remaining_h = (remaining_h - status_h).max(0.0);
    }

    if let Some(text) = main_text
        && remaining_h > 1.0
    {
        paint_wrapped_text(
            &clip_painter,
            Rect::from_min_size(
                Pos2 { x: inner.min.x, y },
                Vec2::new(inner.width(), remaining_h),
            ),
            style,
            measurer,
            text,
            base_text_color,
        );
    }
}

fn paint_wrapped_text(
    painter: &Painter,
    rect: Rect,
    style: &ComputedStyle,
    measurer: &dyn TextMeasurer,
    text: &str,
    color: Color32,
) {
    let max_w = rect.width().max(0.0);
    let max_h = rect.height().max(0.0);
    if max_w <= 1.0 || max_h <= 1.0 {
        return;
    }

    let line_h = measurer.line_height(style).max(1.0);
    let max_lines = (max_h / line_h).floor().max(0.0) as usize;
    if max_lines == 0 {
        return;
    }

    let mut lines = wrap_text_to_width(text, max_w, measurer, style);
    if lines.is_empty() {
        return;
    }

    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            if !last.ends_with('…') {
                last.push('…');
            }
            *last = ellipsize_to_width(last, max_w, measurer, style);
        }
    }

    let font_id = match style.font_size {
        Length::Px(px) => FontId::proportional(px),
    };

    for (i, line) in lines.iter().enumerate() {
        let y = rect.min.y + (i as f32) * line_h;
        if y > rect.max.y {
            break;
        }
        painter.text(
            Pos2 { x: rect.min.x, y },
            Align2::LEFT_TOP,
            line,
            font_id.clone(),
            color,
        );
    }
}

fn wrap_text_to_width(
    text: &str,
    max_width: f32,
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            let w = measurer.measure(word, style);
            if w <= max_width {
                current.push_str(word);
            } else {
                lines.push(ellipsize_to_width(word, max_width, measurer, style));
            }
            continue;
        }

        let candidate = format!("{current} {word}");
        if measurer.measure(&candidate, style) <= max_width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));

            let w = measurer.measure(word, style);
            if w <= max_width {
                current.push_str(word);
            } else {
                lines.push(ellipsize_to_width(word, max_width, measurer, style));
            }
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn ellipsize_to_width(
    text: &str,
    max_width: f32,
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) -> String {
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }
    if !(max_width.is_finite() && max_width > 0.0) {
        return String::new();
    }

    if measurer.measure(text, style) <= max_width {
        return text.to_string();
    }

    let ellipsis = "…";
    if measurer.measure(ellipsis, style) > max_width {
        return String::new();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut lo: usize = 0;
    let mut hi: usize = chars.len();

    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        let mut candidate: String = chars[..mid].iter().collect();
        candidate.push_str(ellipsis);

        if measurer.measure(&candidate, style) <= max_width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    let mut out: String = chars[..lo].iter().collect();
    out.push_str(ellipsis);
    out
}

struct BrowserReplacedInfo<'a> {
    base_url: Option<&'a str>,
    resources: &'a ResourceManager,
}

impl ReplacedElementInfoProvider for BrowserReplacedInfo<'_> {
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

    #[test]
    fn textarea_line_byte_range_prefers_line_end_over_next_start() {
        let value = "a\nb";

        let lines: Vec<TextareaCachedLine> = vec![
            TextareaCachedLine {
                fragments: Vec::new(),
                rect: Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                },
                source_range: Some((0, 1)), // excludes '\n'
            },
            TextareaCachedLine {
                fragments: Vec::new(),
                rect: Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                },
                source_range: Some((2, 3)),
            },
        ];

        assert_eq!(textarea_line_byte_range(&lines, value, 0), (0, 1));
        assert_eq!(textarea_line_byte_range(&lines, value, 1), (2, 3));
    }

    #[derive(Clone, Copy)]
    struct FixedMeasurer;

    impl TextMeasurer for FixedMeasurer {
        fn measure(&self, text: &str, _style: &ComputedStyle) -> f32 {
            text.chars().count() as f32
        }

        fn line_height(&self, _style: &ComputedStyle) -> f32 {
            10.0
        }
    }

    #[test]
    fn ellipsize_to_width_never_exceeds_limit() {
        let measurer = FixedMeasurer;
        let style = ComputedStyle::initial();

        let s = ellipsize_to_width("hello world", 5.0, &measurer, &style);
        assert!(measurer.measure(&s, &style) <= 5.0);
        assert!(s.ends_with('…') || s.is_empty());
    }

    #[test]
    fn wrap_text_to_width_respects_width_per_line() {
        let measurer = FixedMeasurer;
        let style = ComputedStyle::initial();

        let lines = wrap_text_to_width("a bb ccc dddd", 3.0, &measurer, &style);
        assert!(!lines.is_empty());
        for line in &lines {
            assert!(measurer.measure(line, &style) <= 3.0);
        }
    }
}
