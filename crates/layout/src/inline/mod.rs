mod breaker;
mod button;
mod dom_attrs;
mod engine;
mod metrics;
mod options;
mod refine;
mod replaced;
mod textarea;
mod tokens;
mod types;

#[cfg(test)]
mod tests;

pub use button::button_label_from_layout;
pub(crate) use dom_attrs::get_attr;
pub use engine::layout_inline_for_paint;
pub use refine::refine_layout_with_inline;
pub use textarea::layout_textarea_value_for_paint;
pub use types::{InlineActionKind, InlineFragment, LineBox, LineFragment};
