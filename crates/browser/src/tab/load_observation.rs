//! Derived current-input evidence. Never schedules, installs, renders or repairs
//! state. Mechanical settlement is not response eligibility or page assembly.
use super::Tab;
use super::state::{
    CssRuntimeProgress, DocumentLoadFailure, DocumentParseStatus, LoadStateInvariant,
    StylesheetLoadPhase,
};
use core_types::StylesheetSlotId;
use std::fmt::{self, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StaticInputSettlement {
    Idle,
    Pending,
    EvidenceUnavailable,
    Failed,
    SettledInputsOnly,
}

pub(super) const MAX_SNAPSHOT_SHEETS: usize = 64;
pub(super) const MAX_SNAPSHOT_BYTES: usize = 16 * 1024;
const MAX_SNAPSHOT_REDIRECTS: usize = 20;

#[cfg(test)]
thread_local! {
    // Test-only work accounting; never retained engine evidence or a cache.
    pub(super) static QUERY_WORK: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

impl Tab {
    fn load_invariant(
        &self,
        relationship: LoadStateInvariant,
        slot: Option<StylesheetSlotId>,
    ) -> DocumentLoadFailure {
        DocumentLoadFailure::LoadStateInvariantViolation {
            relationship,
            slot,
            slot_records: self.page.stylesheet_lifecycle_slots().len(),
            request_records: self.stylesheet_loads.len(),
        }
    }

    pub(super) fn static_input_settlement(
        &self,
    ) -> Result<StaticInputSettlement, DocumentLoadFailure> {
        #[cfg(test)]
        QUERY_WORK.with(|work| {
            let (calls, visits) = work.get();
            work.set((calls + 1, visits));
        });
        if self.document_load.failure().is_some() {
            return Ok(StaticInputSettlement::Failed);
        }
        let response = self.document_load.response.as_ref();
        if let Some(body) = self.document_load.body_completion {
            if response.is_none() {
                return Err(self.load_invariant(LoadStateInvariant::BodyWithoutAdmission, None));
            }
            if !body.matches_route(core_types::ResourceKind::Html)
                || body.delivered_bytes() != self.document_load.bytes_received
            {
                return Err(self.load_invariant(LoadStateInvariant::BodyAccounting, None));
            }
        }
        let committed = self.dom_handle.is_some()
            && self.page.dom.is_some()
            && self.page.document_mode.is_some()
            && self.page.addresses.is_some();
        if response.is_some()
            && (self.dom_handle.is_some() != self.page.dom.is_some()
                || (self.page.dom.is_some() && !committed))
        {
            return Err(self.load_invariant(LoadStateInvariant::CommittedDocument, None));
        }
        if self.document_load.parse_status == DocumentParseStatus::Complete
            && (response.is_none()
                || self.document_load.body_completion.is_none()
                || !committed
                || !self.document_load.has_finalized_diagnostics()
                || !self
                    .document_load
                    .decode_report
                    .is_some_and(|r| r.eof_processed))
        {
            return Err(self.load_invariant(LoadStateInvariant::ParserCompletion, None));
        }

        let mut pending = false;
        let mut unavailable = false;
        let mut matched_requests = 0;
        let slots = self.page.stylesheet_lifecycle_slots();
        let slot_count = slots.len();
        // Examine all relationships before returning Pending: earlier ordinary
        // work must not hide a later invariant violation. One expected O(1)
        // request lookup per slot; no registry copy or URL/key reconstruction.
        for slot in slots {
            #[cfg(test)]
            QUERY_WORK.with(|work| {
                let (calls, visits) = work.get();
                work.set((calls, visits + 1));
            });
            let id = slot.slot_id();
            let load = self.stylesheet_loads.get(&id);
            if !slot.is_external() {
                if !slot.has_loaded_parse() || load.is_some() {
                    return Err(self.load_invariant(LoadStateInvariant::SlotPhase, Some(id)));
                }
                continue;
            }
            if slot.is_failed() || slot.is_aborted() {
                if load.is_some() {
                    return Err(self.load_invariant(LoadStateInvariant::SlotPhase, Some(id)));
                }
                continue;
            }
            if slot.selected_address().is_none() {
                return Err(self.load_invariant(LoadStateInvariant::SlotPhase, Some(id)));
            }
            let Some(load) = load else {
                return Err(self.load_invariant(LoadStateInvariant::SlotWithoutRequest, Some(id)));
            };
            matched_requests += 1;
            if Some(load.request_id) != self.active_document_request_id() {
                return Err(self.load_invariant(LoadStateInvariant::RequestGeneration, Some(id)));
            }
            if let Some(accepted) = load.phase.accepted_response()
                && accepted.requested_url() != load.issued_key.as_str()
            {
                return Err(self.load_invariant(LoadStateInvariant::RequestProvenance, Some(id)));
            }
            match &load.phase {
                StylesheetLoadPhase::AwaitingStart | StylesheetLoadPhase::Receiving { .. } => {
                    if !slot.is_pending() {
                        return Err(self.load_invariant(LoadStateInvariant::SlotPhase, Some(id)));
                    }
                    pending = true;
                }
                StylesheetLoadPhase::AwaitingRuntime {
                    progress,
                    accept_body,
                    ..
                } => {
                    let installed = *progress == CssRuntimeProgress::BlockReceived;
                    if slot.has_loaded_parse() != installed || (installed && !accept_body) {
                        return Err(self.load_invariant(LoadStateInvariant::SlotPhase, Some(id)));
                    }
                    pending = true;
                }
                StylesheetLoadPhase::Complete { response } => {
                    if !slot.has_loaded_parse() {
                        return Err(self.load_invariant(LoadStateInvariant::SlotPhase, Some(id)));
                    }
                    // Existing CSS admission permits these provenance gaps.
                    // Observe the limitation; never retract usable preview CSS.
                    unavailable |= !matches!(response.response_url(), Some(Ok(_)));
                }
            }
        }
        if matched_requests != self.stylesheet_loads.len() {
            return Err(self.load_invariant(LoadStateInvariant::RequestWithoutSlot, None));
        }
        if response.is_none()
            && self.history.is_empty()
            && self.page.dom.is_none()
            && self.dom_handle.is_none()
            && self.page.addresses.is_none()
            && slot_count == 0
        {
            return Ok(StaticInputSettlement::Idle);
        }
        if pending
            || (self.document_load.parse_status == DocumentParseStatus::Pending
                && (response.is_some() || !self.history.is_empty()))
        {
            return Ok(StaticInputSettlement::Pending);
        }
        if unavailable || !committed || response.is_none() {
            return Ok(StaticInputSettlement::EvidenceUnavailable);
        }
        Ok(StaticInputSettlement::SettledInputsOnly)
    }

    fn document_load_debug_snapshot(&self) -> String {
        let mut output = String::new();
        writeln!(output, "DocumentLoad {{").unwrap();
        writeln!(output, "  state: {:?}", self.document_load).unwrap();
        let settlement = self.static_input_settlement();
        match &settlement {
            Ok(settlement) => writeln!(output, "  inputs: {settlement:?}").unwrap(),
            Err(failure) => writeln!(output, "  inputs: invariant-error({failure:?})").unwrap(),
        }
        let eligibility =
            if self.active_document_request().is_some() && self.document_load.response.is_some() {
                "admitted-static-html-subset"
            } else {
                "not-established"
            };
        writeln!(output, "  response_installation_eligibility: {eligibility}").unwrap();
        output.push_str("  full_page_assembly: not-certified\n");
        write_request_summary(
            &mut output,
            "active_request",
            self.active_document_request(),
        );
        if let Some(attempt) = &self.navigation_attempt {
            let state = match &attempt.terminal {
                Some(Ok(_)) if self.active_document_request_id().is_some() => {
                    "preserved-active-document"
                }
                Some(Ok(_)) => "completed-without-document",
                Some(Err(_)) => "failed",
                None if attempt.non_replacing_response.is_some() => "awaiting-non-replacing-body",
                None => "awaiting-response",
            };
            writeln!(output, "  navigation: {state}").unwrap();
            write_request_summary(&mut output, "navigation_request", Some(&attempt.request));
            write_response_summary(
                &mut output,
                "non_replacing_response",
                attempt.non_replacing_response.as_ref(),
            );
            if let Some(Err(failure)) = &attempt.terminal {
                writeln!(output, "  navigation_failure: {failure:?}").unwrap();
                if matches!(
                    failure,
                    DocumentLoadFailure::ResponseRejected {
                        reason: super::state::DocumentResponseRejection::UnsupportedDocumentCharset,
                        ..
                    }
                ) {
                    output.push_str("  navigation_encoding: rejected-unsupported-charset source=response-charset decode=not-started\n");
                }
                write_response_summary(
                    &mut output,
                    "navigation_failure_context",
                    failure_response(failure),
                );
                if let DocumentLoadFailure::Network { body_progress, .. } = failure {
                    writeln!(output, "  navigation_failure_progress: {body_progress:?}").unwrap();
                }
            }
        }
        writeln!(
            output,
            "  encoding_decision: {:?}",
            self.document_load.encoding_decision
        )
        .unwrap();
        output.push_str("  encoding_policy: utf-8-only bom=retained-as-scalar meta=not-scanned\n");
        match self.document_load.decode_report {
            Some(report) => {
                let counts = report.replacements;
                let total = counts.total();
                let outcome = if !report.eof_processed {
                    "prefix-only"
                } else if total == 0 {
                    "complete-clean"
                } else {
                    "complete-with-replacements"
                };
                writeln!(output,
                    "  decode: {outcome} invalid_sequences={} incomplete_sequences_at_eof={} total_replacements={total}",
                    counts.invalid_sequences, counts.incomplete_sequences_at_eof).unwrap();
            }
            None => {
                let evidence = if self.document_load.failure().is_some() {
                    "unavailable"
                } else {
                    "not-reported"
                };
                writeln!(output, "  decode: {evidence}").unwrap();
            }
        }
        let response = self.document_load.response.as_ref();
        write_response_summary(&mut output, "admitted_response", response);
        let context = self.document_load.failure().and_then(failure_response);
        write_response_summary(&mut output, "failure_response_context", context);
        let failure = self
            .document_load
            .failure()
            .or_else(|| settlement.as_ref().err());
        if let Some(DocumentLoadFailure::Network { body_progress, .. }) = failure {
            match body_progress {
                Some(body) => writeln!(
                    output,
                    "  failure_body_progress: route={:?} delivered_bytes={}",
                    body.route(),
                    body.delivered_bytes()
                )
                .unwrap(),
                None => output.push_str("  failure_body_progress: absent\n"),
            }
        }
        let offending_slot = match failure {
            Some(DocumentLoadFailure::LoadStateInvariantViolation { slot, .. }) => *slot,
            _ => None,
        };
        writeln!(
            output,
            "  committed: {}",
            self.dom_handle.is_some() && self.page.dom.is_some()
        )
        .unwrap();
        if let Some(addresses) = &self.page.addresses {
            writeln!(
                output,
                "  address: present origin={} base={}",
                if addresses.document_origin().is_opaque() {
                    "opaque"
                } else {
                    "tuple"
                },
                if addresses.selected_key().is_some() {
                    "selected-frozen"
                } else {
                    "fallback"
                }
            )
            .unwrap();
        } else {
            output.push_str("  address: absent\n");
        }
        writeln!(
            output,
            "  render_work_pending: {}",
            !self.pending_render_work.is_empty()
        )
        .unwrap();
        output.push_str("  accepted_rendering: requires-checked-frame\n");
        let slots = self.page.stylesheet_lifecycle_slots();
        let total = slots.len();
        let mut written = 0;
        let mut offending_ordinal = None;
        let mut missing_response_bases = 0;
        let mut invalid_response_bases = 0;
        // Inspect the entire authoritative order even when rows are omitted.
        // Correlation and unavailable-evidence counts need no separate registry.
        for (ordinal, slot) in slots.enumerate() {
            if offending_slot == Some(slot.slot_id()) {
                offending_ordinal = Some(ordinal);
            }
            let request = self.stylesheet_loads.get(&slot.slot_id());
            let response_base = match request.map(|load| &load.phase) {
                Some(StylesheetLoadPhase::Complete { response }) => match response.response_url() {
                    None => {
                        missing_response_bases += 1;
                        "missing"
                    }
                    Some(Err(_)) => {
                        invalid_response_bases += 1;
                        "invalid"
                    }
                    Some(Ok(_)) => "parsed",
                },
                _ => "not-established",
            };
            if ordinal >= MAX_SNAPSHOT_SHEETS || ordinal != written {
                continue;
            }
            let state = if slot.is_pending() {
                "pending"
            } else if slot.has_loaded_parse() {
                "loaded"
            } else if slot.is_failed() {
                "failed"
            } else {
                "aborted"
            };
            let phase = match request.map(|load| &load.phase) {
                None => "not-issued-or-retired",
                Some(StylesheetLoadPhase::AwaitingStart) => "awaiting-start",
                Some(StylesheetLoadPhase::Receiving { .. }) => "receiving",
                Some(StylesheetLoadPhase::AwaitingRuntime {
                    progress: CssRuntimeProgress::AwaitingBlock,
                    ..
                }) => "awaiting-block",
                Some(StylesheetLoadPhase::AwaitingRuntime {
                    progress: CssRuntimeProgress::BlockReceived,
                    ..
                }) => "awaiting-runtime-completion",
                Some(StylesheetLoadPhase::AwaitingRuntime { .. }) => "awaiting-runtime-no-block",
                Some(StylesheetLoadPhase::Complete { .. }) => "complete",
            };
            let row = format!(
                "  sheet[{ordinal}]: source={} address={} state={state} request={phase} response_base={response_base}\n",
                if slot.is_external() {
                    "external"
                } else {
                    "inline"
                },
                if slot.selected_address().is_some() {
                    "present"
                } else {
                    "absent"
                }
            );
            // Reserve bounded base counts, invariant correlation, omitted-row
            // marker and terminator, including maximum-width numeric fields.
            if output.len() + row.len() + 512 > MAX_SNAPSHOT_BYTES {
                continue;
            }
            output.push_str(&row);
            written += 1;
        }
        if missing_response_bases != 0 || invalid_response_bases != 0 {
            writeln!(
                output,
                "  stylesheet_response_base_unavailable: missing={missing_response_bases} invalid={invalid_response_bases}"
            )
            .unwrap();
        }
        if let Some(DocumentLoadFailure::LoadStateInvariantViolation { slot, .. }) = failure {
            match (slot, offending_ordinal) {
                (Some(_), Some(ordinal)) => writeln!(
                    output,
                    "  invariant_slot: sheet[{ordinal}] row={}",
                    if ordinal < written {
                        "included"
                    } else {
                        "omitted"
                    }
                )
                .unwrap(),
                (Some(_), None) => {
                    output.push_str("  invariant_slot: absent-from-current-order\n");
                }
                (None, _) => output.push_str("  invariant_slot: not-specified\n"),
            }
        }
        writeln!(output, "  sheets_omitted: {}", total - written).unwrap();
        output.push_str("}\n");
        debug_assert!(output.len() <= MAX_SNAPSHOT_BYTES);
        output
    }
}

fn write_request_summary(output: &mut String, label: &str, request: Option<&net::FetchRequest>) {
    match request {
        None => writeln!(output, "  {label}: absent").unwrap(),
        Some(request) => writeln!(
            output,
            "  {label}: original=present purpose={:?} method={:?} route=Html",
            request.address().kind(),
            request.method()
        )
        .unwrap(),
    }
}

fn failure_response(failure: &DocumentLoadFailure) -> Option<&net::NetworkResponseInfo> {
    match failure {
        DocumentLoadFailure::Network {
            response_context, ..
        } => response_context.as_ref(),
        DocumentLoadFailure::ResponseRequestMismatch(response)
        | DocumentLoadFailure::ResponseUrlUnavailable(response)
        | DocumentLoadFailure::CompletionMetadataMismatch(response) => Some(response),
        DocumentLoadFailure::UnsupportedDocumentCreation {
            response_context, ..
        }
        | DocumentLoadFailure::ResponseRejected {
            response_context, ..
        } => Some(response_context),
        _ => None,
    }
}

fn write_response_summary(
    output: &mut String,
    label: &str,
    response: Option<&net::NetworkResponseInfo>,
) {
    match response {
        None => writeln!(output, "  {label}: absent").unwrap(),
        Some(response) => {
            let provenance = match response.response_url() {
                None => "missing",
                Some(Err(_)) => "invalid",
                Some(Ok(_)) => "parsed",
            };
            let mime = match response.content_type() {
                net::ResponseContentType::Missing => "missing",
                net::ResponseContentType::Empty => "empty",
                net::ResponseContentType::Unusable => "unusable",
                net::ResponseContentType::Parsed(value) if value.mime_type() == "text/html" => {
                    "parsed-html"
                }
                net::ResponseContentType::Parsed(_) => "parsed-other",
            };
            write!(output, "  {label}: original=present acquired_url={provenance} status={:?} redirects={} mime={mime} disposition={:?} redirect_statuses=[",
                response.status_code(), response.redirect_chain().len(), response.content_disposition()).unwrap();
            for (index, hop) in response
                .redirect_chain()
                .iter()
                .take(MAX_SNAPSHOT_REDIRECTS)
                .enumerate()
            {
                if index != 0 {
                    output.push(',');
                }
                write!(output, "{}", hop.status().code()).unwrap();
            }
            writeln!(
                output,
                "] redirect_statuses_omitted={}",
                response
                    .redirect_chain()
                    .len()
                    .saturating_sub(MAX_SNAPSHOT_REDIRECTS)
            )
            .unwrap();
        }
    }
}

impl fmt::Debug for Tab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.document_load_debug_snapshot())
    }
}
