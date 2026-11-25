use std::sync::atomic::AtomicU64;
use std::time::Instant;

use core_types::{DomHandle, DomVersion, RequestId, TabId};
#[cfg(all(test, feature = "parser-failure-injection"))]
use html::internal::{
    ParserFailureInjection, html_parser_with_runtime_diagnostics_and_failure_injection,
};
use html::{DocumentMode, DomPatch, HtmlParseError, HtmlParseOptions, HtmlParser};

pub(crate) static HANDLE_GEN: AtomicU64 = AtomicU64::new(0);

pub(crate) type Key = (TabId, RequestId);

pub(crate) struct RuntimeState {
    pub(crate) total_bytes: usize,
    pub(crate) pending_bytes: usize,
    pub(crate) pending_tokens: usize,
    pub(crate) pending_patch_bytes: usize,
    pub(crate) last_tokens_processed: u64,
    pub(crate) last_emit: Instant,
    pub(crate) logged_large_buffer: bool,
    pub(crate) failed: bool,
    pub(crate) parser: HtmlParser,
    pub(crate) patch_buffer: Vec<DomPatch>,
    pub(crate) patch_buffer_retain: usize,
    pub(crate) max_patch_buffer_len: usize,
    pub(crate) max_patch_buffer_bytes: usize,
    pub(crate) dom_handle: DomHandle,
    pub(crate) version: DomVersion,
    pub(crate) document_mode: Option<DocumentMode>,
}

pub(crate) const PRESELECTION_PATCH_LIMIT: usize = 4096;
pub(crate) const PRESELECTION_BYTE_LIMIT: usize = 4 * 1024 * 1024;

#[cfg(test)]
thread_local! { pub(crate) static DIAGNOSTIC_EXTRACTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

impl RuntimeState {
    pub(crate) fn take_diagnostics(
        &mut self,
    ) -> Option<html::internal::HtmlParserDiagnosticReport> {
        #[cfg(test)]
        DIAGNOSTIC_EXTRACTIONS.with(|n| n.set(n.get() + 1));
        html::internal::take_runtime_parser_diagnostics(&mut self.parser)
    }

    pub(crate) fn new(
        now: Instant,
        patch_buffer_retain: usize,
        dom_handle: DomHandle,
    ) -> Result<Self, HtmlParseError> {
        Ok(Self {
            total_bytes: 0,
            pending_bytes: 0,
            pending_tokens: 0,
            pending_patch_bytes: 0,
            last_tokens_processed: 0,
            last_emit: now,
            logged_large_buffer: false,
            failed: false,
            parser: html::internal::html_parser_with_runtime_diagnostics(runtime_parse_options())?,
            patch_buffer: Vec::new(),
            patch_buffer_retain,
            max_patch_buffer_len: 0,
            max_patch_buffer_bytes: 0,
            dom_handle,
            version: DomVersion::INITIAL,
            document_mode: None,
        })
    }

    #[cfg(all(test, feature = "parser-failure-injection"))]
    pub(crate) fn new_with_failure_injection(
        now: Instant,
        patch_buffer_retain: usize,
        dom_handle: DomHandle,
        injection: ParserFailureInjection,
    ) -> Result<Self, HtmlParseError> {
        let mut state = Self::new(now, patch_buffer_retain, dom_handle)?;
        state.parser = html_parser_with_runtime_diagnostics_and_failure_injection(
            runtime_parse_options(),
            injection,
        )?;
        Ok(state)
    }
}

fn runtime_parse_options() -> HtmlParseOptions {
    let mut options = HtmlParseOptions::default();
    options.error_policy.track = false;
    options.error_policy.max_stored = 0;
    options.error_policy.track_counters = true;
    options
}
