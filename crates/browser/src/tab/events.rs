use super::Tab;
use bus::CoreEvent;
use core_types::ResourceKind;

impl Tab {
    pub fn on_core_event(&mut self, evt: CoreEvent) {
        match evt {
            CoreEvent::NetworkStart {
                tab_id,
                request_id,
                stylesheet_slot_id: _,
                kind: ResourceKind::Html,
                response,
            } if self.is_current(tab_id, request_id) => {
                self.on_html_network_start(response, request_id);
            }

            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                stylesheet_slot_id: _,
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
                stylesheet_slot_id: _,
                kind: ResourceKind::Html,
                response,
                bytes_received,
            } if self.is_current(tab_id, request_id) => {
                self.on_html_network_done(response, bytes_received, request_id);
            }

            CoreEvent::NetworkError {
                tab_id,
                request_id,
                stylesheet_slot_id: _,
                kind: ResourceKind::Html,
                url,
                error_kind,
                status_code,
                error,
            } if self.is_current(tab_id, request_id) => {
                self.on_html_network_error(url, error_kind, status_code, error);
            }

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

            CoreEvent::NetworkStart {
                tab_id,
                request_id,
                stylesheet_slot_id: Some(stylesheet_slot_id),
                kind: ResourceKind::Css,
                response,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_start(stylesheet_slot_id, response);
            }
            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                stylesheet_slot_id: Some(stylesheet_slot_id),
                kind: ResourceKind::Css,
                url,
                bytes,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_chunk(stylesheet_slot_id, url, bytes, request_id);
            }
            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                stylesheet_slot_id: _,
                kind: ResourceKind::Image,
                url,
                bytes,
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_chunk(url, bytes);
            }
            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                stylesheet_slot_id: Some(stylesheet_slot_id),
                kind: ResourceKind::Css,
                response,
                bytes_received,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_done(stylesheet_slot_id, response, bytes_received, request_id);
            }
            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                stylesheet_slot_id: _,
                kind: ResourceKind::Image,
                response,
                ..
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_done(response.requested_url);
            }
            CoreEvent::NetworkError {
                tab_id,
                request_id,
                stylesheet_slot_id: Some(stylesheet_slot_id),
                kind: ResourceKind::Css,
                url,
                error_kind,
                status_code,
                error,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_error(
                    stylesheet_slot_id,
                    url,
                    error_kind,
                    status_code,
                    error,
                    request_id,
                );
            }
            CoreEvent::NetworkError {
                tab_id,
                request_id,
                stylesheet_slot_id: _,
                kind: ResourceKind::Image,
                url,
                error_kind: _,
                status_code: _,
                error,
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_error(url, error);
            }

            CoreEvent::CssDecodedBlock {
                tab_id,
                request_id,
                stylesheet_slot_id,
                css_block,
                ..
            } if self.is_current(tab_id, request_id) => {
                self.on_css_decoded_block(stylesheet_slot_id, css_block);
            }
            CoreEvent::CssSheetDone {
                tab_id,
                request_id,
                stylesheet_slot_id,
                url,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_sheet_done(stylesheet_slot_id, url);
            }

            _ => {}
        }
    }
}
