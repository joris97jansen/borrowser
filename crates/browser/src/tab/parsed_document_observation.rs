//! Bounded, passive views of the active parsed document. No retained inventory or cache.
use super::{
    Tab,
    state::{DocumentEncodingDecision, DocumentParseStatus},
};
use crate::page::stylesheet_discovery::{
    StylesheetDiscovery, StylesheetDiscoveryError, StylesheetDiscoveryLimits,
    StylesheetDiscoverySnapshotError, debug::Snapshot as StylesheetSnapshot, discover_stylesheets,
};
use addressing::{BrowserUrl, Origin};
use core_types::{DomHandle, DomVersion, RequestId};
use html::{
    DocumentMode, ElementNamespace, Node,
    internal::{EventPosition, HtmlDecodeReport, HtmlParserDiagnosticReport},
};
use std::fmt::{self, Write};

const MAX_BYTES: usize = 8192;
const FOOTER_BYTES: usize = 512;
const MAX_NODES: usize = 128;
const MAX_DEPTH: usize = 32;
const MAX_DIAGNOSTICS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParsedDocumentPhase {
    Idle,
    AwaitingResponse,
    Parsing,
    Parsed,
    Failed,
    InvalidEvidence,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParsedTreeScope {
    Absent,
    Preview,
    Final,
    RetainedPreview,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParsedAddressScope {
    Absent,
    Prepared,
    Committed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticAvailability {
    NotStarted,
    AwaitingTerminal,
    Reported,
    Unavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParsedDocumentSnapshotError {
    OutputAllocation,
}

/// Exact fixture view. All ownership and identity authority remains in the borrowed Tab.
pub struct ParsedDocumentObservation<'tab> {
    tab: &'tab Tab,
}

impl<'tab> ParsedDocumentObservation<'tab> {
    /// The sole AJ6 committed-input admission. Lifecycle evidence remains separate.
    pub(super) fn committed_stylesheet_inputs(
        &self,
    ) -> Result<Option<(&'tab Node, &'tab BrowserUrl)>, StylesheetDiscoveryError> {
        match (
            self.document(),
            self.document_handle(),
            self.document_mode(),
            self.tab.page.addresses.as_ref(),
        ) {
            (None, None, None, _) => Ok(None),
            (Some(document), Some(_), Some(_), Some(addresses)) => {
                Ok(Some((document, addresses.active_base_url())))
            }
            _ => Err(StylesheetDiscoveryError::CommittedStateInconsistent),
        }
    }

    pub(super) fn for_fetch_snapshot(tab: &'tab Tab) -> Self {
        Self { tab }
    }
    pub fn stylesheet_fetches(
        &self,
    ) -> Result<
        Option<super::stylesheet_fetch_observation::StylesheetFetchObservation<'tab>>,
        super::stylesheet_fetch_observation::StylesheetFetchObservationError,
    > {
        self.committed_stylesheet_inputs().map(|input|input.map(|_|super::stylesheet_fetch_observation::StylesheetFetchObservation{tab:self.tab})).map_err(|_|super::stylesheet_fetch_observation::StylesheetFetchObservationError::CommittedStateInconsistent)
    }
    /// Observe authored candidates, never effective loader eligibility or work.
    pub fn stylesheet_discovery(
        &self,
    ) -> Result<Option<StylesheetDiscovery<'tab>>, StylesheetDiscoveryError> {
        self.committed_stylesheet_inputs()?
            .map(|(document, base)| {
                discover_stylesheets(document, base, &StylesheetDiscoveryLimits::default())
            })
            .transpose()
    }

    #[cfg(test)]
    pub(super) fn for_test(tab: &'tab Tab) -> Self {
        Self { tab }
    }
    pub fn phase(&self) -> ParsedDocumentPhase {
        let t = self.tab;
        if t.document_load.failure().is_some() {
            return ParsedDocumentPhase::Failed;
        }
        if let Some(body) = t.document_load.body_completion
            && (t.document_load.response.is_none()
                || !body.matches_route(core_types::ResourceKind::Html)
                || body.delivered_bytes() != t.document_load.bytes_received)
        {
            return ParsedDocumentPhase::InvalidEvidence;
        }
        let committed = t.dom_handle.is_some()
            && t.page.dom.is_some()
            && t.page.document_mode.is_some()
            && t.page.addresses.is_some();
        if t.document_load.response.is_some()
            && (t.dom_handle.is_some() != t.page.dom.is_some()
                || (t.page.dom.is_some() && !committed))
        {
            return ParsedDocumentPhase::InvalidEvidence;
        }
        match t.document_load.parse_status {
            DocumentParseStatus::Complete => {
                if t.document_load.has_finalized_diagnostics()
                    && t.document_load
                        .decode_report
                        .is_some_and(|r| r.eof_processed)
                    && t.document_load.body_completion.is_some()
                    && t.document_load.response.is_some()
                    && t.dom_handle.is_some()
                    && self.document().is_some()
                    && self.document_mode().is_some()
                    && t.page.addresses.is_some()
                {
                    ParsedDocumentPhase::Parsed
                } else {
                    ParsedDocumentPhase::InvalidEvidence
                }
            }
            DocumentParseStatus::Pending if t.active_document_request_id().is_some() => {
                ParsedDocumentPhase::Parsing
            }
            DocumentParseStatus::Pending if t.navigation_pending() => {
                ParsedDocumentPhase::AwaitingResponse
            }
            DocumentParseStatus::Pending => ParsedDocumentPhase::Idle,
            DocumentParseStatus::Failed(_) => ParsedDocumentPhase::Failed,
        }
    }
    pub fn navigation_pending(&self) -> bool {
        self.tab.navigation_pending()
    }
    pub fn tree_scope(&self) -> ParsedTreeScope {
        if self.document().is_none() {
            return ParsedTreeScope::Absent;
        }
        match self.phase() {
            ParsedDocumentPhase::Parsed => ParsedTreeScope::Final,
            ParsedDocumentPhase::Failed => ParsedTreeScope::RetainedPreview,
            _ => ParsedTreeScope::Preview,
        }
    }
    pub fn address_scope(&self) -> ParsedAddressScope {
        if self.tab.page.addresses.is_none() {
            ParsedAddressScope::Absent
        } else if self.document().is_some()
            && self.document_handle().is_some()
            && self.document_mode().is_some()
        {
            ParsedAddressScope::Committed
        } else {
            ParsedAddressScope::Prepared
        }
    }
    pub fn diagnostic_availability(&self) -> DiagnosticAvailability {
        if self.parser_diagnostics().is_some() {
            DiagnosticAvailability::Reported
        } else if self.request_id().is_none() {
            DiagnosticAvailability::NotStarted
        } else if self.tab.document_load.parse_status == DocumentParseStatus::Pending {
            DiagnosticAvailability::AwaitingTerminal
        } else {
            DiagnosticAvailability::Unavailable
        }
    }
    pub fn request_id(&self) -> Option<RequestId> {
        self.tab.active_document_request_id()
    }
    pub fn document_handle(&self) -> Option<DomHandle> {
        self.tab.dom_handle
    }
    #[cfg_attr(not(feature = "conformance-observation"), allow(dead_code))]
    pub fn document_version(&self) -> Option<DomVersion> {
        self.tab.dom_handle.map(|_| self.tab.dom_version)
    }
    pub fn body_bytes_received(&self) -> Option<usize> {
        self.request_id()
            .map(|_| self.tab.document_load.bytes_received)
    }
    pub fn normalized_input_bytes(&self) -> Option<usize> {
        self.parser_diagnostics()
            .map(|r| r.normalized_input_bytes())
    }
    pub fn encoding_decision(&self) -> Option<&'static str> {
        self.tab.document_load.encoding_decision.map(|v| match v {
            DocumentEncodingDecision::ResponseCharset => "utf-8-declared",
            DocumentEncodingDecision::MissingCharsetDefault => "utf-8-default-missing",
            DocumentEncodingDecision::EmptyCharsetDefault => "utf-8-default-empty",
        })
    }
    pub fn decoder_report(&self) -> Option<&'tab HtmlDecodeReport> {
        self.tab.document_load.decode_report.as_ref()
    }
    pub fn document(&self) -> Option<&'tab Node> {
        self.tab.page.dom.as_deref()
    }
    pub fn document_mode(&self) -> Option<DocumentMode> {
        self.tab.page.document_mode
    }
    pub fn parser_diagnostics(&self) -> Option<&'tab HtmlParserDiagnosticReport> {
        self.tab.document_load.diagnostics.as_ref()
    }
    pub fn document_url(&self) -> Option<&'tab BrowserUrl> {
        self.tab.page.addresses.as_ref().map(|a| a.document_url())
    }
    pub fn origin(&self) -> Option<&'tab Origin> {
        self.tab
            .page
            .addresses
            .as_ref()
            .map(|a| a.document_origin())
    }
    pub fn fallback_base(&self) -> Option<&'tab BrowserUrl> {
        self.tab
            .page
            .addresses
            .as_ref()
            .map(|a| a.fallback_base_url())
    }
    pub fn active_base(&self) -> Option<&'tab BrowserUrl> {
        self.tab
            .page
            .addresses
            .as_ref()
            .map(|a| a.active_base_url())
    }
}

impl Tab {
    /// Bounded redacted AJ6 regression text, not a public loading/assembly API.
    pub fn stylesheet_discovery_debug_snapshot(
        &self,
    ) -> Result<String, StylesheetDiscoverySnapshotError> {
        let snapshot = StylesheetSnapshot::new()?;
        let view = ParsedDocumentObservation { tab: self };
        let result = view.stylesheet_discovery();
        Ok(snapshot.finish(
            format_args!(
                "phase={:?} tree={:?} address={:?} navigation_pending={}",
                view.phase(),
                view.tree_scope(),
                view.address_scope(),
                view.navigation_pending()
            ),
            result,
        ))
    }

    #[cfg(feature = "conformance-observation")]
    pub fn observe_parsed_document(&self) -> ParsedDocumentObservation<'_> {
        ParsedDocumentObservation { tab: self }
    }

    pub fn parsed_document_debug_snapshot(&self) -> Result<String, ParsedDocumentSnapshotError> {
        let view = ParsedDocumentObservation { tab: self };
        let mut out = Snapshot::new()?;
        out.record(format_args!("ParsedDocument {{"));
        out.record(format_args!(
            "  phase: {:?} navigation_pending={}",
            view.phase(),
            view.navigation_pending()
        ));
        out.record(format_args!(
            "  scopes: tree={:?} address={:?} diagnostics=parser-session",
            view.tree_scope(),
            view.address_scope()
        ));
        out.record(format_args!(
            "  input: body_bytes_received={:?} normalized_available_bytes={:?} encoding={:?}",
            view.body_bytes_received(),
            view.normalized_input_bytes(),
            view.encoding_decision()
        ));
        out.record(format_args!(
            "  document_url: {}",
            scheme(view.document_url())
        ));
        out.record(format_args!(
            "  origin: {}",
            view.origin()
                .map_or("absent", |v| if v.is_opaque() { "opaque" } else { "tuple" })
        ));
        out.record(format_args!(
            "  fallback_base: {} source=document-url",
            scheme(view.fallback_base())
        ));
        out.record(format_args!(
            "  active_base: {} selection={}",
            scheme(view.active_base()),
            if self
                .page
                .addresses
                .as_ref()
                .is_some_and(|a| a.selected_key().is_some())
            {
                "selected-frozen"
            } else {
                "fallback"
            }
        ));
        out.record(format_args!("  document_mode: {:?}", view.document_mode()));
        out.record(format_args!(
            "  diagnostics: {:?}",
            view.diagnostic_availability()
        ));
        let mut shown = 0;
        if let Some(r) = view.parser_diagnostics() {
            out.record(format_args!(
                "  parser_input: {:?} capture={:?}",
                r.completion(),
                r.capture()
            ));
            out.record(format_args!(
                "  counts: semantics={} total={} retained={} dropped={}",
                if r.counts_exact() {
                    "exact"
                } else {
                    "lower-bounds"
                },
                r.total(),
                r.retained(),
                r.dropped()
            ));
            for event in r.events().iter().take(MAX_DIAGNOSTICS) {
                let position = match event.position {
                    EventPosition::Known(_) => "known-normalized",
                    EventPosition::Unavailable(_) => "unavailable-parser-position",
                };
                if !out.record(format_args!(
                    "  diagnostic[{}]: occurrence={} stage={:?} code={:?} position={position}",
                    shown, event.occurrence, event.stage, event.code
                )) {
                    break;
                }
                shown += 1;
            }
        }
        if let Some(r) = view.decoder_report() {
            out.record(format_args!(
                "  decoder: eof={} invalid={} incomplete_at_eof={}",
                r.eof_processed,
                r.replacements.invalid_sequences,
                r.replacements.incomplete_sequences_at_eof
            ));
        }
        let mut tree = TreeWork::default();
        if let Some(document) = view.document() {
            write_tree(document, &mut out, &mut tree);
        }
        let omitted = view
            .parser_diagnostics()
            .map_or(0, |r| r.retained() - shown);
        let complete = view.document().is_some()
            && !tree.nodes_truncated
            && !tree.depth_truncated
            && !out.bytes_truncated
            && !out.record_truncated;
        writeln!(
            out.output,
            "  tree_observation: visited={} complete={complete} node_limit={} depth_limit={}",
            tree.visited, tree.nodes_truncated, tree.depth_truncated
        )
        .expect("String write");
        writeln!(
            out.output,
            "  truncation: diagnostic_rows_omitted={omitted} byte_limit={} record_limit={}",
            out.bytes_truncated, out.record_truncated
        )
        .expect("String write");
        out.output
            .push_str("  full_page_assembly: not-certified\n}\n");
        debug_assert!(out.output.len() <= MAX_BYTES);
        #[cfg(test)]
        OBSERVATION_WORK.with(|w| w.set((tree.visited, tree.frames)));
        Ok(out.output)
    }
}

fn scheme(url: Option<&BrowserUrl>) -> &'static str {
    match url.map(|u| u.scheme()) {
        None => "absent",
        Some("http") => "http-redacted",
        Some("https") => "https-redacted",
        Some("file") => "file-redacted",
        Some(_) => "other-redacted",
    }
}

struct Row {
    bytes: [u8; 256],
    len: usize,
}
impl fmt::Write for Row {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if s.len() > self.bytes.len() - self.len {
            return Err(fmt::Error);
        }
        self.bytes[self.len..self.len + s.len()].copy_from_slice(s.as_bytes());
        self.len += s.len();
        Ok(())
    }
}
struct Snapshot {
    output: String,
    bytes_truncated: bool,
    record_truncated: bool,
}
impl Snapshot {
    fn new() -> Result<Self, ParsedDocumentSnapshotError> {
        let mut output = String::new();
        output
            .try_reserve_exact(MAX_BYTES)
            .map_err(|_| ParsedDocumentSnapshotError::OutputAllocation)?;
        Ok(Self {
            output,
            bytes_truncated: false,
            record_truncated: false,
        })
    }
    fn record(&mut self, args: fmt::Arguments<'_>) -> bool {
        let mut row = Row {
            bytes: [0; 256],
            len: 0,
        };
        if fmt::write(&mut row, args).is_err() {
            self.record_truncated = true;
            return false;
        }
        if self.output.len() + row.len + 1 > MAX_BYTES - FOOTER_BYTES {
            self.bytes_truncated = true;
            return false;
        }
        self.output
            .push_str(std::str::from_utf8(&row.bytes[..row.len]).expect("formatted UTF-8"));
        self.output.push('\n');
        true
    }
}

#[derive(Clone, Copy)]
enum Entry<'a> {
    Node(&'a Node),
    Fragment(&'a html::internal::DocumentFragmentNode),
}
impl<'a> Entry<'a> {
    fn children(self) -> &'a [Node] {
        match self {
            Self::Node(n) => n.children().unwrap_or(&[]),
            Self::Fragment(f) => html::internal::fragment_children(f),
        }
    }
    fn template(self) -> Option<&'a html::internal::DocumentFragmentNode> {
        match self {
            Self::Node(n) => html::internal::template_contents(n),
            Self::Fragment(_) => None,
        }
    }
    fn facts(self) -> (&'static str, &'static str, usize) {
        match self {
            Self::Fragment(_) => ("template-fragment", "none", 0),
            Self::Node(Node::Document { .. }) => ("document", "none", 0),
            Self::Node(Node::DocumentType { .. }) => ("doctype", "none", 0),
            Self::Node(Node::Text { .. }) => ("text", "none", 0),
            Self::Node(Node::Comment { .. }) => ("comment", "none", 0),
            Self::Node(Node::ProcessingInstruction { .. }) => ("processing-instruction", "none", 0),
            Self::Node(Node::Element { element }) => (
                "element",
                match element.namespace() {
                    ElementNamespace::Html => "html",
                    ElementNamespace::Svg => "svg",
                    ElementNamespace::MathMl => "mathml",
                },
                element.attributes().len(),
            ),
        }
    }
}
#[derive(Clone, Copy)]
struct Frame<'a> {
    entry: Entry<'a>,
    next_child: usize,
    association_done: bool,
    label: usize,
    depth: usize,
}
#[derive(Default)]
struct TreeWork {
    visited: usize,
    frames: usize,
    nodes_truncated: bool,
    depth_truncated: bool,
}
fn tree_row(
    entry: Entry<'_>,
    label: usize,
    parent: Option<usize>,
    depth: usize,
    edge: &str,
    out: &mut Snapshot,
) -> bool {
    let (kind, namespace, attributes) = entry.facts();
    out.record(format_args!("  tree[{label}]: parent={parent:?} edge={edge} depth={depth} kind={kind} namespace={namespace} children={} attributes={attributes}",entry.children().len()))
}
fn write_tree(document: &Node, out: &mut Snapshot, work: &mut TreeWork) {
    let root = Entry::Node(document);
    let mut stack = [None; MAX_DEPTH + 1];
    work.visited = 1;
    if !tree_row(root, 0, None, 0, "root", out) {
        return;
    }
    work.frames = 1;
    stack[0] = Some(Frame {
        entry: root,
        next_child: 0,
        association_done: false,
        label: 0,
        depth: 0,
    });
    let mut used = 1;
    while used != 0 {
        if work.visited == MAX_NODES {
            work.nodes_truncated = stack[..used].iter().flatten().any(|f| {
                (!f.association_done && f.entry.template().is_some())
                    || f.next_child < f.entry.children().len()
            });
            break;
        }
        let frame = stack[used - 1].as_mut().expect("live frame");
        if frame.depth == MAX_DEPTH {
            work.depth_truncated |=
                frame.entry.template().is_some() || !frame.entry.children().is_empty();
            used -= 1;
            continue;
        }
        let next = if !frame.association_done {
            frame.association_done = true;
            frame.entry.template().map(Entry::Fragment)
        } else {
            None
        };
        let (next, edge) = if let Some(next) = next {
            (next, "template-contents")
        } else if let Some(child) = frame.entry.children().get(frame.next_child) {
            frame.next_child += 1;
            (Entry::Node(child), "ordinary")
        } else {
            used -= 1;
            continue;
        };
        let parent = frame.label;
        let depth = frame.depth + 1;
        let label = work.visited;
        work.visited += 1;
        if !tree_row(next, label, Some(parent), depth, edge, out) {
            break;
        }
        stack[used] = Some(Frame {
            entry: next,
            next_child: 0,
            association_done: false,
            label,
            depth,
        });
        used += 1;
        work.frames = work.frames.max(used);
    }
}

#[cfg(test)]
thread_local! {pub(super) static OBSERVATION_WORK:std::cell::Cell<(usize,usize)>=const{std::cell::Cell::new((0,0))};}

#[cfg(test)]
pub(super) fn root_admission_work_for_test(document: &Node) -> (usize, usize) {
    let mut out = Snapshot::new().unwrap();
    out.output
        .extend(std::iter::repeat_n(' ', MAX_BYTES - FOOTER_BYTES));
    let mut work = TreeWork::default();
    write_tree(document, &mut out, &mut work);
    (work.visited, work.frames)
}
