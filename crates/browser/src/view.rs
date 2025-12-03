use crate::page::PageState;
use crate::tab::Tab;

use html::{
    Node,
    dom_utils::{
        is_non_rendering_element,
    },
};
use css::{
    build_style_tree,
    StyledNode,
    ComputedStyle,
    Length,
};
use layout::{
    layout_block_tree,
    LayoutBox,
    TextMeasurer,
    inline::{
        LineBox,
        is_inline_element_name,
        collect_inline_runs_for_block,
        layout_inline_runs,
        refine_layout_with_inline,
    }
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
    FontId,
    Painter,
    Stroke,
    TextEdit,
    Sense,
};

pub enum NavigationAction {
    None,
    Back,
    Forward,
    Refresh,
    Navigate(String),
}

struct EguiTextMeasurer<'a> {
    ctx: &'a egui::Context,
}

impl<'a> EguiTextMeasurer<'a> {
    fn new(ctx: &'a egui::Context) -> Self {
        Self { ctx }
    }
}

impl<'a> TextMeasurer for EguiTextMeasurer<'a> {
    fn measure(&self, text: &str, style: &ComputedStyle) -> f32 {
        // We don't really care about color here, but egui wants one for layout.
        let (r, g, b, a) = style.color;
        let color = Color32::from_rgba_unmultiplied(r, g, b, a);

        let font_px = match style.font_size {
            Length::Px(px) => px,
        };
        let font_id = FontId::proportional(font_px);

        self.ctx.fonts(|f| {
            f.layout_no_wrap(text.to_owned(), font_id, color)
                .rect
                .width()
        })
    }

    fn line_height(&self, style: &css::ComputedStyle) -> f32 {
        // Same factor you already used elsewhere; now it's centralized.
        match style.font_size {
            Length::Px(px) => px * 1.2,
        }
    }
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
                .stroke(Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color))
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
            let mut layout_root = layout_block_tree(style_root, available_width);

            // 2b) Refine block heights using inline layout
            let measurer = EguiTextMeasurer::new(ui.ctx());
            refine_layout_with_inline(&measurer, &mut layout_root);

            let content_height = layout_root.rect.height.max(min_height);

            // 3) Reserve paint area
            let (content_rect, _resp) = ui.allocate_exact_size(
                Vec2::new(available_width, content_height),
                Sense::hover(),
            );

            // 4) Paint layout tree (backgrounds + text)
            let painter = ui.painter_at(content_rect);
            let origin = content_rect.min;
            paint_layout_box(&layout_root, &painter, origin);
        });
}

fn paint_line_boxes<'a>(
    painter: &egui::Painter,
    origin: Pos2,
    lines: &[LineBox<'a>],
) {
    for line in lines {
        for frag in &line.fragments {
            let (cr, cg, cb, ca) = frag.style.color;
            let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);

            let font_px = match frag.style.font_size {
                css::Length::Px(px) => px,
            };
            let font_id = FontId::proportional(font_px);

            let pos = Pos2 {
                x: origin.x + frag.rect.x,
                y: origin.y + frag.rect.y,
            };

            painter.text(
                pos,
                Align2::LEFT_TOP,
                &frag.text,
                font_id,
                text_color,
            );
        }
    }
}


fn paint_layout_box<'a>(
    layout: &LayoutBox<'a>,
    painter: &Painter,
    origin: Pos2,
) {
    // 0) Do not paint non-rendering elements (head, style, script, etc.)
    if is_non_rendering_element(layout.node.node) {
        for child in &layout.children {
            paint_layout_box(child, painter, origin);
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
        painter.rect_filled(
            rect,
            0.0,
            Color32::from_rgba_unmultiplied(r, g, b, a),
        );
    }

    // 1) Inline text: only for block-like elements.
    if let Node::Element { name, .. } = layout.node.node {
        if !is_inline_element_name(name) {
            let runs = collect_inline_runs_for_block(layout.node);
            if !runs.is_empty() {
                let measurer = EguiTextMeasurer::new(painter.ctx());
                let lines = layout_inline_runs(&measurer, layout.rect, &layout.style, &runs);
                paint_line_boxes(painter, origin, &lines);
            }
        }
    }

    // 2) Recurse into children
    for child in &layout.children {
        paint_layout_box(child, painter, origin);
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
