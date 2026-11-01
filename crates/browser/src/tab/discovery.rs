use super::Tab;
use super::state::{StylesheetLoadPhase, StylesheetLoadState};
use bus::CoreCommand;
use core_types::{RequestId, ResourceKind, StylesheetSlotId};

impl Tab {
    pub(crate) fn discover_resources(&mut self, request_id: RequestId) {
        let _ = self.discover_stylesheets(request_id);
        self.discover_images(request_id);
    }

    fn discover_stylesheets(&mut self, request_id: RequestId) -> bool {
        let outcome = self.page.reconcile_document_stylesheets();
        self.retire_stylesheet_requests(outcome.retired);
        for fetch in outcome.fetches {
            let command = CoreCommand::FetchStream {
                tab_id: self.tab_id,
                request_id,
                stylesheet_slot_id: Some(fetch.slot_id),
                url: fetch.key.as_str().to_owned(),
                kind: ResourceKind::Css,
            };
            // Register before enqueueing. Event handlers require exclusive &mut
            // Tab access, so none can observe the provisional record during this
            // call. Queue acceptance is not response admission or network execution.
            let std::collections::hash_map::Entry::Vacant(entry) =
                self.stylesheet_loads.entry(fetch.slot_id)
            else {
                // A retained attachment never starts a second request. Preserve
                // any existing admission rather than replacing its provenance.
                continue;
            };
            entry.insert(StylesheetLoadState {
                request_id,
                issued_key: fetch.key,
                phase: StylesheetLoadPhase::AwaitingStart,
            });
            if self
                .cmd_tx
                .as_ref()
                .is_none_or(|tx| tx.send(command).is_err())
            {
                // Nothing was dispatched: rollback without a phantom CssAbort.
                self.stylesheet_loads.remove(&fetch.slot_id);
                let work = self.page.mark_css_failed(fetch.slot_id);
                self.request_optional_render_work(work);
                if self.document_load.parse_status != super::state::DocumentParseStatus::Failed {
                    self.last_status = Some("Stylesheet request delivery failed".into());
                }
                self.update_loading();
            }
        }
        self.request_optional_render_work(outcome.render_invalidation)
    }

    pub(super) fn retire_stylesheet_requests(&mut self, retired: Vec<StylesheetSlotId>) {
        for slot in retired {
            if let Some(load) = self.stylesheet_loads.remove(&slot) {
                self.abort_css_buffer(slot, load);
            }
        }
    }

    /// Retire only actually issued requests, using their original generation.
    /// CssAbort releases CSS runtime buffering; it does not cancel HTTP.
    pub(crate) fn retire_document_stylesheets(&mut self) {
        let mut retired: Vec<_> = self.stylesheet_loads.keys().copied().collect();
        retired.sort_by_key(|id| id.0);
        self.retire_stylesheet_requests(retired);
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
}
