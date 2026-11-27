use super::Tab;
use super::state::{CssRuntimeProgress, StylesheetLoadPhase, StylesheetLoadState};
#[cfg(test)]
use crate::stylesheet_fetch::{AllocationBoundary, allocation_refused};
use crate::stylesheet_fetch::{
    METADATA_BYTES, StylesheetBodyEvidence, StylesheetCollectionStop, StylesheetErrorContext,
    StylesheetEvidenceOmission, StylesheetFetchFailure, StylesheetFetchOutcome,
    StylesheetFetchResult, StylesheetInstallationFailure, StylesheetProtocolFailure,
    StylesheetResponseEvidence, StylesheetResponsePolicy, StylesheetRetentionFailure,
    metadata_charge, try_string,
};
use bus::CoreCommand;
use core_types::{RequestId, StylesheetSlotId};
use net::{BodyProgress, NetworkErrorKind, NetworkResponseInfo};

// Deterministically disconnect the actual test queue after Chunk succeeds. This
// exercises the real second send without a scheduler race or production policy.
#[cfg(test)]
thread_local! {
    pub(super) static DISCONNECT_BEFORE_CSS_DONE: std::cell::RefCell<Option<std::sync::mpsc::Receiver<CoreCommand>>> = const { std::cell::RefCell::new(None) };
}

impl Tab {
    #[cfg(test)]
    pub(super) fn stylesheet_response_for_tests(
        &self,
        slot: StylesheetSlotId,
    ) -> Option<&NetworkResponseInfo> {
        self.stylesheet_loads
            .get(&slot)
            .and_then(|l| l.phase.accepted_response())
            .or_else(|| {
                self.page
                    .stylesheet_fetch_result(slot)
                    .and_then(|r| r.response().response())
            })
    }
    fn accepts_css_network_event(&self, slot: StylesheetSlotId, request: RequestId) -> bool {
        self.document_load.failure().is_none()
            && self.active_document_request_id() == Some(request)
            && self.page.has_external_stylesheet_slot(slot)
            && self.stylesheet_loads.get(&slot).is_some_and(|l| {
                l.request_id == request && matches!(l.phase, StylesheetLoadPhase::Fetching(_))
            })
    }
    fn accepts_css_runtime_event(&self, slot: StylesheetSlotId, request: RequestId) -> bool {
        self.active_document_request_id() == Some(request)
            && self.page.has_external_stylesheet_slot(slot)
            && self.stylesheet_loads.get(&slot).is_some_and(|l| {
                l.request_id == request
                    && matches!(l.phase, StylesheetLoadPhase::AwaitingRuntime { .. })
            })
            && self
                .page
                .stylesheet_fetch_result(slot)
                .is_some_and(|r| r.is_ready())
    }
    fn deliver_css_command(&self, command: CoreCommand) -> bool {
        #[cfg(test)]
        if matches!(command, CoreCommand::CssDone { .. }) {
            DISCONNECT_BEFORE_CSS_DONE.with(|receiver| drop(receiver.borrow_mut().take()));
        }
        self.cmd_tx
            .as_ref()
            .is_some_and(|tx| tx.send(command).is_ok())
    }
    pub(super) fn abort_css_buffer(&mut self, slot: StylesheetSlotId, load: StylesheetLoadState) {
        // Acquiring bytes never creates a CSS runtime buffer. Runtime admission
        // owns an already allocated cleanup URL independent of retained evidence.
        if let StylesheetLoadPhase::AwaitingRuntime { abort_url, .. } = load.phase {
            let _ = self.deliver_css_command(CoreCommand::CssAbort {
                tab_id: self.tab_id,
                request_id: load.request_id,
                stylesheet_slot_id: slot,
                url: abort_url,
            });
        }
    }
    pub(super) fn fail_css_acquisition(
        &mut self,
        slot: StylesheetSlotId,
        failure: StylesheetFetchFailure,
        progress: Option<BodyProgress>,
        stop: StylesheetCollectionStop,
    ) {
        let Some(load) = self.stylesheet_loads.remove(&slot) else {
            return;
        };
        let StylesheetLoadPhase::Fetching(a) = load.phase else {
            self.stylesheet_loads.insert(slot, load);
            return;
        };
        let mut result = a.finish(StylesheetFetchOutcome::Failed(failure), progress, stop);
        if matches!(
            failure,
            StylesheetFetchFailure::Protocol(
                StylesheetProtocolFailure::RequestIdentity
                    | StylesheetProtocolFailure::CompletionMetadata
            )
        ) {
            result.context_omission = Some(StylesheetEvidenceOmission::ConflictingContext);
        }
        if matches!(
            failure,
            StylesheetFetchFailure::Protocol(
                StylesheetProtocolFailure::BodyRoute | StylesheetProtocolFailure::BodyCount
            )
        ) {
            result.producer_progress = None;
            result.progress_omission = Some(StylesheetEvidenceOmission::InconsistentProgress);
        }
        self.publish_css_acquisition_failure(slot, result);
    }
    fn publish_css_acquisition_failure(
        &mut self,
        slot: StylesheetSlotId,
        result: StylesheetFetchResult,
    ) {
        self.page.finish_stylesheet_fetch(slot, result);
        // No parse can have been installed before successful network completion.
        self.page.mark_css_aborted(slot);
        if self.document_load.failure().is_none() {
            self.update_loading();
            self.poke_redraw();
        }
    }
    fn fail_css_installation(
        &mut self,
        slot: StylesheetSlotId,
        failure: StylesheetInstallationFailure,
    ) {
        let Some(load) = self.stylesheet_loads.remove(&slot) else {
            return;
        };
        self.page.finish_stylesheet_installation(slot, Err(failure));
        let work = self.page.mark_css_aborted(slot);
        self.abort_css_buffer(slot, load);
        self.update_loading();
        if !self.request_optional_render_work(work) {
            self.poke_redraw();
        }
    }
    pub(super) fn on_css_network_start(
        &mut self,
        slot: StylesheetSlotId,
        response: NetworkResponseInfo,
        request: RequestId,
    ) {
        if !self.accepts_css_network_event(slot, request) {
            return;
        }
        let StylesheetLoadPhase::Fetching(a) = &self.stylesheet_loads[&slot].phase else {
            unreachable!()
        };
        if a.response.is_some() {
            return;
        }
        let source_matches = self.page.stylesheet_address(slot).is_some_and(|address| {
            address.kind() == a.request.address().kind()
                && address.url() == a.request.original_url()
        });
        if !source_matches || !a.identity_matches(&response) {
            self.fail_css_acquisition(
                slot,
                StylesheetFetchFailure::Protocol(StylesheetProtocolFailure::RequestIdentity),
                None,
                StylesheetCollectionStop::Protocol,
            );
            return;
        }
        if metadata_charge(&a.request, a.origin_allowance, Some(&response))
            .is_none_or(|n| n > METADATA_BYTES)
        {
            let load = self
                .stylesheet_loads
                .remove(&slot)
                .expect("admitted request");
            let StylesheetLoadPhase::Fetching(a) = load.phase else {
                unreachable!()
            };
            let mut result = a.finish(
                StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Retention(
                    StylesheetRetentionFailure::ResponseMetadataLimit,
                )),
                None,
                StylesheetCollectionStop::Retention,
            );
            result.response =
                StylesheetResponseEvidence::Unavailable(StylesheetEvidenceOmission::MetadataLimit);
            result.body =
                StylesheetBodyEvidence::Unavailable(StylesheetEvidenceOmission::CollectionStopped);
            result.context_omission = Some(StylesheetEvidenceOmission::MetadataLimit);
            self.publish_css_acquisition_failure(slot, result);
            return;
        }
        let policy = StylesheetResponsePolicy::classify(&response, &a.request);
        let StylesheetLoadPhase::Fetching(a) = &mut self
            .stylesheet_loads
            .get_mut(&slot)
            .expect("admitted request")
            .phase
        else {
            unreachable!()
        };
        a.response = Some(response);
        a.policy = Some(policy);
    }
    pub(super) fn on_css_network_chunk(
        &mut self,
        slot: StylesheetSlotId,
        url: String,
        bytes: Vec<u8>,
        request: RequestId,
    ) {
        if !self.accepts_css_network_event(slot, request) {
            return;
        }
        let StylesheetLoadPhase::Fetching(a) = &self.stylesheet_loads[&slot].phase else {
            unreachable!()
        };
        let failure = if a.response.is_none() {
            Some(StylesheetProtocolFailure::ChunkBeforeStart)
        } else if url != a.issued_key.as_str() {
            Some(StylesheetProtocolFailure::IssuedKey)
        } else if a
            .response
            .as_ref()
            .and_then(NetworkResponseInfo::status)
            .is_some_and(|s| s.has_null_body())
            && !bytes.is_empty()
        {
            Some(StylesheetProtocolFailure::NullBodyBytes)
        } else {
            None
        };
        if let Some(f) = failure {
            let load = self
                .stylesheet_loads
                .remove(&slot)
                .expect("admitted request");
            let StylesheetLoadPhase::Fetching(a) = load.phase else {
                unreachable!()
            };
            let mut result = a.finish(
                StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Protocol(f)),
                None,
                StylesheetCollectionStop::Protocol,
            );
            result.discarded_event_bytes = bytes.len();
            result.discarded_event_reason = Some(f);
            self.publish_css_acquisition_failure(slot, result);
            return;
        }
        let StylesheetLoadPhase::Fetching(a) = &mut self
            .stylesheet_loads
            .get_mut(&slot)
            .expect("admitted request")
            .phase
        else {
            unreachable!()
        };
        if let Err(f) = a.append(&bytes) {
            self.fail_css_acquisition(
                slot,
                StylesheetFetchFailure::Retention(f),
                None,
                StylesheetCollectionStop::Retention,
            );
        }
    }
    pub(super) fn on_css_network_done(
        &mut self,
        slot: StylesheetSlotId,
        response: NetworkResponseInfo,
        body: BodyProgress,
        request: RequestId,
    ) {
        if !self.accepts_css_network_event(slot, request) {
            return;
        }
        let StylesheetLoadPhase::Fetching(a) = &self.stylesheet_loads[&slot].phase else {
            unreachable!()
        };
        let started = a.response.is_some();
        let metadata_valid = a.response.as_ref() == Some(&response);
        let route_valid = body.matches_route(core_types::ResourceKind::Css);
        let count_valid = body.delivered_bytes() == a.observed_bytes;
        let failure = if !started {
            Some(StylesheetProtocolFailure::CompletionBeforeStart)
        } else if !metadata_valid {
            Some(StylesheetProtocolFailure::CompletionMetadata)
        } else if !route_valid {
            Some(StylesheetProtocolFailure::BodyRoute)
        } else if !count_valid {
            Some(StylesheetProtocolFailure::BodyCount)
        } else {
            None
        };
        if let Some(f) = failure {
            let load = self
                .stylesheet_loads
                .remove(&slot)
                .expect("admitted request");
            let StylesheetLoadPhase::Fetching(a) = load.phase else {
                unreachable!()
            };
            let mut result = a.finish(
                StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Protocol(f)),
                (route_valid && count_valid).then_some(body),
                StylesheetCollectionStop::Protocol,
            );
            if !metadata_valid {
                result.context_omission = Some(StylesheetEvidenceOmission::ConflictingContext);
            }
            if !route_valid || !count_valid {
                result.progress_omission = Some(StylesheetEvidenceOmission::InconsistentProgress);
            }
            self.publish_css_acquisition_failure(slot, result);
            return;
        }
        let load = self
            .stylesheet_loads
            .remove(&slot)
            .expect("admitted request");
        let StylesheetLoadPhase::Fetching(a) = load.phase else {
            unreachable!()
        };
        let outcome = a.policy.expect("Start policy").rejection().map_or(
            StylesheetFetchOutcome::Ready,
            StylesheetFetchOutcome::Rejected,
        );
        self.page.finish_stylesheet_fetch(
            slot,
            a.finish(outcome, Some(body), StylesheetCollectionStop::Protocol),
        );
        if outcome != StylesheetFetchOutcome::Ready {
            self.page.mark_css_failed(slot);
            self.update_loading();
            if self.document_load.failure().is_none() {
                self.last_status = Some("Stylesheet ignored: response rejected".into());
            }
            self.poke_redraw();
            return;
        }
        let Some((chunk, done, abort_url)) = self.prepare_css_handoff(slot, request) else {
            self.page.finish_stylesheet_installation(
                slot,
                Err(StylesheetInstallationFailure::HandoffAllocation),
            );
            self.page.mark_css_failed(slot);
            self.update_loading();
            return;
        };
        if !self.deliver_css_command(chunk) {
            self.page.finish_stylesheet_installation(
                slot,
                Err(StylesheetInstallationFailure::ChunkDeliveryUnavailable),
            );
            self.page.mark_css_failed(slot);
            self.update_loading();
            return;
        }
        self.stylesheet_loads.insert(
            slot,
            StylesheetLoadState {
                request_id: request,
                phase: StylesheetLoadPhase::AwaitingRuntime {
                    progress: CssRuntimeProgress::AwaitingBlock,
                    abort_url,
                },
            },
        );
        if !self.deliver_css_command(done) {
            self.fail_css_installation(
                slot,
                StylesheetInstallationFailure::DoneDeliveryUnavailable,
            );
        }
    }
    fn prepare_css_handoff(
        &self,
        slot: StylesheetSlotId,
        request: RequestId,
    ) -> Option<(CoreCommand, CoreCommand, String)> {
        #[cfg(test)]
        if allocation_refused(AllocationBoundary::Handoff) {
            return None;
        }
        let result = self.page.stylesheet_fetch_result(slot)?;
        let StylesheetBodyEvidence::Complete(bytes) = &result.body else {
            return None;
        };
        let key = result.request.issued_key()?.as_str();
        let mut copy = Vec::new();
        copy.try_reserve_exact(bytes.len()).ok()?;
        copy.extend_from_slice(bytes);
        let chunk_url = try_string(key).ok()?;
        let done_url = try_string(key).ok()?;
        let abort_url = try_string(key).ok()?;
        Some((
            CoreCommand::CssChunk {
                tab_id: self.tab_id,
                request_id: request,
                stylesheet_slot_id: slot,
                url: chunk_url,
                bytes: copy,
            },
            CoreCommand::CssDone {
                tab_id: self.tab_id,
                request_id: request,
                stylesheet_slot_id: slot,
                url: done_url,
            },
            abort_url,
        ))
    }
    pub(super) fn on_css_network_error(
        &mut self,
        slot: StylesheetSlotId,
        url: String,
        kind: NetworkErrorKind,
        response: Option<NetworkResponseInfo>,
        body: Option<BodyProgress>,
        request: RequestId,
    ) {
        if !self.accepts_css_network_event(slot, request) {
            return;
        }
        let load = self
            .stylesheet_loads
            .remove(&slot)
            .expect("admitted request");
        let StylesheetLoadPhase::Fetching(a) = load.phase else {
            unreachable!()
        };
        let key_matches = url == a.issued_key.as_str();
        let route_valid = body.is_none_or(|p| p.matches_route(core_types::ResourceKind::Css));
        let count_valid = body.is_none_or(|p| p.delivered_bytes() == a.observed_bytes);
        let progress_valid = key_matches && route_valid && count_valid;
        let context_conflict = response.as_ref().is_some_and(|r| {
            !a.identity_matches(r) || a.response.as_ref().is_some_and(|selected| selected != r)
        });
        let mut context_omission = None;
        let mut context_kind = StylesheetErrorContext::Absent;
        let mut context = None;
        if let Some(r) = response {
            if !key_matches {
                context_omission = Some(StylesheetEvidenceOmission::UnauthenticatedEvent);
                context_kind = StylesheetErrorContext::Discarded;
            } else if context_conflict {
                context_omission = Some(StylesheetEvidenceOmission::ConflictingContext);
                context_kind = StylesheetErrorContext::Discarded;
            } else if a.response.is_some() {
                context_kind = StylesheetErrorContext::MatchesSelected;
            } else if metadata_charge(&a.request, a.origin_allowance, Some(&r))
                .is_some_and(|n| n <= METADATA_BYTES)
            {
                context = Some(r);
                context_kind = StylesheetErrorContext::Contextual;
            } else {
                context_omission = Some(StylesheetEvidenceOmission::MetadataLimit);
                context_kind = StylesheetErrorContext::Discarded;
            }
        }
        let protocol = if !key_matches {
            Some(StylesheetProtocolFailure::IssuedKey)
        } else if context_conflict {
            Some(StylesheetProtocolFailure::CompletionMetadata)
        } else if !route_valid {
            Some(StylesheetProtocolFailure::BodyRoute)
        } else if !count_valid {
            Some(StylesheetProtocolFailure::BodyCount)
        } else {
            None
        };
        let outcome = StylesheetFetchOutcome::Failed(protocol.map_or(
            StylesheetFetchFailure::Network(kind),
            StylesheetFetchFailure::Protocol,
        ));
        let stop = if protocol.is_some() {
            StylesheetCollectionStop::Protocol
        } else {
            StylesheetCollectionStop::Network
        };
        let mut result = a.finish(outcome, body.filter(|_| progress_valid), stop);
        if let Some(context) = context {
            result.response = StylesheetResponseEvidence::Contextual(context);
        } else if matches!(result.response, StylesheetResponseEvidence::Absent)
            && let Some(reason) = context_omission
        {
            result.response = StylesheetResponseEvidence::Unavailable(reason);
        }
        result.acquisition_error = Some(kind);
        result.error_context = context_kind;
        result.context_omission = context_omission;
        if body.is_some() && !progress_valid {
            result.progress_omission = Some(if key_matches {
                StylesheetEvidenceOmission::InconsistentProgress
            } else {
                StylesheetEvidenceOmission::UnauthenticatedEvent
            });
        }
        let status = result
            .response
            .response()
            .and_then(NetworkResponseInfo::status_code);
        self.page.finish_stylesheet_fetch(slot, result);
        self.page.mark_css_aborted(slot);
        self.update_loading();
        if self.document_load.failure().is_none() {
            self.last_status = Some(super::status::format_network_error(
                "stylesheet",
                "",
                kind,
                status,
                "",
            ));
        }
        self.poke_redraw();
    }
    pub(super) fn on_css_decoded_block(
        &mut self,
        slot: StylesheetSlotId,
        url: &str,
        css_block: String,
        request: RequestId,
    ) {
        if !self.accepts_css_runtime_event(slot, request) {
            return;
        }
        let result = self
            .page
            .stylesheet_fetch_result(slot)
            .expect("Ready evidence");
        if result
            .request
            .issued_key()
            .is_none_or(|key| key.as_str() != url)
        {
            self.fail_css_installation(
                slot,
                StylesheetInstallationFailure::RuntimeProvenanceMismatch,
            );
            return;
        }
        if matches!(
            self.stylesheet_loads[&slot].phase,
            StylesheetLoadPhase::AwaitingRuntime {
                progress: CssRuntimeProgress::BlockReceived,
                ..
            }
        ) {
            return;
        }
        let response = result
            .response
            .response()
            .expect("selected Ready response")
            .clone();
        let work = self.page.apply_css_block(slot, &css_block, &response);
        let StylesheetLoadPhase::AwaitingRuntime { progress, .. } = &mut self
            .stylesheet_loads
            .get_mut(&slot)
            .expect("runtime admission")
            .phase
        else {
            unreachable!()
        };
        *progress = CssRuntimeProgress::BlockReceived;
        self.request_optional_render_work(work);
    }
    pub(super) fn on_css_sheet_done(
        &mut self,
        slot: StylesheetSlotId,
        url: &str,
        request: RequestId,
    ) {
        if !self.accepts_css_runtime_event(slot, request) {
            return;
        }
        let result = self
            .page
            .stylesheet_fetch_result(slot)
            .expect("Ready evidence");
        if result
            .request
            .issued_key()
            .is_none_or(|key| key.as_str() != url)
        {
            self.fail_css_installation(
                slot,
                StylesheetInstallationFailure::RuntimeProvenanceMismatch,
            );
            return;
        }
        if !matches!(
            self.stylesheet_loads[&slot].phase,
            StylesheetLoadPhase::AwaitingRuntime {
                progress: CssRuntimeProgress::BlockReceived,
                ..
            }
        ) {
            self.fail_css_installation(slot, StylesheetInstallationFailure::UnexpectedRuntimeEvent);
            return;
        }
        self.stylesheet_loads.remove(&slot);
        self.page.finish_stylesheet_installation(slot, Ok(()));
        self.page.mark_css_done(slot);
        let settlement = self.update_loading();
        if self.document_load.failure().is_none() {
            self.last_status =
                Some(
                    if settlement
                        == super::load_observation::StaticInputSettlement::SettledInputsOnly
                    {
                        "Stylesheet work settled"
                    } else {
                        "Stylesheet completed"
                    }
                    .into(),
                );
        }
        self.poke_redraw();
    }
}
