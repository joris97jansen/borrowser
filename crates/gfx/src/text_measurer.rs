use std::cell::RefCell;
use std::collections::HashMap;

use css::{ComputedStyle, Length};
use egui::{Color32, Context, FontId};
use layout::TextMeasurer;

/// `egui`-backed adapter for measuring text during layout.
pub struct EguiTextMeasurer {
    ctx: Context,
    space_width_cache: RefCell<HashMap<u32, f32>>,
}

impl EguiTextMeasurer {
    pub fn new(ctx: &Context) -> Self {
        Self {
            ctx: ctx.clone(),
            space_width_cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn context(&self) -> &Context {
        &self.ctx
    }
}

impl TextMeasurer for EguiTextMeasurer {
    fn measure(&self, text: &str, style: &ComputedStyle) -> f32 {
        let Length::Px(font_px) = style.font_size;
        let font_id = FontId::proportional(font_px);

        if text == " " {
            // `Color32` does not affect text metrics; cache width per font size.
            let key = font_px.round().max(0.0) as u32;
            if let Some(w) = self.space_width_cache.borrow().get(&key).copied() {
                return w;
            }

            let (r, g, b, a) = style.color;
            let color = Color32::from_rgba_unmultiplied(r, g, b, a);

            // 1) NBSP is the most stable in egui
            let nbsp = "\u{00A0}";
            let w_nbsp = self.ctx.fonts(|f| {
                f.layout_no_wrap(nbsp.to_owned(), font_id.clone(), color)
                    .rect
                    .width()
            });

            let w = if w_nbsp.is_finite() && w_nbsp > 0.0 {
                w_nbsp
            } else {
                // 2) Difference method as fallback (use chars with low kerning risk)
                let w_with = self.ctx.fonts(|f| {
                    f.layout_no_wrap(format!("x{nbsp}x"), font_id.clone(), color)
                        .rect
                        .width()
                });
                let w_without = self.ctx.fonts(|f| {
                    f.layout_no_wrap("xx".to_owned(), font_id.clone(), color)
                        .rect
                        .width()
                });
                let w = (w_with - w_without).max(0.0);

                if w.is_finite() && w > 0.0 {
                    w
                } else {
                    // 3) Absolute fallback
                    (font_px * 0.33).max(1.0)
                }
            };

            self.space_width_cache.borrow_mut().insert(key, w);
            return w;
        }

        let (r, g, b, a) = style.color;
        let color = Color32::from_rgba_unmultiplied(r, g, b, a);

        self.ctx.fonts(|f| {
            f.layout_no_wrap(text.to_owned(), font_id, color)
                .rect
                .width()
        })
    }

    fn line_height(&self, style: &ComputedStyle) -> f32 {
        let Length::Px(px) = style.font_size;
        px * 1.2
    }
}
