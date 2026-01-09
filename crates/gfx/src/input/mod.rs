mod action;
mod interaction;
mod route;
mod store;

pub use crate::textarea::{TextareaCachedLine, TextareaCachedTextFragment, TextareaLayoutCache};
pub use action::PageAction;
pub use interaction::{ActiveTarget, InputDragState, InteractionState};
pub use route::FormControlHandler;
pub(crate) use route::{FrameInputCtx, route_frame_input};
pub use store::{InputValueStore, SelectionRange, from_input_id, to_input_id};

// Re-export the core InputStore trait for routing abstraction
pub use input_core::InputStore;
