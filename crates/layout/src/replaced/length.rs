use css::Length;

pub fn px_opt(len: Option<Length>) -> Option<f32> {
    match len {
        Some(Length::Px(px)) => Some(px),
        _ => None,
    }
}
