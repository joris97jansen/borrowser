mod index;
mod input_type;
mod seed;

pub use index::FormControlIndex;
pub use input_type::{InputControlType, input_control_type};
pub use seed::seed_input_state_from_dom;

#[cfg(test)]
mod tests;
