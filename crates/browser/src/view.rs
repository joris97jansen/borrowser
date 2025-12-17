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
    Display,
};
use layout::{
    layout_block_tree,
    LayoutBox,
    TextMeasurer,
    Rectangle,
    BoxKind,
    ListMarker,
    ReplacedKind,
    content_x_and_width,
    content_y,
    content_height,
    inline::{
        LineBox,
        InlineFragment,
        layout_inline_for_paint,
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
    StrokeKind,
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

struct EguiTextMeasurer {
    ctx: Context,
}

impl EguiTextMeasurer {
    fn new(ctx: &Context) -> Self {
        Self { ctx: ctx.clone() }
    }
}

impl TextMeasurer for EguiTextMeasurer {
    fn measure(&self, text: &str, style: &ComputedStyle) -> f32 {
        if text == " " {
            let w_with = self.measure("0 0", style);
            let w_without = self.measure("00", style);
            return (w_with - w_without).max(0.0);
        }

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


    fn line_height(&self, style: &ComputedStyle) -> f32 {
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
            page_viewport(ui, &style_root, page);

            if loading { ui.label("⏳ Loading…"); }
            if let Some(s) = status { ui.label(s); }
        });
}

pub fn page_viewport(ui: &mut Ui, style_root: &StyledNode<'_>, page: &PageState) {
    ScrollArea::vertical()
        .id_salt("page_viewport_scroll_area")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // 1) Page geometry
            let available_width = ui.available_width();
            let min_height = ui.available_height().max(200.0);

            // 2) Block layout (inline-aware, single authoritative pass)
            let measurer = EguiTextMeasurer::new(ui.ctx());
            let layout_root = layout_block_tree(style_root, available_width, &measurer);

            let content_height = layout_root.rect.height.max(min_height);

            // 3) Reserve paint area
            let (content_rect, _resp) = ui.allocate_exact_size(
                Vec2::new(available_width, content_height),
                Sense::hover(),
            );

            // 4) Paint layout tree (backgrounds + text)
            let painter = ui.painter_at(content_rect);
            let origin = content_rect.min;
            paint_layout_box(&layout_root, &painter, origin, &measurer, true, page);
        });
}

fn paint_line_boxes<'a>(
    painter: &Painter,
    origin: Pos2,
    lines: &[LineBox<'a>],
    measurer: &dyn TextMeasurer,
    page: &PageState,
) {
    for line in lines {
        for frag in &line.fragments {
            match &frag.kind {
                InlineFragment::Text { text, style } => {
                    let (cr, cg, cb, ca) = style.color;
                    let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);

                    let font_px = match style.font_size {
                        Length::Px(px) => px,
                    };
                    let font_id = FontId::proportional(font_px);

                    let pos = Pos2 {
                        x: origin.x + frag.rect.x,
                        y: origin.y + frag.rect.y,
                    };

                    painter.text(
                        pos,
                        Align2::LEFT_TOP,
                        text,
                        font_id,
                        text_color,
                    );
                }

                InlineFragment::Box { style, layout } => {
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
                            painter,
                            translated_origin,
                            measurer,
                            false, // do NOT skip inline-block children inside this subtree
                            page,
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

                InlineFragment::Replaced { style, kind, layout } => {
                    let rect = Rect::from_min_size(
                        Pos2 { x: origin.x + frag.rect.x, y: origin.y + frag.rect.y },
                        Vec2::new(frag.rect.width, frag.rect.height),
                    );

                    // Fill + stroke (placeholder look)
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

                    // Special case: <input type="text"> draws its value/placeholder inside the box
                    if matches!(kind, ReplacedKind::InputText) {
                        // Determine shown text: value first, else placeholder
                        let mut shown = String::new();

                        if let Some(lb) = layout {
                            let id = lb.node_id();
                            if let Some(v) = page.input_values.get(id) {
                                shown = v.to_string();
                            }
                            
                            if shown.is_empty() {
                                if let Some(ph) = get_attr(lb.node.node, "placeholder") {
                                    if !ph.trim().is_empty() {
                                        shown = ph.to_string();
                                    }
                                }
                            }
                        }

                        // Inner text area from padding (with sane minimums)
                        let bm = style.box_metrics;
                        let pad_l = bm.padding_left.max(4.0);
                        let pad_r = bm.padding_right.max(4.0);
                        let pad_t = bm.padding_top.max(2.0);

                        let text_x = rect.min.x + pad_l;
                        let text_y = rect.min.y + pad_t;
                        let available_text_w = (rect.width() - pad_l - pad_r).max(0.0);

                        let shown = truncate_to_fit(measurer, style, &shown, available_text_w);

                        // Paint in style color
                        let (cr, cg, cb, ca) = style.color;
                        let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);
                        let font_px = match style.font_size { Length::Px(px) => px };
                        let font_id = FontId::proportional(font_px);

                        painter.text(
                            Pos2 { x: text_x, y: text_y },
                            Align2::LEFT_TOP,
                            shown,
                            font_id,
                            text_color,
                        );

                        continue; // skip default label painting below
                    }

                    // Default centered label for other replaced elements
                    let mut label = match kind {
                        ReplacedKind::Img => "IMG".to_string(),
                        ReplacedKind::Button => "BUTTON".to_string(),
                        ReplacedKind::InputText => unreachable!("handled above"),
                    };

                    // If <img alt="...">, show alt text instead
                    if matches!(kind, ReplacedKind::Img) {
                        if let Some(lb) = layout {
                            if let Some(alt) = get_attr(lb.node.node, "alt") {
                                let alt = alt.trim();
                                if !alt.is_empty() {
                                    label = alt.to_string();
                                }
                            }
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
    painter: &Painter,
    origin: Pos2,
    measurer: &dyn TextMeasurer,
    skip_inline_block_children: bool,
    page: &PageState,
) {

    // 0) Do not paint non-rendering elements (head, style, script, etc.)
    if is_non_rendering_element(layout.node.node) {
        for child in &layout.children {
            paint_layout_box(child, painter, origin, measurer, skip_inline_block_children, page);
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

    // 1) List marker (for display:list-item), if any.
    //    This does not affect layout; it's purely visual.
    if matches!(layout.style.display, Display::ListItem) {
        paint_list_marker(layout, painter, origin, measurer);
    }

    // 2) Inline content
    paint_inline_content(layout, painter, origin, measurer, page);

    // 3) Recurse into children
    for child in &layout.children {
        if skip_inline_block_children && matches!(child.kind, BoxKind::InlineBlock) {
            // This inline-block will be painted via the inline formatting context.
            continue;
        }

        paint_layout_box(child, painter, origin, measurer, skip_inline_block_children, page);
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

    let font_px = match style.font_size {
        Length::Px(px) => px,
    };
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
fn paint_inline_content<'a>(
    layout: &LayoutBox<'a>,
    painter: &Painter,
    origin: Pos2,
    measurer: &dyn TextMeasurer,
    page: &PageState,
) {
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

    paint_line_boxes(painter, origin, &lines, measurer, page);
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
