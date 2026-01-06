mod dom;
mod index;
mod seed;

pub use dom::{InputControlType, input_control_type};
pub use index::FormControlIndex;
pub use seed::seed_input_state_from_dom;

#[cfg(test)]
mod tests;
