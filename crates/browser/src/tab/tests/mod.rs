mod css_provenance;
mod css_resources;
mod document_lifecycle;
mod dom_patches;
mod fetch_requests;
mod load_observation;
mod navigation;
mod network;
#[cfg(feature = "conformance-observation")]
mod observation;
mod rendering;
mod style_cache;
mod stylesheets;
mod support;

mod document_addressing;
mod html_resources;
mod resource_associations;

#[cfg(feature = "conformance-observation")]
mod addressing_observation;

mod ai3_responses;
mod ai5_failures;
mod redirects;
mod response_metadata;

mod document_response;

mod parsed_document_observation;
mod stylesheet_discovery;
