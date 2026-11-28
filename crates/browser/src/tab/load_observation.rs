//! Derived current-input evidence. Never schedules, installs, renders or repairs
//! state. Mechanical settlement is not response eligibility or page assembly.
use super::Tab;
use super::state::{
    CssRuntimeProgress, DocumentLoadFailure, DocumentParseStatus, LoadStateInvariant,
    StylesheetLoadPhase,
};
use crate::document_style::{CssRuntimeReceipt, StylesheetDecodingOutcome};
use crate::stylesheet_fetch::{
    ATTACHMENTS, BODY_BYTES, BODY_POOL, METADATA_BYTES, METADATA_POOL, SOURCE_BYTES,
    StylesheetFetchFailure, StylesheetFetchOutcome, StylesheetResponsePolicy, metadata_charge,
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
        self.static_input_settlement_checked(false)
    }
    pub(super) fn validate_static_stylesheets(&self) -> Result<(), DocumentLoadFailure> {
        self.static_input_settlement_checked(true).map(|_| ())
    }
    fn static_input_settlement_checked(
        &self,
        validate_failed: bool,
    ) -> Result<StaticInputSettlement, DocumentLoadFailure> {
        #[cfg(test)]
        QUERY_WORK.with(|work| {
            let (calls, visits) = work.get();
            work.set((calls + 1, visits));
        });
        if !validate_failed && self.document_load.failure().is_some() {
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
        let unavailable = false;
        let mut matched_requests = 0usize;
        let mut source_bytes = 0usize;
        let mut metadata_bytes = 0usize;
        let mut body_bytes = 0usize;
        let slots = self.page.stylesheet_lifecycle_slots();
        let slot_count = slots.len();
        if slot_count > ATTACHMENTS {
            return Err(self.load_invariant(LoadStateInvariant::RetentionAccounting, None));
        }
        for slot in slots {
            #[cfg(test)]
            QUERY_WORK.with(|work| {
                let (calls, visits) = work.get();
                work.set((calls, visits + 1));
            });
            let id = slot.slot_id();
            let live = self.stylesheet_loads.get(&id);
            let result = slot.fetch_result();
            if slot.body_provisional() {
                return Err(self.load_invariant(LoadStateInvariant::RetentionAccounting, Some(id)));
            }
            source_bytes = source_bytes
                .checked_add(slot.source_payload_charge().ok_or_else(|| {
                    self.load_invariant(LoadStateInvariant::RetentionAccounting, Some(id))
                })?)
                .filter(|n| *n <= SOURCE_BYTES)
                .ok_or_else(|| {
                    self.load_invariant(LoadStateInvariant::RetentionAccounting, Some(id))
                })?;

            if slot.metadata_reserved() {
                metadata_bytes = metadata_bytes
                    .checked_add(METADATA_BYTES)
                    .filter(|n| *n <= METADATA_POOL)
                    .ok_or_else(|| {
                        self.load_invariant(LoadStateInvariant::RetentionAccounting, Some(id))
                    })?;
            }
            if slot.body_issued() {
                body_bytes = body_bytes
                    .checked_add(BODY_BYTES)
                    .filter(|n| *n <= BODY_POOL)
                    .ok_or_else(|| {
                        self.load_invariant(LoadStateInvariant::RetentionAccounting, Some(id))
                    })?;
            }
            if slot.parse().is_some_and(|p| {
                p.completion() == css::StylesheetParseCompletion::InvariantViolation
            }) {
                return Err(self.load_invariant(LoadStateInvariant::SlotPhase, Some(id)));
            }
            if !slot.is_external() {
                if !(slot.has_loaded_parse()
                    || slot.is_not_requested() && !slot.host().parses_inline())
                    || live.is_some()
                    || result.is_some()
                    || slot.metadata_reserved()
                    || slot.body_issued()
                    || slot.installation_result().is_some()
                    || slot.decoding().is_some()
                    || slot.runtime_receipt().is_some()
                {
                    return Err(self.load_invariant(LoadStateInvariant::SlotPhase, Some(id)));
                }
                continue;
            }
            if slot.is_not_requested() {
                if slot.host().participates()
                    || live.is_some()
                    || result.is_some()
                    || slot.metadata_reserved()
                    || slot.body_issued()
                    || slot.resource_base().is_some()
                    || slot.decoding().is_some()
                    || slot.runtime_receipt().is_some()
                    || slot.installation_result().is_some()
                {
                    return Err(self.load_invariant(LoadStateInvariant::SlotPhase, Some(id)));
                }
                continue;
            }
            if let Some(outcome) = slot.decoding() {
                if !outcome.coherent_report() {
                    return Err(self.load_invariant(LoadStateInvariant::FetchEvidence, Some(id)));
                }
                let Some(result) = result.filter(|r| r.is_ready()) else {
                    return Err(self.load_invariant(LoadStateInvariant::FetchEvidence, Some(id)));
                };
                let crate::stylesheet_fetch::StylesheetBodyEvidence::Complete(bytes) = &result.body
                else {
                    unreachable!()
                };
                let charset = match result
                    .response
                    .response()
                    .expect("Ready response")
                    .content_type()
                {
                    net::ResponseContentType::Parsed(t) => t.charset(),
                    _ => None,
                };
                if runtime_css::decoding::select_stylesheet_decoding(
                    bytes,
                    charset,
                    self.page.stylesheet_environment(id),
                ) != Ok(outcome.decision())
                {
                    return Err(self.load_invariant(LoadStateInvariant::FetchEvidence, Some(id)));
                }
                if matches!(outcome, StylesheetDecodingOutcome::Rejected(d) if d.effective_encoding().is_utf8())
                    || (!matches!(outcome, StylesheetDecodingOutcome::Rejected(_))
                        && !outcome.decision().effective_encoding().is_utf8())
                {
                    return Err(self.load_invariant(LoadStateInvariant::FetchEvidence, Some(id)));
                }
            }
            // A response base is installed only together with an accepted parse.
            // It may survive retraction of that parse after a protocol abort, but
            // selection or a failed decode never installs one.
            if slot.resource_base().is_some()
                && !slot
                    .decoding()
                    .is_some_and(StylesheetDecodingOutcome::succeeded)
            {
                return Err(self.load_invariant(LoadStateInvariant::RequestProvenance, Some(id)));
            }
            if let Some(parse) = slot.parse() {
                let Some(StylesheetDecodingOutcome::Succeeded {
                    report: bus::css_decode::CssDecodeReport::Finalized(report),
                    ..
                }) = slot.decoding()
                else {
                    return Err(self.load_invariant(LoadStateInvariant::FetchEvidence, Some(id)));
                };
                let retained = parse.input.len_bytes();
                let emitted = report.emitted_utf8_bytes();
                if retained != parse.stats.input_bytes
                    || retained > emitted
                    || (parse.completion() == css::StylesheetParseCompletion::Complete
                        && retained != emitted)
                {
                    return Err(self.load_invariant(LoadStateInvariant::FetchEvidence, Some(id)));
                }
            }
            if let Some(result) = result {
                if !result.coherent_evidence() {
                    return Err(self.load_invariant(LoadStateInvariant::FetchEvidence, Some(id)));
                }
                if let StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Source(failure)) =
                    result.outcome
                {
                    if slot.source_failure() != Some(failure) {
                        return Err(
                            self.load_invariant(LoadStateInvariant::RequestProvenance, Some(id))
                        );
                    }
                } else if slot.source_failure().is_some() {
                    return Err(
                        self.load_invariant(LoadStateInvariant::RequestProvenance, Some(id))
                    );
                }
                if slot.resource_base().is_some_and(|base| {
                    !result.is_ready()
                        || result
                            .response
                            .response()
                            .and_then(net::NetworkResponseInfo::response_url)
                            != Some(base)
                }) || (slot.has_loaded_parse() && slot.resource_base().is_none())
                {
                    return Err(
                        self.load_invariant(LoadStateInvariant::RequestProvenance, Some(id))
                    );
                }

                if result.request.request().is_some() != slot.metadata_reserved() {
                    return Err(
                        self.load_invariant(LoadStateInvariant::RetentionAccounting, Some(id))
                    );
                }
                if result.request.enqueued() != slot.body_issued() {
                    return Err(
                        self.load_invariant(LoadStateInvariant::RetentionAccounting, Some(id))
                    );
                }
                if let Some(request) = result.request.request() {
                    if metadata_charge(
                        request,
                        result.request.origin_allowance(),
                        result.response.response(),
                    )
                    .is_none_or(|n| n > METADATA_BYTES)
                    {
                        return Err(
                            self.load_invariant(LoadStateInvariant::RetentionAccounting, Some(id))
                        );
                    }
                    if slot.selected_address().is_none_or(|a| {
                        a.kind() != request.address().kind() || a.url() != request.original_url()
                    }) {
                        return Err(
                            self.load_invariant(LoadStateInvariant::RequestProvenance, Some(id))
                        );
                    }
                    if request.request_id() != self.active_document_request_id().unwrap_or_default()
                    {
                        return Err(
                            self.load_invariant(LoadStateInvariant::RequestGeneration, Some(id))
                        );
                    }
                }
            }
            if slot.is_failed() || slot.is_aborted() {
                use crate::stylesheet_fetch::StylesheetInstallationFailure as I;
                let outcome = slot.decoding();
                let receipt = slot.runtime_receipt();
                let terminal_valid = result.is_some_and(|result| match result.outcome {
                    StylesheetFetchOutcome::Ready => match slot.installation_result() {
                        None => matches!(outcome, Some(StylesheetDecodingOutcome::Rejected(_))) && receipt.is_none() && slot.resource_base().is_none(),
                        Some(Ok(())) => false,
                        Some(Err(I::Decode(f))) => matches!(outcome, Some(StylesheetDecodingOutcome::Failed { failure, .. }) if failure == f) && receipt == Some(CssRuntimeReceipt::Done) && slot.resource_base().is_none(),
                        Some(Err(I::Runtime(f))) => matches!(outcome, Some(StylesheetDecodingOutcome::Selected(_))) && receipt == Some(CssRuntimeReceipt::Failed(f)) && slot.resource_base().is_none(),
                        Some(Err(I::ParserIntegrity)) => self.document_load.failure().is_some() && outcome.is_some_and(StylesheetDecodingOutcome::succeeded) && receipt.is_none() && slot.resource_base().is_none(),
                        Some(Err(I::InvalidDecodeEvidence(_))) => outcome.is_none_or(|o| matches!(o, StylesheetDecodingOutcome::Selected(_))) && receipt.is_none() && slot.resource_base().is_none(),
                        Some(Err(I::HandoffAllocation | I::ChunkDeliveryUnavailable | I::DoneDeliveryUnavailable | I::UnexpectedRuntimeEvent)) => matches!(outcome, Some(StylesheetDecodingOutcome::Selected(_))) && receipt.is_none() && slot.resource_base().is_none(),
                        Some(Err(I::RuntimeProvenanceMismatch)) => receipt.is_none() && match outcome {
                            Some(StylesheetDecodingOutcome::Selected(_) | StylesheetDecodingOutcome::Failed { .. }) => slot.resource_base().is_none(),
                            Some(StylesheetDecodingOutcome::Succeeded { .. }) => slot.resource_base().is_some() && slot.resource_base() == result.response.response().and_then(net::NetworkResponseInfo::response_url),
                            _ => false,
                        },
                    },
                    StylesheetFetchOutcome::Rejected(_) | StylesheetFetchOutcome::Failed(_) => slot.installation_result().is_none() && outcome.is_none() && receipt.is_none() && slot.resource_base().is_none(),
                });
                if live.is_some() || !terminal_valid {
                    return Err(self.load_invariant(LoadStateInvariant::SlotPhase, Some(id)));
                }
                continue;
            }
            if slot.selected_address().is_none() {
                return Err(self.load_invariant(LoadStateInvariant::SlotPhase, Some(id)));
            }
            match live {
                Some(load) => {
                    matched_requests += 1;
                    if Some(load.request_id) != self.active_document_request_id() {
                        return Err(
                            self.load_invariant(LoadStateInvariant::RequestGeneration, Some(id))
                        );
                    }
                    if !slot.body_issued() || !slot.metadata_reserved() {
                        return Err(
                            self.load_invariant(LoadStateInvariant::RetentionAccounting, Some(id))
                        );
                    }
                    match &load.phase {
                        StylesheetLoadPhase::Fetching(a) => {
                            if a.request.request_id() != load.request_id {
                                return Err(self.load_invariant(
                                    LoadStateInvariant::RequestGeneration,
                                    Some(id),
                                ));
                            }
                            if metadata_charge(&a.request, a.origin_allowance, a.response.as_ref())
                                .is_none_or(|n| n > METADATA_BYTES)
                                || a.bytes.len() != a.observed_bytes
                                || a.observed_bytes > BODY_BYTES
                            {
                                return Err(self.load_invariant(
                                    LoadStateInvariant::RetentionAccounting,
                                    Some(id),
                                ));
                            }
                            if slot.resource_base().is_some()
                                || a.response
                                    .as_ref()
                                    .and_then(net::NetworkResponseInfo::status)
                                    .is_some_and(|status| status.has_null_body())
                                    && a.observed_bytes != 0
                                || a.response.is_none()
                                    && (a.observed_bytes != 0 || !a.bytes.is_empty())
                                || a.policy
                                    != a.response.as_ref().map(|response| {
                                        StylesheetResponsePolicy::classify(response, &a.request)
                                    })
                            {
                                return Err(self
                                    .load_invariant(LoadStateInvariant::FetchEvidence, Some(id)));
                            }
                            if result.is_some()
                                || !slot.is_pending()
                                || slot.installation_result().is_some()
                                || slot.decoding().is_some()
                                || slot.runtime_receipt().is_some()
                            {
                                return Err(
                                    self.load_invariant(LoadStateInvariant::SlotPhase, Some(id))
                                );
                            }
                            if slot.selected_address().is_none_or(|address| {
                                address.kind() != a.request.address().kind()
                                    || address.url() != a.request.original_url()
                            }) || a.response.as_ref().is_some_and(|r| !a.identity_matches(r))
                            {
                                return Err(self.load_invariant(
                                    LoadStateInvariant::RequestProvenance,
                                    Some(id),
                                ));
                            }
                        }
                        StylesheetLoadPhase::AwaitingRuntime {
                            progress,
                            abort_url,
                        } => {
                            if (*progress != CssRuntimeProgress::ParsedAwaitingDone
                                && slot.resource_base().is_some())
                                || result
                                    .and_then(|r| r.request.issued_key())
                                    .is_none_or(|key| key.as_str() != abort_url)
                            {
                                return Err(self.load_invariant(
                                    LoadStateInvariant::RequestProvenance,
                                    Some(id),
                                ));
                            }
                            if (*progress != CssRuntimeProgress::ParsedAwaitingDone
                                && slot.resource_base().is_some())
                                || !result.is_some_and(|r| r.is_ready())
                                || slot.has_loaded_parse()
                                    != (*progress == CssRuntimeProgress::ParsedAwaitingDone)
                                || slot.installation_result().is_some()
                                || slot.runtime_receipt().is_some()
                                || !match progress {
                                    CssRuntimeProgress::AwaitingOutcome => matches!(
                                        slot.decoding(),
                                        Some(StylesheetDecodingOutcome::Selected(_))
                                    ),
                                    CssRuntimeProgress::ParsedAwaitingDone => slot
                                        .decoding()
                                        .is_some_and(StylesheetDecodingOutcome::succeeded),
                                    CssRuntimeProgress::DecodeFailedAwaitingDone => {
                                        slot.decoding()
                                            .is_some_and(StylesheetDecodingOutcome::failed)
                                            && slot.is_pending()
                                    }
                                }
                            {
                                return Err(
                                    self.load_invariant(LoadStateInvariant::SlotPhase, Some(id))
                                );
                            }
                        }
                    }
                    pending = true;
                }
                None => {
                    if !slot.has_loaded_parse()
                        || slot.installation_result() != Some(Ok(()))
                        || slot.runtime_receipt() != Some(CssRuntimeReceipt::Done)
                        || !slot
                            .decoding()
                            .is_some_and(StylesheetDecodingOutcome::succeeded)
                        || !result.is_some_and(|r| r.is_ready())
                    {
                        return Err(
                            self.load_invariant(LoadStateInvariant::SlotWithoutRequest, Some(id))
                        );
                    }
                }
            }
        }
        if matched_requests != self.stylesheet_loads.len() {
            return Err(self.load_invariant(LoadStateInvariant::RequestWithoutSlot, None));
        }
        let (actual_source, actual_metadata, actual_body) = self.page.stylesheet_retention_usage();
        if actual_source != source_bytes
            || actual_metadata != metadata_bytes
            || actual_body != body_bytes
            || metadata_bytes > METADATA_POOL
            || body_bytes > BODY_POOL
        {
            return Err(self.load_invariant(LoadStateInvariant::RetentionAccounting, None));
        }
        if self.document_load.failure().is_some() {
            return Ok(StaticInputSettlement::Failed);
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
            let response_base = match slot.fetch_result().and_then(|r| r.response.response()) {
                Some(response) => match response.response_url() {
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
                None => "not-established",
            };
            if ordinal >= MAX_SNAPSHOT_SHEETS || ordinal != written {
                continue;
            }
            let state = if slot.is_not_requested() {
                "not-requested"
            } else if slot.is_pending() {
                "pending"
            } else if slot.has_loaded_parse() {
                "loaded"
            } else if slot.is_failed() {
                "failed"
            } else {
                "aborted"
            };
            let phase = match request.map(|load| &load.phase) {
                Some(StylesheetLoadPhase::Fetching(a)) if a.response.is_none() => "awaiting-start",
                Some(StylesheetLoadPhase::Fetching(_)) => "receiving",
                Some(StylesheetLoadPhase::AwaitingRuntime {
                    progress: CssRuntimeProgress::AwaitingOutcome,
                    ..
                }) => "awaiting-block",
                Some(StylesheetLoadPhase::AwaitingRuntime {
                    progress: CssRuntimeProgress::ParsedAwaitingDone,
                    ..
                }) => "awaiting-runtime-completion",
                None if slot.installation_result() == Some(Ok(())) => "complete",
                Some(StylesheetLoadPhase::AwaitingRuntime {
                    progress: CssRuntimeProgress::DecodeFailedAwaitingDone,
                    ..
                }) => "decode-failed-awaiting-done",
                None => "not-issued-or-retired",
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
