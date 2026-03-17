mod api;
mod coalescing;
mod determinism;
mod formatting;
mod helpers;
mod insertion_modes;
mod invariants;
mod perf;
mod recovery;
mod state_snapshot;
mod text_mode;

#[cfg(feature = "dom-snapshot")]
mod serialization;
