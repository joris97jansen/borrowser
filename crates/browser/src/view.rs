use crate::page::PageState;
use crate::tab::Tab;

use html::{
    Node,
};
use css::build_style_tree;
use layout::layout_block_tree;
use egui::{
    Align,
    Button,
    Context,
    TopBottomPanel,
    Key,
    CentralPanel,
    ScrollArea,
    Color32,
    Frame,
    Margin,
    Ui,
    Pos2,
    Rect,
    Vec2,
};

pub enum NavigationAction {
    None,
    Back,
    Forward,
    Refresh,
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

            if ui.add_enabled(can_go_back, Button::new("⬅").min_size([BUTTON_WIDTH, BAR_HEIGHT].into())).clicked() {
                action = NavigationAction::Back;
            }
            if ui.add_enabled(can_go_forward, Button::new("➡").min_size([BUTTON_WIDTH, BAR_HEIGHT].into())).clicked() {
                action = NavigationAction::Forward;
            }
            if ui.add_sized([BUTTON_WIDTH, BAR_HEIGHT], Button::new("🔄")).clicked()
            {
                action = NavigationAction::Refresh;
            }

            let response = Frame::new()
                .fill(ui.visuals().extreme_bg_color) // subtle background
                .stroke(egui::Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color))
                .corner_radius(6.0)
                .inner_margin(Margin::symmetric(4, 4))
                .show(ui, |ui| {
                    ui.add_sized(
                        [ui.available_width(), BAR_HEIGHT - 8.0],
                        egui::TextEdit::singleline(&mut tab.url)
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
    page: &PageState,
    status: Option<&String>,
    loading: bool,
) {
    let has_page = page.dom.is_some();

    // 1) Ask egui what the current theme visuals are
    let visuals = ctx.style().visuals.clone();

    // 2) Decide the base fill color:
    //    - no page yet → follow OS/theme (panel fill)
    //    - page loaded → default white (like real browsers)
    let base_fill = if has_page {
        Color32::WHITE
    } else {
        visuals.panel_fill
    };
    CentralPanel::default()
        .frame(Frame::default().fill(base_fill))
        .show(ctx, |ui| {
        if let Some(dom) = page.dom.as_ref() {
            page_viewport(ui, dom);
        }

        if loading { ui.label("⏳ Loading…"); }
        if let Some(s) = status { ui.label(s); }
    });
}

pub fn page_viewport(ui: &mut Ui, dom: &Node) {
    ScrollArea::vertical()
        .id_salt("page_viewport_scroll_area")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // 1) Figure out how wide our "page" is
            let available_width = ui.available_width();
            let min_height = ui.available_height().max(200.0);

            // 2) Build style tree from DOM.
            //    DOM should already have Node::Element.style filled by attach_styles().
            let style_root = build_style_tree(dom, None);

            // 3) Run simple block layout for this style tree
            let layout_root = layout_block_tree(&style_root, available_width);

            // Total content height (layout_root.rect.height) might be very small
            // for now (we're using a constant height per leaf), so ensure a
            // minimum height to avoid weird visuals.
            let content_height = layout_root.rect.height.max(min_height);

            // 4) Allocate a paint area inside the ScrollArea
            let (content_rect, _resp) = ui.allocate_exact_size(
                Vec2::new(available_width, content_height),
                egui::Sense::hover(),
            );
            let painter = ui.painter_at(content_rect);
            let origin = content_rect.min; // top-left corner of our page area

            // 5) Paint layout tree (background colors only for now)
            paint_layout_box(&layout_root, &painter, origin);
        });
}

fn paint_layout_box<'a>(
    layout: &layout::LayoutBox<'a>,
    painter: &egui::Painter,
    origin: Pos2,
) {
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

    let (r, g, b, a) = layout.style.background_color;
    if a > 0 {
        painter.rect_filled(
            rect,
            0.0,
            Color32::from_rgba_unmultiplied(r, g, b, a),
        );
    }

    for child in &layout.children {
        paint_layout_box(child, painter, origin);
    }
}