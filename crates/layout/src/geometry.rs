use css::ComputedStyle;

/// A rectangle in CSS px units (we'll treat everything as px for now).
#[derive(Clone, Copy, Debug)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rectangle {
    pub fn approx_eq(self, other: Self, eps: f32) -> bool {
        (self.x - other.x).abs() <= eps
            && (self.y - other.y).abs() <= eps
            && (self.width - other.width).abs() <= eps
            && (self.height - other.height).abs() <= eps
    }
}

impl PartialEq for Rectangle {
    fn eq(&self, other: &Self) -> bool {
        const EPS: f32 = 1e-6;
        (*self).approx_eq(*other, EPS)
    }
}

/// The inner "content box" of a layout box: border box minus border and padding.
/// We expose it via small helpers so that all code computes content
/// geometry in a single, consistent way.
pub fn content_x_and_width(style: &ComputedStyle, border_x: f32, border_width: f32) -> (f32, f32) {
    let bm = style.box_metrics();

    let content_x = border_x + bm.border_left + bm.padding_left;
    let content_width =
        (border_width - bm.border_left - bm.padding_left - bm.padding_right - bm.border_right)
            .max(0.0);

    debug_assert!(
        content_width >= 0.0,
        "content_x_and_width produced negative width: border_width={border_width}, borders=({}, {}), paddings=({}, {})",
        bm.border_left,
        bm.border_right,
        bm.padding_left,
        bm.padding_right,
    );

    (content_x, content_width)
}

/// Vertical position of the content box top (border box top + border-top + padding-top).
pub fn content_y(style: &ComputedStyle, border_y: f32) -> f32 {
    let bm = style.box_metrics();
    border_y + bm.border_top + bm.padding_top
}

/// Height of the content box (border box height minus vertical border and padding).
pub fn content_height(style: &ComputedStyle, border_height: f32) -> f32 {
    let bm = style.box_metrics();
    let content_height =
        (border_height - bm.border_top - bm.padding_top - bm.padding_bottom - bm.border_bottom)
            .max(0.0);

    debug_assert!(
        content_height >= 0.0,
        "content_height produced negative height: border_height={border_height}, borders=({}, {}), paddings=({}, {})",
        bm.border_top,
        bm.border_bottom,
        bm.padding_top,
        bm.padding_bottom,
    );

    content_height
}
