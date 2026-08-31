use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SyntheticTextMetricsV1 {
    #[default]
    SyntheticTextMetricsV1,
}

impl SyntheticTextMetricsV1 {
    pub const fn stable_label(self) -> &'static str {
        "synthetic-text-metrics-v1"
    }
}

impl layout::TextMeasurer for SyntheticTextMetricsV1 {
    fn measure(&self, text: &str, style: &css::ComputedStyle) -> f32 {
        let css::Length::Px(font_size) = style.font_size();
        text.chars().count() as f32 * font_size * 0.5
    }

    fn line_height(&self, style: &css::ComputedStyle) -> f32 {
        let css::Length::Px(font_size) = style.font_size();
        font_size * 1.2
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AvailableWidthCssPx(u32);

impl AvailableWidthCssPx {
    pub const MINIMUM: u32 = 1;
    pub const MAXIMUM: u32 = 16_777_216;

    pub fn try_new(value: u32) -> Option<Self> {
        (Self::MINIMUM..=Self::MAXIMUM)
            .contains(&value)
            .then_some(Self(value))
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RenderingExecutionVariantId {
    pub environment: SyntheticTextMetricsV1,
    pub available_width_css_px: AvailableWidthCssPx,
}

impl RenderingExecutionVariantId {
    pub const fn stable_environment_label(self) -> &'static str {
        self.environment.stable_label()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_metrics_v1_counts_every_unicode_scalar_without_special_cases() {
        let style = css::ComputedStyle::initial();
        let metrics = SyntheticTextMetricsV1::SyntheticTextMetricsV1;
        assert_eq!(
            layout::TextMeasurer::measure(&metrics, " a\t\n\u{a0}e\u{301}\u{200b}", &style),
            64.0
        );
        assert_eq!(layout::TextMeasurer::measure(&metrics, "", &style), 0.0);
        assert_eq!(layout::TextMeasurer::line_height(&metrics, &style), 19.2);
    }

    #[test]
    fn available_width_has_exact_v1_boundaries() {
        assert!(AvailableWidthCssPx::try_new(0).is_none());
        assert_eq!(AvailableWidthCssPx::try_new(1).unwrap().get(), 1);
        assert_eq!(
            AvailableWidthCssPx::try_new(16_777_216).unwrap().get(),
            16_777_216
        );
        assert!(AvailableWidthCssPx::try_new(16_777_217).is_none());
    }
}
