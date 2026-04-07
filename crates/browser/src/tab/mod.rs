//! Tab-level orchestration and streaming state.
//!
//! Invariants:
//! - `nav_gen` is the navigation/request generation counter. All streaming
//!   events are gated through `is_current` so stale events from previous
//!   generations are ignored.
//! - `PageState::pending_count()` is the single source of truth for whether
//!   the tab is still loading; `loading` is derived from it and only tracks
//!   user-visible state.
//! - Each `Tab` owns its `PageState`, `ResourceManager`, and `DocumentInputState`.
//!   There is no cross-tab sharing of DOM, resources, or input state; any
//!   shared work must go through the bus/runtime layers.

mod css;
mod discovery;
mod dom_style;
mod events;
mod html;
mod image;
mod nav;
mod state;
mod status;
#[cfg(test)]
mod tests;
mod ui;

pub use self::state::Tab;
pub use dom_style::{inherited_color, page_background};
