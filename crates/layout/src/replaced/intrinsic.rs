#[derive(Clone, Copy, Debug, Default)]
pub struct IntrinsicSize {
    pub width: Option<f32>,   // px
    pub height: Option<f32>,  // px
    /// width / height
    pub ratio: Option<f32>,
}

impl IntrinsicSize {
    pub fn from_w_h(width: Option<f32>, height: Option<f32>) -> Self {
        let ratio = match (width, height) {
            (Some(w), Some(h)) if h > 0.0 => Some(w / h),
            _ => None,
        };
        Self { width, height, ratio }
    }
}
