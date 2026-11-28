//! Borrowed AJ8 input evidence. Formatting never invokes CSS execution.
use super::{
    Tab,
    state::{CssRuntimeProgress, StylesheetLoadPhase},
};
use crate::document_style::{StylesheetDecodingOutcome, StylesheetLifecycleView};
use std::fmt::{self, Write};

const BYTES: usize = 16 * 1024;
const ROWS: usize = 64;
const FOOTER: usize = 512;
const DIAGNOSTICS: usize = 128;
const AT_RULES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticStylesheetObservationError {
    Inconsistent,
    OutputAllocation,
}
pub struct StaticStylesheetObservation<'a> {
    tab: &'a Tab,
    committed: bool,
}
pub struct StaticStylesheetEntry<'a> {
    tab: &'a Tab,
    order: usize,
    view: StylesheetLifecycleView<'a>,
}
impl Tab {
    pub fn static_stylesheet_observation(
        &self,
    ) -> Result<StaticStylesheetObservation<'_>, StaticStylesheetObservationError> {
        let committed =
            super::parsed_document_observation::ParsedDocumentObservation::for_fetch_snapshot(self)
                .committed_stylesheet_inputs()
                .map_err(|_| StaticStylesheetObservationError::Inconsistent)?
                .is_some();
        self.validate_static_stylesheets()
            .map_err(|_| StaticStylesheetObservationError::Inconsistent)?;
        if !committed
            && (self.page.stylesheet_slot_count() != 0 || !self.stylesheet_loads.is_empty())
        {
            return Err(StaticStylesheetObservationError::Inconsistent);
        }
        // Validate coordinates for all slots, including rows omitted by formatting.
        for (order, view) in self.page.stylesheet_lifecycle_slots().enumerate() {
            checked_coordinates(order, view.slot_id())
                .map_err(|_| StaticStylesheetObservationError::Inconsistent)?;
        }
        Ok(StaticStylesheetObservation {
            tab: self,
            committed,
        })
    }
}
fn checked_coordinates(
    order: usize,
    slot: core_types::StylesheetSlotId,
) -> Result<(css::StylesheetOrder, css::StylesheetSourceId), css::StylesheetCollectionInputBuildError>
{
    let order = order
        .checked_add(1)
        .ok_or(css::SourceCoordinateError::CounterExhausted {
            coordinate: "browser-stylesheet-order",
        })?;
    Ok((
        css::StylesheetOrder::from_usize(order)?,
        css::StylesheetSourceId::from_browser_slot(slot.0)?,
    ))
}
impl<'a> StaticStylesheetObservation<'a> {
    pub fn has_committed_document(&self) -> bool {
        self.committed
    }
    /// This shares the exact Tab borrow with every entry. Addresses are the
    /// current committed document addresses, never frozen inline sheet bases.
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn document(
        &self,
    ) -> Option<super::parsed_document_observation::ParsedDocumentObservation<'a>> {
        self.committed.then(|| {
            super::parsed_document_observation::ParsedDocumentObservation::for_fetch_snapshot(
                self.tab,
            )
        })
    }
    pub fn entries(&self) -> impl ExactSizeIterator<Item = StaticStylesheetEntry<'a>> + '_ {
        self.tab
            .page
            .stylesheet_lifecycle_slots()
            .enumerate()
            .map(|(order, view)| StaticStylesheetEntry {
                tab: self.tab,
                order,
                view,
            })
    }
    pub fn snapshot(&self) -> Result<String, StaticStylesheetObservationError> {
        self.tab.static_stylesheet_observation()?;
        let mut out = String::new();
        out.try_reserve_exact(BYTES)
            .map_err(|_| StaticStylesheetObservationError::OutputAllocation)?;
        out.push_str(if self.committed {
            "StaticStylesheetSet v2 availability=committed document=local[0]\n"
        } else {
            "StaticStylesheetSet v2 availability=no-document\n"
        });
        let (mut written, mut diagnostic_shown, mut rule_shown, mut row_truncated) = (0, 0, 0, 0);
        for entry in self.entries().take(ROWS) {
            let mut row = Row::new();
            let (mut row_diagnostics, mut row_rules) = (0, 0);
            let parse = entry.view.parse();
            let host = entry.view.host();
            row.record(format_args!(
                "sheet[{}] owner=local[{}] source={} state={} host={} title={} disabled={} alternate={} media={} parse={} decode={} runtime={} acquisition={} installation={} receipt={} participation={}\n",
                entry.order, entry.order, if entry.view.is_external() { "external" } else { "inline-unicode" },
                entry.state(), inline_type_label(host.inline_type()), title_label(host.title()), host.disabled(), host.alternate(),
                if entry.media_deferred() { "deferred" } else { "active" }, entry.completeness(), entry.decoding(), entry.runtime_label(),
                entry.acquisition_label(), installation_label(entry.view.installation_result()), receipt_label(entry.view.runtime_receipt()), entry.participation()
            ));
            let provenance = if !entry.view.is_external() {
                "current-document-base"
            } else if entry.view.resource_base().is_some() {
                "admitted-response-base"
            } else {
                "authored-request-only"
            };
            row.record(format_args!(
                "  provenance={provenance} encoding_source={} report={}\n",
                encoding_source_label(entry.decoding_decision()),
                report_label(entry.decode_report())
            ));
            if let Some(parse) = parse {
                row.record(format_args!(
                    "  diagnostics={} parser_diagnostics_omitted={} integrity={} limited={}\n",
                    parse.diagnostics.len(),
                    parse.diagnostics_omitted(),
                    parse.stats.had_invariant_violation,
                    parse.stats.hit_limit
                ));
                for diagnostic in parse
                    .diagnostics
                    .iter()
                    .take(DIAGNOSTICS - diagnostic_shown)
                {
                    if row.record(format_args!(
                        "  diagnostic={}/{}\n",
                        diagnostic.kind.stable_code(),
                        diagnostic.severity.snapshot_label()
                    )) {
                        row_diagnostics += 1;
                    }
                }
                for rule in &parse.stylesheet.rules {
                    if let css::Rule::At(rule) = rule
                        && rule_shown + row_rules < AT_RULES
                        && row.record(format_args!(
                            "  at-rule={}\n",
                            css::classify_at_rule(rule).stable_label()
                        ))
                    {
                        row_rules += 1;
                    }
                }
            }
            if out.len() + row.len > BYTES - FOOTER {
                break;
            }
            out.push_str(
                std::str::from_utf8(&row.bytes[..row.len]).expect("complete UTF-8 records"),
            );
            row_truncated += usize::from(row.truncated);
            diagnostic_shown += row_diagnostics;
            rule_shown += row_rules;
            written += 1;
        }
        let diagnostic_total: usize = self
            .entries()
            .filter_map(|e| e.view.parse())
            .map(|p| p.diagnostics.len())
            .sum();
        let rule_total: usize = self
            .entries()
            .filter_map(|e| e.view.parse())
            .map(|p| {
                p.stylesheet
                    .rules
                    .iter()
                    .filter(|r| matches!(r, css::Rule::At(_)))
                    .count()
            })
            .sum();
        writeln!(out, "omitted_rows={} truncated_rows={} omitted_retained_diagnostics={} omitted_at_rules={} discovery=available-source-only", self.entries().len()-written, row_truncated, diagnostic_total-diagnostic_shown, rule_total-rule_shown).expect("reserved footer");
        debug_assert!(out.len() <= BYTES);
        Ok(out)
    }
}
impl<'a> StaticStylesheetEntry<'a> {
    pub fn order(&self) -> usize {
        self.order
    }
    pub fn state(&self) -> &'static str {
        if self.view.is_not_requested() {
            "not-requested"
        } else if self.view.is_pending() {
            "pending"
        } else if self.view.has_loaded_parse() {
            "available"
        } else if self.view.is_failed() {
            "failed"
        } else {
            "aborted"
        }
    }
    pub fn completeness(&self) -> &'static str {
        match self.view.parse().map(css::StylesheetParse::completion) {
            None => "unavailable",
            Some(css::StylesheetParseCompletion::Complete) => "complete",
            Some(css::StylesheetParseCompletion::ResourceLimited) => "resource-limited",
            Some(css::StylesheetParseCompletion::InvariantViolation) => "invariant-violation",
        }
    }
    /// Persisted selection evidence says nothing about currently pending work.
    pub fn decoding(&self) -> &'static str {
        match self.view.decoding() {
            None if !self.view.is_external() => "unicode",
            None => "absent",
            Some(StylesheetDecodingOutcome::Selected(_)) => "selected",
            Some(StylesheetDecodingOutcome::Rejected(_)) => "unsupported-rejected",
            Some(StylesheetDecodingOutcome::Succeeded { .. }) => "finalized",
            Some(StylesheetDecodingOutcome::Failed { .. }) => "failed",
        }
    }
    fn runtime_label(&self) -> &'static str {
        match self.runtime_progress_inner() {
            None => "not-live",
            Some(CssRuntimeProgress::AwaitingOutcome) => "awaiting-outcome",
            Some(CssRuntimeProgress::ParsedAwaitingDone) => "parsed-awaiting-done",
            Some(CssRuntimeProgress::DecodeFailedAwaitingDone) => "decode-failed-awaiting-done",
        }
    }
    fn runtime_progress_inner(&self) -> Option<CssRuntimeProgress> {
        match &self.tab.stylesheet_loads.get(&self.view.slot_id())?.phase {
            StylesheetLoadPhase::AwaitingRuntime { progress, .. } => Some(*progress),
            _ => None,
        }
    }
    fn acquisition_label(&self) -> &'static str {
        use crate::stylesheet_fetch::StylesheetFetchOutcome;
        match self.view.fetch_result().map(|r| r.outcome()) {
            Some(StylesheetFetchOutcome::Ready) => "ready",
            Some(StylesheetFetchOutcome::Rejected(_)) => "rejected",
            Some(StylesheetFetchOutcome::Failed(_)) => "failed",
            None if self.tab.stylesheet_loads.contains_key(&self.view.slot_id()) => "pending",
            None => "absent",
        }
    }
    pub fn media_deferred(&self) -> bool {
        css::StylesheetConditionInput::from_optional_raw_media(self.view.media()).is_deferred()
    }
    pub fn participation(&self) -> &'static str {
        if !self.view.host().participates() {
            "host-inactive"
        } else if self.completeness() != "complete" {
            "unavailable-or-incomplete"
        } else if self.media_deferred() {
            "media-deferred"
        } else {
            "eligible-static-input"
        }
    }
    pub fn decoding_decision(&self) -> Option<bus::css_decode::CssDecodingDecision> {
        self.view
            .decoding()
            .map(StylesheetDecodingOutcome::decision)
    }
    pub fn decode_report(&self) -> Option<bus::css_decode::CssDecodeReport> {
        match self.view.decoding()? {
            StylesheetDecodingOutcome::Succeeded { report, .. }
            | StylesheetDecodingOutcome::Failed { report, .. } => Some(report),
            _ => None,
        }
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn slot_id(&self) -> core_types::StylesheetSlotId {
        self.view.slot_id()
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn owner(&self) -> html::internal::Id {
        self.view.owner()
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn css_coordinates(
        &self,
    ) -> Result<
        (css::StylesheetOrder, css::StylesheetSourceId),
        css::StylesheetCollectionInputBuildError,
    > {
        checked_coordinates(self.order, self.view.slot_id())
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn fetch(&self) -> Option<super::stylesheet_fetch_observation::StylesheetFetchEntry<'a>> {
        super::stylesheet_fetch_observation::StylesheetFetchObservation::entry(
            self.tab, self.order, self.view,
        )
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn decoding_outcome(&self) -> Option<crate::document_style::StylesheetDecodingOutcome> {
        self.view.decoding()
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn runtime_progress(&self) -> Option<CssRuntimeProgress> {
        self.runtime_progress_inner()
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn runtime_receipt(&self) -> Option<crate::document_style::CssRuntimeReceipt> {
        self.view.runtime_receipt()
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn installation(
        &self,
    ) -> Option<Result<(), crate::stylesheet_fetch::StylesheetInstallationFailure>> {
        self.view.installation_result()
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn host_eligibility(&self) -> crate::document_style::HostEligibility {
        self.view.host()
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn media(&self) -> css::StylesheetConditionInput<'a> {
        css::StylesheetConditionInput::from_optional_raw_media(self.view.media())
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn parse_availability(&self) -> Option<css::StylesheetParseCompletion> {
        self.view.parse().map(css::StylesheetParse::completion)
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn response_base(
        &self,
    ) -> Option<&'a Result<addressing::UrlParseOutcome, addressing::UrlFailure>> {
        self.view.resource_base()
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn parse(&self) -> Option<&'a css::StylesheetParse> {
        self.view.parse()
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub fn source(&self) -> Option<crate::document_style::StylesheetSourceObservation<'a>> {
        self.view.source_observation()
    }
}
fn inline_type_label(value: crate::document_style::InlineType) -> &'static str {
    use crate::document_style::InlineType::*;
    match value {
        NotApplicable => "external",
        Missing => "missing",
        Empty => "empty",
        Css => "text-css",
        Unsupported => "unsupported-type",
    }
}
fn title_label(value: crate::document_style::SheetTitle) -> &'static str {
    use crate::document_style::SheetTitle::*;
    match value {
        Missing => "missing",
        Empty => "empty",
        Named => "named",
    }
}
fn encoding_source_label(value: Option<bus::css_decode::CssDecodingDecision>) -> &'static str {
    use bus::css_decode::CssEncodingSource::*;
    match value.map(|d| d.selected_source()) {
        None => "absent",
        Some(Bom) => "bom",
        Some(Transport) => "transport",
        Some(CharsetDeclaration) => "charset-declaration",
        Some(Environment) => "environment",
        Some(Default) => "default",
    }
}
fn report_label(value: Option<bus::css_decode::CssDecodeReport>) -> &'static str {
    use bus::css_decode::CssDecodeReport::*;
    match value {
        None => "absent",
        Some(NotStarted) => "not-started",
        Some(Interrupted(_)) => "interrupted",
        Some(Finalized(_)) => "finalized",
    }
}
fn installation_label(
    value: Option<Result<(), crate::stylesheet_fetch::StylesheetInstallationFailure>>,
) -> &'static str {
    use crate::stylesheet_fetch::StylesheetInstallationFailure::*;
    match value {
        None => "absent",
        Some(Ok(())) => "installed",
        Some(Err(f)) => match f {
            HandoffAllocation => "handoff-allocation-failed",
            ChunkDeliveryUnavailable => "chunk-delivery-failed",
            DoneDeliveryUnavailable => "done-delivery-failed",
            RuntimeProvenanceMismatch => "runtime-provenance-mismatch",
            UnexpectedRuntimeEvent => "unexpected-runtime-event",
            Decode(_) => "decode-failed",
            Runtime(_) => "runtime-failed",
            InvalidDecodeEvidence(_) => "invalid-decode-evidence",
            ParserIntegrity => "parser-integrity-failed",
        },
    }
}
fn receipt_label(value: Option<crate::document_style::CssRuntimeReceipt>) -> &'static str {
    use crate::document_style::CssRuntimeReceipt::*;
    match value {
        None => "absent",
        Some(Done) => "done",
        Some(Failed(_)) => "runtime-failed",
    }
}
struct Row {
    bytes: [u8; 1024],
    len: usize,
    truncated: bool,
}
impl Row {
    fn new() -> Self {
        Self {
            bytes: [0; 1024],
            len: 0,
            truncated: false,
        }
    }
    fn record(&mut self, args: fmt::Arguments<'_>) -> bool {
        let checkpoint = self.len;
        if fmt::write(self, args).is_err() {
            self.len = checkpoint;
            self.truncated = true;
            false
        } else {
            true
        }
    }
}
impl Write for Row {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.truncated || self.len + text.len() > self.bytes.len() {
            self.truncated = true;
            return Err(fmt::Error);
        }
        self.bytes[self.len..self.len + text.len()].copy_from_slice(text.as_bytes());
        self.len += text.len();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aj8_correction_record_boundaries_and_checked_coordinates() {
        let mut row = Row::new();
        assert!(row.record(format_args!("complete-record\n")));
        assert!(!row.record(format_args!("diagnostic={:1024}\n", "x")));
        assert_eq!(&row.bytes[..row.len], b"complete-record\n");
        assert!(row.truncated);
        assert!(!row.record(format_args!("at-rule=import-deferred\n")));
        assert_eq!(&row.bytes[..row.len], b"complete-record\n");
        assert!(checked_coordinates(0, core_types::StylesheetSlotId(1)).is_ok());
        assert!(checked_coordinates(usize::MAX, core_types::StylesheetSlotId(1)).is_err());
        assert!(checked_coordinates(0, core_types::StylesheetSlotId(u64::MAX)).is_err());
    }
}
