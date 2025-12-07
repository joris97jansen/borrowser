use crate::values::{
    Length,
    Display,
    parse_color,
    parse_length,
    parse_display,
};

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
}

impl ComputedStyle {
    pub fn initial() -> Self {
        ComputedStyle {
            color: (0, 0, 0, 255),              // black
            background_color: (0, 0, 0, 0),     // transparent
            font_size: Length::Px(16.0),        // "16px" default
            box_metrics: BoxMetrics::zero(),    // zero margins/padding
            display: Display::Block,            // default to Block for now
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
    pub node: &'a html::Node,
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

    // 3. Apply specified declarations (override inherited/initial)
    for (name, value) in specified {
        let name = name.as_str();
        let value = value.as_str();

        match name {
            "color" => {
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
            
            _ => {
                // unsupported property → ignored (CSS spec: unknown declarations are ignored)
            }
        }
    }

    result
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
    use html::Node;

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
                style: base,
                children: styled_children,
            }
        }

        Node::Element { style, children, .. } => {
            let computed = compute_style(style, parent_style);

            let mut styled_children = Vec::new();
            for child in children {
                styled_children.push(build_style_tree(child, Some(&computed)));
            }

            StyledNode {
                node: root,
                style: computed,
                children: styled_children,
            }
        }

        Node::Text { .. } | Node::Comment { .. } => {
            // Inherit everything from parent
            let inherited = parent_style.copied().unwrap_or_else(ComputedStyle::initial);

            StyledNode {
                node: root,
                style: inherited,
                children: Vec::new(),
            }
        }
    }
}