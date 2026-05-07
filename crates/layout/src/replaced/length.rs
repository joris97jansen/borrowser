use css::{Length, LengthPercentage};

pub fn px_opt(value: Option<LengthPercentage>) -> Option<f32> {
    match value {
        Some(LengthPercentage::Length(Length::Px(px))) => Some(px),
        _ => None,
    }
}
