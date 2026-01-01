use crate::values::{Display, Length, parse_color, parse_display, parse_length};

use html::{Id, Node};

#[derive(Clone, Copy, Debug)]
pub struct BoxMetrics {
    // Margins in CSS px
    pub margin_top: f32,
    pub margin_right: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,

    // Padding in CSS px
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,
}

impl BoxMetrics {
    pub fn zero() -> Self {
        BoxMetrics {
            margin_top: 0.0,
            margin_right: 0.0,
            margin_bottom: 0.0,
            margin_left: 0.0,
            padding_top: 0.0,
            padding_right: 0.0,
            padding_bottom: 0.0,
            padding_left: 0.0,
        }
    }
}

#[derive(Clone, Debug, Copy)]
pub struct ComputedStyle {
    /// Inherited by default. Initial: black.
    pub color: (u8, u8, u8, u8),

    /// Not inherited. Initial: transparent.
    pub background_color: (u8, u8, u8, u8),

    /// Inherited. We'll treat this as `px` only for now.
    /// Initial: 16px.
    pub font_size: Length,

    pub box_metrics: BoxMetrics,

    /// CSS `display` value.
    /// Not inherited in CSS. For now we default to Block; later we’ll
    /// override this with per-element defaults.
    pub display: Display,

    /// Optional width property. Not inherited. For now we treat this
    /// as `px` only when specified.
    pub width: Option<Length>,
    pub height: Option<Length>,

    pub min_width: Option<Length>,
    pub max_width: Option<Length>,
}

impl ComputedStyle {
    pub fn initial() -> Self {
        ComputedStyle {
            color: (0, 0, 0, 255),           // black
            background_color: (0, 0, 0, 0),  // transparent
            font_size: Length::Px(16.0),     // "16px" default
            box_metrics: BoxMetrics::zero(), // zero margins/padding
            display: Display::Block,         // default to Block for now
            width: None,                     // auto
            height: None,                    // auto
            min_width: None,                 // none
            max_width: None,                 // none
        }
    }
}

/// A node in the style tree: pairs a DOM node with its computed style
/// and the styled children.
///
/// This forms a parallel tree to the DOM:
/// - Same shape (for elements we care about)
/// - Holds computed, inherited CSS values
pub struct StyledNode<'a> {
    pub node: &'a Node,
    pub node_id: Id,
    pub style: ComputedStyle,
    pub children: Vec<StyledNode<'a>>,
}

/// Compute the final, inherited style for an element, given:
/// - its specified declarations (Node.style)
/// - an optional parent computed style.
///
/// Assumptions:
/// - `specified` already reflects cascade (author + inline etc.)
/// - property names are already lowercase (from `parse_declarations`).
pub fn compute_style(
    tag_name: Option<&str>,
    specified: &[(String, String)],
    parent: Option<&ComputedStyle>,
) -> ComputedStyle {
    // 1. Start from initial values
    let mut result = ComputedStyle::initial();

    // 2. Apply inheritance (per property)
    if let Some(p) = parent {
        // inherited:
        result.color = p.color;
        result.font_size = p.font_size;

        // NOT inherited:
        // result.background_color stays as initial (transparent)
    }

    let mut has_color_decl = false;

    // 3. Apply specified declarations (override inherited/initial)
    for (name, value) in specified {
        let name = name.as_str();
        let value = value.as_str();

        match name {
            "color" => {
                has_color_decl = true;
                if let Some(rgba) = parse_color(value) {
                    result.color = rgba;
                }
            }
            "background-color" => {
                if let Some(rgba) = parse_color(value) {
                    result.background_color = rgba;
                }
            }
            "font-size" => {
                if let Some(len) = parse_length(value) {
                    result.font_size = len;
                }
            }

            // --- Margins (non-inherited, px only) ---
            "margin-top" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.margin_top = px;
                }
            }
            "margin-right" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.margin_right = px;
                }
            }
            "margin-bottom" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.margin_bottom = px;
                }
            }
            "margin-left" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.margin_left = px;
                }
            }

            // --- Padding (non-inherited, px only) ---
            "padding-top" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.padding_top = px;
                }
            }
            "padding-right" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.padding_right = px;
                }
            }
            "padding-bottom" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.padding_bottom = px;
                }
            }
            "padding-left" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.padding_left = px;
                }
            }

            "display" => {
                if let Some(d) = parse_display(value) {
                    result.display = d;
                }
                // unknown values: parse_display returns None → silently ignored
            }
            "width" => {
                let v = value.trim().to_ascii_lowercase();
                if v == "auto" {
                    result.width = None;
                } else if let Some(px) = parse_px(&v).filter(|px| *px >= 0.0) {
                    result.width = Some(Length::Px(px));
                }
            }
            "height" => {
                let v = value.trim().to_ascii_lowercase();
                if v == "auto" {
                    result.height = None;
                } else if let Some(px) = parse_px(&v).filter(|px| *px >= 0.0) {
                    result.height = Some(Length::Px(px));
                }
            }
            "min-width" => {
                let v = value.trim().to_ascii_lowercase();
                if v == "auto" {
                    result.min_width = None;
                } else if let Some(px) = parse_px(&v).filter(|px| *px >= 0.0) {
                    result.min_width = Some(Length::Px(px));
                }
            }

            "max-width" => {
                let v = value.trim().to_ascii_lowercase();
                if v == "auto" {
                    result.max_width = None;
                } else if let Some(px) = parse_px(&v).filter(|px| *px >= 0.0) {
                    result.max_width = Some(Length::Px(px));
                }
            }
            _ => {
                // unsupported property → ignored (CSS spec: unknown declarations are ignored)
            }
        }
    }

    if let Some(tag) = tag_name {
        if tag.eq_ignore_ascii_case("a") && !has_color_decl {
            // UA-ish default link blue
            result.color = (0, 0, 238, 255);
        }
        if tag.eq_ignore_ascii_case("button") {
            // UA-ish paint defaults (safe even if author doesn't style it)
            if result.background_color.3 == 0 {
                result.background_color = (233, 233, 233, 255);
            }

            // UA-ish padding floors (author padding still wins because we only raise minimums)
            result.box_metrics.padding_left = result.box_metrics.padding_left.max(8.0);
            result.box_metrics.padding_right = result.box_metrics.padding_right.max(8.0);
            result.box_metrics.padding_top = result.box_metrics.padding_top.max(4.0);
            result.box_metrics.padding_bottom = result.box_metrics.padding_bottom.max(4.0);
        }
    };

    result
}

fn default_display_for(tag: &str) -> Display {
    // Very small subset for now. We can extend over time.
    // Roughly follows HTML default display types.
    if tag.eq_ignore_ascii_case("span")
        || tag.eq_ignore_ascii_case("a")
        || tag.eq_ignore_ascii_case("em")
        || tag.eq_ignore_ascii_case("strong")
        || tag.eq_ignore_ascii_case("b")
        || tag.eq_ignore_ascii_case("i")
        || tag.eq_ignore_ascii_case("u")
        || tag.eq_ignore_ascii_case("small")
        || tag.eq_ignore_ascii_case("big")
        || tag.eq_ignore_ascii_case("code")
        || tag.eq_ignore_ascii_case("img")
        || tag.eq_ignore_ascii_case("input")
    {
        return Display::Inline;
    }

    // List items are special: they default to list-item
    if tag.eq_ignore_ascii_case("li") {
        return Display::ListItem;
    }

    if tag.eq_ignore_ascii_case("button") {
        return Display::InlineBlock;
    }
    if tag.eq_ignore_ascii_case("textarea") {
        return Display::InlineBlock;
    }
    // Everything else we treat as block for now
    Display::Block
}

fn parse_px(value: &str) -> Option<f32> {
    let v = value.trim();
    if let Some(stripped) = v.strip_suffix("px") {
        stripped.trim().parse::<f32>().ok()
    } else {
        None
    }
}

/// Build a style tree from a DOM root.
/// - `root` is the DOM node (usually the document root)
/// - `parent_style` is the inherited style, if any
///
/// We:
/// - Create a `StyledNode` for Document + Element nodes
/// - Skip Text/Comment nodes for now (can be added later for inline layout)
pub fn build_style_tree<'a>(
    root: &'a html::Node,
    parent_style: Option<&ComputedStyle>,
) -> StyledNode<'a> {
    match root {
        Node::Document { children, .. } => {
            let base = parent_style.copied().unwrap_or_else(ComputedStyle::initial);

            let mut styled_children = Vec::new();
            for child in children {
                // Include *all* node types so we see Text nodes here too
                styled_children.push(build_style_tree(child, Some(&base)));
            }

            StyledNode {
                node: root,
                node_id: root.id(),
                style: base,
                children: styled_children,
            }
        }

        Node::Element {
            name,
            style,
            children,
            ..
        } => {
            // 1) Check if there is an explicit `display:` declaration
            let has_display_decl = style
                .iter()
                .any(|(prop, _)| prop.eq_ignore_ascii_case("display"));

            // 2) Compute the base style (inherits, applies declarations, etc.)
            let mut computed = compute_style(Some(name), style, parent_style);

            // 3) If no explicit `display:` was specified, apply a per-element default
            if !has_display_decl {
                computed.display = default_display_for(name);
            }

            // 4) Recurse into children with this as the parent computed style
            let mut styled_children = Vec::new();
            for child in children {
                styled_children.push(build_style_tree(child, Some(&computed)));
            }

            StyledNode {
                node: root,
                node_id: root.id(),
                style: computed,
                children: styled_children,
            }
        }

        Node::Text { .. } | Node::Comment { .. } => {
            // Inherit everything from parent
            let inherited = parent_style.copied().unwrap_or_else(ComputedStyle::initial);

            StyledNode {
                node: root,
                node_id: root.id(),
                style: inherited,
                children: Vec::new(),
            }
        }
    }
}
