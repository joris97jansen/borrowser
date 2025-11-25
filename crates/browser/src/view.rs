use crate::page::PageState;
use crate::tab::Tab;

use html::{
    Node,
    dom_utils::direct_text_of_element,
};
use css::{
    build_style_tree,
    StyledNode,
};
use layout::{
    layout_block_tree,
};
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
    Align2,
    FontId
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
    // If there is no DOM yet, keep the old behavior: follow OS theme.
    let dom = match page.dom.as_ref() {
        Some(dom) => dom,
        None => {
            let visuals = ctx.style().visuals.clone();
            CentralPanel::default()
                .frame(Frame::default().fill(visuals.panel_fill))
                .show(ctx, |ui| {
                    if loading { ui.label("⏳ Loading…"); }
                    if let Some(s) = status { ui.label(s); }
                });
            return;
        }
    };

    // 1) Build style tree ONCE for this frame
    let style_root = build_style_tree(dom, None);

    // 2) Decide the "page background" from computed styles
    let page_bg = find_page_background_color(&style_root);

    let base_fill = if let Some((r, g, b, a)) = page_bg {
        Color32::from_rgba_unmultiplied(r, g, b, a)
    } else {
        // No explicit page background → default to white like real browsers
        Color32::WHITE
    };

    // 3) Paint CentralPanel with the page background,
    //    then render the scrollable layout on top.
    CentralPanel::default()
        .frame(Frame::default().fill(base_fill))
        .show(ctx, |ui| {
            page_viewport(ui, &style_root);

            if loading { ui.label("⏳ Loading…"); }
            if let Some(s) = status { ui.label(s); }
        });
}

pub fn page_viewport(ui: &mut Ui, style_root: &StyledNode<'_>) {
    ScrollArea::vertical()
        .id_salt("page_viewport_scroll_area")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // 1) Page geometry
            let available_width = ui.available_width();
            let min_height = ui.available_height().max(200.0);

            // 2) Block layout
            let layout_root = layout_block_tree(style_root, available_width);

            let content_height = layout_root.rect.height.max(min_height);

            // 3) Reserve paint area
            let (content_rect, _resp) = ui.allocate_exact_size(
                Vec2::new(available_width, content_height),
                egui::Sense::hover(),
            );

            // 4) Paint layout tree (backgrounds + text)
            let painter = ui.painter_at(content_rect);
            let origin = content_rect.min;
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

    // background
    let (r, g, b, a) = layout.style.background_color;
    if a > 0 {
        painter.rect_filled(
            rect,
            0.0,
            Color32::from_rgba_unmultiplied(r, g, b, a),
        );
    }

    // text for direct text children
    if let Some(text) = direct_text_of_element(layout.node.node) {
        paint_block_text_in_rect(painter, &text, rect, &layout.style);
    }

    // children
    for child in &layout.children {
        paint_layout_box(child, painter, origin);
    }
}

fn paint_block_text_in_rect(
    painter: &egui::Painter,
    text: &str,
    rect: Rect,
    style: &css::ComputedStyle,
) {
    let (cr, cg, cb, ca) = style.color;
    let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);

    let mut font_px = match style.font_size {
        css::Length::Px(px) => px,
    };

    let padding = 4.0;
    let available_height = rect.height() - 2.0 * padding;
    let line_height = font_px * 1.2;

    // Clamp font size if it would obviously overflow
    if line_height > available_height && available_height > 0.0 {
        font_px = (available_height / 1.2).max(8.0);
    }

    let font_id = FontId::proportional(font_px);
    let max_width = rect.width() - 2.0 * padding;

    // --- Very naive word-wrap: split on spaces, accumulate into lines ---
    let words = text.split_whitespace();
    let mut current_line = String::new();
    let mut lines = Vec::new();

    for w in words {
        let candidate = if current_line.is_empty() {
            w.to_string()
        } else {
            format!("{} {}", current_line, w)
        };

        let galley = painter.ctx().fonts(|f| f.layout_no_wrap(candidate.clone(), font_id.clone(), text_color));
        let width = galley.rect.width();

        if width <= max_width || current_line.is_empty() {
            // keep adding to this line
            current_line = candidate;
        } else {
            // push current line, start new
            lines.push(current_line);
            current_line = w.to_string();
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }

    // --- Paint lines, top-down ---
    let mut y = rect.min.y + padding;
    for line in lines {
        if y + line_height > rect.max.y - padding {
            break; // no more vertical space
        }

        let pos = Pos2 {
            x: rect.min.x + padding,
            y,
        };

        painter.text(
            pos,
            Align2::LEFT_TOP,
            line,
            font_id.clone(),
            text_color,
        );

        y += line_height;
    }
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
