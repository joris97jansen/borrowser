#![cfg(all(feature = "html5", feature = "dom-snapshot"))]

#[path = "wpt_manifest.rs"]
mod wpt_manifest;

#[path = "diff_html5/mod.rs"]
mod suite;
