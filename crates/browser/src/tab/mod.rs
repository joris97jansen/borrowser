//! Tab-level orchestration and streaming state.
//!
//! Invariants:
//! - `nav_gen` is the navigation/request generation counter. All streaming
//!   events are gated through `is_current` so stale events from previous
//!   generations are ignored.
//! - Loading combines parser completion with pending stylesheet work. Network
//!   completion and intermediate publications do not establish parser success.
//!   Document failure stays latched until navigation replaces the document.
//! - Each navigation accepts one HTML response start, after redirects resolve.
//!   Duplicate starts preserve streaming state; late HTML network events cannot
//!   reopen a terminal parser lifecycle.
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
pub use self::ui::PageFrameStatus;
pub use dom_style::{inherited_color, page_background};
