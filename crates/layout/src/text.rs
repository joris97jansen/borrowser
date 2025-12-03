use css::ComputedStyle;

/// Layout / inline layout can depend on this without knowing about egui, wgpu, etc.
pub trait TextMeasurer {
    /// Return the width of `text` in CSS px when rendered with `style`.
    fn measure(&self, text: &str, style: &ComputedStyle) -> f32;

    /// Return the line-height in CSS px for the given `style`.
    /// (For now, you can implement this as 1.2 * font-size on the egui side.)
    fn line_height(&self, style: &ComputedStyle) -> f32;
}
