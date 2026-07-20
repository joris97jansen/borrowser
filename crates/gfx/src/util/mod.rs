mod text;
mod url;

pub(crate) use text::{
    clamp_to_char_boundary, ellipsize_to_width, input_text_padding, truncate_to_fit,
    wrap_text_to_width,
};
pub(crate) use url::resolve_relative_url;
