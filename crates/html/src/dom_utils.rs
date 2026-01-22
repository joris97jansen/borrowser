//! Compatibility facade for DOM traversal, collection, and debug helpers.
//! Prefer using the dedicated modules (`collect`, `traverse`, `debug`) directly.

pub use crate::collect::{
    collect_img_srcs, collect_style_texts, collect_stylesheet_hrefs, collect_visible_text,
    collect_visible_text_string,
};
pub use crate::debug::{first_styles, outline_from_dom};
pub use crate::traverse::{find_node_by_id, is_non_rendering_element};
