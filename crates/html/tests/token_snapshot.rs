#![cfg(feature = "html5")]

#[path = "common/mod.rs"]
mod support;
#[path = "common/token_snapshot.rs"]
mod token_snapshot_impl;

pub use token_snapshot_impl::*;
