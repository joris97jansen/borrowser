/// Default inline layout padding in CSS px.
pub(crate) const INLINE_PADDING: f32 = 4.0;

/// Per-call configuration for the inline layout engine.
#[derive(Clone, Copy, Debug)]
pub(crate) struct InlineLayoutOptions {
    pub(crate) padding: f32,
    pub(crate) preserve_leading_spaces: bool,
    pub(crate) preserve_empty_lines: bool,
    pub(crate) break_long_words: bool,
}

impl InlineLayoutOptions {
    /// HTML inline layout defaults used for normal DOM text layout.
    pub(crate) fn html_defaults() -> Self {
        Self {
            padding: INLINE_PADDING,
            preserve_leading_spaces: false,
            preserve_empty_lines: false,
            break_long_words: false,
        }
    }
}
