//! Passive views of existing attachment and live-admission state.
use super::{Tab, state::StylesheetLoadPhase};
#[cfg(any(test, feature = "conformance-observation"))]
use crate::stylesheet_fetch::StylesheetResponsePolicy;
use crate::stylesheet_fetch::{
    StylesheetBodyEvidence, StylesheetErrorContext, StylesheetFetchResult,
    StylesheetInstallationFailure, StylesheetRequestEvidence,
};
use std::fmt::{self, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetFetchObservationError {
    CommittedStateInconsistent,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetFetchSnapshotError {
    OutputAllocation,
}
pub struct StylesheetFetchObservation<'a> {
    pub(super) tab: &'a Tab,
}
#[cfg(any(test, feature = "conformance-observation"))]
pub enum StylesheetLiveAcquisition<'a> {
    Fetching {
        request: &'a net::FetchRequest,
        issued_key: &'a addressing::ResourceIdentityKey,
        response: Option<&'a net::NetworkResponseInfo>,
        policy: Option<StylesheetResponsePolicy>,
        prefix: &'a [u8],
        observed_bytes: usize,
    },
    AwaitingRuntime {
        result: &'a StylesheetFetchResult,
        cleanup_key: &'a str,
        block_received: bool,
    },
}
pub struct StylesheetFetchEntry<'a> {
    tab: &'a Tab,
    id: core_types::StylesheetSlotId,
    order: usize,
    #[cfg(feature = "conformance-observation")]
    owner: html::internal::Id,
}
impl<'a> StylesheetFetchObservation<'a> {
    pub fn entries(&self) -> impl Iterator<Item = StylesheetFetchEntry<'a>> + '_ {
        self.tab
            .page
            .stylesheet_lifecycle_slots()
            .enumerate()
            .filter_map(|(order, s)| {
                s.is_external().then_some(StylesheetFetchEntry {
                    tab: self.tab,
                    id: s.slot_id(),
                    order,
                    #[cfg(feature = "conformance-observation")]
                    owner: s.owner(),
                })
            })
    }
}
impl<'a> StylesheetFetchEntry<'a> {
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn source(&self) -> crate::document_style::StylesheetSourceObservation<'a> {
        self.tab
            .page
            .stylesheet_source_observation(self.id)
            .expect("external attachment view")
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn slot_id(&self) -> core_types::StylesheetSlotId {
        self.id
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn generation(&self) -> Option<core_types::RequestId> {
        self.tab.active_document_request_id()
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn live(&self) -> Option<StylesheetLiveAcquisition<'a>> {
        match &self.tab.stylesheet_loads.get(&self.id)?.phase {
            StylesheetLoadPhase::Fetching(a) => Some(StylesheetLiveAcquisition::Fetching {
                request: &a.request,
                issued_key: &a.issued_key,
                response: a.response.as_ref(),
                policy: a.policy,
                prefix: &a.bytes,
                observed_bytes: a.observed_bytes,
            }),
            StylesheetLoadPhase::AwaitingRuntime {
                progress,
                abort_url,
            } => Some(StylesheetLiveAcquisition::AwaitingRuntime {
                result: self.result()?,
                cleanup_key: abort_url,
                block_received: *progress == super::state::CssRuntimeProgress::BlockReceived,
            }),
        }
    }
    pub fn order(&self) -> usize {
        self.order
    }
    #[cfg(feature = "conformance-observation")]
    pub fn source_node(&self) -> html::internal::Id {
        self.owner
    }
    #[cfg(feature = "conformance-observation")]
    pub fn address(&self) -> Option<&'a addressing::ResourceAddress> {
        self.tab.page.stylesheet_address(self.id)
    }
    pub fn result(&self) -> Option<&'a StylesheetFetchResult> {
        self.tab.page.stylesheet_fetch_result(self.id)
    }
    pub fn request(&self) -> Option<&'a net::FetchRequest> {
        match self.tab.stylesheet_loads.get(&self.id).map(|l| &l.phase) {
            Some(StylesheetLoadPhase::Fetching(a)) => Some(&a.request),
            _ => self.result()?.request().request(),
        }
    }
    pub fn phase(&self) -> &'static str {
        match self.tab.stylesheet_loads.get(&self.id).map(|l| &l.phase) {
            Some(StylesheetLoadPhase::Fetching(a)) if a.response.is_none() => "awaiting-start",
            Some(StylesheetLoadPhase::Fetching(_)) => "receiving",
            Some(StylesheetLoadPhase::AwaitingRuntime { .. }) => "awaiting-runtime",
            None => "terminal",
        }
    }
    pub fn installation(&self) -> Option<Result<(), StylesheetInstallationFailure>> {
        self.tab.page.stylesheet_installation_result(self.id)
    }
}
const SNAPSHOT_BYTES: usize = 16 * 1024;
const SNAPSHOT_FOOTER: usize = 512;
const SNAPSHOT_ROWS: usize = 64;
const SNAPSHOT_REDIRECTS: usize = 32;
const ROW_BYTES: usize = 1024;
#[derive(Clone, Copy)]
struct SnapshotLimits {
    bytes: usize,
    rows: usize,
    redirects: usize,
    scratch: usize,
}
impl SnapshotLimits {
    const DEFAULT: Self = Self {
        bytes: SNAPSHOT_BYTES,
        rows: SNAPSHOT_ROWS,
        redirects: SNAPSHOT_REDIRECTS,
        scratch: ROW_BYTES,
    };
}
struct Row {
    bytes: [u8; ROW_BYTES],
    len: usize,
    limit: usize,
}
impl Row {
    fn new(limit: usize) -> Self {
        Self {
            bytes: [0; ROW_BYTES],
            len: 0,
            limit: limit.min(ROW_BYTES),
        }
    }
    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).expect("formatted UTF-8")
    }
}
impl Write for Row {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self
            .len
            .checked_add(text.len())
            .filter(|end| *end <= self.limit)
            .ok_or(fmt::Error)?;
        self.bytes[self.len..end].copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}
impl StylesheetFetchEntry<'_> {
    fn response(&self) -> Option<&net::NetworkResponseInfo> {
        self.result()
            .and_then(|result| result.response().response())
            .or_else(|| match &self.tab.stylesheet_loads.get(&self.id)?.phase {
                StylesheetLoadPhase::Fetching(a) => a.response.as_ref(),
                _ => None,
            })
    }
    fn write_row(
        &self,
        row: &mut Row,
        redirect_budget: usize,
    ) -> Result<(usize, usize), fmt::Error> {
        write!(
            row,
            "  sheet[{}] owner=local[{}] phase={} request={} installation={:?}",
            self.order(),
            self.order(),
            self.phase(),
            match self.result().map(|r| r.request()) {
                Some(StylesheetRequestEvidence::ConstructedOmitted(_)) => "constructed-omitted",
                Some(r) if r.enqueued() => "issued",
                _ if self.request().is_some() => "constructed",
                _ => "not-constructed",
            },
            self.installation()
        )?;
        if let Some(result) = self.result() {
            write!(
                row,
                " outcome={:?} response={:?} body={} retained_bytes={:?} progress={:?}",
                result.outcome(),
                result.response(),
                result.body().category(),
                result.body().bytes().map(<[u8]>::len),
                result.producer_progress()
            )?;
            if let Some(policy) = result.policy() {
                write!(row, " policy={policy:?}")?;
            }
            if let StylesheetBodyEvidence::Partial {
                observed_bytes,
                unretained_event_bytes,
                stop,
                ..
            } = result.body()
            {
                write!(
                    row,
                    " observed={observed_bytes} unretained_observed={unretained_event_bytes} stop={stop:?}"
                )?;
            }
            if result.acquisition_error().is_some()
                || result.error_context() != StylesheetErrorContext::NotApplicable
            {
                write!(
                    row,
                    " producer_error={:?} context={:?}",
                    result.acquisition_error(),
                    result.error_context()
                )?;
            }
            if result.context_omission().is_some() || result.progress_omission().is_some() {
                write!(
                    row,
                    " omissions={:?}/{:?}",
                    result.context_omission(),
                    result.progress_omission()
                )?;
            }
            if result.discarded_event_reason().is_some() {
                write!(
                    row,
                    " discarded_event_bytes={} discarded_event_reason={:?}",
                    result.discarded_event_bytes(),
                    result.discarded_event_reason()
                )?;
            }
            if let Some(omission) = result.request().omission() {
                write!(row, " request_omission={omission:?}")?;
            }
        } else if let Some(StylesheetLoadPhase::Fetching(a)) = self
            .tab
            .stylesheet_loads
            .get(&self.id)
            .map(|load| &load.phase)
        {
            write!(
                row,
                " admitted_response={} policy={:?} observed={} retained={}",
                a.response.is_some(),
                a.policy,
                a.observed_bytes,
                a.bytes.len()
            )?;
        }
        let mut shown = 0;
        let mut total = 0;
        if let Some(response) = self.response() {
            total = response.redirect_chain().len();
            write!(
                row,
                " status={:?} redirect_statuses=[",
                response.status_code()
            )?;
            for hop in response.redirect_chain().iter().take(redirect_budget) {
                if shown > 0 {
                    row.write_char(',')?;
                }
                write!(row, "{}", hop.status().code())?;
                shown += 1;
            }
            row.write_char(']')?;
        }
        row.write_char('\n')?;
        Ok((shown, total))
    }
}
impl Tab {
    #[cfg(test)]
    pub(super) fn stylesheet_fetch_snapshot_for_tests(
        &self,
        bytes: usize,
        rows: usize,
        redirects: usize,
        scratch: usize,
    ) -> String {
        self.stylesheet_fetch_snapshot_with_limits(SnapshotLimits {
            bytes,
            rows,
            redirects,
            scratch,
        })
        .unwrap()
    }
    pub fn stylesheet_fetch_debug_snapshot(&self) -> Result<String, StylesheetFetchSnapshotError> {
        self.stylesheet_fetch_snapshot_with_limits(SnapshotLimits::DEFAULT)
    }
    fn stylesheet_fetch_snapshot_with_limits(
        &self,
        limits: SnapshotLimits,
    ) -> Result<String, StylesheetFetchSnapshotError> {
        let mut output = String::new();
        output
            .try_reserve_exact(SNAPSHOT_BYTES)
            .map_err(|_| StylesheetFetchSnapshotError::OutputAllocation)?;
        let parsed =
            super::parsed_document_observation::ParsedDocumentObservation::for_fetch_snapshot(self);
        writeln!(
            output,
            "StylesheetFetches {{ phase={:?} tree={:?} address={:?} navigation_pending={}",
            parsed.phase(),
            parsed.tree_scope(),
            parsed.address_scope(),
            parsed.navigation_pending()
        )
        .expect("String");
        let mut shown = 0;
        let mut total = None;
        let mut total_redirects = 0;
        let mut shown_redirects = 0;
        let mut accepted_row_redirects = 0;
        let (mut row_limit, mut byte_limit, mut scratch_limit) = (false, false, false);
        match parsed.stylesheet_fetches() {
            Err(_) => output
                .push_str("  availability=committed-state-inconsistent semantics=unavailable\n"),
            Ok(None) => {
                output.push_str("  availability=no-committed-document semantics=not-applicable\n")
            }
            Ok(Some(_)) if self.static_input_settlement().is_err() => output
                .push_str("  availability=acquisition-state-inconsistent semantics=unavailable\n"),
            Ok(Some(view)) => {
                output.push_str("  availability=inventory semantics=complete\n");
                total = Some(view.entries().count());
                total_redirects = view
                    .entries()
                    .filter_map(|entry| {
                        entry
                            .response()
                            .map(|response| response.redirect_chain().len())
                    })
                    .sum();
                for entry in view.entries() {
                    if shown == limits.rows {
                        row_limit = true;
                        break;
                    }
                    let mut row = Row::new(limits.scratch);
                    let Ok((row_shown, row_total)) =
                        entry.write_row(&mut row, limits.redirects.saturating_sub(shown_redirects))
                    else {
                        scratch_limit = true;
                        break;
                    };
                    if output.len() + row.len > limits.bytes.saturating_sub(SNAPSHOT_FOOTER) {
                        byte_limit = true;
                        break;
                    }
                    // Commit the row and its display accounting atomically.
                    output.push_str(row.as_str());
                    shown += 1;
                    shown_redirects += row_shown;
                    accepted_row_redirects += row_total;
                }
            }
        }
        if let Some(total) = total {
            let redirect_omitted_cap = accepted_row_redirects - shown_redirects;
            let redirect_omitted_rows = total_redirects - accepted_row_redirects;
            let redirect_limit = redirect_omitted_cap > 0;
            writeln!(output,"  shown={shown} total={total} omitted={} format_complete={} row_limit={row_limit} byte_limit={byte_limit} scratch_limit={scratch_limit} redirect_limit={redirect_limit}",total-shown,!(row_limit||byte_limit||scratch_limit||redirect_limit)).expect("String");
            writeln!(output,"  redirects_shown={shown_redirects} redirects_total={total_redirects} redirects_omitted={} redirects_omitted_rows={redirect_omitted_rows} redirects_omitted_cap={redirect_omitted_cap}",total_redirects-shown_redirects).expect("String");
        } else {
            output
                .push_str("  shown=0 total=unavailable omitted=unavailable format_complete=true\n");
        }
        output.push_str("}\n");
        debug_assert!(output.len() <= SNAPSHOT_BYTES);
        Ok(output)
    }
}
