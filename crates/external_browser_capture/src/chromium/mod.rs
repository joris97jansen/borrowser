//! Chromium-only protocol adapter. No protocol identifiers enter neutral provenance.
#![cfg_attr(
    not(target_os = "linux"),
    allow(
        dead_code,
        reason = "The real launch adapter is Linux-only; transport/state tests still compile on other hosts."
    )
)]
pub(crate) mod delivery;
mod events;
mod inspection;
pub mod protocol;
pub(crate) mod session;
