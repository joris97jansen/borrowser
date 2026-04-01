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

use crate::dom_store::DomStore;
use crate::input_state::DocumentInputState;
use crate::page::PageState;
use crate::resources::ResourceManager;
use app_api::RepaintHandle;
use bus::{CoreCommand, CoreEvent};
use std::collections::HashMap;
use std::sync::mpsc;

use html::Node;

use core_types::{
    DomHandle, NetworkErrorKind, NetworkResponseInfo, RequestId, ResourceKind, TabId,
};

mod discovery;
mod dom_style;
mod nav;
mod ui;

#[derive(Clone, Debug, Default)]
struct DocumentLoadState {
    response: Option<NetworkResponseInfo>,
    bytes_received: usize,
}

#[derive(Clone, Debug)]
struct StylesheetLoadState {
    response: NetworkResponseInfo,
    accept_body: bool,
}

pub struct Tab {
    pub tab_id: TabId,

    pub url: String,
    pub history: Vec<String>,
    pub history_index: usize,
    pub nav_gen: RequestId,

    loading: bool,
    last_status: Option<String>,
    document_load: DocumentLoadState,
    stylesheet_loads: HashMap<String, StylesheetLoadState>,

    page: PageState,
    resources: ResourceManager,
    repaint: Option<RepaintHandle>,
    cmd_tx: Option<mpsc::Sender<CoreCommand>>,
    document_input: DocumentInputState,
    dom_store: DomStore,
    dom_handle: Option<DomHandle>,
}

impl Tab {
    pub fn new(tab_id: TabId) -> Self {
        Self {
            tab_id,
            url: String::new(),
            history: Vec::new(),
            history_index: 0,
            nav_gen: 0,
            loading: false,
            last_status: None,
            document_load: DocumentLoadState::default(),
            stylesheet_loads: HashMap::new(),
            page: PageState::new(),
            resources: ResourceManager::new(),
            repaint: None,
            cmd_tx: None,
            document_input: DocumentInputState::default(),
            dom_store: DomStore::new(),
            dom_handle: None,
        }
    }

    // -- Setup Methods ---
    pub fn set_bus_sender(&mut self, tx: mpsc::Sender<CoreCommand>) {
        self.cmd_tx = Some(tx);
    }

    pub fn set_repaint_handle(&mut self, h: RepaintHandle) {
        self.repaint = Some(h);
    }

    // -- Event Handling ---
    pub fn on_core_event(&mut self, evt: CoreEvent) {
        match evt {
            // HTML networking → parser aansturen
            CoreEvent::NetworkStart {
                tab_id,
                request_id,
                kind: ResourceKind::Html,
                response,
            } if self.is_current(tab_id, request_id) => {
                self.on_html_network_start(response, request_id);
            }

            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                kind: ResourceKind::Html,
                url: _,
                bytes,
                ..
            } if self.is_current(tab_id, request_id) => {
                self.on_html_network_chunk(bytes, request_id);
            }

            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                kind: ResourceKind::Html,
                response,
                bytes_received,
            } if self.is_current(tab_id, request_id) => {
                self.on_html_network_done(response, bytes_received, request_id);
            }

            CoreEvent::NetworkError {
                tab_id,
                request_id,
                kind: ResourceKind::Html,
                url,
                error_kind,
                status_code,
                error,
            } if self.is_current(tab_id, request_id) => {
                self.on_html_network_error(url, error_kind, status_code, error);
            }

            // Parser → DOM snapshot + CSS discovery
            CoreEvent::DomUpdate {
                tab_id,
                request_id,
                dom,
            } if self.is_current(tab_id, request_id) => {
                self.on_dom_update(dom, request_id);
            }
            CoreEvent::DomPatchUpdate {
                tab_id,
                request_id,
                handle,
                from,
                to,
                patches,
            } if self.is_current(tab_id, request_id) => {
                if self.dom_handle != Some(handle) {
                    self.dom_store.clear();
                    let _ = self.dom_store.create(handle);
                    self.dom_handle = Some(handle);
                }
                match self.dom_store.apply(handle, from, to, &patches) {
                    Ok(()) => {
                        if let Ok(dom) = self.dom_store.materialize(handle) {
                            self.on_dom_update(dom, request_id);
                        }
                    }
                    Err(err) => {
                        eprintln!("dom patch apply error: {err:?}");
                    }
                }
            }

            // CSS streaming → CSS runtime
            CoreEvent::NetworkStart {
                tab_id,
                request_id,
                kind: ResourceKind::Css,
                response,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_start(response);
            }
            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                kind: ResourceKind::Css,
                url,
                bytes,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_chunk(url, bytes, request_id);
            }
            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                kind: ResourceKind::Image,
                url,
                bytes,
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_chunk(url, bytes);
            }
            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                kind: ResourceKind::Css,
                response,
                bytes_received,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_done(response, bytes_received, request_id);
            }
            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                kind: ResourceKind::Image,
                response,
                ..
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_done(response.requested_url);
            }
            CoreEvent::NetworkError {
                tab_id,
                request_id,
                kind: ResourceKind::Css,
                url,
                error_kind,
                status_code,
                error,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_error(url, error_kind, status_code, error, request_id);
            }
            CoreEvent::NetworkError {
                tab_id,
                request_id,
                kind: ResourceKind::Image,
                url,
                error_kind: _,
                status_code: _,
                error,
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_error(url, error);
            }

            // CSS runtime → apply styles
            CoreEvent::CssParsedBlock {
                tab_id,
                request_id,
                css_block,
                ..
            } if self.is_current(tab_id, request_id) => {
                self.on_css_parsed_block(css_block);
            }
            CoreEvent::CssSheetDone {
                tab_id,
                request_id,
                url,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_sheet_done(url);
            }

            _ => {}
        }
    }

    fn is_current(&self, tab_id: TabId, request_id: RequestId) -> bool {
        tab_id == self.tab_id && request_id == self.nav_gen
    }

    fn on_html_network_start(&mut self, response: NetworkResponseInfo, request_id: RequestId) {
        self.dom_store.clear();
        self.dom_handle = None;
        self.document_load = DocumentLoadState {
            response: Some(response.clone()),
            bytes_received: 0,
        };
        self.stylesheet_loads.clear();
        self.url = response.final_url.clone();
        self.page.start_nav(response.display_url());
        self.loading = true;
        self.last_status = Some(format!(
            "Loading document • {}",
            response_summary(&response, 0)
        ));
        self.send_cmd(CoreCommand::ParseHtmlStart {
            tab_id: self.tab_id,
            request_id,
        });
        self.poke_redraw();
    }

    fn on_html_network_chunk(&mut self, bytes: Vec<u8>, request_id: RequestId) {
        self.document_load.bytes_received = self
            .document_load
            .bytes_received
            .saturating_add(bytes.len());
        self.send_cmd(CoreCommand::ParseHtmlChunk {
            tab_id: self.tab_id,
            request_id,
            bytes,
        });
    }

    fn on_html_network_done(
        &mut self,
        response: NetworkResponseInfo,
        bytes_received: usize,
        request_id: RequestId,
    ) {
        self.document_load = DocumentLoadState {
            response: Some(response.clone()),
            bytes_received,
        };
        self.send_cmd(CoreCommand::ParseHtmlDone {
            tab_id: self.tab_id,
            request_id,
        });
        self.last_status = Some(format!(
            "Document response complete • {}",
            response_summary(&response, bytes_received)
        ));
        self.poke_redraw();
    }

    fn on_html_network_error(
        &mut self,
        url: String,
        error_kind: NetworkErrorKind,
        status_code: Option<u16>,
        error: String,
    ) {
        self.loading = false;
        self.last_status = Some(format_network_error(
            "document",
            &url,
            error_kind,
            status_code,
            &error,
        ));
        self.poke_redraw();
    }

    fn on_dom_update(&mut self, dom: Box<Node>, request_id: RequestId) {
        self.page.dom = Some(dom);
        self.page.update_head_metadata();
        self.page.apply_inline_style_blocks();
        self.page
            .seed_input_values_from_dom(&mut self.document_input.input_values);
        self.page.update_visible_text_cache();

        self.discover_resources(request_id);

        let pending = self.page.pending_count();
        self.loading = pending > 0;
        let response = self.document_load.response.as_ref();
        let base = if pending > 0 {
            format!("Document parsed • fetching {pending} stylesheet(s)")
        } else {
            "Document parsed".to_string()
        };
        self.last_status = Some(match response {
            Some(response) => format!(
                "{base} • {}",
                response_summary(response, self.document_load.bytes_received)
            ),
            None => base,
        });
        self.poke_redraw();
    }

    fn on_css_network_start(&mut self, response: NetworkResponseInfo) {
        let accept_body = should_accept_css_response(response.content_type.as_deref());
        self.stylesheet_loads.insert(
            response.requested_url.clone(),
            StylesheetLoadState {
                response,
                accept_body,
            },
        );
    }

    fn on_css_network_chunk(&mut self, url: String, bytes: Vec<u8>, request_id: RequestId) {
        if !self
            .stylesheet_loads
            .get(&url)
            .map(|state| state.accept_body)
            .unwrap_or(true)
        {
            return;
        }
        self.send_cmd(CoreCommand::CssChunk {
            tab_id: self.tab_id,
            request_id,
            url,
            bytes,
        });
    }

    fn on_image_network_chunk(&mut self, url: String, bytes: Vec<u8>) {
        self.resources.on_network_chunk(&url, &bytes);
    }

    fn on_css_network_done(
        &mut self,
        response: NetworkResponseInfo,
        _bytes_received: usize,
        request_id: RequestId,
    ) {
        let url = response.requested_url.clone();
        self.stylesheet_loads.insert(
            url.clone(),
            StylesheetLoadState {
                accept_body: should_accept_css_response(response.content_type.as_deref()),
                response,
            },
        );
        self.send_cmd(CoreCommand::CssDone {
            tab_id: self.tab_id,
            request_id,
            url,
        });
    }

    fn on_image_network_done(&mut self, url: String) {
        self.resources.on_network_done(&url, self.repaint.clone());
    }

    fn on_css_network_error(
        &mut self,
        url: String,
        error_kind: NetworkErrorKind,
        status_code: Option<u16>,
        error: String,
        request_id: RequestId,
    ) {
        self.stylesheet_loads.remove(&url);
        self.page.mark_css_done(&url);
        self.send_cmd(CoreCommand::CssAbort {
            tab_id: self.tab_id,
            request_id,
            url: url.clone(),
        });
        let remaining = self.page.pending_count();
        self.loading = remaining > 0;
        self.last_status = Some(format!(
            "{} ({} remaining)",
            format_network_error("stylesheet", &url, error_kind, status_code, &error),
            remaining
        ));
        self.poke_redraw();
    }

    fn on_image_network_error(&mut self, url: String, error: String) {
        self.resources.on_network_error(&url, error);
    }

    fn on_css_parsed_block(&mut self, css_block: String) {
        self.page.apply_css_block(&css_block);
        self.poke_redraw();
    }

    fn on_css_sheet_done(&mut self, url: String) {
        self.page.mark_css_done(&url);
        let remaining = self.page.pending_count();
        self.loading = remaining > 0;
        let stylesheet = self.stylesheet_loads.remove(&url);
        self.last_status = Some(match stylesheet {
            Some(state) if !state.accept_body => {
                let content_type = state
                    .response
                    .content_type
                    .unwrap_or_else(|| "unknown".into());
                if remaining > 0 {
                    format!(
                        "Stylesheet ignored • unexpected content type {content_type} ({} remaining)",
                        remaining
                    )
                } else {
                    format!("Stylesheet ignored • unexpected content type {content_type}")
                }
            }
            _ if remaining > 0 => format!("Stylesheet loaded ({} remaining)", remaining),
            _ => "All stylesheets loaded".to_string(),
        });
        self.poke_redraw();
    }

    fn send_cmd(&self, cmd: CoreCommand) {
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.send(cmd);
        }
    }

    fn poke_redraw(&self) {
        if let Some(repaint) = &self.repaint {
            repaint.request_now();
        }
    }
}

pub use dom_style::{inherited_color, page_background};

fn response_summary(response: &NetworkResponseInfo, bytes_received: usize) -> String {
    let mut parts = Vec::new();

    if let Some(status_code) = response.status_code {
        parts.push(format!("HTTP {status_code}"));
    }

    if let Some(content_type) = response.content_type.as_deref() {
        parts.push(content_type.to_string());
    }

    if bytes_received > 0 {
        parts.push(format!("{bytes_received} B"));
    }

    if response.was_redirected() {
        parts.push(format!("final {}", response.final_url));
    } else {
        parts.push(response.display_url().to_string());
    }

    parts.join(" • ")
}

fn format_network_error(
    resource_label: &str,
    url: &str,
    error_kind: NetworkErrorKind,
    status_code: Option<u16>,
    error: &str,
) -> String {
    match error_kind {
        NetworkErrorKind::Cancelled => format!("Cancelled {resource_label} load: {url}"),
        NetworkErrorKind::HttpStatus => match status_code {
            Some(status_code) => {
                format!("HTTP {status_code} while loading {resource_label}: {url}")
            }
            None => format!("HTTP error while loading {resource_label}: {url}"),
        },
        NetworkErrorKind::Transport => {
            format!("Transport error loading {resource_label}: {url} ({error})")
        }
        NetworkErrorKind::LocalFile => {
            format!("Local file error loading {resource_label}: {url} ({error})")
        }
        NetworkErrorKind::Read => format!("Read error loading {resource_label}: {url} ({error})"),
        NetworkErrorKind::ResourceLimit => {
            format!("Resource limit loading {resource_label}: {url} ({error})")
        }
    }
}

fn should_accept_css_response(content_type: Option<&str>) -> bool {
    // Browser compatibility policy:
    // - missing Content-Type stays accepted because some servers omit it for CSS
    // - explicit non-CSS types are rejected so HTML error pages do not enter the CSS pipeline
    match normalized_content_type(content_type) {
        None => true,
        Some(content_type) => content_type == "text/css",
    }
}

fn normalized_content_type(content_type: Option<&str>) -> Option<String> {
    let content_type = content_type?.split(';').next()?.trim();
    if content_type.is_empty() {
        None
    } else {
        Some(content_type.to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::Tab;
    use bus::{CoreCommand, CoreEvent};
    use core_types::{NetworkResponseInfo, ResourceKind};
    use html::{HtmlParseOptions, parse_document};
    use std::sync::mpsc;

    #[test]
    fn redirected_document_response_updates_tab_base_url_and_status() {
        let mut tab = Tab::new(1);
        tab.nav_gen = 1;
        let requested = "https://example.com".to_string();
        let final_url = "https://example.com/landing".to_string();
        let response = NetworkResponseInfo {
            requested_url: requested,
            final_url: final_url.clone(),
            status_code: Some(200),
            content_type: Some("text/html; charset=utf-8".to_string()),
        };

        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: tab.tab_id,
            request_id: 1,
            kind: ResourceKind::Html,
            response: response.clone(),
        });

        let output = parse_document(
            "<!doctype html><title>Example Domain</title><h1>Example Domain</h1>",
            HtmlParseOptions::default(),
        )
        .expect("parse should succeed");

        tab.on_core_event(CoreEvent::NetworkDone {
            tab_id: tab.tab_id,
            request_id: 1,
            kind: ResourceKind::Html,
            response,
            bytes_received: 63,
        });
        tab.on_core_event(CoreEvent::DomUpdate {
            tab_id: tab.tab_id,
            request_id: 1,
            dom: Box::new(output.document),
        });

        assert_eq!(tab.page.base_url.as_deref(), Some(final_url.as_str()));
        assert!(
            tab.last_status
                .as_deref()
                .unwrap_or_default()
                .contains("Document parsed • HTTP 200"),
            "expected structured document status, got {:?}",
            tab.last_status
        );
    }

    #[test]
    fn css_with_html_content_type_is_not_forwarded_to_css_runtime() {
        let mut tab = Tab::new(1);
        let (tx, rx) = mpsc::channel();
        tab.set_bus_sender(tx);
        tab.nav_gen = 7;

        let url = "https://example.com/site.css".to_string();
        assert!(tab.page.register_css(&url));

        let response = NetworkResponseInfo {
            requested_url: url.clone(),
            final_url: url.clone(),
            status_code: Some(404),
            content_type: Some("text/html".to_string()),
        };

        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: tab.tab_id,
            request_id: 7,
            kind: ResourceKind::Css,
            response: response.clone(),
        });
        tab.on_core_event(CoreEvent::NetworkChunk {
            tab_id: tab.tab_id,
            request_id: 7,
            kind: ResourceKind::Css,
            url: url.clone(),
            bytes: b"<html>not css</html>".to_vec(),
        });
        tab.on_core_event(CoreEvent::NetworkDone {
            tab_id: tab.tab_id,
            request_id: 7,
            kind: ResourceKind::Css,
            response,
            bytes_received: 20,
        });

        let queued = rx.try_iter().collect::<Vec<_>>();
        assert!(
            queued
                .iter()
                .all(|cmd| !matches!(cmd, CoreCommand::CssChunk { .. })),
            "unexpected CSS chunks queued for HTML response: {queued:?}"
        );
        assert!(
            queued.iter().any(
                |cmd| matches!(cmd, CoreCommand::CssDone { url: done_url, .. } if done_url == &url)
            ),
            "expected CssDone to clear pending stylesheet state"
        );

        tab.on_css_sheet_done(url);
        assert_eq!(tab.page.pending_count(), 0);
        assert!(
            tab.last_status
                .as_deref()
                .unwrap_or_default()
                .contains("Stylesheet ignored"),
            "expected ignored stylesheet status, got {:?}",
            tab.last_status
        );
    }

    #[test]
    fn resource_limit_document_error_surfaces_status_and_stops_loading() {
        let mut tab = Tab::new(1);
        tab.nav_gen = 3;

        let response = NetworkResponseInfo {
            requested_url: "https://example.com".to_string(),
            final_url: "https://example.com".to_string(),
            status_code: Some(200),
            content_type: Some("text/html".to_string()),
        };

        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: tab.tab_id,
            request_id: 3,
            kind: ResourceKind::Html,
            response,
        });
        tab.on_core_event(CoreEvent::NetworkError {
            tab_id: tab.tab_id,
            request_id: 3,
            kind: ResourceKind::Html,
            url: "https://example.com".to_string(),
            error_kind: core_types::NetworkErrorKind::ResourceLimit,
            status_code: Some(200),
            error: "html response exceeded byte limit of 10485760 bytes".to_string(),
        });

        assert!(
            tab.last_status
                .as_deref()
                .unwrap_or_default()
                .contains("Resource limit loading document"),
            "expected resource-limit document status, got {:?}",
            tab.last_status
        );
        assert!(!tab.loading, "document load should stop after limit error");
    }

    #[test]
    fn stylesheet_resource_limit_aborts_partial_css_and_clears_pending_state() {
        let mut tab = Tab::new(1);
        let (tx, rx) = mpsc::channel();
        tab.set_bus_sender(tx);
        tab.nav_gen = 9;

        let url = "https://example.com/site.css".to_string();
        assert!(tab.page.register_css(&url));

        let response = NetworkResponseInfo {
            requested_url: url.clone(),
            final_url: url.clone(),
            status_code: Some(200),
            content_type: Some("text/css".to_string()),
        };

        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: tab.tab_id,
            request_id: 9,
            kind: ResourceKind::Css,
            response,
        });
        tab.on_core_event(CoreEvent::NetworkChunk {
            tab_id: tab.tab_id,
            request_id: 9,
            kind: ResourceKind::Css,
            url: url.clone(),
            bytes: b"body { color: red; }".to_vec(),
        });
        tab.on_core_event(CoreEvent::NetworkError {
            tab_id: tab.tab_id,
            request_id: 9,
            kind: ResourceKind::Css,
            url: url.clone(),
            error_kind: core_types::NetworkErrorKind::ResourceLimit,
            status_code: Some(200),
            error: "css response exceeded byte limit of 2097152 bytes".to_string(),
        });

        let queued = rx.try_iter().collect::<Vec<_>>();
        assert!(
            queued
                .iter()
                .any(|cmd| matches!(cmd, CoreCommand::CssChunk { url: chunk_url, .. } if chunk_url == &url)),
            "expected partial CSS chunks to be buffered before the limit-triggered abort"
        );
        assert!(
            queued
                .iter()
                .any(|cmd| matches!(cmd, CoreCommand::CssAbort { url: abort_url, .. } if abort_url == &url)),
            "expected CssAbort to discard buffered stylesheet state"
        );
        assert!(
            queued.iter().all(
                |cmd| !matches!(cmd, CoreCommand::CssDone { url: done_url, .. } if done_url == &url)
            ),
            "unexpected CssDone on stylesheet limit failure: {queued:?}"
        );
        assert_eq!(tab.page.pending_count(), 0);
        assert!(
            tab.last_status
                .as_deref()
                .unwrap_or_default()
                .contains("Resource limit loading stylesheet"),
            "expected stylesheet limit status, got {:?}",
            tab.last_status
        );
    }
}
