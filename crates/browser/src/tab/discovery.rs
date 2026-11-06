use super::Tab;
use super::state::{StylesheetLoadPhase, StylesheetLoadState};
use addressing::ResourceAddress;
use bus::CoreCommand;
use core_types::{RequestId, ResourceKind, StylesheetSlotId};
use net::{FetchRequest, HttpHeaders, RequestAdmissionError, RequestMethod};

impl Tab {
    pub(crate) fn discover_resources(&mut self, request_id: RequestId) {
        let _ = self.discover_stylesheets(request_id);
        self.discover_images(request_id);
    }

    fn discover_stylesheets(&mut self, request_id: RequestId) -> bool {
        let outcome = self.page.reconcile_document_stylesheets();
        self.retire_stylesheet_requests(outcome.retired);
        for fetch in outcome.fetches {
            let issued_key = fetch.address.retrieval_key();
            let command = self
                .resource_request(request_id, fetch.address)
                .and_then(|request| {
                    CoreCommand::try_fetch_stream(
                        self.tab_id,
                        Some(fetch.slot_id),
                        ResourceKind::Css,
                        request,
                    )
                });
            let command = match command {
                Ok(command) => command,
                Err(_) => {
                    let work = self.page.mark_css_failed(fetch.slot_id);
                    self.request_optional_render_work(work);
                    if self.document_load.parse_status != super::state::DocumentParseStatus::Failed
                    {
                        self.last_status = Some("Stylesheet request rejected".into());
                    }
                    self.update_loading();
                    continue;
                }
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
                issued_key,
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
        for address in sources {
            // Preserve the existing complete image lookup key, independently
            // from AH's fragmentless transport/retrieval projection.
            let key = address.url().as_str().to_owned();
            let command = self
                .resource_request(request_id, address)
                .and_then(|request| {
                    CoreCommand::try_fetch_stream(tab_id, None, ResourceKind::Image, request)
                });
            self.resources.request_image(request_id, key, || {
                let command = command.map_err(|_| "Image request rejected")?;
                cmd_tx
                    .as_ref()
                    .ok_or("Image request delivery failed")?
                    .send(command)
                    .map_err(|_| "Image request delivery failed")
            });
        }
    }

    fn resource_request(
        &self,
        request_id: RequestId,
        address: ResourceAddress,
    ) -> Result<FetchRequest, RequestAdmissionError> {
        FetchRequest::try_new(
            request_id,
            address,
            RequestMethod::Get,
            HttpHeaders::new(),
            self.page.document_url().cloned(),
            self.page
                .addresses
                .as_ref()
                .map(|state| state.document_origin().clone()),
        )
    }
}
