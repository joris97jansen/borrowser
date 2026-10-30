use super::Tab;
use bus::CoreCommand;
use core_types::{RequestId, ResourceKind, StylesheetSlotId};

impl Tab {
    pub(crate) fn discover_resources(&mut self, request_id: RequestId) {
        let _ = self.discover_stylesheets(request_id);
        self.discover_images(request_id);
    }

    fn discover_stylesheets(&mut self, request_id: RequestId) -> bool {
        let outcome = self.page.reconcile_document_stylesheets();
        let active = self.page.active_external_stylesheet_slots();
        self.stylesheet_loads.retain(|id, _| active.contains(id));
        self.retire_stylesheet_requests(outcome.retired, request_id);
        for fetch in outcome.fetches {
            self.send_fetch(
                request_id,
                Some(fetch.slot_id),
                fetch.url,
                ResourceKind::Css,
            );
        }
        self.request_optional_render_work(outcome.render_invalidation)
    }

    pub(super) fn retire_stylesheet_requests(
        &mut self,
        retired: Vec<crate::document_style::StylesheetFetch>,
        request_id: RequestId,
    ) {
        for request in retired {
            self.stylesheet_loads.remove(&request.slot_id);
            self.send_cmd(CoreCommand::CssAbort {
                tab_id: self.tab_id,
                request_id,
                stylesheet_slot_id: request.slot_id,
                url: request.url,
            });
        }
    }

    /// Dispose the current document's stylesheet buffers before its slots are
    /// cleared or the tab is removed. The generation belongs to those slots.
    pub(crate) fn retire_document_stylesheets(&mut self) {
        let retired = self.page.external_stylesheet_requests();
        self.retire_stylesheet_requests(retired, self.nav_gen);
    }

    fn discover_images(&mut self, request_id: RequestId) {
        let sources = self.page.reconcile_image_sources();
        let cmd_tx = self.cmd_tx.clone();
        let tab_id = self.tab_id;
        for url in sources {
            self.resources.request_image(url, |url| {
                if let Some(tx) = &cmd_tx {
                    let _ = tx.send(CoreCommand::FetchStream {
                        tab_id,
                        request_id,
                        stylesheet_slot_id: None,
                        url,
                        kind: ResourceKind::Image,
                    });
                }
            });
        }
    }

    fn send_fetch(
        &self,
        request_id: RequestId,
        stylesheet_slot_id: Option<StylesheetSlotId>,
        url: String,
        kind: ResourceKind,
    ) {
        self.send_cmd(CoreCommand::FetchStream {
            tab_id: self.tab_id,
            request_id,
            stylesheet_slot_id,
            url,
            kind,
        });
    }
}
