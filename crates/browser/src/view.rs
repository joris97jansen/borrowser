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
    LayoutBox
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

const INLINE_PADDING: f32 = 4.0;

pub enum NavigationAction {
    None,
    Back,
    Forward,
    Refresh,
    Navigate(String),
}

// One fragment of text within a line (later this can be per <span>, <a>, etc.)
struct LineFragment<'a> {
    text: String,
    style: &'a css::ComputedStyle,
    rect: Rect,
}

// One line box: a horizontal slice of inline content.
struct LineBox<'a> {
    fragments: Vec<LineFragment<'a>>,
    rect: Rect,
}

// A logical piece of inline content with a single style
struct InlineRun<'a> {
    text: String,
    style: &'a css::ComputedStyle,
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
            refine_layout_with_inline(ui.ctx(), &mut layout_root);

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

fn layout_inline_runs<'a>(
    ctx: &Context,
    rect: Rect,
    block_style: &'a ComputedStyle,
    runs: &[InlineRun<'a>],
) -> Vec<LineBox<'a>> {
    let padding = INLINE_PADDING;
    let available_height = rect.height() - 2.0 * padding;

    // Use the block's font-size to derive a base line height.
    let mut block_font_px = match block_style.font_size {
        Length::Px(px) => px,
    };
    let mut line_height = block_font_px * 1.2;

    if line_height > available_height && available_height > 0.0 {
        block_font_px = (available_height / 1.2).max(8.0);
        line_height = block_font_px * 1.2;
    }

    let mut lines: Vec<LineBox<'a>> = Vec::new();
    let mut line_fragments: Vec<LineFragment<'a>> = Vec::new();

    let line_start_x = rect.min.x + padding;
    let mut cursor_x = line_start_x;
    let mut cursor_y = rect.min.y + padding;

    let max_x = rect.max.x - padding;
    let bottom_limit = rect.min.y + padding + available_height;

    let mut is_first_in_line = true;

    for run in runs {
        let (cr, cg, cb, ca) = run.style.color;
        let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);
        let font_px = match run.style.font_size {
            Length::Px(px) => px,
        };
        let font_id = FontId::proportional(font_px);

        for word in run.text.split_whitespace() {
            let text_piece = if is_first_in_line {
                word.to_string()
            } else {
                format!(" {}", word)
            };

            let word_width = ctx.fonts(|f| {
                f.layout_no_wrap(
                    text_piece.clone(),
                    font_id.clone(),
                    text_color,
                )
                .rect
                .width()
            });

            let fits = cursor_x + word_width <= max_x;

            if !fits && !is_first_in_line {
                // Close current line
                if !line_fragments.is_empty() {
                    let line_width = cursor_x - line_start_x;
                    let line_rect = Rect::from_min_size(
                        Pos2::new(line_start_x, cursor_y),
                        Vec2::new(line_width, line_height),
                    );

                    lines.push(LineBox {
                        rect: line_rect,
                        fragments: std::mem::take(&mut line_fragments),
                    });
                }

                cursor_y += line_height;
                if cursor_y + line_height > bottom_limit {
                    // No more vertical space in this block
                    return lines;
                }

                cursor_x = line_start_x;
                is_first_in_line = true;

                // Re-measure the same word without leading space at line start
                let text_piece = word.to_string();
                let word_width = ctx.fonts(|f| {
                    f.layout_no_wrap(
                        text_piece.clone(),
                        font_id.clone(),
                        text_color,
                    )
                    .rect
                    .width()
                });

                let frag_rect = Rect::from_min_size(
                    Pos2::new(cursor_x, cursor_y),
                    Vec2::new(word_width, line_height),
                );

                line_fragments.push(LineFragment {
                    text: text_piece,
                    style: run.style,
                    rect: frag_rect,
                });

                cursor_x += word_width;
                is_first_in_line = false;
            } else {
                // Fits (or first token in line, even if too long)
                let frag_rect = Rect::from_min_size(
                    Pos2::new(cursor_x, cursor_y),
                    Vec2::new(word_width, line_height),
                );

                line_fragments.push(LineFragment {
                    text: text_piece,
                    style: run.style,
                    rect: frag_rect,
                });

                cursor_x += word_width;
                is_first_in_line = false;
            }
        }
    }

    // Flush the last line
    if !line_fragments.is_empty() && cursor_y + line_height <= bottom_limit {
        let line_width = cursor_x - line_start_x;
        let line_rect = Rect::from_min_size(
            Pos2::new(line_start_x, cursor_y),
            Vec2::new(line_width, line_height),
        );

        lines.push(LineBox {
            rect: line_rect,
            fragments: line_fragments,
        });
    }

    lines
}

fn paint_line_boxes<'a>(painter: &Painter, lines: &[LineBox<'a>]) {
    for line in lines {
        for frag in &line.fragments {
            let (cr, cg, cb, ca) = frag.style.color;
            let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);

            let font_px = match frag.style.font_size {
                Length::Px(px) => px,
            };
            let font_id = FontId::proportional(font_px);

            painter.text(
                frag.rect.min,
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
                let lines = layout_inline_runs(painter.ctx(), rect, &layout.style, &runs);
                paint_line_boxes(painter, &lines);
            }
        }
    }

    // 2) Recurse into children
    for child in &layout.children {
        paint_layout_box(child, painter, origin);
    }
}

fn collect_inline_runs_for_block<'a>(block: &'a StyledNode<'a>) -> Vec<InlineRun<'a>> {
    let mut runs = Vec::new();

    match block.node {
        Node::Element { .. } | Node::Document { .. } => {
            for child in &block.children {
                collect_inline_runs_desc(child, &mut runs);
            }
        }
        _ => {
            // For text/comment root we do nothing; blocks are Elements/Document.
        }
    }

    runs
}

fn collect_inline_runs_desc<'a>(styled: &'a StyledNode<'a>, out: &mut Vec<InlineRun<'a>>) {
    match styled.node {
        Node::Text { text } => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                out.push(InlineRun {
                    // Keep original contents; we’ll handle spaces in layout
                    text: text.clone(),
                    style: &styled.style,
                });
            }
        }

        Node::Element { name, .. } => {
            if is_inline_element_name(name) {
                // Inline element → dive into children; they inherit/override style.
                for child in &styled.children {
                    collect_inline_runs_desc(child, out);
                }
            } else {
                // Block element → DO NOT descend.
                // This subtree will be handled by its own LayoutBox in a separate paint pass.
            }
        }

        Node::Document { .. } | Node::Comment { .. } => {
            for child in &styled.children {
                collect_inline_runs_desc(child, out);
            }
        }
    }
}

fn refine_layout_with_inline<'a>(
    ctx: &Context,
    layout_root: &mut LayoutBox<'a>,
) {
    let x = layout_root.rect.x;
    let y = layout_root.rect.y;
    let width = layout_root.rect.width;

    let new_height = recompute_block_heights(ctx, layout_root, x, y, width);
    layout_root.rect.height = new_height;
}

fn recompute_block_heights<'a>(
    ctx: &Context,
    node: &mut LayoutBox<'a>,
    x: f32,
    y: f32,
    width: f32,
) -> f32 {
    // Position & width are authoritative here
    node.rect.x = x;
    node.rect.y = y;
    node.rect.width = width;

    // Non-rendering elements: pure containers
    if is_non_rendering_element(node.node.node) {
        let mut cursor_y = y;
        for child in &mut node.children {
            let h = recompute_block_heights(ctx, child, x, cursor_y, width);
            cursor_y += h;
        }
        let height = cursor_y - y;
        node.rect.height = height;
        return height;
    }

    match node.node.node {
        Node::Document { .. } => {
            let mut cursor_y = y;
            for child in &mut node.children {
                let h = recompute_block_heights(ctx, child, x, cursor_y, width);
                cursor_y += h;
            }
            let height = cursor_y - y;
            node.rect.height = height;
            height
        }

        Node::Element { name, .. } => {
            // <html> acts as pure container (no own row)
            if name.eq_ignore_ascii_case("html") {
                let mut cursor_y = y;
                for child in &mut node.children {
                    let h = recompute_block_heights(ctx, child, x, cursor_y, width);
                    cursor_y += h;
                }
                let height = cursor_y - y;
                node.rect.height = height;
                return height;
            }

            // Inline elements do not generate a separate block height here.
            // Their text is handled by the nearest block ancestor via inline layout.
            if is_inline_element_name(name) {
                node.rect.height = 0.0;
                return 0.0;
            }

            // --- Block-level element: inline content + block children ---

            // 1) Compute inline height using line boxes for this block
            let mut inline_height = 0.0;
            let runs = collect_inline_runs_for_block(node.node);

            if !runs.is_empty() {
                // Use a "tall" rect so inline layout is not artificially clipped
                let huge_height = 1_000_000.0;
                let block_rect = Rect::from_min_size(
                    Pos2::new(x, y),
                    Vec2::new(width, huge_height),
                );

                let lines = layout_inline_runs(ctx, block_rect, node.style, &runs);
                if let Some(first) = lines.first() {
                    if let Some(last) = lines.last() {
                        let first_top = first.rect.min.y;
                        let last_bottom = last.rect.min.y + last.rect.height();
                        // Total from block top to last line bottom plus bottom padding
                        inline_height = (last_bottom - y) + INLINE_PADDING;
                    }
                }
            }

            // Fallback: at least one line-height even if no text
            if inline_height <= 0.0 {
                let font_px = match node.style.font_size {
                    Length::Px(px) => px,
                };
                inline_height = font_px * 1.2;
            }

            // 2) Block children start below the inline content
            let mut cursor_y = y + inline_height;
            for child in &mut node.children {
                let h = recompute_block_heights(ctx, child, x, cursor_y, width);
                cursor_y += h;
            }

            let children_height = cursor_y - (y + inline_height);
            let total_height = inline_height + children_height;

            node.rect.height = total_height;
            total_height
        }

        // Text / Comment nodes: no own block height
        Node::Text { .. } | Node::Comment { .. } => {
            node.rect.height = 0.0;
            0.0
        }
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

fn is_inline_element_name(name: &str) -> bool {
    // This is a *starting* set; we can expand as needed.
    name.eq_ignore_ascii_case("span")
        || name.eq_ignore_ascii_case("a")
        || name.eq_ignore_ascii_case("em")
        || name.eq_ignore_ascii_case("strong")
        || name.eq_ignore_ascii_case("b")
        || name.eq_ignore_ascii_case("i")
        || name.eq_ignore_ascii_case("u")
        || name.eq_ignore_ascii_case("small")
        || name.eq_ignore_ascii_case("big")
        || name.eq_ignore_ascii_case("code")
}