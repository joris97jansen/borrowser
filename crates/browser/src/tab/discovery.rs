use super::Tab;
use super::state::{StylesheetLoadPhase, StylesheetLoadState};
use addressing::ResourceAddress;
use bus::CoreCommand;
use core_types::{RequestId, ResourceKind, StylesheetSlotId};
use net::{FetchRequest, HttpHeaders, RequestAdmissionError, RequestMethod};

impl Tab {
    pub(super) fn dispatch_stylesheets(
        &mut self,
        request_id: RequestId,
        fetches: Vec<StylesheetSlotId>,
    ) {
        #[cfg(test)]
        use crate::stylesheet_fetch::{AllocationBoundary, allocation_refused};
        use crate::stylesheet_fetch::{
            METADATA_BYTES, StylesheetFetchAccumulator, StylesheetFetchFailure,
            StylesheetRequestEvidence, StylesheetRetentionFailure, request_charge_before_copy,
        };
        for slot in fetches {
            let reject = |tab: &mut Self, failure: StylesheetFetchFailure| {
                tab.finish_unissued_stylesheet(
                    slot,
                    StylesheetRequestEvidence::NotConstructed,
                    failure,
                );
            };
            if !self.page.stylesheet_body_available() {
                reject(
                    self,
                    StylesheetFetchFailure::Retention(
                        StylesheetRetentionFailure::BodyReservationDenied,
                    ),
                );
                continue;
            }
            if let Err(f) = self.page.reserve_stylesheet_metadata(slot) {
                reject(self, StylesheetFetchFailure::Retention(f));
                continue;
            }
            let opaque = self
                .page
                .addresses
                .as_ref()
                .is_none_or(|a| a.document_origin().is_opaque());
            let origin_allowance = if opaque {
                Some(0)
            } else {
                self.page
                    .document_url()
                    .map_or(Some(32), |u| u.as_str().len().checked_add(32))
            };
            let charge = request_charge_before_copy(
                self.page
                    .stylesheet_address(slot)
                    .expect("prepared frozen address")
                    .url(),
                self.page.document_url(),
                opaque,
            );
            if charge.is_none_or(|n| n > METADATA_BYTES) || origin_allowance.is_none() {
                self.page.release_stylesheet_metadata(slot);
                reject(
                    self,
                    StylesheetFetchFailure::Retention(
                        StylesheetRetentionFailure::RequestMetadataLimit,
                    ),
                );
                continue;
            }
            let request = match self.resource_request(
                request_id,
                self.page
                    .stylesheet_address(slot)
                    .expect("admitted frozen address")
                    .clone(),
            ) {
                Ok(r) => r,
                Err(e) => {
                    self.page.release_stylesheet_metadata(slot);
                    reject(self, StylesheetFetchFailure::RequestAdmission(e));
                    continue;
                }
            };
            let origin_allowance = origin_allowance.expect("checked origin allowance");
            let accumulator = match StylesheetFetchAccumulator::try_new(request, origin_allowance) {
                Ok(accumulator) => accumulator,
                Err(result) => {
                    self.page.release_stylesheet_metadata(slot);
                    self.page.finish_stylesheet_fetch(slot, result);
                    let work = self.page.mark_css_failed(slot);
                    self.request_optional_render_work(work);
                    continue;
                }
            };
            let StylesheetFetchAccumulator {
                request,
                issued_key,
                origin_allowance,
                ..
            } = accumulator;
            if let Err(f) = self.page.provision_stylesheet_body(slot) {
                self.finish_unissued_stylesheet(
                    slot,
                    StylesheetRequestEvidence::Constructed {
                        request,
                        issued_key,
                        enqueued: false,
                        origin_allowance,
                    },
                    StylesheetFetchFailure::Retention(f),
                );
                continue;
            }
            #[cfg(test)]
            let refused = allocation_refused(AllocationBoundary::RequestRecord);
            #[cfg(not(test))]
            let refused = false;
            if refused || self.stylesheet_loads.try_reserve(1).is_err() {
                self.finish_unissued_stylesheet(
                    slot,
                    StylesheetRequestEvidence::Constructed {
                        request,
                        issued_key,
                        enqueued: false,
                        origin_allowance,
                    },
                    StylesheetFetchFailure::Retention(
                        StylesheetRetentionFailure::RequestRecordAllocation,
                    ),
                );
                continue;
            }
            let command = match CoreCommand::try_fetch_stream(
                self.tab_id,
                Some(slot),
                ResourceKind::Css,
                request.clone(),
            ) {
                Ok(c) => c,
                Err(e) => {
                    self.finish_unissued_stylesheet(
                        slot,
                        StylesheetRequestEvidence::Constructed {
                            request,
                            issued_key,
                            enqueued: false,
                            origin_allowance,
                        },
                        StylesheetFetchFailure::RequestAdmission(e),
                    );
                    continue;
                }
            };
            self.stylesheet_loads.insert(
                slot,
                StylesheetLoadState {
                    request_id,
                    phase: StylesheetLoadPhase::Fetching(StylesheetFetchAccumulator {
                        request,
                        issued_key,
                        origin_allowance,
                        response: None,
                        policy: None,
                        bytes: Vec::new(),
                        observed_bytes: 0,
                    }),
                },
            );
            if self
                .cmd_tx
                .as_ref()
                .is_some_and(|tx| tx.send(command).is_ok())
            {
                self.page.issue_stylesheet_body(slot);
            } else {
                let load = self
                    .stylesheet_loads
                    .remove(&slot)
                    .expect("provisional request");
                let StylesheetLoadPhase::Fetching(a) = load.phase else {
                    unreachable!("new request")
                };
                self.finish_unissued_stylesheet(
                    slot,
                    StylesheetRequestEvidence::Constructed {
                        request: a.request,
                        issued_key: a.issued_key,
                        enqueued: false,
                        origin_allowance: a.origin_allowance,
                    },
                    StylesheetFetchFailure::RequestDeliveryUnavailable,
                );
            }
        }
    }
    fn finish_unissued_stylesheet(
        &mut self,
        slot: core_types::StylesheetSlotId,
        request: crate::stylesheet_fetch::StylesheetRequestEvidence,
        failure: crate::stylesheet_fetch::StylesheetFetchFailure,
    ) {
        self.page.rollback_stylesheet_body(slot);
        self.page.finish_stylesheet_fetch(
            slot,
            crate::stylesheet_fetch::StylesheetFetchResult::unissued(request, failure),
        );
        let work = self.page.mark_css_failed(slot);
        self.request_optional_render_work(work);
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

    pub(super) fn discover_images(&mut self, request_id: RequestId) {
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
