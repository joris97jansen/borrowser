use super::super::*;
use crate::text_measurer::EguiTextMeasurer;
use egui::{CentralPanel, Context, Event, Pos2, RawInput, Rect, Sense, Vec2};
use html::{Node, internal::Id};
use input_core::{
    InputId, InputStore, InputValueStore, caret_from_x_with_boundaries, rebuild_cursor_boundaries,
};
use layout::inline::{InlineAction, InlineActionKind, InlineFragment};
use layout::{LayoutBox, Rectangle, TextMeasurer, content_height, content_x_and_width, content_y};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

pub(super) use crate::input::to_input_id;

pub(super) struct TestFormControls;

impl<S: InputStore + ?Sized> FormControlHandler<S> for TestFormControls {
    fn on_radio_clicked(&self, store: &mut S, radio_id: InputId) -> bool {
        store.set_checked(radio_id, true)
    }
}

pub(super) fn doc(children: Vec<Node>) -> Node {
    Node::Document {
        id: Id(0),
        doctype: None,
        children,
    }
}

pub(super) fn elem(
    id: u32,
    name: &str,
    attributes: Vec<(Arc<str>, Option<String>)>,
    style: Vec<(String, String)>,
    children: Vec<Node>,
) -> Node {
    Node::Element {
        id: Id(id),
        name: Arc::from(name),
        attributes,
        style,
        children,
    }
}

pub(super) fn text(id: u32, value: &str) -> Node {
    Node::Text {
        id: Id(id),
        text: value.to_string(),
    }
}

pub(super) fn style_inline_block() -> Vec<(String, String)> {
    vec![("display".to_string(), "inline-block".to_string())]
}

pub(super) fn style_inline() -> Vec<(String, String)> {
    vec![("display".to_string(), "inline".to_string())]
}

pub(super) fn input_text(id: u32) -> Node {
    elem(
        id,
        "input",
        vec![(Arc::from("type"), Some("text".to_string()))],
        style_inline_block(),
        Vec::new(),
    )
}

pub(super) fn input_checkbox(id: u32) -> Node {
    elem(
        id,
        "input",
        vec![(Arc::from("type"), Some("checkbox".to_string()))],
        style_inline_block(),
        Vec::new(),
    )
}

pub(super) fn input_radio(id: u32) -> Node {
    elem(
        id,
        "input",
        vec![(Arc::from("type"), Some("radio".to_string()))],
        style_inline_block(),
        Vec::new(),
    )
}

pub(super) fn link(id: u32, href: &str, children: Vec<Node>) -> Node {
    elem(
        id,
        "a",
        vec![(Arc::from("href"), Some(href.to_string()))],
        style_inline(),
        children,
    )
}

pub(super) fn raw_input(events: Vec<Event>) -> RawInput {
    RawInput {
        events,
        screen_rect: Some(Rect::from_min_size(
            Pos2::new(0.0, 0.0),
            Vec2::new(1200.0, 900.0),
        )),
        ..Default::default()
    }
}

pub(super) fn content_origin(ctx: &Context, size: Vec2) -> Pos2 {
    let origin = RefCell::new(None);
    let _ = ctx.run(raw_input(Vec::new()), |ctx| {
        CentralPanel::default().show(ctx, |ui| {
            let (content_rect, _resp) = ui.allocate_exact_size(size, Sense::hover());
            *origin.borrow_mut() = Some(content_rect.min);
        });
    });
    origin.into_inner().unwrap()
}

pub(super) fn init_context(ctx: &Context) {
    let _ = ctx.run(raw_input(Vec::new()), |_| {});
}

pub(super) struct FrameRun<'a, 'layout, S: InputStore + ?Sized, F: FormControlHandler<S>> {
    pub ctx: &'a Context,
    pub raw_input: RawInput,
    pub layout_root: &'a LayoutBox<'layout>,
    pub measurer: &'a EguiTextMeasurer,
    pub base_url: Option<&'a str>,
    pub input_values: &'a mut S,
    pub form_controls: &'a F,
    pub interaction: &'a mut InteractionState,
    pub content_size: Vec2,
    pub layout_changed: bool,
}

pub(super) fn run_frame<S: InputStore + ?Sized, F: FormControlHandler<S>>(
    args: FrameRun<'_, '_, S, F>,
) -> Option<PageAction> {
    let FrameRun {
        ctx,
        raw_input,
        layout_root,
        measurer,
        base_url,
        input_values,
        form_controls,
        interaction,
        content_size,
        layout_changed,
    } = args;
    let action_cell = RefCell::new(None);
    let _ = ctx.run(raw_input, |ctx| {
        CentralPanel::default().show(ctx, |ui| {
            let (content_rect, resp) = ui.allocate_exact_size(content_size, Sense::hover());
            let origin = content_rect.min;
            let fragment_rects: RefCell<HashMap<Id, Rectangle>> = RefCell::new(HashMap::new());

            let action = route_frame_input(FrameInputCtx {
                ui,
                resp,
                content_rect,
                origin,
                layout_root,
                measurer,
                layout_changed,
                fragment_rects: &fragment_rects,
                base_url,
                input_values,
                form_controls,
                interaction,
            });
            *action_cell.borrow_mut() = action;
        });
    });
    action_cell.into_inner()
}

pub(super) fn pos_in_rect(origin: Pos2, rect: Rectangle, dx: f32, dy: f32) -> Pos2 {
    Pos2::new(origin.x + rect.x + dx, origin.y + rect.y + dy)
}

pub(super) fn pos_center(origin: Pos2, rect: Rectangle) -> Pos2 {
    pos_in_rect(origin, rect, rect.width * 0.5, rect.height * 0.5)
}

pub(super) fn find_link_fragment_rect<'a>(
    root: &'a LayoutBox<'a>,
    measurer: &dyn TextMeasurer,
    link_id: Id,
) -> Option<Rectangle> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if let Node::Element { .. } = node.node.node {
            let (content_x, content_w) =
                content_x_and_width(node.style, node.rect.x, node.rect.width);
            let content_y = content_y(node.style, node.rect.y);
            let content_h = content_height(node.style, node.rect.height);
            let block_rect = Rectangle {
                x: content_x,
                y: content_y,
                width: content_w,
                height: content_h,
            };

            for line in layout::layout_inline_for_paint(measurer, block_rect, node) {
                for frag in line.fragments {
                    let action = match &frag.kind {
                        InlineFragment::Text { action, .. } => action,
                        InlineFragment::Box { action, .. } => action,
                        InlineFragment::Replaced { action, .. } => action,
                    };
                    if let Some(InlineAction {
                        target,
                        kind: InlineActionKind::Link,
                        ..
                    }) = action.as_ref()
                        && *target == link_id
                    {
                        return Some(frag.paint_rect.rect());
                    }
                }
            }
        }
        for child in &node.children {
            stack.push(child);
        }
    }
    None
}

pub(super) fn find_fragment_rect_for_node<'a>(
    root: &'a LayoutBox<'a>,
    measurer: &dyn TextMeasurer,
    node_id: Id,
) -> Option<Rectangle> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if let Node::Element { .. } = node.node.node {
            let (content_x, content_w) =
                content_x_and_width(node.style, node.rect.x, node.rect.width);
            let content_y = content_y(node.style, node.rect.y);
            let content_h = content_height(node.style, node.rect.height);
            let block_rect = Rectangle {
                x: content_x,
                y: content_y,
                width: content_w,
                height: content_h,
            };

            for line in layout::layout_inline_for_paint(measurer, block_rect, node) {
                for frag in line.fragments {
                    let layout_ref = match &frag.kind {
                        InlineFragment::Box { layout, .. } => *layout,
                        InlineFragment::Replaced { layout, .. } => *layout,
                        InlineFragment::Text { .. } => None,
                    };
                    if layout_ref.is_some_and(|lb| lb.node_id() == node_id) {
                        return Some(frag.paint_rect.rect());
                    }
                }
            }
        }
        for child in &node.children {
            stack.push(child);
        }
    }
    None
}

pub(super) fn expected_caret_for_x(
    measurer: &EguiTextMeasurer,
    style: &css::ComputedStyle,
    value: &str,
    x: f32,
) -> usize {
    let mut boundaries = Vec::new();
    rebuild_cursor_boundaries(value, &mut boundaries);
    caret_from_x_with_boundaries(value, &boundaries, x, |s| measurer.measure(s, style))
}

pub(super) type Store = InputValueStore;
