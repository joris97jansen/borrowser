#![cfg(all(feature = "html5", feature = "test-harness"))]

//! Core-v0 contract inventory scaffold.
//!
//! Acceptance inventory metadata lives in declarative tables. Live drift/evidence
//! guards live in focused test modules so CI-backed assertions stay visible.

#[path = "html5_core_v0_acceptance/support.rs"]
mod support;

#[path = "html5_core_v0_acceptance/inventory.rs"]
mod inventory;

#[path = "html5_core_v0_acceptance/live_guards.rs"]
mod live_guards;
